#!/usr/bin/env bash
# crates/mrs-bench/run_all_greedy_sweeps.sh
#
# Generates the mathematically optimized strategy portfolios for all 
# divisions across all hardware sizes from 1 to 15 cores.
#
# Usage:
#   chmod +x crates/mrs-bench/run_all_greedy_sweeps.sh
#   ./crates/mrs-bench/run_all_greedy_sweeps.sh <PATH_TO_SWEEP_RUN_CSV>

set -euo pipefail

if [ "$#" -lt 1 ]; then
    echo "Usage: $0 <PATH_TO_SWEEP_RUN_CSV>"
    echo "Example: $0 crates/mrs-bench/results/casc-30/20260614_154651/run.csv"
    exit 1
fi

RUN_CSV="$1"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

GREEDY_BIN="${WORKSPACE_ROOT}/target/release/greedy_set_cover"

# Build the optimizer if it hasn't been built yet
if [ ! -f "$GREEDY_BIN" ]; then
    echo "Building greedy_set_cover..."
    cargo build --release -p mrs-bench --bin greedy_set_cover
fi

# All CASC-30 untyped first-order divisions
DIVISIONS=(fne feq ueq epu eps icu)

for div in "${DIVISIONS[@]}"; do
    echo "=========================================================="
    echo "  GENERATING OPTIMAL PORTFOLIOS FOR DIVISION: ${div^^}"
    echo "=========================================================="
    for i in {1..15}; do
        echo "--- Portfolio Size $i (Optimal team for $i cores) ---"
        "$GREEDY_BIN" "$RUN_CSV" "$i" --division "$div"
        echo ""
    done
done
