# Building the book and running the labs

```bash
# Install mdBook once
cargo install mdbook

# Serve the book locally
mdbook serve docs/book --open

# Run one lab
nix develop -c cargo run -p mrs-book-labs --example ch01_socrates

# Run all lab tests (fast, deterministic, single-worker)
nix develop -c cargo test -p mrs-book-labs
```

Chapter → example mapping:

| Chapter | Example |
|---|---|
| ch01 First proof | `ch01_socrates` |
| ch02 Clausification | `ch02_pelletier_cnf` |
| ch03 Unification | `ch03_unification` |
| ch04 Orderings | `ch04_orderings` |
| ch05 Resolution | `ch05_resolution` |
| ch06 Given-clause | `ch06_given_clause` |
| ch07 AVATAR | `ch07_avatar` |
| ch08 Portfolio | `ch08_portfolio` |
