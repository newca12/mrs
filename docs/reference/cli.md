# CLI Reference

This page describes the `mrs` binary in the current workspace. The parser is a
small hand-written CLI in `src/main.rs`; there is no generated `--help` output.

## Invocation

```text
mrs [options] <file.p>
```

The positional path may be `-` only in a build with the `proover` feature. The
default time limit is 30 seconds. If `--workers` is omitted, MRS chooses a
memory-aware count based on physical CPUs.

## Search and scheduling

| Option | Meaning |
|---|---|
| `--time SECONDS` | Positive wall-clock budget. |
| `--workers N` | Positive number of search workers. |
| `--schedule NAME` | Select a named schedule; default is `casc`. |
| `--auto-schedule` | Select `casc_ueq`, `casc_epr`, `casc_fne`, or `casc_feq` from clause shape. An explicit `--schedule` wins. |
| `--strategy N` | Run one base strategy for the full budget. Valid IDs are 1 through 15 and require a CASC schedule. |
| `--portfolio IDS` | Run an explicit cooperative portfolio such as `11,12,1,6,10,8,14,4`; supply one ID per worker. |
| `--list-schedules` | Print registered schedule names and exit. |
| `--fast` | Deprecated alias for `--schedule fast`. |
| `--goal-transform MODE` | Override goal transformation: `recursive`/`all`, `maximal`/`top`, or `none`/`off`. |

Named schedules are documented in [Schedules](schedules.md).

## Trust and diagnostics

| Option | Meaning |
|---|---|
| `--self-check` / `--certified` | Asynchronously strict-check candidate refutations and fail closed if certification is rejected or inconclusive. |
| `--cert-reserve-worker` | Reserve one worker for strict certification when using `--self-check`. |
| `--certify-ordered` | Run the bounded ordered-resolution certifier. Requires `--workers 1 --strategy N`. Unsupported inputs return `GaveUp`. |
| `--stats`, `--info`, `--analyze`, `--profile` | Print the textual structural problem profile and exit. |
| `--profile-json` | Print the structural problem profile as JSON and exit. |
| `--include-root DIR` | Include root used by strict checking of stdin or external includes. |
| `--quiet` | Suppress non-SZS output in a `proover` feature build. |

Strict self-checking validates refutations, not arbitrary heuristic saturation.
See [Trust and verification](trust-and-verification.md).

## Search controls

| Option | Effect |
|---|---|
| `--no-bce` | Disable blocked-clause elimination. |
| `--no-ple` | Disable pure-literal elimination. |
| `--no-instgen` | Disable the EPR InstGen prepass. |
| `--no-lrs` | Disable limited-resource passive pruning. |
| `--trace-lrs` | Emit LRS pruning diagnostics. |
| `--no-sharing` | Set `MRS_SHARED_POOL_INTERVAL=0`. |
| `--trace-bce` | Emit preprocessing diagnostics. |
| `--trace-instgen` | Emit InstGen diagnostics. |

### Redundancy-elimination telemetry

The aggregate `% SZS detail` line carries three counters for the demodulation
memo, which remembers terms the unit-equality index has already been shown not to
rewrite:

| Field | Meaning |
|---|---|
| `demod_memo_lookups` | Times the memo was asked whether a term is irreducible. |
| `demod_memo_hits` | Of those, the ones it already knew. `hits / lookups` is the hit rate. |
| `demod_memo_evictions` | Times the memo reached its entry cap and was cleared. Non-zero means the cap, not the hit rate, is the limit. |

The rate is the reading that matters, and it is worth checking on any equational
problem before believing that demodulation is cheap: a memo invalidated as fast
as it is filled has a low rate and no effect, which looks identical to a
memoisation that was never consulted. `demod_memo_evictions=0` with a low rate
means the index is growing faster than the memo can be reused, not that the
memo is broken. See
[Redundancy elimination was 47% of the search](../reports/benchmarks/redundancy-throughput-2026-10.md)
for the profile that motivated it and the measured rates.

## Experimental pre-passes

| Environment variable | Effect |
|---|---|
| `MRS_CONDENSED_DETACHMENT=1` | Enable the bounded, proof-producing pre-pass for the compact LCL `is_a_theorem` condensed-detachment encoding. It has a one-second per-problem budget and five-second process-wide budget; declined cases continue to the normal schedule. |

When the pre-pass runs it prints one line to stderr, whether or not it finds
anything:

```
% SZS detail condensed_detachment=ran shape=matched stop=deadline facts=4358 \
  inferences=35724 elapsed_ms=1001 budget_ms=1000 max_facts=5000 max_inferences=100000
```

`shape` says whether the input carried the detachment rule at all. `stop` is the
reading that matters: `refutation` is the only positive, `deadline` /
`max_facts` / `max_inferences` mean a bound stopped the closure while it was
still making progress, and `facts_exhausted` means the closure ran out of
detachments to make — the one verdict a wider bound cannot change. Without this
line a null result is indistinguishable from a pre-pass that never ran, which is
what made the 2026-10-01 FNE campaigns unreadable; see
[Condensed detachment](../research/condensed-detachment.md).

### Widening the pre-pass bounds (diagnostic only)

| Environment variable | Default | Effect |
|---|---|---|
| `MRS_CD_BUDGET_MS` | `1000` | Per-problem wall budget for the pre-pass. |
| `MRS_CD_PROCESS_MS` | `5000` | Process-wide wall budget, so concurrent jobs cannot each pay the per-problem cap on every problem. |
| `MRS_CD_MAX_FACTS` | `5000` | Theorem-fact bound. |
| `MRS_CD_MAX_INFERENCES` | `100000` | Resolution-step bound. |
| `MRS_CD_ONLY=1` | — | Diagnostic harness mode: stop after the condensed-detachment attempt and return `GaveUp` if it did not refute, so a later portfolio result cannot be misattributed to the pre-pass. |

These exist so a measurement can tell "the closure cannot close this fragment"
apart from "the bound stopped it first". A malformed or zero value falls back to
the default rather than taking effect: zero would turn a bounded pre-pass into
unbounded closure, which is what the bounds exist to prevent.

Widening them spends portfolio budget — the pre-pass runs first and
`total_budget` shrinks by whatever it used — so it is a measurement action, not
a tuning one. `crates/mrs-bench/cd_bound_probe.sh` runs both bound settings over
the LCL cluster and prints the two arms side by side.

## ML options

These options parse in all builds. Their active behavior requires the relevant
feature build.

| Option | Meaning |
|---|---|
| `--log-ml-data DIR` | Write successful-refutation feature traces. With `ml`, schedule traces are written under `DIR/schedule`; worker traces use the configured format. |
| `--ml-log-csv` | Use CSV instead of wincode for trace output. |
| `--ml-weights FILE` | Load clause-selection weights in an `ml-guidance` build and default to the `ml` schedule. |
| `--ml-prune RATIO` | Enable premise pruning in an `ml` build. Requires `--ml-premise-weights`. Pruned workers are not allowed to claim positive saturation. |
| `--ml-premise-weights FILE` | Premise-selector model used with `--ml-prune`. |
| `--ml-schedule` | Deprecated alias for rule-based `--auto-schedule`; the old learned schedule classifier is retired. |
| `--ml-schedule-weights FILE` | Deprecated and ignored. |
| `--parent-guidance-weights FILE` | Load a schema-checked parent-pair linear model in a `parent-guidance` build. Guidance is restricted to the final active strategy slot in multi-strategy portfolios. |
| `--parent-guidance-threshold LOGIT` | Reject candidate inference pairs with model scores below this finite logit. Requires compatible weights; without a threshold, logging remains observational. A worker that rejects pairs cannot claim a definitive refutation or saturation. |

## Features

| Feature | Effect |
|---|---|
| default | Static prover; no stdin or quiet mode. |
| `proover` | Enables stdin input, `--quiet`, and ProoVer-friendly behavior. |
| `ml` | Enables ML trace logging and premise-selection support. |
| `ml-guidance` | Enables in-process ML-guided selection and weight loading. |
| `parent-guidance` | Enables parent-pair feature collection and opt-in inference pruning. |

Examples:

```bash
nix develop -c cargo run -- problems/socrates.p
nix develop -c cargo run -- --workers 1 --schedule casc_fne --strategy 11 problem.p
nix develop -c cargo run -- --auto-schedule --workers 8 problem.p
nix develop -c cargo run -- --workers 8 --schedule casc_feq \
    --portfolio 11,12,1,6,10,8,14,4 problem.p
```

## Environment variables

| Variable | Meaning |
|---|---|
| `TPTP` | TPTP root used to resolve standard `%include` files. |
| `MRS_SHARED_POOL_INTERVAL` | Positive polling interval enables equality sharing; `0` disables it. Default is `0`. |
| `MRS_LRS_FIXED_ITERATIONS` | Replace wall-clock LRS estimation with a fixed logical budget. |
| `MRS_LRS_POLICY=disabled` | Disable LRS. |
| `MRS_SINGLE_STRATEGY=N` | Diagnostic override for the default schedule; `16` selects the zero-time diagnostic slot and gives it the full budget. |
| `MRS_ORDERED` | Diagnostic ordered inference override; positive saturation remains fail-closed unless the bounded certifier is used. |
| `MRS_SINE_MAX_REMOVED_PERCENT` | Refuse a SInE filter that removes more than this share of the clause set. Default `100`, which disables the guard: on 74 measured solved problems SInE removed up to 99.9 % of the clause set on rows `mrs` wins, so a lower threshold switches it off where it pays. Starvation is caught instead by the no-growth and minimum-premises criteria in [`sine.rs`](../../crates/mrs-search/src/sine.rs), which separate every measured case. Values outside `0..=100` and unparsable values keep the default. |
| `TRACE_SINE=1` | Print the SInE decision per worker: clause count before and after, conjecture seed count, removed share, tolerance, depth limit, whether the filter was applied, and the `SineSkip` reason when it was refused. Diagnostic. |
| `MRS_NO_BCE`, `MRS_NO_PLE`, `MRS_NO_INSTGEN`, `MRS_NO_LRS` | Disable the corresponding search mechanism. |
| `MRS_EPR_GROUND=1` | Enable the EPR grounding pre-pass and let it return a refutation. Opt-in; no post-fix full-division coverage measurement. |
| `MRS_EPR_MODEL=1` | Also enable the bounded **model** route of that pre-pass, which can return a `Satisfiable` with a model certificate. Opt-in. Measured null on CASC-30 EPS at `8a1fdd0` — 44/100 with the flag on and off — so it buys latency on rows the cert track already solves, not coverage. See [`eps-2026-09.md`](../reports/benchmarks/eps-2026-09.md). |
| `MRS_EPR_SPLIT=1` | Per-clause equality splitting inside the EPR pre-pass. Measured negative; kept for the shape. |
| `MRS_EPR_EMATCH=1` | Complementary-unit E-matching in the EPR pre-pass. Bounded experiment, off by default. |
| `MRS_EPR_PROBE=1` | Re-decide the EPR pre-pass's ground instance set with the ordinary ground given-clause loop whatever the SAT verdict. Diagnostic. |
| `TRACE_EPR=1`, `TRACE_EPR_MODEL=1`, `TRACE_EPR_DUMP=<path>` | Per-round EPR pre-pass tracing, model-path tracing, and dumping the asserted ground instance set as TPTP. Diagnostic. |
| `MRS_EPS_CERTIFY` | Set to `0` to disable the benchmark wrapper's EPS certification worker. |
| `MRS_CERTIFY_STRATEGY` | EPS wrapper certifier strategy, `1` or `7`. |

## Status semantics

| SZS status | Meaning in the default binary |
|---|---|
| `Theorem` | A refutation of a conjecture was found. |
| `Unsatisfiable` | A refutation was found for a problem without a conjecture. |
| `Satisfiable` / `CounterSatisfiable` | Only a certified complete path or validated model path may emit this. |
| `GaveUp` | Search or verification was incomplete, restricted, unsupported, or fail-closed. |
| `Timeout` | The deadline expired without a definitive result. |
| `ResourceOut` | A resource containment limit was reached. |
| `Error` | Input, parsing, or infrastructure failure. |
