# Troubleshooting

## Missing includes or unexpectedly huge searches

Set `TPTP` to the exact corpus root used by the problem. For benchmark runs,
prefer the wrapper's `CASC_PROBLEMS_ROOT` and avoid inheriting a global TPTP
installation. Compare the processed clause count and `% SZS detail` telemetry
with a known clean canary.

## Timeout versus GaveUp

- `Timeout` means the deadline expired.
- `GaveUp` means a path was incomplete, unsupported, pruned, or failed closed.
- `ResourceOut` means a containment limit fired.

Do not convert `GaveUp` or `Timeout` into a satisfiability claim.

## Non-reproducible telemetry

Use `--workers 1` for a deterministic strategy diagnosis. Disable sharing with
`MRS_SHARED_POOL_INTERVAL=0`. For logical LRS experiments, set
`MRS_LRS_FIXED_ITERATIONS` to a fixed budget. These controls improve diagnosis;
they do not necessarily reproduce competition behavior.

## Stack or memory failures

The benchmark wrapper exports `RUST_MIN_STACK=67108864` and raises the main
stack where possible. Run one problem at a time, use an external timeout, and
reduce `--workers` on low-memory hosts. `mrs-search` has resource containment,
but a host-level OOM is still an infrastructure failure.

## Unexpected positive status

Re-run with `--workers 1 --self-check` where supported. Save the input, output,
stderr, commit, binary hash, and environment. A strict-check rejection is a
release blocker, not a benchmark classification detail.
