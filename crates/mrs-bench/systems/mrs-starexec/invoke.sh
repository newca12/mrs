#!/usr/bin/env bash
# crates/mrs-bench/systems/mrs-starexec/invoke.sh
#
# Simulates StarExec Miami by setting STAREXEC_WALLCLOCK_LIMIT and running
# the official competition entry script: systems/mrs/starexec_run_default.
#
# casc.sh interface: invoke.sh <problem_path> <time_limit_secs>
# Writes all output to stdout; exits with any code.
set -euo pipefail

PROBLEM_PATH="${1:?Usage: invoke.sh <problem_path> <time_limit_secs>}"
TIME_LIMIT="${2:?Usage: invoke.sh <problem_path> <time_limit_secs>}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../../../../" && pwd)"
MRS_SYSTEM_DIR="${SCRIPT_DIR}/../mrs"

# Ensure the current compiled release binary is next to starexec_run_default.
# certification_campaign.sh rebuilds target/release/mrs before search; refresh
# this local StarExec package copy when that build is newer, rather than silently
# measuring a previously copied binary.
SOURCE_BINARY="${MRS_BINARY:-${WORKSPACE_ROOT}/target/release/mrs}"
if [[ "${SOURCE_BINARY}" != /* ]]; then
    SOURCE_BINARY="${PWD}/${SOURCE_BINARY}"
fi
INSTALLED_BINARY="${MRS_SYSTEM_DIR}/mrs"
if [[ ! -x "${SOURCE_BINARY}" ]]; then
    echo "% SZS status Error (mrs binary not found; run: cargo build --release)"
    exit 1
fi
if [[ ! -x "${INSTALLED_BINARY}" || "${SOURCE_BINARY}" -nt "${INSTALLED_BINARY}" ]]; then
    cp -- "${SOURCE_BINARY}" "${INSTALLED_BINARY}"
fi

# Simulate StarExec environment variables
export STAREXEC_WALLCLOCK_LIMIT="${TIME_LIMIT}"
export MRS_WORKERS="${MRS_WORKERS:-8}"

# Run the real competition entry script
exec "${MRS_SYSTEM_DIR}/starexec_run_default" "${PROBLEM_PATH}"
