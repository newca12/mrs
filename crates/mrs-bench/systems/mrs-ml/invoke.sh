#!/usr/bin/env bash
# crates/mrs-bench/systems/mrs-ml/invoke.sh
# Usage: invoke.sh <problem_path> <time_limit_secs>
# Writes all output to stdout; exits with any code.
set -euo pipefail

PROBLEM="${1:?Usage: invoke.sh <problem_path> <time_limit_secs>}"
TIME_LIMIT="${2:?Usage: invoke.sh <problem_path> <time_limit_secs>}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../../../.." && pwd)"

BINARY="${WORKSPACE_ROOT}/target/release/mrs"
if [[ ! -x "${BINARY}" ]]; then
    echo "% SZS status Error (mrs binary not found; run: cargo build --release --features ml)"
    exit 1
fi

if [[ -z "${TPTP:-}" ]]; then
    export TPTP="${SCRIPT_DIR}/../../problems/casc-30"
fi

DIVISION=$(basename $(dirname "$PROBLEM"))
DIV_LOWER="${DIVISION,,}"

# Route to the division portfolio directly from the problem path; the ML
# schedule classifier is frozen (degenerate majority-class model + label
# mismatch, see docs/BENCHMARKS.md) and intentionally NOT used here.
case "${DIV_LOWER}" in
    fne|feq|ueq|epr|eps|epu|icu) SCHEDULE="casc_${DIV_LOWER}" ;;
    *)                           SCHEDULE="casc" ;;
esac

ARGS=(--time "${TIME_LIMIT}" --workers "${MRS_WORKERS:-8}" --schedule "${SCHEDULE}")

# ML premise selection is applied PER WORKER inside the scheduler (a minority
# of strategies run on the pruned axiom set, the rest on the full problem), so
# it is sound in every division — including the satisfiable EPS division —
# and a single aggressive keep-ratio is safe everywhere.
PREMISE_WEIGHTS="${WORKSPACE_ROOT}/models/weights_premise_${DIV_LOWER}.bin"
if [[ -f "${PREMISE_WEIGHTS}" ]]; then
    ARGS+=(--ml-prune 0.6 --ml-premise-weights "${PREMISE_WEIGHTS}")
else
    echo "Warning: No premise selection weights found for ${DIV_LOWER} at ${PREMISE_WEIGHTS}. Skipping axiom pruning." >&2
fi

exec "${BINARY}" "${ARGS[@]}" "${PROBLEM}"
