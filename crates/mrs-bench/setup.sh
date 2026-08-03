#!/usr/bin/env bash
# crates/mrs-bench/setup.sh
# Downloads and extracts the CASC-30 problem archive.
#
# Usage: crates/mrs-bench/setup.sh [--edition casc-30]
#
# After running, the directory crates/mrs-bench/problems/casc-30/ will contain:
#   Problems/<Domain>/<Name>.p   (from Problems.tgz)
#   Axioms/<Domain>/<Name>.ax    (from Axioms.tgz)
#
# Set TPTP=crates/mrs-bench/problems/casc-30 before running mrs so %include resolves.
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
        ;;
    *)
        echo "Unknown edition: ${EDITION}" >&2
        echo "Supported: casc-30, casc-j13" >&2
        exit 1
        ;;
esac

mkdir -p "${DEST}"

download_and_extract() {
    local name="$1"          # e.g. Problems or Axioms
    local url="${BASE_URL}/${name}.tgz"
    local archive="${DEST}/${name}.tgz"
    local dest_check

    # Problems archive extracts directly into division dirs (no Problems/ subdir);
    # Axioms archive extracts into Axioms/.
    if [[ "${name}" == "Problems" ]]; then
        dest_check="${DEST}"
    else
        dest_check="${DEST}/${name}"
    fi

    if [[ -d "${dest_check}" ]] && [[ "${name}" != "Problems" ]]; then
        local count
        count=$(find "${dest_check}" \( -name "*.p" -o -name "*.ax" \) 2>/dev/null | wc -l)
        echo "[setup] ${name}/ already present (${count} files) — skipping download."
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
    count=$(find "${dest_check}" \( -name "*.p" -o -name "*.ax" \) 2>/dev/null | wc -l) || true
    if [[ "${count:-0}" -gt 0 ]]; then
        echo "[setup] ${name}/ extracted: ${count} files."
    else
        echo "[setup] WARNING: ${dest_check}/ not found after extraction."
        echo "[setup]          The archive may use a different layout — check entries above."
    fi
}

download_and_extract "Problems"
download_and_extract "Axioms"

# Generate division list files dynamically if they are not already present
if [[ ! -d "${DEST}/lists" ]]; then
    echo "[setup] Generating lists directory at ${DEST}/lists..."
    mkdir -p "${DEST}/lists"
    # Find all directories directly under DEST (except Axioms and lists)
    for d_path in "${DEST}"/*/; do
        # Ignore wildcards if directory is empty
        [[ -d "${d_path}" ]] || continue
        d=$(basename "${d_path}")
        if [[ "${d}" != "Axioms" && "${d}" != "lists" ]]; then
            list_file="${DEST}/lists/${d,,}.list"
            # Get sorted names of all .p files in the directory, removing .p extension
            find "${d_path}" -maxdepth 1 -name "*.p" -exec basename {} .p \; | sort > "${list_file}"
            echo "[setup] Generated list for division '${d,,}' with $(wc -l < "${list_file}") problems."
        fi
    done
fi

echo ""
echo "[setup] Done. Set TPTP=${DEST} before running mrs. "
echo "        Example: TPTP=${DEST} crates/mrs-bench/casc.sh --systems mrs --divisions fne"
