#!/usr/bin/env bash
# Load and, if needed, build the shared Rust tooling binary for formal scripts.

_FORMAL_TOOLS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
_FORMAL_WORKSPACE_ROOT="$(cd "${_FORMAL_TOOLS_DIR}/../.." && pwd)"
source "${_FORMAL_WORKSPACE_ROOT}/crates/mrs-bench/tools.sh"

formal_tools_setup() {
    if [[ -n "${MRS_TOOLS_BIN:-}" && -x "${MRS_TOOLS_BIN}" ]]; then
        return 0
    fi
    if [[ -n "${IN_NIX_SHELL:-}" ]] && command -v nix >/dev/null 2>&1; then
        (cd "${_FORMAL_WORKSPACE_ROOT}" && nix develop .#default -c cargo build -p mrs-bench --bin mrs_tools)
        export MRS_TOOLS_BIN="${_FORMAL_WORKSPACE_ROOT}/target/debug/mrs_tools"
        return 0
    fi
    (cd "${_FORMAL_WORKSPACE_ROOT}" && cargo build -p mrs-bench --bin mrs_tools)
    export MRS_TOOLS_BIN="${_FORMAL_WORKSPACE_ROOT}/target/debug/mrs_tools"
}
