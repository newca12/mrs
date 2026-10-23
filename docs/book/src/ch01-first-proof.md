# First proof: Socrates

| | |
|---|---|
| Example | `crates/mrs-book-labs/examples/ch01_socrates.rs` |
| Library logic | `crates/mrs-book-labs/src/lib.rs` (`socrates_clauses`, `prove`) |
| Concepts | TPTP `axiom`/`conjecture`, refutation (`axioms ∧ ¬conjecture`), empty clause |
| Problem | `problems/socrates.p` |

## Learning objectives

- Parse a TPTP problem and count axioms vs conjectures.
- Explain why the prover negates the conjecture before searching.

## Runnable sample

```rust
{{#rustdoc_include ../../../crates/mrs-book-labs/examples/ch01_socrates.rs}}
```

Run it:

```bash
nix develop -c cargo run -p mrs-book-labs --example ch01_socrates
```

## Exercises

1. Change the conjecture to `mortal(plato)` and predict the SZS status before running.
2. Run the `mrs` binary on `problems/socrates.p` and compare its `% SZS status` line with the example output.
