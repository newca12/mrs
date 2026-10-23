# mrs-book-labs

Runnable gallery examples for the `mrs` book (*From Zero to Superposition*).

Each example maps to one book chapter in `docs/book/src/` and follows the
`gallery-rs` contract:

```bash
nix develop -c cargo run -p mrs-book-labs --example ch01_socrates
nix develop -c cargo test -p mrs-book-labs
```

All examples are deterministic: single strategy, one worker, small
built-in problems, no network, no `$TPTP` dependency. Shared logic lives
in `src/lib.rs`; `examples/` are thin runnable wrappers so the book can
include them verbatim.
