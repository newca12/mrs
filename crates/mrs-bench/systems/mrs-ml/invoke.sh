#!/usr/bin/env bash
# crates/mrs-bench/systems/mrs-ml/invoke.sh
# Usage: invoke.sh <problem_path> <time_limit_secs>
# Writes all output to stdout; exits with any code.
set -euo pipefail

PROBLEM="${1:?Usage: invoke.sh <problem_path> <time_limit_secs>}"
TIME_LIMIT="${2:?Usage: invoke.sh <problem_path> <time_limit_secs>}"

# Resolve workspace root: four levels above this script
# (crates/mrs-bench/systems/mrs-ml/ -> systems/ -> mrs-bench/ -> crates/ -> root)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../../../.." && pwd)"

BINARY="${WORKSPACE_ROOT}/target/release/mrs"
if [[ ! -x "${BINARY}" ]]; then
    echo "% SZS status Error (mrs binary not found; run: cargo build --release --features ml-guidance)"
    exit 1
fi

# Extract division from the problem path, e.g., .../Problems/FEQ/FEQ123+1.p -> FEQ
DIVISION=$(basename $(dirname "$PROBLEM"))
DIV_LOWER="${DIVISION,,}"

# EPS (EPR Satisfiable) and EPU (EPR Unsatisfiable) share the single EPR-trained
# model and the ml_epr schedule (no separate eps/epu models were trained).
case "${DIV_LOWER}" in
    eps|epu) MODEL_DIV="epr" ;;
    *)       MODEL_DIV="${DIV_LOWER}" ;;
esac

WEIGHTS="${WORKSPACE_ROOT}/models/weights_${MODEL_DIV}.bin"
SCHEDULE="ml_${MODEL_DIV}"

# Fallback to the generic ones if division-specific weights aren't generated yet
if [[ ! -f "${WEIGHTS}" ]]; then
    WEIGHTS="${WORKSPACE_ROOT}/models/weights.bin"
    SCHEDULE="ml"
fi

if [[ ! -f "${WEIGHTS}" ]]; then
    echo "% SZS status Error (ML weights not found at ${WEIGHTS}; run training first)"
    exit 1
fi

# Set TPTP root so %include directives resolve.
# Prefer an already-set TPTP env var; fall back to the CASC-30 extracted archive.
if [[ -z "${TPTP:-}" ]]; then
    export TPTP="${SCRIPT_DIR}/../../problems/casc-30"
fi

exec "${BINARY}" --time "${TIME_LIMIT}" --workers 8 --schedule "${SCHEDULE}" --ml-weights "${WEIGHTS}" "${PROBLEM}"
