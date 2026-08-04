#!/usr/bin/env bash
# Compatibility wrapper for the Rust ProoVer 2026 corpus validator.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
BIN="${WORKSPACE_ROOT}/target/release/validate_proover2026"

if [[ ! -x "${BIN}" ]]; then
    (cd "${WORKSPACE_ROOT}" && nix develop -c cargo build --release -p mrs-bench --bin validate_proover2026)
fi

exec "${BIN}" \
    "${WORKSPACE_ROOT}/crates/mrs-bench/proover-corpus/Proover2026" \
    "$@"
