//! TSTP format output for proofs.
//!
//! Formats proof steps in the TSTP (Thousands of Solutions from Theorem Provers)
//! format, which is the standard output format for automated theorem provers.
//!
//! Input clauses:    `cnf(c0, axiom, p(a), file('/path/to/problem.p', ax1)).`
//! Inferred clauses: `cnf(c5, plain, p(a), inference(resolution, [status(thm)], [c0, c1])).`
//! Empty clause:     `cnf(cN, plain, $false, inference(resolution, [status(thm)], [c3, c7])).`
//! Formula-level (non-clausal) steps use `fof(...)` instead of `cnf(...)`,
//! e.g. `fof(c1, plain, (nnf_formula), inference(fof_nnf_transformation, [status(thm)], [c0])).`
//! — see [`Clause::formula`](mrs_core::clause::Clause::formula).

use std::sync::OnceLock;

use mrs_core::SymbolTable;
use mrs_core::clause::{Clause, ClauseSource};
use mrs_core::display::DisplayWithSymbols;

/// The path of the problem file currently being proved, as passed on the
/// command line (or piped via stdin, in which case it stays unset).
///
/// Set once at startup by the binary entry point via [`set_problem_path`],
/// and read by [`format_tstp`] when emitting `file(...)` leaf annotations.
/// GDV-style proof checkers resolve this path to re-open the original
/// problem file during leaf verification, so it must be the real,
/// resolvable path the prover was invoked with (e.g. the StarExec sandbox
/// path at competition time) rather than a placeholder string.
static PROBLEM_PATH: OnceLock<String> = OnceLock::new();

/// Records the path of the problem file being proved, for use in `file(...)`
/// leaf annotations emitted by [`format_tstp`].
///
/// Only the first call has any effect (matches `mrs`'s one-problem-per-process
/// model); subsequent calls are silently ignored.
pub fn set_problem_path(path: impl Into<String>) {
    let _ = PROBLEM_PATH.set(path.into());
}

/// Returns the path most recently recorded via [`set_problem_path`], or the
/// literal `"input"` placeholder if none was set (e.g. in unit tests, or
/// when the problem was read from stdin).
fn problem_path() -> &'static str {
    PROBLEM_PATH.get().map(String::as_str).unwrap_or("input")
}

/// The TSTP `status` annotation (`status(thm)`, `status(esa)`, `status(cth)`)
/// for an `inference(...)` step, derived from the rule name.
///
/// - `skolemisation`: `esa` (equisatisfiability, not full entailment — a
///   Skolemized formula is not logically equivalent to its parent, only
///   equisatisfiable with it).
/// - `negated_conjecture`: `cth` (the CASC evaluation criteria require the
///   step that negates the conjecture to be annotated `status(cth)`, with a
///   single parent of role `conjecture`).
/// - everything else (`fof_nnf_transformation`, `cnf_transformation`,
///   `resolution`, `superposition`, etc.): `thm` (a genuine entailment).
///
/// Definitional (Tseitin) CNF clauses do NOT need a special status here:
/// each fresh `def_...` predicate gets its own `ClauseSource::Introduced`
/// step (rendered `introduced(definition)`, no status at all), cited as an
/// extra parent by every clause that mentions it — so those clauses remain
/// ordinary `thm` consequences of {skolemization step, definition step(s)}.
fn status_for_rule(rule: &str) -> &'static str {
    match rule {
        "skolemisation" => "esa",
        "negated_conjecture" => "cth",
        _ => "thm",
    }
}

/// Formats a sequence of proof steps as TSTP output.
///
/// The proof should be topologically ordered (inputs first, empty clause last).
///
/// Clauses with `formula: Some(_)` (see
/// [`Clause::formula`](mrs_core::clause::Clause::formula)) are non-clausal,
/// FOF-level proof steps (e.g. NNF conversion or Skolemization results) and
/// are printed as `fof(...)` annotated formulas instead of `cnf(...)`.
pub fn format_tstp(proof: &[Clause], symbols: &SymbolTable) -> String {
    let mut proof_sorted = proof.to_vec();
    proof_sorted.sort_unstable_by_key(|c| c.id.0);

    let mut lines = Vec::new();
    let problem_path = problem_path();

    // Prepend the standard % Proof : <path> header at the very top
    lines.push(format!("% Proof : {}", problem_path));

    for clause in &proof_sorted {
        let id = clause.id.0;
        let is_formula_step = clause.formula.is_some();

        let body = if let Some(formula) = &clause.formula {
            format!("{}", formula.display(symbols))
        } else {
            let mut final_lits = clause.literals.clone();
            for &v in &clause.avatar {
                let sym_name = format!("spl0_{}", v);
                let sym_id = symbols
                    .resolve_name(&sym_name)
                    .expect("spl0 symbol must exist");
                let atom = mrs_core::formula::Atom::pred(sym_id, Vec::new());
                let lit = mrs_core::clause::Literal::neg(atom);
                final_lits.push(lit);
            }
            if final_lits.is_empty() {
                "$false".to_string()
            } else {
                let mut temp_lines = Vec::new();
                for lit in &final_lits {
                    temp_lines.push(format!("{}", lit.display(symbols)));
                }
                temp_lines.join(" | ")
            }
        };

        let annotation = match &clause.source {
            ClauseSource::Input { name, role: _ } => {
                format!("file('{}', '{}')", problem_path, name)
            }
            ClauseSource::Inference { rule, parents } => {
                let parent_names: Vec<String> =
                    parents.iter().map(|p| format!("c{}", p.0)).collect();
                let status = status_for_rule(rule);
                format!(
                    "inference({}, [status({})], [{}])",
                    rule,
                    status,
                    parent_names.join(", ")
                )
            }
            // Sound by construction (conservative extension over a fresh
            // symbol) — no parents/status needed, but GDV's
            // `IsCorrectlySpecifiedDefinition` check requires an explicit
            // `new_symbols(definition, [<symbol>])` info entry naming
            // exactly which symbol is being defined (confirmed against a
            // real GDV build).
            ClauseSource::Introduced { symbol } => format!(
                "introduced(definition, [new_symbols(definition, [{}])])",
                symbols.resolve(*symbol)
            ),
        };

        let role = match &clause.source {
            ClauseSource::Input { role, .. } => role.as_str(),
            // GDV structurally requires a step that negates the conjecture to
            // have role `negated_conjecture` itself, not the generic `plain`
            // fallback — it looks for a role=`negated_conjecture` formula to
            // legitimize a derived formula having a role=`conjecture` parent,
            // and otherwise reports "illegal relationship with its
            // (non-)conjecture parent" (confirmed against a real GDV build).
            ClauseSource::Inference { rule, .. } if *rule == "negated_conjecture" => {
                "negated_conjecture"
            }
            ClauseSource::Inference { .. } => "plain",
            // GDV's `IsCorrectlySpecifiedDefinition` also requires the
            // role itself to be `definition`, not `plain` (confirmed
            // against a real GDV build).
            ClauseSource::Introduced { .. } => "definition",
        };

        let wrapper = if is_formula_step { "fof" } else { "cnf" };
        lines.push(format!(
            "{}(c{}, {}, {}, {}).",
            wrapper, id, role, body, annotation
        ));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::clause::{Clause, ClauseId, ClauseSource};
    use mrs_core::{Atom, Literal, Term};

    #[test]
    fn format_input_clause_path_annotation() {
        // PROBLEM_PATH is a process-global OnceLock (matches mrs's
        // one-problem-per-process model), so both the "unset" and "set"
        // behaviours are asserted within a single test, in order, rather
        // than as separate tests that could race against each other via
        // the shared global if run concurrently in the same test binary.
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");

        let make_clause = || {
            Clause::new(
                ClauseId(0),
                vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
                ClauseSource::Input {
                    name: "ax1".into(),
                    role: "axiom".into(),
                },
            )
        };

        // Before set_problem_path is ever called, the placeholder is "input".
        let output = format_tstp(&[make_clause()], &syms);
        assert!(output.contains("cnf(c0, axiom, p(a), file('input', 'ax1'))."));

        // After set_problem_path, the real path is used instead.
        set_problem_path("/starexec/sandbox/problems/SEU140+2.p");
        let output = format_tstp(&[make_clause()], &syms);
        assert!(output.contains(
            "cnf(c0, axiom, p(a), file('/starexec/sandbox/problems/SEU140+2.p', 'ax1'))."
        ));
    }

    #[test]
    fn format_inferred_clause() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");

        let c = Clause::new(
            ClauseId(5),
            vec![Literal::pos(Atom::prop(p))],
            ClauseSource::Inference {
                rule: "resolution",
                parents: vec![ClauseId(0), ClauseId(1)].into(),
            },
        );

        let output = format_tstp(&[c], &syms);
        assert!(output.contains("inference(resolution, [status(thm)], [c0, c1])"));
        assert!(output.contains("cnf(c5, plain,"));
    }

    #[test]
    fn format_empty_clause() {
        let syms = SymbolTable::new();

        let c = Clause::new(
            ClauseId(10),
            vec![],
            ClauseSource::Inference {
                rule: "resolution",
                parents: vec![ClauseId(3), ClauseId(7)].into(),
            },
        );

        let output = format_tstp(&[c], &syms);
        assert!(output.contains("$false"));
        assert!(output.contains("inference(resolution, [status(thm)], [c3, c7])"));
    }

    #[test]
    fn format_formula_step_uses_fof_wrapper() {
        use mrs_core::Formula;

        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");

        let leaf = Clause::new_formula_step(
            ClauseId(0),
            Formula::atom(Atom::pred(p, vec![Term::constant(a)])),
            ClauseSource::Input {
                name: "ax1".into(),
                role: "axiom".into(),
            },
        );
        let nnf_step = Clause::new_formula_step(
            ClauseId(1),
            Formula::atom(Atom::pred(p, vec![Term::constant(a)])),
            ClauseSource::Inference {
                rule: "fof_nnf_transformation",
                parents: vec![ClauseId(0)].into(),
            },
        );

        let output = format_tstp(&[leaf, nnf_step], &syms);
        assert!(output.contains("fof(c0, axiom, p(a), file('input', 'ax1'))."));
        assert!(output.contains(
            "fof(c1, plain, p(a), inference(fof_nnf_transformation, [status(thm)], [c0]))."
        ));
        // No cnf(...) wrapper should appear anywhere for these formula steps.
        assert!(!output.contains("cnf("));
    }

    #[test]
    fn status_is_esa_for_skolemisation_and_cth_for_negated_conjecture() {
        use mrs_core::Formula;

        let mut syms = SymbolTable::new();
        let p = syms.intern("p");

        let skolem_step = Clause::new_formula_step(
            ClauseId(2),
            Formula::atom(Atom::prop(p)),
            ClauseSource::Inference {
                rule: "skolemisation",
                parents: vec![ClauseId(1)].into(),
            },
        );
        let neg_conj_step = Clause::new_formula_step(
            ClauseId(3),
            Formula::atom(Atom::prop(p)),
            ClauseSource::Inference {
                rule: "negated_conjecture",
                parents: vec![ClauseId(2)].into(),
            },
        );

        let output = format_tstp(&[skolem_step, neg_conj_step], &syms);
        assert!(output.contains("inference(skolemisation, [status(esa)], [c1])"));
        assert!(output.contains("inference(negated_conjecture, [status(cth)], [c2])"));
        // The skolemisation step is a generic derived formula: role `plain`.
        assert!(output.contains("fof(c2, plain,"));
        // The negated_conjecture step must have role `negated_conjecture`
        // itself, not `plain` -- see the doc comment on the role match arm.
        assert!(output.contains("fof(c3, negated_conjecture,"));
    }

    #[test]
    fn negated_conjecture_step_has_role_negated_conjecture_not_plain() {
        // Regression test for the exact failure GDV reported against the
        // real SEU140+2 sample sent to the CASC-J13 organizer:
        //   FAILURE: 'c268' has an illegal relationship with its
        //   (non-)conjecture parent
        // Confirmed via a locally-built GDV (github.com/TPTPWorld/GDV) that
        // this specific role mislabeling (`plain` instead of
        // `negated_conjecture`) is exactly what GDV flags; fixed here.
        use mrs_core::Formula;

        let mut syms = SymbolTable::new();
        let p = syms.intern("p");

        let conjecture_leaf = Clause::new_formula_step(
            ClauseId(0),
            Formula::atom(Atom::prop(p)),
            ClauseSource::Input {
                name: "conj1".into(),
                role: "conjecture".into(),
            },
        );
        let negated_conjecture_step = Clause::new_formula_step(
            ClauseId(1),
            Formula::neg(Formula::atom(Atom::prop(p))),
            ClauseSource::Inference {
                rule: "negated_conjecture",
                parents: vec![ClauseId(0)].into(),
            },
        );

        let output = format_tstp(&[conjecture_leaf, negated_conjecture_step], &syms);
        assert!(
            output.contains("fof(c1, negated_conjecture,"),
            "negated_conjecture step must have its own role set to \
             `negated_conjecture`, not `plain` -- GDV requires this to \
             accept its relationship to the conjecture-role parent, got: {output}"
        );
        assert!(!output.contains("fof(c1, plain,"));
    }

    #[test]
    fn cnf_transformation_cites_skolemisation_step() {
        // Regression test for the bug flagged during CASC-J13 review: final
        // CNF clauses must cite the Skolemization step as parent (so the
        // FOF-to-CNF translation is documented), not the original axiom
        // directly.
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");

        let final_clause = Clause::new(
            ClauseId(4),
            vec![Literal::pos(Atom::prop(p))],
            ClauseSource::Inference {
                rule: "cnf_transformation",
                parents: vec![ClauseId(2)].into(),
            },
        );

        let output = format_tstp(&[final_clause], &syms);
        assert!(
            output
                .contains("cnf(c4, plain, p, inference(cnf_transformation, [status(thm)], [c2])).")
        );
    }

    #[test]
    fn introduced_definition_step_has_no_status_and_no_parents() {
        // Regression test for a real GDV failure found reviewing SEU140+2:
        // clauses using a fresh `def_...` predicate (from definitional/
        // Tseitin CNF) cannot be justified as a `thm`/`esa` of a parent
        // that never mentions that symbol at all -- GDV reports a genuine
        // CounterSatisfiable countermodel, not just a timeout, if you try.
        // The fix is `ClauseSource::Introduced`: assert the fresh
        // predicate's full biconditional with no parents at all (sound by
        // construction, since the symbol is fresh). Role `definition` and
        // an explicit `new_symbols(definition, [<symbol>])` info entry are
        // both required by GDV's `IsCorrectlySpecifiedDefinition` check
        // (confirmed against a real GDV build -- E's own bare
        // `introduced(definition)`/role `plain` convention, which
        // `mrs-proover`'s own checker accepts, is NOT accepted by GDV).
        use mrs_core::Formula;

        let mut syms = SymbolTable::new();
        let def = syms.intern("def_ax1_0");
        let q = syms.intern("q");

        let def_step = Clause::new_formula_step(
            ClauseId(5),
            Formula::iff(Formula::atom(Atom::prop(def)), Formula::atom(Atom::prop(q))),
            ClauseSource::Introduced { symbol: def },
        );

        let output = format_tstp(&[def_step], &syms);
        assert!(output.contains(
            "fof(c5, definition, (def_ax1_0 <=> q), introduced(definition, [new_symbols(definition, [def_ax1_0])]))."
        ));
    }
}
