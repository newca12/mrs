#!/usr/bin/env python3
"""bench/report.py — Summarise a CASC benchmark run.

Usage:
    bench/report.py <run.csv> [--min-systems <N>]

Output:
    Per-division solved/avg-time table, cross-system disagreements, and
    polarity violations (wrong SZS polarity for a known division type).

CSV schema (produced by bench/casc.sh):
    edition,division,problem,system,szs_status,wall_time_s
"""

import argparse
import csv
import sys
from collections import defaultdict
from datetime import datetime
from pathlib import Path

# SZS statuses that count as "solved"
SOLVED = {"Theorem", "Unsatisfiable", "CounterSatisfiable", "Satisfiable"}

# Pairs of SZS statuses that are logically contradictory
CONTRADICTIONS = {
    frozenset({"Theorem", "CounterSatisfiable"}),
    frozenset({"Theorem", "Satisfiable"}),
    frozenset({"Unsatisfiable", "CounterSatisfiable"}),
    frozenset({"Unsatisfiable", "Satisfiable"}),
}

# Expected SZS polarity for divisions where the answer is known a priori.
# Key: lower-case division name.  Value: set of acceptable solved statuses.
DIVISION_POLARITY = {
    "epu": {"Unsatisfiable"},
    "ueq": {"Unsatisfiable"},
    "eps": {"Satisfiable", "CounterSatisfiable"},  # CNF, no conjecture
}


def load_csv(path: Path):
    rows = []
    with path.open(newline="") as f:
        reader = csv.DictReader(f)
        for row in reader:
            try:
                row["wall_time_s"] = float(row["wall_time_s"])
            except (ValueError, KeyError):
                row["wall_time_s"] = 0.0
            rows.append(row)
    return rows


def main():
    parser = argparse.ArgumentParser(
        description="Summarise a CASC benchmark run CSV."
    )
    parser.add_argument("csv", type=Path, help="Path to run.csv")
    parser.add_argument(
        "--min-systems",
        type=int,
        default=1,
        metavar="N",
        help="Only report disagreements when at least N systems solved the problem "
             "(default: 1, i.e. report all contradictions)",
    )
    args = parser.parse_args()

    if not args.csv.exists():
        print(f"Error: file not found: {args.csv}", file=sys.stderr)
        sys.exit(1)

    rows = load_csv(args.csv)
    if not rows:
        print("CSV is empty.", file=sys.stderr)
        sys.exit(1)

    edition = rows[0].get("edition", "unknown")

    # ---- index: data[(division, problem, system)] = (szs_status, wall_time_s)
    data: dict[tuple[str, str, str], tuple[str, float]] = {}
    systems_seen: set[str] = set()
    divisions_seen: list[str] = []
    _div_order: dict[str, int] = {}

    for row in rows:
        div = row["division"].lower()
        prob = row["problem"]
        sys_name = row["system"]
        szs = row["szs_status"]
        t = row["wall_time_s"]
        data[(div, prob, sys_name)] = (szs, t)
        systems_seen.add(sys_name)
        if div not in _div_order:
            _div_order[div] = len(_div_order)
            divisions_seen.append(div)

    systems = sorted(systems_seen)
    # Collect all (div, problem) pairs
    div_problems: dict[str, set[str]] = defaultdict(set)
    for (div, prob, _sys) in data:
        div_problems[div].add(prob)

    # ---- per-division stats
    # stats[(div, sys)] = (solved_count, total_time_of_solved)
    stats: dict[tuple[str, str], tuple[int, float]] = defaultdict(lambda: (0, 0.0))

    for div in divisions_seen:
        for prob in div_problems[div]:
            for sys_name in systems:
                entry = data.get((div, prob, sys_name))
                if entry is None:
                    continue
                szs, t = entry
                if szs in SOLVED:
                    cnt, total = stats[(div, sys_name)]
                    stats[(div, sys_name)] = (cnt + 1, total + t)

    # ---- detect disagreements
    disagreements: list[tuple[str, str, dict[str, str]]] = []
    for div in divisions_seen:
        for prob in sorted(div_problems[div]):
            solved_by: dict[str, str] = {}
            for sys_name in systems:
                entry = data.get((div, prob, sys_name))
                if entry and entry[0] in SOLVED:
                    solved_by[sys_name] = entry[0]
            if len(solved_by) < 2:
                continue
            statuses = set(solved_by.values())
            if len(statuses) > 1:
                # Check for actual logical contradiction
                for s1 in statuses:
                    for s2 in statuses:
                        if frozenset({s1, s2}) in CONTRADICTIONS:
                            disagreements.append((div, prob, solved_by))
                            break
                    else:
                        continue
                    break

    # ---- detect polarity violations
    polarity_violations: list[tuple[str, str, str, str, str]] = []
    for div in divisions_seen:
        expected = DIVISION_POLARITY.get(div)
        if expected is None:
            continue
        for prob in sorted(div_problems[div]):
            for sys_name in systems:
                entry = data.get((div, prob, sys_name))
                if entry is None:
                    continue
                szs, _ = entry
                if szs in SOLVED and szs not in expected:
                    polarity_violations.append(
                        (div, prob, sys_name, szs, f"expected one of {sorted(expected)}")
                    )

    # ---- render ----
    now = datetime.now().strftime("%Y-%m-%d %H:%M")
    total_problems = sum(len(v) for v in div_problems.values())
    n_sys = len(systems)

    header = f"{edition.upper()} Results — {now}  ({total_problems} problems × {n_sys} systems)"
    print(header)
    print("=" * len(header))
    print()

    # Column widths
    div_w = max(len("Division"), max((len(d) for d in divisions_seen), default=0))
    prob_w = max(len("Problems"), max((len(str(len(v))) for v in div_problems.values()), default=0))
    sys_col_w = 18  # "Solved  Avg (s)  "

    # Header row
    div_header = f"{'Division':<{div_w}}  {'Problems':>{prob_w}}"
    sys_headers = "  ".join(
        f"  {s:<{sys_col_w}}" for s in systems
    )
    print(f"{div_header}  {sys_headers}")

    sub_header = " " * (div_w + 2 + prob_w)
    for _ in systems:
        sub_header += "  " + f"  {'Solved':>6}  {'Avg (s)':>7}  "
    print(sub_header)

    sep = "-" * (div_w + 2 + prob_w) + "  " + "  ".join(
        "-" * (sys_col_w + 2) for _ in systems
    )
    print(sep)

    total_solved = {s: 0 for s in systems}
    total_time = {s: 0.0 for s in systems}

    for div in divisions_seen:
        n_probs = len(div_problems[div])
        row_str = f"{div.upper():<{div_w}}  {n_probs:>{prob_w}}"
        for sys_name in systems:
            cnt, t = stats[(div, sys_name)]
            avg = t / cnt if cnt > 0 else 0.0
            row_str += f"  {'':2}{cnt:>6}  {avg:>7.1f}  "
            total_solved[sys_name] += cnt
            total_time[sys_name] += t
        print(row_str)

    print(sep)
    total_probs_all = sum(len(v) for v in div_problems.values())
    row_str = f"{'TOTAL':<{div_w}}  {total_probs_all:>{prob_w}}"
    for sys_name in systems:
        cnt = total_solved[sys_name]
        avg = total_time[sys_name] / cnt if cnt > 0 else 0.0
        row_str += f"  {'':2}{cnt:>6}  {avg:>7.1f}  "
    print(row_str)
    print()

    # ---- disagreements ----
    if disagreements:
        print(f"DISAGREEMENTS — {len(disagreements)} problem(s) where systems gave contradictory answers:")
        for div, prob, solved_by in disagreements:
            parts = "  ".join(f"{s}={szs}" for s, szs in sorted(solved_by.items()))
            print(f"  {div.upper():<6}  {prob:<30}  {parts}  ⚠ SOUNDNESS")
    else:
        print("DISAGREEMENTS — none detected.")
    print()

    # ---- polarity violations ----
    if polarity_violations:
        print(f"POLARITY VIOLATIONS — {len(polarity_violations)} case(s) of wrong SZS polarity:")
        for div, prob, sys_name, szs, note in polarity_violations:
            print(f"  {div.upper():<6}  {prob:<30}  {sys_name}={szs}  ({note})  ⚠ UNSOUND")
    else:
        print("POLARITY VIOLATIONS — none detected.")


if __name__ == "__main__":
    main()
