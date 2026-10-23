# Term orderings: KBO vs LPO

| | |
|---|---|
| Example | `crates/mrs-book-labs/examples/ch04_orderings.rs` |
| Library logic | `ordering_demo_ok` |
| Concepts | Reduction orderings, orientation of equalities, `SymbolConfig` |

## Learning objectives

- State what `TermOrdering::KBO` vs `TermOrdering::LPO` guarantees.
- Explain why superposition needs an ordering at all.

## Runnable sample

```rust
{{#rustdoc_include ../../../crates/mrs-book-labs/examples/ch04_orderings.rs}}
```

```bash
nix develop -c cargo run -p mrs-book-labs --example ch04_orderings
```

## Exercises

1. Build a custom `SymbolConfig` with skewed precedence and re-run the comparison.
2. Run `problems/eq_simple.p` with `--strategy 1` (KBO) vs `--strategy 7` (LPO) and compare.
