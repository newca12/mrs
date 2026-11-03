#!/usr/bin/env bash
# Bash entry point for the Rust CASC canary audit.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/tools.sh"

exec mrs_tools check-canary-drift "${1:?usage: $0 RUN.csv}"
