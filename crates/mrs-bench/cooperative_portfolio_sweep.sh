#!/usr/bin/env bash
# Run one real cooperative portfolio through casc.sh.
#
# Usage:
#   cooperative_portfolio_sweep.sh <edition> <division> <portfolio> [time] [jobs] [output]
#
# Example:
#   cooperative_portfolio_sweep.sh casc-30 feq 11,12,1,6,10,8,14,4 30 4 results/coop-feq
#
# Unlike run_codex_sweep.sh/run_strategy_sweep.sh, this launches one mrs
# process per problem with the requested number of workers. The workers share
# the equality pool exactly as the competition run does. Set
# MRS_SHARED_POOL_INTERVAL=0 for the no-sharing control.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

EDITION="${1:?usage: $0 <edition> <division> <portfolio> [time] [jobs] [output]}"
DIVISION="${2:?usage: $0 <edition> <division> <portfolio> [time] [jobs] [output]}"
PORTFOLIO="${3:?usage: $0 <edition> <division> <portfolio> [time] [jobs] [output]}"
TIME_LIMIT="${4:-30}"
JOBS="${5:-1}"
OUTPUT="${6:-${SCRIPT_DIR}/results/${EDITION}/cooperative-${DIVISION}-$(date +%Y%m%d_%H%M%S)}"

if ! [[ "${PORTFOLIO}" =~ ^[1-9][0-5]*(,[1-9][0-5]*)*$ ]]; then
    echo "Error: portfolio must be comma-separated strategy IDs in 1..15." >&2
    exit 1
fi

WORKERS="${MRS_WORKERS:-8}"
portfolio_count="$(tr ',' '\n' <<< "${PORTFOLIO}" | wc -l)"
if [[ "${portfolio_count}" -ne "${WORKERS}" ]]; then
    echo "Error: portfolio has ${portfolio_count} strategies but MRS_WORKERS=${WORKERS}." >&2
    exit 1
fi

mkdir -p "${OUTPUT}"

echo "[coop] edition=${EDITION} division=${DIVISION} portfolio=${PORTFOLIO}" >&2
echo "[coop] time=${TIME_LIMIT}s jobs=${JOBS} workers=${WORKERS}" >&2
echo "[coop] shared_pool_interval=${MRS_SHARED_POOL_INTERVAL:-500}" >&2

export MRS_PORTFOLIO="${PORTFOLIO}"
export MRS_WORKERS="${WORKERS}"

exec "${SCRIPT_DIR}/casc.sh" \
    --edition "${EDITION}" \
    --systems mrs \
    --divisions "${DIVISION}" \
    --time "${TIME_LIMIT}" \
    --jobs "${JOBS}" \
    --output "${OUTPUT}"
