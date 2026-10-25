import unittest

from paired_campaign import paired_delta


class PairedOutcomes(unittest.TestCase):
    def test_gains_and_losses_are_paired_not_union_coverage(self):
        delta = paired_delta({"a": True, "b": False, "c": False},
                             {"a": False, "b": True, "c": True})
        self.assertEqual(delta["gains"], 2)
        self.assertEqual(delta["losses"], 1)
        self.assertEqual(delta["delta"], 1)
        self.assertEqual(delta["problems"], 3)

    def test_mismatched_evaluation_sets_are_refused(self):
        with self.assertRaises(ValueError):
            paired_delta({"a": True}, {"b": True})


if __name__ == "__main__":
    unittest.main()
