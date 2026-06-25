#!/usr/bin/env bash
#
# fetch_zenodo_corpus.sh
#
# Downloads and extracts the official TSTP FOF Proof Benchmark dataset
# (Zenodo 19792604) published by the Nörgler authors.
# Used by norgler_zenodo_benchmark.sh.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CORPUS_DIR="${SCRIPT_DIR}/zenodo-corpus"

if [[ -d "${CORPUS_DIR}/benchmarks" ]]; then
    echo "[zenodo] Corpus already exists at ${CORPUS_DIR}. Skipping download."
    exit 0
fi

echo "[zenodo] Downloading Zenodo dataset..."
mkdir -p "${CORPUS_DIR}"
TMP_ZIP="$(mktemp --suffix=.zip)"

curl -L "https://zenodo.org/api/records/19792604/files/TstpVerificationBenchmarks.zip/content" -o "${TMP_ZIP}"

echo "[zenodo] Extracting to ${CORPUS_DIR}..."
unzip -q "${TMP_ZIP}" -d "${CORPUS_DIR}"

rm -f "${TMP_ZIP}"

echo "[zenodo] Done. Extracted $(find "${CORPUS_DIR}/benchmarks" -type f | wc -l) files."
