#!/usr/bin/env bash
# crates/mrs-bench/run_codex_sweep.sh
#
# Run a full strategy sweep (s01–s15) using mrs-codex to populate a database
# with deterministic, single-worker coverage data for greedy set-cover.
#
# Usage: ./crates/mrs-bench/run_codex_sweep.sh [TPTP_PATH] [DB_PATH] [TIMEOUT] [JOBS]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

TPTP_PATH="${1:-${TPTP:-}}"
if [[ -z "${TPTP_PATH}" || ! -d "${TPTP_PATH}" ]]; then
    echo "Error: Please provide a valid TPTP root or Problems directory as the first argument." >&2
    exit 1
fi

if [[ "$(basename "${TPTP_PATH%/}")" == "Problems" ]]; then
    PROBLEMS_DIR="${TPTP_PATH}"
    TPTP_ROOT="$(dirname "${TPTP_PATH}")"
else
    PROBLEMS_DIR="${TPTP_PATH}/Problems"
    TPTP_ROOT="${TPTP_PATH}"
fi
if [[ ! -d "${PROBLEMS_DIR}" ]]; then
    echo "Error: Problems directory not found at ${PROBLEMS_DIR}." >&2
    exit 1
fi

MRS_BINARY="${WORKSPACE_ROOT}/target/release/mrs"
if [[ ! -x "${MRS_BINARY}" ]]; then
    echo "Error: mrs release binary not found at ${MRS_BINARY}; build it first." >&2
    exit 1
fi

DB_PATH="${2:-codex_casc_remaining_sweep.db}"
TIMEOUT="${3:-300}"
JOBS="${4:-16}" # Default to 16 parallel files, each using 1 core (total 16 cores)

echo "================================================================================"
echo "Starting Strategy Sweep (s01–s15) via mrs-codex"
echo "TPTP Problems: ${PROBLEMS_DIR}"
echo "TPTP Root:     ${TPTP_ROOT}"
echo "Database:      ${DB_PATH}"
echo "Timeout:       ${TIMEOUT}s"
echo "Parallel Jobs: ${JOBS} (1 core per job)"
echo "================================================================================"

for i in $(seq 1 15); do
    # Format strategy index as s01..s15
    STRAT_NAME=$(printf "mrs-s%02d" "${i}")

    echo "--------------------------------------------------------------------------------"
    echo "[Sweep] Running strategy ${STRAT_NAME} (MRS_SINGLE_STRATEGY=${i}) ..."
    echo "--------------------------------------------------------------------------------"

    # Each mrs process runs one strategy on one worker. Combined with --jobs,
    # this runs independent strategy/problem jobs in parallel.
    if ! nix develop -c cargo run --manifest-path "${WORKSPACE_ROOT}/Cargo.toml" --release -p mrs-codex -- \
        "${PROBLEMS_DIR}" \
        --db "${DB_PATH}" \
        --system "${STRAT_NAME}" \
        --timeout "${TIMEOUT}" \
        --jobs "${JOBS}" \
        --cmd "env TPTP='${TPTP_ROOT}' MRS_SINGLE_STRATEGY=${i} '${MRS_BINARY}' --time {timeout} --workers 1 '{file}'" \
        > "codex_sweep_${STRAT_NAME}.out" 2> "codex_sweep_${STRAT_NAME}.err"; then
            echo "Warning: Sweep for ${STRAT_NAME} exited with a non-zero status. Check codex_sweep_${STRAT_NAME}.err for details."
    fi

    echo "[Sweep] Completed ${STRAT_NAME}."
done

echo "================================================================================"
echo "Strategy Sweep Completed successfully!"
echo "Database populated: ${DB_PATH}"
echo "================================================================================"
