#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
source "$ROOT/formal/superposition-checker/tools.sh"
formal_tools_setup

if [[ -n "${VERUS:-}" ]]; then
  VERUS_BIN="$VERUS"
else
  VERUS_VERSION="$(mrs_tools verus-pin-field "$(dirname "$0")/verus-toolchain.json" verus)"
  VERUS_BIN="${VERUS_INSTALL_DIR:-$HOME/.local/opt/mrs-verus}/$VERUS_VERSION/verus"
fi

TOOLCHAIN_HOME="${RUSTUP_HOME:-${XDG_CACHE_HOME:-$HOME/.cache}/mrs-verus/rustup}"
export RUSTUP_HOME="$TOOLCHAIN_HOME"
export RUSTUP_TOOLCHAIN="$(mrs_tools verus-pin-field "$(dirname "$0")/verus-toolchain.json" rust_toolchain)"

run_check() {
  if [[ -n "${VERUS_LOG:-}" ]]; then
    "$@" 2>&1 | tee -a "$VERUS_LOG"
  else
    "$@"
  fi
}

requested_rust_toolchain() {
  printf '%s' "$RUSTUP_TOOLCHAIN"
}

effective_rust_toolchain() {
  local output
  output="$("$VERUS_BIN" --version 2>&1)"
  printf '%s\n' "$output" | mrs_tools verus-effective-toolchain
}

if [[ -n "${VERUS_LOG:-}" ]]; then
  : > "$VERUS_LOG"
  printf 'verification_source_commit=%s\n' "$(git -C "$(dirname "$0")/../.." rev-parse HEAD)" | tee -a "$VERUS_LOG"
  printf 'verus_binary_sha256=%s\n' "$(sha256sum "$VERUS_BIN" | cut -d' ' -f1)" | tee -a "$VERUS_LOG"
  printf 'rust_toolchain_requested=%s\n' "$(requested_rust_toolchain)" | tee -a "$VERUS_LOG"
  printf 'rust_toolchain_effective=%s\n' "$(effective_rust_toolchain)" | tee -a "$VERUS_LOG"
fi

run_check "$VERUS_BIN" --version
run_check mrs_tools audit-verus-source --source-dir "$(dirname "$0")"
run_check mrs_tools validate-json "$(dirname "$0")/supported-rules.json"
run_check "$VERUS_BIN" --no-cheating --triggers-mode silent "$(dirname "$0")/target_rigidity.rs"
run_check "$VERUS_BIN" --no-cheating --triggers-mode silent "$(dirname "$0")/first_order_core.rs"
run_check "$VERUS_BIN" --no-cheating --triggers-mode silent "$(dirname "$0")/superposition_checker.rs"
run_check "$VERUS_BIN" --no-cheating --triggers-mode silent "$(dirname "$0")/variable_superposition.rs"
run_check mrs_tools verus-runtime-check "$VERUS_BIN" \
  "$(dirname "$0")/first_order_core.rs"
run_check mrs_tools verus-mutation-check "$VERUS_BIN" \
  "$(dirname "$0")/first_order_core.rs"
