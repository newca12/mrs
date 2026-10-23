# Mechanical Reasoning with mrs

A hands-on introduction to automated theorem proving: from zero to
superposition, AVATAR, and strategy portfolios — demonstrated with the
`mrs` prover.

Every chapter maps to one runnable gallery example in
`crates/mrs-book-labs/examples/`. Run any chapter's code with:

```bash
nix develop -c cargo run -p mrs-book-labs --example ch01_socrates
```

All examples are deterministic (`--workers 1` equivalent: single
single-strategy search, no cross-strategy sharing) and finish in seconds.
