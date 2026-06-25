#!/usr/bin/env bash
#
# invoke.sh — wrapper around the Nörgler TSTP proof verifier (ProoVer reference
# competitor). Used by `crates/mrs-bench/norgler_compare.sh` to benchmark
# mrs-proover against Nörgler on the offline proover corpus.
#
# Prerequisites (NOT committed; see crates/mrs-bench/.gitignore `systems/*/bin/`):
#   - bin/noergler-1.0.jar     Download from
#       https://github.com/leoprover/noergler/releases/download/v1.0/noergler-1.0.jar
#   - ../eprover/bin/eprover   E prover (used by Nörgler to discharge thm/cth steps)
#   - ../vampire/bin/vampire   Vampire (idem)
#   - java                     Provided here via `nix-shell -p jre`; on a non-Nix
#                              host replace the `nix-shell ... --run` wrapper with
#                              a direct `java -jar ...` invocation.
#
# Nörgler also expects a model finder (mace4) on disk. We do not ship mace4, so
# this wrapper drops a harmless stub at ../mace4/bin/mace4 (it exits 1, i.e.
# "no counter-model found") and runs Nörgler with --parallel-model-finder-mode
# none so the stub is never actually exercised.
#
# CAVEAT: the numbers produced with this wrapper are PRELIMINARY. Nörgler is
# strict about annotation shapes and rejects several standard E/Vampire rules
# (e.g. `ennf_transformation`, `pure_predicate_removal`) and the
# `assume_negation`-from-conjecture pattern even with all `--relax-*` flags set.
# Those rejections reflect format/config friction in this harness, NOT a
# definitive quality gap. See docs/PROOVER_HARNESS.md.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

JAR_PATH="${SCRIPT_DIR}/bin/noergler-1.0.jar"
EPROVER_PATH="${SCRIPT_DIR}/../eprover/bin/eprover"
VAMPIRE_PATH="${SCRIPT_DIR}/../vampire/bin/vampire"

if [[ ! -f "${JAR_PATH}" ]]; then
    echo "% SZS status Error : noergler-1.0.jar not found at ${JAR_PATH}" >&2
    echo "  Download it from https://github.com/leoprover/noergler/releases" >&2
    exit 1
fi

# Provide a harmless mace4 stub so Nörgler's CLI is satisfied; it is never run
# because we pass --parallel-model-finder-mode none.
MACE4_DIR="${SCRIPT_DIR}/../mace4/bin"
MACE4_PATH="${MACE4_DIR}/mace4"
if [[ ! -x "${MACE4_PATH}" ]]; then
    mkdir -p "${MACE4_DIR}"
    printf '#!/bin/sh\nexit 1\n' > "${MACE4_PATH}"
    chmod +x "${MACE4_PATH}"
fi

# Last positional argument is the proof file; everything before it is options
# (including `--problem <file>`), forwarded verbatim to Nörgler.
PROOF_FILE="${*: -1}"
ARGS=("${@:1:$#-1}")

# Nörgler insists that a `file('<path>', name)` leaf annotation point at the
# exact problem path it was given via --problem. ATP traces in the corpus carry
# the prover's original sandbox path (e.g. /export/starexec/.../theBenchmark.p),
# so we rewrite the leaf path to the absolute problem path before verifying.
TMP_PROOF="$(mktemp --suffix=.s)"
trap 'rm -f "${TMP_PROOF}"' EXIT

PROB_FILE=""
for (( i=0; i<${#ARGS[@]}; i++ )); do
    if [[ "${ARGS[$i]}" == "--problem" ]]; then
        PROB_FILE="${ARGS[$i+1]}"
        break
    fi
done

if [[ -n "${PROB_FILE}" ]]; then
    ABS_PROB="$(readlink -f "${PROB_FILE}")"
    sed -E "s|file\('[^']+',|file\('${ABS_PROB}',|g" "${PROOF_FILE}" > "${TMP_PROOF}"
else
    cp "${PROOF_FILE}" "${TMP_PROOF}"
fi

nix-shell -p jre --run "java -jar ${JAR_PATH} \
    --eprover-path ${EPROVER_PATH} \
    --vampire-path ${VAMPIRE_PATH} \
    --mace4-path ${MACE4_PATH} \
    --parallel-model-finder-mode none \
    --prover eprover,vampire \
    --relax-annotation-format \
    --relax-problem-check \
    --relax-specified-inference-checks \
    --allow-prover-axioms \
    --up-to-esa \
    \"\${ARGS[@]}\" \"${TMP_PROOF}\""
