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
        (
            "superposition resolvent drops the target clause",
            "append_literal_copy(&mut literals, &rewritten_target.literals[j]);",
            "if j == usize::MAX { append_literal_copy(&mut literals, &rewritten_target.literals[j]); }",
        ),
        (
            "DAG step skips verified replay",
            "|| !check_superposition_and_replay(step, remaining_work)",
            "|| false",
        ),
        (
            "DAG step ignores cited source parent",
            "if !same_clause_exact(&step.source, &nodes[node.parents[0]].clause)",
            "if false",
        ),
        (
            "DAG node validation skips superposition replay",
            "let checked = check_dag_superposition_step(node, &dag.nodes,\n            work_limit - work);\n        checked",
            "let checked = true;\n        checked",
        ),
        (
            "certifier skips node validation",
            "if !validate_dag_node(dag, i, per_node_budget) { return DagVerdict::Unknown; }",
            "if false { return DagVerdict::Unknown; }",
        ),
        (
            "input validator forges membership",
            "if same_clause_exact(&node.clause, &dag.input_clauses[k]) {",
            "if true {",
        ),
        (
            "node validator admits forward parents",
            "if node.parents[parent] >= index { return false; }",
            "if false { return false; }",
        ),
        (
            "certifier accepts a nonempty root",
            "if dag.nodes[dag.root].clause.literals.len() != 0 { return DagVerdict::Rejected; }",
            "if false { return DagVerdict::Rejected; }",
        ),
        (
            "resolution checker accepts a forged conclusion",
            "same_clause_exact(&expected, conclusion)",
            "true",
        ),
        (
            "resolution checker ignores predicate identity",
            "if right_predicate != predicate { return false; }",
            "if false { return false; }",
        ),
        (
            "resolution checker ignores oriented polarity",
            "if !left.literals[left_pivot].positive || right_literal.positive {",
            "if false {",
        ),
        (
            "DAG resolution skips rule replay",
            "let checked = check_nullary_resolution(&nodes[node.parents[0]].clause,",
            "let checked = true || check_nullary_resolution(&nodes[node.parents[0]].clause,",
        ),
        (
            "DAG resolution checks a parent instead of the conclusion",
            "&nodes[node.parents[1]].clause, lp, rp, &node.clause, work_limit - work);",
            "&nodes[node.parents[1]].clause, lp, rp, &nodes[node.parents[0]].clause, work_limit);",
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

        closure_guard = (
            "match check_root_ancestor_closure(dag, closure_budget) {\n"
            "        Some(true) => {},\n"
            "        Some(false) => return DagVerdict::Rejected,\n"
            "        None => return DagVerdict::Unknown,\n"
            "    }"
        )
        if source.count(closure_guard) != 1:
            print("root-ancestor closure mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_without_root_closure.rs"
        mutated_path.write_text(source.replace(closure_guard, "", 1))
        binary = pathlib.Path(temp_dir) / "first-order-core-without-root-closure"
        subprocess.run(
            [str(verus), "--triggers-mode", "silent", "--compile", str(mutated_path),
             "-o", str(binary)],
            check=True,
        )
        runtime = subprocess.run(
            [str(binary)], text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
            check=False,
        )
        if runtime.returncode == 0:
            print("runtime accepted mutation: certifier skips root-ancestor closure", file=sys.stderr)
            return 1
        print("runtime mutation rejected: certifier skips root-ancestor closure")

        input_charge = (
            "if !charge_clause_structure(&node.clause, &mut work, work_limit) {\n"
            "                return false;\n"
            "            }\n"
            "            if !charge_clause_structure(&dag.input_clauses[k], &mut work, work_limit) {"
        )
        if source.count(input_charge) != 1:
            print("input comparison budget mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_without_input_compare_charge.rs"
        mutated_path.write_text(source.replace(
            input_charge,
            "if false { return false; }\n"
            "            if !charge_clause_structure(&dag.input_clauses[k], &mut work, work_limit) {",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-without-input-compare-charge"
        subprocess.run(
            [str(verus), "--triggers-mode", "silent", "--compile", str(mutated_path),
             "-o", str(binary)],
            check=True,
        )
        runtime = subprocess.run(
            [str(binary)], text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
            check=False,
        )
        if runtime.returncode == 0:
            print("runtime accepted mutation: input comparison omits node-clause charge",
                  file=sys.stderr)
            return 1
        print("runtime mutation rejected: input leaf comparisons omit repeated node charge")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
