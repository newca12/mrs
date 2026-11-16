#!/usr/bin/env bash
# Competition-mode search and strict self-certification campaign.
#
# The fast gate (`certification_gate.sh`) answers "did anything regress" on the
# committed corpora. This is the expensive question: over a whole CASC edition,
# what fraction of the proofs `mrs` produces can its own strict kernel certify,
# and why not for the rest?
#
# Two phases, because they have very different costs:
#
#   search  by default, run the competition entry point through
#           `mrs-starexec` with CASC time limits and hardware settings. No
#           `--self-check` is used, so certification does not distort search;
#   audit   replay the archived proofs through the strict kernel
#           (`audit_casc_proofs --checks strict`) and summarise the outcome per
#           division with a reason histogram.
#
# The reason histogram is the point: "18 demodulation steps could not be
# replayed" is actionable, "certification is 89%" is not.
#
# By default this selects `--systems mrs-starexec` and `--casc-times`. Use
# `--systems mrs` with `--schedule` or `--portfolio` for local portfolio
# experiments; those are not the competition entry-point measurement.
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
# `--portfolio IDS` selects one base strategy per portfolio worker (IDs 1..15)
# when using the local `mrs` adapter.
# It is checked against `MRS_WORKERS` (default 8), except EPS with certification
# enabled, where one base slot is reserved for the certifier and the portfolio
# needs `MRS_WORKERS-1` IDs. An explicit portfolio cannot cover EPS and other
# divisions in one campaign; split the divisions. `--schedule NAME` remains the
# `casc.sh` option for selecting a named schedule. `MRS_SHARED_POOL_INTERVAL`
# is inherited (default 0); 0 disables cross-strategy equality sharing.
#
# Every successful run archives its complete output directory as a sibling
# `<output>.tar.bz2`. `--audit-only` archives the audited run directory too.
#
# Examples:
#   # Default: competition entry point, CASC limits, 8 workers, one job.
#   crates/mrs-bench/certification_campaign.sh \
#       --edition casc-j13 --divisions fne,feq,ueq \
#       --output results/cert-j13
#
#   # Explicit local portfolio experiment (not the competition entry point).
#   MRS_WORKERS=8 MRS_SHARED_POOL_INTERVAL=0 \
#   crates/mrs-bench/certification_campaign.sh \
#       --edition casc-30 --systems mrs --divisions feq --portfolio 12,11,10,6,8,1,4,13 \
#       --time-feq 240 --jobs 1 --output results/coop-feq-no-sharing
#   crates/mrs-bench/certification_campaign.sh --audit-only results/cert-j13
#
# Environment:
#   MRS_WORKERS             worker count for mrs (default 8)
#   MRS_HARDWARE            hardware profile (default casc)
#   MRS_PORTFOLIO           explicit portfolio (alternative to --portfolio)
#   MRS_EPS_CERTIFY         EPS certification track toggle (default 1)
#   MRS_SHARED_POOL_INTERVAL equality-sharing poll interval (default 0)
#   MRS_PROOF_BYTES_LIMIT   proof bytes limit (default 32MB)
#   CERT_STRICT_TIME   per-proof kernel budget   (default 240)
#   CERT_JOBS          audit parallelism        (default 4)
#   CERT_OUT           report directory         (default <output>/certification)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/tools.sh"
ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
STRICT_TIME="${CERT_STRICT_TIME:-240}"
JOBS="${CERT_JOBS:-4}"
OUT=""
CERT_OUT="${CERT_OUT:-}"
SUBSET_FILE=""
MANIFEST_OUT=""
AUDIT_ONLY=""
PORTFOLIO_ARG=""
DIVISIONS=""
SYSTEMS=""
SYSTEMS_EXPLICIT=0
SCHEDULE_EXPLICIT=0
CASC_TIMES_EXPLICIT=0
GLOBAL_TIME_EXPLICIT=0
CASC_ARGS=()

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
            CASC_ARGS+=("--output" "${args[i + 1]}")
            i=$((i + 1))
            ;;
        --audit-only)
            ((i + 1 < ${#args[@]})) || { echo "campaign: --audit-only requires a run directory" >&2; exit 2; }
            AUDIT_ONLY="${args[i + 1]}"
            i=$((i + 1))
            ;;
        --portfolio)
            ((i + 1 < ${#args[@]})) || { echo "campaign: --portfolio requires strategy IDs" >&2; exit 2; }
            [[ -n "${args[i + 1]}" ]] || { echo "campaign: --portfolio requires strategy IDs" >&2; exit 2; }
            [[ -z "${PORTFOLIO_ARG}" ]] || { echo "campaign: --portfolio may be specified only once" >&2; exit 2; }
            PORTFOLIO_ARG="${args[i + 1]}"
            i=$((i + 1))
            ;;
        --systems)
            ((i + 1 < ${#args[@]})) || { echo "campaign: --systems requires a value" >&2; exit 2; }
            SYSTEMS="${args[i + 1]}"
            SYSTEMS_EXPLICIT=1
            CASC_ARGS+=("--systems" "${args[i + 1]}")
            i=$((i + 1))
            ;;
        --schedule)
            ((i + 1 < ${#args[@]})) || { echo "campaign: --schedule requires a value" >&2; exit 2; }
            SCHEDULE_EXPLICIT=1
            CASC_ARGS+=("--schedule" "${args[i + 1]}")
            i=$((i + 1))
            ;;
        --casc-times)
            CASC_TIMES_EXPLICIT=1
            CASC_ARGS+=("--casc-times")
            ;;
        --time)
            ((i + 1 < ${#args[@]})) || { echo "campaign: --time requires a value" >&2; exit 2; }
            GLOBAL_TIME_EXPLICIT=1
            CASC_ARGS+=("--time" "${args[i + 1]}")
            i=$((i + 1))
            ;;
        --edition)
            ((i + 1 < ${#args[@]})) || { echo "campaign: --edition requires a value" >&2; exit 2; }
            EDITION="${args[i + 1]}"
            CASC_ARGS+=("--edition" "${args[i + 1]}")
            i=$((i + 1))
            ;;
        --divisions)
            ((i + 1 < ${#args[@]})) || { echo "campaign: --divisions requires a value" >&2; exit 2; }
            DIVISIONS="${args[i + 1]}"
            CASC_ARGS+=("--divisions" "${args[i + 1]}")
            i=$((i + 1))
            ;;
        --subset)
            ((i + 1 < ${#args[@]})) || { echo "campaign: --subset requires a file" >&2; exit 2; }
            SUBSET_FILE="${args[i + 1]}"
            CASC_ARGS+=("--subset" "${args[i + 1]}")
            i=$((i + 1))
            ;;
        --subset-list-out)
            ((i + 1 < ${#args[@]})) || { echo "campaign: --subset-list-out requires a file" >&2; exit 2; }
            CASC_ARGS+=("--subset-list-out" "${args[i + 1]}")
            i=$((i + 1))
            ;;
        --manifest-out)
            ((i + 1 < ${#args[@]})) || { echo "campaign: --manifest-out requires a file" >&2; exit 2; }
            MANIFEST_OUT="${args[i + 1]}"
            CASC_ARGS+=("--manifest-out" "${args[i + 1]}")
            i=$((i + 1))
            ;;
        --time-*)
            ((i + 1 < ${#args[@]})) || { echo "campaign: ${args[i]} requires a value" >&2; exit 2; }
            CASC_ARGS+=("${args[i]}" "${args[i + 1]}")
            i=$((i + 1))
            ;;
        *)
            CASC_ARGS+=("${args[i]}")
            ;;
    esac
done
[[ -n "${OUT}" || -n "${AUDIT_ONLY}" ]] || { echo "campaign: --output DIR is required" >&2; exit 2; }

# The default search is the same entry point shipped to StarExec. A caller
# explicitly asking for a named schedule or portfolio is running an experiment;
# use the local `mrs` adapter for that path because the competition wrapper
# intentionally always selects by content with --auto-schedule.
CUSTOM_ROUTING=0
if [[ -n "${PORTFOLIO_ARG}" || -n "${MRS_PORTFOLIO:-}" || \
      "${SCHEDULE_EXPLICIT}" -eq 1 || -n "${MRS_SCHEDULE:-}" ]]; then
    CUSTOM_ROUTING=1
fi
if [[ -z "${AUDIT_ONLY}" ]]; then
    if [[ "${SYSTEMS_EXPLICIT}" -eq 0 ]]; then
        if [[ "${CUSTOM_ROUTING}" -eq 1 ]]; then
            SYSTEMS="mrs"
            echo "campaign: custom schedule/portfolio requested; using local --systems mrs"
            echo "campaign: this search is a portfolio experiment, not the competition entry point"
        else
            SYSTEMS="mrs-starexec"
            echo "campaign: defaulting to the competition entry point (--systems mrs-starexec)"
        fi
        CASC_ARGS+=("--systems" "${SYSTEMS}")
    elif [[ "${CUSTOM_ROUTING}" -eq 1 && ",${SYSTEMS,,}," != *",mrs,"* ]]; then
        echo "campaign: --schedule/--portfolio overrides require --systems mrs; mrs-starexec always uses --auto-schedule" >&2
        exit 2
    fi

    if [[ "${CASC_TIMES_EXPLICIT}" -eq 0 && "${GLOBAL_TIME_EXPLICIT}" -eq 0 ]]; then
        CASC_ARGS+=("--casc-times")
        echo "campaign: defaulting to per-edition CASC time limits"
    fi
fi

# Match casc.sh's edition-specific default when an explicit portfolio is
# validated without a --divisions option. In particular CASC-J12 has no EPS,
# and its default roster includes the THF/TFA/TFN/SLH/ICU divisions.
if [[ -z "${DIVISIONS}" ]]; then
    if [[ "${EDITION}" == "casc-j12" ]]; then
        DIVISIONS="tne,teq,tfi,tfe,tfn,fne,feq,ueq,slh,icu"
    else
        DIVISIONS="fne,feq,epu,eps,ueq,icu"
    fi
fi

# Resolve explicit portfolio configuration before starting the expensive search.
# An audit-only run does not search, so its result must not depend on the caller's
# current prover environment.
if [[ -z "${AUDIT_ONLY}" ]]; then
    # Both the competition entry point and local mrs adapter default to eight
    # workers; use the same value when checking one strategy per portfolio slot.
    if [[ -n "${PORTFOLIO_ARG}" ]]; then
        MRS_PORTFOLIO="${PORTFOLIO_ARG}"
    fi
    MRS_WORKERS="${MRS_WORKERS:-8}"
    MRS_SHARED_POOL_INTERVAL="${MRS_SHARED_POOL_INTERVAL:-0}"
    MRS_HARDWARE="${MRS_HARDWARE:-casc}"
    if ! [[ "${MRS_WORKERS}" =~ ^[1-9][0-9]*$ ]]; then
        echo "campaign: MRS_WORKERS must be a positive integer" >&2
        exit 2
    fi
    if ! [[ "${MRS_SHARED_POOL_INTERVAL}" =~ ^[0-9]+$ ]]; then
        echo "campaign: MRS_SHARED_POOL_INTERVAL must be a non-negative integer" >&2
        exit 2
    fi
    if [[ -n "${MRS_PORTFOLIO:-}" ]]; then
        if ! [[ "${MRS_PORTFOLIO}" =~ ^([1-9]|1[0-5])(,([1-9]|1[0-5]))*$ ]]; then
            echo "campaign: portfolio must be comma-separated strategy IDs in 1..15" >&2
            exit 2
        fi
        IFS=',' read -r -a PORTFOLIO_IDS <<< "${MRS_PORTFOLIO}"
        EXPECTED_PORTFOLIO_WORKERS="${MRS_WORKERS}"
        HAS_EPS=0
        HAS_OTHER_DIVISION=0
        if [[ "${MRS_EPS_CERTIFY:-1}" != "0" ]]; then
            IFS=',' read -r -a SELECTED_DIVISIONS <<< "${DIVISIONS}"
            for division in "${SELECTED_DIVISIONS[@]}"; do
                case "${division,,}" in
                    eps) HAS_EPS=1 ;;
                    "") ;;
                    *) HAS_OTHER_DIVISION=1 ;;
                esac
            done
            if (( HAS_EPS )); then
                if (( MRS_WORKERS < 2 )); then
                    echo "campaign: EPS certification requires MRS_WORKERS >= 2" >&2
                    exit 2
                fi
                if (( HAS_OTHER_DIVISION )); then
                    echo "campaign: one explicit portfolio cannot cover EPS and non-EPS divisions while EPS certification is enabled (EPS uses MRS_WORKERS-1 portfolio workers; other divisions use MRS_WORKERS); split the campaign by division" >&2
                    exit 2
                fi
                EXPECTED_PORTFOLIO_WORKERS=$((MRS_WORKERS - 1))
            fi
        fi
        if (( ${#PORTFOLIO_IDS[@]} != EXPECTED_PORTFOLIO_WORKERS )); then
            if (( HAS_EPS )); then
                echo "campaign: EPS reserves one of MRS_WORKERS=${MRS_WORKERS} for certification; its portfolio needs ${EXPECTED_PORTFOLIO_WORKERS} strategies, got ${#PORTFOLIO_IDS[@]}" >&2
            else
                echo "campaign: portfolio has ${#PORTFOLIO_IDS[@]} strategies but the selected worker count requires ${EXPECTED_PORTFOLIO_WORKERS}" >&2
            fi
            exit 2
        fi
        export MRS_PORTFOLIO
    fi
    export MRS_WORKERS MRS_SHARED_POOL_INTERVAL MRS_HARDWARE ${MRS_PROOF_BYTES_LIMIT:+MRS_PROOF_BYTES_LIMIT}
fi
command -v tar >/dev/null 2>&1 || { echo "campaign: tar is required to archive the run" >&2; exit 2; }

# Build the competition binary and audit tool before the campaign. Building only
# when a binary is *missing* is not enough: stale executables silently measure
# yesterday's prover or checker. Cargo is a no-op when each target is current.
if [[ -z "${AUDIT_ONLY}" ]]; then
    (cd "${ROOT}" && cargo build --release --bin mrs)
fi
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
    (cd "${ROOT}" && ./crates/mrs-bench/casc.sh "${CASC_ARGS[@]}")
    RUN_DIR="${ROOT}/${OUT}"
    if [[ ! -d "${RUN_DIR}" ]]; then RUN_DIR="${OUT}"; fi
    if [[ "${RUN_DIR}" != /* ]]; then RUN_DIR="${PWD}/${RUN_DIR}"; fi
fi
echo "run directory: ${RUN_DIR}"
if [[ ! -f "${RUN_DIR}/run.csv" ]]; then
    echo "campaign: no run.csv at ${RUN_DIR}/run.csv" >&2
    exit 2
fi
RUN_DIR="$(cd "${RUN_DIR}" && pwd -P)"

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
CORPUS_PROBLEM="$(find -L "${PROBLEMS_DIR}" -type f -name '*.p' -print -quit 2>/dev/null || true)"
if [[ -z "${CORPUS_PROBLEM}" ]]; then
    echo "campaign: no .p files under '${PROBLEMS_DIR}'; refusing to audit" >&2
    echo "           refusing to audit proofs against an empty corpus." >&2
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

# Keep the archive beside the run directory, rather than inside it, so the
# archive never attempts to include itself. Stage and rename atomically so an
# interrupted compression cannot leave a plausible-looking partial archive.
RUN_PARENT="$(dirname "${RUN_DIR}")"
RUN_NAME="$(basename "${RUN_DIR}")"
ARCHIVE="${RUN_DIR}.tar.bz2"
if ! ARCHIVE_TMP="$(mktemp "${ARCHIVE}.tmp.XXXXXX")"; then
    echo "campaign: could not create temporary archive beside ${ARCHIVE}" >&2
    exit 2
fi
if ! tar -cjf "${ARCHIVE_TMP}" -C "${RUN_PARENT}" "${RUN_NAME}"; then
    rm -f -- "${ARCHIVE_TMP}"
    echo "campaign: failed to create archive ${ARCHIVE}" >&2
    exit 2
fi
if ! mv -f -- "${ARCHIVE_TMP}" "${ARCHIVE}"; then
    rm -f -- "${ARCHIVE_TMP}"
    echo "campaign: failed to publish archive ${ARCHIVE}" >&2
    exit 2
fi
echo "archive: ${ARCHIVE}"
