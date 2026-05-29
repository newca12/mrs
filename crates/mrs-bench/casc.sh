#!/usr/bin/env bash
# crates/mrs-bench/casc.sh
# Run a CASC benchmark: invoke each registered system on each selected problem,
# collect SZS status + wall time, and write a CSV.
#
# Usage:
#   crates/mrs-bench/casc.sh [OPTIONS]
#
# Options:
#   --edition   <name>         Competition edition directory (default: casc-30)
#   --systems   <s1,s2,...>    Comma-separated system names (default: all in
#                              crates/mrs-bench/systems/ EXCEPT `reference`,
#                              which is a stub system retained only for
#                              regenerating answers.tsv)
#   --divisions <d1,d2,...>    Comma-separated division names (default: fne,feq,epu,eps,ueq,icu)
#   --time      <secs>         Per-problem time limit in seconds (default: 120)
#   --jobs      <N>            Parallel jobs (default: 1)
#   --output    <dir>          Output directory (default: crates/mrs-bench/results/<edition>/TIMESTAMP)
#
# Output:
#   <output>/run.csv    — one row per (problem, system)
#   <output>/run.log    — harness stderr
#
# CSV schema: edition,division,problem,system,szs_status,expected,verdict,wall_time_s
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
ANSWERS="${SCRIPT_DIR}/systems/reference/answers.tsv"
if [[ ! -f "${ANSWERS}" ]]; then
    echo "WARNING: reference answers not found at ${ANSWERS}" >&2
    echo "         all verdicts will be reported as 'unknown'." >&2
    echo "         Run: crates/mrs-bench/systems/reference/fetch_answers.sh" >&2
fi

CSV="${OUTPUT}/run.csv"
echo "edition,division,problem,system,szs_status,expected,verdict,wall_time_s" > "${CSV}"

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
        div_upper="${div^^}"
        prob_path="${PROBLEMS_ROOT}/${div_upper}/${problem}.p"
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
#
# Emits one CSV row:
#   edition,division,problem,system,szs_status,expected,verdict,wall_time_s
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

    # If the OS timeout fired, cap wall time to the stated limit (the +10s
    # grace period would otherwise make it appear as TIME_LIMIT+10).
    if [[ ${exit_code} -eq 124 ]]; then
        wall_s=$(printf '%.3f' "${TIME_LIMIT}")
    fi

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

    printf '%s,%s,%s,%s,%s,%s,%s,%s\n' \
        "${EDITION}" "${div}" "${problem}" "${sys}" \
        "${szs}" "${expected}" "${verdict}" "${wall_s}"
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
export SCRIPT_DIR TIME_LIMIT EDITION ANSWERS

# ---------- execute ----------
GRACE=$((TIME_LIMIT + 10))

# Worker shared by both serial and parallel paths. Appends one CSV row
# (under flock so concurrent writers don't interleave) and prints a
# completion line that names the just-finished (problem,system) and
# its ok/ko/?? marker. Without the marker users would have to wait
# for the run to finish to learn whether the engine agreed with the
# reference; the previous flow paired a fake `reference` row next to
# each real row, which became unreadable as soon as --jobs > 1
# interleaved the output.
run_and_append() {
    local line="$1"
    IFS=$'\t' read -r div problem prob_path sys <<< "${line}"
    local row
    row=$(run_one "${div}" "${problem}" "${prob_path}" "${sys}")
    # Pull `szs_status` (col 5) and `verdict` (col 7) back out for display.
    local szs verdict marker
    szs=$(echo "${row}" | awk -F, '{print $5}')
    verdict=$(echo "${row}" | awk -F, '{print $7}')
    case "${verdict}" in
        ok)      marker="OK" ;;
        ko)      marker="KO" ;;
        unknown) marker="??" ;;
        *)       marker="--" ;;
    esac
    local completed
    (
        flock 9
        printf '%s\n' "${row}" >> "${CSV}"
        # Subtract 1 for the header.
        completed=$(($(wc -l < "${CSV}") - 1))
        printf '[casc] %s %-12s %-12s %-14s (%d/%d)\n' \
            "${marker}" "${sys}" "${problem}" "${szs}" \
            "${completed}" "${total_problems}" >&2
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
    if command -v parallel &>/dev/null; then
        parallel --jobs "${JOBS}" --will-cite \
            'run_and_append {}' :::: "${JOBS_FILE}"
    else
        xargs -P "${JOBS}" -I '{}' bash -c 'run_and_append "$@"' _ '{}' \
            < "${JOBS_FILE}"
    fi
fi

printf '\n' >&2
rm -f "${JOBS_FILE}" "${CSV}.lock"
echo "[casc] Done. Results: ${CSV}" >&2
