#!/usr/bin/env bash
# Reference system: instant TPTP-authoritative SZS status lookup.
#
# Requires answers.tsv — run fetch_answers.sh once to populate it.
#
# casc.sh interface: invoke.sh <problem_path> <time_limit_secs>
# The time limit argument is accepted but ignored; this system is instantaneous.

PROBLEM_PATH="${1:?Usage: invoke.sh <problem_path> <time_limit_secs>}"
# $2 (time limit) intentionally ignored.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# Deduce edition from problem path, e.g. .../problems/casc-j13/FEQ/PROB.p
EDITION=$(basename "$(dirname "$(dirname "$PROBLEM_PATH")")")
ANSWERS="${SCRIPT_DIR}/answers_${EDITION}.tsv"
if [[ ! -f "$ANSWERS" ]]; then
    ANSWERS="${SCRIPT_DIR}/answers.tsv"
fi

problem=$(basename "$PROBLEM_PATH" .p)

if [[ ! -f "$ANSWERS" ]]; then
    echo "% SZS status GaveUp for $problem"
    echo "% (answers_${EDITION}.tsv or answers.tsv missing — run: crates/mrs-bench/systems/reference/fetch_answers.sh --edition ${EDITION})" >&2
    exit 0
fi

status=$(awk -F'\t' -v p="$problem" '$1 == p { print $2; exit }' "$ANSWERS")
echo "% SZS status ${status:-GaveUp} for $problem"
