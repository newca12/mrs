#!/usr/bin/env bash
# crates/mrs-bench/systems/mrs/invoke.sh
# Usage: invoke.sh <problem_path> <time_limit_secs>
# Writes all output to stdout; exits with any code.
set -euo pipefail

PROBLEM="${1:?Usage: invoke.sh <problem_path> <time_limit_secs>}"
TIME_LIMIT="${2:?Usage: invoke.sh <problem_path> <time_limit_secs>}"

# Resolve workspace root: four levels above this script
# (crates/mrs-bench/systems/mrs/ -> systems/ -> mrs-bench/ -> crates/ -> root)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../../../.." && pwd)"

BINARY="${MRS_BINARY:-${WORKSPACE_ROOT}/target/release/mrs}"
if [[ ! -x "${BINARY}" ]]; then
    echo "% SZS status Error (mrs binary not found; run: cargo build --release)"
    exit 1
fi

# Set TPTP root so %include directives resolve.
# Prefer an already-set TPTP env var; fall back to the CASC-30 extracted archive.
if [[ -z "${TPTP:-}" ]]; then
    export TPTP="${SCRIPT_DIR}/../../problems/casc-30"
fi

# Determine the CASC division from the file path
DIVISION=$(basename $(dirname "$PROBLEM"))
DIV_LOWER="${DIVISION,,}"

# Select the appropriate static schedule
SCHEDULE="${MRS_SCHEDULE:-casc_${DIV_LOWER}}"

# Map CASC division names to available named schedules.
# EPS (satisfiable) and EPU (unsatisfiable) now have dedicated data-driven
# schedules; casc_epr is kept as a generic fallback.
case "${SCHEDULE}" in
    casc_feq|casc_fne|casc_ueq|casc_icu) ;;   # already have dedicated schedules
    casc_eps) ;;                               # EPS: s1-first (greedy optimal)
    casc_epu) ;;                               # EPU: s4-first (greedy optimal)
    casc_epr) ;;                               # generic EPR fallback
    *) SCHEDULE="casc" ;;                      # fallback for other divisions
esac

# Run with an internal deadline slightly below the harness limit so mrs's
# own time-check fires and prints its SZS status line (Refutation/GaveUp/
# Timeout) before an external SIGALRM/SIGXCPU could kill it mid-search
# with no output at all.
SOFT_TIME=$(( TIME_LIMIT > 2 ? TIME_LIMIT - 2 : TIME_LIMIT ))

# Raise the stack limit for the parsing/clausification phase (mrs_tptp's
# recursive-descent parser and mrs_cnf's NNF/Skolemization/CNF pipeline both
# run on the main thread, before run_schedule spawns worker threads -- see
# docs/STATUS.md). crates/mrs-tptp/doc/technical.md documents deeply nested
# formulas as a stack-overflow risk. Best-effort: some sandboxes cap the
# hard limit and refuse to raise the soft limit further, which prints a
# warning but does not abort under `set -e`.
ulimit -s unlimited 2>/dev/null || true

# `ulimit -s` above only covers the main thread, and parsing and clausification
# run there; the strategy portfolio and the certifier run on threads of their
# own. Those threads set their stack in code -- see
# mrs_core::RECURSION_STACK_BYTES and the spawn sites in mrs-search's
# strategy.rs and certified.rs, src/coordinator.rs, and mrs-proover's
# verify.rs and atp/ladder.rs -- so nothing here needs to, and nothing should:
# an ambient RUST_MIN_STACK was previously the only thing sizing some of those
# threads, and three of the four scripts that drive the certification path never
# exported it.
#
# Why the size matters at all: the search recurses through mrs-unify, mrs-core
# and mrs-index for the whole given-clause loop, and the strict kernel replays a
# whole derivation, so recursion depth follows the depth of the input terms and
# nothing bounds it. A stack overflow on a worker thread triggers Rust's abort()
# handler, killing the process with zero output, which is worse than a clean
# timeout. 64 MiB is about 300,000 levels of headroom.
#
# A lazily committed stack is free against an RSS budget, which is why 8 workers
# cost nothing here, but it is NOT free against an address-space cap: RLIMIT_AS
# charges the whole reservation, so 64 workers reserve 4 GiB before a single
# clause is processed. Anything that caps address space has to budget for that
# (crates/mrs-bench/perf_probe.sh does, and refuses a worker count that would
# not fit).

# EPS is measured with both the fail-closed certified path and the ordinary
# cooperative portfolio. Keep the normal worker allotment by running one
# certifier worker plus MRS_WORKERS-1 portfolio workers concurrently. Their
# memory ceilings partition the run-level allowance. The certified result can
# add sound EPS coverage while the established portfolio continues searching.
if [[ "${DIV_LOWER}" == "eps" && "${MRS_EPS_CERTIFY:-1}" != "0" ]]; then
    TOTAL_WORKERS="${MRS_WORKERS:-8}"
    if ! [[ "${TOTAL_WORKERS}" =~ ^[0-9]+$ ]] || (( TOTAL_WORKERS < 2 )); then
        echo "% SZS status Error (EPS certification requires MRS_WORKERS >= 2)"
        exit 1
    fi
    CERTIFY_STRATEGY="${MRS_CERTIFY_STRATEGY:-1}"
    if ! [[ "${CERTIFY_STRATEGY}" =~ ^(1|7)$ ]]; then
        echo "% SZS status Error (MRS_CERTIFY_STRATEGY must be 1 (KBO) or 7 (LPO))"
        exit 1
    fi

    TMP_DIR="$(mktemp -d)"
    trap 'rm -rf "${TMP_DIR}"' EXIT

    # The certified fragment's saturation now fans out across workers, so the
    # cert track no longer has to be single-threaded. Measured on the
    # 2026-09-26 casc-30/EPS run: 8 workers give 1.8-4.5x the closure
    # throughput, but they convert no additional problem — the closure-heavy
    # ones (GRP123-4, SYN056-1, PUZ028-3) are work-limited, not cap-limited,
    # and still miss the deadline with the caps raised 15x. So the default
    # stays at 1 and the portfolio keeps its threads; raise this on a box with
    # cores to spare (MRS_CERT_WORKERS=8 alongside a 7-worker portfolio is 15
    # threads) when the extra cert throughput is wanted for its own sake.
    CERT_WORKERS="${MRS_CERT_WORKERS:-1}"
    if ! [[ "${CERT_WORKERS}" =~ ^[0-9]+$ ]] || (( CERT_WORKERS < 1 )); then
        echo "% SZS status Error (MRS_CERT_WORKERS must be a positive integer)"
        exit 1
    fi
    PORTFOLIO_WORKERS=$((TOTAL_WORKERS - 1))
    HARDWARE_ARGS=()
    if [[ -n "${MRS_HARDWARE:-}" ]]; then
        HARDWARE_ARGS+=(--hardware "${MRS_HARDWARE}")
    fi
    if [[ -n "${MRS_SIM_TIME_FACTOR:-}" ]]; then
        HARDWARE_ARGS+=(--sim-time-factor "${MRS_SIM_TIME_FACTOR}")
    fi
    # This is one logical run split across two processes. For fixed CASC-shaped
    # profiles, divide the run-level memory allowance in proportion to worker
    # count so the two independent RSS/RLIMIT_AS guards cannot each consume the
    # full allowance. An explicit MRS_MAX_MEMORY_MB remains the run-level cap.
    CERT_MEMORY_ENV=()
    PORTFOLIO_MEMORY_ENV=()
    case "${MRS_HARDWARE:-}" in
        casc|casc-sim|casc_sim|sim) FIXED_HARDWARE_MEMORY=1 ;;
        *) FIXED_HARDWARE_MEMORY=0 ;;
    esac
    if [[ -n "${MRS_MAX_MEMORY_MB:-}" || "${FIXED_HARDWARE_MEMORY}" -eq 1 ]]; then
        TOTAL_MEMORY_MB="${MRS_MAX_MEMORY_MB:-131072}"
        if [[ "${TOTAL_MEMORY_MB}" =~ ^[0-9]+$ ]]; then
            CONCURRENT_WORKERS=$(( CERT_WORKERS + PORTFOLIO_WORKERS ))
            CERT_MEMORY_MB=$(( TOTAL_MEMORY_MB * CERT_WORKERS / CONCURRENT_WORKERS ))
            PORTFOLIO_MEMORY_MB=$(( TOTAL_MEMORY_MB - CERT_MEMORY_MB ))
            CERT_MEMORY_ENV=(MRS_MAX_MEMORY_MB="${CERT_MEMORY_MB}")
            PORTFOLIO_MEMORY_ENV=(MRS_MAX_MEMORY_MB="${PORTFOLIO_MEMORY_MB}")
        fi
    fi
    env "${CERT_MEMORY_ENV[@]+"${CERT_MEMORY_ENV[@]}"}" "${BINARY}" --time "${SOFT_TIME}" --workers "${CERT_WORKERS}" \
        "${HARDWARE_ARGS[@]+"${HARDWARE_ARGS[@]}"}" --schedule casc_eps \
        --strategy "${CERTIFY_STRATEGY}" --certify-ordered "${PROBLEM}" \
        >"${TMP_DIR}/cert.stdout" 2>"${TMP_DIR}/cert.stderr" &
    CERT_PID=$!

    PORTFOLIO_ARGS=(--time "${SOFT_TIME}" --workers "${PORTFOLIO_WORKERS}" --schedule "${SCHEDULE}")
    if [[ -n "${MRS_PORTFOLIO:-}" ]]; then
        PORTFOLIO_ARGS+=(--portfolio "${MRS_PORTFOLIO}")
    fi
    env "${PORTFOLIO_MEMORY_ENV[@]+"${PORTFOLIO_MEMORY_ENV[@]}"}" "${BINARY}" "${PORTFOLIO_ARGS[@]}" \
        "${HARDWARE_ARGS[@]+"${HARDWARE_ARGS[@]}"}" "${PROBLEM}" \
        >"${TMP_DIR}/portfolio.stdout" 2>"${TMP_DIR}/portfolio.stderr" &
    PORTFOLIO_PID=$!

    status_from() {
        local status=""
        local line
        while IFS= read -r line || [[ -n "${line}" ]]; do
            case "${line}" in
                "% SZS status "*)
                    status="${line#% SZS status }"
                    status="${status%% *}"
                    ;;
            esac
        done <"$1"
        printf '%s' "${status}"
    }

    is_definitive() {
        case "$1" in
            Theorem|Unsatisfiable|Satisfiable|CounterSatisfiable) return 0 ;;
            *) return 1 ;;
        esac
    }

    # Stop the sibling as soon as one track has a definitive answer. The two
    # tracks run on the same budget, and the certified track now answers many
    # EPS problems in milliseconds while the portfolio track still spends the
    # whole limit; blocking on both turned a 0.1 s cert into a 110 s row and
    # cost the sweep a full budget per solved problem.
    #
    # A disagreement needs two answers, so terminating the loser after the
    # winner is definitive cannot hide one: the terminated track never produced
    # a status to disagree with. When both happen to answer inside one poll
    # interval both survive and the check below runs as before. `STOPPED` names
    # the track that was cut short so the archived stderr says the comparison
    # was not applicable rather than silently passing.
    STOPPED="none"
    while kill -0 "${CERT_PID}" 2>/dev/null || kill -0 "${PORTFOLIO_PID}" 2>/dev/null; do
        _cert_now="$(status_from "${TMP_DIR}/cert.stdout")"
        _port_now="$(status_from "${TMP_DIR}/portfolio.stdout")"
        if is_definitive "${_cert_now}" && kill -0 "${PORTFOLIO_PID}" 2>/dev/null; then
            kill -TERM "${PORTFOLIO_PID}" 2>/dev/null || true
            STOPPED="portfolio"
        elif is_definitive "${_port_now}" && kill -0 "${CERT_PID}" 2>/dev/null; then
            kill -TERM "${CERT_PID}" 2>/dev/null || true
            STOPPED="cert"
        fi
        kill -0 "${CERT_PID}" 2>/dev/null || \
            kill -0 "${PORTFOLIO_PID}" 2>/dev/null || break
        sleep 0.25
    done

    CERT_RC=0
    if wait "${CERT_PID}"; then CERT_RC=0; else CERT_RC=$?; fi
    PORTFOLIO_RC=0
    if wait "${PORTFOLIO_PID}"; then PORTFOLIO_RC=0; else PORTFOLIO_RC=$?; fi

    CERT_STATUS="$(status_from "${TMP_DIR}/cert.stdout")"
    PORTFOLIO_STATUS="$(status_from "${TMP_DIR}/portfolio.stdout")"
    CERT_TIER="$(grep -m1 -o 'cert_tier=[^ ]*' "${TMP_DIR}/cert.stderr" || true)"
    CERT_ORDERING="$(grep -m1 -o 'cert_ordering=[^ ]*' "${TMP_DIR}/cert.stderr" || true)"

    printf '%% mrs EPS dual search: certification status=%s exit=%s; portfolio status=%s exit=%s workers=%s+1 time=%ss stopped=%s\n' \
        "${CERT_STATUS:-missing}" "${CERT_RC}" "${PORTFOLIO_STATUS:-missing}" \
        "${PORTFOLIO_RC}" "${PORTFOLIO_WORKERS}" "${SOFT_TIME}" "${STOPPED}" >&2

    if is_definitive "${CERT_STATUS}" && is_definitive "${PORTFOLIO_STATUS}" \
        && [[ "${CERT_STATUS}" != "${PORTFOLIO_STATUS}" ]]; then
        printf '%% SZS detail eps_cert_status=%s eps_portfolio_status=%s selected=disagreement %s %s\n' \
            "${CERT_STATUS}" "${PORTFOLIO_STATUS}" "${CERT_TIER}" "${CERT_ORDERING}" >&2
        echo "% SZS status Error (certified and portfolio searches disagree: ${CERT_STATUS} vs ${PORTFOLIO_STATUS})"
        exit 0
    fi

    if is_definitive "${CERT_STATUS}"; then
        SELECTED="cert"
    elif is_definitive "${PORTFOLIO_STATUS}"; then
        SELECTED="portfolio"
    elif [[ -n "${PORTFOLIO_STATUS}" ]]; then
        SELECTED="portfolio"
    elif [[ -n "${CERT_STATUS}" ]]; then
        SELECTED="cert"
    else
        echo "% SZS status Error (neither EPS search emitted an SZS status)"
        exit 0
    fi

    printf '%% SZS detail eps_cert_status=%s eps_portfolio_status=%s selected=%s workers=%s+%s time=%ss stopped=%s %s %s\n' \
        "${CERT_STATUS:-missing}" "${PORTFOLIO_STATUS:-missing}" "${SELECTED}" \
        "${PORTFOLIO_WORKERS}" "${CERT_WORKERS}" "${SOFT_TIME}" "${STOPPED}" \
        "${CERT_TIER}" "${CERT_ORDERING}" >&2
    cat "${TMP_DIR}/cert.stderr" >&2
    cat "${TMP_DIR}/portfolio.stderr" >&2
    if [[ "${SELECTED}" == "cert" ]]; then
        cat "${TMP_DIR}/cert.stdout"
    else
        cat "${TMP_DIR}/portfolio.stdout"
    fi
    exit 0
fi

ARGS=(--time "${SOFT_TIME}" --workers "${MRS_WORKERS:-8}" --schedule "${SCHEDULE}")
# Hardware profile: casc for a real competition-shaped run, casc-sim to simulate
# one on a development host, adaptive (the default) to fit whatever this is.
if [[ -n "${MRS_HARDWARE:-}" ]]; then
    ARGS+=(--hardware "${MRS_HARDWARE}")
fi
if [[ -n "${MRS_SIM_TIME_FACTOR:-}" ]]; then
    ARGS+=(--sim-time-factor "${MRS_SIM_TIME_FACTOR}")
fi
if [[ -n "${MRS_PORTFOLIO:-}" ]]; then
    ARGS+=(--portfolio "${MRS_PORTFOLIO}")
fi

exec "${BINARY}" "${ARGS[@]}" "${PROBLEM}"
