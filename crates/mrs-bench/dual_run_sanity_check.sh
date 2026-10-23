#!/usr/bin/env bash
# crates/mrs-bench/dual_run_sanity_check.sh
# Usage: dual_run_sanity_check.sh [OPTIONS] [PROBLEMS...]
#
# Automated Dual-Polarity & Metamorphic Soundness Auditing tool.
# Enforces Proposition 3 from Soundness-First Development Methodology:
# - Dual-polarity check: Phi and ~Phi can never both refute unless axioms are unsat.
# - Metamorphic check: 1 worker vs N workers must not return conflicting definitive statuses.
# - Contradiction sensitivity: Injecting $false into a SAT claim must refute.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

# Resolve TPTP root if not set
if [[ -z "${TPTP:-}" ]]; then
    export TPTP="${SCRIPT_DIR}/problems/casc-30"
fi

# Ensure mrs and dual_run_sanity_check are built
BIN="${WORKSPACE_ROOT}/target/release/dual_run_sanity_check"
MRS_BIN="${WORKSPACE_ROOT}/target/release/mrs"

if [[ ! -x "${BIN}" || ! -x "${MRS_BIN}" ]]; then
    echo "Building release targets (mrs, dual_run_sanity_check)..." >&2
    cargo build --release --workspace --bin mrs --bin dual_run_sanity_check
fi

exec "${BIN}" --mrs "${MRS_BIN}" "$@"
