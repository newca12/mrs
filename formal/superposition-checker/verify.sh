#!/usr/bin/env bash
set -euo pipefail

if [[ -n "${VERUS:-}" ]]; then
  VERUS_BIN="$VERUS"
else
  VERUS_VERSION="$(python3 -c 'import json, pathlib, sys; print(json.loads(pathlib.Path(sys.argv[1]).read_text())["verus"])' \
    "$(dirname "$0")/verus-toolchain.json")"
  VERUS_BIN="${VERUS_INSTALL_DIR:-$HOME/.local/opt/mrs-verus}/$VERUS_VERSION/verus"
fi

TOOLCHAIN_HOME="${RUSTUP_HOME:-${XDG_CACHE_HOME:-$HOME/.cache}/mrs-verus/rustup}"
export RUSTUP_HOME="$TOOLCHAIN_HOME"
export RUSTUP_TOOLCHAIN="$(python3 -c 'import json, pathlib, sys; print(json.loads(pathlib.Path(sys.argv[1]).read_text())["rust_toolchain"])' \
  "$(dirname "$0")/verus-toolchain.json")"

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
  printf '%s\n' "$output" | python3 -c '
import re, sys
text = sys.stdin.read()
match = re.search(r"Toolchain:\s+([^\s]+)", text)
if not match:
    raise SystemExit("could not determine Verus effective toolchain")
print(match.group(1))
'
}

if [[ -n "${VERUS_LOG:-}" ]]; then
  : > "$VERUS_LOG"
  printf 'verification_source_commit=%s\n' "$(git -C "$(dirname "$0")/../.." rev-parse HEAD)" | tee -a "$VERUS_LOG"
  printf 'verus_binary_sha256=%s\n' "$(sha256sum "$VERUS_BIN" | cut -d' ' -f1)" | tee -a "$VERUS_LOG"
  printf 'rust_toolchain_requested=%s\n' "$(requested_rust_toolchain)" | tee -a "$VERUS_LOG"
  printf 'rust_toolchain_effective=%s\n' "$(effective_rust_toolchain)" | tee -a "$VERUS_LOG"
fi

run_check "$VERUS_BIN" --version
run_check python3 "$(dirname "$0")/audit-verus-source.py"
run_check python3 -m json.tool "$(dirname "$0")/supported-rules.json" >/dev/null
run_check "$VERUS_BIN" --no-cheating --triggers-mode silent "$(dirname "$0")/target_rigidity.rs"
run_check "$VERUS_BIN" --no-cheating --triggers-mode silent "$(dirname "$0")/first_order_core.rs"
run_check "$VERUS_BIN" --no-cheating --triggers-mode silent "$(dirname "$0")/superposition_checker.rs"
run_check "$VERUS_BIN" --no-cheating --triggers-mode silent "$(dirname "$0")/variable_superposition.rs"
run_check python3 "$(dirname "$0")/runtime-check.py" "$VERUS_BIN" \
  "$(dirname "$0")/first_order_core.rs"
run_check python3 "$(dirname "$0")/mutation-check.py" "$VERUS_BIN" \
  "$(dirname "$0")/first_order_core.rs"
