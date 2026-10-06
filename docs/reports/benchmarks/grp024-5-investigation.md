# GRP024-5: associative commutator, UEQ

Investigation branch: `investigate/grp024-5` (starting at `8bc0765`).

## Input and expected outcome

`crates/mrs-bench/problems/casc-30/UEQ/GRP024-5.p` includes
`Axioms/GRP004-0.ax`. Its six unit CNF clauses consist of left identity,
associativity and left inverse for `multiply`, the definition
`commutator(X,Y) = inverse(X) * (inverse(Y) * (X * Y))`, associative
commutators, and the ground goal `commutator(b,c) * a != a * commutator(b,c)`.
The benchmark's expected result is **Unsatisfiable**: the associative
commutator hypothesis entails that the specified commutator is central.
The goal is explicitly a `negated_conjecture`, despite the CLI reporting
`0 conjectures` (the latter counts conjecture-role formulas, not this role).

## Archived campaigns

Result trees are locally accessible at `/home/fr22192/pve/crates/mrs-bench/results/`.
Each row below is from `run.csv`, with additional configuration from
`run_meta.json` and `raw/mrs/ueq/GRP024-5.stderr` in the named directory.

| Campaign directory | Result / budget | Processed / generated / passive | LRS discarded / forward subsumed | Host and settings |
|---|---|---|---|---|
| `campaign-casc30-ueq-W8P8J1-NOLRS-MB90G-20261005` | Timeout / 240 s; internal elapsed 240995 ms | 15,429 / 1,176,854 / 1,112,975 | 0 / 48,510 | 8 workers pinned to 8 physical cores, `casc-sim`, 90 GB budget, `MRS_NO_LRS=1`, sim factor 1; peak 29,711 MB |
| `campaign-cascj13-ueq-W8C8J1-20261002` | Timeout / 180 s; internal elapsed 184891 ms | 18,140 / 1,049,415 / 108,518 | 863,419 / 59,398 | 8 adaptive workers, ~46.8 GB effective budget, LRS enabled; peak 21,519 MB |

Both reported `timeout=8 saturated=0` and used strategy IDs
`4,8,12,11,2,14,15,1` in that slot order. Neither published/imported
shared clauses. These are **not** controlled LRS A/B runs: they use different
editions, hosts, memory ceilings, binary hashes and revisions (both recorded
as dirty). The large difference in passive size is consistent with the
reported LRS removals; the processed counts cannot be interpreted as an
isolated LRS speedup. Both archives contain a real `% SZS status Timeout`
line, unlike older runs documented in `docs/policies/unresolved-issues.md`
that hit the harness's outer timeout without a status. Earlier archived
`casc-30-W8J2-20260917` and `casc-30-W8J2-noshare-20260922` also timed out
with different orders and sharing settings; neither establishes a solve.

## Local reproduction

Built at the branch base with
`nix develop -c cargo run --release --bin mrs -- ...` (release compilation
completed). Runs below used `target/release/mrs`, an adaptive one-worker
configuration on this four-physical-core host, with an eight-second budget:

| Invocation suffix | Result | Processed / generated / passive | LRS discarded |
|---|---|---|---|
| `--schedule casc_ueq --strategy 4 --workers 1 --hardware adaptive --time 8` | Timeout | 613 / 19,563 / 10,163 | 8,124 |
| same with `MRS_NO_LRS=1` | Timeout | 405 / 16,687 / 15,804 | 0 |
| `--strategy 8` (other flags as above) | Timeout | 57 / 969 / 907 | 0 |
| `--strategy 12` (other flags as above) | Timeout | 444 / 18,748 / 7,380 | 10,735 |

These runs are diagnostic, **not** CASC-comparable. In the strategy-4 run
with `TRACE_SINE=1`, SInE reports `before=6 kept=6 ... applied=false
skip=too_small`: no input axiom is lost to premise selection.
`--pre-phase-only` reports `UEQ/HORN/TINY/TIGHT/CONNECTED`, six clauses,
feasibility `LIKELY`, and a prior (uncalibrated) completion route; that is a
classification, not evidence of a proof.

With `TRACE_PROGRESS=1` and `--strategy 8 --time 12`, the first 69
iterations complete in approximately 0.3 s. The last line is
`[PROGRESS] iter=70 given=130 lits=1 terms=31562 processed=57 passive=907`;
there is no matching `iter=70 generated=...` line, and the final timeout
still reports 57 processed / 969 generated. The same last iteration and
counters appear in a separate two-second run. Thus this worker spends the
remaining budget **inside iteration 70**, after choosing the given clause
and before the end-of-inference progress line. The trace does not identify
the precise operation: it could be given-clause simplification, partner
lookup, superposition, or another stage inside that iteration. Term-bank
growth (29 initially, 31,562 at iteration 70) is substantial even though
only 969 clauses were generated. This local stall cannot be assumed to be
the cause of the archived eight-worker timeouts, which contain no per-worker
progress trace. `TRACE_PROGRESS` emits a line per iteration, so collect a
bounded/filtered trace rather than an unrestricted multi-worker log.

## Next diagnostic

Instrument the entry/exit of the *given-clause* stages between the two
`TRACE_PROGRESS` lines (around `given_clause.rs:1606` and `:2806`) for the
slow worker, with elapsed time and candidate counts at the stalled
iteration. In particular distinguish simplification from the two directions
of indexed superposition and from the cost of generating their term IDs.
Then run single-strategy 8 with a short deadline and verify which stage
consumes it, before proposing a search or indexing change. Keep any
performance comparison on the same revision, hardware mode, input edition,
worker count, and LRS policy.

## Follow-up: LPO pathology and the wider silent-timeout set

In the same branch, a targeted `TRACE_GIVEN_ITER=70` probe in
`given_clause.rs` isolated the strategy-8 delay to the *given as equality
source* superposition phase. Its lookup found 15 indexed targets almost
immediately. A temporary `TRACE_SLOW_SUPERPOSITION` probe in
`superposition.rs` measured individual successful unifications at 1–23 µs,
but the subsequent term ordering comparisons at ~100–265 ms **each**.
These probes were removed after diagnosis.

The LPO implementation (`ordering.rs`) recursively tests the same `(TermId,
TermId)` pairs through subterm, all-arguments, and lexicographic cases. On
shared nested terms, that was combinatorial redundant work. A local memo of
the Boolean `s >_LPO t` result, scoped to one `compare_id` call and reused
for both directions, removes this repeated work without leaking TermIds
between term banks or changing the LPO relation. A regression test compares
80-level shared binary terms in both directions.

At iteration 70 of GRP024-5, all 15 source-target superpositions now finish
in approximately **8 ms** (stage starts at ~125 ms and reverse superposition
starts at ~133 ms) instead of consuming the entire five-second budget.
The same five-second solo run now processes 113 clauses and generates 7,840,
versus 57 and 969 before the cache. At 20 seconds it processes 282 and
generates 28,702 but still times out: this is a performance repair, **not**
yet a proof of GRP024-5.

All 14 other named silent-timeout inputs were run on this host with a
two-second solo strategy-8 budget after the cache. All returned a normal
`% SZS status Timeout` and telemetry; none produced a refutation at this
budget. This short-run observation does **not** establish that the former
outer-harness hangs are all caused by LPO: their old symptom occurred after
180–240 seconds in different eight-worker campaigns. In particular,
`GRP654-10` with `TRACE_PROGRESS=1` spends over 100 ms on each of several
*generated-clause demodulation* calls at iteration 13, with ~26,000 interned
terms. A twelve-second run still times out (71 processed / 2,538 generated).
It has a distinct immediate bottleneck, which needs its own diagnosis
before concluding that the full set shares one cause. The local runs did
not emulate CASC hardware; the follow-up should use controlled old/new
binary comparisons on identical host and campaign settings for all 15.
