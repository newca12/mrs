import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from certification_dashboard import read_snapshot, render


class EvidenceDashboard(unittest.TestCase):
    def test_all_outcomes_remain_visible_and_mutations_are_refused(self):
        with tempfile.TemporaryDirectory() as root:
            data = ("edition,division,problem,system,strict_status\n"
                    "casc-30,feq,A,mrs,VerifiedGood\n"
                    "casc-30,feq,B,mrs,Unknown\n"
                    "casc-30,feq,C,mrs,proof_omitted\n").encode()
            directory = Path(root) / hashlib.sha256(data).hexdigest()
            directory.mkdir()
            (directory / "audit.csv").write_bytes(data)
            identity = dict(schema=1, audit_binary_sha256="auditor", proover_binary_sha256="checker",
                            run_csv_sha256="run", corpus_sha256="corpus", strict_time=120, jobs=1)
            (directory / "audit-identity.json").write_text(json.dumps(identity))
            report = render([directory])
            self.assertIn("| 3 | 1 | 0 |", report)
            self.assertIn("proof_omitted: 1", report)
            self.assertIn("Unknown: 1", report)
            with self.assertRaises(ValueError):
                render([directory, directory])
            (directory / "audit.csv").write_bytes(data + b"forged")
            with self.assertRaises(ValueError):
                read_snapshot(directory)


if __name__ == "__main__":
    unittest.main()
