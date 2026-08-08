#!/usr/bin/env bash
# crates/mrs-bench/distribute_ml_data.sh
#
# Distributes ML feature data collection across multiple servers.
# It assumes:
#  1. The mrs repository is cloned at the exact same path on all servers.
#  2. The TPTP directory is available at the exact same path on all servers.
#  3. You have passwordless SSH access to all servers.
#
# Usage:
#   crates/mrs-bench/distribute_ml_data.sh <SERVERS_FILE> <TPTP_DIR> <OUT_DIR> [JOBS_PER_SERVER] [TIME_LIMIT] [MRS_WORKERS]
#
# Example:
#   crates/mrs-bench/distribute_ml_data.sh servers.txt /path/to/TPTP-v9.2.1 ./ml_logs_cluster 16 300 1

set -euo pipefail

if [ "$#" -lt 3 ]; then
    echo "Usage: $0 <SERVERS_FILE> <TPTP_DIR> <OUT_DIR> [JOBS_PER_SERVER] [TIME_LIMIT] [MRS_WORKERS]"
    echo "Example: $0 servers.txt /path/to/TPTP-v9.2.1 ./ml_logs_cluster 16 300 1"
    exit 1
fi

SERVERS_FILE="$1"
TPTP_DIR="$2"
OUT_DIR="$3"
JOBS_PER_SERVER="${4:-1}"
TIME_LIMIT="${5:-30}"
MRS_WORKERS="${6:-1}"

if [ ! -f "$SERVERS_FILE" ]; then
    echo "Error: SERVERS_FILE '$SERVERS_FILE' does not exist."
    exit 1
fi

# Read non-empty lines from servers file
mapfile -t SERVERS < <(grep -v '^[[:space:]]*$' "$SERVERS_FILE")
NUM_SERVERS=${#SERVERS[@]}

if [ "$NUM_SERVERS" -eq 0 ]; then
    echo "Error: No servers found in $SERVERS_FILE."
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

mkdir -p "$OUT_DIR"

echo "Finding all problems in $TPTP_DIR/Problems ..."
if [ ! -d "$TPTP_DIR/Problems" ]; then
    echo "Error: $TPTP_DIR/Problems does not exist."
    exit 1
fi
FULL_LIST="$OUT_DIR/full_problems.list"
find -L "$TPTP_DIR/Problems" -name "*.p" > "$FULL_LIST"

NUM_PROBS=$(wc -l < "$FULL_LIST")
echo "Found $NUM_PROBS problems. Splitting across $NUM_SERVERS servers..."

# Split the problems list into N chunks
split -n l/"$NUM_SERVERS" -d "$FULL_LIST" "$OUT_DIR/problems_chunk_"

# Array to keep track of background PIDs
declare -a PIDS=()

echo "Starting distributed collection..."

for i in "${!SERVERS[@]}"; do
    SERVER="${SERVERS[$i]}"
    # split generates suffixes like 00, 01, 02...
    SUFFIX=$(printf "%02d" "$i")
    CHUNK_FILE="$OUT_DIR/problems_chunk_$SUFFIX"
    
    echo "Dispatching $(wc -l < "$CHUNK_FILE") problems to $SERVER..."
    
    # 1. Copy the chunk to the remote server
    # 2. Run the collection script on the remote server
    (
        scp "$CHUNK_FILE" "$SERVER:$WORKSPACE_ROOT/problems_chunk.list" >/dev/null
        ssh "$SERVER" "cd $WORKSPACE_ROOT && INPUT_PROBLEMS_LIST=$WORKSPACE_ROOT/problems_chunk.list ./crates/mrs-bench/collect_ml_data.sh $TPTP_DIR $OUT_DIR $JOBS_PER_SERVER $TIME_LIMIT $MRS_WORKERS" > "$OUT_DIR/server_${SERVER}_log.txt" 2>&1
        echo "Finished on $SERVER. Fetching logs..."
        
        # 3. Rsync the generated data back
        mkdir -p "$OUT_DIR/data"
        rsync -avz "$SERVER:$WORKSPACE_ROOT/$OUT_DIR/data/" "$OUT_DIR/data/" >/dev/null
        
        # 4. Cleanup remote chunk
        ssh "$SERVER" "rm -f $WORKSPACE_ROOT/problems_chunk.list"
    ) &
    PIDS+=($!)
done

echo "Waiting for all servers to complete... You can tail the individual server logs in $OUT_DIR/server_HOSTNAME_log.txt"

# Wait for all background jobs to finish
FAIL=0
for pid in "${PIDS[@]}"; do
    wait "$pid" || FAIL=1
done

if [ "$FAIL" -eq 0 ]; then
    NUM_LOGS=$(find "$OUT_DIR/data" -type f -name "*.wincode" | wc -l || echo 0)
    echo "All distributed collections finished successfully!"
    echo "A total of $NUM_LOGS .wincode log files have been aggregated into $OUT_DIR/data."
else
    echo "Warning: One or more server jobs failed. Please check the logs in $OUT_DIR for details."
fi

# Clean up local chunks
rm -f "$OUT_DIR"/problems_chunk_*
