#!/usr/bin/env bash
# crates/mrs-bench/systems/eprover/invoke.sh
# Usage: invoke.sh <problem_path> <time_limit_secs>
# Writes all output to stdout; exits with any code.
#
# E CLI notes (binary is E 3.x; see `eprover --version` in bin/):
#   --auto-schedule    multi-strategy portfolio (CASC-style entry point)
#   --cpu-limit=N      per-invocation CPU budget in seconds
#   --tstp-format      SZS/TSTP output (gives us the `% SZS status …` line
#                      that crates/mrs-bench/casc.sh greps for)
#   --soft-cpu-limit=N optional softer cap; we let `casc.sh`'s outer
#                      `timeout` enforce the wall budget instead
#
# Proof object is intentionally NOT requested: `casc.sh` only consumes
# the SZS status line, and skipping proof emission keeps E's output
# small. Switch to fuzz_proover.sh's flags (`--auto --proof-object`)
# if you ever need the proof body.
set -euo pipefail

PROBLEM="${1:?Usage: invoke.sh <problem_path> <time_limit_secs>}"
TIME_LIMIT="${2:?Usage: invoke.sh <problem_path> <time_limit_secs>}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY="${SCRIPT_DIR}/bin/eprover"

if [[ ! -x "${BINARY}" ]]; then
    echo "% SZS status Error (eprover binary not found at ${BINARY})"
    exit 1
fi

exec "${BINARY}" --auto-schedule --cpu-limit="${TIME_LIMIT}" --tstp-format "${PROBLEM}"
