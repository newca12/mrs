#!/usr/bin/env bash
# A/B sweep for the EPU division on a small host.
#
# The CASC-30 EPU division is 100 problems at a 120 s wall clock on 8 physical
# cores and 128 GB. This host is an i3-5010U with 2 physical cores and 16 GB, so
# a faithful full-division run is not available here: 100 x 120 s of 8-core work
# is roughly 13 CPU-hours of an 8-core machine against 1.3 CPU-hours of this one.
# The sweep therefore measures *relative* improvement on a stratified subset,
# with the worker count pinned to the host's real physical cores, and reports the
# resource pressure it saw so an overlarge run is obvious rather than silent.
#
# Usage:
#   epr_ab_sweep.sh <label> <problem> [problem...]
#   epr_ab_sweep.sh --list          print the default subset
set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DIV="$REPO/crates/mrs-bench/problems/casc-30/EPU"
MRS="$REPO/target/release/mrs"
OUT_ROOT="${EPU_SWEEP_ROOT:-$REPO/crates/mrs-bench/results}"

# One problem per EPR profile and per observed outcome, so the subset moves when
# a change helps one profile and hurts another instead of averaging out. The
# subset spans the whole division, including axiom-only files (no `conjecture`
# role): those are refute-the-axioms tasks graded `Unsatisfiable` like the
# rest, and an earlier cut wrongly excluded them on a retracted "ceiling 82"
# theory (see docs/reports/benchmarks/epu-2026-09.md). HWV065-1 is in
# deliberately: its full Herbrand expansion over {true, false} is UNSAT, the
# counterexample that retired the exclusion.
DEFAULT_SUBSET=(
  HWV039-1 HWV089-1 HWV111-1 HWV041-1 HWV047-1       # epr_equality
  MSC015-1.022 PLA031-1.007 SWV418-1.300              # pure relational
  SWV421-1.500 SWV422-1.505 SWV423-1.010
  SWV418-1.900 SWV421-1.400 SWV422-1.465 SWV419-1.035
  HWV065-1 SYO591-1 MSC024-1                          # axiom-only, all expected Unsatisfiable
)

if [[ "${1:-}" == "--list" ]]; then
  printf '%s\n' "${DEFAULT_SUBSET[@]}"
  exit 0
fi

LABEL="${1:?label required}"
shift
PROBLEMS=("$@")
if [[ ${#PROBLEMS[@]} -eq 0 ]]; then
  PROBLEMS=("${DEFAULT_SUBSET[@]}")
fi

TIME_LIMIT="${EPU_TIME:-15}"
# Physical cores, not logical CPUs: SMT siblings add no capacity, and asking for
# more threads than cores on a 2-core box just adds contention to every
# measurement.
WORKERS="${EPU_WORKERS:-$(nproc --ignore=core 2>/dev/null || nproc)}"
JOBS="${EPU_JOBS:-1}"
OUT="$OUT_ROOT/epu-sweep-$LABEL"
mkdir -p "$OUT"

if [[ ! -x "$MRS" ]]; then
  echo "no binary at $MRS — run: cargo build --release" >&2
  exit 1
fi

printf '%s\n' "problem,status,expected,wall_s,peak_rss_kb,epr_result,epr_fallback,epr_ms,epr_rounds,epr_generated,epr_falsifying,epr_full_grounding,epr_clauses,epr_vars" \
  > "$OUT/run.csv"

run_one() {
  local p="$1"
  [[ "$p" == *.p ]] || p="$p.p"
  local name="${p%.p}"
  local raw="$OUT/raw/$name"
  mkdir -p "$(dirname "$raw")"
  local start end
  start=$(date +%s.%N)
  "$MRS" --time "$TIME_LIMIT" --workers "$WORKERS" "$DIV/$p" \
    > "$raw.stdout" 2> "$raw.stderr"
  end=$(date +%s.%N)

  local status
  status=$(grep -o 'SZS status [A-Za-z]*' "$raw.stdout" | head -1 | awk '{print $3}')
  # Every problem in the CASC-30 EPU division is expected Unsatisfiable; the
  # certified run's `expected` column reads Unsatisfiable for all 100 rows.
  local expected="Unsatisfiable"
  local wall
  wall=$(awk -v a="$start" -v b="$end" 'BEGIN{printf "%.2f", b-a}')

  # Telemetry is a single space-separated key=value run on the SZS detail line.
  local tele
  tele=$(tr ' ' '\n' < "$raw.stderr" | grep -E '^epr_' | tr '\n' ' ')
  get() { sed -n "s/.*[[:space:]]$1=\\([^[:space:]]*\\).*/\\1/p" <<<"$tele"; }
  local rss
  rss=$(grep -o 'Peak memory usage: [0-9]*' "$raw.stdout" | awk '{print $4}')

  printf '%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s\n' \
    "$name" "$status" "$expected" "$wall" "$rss" \
    "$(get epr_result)" "$(get epr_fallback)" "$(get epr_ms)" \
    "$(get epr_rounds)" "$(get epr_generated)" "$(get epr_falsifying)" \
    "$(get epr_full_grounding)" "$(get epr_clauses)" "$(get epr_vars)" \
    >> "$OUT/run.csv"
  printf '  %-16s %-14s %6ss  epr=%s/%s gen=%s fals=%s\n' \
    "$name" "$status" "$wall" "$(get epr_result)" "$(get epr_fallback)" \
    "$(get epr_generated)" "$(get epr_falsifying)"
}

export -f run_one
export MRS DIV OUT TIME_LIMIT WORKERS

printf '[epr-sweep] label=%s time=%ss workers=%s jobs=%s problems=%s\n' \
  "$LABEL" "$TIME_LIMIT" "$WORKERS" "$JOBS" "${#PROBLEMS[@]}"
printf '%s\n' "${PROBLEMS[@]}" | xargs -P "$JOBS" -I{} bash -c 'run_one "$@"' _ {}

solved=$(awk -F, 'NR>1 && $2=="Unsatisfiable"' "$OUT/run.csv" | wc -l)
total=$(awk 'NR>1' "$OUT/run.csv" | wc -l)
printf '[epr-sweep] %s: %d/%d Unsatisfiable -> %s/run.csv\n' \
  "$LABEL" "$solved" "$total" "$OUT"
