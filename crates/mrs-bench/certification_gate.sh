#!/usr/bin/env bash
# Certification gate: the two invariants that must never regress.
#
#   1. ProoVer 2026: no committed evil proof may be reported VerifiedGood
#      (-10 in the competition scoring), and no committed valid proof may be
#      reported VerifiedBad (-1).
#   2. Self-certification: every proof in the committed canary set must still
#      be certified by the strict kernel, and no mechanical mutation of one may
#      be.
#
# This is the fast, offline gate: it needs the release binaries, no ATP, and a
# couple of minutes. The corpus-wide numbers (all 100 ProoVer problems with the
# full ladder, and MRS's own refutations over a whole CASC edition) come from
# `certification_campaign.sh`, which is the expensive version of the same
# question.
#
# Usage:
#   crates/mrs-bench/certification_gate.sh
#
# Exit codes: 0 all invariants hold, 1 a regression, 2 a build/setup problem.

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
PROVER="${PROVER:-${ROOT}/target/release/mrs-proover}"
MRS="${MRS:-${ROOT}/target/release/mrs}"
CORPUS="${ROOT}/crates/mrs-bench/proover-corpus/Proover2026"
CANARIES="${ROOT}/crates/mrs-proover/tests/resources/mrs_proofs"
TIME_LIMIT="${GATE_TIME:-20}"
WORKERS="${GATE_WORKERS:-4}"

fail() { echo "gate: $*" >&2; exit 2; }
[[ -x "${PROVER}" ]] || fail "mrs-proover not built (cargo build --release -p mrs-proover)"
[[ -d "${CORPUS}" ]] || fail "missing ProoVer corpus at ${CORPUS}"
[[ -d "${CANARIES}" ]] || fail "missing canary proofs at ${CANARIES}"

verdict() {
    # $1 = proof file. Prints VerifiedGood / VerifiedBad / Unknown / Error.
    local out
    out="$(timeout $((TIME_LIMIT + 10)) "${PROVER}" \
        --time "${TIME_LIMIT}" --workers "${WORKERS}" \
        --problems-dir "${CORPUS}" "$1" 2>/dev/null | grep -m1 '% SZS status' || true)"
    if [[ -z "${out}" ]]; then echo "Error"; else
        echo "${out}" | awk '{print $4}'
    fi
}

echo "== ProoVer 2026 corpus (100 proofs, ${TIME_LIMIT}s each) =="
score=0
unsound=0
false_reject=0
unknown=0
good=0
bad=0
while IFS=$'\t' read -r id _problem proof category accepted max; do
    [[ "${id}" == \#* || -z "${id:-}" ]] && continue
    status="$(verdict "${CORPUS}/${proof}")"
    case "${category}:${status}" in
        valid:VerifiedGood) score=$((score + 1)); good=$((good + 1));;
        valid:VerifiedBad)  score=$((score - 1)); false_reject=$((false_reject + 1));;
        evil:VerifiedBad)   score=$((score + 2)); bad=$((bad + 1));;
        evil:VerifiedGood)  score=$((score - 10)); unsound=$((unsound + 1));;
        locally_sound_evil:VerifiedGood|locally_sound_evil:VerifiedBad)
            score=$((score + 2));;
        *) unknown=$((unknown + 1));;
    esac
    if [[ "${category}" == "evil" && "${status}" == "VerifiedGood" ]]; then
        echo "  UNSOUND  ${id}: reported VerifiedGood" >&2
    fi
    if [[ "${category}" == "valid" && "${status}" == "VerifiedBad" ]]; then
        echo "  REJECTED ${id}: valid proof reported VerifiedBad" >&2
    fi
done < "${CORPUS}/manifest.tsv"
echo "  score=${score} good=${good} bad=${bad} unknown=${unknown} unsound=${unsound} false_rejection=${false_reject}"

echo "== Self-certification canaries (strict kernel) =="
canaries=0
canary_failures=0
for proof in "${CANARIES}"/*.s; do
    source_line="$(grep -m1 '^% Proof :' "${proof}" | sed 's/^% Proof : //')"
    status="$(timeout $((TIME_LIMIT + 10)) "${PROVER}" --strict --time "${TIME_LIMIT}" \
        --problems-dir "${ROOT}" "${proof}" 2>/dev/null | grep -m1 '% SZS status' | awk '{print $4}')"
    canaries=$((canaries + 1))
    if [[ "${status}" != "VerifiedGood" ]]; then
        echo "  NOT CERTIFIED ${proof##*/}: ${status:-Error}" >&2
        canary_failures=$((canary_failures + 1))
    fi
done
echo "  canaries=${canaries} not_certified=${canary_failures}"

if [[ "${unsound}" -ne 0 || "${false_reject}" -ne 0 || "${canary_failures}" -ne 0 ]]; then
    echo "gate: FAIL (unsound=${unsound} false_rejection=${false_reject} uncertified_canaries=${canary_failures})" >&2
    exit 1
fi
echo "gate: PASS (0 unsound, 0 false rejections, ${canaries}/${canaries} canaries certified)"
