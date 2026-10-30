#!/usr/bin/env python3
"""Paired, order-randomized, strictly audited eight-worker CASC comparisons.

Each pair runs both frozen binaries sequentially on one host. No builds are
triggered and output directories are never reused. Two pairs are the minimum
screening block, not a claim of statistical power. Report the individual pair
deltas; add pairs if they disagree or the effect is marginal. Example:

python3 crates/mrs-bench/paired_campaign.py --baseline /path/mrs-base \
  --candidate /path/mrs-next --problems-dir /path/casc-30 --edition casc-30 \
  --divisions feq,ueq --output /path/new-experiment
"""

import argparse
import csv
import hashlib
import json
import os
from pathlib import Path
import random
import statistics
import subprocess
import sys


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def certified_outcomes(path):
    """Keep incomplete, omitted and unknown rows in the evaluation denominator."""
    outcomes = {}
    with path.open(newline="") as stream:
        for row in csv.DictReader(stream):
            key = (row["edition"], row["division"], row["problem"], row["system"])
            if key in outcomes:
                raise ValueError(f"duplicate audit row: {key}")
            if row["strict_status"] == "VerifiedBad":
                raise ValueError(f"rejected proof blocks promotion: {key}")
            outcomes[key] = row["strict_status"] == "VerifiedGood"
    if not outcomes:
        raise ValueError(f"empty audit: {path}")
    return outcomes


def paired_delta(baseline, candidate):
    if baseline.keys() != candidate.keys():
        raise ValueError("paired audits do not cover the same problem/system set")
    gains = sum(candidate[key] and not baseline[key] for key in baseline)
    losses = sum(baseline[key] and not candidate[key] for key in baseline)
    return {
        "baseline_certified": sum(baseline.values()),
        "candidate_certified": sum(candidate.values()),
        "gains": gains,
        "losses": losses,
        "delta": gains - losses,
        "problems": len(baseline),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--problems-dir", type=Path, required=True)
    parser.add_argument("--edition", choices=("casc-30", "casc-j13"), required=True)
    parser.add_argument("--divisions", default="fne,feq,ueq")
    parser.add_argument("--schedule", help="same explicit schedule for both arms")
    parser.add_argument("--reps", type=int, default=2,
                        help="randomized paired blocks (minimum 2; add more if inconclusive)")
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--sharing-interval", type=int, default=0)
    parser.add_argument("--strict-time", type=int, default=120)
    parser.add_argument("--output", type=Path, required=True)
    root = Path(__file__).resolve().parents[2]
    parser.add_argument("--auditor", type=Path, default=root / "target/release/audit_casc_proofs")
    parser.add_argument("--proover", type=Path, default=root / "target/release/mrs-proover")
    args = parser.parse_args()
    if args.reps < 2 or args.sharing_interval < 0 or args.strict_time <= 0:
        parser.error("require reps >=2, sharing-interval >=0 and strict-time >0")
    binaries = {"baseline": args.baseline.resolve(), "candidate": args.candidate.resolve()}
    for path in (*binaries.values(), args.auditor, args.proover):
        if not path.is_file() or not os.access(path, os.X_OK):
            parser.error(f"missing executable: {path}")
    corpus = args.problems_dir.resolve()
    if not corpus.is_dir():
        parser.error(f"missing corpus: {corpus}")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    environment = os.environ.copy()
    environment.update(
        MRS_HARDWARE="casc", MRS_WORKERS="8",
        MRS_SHARED_POOL_INTERVAL=str(args.sharing_interval),
        CASC_PROBLEMS_ROOT=str(corpus), TPTP=str(corpus),
    )
    # The command line is the only schedule override for this experiment.
    environment.pop("MRS_SCHEDULE", None)
    environment.pop("MRS_PORTFOLIO", None)
    if args.schedule:
        environment["MRS_SCHEDULE"] = args.schedule
    rng = random.Random(args.seed)
    # Balance which arm runs first so a two-pair screen has one order each;
    # randomize the sequence to avoid a fixed baseline-first pattern.
    orders = ([['baseline', 'candidate']] * (args.reps // 2)
              + [['candidate', 'baseline']] * (args.reps // 2))
    if args.reps % 2:
        orders.append(rng.sample(list(binaries), 2))
    rng.shuffle(orders)
    hashes = {name: digest(path) for name, path in binaries.items()}
    manifest = {
        "schema": 1, "arguments": {key: str(value) if isinstance(value, Path) else value
                                  for key, value in vars(args).items()},
        "binary_sha256": hashes, "pair_orders": orders,
        "auditor_sha256": digest(args.auditor), "proover_sha256": digest(args.proover),
        "environment": {key: value for key, value in sorted(environment.items())
                        if key.startswith(("MRS_", "CASC_", "OMP_")) or key == "TPTP"},
    }
    (output / "experiment.json").write_text(json.dumps(manifest, indent=2) + "\n")
    pairs = []
    corpus_sha256 = None
    for index, order in enumerate(orders, start=1):
        audits = {}
        for arm in order:
            if digest(binaries[arm]) != hashes[arm]:
                raise ValueError(f"binary changed during experiment: {arm}")
            if digest(args.auditor) != manifest["auditor_sha256"] or digest(args.proover) != manifest["proover_sha256"]:
                raise ValueError("audit tools changed during experiment")
            run = output / f"pair-{index}" / arm
            env = environment | {"MRS_BINARY": str(binaries[arm])}
            subprocess.run([
                "bash", str(root / "crates/mrs-bench/casc.sh"),
                "--edition", args.edition, "--systems", "mrs", "--divisions", args.divisions,
                "--casc-times", "--jobs", "1", "--output", str(run),
            ], env=env, check=True, cwd=root)
            audit = run / "strict-audit"
            subprocess.run([
                str(args.auditor.resolve()), "--run", str(run / "run.csv"),
                "--problems-dir", str(corpus), "--proover", str(args.proover.resolve()),
                "--checks", "strict", "--strict-time", str(args.strict_time),
                "--jobs", "1", "--output", str(audit),
            ], env=env, check=True, cwd=root)
            identity = json.loads((audit / "audit-identity.json").read_text())
            if corpus_sha256 is None:
                corpus_sha256 = identity["corpus_sha256"]
            elif corpus_sha256 != identity["corpus_sha256"]:
                raise ValueError("corpus/include contents changed between paired arms")
            audits[arm] = certified_outcomes(audit / "audit.csv")
        pairs.append(paired_delta(audits["baseline"], audits["candidate"]))
        deltas = [pair["delta"] for pair in pairs]
        summary = {
            "pairs": pairs, "mean_paired_delta": statistics.mean(deltas),
            "observed_delta_range": [min(deltas), max(deltas)],
            "interpretation": "descriptive paired evidence; two pairs are a screen, add pairs if outcomes disagree or the effect is marginal; no automatic promotion or division-wide noise extrapolation",
        }
        (output / "paired-summary.json").write_text(json.dumps(summary, indent=2) + "\n")
        print(f"pair {index}: certified delta {deltas[-1]:+d}", flush=True)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
