# mrs-bench

CASC benchmark harness and report tool for `mrs`.

## Hardware profiles

A benchmark run has to say what it was measured under, or the number means
nothing. `MRS_HARDWARE` selects the profile for the whole run and the prover
prints the resolved one on its `% Hardware:` line, so an archived `run.csv` is
self-describing.

| `MRS_HARDWARE` | workers | memory | CPU set | wall clock |
|---|---|---|---|---|
| *(unset)* / `adaptive` | one per usable physical core, bounded by memory | 80 % of available RAM, cgroup-aware | unrestricted | `--time` |
| `casc` | **8** | **128 GB** | unrestricted | `--time` |
| `casc-sim` | **8** | **128 GB** + `RLIMIT_AS` | **pinned to 8 physical cores** | `--time` × `--sim-time-factor` (default 2), or unbounded |

Use `casc` for anything whose number will be compared against CASC results: it
never grows to the host, so a 64-core box cannot hand a schedule tuned for 8
cores sixteen workers. Use `casc-sim` to develop against a competition-shaped
constraint on a machine that is not the competition machine — it keeps searching
past the CASC wall clock so a memory-bound failure surfaces instead of being
recorded as a timeout, and reports where the run stood at the CASC limit
(`casc_limit_passed_ms=… state_at_casc_limit=…` in `% SZS detail`). Use the
default when the only question is whether a problem can be solved at all.

Notes:

- The core count is **physical** cores, detected from the kernel's affinity mask,
  the cgroup CPU quota and SMT topology. `num_cpus::get_physical()` reports the
  whole machine and is wrong inside a cpuset, so it is no longer the default.
- In `casc-sim` every job pins to the same 8 cores, so `--jobs N` means N problems
  sharing one CASC machine — which is what the archived `W8J2` runs did.
- When the host cannot represent the profile (fewer than 8 physical cores, less
  RAM than 128 GB) the run continues and says so, rather than silently
  downgrading to something that only looks like a CASC entry.
- A run that stops on a resource ceiling reports which one:
  `% Resource limit: memory limit 131072 MB reached (rss …)`, and the same
  reason in `resource_reason=` for the grader. `bench_report` separates
  `resource_out_memory` from the clause-shaped ceilings.

## Contents

| Path | Purpose |
|------|---------|
| `casc.sh` | Run a full benchmark: invoke each system on each problem, collect SZS status and wall time, archive raw stdout/stderr and hashes, and write `results/<edition>/*/run.csv` |
| `perf_probe.sh` | Measure search throughput on this host with a fixed amount of work, then append the result to the bank in `docs/results/perf/` |
| `cooperative_portfolio_sweep.sh` | Measure an explicit multi-worker portfolio; enable shared equality exchange with a positive `MRS_SHARED_POOL_INTERVAL`, or use `0` for a no-sharing control |
| `cooperative_portfolio_search.sh` | Run one-swap local search over portfolios using cooperative solved-count coverage |
| `setup.sh` | Download and extract the CASC problem and axiom archives from tptp.org |
| `systems/` | Per-system `invoke.sh` scripts (add a new directory here to register a competitor) |
| `problems/` | Extracted TPTP problems and axioms (gitignored, populated by `setup.sh`) |
| `results/` | CSV output from benchmark runs (gitignored) |
| `src/main.rs` | `bench_report` binary — summarises a `run.csv` file |
| `proover.sh` | Per-system harness for `mrs-proover` (CSV of verdicts per proof file) |
| `normalize_proover2026` / `validate_proover2026` / `score_proover2026` / `audit_proover` | Rust normalization, validation, scoring, and status/timing audit for the committed ProoVer corpus |
| `fuzz_proover.sh` | Generate proofs with eprover/vampire on a problem tree, then verify each with `mrs-proover`; surfaces unhandled inference rules and recurring failure reasons |
| `proover_compare.sh` | Run `mrs-proover` over a proof set with each ATP backend in isolation (`--only-mrs` / `--only-eprover` / `--only-vampire`); reports per-backend verdicts and wall times |
| `build_proover_corpus.sh` / `verify_proover_corpus.sh` | Build (network) and verify (offline) the committed deterministic E/Vampire regression corpus; see `docs/PROOVER_HARNESS.md` |
| `fetch_zenodo_corpus.sh` | Download + normalise the Zenodo 19792604 proof-checker benchmark (gitignored under `zenodo-corpus/`) |
| `zenodo_benchmark.sh` | Evaluate `mrs-proover` (optionally Nörgler, `--with-norgler`) on the Zenodo benchmark; checks the original→never-VerifiedBad / falsified→never-VerifiedGood invariants |
| `norgler_compare.sh` | Compare `mrs-proover` vs Nörgler on the committed deterministic corpus |
| `audit_casc_proofs` | Replay archived CASC prover output through strict, MRS-only, and full-ladder checks without rerunning MRS |
| `perf_probe` | The measurement tool behind `perf_probe.sh`: run a fixed-work search and emit a JSON row (`measure`), or append rows to the bank and render a report (`bank`) |

## Quick start

```bash
# 1. Build mrs in release mode
cargo build --release

# 2. Download problems and axioms (~500 MB)
crates/mrs-bench/setup.sh

# 3. (Optional) Add a competitor binary, e.g. Vampire
cp /path/to/vampire crates/mrs-bench/systems/vampire/bin/vampire

# 4. Run the benchmark (12 s per problem, all default divisions)
crates/mrs-bench/casc.sh --systems mrs,vampire --time 12  # omit ,vampire if not installed

# 5. Summarise the latest run
cargo run -p mrs-bench --bin bench_report -- crates/mrs-bench/results/casc-30/<timestamp>/run.csv
```

## `bench_report` binary

```
bench_report <run.csv> [--min-systems <N>]
```

Reads a `run.csv` produced by `casc.sh` and prints:
- Per-division solved count and average solve time per system
- Cross-system disagreements (contradictory SZS answers — soundness flag)
- Polarity violations (wrong SZS polarity for a known-polarity division)

## Portfolio Coverage

`run_strategy_sweep.sh` and `run_codex_sweep.sh` measure individual strategies
with one worker. Their union/set-cover output is diagnostic only: it does not
model the cross-strategy unit-equality pool used by `casc_*` schedules.

Measure the actual cooperative portfolio instead:

```bash
MRS_WORKERS=8 MRS_SHARED_POOL_INTERVAL=500 \
crates/mrs-bench/cooperative_portfolio_sweep.sh \
  casc-30 feq 11,12,1,6,10,8,14,4 30 4 \
  results/cooperative-feq

# Control run with sharing disabled:
MRS_WORKERS=8 MRS_SHARED_POOL_INTERVAL=0 \
crates/mrs-bench/cooperative_portfolio_sweep.sh \
  casc-30 feq 11,12,1,6,10,8,14,4 30 4 \
  results/cooperative-feq-no-sharing
```

The cooperative result is the portfolio-selection objective. Compare the
shared and no-sharing runs to quantify cooperation separately from strategy
diversity. `failure_detail` records `strategy_ids`, `shared_published`, and
`shared_imported` telemetry for each problem.

## Pre-pass bound probes

`cd_bound_probe.sh` measures a pre-pass in isolation, at its production bounds and
at bounds raised enough to tell "the closure cannot close this fragment" apart
from "the bound stopped it first". It runs the pre-pass alone over one cluster,
sequentially, and reads the pre-pass's own `% SZS detail condensed_detachment=`
line rather than the SZS status — a status of `Theorem` with
`stop!=refutation` is the portfolio's work, not the pre-pass's.

```bash
# Production bounds vs 60x the wall budget and 20x each structural bound.
crates/mrs-bench/cd_bound_probe.sh --div fne --edition casc-30 --time 60 \
    --max-facts 100000 --max-inferences 2000000
```

Any refutation is put through `mrs-proover --strict` before it is counted, and
the summary separates the stop-reason distribution from the solve count. See
`docs/research/condensed-detachment.md` for the FNE result and how to read it.

## Deferred CASC proof auditing

`casc.sh` preserves every prover stream below the run directory:

```text
raw/<system>/<division>/<problem>.stdout
raw/<system>/<division>/<problem>.stderr
```

The CSV records those paths and SHA-256 hashes. Use the archived output later
without contaminating CASC generation timing:

```bash
cargo run --release -p mrs-bench --bin audit_casc_proofs -- \
  --run crates/mrs-bench/results/casc-30/<run> \
  --problems-dir crates/mrs-bench/problems/casc-30 \
  --checks strict,mrs,ladder \
  --strict-time 30 --mrs-time 10 --ladder-time 30 \
  --ladder-workers 8 --jobs 1 \
  --output crates/mrs-bench/results/casc-30/<run>/proof-audit
```

`--checks` accepts any non-empty comma-separated subset of `strict`, `mrs`,
and `ladder`. The audit checks all selected policies against one normalized
proof and writes `proof-audit/audit.csv`. It never calls MRS. The report is
resumable, writes `proof-audit/audit-summary.txt` as a per-division ASCII
console report, and can be imported after the original CASC results:

```bash
cargo run --release -p mrs-codex -- \
  --db codex-casc30.db \
  --import-proof-audit crates/mrs-bench/results/casc-30/<run>/proof-audit/audit.csv
```

## Adding a new system

Create `crates/mrs-bench/systems/<name>/invoke.sh` with this interface:

```bash
# Usage: invoke.sh <problem_path> <time_limit_secs>
# Must print "% SZS status <Status> for <problem>" to stdout.
```

`casc.sh` auto-discovers all directories under `systems/` that contain an executable `invoke.sh`.

## Generating a proof corpus for `mrs-proover`

`fuzz_proover.sh` runs a proof generator (eprover or vampire) on every problem in a directory, then verifies each resulting proof with `mrs-proover`. Designed to scale from the tiny in-tree `problems/` directory to the full TPTP-v9 FOF library on a multi-core machine.

```bash
# Smoke test on the in-tree problems/ directory:
crates/mrs-bench/fuzz_proover.sh --jobs 8

# Full TPTP-v9 FOF library, 64 workers, with eprover:
crates/mrs-bench/fuzz_proover.sh \
    --problems-dir /data/TPTP-v9.0.0/Problems \
    --generator eprover --jobs 64 --time 30 \
    --output /data/proover-corpus-eprover

# Same with vampire:
crates/mrs-bench/fuzz_proover.sh \
    --problems-dir /data/TPTP-v9.0.0/Problems \
    --generator vampire --jobs 64 --time 30 \
    --output /data/proover-corpus-vampire
```

When `--problems-dir` is overridden, the default `--pattern` becomes `*+*.p` (TPTP's filename convention for FOF problems). Override with `--pattern '*.p'` for a flat directory of any-dialect problems.

The script writes a `run.csv` plus prints two summary tables at the end: top unhandled inference rules (`Unknown` rows) and top recurring `VerifiedBad` reasons. Both are the highest-leverage signals for prioritising verifier work.

## Reproducible ProoVer 2026 corpus

The committed `proover-corpus/Proover2026/` directory contains exactly 100
official PRV fixtures. Its `manifest.tsv` records the valid/evil classification
and scoring policy, `metadata.toml` records corpus/toolchain/reproduction
metadata, and `SHA256SUMS` covers all corpus and metadata files.

Normalize or refresh it with the parser-backed Rust tool, then validate it offline:

```bash
nix develop -c cargo run -p mrs-bench --bin normalize_proover2026 -- \
  crates/mrs-bench/proover-corpus/Proover2026 --restore-sources --clean-source
nix develop -c cargo run -p mrs-bench --bin validate_proover2026 -- \
  crates/mrs-bench/proover-corpus/Proover2026
```

The evaluator consumes that manifest rather than maintaining a second hardcoded
classification list:

```bash
nix develop -c cargo run -p mrs-bench --bin score_proover2026 -- \
  crates/mrs-bench/proover-corpus/Proover2026 \
  --proover target/release/mrs-proover \
  --output results/proover-2026.tsv
```

The Rust audit runner records separate MRS and verifier timings and exits with
distinct codes: `1` infrastructure failure, `2` confirmed bad proof, `3`
unknown/timeout, and `4` parse error.

```bash
nix develop -c cargo run -p mrs-bench --bin audit_proover -- \
  --list exhaustive_fof_non_theorems.list \
  --tptp "$TPTP" \
  --mrs target/release/mrs \
  --proover target/release/mrs-proover \
  --output reports/soundness-audit.csv
```
