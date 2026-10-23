#!/usr/bin/env bash
# crates/mrs-bench/proover.sh
# Run a ProoVer-style benchmark: invoke a proof verifier on each TSTP proof
# file in a directory, collect SZS verdict + wall time, write a CSV.
#
# Usage:
#   crates/mrs-bench/proover.sh [OPTIONS]
#
# Options:
#   --proofs-dir <dir>    Directory containing *.p proof files (recursive)
#                         (default: crates/mrs-proover/tests/fixtures)
#   --systems  <s1,s2,..> Comma-separated system names from systems/
#                         (default: all that have a verify.sh script)
#   --time     <secs>     Per-proof time limit in seconds (default: 30)
#   --jobs     <N>        Parallel jobs (default: 1)
#   --output   <dir>      Output directory
#                         (default: crates/mrs-bench/results/proover/<timestamp>)
#
# Output:
#   <output>/run.csv  — proof,system,szs_verdict,wall_time_s,detail
#   <output>/run.log  — harness stderr
#
# Verifier convention:
#   crates/mrs-bench/systems/<sys>/verify.sh <proof.p> <time_limit_secs>
#   must print one `% SZS status <verdict>[ : <detail>]` line on stdout.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

PROOFS_DIR="${SCRIPT_DIR}/../mrs-proover/tests/fixtures"
SYSTEMS=""
TIME_LIMIT=30
JOBS=1
OUTPUT=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --proofs-dir) PROOFS_DIR="$2"; shift 2 ;;
        --systems)    SYSTEMS="$2";    shift 2 ;;
        --time)       TIME_LIMIT="$2"; shift 2 ;;
        --jobs)       JOBS="$2";       shift 2 ;;
        --output)     OUTPUT="$2";     shift 2 ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

TIMESTAMP="$(date +%Y%m%d_%H%M%S)"
if [[ -z "${OUTPUT}" ]]; then
    OUTPUT="${SCRIPT_DIR}/results/proover/${TIMESTAMP}"
fi
mkdir -p "${OUTPUT}"

exec 2> >(tee -a "${OUTPUT}/run.log" >&2)

# ---------- discover systems ----------
if [[ -z "${SYSTEMS}" ]]; then
    SYSTEMS_LIST=()
    for d in "${SCRIPT_DIR}/systems"/*/; do
        name="$(basename "${d}")"
        if [[ -x "${d}verify.sh" ]]; then
            SYSTEMS_LIST+=("${name}")
        fi
    done
else
    IFS=',' read -ra SYSTEMS_LIST <<< "${SYSTEMS}"
fi

if [[ ${#SYSTEMS_LIST[@]} -eq 0 ]]; then
    echo "No verifier systems found. Add a directory under crates/mrs-bench/systems/ with a verify.sh." >&2
    exit 1
fi

for sys in "${SYSTEMS_LIST[@]}"; do
    invoke="${SCRIPT_DIR}/systems/${sys}/verify.sh"
    if [[ ! -x "${invoke}" ]]; then
        echo "System '${sys}' has no executable verify.sh at ${invoke}" >&2
        exit 1
    fi
done

# ---------- collect proofs ----------
PROOFS_ABS="$(cd "${PROOFS_DIR}" && pwd)"
mapfile -t PROOF_FILES < <(find "${PROOFS_ABS}" -maxdepth 4 -type f -name '*_proof.p' -o -type f -name '*.proof' | sort)
if [[ ${#PROOF_FILES[@]} -eq 0 ]]; then
    # Fall back to *.p that look like proofs (contain `% Proof :` header).
    mapfile -t PROOF_FILES < <(grep -rl --include='*.p' '^% Proof' "${PROOFS_ABS}" 2>/dev/null | sort)
fi
if [[ ${#PROOF_FILES[@]} -eq 0 ]]; then
    echo "No proofs found in ${PROOFS_DIR}" >&2
    exit 1
fi

CSV="${OUTPUT}/run.csv"
echo "proof,system,szs_verdict,wall_time_s,detail" > "${CSV}"

JOBS_FILE="${OUTPUT}/.jobs"
> "${JOBS_FILE}"
total=0
for proof in "${PROOF_FILES[@]}"; do
    for sys in "${SYSTEMS_LIST[@]}"; do
        printf '%s\t%s\n' "${proof}" "${sys}" >> "${JOBS_FILE}"
        (( total++ )) || true
    done
done

echo "[proover] Systems:  ${SYSTEMS_LIST[*]}" >&2
echo "[proover] Proofs:   ${#PROOF_FILES[@]} from ${PROOFS_ABS}" >&2
echo "[proover] Time:     ${TIME_LIMIT}s" >&2
echo "[proover] Jobs:     ${total} (parallelism: ${JOBS})" >&2
echo "[proover] Output:   ${OUTPUT}" >&2

run_one() {
    local proof="$1" sys="$2"
    local invoke="${SCRIPT_DIR}/systems/${sys}/verify.sh"
    local tmp; tmp="$(mktemp)"
    local name="$(basename "${proof}" .p)"

    local start_ms end_ms wall_s line verdict detail
    start_ms=$(date +%s%3N)
    timeout $(( TIME_LIMIT + 5 )) "${invoke}" "${proof}" "${TIME_LIMIT}" \
        > "${tmp}" 2>/dev/null || true
    end_ms=$(date +%s%3N)
    wall_s=$(echo "scale=3; (${end_ms} - ${start_ms}) / 1000" | bc)

    line=$(grep -m1 '% SZS status' "${tmp}" 2>/dev/null || true)
    if [[ -z "${line}" ]]; then
        verdict="Unknown"
        detail="no SZS line emitted"
    else
        # Parse "% SZS status <verdict> [: <detail>]"
        verdict=$(awk '{print $4}' <<< "${line}")
        if [[ "${line}" == *":"* ]]; then
            detail="${line#*: }"
        else
            detail=""
        fi
    fi
    # CSV-safe: replace commas and newlines in detail.
    detail="${detail//,/;}"
    detail="${detail//$'\n'/ }"
    rm -f "${tmp}"
    printf '%s,%s,%s,%s,%s\n' "${name}" "${sys}" "${verdict}" "${wall_s}" "${detail}"
}
export -f run_one
export SCRIPT_DIR TIME_LIMIT

completed=0
run_and_append() {
    local line="$1"
    IFS=$'\t' read -r proof sys <<< "${line}"
    local row
    row=$(run_one "${proof}" "${sys}")
    printf '%s\n' "${row}" >> "${CSV}"
    (( completed++ )) || true
    printf '\r[proover] %d/%d completed' "${completed}" "${total}" >&2
}

if [[ "${JOBS}" -le 1 ]]; then
    while IFS= read -r line; do
        run_and_append "${line}"
    done < "${JOBS_FILE}"
else
    if command -v parallel &>/dev/null; then
        parallel --jobs "${JOBS}" --colsep '\t' \
            'row=$(run_one {1} {2}); printf "%s\n" "${row}"' \
            :::: "${JOBS_FILE}" >> "${CSV}"
    else
        xargs -P "${JOBS}" -I '{}' bash -c '
            IFS=$'"'"'\t'"'"' read -r proof sys <<< "$1"
            row=$(run_one "${proof}" "${sys}")
            printf "%s\n" "${row}"
        ' _ '{}' < "${JOBS_FILE}" >> "${CSV}"
    fi
fi

printf '\n' >&2
rm -f "${JOBS_FILE}"

# ---------- summary ----------
echo "[proover] Results: ${CSV}" >&2
echo "[proover] Summary:" >&2
awk -F, 'NR>1 {n[$2,$3]++; sys[$2]=1; v[$3]=1}
END {
  for (s in sys) {
    printf "  %s:", s > "/dev/stderr"
    for (k in v) printf " %s=%d", k, n[s SUBSEP k] > "/dev/stderr"
    print "" > "/dev/stderr"
  }
}' "${CSV}"
