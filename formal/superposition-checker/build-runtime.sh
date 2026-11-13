#!/usr/bin/env bash
set -euo pipefail
ROOT="$(git rev-parse --show-toplevel)"
source "$ROOT/formal/superposition-checker/tools.sh"
formal_tools_setup
OUTPUT="${1:?usage: build-runtime.sh OUTPUT}"
VERSION="$(mrs_tools verus-pin-field "$ROOT/formal/superposition-checker/verus-toolchain.json" verus)"
VERUS_BIN="${VERUS:-${VERUS_INSTALL_DIR:-$HOME/.local/opt/mrs-verus}/$VERSION/verus}"
export RUSTUP_HOME="${RUSTUP_HOME:-${XDG_CACHE_HOME:-$HOME/.cache}/mrs-verus/rustup}"
export RUSTUP_TOOLCHAIN="$(mrs_tools verus-pin-field "$ROOT/formal/superposition-checker/verus-toolchain.json" rust_toolchain)"
if command -v nproc >/dev/null 2>&1; then
    DETECTED_THREADS="$(nproc)"
elif command -v getconf >/dev/null 2>&1; then
    DETECTED_THREADS="$(getconf _NPROCESSORS_ONLN)"
else
    DETECTED_THREADS=1
fi
if [[ ! "$DETECTED_THREADS" =~ ^[1-9][0-9]*$ ]]; then
    DETECTED_THREADS=1
fi
VERUS_THREADS="${VERUS_THREADS:-$DETECTED_THREADS}"
if [[ ! "$VERUS_THREADS" =~ ^[1-9][0-9]*$ ]]; then
    echo "VERUS_THREADS must be a positive integer, got: $VERUS_THREADS" >&2
    exit 2
fi
printf 'available_cpu_threads=%s verus_threads=%s\n' "$DETECTED_THREADS" "$VERUS_THREADS"
mrs_tools audit-verus-source --source-dir "$ROOT/formal/superposition-checker"
mrs_tools verus-check-version "$ROOT/formal/superposition-checker/verus-toolchain.json" "$VERUS_BIN"
"$VERUS_BIN" --no-cheating --triggers-mode silent --num-threads "$VERUS_THREADS" --rlimit 30 --compile \
    "$ROOT/formal/superposition-checker/first_order_core.rs" -o "$OUTPUT"
sha256sum "$OUTPUT"
mrs_tools verus-runtime-evidence "$ROOT" "$OUTPUT" "$VERUS_BIN"
