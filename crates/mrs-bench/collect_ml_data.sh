#!/usr/bin/env bash
# crates/mrs-bench/collect_ml_data.sh
#
# Massively collects ML feature data across a full TPTP release directory.
#
# Usage:
#   crates/mrs-bench/collect_ml_data.sh <TPTP_DIR> <OUT_DIR> [JOBS] [TIME_LIMIT] [MRS_WORKERS]
#
# Example:
#   crates/mrs-bench/collect_ml_data.sh /path/to/TPTP-v9.2.1 ./ml_logs 8 30 1

set -euo pipefail

if [ "$#" -lt 2 ]; then
    echo "Usage: $0 <TPTP_DIR> <OUT_DIR> [JOBS] [TIME_LIMIT] [MRS_WORKERS]"
    echo "Example: $0 /path/to/TPTP-v9.2.1 ./ml_logs 8 30 1"
    exit 1
fi

TPTP_DIR="$1"
OUT_DIR="$2"
JOBS="${3:-1}"
TIME_LIMIT="${4:-30}"
MRS_WORKERS="${5:-1}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

mkdir -p "$OUT_DIR"

cd "$WORKSPACE_ROOT"
echo "Building prover with 'ml' feature..."
cargo build --release --features ml

if [ -n "${INPUT_PROBLEMS_LIST:-}" ] && [ -f "$INPUT_PROBLEMS_LIST" ]; then
    echo "Using provided problem list: $INPUT_PROBLEMS_LIST"
    cp "$INPUT_PROBLEMS_LIST" "$OUT_DIR/problems.list"
else
    echo "Finding problems in $TPTP_DIR/Problems ..."
    if [ ! -d "$TPTP_DIR/Problems" ]; then
        echo "Error: $TPTP_DIR/Problems does not exist."
        echo "Make sure you point to the root of a TPTP installation (e.g., TPTP-v9.2.1)."
        exit 1
    fi
    find "$TPTP_DIR/Problems" -name "*.p" > "$OUT_DIR/problems.list"
fi

NUM_PROBS=$(wc -l < "$OUT_DIR/problems.list")
echo "Found $NUM_PROBS problems."

# Export TPTP environment variable for include directives
export TPTP="$TPTP_DIR"

if [ "$TIME_LIMIT" = "auto" ] || [ "$TIME_LIMIT" = "casc" ]; then
    echo "Running data collection with $JOBS parallel jobs, $MRS_WORKERS threads per problem (Time limit: Division-Specific Auto-Scaling)..."
else
    echo "Running data collection with $JOBS parallel jobs, $MRS_WORKERS threads per problem (Time limit: ${TIME_LIMIT}s)..."
fi

export MRS_BIN="$WORKSPACE_ROOT/target/release/mrs"
export LOG_DIR="$OUT_DIR/data"
export WORKERS="$MRS_WORKERS"
mkdir -p "$LOG_DIR"

# Run xargs in parallel. We pipe stdout/stderr to /dev/null so the console isn't flooded,
# since the data we care about is written to the LOG_DIR by the prover.
cat "$OUT_DIR/problems.list" | xargs -P "$JOBS" -n 1 -I {} bash -c '
    FILE="{}"
    PROB_NAME=$(basename "$FILE" .p)
    # The division is typically the parent directory name, e.g. FEQ
    DIVISION=$(basename $(dirname "$FILE"))
    DIV_LOWER=${DIVISION,,}
    
    export PROBLEM_NAME="$PROB_NAME"
    SPECIFIC_LOG_DIR="$LOG_DIR/$DIV_LOWER"
    mkdir -p "$SPECIFIC_LOG_DIR"
    
    # Select the optimal static schedule for this division
    SCHEDULE="casc_${DIV_LOWER}"
    if [[ "$SCHEDULE" != "casc_feq" && "$SCHEDULE" != "casc_fne" && "$SCHEDULE" != "casc_ueq" && "$SCHEDULE" != "casc_epr" ]]; then
        SCHEDULE="casc"
    fi
    
    LIMIT="'"$TIME_LIMIT"'"
    if [[ "$LIMIT" == "auto" || "$LIMIT" == "casc" ]]; then
        case "$DIVISION" in
            FEQ) LIMIT=180 ;;
            FNE) LIMIT=120 ;;
            UEQ) LIMIT=60 ;;
            EPU|EPS|EPR) LIMIT=30 ;;
            *) LIMIT=60 ;;
        esac
    fi

    # timeout acts as a failsafe; mrs also has its own internal --time limit
    timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" >/dev/null 2>&1 || true
'

NUM_LOGS=$(find "$LOG_DIR" -type f -name "*.wincode" | wc -l || echo 0)
echo "Data collection complete."
echo "Generated $NUM_LOGS .wincode log files in $LOG_DIR."
