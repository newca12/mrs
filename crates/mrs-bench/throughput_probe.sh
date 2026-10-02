#!/usr/bin/env bash
# Fixed-wall-clock throughput probe for a given mrs binary.
#
# Usage:
#   throughput_probe.sh <binary> <label> <output.csv> [time] [list-file]
#
# Runs a fixed problem list for a fixed wall clock per problem and records the
# search telemetry the prover already reports.
#
# The unit of measurement is *work retired* (`processed`, `generated`), not
# clauses-per-second and not elapsed time. The search is wall-clock sensitive:
# a given-clause iteration's cost depends on the clause database it runs
# against, and LRS prunes against the clock, so "clauses in 30 s" moves with
# host speed. Retiring more clauses out of an identical budget on one host is
# the only throughput statement that transfers. `docs/results/perf/README.md`
# makes the same argument for the fixed-work clause-count probe, and
# `docs/reports/benchmarks/fne-2026-09.md` §1 measures the host band that makes
# the wall-clock reading untrustworthy.
#
# Runs strictly sequentially. Two jobs sharing a core each retire fewer clauses
# for the same budget, so a parallel run is not comparable with a sequential
# one; `PROBE_JOBS` exists to make that mistake explicit rather than to enable
# it silently.
#
# Each list line is `edition/division/problem`, resolved against the editions
# checked in under `crates/mrs-bench/problems`. A missing problem is skipped
# loudly rather than silently dropped, so a list can span `casc-30` and
# `casc-j13` without pretending the two editions are one corpus.

set -euo pipefail

BINARY="${1:?usage: $0 <binary> <label> <output.csv> [time] [list-file]}"
LABEL="${2:?usage: $0 <binary> <label> <output.csv> [time] [list-file]}"
OUTPUT="${3:?usage: $0 <binary> <label> <output.csv> [time] [list-file]}"
TIME_LIMIT="${4:-12}"
LIST_FILE="${5:-${SCRIPT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)}/throughput_probe_list.txt}"
JOBS="${PROBE_JOBS:-1}"
BENCH_DIR="${MRS_BENCH_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)}"

if [[ ! -x "${BINARY}" ]]; then
    echo "Error: ${BINARY} is not executable." >&2
    exit 1
fi
if [[ ! -f "${LIST_FILE}" ]]; then
    echo "Error: no problem list at ${LIST_FILE}" >&2
    exit 1
fi

mapfile -t ENTRIES < <(grep -v -e '^#' -e '^[[:space:]]*$' "${LIST_FILE}")
if [[ "${#ENTRIES[@]}" -eq 0 ]]; then
    echo "Error: ${LIST_FILE} lists no problems." >&2
    exit 1
fi

mkdir -p "$(dirname "${OUTPUT}")"
BIN_SHA="$(sha256sum "${BINARY}" | cut -c1-16)"
echo "label,problem,result,elapsed_ms,processed,generated,passive,lrs_discarded,weight_discarded,fwd_subsumed,shares_binary" > "${OUTPUT}"

run_one() {
    local entry="$1" file division
    file="${BENCH_DIR}/problems/${entry}.p"
    division="$(echo "${entry}" | cut -d/ -f2 | tr '[:upper:]' '[:lower:]')"
    if [[ ! -f "${file}" ]]; then
        echo "  [skip] ${entry}: no such problem" >&2
        return 0
    fi
    local line
    line="$(
        MRS_SHARED_POOL_INTERVAL=0 timeout $((TIME_LIMIT + 20)) \
            "${BINARY}" --time "${TIME_LIMIT}" --workers 1 --schedule "casc_${division}" \
            "${file}" 2>&1 || true
    )"
    local status detail
    status="$(sed -n 's/^% SZS status \([A-Za-z]*\).*/\1/p' <<<"${line}" | head -1)"
    detail="$(sed -n 's/^% SZS detail //p' <<<"${line}" | head -1)"
    [[ -n "${status}" ]] || status="NoStatus"
    # Peel each `k=v` off a copy of the line. Two variables are needed: `rest`
    # is the unconsumed tail that the next `#* k=` search reads, and `value` is
    # the field just cut out of it. Consuming a single variable makes every
    # column after the first repeat it.
    local rest="${detail}" value field
    for field in elapsed_ms processed generated passive lrs_discarded weight_discarded fwd_subsumed; do
        rest="${rest#* ${field}=}"
        value="${rest%% *}"
        [[ "${value}" =~ ^[0-9]+$ ]] || value=""
        printf -v "${field}" '%s' "${value}"
    done
    echo "${LABEL},${entry},${status},${elapsed_ms},${processed},${generated},${passive},${lrs_discarded},${weight_discarded},${fwd_subsumed},${BIN_SHA}" >> "${OUTPUT}"
    echo "  ${entry}: ${status} processed=${processed} generated=${generated}" >&2
}

export -f run_one
export BINARY LABEL OUTPUT TIME_LIMIT BIN_SHA BENCH_DIR

echo "[probe] binary=${BINARY} (${BIN_SHA}) label=${LABEL} time=${TIME_LIMIT}s jobs=${JOBS}" >&2
echo "[probe] list=${LIST_FILE} count=${#ENTRIES[@]}" >&2

if [[ "${JOBS}" -le 1 ]]; then
    for entry in "${ENTRIES[@]}"; do run_one "${entry}"; done
else
    printf '%s\n' "${ENTRIES[@]}" | xargs -P "${JOBS}" -I{} bash -c 'run_one "$@"' _ {}
fi

echo "[probe] wrote ${OUTPUT}" >&2
