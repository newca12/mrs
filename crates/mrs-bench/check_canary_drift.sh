#!/usr/bin/env bash
# crates/mrs-bench/check_canary_drift.sh
# Usage: check_canary_drift.sh <path_to_run.csv>
#
# Parses a benchmark run.csv and applies the CASC Division Canary Suite
# Methodology to verify if the run suffered from include-drift contamination.

set -euo pipefail

CSV="${1:-}"

if [[ -z "${CSV}" ]]; then
    echo "Usage: $0 <path_to_run.csv>" >&2
    exit 1
fi

if [[ ! -f "${CSV}" ]]; then
    echo "Error: File '${CSV}' not found." >&2
    exit 1
fi

# Define ANSI colors for beautiful reporting
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

echo -e "${CYAN}=== CASC Division Canary Audit Report ===${NC}"
echo -e "Target CSV: ${YELLOW}${CSV}${NC}"
echo "---------------------------------------------------------"

canaries_found=0
is_contaminated=0
reasons=()

# Helper function to extract a value from the details string
# Example: extract_val "lrs_discarded=123" "lrs_discarded"
extract_val() {
    local str="$1"
    local key="$2"
    if [[ "${str}" =~ ${key}=([0-9]+) ]]; then
        echo "${BASH_REMATCH[1]}"
    else
        echo "0"
    fi
}

# Fetch the first CSV row whose exact "problem" column (field 3) matches
# the given canary name. Using an exact-field match (rather than a bare
# substring grep) avoids false hits against unrelated problems that merely
# contain the canary name as a substring (e.g. ALG212-10 vs ALG212-100).
find_canary_line() {
    local name="$1"
    awk -F',' -v p="${name}" '$3 == p { print; exit }' "${CSV}"
}

# Check EPS Canary: HWC004-1
eps_line=$(find_canary_line "HWC004-1")
if [[ -n "${eps_line}" ]]; then
    canaries_found=$((canaries_found + 1))
    
    szs=$(echo "${eps_line}" | cut -d',' -f5)
    time_s=$(echo "${eps_line}" | cut -d',' -f8)
    details=$(echo "${eps_line}" | cut -d',' -f9)
    
    discarded=$(extract_val "${details}" "lrs_discarded")
    passive=$(extract_val "${details}" "passive")
    
    echo -n -e "  [EPS] Canary (HWC004-1)  : SZS=${szs} | Time=${time_s}s | passive=${passive} | lrs_discard=(Σ)${discarded} -> "
    
    if (( discarded > 1000 )) || [[ "${szs}" == "GaveUp" ]] || [[ "${szs}" == "Timeout" ]]; then
        echo -e "${RED}CONTAMINATED${NC}"
        is_contaminated=1
        reasons+=("EPS Canary HWC004-1 bloated with ${discarded} LRS discards (Axioms include-drift)")
    else
        echo -e "${GREEN}CLEAN${NC}"
    fi
fi

# Check FNE Canary: CSR026+3
fne_line=$(find_canary_line "CSR026+3")
if [[ -n "${fne_line}" ]]; then
    canaries_found=$((canaries_found + 1))
    
    szs=$(echo "${fne_line}" | cut -d',' -f5)
    time_s=$(echo "${fne_line}" | cut -d',' -f8)
    details=$(echo "${fne_line}" | cut -d',' -f9)
    
    processed=$(extract_val "${details}" "processed")
    generated=$(extract_val "${details}" "generated")
    
    echo -n -e "  [FNE] Canary (CSR026+3)  : SZS=${szs} | Time=${time_s}s | processed=${processed} | generated=${generated} -> "
    
    if (( processed > 1000 )) || [[ "${szs}" == "Theorem" && "${time_s}" != "0.002" ]]; then
        echo -e "${RED}CONTAMINATED${NC}"
        is_contaminated=1
        reasons+=("FNE Canary CSR026+3 bloated with ${processed} processed clauses (CSR include-drift)")
    else
        echo -e "${GREEN}CLEAN${NC}"
    fi
fi

# Check FEQ Canary: AGT005+1
feq_line=$(find_canary_line "AGT005+1")
if [[ -n "${feq_line}" ]]; then
    canaries_found=$((canaries_found + 1))
    
    szs=$(echo "${feq_line}" | cut -d',' -f5)
    time_s=$(echo "${feq_line}" | cut -d',' -f8)
    details=$(echo "${feq_line}" | cut -d',' -f9)
    
    processed=$(extract_val "${details}" "processed")
    generated=$(extract_val "${details}" "generated")
    
    echo -n -e "  [FEQ] Canary (AGT005+1)  : SZS=${szs} | Time=${time_s}s | processed=${processed} | generated=${generated} -> "
    
    if (( processed > 1000 )) || [[ "${szs}" == "GaveUp" ]]; then
        echo -e "${RED}CONTAMINATED${NC}"
        is_contaminated=1
        reasons+=("FEQ Canary AGT005+1 bloated with ${processed} processed clauses (AGT include-drift)")
    else
        echo -e "${GREEN}CLEAN${NC}"
    fi
fi

# Check UEQ Canary: ALG212-10
ueq_line=$(find_canary_line "ALG212-10")
if [[ -n "${ueq_line}" ]]; then
    canaries_found=$((canaries_found + 1))
    
    szs=$(echo "${ueq_line}" | cut -d',' -f5)
    time_s=$(echo "${ueq_line}" | cut -d',' -f8)
    details=$(echo "${ueq_line}" | cut -d',' -f9)
    
    processed=$(extract_val "${details}" "processed")
    generated=$(extract_val "${details}" "generated")
    
    echo -n -e "  [UEQ] Canary (ALG212-10) : SZS=${szs} | Time=${time_s}s | processed=${processed} | generated=${generated} -> "
    
    if (( processed > 1000 )) || [[ "${szs}" == "GaveUp" ]]; then
        echo -e "${RED}CONTAMINATED${NC}"
        is_contaminated=1
        reasons+=("UEQ Canary ALG212-10 bloated with ${processed} processed clauses (ALG include-drift)")
    else
        echo -e "${GREEN}CLEAN${NC}"
    fi
fi

echo "---------------------------------------------------------"

if (( canaries_found == 0 )); then
    echo -e "Result: ${YELLOW}UNVERIFIABLE${NC} (No division canaries found in this CSV)"
elif (( is_contaminated == 1 )); then
    echo -e "Result: ${RED}KO - CONTAMINATED RUN${NC}"
    for reason in "${reasons[@]}"; do
        echo -e "  ${RED}* Reason:${NC} ${reason}"
    done
    exit 2
else
    echo -e "Result: ${GREEN}OK - CLEAN RUN${NC} (All present canaries verified clean)"
fi
