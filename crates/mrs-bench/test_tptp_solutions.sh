#!/usr/bin/env bash
set -euo pipefail

# test_tptp_solutions.sh
#
# Ad-hoc LIVE spot-check: download random FOF proofs from TPTP and verify them
# with mrs-proover. Unlike the committed corpus (build_proover_corpus.sh +
# verify_proover_corpus.sh), this hits the network and samples fresh each run,
# so it is for exploration, not for a stable regression gate.
#
# To avoid the "every run discovers a new failure" problem, this script now
# samples ONLY systems whose proofs are standard TSTP FOF refutations — the
# format the ProoVer 2026 competition actually uses (E and Vampire). Random
# selection across all ~40 TPTP systems pulls in Alethe (cvc5/Z3), TFF
# (Beagle/iProver), connection-matrix (leanCoP/nanoCoP) and model-finder
# (Darwin/Paradox) outputs that mrs-proover legitimately cannot verify, which
# produced misleading "failures" that were really just unsupported formats.
#
# For the deterministic, offline regression gate run instead:
#   crates/mrs-bench/verify_proover_corpus.sh

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
PROOVER="${WORKSPACE_ROOT}/target/release/mrs-proover"

# Systems whose proofs are standard TSTP FOF refutations. The competition's
# own examples are E and cvc5; E and Vampire are the CASC champions and emit
# clean fof(...)/cnf(...) derivations ending in $false.
ALLOWED_SYSTEM_REGEX='System=(E---|Vampire---)'

if [[ ! -x "${PROOVER}" ]]; then
    echo "Building mrs-proover..."
    cargo build --release -p mrs-proover
fi

WORK_DIR="$(mktemp -d)"
trap 'rm -rf "${WORK_DIR}"' EXIT

mkdir -p "${WORK_DIR}/Problems"
mkdir -p "${WORK_DIR}/Proofs"

echo "Fetching problem list from SYN domain..."
# SYN domain has many FOF problems.
PROBLEMS=($(curl -m 10 -s "https://tptp.org/cgi-bin/SeeTPTP?Category=Solutions&Domain=SYN" | grep -oP 'File=SYN[0-9]{3}[+^_0-9.-]+' | cut -d= -f2 | sort -u | shuf))

COUNT=0
TARGET=10

for PROB in "${PROBLEMS[@]}"; do
    if [[ $COUNT -ge $TARGET ]]; then
        break
    fi

    # Find a solution file (.s) for this problem.
    # Restrict to E / Vampire THM refutations (standard TSTP FOF) — see header.
    SOL_URL_PART=$(curl -m 10 -s "https://tptp.org/cgi-bin/SeeTPTP?Category=Solutions&Domain=SYN&File=${PROB}" | grep -oP 'SeeTPTP\?Category=Solutions&Domain=SYN&File=[^"]+\.s' | grep "THM-" | grep -E "${ALLOWED_SYSTEM_REGEX}" | shuf -n 1 || true)

    if [[ -z "${SOL_URL_PART}" ]]; then
        continue
    fi

    echo "Found solution for ${PROB}. Downloading..."

    # Download the solution file
    RAW_SOL="${WORK_DIR}/Proofs/${PROB}.raw.s"
    curl -m 10 -s "https://tptp.org/cgi-bin/${SOL_URL_PART}" > "${RAW_SOL}"
    
    # Download the problem file
    RAW_PROB="${WORK_DIR}/Problems/${PROB}.raw.p"
    curl -m 10 -s "https://tptp.org/cgi-bin/SeeTPTP?Category=Problems&Domain=SYN&File=${PROB}.p" > "${RAW_PROB}"

    # Extract text from <pre> tags and remove HTML anchors
    awk '
        /<pre>/ { inside=1; sub(".*<pre>", ""); if ($0 == "") next }
        /<\/pre>/ { sub("</pre>.*", ""); print; inside=0; next }
        inside { print }
    ' "${RAW_PROB}" | sed -E -e 's/<[aA] [^>]+>//g' -e 's/<\/[aA]>//g' | sed -e 's/&lt;/</g' -e 's/&gt;/>/g' -e 's/&amp;/\&/g' > "${WORK_DIR}/Problems/${PROB}.p"

    # Skip CNF problems to avoid implicit quantifier ordering mismatch
    if grep -q "^cnf(" "${WORK_DIR}/Problems/${PROB}.p"; then
        echo "Problem ${PROB} is CNF. Skipping..."
        rm -f "${RAW_SOL}" "${RAW_PROB}" "${WORK_DIR}/Problems/${PROB}.p"
        continue
    fi

    # Ensure it is actually an FOF problem
    if ! grep -q "^fof(" "${WORK_DIR}/Problems/${PROB}.p"; then
        echo "Problem ${PROB} is not FOF. Skipping..."
        rm -f "${RAW_SOL}" "${RAW_PROB}" "${WORK_DIR}/Problems/${PROB}.p"
        continue
    fi

    # Extract proof and inject header + normalize cnf to fof, removing HTML anchors
    {
        echo "% Proof : Problems/${PROB}.p"
        awk '
            /<pre>/ { inside=1; sub(".*<pre>", ""); if ($0 == "") next }
            /<\/pre>/ { sub("</pre>.*", ""); print; inside=0; next }
            inside { print }
        ' "${RAW_SOL}" | sed -E -e 's/<[aA] [^>]+>//g' -e 's/<\/[aA]>//g' | sed -e 's/&lt;/</g' -e 's/&gt;/>/g' -e 's/&amp;/\&/g' | sed -E -e 's/^cnf\(/fof(/' -e 's/^%cnf\(/%fof(/'
    } > "${WORK_DIR}/Proofs/${PROB}.s"

    rm "${RAW_SOL}" "${RAW_PROB}"

    # Verify
    echo "Verifying ${PROB}..."
    RES=$(timeout 10 "${PROOVER}" --time 10 --problems-dir "${WORK_DIR}/Problems" "${WORK_DIR}/Proofs/${PROB}.s" 2>&1 || true)
    SZS=$(echo "${RES}" | grep '% SZS status' || echo "% SZS status Timeout/Error")
    
    echo "-> ${SZS}"
    
    COUNT=$((COUNT + 1))
done

echo "Done testing 10 solutions."
