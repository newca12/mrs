//! Instantiation-based reasoning for EPR (Essentially Propositional Reasoning)
//! problems.
//!
//! A clause set is **EPR** iff every term in every literal is either a variable
//! or a constant (nullary function application).  No function symbol of arity ≥ 1
//! occurs.  For EPR the Herbrand universe is finite — it is exactly the set of
//! distinct constants that appear in the clauses.
//!
//! The [`preprocess_epr`] function detects EPR problems and, if detected, expands
//! every non-ground clause into all of its ground instances over the Herbrand
//! universe.  The resulting ground clause set is then passed to the ordinary
//! given-clause loop, which will quickly saturate (SAT) or derive the empty
//! clause (UNSAT).
//!
//! ## Termination guarantee
//! Instantiation over a finite Herbrand universe terminates.  We add a hard
//! instance-count limit (`MAX_INSTANCES`) to protect against combinatorial
//! explosions caused by clauses with many variables.  When the limit would be
//! exceeded we return `None` and the caller falls back to the standard loop.

use crate::HashSet;

use mrs_core::clause::{Clause, ClauseIdGen, ClauseSource, Literal};
use mrs_core::formula::Atom;
use mrs_core::subst::Substitution;
use mrs_core::symbol::SymbolId;
use mrs_core::term::{Term, VarId};

/// Maximum total number of ground instances we are willing to generate.
/// If expanding the clause set would exceed this, we fall back to the
/// standard loop.
const MAX_INSTANCES: usize = 200_000;

/// Tries to preprocess `clauses` as an EPR problem.
///
/// Returns `Some(ground_clauses)` if the clause set is EPR and the expansion
/// fits within [`MAX_INSTANCES`].  Returns `None` otherwise.
///
/// `id_gen` is advanced as new ground clause IDs are minted.
pub fn preprocess_epr(clauses: &[Clause], id_gen: &mut ClauseIdGen) -> Option<Vec<Clause>> {
    if !is_epr(clauses) {
        return None;
    }

    let constants = collect_constants(clauses);
    if constants.is_empty() {
        // No constants at all; the Herbrand universe is empty.
        // This only happens when every clause is all-variable — e.g. { p(X) | ~p(X) }.
        // We fall back to the standard loop which handles this naturally.
        return None;
    }

    // Pre-check: estimate total instance count to avoid blowing up.
    let mut total: usize = 0;
    for clause in clauses {
        let n_vars = collect_clause_vars(clause).len();
        let instances = constants.len().saturating_pow(n_vars as u32);
        total = total.saturating_add(instances);
        if total > MAX_INSTANCES {
            return None;
        }
    }

    // Generate ground instances.
    let mut ground_clauses = Vec::new();
    for clause in clauses {
        let vars: Vec<VarId> = collect_clause_vars(clause).into_iter().collect();
        if vars.is_empty() {
            // Already ground — keep as-is.
            ground_clauses.push(clause.clone());
        } else {
            for subst in enumerate_instances(&vars, &constants) {
                let new_lits: Vec<Literal> = clause
                    .literals
                    .iter()
                    .map(|lit| subst.apply_literal(lit))
                    .collect();
                let new_clause = Clause::new(
                    id_gen.next(),
                    new_lits,
                    ClauseSource::Inference {
                        rule: "instantiation".into(),
                        parents: vec![clause.id],
                    },
                );
                ground_clauses.push(new_clause);
            }
        }
    }

    // Include the original non-ground clauses in the output so they end up
    // in `clause_store` and proof extraction can follow the `Inference` parent
    // pointers back to them.
    //
    // These originals will also be placed in `unprocessed` by `SearchState::new`,
    // which is harmless: they are non-ground and will be subsumed immediately by
    // their ground instances, or will just generate no useful new inferences in
    // a ground-complete search.
    for clause in clauses {
        let vars = collect_clause_vars(clause);
        if !vars.is_empty() {
            ground_clauses.push(clause.clone());
        }
    }

    Some(ground_clauses)
}

/// Returns `true` iff every clause in `clauses` is EPR: all terms are either
/// variables or constants (nullary function applications).
///
/// This is `pub` so that callers can disable AVATAR for EPR problems even when
/// the full Herbrand expansion exceeds [`MAX_INSTANCES`] and `preprocess_epr`
/// returns `None`.
pub fn is_epr(clauses: &[Clause]) -> bool {
    clauses.iter().all(|c| {
        c.literals.iter().all(|lit| match &lit.atom {
            Atom::Pred(_, args) => args.iter().all(term_is_epr),
            Atom::Eq(l, r) => term_is_epr(l) && term_is_epr(r),
        })
    })
}

/// Returns `true` if `term` is a variable or a constant.
fn term_is_epr(term: &Term) -> bool {
    match term {
        Term::Var(_) => true,
        Term::App(_, args) => args.is_empty(),
    }
}

/// Collects all distinct constants (nullary function symbols) from `clauses`.
fn collect_constants(clauses: &[Clause]) -> Vec<SymbolId> {
    let mut seen: HashSet<SymbolId> = HashSet::default();
    let mut constants: Vec<SymbolId> = Vec::new();
    for clause in clauses {
        for lit in &clause.literals {
            match &lit.atom {
                Atom::Pred(_, args) => {
                    for t in args {
                        collect_constants_term(t, &mut seen, &mut constants);
                    }
                }
                Atom::Eq(l, r) => {
                    for t in [l, r] {
                        collect_constants_term(t, &mut seen, &mut constants);
                    }
                }
            }
        }
    }
    constants
}

fn collect_constants_term(term: &Term, seen: &mut HashSet<SymbolId>, out: &mut Vec<SymbolId>) {
    if let Term::App(sym, args) = term
        && args.is_empty()
        && seen.insert(*sym)
    {
        out.push(*sym);
    }
}

/// Collects all distinct variable IDs from a single clause.
fn collect_clause_vars(clause: &Clause) -> HashSet<VarId> {
    let mut vars: HashSet<VarId> = HashSet::default();
    for lit in &clause.literals {
        match &lit.atom {
            Atom::Pred(_, args) => {
                for t in args {
                    collect_vars_term(t, &mut vars);
                }
            }
            Atom::Eq(l, r) => {
                collect_vars_term(l, &mut vars);
                collect_vars_term(r, &mut vars);
            }
        }
    }
    vars
}

fn collect_vars_term(term: &Term, vars: &mut HashSet<VarId>) {
    match term {
        Term::Var(v) => {
            vars.insert(*v);
        }
        Term::App(_, args) => {
            for a in args {
                collect_vars_term(a, vars);
            }
        }
    }
}

/// Enumerates all substitutions that map each variable in `vars` to some
/// constant in `constants`.  Returns `|constants|^|vars|` substitutions.
fn enumerate_instances(vars: &[VarId], constants: &[SymbolId]) -> Vec<Substitution> {
    let mut result = vec![Substitution::new()];
    for &var in vars {
        let mut next_result = Vec::with_capacity(result.len() * constants.len());
        for subst in &result {
            for &c in constants {
                let mut new_subst = subst.clone();
                new_subst.bind(var, Term::constant(c));
                next_result.push(new_subst);
            }
        }
        result = next_result;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::clause::{ClauseIdGen, ClauseSource};
    use mrs_core::{Atom, Literal, SymbolTable, Term};

    fn input_clause(id_gen: &mut ClauseIdGen, lits: Vec<Literal>, name: &str) -> Clause {
        Clause::new(
            id_gen.next(),
            lits,
            ClauseSource::Input {
                name: name.into(),
                role: "axiom".into(),
            },
        )
    }

    #[test]
    fn epr_detection_pure_propositional() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();
        let clause = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
            "ax",
        );
        assert!(is_epr(&[clause]));
    }

    #[test]
    fn epr_detection_with_variables() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let mut id_gen = ClauseIdGen::new();
        let clause = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::var(0)]))],
            "ax",
        );
        assert!(is_epr(&[clause]));
    }

    #[test]
    fn epr_detection_non_epr_function() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let f = syms.intern("f");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();
        // p(f(a)) — f has arity 1, so NOT EPR
        let clause = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(
                p,
                vec![Term::app(f, vec![Term::constant(a)])],
            ))],
            "ax",
        );
        assert!(!is_epr(&[clause]));
    }

    #[test]
    fn preprocess_epr_ground_instances() {
        // p(X) | ~p(X) with constant a:  should expand to p(a) | ~p(a)
        // which is a tautology — but the important thing is that we get 1 ground clause.
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();

        let ax = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
            "ax_ground",
        );
        // ~p(X): should be instantiated to ~p(a)
        let neg = input_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::pred(p, vec![Term::var(0)]))],
            "ax_neg",
        );

        let result = preprocess_epr(&[ax, neg], &mut id_gen);
        assert!(result.is_some(), "EPR problem should be preprocessed");
        let ground = result.unwrap();
        // Every literal in the ground output must be ground.
        for clause in &ground {
            for lit in &clause.literals {
                let is_ground = match &lit.atom {
                    Atom::Pred(_, args) => args.iter().all(|t| !t.is_var()),
                    Atom::Eq(l, r) => !l.is_var() && !r.is_var(),
                };
                // Original non-ground clauses are appended for the clause store;
                // they may contain variables — skip those.
                if matches!(&clause.source, ClauseSource::Inference { rule, .. } if rule == "instantiation")
                {
                    assert!(
                        is_ground,
                        "instantiated clause should be ground: {:?}",
                        clause
                    );
                }
            }
        }
    }

    #[test]
    fn preprocess_epr_refutation() {
        // Simple EPR refutation: p(a), ~p(X) — should become p(a), ~p(a) -> refutation.
        use crate::given_clause::search;
        use crate::state::SearchState;
        use crate::{SearchConfig, SearchResult};
        use mrs_calculus::ordering::SymbolConfig;
        use std::sync::Arc;

        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();

        let pos = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
            "pos",
        );
        let neg = input_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::pred(p, vec![Term::var(0)]))],
            "neg",
        );

        let clauses = vec![pos, neg];
        let ground = preprocess_epr(&clauses, &mut id_gen).expect("should detect EPR");

        let mut state = SearchState::new(
            ground,
            id_gen,
            Arc::new(SymbolConfig::default()),
            std::sync::Arc::new(mrs_core::SymbolTable::new()),
            false,
        );
        let result = search(&mut state, &SearchConfig::default());
        assert!(
            matches!(result, SearchResult::Refutation(..)),
            "EPR refutation should be found, got {:?}",
            result
        );
    }
}
