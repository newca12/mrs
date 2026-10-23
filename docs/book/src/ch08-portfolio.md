# Strategy portfolios

| | |
|---|---|
| Example | `crates/mrs-book-labs/examples/ch08_portfolio.rs` |
| Library logic | `portfolio_proves_socrates` |
| Concepts | 15-strategy portfolio, `StrategySchedule`, `run_schedule`, `MlOptions` |

## Learning objectives

- Explain why portfolios beat single strategies in aggregate.
- Distinguish solo diagnostic coverage from cooperative (shared-pool) coverage.

## Runnable sample

```rust
{{#rustdoc_include ../../../crates/mrs-book-labs/examples/ch08_portfolio.rs}}
```

```bash
nix develop -c cargo run -p mrs-book-labs --example ch08_portfolio
```

## Exercises

1. Compare `--workers 1 --strategy 1` vs the full schedule on `problems/group.p`.
2. Read [`reference/schedules`](../../reference/schedules.md) and explain what
   makes `casc_fne` differ from `casc_feq`.
