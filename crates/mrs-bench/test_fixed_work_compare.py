import unittest

from fixed_work_compare import detail, equivalent_work


class FixedWorkEvidence(unittest.TestCase):
    def test_refuses_equal_counts_with_different_selection_or_wrong_stop(self):
        row = dict(processed="100", generated="200", passive="15", lrs_discarded="0",
                   weight_discarded="0", trajectory_sha256="0:abc", resource_reason="max_processed")
        self.assertTrue(equivalent_work(row, row.copy(), 100))
        for key, value in (("trajectory_sha256", "0:def"), ("resource_reason", "memory"),
                           ("processed", "99"), ("generated", "201")):
            with self.assertRaises(ValueError):
                equivalent_work(row, row | {key: value}, 100)

    def test_missing_or_duplicate_final_telemetry_is_refused(self):
        with self.assertRaises(ValueError):
            detail("")
        with self.assertRaises(ValueError):
            detail("% SZS detail processed=1\n% SZS detail processed=2")
        self.assertEqual(detail("% SZS detail processed=100")['processed'], "100")


if __name__ == "__main__":
    unittest.main()
