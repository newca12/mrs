## Plan: Rust TPTP Parser with Winnow

Implement a full TPTP parser supporting all dialects (CNF, FOF, TFF, TXF, THF, TCF, NXF/NHF) using winnow's combinator-based approach, matching the robustness of the scala-tptp-parser reference implementation.

### Steps

1. **Initialize project structure** — Create Cargo.toml with winnow 0.6+, num-bigint dependencies; set up module layout: `src/lib.rs`, `src/ast/`, `src/parser/`, `src/lexer.rs`

2. **Define AST types** — Create Rust enums/structs mirroring scala-tptp-parser's structure: `AnnotatedFormula` variants (THF/TFF/FOF/TCF/CNF/TPI), per-dialect `Statement`, `Formula`, `Term`, `Type`, `Connective`, `Quantifier` types, plus `Number`, `GeneralTerm`, and `Include`

3. **Implement lexer/token parsers** — Build winnow parsers for: `lower_word`, `upper_word` (variables), `single_quoted`, `distinct_object`, `dollar_word`, `dollar_dollar_word`, integers/rationals/reals, comments (`%` line, `/* */` block), whitespace handling

4. **Implement CNF/FOF parsers** — Start with simpler untyped dialects: `cnf_formula` (disjunctions of literals), `fof_formula` (full first-order with quantifiers `!`/`?`, connectives `<=>`, `=>`, `<=`, `<~>`, `~|`, `~&`, `|`, `&`, `~`), handle operator precedence via grammar structure

5. **Implement TFF/TCF parsers** — Add typed first-order: type declarations, typed variables, `$tType`, sort definitions, TF1 polymorphism with type quantifiers `!>`

6. **Implement THF parser** — Higher-order features: lambda `^`, application `@`, choice `@+`/description `@-`, type-level expressions, function/product/sum type constructors `>`/`*`/`+`

7. **Add extended dialects (TXF, NXF/NHF)** — TXF: FOOL features (conditionals `$ite`, let-expressions `$let`, tuples); NXF/NHF: non-classical operators `{#box}`, `{#dia}`, modal/temporal connectives

8. **Implement top-level and annotations** — `tptp_file` parser combining includes and annotated formulas, formula roles (`axiom`, `conjecture`, etc.), optional annotations with source/useful_info, pretty-printer for round-trip validation

### Further Considerations

1. **Operator precedence strategy?** — Use grammar-based precedence (separate rules per level) matching TPTP BNF.

2. **Error handling depth** — Use `cut_err` after dialect keywords (`fof(`, `thf(`) for committed parsing with context labels. Error messages should be detailled

3. **Testing approach** — Validate against TPTP problem library files. Include a test corpus in the repo.

4. **BigInt dependency** — TPTP numbers can be arbitrarily large. `num-bigint` is not acceptable, use bounded integers with overflow handling
