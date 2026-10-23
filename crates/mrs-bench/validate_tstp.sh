#!/usr/bin/env bash
# Validate generated TSTP proofs with the TPTP World tptp4X syntax checker.
#
# Usage:
#   validate_tstp.sh --proofs-dir results/proofs
#   validate_tstp.sh --tptp4x /path/to/tptp4X proof1.e proof2.e
#
# Set TPTP4X instead of passing --tptp4x. The command must exit successfully
# and emit no ERROR line for every input file.
set -euo pipefail

INPUT_DIR=""
TPTP4X="${TPTP4X:-}"
INPUTS=()

usage() {
    printf '%s\n' \
        "Usage: $0 [--tptp4x PATH] [--proofs-dir DIR | FILE ...]" \
        "  --tptp4x PATH     TPTP World tptp4X executable" \
        "  --proofs-dir DIR  Recursively validate .e, .p, .s, and .proof files" \
        "  --help             Show this help"
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --tptp4x)
            [[ $# -ge 2 ]] || { usage >&2; exit 2; }
            TPTP4X="$2"
            shift 2
            ;;
        --proofs-dir)
            [[ $# -ge 2 ]] || { usage >&2; exit 2; }
            INPUT_DIR="$2"
            shift 2
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        --*)
            printf 'Unknown option: %s\n' "$1" >&2
            usage >&2
            exit 2
            ;;
        *)
            INPUTS+=("$1")
            shift
            ;;
    esac
done

if [[ -n "${INPUT_DIR}" ]]; then
    [[ -d "${INPUT_DIR}" ]] || { printf 'Proof directory does not exist: %s\n' "${INPUT_DIR}" >&2; exit 2; }
    [[ ${#INPUTS[@]} -eq 0 ]] || { printf '%s\n' "--proofs-dir cannot be combined with positional files" >&2; exit 2; }
    INPUT_DIR="$(cd "${INPUT_DIR}" && pwd)"
    shopt -s nullglob globstar
    for path in "${INPUT_DIR}"/**/{*.e,*.p,*.s,*.proof}; do
        [[ -f "${path}" ]] && INPUTS+=("${path}")
    done
    shopt -u nullglob globstar
fi

((${#INPUTS[@]} > 0)) || { printf '%s\n' "No proof files supplied" >&2; usage >&2; exit 2; }

if [[ -z "${TPTP4X}" ]]; then
    TPTP4X="$(command -v tptp4X || true)"
fi
[[ -n "${TPTP4X}" && -x "${TPTP4X}" ]] || {
    printf '%s\n' "tptp4X not found; pass --tptp4x PATH or set TPTP4X" >&2
    exit 2
}

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

passed=0
failed=0
for proof in "${INPUTS[@]}"; do
    [[ -f "${proof}" ]] || {
        printf 'FAIL missing file: %s\n' "${proof}" >&2
        failed=$((failed + 1))
        continue
    }

    log="${TMP_DIR}/$(basename "${proof}").log"
    if "${TPTP4X}" -q1 -ftptp -umachine "${proof}" >"${log}" 2>&1 \
        && ! grep -Eiq '(^|[^A-Za-z])ERROR([^A-Za-z]|$)' "${log}"; then
        printf 'PASS %s\n' "${proof}"
        passed=$((passed + 1))
    else
        printf 'FAIL %s\n' "${proof}" >&2
        grep -Ei 'ERROR|error|syntax' "${log}" >&2 || true
        failed=$((failed + 1))
    fi
done

printf 'tptp4X validation: %d passed, %d failed\n' "${passed}" "${failed}"
((failed == 0))
