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
# casc.sh supplies the edition explicitly, which also works when an archive
# nests the division under a Problems/ directory. Retain path-based discovery
# for direct use of this wrapper.
EDITION="${CASC_EDITION:-}"
if [[ -z "${EDITION}" ]]; then
    problem_dir="$(dirname "${PROBLEM_PATH}")"
    if [[ "$(basename "${problem_dir,,}")" == *problemfiles ]]; then
        problem_dir="$(dirname "${problem_dir}")"
    fi
    division_parent="$(basename "$(dirname "${problem_dir}")")"
    if [[ "${division_parent,,}" == "problems" ]]; then
        EDITION="$(basename "$(dirname "$(dirname "${problem_dir}")")")"
    else
        EDITION="${division_parent}"
    fi
fi
ANSWERS="${CASC_ANSWERS_FILE:-${ANSWERS:-${SCRIPT_DIR}/answers_${EDITION}.tsv}}"
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
