//! Clausification: converting first-order formulas to clause normal form.
//!
//! This crate implements the standard clausification pipeline:
//!
//! 1. **NNF** ([`nnf`]) - Negation Normal Form: push negations inward
//! 2. **Miniscoping** ([`miniscope`]) - Push quantifiers inward to reduce Skolem arity
//! 3. **Skolemization** ([`skolem`]) - Eliminate existential quantifiers
//! 4. **CNF conversion** ([`cnf`]) - Convert to conjunctive normal form
//! 5. **Clause extraction** ([`flatten`]) - Extract clauses from CNF formula
//! 6. **Simplification** ([`simplify`]) - Remove tautologies and duplicates
//!
//! The main entry point is [`clausify`], which runs the full pipeline.
//!
//! # Examples
//!
//! ```
//! use mrs_core::{Formula, Atom, Term, SymbolTable};
//! use mrs_core::clause::ClauseIdGen;
//! use mrs_cnf::clausify;
//!
//! let mut syms = SymbolTable::new();
//! let p = syms.intern("p");
//! let a = syms.intern("a");
//!
//! // Clausify: ∀X. p(X) => p(a)
//! let formula = Formula::forall(0,
//!     Formula::implies(
//!         Formula::atom(Atom::pred(p, vec![Term::var(0)])),
//!         Formula::atom(Atom::pred(p, vec![Term::constant(a)])),
//!     )
//! );
//!
//! let mut id_gen = ClauseIdGen::new();
//! let clauses = clausify(&formula, &mut syms, &mut id_gen, "test", "axiom");
//! assert!(!clauses.is_empty());
//! ```

pub mod cnf;
pub mod definitional;
pub mod flatten;
pub mod miniscope;
pub mod nnf;
pub mod simplify;
pub mod skolem;

use mrs_core::clause::{Clause, ClauseId, ClauseIdGen, ClauseSource};
use mrs_core::{Formula, SymbolTable};

/// Clausifies a formula through the full pipeline.
///
/// Returns a vector of clauses. The `name` and `role` are used to
/// tag the clause source for proof reconstruction.
///
/// Uses definitional CNF (Tseitin) when the formula contains conjunctions
/// under disjunctions (which would cause exponential blowup with distributive
/// CNF). Falls back to simple distributive CNF for formulas without blowup.
///
/// This discards the intermediate FOF-level provenance (NNF/Skolemization
/// steps) — use [`clausify_with_provenance`] to get those too (needed for a
/// TSTP-acceptable proof; see CASC's evaluation criteria on documenting
/// FOF-to-CNF translations). This entry point remains for callers that only
/// need the final clauses and don't produce proof output themselves (e.g.
/// `mrs-proover`'s in-process ATP oracle).
pub fn clausify(
    formula: &Formula,
    symbols: &mut SymbolTable,
    id_gen: &mut ClauseIdGen,
    name: &str,
    role: &str,
) -> Vec<Clause> {
    let leaf_source = ClauseSource::Input {
        name: name.to_string(),
        role: role.to_string(),
    };
    let (_provenance, clauses) =
        clausify_with_provenance(formula, symbols, id_gen, name, leaf_source, None);
    clauses
}

/// Clausifies a formula, also returning the intermediate FOF-level
/// provenance steps (NNF conversion, Skolemization) as separate,
/// citable proof steps.
///
/// Returns `(provenance, clauses)`:
/// - `provenance`: non-clausal FOF-level proof steps — the original leaf
///   formula (cited via `leaf_source`), the `fof_nnf_transformation` step,
///   and the `skolemisation` step (status `esa`). These use
///   [`Clause::new_formula_step`] (empty `literals`, real content in
///   `formula`) and **must never be added to the live given-clause search**
///   (`processed`/`unprocessed`) — only to `clause_store`, for proof
///   extraction. See [`Clause::formula`] for why.
/// - `clauses`: the final, real CNF clauses to feed the search. Each cites
///   the Skolemization step as its parent via a `cnf_transformation`
///   inference (rather than the previous behaviour of citing the original
///   axiom directly as `Input`), so a checker can now see the whole
///   FOF-to-CNF translation instead of just its end result.
///
/// `leaf_id_override` lets a caller supply the leaf's `ClauseId` up front
/// (e.g. when the leaf is itself the target of a preceding step, such as a
/// `negated_conjecture` inference the caller has already created) instead of
/// letting this function allocate a fresh one from `id_gen`.
pub fn clausify_with_provenance(
    formula: &Formula,
    symbols: &mut SymbolTable,
    id_gen: &mut ClauseIdGen,
    name: &str,
    leaf_source: ClauseSource,
    leaf_id_override: Option<ClauseId>,
) -> (Vec<Clause>, Vec<Clause>) {
    let leaf_id = leaf_id_override.unwrap_or_else(|| id_gen.next());
    let leaf_step = Clause::new_formula_step(leaf_id, formula.clone(), leaf_source);

    // Step 1: Convert to NNF
    let nnf_formula = nnf::to_nnf(formula);
    let nnf_id = id_gen.next();
    let nnf_step = Clause::new_formula_step(
        nnf_id,
        nnf_formula.clone(),
        ClauseSource::Inference {
            rule: "fof_nnf_transformation",
            parents: vec![leaf_id].into(),
        },
    );

    // Step 2: Miniscope (push quantifiers inward to reduce Skolem arity).
    // Purely structural (preserves logical equivalence exactly, introduces
    // no new symbols) — folded into the NNF->Skolemization boundary rather
    // than cited as its own step.
    let mini_formula = miniscope::miniscope(&nnf_formula);

    // Step 3: Skolemize (eliminate existential quantifiers)
    let skolem_formula = skolem::skolemize(&mini_formula, symbols, name);
    let skolem_id = id_gen.next();
    let skolem_step = Clause::new_formula_step(
        skolem_id,
        skolem_formula.clone(),
        ClauseSource::Inference {
            rule: "skolemisation",
            parents: vec![nnf_id].into(),
        },
    );

    // Step 4: Drop universal quantifiers (implicit in clausal form). Purely
    // structural, like miniscoping — no separate citation.
    let stripped = strip_forall(&skolem_formula);

    // Step 5: Convert to CNF
    // Use definitional CNF if the formula has And-under-Or (blowup risk),
    // otherwise use simple distributive CNF (no extra definition symbols).
    let cnf_formula = if has_and_under_or(&stripped) {
        definitional::to_cnf_definitional(&stripped, symbols, name)
    } else {
        cnf::to_cnf(&stripped)
    };

    // Step 6: Extract clauses, citing the Skolemization step as parent.
    let cnf_source = ClauseSource::Inference {
        rule: "cnf_transformation",
        parents: vec![skolem_id].into(),
    };
    let clauses = flatten::extract_clauses(&cnf_formula, id_gen, &cnf_source);

    // Step 7: Simplify (only the real clauses; provenance steps are exact
    // transformation records and are not simplified).
    let clauses = simplify::simplify_clauses(clauses);

    (vec![leaf_step, nnf_step, skolem_step], clauses)
}

/// Strips all universal quantifiers from a formula, recursing into
/// And/Or/Neg nodes. After Skolemization, all remaining quantifiers
/// are universal, and in clausal form they are implicit.
fn strip_forall(formula: &Formula) -> Formula {
    match formula {
        Formula::Forall(_, body) => strip_forall(body),
        Formula::And(cs) => Formula::and(cs.iter().map(strip_forall).collect()),
        Formula::Or(ds) => Formula::or(ds.iter().map(strip_forall).collect()),
        Formula::Neg(inner) => Formula::neg(strip_forall(inner)),
        other => other.clone(),
    }
}

/// Returns true if the formula contains an And node that is a descendant
/// of an Or node. This pattern causes exponential blowup with distributive
/// CNF, so we use definitional CNF instead.
fn has_and_under_or(formula: &Formula) -> bool {
    match formula {
        Formula::Or(ds) => ds.iter().any(contains_and),
        Formula::And(cs) => cs.iter().any(has_and_under_or),
        _ => false,
    }
}

/// Returns true if the formula contains an And node (at any depth).
fn contains_and(formula: &Formula) -> bool {
    match formula {
        Formula::And(_) => true,
        Formula::Or(ds) => ds.iter().any(contains_and),
        _ => false,
    }
}

#[cfg(test)]
mod provenance_tests {
    use super::*;
    use mrs_core::{Atom, Term};

    #[test]
    fn provenance_chain_has_three_steps_with_correct_rules_and_parents() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");

        // ![X]: (p(X) => p(a)) -- needs real NNF + Skolemization + CNF work.
        let formula = Formula::forall(
            0,
            Formula::implies(
                Formula::atom(Atom::pred(p, vec![Term::var(0)])),
                Formula::atom(Atom::pred(p, vec![Term::constant(a)])),
            ),
        );

        let mut id_gen = ClauseIdGen::new();
        let leaf_source = ClauseSource::Input {
            name: "ax1".to_string(),
            role: "axiom".to_string(),
        };
        let (provenance, clauses) =
            clausify_with_provenance(&formula, &mut syms, &mut id_gen, "ax1", leaf_source, None);

        assert_eq!(provenance.len(), 3, "expected leaf + nnf + skolem steps");
        assert!(!clauses.is_empty());

        let leaf = &provenance[0];
        let nnf_step = &provenance[1];
        let skolem_step = &provenance[2];

        // All provenance steps are formula-level (non-clausal): empty
        // literals, real content in `formula`.
        for step in &provenance {
            assert!(step.literals.is_empty());
            assert!(step.formula.is_some());
        }

        assert!(matches!(&leaf.source, ClauseSource::Input { name, role }
            if name == "ax1" && role == "axiom"));

        match &nnf_step.source {
            ClauseSource::Inference { rule, parents } => {
                assert_eq!(*rule, "fof_nnf_transformation");
                assert_eq!(parents.as_slice(), [leaf.id]);
            }
            other => panic!("expected Inference, got {other:?}"),
        }

        match &skolem_step.source {
            ClauseSource::Inference { rule, parents } => {
                assert_eq!(*rule, "skolemisation");
                assert_eq!(parents.as_slice(), [nnf_step.id]);
            }
            other => panic!("expected Inference, got {other:?}"),
        }

        // Every final clause must cite the skolemisation step as its parent
        // via a cnf_transformation inference -- this is exactly what was
        // missing before (final clauses used to cite the axiom directly).
        for c in &clauses {
            match &c.source {
                ClauseSource::Inference { rule, parents } => {
                    assert_eq!(*rule, "cnf_transformation");
                    assert_eq!(parents.as_slice(), [skolem_step.id]);
                }
                other => panic!("expected Inference, got {other:?}"),
            }
            assert!(
                c.formula.is_none(),
                "final clauses are real clauses, not formula steps"
            );
        }
    }

    #[test]
    fn clausify_still_returns_only_final_clauses_unchanged() {
        // The plain `clausify` wrapper must keep working for callers that
        // don't care about provenance (e.g. mrs-proover's in-process ATP).
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let mut id_gen = ClauseIdGen::new();

        let formula = Formula::atom(Atom::prop(p));
        let clauses = clausify(&formula, &mut syms, &mut id_gen, "ax1", "axiom");
        assert_eq!(clauses.len(), 1);
        assert!(clauses[0].formula.is_none());
    }
}
