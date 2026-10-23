# mrs-tptp Technical Documentation

This document provides exhaustive technical information about the `mrs-tptp` crate to facilitate contribution and maintenance.

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Module Structure](#module-structure)
3. [Abstract Syntax Tree (AST)](#abstract-syntax-tree-ast)
4. [Parser Implementation](#parser-implementation)
5. [Lexer](#lexer)
6. [TPTP Dialect Support](#tptp-dialect-support)
7. [Operator Precedence](#operator-precedence)
8. [Display / Pretty-Printing](#display--pretty-printing)
9. [Cancellation Support](#cancellation-support)
10. [Testing Strategy](#testing-strategy)
11. [Performance Considerations](#performance-considerations)
12. [Adding New Features](#adding-new-features)
13. [Common Pitfalls](#common-pitfalls)
14. [TPTP Grammar Reference](#tptp-grammar-reference)

---

## Architecture Overview

`mrs-tptp` is a zero-copy parser for the TPTP (Thousands of Problems for Theorem Provers) language. It uses the [winnow](https://crates.io/crates/winnow) parser combinator library for efficient, streaming parsing.

### Design Principles

1. **Zero-copy parsing**: The AST stores `&'a str` references into the original input string, avoiding allocations for identifiers and strings.

2. **Grammar-based precedence**: Operator precedence is handled through the grammar structure itself (separate parsing functions per precedence level), not through a Pratt parser.

3. **Committed parsing with cut_err**: After parsing dialect keywords (e.g., `fof(`, `thf(`), the parser commits using `cut_err` to provide better error messages.

4. **Round-trip fidelity**: The `Display` implementations produce valid TPTP that can be reparsed to an equivalent AST.

5. **Cooperative cancellation**: Long-running parses can be cancelled via a thread-local cancellation flag.

### Dependencies

```toml
[dependencies]
winnow = "0.7"          # Parser combinator library

[dev-dependencies]
pretty_assertions = "1.4"  # Better test diff output
rayon = "1.10"             # Parallel testing
```

---

## Module Structure

```
src/
├── lib.rs              # Public API exports
├── lexer.rs            # Low-level token parsers
├── ast.rs              # AST module aggregator
├── parser.rs           # Parser module aggregator
├── ast/
│   ├── common.rs       # Shared types (Name, Number, BinaryConnective, etc.)
│   ├── cnf.rs          # CNF AST types
│   ├── fof.rs          # FOF AST types
│   ├── tff.rs          # TFF AST types
│   ├── tcf.rs          # TCF AST types
│   ├── thf.rs          # THF AST types
│   └── display.rs      # Display implementations for all AST types
└── parser/
    ├── top.rs          # Top-level parser (parse_tptp, annotated formulas)
    ├── common.rs       # Shared parser utilities
    ├── cnf.rs          # CNF parser
    ├── fof.rs          # FOF parser
    ├── tff.rs          # TFF parser
    ├── tcf.rs          # TCF parser
    └── thf.rs          # THF parser
```

---

## Abstract Syntax Tree (AST)

### Core Types

#### `TPTPProblem<'a>`
The root type representing a parsed TPTP file:

```rust
pub struct TPTPProblem<'a> {
    pub includes: Vec<Include<'a>>,
    pub formulas: Vec<AnnotatedFormula<'a>>,
    pub formula_comments: HashMap<&'a str, Vec<Comment<'a>>>,
}
```

#### `AnnotatedFormula<'a>`
A discriminated union of all TPTP dialect formulas:

```rust
pub enum AnnotatedFormula<'a> {
    THF(Box<THFAnnotated<'a>>),
    TFF(Box<TFFAnnotated<'a>>),
    FOF(Box<FOFAnnotated<'a>>),
    TCF(Box<TCFAnnotated<'a>>),
    CNF(Box<CNFAnnotated<'a>>),
    TPI(Box<TPIAnnotated<'a>>),
}
```

Each variant stores its payload in a `Box`, keeping the top-level enum compact
when a problem mixes dialects.  Construct a variant with `Box::new`; borrowed
pattern matches continue to expose the annotated fields through dereferencing.

Each boxed payload contains:
- `name: Name<'a>` - Formula identifier
- `role: FormulaRole` - Semantic role (axiom, conjecture, etc.)
- `formula: XXXStatement<'a>` - The actual formula content
- `annotations: Option<Annotations<'a>>` - Optional source/useful_info

### Common Types (`ast/common.rs`)

| Type | Description |
|------|-------------|
| `Name<'a>` | Formula names: `Lower(&str)`, `SingleQuoted(&str)`, `Integer(&str)` |
| `Number` | `Integer(String)`, `Rational{num,den}`, `Real{mantissa,exp}` |
| `AtomicWord<'a>` | `Lower(&str)` or `SingleQuoted(&str)` |
| `DefinedWord<'a>` | `$name` (e.g., `$true`, `$i`) |
| `SystemWord<'a>` | `$$name` |
| `BinaryConnective` | `Iff`, `Impl`, `RevImpl`, `Xor`, `Nor`, `Nand`, `Or`, `And` |
| `Quantifier` | `Forall`, `Exists` |
| `FormulaRole` | `Axiom`, `Conjecture`, `Hypothesis`, `Type`, etc. |
| `GeneralTerm<'a>` | Terms used in annotations |

### Dialect-Specific AST

#### FOF (`ast/fof.rs`)
First-order logic with quantifiers and full connectives:

```rust
pub enum FOFFormula<'a> {
    Atomic(FOFAtomicFormula<'a>),
    Negation(Box<FOFFormula<'a>>),
    Quantified { quantifier, variables, formula },
    Binary { left, connective, right },
    Equality(FOFTerm<'a>, FOFTerm<'a>),
    Inequality(FOFTerm<'a>, FOFTerm<'a>),
    Parens(Box<FOFFormula<'a>>),
}
```

#### CNF (`ast/cnf.rs`)
Clause normal form - disjunctions of literals:

```rust
pub enum CNFFormula<'a> {
    Disjunction(Vec<CNFLiteral<'a>>),
    Parens(Box<CNFFormula<'a>>),
}

pub enum CNFLiteral<'a> {
    Positive(CNFAtomicFormula<'a>),
    Negative(CNFAtomicFormula<'a>),
    Equality(FOFTerm<'a>, FOFTerm<'a>),
    Inequality(FOFTerm<'a>, FOFTerm<'a>),
}
```

#### TFF (`ast/tff.rs`)
Typed first-order form with type declarations:

- `TFFType<'a>` - Type expressions (`$i`, `$o`, `$int`, function types `>`, etc.)
- `TFFVariable<'a>` - Variables with optional type annotations
- `TypeQuantifier` - TF1 polymorphism (`!>`, `?*`)
- TXF extensions: `$ite`, `$let`, tuples
- NXF extensions: Non-classical operators

#### THF (`ast/thf.rs`)
Typed higher-order form:

- `THFFormula<'a>` - Full lambda calculus with application (`@`)
- `THFType<'a>` - Higher-order types with `>`, `*`, `+`
- `THFQuantifier` - Extended quantifiers (`^`, `@+`, `@-`, `!>`, `?*`)
- `THFBinaryConnective` - Extended connectives including type operators
- TH1 polymorphism support
- NHF non-classical extensions

### Type Hierarchy

```
THF (most expressive)
 └── TFF (typed first-order)
      └── TF1 (polymorphic)
      └── TXF (FOOL extension)
      └── NXF (non-classical)
 └── FOF (untyped first-order)
      └── CNF (clause form)
 └── TCF (typed clause form)
```

---

## Parser Implementation

### winnow Basics

The parser uses winnow's combinator approach:

```rust
pub type PResult<O> = winnow::error::ModalResult<O, ContextError>;
```

Key winnow combinators used:
- `alt((a, b, c))` - Try alternatives in order
- `opt(p)` - Optional parsing
- `preceded((prefix, ws), body)` - Skip prefix
- `delimited(open, body, close)` - Bracketed content
- `separated(min.., item, sep)` - Separated lists
- `repeat(min.., item)` - Repetition
- `cut_err(p)` - Commit to this branch (no backtracking)
- `.context(StrContext::Label("name"))` - Error context

### Parser Structure

#### Top-Level (`parser/top.rs`)

```rust
pub fn parse_tptp(input: &str) -> Result<TPTPProblem<'_>, String>
```

The main loop:
```rust
fn tptp_file<'a>(input: &mut &'a str) -> PResult<TPTPProblem<'a>> {
    loop {
        check_cancel(input)?;  // Cooperative cancellation
        ws.parse_next(input)?;
        if input.is_empty() { break; }
        match tptp_input.parse_next(input) {
            Ok(item) => { /* add to problem */ }
            Err(Backtrack(_)) => break,
            Err(e) => return Err(e),
        }
    }
}
```

#### Formula Parsing Pattern

Each dialect follows this precedence structure:

```rust
// Lowest precedence
fn xxx_formula(input) -> XXXFormula {
    xxx_binary_formula(input)  // non-associative connectives
}

fn xxx_binary_formula(input) -> XXXFormula {
    let left = xxx_or_formula(input)?;
    // Try <=> => <= <~> ~| ~&
    opt((nonassoc_connective, xxx_or_formula))
}

fn xxx_or_formula(input) -> XXXFormula {
    let first = xxx_and_formula(input)?;
    let rest = repeat(0.., preceded((ws, '|', ws), xxx_and_formula));
    // Left-fold into disjunction
}

fn xxx_and_formula(input) -> XXXFormula {
    let first = xxx_unary_formula(input)?;
    let rest = repeat(0.., preceded((ws, '&', ws), xxx_unary_formula));
    // Left-fold into conjunction
}

fn xxx_unary_formula(input) -> XXXFormula {
    alt((
        preceded(('~', ws), xxx_unary_formula),  // Negation
        xxx_unit_formula,
    ))
}

// Highest precedence
fn xxx_unit_formula(input) -> XXXFormula {
    alt((
        xxx_quantified_formula,
        delimited(('(', ws), xxx_formula, (ws, ')')),  // Parenthesized
        xxx_atomic_formula,
    ))
}
```

### Error Handling

After dialect keywords, use `cut_err` to commit:

```rust
fn thf_annotated<'a>(input: &mut &'a str) -> PResult<THFAnnotated<'a>> {
    "thf".parse_next(input)?;
    // ... parse name, role ...
    let formula = cut_err(thf_statement)
        .context(StrContext::Label("THF formula"))
        .parse_next(input)?;
}
```

---

## Lexer

The lexer (`src/lexer.rs`) provides token-level parsers:

### Token Parsers

| Function | Pattern | Example |
|----------|---------|---------|
| `lower_word` | `[a-z][a-zA-Z0-9_']*` | `human`, `mortal_x1` |
| `upper_word` | `[A-Z][a-zA-Z0-9_']*` | `X`, `Var1` |
| `single_quoted` | `'...'` (with `\'` escapes) | `'quoted name'` |
| `distinct_object` | `"..."` (with `\"` escapes) | `"object"` |
| `dollar_word` | `$[a-z][a-z0-9_]*` | `$true`, `$i` |
| `number` | integers, rationals, reals | `42`, `3/4`, `3.14E10` |
| `ws` | whitespace + comments | ` `, `% comment`, `/* */` |

### Whitespace and Comments

```rust
pub fn ws(input: &mut &str) -> PResult<()> {
    repeat(0.., alt((
        multispace1.void(),
        line_comment.void(),   // % ... \n
        block_comment.void(),  // /* ... */
    ))).parse_next(input)
}
```

Block comments support nesting.

---

## TPTP Dialect Support

### Dialect Keywords

| Keyword | Dialect | Description |
|---------|---------|-------------|
| `cnf()` | CNF | Clause Normal Form |
| `fof()` | FOF | First-Order Form |
| `tff()` | TFF/TF0/TF1/TXF/NXF | Typed First-order Form |
| `tcf()` | TCF | Typed Clause Form |
| `thf()` | THF/TH0/TH1/NHF | Typed Higher-order Form |
| `tpi()` | TPI | TPTP Process Instruction |

### Dialect Extensions

#### TXF (FOOL)
First-Order with Formulas as Terms:
- `$ite(condition, then, else)` - Conditional
- `$let(definitions, body)` - Let expressions
- Tuples and parallel assignment

#### NXF/NHF (Non-Classical)
Modal, temporal, epistemic logic:
- `{#box}(F)`, `{#dia}(F)` - Box/Diamond operators
- `[.](F)`, `<.>(F)` - Short form modal operators
- `$modal == [...]` - Logic specification

---

## Operator Precedence

From lowest to highest:

| Level | Operators | Associativity |
|-------|-----------|---------------|
| 1 | `<=>`, `=>`, `<=`, `<~>`, `~|`, `~&` | Non-associative |
| 2 | `\|` (disjunction) | Left-associative |
| 3 | `&` (conjunction) | Left-associative |
| 4 | `~` (negation) | Right-associative (prefix) |
| 5 | Quantifiers `!`, `?`, `^` | Prefix |
| 6 | `@` (application, THF only) | Left-associative |
| 7 | Atomic formulas, parentheses | - |

---

## Display / Pretty-Printing

The `ast/display.rs` module implements `Display` for all AST types to produce valid TPTP output.

### Round-Trip Property

**Invariant**: For any valid TPTP input:
```rust
let ast1 = parse_tptp(input).unwrap();
let output = ast1.to_string();
let ast2 = parse_tptp(&output).unwrap();
// ast1 and ast2 should be semantically equivalent
```

### Implementation Guidelines

1. **Parentheses**: Only emit parentheses when stored in `Parens` variant
2. **Whitespace**: Use single spaces between tokens
3. **Connectives**: Use the standard TPTP syntax from `as_str()` methods
4. **Escaping**: Properly escape quotes in `single_quoted` and `distinct_object`

---

## Cancellation Support

For long-running parses (e.g., deeply nested formulas), cooperative cancellation is supported:

### API

```rust
// Set cancellation flag before parsing
use mrs_tptp::{set_cancel_flag, clear_cancel_flag, parse_tptp};
use std::sync::atomic::AtomicBool;

let cancel = AtomicBool::new(false);
set_cancel_flag(&cancel);

// In another thread, to cancel:
cancel.store(true, Ordering::Relaxed);

// Parsing will abort at the next check point
let result = parse_tptp(input);
clear_cancel_flag();
```

### Implementation

Thread-local storage holds a pointer to the cancellation flag:

```rust
thread_local! {
    static CANCEL_FLAG: Cell<Option<*const AtomicBool>> = const { Cell::new(None) };
}

pub fn check_cancel(_input: &mut &str) -> PResult<()> {
    if is_cancelled() {
        Err(winnow::error::ErrMode::Cut(ContextError::new()))
    } else {
        Ok(())
    }
}
```

Check points are placed:
- At the start of `tptp_file` loop
- At the start of `xxx_formula` functions
- Inside `repeat` loops for binary operators

---

## Testing Strategy

### Test Structure

```
tests/
├── parser_tests.rs       # Unit tests for basic parsing
├── non_classical_tests.rs # NXF/NHF specific tests
├── syn000_tests.rs       # SYN000 test suite
└── resources/
    ├── SYN000/           # TPTP SYN000 test files
    └── non-classical/    # Non-classical test files
```

### Test Categories

1. **Unit tests**: Test individual parsing functions
2. **Round-trip tests**: Parse → Display → Parse, compare
3. **TPTP corpus tests**: Parse files from TPTP library
4. **Error tests**: Verify proper error handling

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_thf_lambda

# Run with output
cargo test -- --nocapture

# Run parse_folder example for corpus testing
cargo run --release --example parse_folder /path/to/TPTP/Problems/ --timeout 500
```

### Adding Tests

```rust
#[test]
fn test_new_feature() {
    let input = "fof(test, axiom, new_syntax).";
    let result = parse_tptp(input);
    assert!(result.is_ok(), "Parse error: {:?}", result.err());
    
    // Verify round-trip
    let output = result.unwrap().to_string();
    let result2 = parse_tptp(&output);
    assert!(result2.is_ok());
}
```

---

## Performance Considerations

### Zero-Copy Parsing

The AST stores `&'a str` references, avoiding string allocations:

```rust
pub struct FOFAnnotated<'a> {
    pub name: Name<'a>,  // References input
    // ...
}
```

**Trade-off**: The AST cannot outlive the input string.

### Stack Usage

Deeply nested formulas can cause stack overflow. Mitigations:
- Use large stack size (64 MB) for parsing threads
- Cooperative cancellation to abort long-running parses

### Memory Efficiency

For large files:
- Formulas are processed one at a time in the main loop
- Consider streaming processing for very large corpora

---

## Adding New Features

### Adding a New AST Variant

1. **Define type** in `src/ast/xxx.rs`:
   ```rust
   pub enum MyNewNode<'a> {
       Variant1(...),
       Variant2(...),
   }
   ```

2. **Add Display impl** in `src/ast/display.rs`:
   ```rust
   impl<'a> Display for MyNewNode<'a> {
       fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
           // ...
       }
   }
   ```

3. **Add parser** in `src/parser/xxx.rs`:
   ```rust
   fn my_new_node<'a>(input: &mut &'a str) -> PResult<MyNewNode<'a>> {
       // ...
   }
   ```

4. **Wire into grammar**: Add to appropriate `alt()` in parent parser

5. **Add tests**: Both parsing and round-trip tests

### Adding a New Dialect

1. Create `src/ast/newdialect.rs` with AST types
2. Create `src/parser/newdialect.rs` with parser
3. Add `NewDialectAnnotated` to `AnnotatedFormula` enum
4. Add `newdialect_annotated` parser to `annotated_formula` in `top.rs`
5. Export from `src/ast.rs` and `src/parser.rs`

---

## Common Pitfalls

### 1. Backtracking vs Cut

```rust
// BAD: Backtracking after parsing keyword wastes work
"thf".parse_next(input)?;
let formula = thf_statement.parse_next(input)?;  // Can backtrack!

// GOOD: Commit after keyword
"thf".parse_next(input)?;
let formula = cut_err(thf_statement).parse_next(input)?;
```

### 2. Ambiguous Grammar

Some constructs are ambiguous:
- `<=` vs `<=>`: Need lookahead
- `=` vs `=>` vs `==`: Need negative lookahead
- Type vs Formula in THF: May need backtracking

```rust
// Handle <= vs <=>
preceded(("<=", winnow::combinator::not('>')), ...)
```

### 3. Whitespace Handling

Always consume whitespace:
```rust
let name = name.parse_next(input)?;
ws.parse_next(input)?;  // Don't forget!
','.parse_next(input)?;
```

### 4. Lifetime Management

AST references the input string:
```rust
// BAD: String dropped before AST used
let ast = {
    let input = read_file();
    parse_tptp(&input)  // Returns references into `input`
}?;  // `input` dropped here!
use_ast(&ast);  // Dangling references!

// GOOD: Keep input alive
let input = read_file();
let ast = parse_tptp(&input)?;
use_ast(&ast);
```

---

## TPTP Grammar Reference

The official TPTP grammar is defined at:
- http://tptp.org/TPTP/SyntaxBNF.html

Key grammar rules implemented:

```bnf
<TPTP_file>          ::= <TPTP_input>*
<TPTP_input>         ::= <annotated_formula> | <include>

<annotated_formula>  ::= <thf_annotated> | <tff_annotated> | <tcf_annotated> |
                         <fof_annotated> | <cnf_annotated> | <tpi_annotated>

<fof_annotated>      ::= fof(<name>,<formula_role>,<fof_formula><annotations>).
<fof_formula>        ::= <fof_binary_formula> | <fof_unitary_formula>
<fof_binary_formula> ::= <fof_binary_nonassoc> | <fof_binary_assoc>
<fof_binary_assoc>   ::= <fof_or_formula> | <fof_and_formula>
<fof_or_formula>     ::= <fof_unit_formula> <vline> <fof_unit_formula> |
                         <fof_or_formula> <vline> <fof_unit_formula>
<fof_unit_formula>   ::= <fof_quantified_formula> | <fof_unary_formula> |
                         <fof_atomic_formula> | (<fof_logic_formula>)

<thf_formula>        ::= <thf_logic_formula> | <thf_atom_typing> | <thf_subtype>
<thf_logic_formula>  ::= <thf_binary_formula> | <thf_unitary_formula> |
                         <thf_type_formula> | <thf_subtype>
```

For complete grammar, refer to the official TPTP documentation.

---

## Version History

- **0.1.0**: Initial release with full TPTP dialect support
  - CNF, FOF, TFF, TCF, THF parsers
  - TXF (FOOL) extensions
  - NXF/NHF non-classical extensions
  - Cooperative cancellation support

---

## Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass: `cargo test`
5. Ensure round-trip tests pass for affected dialects
6. Submit a pull request

### Code Style

- Use `rustfmt` for formatting
- Add doc comments for public items
- Use `StrContext::Label` for parser error context
- Follow existing naming conventions
