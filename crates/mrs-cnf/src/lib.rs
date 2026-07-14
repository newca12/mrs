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
    let (cnf_formula, definitions) = if has_and_under_or(&stripped) {
        definitional::to_cnf_definitional_with_defs(&stripped, symbols, name)
    } else {
        (cnf::to_cnf(&stripped), Vec::new())
    };

    // Step 5b: for each fresh definitional predicate Tseitin introduced,
    // emit its full biconditional as its own `introduced(definition)` step
    // (no parents, no status — sound by construction, since the symbol is
    // guaranteed fresh: a conservative extension). See
    // `to_cnf_definitional_with_defs`'s doc comment for why citing only the
    // Skolemization step as a clause's parent does not work once that
    // clause mentions one of these fresh symbols.
    let mut def_provenance = Vec::with_capacity(definitions.len());
    let mut def_id_by_symbol: std::collections::HashMap<mrs_core::SymbolId, ClauseId> =
        std::collections::HashMap::new();
    for (def_atom, conjuncts) in &definitions {
        let def_id = id_gen.next();
        let rhs = if conjuncts.len() == 1 {
            conjuncts[0].clone()
        } else {
            Formula::and(conjuncts.clone())
        };
        let biconditional = Formula::iff(Formula::atom(def_atom.clone()), rhs);
        // The definitional-CNF pass runs after `strip_forall`, so the
        // biconditional's variables (shared with the enclosing clause) are
        // free here. FOF requires closed formulas, so re-close with
        // explicit universal quantifiers before citing it as its own step
        // (confirmed against a real GDV build: an unquantified variable is
        // a hard parse error, not just a soundness nit).
        let mut free_vars: Vec<_> = biconditional.free_vars().into_iter().collect();
        free_vars.sort_unstable();
        let closed_biconditional = free_vars
            .into_iter()
            .rev()
            .fold(biconditional, |body, v| Formula::forall(v, body));
        let def_sym = match def_atom {
            mrs_core::Atom::Pred(sym, _) => *sym,
            mrs_core::Atom::Eq(..) => {
                unreachable!("definitional CNF only introduces fresh predicate symbols")
            }
        };
        def_provenance.push(Clause::new_formula_step(
            def_id,
            closed_biconditional,
            ClauseSource::Introduced { symbol: def_sym },
        ));
        def_id_by_symbol.insert(def_sym, def_id);
    }

    // Step 6: Extract clauses, citing the Skolemization step as parent.
    let cnf_source = ClauseSource::Inference {
        rule: "cnf_transformation",
        parents: vec![skolem_id].into(),
    };
    let mut clauses = flatten::extract_clauses(&cnf_formula, id_gen, &cnf_source);

    // Step 6b: any clause that actually mentions one of the fresh
    // definitional predicates also needs that definition's introduction
    // step cited as an additional parent -- otherwise no ATP can prove it
    // follows from the Skolemization step alone, since that step's formula
    // never mentions the fresh symbol at all.
    if !def_id_by_symbol.is_empty() {
        for clause in &mut clauses {
            let mut extra_parents: Vec<ClauseId> = Vec::new();
            for lit in &clause.literals {
                let sym = match &lit.atom {
                    mrs_core::Atom::Pred(s, _) => Some(*s),
                    mrs_core::Atom::Eq(..) => None,
                };
                if let Some(sym) = sym
                    && let Some(&def_id) = def_id_by_symbol.get(&sym)
                    && !extra_parents.contains(&def_id)
                {
                    extra_parents.push(def_id);
                }
            }
            if !extra_parents.is_empty()
                && let ClauseSource::Inference { parents, .. } = &mut clause.source
            {
                for p in extra_parents {
                    if !parents.contains(&p) {
                        parents.push(p);
                    }
                }
            }
        }
    }

    // Step 7: Simplify (only the real clauses; provenance steps are exact
    // transformation records and are not simplified).
    let clauses = simplify::simplify_clauses(clauses);

    let mut provenance = vec![leaf_step, nnf_step, skolem_step];
    provenance.extend(def_provenance);

    (provenance, clauses)
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

    #[test]
    fn definitional_cnf_clauses_cite_the_definition_step_as_extra_parent() {
        // Regression test for a real GDV failure found reviewing SEU140+2:
        // definitional (Tseitin) CNF introduces a fresh `def_...` predicate
        // that does not appear in the cited parent (the pre-CNF Skolemized
        // formula) -- exactly like Skolemization introduces fresh Skolem
        // functions absent from ITS parent. GDV correctly refused to accept
        // these as full `thm` consequences of the Skolemization step alone
        // (it found a genuine CounterSatisfiable countermodel when asked to
        // prove the def_-mentioning clause as a THM of a parent that never
        // mentions def_ at all). The fix: emit the fresh predicate's full
        // biconditional as its own `introduced(definition)` step (sound by
        // construction, no parents needed), and have every clause that
        // actually mentions the fresh predicate cite that step as an
        // *additional* parent alongside the Skolemization step -- both
        // together are ordinary `thm` (`cnf_transformation`) consequences.
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let r = syms.intern("r");

        // p(X) | (q(X) & r(X)) -- And-under-Or, forces definitional CNF.
        let formula = Formula::forall(
            0,
            Formula::or(vec![
                Formula::atom(Atom::pred(p, vec![Term::var(0)])),
                Formula::and(vec![
                    Formula::atom(Atom::pred(q, vec![Term::var(0)])),
                    Formula::atom(Atom::pred(r, vec![Term::var(0)])),
                ]),
            ]),
        );

        let mut id_gen = ClauseIdGen::new();
        let leaf_source = ClauseSource::Input {
            name: "ax1".to_string(),
            role: "axiom".to_string(),
        };
        let (provenance, clauses) =
            clausify_with_provenance(&formula, &mut syms, &mut id_gen, "ax1", leaf_source, None);

        // At least one provenance step introduces a definition, with no
        // parents at all.
        let def_ids: Vec<ClauseId> = provenance
            .iter()
            .filter(|c| matches!(c.source, ClauseSource::Introduced { .. }))
            .map(|c| c.id)
            .collect();
        assert!(
            !def_ids.is_empty(),
            "expected at least one ClauseSource::Introduced provenance step"
        );

        assert!(!clauses.is_empty());
        let mut saw_def_citing_clause = false;
        for c in &clauses {
            match &c.source {
                ClauseSource::Inference { rule, parents } => {
                    assert_eq!(*rule, "cnf_transformation");
                    // Every clause that mentions the fresh def_ predicate
                    // must cite one of the definition steps as a parent.
                    let mentions_def_symbol = c.literals.iter().any(|lit| {
                        matches!(lit.atom, Atom::Pred(sym, _) if syms.resolve(sym).starts_with("def_"))
                    });
                    if mentions_def_symbol {
                        assert!(
                            parents.iter().any(|p| def_ids.contains(p)),
                            "clause mentioning a def_ predicate must cite its introduction step"
                        );
                        saw_def_citing_clause = true;
                    }
                }
                other => panic!("expected Inference, got {other:?}"),
            }
        }
        assert!(
            saw_def_citing_clause,
            "expected at least one clause to mention a def_ predicate"
        );
    }

    #[test]
    fn plain_distributive_cnf_still_cites_cnf_transformation() {
        // No And-under-Or: plain distributive CNF, no fresh symbols
        // introduced, so this must keep the ordinary `cnf_transformation`
        // (status thm) rule name -- only the definitional/Tseitin path
        // needs the new esa-mapped rule name.
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");

        let formula = Formula::forall(
            0,
            Formula::or(vec![
                Formula::atom(Atom::pred(p, vec![Term::var(0)])),
                Formula::atom(Atom::pred(q, vec![Term::var(0)])),
            ]),
        );

        let mut id_gen = ClauseIdGen::new();
        let leaf_source = ClauseSource::Input {
            name: "ax1".to_string(),
            role: "axiom".to_string(),
        };
        let (_provenance, clauses) =
            clausify_with_provenance(&formula, &mut syms, &mut id_gen, "ax1", leaf_source, None);

        assert!(!clauses.is_empty());
        for c in &clauses {
            match &c.source {
                ClauseSource::Inference { rule, .. } => {
                    assert_eq!(*rule, "cnf_transformation");
                }
                other => panic!("expected Inference, got {other:?}"),
            }
        }
    }
}
