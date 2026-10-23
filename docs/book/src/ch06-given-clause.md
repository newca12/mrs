# The given-clause loop and selection

| | |
|---|---|
| Example | `crates/mrs-book-labs/examples/ch06_given_clause.rs` |
| Library logic | `selection_comparison_ok` |
| Concepts | Active/passive sets, `AgeWeight(n)` vs `SmallestFirst`, search stats |

## Learning objectives

- Describe one iteration of the Otter-style loop.
- Read `processed`/`generated` counters and explain what the age/weight ratio controls.

## Runnable sample

```rust
{{#rustdoc_include ../../../crates/mrs-book-labs/examples/ch06_given_clause.rs}}
```

```bash
nix develop -c cargo run -p mrs-book-labs --example ch06_given_clause
```

## Exercises

1. Change `AgeWeight(5)` to `Fifo` and compare `processed` counts.
2. Set `TRACE_LRS=1` on a binary run and identify a pruning line.
