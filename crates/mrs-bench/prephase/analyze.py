#!/usr/bin/env python3
"""Join pre-phase features with measured strategy outcomes and derive routing rules.

The feature side comes from `prephase_dump`, the label side from
`prephase_sweep`. Everything this script prints is a *measurement*; the routing
table it emits at the end is the only part that is a decision, and it is
restricted to thresholds this data supports.

Usage:
    analyze.py features.csv labels.csv [--divisions feq,fne,...] [--emit-rules]
"""

from __future__ import annotations

import argparse
import csv
import math
import sys
from collections import Counter, defaultdict

# ---------------------------------------------------------------- loading ---


def load_features(path):
    """Return {name: row-as-dict}, keeping only `ok` / `empty_after_lowering` rows."""
    rows = {}
    statuses = Counter()
    with open(path, newline="") as handle:
        reader = csv.DictReader(handle)
        for row in reader:
            statuses[row["parse_status"]] += 1
            if row["parse_status"] in ("ok", "empty_after_lowering"):
                rows[row["name"]] = row
    return rows, statuses


def load_labels(path):
    """Return {name: {strategy: row-as-dict}}."""
    per_problem = defaultdict(dict)
    with open(path, newline="") as handle:
        reader = csv.DictReader(handle)
        for row in reader:
            per_problem[row["problem"]][int(row["strategy"])] = row
    return per_problem


def fnum(row, key, default=0.0):
    try:
        return float(row[key])
    except (TypeError, ValueError):
        return default


def division_of(path):
    parts = path.rsplit("/", 2)
    return parts[-2] if len(parts) >= 2 else "?"


# ------------------------------------------------------------- coverage ---


def coverage_table(features, labels, divisions=None, strategies=None):
    """Per-strategy solved counts, over the whole set and per division."""
    strategies = strategies or sorted(next(iter(labels.values())).keys())
    problems = sorted(set(features) & set(labels))
    if divisions:
        problems = [p for p in problems if division_of(features[p]["path"]) in divisions]
    solved = {s: set() for s in strategies}
    refuted = {s: set() for s in strategies}
    sat = {s: set() for s in strategies}
    for problem in problems:
        for strategy in strategies:
            row = labels[problem].get(strategy)
            if row is None:
                continue
            if row["verdict"] in ("refutation", "saturation"):
                solved[strategy].add(problem)
            if row["verdict"] == "refutation":
                refuted[strategy].add(problem)
            if row["verdict"] == "saturation":
                sat[strategy].add(problem)
    return problems, strategies, solved, refuted, sat


def greedy_set_cover(universe, sets):
    """Greedy maximum coverage; returns (chosen, trace)."""
    remaining = set(universe)
    chosen = []
    trace = []
    while remaining:
        best, gain = None, 0
        for name, members in sets.items():
            if name in chosen:
                continue
            covered = len(members & remaining)
            if covered > gain:
                best, gain = name, covered
        if best is None or gain == 0:
            break
        chosen.append(best)
        remaining -= sets[best]
        trace.append((best, gain, len(remaining)))
    return chosen, trace, remaining


# ------------------------------------------------------ conditional rates ---


def bucket(row, key, edges):
    value = fnum(row, key)
    for edge, name in edges:
        if value < edge:
            return name
    return edges[-1][1]


BINNED = {
    "log_n_clauses": [(2.0, "<100"), (3.0, "100-999"), (4.0, "1k-10k"), (6.0, ">10k")],
    "unit_ratio": [(0.25, "<.25"), (0.75, ".25-.75"), (0.999, ".75-1"), (2.0, "=1")],
    "horn_ratio": [(0.5, "<.5"), (0.9, ".5-.9"), (0.999, ".9-1"), (2.0, "=1")],
    "avg_clause_len": [(1.5, "1-2"), (2.5, "2-3"), (4.0, "3-4"), (9.0, "4-9"), (99.0, ">=9")],
    "goal_reachable_ratio": [(0.25, "<.25"), (0.8, ".25-.8"), (1.01, ">=.8")],
    "n_components": [(2, "1"), (4, "2-3"), (1e9, "4+")],
    "max_term_depth": [(3, "1-2"), (6, "3-5"), (11, "6-10"), (1e9, ">10")],
    "n_skolems": [(1, "0"), (11, "1-10"), (101, "11-100"), (1e9, ">100")],
    "max_fun_arity": [(1, "0"), (2, "1"), (3, "2"), (1e9, "3+")],
    "redundant_ratio": [(0.001, "0"), (0.1, "<=.1"), (0.4, "<=.4"), (1.01, ">.4")],
    "negative_literal_ratio": [(0.34, "<.34"), (0.5, ".34-.5"), (0.67, ".5-.67"), (1.01, ">.67")],
    "symbol_concentration": [(0.3, "<.3"), (0.6, ".3-.6"), (0.85, ".6-.85"), (1.01, ">.85")],
    "ground_ratio": [(0.01, "0"), (0.5, "<.5"), (0.99, "<1"), (1.01, "=1")],
    "fvo_ratio": [(0.01, "0"), (0.5, "<.5"), (0.99, "<1"), (1.01, "=1")],
    "avg_vars_per_clause": [(0.5, "0"), (1.5, "0.5-1.5"), (3.5, "1.5-3.5"), (1e9, ">3.5")],
    "n_predicates": [(5, "<5"), (20, "5-20"), (80, "20-80"), (1e9, ">80")],
    "abstraction_trivial_clause_ratio": [(0.01, "0"), (0.5, "<.5"), (0.99, "<1"), (1.01, "=1")],
    "n_goal_clauses": [(1, "0"), (3, "1-2"), (10, "3-9"), (1e9, ">=10")],
    "n_input_cnf_clauses": [(1, "0"), (1e9, ">=1")],
}

CATEGORICAL = [
    "logic_class",
    "shape_class",
    "scale_class",
    "goal_class",
    "decomposition_class",
    "dialect",
]


def groups(features, labels, problems, strategy_sets, strategies):
    """For each feature and each bucket, report per-strategy coverage."""
    out = {}
    for key, edges in BINNED.items():
        for edge, name in edges:
            members = [
                p for p in problems if bucket(features[p], key, edges) == name
            ]
            if not members:
                continue
            per_strategy = {}
            for strategy in strategies:
                per_strategy[strategy] = len(strategy_sets[strategy] & set(members))
            out[(key, name)] = (members, per_strategy)
    for key in CATEGORICAL:
        for name in sorted({features[p][key] for p in problems}):
            members = [p for p in problems if features[p][key] == name]
            per_strategy = {}
            for strategy in strategies:
                per_strategy[strategy] = len(strategy_sets[strategy] & set(members))
            out[(key, name)] = (members, per_strategy)
    return out


def print_group_report(features, labels, problems, strategy_sets, strategies, min_size=8):
    table = groups(features, labels, problems, strategy_sets, strategies)
    union = set().union(*strategy_sets.values()) if strategy_sets else set()
    print(f"{'feature':<34} {'bucket':<12} {'n':>4} {'union':>6}  best-2")
    for (key, name), (members, per_strategy) in sorted(table.items()):
        if len(members) < min_size:
            continue
        members_set = set(members)
        covered = len(union & members_set)
        ranked = sorted(per_strategy.items(), key=lambda kv: -kv[1])
        best = ", ".join(f"s{k}={v}" for k, v in ranked[:4] if v > 0)
        print(f"{key:<34} {name:<12} {len(members):>4} {covered:>6}  {best}")


# ------------------------------------------------------------ main ------


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("features")
    parser.add_argument("labels")
    parser.add_argument("--divisions", default=None)
    parser.add_argument("--emit-rules", action="store_true")
    args = parser.parse_args()

    features, statuses = load_features(args.features)
    labels = load_labels(args.labels)
    divisions = set(args.divisions.split(",")) if args.divisions else None

    print(f"features: {len(features)} analysable of {sum(statuses.values())} files")
    for status, count in statuses.most_common():
        print(f"  {status:<26} {count}")
    print(f"labels:   {len(labels)} problems")

    problems, strategies, solved, refuted, sat = coverage_table(
        features, labels, divisions
    )
    print(f"\njoin: {len(problems)} problems with both features and labels")
    by_division = defaultdict(list)
    for problem in problems:
        by_division[division_of(features[problem]["path"])].append(problem)
    for division, members in sorted(by_division.items()):
        union = set().union(*(solved[s] & set(members) for s in strategies)) if strategies else set()
        print(f"  {division:<6} n={len(members):<4} union={len(union)}")

    print("\nper-strategy coverage (whole sample):")
    ranked = sorted(solved.items(), key=lambda kv: -len(kv[1]))
    union_all = set().union(*solved.values())
    for strategy, members in ranked:
        print(
            f"  s{strategy:<3} solved={len(members):<4} "
            f"({100*len(members)/max(1,len(problems)):.1f}%)  "
            f"refutation={len(refuted[strategy]):<4} saturation={len(sat[strategy])}"
        )
    print(f"  union of all {len(strategies)}: {len(union_all)} "
          f"({100*len(union_all)/max(1,len(problems)):.1f}%)")

    chosen, trace, remaining = greedy_set_cover(problems, solved)
    print("\ngreedy set cover:")
    for name, gain, left in trace:
        print(f"  + s{name:<3} +{gain:<4} uncovered={left}")
    print(f"  coverage {len(problems)-len(remaining)}/{len(problems)} "
          f"with {len(chosen)} strategies; {len(remaining)} never solved")

    print("\nbest strategy per division:")
    for division, members in sorted(by_division.items()):
        members_set = set(members)
        best = max(strategies, key=lambda s: len(solved[s] & members_set))
        division_union = len(set().union(*(solved[s] & members_set for s in strategies)))
        print(f"  {division:<6} best=s{best} ({len(solved[best] & members_set)}/{len(members)}) "
              f"union={division_union}/{len(members)}")

    print("\nconditional coverage by feature bucket:")
    print_group_report(features, labels, problems, solved, strategies)

    # Feasibility: can any static feature tell "solvable" from "not solvable"?
    print("\nfeasibility (is any strategy solved) by label:")
    by_label = defaultdict(list)
    for problem in problems:
        by_label[features[problem]["label"]].append(problem)
    for label, members in sorted(by_label.items(), key=lambda kv: -len(kv[1])):
        if len(members) < 4:
            continue
        members_set = set(members)
        union = len(set().union(*(solved[s] & members_set for s in strategies)))
        best = max(strategies, key=lambda s: len(solved[s] & members_set))
        print(f"  {label:<46} n={len(members):<4} union={union:<4} "
              f"({100*union/len(members):.0f}%) best=s{best}")

    if args.emit_rules:
        print("\n--emit-rules is not implemented yet; see docs/reports/prephase")


if __name__ == "__main__":
    sys.exit(main())