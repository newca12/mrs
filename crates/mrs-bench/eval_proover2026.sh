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

# Ground truth for CASC-J13 Valid (Angel) proofs (50 total)
is_valid_proof() {
    local num="$1"
    case "$num" in
        000|001|004|005|010|011|012|013|014|015|016|017|018|019|020|021|022|027|029|030|033|034|040|041|042|043|044|045|048|055|059|060|061|062|063|064|065|066|070|073|076|079|080|081|082|085|088|089|096|098)
            return 0  # True (Valid Proof)
            ;;
        *)
            return 1  # False (Evil Proof)
            ;;
    esac
}

# Ground truth for locally sound evil mutations (10 total)
is_locally_sound_evil_mutation() {
    local num="$1"
    case "$num" in
        031|046|047|049|050|052|053|058|092|093)
            return 0  # True
            ;;
        *)
            return 1  # False
            ;;
    esac
}

GOOD=0
BAD=0
UNKNOWN=0
UNSOUND=0
TOTAL=0
TOTAL_SCORE=0
non_100_list=()

echo "Running starexec_run_default on crates/mrs-bench/proover-corpus/Proover2026/ ..."
for f in crates/mrs-bench/proover-corpus/Proover2026/*.p; do
    # Extract filename for display
    name=$(basename "$f")
    num="${name:3:3}"
    
    # Run the starexec script
    output=$(./crates/mrs-bench/systems/mrs-proover/starexec_run_default "$f" 2>&1 || true)
    
    # Get the SZS status
    szs=$(echo "$output" | grep -m1 "% SZS status" | awk '{print $4}' || true)
    if [[ -z "$szs" ]]; then
        szs="Unknown"
    fi
    
    TOTAL=$((TOTAL+1))
    
    # Calculate score according to official ProoVer 2026 rules:
    if is_valid_proof "$num"; then
        type="Valid"
        expected="VerifiedGood"
        if [[ "$szs" == "VerifiedGood" ]]; then
            score=1
            GOOD=$((GOOD+1))
        elif [[ "$szs" == "VerifiedBad" ]]; then
            score=-1
            BAD=$((BAD+1))
        else
            score=0
            UNKNOWN=$((UNKNOWN+1))
        fi
        max_score=1
    else
        type="Evil"
        expected="VerifiedBad"
        if is_locally_sound_evil_mutation "$num"; then
            # Locally Sound Evil Mutations can be correctly verified as sound (+2 pts)
            # or correctly rejected as unsound (+2 pts)
            if [[ "$szs" == "VerifiedBad" ]]; then
                score=2
                BAD=$((BAD+1))
            elif [[ "$szs" == "VerifiedGood" ]]; then
                score=2
                GOOD=$((GOOD+1))
            else
                score=0
                UNKNOWN=$((UNKNOWN+1))
            fi
        else
            # Ordinary Evil Proofs must be rejected (+2 pts), else Unsound (-10 pts)
            if [[ "$szs" == "VerifiedBad" ]]; then
                score=2
                BAD=$((BAD+1))
            elif [[ "$szs" == "VerifiedGood" ]]; then
                score=-10
                GOOD=$((GOOD+1))
                UNSOUND=$((UNSOUND+1))
            else
                score=0
                UNKNOWN=$((UNKNOWN+1))
            fi
        fi
        max_score=2
    fi
    
    TOTAL_SCORE=$((TOTAL_SCORE + score))
    
    if [[ "$score" -ne "$max_score" ]]; then
        non_100_list+=("$name [$type] Expected: $expected, Actual: $szs -> Score: $score")
    fi
done

echo "=== Proover2026 Results ==="
echo "Total Problems: $TOTAL"
echo "VerifiedGood:   $GOOD"
echo "VerifiedBad:    $BAD"
echo "Unknown:        $UNKNOWN"
echo "Unsound:        $UNSOUND"
echo "Total Score:    $TOTAL_SCORE / 150"
echo ""

if [[ ${#non_100_list[@]} -gt 0 ]]; then
    echo "=== Problems Not Scoring 100% ==="
    for item in "${non_100_list[@]}"; do
        echo "  - $item"
    done
else
    echo "=== Perfect Score! All problems scored 100% ==="
fi
