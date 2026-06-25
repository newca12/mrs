#!/usr/bin/env bash
#
# zenodo_benchmark.sh
#
# Evaluate mrs-proover (and, optionally, the Nörgler reference verifier) on the
# official TSTP FOF Proof Benchmark dataset (Zenodo 19792604), published by the
# Nörgler authors to evaluate proof checkers.
#
# The dataset pairs, for each of two source provers (PyRes, Otter):
#   - original : genuine refutation proofs   (a checker should NOT FailedVerify)
#   - falsified: mutated/evil proofs         (a checker SHOULD FailedVerify)
#
# So the two soundness invariants we care about are:
#   * original  -> never FailedVerified   (a false reject costs -1 at competition)
#   * falsified -> never Verified          (a false accept costs -10, fatal)
#
# Only PyRes ships problem files; Otter proofs are self-contained leaves with no
# linked problem, so original-Otter leaves cannot be validated against a problem
# (they fall back to NotVerified) while falsified-Otter mutations on inference
# steps are still caught by the entailment check.
#
# Usage:
#   crates/mrs-bench/zenodo_benchmark.sh [options]
#
#   --dataset <PyRes|Otter|all>   Which source prover(s) to run (default: PyRes)
#   --limit <N>                   Cap proofs per {dataset,category} (default: all)
#   --time <SECS>                 Per-proof wall-clock budget (default: 30)
#   --with-norgler                Also run Nörgler (needs systems/norgler/, slow)
#   --output <DIR>                Results dir (default: results/zenodo-benchmark/<ts>)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

DATASET_DIR="${SCRIPT_DIR}/zenodo-corpus/benchmarks"

DATASETS="PyRes"
LIMIT=0          # 0 = no limit
TIME=30
WITH_NORGLER=0
OUTPUT=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dataset)      DATASETS="$2";    shift 2 ;;
        --limit)        LIMIT="$2";       shift 2 ;;
        --time)         TIME="$2";        shift 2 ;;
        --with-norgler) WITH_NORGLER=1;   shift   ;;
        --output)       OUTPUT="$2";      shift 2 ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

[[ "${DATASETS}" == "all" ]] && DATASETS="PyRes Otter"

# Materialise the corpus if absent (download + normalise, idempotent).
if [[ ! -d "${DATASET_DIR}" ]]; then
    echo "[zenodo] Corpus missing; fetching..." >&2
    "${SCRIPT_DIR}/fetch_zenodo_corpus.sh"
fi

PROOVER="${WORKSPACE_ROOT}/target/release/mrs-proover"
if [[ ! -x "${PROOVER}" ]]; then
    echo "mrs-proover not built; run: cargo build --release -p mrs-proover" >&2
    exit 1
fi

NORGLER="${SCRIPT_DIR}/systems/norgler/invoke.sh"
if [[ "${WITH_NORGLER}" -eq 1 && ! -x "${NORGLER}" ]]; then
    echo "Nörgler wrapper missing at ${NORGLER}; drop --with-norgler or install it." >&2
    exit 1
fi

TS="$(date +%Y%m%d_%H%M%S)"
[[ -z "${OUTPUT}" ]] && OUTPUT="${SCRIPT_DIR}/results/zenodo-benchmark/${TS}"
mkdir -p "${OUTPUT}"
exec 2> >(tee -a "${OUTPUT}/run.log" >&2)

CSV="${OUTPUT}/run.csv"
echo "dataset,category,proof,backend,verdict,wall_time_s,detail" > "${CSV}"

echo "[zenodo] Datasets: ${DATASETS}" >&2
echo "[zenodo] Limit:    $([[ ${LIMIT} -eq 0 ]] && echo all || echo "${LIMIT}")/category" >&2
echo "[zenodo] Time:     ${TIME}s / proof / backend" >&2
echo "[zenodo] Nörgler:  $([[ ${WITH_NORGLER} -eq 1 ]] && echo yes || echo no)" >&2
echo "[zenodo] Output:   ${OUTPUT}" >&2

# Run one backend on one proof and emit a CSV row.
run_backend() {
    local backend="$1" dataset="$2" category="$3" proof="$4" prob_file="$5" name="$6"
    local tmp; tmp="$(mktemp)"
    local start_ms end_ms wall_s line verdict detail=""

    start_ms=$(date +%s%3N)
    if [[ "${backend}" == "mrs-proover" ]]; then
        local args=("--time" "${TIME}")
        [[ -n "${prob_file}" ]] && args+=("--problems-dir" "$(dirname "${prob_file}")")
        timeout "$(( TIME + 5 ))" "${PROOVER}" "${args[@]}" "${proof}" > "${tmp}" 2>/dev/null || true
    else
        local args=("--timeout" "${TIME}")
        [[ -n "${prob_file}" ]] && args+=("--problem" "${prob_file}")
        timeout "$(( TIME + 5 ))" "${NORGLER}" "${args[@]}" "${proof}" > "${tmp}" 2>/dev/null || true
    fi
    end_ms=$(date +%s%3N)
    wall_s=$(echo "scale=3; (${end_ms} - ${start_ms}) / 1000" | bc)

    line=$(grep -m1 '% SZS status' "${tmp}" 2>/dev/null || true)
    if [[ -z "${line}" ]]; then
        verdict="NotVerified"
        detail="no SZS line emitted (timeout or crash)"
    else
        verdict=$(awk '{print $4}' <<< "${line}")
        [[ "${line}" == *":"* ]] && detail="${line#*: }" || detail=""
    fi
    detail="${detail//,/;}"; detail="${detail//$'\n'/ }"
    rm -f "${tmp}"
    printf '%s,%s,%s,%s,%s,%s,%s\n' \
        "${dataset}" "${category}" "${name}" "${backend}" "${verdict}" "${wall_s}" "${detail}"
}

run_category() {
    local dataset="$1" category="$2"
    local proof_dir="${DATASET_DIR}/${dataset}/${category}"
    local prob_dir="${DATASET_DIR}/${dataset}/problems"
    [[ -d "${proof_dir}" ]] || { echo "[zenodo] skip ${dataset}/${category} (absent)" >&2; return; }

    local find_cmd=(find "${proof_dir}" -type f -name '*.proof')
    mapfile -t PROOF_FILES < <("${find_cmd[@]}" | sort | { [[ ${LIMIT} -gt 0 ]] && head -n "${LIMIT}" || cat; })

    local n=${#PROOF_FILES[@]} done=0
    echo "[zenodo] ${dataset}/${category}: ${n} proofs" >&2

    for proof in "${PROOF_FILES[@]}"; do
        local name; name="$(basename "${proof}")"
        local prob_name="${name%%_*}"
        local prob_file=""
        [[ -f "${prob_dir}/${prob_name}.p" ]] && prob_file="${prob_dir}/${prob_name}.p"

        run_backend "mrs-proover" "${dataset}" "${category}" "${proof}" "${prob_file}" "${name}" >> "${CSV}"
        [[ "${WITH_NORGLER}" -eq 1 ]] && \
            run_backend "norgler" "${dataset}" "${category}" "${proof}" "${prob_file}" "${name}" >> "${CSV}"

        done=$((done + 1))
        printf '\r[zenodo] %s/%s %d/%d' "${dataset}" "${category}" "${done}" "${n}" >&2
    done
    printf '\n' >&2
}

for ds in ${DATASETS}; do
    run_category "${ds}" "original"
    run_category "${ds}" "falsified"
done

echo "" >&2
echo "[zenodo] Summary (rows = dataset/category/backend):" >&2
awk -F, '
NR>1 { k=$1"/"$2"/"$4; n[k,$5]++; keys[k]=1; t[k]+=$6; c[k]++; }
END {
    printf "  %-34s %-10s %-16s %-12s %-12s\n",
           "dataset/category/backend","Verified","FailedVerified","NotVerified","mean(s)" > "/dev/stderr"
    m = asorti(keys, sk)
    for (i=1; i<=m; i++) {
        k = sk[i]
        printf "  %-34s %-10d %-16d %-12d %-12.3f\n",
               k, n[k,"Verified"]+0, n[k,"FailedVerified"]+0, n[k,"NotVerified"]+0, t[k]/c[k] > "/dev/stderr"
    }
}' "${CSV}"

# Soundness invariants: original must never FailedVerified; falsified must never Verified.
BAD_ORIG=$(awk -F, 'NR>1 && $2=="original"  && $5=="FailedVerified"' "${CSV}" | wc -l)
BAD_FALS=$(awk -F, 'NR>1 && $2=="falsified" && $5=="Verified"'       "${CSV}" | wc -l)
echo "" >&2
echo "[zenodo] Soundness check:" >&2
echo "[zenodo]   original  reported FailedVerified : ${BAD_ORIG} (must be 0)" >&2
echo "[zenodo]   falsified reported Verified       : ${BAD_FALS} (must be 0)" >&2
echo "" >&2
echo "[zenodo] CSV: ${CSV}" >&2

# Only a falsely-accepted evil proof is fatal; a false reject is a soft -1.
[[ "${BAD_FALS}" -eq 0 ]] || { echo "[zenodo] FAIL: an evil proof was Verified." >&2; exit 1; }
