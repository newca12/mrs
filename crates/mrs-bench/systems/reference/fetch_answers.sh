#!/usr/bin/env bash
# Populate the reference-answer TSV for a benchmark edition.
#
# A local TPTP root is preferred, so an archived edition can use the exact
# library release it was selected from. For other editions, missing local
# statuses fall back to SeeTPTP. CASC-J12 deliberately never consults the live
# site: its obfuscated problem archive has no headers, and live statuses may
# have changed since the official TPTP v9.0.0 release.
#
# Usage: fetch_answers.sh [OPTIONS]
#
#   --jobs N             Parallel curl workers (default: 8)
#   --edition E          Edition name (default: casc-30)
#   --problems-root DIR  Extracted edition corpus (default: problems/<edition>)
#   --tptp-root DIR      Full TPTP library root, e.g. TPTP-v9.0.0
#                        (default: $TPTP, then $TPTP_ROOT)
#   --output FILE        Answer TSV path (default: answers_<edition>.tsv)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BENCH_DIR="${SCRIPT_DIR}/../.."

EDITION="casc-30"
JOBS=8
PROBLEMS_ROOT_ARG=""
TPTP_ROOT_ARG=""
OUT_ARG=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --jobs)
            [[ $# -ge 2 ]] || { echo "--jobs requires a value" >&2; exit 2; }
            JOBS="$2"
            shift 2
            ;;
        --edition)
            [[ $# -ge 2 ]] || { echo "--edition requires a value" >&2; exit 2; }
            EDITION="$2"
            shift 2
            ;;
        --problems-root)
            [[ $# -ge 2 ]] || { echo "--problems-root requires a directory" >&2; exit 2; }
            PROBLEMS_ROOT_ARG="$2"
            shift 2
            ;;
        --tptp-root)
            [[ $# -ge 2 ]] || { echo "--tptp-root requires a directory" >&2; exit 2; }
            TPTP_ROOT_ARG="$2"
            shift 2
            ;;
        --output)
            [[ $# -ge 2 ]] || { echo "--output requires a file" >&2; exit 2; }
            OUT_ARG="$2"
            shift 2
            ;;
        *) echo "Unknown argument: $1" >&2; exit 2 ;;
    esac
done

if ! [[ "${JOBS}" =~ ^[1-9][0-9]*$ ]]; then
    echo "--jobs must be a positive integer" >&2
    exit 2
fi

PROBLEMS_DIR="${PROBLEMS_ROOT_ARG:-${CASC_PROBLEMS_ROOT:-${BENCH_DIR}/problems/${EDITION}}}"
TPTP_DIR="${TPTP_ROOT_ARG:-${TPTP:-${TPTP_ROOT:-}}}"
OUT="${OUT_ARG:-${SCRIPT_DIR}/answers_${EDITION}.tsv}"

if [[ ! -d "${PROBLEMS_DIR}" ]]; then
    echo "Error: problem corpus not found: ${PROBLEMS_DIR}" >&2
    echo "       Run crates/mrs-bench/setup.sh --edition ${EDITION} first." >&2
    exit 1
fi
PROBLEMS_DIR="$(cd "${PROBLEMS_DIR}" && pwd -P)"

if [[ -n "${TPTP_DIR}" ]]; then
    if [[ ! -d "${TPTP_DIR}/Problems" ]]; then
        echo "Error: TPTP root must contain Problems/: ${TPTP_DIR}" >&2
        exit 1
    fi
    TPTP_DIR="$(cd "${TPTP_DIR}" && pwd -P)"
fi

OUT_PARENT="$(dirname "${OUT}")"
mkdir -p "${OUT_PARENT}"
OUT_PARENT="$(cd "${OUT_PARENT}" && pwd -P)"
OUT="${OUT_PARENT}/$(basename "${OUT}")"

# Build a division/name inventory from setup-generated lists. Fall back to the
# archive directories when lists are absent, accepting both common layouts.
entries_tmp="$(mktemp)"
answers_tmp="${OUT}.tmp.$$"
trap 'rm -f "${entries_tmp}" "${answers_tmp}"' EXIT

shopt -s nullglob
list_files=("${PROBLEMS_DIR}"/lists/*.list)
shopt -u nullglob
if ((${#list_files[@]} > 0)); then
    for list in "${list_files[@]}"; do
        division="$(basename "${list}" .list)"
        while IFS= read -r problem || [[ -n "${problem}" ]]; do
            problem="${problem#"${problem%%[![:space:]]*}"}"
            problem="${problem%"${problem##*[![:space:]]}"}"
            [[ -z "${problem}" || "${problem}" == \#* ]] && continue
            problem="${problem%.p}"
            printf '%s\t%s\n' "${division,,}" "${problem}" >>"${entries_tmp}"
        done <"${list}"
    done
else
    shopt -s nullglob
    division_dirs=("${PROBLEMS_DIR}"/*/ "${PROBLEMS_DIR}"/Problems/*/)
    shopt -u nullglob
    for division_dir in "${division_dirs[@]}"; do
        [[ -d "${division_dir}" ]] || continue
        division="$(basename "${division_dir%/}")"
        case "${division,,}" in axioms|problems|lists) continue ;; esac
        while IFS= read -r problem_file; do
            printf '%s\t%s\n' "${division,,}" "$(basename "${problem_file}" .p)" >>"${entries_tmp}"
        done < <(find -L "${division_dir}" -type f -name "*.p" -print)
    done
fi

if [[ ! -s "${entries_tmp}" ]]; then
    echo "Error: no division problem lists or .p files found under ${PROBLEMS_DIR}" >&2
    exit 1
fi
LC_ALL=C sort -u "${entries_tmp}" -o "${entries_tmp}"
total="$(wc -l <"${entries_tmp}")"

echo "[reference] Resolving ${total} problem statuses for ${EDITION} (--jobs ${JOBS}) ..."
echo "[reference] Writing to ${OUT}"
if [[ -n "${TPTP_DIR}" ]]; then
    echo "[reference] Pinned source library: ${TPTP_DIR}"
fi
if [[ "${EDITION}" == "casc-j12" && -z "${TPTP_DIR}" ]]; then
    echo "[reference] WARNING: no TPTP v9.0.0 root supplied; using J12 division answer classes where source headers are unavailable." >&2
fi

read_status() {
    local file="$1"
    grep -m1 '% Status' "${file}" 2>/dev/null | awk '{print $NF}' || true
}

fetch_one() {
    local division="$1"
    local problem="$2"
    local domain="${problem:0:3}"
    local status=""
    local source_file=""

    # Read the pinned source release first. CASC archives often strip all
    # comments, including the original % Status line.
    if [[ -n "${TPTP_DIR}" ]]; then
        source_file="${TPTP_DIR}/Problems/${domain}/${problem}.p"
        if [[ -f "${source_file}" ]]; then
            status="$(read_status "${source_file}")"
        fi
    fi

    # For non-J12 runs, preserve the live SeeTPTP fallback when no local
    # library copy supplied a status. J12 stays pinned to its archive/release.
    if [[ "${EDITION}" != "casc-j12" && -z "${status}" ]]; then
        local raw
        raw="$(curl -sS --max-time 20 --retry 2 --get \
            --data-urlencode 'Category=Problems' \
            --data-urlencode "Domain=${domain}" \
            --data-urlencode "File=${problem}.p" \
            'https://tptp.org/cgi-bin/SeeTPTP' 2>/dev/null || true)"
        status="$(printf '%s\n' "${raw}" | grep -m1 '% Status' | awk '{print $NF}')"
    fi

    # Local non-obfuscated problem files may still carry an authoritative
    # status header (e.g. entrant-provided ICU problems).
    if [[ -z "${status}" || "${status}" == "GaveUp" ]]; then
        local local_file=""
        local candidate
        for candidate in \
            "${PROBLEMS_DIR}/${division}/${problem}.p" \
            "${PROBLEMS_DIR}/${division^^}/${problem}.p" \
            "${PROBLEMS_DIR}/Problems/${division^^}/${problem}.p" \
            "${PROBLEMS_DIR}/Problems/${division}/${problem}.p"; do
            if [[ -f "${candidate}" ]]; then
                local_file="${candidate}"
                break
            fi
        done
        if [[ -z "${local_file}" ]]; then
            local division_root
            for division_root in \
                "${PROBLEMS_DIR}/${division}" \
                "${PROBLEMS_DIR}/${division^^}" \
                "${PROBLEMS_DIR}/Problems/${division^^}" \
                "${PROBLEMS_DIR}/Problems/${division}"; do
                if [[ -d "${division_root}" ]]; then
                    local_file="$(find -L "${division_root}" -type f -name "${problem}.p" -print -quit 2>/dev/null || true)"
                    [[ -n "${local_file}" ]] && break
                fi
            done
        fi
        if [[ -n "${local_file}" ]]; then
            local local_status
            local_status="$(read_status "${local_file}")"
            [[ -n "${local_status}" ]] && status="${local_status}"
        fi
    fi

    case "${status}" in
        Theorem|Unsatisfiable|Satisfiable|CounterSatisfiable) ;;
        *) status="" ;;
    esac

    # J12 divisions have a known answer class even when the source problem is
    # generated outside the TPTP library (SLH/ICU), or a TPTP header is absent.
    # TFN's exact positive status varies, but the benchmark grades both
    # Satisfiable and CounterSatisfiable in the same counter-model class.
    if [[ -z "${status}" && "${EDITION}" == "casc-j12" ]]; then
        case "${division,,}" in
            tne|teq|tfi|tfe|fne|feq|slh|icu) status="Theorem" ;;
            ueq) status="Unsatisfiable" ;;
            tfn) status="CounterSatisfiable" ;;
            *) status="GaveUp" ;;
        esac
    fi

    [[ -n "${status}" ]] || status="GaveUp"
    printf '%s\t%s\n' "${problem}" "${status}"
}
export -f read_status fetch_one
export EDITION PROBLEMS_DIR TPTP_DIR

# Each entry is division<TAB>problem. xargs invokes a fresh Bash process for
# each pair, then the output is sorted before the atomic replacement.
awk -F '\t' '{print $1 "\n" $2}' "${entries_tmp}" \
    | xargs -P "${JOBS}" -n 2 bash -c 'fetch_one "$@"' _ \
    | LC_ALL=C sort -t $'\t' -k1,1 -u >"${answers_tmp}"

mv -f -- "${answers_tmp}" "${OUT}"
rm -f "${entries_tmp}"
trap - EXIT

total_lines="$(wc -l <"${OUT}")"
solved="$(awk -F '\t' '$2 != "GaveUp" { count++ } END { print count+0 }' "${OUT}")"
echo "[reference] Done. ${total_lines} problems in $(basename "${OUT}") (${solved} with a definitive answer class)."
