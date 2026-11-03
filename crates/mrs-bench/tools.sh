#!/usr/bin/env bash
# Shared launcher for the Rust benchmark and analysis tools.
# Source this from a harness script, then call `mrs_tools <subcommand> ...`.

_MRS_BENCH_TOOLS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
_MRS_WORKSPACE_ROOT="$(cd "${_MRS_BENCH_TOOLS_DIR}/../.." && pwd)"

mrs_tools() {
    local binary="${MRS_TOOLS_BIN:-}"
    if [[ -z "${binary}" ]]; then
        if command -v cargo >/dev/null 2>&1; then
            echo "[mrs-tools] checking Rust tooling binary" >&2
            (cd "${_MRS_WORKSPACE_ROOT}" && cargo build --release -p mrs-bench --bin mrs_tools) >&2 || return
            binary="${_MRS_WORKSPACE_ROOT}/target/release/mrs_tools"
        elif [[ -x "${_MRS_WORKSPACE_ROOT}/target/release/mrs_tools" ]]; then
            binary="${_MRS_WORKSPACE_ROOT}/target/release/mrs_tools"
        elif [[ -x "${_MRS_WORKSPACE_ROOT}/target/debug/mrs_tools" ]]; then
            binary="${_MRS_WORKSPACE_ROOT}/target/debug/mrs_tools"
        else
            echo "mrs-tools is not built and cargo is unavailable" >&2
            return 127
        fi
    fi
    if [[ -z "${binary}" || ! -x "${binary}" ]]; then
        echo "mrs-tools is not built; run: cargo build --release -p mrs-bench --bin mrs_tools" >&2
        return 127
    fi
    export MRS_TOOLS_BIN="${binary}"
    "${binary}" "$@"
}
