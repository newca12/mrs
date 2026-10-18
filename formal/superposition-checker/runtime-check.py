#!/usr/bin/env python3
"""Compile and execute the checked core's positive/negative DAG regressions."""

from pathlib import Path
import subprocess
import sys
import tempfile


def main() -> None:
    verus, source = map(Path, sys.argv[1:])
    with tempfile.TemporaryDirectory(prefix="mrs-verus-runtime-") as directory:
        binary = Path(directory) / "first-order-core"
        subprocess.run(
            [str(verus), "--triggers-mode", "silent", "--compile", str(source),
             "-o", str(binary)], check=True,
        )
        subprocess.run([str(binary)], check=True, timeout=30)
    print("DAG certification runtime regressions passed")


if __name__ == "__main__":
    main()
