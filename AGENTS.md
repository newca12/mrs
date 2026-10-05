# AGENTS.md

Quick-start context for AI agents working in this repo.

## Agent Workflow Constraint
Do not use the `explore` or `general` subagents (`subagent_type: "explore"` or
`subagent_type: "general"`) in this repository; they may not terminate. Use
bounded direct searches and file reads, or another explicitly requested tool,
instead.

## 1. NixOS WSL Development Environment (SOTA)
The host environment is **NixOS running inside Windows Subsystem for Linux (WSL)**.
- Traditional FHS assumptions do NOT apply. Files and libraries are versioned under `/nix/store/` instead of `/lib` or `/usr/include`.
- This project uses **Nix Flakes** (`flake.nix`) and **direnv** (`.envrc`) to declare its development dependencies (including Rust compiler stable 1.99.0, cargo, clippy, rustfmt, rust-analyzer, pkg-config, git, and cargo-nextest).

### Execution Rule (Critical)
Because your native agent `bash` or terminal execution tool starts in a raw shell that does not automatically load `direnv`, **you must wrap any compilation, testing, or development command in the Nix environment.**

- **DO NOT RUN:** `cargo check` or `cargo test --workspace` directly.
- **DO RUN:** Nest your commands inside `nix develop` or `direnv exec`:
  ```bash
  # Option A (Preferred):
  nix develop -c cargo check
  
  # Option B:
  direnv exec . cargo check
  ```

---

## 2. Commit and Verification Policy (Strictly Enforced)
To maintain the highest repository standards, any AI agent working in this workspace must adhere to the following rules:

1. **Pre-Commit Verification**: Every single code change and commit must be completely clean and fully validated. Before creating any commit, you must run and pass the following Nix-wrapped commands with zero errors or warnings:
   - **Check**: `nix develop -c cargo check`
   - **Lints**: `nix develop -c cargo clippy --all -- -D warnings`
   - **Format**: `nix develop -c cargo fmt --all --check`
   - **Tests**: `nix develop -c cargo test --workspace`

   These wrappers are local developer/CI verification commands. Do not add
   `nix develop` invocations to runtime or benchmark scripts: those scripts
   must work on ordinary Linux hosts that provide Cargo/Rust but do not install
   Nix. Script-triggered builds should invoke `cargo` directly; developers in
   this NixOS WSL workspace can source `.envrc`/use `direnv` before running them.

2. **Git Commits**: You are permitted to create Git commits autonomously to checkpoint stable stages of development. Ensure the commit messages conform to the repository's convention (e.g., `feat: ...`, `fix: ...`, `refactor: ...`).

3. **No Pushing**: You are **strictly forbidden from pushing** commits to the remote tracking branch or any remote repositories. Never run `git push`.

4. **Permission Model**: OpenCode permissions are set to `"allow"` inside `~/.config/opencode/opencode.json` to streamline the agent flow. You do not need to ask for permission before running wrapped bash commands.

---

## 3. What this is

`mrs` is an automated theorem prover in Rust targeting the CASC competition. It reads **TPTP** problem files and outputs **SZS/TSTP**-formatted results. It employs a **parallel strategy portfolio scheduler** running a **superposition calculus** within a **given-clause loop**, augmented by **AVATAR** (using CaDiCaL) for advanced clause splitting and **cross-strategy clause sharing**.

## 4. mrs-tptp

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
nix develop -c cargo test -p mrs-tptp                    # unit + integration tests
nix develop -c cargo test -p mrs-tptp parser_tests       # integration tests only
nix develop -c cargo test -p mrs-tptp -- --nocapture     # see stdout

nix develop -c cargo run -p mrs-tptp --example parse_file
nix develop -c cargo run --release -p mrs-tptp --example parse_folder -- /path/to/TPTP --timeout 5000 --threads 4
```

Integration tests live in `crates/mrs-tptp/tests/`: `parser_tests.rs`, `non_classical_tests.rs`, `syn000_tests.rs`, plus `tests/resources/` fixtures.

## 5. Toolchain

- Rust edition **2024**, resolver **3** — requires stable ≥ 1.85.
- Check version: `nix develop -c rustup show`. Update if needed: `nix develop -c rustup update stable`.

## 6. Developer commands

```bash
nix develop -c cargo build                          # debug build
nix develop -c cargo build --release                # release (use for benchmarking)
nix develop -c cargo check                          # fast type-check, no output
nix develop -c cargo clippy --all                   # lint (always run before committing)
nix develop -c cargo fmt --all                      # format (always run before committing)
nix develop -c cargo fmt --all --check              # CI-style format check

nix develop -c cargo test --workspace               # all tests
nix develop -c cargo test -p mrs-search             # single crate
nix develop -c cargo test -p mrs-calculus resolution  # single test (substring match)
nix develop -c cargo test -p mrs-search -- --nocapture  # show stdout

# Run the binary on a TPTP problem file
nix develop -c cargo run -- problems/socrates.p
nix develop -c cargo run --release -- problems/pel1.p
# Expected output: lines starting with "% SZS status ..."

# Pick a non-default strategy schedule
nix develop -c cargo run --release -- --schedule fast problems/socrates.p
nix develop -c cargo run --release -- --list-schedules
```

### Measure search speed on this host

`crates/mrs-bench/perf_probe.sh` runs a **fixed amount of work** (an
iteration-counted clause ceiling, not a wall clock) on a generated clause set,
for both a `native` and a `haswell` build, and appends the result to
`docs/results/perf/bank.tsv` with a dated report. Work is what makes two hosts
comparable: a "run it for 30 s and count clauses" number is not, because the
search is wall-clock sensitive. The whole probe is capped at 12 GiB. A CPU that
cannot run the `haswell` build (pre-Haswell) measures `native` only, and the
probe **refuses** a worker count the 12 GiB ceiling cannot hold — 32 workers
under the default `rlimit-as` — so a wide host needs an explicit `--workers`.

```bash
crates/mrs-bench/perf_probe.sh              # measure, bank, and report (~3 min)
crates/mrs-bench/perf_probe.sh --no-bank    # measure only
crates/mrs-bench/perf_probe.sh --help
```

It calls `cargo` directly rather than through `nix develop`, so `direnv exec .`
(or a loaded shell) must already provide the toolchain. Method and comparability
rules: `docs/results/perf/README.md`.

---

## 7. CLI flags

| Flag | Default | Description |
|------|---------|-------------|
| `--time <seconds>` | `30` | Wall-clock time limit |
| `--hardware <mode>` | `adaptive` | Hardware profile: `casc` (8 workers, 128 GB, never auto-adapted), `casc-sim` (that plus a CPU pin to 8 physical cores, an `RLIMIT_AS` ceiling, and a wall clock extended past the CASC limit), or `adaptive` (fit the host). `MRS_HARDWARE` sets it for a whole harness run. See §11 for what each is for and what its numbers mean. |
| `--sim-time-factor <mult\|0\|unbounded>` | `2` | casc-sim only: multiple of the CASC wall clock to keep searching after, or `0`/`unbounded` to drop the wall clock and let the resource ceilings decide. The run records where it stood at the CASC limit either way. |
| `--workers <N>` | per hardware mode | Max parallel search threads. Overrides the mode, so `casc --workers 2` really runs 2 workers. **Reproducibility note:** with `N>1` (the default), strategies run concurrently and may share a pool of derived unit equalities when `MRS_SHARED_POOL_INTERVAL` is positive; sharing is disabled by default (see "Architecture notes" below). With sharing enabled, per-run telemetry (`processed`/`generated`/`lrs_discarded`) and even the pass/fail outcome on borderline problems are not bit-reproducible — sibling-thread timing and CPU contention both feed into the wall-clock-sensitive LRS pruning heuristic. Use `--workers 1` for a fully deterministic, sequential single-strategy run (no clause-pool cross-talk, no contention) when diagnosing or reproducing a specific strategy's behavior. |
| `--strategy <N>` | — | Run exact base strategy `N` (1–15) from the selected CASC division schedule for the full budget; diagnostic solo coverage only. |
| `--portfolio <IDs>` | — | Run an explicit cooperative portfolio, e.g. `11,12,1,6,10,8,14,4`; one ID is required per worker. Set `MRS_SHARED_POOL_INTERVAL` to a positive value to enable shared equality exchange. |
| `--schedule <name>` | `casc` | Strategy schedule; see registry below |
| `--auto-schedule` | — | Rule-based division detection (EPR/UEQ/FNE/FEQ) picks the matching `casc_*` portfolio; an explicit `--schedule` wins. Replaces the retired ML schedule classifier (`--ml-schedule` is a deprecated alias). |
| `--list-schedules` | — | Print known schedule names and exit |
| `--fast` | — | Deprecated alias for `--schedule fast` |
| `--log-ml-data <dir>` | — | Write labeled clause traces after a refutation; **needs `ml` feature build** to actually log |
| `--ml-log-csv` | — | Trace format: CSV instead of wincode |
| `--ml-weights <file>` | — | Load Burn model weights for ML-guided selection; **needs `ml-guidance`**, defaults schedule to `ml` |
| `--quiet` | — | Suppress non-SZS stderr; **requires `proover` feature** |
| `-` (positional) | — | Read TPTP from stdin; **requires `proover` feature** |
| `--certify-ordered` | — | Diagnostic bounded function-free EPR ordered-resolution certification; use with `--workers 1 --strategy N`; unsupported inputs return `GaveUp` |
| `--pre-phase` | — | Run the pre-phase: measure the problem, print the routing decision and the evidence for it, and — **only when no `--schedule`/`--strategy`/`--portfolio` was named** — let the routed plan replace the portfolio. Experimental; see `docs/reports/prephase/` |
| `--pre-phase-only` | — | Analyse, print, and exit without searching. For auditing a routing decision |
| `--pre-phase-probe` | — | Also run the bounded search-behaviour probe and print its trajectory |
| `--list-rules` | — | Print the pre-phase routing table, one row per rule, with each row's conditions, order and a calibrated/prior flag |

Named schedules live in `mrs_search::strategy::named` (`crates/mrs-search/src/strategy/named.rs`):

| Name | Strategies | Use case |
|------|------------|----------|
| `casc` (aliases `default`, `casc_feq`) | 16-strategy portfolio (15 active + 1 diagnostic) | CASC competition; default behavior |
| `casc_fne` / `casc_feq` / `casc_ueq` / `casc_epr` / `casc_eps` / `casc_epu` / `casc_icu` | one strategy per worker (scales with `--workers`); candidate orders informed by solo sweeps and validated cooperatively | division-tuned portfolios; see §"CASC Hardware & --casc Decision Rule" for how to optimise |
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

---

## 8. Root crate features

| Feature | Off-by-default | Effect |
|---------|----------------|--------|
| `proover` | yes | Enables `--quiet` and stdin (`-`); used by `mrs-proover`'s in-process `MrsAtp` backend. Build with `nix develop -c cargo build --release --features proover --bin mrs`. |
| `ml` | yes | Enables ML trace logging (`--log-ml-data`); pulls `mrs-core/ml` + `mrs-search/ml-guidance` (Burn, wincode). Used by `crates/mrs-bench/collect_ml_data.sh`. |
| `ml-guidance` | yes | Same flags as `ml`; enables in-process inference with `--ml-weights`. |

`--schedule`, `--list-schedules`, `--workers`, and `--fast` are **unconditional** — they work in any build. The `--log-ml-data`/`--ml-weights` flags parse in any build but are no-ops (with a warning for `--ml-weights`) without the `ml`/`ml-guidance` features.

---

## 9. Workspace layout

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

---

## 10. Architecture notes

- **Strategy portfolio:** 15 active strategies run **in parallel**. When `MRS_SHARED_POOL_INTERVAL` is positive they share a pool of derived unit equalities; sharing is disabled by default. A 16th diagnostic strategy (`MRS_SINGLE_STRATEGY=16`) gets `Duration::ZERO` in normal runs.
- **Pre-phase:** off by default (`--pre-phase` / `MRS_PREPHASE=1` enables it). When enabled and no schedule was named explicitly, it replaces the portfolio with the routed plan. When a schedule *is* named it only reports the decision, so the feature's own A/B stays readable. See §12.
- **Default time budget:** 30 seconds; overridable with `--time <seconds>`.
- **LRS (Limited Resource Strategy):** every 100 given-clause iterations, the prover estimates the remaining iteration budget from `elapsed/iteration` and prunes the passive queue to that size (min 2000). This prevents memory explosion and teardown latency on hard problems. Set `TRACE_LRS=1` to see per-prune log lines on stderr.
- **Refutation-based:** conjectures are negated before search. A problem with no `conjecture` role checks satisfiability (outputs `Unsatisfiable`/`Satisfiable`).
- **TSTP proof output** only on `Refutation`; other statuses produce only the SZS status line.

---

## 11. CASC Hardware & `--casc` Decision Rule

> **This section is permanent policy. Do not remove or weaken it.**

**CASC competition hardware is exactly 8 CPU cores.** Every entry at CASC runs
with a wall-clock time limit (240 s for FEQ/FNE/UEQ, 120 s for EPS/EPU) on a
machine with 8 physical cores. All portfolio design, strategy selection, and
time-budget arithmetic **must treat 8 as the canonical core count**.

### Canonical hardware, and the modes that reproduce it

| | CASC entry | `--hardware casc` | `--hardware casc-sim` | `--hardware adaptive` (default) |
|---|---|---|---|---|
| workers | 8 | **8**, never auto-adapted | **8** | one per usable physical core, bounded by memory |
| memory allowance | 128 GB | **128 GB** | **128 GB**, plus `RLIMIT_AS` | 80 % of currently available RAM, cgroup-aware |
| CPU set | 8 physical cores | unrestricted | **pinned to 8 physical cores** (all SMT siblings of each) | unrestricted |
| wall clock | the per-division limit | the per-division limit | the limit, then **2× longer** (`--sim-time-factor`, `0` = until a resource cap) | the caller's `--time` |

- **`casc` is the honest setting for any number meant to be compared against
  CASC results.** It never grows to the host, so a 64-core box cannot quietly
  hand a schedule tuned for 8 cores sixteen workers. On a host with fewer than 8
  usable physical cores it warns and continues, and says so on the `% Hardware:`
  line, which every run prints.
- **`casc-sim` is for developing away from the competition machine.** It makes
  the constraint real rather than nominal: the process is pinned, its address
  space is capped, and it keeps searching past the CASC wall clock so a
  memory-bound failure surfaces instead of being recorded as a timeout. Each run
  reports where it stood when it crossed the CASC limit
  (`casc_limit_passed_ms=… state_at_casc_limit=…`), which is what separates "would
  have timed out at CASC" from "got past the CASC limit and then ran out of
  memory". Concurrent jobs all pin to the same 8 cores, so `--jobs N` behaves like
  N problems sharing one CASC machine — which is what the W8J2 runs did.
- **`adaptive` is for development and for benchmarking on a large box**, where
  the only question is whether a problem can be solved at all. It is not a CASC
  number and must never be reported as one.

Physical cores, not logical CPUs, throughout: SMT siblings add no capacity, so
pinning 8 logical CPUs would be simulating 4 cores. `casc` and `casc-sim` also
report `effective_mem_mb` and warn when the host has less RAM than the allowance,
because a limit above what the box can supply is not a limit.

A run that stops on a resource ceiling says which one — memory, term bank,
processed clauses or passive queue — in `% Resource limit:` and in the
`resource_reason=` field of the `% SZS detail` line the harness grades.

### Goal

Maximize the number of CASC problems solved across all entered divisions
(FEQ, FNE, UEQ, EPS/EPU). The competition `invoke.sh` already routes each
problem to the correct per-division schedule (`casc_feq/fne/ueq/epr`).
The question is whether those division schedules are optimal for 8 cores.

> **EPS exception — do not run the portfolio-optimisation workflow.** EPS asks
> for `Satisfiable`; the ordinary portfolio cannot make that claim, even when
> its clause set saturates, because completeness of the general search path is
> not established. EPS results come from the certified track. On the canonical
> shape (8 workers, 8 physical cores, `jobs=1`, campaign at `c07cac9`), the
> portfolio produced a definitive status on only 2 of 100 problems, both also
> answered by the certified track, and added 0 unique solves. Thus
> `greedy_set_cover --division eps` and `cooperative_portfolio_sweep.sh` do not
> optimize EPS coverage. Focus EPS work on the certified path and grounding
> tiers; see `docs/reports/benchmarks/eps-2026-09.md`.

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

**Step 1 — Generate optional solo diagnostic coverage data (run once per TPTP release):**

```bash
# Run every mrs strategy solo on one division (30 s per problem, 4 parallel jobs).
# Requires: nix develop -c cargo build --release
export TPTP=/path/to/TPTP-v9.x.x
./crates/mrs-bench/run_strategy_sweep.sh --divisions fne --time 30 --jobs 4 \
    --output results/sweep-fne-$(date +%Y%m%d)
```

This produces `run.csv` where each `system` column is `mrs-s01..mrs-s15`.
These runs measure individual strategies with one worker and no cross-strategy
shared equality pool. Their set-cover output is diagnostic only, not the final
cooperative portfolio objective.

**Step 2 — Use solo set-cover only to generate candidate portfolios:**

```bash
nix develop -c cargo run --release --bin greedy_set_cover -- results/sweep-fne-*/run.csv 8 --division fne
nix develop -c cargo run --release --bin greedy_set_cover -- results/sweep-fne-*/run.csv 8 --division ueq
nix develop -c cargo run --release --bin greedy_set_cover -- results/sweep-fne-*/run.csv 8 --division eps
```

**Step 3 — Measure the actual cooperative portfolio:**

```bash
# One mrs process per problem, eight workers, shared equality pool enabled.
MRS_WORKERS=8 MRS_SHARED_POOL_INTERVAL=500 \
./crates/mrs-bench/cooperative_portfolio_sweep.sh \
    casc-30 fne 11,4,12,1,6,8,2,3 30 4 \
    results/cooperative-fne-$(date +%Y%m%d)

# Control: same portfolio with sharing disabled.
MRS_WORKERS=8 MRS_SHARED_POOL_INTERVAL=0 \
./crates/mrs-bench/cooperative_portfolio_sweep.sh \
    casc-30 fne 11,4,12,1,6,8,2,3 30 4 \
    results/cooperative-fne-no-sharing-$(date +%Y%m%d)
```

The cooperative result is the portfolio-selection objective. The difference
between shared and no-sharing runs measures cooperation gain separately from
strategy diversity. Use `cooperative_portfolio_search.sh` for one-swap local
search over candidate portfolios.

**Step 4 — Act on cooperative results:**

- If greedy FNE portfolio = strategies `[s3, s7, s11, s1, s12, s6, s2, s10]` (example),
  replace the `casc_fne` loop-generated body in `named.rs` with those 8 explicit
  `SearchConfig` entries.
- Use the strategy descriptions in `strategy.rs` as reference for what each Sn is.
- Only add `--casc` to the binary if required (see decision rule above).

### Current status

The `casc_feq`, `casc_fne`, `casc_ueq`, `casc_epr`, `casc_eps`, `casc_epu`, and
`casc_icu` schedules have candidate priority orders derived from solo
strategy-sweep data. Solo coverage is diagnostic only; when enabled, the
cooperative portfolio can also share derived unit equalities through a
cross-strategy pool. Use
`cooperative_portfolio_sweep.sh` and `cooperative_portfolio_search.sh` to
validate or replace these orders against the actual 8-worker objective. See
`docs/reports/benchmarks/divisions-2026-09.md` for the workflow and telemetry
details.

---

## 12. Pre-phase (experimental, `exp/prephase-study`)

`mrs-prephase` holds the pre-phase: a bounded analysis of a problem run before
the first inference, whose output is a class label, a feasibility estimate, and a
routing plan. It is **off by default** and every routing rule is marked
`calibrated` or `prior` in `mrs --list-rules`.

**What it does**

| Symbol | Role |
|--------|------|
| `mrs_prephase::analyze` | ~80 structural features over the *clausified* clause set: scale, polarity shape, equality/variable structure, symbol and term shape, goal reachability under the engine's own symbol-distance BFS, clause-graph decomposition, measured redundancy (it runs the real PLE/BCE reducer), algebraic signature, propositional-abstraction size |
| `mrs_prephase::Analysis::label` | the class taxonomy: `logic/shape/scale/goal/decomposition`. 172 distinct values over the 26,990-problem TPTP distribution; 54 of them cover 95.9% of it |
| `mrs_prephase::Analysis::feasibility` | `LIKELY`/`UNKNOWN`/`UNLIKELY` — the one prediction the sweep supports, with a 5x spread between the extremes |
| `mrs_prephase::plan::rules` | the routing table: conditions → algorithm → priority order → pre-passes, each row with its own evidence string and calibrated/prior flag |
| `mrs_search::prephase::schedule_from_plan` | plan → `StrategySchedule`, one strategy per worker, cycled from the plan's priority order |
| `mrs_search::prephase::probe` | bounded reference search on a fixed *clause* budget, reporting generation/redundancy rates. Its verdict is deliberately discarded |
| `mrs_prephase::preprocessing` | the tautology / pure-literal / blocked-clause reducer, moved here from `mrs-search` so the analysis can measure the redundancy the engine actually removes |

**Instruments** (`crates/mrs-bench/prephase/`)

| Tool | Role |
|------|------|
| `prephase_dump` | one CSV row of features per `.p` file. `--rlimit-mb` isolates each file under an `RLIMIT_AS` ceiling, so a problem too large for the host is recorded as `resource_limit` instead of taking the run out with the OOM killer. `--probe` appends the probe's ten columns |
| `prephase_sweep` | the label: configuration × problem outcome matrix with time-to-solution and the engine's own telemetry. Stratified sampling, resumable |
| `analyze.py` | joins the two; coverage, greedy set cover, per-bucket conditional coverage, behavioural taxonomy, portfolio simulation, cross-validation, exact sign tests. `--features-only` runs the label-free corpus taxonomy |
| `ab.sh` | cooperative A/B of the pre-phase against the shipped per-division portfolio |

```bash
nix develop -c cargo build --release -p mrs-bench --bin prephase_dump --bin prephase_sweep

TPTP=~/TPTP-v9.3.0 ./target/release/prephase_dump \
    --root ~/TPTP-v9.3.0/Problems --out features.csv --jobs 2 --rlimit-mb 2500
TPTP=crates/mrs-bench/problems/casc-30 ./target/release/prephase_sweep \
    --features features.csv --sample 600 --time 3 --jobs 2 --out labels.csv
python3 crates/mrs-bench/prephase/analyze.py features.csv labels.csv
```

**Results.** `docs/reports/prephase/2026-10-prephase-study.md`. The headline is a
negative one and should be read before any routing work: on CASC-30 at 3 s and on
the hard tail at 30 s, routing on static structure is *worse* than the shipped
per-division orders, which are within three problems of the greedy optimum at
every portfolio width. Feasibility prediction works (5x spread); routing does not.
Do not add a routing rule without a measurement showing it wins on the subset it
fires on.

---

## 13. Testing

- Most tests are `#[cfg(test)]` inline modules — no separate test directories, no fixtures.
- Exception: `mrs-tptp` has integration tests under `crates/mrs-tptp/tests/` with fixture files in `tests/resources/`.
- Some `mrs-search` and `mrs-calculus` tests run the full given-clause loop with real `Duration::from_secs(5)` timeouts.
- `problems/` is for manual binary runs only, not `nix develop -c cargo test`.

---

## 14. Runtime env var

`TPTP=/path/to/TPTP` — only needed at runtime when problems use `%include` pointing to the standard TPTP library. The benchmark harness (`crates/mrs-bench/systems/mrs/invoke.sh`) sets this automatically to `crates/mrs-bench/problems/casc-30`, so it is not required for normal benchmark runs. Only set it manually when running the binary directly on problems that use `%include`.
