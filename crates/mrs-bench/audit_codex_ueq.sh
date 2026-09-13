#!/usr/bin/env bash
# Sequentially regenerate and independently verify every solved UEQ result in
# codex.db using only the local sanitized CASC problem trees.
#
# The database is opened read-only by sqlite3. Proofs and reports are written
# to an external audit directory; codex.db is never updated.
#
# Usage:
#   crates/mrs-bench/audit_codex_ueq.sh [--db PATH] [--output DIR]
#                                      [--jobs N] [--strict-time SECS]
#                                      [--ladder-time SECS] [--limit N]
#                                      [--resume] [--stop-on-bad]
#
# This audit is intentionally sequential. --jobs is accepted only as a
# compatibility check and must remain 1.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

DB_PATH="${WORKSPACE_ROOT}/codex.db"
OUTPUT="${WORKSPACE_ROOT}/ueq-proover-audit-$(date +%Y%m%d_%H%M%S)"
GEN_JOBS=1
STRICT_TIME=30
LADDER_TIME=30
ROW_LIMIT=0
RESUME=0
STOP_ON_BAD=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --db)
            DB_PATH="$2"
            shift 2
            ;;
        --output)
            OUTPUT="$2"
            shift 2
            ;;
        --jobs)
            GEN_JOBS="$2"
            shift 2
            ;;
        --strict-time)
            STRICT_TIME="$2"
            shift 2
            ;;
        --ladder-time)
            LADDER_TIME="$2"
            shift 2
            ;;
        --limit)
            ROW_LIMIT="$2"
            shift 2
            ;;
        --resume)
            RESUME=1
            shift
            ;;
        --stop-on-bad)
            STOP_ON_BAD=1
            shift
            ;;
        -h|--help)
            printf '%s\n' \
                "usage: $0 [--db PATH] [--output DIR] [--jobs 1]" \
                "          [--strict-time SECS] [--ladder-time SECS] [--limit N] [--resume] [--stop-on-bad]"
            exit 0
            ;;
        *)
            printf 'unknown argument: %s\n' "$1" >&2
            exit 2
            ;;
    esac
done

if [[ "${GEN_JOBS}" != "1" ]]; then
    printf '%s\n' 'This audit is intentionally sequential; --jobs must be 1.' >&2
    exit 2
fi

for value_name in STRICT_TIME LADDER_TIME; do
    value="${!value_name}"
    if ! [[ "${value}" =~ ^[1-9][0-9]*$ ]]; then
        printf '%s must be a positive integer\n' "${value_name}" >&2
        exit 2
    fi
done

if ! [[ "${ROW_LIMIT}" =~ ^[0-9]+$ ]]; then
    printf '%s must be a non-negative integer\n' 'ROW_LIMIT' >&2
    exit 2
fi

if [[ ! -f "${DB_PATH}" ]]; then
    printf 'database not found: %s\n' "${DB_PATH}" >&2
    exit 1
fi

MRS_BIN="${WORKSPACE_ROOT}/target/release/mrs"
PROOVER_BIN="${WORKSPACE_ROOT}/target/release/mrs-proover"
EPROVER_BIN="${WORKSPACE_ROOT}/crates/mrs-bench/systems/eprover/bin/eprover"
VAMPIRE_BIN="${WORKSPACE_ROOT}/crates/mrs-bench/systems/vampire/bin/vampire"

for executable in "${MRS_BIN}" "${PROOVER_BIN}" "${EPROVER_BIN}" "${VAMPIRE_BIN}"; do
    if [[ ! -x "${executable}" ]]; then
        printf 'required executable not found: %s\n' "${executable}" >&2
        exit 1
    fi
done

if ! command -v sqlite3 >/dev/null 2>&1; then
    printf '%s\n' 'sqlite3 is required.' >&2
    exit 1
fi
if ! command -v sha256sum >/dev/null 2>&1; then
    printf '%s\n' 'sha256sum is required.' >&2
    exit 1
fi
if ! command -v timeout >/dev/null 2>&1; then
    printf '%s\n' 'GNU timeout is required.' >&2
    exit 1
fi

mkdir -p "${OUTPUT}/manifest" "${OUTPUT}/proofs/casc-30/Problems" \
    "${OUTPUT}/proofs/casc-j13/Problems" "${OUTPUT}/raw"

MANIFEST="${OUTPUT}/manifest/ueq.tsv"
REPORT="${OUTPUT}/audit.tsv"
SUMMARY="${OUTPUT}/summary.txt"
RUN_LOG="${OUTPUT}/run.log"

if [[ "${RESUME}" -eq 1 && ! -f "${REPORT}" ]]; then
    printf 'cannot resume; report not found: %s\n' "${REPORT}" >&2
    exit 1
fi

if [[ "${RESUME}" -eq 0 ]]; then
    printf '%s\n' \
        $'corpus\tproblem_name\tcanonical_name\tdb_status\tdb_time_to_solve\tdb_timeout\tproblem_path\tproof_path\tproof_sha256\tgeneration_status\tgeneration_exit\tstrict_status\tstrict_detail\tladder_status\tladder_detail' \
        > "${REPORT}"
fi

# The immutable URI prevents a concurrently-created WAL from changing the
# audit input and makes the read-only intent explicit.
sqlite3 -readonly "file:${DB_PATH}?immutable=1" <<'SQL' > "${MANIFEST}"
.mode tabs
.headers off
SELECT corpus,
       problem_name,
       canonical_name,
       status,
       printf('%.6f', COALESCE(time_to_solve, 0.0)),
       timeout
FROM results
WHERE division = 'UEQ'
  AND status IN ('Theorem', 'Unsatisfiable')
ORDER BY corpus, problem_name;
SQL

if [[ "${ROW_LIMIT}" -gt 0 ]]; then
    # Keep the manifest deterministic while allowing a bounded smoke test.
    awk -F '\t' -v limit="${ROW_LIMIT}" 'NR <= limit' "${MANIFEST}" > "${MANIFEST}.limited"
    mv "${MANIFEST}.limited" "${MANIFEST}"
fi

expected_rows=479
if [[ "${ROW_LIMIT}" -gt 0 ]]; then
    expected_rows="${ROW_LIMIT}"
fi
actual_rows="$(wc -l < "${MANIFEST}")"
if [[ "${actual_rows}" -ne "${expected_rows}" ]]; then
    printf 'expected %s solved UEQ rows, found %s\n' "${expected_rows}" "${actual_rows}" >&2
    exit 1
fi

for corpus in casc-30 casc-j13; do
    case "${corpus}" in
        casc-30) expected_corpus_rows=222 ;;
        casc-j13) expected_corpus_rows=257 ;;
    esac
    corpus_rows="$(awk -F '\t' -v c="${corpus}" '$1 == c { n++ } END { print n + 0 }' "${MANIFEST}")"
    if [[ "${ROW_LIMIT}" -eq 0 && "${corpus_rows}" -ne "${expected_corpus_rows}" ]]; then
        printf '%s: expected %s rows, found %s\n' "${corpus}" "${expected_corpus_rows}" "${corpus_rows}" >&2
        exit 1
    fi
    if [[ ! -d "${WORKSPACE_ROOT}/crates/mrs-bench/problems/${corpus}/UEQ" ]]; then
        printf 'missing local UEQ corpus: %s\n' "${corpus}" >&2
        exit 1
    fi
    if [[ ! -d "${WORKSPACE_ROOT}/crates/mrs-bench/problems/${corpus}/Axioms" ]]; then
        printf 'missing local Axioms corpus: %s\n' "${corpus}" >&2
        exit 1
    fi
done

printf '%s\n' "audit_output=${OUTPUT}" "db=${DB_PATH}" \
    "manifest_rows=${actual_rows}" "strict_time=${STRICT_TIME}" \
    "ladder_time=${LADDER_TIME}" "generation_jobs=${GEN_JOBS}" "row_limit=${ROW_LIMIT}" \
    "stop_on_bad=${STOP_ON_BAD}" \
    "mrs_sha256=$(sha256sum "${MRS_BIN}" | cut -d ' ' -f 1)" \
    "proover_sha256=$(sha256sum "${PROOVER_BIN}" | cut -d ' ' -f 1)" \
    "eprover_sha256=$(sha256sum "${EPROVER_BIN}" | cut -d ' ' -f 1)" \
    "vampire_sha256=$(sha256sum "${VAMPIRE_BIN}" | cut -d ' ' -f 1)" \
    > "${SUMMARY}"

printf '[audit] output: %s\n' "${OUTPUT}" | tee -a "${RUN_LOG}"
printf '[audit] manifest: %s rows\n' "${actual_rows}" | tee -a "${RUN_LOG}"
printf '%s\n' '[audit] generation, strict verification, and ladder verification are sequential.' | tee -a "${RUN_LOG}"
if [[ "${RESUME}" -eq 1 ]]; then
    printf '%s\n' '[audit] resuming; rows already present in audit.tsv will be skipped.' | tee -a "${RUN_LOG}"
fi

csv_field() {
    local value="$1"
    value="${value//$'\t'/ }"
    value="${value//$'\r'/ }"
    value="${value//$'\n'/ }"
    printf '%s' "${value}"
}

extract_szs() {
    local file="$1"
    awk '/% SZS status/ { print $4; exit }' "${file}" 2>/dev/null || true
}

extract_detail() {
    local file="$1"
    awk '/% SZS status/ { if (index($0, ": ") > 0) { sub(/^.*: /, ""); print; exit } }' "${file}" 2>/dev/null || true
}

declare -A COMPLETED_ROWS=()
if [[ "${RESUME}" -eq 1 ]]; then
    while IFS=$'\t' read -r completed_corpus completed_problem _; do
        if [[ "${completed_corpus}" != "corpus" && -n "${completed_corpus}" && -n "${completed_problem}" ]]; then
            COMPLETED_ROWS["${completed_corpus}|${completed_problem}"]=1
        fi
    done < "${REPORT}"
fi

row_number=0
while IFS=$'\t' read -r corpus problem_name canonical_name db_status db_time db_timeout; do
    row_number=$((row_number + 1))
    if [[ "${COMPLETED_ROWS["${corpus}|${problem_name}"]+present}" == "present" ]]; then
        continue
    fi
    filename="${problem_name##*/}"
    problem_path="${WORKSPACE_ROOT}/crates/mrs-bench/problems/${problem_name}"
    corpus_root="${WORKSPACE_ROOT}/crates/mrs-bench/problems/${corpus}"
    proof_root="${OUTPUT}/proofs/${corpus}"
    proof_path="${proof_root}/${filename%.p}.s"
    generation_output="${OUTPUT}/raw/${corpus}__${filename}.generation"
    strict_output="${OUTPUT}/raw/${corpus}__${filename}.strict"
    ladder_output="${OUTPUT}/raw/${corpus}__${filename}.ladder"

    if [[ ! -f "${problem_path}" ]]; then
        printf '%s\n' "missing problem: ${problem_path}" | tee -a "${RUN_LOG}" >&2
        printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
            "${corpus}" "${problem_name}" "${canonical_name}" "${db_status}" "${db_time}" "${db_timeout}" \
            "${problem_path}" "${proof_path}" "" "missing_problem" "" "not_run" "" "not_run" "" \
            >> "${REPORT}"
        continue
    fi

    # The proof header points at a local Problems/ path. Keep a symlink to the
    # edition's exact sanitized problem so mrs-proover sees the same bytes that
    # mrs used, while TPTP includes still resolve from corpus_root.
    mkdir -p "${proof_root}/Problems"
    ln -sfn "${problem_path}" "${proof_root}/Problems/${filename}"

    generation_status="no_refutation"
    generation_exit=0
    strict_status="not_run"
    strict_detail=""
    ladder_status="not_run"
    ladder_detail=""
    proof_sha256=""

    printf '[audit %s/%s] %s %s\n' "${row_number}" "${actual_rows}" "${corpus}" "${filename}" | tee -a "${RUN_LOG}"

    # Use the database timeout for regeneration. The local corpus is selected
    # explicitly; no external TPTP path is consulted.
    set +e
    TPTP="${corpus_root}" RUST_MIN_STACK=67108864 \
        timeout --foreground "$((db_timeout + 5))" \
        "${MRS_BIN}" --time "${db_timeout}" --workers 8 --schedule casc_ueq "${problem_path}" \
        > "${generation_output}" 2>&1
    generation_exit=$?
    set -e

    generation_szs="$(extract_szs "${generation_output}")"
    if [[ "${generation_szs}" == "Theorem" || "${generation_szs}" == "Unsatisfiable" ]] \
        && command grep -q '\$false' "${generation_output}"; then
        # mrs-proover requires the proof header to resolve relative to the
        # proof directory and to match every leaf's file(...) provenance. MRS
        # emits the absolute invocation path, so normalize only the header and
        # leaf source strings to the local Problems/ path. The TSTP formulas,
        # annotations, and inference DAG are otherwise byte-for-byte preserved.
        proof_source="Problems/${filename}"
        sed -E \
            -e "s#^% Proof : .*#% Proof : ${proof_source}#" \
            -e "s#file\('[^']*',#file('${proof_source}',#g" \
            "${generation_output}" > "${proof_path}"
        proof_sha256="$(sha256sum "${proof_path}" | cut -d ' ' -f 1)"
        generation_status="refutation"

        set +e
        TPTP="${corpus_root}" RUST_MIN_STACK=67108864 \
            timeout --foreground "$((STRICT_TIME + 5))" \
            "${PROOVER_BIN}" --strict --workers 1 --time "${STRICT_TIME}" \
            --problems-dir "${proof_root}" "${proof_path}" \
            > "${strict_output}" 2>&1
        set -e
        strict_status="$(extract_szs "${strict_output}")"
        [[ -n "${strict_status}" ]] || strict_status="Unknown"
        strict_detail="$(extract_detail "${strict_output}")"

        set +e
        TPTP="${corpus_root}" RUST_MIN_STACK=67108864 \
            timeout --foreground "$((LADDER_TIME + 5))" \
            "${PROOVER_BIN}" --workers 8 --time "${LADDER_TIME}" \
            --eprover "${EPROVER_BIN}" --vampire "${VAMPIRE_BIN}" \
            --problems-dir "${proof_root}" "${proof_path}" \
            > "${ladder_output}" 2>&1
        set -e
        ladder_status="$(extract_szs "${ladder_output}")"
        [[ -n "${ladder_status}" ]] || ladder_status="Unknown"
        ladder_detail="$(extract_detail "${ladder_output}")"
    fi

    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
        "${corpus}" "${problem_name}" "${canonical_name}" "${db_status}" "${db_time}" "${db_timeout}" \
        "${problem_path}" "${proof_path}" "${proof_sha256}" "${generation_status}" "${generation_exit}" \
        "${strict_status}" "$(csv_field "${strict_detail}")" "${ladder_status}" "$(csv_field "${ladder_detail}")" \
        >> "${REPORT}"

    printf '[audit %s/%s] generated=%s strict=%s ladder=%s\n' \
        "${row_number}" "${actual_rows}" "${generation_status}" "${strict_status}" "${ladder_status}" \
        | tee -a "${RUN_LOG}"

    if [[ "${STOP_ON_BAD}" -eq 1 \
        && ( "${strict_status}" == "VerifiedBad" || "${ladder_status}" == "VerifiedBad" ) ]]; then
        printf '[audit] stopping at first VerifiedBad: %s/%s\n' "${corpus}" "${filename}" \
            | tee -a "${RUN_LOG}"
        break
    fi
done < "${MANIFEST}"

printf '\n[audit] summary\n' | tee -a "${RUN_LOG}"
awk -F '\t' '
    NR == 1 { next }
    { generation[$10]++; strict[$12]++; ladder[$14]++ }
    END {
        printf "generation\n"
        for (k in generation) printf "  %s=%d\n", k, generation[k]
        printf "strict\n"
        for (k in strict) printf "  %s=%d\n", k, strict[k]
        printf "ladder\n"
        for (k in ladder) printf "  %s=%d\n", k, ladder[k]
    }
' "${REPORT}" | tee -a "${RUN_LOG}"

printf '[audit] report: %s\n' "${REPORT}"
