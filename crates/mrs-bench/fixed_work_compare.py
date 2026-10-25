#!/usr/bin/env python3
"""Compare two frozen binaries/queue policies at identical selected-clause work.

Archives each arm's output and identity. A timing ratio is reported only when
both runs reach the declared processed ceiling with matching selection digests,
generation counts and resource reasons. Use release builds for performance.
Hashing is enabled equally in both arms and its overhead remains in timings.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import random
import shlex
import subprocess
import time


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def detail(stderr):
    lines = [line for line in stderr.splitlines() if line.startswith("% SZS detail ")]
    if len(lines) != 1:
        raise ValueError("expected exactly one final search detail line")
    return dict(token.split("=", 1) for token in shlex.split(lines[0]) if "=" in token)


def equivalent_work(baseline, candidate, processed):
    required = ("processed", "generated", "passive", "lrs_discarded", "weight_discarded",
                "trajectory_sha256", "resource_reason")
    for arm in (baseline, candidate):
        if any(not arm.get(key) for key in required):
            raise ValueError("incomplete work telemetry")
        if arm.get("processed") != str(processed):
            raise ValueError("run did not reach the declared fixed-work ceiling")
        if arm.get("resource_reason") != "max_processed":
            raise ValueError("fixed-work run stopped for another reason")
        if not arm.get("trajectory_sha256"):
            raise ValueError("missing selection trajectory digest")
    for key in ("trajectory_sha256", "generated", "passive", "lrs_discarded", "weight_discarded"):
        if baseline.get(key) != candidate.get(key):
            raise ValueError(f"work trajectory differs: {key}")
    return True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--problem", type=Path, required=True)
    parser.add_argument("--schedule", default="casc_ueq")
    parser.add_argument("--strategy", type=int, default=4, choices=range(1, 16))
    parser.add_argument("--processed", type=int, default=1000)
    parser.add_argument("--guard-seconds", type=int, default=120)
    parser.add_argument("--reps", type=int, default=5)
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--candidate-specialize", action="store_true")
    parser.add_argument("--candidate-compact", action="store_true")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if min(args.processed, args.guard_seconds, args.reps) <= 0:
        parser.error("work, guard and repetition counts must be positive")
    binaries = {"baseline": args.baseline.resolve(), "candidate": args.candidate.resolve()}
    problem = args.problem.resolve()
    hashes = {name: digest(path) for name, path in binaries.items()}
    problem_hash = digest(problem)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    env = os.environ.copy()
    env.update(MRS_NO_LRS="1", MRS_SHARED_POOL_INTERVAL="0", MRS_MAX_PROCESSED=str(args.processed),
               MRS_TRAJECTORY_DIGEST="1", MRS_SPECIALIZE_QUEUES="0", MRS_COMPACT_QUEUES="0",
               MRS_PREPHASE="0")
    rng = random.Random(args.seed)
    orders = [rng.sample(list(binaries), 2) for _ in range(args.reps)]
    manifest = {"schema": 1, "binary_sha256": hashes, "problem_sha256": problem_hash,
                "problem": str(problem), "arguments": {key: str(value) if isinstance(value, Path) else value
                                                       for key, value in vars(args).items()},
                "orders": orders, "environment": {key: value for key, value in sorted(env.items())
                                                   if key.startswith("MRS_") or key == "TPTP"}}
    (output / "experiment.json").write_text(json.dumps(manifest, indent=2) + "\n")
    pairs = []
    reference = None
    for index, order in enumerate(orders, 1):
        arms = {}
        for arm in order:
            if digest(binaries[arm]) != hashes[arm] or digest(problem) != problem_hash:
                raise ValueError("binary or problem changed during measurement")
            arm_env = env.copy()
            if arm == "candidate":
                arm_env.update(MRS_SPECIALIZE_QUEUES=str(int(args.candidate_specialize)),
                               MRS_COMPACT_QUEUES=str(int(args.candidate_compact)))
            command = [str(binaries[arm]), "--workers", "1", "--time", str(args.guard_seconds),
                       "--schedule", args.schedule, "--strategy", str(args.strategy), str(problem)]
            start = time.monotonic()
            result = subprocess.run(command, env=arm_env, capture_output=True, text=True,
                                    timeout=args.guard_seconds + 10)
            elapsed = time.monotonic() - start
            prefix = output / f"pair-{index}-{arm}"
            prefix.with_suffix(".stdout").write_text(result.stdout)
            prefix.with_suffix(".stderr").write_text(result.stderr)
            if result.returncode != 0:
                raise ValueError(f"{arm} exited {result.returncode}")
            arms[arm] = {"elapsed_s": elapsed, "detail": detail(result.stderr)}
        equivalent_work(arms["baseline"]["detail"], arms["candidate"]["detail"], args.processed)
        if reference is not None:
            equivalent_work(reference, arms["baseline"]["detail"], args.processed)
        reference = arms["baseline"]["detail"]
        pairs.append({"arms": arms, "candidate_over_baseline_elapsed":
                      arms["candidate"]["elapsed_s"] / arms["baseline"]["elapsed_s"]})
        (output / "comparison.json").write_text(json.dumps({"pairs": pairs,
            "interpretation": "paired fixed-work diagnostics including hashing/startup; no automatic promotion"}, indent=2) + "\n")
        print(f"pair {index}: equivalent selected work, elapsed ratio {pairs[-1]['candidate_over_baseline_elapsed']:.3f}")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        raise SystemExit(f"fixed-work comparison: {error}")
