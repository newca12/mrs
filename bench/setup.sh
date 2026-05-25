#!/usr/bin/env bash
# bench/setup.sh
# Downloads and extracts the CASC-30 problem archive.
#
# Usage: bench/setup.sh [--edition casc-30]
#
# After running, the directory bench/problems/casc-30/ will contain:
#   Problems/<Domain>/<Name>.p   (from Problems.tgz)
#   Axioms/<Domain>/<Name>.ax    (from Axioms.tgz)
#
# Set TPTP=bench/problems/casc-30 before running mrs so %include resolves.
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
    *)
        echo "Unknown edition: ${EDITION}" >&2
        echo "Supported: casc-30" >&2
        exit 1
        ;;
esac

mkdir -p "${DEST}"

download_and_extract() {
    local name="$1"          # e.g. Problems
    local url="${BASE_URL}/${name}.tgz"
    local archive="${DEST}/${name}.tgz"

    if [[ -d "${DEST}/${name}" ]]; then
        local count
        count=$(find "${DEST}/${name}" -name "*.p" -o -name "*.ax" 2>/dev/null | wc -l)
        echo "[setup] ${name}/ already present (${count} files) — skipping download."
        return
    fi

    echo "[setup] Downloading ${url} ..."
    curl --fail --location --continue-at - --progress-bar \
         --output "${archive}" "${url}"

    echo "[setup] Extracting ${archive} ..."
    tar -xzf "${archive}" -C "${DEST}"
    rm -f "${archive}"

    local count
    count=$(find "${DEST}/${name}" -name "*.p" -o -name "*.ax" 2>/dev/null | wc -l)
    echo "[setup] ${name}/ extracted: ${count} files."
}

download_and_extract "Problems"
download_and_extract "Axioms"

# Spot-check: verify a known problem file exists
SPOT="${DEST}/Problems/BOO/BOO109+1.p"
if [[ -f "${SPOT}" ]]; then
    echo "[setup] Spot-check passed: ${SPOT}"
else
    echo "[setup] WARNING: spot-check file not found: ${SPOT}"
    echo "        The archive may use a different layout."
fi

echo ""
echo "[setup] Done. Set TPTP=${DEST} before running mrs."
echo "        Example: TPTP=${DEST} bench/casc.sh --systems mrs --divisions fne"
