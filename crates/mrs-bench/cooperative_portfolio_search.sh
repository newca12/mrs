#!/usr/bin/env bash
# Bash entry point for the Rust cooperative portfolio optimizer.
#
# Usage:
#   cooperative_portfolio_search.sh <edition> <division> [time] [jobs] [rounds] [output]
#
# Candidate generation, run.csv scoring, caching, and one-swap search live in
# `mrs-tools`; this wrapper preserves the benchmark harness's shell interface.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/tools.sh"

exec mrs_tools cooperative-portfolio-search "$@"
