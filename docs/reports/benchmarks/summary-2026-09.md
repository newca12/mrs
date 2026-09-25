# Benchmark Summary

> Status: Dated benchmark interpretation. It is not a current baseline for
> commit `5a9c687` unless rerun.

This document is the durable interpretation of the benchmark work. It is not
an append-only run log. Raw commands and individual reports belong in the
historical [`benchmark-log.md`](../../history/benchmark-log.md); methodology
and soundness requirements belong in [`methodology.md`](../../policies/methodology.md).

The central rule is:

> Optimize the result that the competition actually measures, and never turn
> incomplete search into a definitive SZS answer.

## 1. Trust And Reporting Rules

- A `Theorem` or `Unsatisfiable` result is refutation-based and must have a complete derivation/proof path.
- `Satisfiable` and `CounterSatisfiable` require complete saturation or an independently checkable model certificate. A passing refutation verifier does not validate a satisfiability claim.
- Incomplete search, SInE filtering, SOS restriction, unit-only inference, LRS pruning, ML premise pruning, or bounded abstractions must fail closed as `GaveUp`, `Timeout`, or `Unknown`.
- Any reference-status violation is release-blocking, even when the run is faster or solves more problems.
- A benchmark is not clean merely because its solved count looks plausible. Require zero disagreements, zero polarity violations, zero reference violations, and no unexplained errors or panics.
- Historical benchmark numbers are evidence, not current baselines. Always record the exact commit, TPTP edition/release, problem count, division, timeout, workers, external jobs, hardware, schedule, environment variables, and output CSV.
- Local results are not official CASC ranking results. The local harness undershoots known systems such as Vampire and E by different amounts across divisions.

## 2. Benchmark Modes

### Normal `casc.sh` Portfolio

For an FEQ problem, the normal path is:

```text
casc.sh
  -> systems/mrs/invoke.sh
  -> mrs --workers 8 --schedule casc_feq problem.p
  -> eight concurrent strategy workers
```

The normal `casc.sh --systems mrs` run is already cooperative:

- Workers run different strategy configurations concurrently.
- Each worker owns its own `SearchState` and `TermBank`.
- Workers share derived positive unit equalities when
  `MRS_SHARED_POOL_INTERVAL` is positive; sharing is disabled by default.
- Workers exchange complete ancestor chains, not parent-less clause stubs.
- A shared stop flag ends sibling searches after a refutation or genuinely definitive result.

`MRS_WORKERS` controls search workers inside each MRS process. `--jobs` controls
how many independent problem processes `casc.sh` runs concurrently. On canonical
8-core CASC hardware, use `MRS_WORKERS=8 --jobs 1` for a non-oversubscribed
run.

### Solo Strategy Sweep

`run_strategy_sweep.sh` and `run_codex_sweep.sh` run one strategy with one
worker. They are useful for:

- Measuring individual strategy capability.
- Finding candidate strategies for a portfolio.
- Generating diagnostic greedy set-cover candidates.
- Studying per-strategy failure modes.

They do not measure the competition portfolio. There is no sibling strategy and
therefore no cross-strategy equality exchange.

The exact solo path should use the division schedule configuration:

```bash
mrs --schedule casc_feq --strategy 11 --workers 1 problem.p
```

`--strategy N` selects base strategy `N` from the selected division schedule.
It is preferable to the older generic `MRS_SINGLE_STRATEGY` interpretation for
portfolio studies because UEQ/ICU slot transformations and schedule-specific
SInE settings are preserved.

### Explicit Cooperative Portfolio

An explicit portfolio makes the strategy IDs and slot order independently
testable without editing `named.rs`:

```bash
MRS_SHARED_POOL_INTERVAL=500 mrs --schedule casc_feq \
    --workers 8 \
    --portfolio 11,12,1,6,10,8,14,4 \
    problem.p
```

The portfolio must contain exactly one ID per worker. It runs the same
cooperative machinery as the normal portfolio, including complete proof
ancestry; the command above explicitly enables shared equality exchange.

The benchmark wrapper is:

```bash
MRS_WORKERS=8 MRS_SHARED_POOL_INTERVAL=500 \
crates/mrs-bench/cooperative_portfolio_sweep.sh \
  casc-30 feq 11,12,1,6,10,8,14,4 240 1 \
  results/cooperative-feq
```

### No-Sharing Control

Use the same portfolio and disable only cross-strategy equality exchange:

```bash
MRS_WORKERS=8 MRS_SHARED_POOL_INTERVAL=0 \
crates/mrs-bench/cooperative_portfolio_sweep.sh \
  casc-30 feq 11,12,1,6,10,8,14,4 240 1 \
  results/cooperative-feq-no-sharing
```

This isolates sharing from ordinary strategy diversity. The comparison is:

```text
sharing_gain = solved(shared portfolio) - solved(same portfolio without sharing)
```

Because LRS and worker timing are wall-clock-sensitive, one pair of runs is not
enough to establish a small gain or loss. Repeat borderline comparisons.

## 3. Shared Equality Pool

The shared equality pool is a major MRS feature, not a new benchmark-only
addition.

- `0e8098d` introduced the parallel strategy portfolio.
- `78f0021` introduced cross-strategy unit-equality sharing.
- `05fe51f` capped the pool after rebroadcast flooding was observed.
- `0c7b12e` added deterministic polling epochs and stable import ordering.
- `bc4c156` made shared ancestor chains topologically ordered.
- `497b246` keyed sharing by the complete proof chain.
- `b124e7d` fixed worker-local `SymbolId` remapping for shared clauses.

The current pool transports positive unit equalities with complete ancestor
chains. Symbols are transferred by name and re-interned in the receiving
worker. Sharing is disabled by default. A positive interval opts in to polling
and publishing, for example:

```text
MRS_SHARED_POOL_INTERVAL=500
```

`MRS_SHARED_POOL_INTERVAL=0` disables publishing and importing.

Sharing has asymmetric benefits and costs:

| Lower interval | Higher interval |
|---|---|
| Equalities arrive sooner | More independent strategy exploration |
| Less duplicated equality work | Less lock/import/remapping overhead |
| More demodulation opportunities | Less worker convergence |
| More queue/index churn | Useful equalities may arrive late |
| Can reduce portfolio diversity | Can duplicate work unnecessarily |

The pool is equality-only. It does not currently share arbitrary clauses or
predicate-unit lemmas. Current telemetry has shown little or no equality import
activity on some FNE/EPS/EPU workloads, so a zero-sharing result there does not
prove equality sharing is the bottleneck.

## 4. Recorded Evidence

### Codex Profile Database

The repository snapshot `codex.db` contains:

| Corpus | Profiles | Complete | Results |
|---|---:|---:|---:|
| CASC-30 | 2,901 | 2,849 | 1,039 |
| CASC-J13 | 1,293 | 1,277 | 800 |
| Total | 4,194 | 4,126 | 1,839 |

Profile fields include clause/literal/axiom counts, unit/Horn/equality ratios,
term depth and size, EPR/FNE/FEQ/UEQ/FVO flags, AC/identity/inverse/idempotence
markers, conjecture-symbol overlap, and large-theory indicators.

Important limitations:

- 68 profiles are incomplete and must be excluded from structural aggregates.
- Result division and structural profile division are not always identical.
- `recommended_schedule` and `recommended_engine` are rule-based profile classifications, not measured cooperative labels.
- The database contains whole-portfolio result rows, not enough controlled labels for the best portfolio or sharing interval.
- Profile/result correlations are hypotheses, not causal proof.
- The competition binary must profile the input itself; it must not depend on a filename lookup in `codex.db`.

### Current CASC-30 Snapshot

The Codex summary records one 8-worker run over 1,039 rows:

| Division | Evaluated | Definitive | Rate |
|---|---:|---:|---:|
| FNE | 100 | 43 | 43.0% |
| FEQ | 400 | 112 | 28.0% |
| UEQ | 300 | 222 | 74.0% |
| EPU | 100 | 18 | 18.0% |
| EPS | 100 | 17 | 17.0% |
| ICU | 39 | 7 | 17.9% |
| Total | 1,039 | 419 | 40.3% |

The run had no reference-status violations. ICU also contained the recorded
OS-OOM event and must not be treated as an ordinary performance result.

### Current CASC-J13 Snapshot

The stored 800-row run records:

| Division | Evaluated | Definitive | Rate |
|---|---:|---:|---:|
| FNE | 100 | 35 | 35.0% |
| FEQ | 300 | 72 | 24.0% |
| UEQ | 400 | 257 | 64.3% |
| Total | 800 | 364 | 45.5% |

There were no errors, no `ko` rows, and no reference or polarity violations.

### Cooperative Sharing Evidence

The controlled results so far show that sharing is workload-dependent.

| Workload | Shared | No sharing | Observation |
|---|---:|---:|---|
| CASC-30 FEQ, explicit portfolio | 119/400 | 122/400 | One pair; sharing was three solves lower and faster on solved rows |
| CASC-J13 full FNE/FEQ/UEQ | 364/800 | 225/800 | Separate runs; difference concentrated in UEQ, not a strict same-run A/B |

Do not conclude that sharing is globally harmful from the FEQ pair or globally
beneficial from the J13 pair. Repeat controlled runs with identical hardware,
jobs, timeout, binary, portfolio, and problem set.

### UEQ Sharing-Interval Sweep

One CASC-30 UEQ interval sweep produced:

| Interval | Solved | Average |
|---:|---:|---:|
| 25 | 224/300 | 12.747 s |
| 50 | 226/300 | 12.491 s |
| 100 | **228/300** | **12.160 s** |
| 250 | 222/300 | 11.549 s |

All reported zero disagreements, zero polarity violations, and zero reference
violations. Interval 100 is a historical UEQ candidate, not a proven global
default. The runtime default is now no sharing; retain explicit `0` and
positive-interval controls such as `500` when repeating this experiment.

## 5. Runtime Profile And ML Direction

The profile-guided runtime architecture should be staged:

```text
parse/lower/clausify
        -> extract structural profile
        -> hard semantic routing
        -> profile-guided portfolio/sharing policy
        -> cooperative search
```

Hard routing comes first:

- Pure unit equality routes to UEQ.
- EPR structure routes only to supported EPR/refutation paths.
- Equality-free first-order routes to FNE.
- General first-order equality routes to FEQ.
- Unsupported or incomplete profiles fall back conservatively.

The model must never decide whether a search result is logically definitive.
It may only choose a performance policy such as:

- Strategy portfolio IDs.
- Slot order.
- Sharing interval.
- SInE/AVATAR policy where already soundly supported.
- Confidence/fallback level.

Training labels must come from cooperative experiments, not from
`recommended_schedule` and not from the union of solo coverage. The target
should be:

```text
(profile, portfolio, sharing_interval)
    -> solved / time / reference correctness / telemetry
```

Use domain- or family-grouped train/test splits to avoid leakage. First deploy
the model in shadow mode, logging its proposed policy while the static schedule
still runs. Only enable model-selected policies after repeated cooperative
validation and zero reference violations.

The previous ML experiment is a warning:

- A retrained model achieved validation AUC around 0.84 on FEQ traces.
- Proving performance still fell from the static baseline.
- A better clause classifier did not fix schedule composition or objective mismatch.
- ML is therefore frozen for competition until its labels and integration objective are cooperative and outcome-based.

## 6. Recommended Future Experiments

1. Repeat UEQ intervals `0, 25, 50, 100, 250, 500, 1000` with identical controls.
2. Run the same interval grid on FEQ and FNE, not only UEQ.
3. Record `strategy_ids`, `shared_published`, and `shared_imported` for every row.
4. Add eligible/published/imported/rejected/used counts before extending sharing beyond equality clauses.
5. Run one-swap cooperative portfolio search using `cooperative_portfolio_search.sh`.
6. Compare current portfolio, candidate portfolio, and no-sharing controls on the same problem set.
7. Join `codex.db` profiles to cooperative results by corpus and canonical problem identity.
8. Train a policy model only after the cooperative experiment table is sufficiently populated.
9. Keep all incomplete paths fail-closed as `GaveUp`, `Timeout`, or `Unknown`.
10. Treat zero reference violations as a merge/release gate, not merely a reporting detail.

## 7. Command Reference

Build the relevant binaries:

```bash
nix develop -c cargo build --release -p mrs -p mrs-bench --bin bench_report
```

Run a cooperative portfolio:

```bash
MRS_WORKERS=8 \
crates/mrs-bench/cooperative_portfolio_sweep.sh \
  casc-j13 feq 11,12,1,6,10,8,14,4 180 1 \
  results/cooperative-j13-feq
```

Run the no-sharing control:

```bash
MRS_WORKERS=8 MRS_SHARED_POOL_INTERVAL=0 \
crates/mrs-bench/cooperative_portfolio_sweep.sh \
  casc-j13 feq 11,12,1,6,10,8,14,4 180 1 \
  results/cooperative-j13-feq-no-sharing
```

Run one-swap cooperative search:

```bash
crates/mrs-bench/cooperative_portfolio_search.sh \
  casc-j13 feq 180 1 1 results/cooperative-search-j13-feq
```

Inspect a completed CSV:

```bash
target/release/bench_report results/cooperative-j13-feq/run.csv
```

Run solo diagnostic coverage:

```bash
crates/mrs-bench/run_strategy_sweep.sh \
  --edition casc-j13 --divisions fne,feq,ueq \
  --casc-times --jobs 30 \
  --output results/solo-j13
```

Solo results are candidate-generation evidence only. Final portfolio decisions
must be based on cooperative runs.

## 8. Maintainer Checklist

Before changing a `casc_*` order or sharing interval:

- Confirm the candidate uses exactly one strategy per worker.
- Run shared and no-sharing controls on the same set.
- Repeat if the solve-count difference is small.
- Check zero disagreements, polarity violations, and reference violations.
- Preserve raw CSV, stderr, command, commit, hardware, TPTP root, and environment.
- Do not interpret a better average time on solved rows as a coverage gain.
- Do not convert solo set-cover coverage into a cooperative claim.
- Do not allow profile/ML policy errors to produce a new definitive SZS status.
- Update the benchmark log and this summary only after the raw result exists.
