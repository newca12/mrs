#!/usr/bin/env bash
# bench/systems/vampire/invoke.sh
# Usage: invoke.sh <problem_path> <time_limit_secs>
set -euo pipefail

PROBLEM="${1:?Usage: invoke.sh <problem_path> <time_limit_secs>}"
TIME_LIMIT="${2:?Usage: invoke.sh <problem_path> <time_limit_secs>}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# 1. Detect input language dialect (matching official egrep logic)
INPUT_LANGUAGE=$(egrep -om1 "^(thf|tff|tcf|fof|cnf)" "${PROBLEM}" || echo "cnf")

# Default to first-order vampire
BINARY="${SCRIPT_DIR}/bin/vampire"

# 2. Binary selection (falling back gracefully to standard vampire if vampire-ho isn't bundled)
if [[ "${INPUT_LANGUAGE}" == "thf" ]]; then
    if [[ -x "${SCRIPT_DIR}/bin/vampire-ho" ]]; then
        BINARY="${SCRIPT_DIR}/bin/vampire-ho"
    fi
fi

if [[ ! -x "${BINARY}" ]]; then
    echo "% SZS status Error (vampire binary not found at ${BINARY})"
    exit 1
fi

# 3. Execute with official Vampire CASC parameters:
# -m 16384   (16GB RAM limit)
# --cores 7  (parallel search scaling limit)
# -t         (time limit in seconds)
exec "${BINARY}" --input_syntax tptp --mode casc -m 16384 --cores 7 -t "${TIME_LIMIT}" "${PROBLEM}"
