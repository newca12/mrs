#!/usr/bin/env python3
"""Check that Verus rejects a broken executable/specification refinement."""

from __future__ import annotations

import pathlib
import subprocess
import sys
import tempfile


def main() -> int:
    if len(sys.argv) != 3:
        print(f"usage: {sys.argv[0]} VERUS_BIN FIRST_ORDER_CORE_RS", file=sys.stderr)
        return 2

    verus = pathlib.Path(sys.argv[1])
    source_path = pathlib.Path(sys.argv[2])
    source = source_path.read_text()
    mutations = [
        (
            "literal substitution",
            "atom: substitute_atom_exec(&literal.atom, bindings),",
            "atom: copy_atom(&literal.atom),",
        ),
        (
            "literal polarity comparison",
            "left.positive == right.positive && same_atom(&left.atom, &right.atom)",
            "true && same_atom(&left.atom, &right.atom)",
        ),
        (
            "selected term-position lookup",
            "let result = subterm_at_exec(&root, path, 0);",
            "let result = Some(copy_term(&root));",
        ),
    ]

    with tempfile.TemporaryDirectory(prefix="mrs-verus-mutation-") as temp_dir:
        for index, (name, correct, mutated) in enumerate(mutations):
            if source.count(correct) != 1:
                print(f"{name}: mutation anchor is missing or ambiguous", file=sys.stderr)
                return 2

            mutated_path = pathlib.Path(temp_dir) / f"first_order_core_mutated_{index}.rs"
            mutated_path.write_text(source.replace(correct, mutated, 1))
            result = subprocess.run(
                [str(verus), "--triggers-mode", "silent", str(mutated_path)],
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                check=False,
            )
            if result.returncode == 0:
                print(f"Verus accepted mutation: {name}", file=sys.stderr)
                return 1
            print(f"mutation rejected: {name}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
