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

TPTP="${1:-${TPTP:-}}"
if [[ -z "${TPTP}" || ! -d "${TPTP}" ]]; then
    echo "Error: Please provide a valid TPTP Problems directory as the first argument." >&2
    exit 1
fi

DB_PATH="${2:-codex_casc_remaining_sweep.db}"
TIMEOUT="${3:-300}"
JOBS="${4:-16}" # Default to 16 parallel files, each using 1 core (total 16 cores)

echo "================================================================================"
echo "Starting Strategy Sweep (s01–s15) via mrs-codex"
echo "TPTP Problems: ${TPTP}"
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
    
    # We pass MRS_WORKERS=1 to make each strategy run sequentially on a single core.
    # Combined with --jobs 16, this utilizes exactly 16 cores in total on the server.
    cargo run --release -p mrs-codex -- \
        "${TPTP}" \
        --db "${DB_PATH}" \
        --system "${STRAT_NAME}" \
        --timeout "${TIMEOUT}" \
        --jobs "${JOBS}" \
        --cmd "env MRS_SINGLE_STRATEGY=${i} MRS_WORKERS=1 ./crates/mrs-bench/systems/mrs/invoke.sh {file} {timeout}" \
        > "codex_sweep_${STRAT_NAME}.out" 2> "codex_sweep_${STRAT_NAME}.err" || {
            echo "Warning: Sweep for ${STRAT_NAME} exited with a non-zero status. Check codex_sweep_${STRAT_NAME}.err for details."
        }
        
    echo "[Sweep] Completed ${STRAT_NAME}."
done

echo "================================================================================"
echo "Strategy Sweep Completed successfully!"
echo "Database populated: ${DB_PATH}"
echo "================================================================================"
