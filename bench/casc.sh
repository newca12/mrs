#!/usr/bin/env bash
# bench/casc.sh
# Run a CASC benchmark: invoke each registered system on each selected problem,
# collect SZS status + wall time, and write a CSV.
#
# Usage:
#   bench/casc.sh [OPTIONS]
#
# Options:
#   --edition   <name>         Competition edition directory (default: casc-30)
#   --systems   <s1,s2,...>    Comma-separated system names (default: all in bench/systems/)
#   --divisions <d1,d2,...>    Comma-separated division names (default: fne,feq,epu,eps,ueq,icu)
#   --time      <secs>         Per-problem time limit in seconds (default: 120)
#   --jobs      <N>            Parallel jobs (default: 1)
#   --output    <dir>          Output directory (default: bench/results/<edition>/TIMESTAMP)
#
# Output:
#   <output>/run.csv    — one row per (problem, system)
#   <output>/run.log    — harness stderr
#
# CSV schema: edition,division,problem,system,szs_status,wall_time_s
#
# Requires: bash >= 4, bc, timeout (GNU coreutils)
# For --jobs > 1 also requires: GNU parallel OR xargs -P
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# ---------- defaults ----------
EDITION="casc-30"
SYSTEMS=""          # empty = auto-discover
DIVISIONS="fne,feq,epu,eps,ueq,icu"
TIME_LIMIT=120
JOBS=1
OUTPUT=""

# ---------- arg parsing ----------
while [[ $# -gt 0 ]]; do
    case "$1" in
        --edition)   EDITION="$2";    shift 2 ;;
        --systems)   SYSTEMS="$2";    shift 2 ;;
        --divisions) DIVISIONS="$2";  shift 2 ;;
        --time)      TIME_LIMIT="$2"; shift 2 ;;
        --jobs)      JOBS="$2";       shift 2 ;;
        --output)    OUTPUT="$2";     shift 2 ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

TIMESTAMP="$(date +%Y%m%d_%H%M%S)"
if [[ -z "${OUTPUT}" ]]; then
    OUTPUT="${SCRIPT_DIR}/results/${EDITION}/${TIMESTAMP}"
fi
mkdir -p "${OUTPUT}"

# Redirect harness stderr to run.log (tee so it still shows on terminal)
exec 2> >(tee -a "${OUTPUT}/run.log" >&2)

PROBLEMS_DIR="${SCRIPT_DIR}/problems/${EDITION}"
LISTS_DIR="${PROBLEMS_DIR}/lists"
PROBLEMS_ROOT="${PROBLEMS_DIR}/Problems"

# Set TPTP so %include directives resolve (can be overridden by caller)
if [[ -z "${TPTP:-}" ]]; then
    export TPTP="${PROBLEMS_DIR}"
fi

# ---------- discover systems ----------
if [[ -z "${SYSTEMS}" ]]; then
    SYSTEMS_LIST=()
    for d in "${SCRIPT_DIR}/systems"/*/; do
        name="$(basename "${d}")"
        if [[ -x "${d}invoke.sh" ]]; then
            SYSTEMS_LIST+=("${name}")
        fi
    done
else
    IFS=',' read -ra SYSTEMS_LIST <<< "${SYSTEMS}"
fi

if [[ ${#SYSTEMS_LIST[@]} -eq 0 ]]; then
    echo "No systems found. Add a directory under bench/systems/ with an invoke.sh." >&2
    exit 1
fi

# ---------- validate systems ----------
for sys in "${SYSTEMS_LIST[@]}"; do
    invoke="${SCRIPT_DIR}/systems/${sys}/invoke.sh"
    if [[ ! -x "${invoke}" ]]; then
        echo "System '${sys}' has no executable invoke.sh at ${invoke}" >&2
        exit 1
    fi
done

# ---------- build job list ----------
IFS=',' read -ra DIVISION_LIST <<< "${DIVISIONS}"

CSV="${OUTPUT}/run.csv"
echo "edition,division,problem,system,szs_status,wall_time_s" > "${CSV}"

JOBS_FILE="${OUTPUT}/.jobs"
> "${JOBS_FILE}"

total_problems=0
for div in "${DIVISION_LIST[@]}"; do
    list="${LISTS_DIR}/${div}.list"
    if [[ ! -f "${list}" ]]; then
        echo "WARNING: no list file for division '${div}' at ${list}" >&2
        continue
    fi
    while IFS= read -r problem || [[ -n "${problem}" ]]; do
        [[ -z "${problem}" ]] && continue
        domain="${problem:0:3}"
        prob_path="${PROBLEMS_ROOT}/${domain}/${problem}.p"
        for sys in "${SYSTEMS_LIST[@]}"; do
            printf '%s\t%s\t%s\t%s\n' "${div}" "${problem}" "${prob_path}" "${sys}" >> "${JOBS_FILE}"
            (( total_problems++ )) || true
        done
    done < "${list}"
done

echo "[casc] Edition:   ${EDITION}" >&2
echo "[casc] Systems:   ${SYSTEMS_LIST[*]}" >&2
echo "[casc] Divisions: ${DIVISION_LIST[*]}" >&2
echo "[casc] Time/prob: ${TIME_LIMIT}s" >&2
echo "[casc] Jobs:      ${TOTAL_JOBS:-${total_problems}} (parallelism: ${JOBS})" >&2
echo "[casc] Output:    ${OUTPUT}" >&2
echo "[casc] TPTP:      ${TPTP}" >&2
echo "[casc] Total jobs: ${total_problems}" >&2

# ---------- worker function ----------
# Arguments: div  problem  prob_path  sys
run_one() {
    local div="$1" problem="$2" prob_path="$3" sys="$4"
    local invoke="${SCRIPT_DIR}/systems/${sys}/invoke.sh"
    local tmp
    tmp="$(mktemp)"

    local start_ms end_ms wall_s szs exit_code
    start_ms=$(date +%s%3N)
    # Give the system TIME_LIMIT seconds; add 10s grace for it to flush output.
    timeout $(( TIME_LIMIT + 10 )) "${invoke}" "${prob_path}" "${TIME_LIMIT}" \
        > "${tmp}" 2>/dev/null
    exit_code=$?
    end_ms=$(date +%s%3N)

    wall_s=$(echo "scale=3; (${end_ms} - ${start_ms}) / 1000" | bc)

    # Extract SZS status from output.
    # Vampire: "% SZS status Theorem for ..."
    # mrs:     "% SZS status Theorem for ..."
    szs=$(grep -m1 '% SZS status' "${tmp}" 2>/dev/null | awk '{print $4}' || true)

    if [[ -z "${szs}" ]]; then
        if [[ ${exit_code} -eq 124 ]]; then
            szs="Timeout"
        else
            szs="GaveUp"
        fi
    fi

    rm -f "${tmp}"
    printf '%s,%s,%s,%s,%s,%s\n' \
        "${EDITION}" "${div}" "${problem}" "${sys}" "${szs}" "${wall_s}"
}
export -f run_one
export SCRIPT_DIR TIME_LIMIT EDITION

# ---------- execute ----------
completed=0
GRACE=$((TIME_LIMIT + 10))

run_and_append() {
    local line="$1"
    IFS=$'\t' read -r div problem prob_path sys <<< "${line}"
    local row
    row=$(run_one "${div}" "${problem}" "${prob_path}" "${sys}")
    # Append atomically (single printf is atomic for short lines on Linux)
    printf '%s\n' "${row}" >> "${CSV}"
    (( completed++ )) || true
    printf '\r[casc] %d/%d completed' "${completed}" "${total_problems}" >&2
}

if [[ "${JOBS}" -le 1 ]]; then
    while IFS= read -r line; do
        run_and_append "${line}"
    done < "${JOBS_FILE}"
else
    # Parallel execution via GNU parallel or xargs -P
    if command -v parallel &>/dev/null; then
        export -f run_one
        # parallel reads TSV lines; split on tab to get fields
        parallel --jobs "${JOBS}" --colsep '\t' \
            'row=$(run_one {1} {2} {3} {4}); printf "%s\n" "${row}"' \
            :::: "${JOBS_FILE}" >> "${CSV}"
    else
        # xargs fallback: wrap each line as a single argument
        xargs -P "${JOBS}" -I '{}' bash -c '
            IFS=$'"'"'\t'"'"' read -r div problem prob_path sys <<< "$1"
            row=$(run_one "${div}" "${problem}" "${prob_path}" "${sys}")
            printf "%s\n" "${row}"
        ' _ '{}' < "${JOBS_FILE}" >> "${CSV}"
    fi
fi

printf '\n' >&2
rm -f "${JOBS_FILE}"
echo "[casc] Done. Results: ${CSV}" >&2
