#!/usr/bin/env python3
"""Compile and execute the checked core's positive/negative DAG regressions."""

import hashlib
from pathlib import Path
import subprocess
import sys
import tempfile


def main() -> None:
    verus, source = map(Path, sys.argv[1:])
    with tempfile.TemporaryDirectory(prefix="mrs-verus-runtime-") as directory:
        binary = Path(directory) / "first-order-core"
        reference = Path(directory) / "first-order-core.reference"
        subprocess.run(
            [str(verus), "--triggers-mode", "silent", "--compile", str(source),
             "-o", str(binary)], check=True,
        )
        first_hash = hashlib.sha256(binary.read_bytes()).hexdigest()
        reference.write_bytes(binary.read_bytes())
        subprocess.run(
            [str(verus), "--triggers-mode", "silent", "--compile", str(source),
             "-o", str(binary)], check=True,
        )
        second_hash = hashlib.sha256(binary.read_bytes()).hexdigest()
        if first_hash != second_hash:
            raise RuntimeError(
                "verified runtime artifact is not byte-reproducible: "
                f"{first_hash} != {second_hash}"
            )
        if reference.read_bytes() != binary.read_bytes():
            raise RuntimeError("verified runtime artifact changed between identical builds")
        print(f"verified_runtime_sha256={second_hash}")
        subprocess.run([str(binary)], check=True, timeout=30)
    print("reproducible verified runtime and DAG regressions passed")


if __name__ == "__main__":
    main()
