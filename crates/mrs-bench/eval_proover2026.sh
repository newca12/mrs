#!/usr/bin/env bash
set -euo pipefail

# Resolve workspace root dynamically relative to this script
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/../.."

echo "Building mrs-proover release..."
cargo build --release -p mrs-proover >/dev/null 2>&1

echo "Linking binary to starexec directory..."
ln -sf "$(pwd)/target/release/mrs-proover" crates/mrs-bench/systems/mrs-proover/mrs-proover

export STAREXEC_WALLCLOCK_LIMIT=10
export RUST_MIN_STACK=67108864

GOOD=0
BAD=0
UNKNOWN=0
UNSOUND=0
REJECT=0
TOTAL=0

echo "Running starexec_run_default on crates/mrs-bench/proover-corpus/Proover2026/ ..."
for f in crates/mrs-bench/proover-corpus/Proover2026/*.p; do
    # Extract filename for display
    name=$(basename "$f")
    
    # Run the starexec script
    output=$(./crates/mrs-bench/systems/mrs-proover/starexec_run_default "$f" 2>&1 || true)
    
    # Get the SZS status
    szs=$(echo "$output" | grep -m1 "% SZS status" | awk '{print $4}' || true)
    
    TOTAL=$((TOTAL+1))
    
    case "$szs" in
        "VerifiedGood")
            GOOD=$((GOOD+1))
            ;;
        "VerifiedBad")
            BAD=$((BAD+1))
            ;;
        "Unknown")
            UNKNOWN=$((UNKNOWN+1))
            ;;
        "Unsound")
            UNSOUND=$((UNSOUND+1))
            ;;
        *)
            echo "Unexpected SZS for $name: $szs (Output: $output)"
            ;;
    esac
done

echo "=== Proover2026 Results ==="
echo "Total Problems: $TOTAL"
echo "VerifiedGood: $GOOD"
echo "VerifiedBad:  $BAD"
echo "Unknown:      $UNKNOWN"
echo "Unsound:      $UNSOUND"
