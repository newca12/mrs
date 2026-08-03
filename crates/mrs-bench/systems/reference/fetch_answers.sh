#!/usr/bin/env bash
# Populate systems/reference/answers.tsv from tptp.org.
#
# Fetches the authoritative TPTP "% Status" for every problem in the CASC
# problem lists and writes a tab-separated lookup table used by invoke.sh.
# Run once (or to refresh). Takes roughly 1-2 minutes with the default
# parallelism.
#
# For problems not yet in the public TPTP library (e.g. new CASC-30 ICU
# domains), falls back to reading the local problem file's % Status header.
#
# Usage: fetch_answers.sh [--jobs N] [--edition casc-30]
#
#   --jobs N      parallel curl workers (default: 8)
#   --edition E   problem edition directory (default: casc-30)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BENCH_DIR="${SCRIPT_DIR}/../.."

EDITION="casc-30"
JOBS=8

while [[ $# -gt 0 ]]; do
    case "$1" in
        --jobs)    JOBS="$2";    shift 2 ;;
        --edition) EDITION="$2"; shift 2 ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

PROBLEMS_DIR="${BENCH_DIR}/problems/${EDITION}"
OUT="${SCRIPT_DIR}/answers_${EDITION}.tsv"

if [[ ! -d "${PROBLEMS_DIR}/lists" ]]; then
    echo "Error: ${PROBLEMS_DIR}/lists not found." >&2
    echo "       Run crates/mrs-bench/setup.sh first." >&2
    exit 1
fi

# Collect all non-empty problem names from every list file.
all_problems=$(grep -h '[A-Z]' "${PROBLEMS_DIR}/lists/"*.list)
total=$(printf '%s\n' "$all_problems" | wc -l)

echo "[reference] Fetching ${total} problem statuses from tptp.org (--jobs ${JOBS}) ..."
echo "[reference] Writing to ${OUT}"

# ---------------------------------------------------------------------------
# fetch_one <problem>
#   1. Fetches the SeeTPTP page for <problem>; extracts "% Status : <Word>".
#   2. If the website has no answer, falls back to the local problem file's
#      % Status header (useful for new CASC domains not yet in TPTP v9.x).
#   3. Maps anything other than the four canonical SZS solved statuses to
#      GaveUp, then prints a tab-separated line "<problem>\t<status>".
# ---------------------------------------------------------------------------
fetch_one() {
    local problem="$1"
    local domain="${problem:0:3}"
    # URL-encode '+' as '%2B' so the CGI receives the literal plus sign.
    local encoded="${problem//+/%2B}"
    local url="https://tptp.org/cgi-bin/SeeTPTP?Category=Problems&Domain=${domain}&File=${encoded}.p"

    local raw status
    raw=$(curl -s --max-time 20 --retry 2 "$url" 2>/dev/null || true)
    # The status line looks like:  % Status   : Theorem
    status=$(printf '%s\n' "$raw" | grep -m1 '% Status' | awk '{print $NF}')

    # If the website had no answer, try the local problem file.
    if [[ -z "$status" || "$status" == "GaveUp" ]]; then
        local local_file
        local_file=$(find "$PROBLEMS_DIR" -maxdepth 2 -name "${problem}.p" 2>/dev/null | head -1)
        if [[ -n "$local_file" ]]; then
            local local_status
            local_status=$(grep -m1 '% Status' "$local_file" 2>/dev/null | awk '{print $NF}')
            [[ -n "$local_status" ]] && status="$local_status"
        fi
    fi

    case "$status" in
        Theorem|Unsatisfiable|Satisfiable|CounterSatisfiable) ;;
        *) status="GaveUp" ;;
    esac

    printf '%s\t%s\n' "$problem" "$status"
}
export -f fetch_one
export PROBLEMS_DIR

# Run fetches in parallel, sort by problem name, write atomically via a temp file.
tmp="${OUT}.tmp.$$"
trap 'rm -f "$tmp"' EXIT

printf '%s\n' "$all_problems" \
    | xargs -P "$JOBS" -I {} bash -c 'fetch_one "$@"' _ {} \
    | sort \
    > "$tmp"

mv "$tmp" "$OUT"
trap - EXIT

total_lines=$(wc -l < "$OUT")
solved=$(awk -F'\t' '$2 != "GaveUp" {c++} END {print c+0}' "$OUT")
echo "[reference] Done. ${total_lines} problems in answers_${EDITION}.tsv (${solved} with definitive status)."
