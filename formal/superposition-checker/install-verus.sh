#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
PIN_FILE="$ROOT/formal/superposition-checker/verus-toolchain.json"
DESTINATION="${VERUS_INSTALL_DIR:-$HOME/.local/opt/mrs-verus}"
TOOLCHAIN_HOME="${RUSTUP_HOME:-${XDG_CACHE_HOME:-$HOME/.cache}/mrs-verus/rustup}"

json_field() {
  python3 -c 'import json, pathlib, sys; print(json.loads(pathlib.Path(sys.argv[1]).read_text())[sys.argv[2]])' \
    "$PIN_FILE" "$1"
}

VERSION="$(json_field verus)"
TAG="$(json_field release_tag)"
ASSET="$(json_field linux_x86_64_asset)"
EXPECTED_SHA="$(json_field linux_x86_64_sha256)"
RUST_TOOLCHAIN="$(json_field rust_toolchain)"

if [[ "$(uname -s)" != Linux || "$(uname -m)" != x86_64 ]]; then
  echo "Pinned Verus binary install currently supports Linux x86_64 only" >&2
  exit 1
fi

command -v curl >/dev/null
command -v sha256sum >/dev/null
command -v unzip >/dev/null
command -v rustup >/dev/null
command -v python3 >/dev/null

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
