#!/usr/bin/env bash
# crates/mrs-bench/fuzz_proover.sh
# Generate TSTP proofs from in-tree problems using eprover, then verify them
# with mrs-proover. Surfaces inference rules that we can't yet handle.
#
# Usage:
#   crates/mrs-bench/fuzz_proover.sh [--problems-dir DIR] [--time SECS]
set -euo pipefail
# Allow `unknown_rules[...]` to be unset.
declare -A unknown_rules

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

PROBLEMS_DIR="${WORKSPACE_ROOT}/problems"
TIME=10

while [[ $# -gt 0 ]]; do
    case "$1" in
        --problems-dir) PROBLEMS_DIR="$2"; shift 2 ;;
        --time)         TIME="$2";         shift 2 ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

EPROVER="${SCRIPT_DIR}/systems/eprover/bin/eprover"
PROOVER="${WORKSPACE_ROOT}/target/release/mrs-proover"

if [[ ! -x "${EPROVER}" ]]; then
    echo "eprover not found at ${EPROVER}" >&2
    exit 1
fi
if [[ ! -x "${PROOVER}" ]]; then
    echo "mrs-proover binary not found; run: cargo build --release -p mrs-proover" >&2
    exit 1
fi

OUT_DIR="$(mktemp -d -t fuzz-proover-XXXXXX)"
echo "[fuzz] Output: ${OUT_DIR}" >&2

# Set up a Problems/ subdir as expected by mrs-proover.
mkdir -p "${OUT_DIR}/Problems"

n_total=0
n_verified=0
n_failed=0
n_unverified=0
n_no_proof=0

for prob in "${PROBLEMS_DIR}"/*.p; do
    [[ -f "${prob}" ]] || continue
    name="$(basename "${prob}" .p)"
    (( n_total++ )) || true

    # Copy problem into the workspace expected by the verifier.
    cp "${prob}" "${OUT_DIR}/Problems/${name}.p"

    # Generate proof with eprover.
    proof_out="$(mktemp)"
    "${EPROVER}" --auto --proof-object --cpu-limit="${TIME}" \
        --tstp-format "${prob}" > "${proof_out}" 2>/dev/null || true

    # Quick check: did eprover find a proof at all?
    if ! grep -q '^cnf(.*\$false' "${proof_out}" && \
       ! grep -q '^fof(.*\$false' "${proof_out}"; then
        (( n_no_proof++ )) || true
        rm -f "${proof_out}"
        continue
    fi

    # Stitch a verifier-ready proof file: add the `% Proof :` header.
    # eprover's proof object may include cnf() lines; we rewrite them as
    # fof(), which is sound at the top level (free vars are implicitly
    # universally closed in both dialects).
    if ! grep -qE '^(cnf|fof)\(' "${proof_out}"; then
        (( n_no_proof++ )) || true
        rm -f "${proof_out}"
        continue
    fi

    proof_path="${OUT_DIR}/${name}_proof.p"
    {
        echo "% Proof : Problems/${name}.p"
        # Rewrite cnf(...) -> fof(...). Drop the comment-y first character if
        # eprover commented out a clause.
        sed -E -e 's/^cnf\(/fof(/' -e 's/^%cnf\(/%fof(/' "${proof_out}"
    } > "${proof_path}"
    rm -f "${proof_out}"

    # Run mrs-proover on the proof.
    res=$("${PROOVER}" --problems-dir "${OUT_DIR}/Problems" "${proof_path}" 2>/dev/null || true)
    verdict=$(awk '{print $4}' <<< "${res}")
    detail="${res#*: }"
    if [[ "${detail}" == "${res}" ]]; then detail=""; fi

    case "${verdict}" in
        Verified)       (( n_verified++ ))   || true ;;
        FailedVerified) (( n_failed++ ))     || true ;;
        *)              (( n_unverified++ )) || true
                        # Extract rule from detail like '(rule="<name>")' or 'rule=Some("<name>")'.
                        rule=$(grep -oE 'rule=Some\("[^"]+"\)' <<< "${detail}" | head -1 || true)
                        if [[ -n "${rule}" ]]; then
                            rule=${rule#rule=Some(\"}; rule=${rule%\")}
                            unknown_rules["${rule}"]=$(( ${unknown_rules["${rule}"]:-0} + 1 ))
                        fi
                        ;;
    esac
    printf '  %-32s -> %-20s %s\n' "${name}" "${verdict}" "${detail}" >&2
done

echo "" >&2
echo "[fuzz] Totals: ${n_total} problems" >&2
echo "[fuzz]   no proof generated:    ${n_no_proof}" >&2
echo "[fuzz]   Verified:              ${n_verified}" >&2
echo "[fuzz]   FailedVerified:        ${n_failed}" >&2
echo "[fuzz]   NotVerified (unknown): ${n_unverified}" >&2

set +u
if (( ${#unknown_rules[@]} > 0 )); then
    echo "" >&2
    echo "[fuzz] Unhandled inference rules (sorted by frequency):" >&2
    for k in "${!unknown_rules[@]}"; do
        printf '  %-30s %d\n' "${k}" "${unknown_rules[${k}]}"
    done | sort -k2 -rn >&2
fi
set -u

echo "" >&2
echo "[fuzz] Artifacts kept at: ${OUT_DIR}" >&2
