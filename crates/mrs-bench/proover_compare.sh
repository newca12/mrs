#!/usr/bin/env bash
# crates/mrs-bench/proover_compare.sh
#
# Compare ATP backends (mrs, eprover, vampire) when used as the engine
# behind mrs-proover. Runs each proof file once per backend, with each
# backend forced to be the sole ATP via --only-<backend>. Reports:
#
#   - per-backend VerifiedGood / VerifiedBad / Unknown counts,
#   - per-proof verdict + wall time for every backend,
#   - sum of wall times, mean per proof,
#   - agreement matrix (where backends disagree).
#
# Usage:
#   crates/mrs-bench/proover_compare.sh [OPTIONS]
#
# Options:
#   --proofs-dir DIR   Directory containing *_proof.p files
#                      (default: crates/mrs-proover/tests/fixtures)
#   --time SECS        Per-proof wall-clock budget (default: 30)
#   --backends LIST    Comma-separated subset of {mrs,eprover,vampire}
#                      (default: all three)
#   --output DIR       Where to write the run.csv + run.log
#                      (default: crates/mrs-bench/results/proover-compare/<ts>)
#
# Requires the corresponding binaries to be built / installed:
#   - mrs                  (built with `--features proover` for fair play)
#   - eprover, vampire     (in crates/mrs-bench/systems/*/bin/)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

PROOFS_DIR="${WORKSPACE_ROOT}/crates/mrs-proover/tests/fixtures"
TIME=30
BACKENDS="mrs,eprover,vampire"
OUTPUT=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --proofs-dir) PROOFS_DIR="$2"; shift 2 ;;
        --time)       TIME="$2";       shift 2 ;;
        --backends)   BACKENDS="$2";   shift 2 ;;
        --output)     OUTPUT="$2";     shift 2 ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

TS="$(date +%Y%m%d_%H%M%S)"
[[ -z "${OUTPUT}" ]] && OUTPUT="${SCRIPT_DIR}/results/proover-compare/${TS}"
mkdir -p "${OUTPUT}"
exec 2> >(tee -a "${OUTPUT}/run.log" >&2)

PROOVER="${WORKSPACE_ROOT}/target/release/mrs-proover"
if [[ ! -x "${PROOVER}" ]]; then
    echo "mrs-proover not built; run: cargo build --release -p mrs-proover" >&2
    exit 1
fi
EPROVER="${SCRIPT_DIR}/systems/eprover/bin/eprover"
VAMPIRE="${SCRIPT_DIR}/systems/vampire/bin/vampire"
MRS="${WORKSPACE_ROOT}/target/release/mrs"

IFS=',' read -ra BACKENDS_LIST <<< "${BACKENDS}"
for b in "${BACKENDS_LIST[@]}"; do
    case "${b}" in
        mrs)
            [[ -x "${MRS}" ]] || { echo "mrs binary missing; run: cargo build --release --features proover --bin mrs" >&2; exit 1; }
            ;;
        eprover)
            [[ -x "${EPROVER}" ]] || { echo "eprover missing at ${EPROVER}" >&2; exit 1; }
            ;;
        vampire)
            [[ -x "${VAMPIRE}" ]] || { echo "vampire missing at ${VAMPIRE}" >&2; exit 1; }
            ;;
        *) echo "Unknown backend: ${b}" >&2; exit 1 ;;
    esac
done

PROOFS_ABS="$(cd "${PROOFS_DIR}" && pwd)"
mapfile -t PROOF_FILES < <(find "${PROOFS_ABS}" -maxdepth 4 -type f -name '*_proof.p' | sort)
if [[ ${#PROOF_FILES[@]} -eq 0 ]]; then
    mapfile -t PROOF_FILES < <(grep -rl --include='*.p' '^% Proof' "${PROOFS_ABS}" 2>/dev/null | sort)
fi
if [[ ${#PROOF_FILES[@]} -eq 0 ]]; then
    echo "No proofs found under ${PROOFS_DIR}" >&2
    exit 1
fi

CSV="${OUTPUT}/run.csv"
echo "proof,backend,verdict,wall_time_s,detail" > "${CSV}"

echo "[compare] Proofs:   ${#PROOF_FILES[@]} from ${PROOFS_ABS}" >&2
echo "[compare] Backends: ${BACKENDS_LIST[*]}" >&2
echo "[compare] Time:     ${TIME}s / proof / backend" >&2
echo "[compare] Output:   ${OUTPUT}" >&2

run_one() {
    local proof="$1" backend="$2"
    local name; name="$(basename "${proof}" .p)"
    local proof_dir; proof_dir="$(dirname "${proof}")"
    local extra_args=()
    case "${backend}" in
        mrs)     extra_args=(--only-mrs) ;;
        eprover) extra_args=(--only-eprover --eprover "${EPROVER}") ;;
        vampire) extra_args=(--only-vampire --vampire "${VAMPIRE}") ;;
    esac

    local tmp; tmp="$(mktemp)"
    local start_ms end_ms wall_s line verdict detail
    start_ms=$(date +%s%3N)
    timeout $(( TIME + 5 )) "${PROOVER}" \
        --time "${TIME}" \
        --problems-dir "${proof_dir}" \
        "${extra_args[@]}" \
        "${proof}" > "${tmp}" 2>/dev/null || true
    end_ms=$(date +%s%3N)
    wall_s=$(echo "scale=3; (${end_ms} - ${start_ms}) / 1000" | bc)

    line=$(grep -m1 '% SZS status' "${tmp}" 2>/dev/null || true)
    if [[ -z "${line}" ]]; then
        verdict="Unknown"
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
    printf '%s,%s,%s,%s,%s\n' "${name}" "${backend}" "${verdict}" "${wall_s}" "${detail}"
}

total=$(( ${#PROOF_FILES[@]} * ${#BACKENDS_LIST[@]} ))
done=0
for proof in "${PROOF_FILES[@]}"; do
    for backend in "${BACKENDS_LIST[@]}"; do
        row=$(run_one "${proof}" "${backend}")
        printf '%s\n' "${row}" >> "${CSV}"
        (( done++ )) || true
        printf '\r[compare] %d/%d completed' "${done}" "${total}" >&2
    done
done
printf '\n' >&2

# ------------------------------------------------------------------
# Summary
# ------------------------------------------------------------------
echo "" >&2
echo "[compare] Summary (rows = backend, cols = verdict):" >&2
awk -F, '
NR>1 {
    n[$2,$3]++; sys[$2]=1; v[$3]=1;
    t[$2]+=$4; c[$2]++;
}
END {
    printf "  %-12s %-12s %-16s %-12s %-14s %-14s\n",
           "backend","VerifiedGood","VerifiedBad","Unknown","tot_time(s)","mean(s)" > "/dev/stderr"
    for (s in sys) {
        printf "  %-12s %-12d %-16d %-12d %-14.3f %-14.3f\n",
               s, n[s,"VerifiedGood"]+0, n[s,"VerifiedBad"]+0, n[s,"Unknown"]+0,
               t[s], t[s]/c[s] > "/dev/stderr"
    }
}' "${CSV}"

echo "" >&2
echo "[compare] Per-proof verdicts (V=VerifiedGood F=VerifiedBad N=Unknown):" >&2
awk -F, -v backends="${BACKENDS}" '
BEGIN {
    n=split(backends, B, ",");
    printf "  %-32s", "proof" > "/dev/stderr"
    for (i=1; i<=n; i++) printf " %-12s", B[i] > "/dev/stderr"
    print "" > "/dev/stderr"
}
NR>1 {
    g[$1,$2]=substr($3,1,1);
    w[$1,$2]=$4;
    seen[$1]=1
}
END {
    for (p in seen) print p
}' "${CSV}" | sort | while read -r proof; do
    line=$(printf "  %-32s" "${proof}")
    for backend in "${BACKENDS_LIST[@]}"; do
        # Look up the verdict letter and time.
        row=$(awk -F, -v p="${proof}" -v b="${backend}" \
            '$1==p && $2==b { v=substr($3,1,1); printf "%s %5.2fs", v, $4 }' "${CSV}")
        line+=$(printf " %-12s" "${row}")
    done
    printf '%s\n' "${line}" >&2
done

echo "" >&2
echo "[compare] CSV: ${CSV}" >&2
