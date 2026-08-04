#!/usr/bin/env bash
# Compatibility wrapper for the Rust ProoVer 2026 scorer.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
BIN="${WORKSPACE_ROOT}/target/release/score_proover2026"
ROOT="${WORKSPACE_ROOT}/crates/mrs-bench/proover-corpus/Proover2026"
FORWARD=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        --manifest)
            manifest="$2"
            ROOT="$(cd "$(dirname "${manifest}")" && pwd)"
            shift 2
            ;;
        *)
            FORWARD+=("$1")
            shift
            ;;
    esac
done

if [[ ! -x "${BIN}" ]]; then
    (cd "${WORKSPACE_ROOT}" && nix develop -c cargo build --release -p mrs-bench --bin score_proover2026)
fi

exec "${BIN}" "${ROOT}" "${FORWARD[@]}"
