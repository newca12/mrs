#!/usr/bin/env bash
# crates/mrs-bench/casc.sh
# Run a CASC benchmark: invoke each registered system on each selected problem,
# collect SZS status + wall time, and write a CSV.
#
# Usage:
#   crates/mrs-bench/casc.sh [OPTIONS]
#
# Options:
#   --edition     <name>         Competition edition directory (default: casc-30)
#   --systems     <s1,s2,...>    Comma-separated system names (default: all in
#                                crates/mrs-bench/systems/ EXCEPT `reference`)
#   --divisions   <d1,d2,...>    Comma-separated division names
#                                (default: fne,feq,epu,eps,ueq,icu)
#   --time        <secs>         Per-problem time limit for ALL divisions (default: 120)
#                                Overridden per-division by --casc-times or --time-DIV.
#   --casc-times                 Use official CASC-30 times per division:
#                                  fne/feq/ueq → 240s, eps/epu → 120s,
#                                  icu → 480s, slh → 15s
#                                (overrides --time for the named divisions)
#   --time-DIV    <secs>         Override time limit for a specific division, e.g.
#                                --time-feq 120  --time-ueq 240
#                                (takes precedence over both --time and --casc-times)
#   --jobs        <N>            Parallel jobs (default: 1)
#   --output      <dir>          Output directory
#                                (default: crates/mrs-bench/results/<edition>/TIMESTAMP)
#
# Output:
#   <output>/run.csv    — one row per (problem, system)
#   <output>/run.log    — harness stderr
#
# CSV schema: edition,division,problem,system,szs_status,expected,verdict,wall_time_s,peak_memory_mb,failure_detail
#   verdict ∈ {ok, ko, unknown}
#     ok      — system status agrees with the reference answer
#     ko      — system status disagrees with the reference answer
#     unknown — system gave up / timed out, or no reference answer exists
#
# Requires: bash >= 4, bc, timeout (GNU coreutils)
# For --jobs > 1 also requires: GNU parallel OR xargs -P
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# ---------- defaults ----------
EDITION="casc-30"
SYSTEMS=""          # empty = auto-discover
DIVISIONS="fne,feq,epu,eps,ueq,icu"
TIME_LIMIT=120      # global fallback (seconds)
JOBS=1
OUTPUT=""
USE_CASC_TIMES=0    # 0 = use TIME_LIMIT for all; 1 = use CASC-30 official times

# Official CASC-30 wall-clock time limits per division (seconds).
# SLH is CPU-time limited but we approximate with wall clock here.
declare -A CASC30_TIMES=(
    [fne]=240   # FOF, no equality
    [feq]=240   # FOF, with equality
    [eps]=120   # EPR, satisfiable
    [epu]=120   # EPR, unsatisfiable
    [ueq]=240   # unit equality
    [icu]=480   # ICU
    [slh]=15    # SLH (CPU time at competition; wall clock approximation here)
    [tne]=240   # THF, no equality
    [teq]=240   # THF, with equality
    [tfi]=120   # TFA, integer arithmetic
    [tfe]=120   # TFA, real arithmetic
    [tfn]=120   # TFN (typed first-order non-theorems)
)

# Official CASC-J13 wall-clock time limits per division (seconds).
declare -A CASCJ13_TIMES=(
    [fne]=180   # FOF, no equality
    [feq]=180   # FOF, with equality
    [ueq]=180   # unit equality
    [fnn]=180   # FNT, no equality
    [fnq]=180   # FNT, with equality
    [tne]=180   # THF, no equality
    [teq]=180   # THF, with equality
    [prv]=180   # PRV
)

# Per-division overrides set via --time-DIV flags; populated during arg parsing.
declare -A DIV_OVERRIDE=()

# ---------- arg parsing ----------
while [[ $# -gt 0 ]]; do
    case "$1" in
        --edition)    EDITION="$2";    shift 2 ;;
        --systems)    SYSTEMS="$2";    shift 2 ;;
        --divisions)  DIVISIONS="$2";  shift 2 ;;
        --time)       TIME_LIMIT="$2"; shift 2 ;;
        --casc-times) USE_CASC_TIMES=1; shift ;;
        --jobs)       JOBS="$2";       shift 2 ;;
        --output)     OUTPUT="$2";     shift 2 ;;
        --time-*)
            # --time-DIV N  e.g. --time-feq 240
            div_key="${1#--time-}"   # strip "--time-"
            DIV_OVERRIDE["${div_key}"]="$2"
            shift 2
            ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

# ---------- resolve per-division time limit ----------
# Priority (highest to lowest):
#   1. --time-DIV N  (explicit per-division override)
#   2. --casc-times  (official CASC wall-clock defaults for the active edition)
#   3. --time N      (global fallback)
div_time() {
    local div="$1"
    if [[ -n "${DIV_OVERRIDE[${div}]+x}" ]]; then
        echo "${DIV_OVERRIDE[${div}]}"
    elif [[ "${USE_CASC_TIMES}" -eq 1 ]]; then
        if [[ "${EDITION}" == "casc-j13" ]]; then
            if [[ -n "${CASCJ13_TIMES[${div}]+x}" ]]; then
                echo "${CASCJ13_TIMES[${div}]}"
            else
                echo "${TIME_LIMIT}"
            fi
        else
            if [[ -n "${CASC30_TIMES[${div}]+x}" ]]; then
                echo "${CASC30_TIMES[${div}]}"
            else
                echo "${TIME_LIMIT}"
            fi
        fi
    else
        echo "${TIME_LIMIT}"
    fi
}
TIMESTAMP="$(date +%Y%m%d_%H%M%S)"
if [[ -z "${OUTPUT}" ]]; then
    OUTPUT="${SCRIPT_DIR}/results/${EDITION}/${TIMESTAMP}"
fi
mkdir -p "${OUTPUT}"

# Redirect harness stderr to run.log (tee so it still shows on terminal)
exec 2> >(tee -a "${OUTPUT}/run.log" >&2)

PROBLEMS_DIR="${SCRIPT_DIR}/problems/${EDITION}"
LISTS_DIR="${PROBLEMS_DIR}/lists"
PROBLEMS_ROOT="${PROBLEMS_DIR}"

# Set TPTP so %include directives resolve (can be overridden by caller)
if [[ -z "${TPTP:-}" ]]; then
    export TPTP="${PROBLEMS_DIR}"
fi

# ---------- discover systems ----------
# `reference` is a stub system (it just echoes the TPTP-library answer);
# scheduling it as a benchmark target wastes a run slot per problem and
# clutters results with rows whose verdict is tautologically correct.
# Skip it during auto-discovery. Users who explicitly pass
# `--systems reference` still get it (handy for verifying answers.tsv).
if [[ -z "${SYSTEMS}" ]]; then
    SYSTEMS_LIST=()
    for d in "${SCRIPT_DIR}/systems"/*/; do
        name="$(basename "${d}")"
        [[ "${name}" == "reference" ]] && continue
        if [[ -x "${d}invoke.sh" ]]; then
            SYSTEMS_LIST+=("${name}")
        fi
    done
else
    IFS=',' read -ra SYSTEMS_LIST <<< "${SYSTEMS}"
fi

if [[ ${#SYSTEMS_LIST[@]} -eq 0 ]]; then
    echo "No systems found. Add a directory under crates/mrs-bench/systems/ with an invoke.sh." >&2
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

# Reference answers file. Used inline by the worker to grade each
# system run. Missing file → every verdict is `unknown`.
ANSWERS="${SCRIPT_DIR}/systems/reference/answers_${EDITION}.tsv"
if [[ ! -f "${ANSWERS}" ]]; then
    ANSWERS="${SCRIPT_DIR}/systems/reference/answers.tsv"
fi
if [[ ! -f "${ANSWERS}" ]]; then
    echo "WARNING: reference answers not found at ${SCRIPT_DIR}/systems/reference/answers_${EDITION}.tsv or answers.tsv" >&2
    echo "         all verdicts will be reported as 'unknown'." >&2
    echo "         Run: crates/mrs-bench/systems/reference/fetch_answers.sh --edition ${EDITION}" >&2
fi

CSV="${OUTPUT}/run.csv"
echo "edition,division,problem,system,szs_status,expected,verdict,wall_time_s,peak_memory_mb,failure_detail" > "${CSV}"

JOBS_FILE="${OUTPUT}/.jobs"
> "${JOBS_FILE}"

total_problems=0
for div in "${DIVISION_LIST[@]}"; do
    list="${LISTS_DIR}/${div}.list"
    if [[ ! -f "${list}" ]]; then
        echo "WARNING: no list file for division '${div}' at ${list}" >&2
        continue
    fi
    t="$(div_time "${div}")"
    div_upper="${div^^}"
    while IFS= read -r problem || [[ -n "${problem}" ]]; do
        [[ -z "${problem}" ]] && continue
        prob_path="${PROBLEMS_ROOT}/${div_upper}/${problem}.p"
        for sys in "${SYSTEMS_LIST[@]}"; do
            # Fields: div  problem  prob_path  sys  time_limit
            printf '%s\t%s\t%s\t%s\t%s\n' \
                "${div}" "${problem}" "${prob_path}" "${sys}" "${t}" >> "${JOBS_FILE}"
            (( total_problems++ )) || true
        done
    done < "${list}"
done

# Build a human-readable summary of per-division times for the log.
div_times_summary=""
for div in "${DIVISION_LIST[@]}"; do
    t="$(div_time "${div}")"
    div_times_summary+="${div}=${t}s "
done

echo "[casc] Edition:     ${EDITION}" >&2
echo "[casc] Systems:     ${SYSTEMS_LIST[*]}" >&2
echo "[casc] Divisions:   ${DIVISION_LIST[*]}" >&2
echo "[casc] Time limits: ${div_times_summary% }" >&2
if [[ "${USE_CASC_TIMES}" -eq 1 ]]; then
    if [[ "${EDITION}" == "casc-j13" ]]; then
        echo "[casc] Mode:        --casc-times (official CASC-J13 wall-clock limits)" >&2
    else
        echo "[casc] Mode:        --casc-times (official CASC-30 wall-clock limits)" >&2
    fi
else
    echo "[casc] Mode:        --time ${TIME_LIMIT} (uniform, all divisions)" >&2
fi
echo "[casc] Jobs:        ${JOBS} (parallel)" >&2
echo "[casc] Output:      ${OUTPUT}" >&2
echo "[casc] TPTP:        ${TPTP}" >&2
echo "[casc] Total jobs:  ${total_problems}" >&2

# ---------- worker function ----------
# Arguments: div  problem  prob_path  sys  time_limit
#
# Emits one CSV row:
#   edition,division,problem,system,szs_status,expected,verdict,wall_time_s,peak_memory_mb,failure_detail
#
# `verdict` compares the system's SZS status against the reference
# answer for `problem` (from systems/reference/answers.tsv):
#   ok      — both map to the same provability class (provable /
#             counter-provable)
#   ko      — system disagrees with the reference (potential soundness
#             bug, mis-configuration, or a genuine reference error)
#   unknown — system gave up / timed out, or no reference answer
#             exists for this problem
run_one() {
    local div="$1" problem="$2" prob_path="$3" sys="$4" tlimit="$5"
    local invoke="${SCRIPT_DIR}/systems/${sys}/invoke.sh"
    local tmp tmp_err
    tmp="$(mktemp)"
    tmp_err="$(mktemp)"

    local start_ms end_ms wall_s szs exit_code
    start_ms=$(date +%s%3N)
    # Give the system tlimit seconds; add 10s grace for it to flush output.
    timeout $(( tlimit + 10 )) "${invoke}" "${prob_path}" "${tlimit}" \
        > "${tmp}" 2>"${tmp_err}"
    exit_code=$?
    end_ms=$(date +%s%3N)

    wall_s=$(echo "scale=3; (${end_ms} - ${start_ms}) / 1000" | bc)

    # If the OS timeout fired, cap wall time to the stated limit (the +10s
    # grace period would otherwise make it appear as tlimit+10).
    if [[ ${exit_code} -eq 124 ]]; then
        wall_s=$(printf '%.3f' "${tlimit}")
    fi

    # Extract SZS status from stdout.
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

    # Extract MRS's reported peak virtual memory from stdout. MRS currently
    # formats this as "% Peak memory usage: N MB" from /proc/self/status.
    local peak_memory_mb=""
    peak_memory_mb=$(awk '
        $1 == "%" && $2 == "Peak" && $3 == "memory" && $4 == "usage:" && $6 == "MB" {
            print $5
            exit
        }
    ' "${tmp}" 2>/dev/null || true)

    # Extract structured failure detail from stderr ("% SZS detail ...").
    # Stores the key=value portion; empty string if not present.
    local failure_detail=""
    failure_detail=$(grep -m1 '% SZS detail' "${tmp_err}" 2>/dev/null | sed 's/^% SZS detail //' || true)

    rm -f "${tmp}" "${tmp_err}"

    # Look up reference answer and grade.
    local expected="" verdict="unknown"
    if [[ -f "${ANSWERS}" ]]; then
        expected=$(awk -F'\t' -v p="${problem}" '$1 == p { print $2; exit }' "${ANSWERS}")
    fi
    if [[ -n "${expected}" ]]; then
        local sys_class ref_class
        sys_class=$(szs_class "${szs}")
        ref_class=$(szs_class "${expected}")
        if [[ "${sys_class}" == "inconclusive" ]]; then
            verdict="unknown"
        elif [[ "${ref_class}" == "inconclusive" ]]; then
            # Reference itself is non-committal; we cannot grade.
            verdict="unknown"
        elif [[ "${sys_class}" == "${ref_class}" ]]; then
            verdict="ok"
        else
            verdict="ko"
        fi
    fi

    printf '%s,%s,%s,%s,%s,%s,%s,%s,%s,%s\n' \
        "${EDITION}" "${div}" "${problem}" "${sys}" \
        "${szs}" "${expected}" "${verdict}" "${wall_s}" "${peak_memory_mb}" "${failure_detail}"
}

# Map an SZS status to a coarse provability class so different
# successful statuses (Theorem vs Unsatisfiable vs ContradictoryAxioms)
# compare equal. Anything we don't recognise as a definitive verdict
# (Timeout, GaveUp, ResourceOut, Unknown, …) maps to `inconclusive`.
szs_class() {
    case "$1" in
        Theorem|Unsatisfiable|ContradictoryAxioms)
            echo "provable" ;;
        CounterSatisfiable|Satisfiable)
            echo "counter" ;;
        *)
            echo "inconclusive" ;;
    esac
}
export -f run_one szs_class
export SCRIPT_DIR EDITION ANSWERS

# ---------- execute ----------

# Worker shared by both serial and parallel paths. Appends one CSV row
# (under flock so concurrent writers don't interleave) and refreshes a
# single-line carriage-return progress counter. Progress is computed
# by counting CSV lines so it works whether one or many workers are
# active. Per-row grading is recorded in the CSV (`verdict` column)
# rather than printed, to keep the live output a single line.
run_and_append() {
    local line="$1"
    IFS=$'\t' read -r div problem prob_path sys tlimit <<< "${line}"
    local row
    row=$(run_one "${div}" "${problem}" "${prob_path}" "${sys}" "${tlimit}")
    local completed
    (
        flock 9
        printf '%s\n' "${row}" >> "${CSV}"
        # Subtract 1 for the header.
        completed=$(($(wc -l < "${CSV}") - 1))
        printf '\r[casc] %d/%d completed' "${completed}" "${total_problems}" >&2
    ) 9>>"${CSV}.lock"
}
export -f run_and_append
export CSV total_problems

if [[ "${JOBS}" -le 1 ]]; then
    while IFS= read -r line; do
        run_and_append "${line}"
    done < "${JOBS_FILE}"
else
    # Parallel execution. Each worker calls run_and_append directly so the
    # progress counter advances in real time (instead of only at the end).
    if command -v parallel &>/dev/null && parallel --version 2>/dev/null | grep -q "GNU"; then
        parallel --jobs "${JOBS}" --will-cite \
            'run_and_append {}' :::: "${JOBS_FILE}"
    else
        # xargs -P launches fresh bash subprocesses that don't inherit exported
        # bash functions (export -f is a bashism that doesn't cross process
        # boundaries via xargs).  Work around this by passing the worker body
        # as a here-string that sources the required functions inline.
        xargs -P "${JOBS}" -I '{}' bash -c "$(declare -f run_one szs_class run_and_append); run_and_append \"{}\""  < "${JOBS_FILE}"
    fi
fi

printf '\n' >&2
rm -f "${JOBS_FILE}" "${CSV}.lock"
echo "[casc] Done. Results: ${CSV}" >&2
