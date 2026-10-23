# AVATAR splitting

| | |
|---|---|
| Example | `crates/mrs-book-labs/examples/ch07_avatar.rs` |
| Library logic | `avatar_both_ok` |
| Concepts | Clause splitting, SAT delegation, `use_avatar` on/off |
| Problem shape | Unsatisfiable propositional split over `p, q` |

## Learning objectives

- Explain what AVATAR splits and what the SAT solver decides.
- State when disabling AVATAR helps (deep equational chains) vs hurts.

## Runnable sample

```rust
{{#rustdoc_include ../../../crates/mrs-book-labs/examples/ch07_avatar.rs}}
```

```bash
nix develop -c cargo run -p mrs-book-labs --example ch07_avatar
```

## Exercises

1. Run `problems/bool_hunt.p` with `--no-sharing` and compare outcomes.
2. Find `emit_avatar_trace` in `src/main.rs` and explain what the trace is for.
