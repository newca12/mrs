#!/usr/bin/env bash
set -euo pipefail

# test_tptp_solutions.sh
# Downloads 10 random FOF solutions from TPTP and tests them with mrs-proover.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
PROOVER="${WORKSPACE_ROOT}/target/release/mrs-proover"

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
    SOL_URL_PART=$(curl -m 10 -s "https://tptp.org/cgi-bin/SeeTPTP?Category=Solutions&Domain=SYN&File=${PROB}" | grep -oP 'SeeTPTP\?Category=Solutions&Domain=SYN&File=[^"]+\.s' | shuf -n 1 || true)
    
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
    ' "${RAW_PROB}" | sed -E -e 's/<a [^>]+>//g' -e 's/<\/a>//g' > "${WORK_DIR}/Problems/${PROB}.p"

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
        ' "${RAW_SOL}" | sed -E -e 's/<a [^>]+>//g' -e 's/<\/a>//g' | sed -E -e 's/^cnf\(/fof(/' -e 's/^%cnf\(/%fof(/'
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
