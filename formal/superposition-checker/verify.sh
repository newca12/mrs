#!/usr/bin/env bash
set -euo pipefail

if [[ -n "${VERUS:-}" ]]; then
  VERUS_BIN="$VERUS"
else
  VERUS_VERSION="$(jq -r '.verus' "$(dirname "$0")/verus-toolchain.json")"
  VERUS_BIN="${VERUS_INSTALL_DIR:-$HOME/.local/opt/mrs-verus}/$VERUS_VERSION/verus"
fi

TOOLCHAIN_HOME="${RUSTUP_HOME:-${XDG_CACHE_HOME:-$HOME/.cache}/mrs-verus/rustup}"
export RUSTUP_HOME="$TOOLCHAIN_HOME"
export RUSTUP_TOOLCHAIN="$(jq -r '.rust_toolchain' "$(dirname "$0")/verus-toolchain.json")"

run_check() {
  if [[ -n "${VERUS_LOG:-}" ]]; then
    "$@" 2>&1 | tee -a "$VERUS_LOG"
  else
    "$@"
  fi
}

if [[ -n "${VERUS_LOG:-}" ]]; then
  : > "$VERUS_LOG"
  printf 'verification_source_commit=%s\n' "$(git -C "$(dirname "$0")/../.." rev-parse HEAD)" | tee -a "$VERUS_LOG"
  printf 'verus_binary_sha256=%s\n' "$(sha256sum "$VERUS_BIN" | cut -d' ' -f1)" | tee -a "$VERUS_LOG"
  printf 'rust_toolchain=%s\n' "$RUSTUP_TOOLCHAIN" | tee -a "$VERUS_LOG"
fi

run_check "$VERUS_BIN" --version
run_check python3 "$(dirname "$0")/audit-verus-source.py"
run_check "$VERUS_BIN" --triggers-mode silent "$(dirname "$0")/target_rigidity.rs"
run_check "$VERUS_BIN" --triggers-mode silent "$(dirname "$0")/first_order_core.rs"
run_check "$VERUS_BIN" --triggers-mode silent "$(dirname "$0")/superposition_checker.rs"
run_check "$VERUS_BIN" --triggers-mode silent "$(dirname "$0")/variable_superposition.rs"
