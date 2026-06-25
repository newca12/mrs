#!/usr/bin/env bash
#
# norgler_zenodo_benchmark.sh
#
# Benchmarks mrs-proover against Nörgler on the official TSTP Verification
# Benchmark dataset (Zenodo 19792604).
#
# We run on a subset of the dataset to keep benchmark times reasonable, or
# you can run on the full dataset.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

DATASET_DIR="${SCRIPT_DIR}/zenodo-corpus/benchmarks"

TIME=30
OUTPUT=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --time)       TIME="$2";       shift 2 ;;
        --output)     OUTPUT="$2";     shift 2 ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

TS="$(date +%Y%m%d_%H%M%S)"
[[ -z "${OUTPUT}" ]] && OUTPUT="${SCRIPT_DIR}/results/zenodo-benchmark/${TS}"
mkdir -p "${OUTPUT}"
exec 2> >(tee -a "${OUTPUT}/run.log" >&2)

PROOVER="${WORKSPACE_ROOT}/target/release/mrs-proover"
NORGLER="${SCRIPT_DIR}/systems/norgler/invoke.sh"

CSV="${OUTPUT}/run.csv"
echo "dataset,category,proof,backend,verdict,wall_time_s,detail" > "${CSV}"

echo "[zenodo] Time:     ${TIME}s / proof / backend" >&2
echo "[zenodo] Output:   ${OUTPUT}" >&2

run_backend() {
    local backend="$1" dataset="$2" category="$3" proof="$4" prob_file="$5" name="$6"
    local tmp; tmp="$(mktemp)"
    local start_ms end_ms wall_s line verdict detail=""
    
    start_ms=$(date +%s%3N)
    
    if [[ "${backend}" == "mrs-proover" ]]; then
        local args=("--time" "${TIME}")
        if [[ -n "${prob_file}" ]]; then
            args+=("--problems-dir" "$(dirname "${prob_file}")")
        fi
        timeout "${TIME}" "${PROOVER}" "${args[@]}" "${proof}" > "${tmp}" 2>/dev/null || true
    else
        local args=("--timeout" "${TIME}")
        if [[ -n "${prob_file}" ]]; then
            args+=("--problem" "${prob_file}")
        fi
        # Tighten timeout to keep things moving. Zenodo dataset is large.
        timeout "${TIME}" "${NORGLER}" "${args[@]}" "${proof}" > "${tmp}" 2>/dev/null || true
    fi
    
    end_ms=$(date +%s%3N)
    wall_s=$(echo "scale=3; (${end_ms} - ${start_ms}) / 1000" | bc)

    line=$(grep -m1 '% SZS status' "${tmp}" 2>/dev/null || true)
    if [[ -z "${line}" ]]; then
        verdict="NotVerified"
        detail="no SZS line emitted (timeout or crash)"
    else
        verdict=$(awk '{print $4}' <<< "${line}")
        if [[ "${line}" == *":"* ]]; then
            detail="${line#*: }"
        else
            detail=""
        fi
    fi
    detail="${detail//,/;}"
    detail="${detail//$'\n'/ }"
    rm -f "${tmp}"
    printf '%s,%s,%s,%s,%s,%s,%s\n' "${dataset}" "${category}" "${name}" "${backend}" "${verdict}" "${wall_s}" "${detail}"
}

run_dataset() {
    local dataset="$1" category="$2" limit="$3"
    local proof_dir="${DATASET_DIR}/${dataset}/${category}"
    local prob_dir="${DATASET_DIR}/${dataset}/problems"
    
    mapfile -t PROOF_FILES < <(find "${proof_dir}" -type f -name '*.proof' | sort | head -n "${limit}")
    
    local total=$(( ${#PROOF_FILES[@]} * 2 ))
    local done=0
    
    echo "[zenodo] Running ${dataset}/${category} (limit: ${limit} proofs) ..." >&2
    
    for proof in "${PROOF_FILES[@]}"; do
        local name="$(basename "${proof}")"
        local prob_name="${name%%_*}"
        local prob_file=""
        
        if [[ -f "${prob_dir}/${prob_name}.p" ]]; then
            prob_file="${prob_dir}/${prob_name}.p"
        fi
        
        row_mrs=$(run_backend "mrs-proover" "${dataset}" "${category}" "${proof}" "${prob_file}" "${name}")
        printf '%s\n' "${row_mrs}" >> "${CSV}"
        (( done++ )) || true
        
        # row_norgler=$(run_backend "norgler" "${dataset}" "${category}" "${proof}" "${prob_file}" "${name}")
        # printf '%s\n' "${row_norgler}" >> "${CSV}"
        # (( done++ )) || true
        
        printf '\r[zenodo] %s/%s %d/%d completed' "${dataset}" "${category}" "${done}" "$((total / 2))" >&2
    done
    printf '\n' >&2
}

# We sample the datasets to avoid a multi-hour run, e.g. 50 proofs each.
run_dataset "PyRes" "original" 100
run_dataset "PyRes" "falsified" 100

echo "" >&2
echo "[zenodo] Summary (rows = dataset/category/backend, cols = verdict):" >&2
awk -F, '
NR>1 {
    k=$1"/"$2"/"$4;
    n[k,$5]++; keys[k]=1; v[$5]=1;
    t[k]+=$6; c[k]++;
}
END {
    printf "  %-35s %-12s %-16s %-12s %-14s %-14s\n",
           "dataset/category/backend","Verified","FailedVerified","NotVerified","tot_time(s)","mean(s)" > "/dev/stderr"
    
    n_keys = asorti(keys, sorted_keys)
    for (i=1; i<=n_keys; i++) {
        k = sorted_keys[i]
        printf "  %-35s %-12d %-16d %-12d %-14.3f %-14.3f\n",
               k, n[k,"Verified"]+0, n[k,"FailedVerified"]+0, n[k,"NotVerified"]+0,
               t[k], t[k]/c[k] > "/dev/stderr"
    }
}' "${CSV}"

echo "" >&2
echo "[zenodo] CSV: ${CSV}" >&2
