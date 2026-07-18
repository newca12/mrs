#!/usr/bin/env bash
# run_soundness_audit.sh
#
# Iterates over a list of officially-certified FOF TPTP non-theorems (status
# Satisfiable or CounterSatisfiable) and verifies that the `mrs` theorem prover
# never soundness-fails (i.e. never outputs Theorem/Unsatisfiable refutations).

set -euo pipefail

LIST_FILE="fof_non_theorems.list"
MRS_BIN="./target/release/mrs"
TIMEOUT_SECS=5

# ANSI Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
NC='\033[0m'

# Resolve TPTP directory dynamically
TPTP_DIR="${TPTP:-}"

if [[ -z "${TPTP_DIR}" ]]; then
    # Try default known paths on server97, server03, etc.
    default_paths=(
        "/mnt/fastdata/TPTP-v9.2.1"
        "/DATA/ai/user/TPTP-v9.2.1"
        "/mnt/sdd/TPTP-v9.2.1"
        "/mnt/sda1/TPTP-v9.2.1"
    )
    for path in "${default_paths[@]}"; do
        if [[ -d "${path}" ]]; then
            TPTP_DIR="${path}"
            break
        fi
    done
fi

if [[ -z "${TPTP_DIR}" ]]; then
    echo -e "${RED}Error:${NC} \$TPTP environment variable is not set and no default TPTP library was found." >&2
    echo -e "Please export \$TPTP before running this script, e.g.:" >&2
    echo -e "  export TPTP=/path/to/TPTP-v9.2.1" >&2
    exit 1
fi

if [[ ! -f "${LIST_FILE}" ]]; then
    echo "Error: '${LIST_FILE}' not found. Please run the scanner first." >&2
    exit 1
fi

if [[ ! -f "${MRS_BIN}" ]]; then
    echo "Warning: '${MRS_BIN}' not found. Attempting to build 'mrs' in release mode..."
    cargo build --release --workspace
fi

echo -e "${CYAN}=== Starting MRS Soundness Audit ===${NC}"
echo -e "Control Corpus: ${YELLOW}${LIST_FILE}${NC}"
echo -e "TPTP Library  : ${YELLOW}${TPTP_DIR}${NC}"
echo -e "Prover Binary : ${YELLOW}${MRS_BIN}${NC}"
echo -e "Workers Count : ${YELLOW}${MRS_WORKERS:-8}${NC}"
echo -e "Time Limit    : ${YELLOW}${TIMEOUT_SECS}s${NC} per problem"
echo "--------------------------------------------------------"

total=0
passed=0
failed=0
skipped=0

while read -r problem || [[ -n "${problem}" ]]; do
    # Skip empty lines or comments
    [[ -z "${problem}" || "${problem}" =~ ^# ]] && continue
    
    total=$((total + 1))
    
    # Resolve absolute path using dynamically determined TPTP directory
    full_path="${TPTP_DIR}/${problem}"
    
    if [[ ! -f "${full_path}" ]]; then
        echo -e "[${YELLOW}SKIP${NC}] ${problem} (File not found at ${full_path})"
        skipped=$((skipped + 1))
        continue
    fi
    
    # Run mrs and extract the SZS status
    # We redirect stderr to ignore warnings/logs and only capture stdout
    # Wrap in OS-level timeout as a safety guard against solver hangs or deep demodulation deadlocks
    status_line=$(timeout "$(( TIMEOUT_SECS + 3 ))" "${MRS_BIN}" --time "${TIMEOUT_SECS}" --workers "${MRS_WORKERS:-8}" --auto-schedule "${full_path}" 2>/dev/null | grep -E "% SZS status" || true)
    
    if [[ -z "${status_line}" ]]; then
        # If no status line is printed, it usually timed out or exited without a decision
        echo -e "[${GREEN}OK${NC}] $(basename "${problem}") -> No SZS decision (Timeout/GaveUp)"
        passed=$((passed + 1))
    elif [[ "${status_line}" =~ "Theorem" ]] || [[ "${status_line}" =~ "Unsatisfiable" ]]; then
        echo -e "[${RED}FAIL${NC}] $(basename "${problem}") -> ${RED}UNSOUND REFUTATION!${NC} (${status_line})"
        failed=$((failed + 1))
    else
        echo -e "[${GREEN}OK${NC}] $(basename "${problem}") -> ${GREEN}${status_line}${NC}"
        passed=$((passed + 1))
    fi
done < "${LIST_FILE}"

echo "--------------------------------------------------------"
echo -e "${CYAN}Audit Finished!${NC}"
echo -e "  * Total Evaluated : ${total}"
echo -e "  * Passed (Sound)  : ${GREEN}${passed}${NC}"
if (( skipped > 0 )); then
    echo -e "  * Skipped (Missing): ${YELLOW}${skipped}${NC}"
fi
if (( failed > 0 )); then
    echo -e "  * ${RED}Failed (Unsound) : ${failed}${NC} ⚠"
    exit 2
else
    echo -e "  * Failed (Unsound) : ${GREEN}0${NC}"
    echo -e "${GREEN}SUCCESS: mrs is 100% sound over all evaluated control problems!${NC}"
fi
