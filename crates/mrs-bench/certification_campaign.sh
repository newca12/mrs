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
# Examples (8-core competition hardware, two jobs of four workers):
#   MRS_WORKERS=8 crates/mrs-bench/certification_campaign.sh \
#       --edition casc-j13 --systems mrs --divisions fne,feq,ueq \
#       --casc-times --jobs 2 --output results/cert-j13
#   crates/mrs-bench/certification_campaign.sh \
#       --edition casc-30 --systems mrs --divisions eps,epu,feq --casc-times --jobs 2
#
# Environment:
#   CERT_STRICT_TIME   per-proof kernel budget   (default 120)
#   CERT_JOBS          audit parallelism        (default 4)
#   CERT_OUT           report directory         (default <output>/certification)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
STRICT_TIME="${CERT_STRICT_TIME:-120}"
JOBS="${CERT_JOBS:-4}"
OUT=""
CERT_OUT="${CERT_OUT:-}"
SUBSET_FILE=""
MANIFEST_OUT=""

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
[[ -n "${OUT}" ]] || { echo "campaign: --output DIR is required" >&2; exit 2; }

# Always build (cargo no-ops when up to date). Building only when the binary is
# *missing* is not enough: the audit is a measurement of the current kernel, and
# a stale binary silently applies yesterday's rules. That happened here -- a
# campaign re-audited with a pre-fix `audit_casc_proofs` and reported every
# model as invalid, which is indistinguishable from "the prover emitted none".
AUDIT_BIN="${ROOT}/target/release/audit_casc_proofs"
(cd "${ROOT}" && cargo build --release -p mrs-bench --bin audit_casc_proofs)
[[ -x "${AUDIT_BIN}" ]] || { echo "campaign: ${AUDIT_BIN} not built" >&2; exit 2; }

echo "== phase 1/2: search (normal route, no self-check) =="
(cd "${ROOT}" && ./crates/mrs-bench/casc.sh "${args[@]}")

RUN_DIR="${ROOT}/${OUT}"
if [[ ! -d "${RUN_DIR}" ]]; then RUN_DIR="${OUT}"; fi
if [[ "${RUN_DIR}" != /* ]]; then RUN_DIR="${PWD}/${RUN_DIR}"; fi
echo "run directory: ${RUN_DIR}"
if [[ ! -f "${RUN_DIR}/run.csv" ]]; then
    echo "campaign: no run.csv at ${RUN_DIR}/run.csv" >&2
    exit 2
fi

echo "== phase 2/2: strict-kernel audit of the archived proofs =="
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
    AUDIT_SUBSET_ARGS+=(--subset "${RUN_DIR}/subset_resolved.txt")
fi
"${AUDIT_BIN}" \
    --run "${RUN_DIR}" \
    --problems-dir "${PROBLEMS_DIR}" \
    "${AUDIT_SUBSET_ARGS[@]}" \
    --checks strict \
    --strict-time "${STRICT_TIME}" \
    --jobs "${JOBS}" \
    --output "${CERT_OUT}" \
    --force

REPORT="${CERT_OUT}/audit.csv"
[[ -f "${REPORT}" ]] || { echo "campaign: no audit report at ${REPORT}" >&2; exit 2; }

echo "== outcome =="
python3 - "${REPORT}" "${RUN_DIR}/run_meta.txt" <<'PY'
import collections, csv, re, sys

with open(sys.argv[1], newline="") as report:
    rows = list(csv.DictReader(report))
audit_checks = set(filter(None, (rows[0].get("checks", "").split(",") if rows else [])))
run_meta = sys.argv[2]
resolved_subset_path = None
try:
    for line in open(run_meta):
        if line.startswith("subset_resolved="):
            resolved_subset_path = line.split("=", 1)[1].strip()
            break
except OSError:
    pass
if resolved_subset_path:
    try:
        selected = set()
        for line in open(resolved_subset_path):
            entry = line.strip()
            if entry:
                division, problem = entry.split("/", 1)
                selected.add((division.lower(), problem.removesuffix(".p")))
        rows = [row for row in rows
                if (row["division"].lower(), row["problem"].removesuffix(".p")) in selected]
    except OSError as error:
        print(f"campaign: cannot read resolved subset {resolved_subset_path}: {error}", file=sys.stderr)
        raise SystemExit(2)
refutations = [r for r in rows if r["generation_status"] in ("Theorem", "Unsatisfiable")]
models = [r for r in rows if r["generation_status"] in ("Satisfiable", "CounterSatisfiable")]
if not refutations:
    print("no refutations in this run")

by_division = collections.defaultdict(collections.Counter)
for row in refutations:
    by_division[row["division"]][row["strict_status"]] += 1
    by_division[row["division"]]["refutations"] += 1
for row in models:
    by_division[row["division"]]["models"] += 1
    if row["strict_status"] == "VerifiedGood" and "certified_model" in (row["strict_detail"] or ""):
        by_division[row["division"]]["certified_models"] += 1
    elif row["strict_status"] == "not_run" and "strict" not in audit_checks:
        by_division[row["division"]]["certified_models"] += 1

print(f"{'division':10s} {'refutations':>12s} {'certified':>10s} {'rejected':>9s} {'unknown':>8s}")
total = collections.Counter()
for division in sorted(by_division):
    counts = by_division[division]
    n = counts["refutations"]
    total.update({key: counts[key] for key in ("VerifiedGood", "VerifiedBad", "Unknown", "Timeout", "Error")})
    print(f"{division:10s} {n:12d} {counts['VerifiedGood']:10d} "
          f"{counts['VerifiedBad']:9d} {counts['Unknown'] + counts['Timeout']:8d}")
    if counts["models"]:
        print(f"  models: {counts['certified_models']}/{counts['models']} certified")
n = total["VerifiedGood"] + total["VerifiedBad"] + total["Unknown"] + total["Timeout"] + total["Error"]
print(f"{'TOTAL':10s} {n:12d} {total['VerifiedGood']:10d} {total['VerifiedBad']:9d} "
      f"{total['Unknown'] + total['Timeout']:8d}")
if n:
    print(f"certified: {100.0 * total['VerifiedGood'] / n:.1f}%")

if not rows:
    print("no benchmark rows matched the selected subset", file=sys.stderr)
    raise SystemExit(2)

print("\nreasons (rejected first: those are the ones to investigate)")
reasons = collections.Counter()
for row in refutations:
    if row["strict_status"] == "VerifiedGood":
        continue
    detail = re.sub(r"\bc\d+\b", "N", row["strict_detail"] or "")
    detail = re.sub(r"\d+", "N", detail)
    reasons[(row["strict_status"], detail[:100])] += 1
for (status, detail), count in reasons.most_common(25):
    print(f"  {count:5d} {status:10s} {detail}")

PY
