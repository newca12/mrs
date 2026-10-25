#!/usr/bin/env python3
"""Freeze corpus content and a conservative family-disjoint tuning/holdout split.

Default families are TPTP's three-letter domains (deliberately conservative).
An explicit JSON name->family mapping can define finer reviewed families.
This is a dataset manifest, not proof that arbitrary variants are independent.
"""

import argparse
import hashlib
import json
from pathlib import Path


def split_family(family, seed, holdout_percent):
    key = f"mrs-corpus-split-v1\0{seed}\0{family}".encode()
    bucket = int.from_bytes(hashlib.sha256(key).digest()[:8], "big") % 100
    return "holdout" if bucket < holdout_percent else "development"


def freeze(root, seed=42, holdout_percent=20, families=None):
    if not 0 < holdout_percent < 100:
        raise ValueError("holdout percentage must be in 1..99")
    files = []
    rows = []
    for path in sorted(root.rglob("*")):
        if path.is_symlink():
            raise ValueError(f"materialize corpus symlink before freezing: {path}")
        if not path.is_file():
            continue
        relative = path.relative_to(root).as_posix()
        with path.open("rb") as stream:
            digest = hashlib.file_digest(stream, "sha256").hexdigest()
        files.append({"path": relative, "sha256": digest, "bytes": path.stat().st_size})
        if path.suffix != ".p":
            continue
        family = families.get(relative) if families is not None else path.stem[:3]
        if not isinstance(family, str) or not family:
            raise ValueError(f"missing/non-string family for {relative}")
        rows.append({"path": relative, "family": family,
                     "split": split_family(family, seed, holdout_percent)})
    if not rows:
        raise ValueError("corpus contains no .p problems")
    present = {row["split"] for row in rows}
    if len(present) != 2:
        raise ValueError("split has an empty arm; choose a larger corpus or another declared seed")
    digest = hashlib.sha256(json.dumps(files, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    return {"schema": 1, "seed": seed, "holdout_percent": holdout_percent,
            "family_policy": "explicit-reviewed-map" if families is not None else "tptp-domain-prefix",
            "corpus_sha256": digest, "files": files, "problems": rows}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--holdout-percent", type=int, default=20)
    parser.add_argument("--family-map", type=Path)
    args = parser.parse_args()
    try:
        root = args.root.resolve(strict=True)
        output = args.output.resolve()
        if not root.is_dir() or output == root or root in output.parents:
            raise ValueError("output must be outside the corpus directory")
        families = json.loads(args.family_map.read_text()) if args.family_map else None
        manifest = freeze(root, args.seed, args.holdout_percent, families)
        output.parent.mkdir(parents=True, exist_ok=True)
        with output.open("x") as stream:
            json.dump(manifest, stream, indent=2)
            stream.write("\n")
    except (OSError, ValueError) as error:
        parser.exit(1, f"freeze-corpus: {error}\n")


if __name__ == "__main__":
    main()
