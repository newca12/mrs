#!/usr/bin/env bash
# crates/mrs-bench/fuzz_proover.sh
#
# Generate TSTP proofs from a directory of TPTP problems using an ATP
# (eprover or vampire), then verify every produced proof with mrs-proover.
# Aggregates Verified / FailedVerified / NotVerified counts and prints a
# frequency table of inference rules mrs-proover could not handle — the
# primary signal for prioritising verifier work.
#
# Designed to scale from the small in-tree `problems/` directory (~70
# problems) up to the full TPTP-v9 FOF subset (~9,500 problems) on a
# multi-core machine.
#
# Usage:
#   crates/mrs-bench/fuzz_proover.sh [OPTIONS]
#
# Options:
#   --problems-dir DIR    Root of a TPTP problem tree, searched recursively
#                         (default: <repo>/problems)
#   --pattern GLOB        Filename glob passed to find -name
#                         (default: '*+*.p' = TPTP FOF problems;
#                          use '*.p' for the in-tree flat directory)
#   --generator NAME      Proof generator: eprover | vampire
#                         (default: eprover)
#   --time SECS           Per-problem CPU/wall-clock budget for generation
#                         AND per-proof budget for verification (default: 10)
#   --jobs N              Number of parallel workers (default: 1)
#   --output DIR          Where to keep generated proofs + summary
#                         (default: a fresh mktemp directory)
#   --limit N             Process only the first N problems after discovery
#                         (useful for smoke tests)
#   --verify-only         Skip generation; re-verify proofs already present
#                         under <output>/proofs/ (and Problems/). Useful for
#                         measuring verifier changes against a fixed corpus.
#
# Examples:
#   # In-tree smoke test (eprover, single threaded):
#   fuzz_proover.sh
#
#   # Full TPTP FOF set with eprover, 64 workers:
#   fuzz_proover.sh --problems-dir /data/TPTP-v9.0.0/Problems \
#                   --generator eprover --jobs 64 --time 30
#
#   # Same, with vampire:
#   fuzz_proover.sh --problems-dir /data/TPTP-v9.0.0/Problems \
#                   --generator vampire --jobs 64 --time 30
#
# Requires: a release-built mrs-proover, plus whichever generator you pick:
#   cargo build --release -p mrs-proover
#   crates/mrs-bench/systems/{eprover,vampire}/bin/  (from setup.sh)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

PROBLEMS_DIR="${WORKSPACE_ROOT}/problems"
PATTERN='*.p'         # Overridden to '*+*.p' below if the user didn't set --pattern
PATTERN_EXPLICIT=0
GENERATOR="eprover"
TIME=10
JOBS=1
OUTPUT=""
LIMIT=0
VERIFY_ONLY=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --problems-dir) PROBLEMS_DIR="$2";          shift 2 ;;
        --pattern)      PATTERN="$2"; PATTERN_EXPLICIT=1; shift 2 ;;
        --generator)    GENERATOR="$2";             shift 2 ;;
        --time)         TIME="$2";                  shift 2 ;;
        --jobs)         JOBS="$2";                  shift 2 ;;
        --output)       OUTPUT="$2";                shift 2 ;;
        --limit)        LIMIT="$2";                 shift 2 ;;
        --verify-only)  VERIFY_ONLY=1;              shift ;;
        -h|--help) sed -n '2,55p' "$0"; exit 0 ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

# If the user pointed us at something other than the in-tree problems/ dir
# and didn't override --pattern, default to FOF problems by TPTP filename
# convention ('+' before the version digit means FOF).
if [[ "${PATTERN_EXPLICIT}" -eq 0 && "${PROBLEMS_DIR}" != "${WORKSPACE_ROOT}/problems" ]]; then
    PATTERN='*+*.p'
fi

EPROVER="${SCRIPT_DIR}/systems/eprover/bin/eprover"
VAMPIRE="${SCRIPT_DIR}/systems/vampire/bin/vampire"
PROOVER="${PROOVER:-${WORKSPACE_ROOT}/target/release/mrs-proover}"

case "${GENERATOR}" in
    eprover)
        if [[ "${VERIFY_ONLY}" -eq 0 ]]; then
            [[ -x "${EPROVER}" ]] || { echo "eprover not found at ${EPROVER}" >&2; exit 1; }
        fi
        ;;
    vampire)
        if [[ "${VERIFY_ONLY}" -eq 0 ]]; then
            [[ -x "${VAMPIRE}" ]] || { echo "vampire not found at ${VAMPIRE}" >&2; exit 1; }
        fi
        ;;
    *) echo "Unknown --generator: ${GENERATOR} (want eprover|vampire)" >&2; exit 1 ;;
esac
[[ -x "${PROOVER}" ]] || { echo "mrs-proover not built; run: cargo build --release -p mrs-proover" >&2; exit 1; }

if [[ -z "${OUTPUT}" ]]; then
    OUTPUT="$(mktemp -d -t fuzz-proover-XXXXXX)"
else
    mkdir -p "${OUTPUT}"
fi
mkdir -p "${OUTPUT}/Problems" "${OUTPUT}/proofs"

CSV="${OUTPUT}/run.csv"
if [[ "${VERIFY_ONLY}" -eq 1 ]]; then
    # Verify-only writes to a sibling CSV so we never clobber the original
    # generation run. Timestamped so repeated re-verifies don't overwrite
    # each other either.
    CSV="${OUTPUT}/run-reverify-$(date +%Y%m%d-%H%M%S).csv"
fi
echo "problem,generator,verdict,detail" > "${CSV}"

echo "[fuzz] Problems dir: ${PROBLEMS_DIR}" >&2
echo "[fuzz] Pattern:      ${PATTERN}" >&2
echo "[fuzz] Generator:    ${GENERATOR}" >&2
echo "[fuzz] Time budget:  ${TIME}s / problem" >&2
echo "[fuzz] Workers:      ${JOBS}" >&2
echo "[fuzz] Output:       ${OUTPUT}" >&2
if [[ "${VERIFY_ONLY}" -eq 1 ]]; then
    echo "[fuzz] Mode:         verify-only (re-verify existing proofs/)" >&2
fi

# Discover problems. In verify-only mode we walk the existing proofs/
# directory (so re-verification doesn't require the original TPTP tree).
# Otherwise we walk the problem tree as configured.
if [[ "${VERIFY_ONLY}" -eq 1 ]]; then
    mapfile -t PROBLEMS < <(
        find "${OUTPUT}/proofs" -type f -name '*_proof.p' \
        | sed -E 's|/([^/]+)_proof\.p$|/\1.p|' \
        | sed -E "s|${OUTPUT}/proofs|${OUTPUT}/Problems|" \
        | sort
    )
else
    mapfile -t PROBLEMS < <(find "${PROBLEMS_DIR}" -type f -name "${PATTERN}" | sort)
fi
if [[ "${LIMIT}" -gt 0 && "${#PROBLEMS[@]}" -gt "${LIMIT}" ]]; then
    PROBLEMS=("${PROBLEMS[@]:0:${LIMIT}}")
fi
TOTAL="${#PROBLEMS[@]}"
echo "[fuzz] Discovered:   ${TOTAL} problems" >&2

if [[ "${TOTAL}" -eq 0 ]]; then
    echo "[fuzz] Nothing to do." >&2
    exit 0
fi

# --- Per-problem worker -----------------------------------------------------
#
# Runs the generator, extracts a proof (if any), writes it under
# ${OUTPUT}/proofs/, runs mrs-proover, and appends one CSV row.
# Designed to be invoked under xargs -P, so it must not share state with
# siblings beyond the append-only CSV (which we serialise via flock).
process_one() {
    local prob="$1"
    local name; name="$(basename "${prob}" .p)"

    # Copy the problem into the Problems/ subdir that mrs-proover expects to
    # find via the '% Proof : Problems/<name>.p' header. Use atomic mv so a
    # crashed worker doesn't leave a half-written file racing another worker.
    local prob_dst="${OUTPUT}/Problems/${name}.p"
    if [[ ! -f "${prob_dst}" ]]; then
        cp "${prob}" "${prob_dst}.tmp.$$" && mv "${prob_dst}.tmp.$$" "${prob_dst}"
    fi

    local proof_path="${OUTPUT}/proofs/${name}_proof.p"

    if [[ "${VERIFY_ONLY}" -eq 1 ]]; then
        # Re-verify mode: skip generation entirely. A missing proof file
        # means the previous run produced no proof for this problem, so we
        # record NoProof and move on (preserves denominators across runs).
        if [[ ! -s "${proof_path}" ]]; then
            emit_row "${name}" "NoProof" ""
            return
        fi
    else
    # Run the chosen generator with a hard wall-clock cap. `timeout` is the
    # outer safety net; the generator's own --cpu-limit / --time_limit is
    # the inner one.
    local raw; raw="$(mktemp)"
    case "${GENERATOR}" in
        eprover)
            timeout $(( TIME + 5 )) "${EPROVER}" \
                --auto --proof-object --cpu-limit="${TIME}" --tstp-format \
                "${prob}" > "${raw}" 2>/dev/null || true
            ;;
        vampire)
            timeout $(( TIME + 5 )) "${VAMPIRE}" \
                --time_limit "${TIME}" --input_syntax tptp --proof tptp \
                "${prob}" > "${raw}" 2>/dev/null || true
            ;;
    esac

    # Did the generator actually find a refutation? Two cheap signals:
    #   (a) an SZS status line announcing Theorem / Unsatisfiable, and
    #   (b) a `$false` literal somewhere in the body.
    # Vampire writes multi-line clauses so a single-line regex isn't enough.
    if ! grep -qE '% SZS status (Theorem|Unsatisfiable|ContradictoryAxioms)' "${raw}" \
        || ! grep -q '\$false' "${raw}"; then
        rm -f "${raw}"
        emit_row "${name}" "NoProof" ""
        return
    fi

    # Extract just the proof body. For vampire the body is bracketed by
    # `% SZS output start/end Proof`; for eprover --proof-object the whole
    # stdout is the proof. We accept either.
    local proof_body; proof_body="$(mktemp)"
    if grep -q '% SZS output start' "${raw}"; then
        # Print everything strictly between the start/end markers.
        awk '
            /% SZS output start/ { inside=1; next }
            /% SZS output end/   { inside=0 }
            inside { print }
        ' "${raw}" > "${proof_body}"
    else
        # Filter to lines that look like TPTP annotated formulas (cnf/fof/tff)
        # plus their continuations. Cheap heuristic: skip leading '#' comments
        # eprover emits as commentary.
        grep -vE '^#' "${raw}" > "${proof_body}" || true
    fi
    rm -f "${raw}"

    # Stitch a verifier-ready file. cnf(...) lines are rewritten to fof(...)
    # because mrs-proover treats top-level free variables as universally
    # closed in either dialect — equivalent at this position.
    {
        echo "% Proof : Problems/${name}.p"
        sed -E -e 's/^cnf\(/fof(/' -e 's/^%cnf\(/%fof(/' "${proof_body}"
    } > "${proof_path}.tmp.$$" && mv "${proof_path}.tmp.$$" "${proof_path}"
    rm -f "${proof_body}"
    fi

    # Verify.
    local res verdict detail
    res="$(timeout $(( TIME + 5 )) "${PROOVER}" \
        --time "${TIME}" \
        --problems-dir "${OUTPUT}/Problems" \
        "${proof_path}" 2>/dev/null || true)"

    local szs_line; szs_line="$(grep -m1 '% SZS status' <<< "${res}" || true)"
    if [[ -z "${szs_line}" ]]; then
        verdict="NotVerified"
        detail="no SZS line emitted"
    else
        verdict="$(awk '{print $4}' <<< "${szs_line}")"
        if [[ "${szs_line}" == *":"* ]]; then
            detail="${szs_line#*: }"
        else
            detail=""
        fi
    fi
    emit_row "${name}" "${verdict}" "${detail}"
}

# CSV append, serialised across workers via flock on the file itself.
emit_row() {
    local name="$1" verdict="$2" detail="$3"
    detail="${detail//,/;}"
    detail="${detail//$'\n'/ }"
    local row; row="$(printf '%s,%s,%s,%s' "${name}" "${GENERATOR}" "${verdict}" "${detail}")"
    (
        flock 9
        printf '%s\n' "${row}" >> "${CSV}"
    ) 9>>"${CSV}.lock"
}

export -f process_one emit_row
export OUTPUT GENERATOR TIME EPROVER VAMPIRE PROOVER CSV VERIFY_ONLY

# Drive the workers. xargs -P is portable and good enough; GNU parallel
# would be slightly nicer but is an extra dependency.
printf '%s\0' "${PROBLEMS[@]}" | \
    xargs -0 -P "${JOBS}" -I{} bash -c 'process_one "$@"' _ {}

# --- Summary ---------------------------------------------------------------
echo "" >&2
echo "[fuzz] Done. ${TOTAL} problems processed via ${GENERATOR}." >&2

awk -F, '
    NR>1 { n[$3]++; total++ }
    END {
        printf "[fuzz]   Verified:               %6d\n", n["Verified"]+0       > "/dev/stderr"
        printf "[fuzz]   FailedVerified:         %6d\n", n["FailedVerified"]+0 > "/dev/stderr"
        printf "[fuzz]   NotVerified:            %6d\n", n["NotVerified"]+0    > "/dev/stderr"
        printf "[fuzz]   NoProof (gen gave up):  %6d\n", n["NoProof"]+0        > "/dev/stderr"
        printf "[fuzz]   Total:                  %6d\n", total                 > "/dev/stderr"
    }
' "${CSV}"

# Frequency table of unhandled inference rules. The 'detail' column for a
# NotVerified row often contains `rule=Some("<name>")`; surface the top
# offenders so the verifier team knows where to spend effort.
echo "" >&2
echo "[fuzz] Top unhandled inference rules (NotVerified rows):" >&2
awk -F, '
    NR>1 && $3=="NotVerified" {
        if (match($0, /rule=Some\("[^"]+"\)/)) {
            # `rule=Some("` is 11 chars; `")` suffix is 2 chars; trim both.
            r=substr($0, RSTART+11, RLENGTH-13)
            n[r]++
        } else if (match($0, /rule="[^"]+"/)) {
            # `rule="` is 6 chars; `"` suffix is 1 char.
            r=substr($0, RSTART+6, RLENGTH-7)
            n[r]++
        }
    }
    END {
        for (r in n) printf "  %-30s %6d\n", r, n[r]
    }
' "${CSV}" | sort -k2 -rn | head -30 >&2

# Frequency table of FailedVerified detail patterns. These are proofs the
# verifier actively rejected — could be real bugs in the proof, but on a
# corpus run they're more often verifier-side mismatches (e.g. unknown
# axiom names from vampire's `file(..., unknown)` annotations). Surface
# the most common detail prefix to triage them in bulk.
echo "" >&2
echo "[fuzz] Top FailedVerified reasons (first 4 words of detail):" >&2
awk -F, '
    NR>1 && $3=="FailedVerified" {
        # Take the part after "step XYZ: " if present, else the whole detail.
        d=$4
        sub(/^step [^:]*: */, "", d)
        # Collapse to the first 4 whitespace-separated tokens for grouping.
        split(d, w, " ")
        key=w[1]" "w[2]" "w[3]" "w[4]
        n[key]++
    }
    END {
        for (k in n) printf "  %-50s %6d\n", k, n[k]
    }
' "${CSV}" | sort -k2 -rn | head -30 >&2

echo "" >&2
echo "[fuzz] CSV:     ${CSV}" >&2
echo "[fuzz] Proofs:  ${OUTPUT}/proofs/" >&2
