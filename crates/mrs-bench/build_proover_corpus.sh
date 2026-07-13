#!/usr/bin/env bash
#
# build_proover_corpus.sh
#
# Build (or refresh) the *committed, offline* mrs-proover regression corpus.
#
# WHY THIS EXISTS
# ---------------
# The earlier `test_tptp_solutions.sh` picked ONE RANDOM solution out of the
# ~40 systems TPTP stores per problem. Most of those systems (cvc5/Z3 emit
# Alethe S-expressions, Beagle/iProver emit TFF, leanCoP/nanoCoP emit
# connection-matrix proofs, Darwin/Paradox emit finite models, ...) produce
# proof formats that mrs-proover legitimately cannot verify. Every run drew a
# different random sample, so every run "discovered" a different set of
# Unknown/VerifiedBad results — there was no stable signal and no way to
# tell a real regression from format noise.
#
# The ProoVer 2026 competition feeds proofs from a NARROW, well-formed
# distribution (TPTP/TSTP FOF refutations — in practice E, Vampire, cvc5). So
# the right regression gate is: a FIXED set of E + Vampire FOF refutations,
# stored in-repo, verified DETERMINISTICALLY and OFFLINE. That is this corpus.
#
# WHAT IT DOES
# ------------
# For each problem in PROBLEMS below, download the E and Vampire THM proofs
# (and the problem file), normalise them exactly the way the competition
# wrapper does (strip the SeeTPTP HTML, rewrite cnf(->fof(, inject the
# `% Proof :` header), and store them under:
#
#     crates/mrs-bench/proover-corpus/Problems/<PROB>.p
#     crates/mrs-bench/proover-corpus/proofs/<PROB>__<system>.s
#
# Run `verify_proover_corpus.sh` afterwards to verify them offline.
#
# This script is only needed to (re)generate the corpus; day-to-day regression
# runs use the committed files and never touch the network.
#
# Usage:
#   crates/mrs-bench/build_proover_corpus.sh            # refresh all
#   crates/mrs-bench/build_proover_corpus.sh --list     # print problem list

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CORPUS_DIR="${SCRIPT_DIR}/proover-corpus"
PROBLEMS_DIR="${CORPUS_DIR}/Problems"
PROOFS_DIR="${CORPUS_DIR}/proofs"

# Allowlist of systems whose proofs are standard TSTP FOF refutations — i.e.
# the format the competition actually uses. Keep this tight: every system here
# must reliably emit fof(...)/cnf(...) annotated formulas ending in $false.
# The version suffix is matched as a prefix so it survives minor version bumps.
ALLOWED_SYSTEMS=(
    "E---"
    "Vampire---"
)

# Fixed problem list: small, fast FOF theorems from several TPTP domains that
# both E and Vampire solve in well under a second. Chosen to exercise a spread
# of proof features (resolution, paramodulation, AVATAR splitting, Skolem/
# choice-axiom introduction, definition folding) while staying tiny so the
# committed corpus is only a few hundred KB. ~25 problems x up to 2 systems
# ~= up to 50 proof files.
PROBLEMS=(
    SYN040+1 SYN041+1 SYN044+1 SYN045+1 SYN046+1
    SYN047+1 SYN048+1 SYN049+1 SYN050+1 SYN051+1
    SYN052+1 SYN054+1 SYN055+1 SYN056+1 SYN057+1
    SYN315+1 SYN317+1 SYN320+1 SYN333+1 SYN340+1
    SYN347+1 SYN349+1 SYN351+1 SYN730+1 SYN731+1
)

DOMAIN_OF() {
    # SYN040+1 -> SYN ; PUZ001+1 -> PUZ
    printf '%s' "${1%%[0-9]*}"
}

if [[ "${1:-}" == "--list" ]]; then
    printf '%s\n' "${PROBLEMS[@]}"
    exit 0
fi

mkdir -p "${PROBLEMS_DIR}" "${PROOFS_DIR}"

# Strip TPTP's SeeTPTP HTML wrapper to the raw TPTP text.
extract_tptp() {
    awk '
        /<pre>/ { inside=1; sub(".*<pre>", ""); if ($0 == "") next }
        /<\/pre>/ { sub("</pre>.*", ""); print; inside=0; next }
        inside { print }
    ' \
    | sed -E -e 's/<[aA] [^>]+>//g' -e 's/<\/[aA]>//g' \
    | sed -e 's/&lt;/</g' -e 's/&gt;/>/g' -e 's/&amp;/\&/g'
}

system_allowed() {
    local sys="$1" allow
    for allow in "${ALLOWED_SYSTEMS[@]}"; do
        [[ "${sys}" == "${allow}"* ]] && return 0
    done
    return 1
}

echo "[corpus] Allowed systems: ${ALLOWED_SYSTEMS[*]}" >&2
echo "[corpus] Problems: ${#PROBLEMS[@]}" >&2

N_PROOFS=0
for PROB in "${PROBLEMS[@]}"; do
    DOMAIN="$(DOMAIN_OF "${PROB}")"

    # Download + normalise the problem file.
    PROB_DST="${PROBLEMS_DIR}/${PROB}.p"
    curl -m 15 -s "https://tptp.org/cgi-bin/SeeTPTP?Category=Problems&Domain=${DOMAIN}&File=${PROB}.p" \
        | extract_tptp > "${PROB_DST}"
    if ! grep -q '^fof(' "${PROB_DST}"; then
        echo "[corpus]   ${PROB}: not an FOF problem, skipping" >&2
        rm -f "${PROB_DST}"
        continue
    fi

    # Enumerate THM solutions, keep only allowlisted systems.
    SOLUTIONS=$(curl -m 15 -s "https://tptp.org/cgi-bin/SeeTPTP?Category=Solutions&Domain=${DOMAIN}&File=${PROB}" \
        | grep -oP 'SeeTPTP\?Category=Solutions&Domain='"${DOMAIN}"'&File=[^"]+\.s' \
        | grep 'THM-' || true)

    for SOL in ${SOLUTIONS}; do
        SYS=$(printf '%s' "${SOL}" | grep -oP 'System=\K[^.]+(\.[^.]+)*\.THM-[A-Za-z]+' || true)
        [[ -z "${SYS}" ]] && continue
        system_allowed "${SYS}" || continue

        # Short, filesystem-safe tag: E---3.3.0.THM-CRf -> E
        TAG=$(printf '%s' "${SYS}" | sed -E 's/---.*//')
        PROOF_DST="${PROOFS_DIR}/${PROB}__${TAG}.s"

        {
            echo "% Proof : Problems/${PROB}.p"
            curl -m 15 -s "https://tptp.org/cgi-bin/${SOL}" \
                | extract_tptp \
                | sed -E -e 's/^cnf\(/fof(/' -e 's/^%cnf\(/%fof(/'
        } > "${PROOF_DST}"

        if grep -qE '^\s*fof\(' "${PROOF_DST}" && grep -q '\$false' "${PROOF_DST}"; then
            echo "[corpus]   ${PROB} <- ${TAG}" >&2
            N_PROOFS=$((N_PROOFS + 1))
        else
            echo "[corpus]   ${PROB} <- ${TAG}: no usable refutation, dropping" >&2
            rm -f "${PROOF_DST}"
        fi
    done
done

echo "[corpus] Done. ${N_PROOFS} proof files under ${PROOFS_DIR}" >&2
