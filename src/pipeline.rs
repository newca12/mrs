//! The shared parse → lower → include → clausify pipeline.
//!
//! Both the `mrs` binary and the offline pre-phase dumper in `mrs-bench` need
//! the exact clause set the search will see. Keeping one implementation is not a
//! convenience: the dumper's feature vector is only a description of the search's
//! real input if it is computed from the same clauses, and a second copy of the
//! clausification loop is a permanent invitation for the two to drift.

use std::path::Path;
use std::path::PathBuf;

use mrs_core::Formula;
use mrs_core::clause::{Clause, ClauseSource};

use crate::lowering::{LoweredProblem, lower_problem};
use mrs_prephase::MetaInput;
use mrs_tptp::TPTPProblem;

/// A problem lowered and clausified, together with the metadata the pre-phase
/// needs and the non-clausal proof steps that document the translation.
pub struct Prepared {
    pub lowered: LoweredProblem,
    /// Input clauses plus the clauses of every axiom, plus the negated
    /// conjecture. This is what the search is handed.
    pub clauses: Vec<Clause>,
    /// NNF / Skolemization / conjecture-negation steps, for the TSTP proof.
    pub provenance: Vec<Clause>,
    pub meta: MetaInput,
    /// Include resolution errors are retained so callers can warn the user or
    /// mark a corpus row as a partial input instead of silently analysing it.
    pub include_error: Option<String>,
    #[allow(dead_code)] // read by the pre-phase wiring added below
    pub has_conjecture: bool,
}

/// Lower and clausify an already-parsed problem.
///
/// `problem_path` is used only to resolve `%include` directives; pass `None` for
/// stdin input, which cannot include anything.
pub fn prepare(
    problem: &TPTPProblem<'_>,
    problem_path: Option<&str>,
    input_bytes: Option<u64>,
) -> Prepared {
    let mut lowered = lower_problem(problem);

    let mut meta = MetaInput {
        input_bytes,
        dialect: predominant_dialect(problem),
        n_includes: problem.includes.len(),
        n_input_formulas: problem.formulas.len(),
        ..MetaInput::default()
    };
    for formula in &problem.formulas {
        match formula.role() {
            mrs_tptp::FormulaRole::Conjecture | mrs_tptp::FormulaRole::NegatedConjecture => {
                meta.n_conjecture_formulas += 1;
            }
            mrs_tptp::FormulaRole::Type => meta.n_type_formulas += 1,
            mrs_tptp::FormulaRole::Definition => meta.n_definition_formulas += 1,
            _ => {}
        }
        if matches!(formula, mrs_tptp::AnnotatedFormula::CNF(_)) {
            meta.n_input_cnf_clauses += 1;
        }
    }

    let include_error = if !problem.includes.is_empty()
        && let Some(path) = problem_path
    {
        let base_dir = Path::new(path).parent().unwrap_or(Path::new("."));
        let tptp_root: Option<PathBuf> = std::env::var("TPTP").ok().map(PathBuf::from);
        // Keep the established behavior of searching the resolvable top-level
        // formulas after an include failure, but surface that the view is partial.
        crate::include::resolve_and_lower(problem, &mut lowered, base_dir, tptp_root.as_deref())
            .err()
            .map(|error| error.to_string())
    } else {
        None
    };

    let (clauses, provenance) = clausify(&mut lowered);
    let has_conjecture = !lowered.conjectures.is_empty();
    Prepared {
        lowered,
        clauses,
        provenance,
        meta,
        include_error,
        has_conjecture,
    }
}

/// Clausify a lowered problem into the clause set the search sees.
///
/// Conjectures are negated (refutation-based proving: `axioms ∧ ¬P` must be
/// unsatisfiable), negated-goal clauses carry `distance == 0` so the
/// goal-directed heuristics treat them as goal-connected, and the
/// FOF-level translation steps are returned separately as proof provenance rather
/// than entering the search.
pub fn clausify(lowered: &mut LoweredProblem) -> (Vec<Clause>, Vec<Clause>) {
    let mut id_gen = lowered.id_gen.clone();
    let mut all_clauses: Vec<Clause> = lowered
        .cnf_clauses
        .iter()
        .map(|c| {
            let is_negated_conjecture = matches!(
                &c.source,
                ClauseSource::Input { role, .. } if role == "negated_conjecture"
            );
            c.clone()
                .with_distance(if is_negated_conjecture { 0 } else { 100 })
        })
        .collect();
    let mut provenance: Vec<Clause> = Vec::new();

    for formula in &lowered.axioms {
        let leaf_source = ClauseSource::Input {
            name: formula.name.clone(),
            role: formula.role.clone(),
        };
        let (steps, clauses) = mrs_cnf::clausify_with_provenance(
            &formula.formula,
            &mut lowered.symbols,
            &mut id_gen,
            &formula.name,
            leaf_source,
            None,
        );
        provenance.extend(steps);
        all_clauses.extend(clauses.into_iter().map(|c| c.with_distance(100)));
    }

    for formula in &lowered.conjectures {
        let conjecture_leaf = id_gen.next();
        provenance.push(Clause::new_formula_step(
            conjecture_leaf,
            formula.formula.clone(),
            ClauseSource::Input {
                name: formula.name.clone(),
                role: "conjecture".to_string(),
            },
        ));
        let negated = Formula::neg(formula.formula.clone());
        let (steps, clauses) = mrs_cnf::clausify_with_provenance(
            &negated,
            &mut lowered.symbols,
            &mut id_gen,
            &formula.name,
            ClauseSource::Inference {
                rule: "negated_conjecture",
                parents: vec![conjecture_leaf].into(),
            },
            None,
        );
        provenance.extend(steps);
        all_clauses.extend(clauses.into_iter().map(|c| c.with_distance(0)));
    }

    (all_clauses, provenance)
}

fn predominant_dialect(problem: &TPTPProblem<'_>) -> String {
    use std::collections::HashMap;
    let mut counts: HashMap<&'static str, usize> = HashMap::new();
    for formula in &problem.formulas {
        let name = match formula {
            mrs_tptp::AnnotatedFormula::THF(_) => "THF",
            mrs_tptp::AnnotatedFormula::TFF(_) => "TFF",
            mrs_tptp::AnnotatedFormula::FOF(_) => "FOF",
            mrs_tptp::AnnotatedFormula::TCF(_) => "TCF",
            mrs_tptp::AnnotatedFormula::CNF(_) => "CNF",
            mrs_tptp::AnnotatedFormula::TPI(_) => "TPI",
        };
        *counts.entry(name).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(name, _)| name.to_string())
        .unwrap_or_else(|| "CNF".to_string())
}
