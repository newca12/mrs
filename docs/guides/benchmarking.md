# Benchmarking Guide

The benchmark harness is in `crates/mrs-bench`. It archives raw stdout/stderr,
hashes, host metadata, toolchain metadata, and one CSV row per problem/system.

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
