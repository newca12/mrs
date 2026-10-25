#!/usr/bin/env python3
"""Render identity-bound audit snapshots as a certification evidence dashboard.

Pass snapshot directories explicitly. Distinct campaigns/checker revisions are
never pooled into a HEAD score. Output goes to stdout; redirect it to a report.
"""

import argparse
from collections import Counter, defaultdict
import csv
import hashlib
import json
from pathlib import Path


def read_snapshot(directory):
    identity = json.loads((directory / "audit-identity.json").read_text())
    data = (directory / "audit.csv").read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    if directory.name != digest:
        raise ValueError(f"snapshot content digest mismatch: {directory}")
    required = ("audit_binary_sha256", "proover_binary_sha256", "run_csv_sha256", "corpus_sha256")
    if identity.get("schema") != 1 or any(not identity.get(key) for key in required):
        raise ValueError(f"incomplete snapshot identity: {directory}")
    groups = defaultdict(Counter)
    seen = set()
    for row in csv.DictReader(data.decode().splitlines()):
        key = tuple(row[field] for field in ("edition", "division", "problem", "system"))
        if key in seen:
            raise ValueError(f"duplicate audit outcome: {key}")
        seen.add(key)
        status = row["strict_status"] or "not_run"
        groups[(row["edition"], row["division"], row["system"])][status] += 1
    if not seen:
        raise ValueError(f"empty snapshot: {directory}")
    return identity, digest, groups


def render(directories):
    lines = ["# Certification evidence dashboard", "",
             "Each section is one immutable audit snapshot. Counts are not a HEAD baseline,",
             "and zero rejected proofs is not a global soundness theorem.", ""]
    seen = set()
    for directory in directories:
        identity, digest, groups = read_snapshot(directory)
        key = (json.dumps(identity, sort_keys=True), digest)
        if key in seen:
            raise ValueError("duplicate evidence snapshot supplied")
        seen.add(key)
        lines += [f"## Snapshot `{digest}`", "",
                  f"- Artifact directory: `{directory}`",
                  f"- Run CSV: `{identity['run_csv_sha256']}`",
                  f"- Auditor: `{identity['audit_binary_sha256']}`",
                  f"- Checker: `{identity['proover_binary_sha256']}`",
                  f"- Corpus/includes: `{identity['corpus_sha256']}`",
                  f"- Strict budget: {identity.get('strict_time', '?')} s; jobs: {identity.get('jobs', '?')}",
                  "", "| Edition | Division | System | Rows | Strict certified | Rejected | Other outcomes |",
                  "|---|---|---|---:|---:|---:|---|"]
        for (edition, division, system), counts in sorted(groups.items()):
            other = "; ".join(f"{status}: {count}" for status, count in sorted(counts.items())
                              if status not in ("VerifiedGood", "VerifiedBad")) or "—"
            lines.append(f"| {edition} | {division} | {system} | {sum(counts.values())} | "
                         f"{counts['VerifiedGood']} | {counts['VerifiedBad']} | {other} |")
        lines.append("")
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("snapshots", nargs="+", type=Path)
    args = parser.parse_args()
    try:
        print(render(args.snapshots))
    except (OSError, ValueError, KeyError) as error:
        parser.exit(1, f"dashboard: {error}\n")


if __name__ == "__main__":
    main()
