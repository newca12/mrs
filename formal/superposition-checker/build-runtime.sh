#!/usr/bin/env bash
set -euo pipefail
ROOT="$(git rev-parse --show-toplevel)"
OUTPUT="${1:?usage: build-runtime.sh OUTPUT}"
VERSION="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["verus"])' "$ROOT/formal/superposition-checker/verus-toolchain.json")"
VERUS_BIN="${VERUS:-${VERUS_INSTALL_DIR:-$HOME/.local/opt/mrs-verus}/$VERSION/verus}"
export RUSTUP_HOME="${RUSTUP_HOME:-${XDG_CACHE_HOME:-$HOME/.cache}/mrs-verus/rustup}"
export RUSTUP_TOOLCHAIN="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["rust_toolchain"])' "$ROOT/formal/superposition-checker/verus-toolchain.json")"
python3 "$ROOT/formal/superposition-checker/audit-verus-source.py"
"$VERUS_BIN" --no-cheating --triggers-mode silent --compile \
    "$ROOT/formal/superposition-checker/first_order_core.rs" -o "$OUTPUT"
sha256sum "$OUTPUT"
python3 - "$ROOT" "$OUTPUT" <<'PY'
import hashlib, json, pathlib, subprocess, sys
root, binary = map(pathlib.Path, sys.argv[1:])
formal = root / "formal/superposition-checker"
evidence = {
    "protocol_version": 1,
    "source_commit": subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True).strip(),
    "source_dirty": bool(subprocess.check_output(["git", "-C", str(root), "diff", "HEAD", "--", str(formal)])),
    "core_sha256": hashlib.sha256((formal / "first_order_core.rs").read_bytes()).hexdigest(),
    "artifact_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
    "toolchain": json.loads((formal / "verus-toolchain.json").read_text()),
    "supported_rules": json.loads((formal / "supported-rules.json").read_text()),
    "verified_build_flags": ["--no-cheating", "--triggers-mode", "silent", "--compile"],
    "unverified_boundary": ["TPTP parser", "Rust lowering", "wire decoder", "process I/O"],
}
pathlib.Path(str(binary) + ".json").write_text(json.dumps(evidence, indent=2) + "\n")
PY
