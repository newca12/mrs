#!/usr/bin/env bash
# crates/mrs-bench/systems/mrs/invoke.sh
# Usage: invoke.sh <problem_path> <time_limit_secs>
# Writes all output to stdout; exits with any code.
set -euo pipefail

PROBLEM="${1:?Usage: invoke.sh <problem_path> <time_limit_secs>}"
TIME_LIMIT="${2:?Usage: invoke.sh <problem_path> <time_limit_secs>}"

# Resolve workspace root: four levels above this script
# (crates/mrs-bench/systems/mrs/ -> systems/ -> mrs-bench/ -> crates/ -> root)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../../../.." && pwd)"

BINARY="${WORKSPACE_ROOT}/target/release/mrs"
if [[ ! -x "${BINARY}" ]]; then
    echo "% SZS status Error (mrs binary not found; run: cargo build --release)"
    exit 1
fi

# Set TPTP root so %include directives resolve.
# Prefer an already-set TPTP env var; fall back to the CASC-30 extracted archive.
if [[ -z "${TPTP:-}" ]]; then
    export TPTP="${SCRIPT_DIR}/../../problems/casc-30"
fi

# Determine the CASC division from the file path
DIVISION=$(basename $(dirname "$PROBLEM"))
DIV_LOWER="${DIVISION,,}"

# Select the appropriate static schedule
SCHEDULE="casc_${DIV_LOWER}"

# Map CASC division names to available named schedules.
# EPS (satisfiable) and EPU (unsatisfiable) now have dedicated data-driven
# schedules; casc_epr is kept as a generic fallback.
case "${SCHEDULE}" in
    casc_feq|casc_fne|casc_ueq|casc_icu) ;;   # already have dedicated schedules
    casc_eps) ;;                               # EPS: s1-first (greedy optimal)
    casc_epu) ;;                               # EPU: s4-first (greedy optimal)
    casc_epr) ;;                               # generic EPR fallback
    *) SCHEDULE="casc" ;;                      # fallback for other divisions
esac

# Run with an internal deadline slightly below the harness limit so mrs's
# own time-check fires and prints its SZS status line (Refutation/GaveUp/
# Timeout) before an external SIGALRM/SIGXCPU could kill it mid-search
# with no output at all.
SOFT_TIME=$(( TIME_LIMIT > 2 ? TIME_LIMIT - 2 : TIME_LIMIT ))

# Raise the stack limit for the parsing/clausification phase (mrs_tptp's
# recursive-descent parser and mrs_cnf's NNF/Skolemization/CNF pipeline both
# run on the main thread, before run_schedule spawns worker threads -- see
# docs/STATUS.md). crates/mrs-tptp/doc/technical.md documents deeply nested
# formulas as a stack-overflow risk. Best-effort: some sandboxes cap the
# hard limit and refuse to raise the soft limit further, which prints a
# warning but does not abort under `set -e`.
ulimit -s unlimited 2>/dev/null || true

# `ulimit -s` above only covers the main thread. The strategy portfolio
# (std::thread::scope in strategy.rs, default 8 workers) spawns worker
# threads with Rust's own runtime default of 2 MiB (DEFAULT_MIN_STACK_SIZE,
# smaller than the typical 8 MiB main-thread default) unless RUST_MIN_STACK
# is set in the environment before the process starts. Those threads run
# genuinely recursive code from mrs-unify/mrs-core/mrs-index throughout the
# entire given-clause search -- a stack overflow there triggers Rust's
# abort() handler, killing the whole process with zero output, which is
# worse than a clean timeout. 64 MiB matches the precedent already set by
# crates/mrs-tptp/examples/parse_folder.rs's stack_size(64 * 1024 * 1024)
# and gives on the order of 300,000 levels of recursion headroom --
# comfortably more than any real TPTP problem's term nesting depth, at
# negligible cost (thread stacks are lazily-committed virtual memory, not
# counted against RSS until used).
export RUST_MIN_STACK=67108864

exec "${BINARY}" --time "${SOFT_TIME}" --workers "${MRS_WORKERS:-8}" --schedule "${SCHEDULE}" "${PROBLEM}"
