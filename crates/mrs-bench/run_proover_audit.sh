#!/usr/bin/env bash
# run_proover_audit.sh
#
# Independent-proof-verification companion to run_soundness_audit.sh.
#
# Where run_soundness_audit.sh only checks the SZS *status* line against a
# curated list of known non-theorems (so it can only catch bugs on problems
# whose ground-truth answer we already know), this script runs `mrs` through
# `mrs-codex`, which automatically hands every `Theorem`/`Unsatisfiable`
# result's TSTP proof to `mrs-proover --only-mrs` for independent structural
# verification. This catches unsound *proofs* regardless of whether the
# problem's answer was known in advance -- it is the general-purpose net that
# would have caught the PRO013+3.p incident even though that problem was not
# in any curated non-theorem corpus.
#
# Usage:
#   crates/mrs-bench/run_proover_audit.sh [list-file] [db-file]
#
#   list-file  Relative-path list of TPTP problems (one per line), resolved
#              against $TPTP. Defaults to fof_non_theorems.list.
#   db-file    SQLite database written by mrs-codex. Defaults to
#              proover_audit.db in the current directory.
#
# Env vars:
#   MRS_AUDIT_TIMEOUT  Per-problem time limit in seconds. Default 30.
#   MRS_WORKERS        Worker threads per `mrs` invocation. Default 8
#                       (matches CASC hardware).
#   MRS_CODEX_JOBS      Parallel problems run at once by mrs-codex. Default
#                       max(1, detected-cores / MRS_WORKERS), so
#                       jobs * MRS_WORKERS never oversubscribes the machine.
#                       Override if you want something else, e.g.
#                       MRS_WORKERS=1 MRS_CODEX_JOBS=$(nproc) for maximum
#                       throughput via fully deterministic single-strategy
#                       runs (see AGENTS.md's --workers 1 note) instead of
#                       fewer, competition-faithful 8-worker runs.
#
# Requires: $TPTP set (or one of the default paths below), and
#   cargo build --release -p mrs -p mrs-proover -p mrs-codex
# (this script will attempt that build automatically if binaries are missing).
#
# Any line containing "[FAILED Verif]" is a confirmed soundness bug: mrs
# reported Theorem/Unsatisfiable but mrs-proover could not validate the proof.
# Treat that as a blocking failure -- do not submit.
#
# Any Theorem/Unsatisfiable line with *no* verification marker at all means
# mrs-proover returned Unknown (could not decide, e.g. CWA's componentwise
# proofs are not run through a connected checkable derivation and the
# `split_component` rule has no dedicated check in mrs-proover). These are
# not proven unsound, but are not proven sound either -- inspect them by hand,
# e.g. by re-running the full ATP ladder directly:
#   ./target/release/mrs-proover <proof.p>            # eprover+vampire+mrs
# instead of the `--only-mrs` fast path mrs-codex uses internally.
#
# Concurrency note: without an explicit --jobs, mrs-codex would default to
# one parallel problem per CPU core while each of those problems' `mrs`
# invocation *also* spawns MRS_WORKERS threads internally -- N cores x 8
# workers/job = 8N threads on an N-core box. That's severe oversubscription
# that both wastes wall-clock time and (per AGENTS.md's architecture notes
# on wall-clock-sensitive LRS pruning) can produce spurious timeouts/GaveUps
# that mask exactly the proof issues this audit exists to catch. See the
# MRS_CODEX_JOBS env var above for how this script avoids that.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

LIST_FILE="${1:-${SCRIPT_DIR}/fof_non_theorems.list}"
DB_FILE="${2:-${WORKSPACE_ROOT}/proover_audit.db}"
TIMEOUT_SECS="${MRS_AUDIT_TIMEOUT:-30}"
WORKERS_PER_JOB="${MRS_WORKERS:-8}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
NC='\033[0m'

# Resolve TPTP directory (same convention as run_soundness_audit.sh).
TPTP_DIR="${TPTP:-}"
if [[ -z "${TPTP_DIR}" ]]; then
    default_paths=(
        "/mnt/fastdata/TPTP-v9.2.1"
        "/DATA/ai/user/TPTP-v9.2.1"
        "/mnt/sdd/TPTP-v9.2.1"
        "/mnt/sda1/TPTP-v9.2.1"
    )
    for path in "${default_paths[@]}"; do
        if [[ -d "${path}" ]]; then
            TPTP_DIR="${path}"
            break
        fi
    done
fi

if [[ -z "${TPTP_DIR}" ]]; then
    echo -e "${RED}Error:${NC} \$TPTP is not set and no default TPTP library was found." >&2
    echo -e "Please export \$TPTP before running this script, e.g.:" >&2
    echo -e "  export TPTP=/path/to/TPTP-v9.2.1" >&2
    exit 1
fi

if [[ ! -f "${LIST_FILE}" ]]; then
    echo "Error: list file '${LIST_FILE}' not found." >&2
    exit 1
fi

MRS_BIN="${WORKSPACE_ROOT}/target/release/mrs"
PROOVER_BIN="${WORKSPACE_ROOT}/target/release/mrs-proover"
if [[ ! -x "${MRS_BIN}" || ! -x "${PROOVER_BIN}" ]]; then
    echo -e "${YELLOW}mrs / mrs-proover release binaries missing; building...${NC}"
    (cd "${WORKSPACE_ROOT}" && cargo build --release -p mrs -p mrs-proover -p mrs-codex)
fi

# Detect available cores, portably. Falls back to 1 (i.e. --jobs 1) if
# neither `nproc` nor `getconf` is available, which is always safe (just
# slower), never oversubscribed.
CORES="$(nproc 2>/dev/null || getconf _NPROCESSORS_ONLN 2>/dev/null || echo 1)"

# Size --jobs so that jobs * workers-per-job never exceeds the core count.
# MRS_CODEX_JOBS overrides this entirely if set.
if [[ -n "${MRS_CODEX_JOBS:-}" ]]; then
    CODEX_JOBS="${MRS_CODEX_JOBS}"
else
    CODEX_JOBS=$(( CORES / WORKERS_PER_JOB ))
    if (( CODEX_JOBS < 1 )); then
        CODEX_JOBS=1
    fi
fi

STAGE_DIR="$(mktemp -d)"
trap 'rm -rf "${STAGE_DIR}"' EXIT

echo -e "${CYAN}=== Staging problems from ${LIST_FILE} ===${NC}"
staged=0
missing=0
while read -r problem || [[ -n "${problem}" ]]; do
    [[ -z "${problem}" || "${problem}" =~ ^# ]] && continue
    src="${TPTP_DIR}/${problem}"
    if [[ ! -f "${src}" ]]; then
        missing=$((missing + 1))
        continue
    fi
    dst="${STAGE_DIR}/${problem}"
    mkdir -p "$(dirname "${dst}")"
    cp "${src}" "${dst}"
    staged=$((staged + 1))
done < "${LIST_FILE}"

echo -e "Staged  : ${GREEN}${staged}${NC} problem(s)"
if (( missing > 0 )); then
    echo -e "Missing : ${YELLOW}${missing}${NC} problem(s) not found under \$TPTP (skipped)"
fi
if (( staged == 0 )); then
    echo -e "${RED}Nothing to audit.${NC}" >&2
    exit 1
fi

echo -e "${CYAN}=== Running mrs-codex (mrs + automatic mrs-proover --only-mrs verification) ===${NC}"
echo -e "TPTP Library  : ${YELLOW}${TPTP_DIR}${NC}"
echo -e "Database      : ${YELLOW}${DB_FILE}${NC}"
echo -e "Time Limit    : ${YELLOW}${TIMEOUT_SECS}s${NC} per problem"
echo -e "Workers/job   : ${YELLOW}${WORKERS_PER_JOB}${NC}"
echo -e "Cores detected: ${YELLOW}${CORES}${NC}"
echo -e "Parallel jobs : ${YELLOW}${CODEX_JOBS}${NC} (jobs x workers = $(( CODEX_JOBS * WORKERS_PER_JOB )), avoiding oversubscription of ${CORES} core(s))"
echo "--------------------------------------------------------"

LOG_FILE="$(mktemp)"
"${WORKSPACE_ROOT}/target/release/mrs-codex" "${STAGE_DIR}" \
    --db "${DB_FILE}" \
    --system "mrs-0.2.1-proover-audit" \
    --timeout "${TIMEOUT_SECS}" \
    --jobs "${CODEX_JOBS}" \
    --cmd "${MRS_BIN} --schedule casc --workers ${WORKERS_PER_JOB} --time {timeout} {file}" \
    | tee "${LOG_FILE}"

echo "--------------------------------------------------------"

failed=$(grep -c "\[FAILED Verif\]" "${LOG_FILE}" || true)
unknown=$(grep -E "\.p \.\.\. (Theorem|Unsatisfiable)" "${LOG_FILE}" | grep -vc "\[Verified\]\|\[FAILED Verif\]" || true)

echo -e "${CYAN}Audit Finished!${NC}"
echo -e "  * Confirmed unsound (FAILED Verif) : ${RED}${failed}${NC}"
echo -e "  * Unverified Theorem/Unsatisfiable  : ${YELLOW}${unknown}${NC} (mrs-proover returned Unknown; inspect by hand)"

if (( failed > 0 )); then
    echo -e "${RED}SOUNDNESS BUG CONFIRMED by mrs-proover -- do NOT submit.${NC}"
    grep "\[FAILED Verif\]" "${LOG_FILE}"
    rm -f "${LOG_FILE}"
    exit 2
fi

if (( unknown > 0 )); then
    echo -e "${YELLOW}No confirmed unsoundness, but ${unknown} result(s) could not be independently verified.${NC}"
    echo -e "${YELLOW}Re-check these with the full ATP ladder (./target/release/mrs-proover <proof.p>) before submitting.${NC}"
fi

rm -f "${LOG_FILE}"
echo -e "${GREEN}No confirmed unsoundness detected by mrs-proover.${NC}"
