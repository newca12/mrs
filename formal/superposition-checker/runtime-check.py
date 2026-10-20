#!/usr/bin/env python3
"""Compile and execute the checked core's positive/negative DAG regressions."""

import hashlib
import os
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
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile", str(source),
             "-o", str(binary)], check=True,
        )
        first_hash = hashlib.sha256(binary.read_bytes()).hexdigest()
        reference.write_bytes(binary.read_bytes())
        subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile", str(source),
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
        # A minimal p, ~p -> empty proof, decoded from bytes rather than the
        # in-memory regression constructors. Every malformed suffix/prefix must
        # suppress the positive response.
        request = "1 2 100000 3 2 2 1 1 0 9 1 0 0 9 " \
                  "0 1 1 0 9 0 0 1 1 0 0 9 0 0 2 0 2 2 0 1 0 0"
        def check_wire(payload: bytes, expected: bytes) -> None:
            result = subprocess.run([str(binary), "--wire-v1"], input=payload,
                                    stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                    check=True, timeout=10)
            if result.stdout != expected:
                raise RuntimeError(f"wire response mismatch: {result.stdout!r} != {expected!r}")
        check_wire(request.encode(), b"MRS_VERUS 1 CertifiedIR\n")
        for malformed in [request + " 0", request.replace("1 2", "2 2", 1),
                          request.replace("3 2 2", "513 2 2", 1),
                          "1 1 100000 1 2", "1 99 10", "-1 0 0", "1 0 0 extra"]:
            check_wire(malformed.encode(), b"MRS_VERUS 1 Unknown\n")
        for end in range(len(request.split())):
            check_wire(" ".join(request.split()[:end]).encode(), b"MRS_VERUS 1 Unknown\n")
        check_wire(b"1 0 0 \xff", b"MRS_VERUS 1 Unknown\n")
        check_wire(b"0" * (4 * 1024 * 1024 + 1), b"MRS_VERUS 1 Unknown\n")
        if os.environ.get("MRS_VERUS_BRIDGE_TESTS") == "1":
            env = dict(os.environ, MRS_VERUS_TEST_BINARY=str(binary),
                       MRS_VERUS_TEST_SHA256=second_hash)
            subprocess.run(["cargo", "test", "-p", "mrs-proof-kernel", "--test", "verus_bridge",
                            "--", "--include-ignored"], env=env, check=True, timeout=300)
            proof = source.parent / "fixtures/superposition-refutation-proof.p"
            for mode, sha, expected in [("--verus-shadow", second_hash, b"VerifiedGood"),
                                        ("--verus-gate", second_hash, b"VerifiedGood"),
                                        ("--verus-gate", "0" * 64, b"Unknown")]:
                cli = subprocess.run(["cargo", "run", "-p", "mrs-proover", "--bin", "mrs-proover",
                                      "--", mode, str(binary), "--verus-sha256", sha, str(proof)],
                                     stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                     check=True, timeout=300)
                if expected not in cli.stdout:
                    raise RuntimeError(f"ProoVer CLI bridge failed: {cli.stdout!r} {cli.stderr!r}")
    print("reproducible verified runtime and DAG regressions passed")


if __name__ == "__main__":
    main()
