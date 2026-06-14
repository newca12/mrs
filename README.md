# mrs — Mechanical Reasoning System

[![Crates.io](https://img.shields.io/crates/v/mrs.svg)](https://crates.io/crates/mrs)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

An automated theorem prover written in Rust, targeting the [CASC](http://www.tptp.org/CASC/) competition.

Reads [TPTP](https://www.tptp.org/) problem files and outputs results in [SZS/TSTP](https://tptp.org/Seminars/TPTPContentAndStandards/SZSPresentationSlides.pdf) format. It employs a **parallel strategy portfolio scheduler** running a **superposition calculus** within a **given-clause loop**, augmented by **AVATAR** (using CaDiCaL) for advanced clause splitting and **cross-strategy clause sharing**.

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
mrs [--time <seconds>] [--workers <N>] [--schedule <name>] <file.p>
```

| Option | Default | Description |
|--------|---------|-------------|
| `--time <n>` | `30` | Wall-clock time limit in seconds |
| `--workers <N>` | all cores | Maximum number of parallel search threads |
| `--schedule <name>` | `casc` | Strategy schedule to run. Built-ins: `casc` (the default CASC portfolio; aliases `default`, `casc_feq`), `casc_fne`/`casc_ueq`/`casc_epr` (division-tuned portfolios, one strategy per worker), `fast` (single KBO strategy for short budgets), `mini` (3-strategy compact portfolio), and `ml*` variants for ML-guided selection (require an `ml-guidance` build and `--ml-weights`). |
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
4. **Search** — `mrs-search` runs a given-clause loop with a strategy portfolio of 11 configurations tried in parallel; the first refutation found wins.
5. **Output** — `mrs-szs` formats the SZS status line; `mrs-proof` extracts and formats the TSTP proof on refutation.

### Strategy portfolio

15 active strategies run in parallel, each with a fresh search state but sharing a pool of globally discovered unit equalities. Time is distributed from the total budget to bound execution:

| # | Selection | Weight fn | Literal selection | Ordering | Time share | Notes |
|---|-----------|-----------|-------------------|----------|------------|-------|
| 1 | AgeWeight(3) | Standard | AllNegative | KBO | 14% | balanced exploration |
| 2 | SmallestFirst | Standard | AllNegative | KBO | 10% | no weight limit + no AVATAR (deep chain proofs) |
| 3 | SmallestFirst | Standard | AllNegative | KBO | 10% | pure best-first |
| 4 | AgeWeight(8) | Standard | MaxNegativeOrMaxPositive | KBO | 9% | aggressive selection |
| 5 | AgeWeight(5) | Standard | All | KBO | 9% | unrestricted literal selection |
| 6 | AgeWeight(10) | Standard | All | KBO | 10% | no AVATAR (FNE/definitional CNF) |
| 7 | AgeWeight(3) | Standard | AllNegative | LPO | 14% | LPO balanced exploration |
| 8 | GoalDirected(10) | Standard | AllNegative | LPO | 9% | LPO goal-directed |
| 9 | SmallestFirst | Standard | AllNegative | LPO | 9% | LPO best-first |
| 10 | AgeWeight(12) | Standard | AllNegative | KBO | 5% | SOS (sos_depth=100) + KBO |
| 11 | AgeWeight(6) | ConjSymbolBoost | AllNegative | KBO | 5% | goal-symbol boosted weight |
| 12 | AgeWeight(5) | HornHeuristic | AllNegative | KBO | 3% | Horn-preferred weight, no AVATAR |
| 13 | AgeWeight(5) | FunctionWeightPenalty | AllNegative | KBO | 2% | SOS + quadratic depth weight |
| 14 | SmallestFirst | ConjSymbolBoost | All | KBO | 2% | FEQ: goal-symbol + All selection |
| 15 | AgeWeight(4) | SymbolWeight | AllNegative | KBO | ~1% | precedence-based symbol weight |

Each strategy runs until its time slice expires or the search space is exhausted.
LRS (Limited Resource Strategy) periodically prunes the passive queue to stay within the time budget.

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
