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

PREMISE_WEIGHTS="${WORKSPACE_ROOT}/models/weights_premise_${DIV_LOWER}.bin"
SCHEDULE_WEIGHTS="${WORKSPACE_ROOT}/models/weights_schedule_${DIV_LOWER}.bin"

ARGS=(--time "${TIME_LIMIT}" --workers "${MRS_WORKERS:-8}" --ml-schedule)

if [[ -f "${SCHEDULE_WEIGHTS}" ]]; then
    ARGS+=(--ml-schedule-weights "${SCHEDULE_WEIGHTS}")
else
    echo "% SZS status Error (mrs-ml: expected schedule model weights not found at ${SCHEDULE_WEIGHTS})" >&2
    exit 1
fi

if [[ -f "${PREMISE_WEIGHTS}" ]]; then
    if [[ "${DIV_LOWER}" == "eps" ]]; then
        # Satisfiable division: NO pruning allowed to preserve soundness of saturation!
        echo "Satisfiable division (eps): skipping premise selection to preserve soundness of saturation." >&2
    elif [[ "${DIV_LOWER}" == "fne" ]]; then
        ARGS+=(--ml-prune 0.85 --ml-premise-weights "${PREMISE_WEIGHTS}")
    else
        ARGS+=(--ml-prune 0.6 --ml-premise-weights "${PREMISE_WEIGHTS}")
    fi
else
    echo "Warning: No premise selection weights found for ${DIV_LOWER} at ${PREMISE_WEIGHTS}. Skipping axiom pruning." >&2
fi

exec "${BINARY}" "${ARGS[@]}" "${PROBLEM}"
