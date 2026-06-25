#!/usr/bin/env bash
#
# fetch_zenodo_corpus.sh
#
# Download, extract, and normalise the official TSTP FOF Proof Benchmark
# dataset (Zenodo 19792604, https://zenodo.org/records/19792604) published by
# the Nörgler authors. Used by zenodo_benchmark.sh.
#
# The dataset ships:
#   benchmarks/PyRes/{original,falsified}/*.proof  (+ benchmarks/PyRes/problems/*.p)
#   benchmarks/Otter/{original,falsified}/*.proof  (no problem files)
#
# Normalisation: PyRes proofs reference their problem via `file('<PROB>.p',_)`
# leaves but carry no `% Proof : …` header, which mrs-proover needs to locate
# the linked problem file. We inject that header (idempotently) so the corpus
# verifies straight after a fresh fetch. Otter proofs ship no problem files, so
# no header is injected for them.
#
# The extracted corpus (~28 MB, ~4k files) is git-ignored; re-run this script
# to (re)materialise it.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CORPUS_DIR="${SCRIPT_DIR}/zenodo-corpus"
MARKER="${CORPUS_DIR}/.normalised"

ZENODO_URL="https://zenodo.org/api/records/19792604/files/TstpVerificationBenchmarks.zip/content"

if [[ -f "${MARKER}" ]]; then
    echo "[zenodo] Corpus already present and normalised at ${CORPUS_DIR}. Nothing to do."
    exit 0
fi

if [[ ! -d "${CORPUS_DIR}/benchmarks" ]]; then
    echo "[zenodo] Downloading dataset from Zenodo..."
    mkdir -p "${CORPUS_DIR}"
    TMP_ZIP="$(mktemp --suffix=.zip)"
    trap 'rm -f "${TMP_ZIP}"' EXIT
    curl -fL "${ZENODO_URL}" -o "${TMP_ZIP}"
    echo "[zenodo] Extracting to ${CORPUS_DIR}..."
    unzip -q "${TMP_ZIP}" -d "${CORPUS_DIR}"
fi

# Inject a `% Proof : <PROB>.p` header into every PyRes proof so mrs-proover can
# resolve the linked problem file (see crates/mrs-proover/src/load.rs). The
# proof filename is `<PROB>_PyRes---<ver>[...].proof`, e.g.
# `ALG171+1_PyRes---1.5.proof` -> problem `ALG171+1.p`.
echo "[zenodo] Normalising PyRes proofs (injecting % Proof headers)..."
injected=0
while IFS= read -r -d '' proof; do
    if ! grep -q '^% Proof :' "${proof}"; then
        prob="$(basename "${proof}")"
        prob="${prob%%_*}.p"
        sed -i "1i % Proof : ${prob}" "${proof}"
        injected=$((injected + 1))
    fi
done < <(find "${CORPUS_DIR}/benchmarks/PyRes" -type f -name '*.proof' -print0)

touch "${MARKER}"

echo "[zenodo] Done."
echo "[zenodo]   total files : $(find "${CORPUS_DIR}/benchmarks" -type f | wc -l)"
echo "[zenodo]   PyRes headers injected : ${injected}"
