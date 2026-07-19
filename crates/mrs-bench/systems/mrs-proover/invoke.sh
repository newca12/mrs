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

exec timeout --foreground "${SOFT}s" "${BINARY}" "${ARGS[@]}" "${PROOF}" \
    || echo "% SZS status Unknown : exhausted wall-clock budget"
