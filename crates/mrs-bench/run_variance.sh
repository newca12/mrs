#!/usr/bin/env bash
# Replicated benchmark runs, for measuring run-to-run variance.
#
# Usage:
#   run_variance.sh <reps> <casc-args...>
#
# Examples:
#   crates/mrs-bench/run_variance.sh 5 --edition casc-30 --systems mrs \
#       --divisions ueq --casc-times --jobs 1 \
#       --subset crates/mrs-bench/results/ueq-variance/subset.txt \
#       --output crates/mrs-bench/results/variance-ueq
#
# Why this exists
# ---------------
# A single run is not a measurement. On a fixed 25-problem casc-30 UEQ subset,
# five replicates of an identical configuration solved 21 / 20 / 19 / 21 / 20 —
# mean 20.2, range 19-21, with nothing changed between runs. That is +-1 per 25,
# which scales to **+-12 per 300**: the same size as any portfolio difference the
# A/B tooling is asked to detect.
#
# Two consequences for everything else in this directory:
#
#   * `cooperative_portfolio_sweep.sh` is one `casc.sh` exec with no replicates,
#     so it scores every candidate from a single observation.
#   * `cooperative_portfolio_search.sh` maximises over ~120 such observations per
#     round (rounds x 8 slots x 15 candidates), which selects on noise.
#   * `greedy_set_cover` builds each strategy's solved-set from one `run.csv`.
#
# Against that noise floor, prefer a mean over >=5 replicates to a
# single number. See `docs/policies/unresolved-issues.md` UI-8.
#
# Method
# ------
# Replicates are full casc.sh campaigns and therefore run sequentially (blocked).
# This measures within-configuration variance, but does not protect a comparison
# between configurations from host drift; use paired/interleaved runs for that.
#
# Variance on this corpus is bimodal per problem rather than a uniform jitter --
# `KLE152-10` takes ~118 s or ~217 s with nothing in between, while `GRP423-1` is
# stable to +-0.2% -- because concurrent strategies race and CPU contention picks
# the winner. That is why per-problem spread, not just the total, is reported.
#
# `casc.sh` refuses to write into a non-empty output directory, so each replicate
# gets its own and nothing can be overwritten.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPS="${1:?usage: $0 <reps> <casc-args...>}"
shift || true

if ! [[ "${REPS}" =~ ^[1-9][0-9]*$ ]]; then
  echo "Error: reps must be a positive integer, got '${REPS}'." >&2
  exit 2
fi

if [[ $# -eq 0 ]]; then
  echo "Error: no casc.sh arguments given. Usage: $0 <reps> <casc-args...>" >&2
  exit 2
fi

# Find --output, or choose a parent for the per-replicate directories.
OUTPUT=""
prev=""
for arg in "$@"; do
  if [[ "${prev}" == "--output" ]]; then
    OUTPUT="${arg}"
  fi
  prev="${arg}"
done

if [[ -z "${OUTPUT}" ]]; then
  PARENT="${SCRIPT_DIR}/results/variance-$(date +%Y%m%d_%H%M%S)"
  mkdir -p "${PARENT}"
else
  PARENT="${OUTPUT}"
  # casc.sh owns --output per replicate; strip it from the forwarded args.
  forwarded=()
  skip_next=0
  for arg in "$@"; do
    if (( skip_next )); then
      skip_next=0
      continue
    fi
    if [[ "${arg}" == "--output" ]]; then
      skip_next=1
      continue
    fi
    forwarded+=("${arg}")
  done
  set -- "${forwarded[@]}"
fi

echo "[variance] reps=${REPS} parent=${PARENT}" >&2

for r in $(seq 1 "${REPS}"); do
  rep_dir="${PARENT}/rep${r}"
  if [[ -e "${rep_dir}" ]]; then
    echo "[variance] refusing to overwrite existing ${rep_dir}" >&2
    exit 2
  fi
  mkdir -p "${rep_dir}"
  echo "[variance] === rep ${r}/${REPS} -> ${rep_dir}" >&2
  "${SCRIPT_DIR}/casc.sh" "$@" --output "${rep_dir}"
done

# ---------------------------------------------------------------------------
# Per-problem spread. Uses run.csv only; no search runs again.
# ---------------------------------------------------------------------------
python3 - "${PARENT}" "${REPS}" <<'PY'
import csv
import pathlib
import sys
from collections import defaultdict

parent = pathlib.Path(sys.argv[1])
reps = int(sys.argv[2])

rows = defaultdict(dict)
systems = set()
for r in range(1, reps + 1):
    csv_path = parent / f"rep{r}" / "run.csv"
    if not csv_path.is_file():
        sys.exit(f"variance: missing {csv_path}")
    with csv_path.open(newline="") as fh:
        for row in csv.DictReader(fh):
            key = (row["division"], row["system"], row["problem"])
            if r in rows[key]:
                sys.exit(f"variance: duplicate row for {key} in rep{r}")
            rows[key][r] = row
            systems.add(row["system"])

def solved(row):
    """A correctly graded solve, not a wrong or ungraded definitive answer."""
    return row["verdict"] == "ok"

print(f"\n{'division':10} {'system':16} {'problem':16} {'solved':>12} {'verdicts'}")
print("-" * 82)
unstable = []
for key in sorted(rows):
    per_rep = rows[key]
    tally = sorted(r for r, row in per_rep.items() if solved(row))
    marks = "".join("." if r in tally else "x" for r in range(1, reps + 1))
    outcomes = {
        (per_rep[r]["szs_status"], per_rep[r]["verdict"])
        for r in per_rep
    }
    print(f"{key[0]:10} {key[1]:16} {key[2]:16} {marks:>12}   "
          f"{'/'.join(f'{status}:{verdict}' for status, verdict in sorted(outcomes))}")
    # Unstable = the grading outcome differs between replicates. These are the
    # problem/system pairs that a single run cannot speak for.
    if len(outcomes) > 1 or len(per_rep) != reps:
        unstable.append(key)

totals_by_system = {
    system: [
        sum(
            1 for (_, row_system, _), per_rep in rows.items()
            if row_system == system and r in per_rep and solved(per_rep[r])
        )
        for r in range(1, reps + 1)
    ]
    for system in sorted(systems)
}
print("-" * 82)
for system, totals in totals_by_system.items():
    print(f"{system} solved per replicate: {totals}")
    if totals:
        print(f"  mean {sum(totals)/len(totals):.2f}  min {min(totals)}  max {max(totals)}"
              f"  spread {max(totals)-min(totals)}")

if unstable:
    print(f"\n{len(unstable)} problem(s) changed verdict between replicates:")
    for key in unstable:
        print(f"  {key[0]}/{key[1]}/{key[2]}")
    print("\nA single-run A/B cannot separate these from a real effect. Report means.")
else:
    print("\nno problem changed verdict across replicates.")

print(f"\nreplicate runs under {parent}")
PY
