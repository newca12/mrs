#!/usr/bin/env bash
# crates/mrs-bench/run_strategy_sweep.sh
#
# Run all mrs strategies (s01–s15) as independent "systems" on a set of
# CASC problems and produce a single run.csv for greedy_set_cover.
#
# This is the first step in the per-division portfolio optimisation workflow:
#
#   Step 1 — Generate per-strategy coverage data (this script):
#     ./crates/mrs-bench/run_strategy_sweep.sh --divisions fne --time 30
#
#   Step 2 — Find the K most complementary strategies per division:
#     ./target/release/greedy_set_cover results/.../run.csv 8 --division fne
#
#   Step 3 — Compare with the generic casc portfolio:
#     casc.sh --systems mrs --divisions fne --casc-times ...
#
#   Step 4 — If greedy portfolio > generic casc, update casc_fne in named.rs.
#            See AGENTS.md §"CASC Hardware & --casc Decision Rule" for details.
#
# Usage:
#   run_strategy_sweep.sh [OPTIONS]
#
# Options are forwarded to casc.sh.  The --systems argument is fixed to
# mrs-s01 through mrs-s15 and must not be specified separately.
#
# Example:
#   # FNE at 30 s, 4 parallel jobs:
#   ./crates/mrs-bench/run_strategy_sweep.sh \
#       --divisions fne --time 30 --jobs 4
#
#   # All main divisions at CASC times:
#   ./crates/mrs-bench/run_strategy_sweep.sh \
#       --divisions fne,feq,ueq,eps --casc-times --jobs 4
#
# The output directory is printed at the end; pass its run.csv to
# greedy_set_cover.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Build the strategy system list (s01..s15)
SYSTEMS=$(seq 1 15 | awk '{printf "mrs-s%02d%s", $1, (NR<15?",":"")}')

exec bash "${SCRIPT_DIR}/casc.sh" --systems "${SYSTEMS}" "$@"
