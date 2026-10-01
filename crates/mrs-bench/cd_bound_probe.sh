#!/usr/bin/env bash
# cd_bound_probe.sh
#
# Asks the one question the FNE condensed-detachment campaigns could not:
# **is the pre-pass's 1 s bound what stops it, or the method itself?**
#
# The 2026-10-01 FNE campaigns (docs/research/condensed-detachment.md) ran the
# pre-pass at its production bounds over both CASC editions and produced zero
# refutations. That is not yet a verdict on the method: `cd_budget` is capped at
# one second against a 238 s problem, and 29 of the 48 rows with a measurable
# budget shift hit that full 1 s cap rather than CD's own fact/step bound. So
# those rows record a closure that was *cut off*, not a closure that *failed*.
#
# This script separates the two by running the pre-pass alone, at two bound
# settings, over the LCL cluster it targets:
#
#   prod   MRS_CD_BUDGET_MS unset  -> the production 1 s / 5 000 facts /
#           100 000 inferences, i.e. exactly what the campaign ran
#   wide   the bounds raised (default 60 s / 100 000 facts / 2 000 000
#           inferences), which is 60x the wall budget and 20x each structural
#           bound
#
# Two deliberate constraints keep the result about the pre-pass:
#
#   - `--time` tracks each arm's own pre-pass budget, plus two seconds of parse
#     headroom. MRS_CD_BUDGET_MS caps the pre-pass independently, so the headroom
#     does not dilute the arm being measured; what it bounds is the portfolio
#     that would otherwise inherit the remainder. That inheritance happens
#     whenever the pre-pass stops early — `facts_exhausted`, `no_fragment` —
#     and then the portfolio can solve the problem on its own. It does not
#     corrupt the reading, because the two are distinguishable in the row: a
#     pre-pass refutation is always `stop=refutation`, so `stop!=refutation`
#     with `szs_status=Theorem` is the portfolio's work. Only `stop=refutation`
#     is counted as pre-pass coverage below.
#   - The pre-pass is single-threaded, so the result does not depend on the
#     worker count. `--workers 1` is therefore not a resource concession, it is
#     what makes the number comparable between the hosts this has been run on
#     (a 32-core CASC box and a 2-core laptop).
#
# The reading is the `% SZS detail condensed_detachment=` line, which reports the
# stop reason that the campaign runs could not report. `stop=refutation` is the
# only positive. `stop=deadline` still means the bound was too small and the
# question is still open; `stop=facts_exhausted` means the closure ran out of
# detachments to make, which is a verdict on the method.
#
# Any refutation is put through `mrs-proover --strict` before it is counted. A
# pre-pass result that the kernel will not accept is not coverage.
#
# Usage:
#   cd_bound_probe.sh                      # both arms, production problems dir
#   cd_bound_probe.sh --arm wide           # one arm
#   cd_bound_probe.sh --div fne --edition casc-j13
#   cd_bound_probe.sh --time 300 --max-facts 200000 --max-inferences 4000000
#   cd_bound_probe.sh --problems <file>    # explicit problem list, one path/line
#
# Environment:
#   MRS_BINARY   mrs binary; default ./target/release/mrs
#   CD_OUT       output directory; default results/cd-probe-<timestamp>
#   TPTP         only needed if the problems use %include

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# crates/mrs-bench/ -> crates/ -> workspace root
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

MRS_BIN="${MRS_BINARY:-${WORKSPACE_ROOT}/target/release/mrs}"
if [[ ! -x "${MRS_BIN}" ]]; then
    echo "cd_bound_probe: mrs binary not found at ${MRS_BIN}" >&2
    echo "  build it, or set MRS_BINARY" >&2
    exit 1
fi

PROVER="${MRS_PROOVER:-${WORKSPACE_ROOT}/target/release/mrs-proover}"

ARM="both"
DIVISION="fne"
EDITION="casc-30"
PROBLEM_GLOB=""
PROBLEMS_FILE=""
CD_TIME=60
CD_MAX_FACTS=100000
CD_MAX_INFERENCES=2000000
# The production arm is the reference, so its bounds are fixed rather than
# configurable: an A arm that drifts is not the campaign any more.
PROD_BUDGET_MS=1000
PROD_PROCESS_MS=5000
PROD_MAX_FACTS=5000
PROD_MAX_INFERENCES=100000

usage() {
    # The header comment is the documentation; print it rather than keeping a
    # second copy that can drift.
    sed -n '2,/^set -euo/p' "${BASH_SOURCE[0]}" | sed '$d'
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --arm) ARM="$2"; shift 2 ;;
        --div|--division) DIVISION="$2"; shift 2 ;;
        --edition) EDITION="$2"; shift 2 ;;
        --glob) PROBLEM_GLOB="$2"; shift 2 ;;
        --problems) PROBLEMS_FILE="$2"; shift 2 ;;
        --time) CD_TIME="$2"; shift 2 ;;
        --max-facts) CD_MAX_FACTS="$2"; shift 2 ;;
        --max-inferences) CD_MAX_INFERENCES="$2"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "cd_bound_probe: unknown argument $1" >&2; exit 2 ;;
    esac
done

case "${ARM}" in prod|wide|both) ;; *)
    echo "cd_bound_probe: --arm must be prod, wide, or both" >&2; exit 2 ;;
esac

if [[ -z "${TPTP:-}" ]]; then
    export TPTP="${SCRIPT_DIR}/problems/${EDITION}"
fi
DIV_DIR="${TPTP}/${DIVISION^^}"
if [[ ! -d "${DIV_DIR}" ]]; then
    echo "cd_bound_probe: no division directory at ${DIV_DIR}" >&2
    exit 1
fi

OUT="${CD_OUT:-${SCRIPT_DIR}/results/cd-probe-$(date +%Y%m%d_%H%M%S)}"
mkdir -p "${OUT}"
REPORT="${OUT}/cd-probe.csv"
LOG="${OUT}/cd-probe.log"

if [[ -n "${PROBLEMS_FILE}" ]]; then
    mapfile -t PROBLEMS < "${PROBLEMS_FILE}"
elif [[ -n "${PROBLEM_GLOB}" ]]; then
    mapfile -t PROBLEMS < <(find "${DIV_DIR}" -maxdepth 1 -name "${PROBLEM_GLOB}" | sort)
else
    # The pre-pass only recognises the `is_a_theorem` fragment, which in the FNE
    # divisions is the LCL cluster. Defaulting to it keeps a bare invocation
    # aimed at the fragment instead of at 100 unrelated problems.
    mapfile -t PROBLEMS < <(find "${DIV_DIR}" -maxdepth 1 -name 'LCL*.p' | sort)
fi
if [[ "${#PROBLEMS[@]}" -eq 0 ]]; then
    echo "cd_bound_probe: no problems selected" >&2
    exit 1
fi

{
    echo "division=${DIVISION} edition=${EDITION} problems=${#PROBLEMS[@]}"
    echo "binary=${MRS_BIN}"
    echo "prover=${PROVER}"
    echo "wide: budget_ms=$((CD_TIME * 1000)) wall=$((CD_TIME + 2)) max_facts=${CD_MAX_FACTS} max_inferences=${CD_MAX_INFERENCES}"
    echo "prod: budget_ms=${PROD_BUDGET_MS} wall=$((PROD_BUDGET_MS / 1000 + 2)) max_facts=${PROD_MAX_FACTS} max_inferences=${PROD_MAX_INFERENCES}"
} | tee "${LOG}"

echo "edition,division,problem,arm,szs_status,cd_shape,cd_stop,cd_facts,cd_inferences,cd_elapsed_ms,cd_budget_ms,strict,proof_path" > "${REPORT}"

# One arm: run the pre-pass alone over every problem, sequentially.
#
# Sequential on purpose. The production arm of the FNE campaigns lost problems to
# the OS OOM killer because two campaigns ran at once on one 128 GB box, and this
# script must not reproduce that confound in a measurement whose whole point is
# that it is clean.
run_arm() {
    local arm="$1" budget_ms="$2" process_ms="$3" max_facts="$4" max_inferences="$5"
    local arm_dir="${OUT}/${arm}"
    mkdir -p "${arm_dir}"
    local n=0 solved=0 deadline=0 exhausted=0
    # `--time` only has to exceed the pre-pass budget: MRS_CD_BUDGET_MS caps the
    # pre-pass independently, so the extra seconds are parse headroom and the
    # portfolio's leftover. Keeping --time tied to each arm's own budget is what
    # stops a 1 s arm from spending a minute of portfolio time per problem --
    # which is both wasted and a chance to hit the host's memory ceiling on the
    # problems that want 40 GB.
    local wall=$(( budget_ms / 1000 + 2 ))
    for problem in "${PROBLEMS[@]}"; do
        local name
        name="$(basename "${problem}" .p)"
        local stdout="${arm_dir}/${name}.stdout" stderr="${arm_dir}/${name}.stderr"
        n=$((n + 1))
        # No colour: this log is the measurement, and escape codes make it
        # awkward to grep or paste into a report.
        printf '[%s] %s %3d/%3d\n' "${arm}" "${name}" "${n}" "${#PROBLEMS[@]}" >&2

        # MRS_MAX_MEMORY_MB makes a runaway closure report a resource
        # ceiling instead of inviting the OS OOM killer onto a shared box.
        MRS_CONDENSED_DETACHMENT=1 \
        MRS_CD_BUDGET_MS="${budget_ms}" \
        MRS_CD_PROCESS_MS="${process_ms}" \
        MRS_CD_MAX_FACTS="${max_facts}" \
        MRS_CD_MAX_INFERENCES="${max_inferences}" \
        MRS_MAX_MEMORY_MB="${CD_MAX_MEMORY_MB:-8000}" \
        TPTP="${TPTP}" \
            "${MRS_BIN}" --time "${wall}" --workers 1 --schedule "casc_${DIVISION,,}" \
            "${problem}" >"${stdout}" 2>"${stderr}" || true

        local status shape stop facts inferences elapsed_ms budget strict proof
        status="$(grep -m1 -o '^% SZS status [A-Za-z]*' "${stdout}" | awk '{print $4}' || true)"
        status="${status:-none}"
        read -r shape stop facts inferences elapsed_ms budget <<<"$(
            grep -m1 -o 'condensed_detachment=ran .*' "${stderr}" |
            sed -E 's/.*shape=([a-z]+) stop=([a-z_]+) facts=([0-9]+) inferences=([0-9]+) elapsed_ms=([0-9]+) budget_ms=([0-9]+).*/\1 \2 \3 \4 \5 \6/' || true
        )"
        shape="${shape:-absent}"; stop="${stop:-absent}"
        facts="${facts:-0}"; inferences="${inferences:-0}"
        elapsed_ms="${elapsed_ms:-0}"; budget="${budget:-0}"

        # A refutation is only coverage once the kernel accepts the proof.
        strict="n/a"; proof=""
        if [[ "${status}" == "Theorem" || "${status}" == "Unsatisfiable" ]]; then
            if [[ -x "${PROVER}" ]]; then
                sed -n '/^% SZS output start/,/^% SZS output end/p' "${stdout}" \
                    | sed -e '1d' -e '$d' > "${arm_dir}/${name}.tstp"
                proof="${arm_dir}/${name}.tstp"
                strict="$("${PROVER}" --strict --only-mrs --no-atp "${proof}" 2>&1 |
                    grep -m1 -o '% SZS status [A-Za-z]*' | awk '{print $4}' || true)"
                strict="${strict:-none}"
            else
                strict="no_proover"
            fi
        fi

        case "${stop}" in
            refutation) solved=$((solved + 1)) ;;
            deadline) deadline=$((deadline + 1)) ;;
            facts_exhausted) exhausted=$((exhausted + 1)) ;;
        esac

        printf '%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s\n' \
            "${EDITION}" "${DIVISION}" "${name}" "${arm}" "${status}" \
            "${shape}" "${stop}" "${facts}" "${inferences}" "${elapsed_ms}" "${budget}" \
            "${strict}" "${proof}" >> "${REPORT}"
    done
    {
        echo "--- arm ${arm}: refutations=${solved} deadline=${deadline} facts_exhausted=${exhausted} of ${#PROBLEMS[@]}"
    } | tee -a "${LOG}" >&2
}

if [[ "${ARM}" == "prod" || "${ARM}" == "both" ]]; then
    run_arm prod "${PROD_BUDGET_MS}" "${PROD_PROCESS_MS}" "${PROD_MAX_FACTS}" "${PROD_MAX_INFERENCES}"
fi
if [[ "${ARM}" == "wide" || "${ARM}" == "both" ]]; then
    run_arm wide "$((CD_TIME * 1000))" "$((CD_TIME * 1000 + PROD_PROCESS_MS))" "${CD_MAX_FACTS}" "${CD_MAX_INFERENCES}"
fi

echo
echo "report: ${REPORT}"
echo
echo "pre-pass verdicts by arm (only stop=refutation is coverage):"
awk -F, 'NR>1 {n[$4]++; if ($7=="refutation") r[$4]++; if ($7=="deadline") d[$4]++; if ($7=="facts_exhausted") x[$4]++; if ($7=="max_facts"||$7=="max_inferences") b[$4]++}
     END {for (a in n) printf "  %-5s problems=%d refutation=%d deadline=%d facts_exhausted=%d bound_hit=%d\n", a, n[a], r[a]+0, d[a]+0, x[a]+0, b[a]+0}' "${REPORT}" | sort
echo
echo "rows where the pre-pass refuted (all must be VerifiedGood):"
awk -F, 'NR>1 && $7=="refutation" {print "  " $3 "  status=" $5 "  strict=" $12}' "${REPORT}"