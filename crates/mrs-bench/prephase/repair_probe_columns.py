#!/usr/bin/env python3
"""Repair feature CSVs written with the original missing probe separator.

The legacy `prephase_dump --probe` writer concatenated the first probe value to
the final `label` column. The remaining probe values were consequently shifted
left, and `probe_throughput_per_ms` was missing. This tool restores the static
label from its five class columns and shifts the probe fields back into place.

Usage: repair_probe_columns.py BROKEN.csv > repaired.csv
"""

from __future__ import annotations

import csv
import sys

CLASS_FIELDS = (
    "logic_class",
    "shape_class",
    "scale_class",
    "goal_class",
    "decomposition_class",
)
PROBE_FIELDS = (
    "probe_iterations",
    "probe_generated",
    "probe_processed",
    "probe_forward_subsumed",
    "probe_weight_discarded",
    "probe_passive",
    "probe_elapsed_ms",
    "probe_generation_rate",
    "probe_redundancy_rate",
    "probe_throughput_per_ms",
)
RECOVERABLE = {"ok", "empty_after_lowering", "include_error"}


def repair_row(row: dict[str, str | None]) -> str:
    """Repair one row in place; return `repaired`, `already_fixed`, or `blank`."""
    if row.get(None) is not None:
        raise ValueError(f"row has unexpected extra CSV fields: {row.get('path')}")

    label = row.get("label") or ""
    if row.get("parse_status") not in RECOVERABLE:
        for field in PROBE_FIELDS:
            row[field] = ""
        return "blank"

    static_label = "/".join(row[field] or "" for field in CLASS_FIELDS)
    if label == static_label and row.get("probe_throughput_per_ms") not in (None, ""):
        return "already_fixed"

    if not label.startswith(static_label):
        raise ValueError(f"label does not match class columns for {row.get('name')}")
    iterations = label[len(static_label) :]
    if not iterations.isdecimal():
        raise ValueError(f"cannot recover probe iteration suffix for {row.get('name')}")

    # The broken row had no comma between `label` and `probe_iterations`:
    # label_suffix, old probe_iterations, old probe_generated, ...
    old = {field: row.get(field) or "" for field in PROBE_FIELDS}
    if not old["probe_redundancy_rate"]:
        raise ValueError(f"missing shifted throughput value for {row.get('name')}")
    row["label"] = static_label
    row["probe_iterations"] = iterations
    for destination, source in zip(PROBE_FIELDS[1:], PROBE_FIELDS[:-1], strict=True):
        row[destination] = old[source]
    row["probe_throughput_per_ms"] = old["probe_redundancy_rate"]
    return "repaired"


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__, file=sys.stderr)
        return 1

    repaired = blank = already_fixed = 0
    with open(sys.argv[1], newline="") as source:
        reader = csv.DictReader(source)
        if reader.fieldnames is None or any(field not in reader.fieldnames for field in PROBE_FIELDS):
            raise SystemExit("input CSV does not contain all probe columns")
        writer = csv.DictWriter(sys.stdout, fieldnames=reader.fieldnames)
        writer.writeheader()
        for row in reader:
            action = repair_row(row)
            if action == "repaired":
                repaired += 1
            elif action == "blank":
                blank += 1
            else:
                already_fixed += 1
            writer.writerow(row)

    print(
        f"repaired={repaired} blank_probe_rows={blank} already_fixed={already_fixed}",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
