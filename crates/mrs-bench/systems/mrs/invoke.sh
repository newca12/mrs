#!/usr/bin/env bash
# crates/mrs-bench/systems/mrs/invoke.sh
# Usage: invoke.sh <problem_path> <time_limit_secs>
# Writes all output to stdout; exits with any code.
set -euo pipefail

PROBLEM="${1:?Usage: invoke.sh <problem_path> <time_limit_secs>}"
TIME_LIMIT="${2:?Usage: invoke.sh <problem_path> <time_limit_secs>}"

# Resolve workspace root: four levels above this script
# (crates/mrs-bench/systems/mrs/ -> systems/ -> mrs-bench/ -> crates/ -> root)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../../../.." && pwd)"

BINARY="${WORKSPACE_ROOT}/target/release/mrs"
if [[ ! -x "${BINARY}" ]]; then
    echo "% SZS status Error (mrs binary not found; run: cargo build --release)"
    exit 1
fi

# Set TPTP root so %include directives resolve.
# Prefer an already-set TPTP env var; fall back to the CASC-30 extracted archive.
if [[ -z "${TPTP:-}" ]]; then
    export TPTP="${SCRIPT_DIR}/../../problems/casc-30"
fi

# Determine the CASC division from the file path
DIVISION=$(basename $(dirname "$PROBLEM"))
DIV_LOWER="${DIVISION,,}"

# Select the appropriate static schedule
SCHEDULE="casc_${DIV_LOWER}"

# Fallback to standard casc if we don't have a specialized schedule
if [[ "$SCHEDULE" != "casc_feq" && "$SCHEDULE" != "casc_fne" && "$SCHEDULE" != "casc_ueq" && "$SCHEDULE" != "casc_epr" ]]; then
    SCHEDULE="casc"
fi

exec "${BINARY}" --time "${TIME_LIMIT}" --workers "${MRS_WORKERS:-8}" --schedule "${SCHEDULE}" "${PROBLEM}"
