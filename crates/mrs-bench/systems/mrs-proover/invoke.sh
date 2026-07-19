#!/usr/bin/env bash
# crates/mrs-bench/systems/mrs-proover/invoke.sh
# Usage: invoke.sh <proof_path> <time_limit_secs>
#
# Verifies the TSTP proof at <proof_path>, with the given wall-clock budget
# (in seconds). Emits exactly one SZS line on stdout.
set -euo pipefail

PROOF="${1:?Usage: invoke.sh <proof_path> <time_limit_secs>}"
TIME_LIMIT="${2:?Usage: invoke.sh <proof_path> <time_limit_secs>}"

# Resolve workspace root.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../../../.." && pwd)"

BINARY="${WORKSPACE_ROOT}/target/release/mrs-proover"
if [[ ! -x "${BINARY}" ]]; then
    echo "% SZS status Unknown : mrs-proover binary not found; run: cargo build --release -p mrs-proover"
    exit 0
fi

# Locate bundled ATP backends if present.
EPROVER="${WORKSPACE_ROOT}/crates/mrs-bench/systems/eprover/bin/eprover"
VAMPIRE="${WORKSPACE_ROOT}/crates/mrs-bench/systems/vampire/bin/vampire"

ARGS=()
# The proof file usually contains `% Proof : Problems/foo.p`, where the path
# is relative to the directory containing the proof. Default behaviour is to
# resolve it that way. Setting `--problems-dir` to the proof's parent makes
# the lookup robust to whatever absolute path the harness uses.
PROOF_DIR="$(cd "$(dirname "${PROOF}")" && pwd)"
ARGS+=(--problems-dir "${PROOF_DIR}")

if [[ -x "${EPROVER}" ]]; then
    ARGS+=(--eprover "${EPROVER}")
fi
if [[ -x "${VAMPIRE}" ]]; then
    ARGS+=(--vampire "${VAMPIRE}")
fi

# Run with a wall-clock timeout one second below the harness limit so we
# always emit something rather than being killed. Never let this evaluate
# to 0: GNU `timeout 0s <cmd>` disables the timeout entirely (runs
# unbounded) rather than killing immediately.
SOFT=$(( TIME_LIMIT > 1 ? TIME_LIMIT - 1 : 1 ))

# Raise the stack limit for the parsing/DAG-building phase (load() and
# dag::build() both run on the main thread, before the parallel ATP-ladder
# verification pass spawns worker threads -- see docs/STATUS.md). Both use
# mrs-tptp's recursive-descent parser, which crates/mrs-tptp/doc/technical.md
# documents as a stack-overflow risk on deeply nested formulas. Best-effort:
# some sandboxes cap the hard limit and refuse to raise the soft limit
# further, which prints a warning but does not abort under `set -e`.
ulimit -s unlimited 2>/dev/null || true

# `ulimit -s` above only covers the main thread. The parallel ATP-ladder
# verification pass (std::thread::scope in verify.rs, default 8 workers)
# spawns worker threads with Rust's own runtime default of 2 MiB
# (DEFAULT_MIN_STACK_SIZE, smaller than the typical 8 MiB main-thread
# default) unless RUST_MIN_STACK is set in the environment before the
# process starts. Those threads run genuinely recursive code from
# mrs-unify/mrs-core/mrs-index on every step (including the MrsAtp
# in-process fallback, which runs a full given-clause search) -- a stack
# overflow there triggers Rust's abort() handler, killing the whole
# process with zero output, which is worse than a clean timeout. 64 MiB
# matches the precedent already set by crates/mrs-tptp/examples/
# parse_folder.rs's stack_size(64 * 1024 * 1024) and gives on the order of
# 300,000 levels of recursion headroom -- comfortably more than any real
# TPTP problem's term nesting depth, at negligible cost (thread stacks are
# lazily-committed virtual memory, not counted against RSS until used).
export RUST_MIN_STACK=67108864

exec timeout --foreground "${SOFT}s" "${BINARY}" "${ARGS[@]}" "${PROOF}" \
    || echo "% SZS status Unknown : exhausted wall-clock budget"
