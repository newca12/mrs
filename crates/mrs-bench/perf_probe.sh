#!/usr/bin/env bash
# crates/mrs-bench/perf_probe.sh
#
# Quick, low-footprint evaluation of mrs performance on whatever machine this
# runs on, with the results appended to a bank under docs/results/perf/.
#
# What it measures, and why it is comparable across machines:
#
#   Every search is stopped by an iteration-counted ceiling (max_processed) and
#   LRS is given a matching logical-iteration budget per strategy, so the amount of work is
#   fixed and identical on every host. Only the elapsed time varies. That is
#   what makes a row from one machine comparable with a row from another; a
#   "run it for 30 seconds and count clauses" measurement is not, because the
#   search is wall-clock sensitive.
#
#   The workload is a generated clause set, so no TPTP corpus, no `%include`,
#   and no TPTP environment variable is involved. It is driven through
#   mrs_search::strategy::run_schedule, the same entry point the prover uses
#   for a real problem.
#
# Usage:
#   perf_probe.sh [OPTIONS]
#
# Options:
#   --processed N       Fixed-work ceiling, clauses per worker (default: 5000)
#   --repeat N          Runs per configuration; the fastest is banked and the
#                       spread of the rest is recorded as the row's noise floor
#                       (default: 3)
#   --workers LIST      Comma-separated worker counts to measure
#                       (default: 1,<physical cores>)
#   --shape NAME        mixed | equational | relational (default: mixed)
#   --clauses N         Generated input clauses (default: 600)
#   --seed N            Generator seed, decimal or 0x hex
#   --variants LIST     Comma-separated target-cpu builds (default: native,haswell).
#                       A build the CPU cannot execute is dropped, not run; if
#                       that leaves nothing, native is measured instead
#   --memory-budget-mb N  Process RAM ceiling (default: 12288)
#   --hard-cap MODE     none | cgroup-memory | rlimit-as | auto (default: auto)
#   --out-dir DIR       Where to keep the raw JSON rows (default:
#                       crates/mrs-bench/results/perf/<timestamp>)
#   --no-bank           Measure only; do not touch docs/
#   --no-verify         Skip the repeat run that confirms the work is identical
#   -h, --help          Show this help
#
# Environment:
#   MRS_PERF_BINARY     Use this prebuilt perf_probe binary and do not build
#   MRS_PERF_SKIP_BUILD=1  Do not build; use the existing binary
#   MRS_PERF_CPUINFO   Read CPU flags from this file instead of /proc/cpuinfo.
#                       Test hook: point it at a cpuinfo with the AVX2 flags
#                       stripped to see the pre-Haswell path on a host that
#                       supports them
#
# Examples:
#   # Everything: two target-cpu builds, one worker and all physical cores.
#   crates/mrs-bench/perf_probe.sh
#
#   # Fast look, do not write to docs/.
#   crates/mrs-bench/perf_probe.sh --processed 1000 --no-bank
#
#   # A single build, no AVATAR, on two workers.
#   MRS_PERF_SKIP_BUILD=1 crates/mrs-bench/perf_probe.sh --variants native --workers 2
#
# On a host with more cores than the 12 GiB ceiling can hold workers for, the
# probe refuses the default width and prints the largest worker count it can run.
# On a Nix-based development shell, `direnv exec .` (or an interactive shell
# with the flake loaded) must already provide cargo. This script deliberately
# does not invoke `nix develop`: it has to run unchanged on ordinary Linux
# hosts that just have a Rust toolchain.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

# ---------------------------------------------------------------------------
# Defaults
# ---------------------------------------------------------------------------
PROCESSED=5000
REPEAT=3
WORKER_LIST=""
SHAPE=mixed
CLAUSES=600
SEED=""
VARIANTS="native,haswell"
MEMORY_BUDGET_MB=12288
HARD_CAP=auto
OUT_DIR=""
DO_BANK=1
DO_VERIFY=1

# Total RAM the probe is allowed to use across everything it runs. The phases
# run one at a time, so this is also the per-process ceiling; `MRS_MAX_MEMORY_MB`
# hands it to the process-wide RSS watchdog. The watchdog is a poller, so a fast
# allocation burst can overshoot it slightly; `--hard-cap` adds a kernel-level
# ceiling when an exact bound matters.
#
# That ceiling is a *total*, and it is shared by every worker, so it also caps
# how many workers this host may be asked for. See `worker_ceiling`.
readonly TOTAL_BUDGET_MB=12288
# Below this the host has nothing left to measure with, so refuse rather than
# produce a number shaped by swapping.
readonly MIN_AVAILABLE_MB=2048

# What one search worker costs out of the budget. The heap figure is the
# marginal RSS the bank already measures for a worker: 288 MB on the i7-5820K
# ((1772-332)/5), 285 MB on the i7-10610U ((1187-333)/3) and 293 MB on the
# i3-5010U (625-332) — consistent to within 3%, and rounded up for margin.
readonly WORKER_HEAP_MB=320
# Each worker also reserves a 64 MiB stack (mrs-search/src/strategy.rs, kept
# large on purpose: the search recurses and an overflow aborts with no output).
# A lazily committed stack is free against an RSS budget, but an address-space
# cap charges the whole reservation up front — 64 workers is 4 GiB before a
# single clause is processed, which is what made a 64-core host abort.
readonly WORKER_STACK_MB=64

# ISA extensions the `haswell` build needs. Building it on an older chip and
# running it would die with SIGILL, which would look like a probe failure
# instead of an unsupported configuration, so the list has to cover every
# instruction `-C target-cpu=haswell` can emit — a CPU that passes a short list
# and still dies leaves the reader with a core dump instead of a missing row.
#
# Two lists, because the two namespaces do not agree. `rustc --print cfg -C
# target-cpu=haswell` reports `cmpxchg16b` where /proc/cpuinfo says `cx16`, and
# names below are the cpuinfo ones.
#
# The `sse*`, `xsave*`, `fxsr` and `xsaveopt` features that command also reports
# are omitted: anything with AVX2 has them. So is `lzcnt`, which the kernel does
# not report for Intel CPUs at all (they have had it since Core 2, but their
# flags line never lists it), so gating on it would skip `haswell` on every
# Intel host. Those omissions cannot cost a real row; they only mean the gate
# is not a proof, and the SIGILL check in `run_measure` is the backstop.
#
# A pre-Haswell chip (Xeon E5-2407, for instance) fails the first flag and is
# measured as `native` only.
readonly HASWELL_FLAGS="avx avx2 bmi1 bmi2 cx16 f16c fma movbe pclmulqdq popcnt rdrand"

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
log() { printf '[perf] %s\n' "$*" >&2; }
die() { printf '[perf] error: %s\n' "$*" >&2; exit 1; }

usage() { sed -n '2,/^set -euo/p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//; $d'; }

# Count process-usable physical cores from the kernel affinity mask and sysfs
# SMT topology, then cap by any cgroup CPU quota. `nproc` alone counts logical
# CPUs and ignores topology, so it is only the last-resort fallback.
physical_cores() {
    local cpu_list cpu sibling first path count=0 part lo hi quota period quota_cores
    local sibling_part sibling_cpu sibling_lo sibling_hi
    declare -A allowed=()
    declare -A seen=()
    cpu_list="$(awk '/^Cpus_allowed_list:/{print $2; exit}' /proc/self/status 2>/dev/null || true)"
    if [[ -n "${cpu_list}" ]]; then
        local -a parts=()
        IFS=',' read -r -a parts <<<"${cpu_list}"
        for part in "${parts[@]}"; do
            if [[ "${part}" =~ ^([0-9]+)-([0-9]+)$ ]]; then
                lo="${BASH_REMATCH[1]}"
                hi="${BASH_REMATCH[2]}"
                for ((cpu = lo; cpu <= hi; cpu++)); do
                    allowed["${cpu}"]=1
                done
            elif [[ "${part}" =~ ^[0-9]+$ ]]; then
                allowed["${part}"]=1
            fi
        done
    fi
    for cpu in "${!allowed[@]}"; do
        path="/sys/devices/system/cpu/cpu${cpu}/topology/thread_siblings_list"
        if [[ -r "${path}" ]]; then
            sibling="$(<"${path}")"
            first=""
            local -a sibling_parts=()
            IFS=',' read -r -a sibling_parts <<<"${sibling}"
            for sibling_part in "${sibling_parts[@]}"; do
                if [[ "${sibling_part}" =~ ^([0-9]+)-([0-9]+)$ ]]; then
                    sibling_lo="${BASH_REMATCH[1]}"
                    sibling_hi="${BASH_REMATCH[2]}"
                    for ((sibling_cpu = sibling_lo; sibling_cpu <= sibling_hi; sibling_cpu++)); do
                        if [[ -n "${allowed[${sibling_cpu}]:-}" ]]; then
                            first="${sibling_cpu}"
                            break
                        fi
                    done
                elif [[ "${sibling_part}" =~ ^[0-9]+$ ]] \
                    && [[ -n "${allowed[${sibling_part}]:-}" ]]; then
                    first="${sibling_part}"
                fi
                [[ -n "${first}" ]] && break
            done
            [[ -n "${first}" ]] || first="${cpu}"
        else
            first="${cpu}"
        fi
        if [[ -z "${seen[${first}]:-}" ]]; then
            seen["${first}"]=1
            (( count += 1 ))
        fi
    done
    if (( count == 0 )); then
        count="$(nproc 2>/dev/null || printf '1')"
    fi

    if [[ -r /sys/fs/cgroup/cpu.max ]]; then
        read -r quota period < /sys/fs/cgroup/cpu.max
        if [[ "${quota}" =~ ^[0-9]+$ && "${period:-0}" =~ ^[0-9]+$ ]] && (( period > 0 )); then
            quota_cores=$(( (quota + period - 1) / period ))
            (( quota_cores > 0 && quota_cores < count )) && count="${quota_cores}"
        fi
    elif [[ -r /sys/fs/cgroup/cpu/cpu.cfs_quota_us && -r /sys/fs/cgroup/cpu/cpu.cfs_period_us ]]; then
        read -r quota < /sys/fs/cgroup/cpu/cpu.cfs_quota_us
        read -r period < /sys/fs/cgroup/cpu/cpu.cfs_period_us
        if [[ "${quota}" =~ ^[0-9]+$ && "${period}" =~ ^[0-9]+$ ]] && (( quota > 0 && period > 0 )); then
            quota_cores=$(( (quota + period - 1) / period ))
            (( quota_cores > 0 && quota_cores < count )) && count="${quota_cores}"
        fi
    fi
    (( count > 0 )) || count=1
    printf '%s' "${count}"
}

mem_available_mb() {
    awk '/^MemAvailable:/{printf "%d", $2/1024; exit}' /proc/meminfo 2>/dev/null || printf '0'
}

# True when the CPU advertises every flag in $1.
#
# `MRS_PERF_CPUINFO` exists so the skip path can be exercised on a host that
# does have the instructions: point it at a cpuinfo with the flags stripped and
# the probe measures `native` only, exactly as it would on a pre-Haswell CPU.
cpu_has_flags() {
    local flags="$1" flag cpuinfo="${MRS_PERF_CPUINFO:-/proc/cpuinfo}"
    [[ -r "${cpuinfo}" ]] || return 1
    for flag in ${flags}; do
        grep -qw "${flag}" "${cpuinfo}" || return 1
    done
    return 0
}

# Most workers this host may be asked for, given the memory budget.
#
# Divides the budget by what one worker costs. Under an address-space cap the
# stack reservation is charged on top of the heap, because that is what the
# kernel can fail the allocation on; a cgroup memory cap and the prover's own
# RSS watchdog both look at resident pages, which never include an untouched
# stack.
worker_ceiling() {
    local per_worker="${WORKER_HEAP_MB}"
    if [[ "${HARD_CAP_IMPL}" == "rlimit-as" ]]; then
        per_worker=$(( per_worker + WORKER_STACK_MB ))
    fi
    printf '%s' "$(( MEMORY_BUDGET_MB / per_worker ))"
}

json_field() {
    # First value of a string field from a JSONL row, without needing jq.
    local file="$1" field="$2"
    sed -n "s/.*\"${field}\":\"\\([^\"]*\\)\".*/\\1/p" "${file}" | head -1
}

json_number() {
    # First value of a numeric field from a JSONL row.
    local file="$1" field="$2"
    sed -n "s/.*\"${field}\":\\([0-9][0-9.eE+-]*\\).*/\\1/p" "${file}" | head -1
}

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------
while [[ $# -gt 0 ]]; do
    case "$1" in
        --processed)          PROCESSED="${2:?}"; shift 2 ;;
        --repeat)             REPEAT="${2:?}"; shift 2 ;;
        --workers)            WORKER_LIST="${2:?}"; shift 2 ;;
        --shape)              SHAPE="${2:?}"; shift 2 ;;
        --clauses)            CLAUSES="${2:?}"; shift 2 ;;
        --seed)               SEED="${2:?}"; shift 2 ;;
        --variants)           VARIANTS="${2:?}"; shift 2 ;;
        --memory-budget-mb)   MEMORY_BUDGET_MB="${2:?}"; shift 2 ;;
        --hard-cap)           HARD_CAP="${2:?}"; shift 2 ;;
        --out-dir)            OUT_DIR="${2:?}"; shift 2 ;;
        --no-bank)            DO_BANK=0; shift ;;
        --no-verify)          DO_VERIFY=0; shift ;;
        -h|--help)            usage; exit 0 ;;
        *)                    die "unknown option $1 (try --help)" ;;
    esac
done

case "${HARD_CAP}" in
    auto|none|cgroup-memory|rlimit-as) ;;
    *) die "--hard-cap must be auto, none, cgroup-memory, or rlimit-as" ;;
esac

# Decide which builds can actually run here, before anything is built or run, so
# that the measure loop, the verification pass and the recorded command all see
# the same list. A CPU that cannot execute a build is not a failure to report
# around: on a pre-Haswell host the `haswell` row simply does not exist, and
# `native` is the only comparable number available.
IFS=',' read -r -a REQUESTED_VARIANTS <<< "${VARIANTS}"
VARIANT_LIST=()
for variant in "${REQUESTED_VARIANTS[@]}"; do
    if [[ "${variant}" == "haswell" ]] && ! cpu_has_flags "${HASWELL_FLAGS}"; then
        missing=""
        for flag in ${HASWELL_FLAGS}; do
            cpu_has_flags "${flag}" || missing="${missing} ${flag}"
        done
        log "skipping target-cpu=haswell: this CPU is missing${missing}"
        continue
    fi
    VARIANT_LIST+=("${variant}")
done
if (( ${#VARIANT_LIST[@]} == 0 )); then
    log "warning: none of the requested builds (${VARIANTS}) can run on this CPU; measuring target-cpu=native instead"
    VARIANT_LIST=(native)
fi
VARIANTS="$(IFS=','; printf '%s' "${VARIANT_LIST[*]}")"

if [[ -n "${MRS_PERF_BINARY:-}" && "${VARIANTS}" == *,* ]]; then
    die "MRS_PERF_BINARY supplies one build; set --variants to exactly one target-cpu name"
fi

[[ "${PROCESSED}" =~ ^[0-9]+$ ]] && (( PROCESSED > 0 )) || die "--processed must be a positive integer"
[[ "${REPEAT}" =~ ^[0-9]+$ ]] && (( REPEAT > 0 )) || die "--repeat must be a positive integer"
[[ "${MEMORY_BUDGET_MB}" =~ ^[0-9]+$ ]] && (( MEMORY_BUDGET_MB > 0 )) || die "--memory-budget-mb must be a positive integer"
[[ "${CLAUSES}" =~ ^[0-9]+$ ]] && (( CLAUSES > 0 )) || die "--clauses must be a positive integer"
(( MEMORY_BUDGET_MB <= TOTAL_BUDGET_MB )) \
    || die "--memory-budget-mb ${MEMORY_BUDGET_MB} exceeds the ${TOTAL_BUDGET_MB} MB (12 GiB) ceiling this probe promises"

# The default worker list follows the host's core count, but the authoritative
# count is the prover's own (`perf_probe cores`), which is not known until a
# binary exists. `WORKER_LIST` is therefore provisional when it comes from the
# count and final when the caller passed it: see `resolve_core_count`, which
# re-derives the default before anything is measured.
CORES="$(physical_cores)"
# `nproc` honours the affinity mask, so this is the logical CPU count the same
# process may use. Reported next to the physical count because the two must
# differ on any SMT host, and a run where they agree is either a host without
# SMT or a topology that could not be read.
LOGICAL_CPUS="$(nproc 2>/dev/null || printf '0')"
WORKER_LIST_FROM_COUNT=0
if [[ -z "${WORKER_LIST}" ]]; then
    WORKER_LIST_FROM_COUNT=1
    if (( CORES == 1 )); then
        WORKER_LIST="1"
    else
        WORKER_LIST="1,${CORES}"
    fi
fi

TIMESTAMP="$(date +%Y%m%d_%H%M%S)"
[[ -n "${OUT_DIR}" ]] || OUT_DIR="${SCRIPT_DIR}/results/perf/${TIMESTAMP}"
mkdir -p "${OUT_DIR}"
ROWS="${OUT_DIR}/rows.jsonl"
: > "${ROWS}"

# ---------------------------------------------------------------------------
# Pre-flight
# ---------------------------------------------------------------------------
AVAILABLE_MB="$(mem_available_mb)"
if (( AVAILABLE_MB < MIN_AVAILABLE_MB )); then
    die "only ${AVAILABLE_MB} MB available; the probe needs at least ${MIN_AVAILABLE_MB} MB to produce a meaningful number"
fi
if (( AVAILABLE_MB < MEMORY_BUDGET_MB )); then
    log "warning: ${AVAILABLE_MB} MB available but the ceiling is ${MEMORY_BUDGET_MB} MB."
    log "warning: the ceiling is a limit, not a reservation, so the run is fine; it does mean"
    log "warning: the watchdog is the only thing bounding memory on this host."
fi

# Resolve the hard-cap mechanism, and say which one the rows will claim.
#
# `rlimit-as` is the default because it is the sounder of the two: an
# RLIMIT_AS bounds the process's whole address space, and resident memory can
# never exceed address space, so it is a real kernel guarantee that the probe
# stays under the stated RAM budget. It also needs no daemon. A cgroup
# `MemoryMax` bounds RSS more directly, but creating a transient scope unit
# needs a D-Bus connection to the system manager and fails under load or
# outside a session, which is worse than a cap that always works.
HARD_CAP_IMPL="none"
if [[ "${HARD_CAP}" == "auto" ]]; then
    if command -v prlimit >/dev/null 2>&1; then
        HARD_CAP_IMPL="rlimit-as"
    elif command -v systemd-run >/dev/null 2>&1; then
        HARD_CAP_IMPL="cgroup-memory"
        log "prlimit is unavailable; using a cgroup memory cap, which needs a D-Bus session"
    else
        log "no kernel-level memory cap available; the prover's RSS watchdog is the only ceiling"
    fi
else
    HARD_CAP_IMPL="${HARD_CAP}"
fi

# Refuse a worker count the memory budget cannot hold, rather than starting a
# run that cannot finish. The budget is a total shared by every worker, so this
# is a real limit and not a formality: asking a 64-core host for 64 workers put
# the process 4 GiB over on stack reservations alone, the OS refused the thread,
# and the search died taking the process with it.
#
# The probe measures strong scaling with a fixed work ceiling per worker, so
# there is no honest way to keep the requested width by shrinking each worker's
# share: at 64 workers that is a handful of clauses each, which measures nothing.
# The budget itself cannot be raised either — the 12 GiB promise is the point of
# the probe. So the answer on a wide host is a narrower `--workers`.
WORKER_CEILING="$(worker_ceiling)"
(( WORKER_CEILING >= 1 )) || die "the ${MEMORY_BUDGET_MB} MB ceiling leaves no room for a single ${WORKER_HEAP_MB} MB worker"
# Check the widths now, but only the ones the caller chose. A width derived from
# the core count is only provisional until `resolve_core_count` gets the
# prover's own answer, and refusing on a provisional count is how a wrong count
# turns into a wrong refusal. That check happens after the first build, before
# anything is measured.
check_worker_widths() {
    local workers
    IFS=',' read -r -a REQUESTED_WORKERS <<< "${WORKER_LIST}"
    for workers in "${REQUESTED_WORKERS[@]}"; do
        [[ "${workers}" =~ ^[0-9]+$ ]] && (( workers > 0 )) || die "bad worker count: ${workers}"
        if (( workers > WORKER_CEILING )); then
            die "${workers} workers will not fit in the ${MEMORY_BUDGET_MB} MB ceiling (${WORKER_CEILING} is the most this host can run under it); \
rerun with --workers 1,${WORKER_CEILING}"
        fi
    done
}
(( WORKER_LIST_FROM_COUNT )) || check_worker_widths

# The exact command, recorded in the generated report so a reader can rerun it.
# Built after the core count is final, so it names the width that was measured.
record_command() {
    COMMAND="crates/mrs-bench/perf_probe.sh --processed ${PROCESSED} --repeat ${REPEAT} --workers ${WORKER_LIST} --shape ${SHAPE} --clauses ${CLAUSES} --variants ${VARIANTS} --memory-budget-mb ${MEMORY_BUDGET_MB} --hard-cap ${HARD_CAP}"
    [[ -n "${SEED}" ]] && COMMAND="${COMMAND} --seed ${SEED}"
    return 0
}


# ---------------------------------------------------------------------------
# Build provenance
# ---------------------------------------------------------------------------
COMMIT="$(git -C "${WORKSPACE_ROOT}" rev-parse --short HEAD 2>/dev/null || printf 'unknown')"
if [[ -n "$(git -C "${WORKSPACE_ROOT}" status --porcelain 2>/dev/null)" ]]; then
    DIRTY="dirty"
else
    DIRTY="clean"
fi
RUSTC_VERSION="$(rustc --version 2>/dev/null | awk '{print $2}')"
[[ -n "${RUSTC_VERSION}" ]] || RUSTC_VERSION="unknown"
# No `head` in this pipeline: under `set -o pipefail` an early-exiting `head`
# gives `ldd` a SIGPIPE, the pipeline reports that failure, and a `||` fallback
# would append its own answer to the one that was already found.
LDD_OUTPUT="$(ldd --version 2>/dev/null || true)"
GLIBC_VERSION="$(printf '%s\n' "${LDD_OUTPUT}" | sed -n '1s/.*[^0-9.]\([0-9][0-9.]*\)$/\1/p')"
[[ -n "${GLIBC_VERSION}" ]] || GLIBC_VERSION="unknown"

# ---------------------------------------------------------------------------
# Build one target-cpu variant
#
# Each variant gets its own CARGO_TARGET_DIR, because switching RUSTFLAGS in a
# shared directory invalidates every cached artifact and turns the second
# variant's build into a full rebuild of the workspace. Separate stable
# directories mean the first build is slow once and every later run is a
# no-op cargo cache hit.
# ---------------------------------------------------------------------------
declare -A VARIANT_BINARIES=()

build_variant() {
    local variant="$1" target_dir="${WORKSPACE_ROOT}/target/perf/${1}"
    if [[ -n "${MRS_PERF_BINARY:-}" ]]; then
        PROBE_BINARY="${MRS_PERF_BINARY}"
        VARIANT_BINARIES["${variant}"]="${PROBE_BINARY}"
        log "using MRS_PERF_BINARY=${PROBE_BINARY} (no build)"
        return
    fi
    if [[ "${MRS_PERF_SKIP_BUILD:-0}" == "1" ]]; then
        PROBE_BINARY="${target_dir}/release/perf_probe"
        [[ -x "${PROBE_BINARY}" ]] || die "MRS_PERF_SKIP_BUILD=1 but ${PROBE_BINARY} does not exist"
        VARIANT_BINARIES["${variant}"]="${PROBE_BINARY}"
        return
    fi
    log "building target-cpu=${variant} into ${target_dir#"${WORKSPACE_ROOT}"/}"
    # RUSTFLAGS replaces the [build] rustflags in .cargo/config.toml, so it must
    # name the target explicitly for both variants.
    CARGO_TARGET_DIR="${target_dir}" RUSTFLAGS="-C target-cpu=${variant}" \
        cargo build --release -p mrs-bench --bin perf_probe >&2
    PROBE_BINARY="${target_dir}/release/perf_probe"
    VARIANT_BINARIES["${variant}"]="${PROBE_BINARY}"
}

# The binary built for a given target-cpu, kept per variant rather than read from
# one global. A single global is the last variant built, so the verification pass
# — which re-measures the *first* configuration — used to run the other build's
# binary while labelling it with the first one's target-cpu.
variant_binary() {
    printf '%s' "${VARIANT_BINARIES[$1]:-}"
}

# ---------------------------------------------------------------------------
# Run one measurement
# ---------------------------------------------------------------------------
run_measure() {
    local variant="$1" workers="$2" out_file="$3" repeat="${4:-1}" spread="${5:-0}"
    local binary status
    binary="$(variant_binary "${variant}")"
    [[ -n "${binary}" ]] || die "internal error: no binary recorded for target-cpu=${variant}"
    local -a prefix=()
    case "${HARD_CAP_IMPL}" in
        cgroup-memory)
            # A real kernel ceiling: exceeding it is an OOM kill, not a
            # graceful ResourceOut, so the caller must treat a missing row as
            # a failure rather than a measurement.
            prefix=(systemd-run --scope --quiet -p "MemoryMax=${MEMORY_BUDGET_MB}M" -p MemorySwapMax=0 --)
            ;;
        rlimit-as)
            # Bounds the whole address space, so resident memory cannot exceed
            # it. Virtual size runs ahead of resident size, so this is a bound
            # on allocations rather than a measurement of the working set; the
            # working set is what the row's peak RSS column reports. It also
            # charges every thread's stack reservation, which is why the worker
            # count is checked against the budget before any of this runs.
            prefix=(prlimit --as=$((MEMORY_BUDGET_MB * 1024 * 1024)))
            ;;
    esac

    local -a args=(
        "${binary}" measure
        --processed "${PROCESSED}"
        --workers "${workers}"
        --shape "${SHAPE}"
        --clauses "${CLAUSES}"
        --memory-budget-mb "${MEMORY_BUDGET_MB}"
        --binary "${binary}"
        --commit "${COMMIT}"
        --dirty "${DIRTY}"
        --target-cpu "${variant}"
        --rustc "${RUSTC_VERSION}"
        --glibc "${GLIBC_VERSION}"
        --hard-cap "${HARD_CAP_IMPL}"
        --repeat "${repeat}"
        --repeat-spread-pct "${spread}"
    )
    [[ -n "${SEED}" ]] && args+=(--seed "${SEED}")

    status=0
    "${prefix[@]}" "${args[@]}" > "${out_file}" 2>>"${OUT_DIR}/measure.log" || status=$?
    if (( status != 0 )); then
        # Name the signal. A bare "measurement failed" is what a 64-core host
        # got when a worker thread was refused, and the signal is the whole
        # diagnosis: 132 is SIGILL (this build cannot run on this CPU), 134 is
        # SIGABRT (the process aborted, e.g. an allocation failed).
        signal=""
        if (( status > 128 )); then
            signal=" (signal $(( status - 128 )))"
        fi
        die "measurement failed with status ${status}${signal} (target-cpu=${variant}, workers=${workers}); see ${OUT_DIR}/measure.log"
    fi
    [[ -s "${out_file}" ]] \
        || die "measurement produced no row (target-cpu=${variant}, workers=${workers}); a kernel memory cap kills the process without output, so check --hard-cap and --memory-budget-mb"
}

# Measure one configuration REPEAT times and keep the fastest row.
#
# The work is identical in every repeat — that is the whole point of the fixed
# ceiling — so the repeats differ only in how much the machine was disturbed
# while they ran. The fastest run is the closest to the hardware's own speed,
# and the spread between the slowest and fastest is recorded as the row's noise
# floor, so a later comparison knows the difference has to be bigger than that
# to mean anything.
measure_best_of() {
    local variant="$1" workers="$2" keep="$3"
    local -a times=()
    local i run_file best_file best_time

    for ((i = 0; i < REPEAT; i++)); do
        run_file="${OUT_DIR}/run-${variant}-w${workers}-${i}.json"
        run_measure "${variant}" "${workers}" "${run_file}" 1 0
        local elapsed
        elapsed="$(json_number "${run_file}" schedule_ms)"
        [[ -n "${elapsed}" ]] || die "could not read schedule_ms from ${run_file}"
        times+=("${elapsed}")
        if [[ -z "${best_time:-}" ]] || (( elapsed < best_time )); then
            best_time="${elapsed}"
            best_file="${run_file}"
        fi
    done

    local min="${best_time}" max="${best_time}"
    for elapsed in "${times[@]}"; do
        (( elapsed < min )) && min="${elapsed}"
        (( elapsed > max )) && max="${elapsed}"
    done
    local spread
    spread="$(awk -v lo="${min}" -v hi="${max}" 'BEGIN{ if (lo > 0) printf "%.1f", (hi-lo)*100/lo; else printf "0.0" }')"

    # Bank the fastest run, not a fresh one, and stamp it with the repeat count
    # and the spread those repeats actually showed. The two fields are adjacent
    # in the serialized row, so one substitution covers both; if it does not
    # match, the row would claim a noise floor nobody measured, so stop.
    cp "${best_file}" "${keep}"
    sed -i "s/\"repeat\":1,\"repeat_spread_pct\":0\.0/\"repeat\":${REPEAT},\"repeat_spread_pct\":${spread}/" "${keep}"
    grep -q "\"repeat\":${REPEAT},\"repeat_spread_pct\":${spread}" "${keep}" \
        || die "could not stamp the repeat count and spread onto ${keep}; the row layout changed"
    log "  best of ${REPEAT}: ${min} ms (spread ${spread}%, ${#times[@]} run(s))"
}

# ---------------------------------------------------------------------------
# Settle the core count, then measure
# ---------------------------------------------------------------------------

# Replace the bash core count with the prover's own, and re-derive the default
# worker width from it.
#
# These are two implementations of one policy — `physical_cores` in bash and
# `mrs_search::usable_physical_cores` in the binary that writes the row — and
# they have been observed to disagree by 8x on a dual-socket Xeon E5-2407, where
# the bash count said 64 workers on a host with 8 physical cores. Sizing a run
# from the wrong one asks for eight times the memory the budget holds, and the
# result is a search that cannot start.
#
# The binary's count wins, because it is the one that ends up in the bank's
# `physical_cores` column: sizing the workers by it is what keeps that column
# and the worker list describing the same run. A disagreement is still reported,
# because it means the bash count is wrong somewhere and the next host to hit it
# deserves to know.
resolve_core_count() {
    local probe_cores
    probe_cores="$("${PROBE_BINARY}" cores 2>/dev/null || true)"
    if [[ -z "${probe_cores}" ]]; then
        log "warning: could not ask the probe binary for its core count; using this script's count (${CORES})"
        return 0
    fi
    if [[ "${probe_cores}" != "${CORES}" ]]; then
        log "warning: this script's core counter says ${CORES}, the prover's says ${probe_cores}."
        log "warning: the two are separate implementations of the same policy, and a worker list"
        log "warning: sized by the wrong one exceeds the memory ceiling. The prover's count is"
        log "warning: authoritative here because it is what the bank records. This is a bug in"
        log "warning: physical_cores() in this script."
    fi
    CORES="${probe_cores}"
    if (( WORKER_LIST_FROM_COUNT )); then
        if (( CORES == 1 )); then
            WORKER_LIST="1"
        else
            WORKER_LIST="1,${CORES}"
        fi
        check_worker_widths
    fi
}

log "commit: ${COMMIT} (${DIRTY}), rustc ${RUSTC_VERSION}, glibc ${GLIBC_VERSION}"

RESOLVED=0
for variant in "${VARIANT_LIST[@]}"; do
    build_variant "${variant}"
    if (( ! RESOLVED )); then
        RESOLVED=1
        resolve_core_count
        record_command
        log "host: ${CORES} physical cores, ${LOGICAL_CPUS} logical CPUs, ${AVAILABLE_MB} MB available, ceiling ${MEMORY_BUDGET_MB} MB (${HARD_CAP_IMPL})"
        log "memory: ${WORKER_CEILING} workers is the most the ${MEMORY_BUDGET_MB} MB ceiling holds here"
        if (( LOGICAL_CPUS > 0 && CORES >= LOGICAL_CPUS )); then
            log "note: the physical count equals the logical count, so no SMT sibling was counted."
            log "note: that is right for a host without SMT, and it is also exactly what an unreadable"
            log "note: /sys/devices/system/cpu/*/topology looks like. Nothing here can tell those apart."
        fi
        log "workload: shape=${SHAPE} clauses=${CLAUSES} processed=${PROCESSED} workers=${WORKER_LIST}"
    fi
    IFS=',' read -r -a WORKERS_LIST <<< "${WORKER_LIST}"
    for workers in "${WORKERS_LIST[@]}"; do
        log "measuring target-cpu=${variant} workers=${workers} (best of ${REPEAT})"
        row="${OUT_DIR}/row-${variant}-w${workers}.json"
        measure_best_of "${variant}" "${workers}" "${row}"
        cat "${row}" >> "${ROWS}"
    done
done

[[ -s "${ROWS}" ]] || die "no measurement was taken"

# ---------------------------------------------------------------------------
# Confirm the work really was identical before anything is banked
#
# The whole comparison rests on every run doing the same work. Repeat the
# first configuration in a fresh process and compare the work fingerprint. A
# mismatch means some wall-clock-dependent heuristic leaked into the search,
# and the timings must not be banked.
# ---------------------------------------------------------------------------
if (( DO_VERIFY )); then
    first_variant="${VARIANT_LIST[0]}"
    first_workers="${WORKERS_LIST[0]}"
    reference="${OUT_DIR}/row-${first_variant}-w${first_workers}.json"
    repeat="${OUT_DIR}/row-verify.json"
    [[ -s "${reference}" ]] \
        || die "internal error: ${reference} is missing, so there is nothing to verify the repeat run against"
    log "verifying that the fixed work repeats in a fresh process"
    run_measure "${first_variant}" "${first_workers}" "${repeat}" 1 0
    a="$(json_field "${reference}" work_sha)"
    b="$(json_field "${repeat}" work_sha)"
    if [[ -z "${a}" || "${a}" != "${b}" ]]; then
        die "the repeat run did the same work as the first (work_sha ${a} vs ${b}); refusing to bank a timing whose work is not reproducible"
    fi
    log "work fingerprint ${a} reproduced in a second process"
    rm -f "${repeat}"
else
    log "skipping the repeat verification (--no-verify)"
fi

# ---------------------------------------------------------------------------
# Bank
# ---------------------------------------------------------------------------
if (( DO_BANK )); then
    BANK_DIR="${WORKSPACE_ROOT}/docs/results/perf"
    BANK_TSV="${BANK_DIR}/bank.tsv"
    HOST_SLUG="$(json_field "${ROWS}" host_slug)"
    [[ -n "${HOST_SLUG}" ]] || die "could not read host_slug from ${ROWS}"
    [[ -n "${COMMAND:-}" ]] || die "internal error: the rerun command was never recorded"
    REPORT="${BANK_DIR}/$(date +%F)-${HOST_SLUG}.md"

    "${PROBE_BINARY}" bank \
        --rows "${ROWS}" \
        --tsv "${BANK_TSV}" \
        --md "${REPORT}" \
        --command "${COMMAND}"
    log "banked in ${BANK_TSV#"${WORKSPACE_ROOT}"/} and ${REPORT#"${WORKSPACE_ROOT}"/}"
    log "raw rows kept in ${OUT_DIR#"${WORKSPACE_ROOT}"/}"
else
    log "not banking (--no-bank); raw rows are in ${ROWS}"
fi
