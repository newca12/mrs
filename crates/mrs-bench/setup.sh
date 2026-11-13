#!/usr/bin/env bash
# crates/mrs-bench/setup.sh
# Downloads and extracts a CASC problem archive.
#
# Usage: crates/mrs-bench/setup.sh [--edition casc-30|casc-j13|casc-j12]
#
# The extracted edition directory will contain:
#   Problems/<Domain>/<Name>.p   (from Problems.tgz)
#   Axioms/<Domain>/<Name>.ax    (when the edition publishes Axioms.tgz)
#
# CASC-J12 uses TPTP v9.0.0 and publishes no edition-local Axioms.tgz. Download
# the full TPTP-v9.0.0 distribution separately and set TPTP to that extracted
# root when running the benchmark, so %include directives resolve against the
# same library release used by the competition.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

EDITION="casc-30"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --edition) EDITION="$2"; shift 2 ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

case "${EDITION}" in
    casc-30)
        BASE_URL="https://tptp.org/CASC/30"
        DEST="${SCRIPT_DIR}/problems/casc-30"
        ;;
    casc-j13)
        BASE_URL="https://tptp.org/CASC/J13"
        DEST="${SCRIPT_DIR}/problems/casc-j13"
        DOWNLOAD_AXIOMS=1
        ;;
    casc-j12)
        BASE_URL="https://tptp.org/CASC/J12"
        DEST="${SCRIPT_DIR}/problems/casc-j12"
        DOWNLOAD_AXIOMS=0
        ;;
    *)
        echo "Unknown edition: ${EDITION}" >&2
        echo "Supported: casc-30, casc-j13, casc-j12" >&2
        exit 1
        ;;
esac

if [[ "${EDITION}" == "casc-30" ]]; then
    DOWNLOAD_AXIOMS=1
fi

mkdir -p "${DEST}"

download_and_extract() {
    local name="$1"          # e.g. Problems or Axioms
    local url="${BASE_URL}/${name}.tgz"
    local archive="${DEST}/${name}.tgz"
    local dest_check

    # Archives have used both direct division directories and a Problems/
    # prefix. Count recursively so either layout is recognized as already
    # installed.
    if [[ "${name}" == "Problems" ]]; then
        dest_check="${DEST}"
    else
        dest_check="${DEST}/${name}"
    fi

    local count=0
    if [[ -d "${dest_check}" ]]; then
        if [[ "${name}" == "Problems" ]]; then
            count=$(find -L "${dest_check}" -type f -name "*.p" 2>/dev/null | wc -l)
        else
            count=$(find -L "${dest_check}" -type f -name "*.ax" 2>/dev/null | wc -l)
        fi
    fi

    if [[ "${count}" -gt 0 ]]; then
        echo "[setup] ${name} already present (${count} files) — skipping download."
        return
    fi

    echo "[setup] Downloading ${url} ..."
    curl --fail --location --continue-at - --progress-bar \
         --output "${archive}" "${url}"

    echo "[setup] Extracting ${archive} ..."
    # Show the top-level entries so we know the archive layout.
    echo "[setup] Archive top-level entries:"
    tar -tzf "${archive}" | awk -F'/' '{print $1}' | sort -u | head -20 | sed 's/^/[setup]   /'
    tar -xzf "${archive}" -C "${DEST}"
    rm -f "${archive}"

    local count
    count=$(find -L "${dest_check}" \( -name "*.p" -o -name "*.ax" \) 2>/dev/null | wc -l) || true
    if [[ "${count:-0}" -gt 0 ]]; then
        echo "[setup] ${name}/ extracted: ${count} files."
    else
        echo "[setup] WARNING: ${dest_check}/ not found after extraction."
        echo "[setup]          The archive may use a different layout — check entries above."
    fi
}

download_and_extract "Problems"
if [[ "${DOWNLOAD_AXIOMS}" -eq 1 ]]; then
    download_and_extract "Axioms"
else
    echo "[setup] CASC-J12 has no Axioms.tgz; use the full TPTP v9.0.0 library for includes."
fi

# Generate any missing division lists. Supporting both <DEST>/<DIV> and
# <DEST>/Problems/<DIV> matters for older CASC archives with a Problems/ prefix.
mkdir -p "${DEST}/lists"
shopt -s nullglob
division_dirs=("${DEST}"/*/ "${DEST}"/Problems/*/)
shopt -u nullglob
for d_path in "${division_dirs[@]}"; do
    [[ -d "${d_path}" ]] || continue
    d="$(basename "${d_path%/}")"
    case "${d}" in
        Axioms|Problems|lists) continue ;;
    esac

    list_file="${DEST}/lists/${d,,}.list"
    [[ -s "${list_file}" ]] && continue

    find -L "${d_path}" -type f -name "*.p" -exec basename {} .p \; \
        | LC_ALL=C sort > "${list_file}"
    if [[ -s "${list_file}" ]]; then
        echo "[setup] Generated list for division '${d,,}' with $(wc -l < "${list_file}") problems."
    else
        rm -f -- "${list_file}"
    fi
done

echo ""
echo "[setup] Done. Problem corpus: ${DEST}"
if [[ "${EDITION}" == "casc-j12" ]]; then
    echo "[setup] CASC-J12 used TPTP v9.0.0 (the full distribution expands to about 9.8 GB):"
    echo "        https://tptp.org/TPTP/Distribution/TPTP-v9.0.0.tgz"
    echo "        TPTP=/path/to/TPTP-v9.0.0 CASC_PROBLEMS_ROOT=${DEST} crates/mrs-bench/casc.sh --edition casc-j12 --systems mrs --casc-times --jobs 1"
else
    echo "[setup] Set TPTP=${DEST} before running mrs so %include resolves."
    echo "        Example: TPTP=${DEST} crates/mrs-bench/casc.sh --systems mrs --divisions fne"
fi
