#!/usr/bin/env python3
"""Check that Verus rejects a broken executable/specification refinement."""

from __future__ import annotations

import concurrent.futures
import os
import pathlib
import subprocess
import sys
import tempfile

EXPECTED_VERIFIER_NEGATIVE_MUTATIONS = 58
EXPECTED_COMPILED_RUNTIME_MUTATIONS = 47


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
            "let Some(exact_atoms_match) = same_atom_with_work(\n"
            "        &left.literals[lp].atom, &right.literals[rp].atom, work, work_limit,\n"
            "    ) else { return false; };",
            "let Some(exact_atoms_match) = Some(true) else { return false; };",
        ),
        (
            "exact resolution ignores pivot polarity",
            "if !left.literals[lp].positive || right.literals[rp].positive { return false; }",
            "if false { return false; }",
        ),
        (
            "freshening ignores forbidden variable images",
            "if is_forbidden { return false; }",
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
            "|| !check_superposition_and_replay_with_work(step, work, work_limit)",
            "|| false",
        ),
        (
            "DAG step ignores cited source parent",
            "let Some(source_matches) = same_clause_exact_with_work(\n"
            "        &step.source, &nodes[node.parents[0]].clause, work, work_limit,\n"
            "    ) else { return false; };",
            "let Some(source_matches) = Some(true) else { return false; };",
        ),
        (
            "DAG node validation skips superposition replay",
            "check_dag_superposition_step_with_work(node, &dag.nodes, work, work_limit)",
            "true",
        ),
        (
            "certifier skips node validation",
            "        if !validate_dag_node_with_work(dag, i, work, work_limit) {\n"
            "            return DagVerdict::Unknown;\n"
            "        }",
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
            "let Some(same) = same_clause_exact_with_work(\n"
            "        &expected, conclusion, work, work_limit,\n"
            "    ) else { return false; };",
            "let Some(same) = Some(true) else { return false; };",
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
            "let checked = check_nullary_resolution_with_work(",
            "let checked = true || check_nullary_resolution_with_work(",
        ),
        (
            "DAG resolution checks a parent instead of the conclusion",
            "        &nodes[node.parents[1]].clause,\n"
            "        lp,\n"
            "        rp,\n"
            "        &node.clause,\n"
            "        work,\n"
            "        work_limit,\n"
            "    );",
            "        &nodes[node.parents[1]].clause,\n"
            "        lp,\n"
            "        rp,\n"
            "        &nodes[node.parents[0]].clause,\n"
            "        work,\n"
            "        work_limit,\n"
            "    );",
        ),
        (
            "tokenizer span correspondence omits inductive decimal append lemma",
            "lemma_decimal_digits_value_append(\n                        bytes@.subrange(token_start as int, cursor as int), byte);",
            "// omitted",
        ),
        (
            "first-order resolution ignores unified atom equality",
            "let Some(exact_atoms_match) = same_atom_with_work(\n"
            "        left_pivot_atom, right_pivot_atom, work, work_limit,\n"
            "    ) else { return false; };",
            "let Some(exact_atoms_match) = Some(true) else { return false; };",
        ),
        (
            "first-order resolution ignores pivot polarity",
            "if !certificate.left.literals[certificate.left_pivot].positive\n        || certificate.right.literals[certificate.right_pivot].positive\n    {",
            "if false {",
        ),
        (
            "DAG first-order resolution skips rule replay",
            "|| !check_first_order_resolution_and_replay_with_work(step, work, work_limit)",
            "|| false",
        ),
        (
            "equality resolution ignores pivot polarity",
            "if pivot_lit.positive {\n        return false;\n    }",
            "if false {\n        return false;\n    }",
        ),
        (
            "equality resolution ignores unified terms equality",
            "if !pivot_terms_match {\n        return false;\n    }",
            "if false {\n        return false;\n    }",
        ),
        (
            "DAG equality resolution skips rule replay",
            "|| !check_equality_resolution_and_replay_with_work(step, work, work_limit)",
            "|| false",
        ),
        (
            "clause factoring ignores polarity agreement",
            "if first_lit.positive != second_lit.positive {\n        return false;\n    }",
            "if false {\n        return false;\n    }",
        ),
        (
            "clause factoring ignores unified atom equality",
            "let Some(factored_atoms_match) = same_atom_with_work(\n"
            "        &inst_clause.literals[certificate.first].atom,\n"
            "        &inst_clause.literals[certificate.second].atom,\n"
            "        work,\n"
            "        work_limit,\n"
            "    ) else { return false; };",
            "let Some(factored_atoms_match) = Some(true) else { return false; };",
        ),
        (
            "DAG clause factoring skips rule replay",
            "|| !check_clause_factoring_and_replay_with_work(step, work, work_limit)",
            "|| false",
        ),
        (
            "equality factoring ignores premise literal positive polarity",
            "if !first_lit.positive || !second_lit.positive {\n        return false;\n    }",
            "if false {\n        return false;\n    }",
        ),
        (
            "equality factoring ignores unified term equality",
            "if !equal { return None; }",
            "if false { return None; }",
        ),
        (
            "DAG equality factoring skips rule replay",
            "|| !check_equality_factoring_and_replay_with_work(step, work, work_limit)",
            "|| false",
        ),
        (
            "subsumption resolution ignores pivot polarity",
            "if target_pivot_lit.positive == active_pivot_lit.positive {\n        return false;\n    }",
            "if false {\n        return false;\n    }",
        ),
        (
            "subsumption resolution ignores pivot atom equality",
            "let Some(pivot_atoms_match) = same_atom_with_work(\n"
            "        &target_pivot_lit.atom, &active_pivot_lit.atom, work, work_limit,\n"
            "    ) else { return false; };",
            "let Some(pivot_atoms_match) = Some(true) else { return false; };",
        ),
        (
            "subsumption resolution ignores non-pivot literal match",
            "let Some(literals_match) = same_literal_with_work(\n"
            "                &inst_active.literals[j], &certificate.target.literals[target_idx],\n"
            "                work, work_limit,\n"
            "            ) else { return false; };",
            "let Some(literals_match) = Some(true) else { return false; };",
        ),
        (
            "DAG subsumption resolution skips rule replay",
            "|| !check_subsumption_resolution_and_replay_with_work(step, work, work_limit)",
            "|| false",
        ),
        (
            "weakening ignores literal agreement",
            "let Some(literal_matches) = same_literal_with_work(\n"
            "            &certificate.parent.literals[i], &certificate.conclusion.literals[target_idx],\n"
            "            work, work_limit,\n"
            "        ) else { return false; };",
            "let Some(literal_matches) = Some(true) else { return false; };",
        ),
        (
            "DAG weakening skips rule replay",
            "|| !check_weakening_and_replay_with_work(step, work, work_limit)",
            "|| false",
        ),
        (
            "demodulation ignores equality positive polarity",
            "if !eq_lit.positive {\n        return false;\n    }",
            "if false {\n        return false;\n    }",
        ),
        (
            "demodulation ignores unified term equality",
            "let Some(selected_term_matches) = same_term_with_work(\n"
            "        &from_inst, &selected_subterm, work, work_limit,\n"
            "    ) else { return false; };",
            "let Some(selected_term_matches) = Some(true) else { return false; };",
        ),
        (
            "DAG demodulation skips rule replay",
            "|| !check_demodulation_and_replay_with_work(step, work, work_limit)",
            "|| false",
        ),
        (
            "instantiation ignores conclusion agreement",
            "let Some(expected) = substitute_clause_exec_with_work(\n"
            "        &certificate.parent, &certificate.substitution, work, work_limit,\n"
            "    ) else { return false; };\n"
            "    let Some(conclusion_matches) = same_clause_exact_with_work(\n"
            "        &expected, &certificate.conclusion, work, work_limit,\n"
            "    ) else { return false; };",
            "let Some(expected) = substitute_clause_exec_with_work(\n"
            "        &certificate.parent, &certificate.substitution, work, work_limit,\n"
            "    ) else { return false; };\n"
            "    let Some(conclusion_matches) = Some(true) else { return false; };",
        ),
        (
            "DAG instantiation skips rule replay",
            "|| !check_instantiation_and_replay_with_work(step, work, work_limit)",
            "|| false",
        ),
        (
            "condensation ignores literal agreement",
            "let Some(literal_matches) = same_literal_with_work(\n"
            "            &instantiated, &certificate.conclusion.literals[target_idx], work, work_limit,\n"
            "        ) else { return false; };",
            "let Some(literal_matches) = Some(true) else { return false; };",
        ),
        (
            "DAG condensation skips rule replay",
            "|| !check_condensation_and_replay_with_work(step, work, work_limit)",
            "|| false",
        ),
        (
            "reflexivity ignores identical terms",
            "let Some(terms_match) = same_term_with_work(s, t, work, work_limit) else {\n"
            "        return false;\n"
            "    };",
            "let Some(terms_match) = Some(true) else { return false; };",
        ),
        (
            "transitivity ignores valid chained orientation",
            "                if step_valid {\n"
            "                    assert(view_clause(*clause).literals[k as int] == view_literal(*p_lit));",
            "if true {",
        ),
        (
            "DAG transitivity skips rule replay",
            "    // Keep parent structure and all transitivity comparisons on the node meter.\n"
            "    if !check_transitivity_and_replay_with_work(\n"
            "        &nodes[node.parents[0]].clause,\n"
            "        &nodes[node.parents[1]].clause,\n"
            "        &node.clause,\n"
            "        work,\n"
            "        work_limit,\n"
            "    ) {\n"
            "        return false;\n"
            "    }",
            "if false {\n        return false;\n    }",
        ),
        (
            "symmetry ignores valid commuted orientation",
            "let Some(step_valid) = check_symmetry_step_exec_with_work(\n"
            "                    s, t, u, v, work, work_limit,\n"
            "                ) else { return None; };\n"
            "                if step_valid {",
            "let Some(step_valid) = Some(true) else { return None; };\n"
            "                if step_valid {",
        ),
        (
            "DAG symmetry skips rule replay",
            "    // Keep parent structure and all symmetry comparisons on the node meter.\n"
            "    if !check_symmetry_and_replay_with_work(\n"
            "        &nodes[node.parents[0]].clause,\n"
            "        &node.clause,\n"
            "        work,\n"
            "        work_limit,\n"
            "    ) {\n"
            "        return false;\n"
            "    }",
            "if false {\n        return false;\n    }",
        ),
        (
            "excluded middle ignores opposite polarity or atom agreement",
            "let Some(step_valid) = check_excluded_middle_step_exec_with_work(\n"
            "        l1, l2, work, work_limit,\n"
            "    ) else { return false; };",
            "let Some(step_valid) = Some(true) else { return false; };",
        ),
        (
            "DAG excluded middle skips rule replay",
            "    // Continue per-node charges through the polarity and atom checks.\n"
            "    if !check_excluded_middle_and_replay_with_work(\n"
            "        &node.clause,\n"
            "        work,\n"
            "        work_limit,\n"
            "    ) {\n"
            "        return false;\n"
            "    }",
            "if false {\n        return false;\n    }",
        ),
        (
            "congruence ignores function symbol or argument agreement",
            "let Some(step_valid) = check_congruence_step_exec_with_work(\n"
            "                    s, t, u, v, work, work_limit,\n"
            "                ) else { return None; };\n"
            "                if step_valid {",
            "let Some(step_valid) = Some(true) else { return None; };\n"
            "                if step_valid {",
        ),
        (
            "nested congruence ignores unchanged binary sibling",
            "let Some(sibling_same) = same_term_with_work(b1, b2, work, work_limit) else {\n"
            "                    return None;\n"
            "                };",
            "let sibling_same = true;",
        ),
        (
            "nested congruence ignores inner unary function symbol",
            "(ExecTerm::App1(f1, a1), ExecTerm::App1(f2, a2)) => {\n"
            "            if *f1 != *f2 { return Some(false); }",
            "(ExecTerm::App1(f1, a1), ExecTerm::App1(f2, a2)) => {\n"
            "            if false { return Some(false); }",
        ),
        (
            "DAG congruence skips rule replay",
            "    // Keep parent structure and recursive term comparisons on the node meter.\n"
            "    if !check_congruence_and_replay_with_work(\n"
            "        &nodes[node.parents[0]].clause,\n"
            "        &node.clause,\n"
            "        work,\n"
            "        work_limit,\n"
            "    ) {\n"
            "        return false;\n"
            "    }",
            "if false {\n        return false;\n    }",
        ),
        (
            "tautology skips clause validation",
            "let Some(is_tautology) = check_clause_tautology_exec_with_work(\n"
            "        conclusion, work, work_limit,\n"
            "    ) else { return false; };",
            "let Some(is_tautology) = Some(true) else { return false; };",
        ),
        (
            "DAG tautology skips rule replay",
            "    // Keep the node counter through the bounded clause scan.\n"
            "    if !check_tautology_and_replay_with_work(\n"
            "        &node.clause,\n"
            "        work,\n"
            "        work_limit,\n"
            "    ) {\n"
            "        return false;\n"
            "    }",
            "if false {\n        return false;\n    }",
        ),
    ]

    if len(mutations) != EXPECTED_VERIFIER_NEGATIVE_MUTATIONS:
        print(
            "verifier-negative mutation inventory changed: "
            f"expected {EXPECTED_VERIFIER_NEGATIVE_MUTATIONS}, found {len(mutations)}",
            file=sys.stderr,
        )
        return 2
    compiled_runtime_mutations = 0

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

        def check_mutation(item: tuple[int, tuple[str, str, str]]) -> tuple[int, str, int, str]:
            index, (name, correct, mutated) = item
            if source.count(correct) != 1:
                return (index, name, 2, f"{name}: mutation anchor is missing or ambiguous")
            mutated_path = pathlib.Path(temp_dir) / f"first_order_core_mutated_{index}.rs"
            mutated_path.write_text(source.replace(correct, mutated, 1))
            result = subprocess.run(
                [str(verus), "--triggers-mode", "silent", "--num-threads", "1", str(mutated_path)],
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                check=False,
            )
            if result.returncode == 0:
                return (index, name, 1, f"Verus accepted mutation: {name}")
            return (index, name, 0, f"mutation rejected: {name}")

        # Each verifier process can consume substantial memory and CPU; keep
        # the default concurrency bounded even on large hosts. CI/developers
        # may explicitly raise it when resources permit.
        workers = int(os.environ.get("VERUS_MUTATION_WORKERS", 2))
        with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as executor:
            futures = [executor.submit(check_mutation, item) for item in enumerate(mutations)]
            for future in concurrent.futures.as_completed(futures):
                index, name, code, msg = future.result()
                if code != 0:
                    print(msg, file=sys.stderr)
                    return code
                print(msg, flush=True)

        closure_guard = (
            "match check_root_ancestor_closure_with_work(dag, work, work_limit) {\n"
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
        compiled_runtime_mutations += 1

        input_charge = (
            "if !charge_clause_structure(&node.clause, work, work_limit) {\n"
            "                return false;\n"
            "            }\n"
            "            if !charge_clause_structure(&dag.input_clauses[k], work, work_limit) {"
        )
        if source.count(input_charge) != 1:
            print("input comparison budget mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_without_input_compare_charge.rs"
        mutated_path.write_text(source.replace(
            input_charge,
            "if false { return false; }\n"
            "            if !charge_clause_structure(&dag.input_clauses[k], work, work_limit) {",
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
        compiled_runtime_mutations += 1

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
        compiled_runtime_mutations += 1

        target_freshening_anchors = (
            "|| !extend_unique_with_work(\n"
            "        &mut avoid_target, &fresh_source_variables, work, work_limit,\n"
            "    )",
            "|| !extend_unique_with_work(\n"
            "        &mut target_avoid, &fresh_source_variables, work, work_limit,\n"
            "    )",
        )
        mutated_source = source
        for anchor in target_freshening_anchors:
            if mutated_source.count(anchor) != 1:
                print(
                    "target-parent freshness mutation anchor is missing or ambiguous: "
                    f"{anchor}",
                    file=sys.stderr,
                )
                return 2
            mutated_source = mutated_source.replace(
                anchor,
                "|| { let _fresh_source_variable_count = fresh_source_variables.len(); false }",
                1,
            )
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_without_target_freshening_guard.rs"
        mutated_path.write_text(mutated_source)
        binary = pathlib.Path(temp_dir) / "first-order-core-without-target-freshening-guard"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "target-parent freshness mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "superposition parent-variable collision regression failed"
        if runtime.returncode == 0:
            print(
                "runtime accepted mutation: target freshening permits a source-variable collision",
                file=sys.stderr,
            )
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "target-parent freshness mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print(
            "runtime mutation rejected: target freshening permits a source-variable collision"
        )
        compiled_runtime_mutations += 1

        freshening_lookup_charge = (
            "        // Charge each binding-domain comparison and every node copied from a\n"
            "        // matching replacement term.\n"
            "        if !consume_work(work, work_limit) { return None; }"
        )
        if source.count(freshening_lookup_charge) != 1:
            print("freshening lookup-charge mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_without_freshening_lookup_charge.rs"
        mutated_path.write_text(source.replace(
            freshening_lookup_charge,
            "        // Mutant omits the binding-domain comparison charge.\n"
            "        if false { return None; }",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-without-freshening-lookup-charge"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "freshening lookup-charge mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "freshening cumulative work regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: freshening omits lookup work", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "freshening lookup-charge mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: freshening omits lookup work")
        compiled_runtime_mutations += 1

        clause_collection_charge = (
            "        // Charge the clause-literal iteration as well as recursive term visits.\n"
            "        if !consume_work(work, limit) { return false; }"
        )
        if source.count(clause_collection_charge) != 1:
            print("clause-collection work mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_without_clause_literal_charge.rs"
        mutated_path.write_text(source.replace(
            clause_collection_charge,
            "        // Mutant omits the clause-literal traversal charge.\n"
            "        if false { return false; }",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-without-clause-literal-charge"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "clause-collection charge mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "clause freshening cumulative work regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: clause collection omits literal work", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "clause-collection work mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: clause collection omits literal work")
        compiled_runtime_mutations += 1

        freshening_materialization_charge = (
            "    // Charge each term node visited during substitution materialization.\n"
            "    if !consume_work(work, work_limit) { return None; }"
        )
        if source.count(freshening_materialization_charge) != 1:
            print("freshening materialization mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_without_freshening_materialization_charge.rs"
        mutated_path.write_text(source.replace(
            freshening_materialization_charge,
            "    // Mutant omits a term-node materialization charge.\n"
            "    if false { return None; }",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-without-freshening-materialization-charge"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "freshening materialization mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "clause freshening cumulative work regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: freshening omits materialization work", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "freshening materialization mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: freshening omits materialization work")
        compiled_runtime_mutations += 1

        replay_substitution_meter = (
            "    let Some(instantiated_source) = substitute_clause_exec_with_work(\n"
            "        source, unifier, work, work_limit,\n"
            "    ) else { return None; };"
        )
        if source.count(replay_substitution_meter) != 1:
            print("replay substitution mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_without_replay_substitution_meter.rs"
        mutated_path.write_text(source.replace(
            replay_substitution_meter,
            "    let Some(instantiated_source) = Some(substitute_clause_exec(source, unifier))\n"
            "    else { return None; };",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-without-replay-substitution-meter"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "replay substitution mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "replay substitution cumulative work regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: replay omits parent substitution work", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "replay substitution mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: replay omits parent substitution work")
        compiled_runtime_mutations += 1

        replay_output_charges = (
            (
                "    // Charge the target traversal and replacement data before recursive replay.\n"
                "    if !charge_clause_structure(clause, work, work_limit) { return None; }",
                "replay target-replacement mutation anchor is missing or ambiguous",
                "first_order_core_without_replay_target_replacement_charge.rs",
                "first-order-core-without-replay-target-replacement-charge",
                "runtime accepted mutation: replay replacement omits target-structure work",
                "runtime mutation rejected: replay replacement omits target-structure work",
            ),
            (
                "    // Charge both input clauses before copying the resolvent literals.\n"
                "    if !charge_clause_structure(source, work, work_limit)\n"
                "        || !charge_clause_structure(rewritten_target, work, work_limit)\n"
                "    {\n"
                "        return None;\n"
                "    }",
                "replay conclusion-build mutation anchor is missing or ambiguous",
                "first_order_core_without_replay_conclusion_build_charge.rs",
                "first-order-core-without-replay-conclusion-build-charge",
                "runtime accepted mutation: replay conclusion assembly omits input work",
                "runtime mutation rejected: replay conclusion assembly omits input work",
            ),
            (
                "    // Charge both compared clause structures before exact ordered comparison.\n"
                "    if !charge_clause_structure(left, work, work_limit)\n"
                "        || !charge_clause_structure(right, work, work_limit)\n"
                "    {\n"
                "        return None;\n"
                "    }",
                "replay comparison mutation anchor is missing or ambiguous",
                "first_order_core_without_replay_comparison_charge.rs",
                "first-order-core-without-replay-comparison-charge",
                "runtime accepted mutation: replay conclusion comparison omits structure work",
                "runtime mutation rejected: replay conclusion comparison omits structure work",
            ),
        )
        for anchor, missing_message, filename, binary_name, accepted_message, rejected_message in replay_output_charges:
            if source.count(anchor) != 1:
                print(missing_message, file=sys.stderr)
                return 2
            mutated_path = pathlib.Path(temp_dir) / filename
            mutated_path.write_text(source.replace(
                anchor,
                anchor.splitlines()[0] + "\n    if false { return None; }",
                1,
            ))
            binary = pathlib.Path(temp_dir) / binary_name
            compile_result = subprocess.run(
                [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
                 str(mutated_path), "-o", str(binary)],
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                check=False,
                timeout=300,
            )
            if compile_result.returncode != 0:
                print(
                    "replay output mutant must pass Verus before its runtime test:\n"
                    + compile_result.stdout,
                    file=sys.stderr,
                )
                return 1
            runtime = subprocess.run(
                [str(binary)],
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                check=False,
                timeout=30,
            )
            expected_failure = "replay output cumulative work regression failed"
            if runtime.returncode == 0:
                print(accepted_message, file=sys.stderr)
                return 1
            if expected_failure not in runtime.stdout:
                print(
                    "replay output mutation failed for an unexpected reason:\n"
                    + runtime.stdout,
                    file=sys.stderr,
                )
                return 1
            print(rejected_message)
            compiled_runtime_mutations += 1

        superposition_preflight_charges = (
            (
                "    // Charge both path sequences before element-wise comparison.\n"
                "    if !charge_u64_sequence(left, work, work_limit)\n"
                "        || !charge_u64_sequence(right, work, work_limit)\n"
                "    {\n"
                "        return None;\n"
                "    }",
                "path-sequence comparison mutation anchor is missing or ambiguous",
                "first_order_core_without_path_sequence_charge.rs",
                "first-order-core-without-path-sequence-charge",
                "runtime accepted mutation: path comparison omits sequence work",
                "runtime mutation rejected: path comparison omits sequence work",
                "path sequence cumulative work regression failed",
            ),
            (
                "    // Charge the atom traversal and path before selecting/copying a subterm.\n"
                "    if !charge_atom_structure(atom, work, work_limit) { return None; }\n"
                "    if !charge_u64_sequence(path, work, work_limit) { return None; }",
                "selected-term lookup mutation anchor is missing or ambiguous",
                "first_order_core_without_selected_term_charge.rs",
                "first-order-core-without-selected-term-charge",
                "runtime accepted mutation: selected-term lookup omits structure work",
                "runtime mutation rejected: selected-term lookup omits structure work",
                "selected-term cumulative work regression failed",
            ),
        )
        for anchor, missing_message, filename, binary_name, accepted_message, rejected_message, expected_failure in superposition_preflight_charges:
            if source.count(anchor) != 1:
                print(missing_message, file=sys.stderr)
                return 2
            mutated_path = pathlib.Path(temp_dir) / filename
            mutated_path.write_text(source.replace(
                anchor,
                anchor.splitlines()[0] + "\n    if false { return None; }",
                1,
            ))
            binary = pathlib.Path(temp_dir) / binary_name
            compile_result = subprocess.run(
                [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
                 str(mutated_path), "-o", str(binary)],
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                check=False,
                timeout=300,
            )
            if compile_result.returncode != 0:
                print(
                    "superposition preflight mutant must pass Verus before its runtime test:\n"
                    + compile_result.stdout,
                    file=sys.stderr,
                )
                return 1
            runtime = subprocess.run(
                [str(binary)],
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                check=False,
                timeout=30,
            )
            if runtime.returncode == 0:
                print(accepted_message, file=sys.stderr)
                return 1
            if expected_failure not in runtime.stdout:
                print(
                    "superposition preflight mutation failed for an unexpected reason:\n"
                    + runtime.stdout,
                    file=sys.stderr,
                )
                return 1
            print(rejected_message)
            compiled_runtime_mutations += 1

        atom_comparison_charge = (
            "    // Charge both operands before the exact recursive atom comparison.\n"
            "    if !charge_atom_structure(left, work, work_limit)\n"
            "        || !charge_atom_structure(right, work, work_limit)\n"
            "    {\n"
            "        return None;\n"
            "    }"
        )
        if source.count(atom_comparison_charge) != 1:
            print("atom-comparison work mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_without_atom_comparison_charge.rs"
        mutated_path.write_text(source.replace(
            atom_comparison_charge,
            "    // Mutant omits recursive atom-comparison charges.\n"
            "    if false { return None; }",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-without-atom-comparison-charge"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "atom-comparison mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "atom comparison cumulative work regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: atom comparison omitted operand work", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "atom-comparison mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: atom comparison omitted operand work")
        compiled_runtime_mutations += 1

        clause_deletion_charge = (
            "    // Charge the clause before copying the retained literal context.\n"
            "    if !charge_clause_structure(clause, work, work_limit) { return None; }"
        )
        if source.count(clause_deletion_charge) != 1:
            print("clause-deletion work mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_without_clause_deletion_charge.rs"
        mutated_path.write_text(source.replace(
            clause_deletion_charge,
            "    // Mutant omits the retained-context structure charge.\n"
            "    if false { return None; }",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-without-clause-deletion-charge"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "clause-deletion mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "clause deletion cumulative work regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: clause deletion omitted structure work", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "clause-deletion mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: clause deletion omitted structure work")
        compiled_runtime_mutations += 1

        replay_meter_handoff = (
            "    // Continue the preflight counter through replay freshening and output.\n"
            "    let Some(target_variables) = collect_clause_variables_bounded_with_work(\n"
        )
        if source.count(replay_meter_handoff) != 1:
            print("superposition replay-meter handoff mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_resets_work_before_replay.rs"
        mutated_path.write_text(source.replace(
            replay_meter_handoff,
            "    // Mutant resets the shared counter before replay.\n"
            "    *work = 0;\n"
            "    let Some(target_variables) = collect_clause_variables_bounded_with_work(\n",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-resets-work-before-replay"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "replay meter-reset mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "superposition preflight/replay shared work regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: replay reset the shared work counter", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "replay meter-reset mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: replay reset the shared work counter")
        compiled_runtime_mutations += 1

        dag_superposition_handoff = (
            "    // Keep all DAG structure and parent-comparison charges in local replay.\n"
            "    if !conclusion_matches\n"
            "        || !check_superposition_and_replay_with_work(step, work, work_limit)\n"
            "    {"
        )
        if source.count(dag_superposition_handoff) != 1:
            print("DAG superposition meter handoff mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_resets_work_at_dag_superposition.rs"
        mutated_path.write_text(source.replace(
            dag_superposition_handoff,
            "    // Mutant isolates local replay from DAG work.\n"
            "    let mut isolated_replay_work: usize = 0;\n"
            "    if !conclusion_matches\n"
            "        || !check_superposition_and_replay_with_work(\n"
            "            step, &mut isolated_replay_work, work_limit,\n"
            "        )\n"
            "    {",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-resets-work-at-dag-superposition"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "DAG replay meter-reset mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "DAG superposition shared work regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: DAG checks were not charged to replay", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "DAG superposition meter-reset mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: DAG work was reset before local replay")
        compiled_runtime_mutations += 1

        dag_first_order_handoff = (
            "    // Keep DAG node and cited-parent charges in first-order resolution replay.\n"
            "    if !conclusion_matches\n"
            "        || !check_first_order_resolution_and_replay_with_work(step, work, work_limit)\n"
            "    {"
        )
        if source.count(dag_first_order_handoff) != 1:
            print("DAG first-order resolution work-handoff mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_resets_work_at_dag_first_order_resolution.rs"
        mutated_path.write_text(source.replace(
            dag_first_order_handoff,
            "    // Mutant isolates first-order replay from DAG work.\n"
            "    let mut isolated_resolution_work: usize = 0;\n"
            "    if !conclusion_matches\n"
            "        || !check_first_order_resolution_and_replay_with_work(\n"
            "            step, &mut isolated_resolution_work, work_limit,\n"
            "        )\n"
            "    {",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-resets-work-at-dag-first-order-resolution"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "DAG first-order replay meter-reset mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "DAG first-order resolution shared work regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: DAG first-order work was not shared", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "DAG first-order resolution meter-reset mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: DAG first-order work was reset before replay")
        compiled_runtime_mutations += 1

        dag_equality_resolution_handoff = (
            "    // Keep DAG node and cited-parent charges in equality-resolution replay.\n"
            "    if !conclusion_matches\n"
            "        || !check_equality_resolution_and_replay_with_work(step, work, work_limit)\n"
            "    {"
        )
        if source.count(dag_equality_resolution_handoff) != 1:
            print("DAG equality-resolution work-handoff mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_resets_work_at_dag_equality_resolution.rs"
        mutated_path.write_text(source.replace(
            dag_equality_resolution_handoff,
            "    // Mutant isolates equality-resolution replay from DAG work.\n"
            "    let mut isolated_equality_work: usize = 0;\n"
            "    if !conclusion_matches\n"
            "        || !check_equality_resolution_and_replay_with_work(\n"
            "            step, &mut isolated_equality_work, work_limit,\n"
            "        )\n"
            "    {",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-resets-work-at-dag-equality-resolution"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "DAG equality-resolution meter-reset mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "DAG equality resolution shared work regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: DAG equality-resolution work was not shared", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "DAG equality-resolution meter-reset mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: DAG equality-resolution work was reset before replay")
        compiled_runtime_mutations += 1

        dag_factoring_handoff = (
            "    // Keep DAG node and cited-parent charges in clause factoring replay.\n"
            "    if !conclusion_matches\n"
            "        || !check_clause_factoring_and_replay_with_work(step, work, work_limit)\n"
            "    {"
        )
        if source.count(dag_factoring_handoff) != 1:
            print("DAG factoring work-handoff mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_resets_work_at_dag_factoring.rs"
        mutated_path.write_text(source.replace(
            dag_factoring_handoff,
            "    // Mutant isolates clause factoring from DAG work.\n"
            "    let mut isolated_factoring_work: usize = 0;\n"
            "    if !conclusion_matches\n"
            "        || !check_clause_factoring_and_replay_with_work(\n"
            "            step, &mut isolated_factoring_work, work_limit,\n"
            "        )\n"
            "    {",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-resets-work-at-dag-factoring"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "DAG factoring meter-reset mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "DAG clause factoring shared work regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: DAG clause factoring work was not shared", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "DAG clause factoring meter-reset mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: DAG clause factoring work was reset before replay")
        compiled_runtime_mutations += 1

        equality_factoring_pivot_charge = (
            "    let Some(equal) = same_term_with_work(&left_instance, &right_instance, work, work_limit)\n"
            "    else { return None; };"
        )
        if source.count(equality_factoring_pivot_charge) != 1:
            print("equality-factoring pivot comparison mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_without_equality_factoring_pivot_charge.rs"
        mutated_path.write_text(source.replace(
            equality_factoring_pivot_charge,
            "    let equal = same_term(&left_instance, &right_instance);",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-without-equality-factoring-pivot-charge"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "equality-factoring pivot mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "equality factoring pivot work regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: equality-factoring pivot comparison was not charged", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "equality-factoring pivot mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: equality-factoring pivot comparison was unmetered")
        compiled_runtime_mutations += 1

        dag_equality_factoring_handoff = (
            "    // Keep DAG node and cited-parent charges in equality factoring replay.\n"
            "    if !conclusion_matches\n"
            "        || !check_equality_factoring_and_replay_with_work(step, work, work_limit)\n"
            "    {"
        )
        if source.count(dag_equality_factoring_handoff) != 1:
            print("DAG equality-factoring work-handoff mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_resets_work_at_dag_equality_factoring.rs"
        mutated_path.write_text(source.replace(
            dag_equality_factoring_handoff,
            "    // Mutant isolates equality factoring from DAG work.\n"
            "    let mut isolated_equality_factoring_work: usize = 0;\n"
            "    if !conclusion_matches\n"
            "        || !check_equality_factoring_and_replay_with_work(\n"
            "            step, &mut isolated_equality_factoring_work, work_limit,\n"
            "        )\n"
            "    {",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-resets-work-at-dag-equality-factoring"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "DAG equality-factoring meter-reset mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "DAG equality factoring shared work regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: DAG equality factoring work was not shared", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "DAG equality-factoring meter-reset mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: DAG equality-factoring work was reset before replay")
        compiled_runtime_mutations += 1

        subsumption_literal_charge = (
            "    // Charge literal polarity and both atom structures before recursive match.\n"
            "    if !consume_work(work, work_limit)\n"
            "        || !charge_atom_structure(&left.atom, work, work_limit)"
        )
        if source.count(subsumption_literal_charge) != 1:
            print("subsumption literal-comparison mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_without_subsumption_literal_charge.rs"
        mutated_path.write_text(source.replace(
            subsumption_literal_charge,
            "    // Mutant omits literal-polarity comparison work.\n"
            "    if false { return None; }\n"
            "    if !charge_atom_structure(&left.atom, work, work_limit)",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-without-subsumption-literal-charge"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "subsumption literal mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "subsumption literal comparison work regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: subsumption literal comparison was not charged", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "subsumption literal-comparison mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: subsumption literal comparison was unmetered")
        compiled_runtime_mutations += 1

        dag_subsumption_handoff = (
            "    // Keep cited-parent comparisons in subsumption-resolution replay's meter.\n"
            "    if !conclusion_matches\n"
            "        || !check_subsumption_resolution_and_replay_with_work(step, work, work_limit)\n"
            "    {"
        )
        if source.count(dag_subsumption_handoff) != 1:
            print("DAG subsumption work-handoff mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_resets_work_at_dag_subsumption.rs"
        mutated_path.write_text(source.replace(
            dag_subsumption_handoff,
            "    // Mutant isolates subsumption replay from DAG work.\n"
            "    let mut isolated_subsumption_work: usize = 0;\n"
            "    if !conclusion_matches\n"
            "        || !check_subsumption_resolution_and_replay_with_work(\n"
            "            step, &mut isolated_subsumption_work, work_limit,\n"
            "        )\n"
            "    {",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-resets-work-at-dag-subsumption"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "DAG subsumption meter-reset mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "DAG subsumption resolution shared work regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: DAG subsumption work was not shared", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "DAG subsumption meter-reset mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: DAG subsumption work was reset before replay")
        compiled_runtime_mutations += 1

        dag_weakening_handoff = (
            "    // Keep parent and conclusion structure comparisons in the weakening meter.\n"
            "    if !conclusion_matches\n"
            "        || !check_weakening_and_replay_with_work(step, work, work_limit)\n"
            "    {"
        )
        if source.count(dag_weakening_handoff) != 1:
            print("DAG weakening work-handoff mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_resets_work_at_dag_weakening.rs"
        mutated_path.write_text(source.replace(
            dag_weakening_handoff,
            "    // Mutant isolates weakening replay from DAG work.\n"
            "    let mut isolated_weakening_work: usize = 0;\n"
            "    if !conclusion_matches\n"
            "        || !check_weakening_and_replay_with_work(\n"
            "            step, &mut isolated_weakening_work, work_limit,\n"
            "        )\n"
            "    {",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-resets-work-at-dag-weakening"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "DAG weakening meter-reset mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "DAG weakening shared work regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: DAG weakening work was not shared", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "DAG weakening meter-reset mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: DAG weakening work was reset before replay")
        compiled_runtime_mutations += 1

        dag_instantiation_handoff = (
            "    // Keep parent and conclusion structure comparisons in instantiation's meter.\n"
            "    if !conclusion_matches\n"
            "        || !check_instantiation_and_replay_with_work(step, work, work_limit)\n"
            "    {"
        )
        if source.count(dag_instantiation_handoff) != 1:
            print("DAG instantiation work-handoff mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_resets_work_at_dag_instantiation.rs"
        mutated_path.write_text(source.replace(
            dag_instantiation_handoff,
            "    // Mutant isolates instantiation replay from DAG work.\n"
            "    let mut isolated_instantiation_work: usize = 0;\n"
            "    if !conclusion_matches\n"
            "        || !check_instantiation_and_replay_with_work(\n"
            "            step, &mut isolated_instantiation_work, work_limit,\n"
            "        )\n"
            "    {",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-resets-work-at-dag-instantiation"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "DAG instantiation meter-reset mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "DAG instantiation shared work regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: DAG instantiation work was not shared", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "DAG instantiation meter-reset mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: DAG instantiation work was reset before replay")
        compiled_runtime_mutations += 1

        dag_condensation_handoff = (
            "    // Keep parent and conclusion structure comparisons in condensation's meter.\n"
            "    if !conclusion_matches\n"
            "        || !check_condensation_and_replay_with_work(step, work, work_limit)\n"
            "    {"
        )
        if source.count(dag_condensation_handoff) != 1:
            print("DAG condensation work-handoff mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_resets_work_at_dag_condensation.rs"
        mutated_path.write_text(source.replace(
            dag_condensation_handoff,
            "    // Mutant isolates condensation replay from DAG work.\n"
            "    let mut isolated_condensation_work: usize = 0;\n"
            "    if !conclusion_matches\n"
            "        || !check_condensation_and_replay_with_work(\n"
            "            step, &mut isolated_condensation_work, work_limit,\n"
            "        )\n"
            "    {",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-resets-work-at-dag-condensation"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "DAG condensation meter-reset mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "DAG condensation shared work regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: DAG condensation work was not shared", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "DAG condensation meter-reset mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: DAG condensation work was reset before replay")
        compiled_runtime_mutations += 1

        dag_demodulation_handoff = (
            "    // Keep parent and conclusion structure comparisons in the demodulation meter.\n"
            "    if !conclusion_matches\n"
            "        || !check_demodulation_and_replay_with_work(step, work, work_limit)\n"
            "    {"
        )
        if source.count(dag_demodulation_handoff) != 1:
            print("DAG demodulation work-handoff mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_resets_work_at_dag_demodulation.rs"
        mutated_path.write_text(source.replace(
            dag_demodulation_handoff,
            "    // Mutant isolates demodulation replay from DAG work.\n"
            "    let mut isolated_demodulation_work: usize = 0;\n"
            "    if !conclusion_matches\n"
            "        || !check_demodulation_and_replay_with_work(\n"
            "            step, &mut isolated_demodulation_work, work_limit,\n"
            "        )\n"
            "    {",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-resets-work-at-dag-demodulation"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "DAG demodulation meter-reset mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "DAG demodulation shared work regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: DAG demodulation work was not shared", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "DAG demodulation meter-reset mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: DAG demodulation work was reset before replay")
        compiled_runtime_mutations += 1

        dag_exact_resolution_handoff = (
            "    // Keep node and cited-parent charges in exact-resolution replay's meter.\n"
            "    if !check_exact_resolution_with_work(\n"
            "        &nodes[node.parents[0]].clause, &nodes[node.parents[1]].clause,\n"
            "        lp, rp, &node.clause, work, work_limit,\n"
            "    ) { return false; }"
        )
        if source.count(dag_exact_resolution_handoff) != 1:
            print("DAG exact-resolution work-handoff mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_resets_work_at_dag_exact_resolution.rs"
        mutated_path.write_text(source.replace(
            dag_exact_resolution_handoff,
            "    // Mutant isolates exact-resolution replay from DAG work.\n"
            "    let mut isolated_exact_resolution_work: usize = 0;\n"
            "    if !check_exact_resolution_with_work(\n"
            "        &nodes[node.parents[0]].clause, &nodes[node.parents[1]].clause,\n"
            "        lp, rp, &node.clause, &mut isolated_exact_resolution_work, work_limit,\n"
            "    ) { return false; }",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-resets-work-at-dag-exact-resolution"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "DAG exact-resolution meter-reset mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "DAG exact resolution shared work regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: DAG exact-resolution work was not shared", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "DAG exact-resolution meter-reset mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: DAG exact-resolution work was reset before replay")
        compiled_runtime_mutations += 1

        dag_nullary_resolution_handoff = (
            "    // Keep node and cited-parent charges in nullary-resolution replay's meter.\n"
            "    let checked = check_nullary_resolution_with_work(\n"
            "        &nodes[node.parents[0]].clause,\n"
            "        &nodes[node.parents[1]].clause,\n"
            "        lp,\n"
            "        rp,\n"
            "        &node.clause,\n"
            "        work,\n"
            "        work_limit,\n"
            "    );"
        )
        if source.count(dag_nullary_resolution_handoff) != 1:
            print("DAG nullary-resolution work-handoff mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_resets_work_at_dag_nullary_resolution.rs"
        mutated_path.write_text(source.replace(
            dag_nullary_resolution_handoff,
            "    // Mutant isolates nullary-resolution replay from DAG work.\n"
            "    let mut isolated_nullary_resolution_work: usize = 0;\n"
            "    let checked = check_nullary_resolution_with_work(\n"
            "        &nodes[node.parents[0]].clause,\n"
            "        &nodes[node.parents[1]].clause,\n"
            "        lp,\n"
            "        rp,\n"
            "        &node.clause,\n"
            "        &mut isolated_nullary_resolution_work,\n"
            "        work_limit,\n"
            "    );",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-resets-work-at-dag-nullary-resolution"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "DAG nullary-resolution meter-reset mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "DAG nullary resolution shared work regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: DAG nullary-resolution work was not shared", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "DAG nullary-resolution meter-reset mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: DAG nullary-resolution work was reset before replay")
        compiled_runtime_mutations += 1

        reflexivity_term_charge = (
            "    let Some(terms_match) = same_term_with_work(s, t, work, work_limit) else {\n"
            "        return false;\n"
            "    };"
        )
        if source.count(reflexivity_term_charge) != 1:
            print("reflexivity term-comparison mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_without_reflexivity_term_charge.rs"
        mutated_path.write_text(source.replace(
            reflexivity_term_charge,
            "    let terms_match = same_term(s, t);",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-without-reflexivity-term-charge"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "reflexivity term-meter mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "reflexivity work-meter regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: reflexivity comparison work was not charged", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "reflexivity term-meter mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: reflexivity comparison work was unmetered")
        compiled_runtime_mutations += 1

        dag_reflexivity_handoff = (
            "    // Continue per-node work through reflexivity's equality comparison.\n"
            "    if !check_reflexivity_and_replay_with_work(&node.clause, work, work_limit) {"
        )
        if source.count(dag_reflexivity_handoff) != 1:
            print("DAG reflexivity work-handoff mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_resets_work_at_dag_reflexivity.rs"
        mutated_path.write_text(source.replace(
            dag_reflexivity_handoff,
            "    // Mutant isolates reflexivity replay from DAG work.\n"
            "    let mut isolated_reflexivity_work: usize = 0;\n"
            "    if !check_reflexivity_and_replay_with_work(\n"
            "        &node.clause, &mut isolated_reflexivity_work, work_limit,\n"
            "    ) {",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-resets-work-at-dag-reflexivity"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "DAG reflexivity meter-reset mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "DAG reflexivity shared work regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: DAG reflexivity work was not shared", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "DAG reflexivity meter-reset mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: DAG reflexivity work was reset before replay")
        compiled_runtime_mutations += 1

        transitivity_term_charge = (
            "    let Some(direct_left) = same_term_with_work(left_a, right_a, work, work_limit) else {\n"
            "        return None;\n"
            "    };"
        )
        if source.count(transitivity_term_charge) != 1:
            print("transitivity term-comparison mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_without_transitivity_term_charge.rs"
        mutated_path.write_text(source.replace(
            transitivity_term_charge,
            "    let direct_left = same_term(left_a, right_a);",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-without-transitivity-term-charge"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "transitivity comparison mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "transitivity step work-meter regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: transitivity term comparison was not charged", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "transitivity comparison mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: transitivity term comparison was unmetered")
        compiled_runtime_mutations += 1

        dag_transitivity_handoff = (
            "    // Keep parent structure and all transitivity comparisons on the node meter.\n"
            "    if !check_transitivity_and_replay_with_work(\n"
            "        &nodes[node.parents[0]].clause,\n"
            "        &nodes[node.parents[1]].clause,\n"
            "        &node.clause,\n"
            "        work,\n"
            "        work_limit,\n"
            "    ) {\n"
            "        return false;\n"
            "    }"
        )
        if source.count(dag_transitivity_handoff) != 1:
            print("DAG transitivity work-handoff mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_resets_work_at_dag_transitivity.rs"
        mutated_path.write_text(source.replace(
            dag_transitivity_handoff,
            "    // Mutant isolates transitivity replay from DAG work.\n"
            "    let mut isolated_transitivity_work: usize = 0;\n"
            "    if !check_transitivity_and_replay_with_work(\n"
            "        &nodes[node.parents[0]].clause,\n"
            "        &nodes[node.parents[1]].clause,\n"
            "        &node.clause,\n"
            "        &mut isolated_transitivity_work,\n"
            "        work_limit,\n"
            "    ) {\n"
            "        return false;\n"
            "    }",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-resets-work-at-dag-transitivity"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "DAG transitivity meter-reset mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "DAG transitivity shared work regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: DAG transitivity work was not shared", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "DAG transitivity meter-reset mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: DAG transitivity work was reset before replay")
        compiled_runtime_mutations += 1

        symmetry_term_charge = (
            "    let Some(direct_left) = same_term_with_work(u, s, work, work_limit) else {\n"
            "        return None;\n"
            "    };"
        )
        if source.count(symmetry_term_charge) != 1:
            print("symmetry term-comparison mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_without_symmetry_term_charge.rs"
        mutated_path.write_text(source.replace(
            symmetry_term_charge,
            "    let direct_left = same_term(u, s);",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-without-symmetry-term-charge"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "symmetry comparison mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "symmetry step work-meter regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: symmetry term comparison was not charged", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "symmetry comparison mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: symmetry term comparison was unmetered")
        compiled_runtime_mutations += 1

        dag_symmetry_handoff = (
            "    // Keep parent structure and all symmetry comparisons on the node meter.\n"
            "    if !check_symmetry_and_replay_with_work(\n"
            "        &nodes[node.parents[0]].clause,\n"
            "        &node.clause,\n"
            "        work,\n"
            "        work_limit,\n"
            "    ) {\n"
            "        return false;\n"
            "    }"
        )
        if source.count(dag_symmetry_handoff) != 1:
            print("DAG symmetry work-handoff mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_resets_work_at_dag_symmetry.rs"
        mutated_path.write_text(source.replace(
            dag_symmetry_handoff,
            "    // Mutant isolates symmetry replay from DAG work.\n"
            "    let mut isolated_symmetry_work: usize = 0;\n"
            "    if !check_symmetry_and_replay_with_work(\n"
            "        &nodes[node.parents[0]].clause,\n"
            "        &node.clause,\n"
            "        &mut isolated_symmetry_work,\n"
            "        work_limit,\n"
            "    ) {\n"
            "        return false;\n"
            "    }",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-resets-work-at-dag-symmetry"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "DAG symmetry meter-reset mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "DAG symmetry shared work regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: DAG symmetry work was not shared", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "DAG symmetry meter-reset mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: DAG symmetry work was reset before replay")
        compiled_runtime_mutations += 1

        excluded_middle_atom_charge = (
            "    let Some(atoms_match) = same_atom_with_work(&l1.atom, &l2.atom, work, work_limit) else {\n"
            "        return None;\n"
            "    };"
        )
        if source.count(excluded_middle_atom_charge) != 1:
            print("excluded-middle atom-comparison mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_without_excluded_middle_atom_charge.rs"
        mutated_path.write_text(source.replace(
            excluded_middle_atom_charge,
            "    let atoms_match = same_atom(&l1.atom, &l2.atom);",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-without-excluded-middle-atom-charge"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "excluded-middle comparison mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "excluded middle work-meter regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: excluded-middle atom comparison was not charged", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "excluded-middle comparison mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: excluded-middle atom comparison was unmetered")
        compiled_runtime_mutations += 1

        dag_excluded_middle_handoff = (
            "    // Continue per-node charges through the polarity and atom checks.\n"
            "    if !check_excluded_middle_and_replay_with_work(\n"
            "        &node.clause,\n"
            "        work,\n"
            "        work_limit,\n"
            "    ) {\n"
            "        return false;\n"
            "    }"
        )
        if source.count(dag_excluded_middle_handoff) != 1:
            print("DAG excluded-middle work-handoff mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_resets_work_at_dag_excluded_middle.rs"
        mutated_path.write_text(source.replace(
            dag_excluded_middle_handoff,
            "    // Mutant isolates excluded-middle replay from DAG work.\n"
            "    let mut isolated_excluded_middle_work: usize = 0;\n"
            "    if !check_excluded_middle_and_replay_with_work(\n"
            "        &node.clause,\n"
            "        &mut isolated_excluded_middle_work,\n"
            "        work_limit,\n"
            "    ) {\n"
            "        return false;\n"
            "    }",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-resets-work-at-dag-excluded-middle"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "DAG excluded-middle meter-reset mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "DAG excluded middle shared work regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: DAG excluded-middle work was not shared", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "DAG excluded-middle meter-reset mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: DAG excluded-middle work was reset before replay")
        compiled_runtime_mutations += 1

        congruence_term_charge = (
            "    let Some(direct_left) = same_term_with_work(a, s, work, work_limit) else { return None; };"
        )
        if source.count(congruence_term_charge) != 1:
            print("congruence term-comparison mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_without_congruence_term_charge.rs"
        mutated_path.write_text(source.replace(
            congruence_term_charge,
            "    let direct_left = same_term(a, s);",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-without-congruence-term-charge"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "congruence comparison mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "congruence step work-meter regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: congruence term comparison was not charged", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "congruence comparison mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: congruence term comparison was unmetered")
        compiled_runtime_mutations += 1

        dag_congruence_handoff = (
            "    // Keep parent structure and recursive term comparisons on the node meter.\n"
            "    if !check_congruence_and_replay_with_work(\n"
            "        &nodes[node.parents[0]].clause,\n"
            "        &node.clause,\n"
            "        work,\n"
            "        work_limit,\n"
            "    ) {\n"
            "        return false;\n"
            "    }"
        )
        if source.count(dag_congruence_handoff) != 1:
            print("DAG congruence work-handoff mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_resets_work_at_dag_congruence.rs"
        mutated_path.write_text(source.replace(
            dag_congruence_handoff,
            "    // Mutant isolates congruence replay from DAG work.\n"
            "    let mut isolated_congruence_work: usize = 0;\n"
            "    if !check_congruence_and_replay_with_work(\n"
            "        &nodes[node.parents[0]].clause,\n"
            "        &node.clause,\n"
            "        &mut isolated_congruence_work,\n"
            "        work_limit,\n"
            "    ) {\n"
            "        return false;\n"
            "    }",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-resets-work-at-dag-congruence"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "DAG congruence meter-reset mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "DAG congruence shared work regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: DAG congruence work was not shared", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "DAG congruence meter-reset mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: DAG congruence work was reset before replay")
        compiled_runtime_mutations += 1

        tautology_atom_charge = (
            "                let Some(atoms_match) = same_atom_with_work(\n"
            "                    &clause.literals[i].atom, &clause.literals[j].atom, work, work_limit,\n"
            "                ) else { return None; };"
        )
        if source.count(tautology_atom_charge) != 1:
            print("tautology atom-comparison mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_without_tautology_atom_charge.rs"
        mutated_path.write_text(source.replace(
            tautology_atom_charge,
            "                let atoms_match = same_atom(\n"
            "                    &clause.literals[i].atom, &clause.literals[j].atom,\n"
            "                );",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-without-tautology-atom-charge"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "tautology comparison mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "tautology work-meter regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: tautology atom comparison was not charged", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "tautology comparison mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: tautology atom comparison was unmetered")
        compiled_runtime_mutations += 1

        dag_tautology_handoff = (
            "    // Keep the node counter through the bounded clause scan.\n"
            "    if !check_tautology_and_replay_with_work(\n"
            "        &node.clause,\n"
            "        work,\n"
            "        work_limit,\n"
            "    ) {\n"
            "        return false;\n"
            "    }"
        )
        if source.count(dag_tautology_handoff) != 1:
            print("DAG tautology work-handoff mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_resets_work_at_dag_tautology.rs"
        mutated_path.write_text(source.replace(
            dag_tautology_handoff,
            "    // Mutant isolates tautology replay from DAG work.\n"
            "    let mut isolated_tautology_work: usize = 0;\n"
            "    if !check_tautology_and_replay_with_work(\n"
            "        &node.clause,\n"
            "        &mut isolated_tautology_work,\n"
            "        work_limit,\n"
            "    ) {\n"
            "        return false;\n"
            "    }",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-resets-work-at-dag-tautology"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "DAG tautology meter-reset mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "DAG tautology shared work regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: DAG tautology work was not shared", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "DAG tautology meter-reset mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: DAG tautology work was reset before replay")
        compiled_runtime_mutations += 1

        dag_structural_work_handoff = (
            "    match check_dag_structure_with_work(dag, node_limit, work_limit, work, work_limit) {"
        )
        if source.count(dag_structural_work_handoff) != 1:
            print("DAG structural-work handoff mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_resets_work_after_dag_structure.rs"
        mutated_path.write_text(source.replace(
            dag_structural_work_handoff,
            "    let mut isolated_structural_work: usize = 0;\n"
            "    match check_dag_structure_with_work(\n"
            "        dag, node_limit, work_limit, &mut isolated_structural_work, work_limit,\n"
            "    ) {",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-resets-work-after-dag-structure"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "DAG structural-work reset mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "DAG cumulative work composition regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: DAG structural work was not shared", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "DAG structural-work reset mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: DAG structural work was reset before closure")
        compiled_runtime_mutations += 1

        dag_closure_work_handoff = (
            "    match check_root_ancestor_closure_with_work(dag, work, work_limit) {"
        )
        if source.count(dag_closure_work_handoff) != 1:
            print("DAG closure-work handoff mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_resets_work_after_dag_closure.rs"
        mutated_path.write_text(source.replace(
            dag_closure_work_handoff,
            "    let mut isolated_closure_work: usize = 0;\n"
            "    match check_root_ancestor_closure_with_work(\n"
            "        dag, &mut isolated_closure_work, work_limit,\n"
            "    ) {",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-resets-work-after-dag-closure"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "DAG closure-work reset mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        if runtime.returncode == 0:
            print("runtime accepted mutation: DAG closure work was not shared", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "DAG closure-work reset mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: DAG closure work was reset before node replay")
        compiled_runtime_mutations += 1

        dag_node_work_handoff = (
            "        if !validate_dag_node_with_work(dag, i, work, work_limit) {"
        )
        if source.count(dag_node_work_handoff) != 1:
            print("DAG node-work handoff mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_resets_work_for_dag_nodes.rs"
        mutated_path.write_text(source.replace(
            dag_node_work_handoff,
            "        let mut isolated_node_work: usize = 0;\n"
            "        if !validate_dag_node_with_work(dag, i, &mut isolated_node_work, work_limit) {",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-resets-work-for-dag-nodes"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "DAG node-work reset mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        if runtime.returncode == 0:
            print("runtime accepted mutation: DAG node replay work was not shared", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "DAG node-work reset mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: DAG node work was reset between nodes")
        compiled_runtime_mutations += 1

        unifier_term_charge = (
            "    // Charge one unit for each compared term-node pair.\n"
            "    if !consume_work(work, work_limit) { return None; }"
        )
        if source.count(unifier_term_charge) != 1:
            print("unifier term-comparison mutation anchor is missing or ambiguous", file=sys.stderr)
            return 2
        mutated_path = pathlib.Path(temp_dir) / "first_order_core_without_unifier_term_charge.rs"
        mutated_path.write_text(source.replace(
            unifier_term_charge,
            "    // Mutant omits term-node comparison work.\n"
            "    if false { return None; }",
            1,
        ))
        binary = pathlib.Path(temp_dir) / "first-order-core-without-unifier-term-charge"
        compile_result = subprocess.run(
            [str(verus), "--no-cheating", "--triggers-mode", "silent", "--compile",
             str(mutated_path), "-o", str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=300,
        )
        if compile_result.returncode != 0:
            print(
                "unifier comparison mutant must pass Verus before its runtime test:\n"
                + compile_result.stdout,
                file=sys.stderr,
            )
            return 1
        runtime = subprocess.run(
            [str(binary)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=30,
        )
        expected_failure = "unifier cumulative work regression failed"
        if runtime.returncode == 0:
            print("runtime accepted mutation: unifier omits comparison work", file=sys.stderr)
            return 1
        if expected_failure not in runtime.stdout:
            print(
                "unifier comparison mutation failed for an unexpected reason:\n"
                + runtime.stdout,
                file=sys.stderr,
            )
            return 1
        print("runtime mutation rejected: unifier omits comparison work")
        compiled_runtime_mutations += 1

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
        compiled_runtime_mutations += 1

    if compiled_runtime_mutations != EXPECTED_COMPILED_RUNTIME_MUTATIONS:
        print(
            "compiled-runtime mutation inventory changed: "
            f"expected {EXPECTED_COMPILED_RUNTIME_MUTATIONS}, found {compiled_runtime_mutations}",
            file=sys.stderr,
        )
        return 2
    print(
        "mutation gate passed: "
        f"verifier_negative={len(mutations)} "
        f"compiled_runtime={compiled_runtime_mutations}"
    )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
