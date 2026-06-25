#!/usr/bin/env bash
#
# norgler_compare.sh
#
# Benchmarks mrs-proover against the Nörgler reference verifier on the offline
# proover corpus (crates/mrs-bench/proover-corpus). Reports per-backend
# Verified / FailedVerified / NotVerified counts and wall times into run.csv.
#
# Requires:
#   - target/release/mrs-proover            (cargo build --release -p mrs-proover)
#   - crates/mrs-bench/systems/norgler/      (see that dir's invoke.sh header for
#                                             the noergler-1.0.jar / java setup)
#
# CAVEAT: the Nörgler numbers are PRELIMINARY. Nörgler is strict about
# annotation shapes and rejects several standard E/Vampire rules even with the
# `--relax-*` flags this harness passes, so its FailedVerified/NotVerified
# counts reflect format/config friction, NOT a definitive quality gap. Do not
# read the raw totals as an mrs-proover "win". See docs/PROOVER_HARNESS.md.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

PROOFS_DIR="${SCRIPT_DIR}/proover-corpus/proofs"
PROBLEMS_DIR="${SCRIPT_DIR}/proover-corpus/Problems"

TIME=30
OUTPUT=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --time)       TIME="$2";       shift 2 ;;
        --output)     OUTPUT="$2";     shift 2 ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

TS="$(date +%Y%m%d_%H%M%S)"
[[ -z "${OUTPUT}" ]] && OUTPUT="${SCRIPT_DIR}/results/norgler-compare/${TS}"
mkdir -p "${OUTPUT}"
exec 2> >(tee -a "${OUTPUT}/run.log" >&2)

PROOVER="${WORKSPACE_ROOT}/target/release/mrs-proover"
if [[ ! -x "${PROOVER}" ]]; then
    echo "mrs-proover not built; run: cargo build --release -p mrs-proover" >&2
    exit 1
fi

NORGLER="${SCRIPT_DIR}/systems/norgler/invoke.sh"
if [[ ! -x "${NORGLER}" ]]; then
    echo "Nörgler not installed; ensure invoke.sh exists and is executable." >&2
    exit 1
fi

mapfile -t PROOF_FILES < <(find "${PROOFS_DIR}" -type f -name '*.s' | sort)
if [[ ${#PROOF_FILES[@]} -eq 0 ]]; then
    echo "No proofs found under ${PROOFS_DIR}. Run build_proover_corpus.sh first." >&2
    exit 1
fi

CSV="${OUTPUT}/run.csv"
echo "proof,backend,verdict,wall_time_s,detail" > "${CSV}"

echo "[benchmark] Proofs:   ${#PROOF_FILES[@]} from ${PROOFS_DIR}" >&2
echo "[benchmark] Backends: mrs-proover, norgler" >&2
echo "[benchmark] Time:     ${TIME}s / proof / backend" >&2
echo "[benchmark] Output:   ${OUTPUT}" >&2

run_mrs_proover() {
    local proof="$1" prob_file="$2" name="$3"
    local tmp; tmp="$(mktemp)"
    local start_ms end_ms wall_s line verdict detail
    
    start_ms=$(date +%s%3N)
    timeout $(( TIME + 5 )) "${PROOVER}" \
        --time "${TIME}" \
        --problems-dir "${PROBLEMS_DIR}" \
        "${proof}" > "${tmp}" 2>/dev/null || true
    end_ms=$(date +%s%3N)
    wall_s=$(echo "scale=3; (${end_ms} - ${start_ms}) / 1000" | bc)

    line=$(grep -m1 '% SZS status' "${tmp}" 2>/dev/null || true)
    if [[ -z "${line}" ]]; then
        verdict="NotVerified"
        detail="no SZS line emitted"
    else
        verdict=$(awk '{print $4}' <<< "${line}")
        if [[ "${line}" == *":"* ]]; then
            detail="${line#*: }"
        else
            detail=""
        fi
    fi
    detail="${detail//,/;}"
    detail="${detail//$'\n'/ }"
    rm -f "${tmp}"
    printf '%s,mrs-proover,%s,%s,%s\n' "${name}" "${verdict}" "${wall_s}" "${detail}"
}

run_norgler() {
    local proof="$1" prob_file="$2" name="$3"
    local tmp; tmp="$(mktemp)"
    local start_ms end_ms wall_s line verdict detail
    
    start_ms=$(date +%s%3N)
    # Nörgler is slow to start (JVM + nix-shell) and can hang on steps it cannot
    # model; cap it at TIME seconds (hard) with a matching soft --timeout.
    timeout "${TIME}" "${NORGLER}" \
        --timeout "${TIME}" \
        --problem "${prob_file}" \
        "${proof}" > "${tmp}" 2>/dev/null || true
    end_ms=$(date +%s%3N)
    wall_s=$(echo "scale=3; (${end_ms} - ${start_ms}) / 1000" | bc)

    line=$(grep -m1 '% SZS status' "${tmp}" 2>/dev/null || true)
    if [[ -z "${line}" ]]; then
        verdict="NotVerified"
        detail="no SZS line emitted (timeout)"
    else
        verdict=$(awk '{print $4}' <<< "${line}")
        if [[ "${line}" == *":"* ]]; then
            detail="${line#*: }"
        else
            detail=""
        fi
    fi
    detail="${detail//,/;}"
    detail="${detail//$'\n'/ }"
    rm -f "${tmp}"
    printf '%s,norgler,%s,%s,%s\n' "${name}" "${verdict}" "${wall_s}" "${detail}"
}

total=$(( ${#PROOF_FILES[@]} * 2 ))
done=0
for proof in "${PROOF_FILES[@]}"; do
    name="$(basename "${proof}" .s)"
    prob_name="${name%%__*}"
    prob_file="${PROBLEMS_DIR}/${prob_name}.p"
    
    row_mrs=$(run_mrs_proover "${proof}" "${prob_file}" "${name}")
    printf '%s\n' "${row_mrs}" >> "${CSV}"
    (( done++ )) || true
    
    row_norgler=$(run_norgler "${proof}" "${prob_file}" "${name}")
    printf '%s\n' "${row_norgler}" >> "${CSV}"
    (( done++ )) || true
    
    printf '\r[benchmark] %d/%d completed' "${done}" "${total}" >&2
done
printf '\n' >&2

echo "" >&2
echo "[benchmark] Summary (rows = backend, cols = verdict):" >&2
awk -F, '
NR>1 {
    n[$2,$3]++; sys[$2]=1; v[$3]=1;
    t[$2]+=$4; c[$2]++;
}
END {
    printf "  %-12s %-12s %-16s %-12s %-14s %-14s\n",
           "backend","Verified","FailedVerified","NotVerified","tot_time(s)","mean(s)" > "/dev/stderr"
    for (s in sys) {
        printf "  %-12s %-12d %-16d %-12d %-14.3f %-14.3f\n",
               s, n[s,"Verified"]+0, n[s,"FailedVerified"]+0, n[s,"NotVerified"]+0,
               t[s], t[s]/c[s] > "/dev/stderr"
    }
}' "${CSV}"

echo "" >&2
echo "[benchmark] CSV: ${CSV}" >&2
