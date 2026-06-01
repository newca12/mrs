#!/usr/bin/env bash
# Runs mrs-proover against all evil proofs and reports the results.

set -euo pipefail

# Find project root (assumes script is in evil-proofs/scripts)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
EXPLOITS_DIR="$SCRIPT_DIR/../exploits"

# Build mrs-proover
echo "Building mrs-proover..."
cargo build --release -p mrs-proover

VERIFIER="$ROOT_DIR/target/release/mrs-proover"

echo "========================================="
echo "Running Evil Proofs against mrs-proover"
echo "========================================="

# Run each exploit
for EXPLOIT in "$EXPLOITS_DIR"/*; do
  if [ -d "$EXPLOIT" ]; then
    NAME=$(basename "$EXPLOIT")
    echo "Testing Exploit: $NAME"
    
    PROBLEM_DIR="$EXPLOIT"
    PROOF_FILE="$EXPLOIT/proof.p"
    
    if [ -f "$PROOF_FILE" ]; then
      set +e
      # Capture output and status
      OUTPUT=$("$VERIFIER" --problems-dir "$PROBLEM_DIR" "$PROOF_FILE" 2>&1)
      set -e
      
      # Determine if it bypassed (Verified) or got caught (FailedVerified / NotVerified)
      if echo "$OUTPUT" | grep -q "% SZS status Verified"; then
        echo -e "\033[0;31m[BYPASSED]\033[0m mrs-proover incorrectly verified this evil proof!"
      elif echo "$OUTPUT" | grep -q "% SZS status FailedVerified"; then
        echo -e "\033[0;32m[CAUGHT]\033[0m mrs-proover correctly rejected this evil proof (FailedVerified)."
        echo "Reason: $(echo "$OUTPUT" | grep "% SZS status" | sed 's/.*: //')"
      elif echo "$OUTPUT" | grep -q "% SZS status NotVerified"; then
        echo -e "\033[0;33m[UNKNOWN]\033[0m mrs-proover could not verify this evil proof (NotVerified)."
        echo "Reason: $(echo "$OUTPUT" | grep "% SZS status" | sed 's/.*: //')"
      else
        echo -e "\033[0;31m[ERROR]\033[0m Unexpected output from verifier:"
        echo "$OUTPUT"
      fi
    else
      echo "No proof.p found in $NAME"
    fi
    echo "-----------------------------------------"
  fi
done
