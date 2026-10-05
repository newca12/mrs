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

## Certifier returns `GaveUp` without a `TRACE_CERTIFY` reason

Some certified-path refusals are currently silent. Enable tracing to determine
how far the problem gets:

```bash
TRACE_CERTIFY=1 ./target/release/mrs --time 120 --workers 1 \
    --schedule casc_eps --strategy 1 --certify-ordered EPS/<problem>.p 2>&1 \
    | grep CERTIFY
```

- **No output** can mean the input was rejected before grounding. For example,
  `EPS/HWC004-1` is refused in about 1 ms because its equality sides contain
  function symbols.
- **`phase=ground …` followed by silence** can indicate an unsupported
  non-unit positive equality or an equality-expansion deadline. The current
  implementation does not log those refusal reasons; the EPS campaign report
  documents the measured cases and boundary.

Other refusals may include explicit reasons such as `instance_count_overflow`,
`tier_size`, `atom_limit`, or `closure_time`. Treat a refusal as a fail-closed
result, not a satisfiability conclusion.

## Non-reproducible telemetry

Use `--workers 1` for a deterministic strategy diagnosis. Disable sharing with
`MRS_SHARED_POOL_INTERVAL=0`. For logical LRS experiments, set
`MRS_LRS_FIXED_ITERATIONS` to a fixed budget. These controls improve diagnosis;
they do not necessarily reproduce competition behavior.

## Stack or memory failures

The benchmark wrapper raises the main-thread stack with `ulimit -s`, and every
thread that recurses sizes its own stack in code
(`mrs_core::RECURSION_STACK_BYTES`, 64 MiB) — the search workers, the certifier,
the proof coordinator, the verification workers and the ATP ladder. Nothing
depends on an ambient `RUST_MIN_STACK` any more, so a direct `cargo run` gets the
same stacks the wrappers do. Run one problem at a time, use an external timeout,
and reduce `--workers` on low-memory hosts. `mrs-search` has resource
containment, but a host-level OOM is still an infrastructure failure.

Because a thread stack is reserved address space and committed on demand, those
reservations cost nothing in resident memory — but they are charged in full by
an `RLIMIT_AS` cap, at 64 MiB per worker. A tool that caps address space has to
budget for that; `crates/mrs-bench/perf_probe.sh` does, and refuses a worker
count that will not fit.

**If it still overflows, suspect the main thread.** Parsing and clausification
run there, and the main thread's stack is the one thing an environment variable
still sets (`ulimit -s unlimited`, which the competition wrappers do and a bare
`cargo run` does not). A stack overflow in any thread aborts the process with no
message, so an unexplained instant death on a deeply nested problem is this
until proven otherwise: check `ulimit -s`, or re-run under the wrapper. Removing
that last environment dependence is tracked under "Known Boundaries" in
[`docs/policies/release.md`](../policies/release.md).

## Unexpected positive status

Re-run with `--workers 1 --self-check` where supported. Save the input, output,
stderr, commit, binary hash, and environment. A strict-check rejection is a
release blocker, not a benchmark classification detail.
