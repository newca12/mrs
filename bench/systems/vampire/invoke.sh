#!/usr/bin/env bash
# bench/systems/vampire/invoke.sh
# Usage: invoke.sh <problem_path> <time_limit_secs>
# Writes all output to stdout; exits with any code.
#
# Vampire CLI notes (adjust flags to match your binary version):
#   Vampire 4.x:  --mode casc --time_limit <N>
#   Vampire 4.9+: --mode casc -t <N>   (--time_limit also accepted)
# The flags below work for Vampire 4.x/4.9. If your binary uses different
# flags, edit the exec line at the bottom.
set -euo pipefail

PROBLEM="${1:?Usage: invoke.sh <problem_path> <time_limit_secs>}"
TIME_LIMIT="${2:?Usage: invoke.sh <problem_path> <time_limit_secs>}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY="${SCRIPT_DIR}/bin/vampire"

if [[ ! -x "${BINARY}" ]]; then
    echo "% SZS status Error (vampire binary not found at ${BINARY})"
    exit 1
fi

exec "${BINARY}" --mode casc --time_limit "${TIME_LIMIT}" "${PROBLEM}"
