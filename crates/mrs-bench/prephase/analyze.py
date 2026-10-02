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

    behavioural_taxonomy(features, problems, solved, strategies, divisions)
    feasibility_trees(features, problems, solved, strategies)

    if args.emit_rules:
        emit_rules(features, problems, solved, strategies)


# ------------------------------------------------- behavioural taxonomy ---


def behavioural_taxonomy(features, problems, solved, strategies, elapsed):
    """Group problems by *which* configurations solve them, not by how they look.

    The syntactic classes say what a problem is; this says what happens to it.
    Comparing the two is the study's real question: if the syntactic axes explain
    the behaviour, routing can use them.
    """
    print("\nbehavioural summary:")
    # 1. How many configurations solve each problem. The gap between "one" and
    #    "several" is what a portfolio buys over a single configuration.
    histogram = Counter()
    for problem in problems:
        count = sum(1 for s in strategies if problem in solved[s])
        if count == 0:
            bucket = "0"
        elif count == 1:
            bucket = "1"
        elif count <= 3:
            bucket = "2-3"
        elif count <= 7:
            bucket = "4-7"
        else:
            bucket = "8-15"
        histogram[bucket] += 1
    for bucket in ("0", "1", "2-3", "4-7", "8-15"):
        count = histogram.get(bucket, 0)
        print(f"  solved by {bucket:>5} configurations: {count:>4} "
              f"({100*count/max(1,len(problems)):.1f}%)")

    # 2. Exclusive coverage of pairs: the complementarity a portfolio exists for.
    print("\nexclusive coverage (problems only one of the pair can solve):")
    pairs = []
    for i, a in enumerate(strategies):
        for b in strategies[i + 1:]:
            only_a = solved[a] - solved[b]
            only_b = solved[b] - solved[a]
            pairs.append((len(only_a) + len(only_b), a, b, len(only_a), len(only_b)))
    pairs.sort(reverse=True)
    for total, a, b, only_a, only_b in pairs[:8]:
        print(f"  s{a:<3}+s{b:<3}: only-s{a}={only_a:<4} only-s{b}={only_b:<4} "
              f"(both={len(solved[a] & solved[b])})")

    # 3. Modality: the single best configuration for each problem, which is what a
    #    one-worker run would find, and how far that is from the union.
    modal = Counter()
    for problem in problems:
        best = max(strategies, key=lambda s: (1 if problem in solved[s] else 0,))
        winners = [s for s in strategies if problem in solved[s]]
        if not winners:
            modal["none"] += 1
        else:
            # The fastest solver when several succeed: the one a latency-oriented
            # router should name.
            fastest = min(winners, key=lambda s: elapsed[problem].get(s, float("inf")))
            modal[f"s{fastest}"] += 1
    print("\nfastest solving configuration per problem:")
    print("  " + "  ".join(f"{k}={v}" for k, v in modal.most_common(10)))

    # 4. Fragmentation of the behaviour space, and whether a syntactic label
    #    predicts it.
    signatures = defaultdict(list)
    for problem in problems:
        signatures[tuple(sorted(s for s in strategies if problem in solved[s]))].append(problem)
    print(f"\nbehaviour space: {len(signatures)} distinct solver sets over "
          f"{len(problems)} problems")
    big = sorted((k, v) for k, v in signatures.items() if len(v) >= 4)
    big.sort(key=lambda kv: -len(kv[1]))
    for key, members in big[:10]:
        by_label = Counter(features[p]["label"] for p in members)
        top = ", ".join(f"{label}({count})" for label, count in by_label.most_common(2))
        described = ",".join(f"s{k}" for k in key) if key else "-"
        print(f"  n={len(members):<4} solved_by={described:<30} {top}")

    label_to_sigs = defaultdict(Counter)
    for problem in problems:
        key = tuple(sorted(s for s in strategies if problem in solved[s]))
        label_to_sigs[features[problem]["label"]][key] += 1
    pure = 0
    total = 0
    for label, counter in sorted(label_to_sigs.items(), key=lambda kv: -sum(kv[1].values())):
        size = sum(counter.values())
        if size < 4:
            continue
        top_signature, top_count = counter.most_common(1)[0]
        pure += top_count
        total += size
        modal_signature = "s" + ",".join("s%d" % s for s in top_signature) if top_signature else "-"
        print(f"  {label:<46} n={size:<4} behaviours={len(counter):<3} "
              f"modal={modal_signature:<26} purity={top_count/size:.2f}")
    if total:
        print(f"  weighted modal purity: {pure/total:.3f} "
              f"(1.0 would mean the syntactic label determines the outcome)")


# ------------------------------------------------------ portfolio simulation ---

# The shipped per-division priority orders, so the baseline the pre-phase has to
# beat is measured rather than assumed. Mirrors `strategy/named.rs`.
NAMED_ORDERS = {
    "casc":    [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    "casc_feq":[11, 12, 1, 6, 10, 8, 14, 4, 5, 2, 3, 7, 9, 13, 15],
    "casc_fne":[11, 8, 4, 15, 10, 3, 12, 1, 6, 2, 5, 7, 9, 13, 14],
    "casc_ueq":[4, 8, 12, 11, 2, 14, 15, 1, 3, 5, 6, 7, 9, 10, 13],
    "casc_epu":[1, 6, 14, 11, 4, 2, 3, 7, 5, 8, 10, 9, 12, 13, 15],
    "casc_eps":[2, 3, 1, 8, 11, 12, 9, 14, 7, 10, 5, 13, 15, 6, 4],
    "casc_epr":[6, 2, 1, 3, 4, 5, 7, 8, 9, 10, 11, 12, 13, 14, 15],
}


def coverage_of_order(problems, solved, order, workers):
    """Coverage of a portfolio that runs the first `workers` entries of `order`.

    Portfolio workers run concurrently, so each receives the whole wall clock: a
    solo run at the full budget is exactly what one slot gets. The union of the
    selected configurations' solved sets is therefore the portfolio's coverage,
    with no further measurement needed. That equivalence is what makes a solo
    sweep sufficient to evaluate a portfolio.
    """
    chosen = set(order[:workers])
    return {
        problem
        for problem in problems
        if any(problem in solved[s] for s in chosen)
    }


def best_fixed_portfolio(problems, solved, strategies, workers):
    chosen, trace, remaining = greedy_set_cover(problems, {s: solved[s] for s in strategies})
    coverage = len(problems) - len(remaining)
    return chosen[:workers], coverage, trace, remaining


def simulate_portfolios(features, labels, problems, strategies, solved, routed_orders):
    """Fixed per-division portfolio versus a routed portfolio, at each width."""
    print("\nportfolio simulation (coverage = problems solved by any selected slot):")
    print(f"{'workers':>7} {'oracle':>7} {'best fixed':>11} {'routed':>7} "
          f"{'casc':>6} {'casc_div':>9} {'delta':>6}")

    union_all = set().union(*solved.values())
    n = len(problems)
    for workers in (1, 2, 4, 8, 12, 15):
        fixed_chosen, fixed_cov, _, _ = best_fixed_portfolio(problems, solved, strategies, workers)
        # Routed: each problem is covered if one of its own top-`workers` entries
        # solves it.
        routed_cov = 0
        for problem in problems:
            order = routed_orders.get(problem, [])
            if any(problem in solved[s] for s in order[:workers]):
                routed_cov += 1
        casc_cov = len(coverage_of_order(problems, solved, NAMED_ORDERS["casc"], workers))
        # The division schedule that matches the problem's own division.
        div_cov = 0
        for problem in problems:
            division = division_of(features[problem]["path"]).lower()
            order = NAMED_ORDERS.get(f"casc_{division}", NAMED_ORDERS["casc"])
            if any(problem in solved[s] for s in order[:workers]):
                div_cov += 1
        delta = routed_cov - div_cov
        print(f"{workers:>7} {len(union_all):>7} {fixed_cov:>11} {routed_cov:>7} "
              f"{casc_cov:>6} {div_cov:>9} {delta:>+6}")
    print(f"  (n={n}; oracle = union over all {len(strategies)} configurations)")


def cross_validate(features, labels, problems, strategies, solved, folds=2):
    """Fit the routing table on one half of the sample, score it on the other.

    The routing table is fitted by choosing, per syntactic label, the priority
    order that maximises greedy coverage on the *fit* half, and then applied
    unchanged to the *held-out* half. Reporting the held-out number is the only
    way to tell a routing table that generalises from one that has memorised the
    sample.
    """
    ordered = sorted(problems)
    half = len(ordered) // folds
    print(f"\ncross-validation ({folds} folds, {half} problems each):")
    held_out_total = 0
    baseline_total = 0
    for fold in range(folds):
        fit = [p for i, p in enumerate(ordered) if i % folds == fold]
        test = [p for i, p in enumerate(ordered) if i % folds != fold]
        orders = fit_orders(features, fit, strategies, solved)
        for problem in test:
            order = orders.get(features[problem]["label"], list(strategies))
            if any(problem in solved[s] for s in order[:8]):
                held_out_total += 1
            division = division_of(features[problem]["path"]).lower()
            named = NAMED_ORDERS.get(f"casc_{division}", NAMED_ORDERS["casc"])
            if any(problem in solved[s] for s in named[:8]):
                baseline_total += 1
    print(f"  held-out coverage at 8 slots: routed {held_out_total}/{len(problems)} "
          f"({100*held_out_total/max(1,len(problems)):.1f}%)")
    print(f"  held-out coverage at 8 slots: per-division baseline "
          f"{baseline_total}/{len(problems)} ({100*baseline_total/max(1,len(problems)):.1f}%)")
    print(f"  delta: {held_out_total - baseline_total:+d}")


def fit_orders(features, fit_problems, strategies, solved):
    """Per label, the priority order maximizing greedy coverage on `fit_problems`."""
    by_label = defaultdict(list)
    for problem in fit_problems:
        by_label[features[problem]["label"]].append(problem)
    orders = {}
    for label, members in by_label.items():
        members = set(members)
        local = {s: solved[s] & members for s in strategies}
        chosen, _, _ = greedy_set_cover(list(members), local)
        orders[label] = [s for s in strategies if s not in chosen] + chosen
    return orders


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

    problems, strategies, solved, refuted, sat, elapsed = coverage_table(
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

    behavioural_taxonomy(features, problems, solved, strategies, elapsed)

    # Two routed policies: the one fitted on this sample (an upper bound, and one
    # that overfits) and the cross-validated one. Both are reported so the gap
    # between them is visible.
    fitted = fit_orders(features, problems, strategies, solved)
    routed = {problem: fitted.get(features[problem]["label"], list(strategies))
              for problem in problems}
    simulate_portfolios(features, labels, problems, strategies, solved, routed)
    cross_validate(features, labels, problems, strategies, solved)

    if args.emit_rules:
        emit_rules(features, problems, solved, strategies)


def load_features(path):
    """Return {name: row-as-dict} for rows that produced a clause set."""
    rows = {}
    statuses = Counter()
    with open(path, newline="") as handle:
        for row in csv.DictReader(handle):
            statuses[row["parse_status"]] += 1
            if row["parse_status"] in ("ok", "empty_after_lowering"):
                rows[row["name"]] = row
    return rows, statuses


def load_labels(path):
    """Return {problem: {strategy: row}} and {problem: {strategy: seconds}}."""
    per_problem = defaultdict(dict)
    with open(path, newline="") as handle:
        for row in csv.DictReader(handle):
            per_problem[row["problem"]][int(row["strategy"])] = row
    return per_problem


def fnum(row, key, default=0.0):
    try:
        return float(row[key])
    except (TypeError, ValueError, KeyError):
        return default


def division_of(path):
    parts = path.rsplit("/", 2)
    return parts[-2] if len(parts) >= 2 else "?"


# ------------------------------------------------------------- coverage ---


def coverage_table(features, labels, divisions=None):
    strategies = sorted(next(iter(labels.values())).keys())
    problems = sorted(set(features) & set(labels))
    if divisions:
        problems = [p for p in problems if division_of(features[p]["path"]) in divisions]
    solved = {s: set() for s in strategies}
    refuted = {s: set() for s in strategies}
    sat = {s: set() for s in strategies}
    elapsed = defaultdict(dict)
    for problem in problems:
        for strategy in strategies:
            row = labels[problem].get(strategy)
            if row is None:
                continue
            try:
                elapsed[problem][strategy] = float(row["elapsed_s"])
            except ValueError:
                elapsed[problem][strategy] = float("inf")
            if row["verdict"] in ("refutation", "saturation"):
                solved[strategy].add(problem)
            if row["verdict"] == "refutation":
                refuted[strategy].add(problem)
            if row["verdict"] == "saturation":
                sat[strategy].add(problem)
    return problems, strategies, solved, refuted, sat, elapsed


def greedy_set_cover(universe, sets):
    """Greedy maximum coverage; returns (chosen, trace, uncovered)."""
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
    "largest_component_ratio": [(0.5, "<.5"), (0.9, ".5-.9"), (1.01, ">=.9")],
    "avg_term_depth": [(1.5, "<1.5"), (3.0, "1.5-3"), (1e9, ">=3")],
    "nonlinear_ratio": [(0.01, "0"), (0.3, "<.3"), (0.8, "<.8"), (1.01, ">=.8")],
    "rewrite_rule_ratio": [(0.01, "0"), (0.3, "<.3"), (0.8, "<.8"), (1.01, ">=.8")],
    "n_literals": [(50, "<50"), (500, "50-500"), (5000, "500-5k"), (1e9, ">=5k")],
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
    """For each feature and bucket, the per-strategy coverage inside that bucket."""
    out = {}
    for key, edges in BINNED.items():
        for _, name in edges:
            members = [p for p in problems if bucket(features[p], key, edges) == name]
            if not members:
                continue
            per_strategy = {s: len(strategy_sets[s] & set(members)) for s in strategies}
            out[(key, name)] = (members, per_strategy)
    for key in CATEGORICAL:
        for name in sorted({features[p][key] for p in problems}):
            members = [p for p in problems if features[p][key] == name]
            per_strategy = {s: len(strategy_sets[s] & set(members)) for s in strategies}
            out[(key, name)] = (members, per_strategy)
    return out


def print_group_report(features, labels, problems, strategy_sets, strategies, min_size=8):
    table = groups(features, labels, problems, strategy_sets, strategies)
    union = set().union(*strategy_sets.values()) if strategy_sets else set()
    print(f"{'feature':<34} {'bucket':<12} {'n':>4} {'union':>6}  best-4")
    for (key, name), (members, per_strategy) in sorted(table.items()):
        if len(members) < min_size:
            continue
        members_set = set(members)
        covered = len(union & members_set)
        ranked = sorted(per_strategy.items(), key=lambda kv: -kv[1])
        best = ", ".join(f"s{k}={v}" for k, v in ranked[:4] if v > 0)
        print(f"{key:<34} {name:<12} {len(members):>4} {covered:>6}  {best}")


def find_best_split(rows, features, solved):
    """Best single feature/threshold split by the lift in "solved by anyone".

    Deliberately a plain exhaustive search over the thresholds already used for
    grouping, with no model: the output is a rule, and a rule can be read.
    """
    union = set().union(*solved.values()) if solved else set()
    best = None
    for key, edges in BINNED.items():
        cuts = [cut for cut, _ in edges][:-1]
        for cut in cuts:
            low = [p for p in rows if fnum(features[p], key) < cut]
            high = [p for p in rows if fnum(features[p], key) >= cut]
            if len(low) < 4 or len(high) < 4:
                continue
            low_rate = len(union & set(low)) / len(low)
            high_rate = len(union & set(high)) / len(high)
            gain = abs(low_rate - high_rate)
            if best is None or gain > best[0]:
                best = (gain, key, cut, low_rate, high_rate, len(low), len(high))
    return best


def feasibility_trees(features, problems, solved):
    """Greedy rule tree separating 'any configuration solves this' from 'none does'."""
    union = set().union(*solved.values()) if solved else set()
    rows = list(problems)
    if not rows:
        return
    overall = len(union & set(rows)) / len(rows)
    print(f"\nfeasibility: {100*overall:.1f}% of the sample is solved by at least one "
          f"configuration. First split:")
    best = find_best_split(rows, features, solved)
    if best is None or best[0] < 0.1:
        print("  no single feature threshold separates the classes by >= 10 points")
        return
    gain, key, cut, low_rate, high_rate, low_n, high_n = best
    print(f"  {key} < {cut}")
    print(f"    below:  {low_n:>4} problems, {100*low_rate:.0f}% solved")
    print(f"    at/above:{high_n:>4} problems, {100*high_rate:.0f}% solved")
    for candidate_key, edges in BINNED.items():
        if candidate_key == key:
            continue
        sub_best = None
        for cut2 in [c for c, _ in edges][:-1]:
            low = [p for p in rows if fnum(features[p], key) < cut and fnum(features[p], candidate_key) < cut2]
            high = [p for p in rows if fnum(features[p], key) < cut and fnum(features[p], candidate_key) >= cut2]
            if len(low) < 4 or len(high) < 4:
                continue
            rate = abs(len(union & set(low)) / len(low) - len(union & set(high)) / len(high))
            if sub_best is None or rate > sub_best[0]:
                sub_best = (rate, cut2,
                            len(union & set(low)) / len(low),
                            len(union & set(high)) / len(high), len(low), len(high))
        if sub_best and sub_best[0] >= 0.2:
            print(f"    then {candidate_key} < {sub_best[1]}: "
                  f"{sub_best[4]} problems at {100*sub_best[2]:.0f}% vs "
                  f"{sub_best[5]} at {100*sub_best[3]:.0f}%")


def emit_rules(features, problems, solved, strategies):
    """Print the per-label configuration ranking a routing table would use."""
    print("\nrouting evidence (per label: configurations ranked by coverage):")
    by_label = defaultdict(list)
    for problem in problems:
        by_label[features[problem]["label"]].append(problem)
    for label, members in sorted(by_label.items(), key=lambda kv: -len(kv[1])):
        if len(members) < 3:
            continue
        members_set = set(members)
        rates = sorted(
            ((s, len(solved[s] & members_set) / len(members_set)) for s in strategies),
            key=lambda kv: -kv[1],
        )
        union = len(set().union(*(solved[s] & members_set for s in strategies))) / len(members_set)
        ordered = " ".join(f"s{s}={r:.2f}" for s, r in rates[:6] if r > 0)
        print(f"  {label:<46} n={len(members):<4} union={union:.2f}  {ordered}")


if __name__ == "__main__":
    sys.exit(main())
