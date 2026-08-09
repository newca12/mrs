# mrs-tptp

A robust, zero-copy TPTP (Thousands of Problems for Theorem Provers) parser for Rust.

## Overview

`mrs-tptp` provides a complete parser for the TPTP language, which is the standard input format for automated theorem provers. Built with [winnow](https://crates.io/crates/winnow) for efficient, zero-copy parsing — the AST borrows directly from the original input string.

### Supported Dialects

- **CNF** - Clause Normal Form
- **FOF** - First-Order Form
- **TFF** - Typed First-order Form (including TXF/FOOL extensions)
- **TCF** - Typed Clause Form
- **THF** - Typed Higher-order Form
- **NXF/NHF** - Non-classical extensions (modal, temporal, epistemic)

### TPTP Version Compatibility

`mrs-tptp` implements a single, high-performance, permissive parser supporting a superset of all TPTP language standards up to **TPTP v9.x.x** (including classical CNF/FOF, typed TFF/TCF/THF, TXF/FOOL extensions, and non-classical NXF/NHF). Because the TPTP BNF is designed to be backwards-compatible, older problem files from earlier TPTP releases (e.g. v3.x.x through v8.x.x) are parsed natively without any configuration.


## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
mrs-tptp = "0.1"
```

## Quick Start

```rust
use mrs_tptp::parse_tptp;

fn main() {
    let input = "fof(axiom1, axiom, ![X]: (human(X) => mortal(X))).";

    match parse_tptp(input) {
        Ok(problem) => {
            println!("Parsed {} formula(s)", problem.formulas.len());
        }
        Err(e) => eprintln!("Parse error: {}", e),
    }
}
```

## Usage Examples

### Parsing FOF (First-Order Form)

```rust
use mrs_tptp::parse_tptp;

let input = r#"
    fof(human_socrates, axiom, human(socrates)).
    fof(mortality, axiom, ![X]: (human(X) => mortal(X))).
    fof(goal, conjecture, mortal(socrates)).
"#;

let problem = parse_tptp(input).unwrap();

for formula in &problem.formulas {
    println!("Formula: {} (role: {:?})", formula.name(), formula.role());
}
```

### Parsing CNF (Clause Normal Form)

```rust
use mrs_tptp::parse_tptp;

let input = r#"
    cnf(clause1, axiom, p(X) | ~q(X, Y)).
    cnf(clause2, axiom, q(a, b)).
    cnf(goal, negated_conjecture, ~p(a)).
"#;

let problem = parse_tptp(input).unwrap();
assert_eq!(problem.formulas.len(), 3);
```

### Parsing TFF (Typed First-order Form)

```rust
use mrs_tptp::parse_tptp;

let input = r#"
    tff(human_type, type, human: $i > $o).
    tff(mortal_type, type, mortal: $i > $o).
    tff(socrates_type, type, socrates: $i).
    tff(axiom1, axiom, human(socrates)).
    tff(axiom2, axiom, ![X: $i]: (human(X) => mortal(X))).
"#;

let problem = parse_tptp(input).unwrap();
```

### Parsing THF (Typed Higher-order Form)

```rust
use mrs_tptp::parse_tptp;

let input = r#"
    thf(p_type, type, p: $i > $o).
    thf(q_type, type, q: ($i > $o) > $o).
    thf(ax1, axiom, q @ (^[X: $i]: p @ X)).
"#;

let problem = parse_tptp(input).unwrap();
```

### Handling Include Directives

```rust
use mrs_tptp::parse_tptp;

let input = r#"
    include('Axioms/SET001+0.ax').
    include('Axioms/SET001+1.ax', [subset_def, union_def]).
    fof(my_theorem, conjecture, subset(a, union(a, b))).
"#;

let problem = parse_tptp(input).unwrap();

for include in &problem.includes {
    println!("Include: {}", include.file_name);
    if let Some(selection) = &include.selection {
        println!("  Selection: {:?}", selection);
    }
}
```

### Inspecting Formula Structure

```rust
use mrs_tptp::{parse_tptp, AnnotatedFormula, FOFFormula};

let input = "fof(ax1, axiom, ![X]: (p(X) => q(X))).";
let problem = parse_tptp(input).unwrap();

if let AnnotatedFormula::FOF(fof) = &problem.formulas[0] {
    println!("Name: {}", fof.name.as_str());
    println!("Role: {:?}", fof.role);
    // Access the formula AST
    if let FOFFormula::Quantified { quantifier, variables, formula } = &*fof.formula {
        println!("Quantifier: {:?}", quantifier);
        println!("Variables: {:?}", variables);
    }
}
```

### Streaming Parsing with `TPTPIterator`

For large files, use `TPTPIterator` to process one input at a time without collecting everything into a `Vec`:

```rust
use mrs_tptp::{TPTPIterator, TPTPInput};

let input = "fof(ax1, axiom, p). fof(ax2, axiom, q). fof(goal, conjecture, r).";
let mut iter = TPTPIterator::new(input);

for item in &mut iter {
    match item.expect("parse error") {
        TPTPInput::Formula(f) => println!("Formula: {}", f.name()),
        TPTPInput::Include(inc) => println!("Include: {}", inc.file_name),
    }
}

// Check for unconsumed input after iteration
assert!(iter.remaining().is_empty());
```

### Display / Round-trip

All AST types implement `Display`, producing valid TPTP output that can be reparsed to an equivalent AST:

```rust
use mrs_tptp::parse_tptp;

let input = "fof(ax1, axiom, ![X]: (p(X) => q(X))).";
let problem = parse_tptp(input).unwrap();

for formula in &problem.formulas {
    println!("{}", formula); // prints valid TPTP
}
```

## Features

### `cancellation`

Enable cooperative cancellation of long-running parses. Useful for timeout support in multi-threaded contexts:

```toml
[dependencies]
mrs-tptp = { version = "0.1", features = ["cancellation"] }
```

```rust
#[cfg(feature = "cancellation")]
use mrs_tptp::{set_cancel_flag, clear_cancel_flag, is_cancelled};
use std::sync::atomic::AtomicBool;

let flag = AtomicBool::new(false);
mrs_tptp::set_cancel_flag(&flag);

// On another thread: flag.store(true, Ordering::Relaxed);

let result = mrs_tptp::parse_tptp(large_input);
mrs_tptp::clear_cancel_flag();
```

## AST Types

**Top-level types:**

| Type | Description |
|------|-------------|
| `TPTPProblem<'a>` | A complete parsed TPTP file (`includes`, `formulas`, `formula_comments`) |
| `AnnotatedFormula<'a>` | One annotated formula — `THF`, `TFF`, `FOF`, `TCF`, `CNF`, or `TPI` variant |
| `TPTPInput<'a>` | A single streamed item — `Formula` or `Include` |
| `FormulaRole` | Role of a formula: `Axiom`, `Conjecture`, `NegatedConjecture`, `Type`, `Definition`, `Lemma`, `Theorem`, … |
| `Include<'a>` | An `include(…)` directive with `file_name` and optional `selection` |
| `Comment<'a>` | A `% line` or `/* block */` comment |

**Dialect-specific formula types:**

| Type | Dialect |
|------|---------|
| `FOFFormula<'a>`, `FOFTerm<'a>` | First-order logic |
| `CNFFormula<'a>`, `CNFLiteral<'a>` | Clause normal form |
| `TFFFormula<'a>`, `TFFTerm<'a>`, `TFFType<'a>` | Typed first-order |
| `TCFFormula<'a>` | Typed clause form |
| `THFFormula<'a>`, `THFType<'a>` | Typed higher-order |

## Examples

The `examples/` directory contains runnable programs:

```sh
# Parse and display a TPTP file
cargo run --example parse_file

# Recursively parse all .p files in a folder (parallel, with optional timeout)
cargo run --example parse_folder -- /path/to/TPTP --timeout 5000 --threads 4

# Benchmark parse throughput
cargo run --release --example benchmark_parse -- DATASET/ITP212_2.p
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
