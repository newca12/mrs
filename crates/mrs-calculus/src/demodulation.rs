//! Demodulation: simplification by rewriting with unit equalities.
//!
//! Forward demodulation uses oriented unit equalities `l = r` (where `l ≻ r`)
//! to rewrite terms in other clauses. This is a simplification rule that
//! reduces clause complexity without losing completeness.
//!
//! Uses one-way matching (not unification) since unit equalities are
//! universally quantified: we find instances of the left side in the
//! target clause and replace them with corresponding instances of the right side.

use mrs_core::Atom;
use mrs_core::clause::{Clause, ClauseId, ClauseIdGen, ClauseSource, Literal};
use mrs_core::term::Term;
use mrs_unify::match_term;

use crate::ordering::{TermComparison, TermOrdering};

/// Forward demodulation: simplify a clause using oriented unit equalities.
///
/// For each unit equality `{l = r}` where `l ≻ r`, finds instances of `l`
/// in the target clause via one-way matching and replaces them with the
/// corresponding instance of `r`.
///
/// Returns `Some(simplified)` if any rewrite was applied, `None` otherwise.
/// The simplified clause gets a new ID and records a demodulation inference
/// with the original clause and the rewriting unit(s) as parents.
pub fn demodulate(
    clause: &Clause,
    demod_index: &mrs_index::dtree::DTree<(Term, Term, ClauseId)>,
    id_gen: &mut ClauseIdGen,
) -> Option<Clause> {
    let mut current_lits = clause.literals.clone();
    let mut changed = false;
    let mut used_unit_ids = Vec::new();

    // Iterate to fixpoint
    loop {
        let mut changed_this_pass = false;
        for lit in &mut current_lits {
            if rewrite_literal(lit, demod_index, &mut used_unit_ids) {
                changed = true;
                changed_this_pass = true;
            }
        }
        if !changed_this_pass {
            break;
        }
    }

    if changed {
        let mut parents = vec![clause.id];
        parents.extend(used_unit_ids);
        Some(Clause::new(
            id_gen.next(),
            current_lits,
            ClauseSource::Inference {
                rule: "demodulation".into(),
                parents,
            },
        ))
    } else {
        None
    }
}

/// Tries to rewrite terms in a literal using the demodulation index.
/// Returns true if any rewrite was performed.
fn rewrite_literal(lit: &mut Literal, demod_index: &mrs_index::dtree::DTree<(Term, Term, ClauseId)>, used_unit_ids: &mut Vec<ClauseId>) -> bool {
    let mut changed = false;
    let new_atom = match &lit.atom {
        Atom::Pred(p, args) => {
            let new_args: Vec<Term> = args
                .iter()
                .map(|arg| {
                    let (new_arg, ch) = rewrite_term(arg, demod_index, used_unit_ids);
                    if ch {
                        changed = true;
                    }
                    new_arg
                })
                .collect();
            Atom::Pred(*p, new_args)
        }
        Atom::Eq(l, r) => {
            let (new_l, ch_l) = rewrite_term(l, demod_index, used_unit_ids);
            let (new_r, ch_r) = rewrite_term(r, demod_index, used_unit_ids);
            if ch_l || ch_r {
                changed = true;
            }
            Atom::Eq(new_l, new_r)
        }
    };
    if changed {
        lit.atom = new_atom;
    }
    changed
}

/// Rewrites a term using the demodulation index.
/// Recurses into subterms, applying the first match found at each level.
fn rewrite_term(term: &Term, demod_index: &mrs_index::dtree::DTree<(Term, Term, ClauseId)>, used_unit_ids: &mut Vec<ClauseId>) -> (Term, bool) {
    // Try matching at the current position first
    let rules = demod_index.get_generalizations(term);
    for (from, to, unit_id) in rules {
        if let Ok(sigma) = match_term(&from, term) {
            if !used_unit_ids.contains(&unit_id) {
                used_unit_ids.push(unit_id);
            }
            return (apply_matching_subst(&sigma, &to), true);
        }
    }

    // Recurse into subterms
    match term {
        Term::Var(_) => (term.clone(), false),
        Term::App(f, args) => {
            let mut changed = false;
            let new_args: Vec<Term> = args.iter().map(|arg| {
                let (new_arg, ch) = rewrite_term(arg, demod_index, used_unit_ids);
                if ch { changed = true; }
                new_arg
            }).collect();
            if changed {
                (Term::App(*f, new_args), true)
            } else {
                (term.clone(), false)
            }
        }
    }
}

/// Applies a matching substitution without following variable chains.
fn apply_matching_subst(sigma: &mrs_core::Substitution, term: &Term) -> Term {
    match term {
        Term::Var(v) => match sigma.lookup(*v) {
            Some(t) => t.clone(),
            None => term.clone(),
        },
        Term::App(f, args) => {
            let new_args: Vec<Term> = args
                .iter()
                .map(|a| apply_matching_subst(sigma, a))
                .collect();
            Term::App(*f, new_args)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::clause::{Clause, ClauseIdGen, ClauseSource};
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
    fn demodulate_simple() {
        // Unit: f(a) = b (f(a) > b by weight)
        // Target: p(f(a))
        // Expected: p(b)
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let p = syms.intern("p");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let mut id_gen = ClauseIdGen::new();

        let unit = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::eq(
                Term::app(f, vec![Term::constant(a)]),
                Term::constant(b),
            ))],
            "unit",
        );

        let target = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(
                p,
                vec![Term::app(f, vec![Term::constant(a)])],
            ))],
            "target",
        );

        let mut demod_index = mrs_index::dtree::DTree::new();
        demod_index.insert(&Term::app(f, vec![Term::constant(a)]), (Term::app(f, vec![Term::constant(a)]), Term::constant(b), unit.id));

        let result = demodulate(&target, &demod_index, &mut id_gen);
        assert!(result.is_some());
        let simplified = result.unwrap();
        // Verify demodulation source is recorded
        match &simplified.source {
            ClauseSource::Inference { rule, parents } => {
                assert_eq!(rule, "demodulation");
                assert_eq!(parents[0], target.id);
                assert_eq!(parents[1], unit.id);
            }
            _ => panic!("expected inference source"),
        }
        match &simplified.literals[0].atom {
            Atom::Pred(_, args) => {
                assert_eq!(args[0], Term::constant(b));
            }
            _ => panic!("expected predicate"),
        }
    }

    #[test]
    fn demodulate_no_match() {
        // Unit: f(a) = b
        // Target: p(g(a))  (no f(a) subterm)
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let g = syms.intern("g");
        let p = syms.intern("p");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let mut id_gen = ClauseIdGen::new();

        let unit = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::eq(
                Term::app(f, vec![Term::constant(a)]),
                Term::constant(b),
            ))],
            "unit",
        );

        let target = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(
                p,
                vec![Term::app(g, vec![Term::constant(a)])],
            ))],
            "target",
        );

        let mut demod_index = mrs_index::dtree::DTree::new();
        demod_index.insert(&Term::app(f, vec![Term::constant(a)]), (Term::app(f, vec![Term::constant(a)]), Term::constant(b), unit.id));

        let result = demodulate(&target, &demod_index, &mut id_gen);
        assert!(result.is_none());
    }

    #[test]
    fn demodulate_with_variable_matching() {
        // Unit: f(X) = X (collapse rule, f(X) > X by weight)
        // Target: p(f(a))
        // Expected: p(a) via matching X=a
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let p = syms.intern("p");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();

        let unit = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::eq(
                Term::app(f, vec![Term::var(0)]),
                Term::var(0),
            ))],
            "unit",
        );

        let target = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(
                p,
                vec![Term::app(f, vec![Term::constant(a)])],
            ))],
            "target",
        );

        let mut demod_index = mrs_index::dtree::DTree::new();
        demod_index.insert(&Term::app(f, vec![Term::var(0)]), (Term::app(f, vec![Term::var(0)]), Term::var(0), unit.id));

        let result = demodulate(&target, &demod_index, &mut id_gen);
        assert!(result.is_some());
        let simplified = result.unwrap();
        match &simplified.literals[0].atom {
            Atom::Pred(_, args) => {
                assert_eq!(args[0], Term::constant(a));
            }
            _ => panic!("expected predicate"),
        }
    }

    #[test]
    fn demodulate_non_unit_skipped() {
        // Non-unit clause: a = b ∨ p(c) — not used for demodulation
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let c = syms.intern("c");
        let mut id_gen = ClauseIdGen::new();

        let non_unit = input_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::eq(Term::constant(a), Term::constant(b))),
                Literal::pos(Atom::pred(p, vec![Term::constant(c)])),
            ],
            "non_unit",
        );

        let target = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
            "target",
        );

        let mut demod_index = mrs_index::dtree::DTree::new();
        // non_unit is not inserted because it is not a unit equation

        let result = demodulate(&target, &demod_index, &mut id_gen);
        assert!(result.is_none());
    }
}
