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
            "exact resolution ignores atom identity",
            "if !same_atom(&left.literals[lp].atom, &right.literals[rp].atom) { return false; }",
            "if false { return false; }",
        ),
        (
            "exact resolution ignores pivot polarity",
            "if !left.literals[lp].positive || right.literals[rp].positive { return false; }",
            "if false { return false; }",
        ),
        (
            "freshening ignores forbidden variable images",
            "if contains_u64(avoid, fresh) { return false; }",
            "if false { return false; }",
        ),
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
            "let same = same_clause_exact(&expected, conclusion);",
            "let same = true;",
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
        (
            "tokenizer span correspondence omits inductive decimal append lemma",
            "lemma_decimal_digits_value_append(\n                        bytes@.subrange(token_start as int, cursor as int), byte);",
            "// omitted",
        ),
        (
            "first-order resolution ignores unified atom equality",
            "if !same_atom(&inst_left.literals[certificate.left_pivot].atom, &inst_right.literals[certificate.right_pivot].atom) {",
            "if false {",
        ),
        (
            "first-order resolution ignores pivot polarity",
            "if !certificate.left.literals[certificate.left_pivot].positive\n        || certificate.right.literals[certificate.right_pivot].positive\n    {",
            "if false {",
        ),
        (
            "DAG first-order resolution skips rule replay",
            "|| !check_first_order_resolution_and_replay(step, remaining_work)",
            "|| false",
        ),
        (
            "equality resolution ignores pivot polarity",
            "if pivot_lit.positive {\n        return false;\n    }",
            "if false {\n        return false;\n    }",
        ),
        (
            "equality resolution ignores unified terms equality",
            "if !same_term(&left_inst, &right_inst) {\n        return false;\n    }",
            "if false {\n        return false;\n    }",
        ),
        (
            "DAG equality resolution skips rule replay",
            "|| !check_equality_resolution_and_replay(step, remaining_work)",
            "|| false",
        ),
        (
            "clause factoring ignores polarity agreement",
            "if first_lit.positive != second_lit.positive {\n        return false;\n    }",
            "if false {\n        return false;\n    }",
        ),
        (
            "clause factoring ignores unified atom equality",
            "if !same_atom(&inst_clause.literals[certificate.first].atom, &inst_clause.literals[certificate.second].atom) {\n        return false;\n    }",
            "if false {\n        return false;\n    }",
        ),
        (
            "DAG clause factoring skips rule replay",
            "|| !check_clause_factoring_and_replay(step, remaining_work)",
            "|| false",
        ),
    ]

    with tempfile.TemporaryDirectory(prefix="mrs-verus-mutation-") as temp_dir:
        cheating_path = pathlib.Path(temp_dir) / "cheating_probe.rs"
        cheating_path.write_text(
            "use vstd::prelude::*;\n"
            "verus! { proof fn cheating_probe() { assume(false); } }\n"
            "fn main() {}\n"
        )
        subprocess.run([str(verus), str(cheating_path)], check=True)
        no_cheating = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", str(cheating_path)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
        )
        if no_cheating.returncode == 0:
            print("Verus --no-cheating accepted an assumption probe", file=sys.stderr)
            return 1
        print("mutation rejected: Verus --no-cheating rejects a valid assumption probe")

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
            print(f"mutation rejected: {name}", flush=True)

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

        duplicate_id_guard = "if dag.nodes[prior].id == node.id {\n                return DagVerdict::Rejected;\n            }"
        if source.count(duplicate_id_guard) != 1:
            print("duplicate node-ID mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_accepts_duplicate_node_ids.rs"
        mutated_path.write_text(source.replace(duplicate_id_guard, "if false {\n"
            "                return DagVerdict::Rejected;\n            }", 1))
        binary = pathlib.Path(temp_dir) / "first-order-core-accepts-duplicate-node-ids"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=False,
        )
        if compile_result.returncode != 0:
            if "invariant not satisfied" not in compile_result.stdout or \
                    "dag.nodes[j].id != node.id" not in compile_result.stdout:
                print("duplicate node-ID mutation failed for an unexpected reason:",
                      compile_result.stdout, file=sys.stderr)
                return 1
            print("mutation rejected: duplicate DAG node IDs violate the verified loop invariant")
        else:
            runtime = subprocess.run(
                [str(binary)], text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                check=False,
            )
            if runtime.returncode == 0:
                print("runtime accepted mutation: duplicate DAG node IDs", file=sys.stderr)
                return 1
            print("runtime mutation rejected: duplicate DAG node IDs")

        tokenizer_guard = "if byte < 48 || byte > 57 { return None; }"
        digit_mapping = "let digit = (byte - 48) as u64;"
        if source.count(tokenizer_guard) != 1:
            print("verified tokenizer mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_accepts_nondecimal.rs"
        mutated_source = source.replace(tokenizer_guard,
            "if byte < 48 || byte > 57 { if byte != 255 { return None; } }", 1)
        if mutated_source == source or mutated_source.count(digit_mapping) != 1:
            print("verified tokenizer digit-mapping mutation anchor is missing", file=sys.stderr)
            return 2
        mutated_source = mutated_source.replace(digit_mapping,
            "let digit = if byte == 255 { 0u64 } else { (byte - 48) as u64 };", 1)
        mutated_path.write_text(mutated_source)
        binary = pathlib.Path(temp_dir) / "first-order-core-accepts-nondecimal"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=False,
        )
        if compile_result.returncode != 0:
            if "is_ascii_digit" not in compile_result.stdout and "precondition not satisfied" not in compile_result.stdout:
                print("tokenizer mutation failed for an unexpected reason:", compile_result.stdout, file=sys.stderr)
                return 1
            print("mutation rejected: nondecimal token violates the verified ascii-digit invariant")
        else:
            malformed = subprocess.run(
                [str(binary), "--wire-v1"], input=b"1 0\xff0", stdout=subprocess.PIPE,
                stderr=subprocess.PIPE, check=True,
            )
            if malformed.stdout != b"MRS_VERUS 1 Unknown\n":
                print("runtime accepted mutation: tokenizer accepts nondecimal token", file=sys.stderr)
                return 1
            print("runtime mutation rejected: tokenizer accepts nondecimal bytes")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
