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

## Quick start

```bash
# 1. Build mrs in release mode
cargo build --release

# 2. Download problems and axioms (~500 MB)
crates/mrs-bench/setup.sh

# 3. Run the benchmark (12 s per problem, all default divisions)
crates/mrs-bench/casc.sh --systems mrs --time 12

# 4. Summarise the latest run
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
