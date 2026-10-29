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

use std::fmt::Write;
use std::sync::OnceLock;

use mrs_core::SymbolTable;
use mrs_core::clause::{Clause, ClauseCertificate, ClauseSource};
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
        "skolemisation"
        | "avatar_split_clause"
        | "avatar_component_clause"
        | "avatar_branch_refutation"
        | "split_component" => "esa",
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
    let mut output = String::new();
    if write_tstp(proof, symbols, &mut output).is_err() {
        return String::new();
    }
    output
}

/// Serialize to a caller-owned sink, without retaining a second proof-sized
/// collection of formatted lines. A failed sink can contain a partial proof;
/// callers must discard it on error and never certify/output it as complete.
pub fn write_tstp(
    proof: &[Clause],
    symbols: &SymbolTable,
    output: &mut impl Write,
) -> std::fmt::Result {
    // Validate original witness/source closure before elaboration can replace
    // compound nodes with fresh witnesses. Invalid metadata must not disappear
    // merely because a reconstruction fallback found a valid derivation.
    let validated = match crate::elaborate::topological_sort(proof) {
        Ok(v) => v,
        Err(e) => {
            if std::env::var("TRACE_SEARCH").is_ok() {
                eprintln!("[TRACE] write_tstp: topological_sort failed: {:?}", e);
            }
            return Err(std::fmt::Error);
        }
    };
    // Elaboration is an optional refinement. If a composite step cannot be
    // replayed, retain a dependency-topological compact proof so the strict
    // kernel can classify it as inconclusive. Never fall back to numeric-ID
    // sorting, and never emit a graph with missing or cyclic dependencies.
    let proof_sorted = match crate::elaborate::elaborate(&validated, symbols) {
        Ok(elaborated) => elaborated.clauses,
        Err(_) => validated,
    };

    let problem_path = problem_path();

    // Prepend the standard % Proof : <path> header at the very top
    write!(output, "% Proof : {problem_path}")?;

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
                let mut info = format!("status({status})");
                if let Some(certificate) = &clause.certificate {
                    info.push_str(", ");
                    info.push_str(&format_certificate(certificate));
                }
                if let Some(steps) = format_demodulation_steps(clause) {
                    info.push_str(", ");
                    info.push_str(&steps);
                }
                format!(
                    "inference({}, [{}], [{}])",
                    rule,
                    info,
                    parent_names.join(", ")
                )
            }
            // Sound by construction (conservative extension over a fresh
            // symbol) — no parents/status needed, but GDV's
            // `IsCorrectlySpecifiedDefinition` check requires an explicit
            // `new_symbols(definition, [<symbol>])` info entry naming
            // exactly which symbol is being defined (confirmed against a
            // real GDV build).
            ClauseSource::Introduced { symbol, .. } => format!(
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
        write!(output, "\n{wrapper}(c{id}, {role}, {body}, {annotation}).")?;
    }

    Ok(())
}

/// Bound serialized bytes while writing. Exhaustion returns no partial proof.
/// This bounds output storage, not elaboration or the largest per-node string.
pub fn format_tstp_bounded(
    proof: &[Clause],
    symbols: &SymbolTable,
    max_bytes: usize,
) -> Result<String, std::fmt::Error> {
    struct BoundedOutput {
        text: String,
        limit: usize,
    }
    impl Write for BoundedOutput {
        fn write_str(&mut self, text: &str) -> std::fmt::Result {
            if text.len() > self.limit.saturating_sub(self.text.len()) {
                return Err(std::fmt::Error);
            }
            self.text.push_str(text);
            Ok(())
        }
    }
    let mut output = BoundedOutput {
        text: String::new(),
        limit: max_bytes,
    };
    write_tstp(proof, symbols, &mut output)?;
    Ok(output.text)
}

fn format_certificate(certificate: &ClauseCertificate) -> String {
    match certificate {
        ClauseCertificate::AvatarSplit {
            inherited,
            components,
        } => {
            let components = components
                .iter()
                .map(|component| {
                    format!(
                        "branch({}, spl0_{}, [{}])",
                        component.branch_index,
                        component.sat_var,
                        component
                            .literal_indices
                            .iter()
                            .map(|index| index.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let inherited = inherited
                .iter()
                .map(|var| format!("spl0_{var}"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("avatar_split([{components}], [{inherited}])")
        }
        ClauseCertificate::AvatarComponent {
            split_parent,
            branch_index,
            sat_var,
        } => format!(
            "avatar_component(c{}, {}, spl0_{})",
            split_parent.0, branch_index, sat_var
        ),
        ClauseCertificate::AvatarBranchRefutation { context } => {
            let context = context
                .iter()
                .map(|var| format!("spl0_{var}"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("avatar_context([{context}])")
        }
        ClauseCertificate::AvatarSatRefutation {
            split_nodes,
            branch_roots,
            sat_trace,
        } => {
            let split_nodes = clause_id_list(split_nodes);
            let branch_roots = clause_id_list(branch_roots);
            let trace = format_sat_trace(sat_trace);
            format!("avatar_sat_refutation([{split_nodes}], [{branch_roots}]{trace})")
        }
        ClauseCertificate::SatBackedRefutation { inputs, sat_trace } => {
            let inputs = clause_id_list(inputs);
            let trace = format_sat_trace(sat_trace);
            format!("sat_backed_refutation([{inputs}]{trace})")
        }
    }
}

fn clause_id_list(ids: &[mrs_core::clause::ClauseId]) -> String {
    ids.iter()
        .map(|id| format!("c{}", id.0))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Emit the recorded rewrite sequence of a `demodulation` step as
/// `demodulation_steps(rule(1, 0, [0]), …)`.
///
/// A demodulation step collapses a whole fixpoint of unit-equality rewriting
/// into one node, so the parent/conclusion pair alone does not say *where* or
/// *with which* equality each rewrite happened. Recording it lets a verifier
/// replay the step deterministically instead of searching for a rewrite
/// sequence — which is what the strict kernel has to fall back to for proofs
/// from other provers, and what made MRS's own equational proofs unverifiable.
///
/// Each entry's `rule_parent` is the position of the cited unit equality in
/// this step's parent list (0 is the rewritten clause). A rewrite that cannot
/// be mapped to a cited parent suppresses the whole annotation rather than
/// emitting a reference a verifier would have to reject.
fn format_demodulation_steps(clause: &Clause) -> Option<String> {
    let ClauseSource::Inference { rule, parents } = &clause.source else {
        return None;
    };
    if *rule != "demodulation" {
        return None;
    }
    let mrs_core::witness::ProofWitness::Demodulation {
        rule_parents,
        steps,
        ..
    } = clause.witness.as_ref()?
    else {
        return None;
    };
    if steps.is_empty() {
        return None;
    }
    // Parent index 0 is the clause being rewritten; the unit equalities
    // follow in citation order.
    let position = |node: mrs_core::witness::ProofNodeId| -> Option<usize> {
        // Witness lists may contain repeated applications; exported parent
        // lists are deduplicated. Resolve the actual cited identity rather
        // than interpreting a witness-list offset as a citation offset.
        if !rule_parents.contains(&node) {
            return None;
        }
        parents
            .iter()
            .position(|parent| parent.0 == node.0)
            .filter(|index| *index > 0)
    };
    let mut entries = Vec::with_capacity(steps.len());
    for step in steps {
        let index = position(step.rule_parent)?;
        debug_assert!(
            parents.get(index).is_some(),
            "recorded parent index must be inside the step's parents"
        );
        let path = step
            .term_path
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        entries.push(format!("rule({index}, {}, [{path}])", step.lit_idx));
    }
    Some(format!("demodulation_steps({})", entries.join(", ")))
}

fn format_sat_trace(sat_trace: &Option<mrs_core::clause::AvatarSatTrace>) -> String {
    sat_trace
        .as_ref()
        .map(|trace| {
            let ids = trace
                .original_ids
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            let clauses = trace
                .clauses
                .iter()
                .map(|clause| {
                    format!(
                        "[{}]",
                        clause
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let cited_indices = trace
                .cited_indices
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                ", sat_trace('{}', {}, '{}', [{}], [{}], [{}], '{}')",
                trace.format,
                trace.variables,
                hex_encode(&trace.digest),
                ids,
                cited_indices,
                clauses,
                hex_encode(&trace.trace)
            )
        })
        .unwrap_or_default()
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    #[test]
    fn bounded_output_preserves_exact_bytes_and_refuses_truncation() {
        let symbols = mrs_core::SymbolTable::new();
        let proof = [Clause::new(
            mrs_core::clause::ClauseId(41),
            vec![],
            ClauseSource::Input {
                name: "empty".into(),
                role: "axiom".into(),
            },
        )];
        let expected = super::format_tstp(&proof, &symbols);
        assert!(!expected.is_empty());
        assert_eq!(
            super::format_tstp_bounded(&proof, &symbols, expected.len()).unwrap(),
            expected
        );
        assert!(super::format_tstp_bounded(&proof, &symbols, expected.len() - 1).is_err());
        assert!(super::format_tstp_bounded(&proof, &symbols, 0).is_err());
        let mut streamed = String::new();
        super::write_tstp(&proof, &symbols, &mut streamed).unwrap();
        assert_eq!(streamed, expected);
    }
    use super::*;
    use mrs_core::clause::{Clause, ClauseId, ClauseSource};
    use mrs_core::witness::{ProofNodeId, ProofWitness};
    use mrs_core::{Atom, Literal, Term};

    /// `PROBLEM_PATH` is a process-global `OnceLock`, so any test that reads or
    /// writes it must hold this lock. Without it, a test asserting on the
    /// placeholder path can observe `"input"` from `format_tstp` and then read
    /// the path the other test has just installed, which is an intermittent
    /// failure rather than a real one.
    static PROBLEM_PATH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn format_input_clause_path_annotation() {
        let _guard = PROBLEM_PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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

        let parent0 = Clause::new(
            ClauseId(0),
            vec![],
            ClauseSource::Input {
                name: "ax1".into(),
                role: "axiom".into(),
            },
        );
        let parent1 = Clause::new(
            ClauseId(1),
            vec![],
            ClauseSource::Input {
                name: "ax2".into(),
                role: "axiom".into(),
            },
        );

        let c = Clause::new(
            ClauseId(5),
            vec![Literal::pos(Atom::prop(p))],
            ClauseSource::Inference {
                rule: "resolution",
                parents: vec![ClauseId(0), ClauseId(1)].into(),
            },
        );

        let output = format_tstp(&[parent0, parent1, c], &syms);
        assert!(output.contains("inference(resolution, [status(thm)], [c0, c1])"));
        assert!(output.contains("cnf(c5, plain,"));
    }

    #[test]
    fn format_empty_clause() {
        let syms = SymbolTable::new();

        let parent3 = Clause::new(
            ClauseId(3),
            vec![],
            ClauseSource::Input {
                name: "ax1".into(),
                role: "axiom".into(),
            },
        );
        let parent7 = Clause::new(
            ClauseId(7),
            vec![],
            ClauseSource::Input {
                name: "ax2".into(),
                role: "axiom".into(),
            },
        );

        let c = Clause::new(
            ClauseId(10),
            vec![],
            ClauseSource::Inference {
                rule: "resolution",
                parents: vec![ClauseId(3), ClauseId(7)].into(),
            },
        );

        let output = format_tstp(&[parent3, parent7, c], &syms);
        assert!(output.contains("$false"));
        assert!(output.contains("inference(resolution, [status(thm)], [c3, c7])"));
    }

    #[test]
    fn format_tstp_does_not_emit_incomplete_parent_list() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let child = Clause::new(
            ClauseId(2),
            vec![Literal::pos(Atom::prop(p))],
            ClauseSource::Inference {
                rule: "resolution",
                parents: vec![ClauseId(99)].into(),
            },
        );
        assert!(format_tstp(&[child], &symbols).is_empty());
    }

    #[test]
    fn format_ac_inference_with_auxiliary_axiom_parent() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let f = symbols.intern("f");
        let a = symbols.intern("a");
        let b = symbols.intern("b");
        let x = Term::Var(0);
        let y = Term::Var(1);

        let left = Clause::new(
            ClauseId(1),
            vec![Literal::pos(Atom::pred(
                p,
                vec![Term::App(f, vec![Term::constant(a), Term::constant(b)])],
            ))],
            ClauseSource::Input {
                name: "left".into(),
                role: "axiom".into(),
            },
        );
        let right = Clause::new(
            ClauseId(2),
            vec![Literal::neg(Atom::pred(
                p,
                vec![Term::App(f, vec![Term::constant(b), Term::constant(a)])],
            ))],
            ClauseSource::Input {
                name: "right".into(),
                role: "axiom".into(),
            },
        );
        let commutativity = Clause::new(
            ClauseId(3),
            vec![Literal::pos(Atom::eq(
                Term::App(f, vec![x.clone(), y.clone()]),
                Term::App(f, vec![y, x]),
            ))],
            ClauseSource::Input {
                name: "commutativity".into(),
                role: "axiom".into(),
            },
        );
        let mut conclusion = Clause::new(
            ClauseId(4),
            vec![],
            ClauseSource::Inference {
                rule: "ac_resolution",
                parents: vec![ClauseId(1), ClauseId(2), ClauseId(3)].into(),
            },
        );
        conclusion.witness = Some(ProofWitness::Resolution {
            parent_left: ProofNodeId(1),
            parent_right: ProofNodeId(2),
            lit_idx_left: 0,
            lit_idx_right: 0,
            renaming_offset: 0,
            unifier: None,
        });

        let output = format_tstp(&[left, right, commutativity, conclusion], &symbols);
        assert!(output.contains("inference(ac_resolution, [status(thm)], [c1, c2, c3])"));
        assert!(output.contains("cnf(c4, plain, $false"));
    }

    #[test]
    fn format_formula_step_uses_fof_wrapper() {
        use mrs_core::Formula;
        let _guard = PROBLEM_PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());

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
        let path = problem_path();
        assert!(output.contains(&format!("fof(c0, axiom, p(a), file('{path}', 'ax1')).")));
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

        let parent1 = Clause::new_formula_step(
            ClauseId(1),
            Formula::atom(Atom::prop(p)),
            ClauseSource::Input {
                name: "ax1".into(),
                role: "axiom".into(),
            },
        );
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

        let output = format_tstp(&[parent1, skolem_step, neg_conj_step], &syms);
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

        let parent2 = Clause::new(
            ClauseId(2),
            vec![],
            ClauseSource::Input {
                name: "ax1".into(),
                role: "axiom".into(),
            },
        );

        let final_clause = Clause::new(
            ClauseId(4),
            vec![Literal::pos(Atom::prop(p))],
            ClauseSource::Inference {
                rule: "cnf_transformation",
                parents: vec![ClauseId(2)].into(),
            },
        );

        let output = format_tstp(&[parent2, final_clause], &syms);
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
            ClauseSource::Introduced {
                symbol: def,
                parents: smallvec::SmallVec::new(),
            },
        );

        let output = format_tstp(&[def_step], &syms);
        assert!(output.contains(
            "fof(c5, definition, (def_ax1_0 <=> q), introduced(definition, [new_symbols(definition, [def_ax1_0])]))."
        ));
    }

    #[test]
    fn format_avatar_certificate_metadata() {
        use mrs_core::clause::{AvatarComponent, ClauseCertificate};

        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let parent = Clause::new(
            mrs_core::clause::ClauseId(1),
            vec![Literal::pos(Atom::pred(p, vec![]))],
            ClauseSource::Input {
                name: "parent".into(),
                role: "axiom".into(),
            },
        );
        let clause = Clause::new(
            mrs_core::clause::ClauseId(3),
            vec![Literal::pos(Atom::pred(p, vec![]))],
            ClauseSource::Inference {
                rule: "avatar_split_clause",
                parents: vec![mrs_core::clause::ClauseId(1)].into(),
            },
        );
        let mut clause = clause;
        clause.certificate = Some(ClauseCertificate::AvatarSplit {
            inherited: vec![],
            components: vec![AvatarComponent {
                branch_index: 0,
                sat_var: 7,
                literal_indices: vec![0],
            }],
        });
        let output = format_tstp(&[parent, clause], &symbols);
        assert!(output.contains("avatar_split([branch(0, spl0_7, [0])], [])"));
    }

    #[test]
    fn format_avatar_step_status_uses_esa() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");

        let input = Clause::new(
            ClauseId(0),
            vec![Literal::pos(Atom::pred(p, vec![]))],
            ClauseSource::Input {
                name: "input".into(),
                role: "axiom".into(),
            },
        );

        let split_clause = Clause::new(
            ClauseId(1),
            vec![Literal::pos(Atom::pred(p, vec![]))],
            ClauseSource::Inference {
                rule: "avatar_split_clause",
                parents: vec![ClauseId(0)].into(),
            },
        );
        let component_clause = Clause::new(
            ClauseId(2),
            vec![Literal::pos(Atom::pred(p, vec![]))],
            ClauseSource::Inference {
                rule: "avatar_component_clause",
                parents: vec![ClauseId(1)].into(),
            },
        );
        let branch_refutation = Clause::new(
            ClauseId(3),
            vec![],
            ClauseSource::Inference {
                rule: "avatar_branch_refutation",
                parents: vec![ClauseId(2)].into(),
            },
        );
        let sat_refutation = Clause::new(
            ClauseId(4),
            vec![],
            ClauseSource::Inference {
                rule: "avatar_sat_refutation",
                parents: vec![ClauseId(1), ClauseId(3)].into(),
            },
        );

        let output = format_tstp(
            &[
                input,
                split_clause,
                component_clause,
                branch_refutation,
                sat_refutation,
            ],
            &symbols,
        );
        assert!(output.contains("inference(avatar_split_clause, [status(esa)"));
        assert!(output.contains("inference(avatar_component_clause, [status(esa)"));
        assert!(output.contains("inference(avatar_branch_refutation, [status(esa)"));
        assert!(output.contains("inference(avatar_sat_refutation, [status(thm)"));
    }
}
