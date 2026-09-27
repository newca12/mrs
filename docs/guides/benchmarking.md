# Benchmarking Guide

The benchmark harness is in `crates/mrs-bench`. It archives raw stdout/stderr,
hashes, host metadata, toolchain metadata, and one CSV row per problem/system.

Two different questions need two different tools:

| Question | Tool |
|---|---|
| How many TPTP problems does `mrs` solve? | `casc.sh` (below) |
| How fast does `mrs` search on *this* machine? | [`perf_probe.sh`](../results/perf/README.md) |

A solved count is the objective the schedules were tuned for. Throughput is a
diagnostic: it explains *why* a run is slow on a given box, and it is the only
one of the two that can be compared across machines without a TPTP corpus.

## Quick performance probe

```bash
crates/mrs-bench/perf_probe.sh            # writes docs/results/perf/bank.tsv
crates/mrs-bench/perf_probe.sh --no-bank  # measure only, touch nothing
```

It generates a clause set, runs a fixed amount of search work on it, and banks
the result against the host's fingerprint. Work is stopped by an
iteration-counted ceiling rather than a clock, so the timings from two machines
are comparable; a "run it for 30 seconds and count clauses" measurement is not,
because the search is wall-clock sensitive. It builds both a `native` and a
`haswell` binary so ISA effects are separable, and it caps the whole probe at
12 GiB, divided across workers for the search's RSS watchdog. The driver uses
the same affinity/cgroup-aware physical-core count as the prover for its default
worker list. See [`docs/results/perf/README.md`](../results/perf/README.md) for the
method, the comparability rules, and how to add another host.

## Prepare the corpus

```bash
nix develop -c cargo build --release
crates/mrs-bench/setup.sh
```

The extracted corpus is normally under
`crates/mrs-bench/problems/<edition>`. Set `TPTP` or `CASC_PROBLEMS_ROOT`
explicitly when using a different TPTP checkout.

## Run a benchmark

```bash
MRS_WORKERS=8 crates/mrs-bench/casc.sh \
  --edition casc-30 \
  --systems mrs \
  --divisions fne,feq,ueq \
  --casc-times \
  --jobs 1 \
  --output crates/mrs-bench/results/casc-30/current-run
```

On canonical eight-core competition hardware, use `MRS_WORKERS=8 --jobs 1` to
avoid oversubscribing the machine. `--jobs` controls independent problem
processes; `MRS_WORKERS` controls workers inside one MRS process.

Summarize a run with:

```bash
nix develop -c cargo run --release -p mrs-bench --bin bench_report -- \
  crates/mrs-bench/results/casc-30/current-run/run.csv
```

## Solo strategy diagnostics

```bash
crates/mrs-bench/run_strategy_sweep.sh \
  --edition casc-j13 --divisions fne,feq,ueq \
  --casc-times --jobs 4 \
  --output crates/mrs-bench/results/solo-j13
```

Solo sweeps measure one strategy with one worker. Their union and greedy
set-cover output are candidate-generation diagnostics only. They do not model
cooperative strategy execution or equality sharing.

## Cooperative portfolio experiments

```bash
MRS_WORKERS=8 MRS_SHARED_POOL_INTERVAL=500 \
  crates/mrs-bench/cooperative_portfolio_sweep.sh \
  casc-30 feq 11,12,1,6,10,8,14,4 30 1 \
  crates/mrs-bench/results/cooperative-feq

MRS_WORKERS=8 MRS_SHARED_POOL_INTERVAL=0 \
  crates/mrs-bench/cooperative_portfolio_sweep.sh \
  casc-30 feq 11,12,1,6,10,8,14,4 30 1 \
  crates/mrs-bench/results/cooperative-feq-no-sharing
```

Compare the same portfolio, corpus, binary, hardware, timeout, and process
concurrency. Repeat small differences because LRS and worker scheduling are
wall-clock-sensitive.

## Proof audits

The harness preserves generated proofs for deferred audit:

```bash
nix develop -c cargo run --release -p mrs-bench --bin audit_casc_proofs -- \
  --run crates/mrs-bench/results/casc-30/current-run \
  --problems-dir crates/mrs-bench/problems/casc-30 \
  --checks strict,mrs,ladder \
  --strict-time 30 --mrs-time 10 --ladder-time 30 \
  --ladder-workers 8 --jobs 1 \
  --output crates/mrs-bench/results/casc-30/current-run/proof-audit
```

## Required provenance

Every report must record:

- commit and dirty-worktree state;
- exact TPTP edition or checkout;
- problem count and divisions;
- per-division timeout;
- MRS workers and outer jobs;
- CPU, RAM, operating system, and Rust version;
- schedule, portfolio, and relevant environment variables;
- raw CSV and proof-audit paths; and
- disagreements, polarity violations, reference violations, errors, and OOMs.

Never call a local run an official competition ranking.
