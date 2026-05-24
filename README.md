# mrs — Mechanical Reasoning System

An automated theorem prover written in Rust, targeting the [CASC](http://www.tptp.org/CASC/) competition.

Reads [TPTP](https://www.tptp.org/) problem files and outputs results in SZS/TSTP format using a superposition calculus with a given-clause loop and a strategy portfolio scheduler.

## Prerequisites

**Rust toolchain:** edition 2024, resolver 3 — requires stable ≥ 1.85.

```bash
rustup show          # check current version
rustup update stable # update if needed
```

## Building

```bash
cargo build           # debug build
cargo build --release # release (use for benchmarking)
cargo check           # fast type-check only
```

## Usage

```bash
cargo run -- problems/socrates.p
cargo run --release -- problems/pel1.p
```

Output lines begin with `% SZS status ...`. A refutation also produces a TSTP proof block.

**Example** (`problems/socrates.p`):

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

### TPTP `%include` directives

Set the `TPTP` environment variable to the root of a local TPTP library installation when problems use standard library includes:

```bash
TPTP=/path/to/TPTP cargo run --release -- problem.p
```

This is not needed for the bundled `problems/` files.

## Architecture

The pipeline for each problem:

1. **Parse** — `mrs-tptp-parser` converts TPTP text to an AST.
2. **Lower** — `src/lowering.rs` maps the AST to `mrs-core` types.
3. **Clausify** — `mrs-cnf` transforms formulas to CNF (NNF → Skolemization → definitional CNF). Conjectures are negated for refutation-based proving.
4. **Search** — `mrs-search` runs a given-clause loop. A strategy portfolio of 9 configurations is tried in sequence; the first refutation found wins.
5. **Output** — `mrs-szs` formats the SZS status line; `mrs-proof` extracts and formats the TSTP proof on refutation.

### Strategy portfolio

Nine strategies run serially, each with a fresh `SearchState`. Time is split across them proportionally from the 30-second budget:

| # | Selection | Literal selection | Ordering | Time share |
|---|-----------|-------------------|----------|------------|
| 1 | AgeWeight(5) | AllNegative | KBO | 15% |
| 2 | AgeWeight(10) | AllNegative | KBO | 10% |
| 3 | SmallestFirst | AllNegative | KBO | 10% |
| 4 | AgeWeight(5) | MaxNegative | KBO | 10% |
| 5 | AgeWeight(5) | All | KBO | 10% |
| 6 | FIFO | AllNegative | KBO | 10% |
| 7 | AgeWeight(5) | AllNegative | LPO | 15% |
| 8 | AgeWeight(10) | AllNegative | LPO | 10% |
| 9 | SmallestFirst | AllNegative | LPO | ~10% |

Resource limits per strategy: 50,000 clauses (`ResourceOut`) or the allocated time slice (`Timeout`).

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
    └── mrs-tptp-parser/ TPTP parser (crate name: mrs-tptp)
```

The root `Cargo.toml` is both `[workspace]` and `[package]`.

## Testing

```bash
cargo test --workspace                             # all tests
cargo test -p mrs-search                           # single crate
cargo test -p mrs-calculus resolution              # single test (substring match)
cargo test -p mrs-search -- --nocapture            # show stdout
```

Most tests are inline `#[cfg(test)]` modules. `mrs-tptp` also has integration tests under `crates/mrs-tptp-parser/tests/`. The `problems/` directory is for manual binary runs only.

Some integration tests in `mrs-search` and `mrs-calculus` run a full given-clause loop with real 5-second timeouts.

## Linting & formatting

```bash
cargo clippy --workspace
cargo fmt
cargo fmt --check  # CI-style check
```

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.
