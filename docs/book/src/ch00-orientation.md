# Orientation

## Learning objectives

- Install the toolchain and run `mrs` on `problems/socrates.p`.
- Read `% SZS status` lines and know where the pipeline lives.

## Try it

```bash
nix develop -c cargo run -- problems/socrates.p
```

Expected: a line starting with `% SZS status Theorem`.

## Pipeline map

`mrs-tptp` (parse) → `mrs-core` (terms/formulas) → `mrs-cnf`
(clausify) → `mrs-search` (given-clause loop) → `mrs-proof`/`mrs-szs`
(output). The rest of this book walks that pipeline left to right.

## Reproducibility note

`mrs` runs a parallel portfolio by default, so telemetry varies run to
run. Every lab example in this book uses a single strategy with one
worker so results are bit-reproducible.

Next: [First proof](ch01-first-proof.md).
