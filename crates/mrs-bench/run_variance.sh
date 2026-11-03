#!/usr/bin/env bash
# Replicated benchmark runs, for measuring run-to-run variance.
#
# Usage:
#   run_variance.sh <reps> <casc-args...>
#
# Examples:
#   crates/mrs-bench/run_variance.sh 2 --edition casc-30 --systems mrs \
#       --divisions ueq --casc-times --jobs 1 \
#       --subset crates/mrs-bench/results/ueq-variance/subset.txt \
#       --output crates/mrs-bench/results/variance-ueq
#
# Why this exists
# ---------------
# A single run is not a measurement. On a fixed 25-problem casc-30 UEQ subset,
# five replicates of an identical configuration solved 21 / 20 / 19 / 21 / 20 —
# mean 20.2, range 19-21, with nothing changed between runs. That is +-1 per 25,
# This demonstrates variability on those 25 selected problems. It does not
# establish a +-12 noise floor for 300 problems: that requires measuring the
# full evaluation set and accounting for correlations and problem selection.
#
# Two consequences for everything else in this directory:
#
#   * `cooperative_portfolio_sweep.sh` is one `casc.sh` exec with no replicates,
#     so it scores every candidate from a single observation.
#   * `cooperative_portfolio_search.sh` maximises over ~120 such observations per
#     round (rounds x 8 slots x 15 candidates), which selects on noise.
#   * `greedy_set_cover` builds each strategy's solved-set from one `run.csv`.
#
# Use at least two runs to screen within-configuration variation; add runs when
# outcomes are unstable or the decision is marginal. This script measures one
# configuration, not a paired A/B. See UI-8 in `docs/policies/unresolved-issues.md`.
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
source "${SCRIPT_DIR}/tools.sh"
REPS="${1:?usage: $0 <reps> <casc-args...>}"
shift || true

if ! [[ "${REPS}" =~ ^[1-9][0-9]*$ ]] || (( REPS < 2 )); then
  echo "Error: reps must be an integer >= 2, got '${REPS}'." >&2
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

mrs_tools --help >/dev/null

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
mrs_tools variance-report "${PARENT}" "${REPS}"
