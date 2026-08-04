# mrs-bench

CASC benchmark harness and report tool for `mrs`.

## Contents

| Path | Purpose |
|------|---------|
| `casc.sh` | Run a full benchmark: invoke each system on each problem, collect SZS status and wall time, write `results/<edition>/*/run.csv` |
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
