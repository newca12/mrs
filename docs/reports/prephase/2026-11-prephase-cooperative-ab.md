# CASC-30 cooperative A/B screen for pre-phase routing

*Analysis date: 2026-11-05.*
*Raw outputs:*
`/home/hack/results/prephase30/ab-casc-30-FEQ-20261103-133000/`,
`/home/hack/results/prephase30/ab-casc-30-UEQ-20261102-135248/`, and
`/home/hack/results/prephase30/ab-casc-30-FNE-20261102-110114/`.

> **Status:** preliminary observation only. The complete FEQ and UEQ runs compare
> a static division schedule with the routed schedule, but the supposed baseline
> also ran pre-phase analysis in report-only mode. This is not a clean
> pre-phase-off/on comparison, a replicated paired campaign, or a strict proof
> audit. Do not use it to promote a schedule or enable routing by default.

## Results

The saved FEQ and UEQ `run.csv` files contain all 400 and 300 division entries,
respectively. “Solved” below counts definitive SZS statuses (`Theorem`,
`Unsatisfiable`, `Satisfiable`, or `CounterSatisfiable`); these CSVs contain only
raw SZS statuses, not strict-certification verdicts. The memory resource-out
counts are from each problem's `resource_reason=memory` telemetry. The runs
targeted 240 s per problem; `ab.sh` passes `--time 238` to leave a two-second
harness margin. The stderr logs report 8 workers in `casc` hardware mode.

| Division | Static schedule | Routed schedule | Routed-only / static-only | Memory `ResourceOut`, static / routed |
|---|---:|---:|---:|---:|
| FEQ | 114/400 | 111/400 | 3 / 6 | 3 / 5 |
| UEQ | 150/300 | 151/300 | 3 / 2 | 29 / 28 |
| **FEQ + UEQ** | **264/700** | **262/700** | **6 / 8** | **32 / 33** |

The net is **−2** routed solves over the 700 completed instances, with 14
problem-level solved/unsolved flips. The small net hides churn in both
directions:

- **FEQ:** routing gained `REL020+2`, `REL021+1`, and `REL035+1`
  (`GaveUp` → `Theorem`). The static schedule alone solved `COM135+1`,
  `GEO278+1`, `GEO340+1`, `KLE132+1`, `SWW345+1`, and `SWW383+1`.
- **UEQ:** routing gained `LAT177-1`, `LAT230-10`, and `REL020-2`
  (`Timeout` → `Unsatisfiable`). The static schedule alone solved `LCL024-10`
  and `RNG006-10` (`Unsatisfiable` → `Timeout`).

These are per-instance observations from one run of each arm, not evidence that
the differences are stable. In particular, the mix of gains and losses is not a
measured routing advantage or disadvantage.

## FNE run stopped before the routed arm

The FNE input directory contains 100 problems. The baseline CSV has 79 completed
rows: 40 `Theorem`, 35 `Timeout`, and 4 `GaveUp`. The attempt for `NLP260+1` has a
stderr file but no SZS result or CSV row; its log records a clausified set of
1,026,861 clauses and ends at search start. The pre-phase CSV contains only its
header, so no FNE A/B comparison was completed. The saved files do not establish
the process's exact exit cause; host-memory pressure is plausible but should not
be reported as a confirmed OOM.

The runner uses `set -e`, so a non-zero prover exit stops the arm before its
status is appended and prevents the next arm from running. A future campaign
should record the process exit/resource reason as a row and continue, while
preserving a separate indication that the problem did not produce an SZS status.

## What this experiment actually compared

The arm setup in `crates/mrs-bench/prephase/ab.sh` passes the arm name as the
`MRS_PREPHASE` value (`baseline` or `prephase`). `src/main.rs` enables pre-phase
when that variable is present with any value other than `0`. Therefore the
baseline arm **did run pre-phase analysis**. Its explicit `--schedule casc_feq`
or `--schedule casc_ueq` prevented the resulting route from replacing the
schedule, and its stderr logs say the routing decision was only reported. The
routed arm applied the plan. Both arms consequently paid pre-phase analysis
overhead; this is a static-schedule-versus-routed-schedule screen with that
overhead in both arms, not a comparison against the normal no-pre-phase static
run. The data cannot establish the end-to-end cost or benefit of enabling the
feature relative to the no-pre-phase default.

Other limits on interpretation:

- This is one fixed-order campaign per division: the static arm ran before the
  routed arm. It is not the minimum two randomized paired blocks required by
  `AGENTS.md` §11.
- The saved `run.csv` files report SZS statuses only; no strict proof-audit
  results are included. These counts are not `VerifiedGood` scores.
- The logs report `hardware=casc`, 8 workers, 16 host physical cores, and
  `effective_mem_mb=54198` against a 131,072 MB allowance. The host warning says
  only 63,763 MB was available, so the 128 GiB CASC allowance was not
  representable. This is not a standardized CASC-hardware result.
- `ab.sh` assigns its `JOBS` argument but does not use it; as written, each arm's
  problem loop is serial. Do not infer an outer jobs count from that argument.
- The output folders contain CSVs and per-problem stderr, but no producer binary
  hash or `run_meta.json`.

## Interpretation and follow-up

The complete FEQ/UEQ screen is close to even and mixed by division, but the
baseline environment mistake, one-run design, absent strict audit, and limited
host memory make it insufficient to accept or reject the router. No shipped
schedule should change on this result. Before rerunning, make the baseline set
`MRS_PREPHASE=0` and the routed arm set it explicitly to `1`; make the harness
record failures and continue; then follow the randomized, replicated, audited
cooperative procedure in `AGENTS.md` §11. The FEQ, UEQ, and incomplete FNE outputs
remain under `/home/hack/results/prephase30/` for reference.
