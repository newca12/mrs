"""Regression tests for the pre-phase solo-matrix portfolio analysis."""

import unittest

import analyze
import repair_probe_columns


class PortfolioOrderTests(unittest.TestCase):
    def setUp(self):
        self.problems = ["p1", "p2", "p3"]
        self.features = {p: {"label": "same-class"} for p in self.problems}
        self.strategies = [1, 2, 3]
        self.solved = {
            1: {"p1", "p2"},
            2: {"p2", "p3"},
            3: {"p3"},
        }

    def test_fitted_priority_puts_greedy_cover_first(self):
        orders = analyze.fit_orders(
            self.features, self.problems, self.strategies, self.solved
        )

        self.assertEqual(orders["same-class"][:2], [1, 2])
        self.assertEqual(
            set().union(*(self.solved[s] for s in orders["same-class"][:2])),
            set(self.problems),
        )

    def test_greedy_fixed_coverage_uses_only_the_requested_prefix(self):
        chosen, coverage, _, remaining = analyze.best_fixed_portfolio(
            self.problems, self.solved, self.strategies, workers=1
        )

        self.assertEqual(chosen, [1])
        self.assertEqual(coverage, 2)
        self.assertEqual(remaining, {"p3"})


class ProbeCsvRepairTests(unittest.TestCase):
    def test_repairs_the_label_suffix_and_shifts_probe_columns(self):
        row = {
            "parse_status": "ok",
            "name": "probe-case",
            "logic_class": "FEQ",
            "shape_class": "HORN",
            "scale_class": "TINY",
            "goal_class": "TIGHT",
            "decomposition_class": "CONNECTED",
            "label": "FEQ/HORN/TINY/TIGHT/CONNECTED4",
            "probe_iterations": "10",
            "probe_generated": "3",
            "probe_processed": "2",
            "probe_forward_subsumed": "1",
            "probe_weight_discarded": "5",
            "probe_passive": "50",
            "probe_elapsed_ms": "2.5",
            "probe_generation_rate": "0.2",
            "probe_redundancy_rate": "0.06",
            "probe_throughput_per_ms": None,
        }

        action = repair_probe_columns.repair_row(row)

        self.assertEqual(action, "repaired")
        self.assertEqual(row["label"], "FEQ/HORN/TINY/TIGHT/CONNECTED")
        self.assertEqual(row["probe_iterations"], "4")
        self.assertEqual(row["probe_generated"], "10")
        self.assertEqual(row["probe_processed"], "3")
        self.assertEqual(row["probe_forward_subsumed"], "2")
        self.assertEqual(row["probe_weight_discarded"], "1")
        self.assertEqual(row["probe_passive"], "5")
        self.assertEqual(row["probe_elapsed_ms"], "50")
        self.assertEqual(row["probe_generation_rate"], "2.5")
        self.assertEqual(row["probe_redundancy_rate"], "0.2")
        self.assertEqual(row["probe_throughput_per_ms"], "0.06")


if __name__ == "__main__":
    unittest.main()
