#!/usr/bin/env python3
"""Fail the formal gate on unchecked proof escape hatches."""

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parent
ESCAPE_HATCHES = re.compile(
    r"#\[(?:verifier::(?:external_body|allow)|verus::external_body)|\b(?:assume|admit)\s*\("
)

failures = []
for source in sorted(ROOT.glob("*.rs")):
    for line_number, line in enumerate(source.read_text(encoding="utf-8").splitlines(), 1):
        if ESCAPE_HATCHES.search(line):
            failures.append(f"{source.name}:{line_number}: unchecked proof escape hatch")

if failures:
    print("\n".join(failures), file=sys.stderr)
    sys.exit(1)
