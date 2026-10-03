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
#   baseline  MRS_PREPHASE=0 *with* `--schedule casc_<division>`
#   prephase  MRS_PREPHASE=1 and *without* `--schedule`
#
# The flag is omitted in the pre-phase arm on purpose. The pre-phase refuses to
# override an explicit schedule -- so that its own A/B stays readable -- which
# means naming one would silently turn the experiment into "pre-phase reports and
# does nothing".
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
    local schedule_args=("$@")
    local arm_dir="${OUTPUT}/${arm}"
    mkdir -p "${arm_dir}"

    local first=1
    while IFS= read -r problem; do
        [[ -n "${problem}" ]] || continue
        local name
        name="$(basename "${problem}" .p)"
        local status
        status="$(mktemp)"

        env MRS_PREPHASE="${arm}" "${BINARY}" \
            --time "${SOFT_TIME}" \
            --workers "${WORKERS}" \
            "${schedule_args[@]}" \
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
# PROBLEM_LIMIT truncates the list. A full CASC division at a competition
# budget is a multi-hour run on a development host; the flag exists so a smoke
# test of the harness itself does not need one.
if [[ -n "${PROBLEM_LIMIT:-}" ]]; then
    head -n "${PROBLEM_LIMIT}" "${PROBLEM_LIST}" >"${PROBLEM_LIST}.trim"
    mv "${PROBLEM_LIST}.trim" "${PROBLEM_LIST}"
fi

echo "[ab] $(wc -l <"${PROBLEM_LIST}") problems, division=${DIVISION}, time=${TIME_LIMIT}s, workers=${WORKERS}"
echo "[ab] output: ${OUTPUT}"

# `jobs` parallel jobs, one problem at a time within each arm. Two arms run in
# sequence so they never contend for the same cores: a contended arm measures
# contention, not routing.
printf 'division,problem,arm,szs_status\n' | tee "${OUTPUT}/baseline/run.csv" >/dev/null 2>&1 || true
mkdir -p "${OUTPUT}/baseline" "${OUTPUT}/prephase"
printf 'division,problem,arm,szs_status\n' >"${OUTPUT}/baseline/run.csv"
printf 'division,problem,arm,szs_status\n' >"${OUTPUT}/prephase/run.csv"

run_arm baseline --schedule "casc_${DIVISION,,}"
run_arm prephase

echo
echo "[ab] === summary ==="
python3 - "${OUTPUT}" <<'PY'
import csv
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
def load(arm):
    with open(root / arm / "run.csv", newline="") as handle:
        return {row["problem"]: row["szs_status"] for row in csv.DictReader(handle)
                if row["problem"] != "problem"}

SOLVED = {"Theorem", "Unsatisfiable", "Satisfiable", "CounterSatisfiable"}
baseline, prephase = load("baseline"), load("prephase")
problems = sorted(set(baseline) | set(prephase))
def solved(statuses):
    return sum(1 for p in problems if statuses.get(p) in SOLVED)

base_n, pre_n = solved(baseline), solved(prephase)
print(f"  baseline: {base_n}/{len(problems)}")
print(f"  prephase: {pre_n}/{len(problems)}")
print(f"  delta:    {pre_n - base_n:+d}")
print("\n  problems where the arms disagree:")
for problem in problems:
    b, p = baseline.get(problem, "missing"), prephase.get(problem, "missing")
    if (b in SOLVED) != (p in SOLVED):
        print(f"    {problem:<20} baseline={b or 'none':<14} prephase={p or 'none'}")
print(f"\n  raw output under {root}")
PY
