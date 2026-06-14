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
| `casc_fne` / `casc_ueq` / `casc_epr` | one strategy per worker (scales with `--workers`); **not yet data-driven** | division-tuned portfolios; see §"CASC Hardware & --casc Decision Rule" for how to optimise |
| `fast` | 1 KBO `AgeWeight(5)+AllNegative` | Sub-second ATP queries (e.g. `mrs-proover` backend) |
| `mini` | 3-strategy compact portfolio | 1–5 s budgets |
| `ml` (alias `ml_feq`), `ml_fne`, `ml_ueq`, `ml_epr` | ML-guided variants | require `ml-guidance` build + `--ml-weights`; degrade to weight-based selection otherwise |

The `casc` portfolio runs strategies 1–9 (KBO/LPO baseline, ~88% of budget) and 10–15 (new heuristic strategies, ~12% combined).  A 16th diagnostic strategy is always present but gets `Duration::ZERO` in
normal runs; use `MRS_SINGLE_STRATEGY=16` to run it alone for the full budget.

Strategies 10–15 use the `ClauseWeightFn` and `sos_depth` fields of `SearchConfig`:
- **s10**: SOS (selection + inference level, `sos_depth=100`) + AgeWeight(12) + AllNegative + KBO
- **s11**: `ConjSymbolBoost` + AgeWeight(6) + AllNegative + KBO
- **s12**: `HornHeuristic` + AgeWeight(5) + AllNegative + KBO (no AVATAR, no weight cap)
- **s13**: `FunctionWeightPenalty` + SOS + AgeWeight(5) + AllNegative + KBO
- **s14**: `ConjSymbolBoost` + SmallestFirst + All + KBO (no AVATAR, weight cap 100)
- **s15**: `SymbolWeight` + AgeWeight(4) + AllNegative + KBO (no AVATAR, no weight cap)

The `AgeWeight(n)` ratio means: every n-th iteration picks by age (FIFO), all others by weight.
Higher n = more weight-biased; lower n = more age-inclusive (broader exploration).

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
- **LRS (Limited Resource Strategy):** every 100 given-clause iterations, the prover estimates the remaining iteration budget from `elapsed/iteration` and prunes the passive queue to that size (min 2000). This prevents memory explosion and teardown latency on hard problems.  Set `TRACE_LRS=1` to see per-prune log lines on stderr.
- **Refutation-based:** conjectures are negated before search. A problem with no `conjecture` role checks satisfiability (outputs `Unsatisfiable`/`Satisfiable`).
- **TSTP proof output** only on `Refutation`; other statuses produce only the SZS status line.

## CASC Hardware & `--casc` Decision Rule

> **This section is permanent policy.  Do not remove or weaken it.**

**CASC competition hardware is exactly 8 CPU cores.**  Every entry at CASC runs
with a wall-clock time limit (240 s for FEQ/FNE/UEQ, 120 s for EPS/EPU) on a
machine with 8 physical cores.  All portfolio design, strategy selection, and
time-budget arithmetic **must treat 8 as the canonical core count**.

### Goal

Maximize the number of CASC problems solved across all entered divisions
(FEQ, FNE, UEQ, EPS/EPU).  The competition `invoke.sh` already routes each
problem to the correct per-division schedule (`casc_feq/fne/ueq/epr`).
The question is whether those division schedules are optimal for 8 cores.

### Decision rule for implementing `--casc`

A dedicated `--casc` flag (hard-coded 8-strategy per-division portfolio,
possibly with in-binary division detection) is **only worth implementing if**
data from the greedy set-cover analysis shows a meaningful gap between the
current generic schedule and the data-driven optimal portfolio.

| Condition | Action |
|-----------|--------|
| `greedy_set_cover --division X run.csv 8` gives same coverage as `--workers 8 --schedule casc_X` | No `--casc` flag needed; update `casc_X` with the greedy-selected strategies |
| Greedy portfolio covers >5% more problems per division | Replace loop-generated `casc_X` with a fixed 8-strategy hand-crafted portfolio; still no `--casc` flag needed |
| Greedy portfolio covers materially more problems AND requires division auto-detection inside the binary (not just invoke.sh) | Implement `--casc` flag that auto-detects division from TPTP problem path and selects the matching optimal 8-strategy schedule |

### Workflow: Per-Division Portfolio Optimisation

**Step 1 — Generate per-strategy coverage data (run once per TPTP release):**

```bash
# Run every mrs strategy solo on one division (30 s per problem, 4 parallel jobs).
# Requires: cargo build --release
export TPTP=/path/to/TPTP-v9.x.x
./crates/mrs-bench/run_strategy_sweep.sh --divisions fne --time 30 --jobs 4 \
    --output results/sweep-fne-$(date +%Y%m%d)
```

This produces `run.csv` where each `system` column is `mrs-s01..mrs-s15`.

**Step 2 — Find optimal 8-strategy portfolio per division:**

```bash
./target/release/greedy_set_cover results/sweep-fne-*/run.csv 8 --division fne
./target/release/greedy_set_cover results/sweep-fne-*/run.csv 8 --division ueq
./target/release/greedy_set_cover results/sweep-fne-*/run.csv 8 --division eps
```

**Step 3 — Baseline comparison:**

```bash
# Run the current generic casc portfolio on the same problems:
./crates/mrs-bench/casc.sh --systems mrs --divisions fne --time 30 --jobs 4 \
    --output results/baseline-fne-$(date +%Y%m%d)
# Compare solved counts between baseline and greedy-selected portfolio.
```

**Step 4 — Act on the results:**

- If greedy FNE portfolio = strategies `[s3, s7, s11, s1, s12, s6, s2, s10]` (example),
  replace the `casc_fne` loop-generated body in `named.rs` with those 8 explicit
  `SearchConfig` entries.
- Use the strategy descriptions in `strategy.rs` as reference for what each Sn is.
- Only add `--casc` to the binary if required (see decision rule above).

### Current status

The `casc_fne`, `casc_ueq`, and `casc_epr` schedules currently generate strategies
via modular arithmetic (loop over worker index).  They are **not data-driven**.
Run the workflow above to get data-driven portfolios before the next CASC entry.


## Testing

- Most tests are `#[cfg(test)]` inline modules — no separate test directories, no fixtures.
- Exception: `mrs-tptp` has integration tests under `crates/mrs-tptp/tests/` with fixture files in `tests/resources/`.
- Some `mrs-search` and `mrs-calculus` tests run the full given-clause loop with real `Duration::from_secs(5)` timeouts.
- `problems/` is for manual binary runs only, not `cargo test`.

## Runtime env var

`TPTP=/path/to/TPTP` — only needed at runtime when problems use `%include` pointing to the standard TPTP library. The benchmark harness (`crates/mrs-bench/systems/mrs/invoke.sh`) sets this automatically to `crates/mrs-bench/problems/casc-30`, so it is not required for normal benchmark runs. Only set it manually when running the binary directly on problems that use `%include`.
