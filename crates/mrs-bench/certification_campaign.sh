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

# `--edition` and the problems root must agree with what phase 1 actually ran
# against. Both were previously reconstructed from --output, which has no
# relation to either: for the documented
#     --output results/cert-j13
# the audit was pointed at `crates/mrs-bench/problems/crates` and could not
# resolve a single problem.
args=("$@")
EDITION="casc-30"
for ((i = 0; i < ${#args[@]}; i++)); do
    case "${args[i]}" in
        --output)  OUT="${args[i + 1]:-}" ;;
        --edition) EDITION="${args[i + 1]:-}" ;;
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
echo "run directory: ${RUN_DIR}"

echo "== phase 2/2: strict-kernel audit of the archived proofs =="
# Resolve the corpus the same way casc.sh does (CASC_PROBLEMS_ROOT, else
# problems/<edition>), and prefer the run's own record over both: it is what
# phase 1 provably used.
PROBLEMS_DIR=""
if [[ -f "${RUN_DIR}/run_meta.txt" ]]; then
    PROBLEMS_DIR="$(sed -n 's/^problems_dir=//p' "${RUN_DIR}/run_meta.txt" | head -1)"
fi
if [[ -z "${PROBLEMS_DIR}" || ! -d "${PROBLEMS_DIR}" ]]; then
    PROBLEMS_DIR="${CASC_PROBLEMS_ROOT:-${ROOT}/crates/mrs-bench/problems/${EDITION}}"
fi
# An audit that cannot see the problems would report every proof as unknown, or
# nothing at all. Refuse to start rather than produce a coverage number.
if [[ ! -d "${PROBLEMS_DIR}" ]]; then
    echo "campaign: problems root '${PROBLEMS_DIR}' does not exist." >&2
    echo "campaign: pass --edition <name> (or set CASC_PROBLEMS_ROOT) so it names a" >&2
    echo "           corpus under crates/mrs-bench/problems/." >&2
    exit 2
fi
if ! compgen -G "${PROBLEMS_DIR}/*/*.p" >/dev/null && \
   ! compgen -G "${PROBLEMS_DIR}/*/Problems/*.p" >/dev/null; then
    echo "campaign: no .p files under '${PROBLEMS_DIR}'; refusing to audit" >&2
    echo "           ${#JOBS} job(s) of proofs against an empty corpus." >&2
    exit 2
fi
echo "audit problems root: ${PROBLEMS_DIR}"
"${AUDIT_BIN}" \
    --run "${RUN_DIR}" \
    --problems-dir "${PROBLEMS_DIR}" \
    --checks strict \
    --strict-time "${STRICT_TIME}" \
    --jobs "${JOBS}" \
    --output "${CERT_OUT:-${RUN_DIR}/certification}" \
    --force

REPORT="${CERT_OUT:-${RUN_DIR}/certification}/audit.csv"
[[ -f "${REPORT}" ]] || { echo "campaign: no audit report at ${REPORT}" >&2; exit 2; }

echo "== outcome =="
python3 - "${REPORT}" <<'PY'
import collections, csv, re, sys

rows = list(csv.DictReader(open(sys.argv[1])))
refutations = [r for r in rows if r["strict_status"] in
               ("VerifiedGood", "VerifiedBad", "Unknown", "Timeout", "Error")]
if not refutations:
    print("no refutations in this run: nothing to certify")
    raise SystemExit(0)

by_division = collections.defaultdict(collections.Counter)
for row in refutations:
    by_division[row["division"]][row["strict_status"]] += 1

print(f"{'division':10s} {'refutations':>12s} {'certified':>10s} {'rejected':>9s} {'unknown':>8s}")
total = collections.Counter()
for division in sorted(by_division):
    counts = by_division[division]
    n = sum(counts.values())
    total.update(counts)
    print(f"{division:10s} {n:12d} {counts['VerifiedGood']:10d} "
          f"{counts['VerifiedBad']:9d} {counts['Unknown'] + counts['Timeout']:8d}")
n = sum(total.values())
print(f"{'TOTAL':10s} {n:12d} {total['VerifiedGood']:10d} {total['VerifiedBad']:9d} "
      f"{total['Unknown'] + total['Timeout']:8d}")
if n:
    print(f"certified: {100.0 * total['VerifiedGood'] / n:.1f}%")

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

models = [r for r in rows if r["generation_status"] in ("Satisfiable", "CounterSatisfiable")]
if models:
    certified = [r for r in models if "certified_model" in (r["strict_detail"] or "")]
    print(f"\nmodel certificates: {len(certified)}/{len(models)} satisfiability results carry "
          "a model the kernel accepted")
PY
