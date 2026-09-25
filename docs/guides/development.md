# Development Guide

The repository uses Nix flakes and direnv. The raw shell used by an agent or
CI job does not necessarily load direnv, so wrap compilation, tests, and Cargo
commands in `nix develop -c`.

## Toolchain

```bash
nix develop -c rustc --version
nix develop -c cargo --version
```

The workspace currently pins Rust `1.98.1` through `Cargo.toml` and `flake.nix`.

## Build and test

```bash
nix develop -c cargo build
nix develop -c cargo build --release
nix develop -c cargo check
nix develop -c cargo clippy --all -- -D warnings
nix develop -c cargo fmt --all --check
nix develop -c cargo test --workspace
```

Focused examples:

```bash
nix develop -c cargo test -p mrs-tptp
nix develop -c cargo test -p mrs-search
nix develop -c cargo test -p mrs-calculus resolution
nix develop -c cargo test -p mrs-proover -- --nocapture
```

The full validation commands above are required before a stable checkpoint.
They do not replace semantic review or benchmark gates for soundness-sensitive
changes.

## Feature builds

```bash
nix develop -c cargo build --release --features proover --bin mrs
nix develop -c cargo build --release --features ml --bin mrs
nix develop -c cargo build --release --features ml-guidance --bin mrs
nix develop -c cargo build --release -p mrs-proover
nix develop -c cargo build --release -p mrs-train
```

## Repository checks

Before committing a documentation or source change, inspect:

```bash
```

Do not include benchmark databases, binaries, `.direnv` state, model outputs,
or machine-specific logs unless the artifact is intentionally part of a
reproducible corpus.
