# Resolution, propositionally

| | |
|---|---|
| Example | `crates/mrs-book-labs/examples/ch05_resolution.rs` |
| Library logic | `resolution_demo_ok` |
| Concepts | Binary resolution chain: `p`, `p => q ⊢ q` |

## Learning objectives

- Trace the unit-resolution refutation of `[p], [¬p, q], [¬q]` on paper.
- Connect each step to clauses the search loop generates.

## Runnable sample

```rust
{{#rustdoc_include ../../../crates/mrs-book-labs/examples/ch05_resolution.rs}}
```

```bash
nix develop -c cargo run -p mrs-book-labs --example ch05_resolution
```

## Exercises

1. Add the clause `[¬q, r]` and conjecture `r`; predict the refutation length.
2. Find `resolution` and `factoring` in `crates/mrs-calculus/src/` and note their inputs.
