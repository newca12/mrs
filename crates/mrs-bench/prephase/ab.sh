#!/usr/bin/env bash
# crates/mrs-bench/prephase/ab.sh
#
# A/B the pre-phase against the shipped per-division portfolio on a real
# cooperative run.
#
#   ab.sh <division> <edition> [time] [jobs] [workers]
#
# Two arms, identical in every other respect:
#
#   baseline  MRS_PREPHASE unset, so `--schedule casc_<division>` is used
#   prephase  MRS_PREPHASE=1, so the routed plan replaces it
#
# The measurement is the one that matters at competition time: a single mrs
# process per problem with the full worker count, so the arms differ only in how
# the workers were chosen. `--workers 1` is available for diagnosis, but with one
# worker the routing decision barely has anything to route, so a positive result
# there is a statement about strategy choice, not about routing.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

DIVISION="${1:?usage: ab.sh <division> <edition> [time] [jobs] [workers]}"
EDITION="${2:?usage: ab.sh <division> <edition> [time] [jobs] [workers]}"
TIME_LIMIT="${3:-30}"
JOBS="${4:-2}"
WORKERS="${5:-2}"

OUTPUT="${ROOT}/results/prephase/ab-${EDITION}-${DIVISION}-$(date +%Y%m%d-%H%M%S)"
mkdir -p "${OUTPUT}"

BINARY="${MRS_BINARY:-${ROOT}/target/release/mrs}"
if [[ ! -x "${BINARY}" ]]; then
    echo "mrs binary not found at ${BINARY}; run: cargo build --release" >&2
    exit 1
fi

export TPTP="${TPTP:-${ROOT}/crates/mrs-bench/problems/casc-30}"
export MRS_HARDWARE="${MRS_HARDWARE:-casc}"
export MRS_WORKERS="${WORKERS}"

# One process per problem per arm. The two arms share nothing but the input list,
# so a difference in the solved count cannot come from a different problem set.
run_arm() {
    local arm="$1"
    shift
    local arm_dir="${OUTPUT}/${arm}"
    mkdir -p "${arm_dir}"

    while IFS= read -r problem; do
        [[ -n "${problem}" ]] || continue
        local name
        name="$(basename "${problem}" .p)"
        local status
        status="$(mktemp)"

        env "$@" "${BINARY}" \
            --time "${SOFT_TIME}" \
            --workers "${WORKERS}" \
            --schedule "casc_${DIVISION,,}" \
            "${problem}" >"${status}" 2>"${arm_dir}/${name}.stderr"

        local verdict
        verdict="$(sed -n 's/^% SZS status \([A-Za-z]*\).*/\1/p' "${status}" | head -1)"
        echo "${DIVISION},${name},${arm},${verdict:-none}" >>"${arm_dir}/run.csv"
        rm -f "${status}"
    done <"${PROBLEM_LIST}"
}

SOFT_TIME=$(( TIME_LIMIT > 2 ? TIME_LIMIT - 2 : TIME_LIMIT ))

PROBLEM_LIST="$(mktemp)"
trap 'rm -f "${PROBLEM_LIST}"' EXIT
find "${TPTP}/${DIVISION}" -maxdepth 1 -name '*.p' | sort >"${PROBLEM_LIST}"

echo "[ab] ${#} problems, division=${DIVISION}, time=${TIME_LIMIT}s, workers=${WORKERS}, jobs=${JOBS}"
echo "[ab] output: ${OUTPUT}"

# `jobs` parallel jobs, one problem at a time within each arm. Two arms run in
# sequence so they never contend for the same cores: a contended arm measures
# contention, not routing.
printf 'division,problem,arm,szs_status\n' | tee "${OUTPUT}/baseline/run.csv" >/dev/null 2>&1 || true
mkdir -p "${OUTPUT}/baseline" "${OUTPUT}/prephase"
printf 'division,problem,arm,szs_status\n' >"${OUTPUT}/baseline/run.csv"
printf 'division,problem,arm,szs_status\n' >"${OUTPUT}/prephase/run.csv"

run_arm baseline MRS_PREPHASE=0
run_arm prephase MRS_PREPHASE=1

echo
echo "[ab] === summary ==="
for arm in baseline prephase; do
    solved=$(awk -F, 'NR>1 && ($4=="Theorem"||$4=="Unsatisfiable"||$4=="Satisfiable"||$4=="CounterSatisfiable")' "${OUTPUT}/${arm}/run.csv" | wc -l)
    total=$(awk 'NR>1' "${OUTPUT}/${arm}/run.csv" | wc -l)
    echo "  ${arm}: ${solved}/${total}"
done
echo
echo "[ab] problems solved by exactly one arm:"
join -t, -j2 \
    <(sort -t, -k2,2 "${OUTPUT}/baseline/run.csv") \
    <(sort -t, -k2,2 "${OUTPUT}/prephase/run.csv") \
    2>/dev/null | awk -F, '$4!=$8 && ($4!="none" || $8!="none") {print "  " $2 "  baseline=" $4 " prephase=" $8}' || true
echo "[ab] full output under ${OUTPUT}"