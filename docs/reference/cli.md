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

## Features

| Feature | Effect |
|---|---|
| default | Static prover; no stdin or quiet mode. |
| `proover` | Enables stdin input, `--quiet`, and ProoVer-friendly behavior. |
| `ml` | Enables ML trace logging and premise-selection support. |
| `ml-guidance` | Enables in-process ML-guided selection and weight loading. |

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
| `MRS_NO_BCE`, `MRS_NO_PLE`, `MRS_NO_INSTGEN`, `MRS_NO_LRS` | Disable the corresponding search mechanism. |
| `MRS_EPS_CERTIFY` | Set to `0` to disable the benchmark wrapper's EPS certification worker. |
| `MRS_CERTIFY_STRATEGY` | EPS wrapper certifier strategy, `1` or `7`. |
| `RUST_MIN_STACK` | Worker-thread stack size; the benchmark wrapper sets 64 MiB. |

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
