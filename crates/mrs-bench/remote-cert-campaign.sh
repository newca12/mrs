#!/usr/bin/env bash
# crates/mrs-bench/remote-cert-campaign.sh
#
# Remote validation campaign for ordered-inference certification (R0-R6).
# Designed for a beefy remote box (16+ cores, 100+ GB RAM); runs anywhere
# with bash, GNU coreutils/time, and a cargo-built release binary.
#
# Usage:
#   ./crates/mrs-bench/remote-cert-campaign.sh <phase> [phase...]
#   Phases: r0 r1 r2 r3 r4 r5 r6 all
#
#   r0   Calibration: vendored casc-30 EPS+EPU, 10 s, both orderings.
#        Must reproduce the local gate (zero ko; EPS coverage in band).
#   r1   Full-division soundness sweep: ALL TPTP EPS+EPU, 10 s, both
#        orderings. Primary metric: false-positive count (must be 0).
#   r2   Budget scaling: fail-closed subsets at 60/300/600 s for
#        coverage-vs-budget curves per tier.
#   r3   Emission validation: EPU Tier-2 window at deep budget with
#        TRACE_CERTIFY=1 and MRS_SELF_CHECK=1; kernel-accept rate, proof
#        sizes, RAT-witness incidence from the trace lines.
#   r4   Tier-3 small-core study: full EPU fail-closed set at deep budget;
#        conversion count (currently 0/200 on casc-30).
#   r5   Price of certification: default portfolio vs certified coverage
#        on the same corpus (uses the standard mrs system, CASC times).
#   r6   Repeatability: 3x stratified sample; verdict stability + wobble.
#
# Required env:
#   TPTP_ROOT   full TPTP checkout root (contains Problems/ and Axioms/).
#               Skipped for r0 (uses the vendored casc-30 corpus).
# Optional env (defaults tuned for 16 cores / 192 GB):
#   JOBS (16), OUT_ROOT (crates/mrs-bench/results/remote-cert),
#   R2_SAMPLE (200), R2_TIMES ("60 300 600"), R2_DEEP (300),
#   R6_REPEATS (3), R6_SAMPLE (100), MRS_BIN (target/release/mrs).
#
# Toolchain: plain cargo/rustup (no nix on remote). Pin rustc to the same
# version the local gate used (1.98.1; `rustup toolchain install 1.98.1`)
# and record `rustc --version` + git rev in every phase output. A version
# mismatch warns but does not stop the campaign; note it in the report.
# Reference answers for full divisions are generated from TPTP % Status
# headers into systems/reference/answers_tptp-full.tsv (fetch_answers.sh
# equivalent for offline checkouts); problems without a header get their
# verdict from the division label and are flagged in the summary.
#
# Every phase writes $OUT_ROOT/<phase>/ (casc.sh output dirs or run.csv
# files inside) plus PHASE_SUMMARY.md. Any `ko` verdict fails the phase
# (exit nonzero): soundness findings stop the campaign.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

JOBS="${JOBS:-16}"
OUT_ROOT="${OUT_ROOT:-${SCRIPT_DIR}/results/remote-cert}"
R2_SAMPLE="${R2_SAMPLE:-200}"
R2_TIMES="${R2_TIMES:-60 300 600}"
R2_DEEP="${R2_DEEP:-300}"
R6_REPEATS="${R6_REPEATS:-3}"
R6_SAMPLE="${R6_SAMPLE:-100}"
MRS_BIN="${MRS_BIN:-${WORKSPACE_ROOT}/target/release/mrs}"
EXPECTED_RUSTC="${EXPECTED_RUSTC:-1.98.1}"

log() { echo "[remote-cert] $*" >&2; }
fail() { echo "[remote-cert] PHASE FAILED: $*" >&2; exit 1; }

need_cmd() {
    command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
}

# ---------- preflight ----------
preflight() {
    need_cmd cargo
    need_cmd timeout
    if [[ ! -x "${MRS_BIN}" ]]; then
        log "building release binary (plain cargo, remote has no nix)..."
        (cd "${WORKSPACE_ROOT}" && cargo build --release --bin mrs) || fail "release build failed"
    fi
    local rustc_ver
    rustc_ver="$(rustc --version 2>/dev/null || echo unknown)"
    log "toolchain: ${rustc_ver} (local gate used ${EXPECTED_RUSTC})"
    if [[ "${rustc_ver}" != *"${EXPECTED_RUSTC}"* ]]; then
        log "WARNING: toolchain differs from the local gate; record this in the report."
    fi
    log "git: $(git -C "${WORKSPACE_ROOT}" rev-parse --short HEAD 2>/dev/null || echo unknown)"
    mkdir -p "${OUT_ROOT}"
}

# ---------- reference answers from TPTP headers ----------
gen_answers() {
    # $1 = edition name for answers_tptp file; scans $TPTP_ROOT/Problems/{EPS,EPU}
    local edition="$1"
    local out="${SCRIPT_DIR}/systems/reference/answers_${edition}.tsv"
    if [[ -f "${out}" ]]; then
        log "answers file exists: ${out}"
        return 0
    fi
    [[ -n "${TPTP_ROOT:-}" && -d "${TPTP_ROOT}/Problems" ]] \
        || fail "TPTP_ROOT must point at a full TPTP checkout (Problems/ + Axioms/)"
    log "generating reference answers from TPTP headers (this scans the corpus once)..."
    : > "${out}.tmp"
    local missing=0 total=0
    for div in EPS EPU; do
        for problem in "${TPTP_ROOT}/Problems/${div}"/*.p; do
            [[ -e "${problem}" ]] || continue
            total=$((total + 1))
            local base status
            base="$(basename "${problem}" .p)"
            status="$(grep -m1 -oP '^%\s*Status\s*:\s*\K[A-Za-z]+' "${problem}" || true)"
            if [[ -z "${status}" ]]; then
                # Fall back to the division label (EPS satisfiable, EPU unsatisfiable).
                if [[ "${div}" == "EPS" ]]; then status="Satisfiable"; else status="Unsatisfiable"; fi
                missing=$((missing + 1))
            fi
            printf '%s\t%s\n' "${base}" "${status}" >> "${out}.tmp"
        done
    done
    mv "${out}.tmp" "${out}"
    log "answers: ${out} (${total} problems, ${missing} from division labels)"
}

# ---------- shared grading ----------
# phase_gate <run.csv>: fail the phase on any ko verdict; print coverage table.
phase_gate() {
    local csv="$1"
    local ko
    ko="$(awk -F, 'NR>1 && $8=="ko"' "${csv}" | head -20)"
    if [[ -n "${ko}" ]]; then
        echo "${ko}" >&2
        fail "false positives in ${csv} (see rows above)"
    fi
    log "gate: zero ko in ${csv}"
    awk -F, 'NR>1 {status[$6]++} END {for (s in status) print "  status " s ": " status[s]}' "${csv}" >&2
    awk -F, 'NR>1 {n=split($11,a," "); for(i=1;i<=n;i++) if(a[i]~/^cert_tier=/){tier[a[i]]++}} END {for (t in tier) print "  " t ": " tier[t]}' "${csv}" >&2 || true
}

write_summary() {
    # $1 = phase dir, $2 = extra notes
    local dir="$1" notes="${2:-}"
    {
        echo "# Remote certification campaign: $dir"
        echo
        echo "- date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
        echo "- git: $(git -C "${WORKSPACE_ROOT}" rev-parse --short HEAD 2>/dev/null || echo unknown)"
        echo "- rustc: $(rustc --version 2>/dev/null || echo unknown)"
        echo "- binary: $("${MRS_BIN}" --version 2>/dev/null || echo "${MRS_BIN}")"
        echo "- notes: ${notes}"
    } > "${dir}/PHASE_SUMMARY.md"
    log "summary: ${dir}/PHASE_SUMMARY.md"
}

# ---------- R0: calibration on vendored corpus ----------
phase_r0() {
    local out="${OUT_ROOT}/r0-calibration"
    log "R0: vendored casc-30 EPS+EPU, both orderings, 10 s..."
    for strat in 1 7; do
        MRS_CERTIFY_STRATEGY="${strat}" "${SCRIPT_DIR}/casc.sh" \
            --systems mrs-certify --divisions eps,epu --time 10 \
            --jobs "${JOBS}" --output "${out}/s${strat}"
    done
    for strat in 1 7; do
        phase_gate "${out}/s${strat}/run.csv"
    done
    write_summary "${out}" "Must reproduce the local gate (EPS ~5-10 certified, EPU 0, zero ko)."
}

# ---------- R1: full-division soundness sweep ----------
phase_r1() {
    [[ -n "${TPTP_ROOT:-}" ]] || fail "R1 needs TPTP_ROOT (full TPTP checkout root)"
    gen_answers tptp-full
    export CASC_ANSWERS_FILE="${SCRIPT_DIR}/systems/reference/answers_tptp-full.tsv"
    local out="${OUT_ROOT}/r1-full-sweep"
    export CASC_PROBLEMS_ROOT="${TPTP_ROOT}"
    for strat in 1 7; do
        MRS_CERTIFY_STRATEGY="${strat}" "${SCRIPT_DIR}/casc.sh" \
            --edition tptp-full --systems mrs-certify --divisions eps,epu \
            --time 10 --jobs "${JOBS}" --output "${out}/s${strat}"
    done
    unset CASC_PROBLEMS_ROOT CASC_ANSWERS_FILE
    for strat in 1 7; do
        phase_gate "${out}/s${strat}/run.csv"
    done
    write_summary "${out}" "Full-division soundness sweep at 10 s; primary metric is zero ko."
}

# ---------- subset helper: random basenames from a run.csv ----------
# subset_problems <run.csv> <division> <count> <seed> -> stdout basenames
subset_problems() {
    awk -F, -v div="$2" 'NR>1 && $2==div && ($6=="GaveUp" || $6=="Timeout") {print $3}' "$1" \
        | shuf --random-source=<(yes "$4") -n "$3" 2>/dev/null \
        || awk -F, -v div="$2" 'NR>1 && $2==div && ($6=="GaveUp" || $6=="Timeout") {print $3}' "$1" | head -n "$3"
}

# ---------- R2: budget scaling on fail-closed subsets ----------
phase_r2() {
    [[ -n "${TPTP_ROOT:-}" ]] || fail "R2 needs TPTP_ROOT"
    gen_answers tptp-full
    local out="${OUT_ROOT}/r2-budget-scaling"
    mkdir -p "${out}"
    # NOTE: casc.sh has no subset mode; subsets run through a staged
    # edition directory (symlinks keep disk use at zero). TPTP stays
    # pointed at the full checkout so %include resolves; answers come
    # from the generated full-division file.
    local old_tptp="${TPTP:-}"
    export TPTP="${TPTP_ROOT}"
    export CASC_ANSWERS_FILE="${SCRIPT_DIR}/systems/reference/answers_tptp-full.tsv"
    for div_lower in eps epu; do
        local div_upper="${div_lower^^}"
        local src="${OUT_ROOT}/r1-full-sweep/s1/run.csv"
        [[ -f "${src}" ]] || fail "R2 needs R1 output at ${src} (run r1 first)"
        local staged="${OUT_ROOT}/r2-stage-${div_lower}"
        mkdir -p "${staged}/${div_upper}"
        subset_problems "${src}" "${div_lower}" "${R2_SAMPLE}" "42" | while read -r base; do
            [[ -n "${base}" ]] && ln -sf "${TPTP_ROOT}/Problems/${div_upper}/${base}.p" "${staged}/${div_upper}/${base}.p"
        done
        for t in ${R2_TIMES}; do
            for strat in 1 7; do
                MRS_CERTIFY_STRATEGY="${strat}" CASC_PROBLEMS_ROOT="${staged}" \
                    "${SCRIPT_DIR}/casc.sh" \
                    --edition "r2-${div_lower}" --systems mrs-certify \
                    --divisions "${div_lower}" --time "${t}" \
                    --jobs "${JOBS}" --output "${out}/${div_lower}-t${t}-s${strat}"
            done
        done
    done
    unset CASC_PROBLEMS_ROOT CASC_ANSWERS_FILE
    if [[ -n "${old_tptp}" ]]; then export TPTP="${old_tptp}"; else unset TPTP; fi
    write_summary "${out}" "Coverage-vs-budget curves; compare certified counts across ${R2_TIMES}."
}

# ---------- R3: emission validation (deep EPU + TRACE + self-check) ----------
phase_r3() {
    [[ -n "${TPTP_ROOT:-}" ]] || fail "R3 needs TPTP_ROOT"
    gen_answers tptp-full
    local out="${OUT_ROOT}/r3-emission"
    export CASC_PROBLEMS_ROOT="${TPTP_ROOT}"
    export CASC_ANSWERS_FILE="${SCRIPT_DIR}/systems/reference/answers_tptp-full.tsv"
    # Self-check kernel-verifies every emitted proof in-process: an
    # Unsatisfiable status here means kernel-accepted, GaveUp means
    # rejected-or-unfinished. TRACE lines give proof sizes + RAT data.
    for strat in 1 7; do
        MRS_CERTIFY_STRATEGY="${strat}" MRS_SELF_CHECK=1 TRACE_CERTIFY=1 \
            "${SCRIPT_DIR}/casc.sh" \
            --edition tptp-full --systems mrs-certify --divisions epu \
            --time "${R2_DEEP}" --jobs "${JOBS}" --output "${out}/s${strat}"
    done
    unset CASC_PROBLEMS_ROOT CASC_ANSWERS_FILE
    for strat in 1 7; do
        phase_gate "${out}/s${strat}/run.csv"
    done
    log "RAT/size telemetry (from raw stderr of Tier-2 runs):"
    grep -rh -oP 'sat_trace_capture \K.*' "${out}"/*/raw/*/*.stderr 2>/dev/null \
        | sort | uniq -c | sort -rn | head -20 >&2 || true
    write_summary "${out}" "Emission validation: kernel-accept rate, proof sizes, RAT incidence."
}

# ---------- R4: Tier-3 small-core study (full EPU fail-closed set, deep) ----------
phase_r4() {
    [[ -n "${TPTP_ROOT:-}" ]] || fail "R4 needs TPTP_ROOT"
    local out="${OUT_ROOT}/r4-tier3"
    export CASC_PROBLEMS_ROOT="${TPTP_ROOT}"
    # Exhaustive (not sampled): every EPU problem that fail-closed in R1.
    # Staged edition keeps casc.sh unmodified.
    local old_tptp="${TPTP:-}"
    export TPTP="${TPTP_ROOT}"
    gen_answers tptp-full
    export CASC_ANSWERS_FILE="${SCRIPT_DIR}/systems/reference/answers_tptp-full.tsv"
    local staged="${OUT_ROOT}/r4-stage-epu"
    mkdir -p "${staged}/EPU"
    awk -F, 'NR>1 && $2=="epu" && ($6=="GaveUp" || $6=="Timeout") {print $3}' \
        "${OUT_ROOT}/r1-full-sweep/s1/run.csv" | while read -r base; do
        [[ -n "${base}" ]] && ln -sf "${TPTP_ROOT}/Problems/EPU/${base}.p" "${staged}/EPU/${base}.p"
    done
    for strat in 1 7; do
        MRS_CERTIFY_STRATEGY="${strat}" CASC_PROBLEMS_ROOT="${staged}" \
            "${SCRIPT_DIR}/casc.sh" \
            --edition "r4-epu" --systems mrs-certify \
            --divisions epu --time "${R2_DEEP}" \
            --jobs "${JOBS}" --output "${out}/s${strat}"
    done
    unset CASC_PROBLEMS_ROOT CASC_ANSWERS_FILE
    if [[ -n "${old_tptp}" ]]; then export TPTP="${old_tptp}"; else unset TPTP; fi
    for strat in 1 7; do
        phase_gate "${out}/s${strat}/run.csv"
    done
    write_summary "${out}" "Tier-3 conversion count on the full EPU fail-closed set."
}

# ---------- R5: price of certification (portfolio baseline) ----------
phase_r5() {
    [[ -n "${TPTP_ROOT:-}" ]] || fail "R5 needs TPTP_ROOT"
    gen_answers tptp-full
    local out="${OUT_ROOT}/r5-baseline"
    export CASC_PROBLEMS_ROOT="${TPTP_ROOT}"
    export CASC_ANSWERS_FILE="${SCRIPT_DIR}/systems/reference/answers_tptp-full.tsv"
    # Default portfolio at CASC per-division times; 8 workers per problem
    # (CASC hardware), so scale jobs to core count accordingly.
    MRS_WORKERS=8 "${SCRIPT_DIR}/casc.sh" \
        --edition tptp-full --systems mrs --divisions eps,epu \
        --casc-times --jobs 2 --output "${out}/portfolio"
    unset CASC_PROBLEMS_ROOT CASC_ANSWERS_FILE
    write_summary "${out}" "Compare portfolio coverage vs certified coverage (R1) on the same corpus."
}

# ---------- R6: repeatability (3x stratified sample) ----------
phase_r6() {
    [[ -n "${TPTP_ROOT:-}" ]] || fail "R6 needs TPTP_ROOT"
    local out="${OUT_ROOT}/r6-repeats"
    export CASC_PROBLEMS_ROOT="${TPTP_ROOT}"
    local old_tptp="${TPTP:-}"
    export TPTP="${TPTP_ROOT}"
    gen_answers tptp-full
    export CASC_ANSWERS_FILE="${SCRIPT_DIR}/systems/reference/answers_tptp-full.tsv"
    local staged="${OUT_ROOT}/r6-stage"
    mkdir -p "${staged}/EPS" "${staged}/EPU"
    for div in EPS EPU; do
        div_lower="${div,,}"
        awk -F, -v div="${div_lower}" 'NR>1 && $2==div {print $3}' \
            "${OUT_ROOT}/r1-full-sweep/s1/run.csv" \
            | shuf -n "${R6_SAMPLE}" 2>/dev/null \
            | while read -r base; do
                [[ -n "${base}" ]] && ln -sf "${TPTP_ROOT}/Problems/${div}/${base}.p" "${staged}/${div}/${base}.p"
            done
    done
    local rep
    for rep in $(seq 1 "${R6_REPEATS}"); do
        MRS_CERTIFY_STRATEGY=1 CASC_PROBLEMS_ROOT="${staged}" \
            "${SCRIPT_DIR}/casc.sh" \
            --edition "r6-sample" --systems mrs-certify \
            --divisions eps,epu --time 10 \
            --jobs "${JOBS}" --output "${out}/rep${rep}"
    done
    unset CASC_PROBLEMS_ROOT CASC_ANSWERS_FILE
    if [[ -n "${old_tptp}" ]]; then export TPTP="${old_tptp}"; else unset TPTP; fi
    write_summary "${out}" "Verdict stability across ${R6_REPEATS} repeats; certified-set overlap."
}

# ---------- main ----------
main() {
    [[ $# -ge 1 ]] || { echo "Usage: $0 <r0|r1|r2|r3|r4|r5|r6|all> [...]" >&2; exit 2; }
    preflight
    for phase in "$@"; do
        case "${phase}" in
            r0) phase_r0 ;;
            r1) phase_r1 ;;
            r2) phase_r2 ;;
            r3) phase_r3 ;;
            r4) phase_r4 ;;
            r5) phase_r5 ;;
            r6) phase_r6 ;;
            all) phase_r0; phase_r1; phase_r2; phase_r3; phase_r4; phase_r5; phase_r6 ;;
            *) echo "Unknown phase: ${phase}" >&2; exit 2 ;;
        esac
    done
    log "campaign complete: ${OUT_ROOT}"
}

main "$@"
