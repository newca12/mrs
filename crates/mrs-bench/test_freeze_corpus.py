from pathlib import Path
import tempfile
import unittest

from freeze_corpus import freeze, split_family


class FrozenCorpus(unittest.TestCase):
    def test_variants_stay_together_and_includes_affect_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            families = [f"family-{i}" for i in range(100)]
            dev = next(f for f in families if split_family(f, 42, 20) == "development")
            hold = next(f for f in families if split_family(f, 42, 20) == "holdout")
            for name in ("a.p", "a-variant.p", "b.p", "include.ax"):
                (root / name).write_text("premise")
            mapping = {"a.p": dev, "a-variant.p": dev, "b.p": hold}
            first = freeze(root, families=mapping)
            self.assertEqual(first, freeze(root, families=mapping))
            assignments = {row["path"]: row["split"] for row in first["problems"]}
            self.assertEqual(assignments["a.p"], assignments["a-variant.p"])
            self.assertNotEqual(assignments["a.p"], assignments["b.p"])
            (root / "include.ax").write_text("changed")
            self.assertNotEqual(first["corpus_sha256"], freeze(root, families=mapping)["corpus_sha256"])
            with self.assertRaises(ValueError):
                freeze(root, families={})


if __name__ == "__main__":
    unittest.main()
