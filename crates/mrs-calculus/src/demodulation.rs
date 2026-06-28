use crate::{HashMap, HashSet};

use mrs_core::clause::{Clause, ClauseId, ClauseIdGen, ClauseSource, Literal};
use mrs_core::formula::Atom;
use mrs_core::subst::Substitution;
use mrs_core::term::Term;
use mrs_unify::matching::match_term;

/// Performs forward demodulation on a clause using the provided index of unit equalities.
///
/// Returns `Some(simplified_clause)` if the clause was rewritten, or `None` if
/// no rewriting occurred. The returned clause has its redundant literals
/// simplified and parents recorded for proof extraction.
pub fn demodulate(
    clause: &Clause,
    demod_index: &mrs_index::dtree::DTree<(Term, Term, ClauseId)>,
    clause_store: &HashMap<ClauseId, Clause>,
    id_gen: &mut ClauseIdGen,
) -> Option<Clause> {
    let mut current_lits = clause.literals.clone();
    let mut changed = false;
    let mut used_unit_ids = Vec::new();

    // Iterate to fixpoint
    loop {
        let mut changed_this_pass = false;
        for lit in &mut current_lits {
            if rewrite_literal(
                lit,
                &clause.avatar,
                demod_index,
                clause_store,
                &mut used_unit_ids,
            ) {
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
        parents.extend_from_slice(&used_unit_ids);

        // Deduplicate the parents list (retaining insertion order)
        let mut unique_parents = Vec::new();
        let mut seen = HashSet::default();
        for p in parents {
            if seen.insert(p) {
                unique_parents.push(p);
            }
        }

        Some(Clause::new_avatar(
            id_gen.next(),
            current_lits,
            ClauseSource::Inference {
                rule: "demodulation".into(),
                parents: unique_parents,
            },
            clause.avatar.clone(),
        ))
    } else {
        None
    }
}

/// Tries to rewrite terms in a literal using the demodulation index.
/// Returns true if any rewrite was performed.
fn rewrite_literal(
    lit: &mut Literal,
    target_avatar: &[u32],
    demod_index: &mrs_index::dtree::DTree<(Term, Term, ClauseId)>,
    clause_store: &HashMap<ClauseId, Clause>,
    used_unit_ids: &mut Vec<ClauseId>,
) -> bool {
    let mut changed = false;
    let new_atom = match &lit.atom {
        Atom::Pred(p, args) => {
            let new_args: Vec<Term> = args
                .iter()
                .map(|arg| {
                    let (new_arg, ch) =
                        rewrite_term(arg, target_avatar, demod_index, clause_store, used_unit_ids);
                    if ch {
                        changed = true;
                    }
                    new_arg
                })
                .collect();
            Atom::Pred(*p, new_args)
        }
        Atom::Eq(l, r) => {
            let (new_l, ch_l) =
                rewrite_term(l, target_avatar, demod_index, clause_store, used_unit_ids);
            let (new_r, ch_r) =
                rewrite_term(r, target_avatar, demod_index, clause_store, used_unit_ids);
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
fn rewrite_term(
    term: &Term,
    target_avatar: &[u32],
    demod_index: &mrs_index::dtree::DTree<(Term, Term, ClauseId)>,
    clause_store: &HashMap<ClauseId, Clause>,
    used_unit_ids: &mut Vec<ClauseId>,
) -> (Term, bool) {
    // Try matching at the current position first
    let rules = demod_index.get_generalizations(term);
    for (from, to, unit_id) in rules {
        if let Some(rule_clause) = clause_store.get(&unit_id) {
            let subset = rule_clause.avatar.iter().all(|a| target_avatar.contains(a));
            if !subset {
                continue;
            }
            if let Ok(sigma) = match_term(&from, term) {
                if !used_unit_ids.contains(&unit_id) {
                    used_unit_ids.push(unit_id);
                }
                return (apply_matching_subst(&sigma, &to), true);
            }
        }
    }

    // Recurse into subterms
    match term {
        Term::Var(_) => (term.clone(), false),
        Term::App(f, args) => {
            let mut changed = false;
            let new_args: Vec<Term> = args
                .iter()
                .map(|arg| {
                    let (new_arg, ch) =
                        rewrite_term(arg, target_avatar, demod_index, clause_store, used_unit_ids);
                    if ch {
                        changed = true;
                    }
                    new_arg
                })
                .collect();
            if changed {
                (Term::App(*f, new_args), true)
            } else {
                (term.clone(), false)
            }
        }
    }
}

fn apply_matching_subst(sigma: &Substitution, term: &Term) -> Term {
    sigma.apply_term(term)
}

use mrs_core::term_bank::{IdAtom, IdClause, IdLiteral, TermBank, TermId};

pub fn demodulate_id(
    clause: &IdClause,
    bank: &mut TermBank,
    demod_index: &mrs_index::stree::STreeId<(TermId, TermId, ClauseId)>,
    clause_store: &HashMap<ClauseId, IdClause>,
    id_gen: &mut ClauseIdGen,
) -> Option<IdClause> {
    let mut current_lits = clause.literals.clone();
    let mut changed = false;
    let mut used_unit_ids = Vec::new();
    let mut passes = 0usize;

    loop {
        // Equational problems can generate cyclic rewrite rules (a→b and b→a).
        // Without a pass limit the rewriter loops indefinitely.  100 passes is
        // a safe upper bound for any real proof step; exceeding it indicates a
        // rewrite cycle and we bail out with whatever simplification we have.
        if passes >= 100 {
            break;
        }
        passes += 1;
        let mut changed_this_pass = false;
        for lit in &mut current_lits {
            if rewrite_literal_id(
                lit,
                &clause.avatar,
                bank,
                demod_index,
                clause_store,
                &mut used_unit_ids,
            ) {
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
        parents.extend_from_slice(&used_unit_ids);

        let mut unique_parents = Vec::new();
        let mut seen = HashSet::default();
        for p in parents {
            if seen.insert(p) {
                unique_parents.push(p);
            }
        }

        Some(IdClause::new_avatar(
            id_gen.next(),
            current_lits,
            ClauseSource::Inference {
                rule: "demodulation".into(),
                parents: unique_parents,
            },
            clause.avatar.clone(),
        ))
    } else {
        None
    }
}

fn rewrite_literal_id(
    lit: &mut IdLiteral,
    target_avatar: &[u32],
    bank: &mut TermBank,
    demod_index: &mrs_index::stree::STreeId<(TermId, TermId, ClauseId)>,
    clause_store: &HashMap<ClauseId, IdClause>,
    used_unit_ids: &mut Vec<ClauseId>,
) -> bool {
    let mut changed = false;
    let new_atom = match &lit.atom {
        IdAtom::Pred(p, args) => {
            let new_args: smallvec::SmallVec<[TermId; 4]> = args
                .iter()
                .map(|arg| {
                    let (new_arg, ch) = rewrite_term_id(
                        *arg,
                        target_avatar,
                        bank,
                        demod_index,
                        clause_store,
                        used_unit_ids,
                    );
                    if ch {
                        changed = true;
                    }
                    new_arg
                })
                .collect();
            IdAtom::Pred(*p, new_args)
        }
        IdAtom::Eq(l, r) => {
            let (new_l, ch_l) = rewrite_term_id(
                *l,
                target_avatar,
                bank,
                demod_index,
                clause_store,
                used_unit_ids,
            );
            let (new_r, ch_r) = rewrite_term_id(
                *r,
                target_avatar,
                bank,
                demod_index,
                clause_store,
                used_unit_ids,
            );
            if ch_l || ch_r {
                changed = true;
            }
            IdAtom::Eq(new_l, new_r)
        }
    };
    if changed {
        lit.atom = new_atom;
    }
    changed
}

fn rewrite_term_id(
    term: TermId,
    target_avatar: &[u32],
    bank: &mut TermBank,
    demod_index: &mrs_index::stree::STreeId<(TermId, TermId, ClauseId)>,
    clause_store: &HashMap<ClauseId, IdClause>,
    used_unit_ids: &mut Vec<ClauseId>,
) -> (TermId, bool) {
    let rules = demod_index.get_generalizations(term, bank);
    for (from, to, unit_id) in rules {
        if let Some(rule_clause) = clause_store.get(&unit_id) {
            let subset = rule_clause.avatar.iter().all(|a| target_avatar.contains(a));
            if !subset {
                continue;
            }

            if let Ok(sigma) = mrs_unify::matching::match_term_id(from, term, bank) {
                if !used_unit_ids.contains(&unit_id) {
                    used_unit_ids.push(unit_id);
                }
                return (apply_matching_subst_id(&sigma, to, bank), true);
            }
        }
    }

    if let mrs_core::term_bank::TermNode::App(sym, args) = bank.get(term).clone() {
        let mut changed = false;
        let mut new_args = Vec::with_capacity(args.len());
        for arg in args {
            let (new_arg, ch) = rewrite_term_id(
                arg,
                target_avatar,
                bank,
                demod_index,
                clause_store,
                used_unit_ids,
            );
            if ch {
                changed = true;
            }
            new_args.push(new_arg);
        }
        if changed {
            return (bank.intern_app(sym, new_args), true);
        }
    }

    (term, false)
}

fn apply_matching_subst_id(
    sigma: &mrs_core::term_bank::IdSubstitution,
    term: TermId,
    bank: &mut TermBank,
) -> TermId {
    match bank.get(term).clone() {
        mrs_core::term_bank::TermNode::Var(v) => match sigma.get(v) {
            Some(t) => t,
            None => term,
        },
        mrs_core::term_bank::TermNode::App(f, args) => {
            let new_args: Vec<TermId> = args
                .iter()
                .map(|&a| apply_matching_subst_id(sigma, a, bank))
                .collect();
            bank.intern_app(f, new_args)
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

        let mut clause_store = HashMap::default();
        clause_store.insert(unit.id, unit.clone());

        let mut demod_index = mrs_index::dtree::DTree::new();
        demod_index.insert(
            &Term::app(f, vec![Term::constant(a)]),
            (
                Term::app(f, vec![Term::constant(a)]),
                Term::constant(b),
                unit.id,
            ),
        );

        let result = demodulate(&target, &demod_index, &clause_store, &mut id_gen);
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

        let mut clause_store = HashMap::default();
        clause_store.insert(unit.id, unit.clone());

        let mut demod_index = mrs_index::dtree::DTree::new();
        demod_index.insert(
            &Term::app(f, vec![Term::constant(a)]),
            (
                Term::app(f, vec![Term::constant(a)]),
                Term::constant(b),
                unit.id,
            ),
        );

        let result = demodulate(&target, &demod_index, &clause_store, &mut id_gen);
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

        let mut clause_store = HashMap::default();
        clause_store.insert(unit.id, unit.clone());

        let mut demod_index = mrs_index::dtree::DTree::new();
        demod_index.insert(
            &Term::app(f, vec![Term::var(0)]),
            (Term::app(f, vec![Term::var(0)]), Term::var(0), unit.id),
        );

        let result = demodulate(&target, &demod_index, &clause_store, &mut id_gen);
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

        let _non_unit = input_clause(
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

        let clause_store = HashMap::default();
        let demod_index = mrs_index::dtree::DTree::new();
        // non_unit is not inserted because it is not a unit equation

        let result = demodulate(&target, &demod_index, &clause_store, &mut id_gen);
        assert!(result.is_none());
    }
}
