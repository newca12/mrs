#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
PIN_FILE="$ROOT/formal/superposition-checker/verus-toolchain.json"
DESTINATION="${VERUS_INSTALL_DIR:-$HOME/.local/opt/mrs-verus}"
TOOLCHAIN_HOME="${RUSTUP_HOME:-${XDG_CACHE_HOME:-$HOME/.cache}/mrs-verus/rustup}"

if ! command -v jq >/dev/null 2>&1; then
  echo "install-verus.sh requires jq" >&2
  exit 1
fi

VERSION="$(jq -r '.verus' "$PIN_FILE")"
TAG="$(jq -r '.release_tag' "$PIN_FILE")"
ASSET="$(jq -r '.linux_x86_64_asset' "$PIN_FILE")"
EXPECTED_SHA="$(jq -r '.linux_x86_64_sha256' "$PIN_FILE")"
RUST_TOOLCHAIN="$(jq -r '.rust_toolchain' "$PIN_FILE")"

if [[ "$(uname -s)" != Linux || "$(uname -m)" != x86_64 ]]; then
  echo "Pinned Verus binary install currently supports Linux x86_64 only" >&2
  exit 1
fi

command -v curl >/dev/null
command -v sha256sum >/dev/null
command -v unzip >/dev/null
command -v rustup >/dev/null

mkdir -p "$DESTINATION"
TEMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/mrs-verus.XXXXXXXX")"
trap 'rm -rf "$TEMP_DIR"' EXIT

ARCHIVE="$TEMP_DIR/$ASSET"
URL="https://github.com/verus-lang/verus/releases/download/$TAG/$ASSET"
curl --fail --location --retry 3 --output "$ARCHIVE" "$URL"
echo "$EXPECTED_SHA  $ARCHIVE" | sha256sum --check --status

mkdir -p "$TOOLCHAIN_HOME"
RUSTUP_HOME="$TOOLCHAIN_HOME" rustup toolchain install "$RUST_TOOLCHAIN" --profile minimal
unzip -q "$ARCHIVE" -d "$TEMP_DIR/unpacked"
mkdir -p "$DESTINATION/$VERSION"
cp -a "$TEMP_DIR/unpacked/verus-x86-linux/." "$DESTINATION/$VERSION/"

printf 'Installed Verus %s at %s\n' "$VERSION" "$DESTINATION/$VERSION/verus"
printf 'Run with: VERUS="%s/%s/verus" formal/superposition-checker/verify.sh\n' \
  "$DESTINATION" "$VERSION"
