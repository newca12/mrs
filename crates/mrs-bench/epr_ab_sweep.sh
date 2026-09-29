#!/usr/bin/env bash
# A/B sweep for the CASC-30 EPU division.
#
# Defaults to the **whole division**, run through `casc.sh` at the competition
# shape, and graded by `audit_casc_proofs`. That is deliberate:
#
#   - The denominator has to be the division. A sweep over a subset reports a
#     number that cannot be compared to the division's score, and an earlier cut
#     of this script defaulted to a 14-problem subset whose rationale ("18
#     axiom-only files are unscorable, so the real ceiling is 82") was wrong.
#     The answer key grades all 18 of them `Unsatisfiable`, so all 18 are
#     scoreable and the ceiling is not 82. See
#     `docs/reports/benchmarks/epu-2026-09.md`.
#   - The number has to carry its proof column. A `Satisfiable`-shaped verdict on
#     a refutation division is worth nothing without a proof the kernel accepts,
#     and the certificate gate is the only thing that says so. This script runs
#     it and prints the `VerifiedGood` column beside the solved count, because
#     **an EPU number quoted without it is not a number**.
#   - The run has to be CASC-shaped to mean anything. The division is 100
#     problems at 120 s on 8 physical cores and 128 GB, so the default is
#     `--hardware casc` and `--casc-times`. On a host that cannot represent that,
#     the run says so on its `% Hardware:` line and the number is not a CASC
#     number — see "Running this where the shape is not available" below.
#
# Usage:
#   epr_ab_sweep.sh <label>                  # whole division, pre-pass off
#   MRS_EPR_GROUND=1 epr_ab_sweep.sh <label> # whole division, pre-pass on
#   epr_ab_sweep.sh --subset <label>         # the stratified subset, for a quick look
#   epr_ab_sweep.sh --list-subset            # print the subset
#
# Environment:
#   MRS_EPR_GROUND=1      enable the EPR grounding pre-pass (this is the A arm)
#   MRS_HARDWARE          passed through; defaults to `casc`
#   EPU_JOBS              casc.sh --jobs, default 1 (one problem at a time: the
#                         honest shape when each problem already wants all 8
#                         workers)
#   EPU_TIME              per-problem seconds; unset means --casc-times (120 s)
#   EPU_SUBSET_FILE       replace the stratified subset
#   EPU_SWEEP_ROOT        where run directories are written
set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CASC="$REPO/crates/mrs-bench/casc.sh"
AUDIT="$REPO/target/release/audit_casc_proofs"
PROBLEMS_DIR="${EPU_PROBLEMS_DIR:-$REPO/crates/mrs-bench/problems/casc-30}"
EDITION="$(basename "$PROBLEMS_DIR")"
DIVISION="epu"
OUT_ROOT="${EPU_SWEEP_ROOT:-$REPO/crates/mrs-bench/results}"
JOBS="${EPU_JOBS:-1}"
HARDWARE="${MRS_HARDWARE:-casc}"
PRE_PASS="off"
[[ "${MRS_EPR_GROUND:-}" == "1" ]] && PRE_PASS="on"

# Stratified: one problem per EPR profile and per observed outcome, plus the
# axiom-only counterexample, so a subset moves when a change helps one profile
# and hurts another instead of averaging out. This is a *diagnostic* mode, not
# the measurement: a subset's solved count is not a division number.
SUBSET_DEFAULT=(
  HWV039-1 HWV089-1 HWV111-1 HWV041-1 HWV047-1       # epr_equality
  MSC015-1.022 PLA031-1.007 SWV418-1.300              # pure relational
  SWV421-1.500 SWV422-1.505 SWV423-1.010
  SWV418-1.900 SWV421-1.400 SWV422-1.465 SWV419-1.035
  HWV065-1 SYO591-1 MSC024-1                          # axiom-only, graded Unsatisfiable
)

MODE="full"
case "${1:-}" in
  --list-subset)
    printf '%s\n' "${SUBSET_DEFAULT[@]}"
    exit 0
    ;;
  --subset)
    MODE="subset"
    shift
    ;;
esac

LABEL="${1:?usage: epr_ab_sweep.sh [--subset] <label>}"
shift || true
OUT="$OUT_ROOT/epu-sweep-$LABEL"
mkdir -p "$OUT"

SUBSET_ARGS=()
if [[ "$MODE" == "subset" ]]; then
  SUBSET_FILE="${EPU_SUBSET_FILE:-$OUT/subset.txt}"
  printf '%s\n' "${SUBSET_DEFAULT[@]}" > "$SUBSET_FILE"
  SUBSET_ARGS=(--subset "$SUBSET_FILE" --subset-list-out "$OUT/subset-resolved.txt")
fi

# `--casc-times` gives the division's official 120 s. `EPU_TIME` is the escape
# hatch for a quick look, and is recorded in the run metadata so a number from it
# can never be mistaken for one at the competition limit.
TIME_ARGS=(--casc-times)
if [[ -n "${EPU_TIME:-}" ]]; then
  TIME_ARGS=(--time "$EPU_TIME")
fi

# run.csv records the git commit, the division, the per-problem time limit and
# `use_casc_times`, so the conditions travel with the numbers.
{
  printf '# epr_ab_sweep label=%s pre_pass=%s hardware=%s jobs=%s mode=%s time_args=%s\n' \
    "$LABEL" "$PRE_PASS" "$HARDWARE" "$JOBS" "$MODE" "${TIME_ARGS[*]}"
  printf '# problems_dir=%s division=%s problems=%s\n' \
    "$PROBLEMS_DIR" "$DIVISION" "$([ "$MODE" = full ] && echo 100 || echo "${#SUBSET_DEFAULT[@]}")"
  printf '# MRS_EPR_GROUND=%s MRS_HARDWARE=%s\n' "${MRS_EPR_GROUND:-0}" "$HARDWARE"
} > "$OUT/conditions.txt"

echo "[epr-sweep] label=$LABEL pre_pass=$PRE_PASS mode=$MODE hardware=$HARDWARE jobs=$JOBS"
echo "[epr-sweep] conditions: $OUT/conditions.txt"

MRS_HARDWARE="$HARDWARE" "$CASC" \
  --edition "$EDITION" --systems mrs --divisions "$DIVISION" \
  --jobs "$JOBS" --output "$OUT/run" \
  "${TIME_ARGS[@]}" "${SUBSET_ARGS[@]}"
CASC_RC=$?
if [[ $CASC_RC -ne 0 ]]; then
  echo "[epr-sweep] casc.sh failed with $CASC_RC — no number to report" >&2
  exit "$CASC_RC"
fi

RUN_CSV="$OUT/run/run.csv"
if [[ ! -f "$RUN_CSV" ]]; then
  echo "[epr-sweep] no run.csv at $RUN_CSV" >&2
  exit 1
fi

# The proof column. Skipped with a loud warning rather than silently, because a
# solved count without it is the failure mode this script exists to prevent.
VERIFIED="n/a"
if [[ -x "$AUDIT" ]]; then
  "$AUDIT" --run "$OUT/run" --problems-dir "$PROBLEMS_DIR" --checks strict \
    --output "$OUT/certification" >/dev/null 2>&1 \
    && VERIFIED="$(python3 -c '
import csv, sys
good = total = 0
for r in csv.DictReader(open(sys.argv[1])):
    if r["generation_status"] in ("Unsatisfiable", "Theorem"):
        total += 1
        if r["strict_status"] == "VerifiedGood":
            good += 1
print(f"{good}/{total}")' "$OUT/certification/audit.csv")" \
    || VERIFIED="audit-failed"
else
  echo "[epr-sweep] WARNING: audit_casc_proofs not built, the proof column is missing." >&2
  echo "[epr-sweep]   cargo build --release -p mrs-bench --bin audit_casc_proofs" >&2
fi

# EPR pre-pass telemetry, joined from the archived stderr that casc.sh kept.
# This is the diagnostic: it says whether the pre-pass searched, which is a
# different question from whether it won.
python3 - "$RUN_CSV" "$OUT" <<'PY' 2>/dev/null || true
# Joins the EPR pre-pass telemetry out of the archived stderr. "Did the pre-pass
# search" is a different question from "did it win", and only this answers the
# first: a run where epr_generated is 0 says the ladder never ran, which is a
# different defect from one where it ran and lost.
import csv, os, re, sys
run_csv, out = sys.argv[1], sys.argv[2]
keys = ("epr_attempted","epr_route","epr_generated","epr_rounds","epr_falsifying",
        "epr_full_grounding","epr_clauses","epr_vars","epr_ms","epr_result","epr_fallback")
rows = list(csv.DictReader(open(run_csv)))
arch = os.path.join(out, "run", "raw", "mrs", "epu")
with open(os.path.join(out, "epr_telemetry.csv"), "w") as fh:
    fh.write("problem,szs_status," + ",".join(keys) + "\n")
    for r in rows:
        p = os.path.join(arch, r["problem"] + ".stderr")
        tele = {}
        try:
            for tok in open(p, errors="replace").read().split():
                m = re.match(r"^(epr_\w+)=(.*)$", tok)
                if m: tele[m.group(1)] = m.group(2)
        except OSError:
            pass
        fh.write(",".join([r["problem"], r["szs_status"]] + [tele.get(k, "") for k in keys]) + "\n")
PY

TOTAL=$(awk -F, 'NR>1 && $0 !~ /^#/ {n++} END {print n+0}' "$RUN_CSV")
SOLVED=$(awk -F, 'NR>1 && $6=="Unsatisfiable" {n++} END {print n+0}' "$RUN_CSV")
KO=$(awk -F, 'NR>1 && $8=="ko" {n++} END {print n+0}' "$RUN_CSV")
MAXRSS=$(awk -F, 'NR>1 && $10+0>m {m=$10+0} END {print m+0}' "$RUN_CSV")
MS=$(awk -F, 'NR>1 && $6=="Unsatisfiable" {t+=$9+0; n++} END {if (n) printf "%.1f", t/n; else print "-"}' "$RUN_CSV")

echo
echo "[epr-sweep] ===== $LABEL (pre_pass=$PRE_PASS) ====="
printf '[epr-sweep] solved       %d/%d Unsatisfiable\n' "$SOLVED" "$TOTAL"
printf '[epr-sweep] proof column VerifiedGood %s   (reference violations: %d)\n' "$VERIFIED" "$KO"
printf '[epr-sweep] mean wall on solved rows: %ss   peak RSS: %s MB\n' "$MS" "$MAXRSS"
echo "[epr-sweep] run.csv          $RUN_CSV"
echo "[epr-sweep] conditions.txt   $OUT/conditions.txt"
[[ -f "$OUT/epr_telemetry.csv" ]] && echo "[epr-sweep] epr_telemetry.csv  $OUT/epr_telemetry.csv"
echo "[epr-sweep] audit summary    $OUT/certification/audit-summary.txt"

if [[ "$KO" -gt 0 ]]; then
  echo "[epr-sweep] REFERENCE VIOLATIONS: $KO. Do not report this run." >&2
fi

# Running this where the shape is not available
# -------------------------------------------
# A development host with fewer than 8 physical cores or less than 128 GB cannot
# produce a CASC number. The run still completes and still records the gap on its
# `% Hardware:` line, which is what makes the limitation visible instead of
# silent. On such a host, prefer the honest statement over a flattering subset:
#
#   EPU_TIME=15 EPU_JOBS=1 MRS_HARDWARE=adaptive epr_ab_sweep.sh quick
#
# and record that it is relative. What must not happen is quoting a subset's
# solved count as a division number, or a number without its `VerifiedGood`
# column.
