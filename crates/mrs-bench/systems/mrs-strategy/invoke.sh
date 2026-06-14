#!/usr/bin/env bash
# crates/mrs-bench/systems/mrs-strategy/invoke.sh
#
# Parametric single-strategy mrs invocation for per-strategy benchmarking.
#
# The strategy number is embedded in the system name passed by casc.sh.
# Name the system "mrs-s01" through "mrs-s16" and this script derives
# MRS_SINGLE_STRATEGY=N automatically.
#
# Usage (via casc.sh):
#   casc.sh --systems mrs-s01,mrs-s02,...,mrs-s15 --divisions fne --time 30 ...
#
# The resulting run.csv has each strategy as a distinct "system" column value,
# which greedy_set_cover can use directly:
#   greedy_set_cover run.csv 8 --division fne
#
# Usage (direct):
#   invoke.sh <problem_path> <time_limit_secs>
#
# Writes all output to stdout; exits with any code.
set -euo pipefail

PROBLEM="${1:?Usage: invoke.sh <problem_path> <time_limit_secs>}"
TIME_LIMIT="${2:?Usage: invoke.sh <problem_path> <time_limit_secs>}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../../../.." && pwd)"

BINARY="${WORKSPACE_ROOT}/target/release/mrs"
if [[ ! -x "${BINARY}" ]]; then
    echo "% SZS status Error (mrs binary not found; run: cargo build --release)"
    exit 1
fi

# Derive strategy number from the name of this script's parent directory.
# casc.sh invokes systems/<system>/invoke.sh, so the directory name IS the system name.
# Expected format: mrs-sNN  (e.g. mrs-s01, mrs-s10, mrs-s15)
SYSTEM_NAME="$(basename "${SCRIPT_DIR}")"
STRATEGY_NUM="${SYSTEM_NAME##mrs-s}"  # strip leading "mrs-s"
# Strip leading zeros so e.g. "01" becomes "1" (shell arithmetic)
STRATEGY_NUM="${STRATEGY_NUM#0}"

# Validate: must be a number in [1..16]
if ! [[ "${STRATEGY_NUM}" =~ ^[0-9]+$ ]] || (( STRATEGY_NUM < 1 || STRATEGY_NUM > 16 )); then
    echo "% SZS status Error (mrs-strategy: cannot determine strategy number from directory '${SYSTEM_NAME}')"
    exit 1
fi

# Set TPTP root so %include directives resolve.
if [[ -z "${TPTP:-}" ]]; then
    export TPTP="${SCRIPT_DIR}/../../problems/casc-30"
fi

# Run with a single strategy and 1 worker (isolates that strategy's performance).
exec env MRS_SINGLE_STRATEGY="${STRATEGY_NUM}" \
    "${BINARY}" --time "${TIME_LIMIT}" --workers 1 "${PROBLEM}"
