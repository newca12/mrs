# AGENTS.md

Quick-start context for AI agents working in this repo.

## What this is

`mrs` is an automated theorem prover in Rust targeting the CASC competition. It reads **TPTP** problem files and outputs **SZS/TSTP**-formatted results. It employs a **parallel strategy portfolio scheduler** running a **superposition calculus** within a **given-clause loop**, augmented by **AVATAR** (using CaDiCaL) for advanced clause splitting and **cross-strategy clause sharing**.

## mrs-tptp

Zero-copy TPTP parser built with [winnow](https://crates.io/crates/winnow). Lives at `crates/mrs-tptp/`; crate name `mrs-tptp`. The AST borrows `&str` slices directly from the input — no per-token allocation. Single library crate, edition 2024.

**Supported dialects:** CNF, FOF, TFF, TCF, THF, TXF, NXF/NHF.

**Feature flags** (both off by default; `mrs` uses neither):

| Flag | Effect |
|------|--------|
| `cancellation` | Cooperative parse cancellation via `set_cancel_flag` / `clear_cancel_flag` |
| `owned` | `OwnedTPTPProblem` / `parse_tptp_file` — owns its data, no lifetime parameter |

**Key public API used by `mrs`:**

| Symbol | Description |
|--------|-------------|
| `parse_tptp(input: &str)` | Parse a full problem into `TPTPProblem<'_>` |
| `TPTPIterator::new(input)` | Streaming iterator, one `TPTPInput` per item |
| `TPTPInput::{Formula, Include}` | Variants yielded by the iterator |
| `AnnotatedFormula::{FOF, CNF, TFF, …}` | Enum over dialect-specific annotated formulas |
| `FormulaRole` | `Axiom`, `Conjecture`, `NegatedConjecture`, `Type`, `Definition`, … |
| `ParseError` | Carries byte offset; `.line()`, `.column()`, `.snippet()` helpers |

**Testing:**

```bash
cargo test -p mrs-tptp                    # unit + integration tests
cargo test -p mrs-tptp parser_tests       # integration tests only
cargo test -p mrs-tptp -- --nocapture     # see stdout

cargo run -p mrs-tptp --example parse_file
cargo run --release -p mrs-tptp --example parse_folder -- /path/to/TPTP --timeout 5000 --threads 4
```

Integration tests live in `crates/mrs-tptp/tests/`: `parser_tests.rs`, `non_classical_tests.rs`, `syn000_tests.rs`, plus `tests/resources/` fixtures.

## Toolchain

- Rust edition **2024**, resolver **3** — requires stable ≥ 1.85.
- Check version: `rustup show`. Update if needed: `rustup update stable`.

## Developer commands

```bash
cargo build                          # debug build
cargo build --release                # release (use for benchmarking)
cargo check                          # fast type-check, no output
cargo clippy --all                   # lint (always run before committing)
cargo fmt --all                      # format (always run before committing)
cargo fmt --all --check              # CI-style format check

cargo test --workspace               # all tests (use --workspace; bare cargo test only runs root crate)
cargo test -p mrs-search             # single crate
cargo test -p mrs-calculus resolution  # single test (substring match)
cargo test -p mrs-search -- --nocapture  # show stdout

# Run the binary on a TPTP problem file
cargo run -- problems/socrates.p
cargo run --release -- problems/pel1.p
# Expected output: lines starting with "% SZS status ..."

# Pick a non-default strategy schedule
cargo run --release -- --schedule fast problems/socrates.p
cargo run --release -- --list-schedules
```

## CLI flags

| Flag | Default | Description |
|------|---------|-------------|
| `--time <seconds>` | `30` | Wall-clock time limit |
| `--workers <N>` | all cores | Max parallel search threads |
| `--schedule <name>` | `casc` | Strategy schedule; see registry below |
| `--list-schedules` | — | Print known schedule names and exit |
| `--fast` | — | Deprecated alias for `--schedule fast` |
| `--log-ml-data <dir>` | — | Write labeled clause traces after a refutation; **needs `ml` feature build** to actually log |
| `--ml-log-csv` | — | Trace format: CSV instead of wincode |
| `--ml-weights <file>` | — | Load Burn model weights for ML-guided selection; **needs `ml-guidance`**, defaults schedule to `ml` |
| `--quiet` | — | Suppress non-SZS stderr; **requires `proover` feature** |
| `-` (positional) | — | Read TPTP from stdin; **requires `proover` feature** |

Named schedules live in `mrs_search::strategy::named` (`crates/mrs-search/src/strategy/named.rs`):

| Name | Strategies | Use case |
|------|------------|----------|
| `casc` (aliases `default`, `casc_feq`) | 16-strategy portfolio (15 active + 1 diagnostic) | CASC competition; default behavior |
| `casc_fne` / `casc_ueq` / `casc_epr` | one strategy per worker (scales with `--workers`) | division-tuned static portfolios |
| `fast` | 1 KBO `AgeWeight(5)+AllNegative` | Sub-second ATP queries (e.g. `mrs-proover` backend) |
| `mini` | 3-strategy compact portfolio | 1–5 s budgets |
| `ml` (alias `ml_feq`), `ml_fne`, `ml_ueq`, `ml_epr` | ML-guided variants | require `ml-guidance` build + `--ml-weights`; degrade to weight-based selection otherwise |

The `casc` portfolio runs strategies 1–9 (KBO/LPO baseline, ~88% of budget) and 10–15 (new heuristic strategies, ~12% combined).  A 16th diagnostic strategy is always present but gets `Duration::ZERO` in
normal runs; use `MRS_SINGLE_STRATEGY=16` to run it alone for the full budget.

Strategies 10–15 use the `ClauseWeightFn` and `sos_depth` fields of `SearchConfig`:
- **s10**: SOS (selection + inference level, `sos_depth=100`) + AgeWeight(5) + AllNegative + KBO
- **s11**: `ConjSymbolBoost` + AgeWeight(5) + AllNegative + KBO
- **s12**: `HornHeuristic` + AgeWeight(5) + AllNegative + KBO (no AVATAR, no weight cap)
- **s13**: `FunctionWeightPenalty` + SOS + AgeWeight(5) + AllNegative + KBO
- **s14**: `ConjSymbolBoost` + SmallestFirst + All + KBO (no AVATAR, weight cap 100)
- **s15**: `SymbolWeight` + AgeWeight(5) + AllNegative + KBO (no AVATAR, no weight cap)

To add a new schedule: implement a constructor in `strategy::named`, then add its name to `ALL` and the `by_name` match. `default_schedule()` must stay synonymous with `casc` so unflagged CASC runs are unaffected.

## Root crate features

| Feature | Off-by-default | Effect |
|---------|----------------|--------|
| `proover` | yes | Enables `--quiet` and stdin (`-`); used by `mrs-proover`'s in-process `MrsAtp` backend. Build with `cargo build --release --features proover --bin mrs`. |
| `ml` | yes | Enables ML trace logging (`--log-ml-data`); pulls `mrs-core/ml` + `mrs-search/ml-guidance` (Burn, wincode). Used by `crates/mrs-bench/collect_ml_data.sh`. |
| `ml-guidance` | yes | Same flags as `ml`; enables in-process inference with `--ml-weights`. |

`--schedule`, `--list-schedules`, `--workers`, and `--fast` are **unconditional** — they work in any build. The `--log-ml-data`/`--ml-weights` flags parse in any build but are no-ops (with a warning for `--ml-weights`) without the `ml`/`ml-guidance` features.

## Workspace layout

```
mrs/                  ← workspace root AND the binary crate (src/main.rs)
├── src/
│   ├── main.rs       ← CLI entrypoint; orchestrates the full pipeline
│   ├── lowering.rs   ← TPTP AST → mrs-core types
│   └── include.rs    ← resolves TPTP %include directives
├── crates/
│   ├── mrs-core/     ← Term, Formula, Clause, Literal, Substitution, SymbolTable
│   ├── mrs-szs/      ← SZS status enum + formatting
│   ├── mrs-cnf/      ← clausification: NNF, Skolemization, definitional CNF
│   ├── mrs-unify/    ← Robinson unification + matching
│   ├── mrs-calculus/ ← inference rules, KBO/LPO ordering, literal selection
│   ├── mrs-index/    ← discrimination tree indexing (indirect dep via mrs-search)
│   ├── mrs-proof/    ← proof extraction + TSTP output
│   ├── mrs-search/   ← given-clause loop, clause weighting, strategy scheduler
│   ├── mrs-tptp/     ← TPTP parser
│   ├── mrs-proover/  ← TSTP proof verifier (ProoVer 2026 entry); see crates/mrs-proover/README.md
│   ├── mrs-train/    ← offline GPU training for ML-guided clause selection (Burn); see crates/mrs-train/README.md
│   └── mrs-bench/    ← CASC benchmark harness (casc.sh, setup.sh) + bench_report and categorize_tptp binaries
└── problems/         ← curated TPTP .p files for manual testing (not wired into cargo test)
```

The root `Cargo.toml` is both `[workspace]` and `[package]` — valid but unusual.

## Architecture notes

- **Strategy portfolio:** 15 active strategies run **in parallel**, sharing a pool of derived unit equalities. A 16th diagnostic strategy (`MRS_SINGLE_STRATEGY=16`) gets `Duration::ZERO` in normal runs.
- **Default time budget:** 30 seconds; overridable with `--time <seconds>`.
- **`max_clauses`:** 50,000 per strategy. Hitting this gives `ResourceOut`, not `Timeout`.
- **Refutation-based:** conjectures are negated before search. A problem with no `conjecture` role checks satisfiability (outputs `Unsatisfiable`/`Satisfiable`).
- **TSTP proof output** only on `Refutation`; other statuses produce only the SZS status line.

## Testing

- Most tests are `#[cfg(test)]` inline modules — no separate test directories, no fixtures.
- Exception: `mrs-tptp` has integration tests under `crates/mrs-tptp/tests/` with fixture files in `tests/resources/`.
- Some `mrs-search` and `mrs-calculus` tests run the full given-clause loop with real `Duration::from_secs(5)` timeouts.
- `problems/` is for manual binary runs only, not `cargo test`.

## Runtime env var

`TPTP=/path/to/TPTP` — only needed at runtime when problems use `%include` pointing to the standard TPTP library. The benchmark harness (`crates/mrs-bench/systems/mrs/invoke.sh`) sets this automatically to `crates/mrs-bench/problems/casc-30`, so it is not required for normal benchmark runs. Only set it manually when running the binary directly on problems that use `%include`.
