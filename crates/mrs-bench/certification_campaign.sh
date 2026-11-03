#!/usr/bin/env bash
# Full-corpus self-certification campaign.
#
# The fast gate (`certification_gate.sh`) answers "did anything regress" on the
# committed corpora. This is the expensive question: over a whole CASC edition,
# what fraction of the proofs `mrs` produces can its own strict kernel certify,
# and why not for the rest?
#
# Two phases, because they have very different costs:
#
#   search  run the normal competition route (`casc.sh`) so the proofs are the
#           ones a competition run would produce — no `--self-check`, so the
#           prover is not distorted by certification;
#   audit   replay the archived proofs through the strict kernel
#           (`audit_casc_proofs --checks strict`) and summarise the outcome per
#           division with a reason histogram.
#
# The reason histogram is the point: "18 demodulation steps could not be
# replayed" is actionable, "certification is 89%" is not.
#
# Usage:
#   crates/mrs-bench/certification_campaign.sh <casc-args...>
#
# For a pinned local corpus, `--manifest-out FILE` to `casc.sh` writes the
# division/name inventory without running any systems. Pass that inventory as
# `--subset FILE` here to run only a selected set; the campaign audits exactly
# the names resolved by phase 1.
#
# `--audit-only RUN_DIR` skips the search and audits a run directory that
# already exists. Phase 1 is the expensive half (every problem, full wall
# clock); re-running a campaign to reach the audit repeats all of it.
#
# Examples (8-core competition hardware, two jobs of four workers):
#   MRS_WORKERS=8 crates/mrs-bench/certification_campaign.sh \
#       --edition casc-j13 --systems mrs --divisions fne,feq,ueq \
#       --casc-times --jobs 2 --output results/cert-j13
#   crates/mrs-bench/certification_campaign.sh \
#       --edition casc-30 --systems mrs --divisions eps,epu,feq --casc-times --jobs 2
#   crates/mrs-bench/certification_campaign.sh --audit-only results/cert-j13
#
# Environment:
#   CERT_STRICT_TIME   per-proof kernel budget   (default 120)
#   CERT_JOBS          audit parallelism        (default 4)
#   CERT_OUT           report directory         (default <output>/certification)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/tools.sh"
ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
STRICT_TIME="${CERT_STRICT_TIME:-120}"
JOBS="${CERT_JOBS:-4}"
OUT=""
CERT_OUT="${CERT_OUT:-}"
SUBSET_FILE=""
MANIFEST_OUT=""
AUDIT_ONLY=""

# `--edition` and the problems root must agree with what phase 1 actually ran
# against. The run metadata records the corpus root; --edition and
# CASC_PROBLEMS_ROOT are fallbacks for older run directories.
args=("$@")
EDITION="casc-30"
for ((i = 0; i < ${#args[@]}; i++)); do
    case "${args[i]}" in
        --output)
            ((i + 1 < ${#args[@]})) || { echo "campaign: --output requires a directory" >&2; exit 2; }
            OUT="${args[i + 1]}"
            i=$((i + 1))
            ;;
        --audit-only)
            ((i + 1 < ${#args[@]})) || { echo "campaign: --audit-only requires a run directory" >&2; exit 2; }
            AUDIT_ONLY="${args[i + 1]}"
            i=$((i + 1))
            ;;
        --edition)
            ((i + 1 < ${#args[@]})) || { echo "campaign: --edition requires a value" >&2; exit 2; }
            EDITION="${args[i + 1]}"
            i=$((i + 1))
            ;;
        --subset)
            ((i + 1 < ${#args[@]})) || { echo "campaign: --subset requires a file" >&2; exit 2; }
            SUBSET_FILE="${args[i + 1]}"
            i=$((i + 1))
            ;;
        --subset-list-out)
            ((i + 1 < ${#args[@]})) || { echo "campaign: --subset-list-out requires a file" >&2; exit 2; }
            i=$((i + 1))
            ;;
        --manifest-out)
            ((i + 1 < ${#args[@]})) || { echo "campaign: --manifest-out requires a file" >&2; exit 2; }
            MANIFEST_OUT="${args[i + 1]}"
            i=$((i + 1))
            ;;
        --time-*)
            ((i + 1 < ${#args[@]})) || { echo "campaign: ${args[i]} requires a value" >&2; exit 2; }
            i=$((i + 1))
            ;;
    esac
done
[[ -n "${OUT}" || -n "${AUDIT_ONLY}" ]] || { echo "campaign: --output DIR is required" >&2; exit 2; }

# Always build (cargo no-ops when up to date). Building only when the binary is
# *missing* is not enough: the audit is a measurement of the current kernel, and
# a stale binary silently applies yesterday's rules. That happened here -- a
# campaign re-audited with a pre-fix `audit_casc_proofs` and reported every
# model as invalid, which is indistinguishable from "the prover emitted none".
AUDIT_BIN="${ROOT}/target/release/audit_casc_proofs"
(cd "${ROOT}" && cargo build --release -p mrs-bench --bin audit_casc_proofs)
[[ -x "${AUDIT_BIN}" ]] || { echo "campaign: ${AUDIT_BIN} not built" >&2; exit 2; }
mrs_tools --help >/dev/null

if [[ -n "${AUDIT_ONLY}" ]]; then
    RUN_DIR="${AUDIT_ONLY}"
    if [[ "${RUN_DIR}" != /* ]]; then RUN_DIR="${PWD}/${RUN_DIR}"; fi
    echo "audit-only: search skipped, auditing the archived run in place"
else
    echo "== phase 1/2: search (normal route, no self-check) =="
    (cd "${ROOT}" && ./crates/mrs-bench/casc.sh "${args[@]}")
    RUN_DIR="${ROOT}/${OUT}"
    if [[ ! -d "${RUN_DIR}" ]]; then RUN_DIR="${OUT}"; fi
    if [[ "${RUN_DIR}" != /* ]]; then RUN_DIR="${PWD}/${RUN_DIR}"; fi
fi
echo "run directory: ${RUN_DIR}"
if [[ ! -f "${RUN_DIR}/run.csv" ]]; then
    echo "campaign: no run.csv at ${RUN_DIR}/run.csv" >&2
    exit 2
fi

if [[ -n "${AUDIT_ONLY}" ]]; then
    echo "== strict-kernel audit of the archived proofs =="
else
    echo "== phase 2/2: strict-kernel audit of the archived proofs =="
fi
# Resolve the corpus the same way casc.sh does (CASC_PROBLEMS_ROOT, else
# problems/<edition>), and prefer the run's own record over both: it is what
# phase 1 provably used.
PROBLEMS_DIR=""
if [[ -f "${RUN_DIR}/run_meta.txt" ]]; then
    PROBLEMS_DIR="$(sed -n 's/^problems_dir=//p' "${RUN_DIR}/run_meta.txt" | head -1)"
fi
if [[ -z "${PROBLEMS_DIR}" || ! -d "${PROBLEMS_DIR}" ]]; then
    PROBLEMS_DIR="${CASC_PROBLEMS_ROOT:-${SCRIPT_DIR}/problems/${EDITION}}"
fi
if [[ -n "${SUBSET_FILE}" && -f "${RUN_DIR}/subset_resolved.txt" ]]; then
    SUBSET_FILE="${RUN_DIR}/subset_resolved.txt"
fi
if [[ -n "${SUBSET_FILE}" && ! -f "${SUBSET_FILE}" ]]; then
    echo "campaign: subset file does not exist: ${SUBSET_FILE}" >&2
    exit 2
fi
if [[ -n "${SUBSET_FILE}" && "${SUBSET_FILE}" != /* ]]; then
    SUBSET_FILE="${PWD}/${SUBSET_FILE}"
fi
if [[ -z "${CERT_OUT}" ]]; then
    CERT_OUT="${RUN_DIR}/certification"
elif [[ "${CERT_OUT}" != /* ]]; then
    CERT_OUT="${PWD}/${CERT_OUT}"
fi
# An audit that cannot see the problems would report every proof as unknown, or
# nothing at all. Refuse to start rather than produce a coverage number.
if [[ ! -d "${PROBLEMS_DIR}" ]]; then
    echo "campaign: problems root '${PROBLEMS_DIR}' does not exist." >&2
    echo "campaign: use the run's recorded corpus root, --edition, or CASC_PROBLEMS_ROOT." >&2
    exit 2
fi
shopt -s nullglob
CORPUS_PROBLEMS=("${PROBLEMS_DIR}"/*/*.p "${PROBLEMS_DIR}"/Problems/*/*.p)
shopt -u nullglob
if (( ${#CORPUS_PROBLEMS[@]} == 0 )); then
    echo "campaign: no .p files under '${PROBLEMS_DIR}'; refusing to audit" >&2
    echo "           ${#JOBS} job(s) of proofs against an empty corpus." >&2
    exit 2
fi
echo "audit problems root: ${PROBLEMS_DIR}"
AUDIT_SUBSET_ARGS=()
if [[ -n "${SUBSET_FILE}" ]]; then
    AUDIT_SUBSET_ARGS=(--subset "${SUBSET_FILE}")
fi
"${AUDIT_BIN}" \
    --run "${RUN_DIR}" \
    --problems-dir "${PROBLEMS_DIR}" \
    "${AUDIT_SUBSET_ARGS[@]+"${AUDIT_SUBSET_ARGS[@]}"}" \
    --checks strict \
    --strict-time "${STRICT_TIME}" \
    --jobs "${JOBS}" \
    --output "${CERT_OUT}" \
    --force

REPORT="${CERT_OUT}/audit.csv"
[[ -f "${REPORT}" ]] || { echo "campaign: no audit report at ${REPORT}" >&2; exit 2; }

echo "== outcome =="
mrs_tools summarize-certification "${REPORT}" "${RUN_DIR}/run_meta.txt"
