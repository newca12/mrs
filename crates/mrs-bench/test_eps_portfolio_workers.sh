#!/usr/bin/env bash
# Regression coverage for the EPS certification/portfolio worker split.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf -- "${TMP_DIR}"' EXIT

FAKE_BIN="${TMP_DIR}/bin"
mkdir -p "${FAKE_BIN}"
cat >"${FAKE_BIN}/cargo" <<'EOF'
#!/usr/bin/env bash
echo "test: portfolio validation passed" >&2
exit 73
EOF
chmod +x "${FAKE_BIN}/cargo"

cat >"${TMP_DIR}/fake-mrs" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$*" >>"${MRS_TEST_ARGS_LOG}"
echo "% SZS status GaveUp for test"
EOF
chmod +x "${TMP_DIR}/fake-mrs"

expect_result() {
    local expected_status="$1"
    local expected_output="$2"
    shift 2

    local output
    local status
    if output="$("$@" 2>&1)"; then
        status=0
    else
        status=$?
    fi

    if [[ "${status}" != "${expected_status}" ]]; then
        printf 'expected exit %s, got %s:\n%s\n' "${expected_status}" "${status}" "${output}" >&2
        return 1
    fi
    if [[ "${output}" != *"${expected_output}"* ]]; then
        printf 'expected output containing %q, got:\n%s\n' "${expected_output}" "${output}" >&2
        return 1
    fi
}

campaign="${SCRIPT_DIR}/certification_campaign.sh"
expect_result 2 \
    "EPS reserves one of MRS_WORKERS=8 for certification; its portfolio needs 7 strategies, got 8" \
    env MRS_WORKERS=8 bash "${campaign}" \
    --edition casc-30 --systems mrs --divisions eps \
    --portfolio 1,2,3,4,5,6,7,8 --output "${TMP_DIR}/invalid-eps"

expect_result 2 \
    "one explicit portfolio cannot cover EPS and non-EPS divisions" \
    env MRS_WORKERS=8 bash "${campaign}" \
    --edition casc-30 --systems mrs --divisions feq,eps \
    --portfolio 1,2,3,4,5,6,7,8 --output "${TMP_DIR}/mixed-divisions"

expect_result 73 \
    "test: portfolio validation passed" \
    env PATH="${FAKE_BIN}:${PATH}" MRS_WORKERS=8 \
    bash "${campaign}" --edition casc-30 --systems mrs --divisions eps \
    --portfolio 1,2,3,4,5,6,7 --output "${TMP_DIR}/valid-eps"

invoke="${SCRIPT_DIR}/systems/mrs/invoke.sh"
problem="${SCRIPT_DIR}/problems/casc-30/EPS/GRP123-4.004.p"
export MRS_TEST_ARGS_LOG="${TMP_DIR}/mrs-args.log"

expect_result 1 \
    "EPS portfolio has 8 strategies for 7 workers" \
    env MRS_BINARY="${TMP_DIR}/fake-mrs" MRS_WORKERS=8 \
    MRS_PORTFOLIO=1,2,3,4,5,6,7,8 \
    bash "${invoke}" "${problem}" 5

expect_result 0 \
    "portfolio status=GaveUp exit=0 workers=7+1" \
    env MRS_BINARY="${TMP_DIR}/fake-mrs" MRS_WORKERS=8 \
    MRS_PORTFOLIO=1,2,3,4,5,6,7 \
    bash "${invoke}" "${problem}" 5

if [[ "$(wc -l <"${MRS_TEST_ARGS_LOG}")" -ne 2 ]] \
    || ! grep -Fq -- '--workers 7' "${MRS_TEST_ARGS_LOG}" \
    || ! grep -Fq -- '--portfolio 1,2,3,4,5,6,7' "${MRS_TEST_ARGS_LOG}"; then
    echo "EPS invocation did not pass the seven-ID portfolio to seven workers" >&2
    cat "${MRS_TEST_ARGS_LOG}" >&2
    exit 1
fi

echo "EPS portfolio worker contract: ok"
