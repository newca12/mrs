#!/usr/bin/env sh
set -eu

VERUS=${VERUS:-verus}
"$VERUS" --version
"$VERUS" --triggers-mode silent "$(dirname "$0")/target_rigidity.rs"
"$VERUS" --triggers-mode silent "$(dirname "$0")/first_order_core.rs"
"$VERUS" --triggers-mode silent "$(dirname "$0")/superposition_checker.rs"
