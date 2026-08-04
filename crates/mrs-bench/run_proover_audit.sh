#!/usr/bin/env bash
# Compatibility wrapper for the Rust audit_proover binary.
#
# Usage: run_proover_audit.sh [list-file] [report-file]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
LIST_FILE="${1:-${SCRIPT_DIR}/fof_non_theorems.list}"
REPORT_FILE="${2:-${WORKSPACE_ROOT}/proover_audit.csv}"
TPTP_DIR="${TPTP:-}"
TIMEOUT_SECS="${MRS_AUDIT_TIMEOUT:-30}"
WORKERS="${MRS_WORKERS:-8}"
JOBS="${MRS_CODEX_JOBS:-1}"
MODE="${MRS_AUDIT_MODE:-competition}"

if [[ -z "${TPTP_DIR}" || ! -d "${TPTP_DIR}" ]]; then
    echo "Error: set TPTP to the TPTP library root." >&2
    exit 1
fi
if [[ ! -f "${LIST_FILE}" ]]; then
    echo "Error: list file not found: ${LIST_FILE}" >&2
    exit 1
fi

BIN="${WORKSPACE_ROOT}/target/release/audit_proover"
if [[ ! -x "${BIN}" ]]; then
    (cd "${WORKSPACE_ROOT}" && nix develop -c cargo build --release -p mrs-bench --bin audit_proover)
fi

MODE_ARG="--competition"
if [[ "${MODE}" == "kernel" ]]; then
    MODE_ARG="--kernel"
elif [[ "${MODE}" != "competition" ]]; then
    echo "Error: MRS_AUDIT_MODE must be kernel or competition." >&2
    exit 1
fi

exec "${BIN}" \
    --list "${LIST_FILE}" \
    --tptp "${TPTP_DIR}" \
    --mrs "${WORKSPACE_ROOT}/target/release/mrs" \
    --proover "${WORKSPACE_ROOT}/target/release/mrs-proover" \
    --timeout "${TIMEOUT_SECS}" \
    --workers "${WORKERS}" \
    --jobs "${JOBS}" \
    "${MODE_ARG}" \
    --output "${REPORT_FILE}"
