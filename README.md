# mrs — Mechanical Reasoning System

[![Crates.io](https://img.shields.io/crates/v/mrs.svg)](https://crates.io/crates/mrs)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

An automated theorem prover written in Rust, targeting the [CASC](http://www.tptp.org/CASC/) competition.

Reads [TPTP](https://www.tptp.org/) problem files and outputs results in [SZS/TSTP](https://tptp.org/Seminars/TPTPContentAndStandards/SZSPresentationSlides.pdf) format using a superposition calculus with a given-clause loop and a strategy portfolio scheduler.

## Install

The easiest way to get `mrs` is via [crates.io](https://crates.io/crates/mrs):

```bash
cargo install mrs
```

This requires a Rust toolchain ≥ 1.85 (`rustup update stable` if needed).

Pre-built binaries are not yet provided; see [Building from source](#building-from-source) if you prefer not to use `cargo install`.

## Basic usage

```bash
mrs problem.p
```

Output lines begin with `% SZS status ...`.  On a successful refutation a TSTP proof block is also printed.

**Example** — given `socrates.p`:

```tptp
fof(ax1, axiom, ![X]: (human(X) => mortal(X))).
fof(ax2, axiom, human(socrates)).
fof(goal, conjecture, mortal(socrates)).
```

```
% SZS status Theorem for socrates
% SZS output start Proof for socrates
...
% SZS output end Proof for socrates
```

## Options

```
mrs [--time <seconds>] [--schedule <name>] <file.p>
```

| Option | Default | Description |
|--------|---------|-------------|
| `--time <n>` | `30` | Wall-clock time limit in seconds |
| `--schedule <name>` | `casc` | Strategy schedule to run. Built-ins: `casc` (the default 9-strategy CASC portfolio), `fast` (single KBO strategy for short budgets), `mini` (3-strategy compact portfolio). |
| `--list-schedules` | — | Print known schedule names and exit |

## TPTP `%include` directives

Problems that reference the standard TPTP library via `%include` need the `TPTP` environment variable set to the root of a local TPTP installation:

```bash
TPTP=/path/to/TPTP-v9.x.x mrs problem.p
```

This is not needed for problems that are self-contained.

## Building from source

```bash
git clone https://github.com/newca12/mrs
cd mrs
cargo build --release      # binary at target/release/mrs
cargo test --workspace     # run all tests
```

## Architecture

The pipeline for each problem:

1. **Parse** — `mrs-tptp` converts TPTP text to a zero-copy AST.
2. **Lower** — `src/lowering.rs` maps the AST to `mrs-core` types.
3. **Clausify** — `mrs-cnf` transforms formulas to CNF (NNF → Skolemization → definitional CNF). Conjectures are negated for refutation-based proving.
4. **Search** — `mrs-search` runs a given-clause loop with a strategy portfolio of 9 configurations tried in sequence; the first refutation found wins.
5. **Output** — `mrs-szs` formats the SZS status line; `mrs-proof` extracts and formats the TSTP proof on refutation.

### Strategy portfolio

Nine strategies run serially, each with a fresh search state. Time is split proportionally from the total budget:

| # | Selection | Literal selection | Ordering | Time share |
|---|-----------|-------------------|----------|------------|
| 1 | AgeWeight(5) | AllNegative | KBO | 15% |
| 2 | GoalDirected(5) | AllNegative | KBO | 10% |
| 3 | SmallestFirst | AllNegative | KBO | 10% |
| 4 | AgeWeight(5) | MaxNegativeOrMaxPositive | KBO | 10% |
| 5 | AgeWeight(5) | All | KBO | 10% |
| 6 | FIFO | AllNegative | KBO | 10% |
| 7 | AgeWeight(5) | AllNegative | LPO | 15% |
| 8 | GoalDirected(10) | AllNegative | LPO | 10% |
| 9 | SmallestFirst | AllNegative | LPO | ~10% |

Each strategy runs until its time slice expires or the search space is exhausted.

### Workspace layout

```
mrs/
├── src/
│   ├── main.rs        CLI entrypoint; orchestrates the full pipeline
│   ├── lowering.rs    TPTP AST → mrs-core types
│   └── include.rs     resolves TPTP %include directives
└── crates/
    ├── mrs-core/      Term, Formula, Clause, Literal, Substitution, SymbolTable
    ├── mrs-szs/       SZS status enum + formatting
    ├── mrs-cnf/       clausification: NNF, Skolemization, definitional CNF
    ├── mrs-unify/     Robinson unification + matching
    ├── mrs-calculus/  inference rules (resolution, superposition, …), KBO/LPO, literal selection
    ├── mrs-index/     discrimination tree indexing
    ├── mrs-proof/     proof extraction + TSTP output
    ├── mrs-search/    given-clause loop, clause weighting, strategy scheduler
    └── mrs-tptp/      zero-copy TPTP parser
```

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.
