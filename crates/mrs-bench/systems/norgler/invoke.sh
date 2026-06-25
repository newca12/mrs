#!/usr/bin/env bash
#
# invoke.sh — wrapper around the Nörgler TSTP proof verifier (ProoVer reference
# competitor). Used by `crates/mrs-bench/norgler_compare.sh` and
# `crates/mrs-bench/zenodo_benchmark.sh --with-norgler`.
#
# Prerequisites (NOT committed; see crates/mrs-bench/.gitignore `systems/*/bin/`):
#   - bin/noergler-1.0.jar     Download from
#       https://github.com/leoprover/noergler/releases/download/v1.0/noergler-1.0.jar
#   - ../eprover/bin/eprover   E prover (used by Nörgler to discharge thm/cth steps)
#   - ../vampire/bin/vampire   Vampire (idem)
#   - a Java runtime (>= 17)
#
# Java resolution (in order):
#   1. $MRS_NORGLER_JAVA, if set and executable.
#   2. `java` on $PATH.
#   3. Resolved ONCE via `nix-shell -p jre` and cached in `.java_path` next to
#      this script. The per-call `nix-shell` evaluation costs ~20 s, so caching
#      is essential for benchmarking; delete `.java_path` to re-resolve.
#
# Nörgler also expects a model finder (mace4) on disk. We do not ship mace4, so
# this wrapper drops a harmless stub at ../mace4/bin/mace4 (it exits 1, i.e.
# "no counter-model found") and runs Nörgler with --parallel-model-finder-mode
# none so the stub is never actually exercised.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

JAR_PATH="${SCRIPT_DIR}/bin/noergler-1.0.jar"
EPROVER_PATH="${SCRIPT_DIR}/../eprover/bin/eprover"
VAMPIRE_PATH="${SCRIPT_DIR}/../vampire/bin/vampire"
JAVA_CACHE="${SCRIPT_DIR}/.java_path"

if [[ ! -f "${JAR_PATH}" ]]; then
    echo "% SZS status Error : noergler-1.0.jar not found at ${JAR_PATH}" >&2
    echo "  Download it from https://github.com/leoprover/noergler/releases" >&2
    exit 1
fi

# --- Resolve a Java binary once, then cache it. ---------------------------
resolve_java() {
    if [[ -n "${MRS_NORGLER_JAVA:-}" && -x "${MRS_NORGLER_JAVA}" ]]; then
        printf '%s' "${MRS_NORGLER_JAVA}"; return
    fi
    if command -v java >/dev/null 2>&1; then
        command -v java; return
    fi
    if [[ -s "${JAVA_CACHE}" ]]; then
        local cached; cached="$(cat "${JAVA_CACHE}")"
        [[ -x "${cached}" ]] && { printf '%s' "${cached}"; return; }
    fi
    # Last resort: resolve via nix (slow, one-off) and cache the store path.
    local resolved
    resolved="$(nix-shell -p jre --run 'command -v java' 2>/dev/null || true)"
    if [[ -n "${resolved}" && -x "${resolved}" ]]; then
        printf '%s' "${resolved}" > "${JAVA_CACHE}"
        printf '%s' "${resolved}"; return
    fi
    echo "% SZS status Error : no Java runtime found (set MRS_NORGLER_JAVA)" >&2
    exit 1
}
JAVA="$(resolve_java)"

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
# problem it was given via --problem. ATP traces in the *deterministic* corpus
# carry the prover's original sandbox path (e.g. /export/starexec/.../bench.p),
# so by default we rewrite the leaf path to the absolute --problem path. The
# Zenodo dataset, however, already ships corrected relative file records, and
# rewriting them to an absolute path makes Nörgler reject otherwise-valid
# proofs — so callers on that dataset set MRS_NORGLER_NO_REWRITE=1.
TMP_PROOF="$(mktemp --suffix=.s)"

PROB_FILE=""
for (( i=0; i<${#ARGS[@]}; i++ )); do
    if [[ "${ARGS[$i]}" == "--problem" ]]; then
        PROB_FILE="${ARGS[$i+1]}"
        break
    fi
done

if [[ -n "${PROB_FILE}" && "${MRS_NORGLER_NO_REWRITE:-0}" != "1" ]]; then
    ABS_PROB="$(readlink -f "${PROB_FILE}")"
    sed -E "s|file\('[^']+',|file\('${ABS_PROB}',|g" "${PROOF_FILE}" > "${TMP_PROOF}"
else
    cp "${PROOF_FILE}" "${TMP_PROOF}"
fi

# Run Nörgler as a child and forward termination signals to it, so that a
# `timeout` wrapping this script actually kills the JVM (no orphans) while we
# still clean up the temp proof afterwards.
"${JAVA}" -jar "${JAR_PATH}" \
    --eprover-path "${EPROVER_PATH}" \
    --vampire-path "${VAMPIRE_PATH}" \
    --mace4-path "${MACE4_PATH}" \
    --parallel-model-finder-mode none \
    --prover eprover,vampire \
    --relax-annotation-format \
    --relax-problem-check \
    --relax-specified-inference-checks \
    --allow-prover-axioms \
    --up-to-esa \
    "${ARGS[@]}" "${TMP_PROOF}" &
JAVA_PID=$!
trap 'kill "${JAVA_PID}" 2>/dev/null || true; rm -f "${TMP_PROOF}"' TERM INT EXIT
wait "${JAVA_PID}"

