#!/usr/bin/env bash
#
# verify_proover_corpus.sh
#
# Deterministic, OFFLINE regression gate for mrs-proover.
#
# Verifies every proof in the committed corpus
# (crates/mrs-bench/proover-corpus/, built by build_proover_corpus.sh) and
# checks the invariant that matters for the competition:
#
#     NO known-valid proof is ever reported VerifiedBad.
#
# Scoring rationale (ProoVer 2026): a false VerifiedBad on a good proof is
# -1; Unknown is 0; VerifiedGood is +1. Every proof in this corpus is a real E
# or Vampire refutation of a true theorem, so the only *wrong* outcome is
# VerifiedBad. VerifiedGood is ideal (+1) and Unknown is acceptable (0, e.g.
# a step we structurally can't certify and the ATP can't close in the budget).
#
# Exit status:
#   0  -> no VerifiedBad (gate passes)
#   1  -> at least one VerifiedBad (regression)
#
# This script never touches the network; it runs purely on the committed
# fixtures, so its result is stable across runs and machines.
#
# Usage:
#   crates/mrs-bench/verify_proover_corpus.sh            # build release if needed, verify
#   PROOVER=/path/to/mrs-proover crates/mrs-bench/verify_proover_corpus.sh
#   crates/mrs-bench/verify_proover_corpus.sh --time 10  # per-proof budget (default 10s)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
CORPUS_DIR="${SCRIPT_DIR}/proover-corpus"
PROBLEMS_DIR="${CORPUS_DIR}/Problems"
PROOFS_DIR="${CORPUS_DIR}/proofs"

TIME_BUDGET=10
while [[ $# -gt 0 ]]; do
    case "$1" in
        --time) TIME_BUDGET="$2"; shift 2 ;;
        *) echo "Unknown argument: $1" >&2; exit 2 ;;
    esac
done

PROOVER="${PROOVER:-${WORKSPACE_ROOT}/target/release/mrs-proover}"
if [[ ! -x "${PROOVER}" ]]; then
    echo "[corpus] Building mrs-proover (release)..." >&2
    (cd "${WORKSPACE_ROOT}" && cargo build --release -p mrs-proover >&2)
fi

if [[ ! -d "${PROOFS_DIR}" ]]; then
    echo "[corpus] No corpus found at ${PROOFS_DIR}." >&2
    echo "[corpus] Run crates/mrs-bench/build_proover_corpus.sh first." >&2
    exit 2
fi

VERIFIED=0
NOTVERIFIED=0
FAILED=0
FAILED_LIST=()

shopt -s nullglob
PROOFS=("${PROOFS_DIR}"/*.s)
shopt -u nullglob

echo "[corpus] Verifying ${#PROOFS[@]} proofs (budget ${TIME_BUDGET}s each)..." >&2
echo "" >&2

for PROOF in "${PROOFS[@]}"; do
    NAME="$(basename "${PROOF}" .s)"
    RES="$(timeout $((TIME_BUDGET + 5)) "${PROOVER}" --time "${TIME_BUDGET}" \
        --problems-dir "${PROBLEMS_DIR}" "${PROOF}" 2>/dev/null || true)"
    SZS="$(grep -m1 '% SZS status' <<< "${RES}" || echo '% SZS status Unknown : no output')"
    STATUS="$(awk '{print $4}' <<< "${SZS}")"

    case "${STATUS}" in
        VerifiedGood)       VERIFIED=$((VERIFIED + 1));    printf '  [ OK ] %-28s VerifiedGood\n' "${NAME}" >&2 ;;
        Unknown)    NOTVERIFIED=$((NOTVERIFIED + 1)); printf '  [ -- ] %-28s Unknown\n' "${NAME}" >&2 ;;
        VerifiedBad) FAILED=$((FAILED + 1)); FAILED_LIST+=("${NAME}: ${SZS#*: }")
                        printf '  [FAIL] %-28s %s\n' "${NAME}" "${SZS#*status }" >&2 ;;
        *)              NOTVERIFIED=$((NOTVERIFIED + 1)); printf '  [ ?? ] %-28s %s\n' "${NAME}" "${STATUS}" >&2 ;;
    esac
done

echo "" >&2
echo "[corpus] ============ Summary ============" >&2
printf '[corpus]   VerifiedGood      : %3d\n' "${VERIFIED}"    >&2
printf '[corpus]   Unknown   : %3d\n' "${NOTVERIFIED}" >&2
printf '[corpus]   VerifiedBad: %3d  (must be 0)\n' "${FAILED}" >&2
echo "[corpus] =================================" >&2

if [[ "${FAILED}" -gt 0 ]]; then
    echo "" >&2
    echo "[corpus] REGRESSION: the following known-valid proofs were rejected:" >&2
    for f in "${FAILED_LIST[@]}"; do
        echo "  - ${f}" >&2
    done
    exit 1
fi

echo "[corpus] PASS: no known-valid proof was VerifiedBad." >&2
exit 0
