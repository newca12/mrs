# Unification

| | |
|---|---|
| Example | `crates/mrs-book-labs/examples/ch03_unification.rs` |
| Library logic | `unify_demo_ok` |
| Concepts | Most general unifier, occurs check, variable hygiene |

## Learning objectives

- Compute the MGU of `f(X, a)` and `f(b, Y)` by hand, then confirm with `mrs_unify::unify`.
- Explain why `X` cannot unify with `f(X)`.

## Runnable sample

```rust
{{#rustdoc_include ../../../crates/mrs-book-labs/examples/ch03_unification.rs}}
```

```bash
nix develop -c cargo run -p mrs-book-labs --example ch03_unification
```

## Exercises

1. Extend the demo with a failing case (symbol clash) and match on `UnifyError`.
2. Read `crates/mrs-unify/src/robinson.rs` and identify where the occurs check happens.
