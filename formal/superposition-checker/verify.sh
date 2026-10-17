#!/usr/bin/env bash
set -eu

if [[ -n "${VERUS:-}" ]]; then
  VERUS_BIN="$VERUS"
else
  VERUS_VERSION="$(jq -r '.verus' "$(dirname "$0")/verus-toolchain.json")"
  VERUS_BIN="${VERUS_INSTALL_DIR:-$HOME/.local/opt/mrs-verus}/$VERUS_VERSION/verus"
fi

TOOLCHAIN_HOME="${RUSTUP_HOME:-${XDG_CACHE_HOME:-$HOME/.cache}/mrs-verus/rustup}"
export RUSTUP_HOME="$TOOLCHAIN_HOME"
export RUSTUP_TOOLCHAIN="$(jq -r '.rust_toolchain' "$(dirname "$0")/verus-toolchain.json")"

"$VERUS_BIN" --version
python3 "$(dirname "$0")/audit-verus-source.py"
"$VERUS_BIN" --triggers-mode silent "$(dirname "$0")/target_rigidity.rs"
"$VERUS_BIN" --triggers-mode silent "$(dirname "$0")/first_order_core.rs"
"$VERUS_BIN" --triggers-mode silent "$(dirname "$0")/superposition_checker.rs"
"$VERUS_BIN" --triggers-mode silent "$(dirname "$0")/variable_superposition.rs"
