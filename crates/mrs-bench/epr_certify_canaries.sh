#!/usr/bin/env bash
# EPR certification canaries: zero false-positive gate for --certify-ordered.
#
# Ground truth comes from CASC division labels, not from the prover itself:
#   EPS/ = EPR Satisfiable   -> a Refutation (Theorem/Unsatisfiable) is a FALSE POSITIVE
#   EPU/ = EPR Unsatisfiable -> a Saturation (Satisfiable/CounterSatisfiable) is a FALSE POSITIVE
# GaveUp / Timeout / ResourceOut / Error / NoStatus are allowed (fail-closed).
#
# Every problem runs under both certified orderings:
#   --strategy 1  (KBO) and  --strategy 7  (LPO)
# with --workers 1 --certify-ordered. Exits nonzero iff any false positive
# is observed.
#
# Usage:
#   ./crates/mrs-bench/epr_certify_canaries.sh [--corpus DIR] [--time SECS] [--jobs N]
#
# Env overrides: MRS_BIN (prover binary, default ./target/release/mrs),
#   MRS_CANARY_TIME, MRS_CANARY_JOBS.
set -u

CORPUS="crates/mrs-bench/problems/casc-30"
TIME_SECS="${MRS_CANARY_TIME:-10}"
JOBS="${MRS_CANARY_JOBS:-4}"
BIN="${MRS_BIN:-./target/release/mrs}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --corpus) CORPUS="$2"; shift 2 ;;
        --time) TIME_SECS="$2"; shift 2 ;;
        --jobs) JOBS="$2"; shift 2 ;;
        *) echo "Unknown argument: $1" >&2; exit 2 ;;
    esac
done

cd "$(dirname "$0")/../.." || exit 2

if [[ ! -x "$BIN" ]]; then
    echo "% Building release binary for canary run..." >&2
    cargo build --release --bin mrs >&2 || exit 2
fi

# Includes in the corpus resolve against the corpus root.
export TPTP="$CORPUS"

RESULTS_DIR="$(mktemp -d)"
trap 'rm -rf "$RESULTS_DIR"' EXIT

# Phase 1: run the prover over the whole corpus with a bounded job pool.
# Each job writes one .out file; classification happens afterwards.
running=0
for division in EPS EPU; do
    for strategy in 1 7; do
        while IFS= read -r problem; do
            base="$(basename "$problem" .p)"
            out="$RESULTS_DIR/${division}-s${strategy}-${base}.out"
            timeout "$((TIME_SECS + 60))" "$BIN" \
                --time "$TIME_SECS" --workers 1 --strategy "$strategy" \
                --certify-ordered "$problem" >"$out" 2>/dev/null &
            running=$((running + 1))
            if [[ "$running" -ge "$JOBS" ]]; then
                wait -n
                running=$((running - 1))
            fi
        done < <(find "$CORPUS/$division" -name '*.p' | sort)
    done
done
wait

# Phase 2: classify. Only a status contradicting the division label counts.
overall_fp=0
for division in EPS EPU; do
    for strategy in 1 7; do
        total=0; certified=0; fail_closed=0; fps=0; fp_list=""
        for out in "$RESULTS_DIR/${division}-s${strategy}-"*.out; do
            [[ -e "$out" ]] || continue
            total=$((total + 1))
            status="$(sed -n 's/^% SZS status \([A-Za-z]*\).*/\1/p' "$out" | head -1)"
            [[ -z "$status" ]] && status="NoStatus"
            case "$division:$status" in
                EPS:Theorem|EPS:Unsatisfiable|EPU:Satisfiable|EPU:CounterSatisfiable)
                    fps=$((fps + 1))
                    fp_list="$fp_list $(basename "$out" .out)($status)"
                    ;;
                EPS:Satisfiable|EPS:CounterSatisfiable|EPU:Theorem|EPU:Unsatisfiable)
                    certified=$((certified + 1))
                    ;;
                *)
                    fail_closed=$((fail_closed + 1))
                    ;;
            esac
        done
        echo "division=$division strategy=$strategy total=$total certified=$certified fail_closed=$fail_closed false_positives=$fps"
        if [[ -n "$fp_list" ]]; then
            echo "  FALSE POSITIVES:$fp_list"
        fi
        overall_fp=$((overall_fp + fps))
    done
done

echo "false_positives_total=$overall_fp"
if [[ "$overall_fp" -gt 0 ]]; then
    echo "CANARY GATE FAILED: $overall_fp false positive status(es)" >&2
    exit 1
fi
echo "CANARY GATE PASSED: zero false positives"
