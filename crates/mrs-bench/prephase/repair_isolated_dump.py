#!/usr/bin/env python3
"""Repair a `prephase_dump --rlimit-mb` CSV produced before the child/parent
field-count fix.

The isolated path re-executes `prephase_dump --child`, and the child used to
print three fields (path, status, analysis row) while the parent split off four
(path, status, detail, analysis row). The parent's `detail` was then
overwritten with an empty string, so the analysis row lost its first column --
`name` -- and every later column shifted one to the left. The row still had the
right *number* of values, which is why the misalignment was silent: the header
and every row agreed in width and disagreed in meaning.

This script re-inserts `name` at column 3. `name` is the path's file stem, so it
is recovered exactly rather than guessed. Applied to a dump taken with the fixed
binary it is a no-op, because such a dump already has the right column count.

Usage: repair_isolated_dump.py BROKEN.csv > FIXED.csv
"""

import csv
import os
import sys

RECOVERABLE = ("ok", "empty_after_lowering")


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__, file=sys.stderr)
        return 1
    reader = csv.reader(open(sys.argv[1], newline=""))
    writer = csv.writer(sys.stdout)
    header = next(reader)
    writer.writerow(header)
    recovered = degraded = 0
    for row in reader:
        if len(row) == len(header):
            writer.writerow(row)
            continue
        if len(row) != len(header) - 1:
            raise SystemExit(f"unexpected row width {len(row)} at {row[0]}")
        status = row[1]
        if status in RECOVERABLE:
            name = os.path.splitext(os.path.basename(row[0]))[0]
            recovered += 1
        else:
            # The child's `detail` text (a parse error) is not recoverable. The
            # analysis columns are `empty_row` for these rows regardless, so only
            # the diagnostic string is degraded.
            name = ""
            degraded += 1
        writer.writerow([row[0], status, "", name] + row[3:])
    print(f"recovered name: {recovered}, detail text lost: {degraded}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
