#!/usr/bin/env bash
# Search cooperative 8-worker portfolios by measuring the actual shared-pool
# solver, not the union of independent strategy results.
#
# Usage:
#   cooperative_portfolio_search.sh <edition> <division> [time] [jobs] [rounds] [output]
#
# Each one-swap candidate runs through cooperative_portfolio_sweep.sh.  The
# objective is the number of reference-agreeing solved problems in that actual
# portfolio run.  This is intentionally slower than solo set-cover: it
# measures strategy interaction, shared equality propagation, and slot order.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

EDITION="${1:?usage: $0 <edition> <division> [time] [jobs] [rounds] [output]}"
DIVISION="${2:?usage: $0 <edition> <division> [time] [jobs] [rounds] [output]}"
TIME_LIMIT="${3:-30}"
JOBS="${4:-1}"
ROUNDS="${5:-1}"
OUTPUT="${6:-${SCRIPT_DIR}/results/${EDITION}/cooperative-search-${DIVISION}-$(date +%Y%m%d_%H%M%S)}"

case "${DIVISION,,}" in
    feq) CURRENT=(11 12 1 6 10 8 14 4) ;;
    fne) CURRENT=(11 4 12 1 6 8 2 3) ;;
    ueq) CURRENT=(11 4 2 8 14 1 15 3) ;;
    epr) CURRENT=(6 2 1 3 4 5 7 8) ;;
    eps) CURRENT=(2 3 1 8 11 12 9 14) ;;
    epu) CURRENT=(1 6 14 11 4 2 3 7) ;;
    icu) CURRENT=(12 1 2 3 4 5 6 7) ;;
    *) echo "Error: unsupported division '${DIVISION}'" >&2; exit 1 ;;
esac

if ! [[ "${ROUNDS}" =~ ^[1-9][0-9]*$ ]]; then
    echo "Error: rounds must be a positive integer" >&2
    exit 1
fi

mkdir -p "${OUTPUT}"
RESULTS="${OUTPUT}/search.tsv"
printf 'round\tportfolio\tsolved\trun_csv\tshared_interval\n' > "${RESULTS}"

declare -A SCORES=()
declare -A RUNS=()

portfolio_key() {
    local IFS=,
    printf '%s' "$*"
}

score_run() {
    local csv="$1"
    awk -F',' 'NR > 1 && $7 == "ok" { solved[$3] = 1 } END { print length(solved) + 0 }' "${csv}"
}

evaluate() {
    local round="$1"
    local label="$2"
    local portfolio="$3"
    local sharing="$4"
    local key
    key="${portfolio}|${sharing}"
    if [[ -n "${SCORES[${key}]+x}" ]]; then
        printf '%s\n' "${SCORES[${key}]}"
        return
    fi

    local run_dir="${OUTPUT}/r${round}-${label}-$(echo "${portfolio}" | tr ',' '-')"
    local run_csv="${run_dir}/run.csv"
    if [[ "${sharing}" == "0" ]]; then
        MRS_SHARED_POOL_INTERVAL=0 \
            "${SCRIPT_DIR}/cooperative_portfolio_sweep.sh" \
            "${EDITION}" "${DIVISION}" "${portfolio}" "${TIME_LIMIT}" "${JOBS}" "${run_dir}" \
            > "${run_dir}.out" 2> "${run_dir}.err"
    else
        "${SCRIPT_DIR}/cooperative_portfolio_sweep.sh" \
            "${EDITION}" "${DIVISION}" "${portfolio}" "${TIME_LIMIT}" "${JOBS}" "${run_dir}" \
            > "${run_dir}.out" 2> "${run_dir}.err"
    fi
    local score
    score="$(score_run "${run_csv}")"
    SCORES["${key}"]="${score}"
    RUNS["${key}"]="${run_csv}"
    printf '%s\t%s\t%s\t%s\t%s\n' \
        "${round}" "${portfolio}" "${score}" "${run_csv}" "${sharing}" >> "${RESULTS}"
    printf '%s\n' "${score}"
}

join_ids() {
    local IFS=,
    printf '%s' "$*"
}

current_portfolio="$(join_ids "${CURRENT[@]}")"
best_score="$(evaluate 0 initial "${current_portfolio}" 1)"
echo "Initial cooperative portfolio ${current_portfolio}: ${best_score} solved" >&2

for round in $(seq 1 "${ROUNDS}"); do
    improved=0
    round_best_score="${best_score}"
    round_best_portfolio="${current_portfolio}"

    for slot in "${!CURRENT[@]}"; do
        for candidate in $(seq 1 15); do
            already=0
            for selected in "${CURRENT[@]}"; do
                [[ "${selected}" == "${candidate}" ]] && already=1
            done
            (( already == 1 )) && continue

            trial=("${CURRENT[@]}")
            trial["${slot}"]="${candidate}"
            trial_key="$(join_ids "${trial[@]}")"
            score="$(evaluate "${round}" "s${slot}-c${candidate}" "${trial_key}" 1)"
            echo "round=${round} slot=${slot} candidate=${candidate} portfolio=${trial_key} solved=${score}" >&2
            if (( score > round_best_score )); then
                round_best_score="${score}"
                round_best_portfolio="${trial_key}"
            fi
        done
    done

    if (( round_best_score <= best_score )); then
        break
    fi
    best_score="${round_best_score}"
    current_portfolio="${round_best_portfolio}"
    IFS=',' read -r -a CURRENT <<< "${current_portfolio}"
    echo "Accepted round ${round}: ${current_portfolio} (${best_score} solved)" >&2
done

control_score="$(evaluate "${ROUNDS}-control" no-sharing "${current_portfolio}" 0)"
echo "Best cooperative portfolio: ${current_portfolio} (${best_score} solved)" >&2
echo "Same portfolio without shared pool: ${control_score} solved" >&2
echo "Results: ${RESULTS}" >&2
