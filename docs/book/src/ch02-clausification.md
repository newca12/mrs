# Clausification: Pelletier and CNF

| | |
|---|---|
| Example | `crates/mrs-book-labs/examples/ch02_pelletier_cnf.rs` |
| Library logic | `pelletier_clause_count` |
| Concepts | NNF → Skolemization → CNF, definitional (Tseitin) CNF |
| Problem | `problems/pel1.p`: `(p => q) <=> (~q => ~p)` |

## Learning objectives

- Describe the clausification pipeline and why biconditionals blow up naive CNF.
- State what `mrs_cnf::clausify` returns and how clause sources tag proofs.

## Runnable sample

```rust
{{#rustdoc_include ../../../crates/mrs-book-labs/examples/ch02_pelletier_cnf.rs}}
```

```bash
nix develop -c cargo run -p mrs-book-labs --example ch02_pelletier_cnf
```

## Exercises

1. Predict the clause count for `pel2.p`, then check with `mrs --stats`.
2. Find which `mrs-cnf` module handles each pipeline stage (`nnf`, `skolem`, `cnf`, `definitional`).
