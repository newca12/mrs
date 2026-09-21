#!/usr/bin/env bash
# crates/mrs-bench/systems/mrs-certify/invoke.sh
#
# Ordered-inference certification runs for benchmark harnesses.
#
# Runs ONE base strategy with --workers 1 --certify-ordered, so the result
# is either a certified verdict (Theorem/Unsatisfiable/Satisfiable) or a
# fail-closed GaveUp/Timeout -- never an uncertified saturation claim.
# The base strategy is taken from MRS_CERTIFY_STRATEGY (default 1, KBO);
# strategy 7 selects the LPO variant. Schedule follows the problem
# division, exactly like mrs-s01.
#
# Coverage attribution (tier + ordering) is read from the prover's
# `% SZS detail ... cert_tier=N cert_ordering=...` stderr line -- no
# TRACE output needed. Refusals carry no cert_tier (see
# docs/ORDERED_INFERENCE_CERTIFICATION.md).
#
# Usage (via casc.sh):
#   casc.sh --systems mrs-certify --divisions eps,epu --casc-times --jobs 16 ...
#   MRS_CERTIFY_STRATEGY=7 casc.sh --systems mrs-certify --divisions eps,epu ...
#
# Usage (direct):
#   invoke.sh <problem_path> <time_limit_secs>
#
# Writes all output to stdout; exits with any code.
set -euo pipefail

PROBLEM="${1:?Usage: invoke.sh <problem_path> <time_limit_secs>}"
TIME_LIMIT="${2:?Usage: invoke.sh <problem_path> <time_limit_secs>}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../../../.." && pwd)"

BINARY="${WORKSPACE_ROOT}/target/release/mrs"
if [[ ! -x "${BINARY}" ]]; then
    echo "% SZS status Error (mrs binary not found; run: cargo build --release)"
    exit 1
fi

# Set TPTP root so %include directives resolve.
if [[ -z "${TPTP:-}" ]]; then
    export TPTP="${SCRIPT_DIR}/../../problems/casc-30"
fi

STRATEGY_NUM="${MRS_CERTIFY_STRATEGY:-1}"
if ! [[ "${STRATEGY_NUM}" =~ ^[0-9]+$ ]] || (( STRATEGY_NUM < 1 || STRATEGY_NUM > 15 )); then
    echo "% SZS status Error (mrs-certify: MRS_CERTIFY_STRATEGY must be in 1..15, got '${STRATEGY_NUM}')"
    exit 1
fi

# Division schedule determines which base strategy the ID resolves to
# (single_strategy looks the base ID up in the division's canonical order).
DIVISION=$(basename "$(dirname "${PROBLEM}")")
DIV_LOWER="${DIVISION,,}"
case "${DIV_LOWER}" in
    feq|fne|ueq|epr|eps|epu|icu) SCHEDULE="casc_${DIV_LOWER}" ;;
    *) SCHEDULE="casc" ;;
esac

# The certifier is single-strategy by construction: one worker, one
# strategy, no shared pool. MRS_WORKERS is deliberately ignored here so a
# harness-wide worker setting cannot silently parallelize (and de-isolate)
# a certification run.
exec "${BINARY}" --time "${TIME_LIMIT}" --workers 1 \
    --schedule "${SCHEDULE}" --strategy "${STRATEGY_NUM}" \
    --certify-ordered "${PROBLEM}"
