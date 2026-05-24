//! Binary resolution: the core inference rule of resolution-based theorem proving.
//!
//! Given two clauses C1 and C2, binary resolution selects one literal L1 from C1
//! and one literal L2 from C2 with complementary polarity (one positive, one
//! negative). If their atoms unify with MGU σ, the resolvent is:
//!
//!   σ(C1 \ {L1}) ∪ σ(C2 \ {L2})
//!
//! Before resolution, C2's variables are renamed to be disjoint from C1's.

use mrs_core::clause::{Clause, ClauseIdGen, ClauseSource};
use mrs_core::term::Term;
use mrs_core::{Atom, Literal};

use crate::rename::{max_var, rename_clause};

/// Converts an atom to a term for unification purposes.
///
/// `Atom::Pred(p, args)` maps to `Term::App(p, args)` since predicates and
/// functions share the same `SymbolId` namespace.
///
/// `Atom::Eq` returns `None` (equality is handled in Phase 4 via superposition).
fn atom_to_term(atom: &Atom) -> Option<Term> {
    match atom {
        Atom::Pred(p, args) => Some(Term::app(*p, args.clone())),
        Atom::Eq(_, _) => None,
    }
}

/// Produces all binary resolvents of two clauses.
///
/// Renames `c2`'s variables to be disjoint from `c1`, then tries every pair
/// of complementary literals (one positive, one negative). When their atoms
/// unify, a resolvent is produced.
///
/// Returns an empty vector if no resolution is possible.
pub fn resolve(c1: &Clause, c2: &Clause, id_gen: &mut ClauseIdGen) -> Vec<Clause> {
    resolve_selected(c1, c2, id_gen, None, None)
}

/// Like [`resolve`], but restricted to selected literals.
///
/// Only literal pairs where `l1` is in `sel1` (if provided) AND `l2` is in
/// `sel2` (if provided) are considered. `None` means all literals are eligible.
pub fn resolve_selected(
    c1: &Clause,
    c2: &Clause,
    id_gen: &mut ClauseIdGen,
    sel1: Option<&[usize]>,
    sel2: Option<&[usize]>,
) -> Vec<Clause> {
    let offset = max_var(c1);
    let c2r = rename_clause(c2, offset);

    let mut resolvents = Vec::new();

    for (i, l1) in c1.literals.iter().enumerate() {
        if let Some(sel) = sel1
            && !sel.contains(&i)
        {
            continue;
        }
        for (j, l2) in c2r.literals.iter().enumerate() {
            if let Some(sel) = sel2
                && !sel.contains(&j)
            {
                continue;
            }
            // Need complementary polarity
            if l1.positive == l2.positive {
                continue;
            }

            // Convert atoms to terms for unification
            let Some(t1) = atom_to_term(&l1.atom) else {
                continue;
            };
            let Some(t2) = atom_to_term(&l2.atom) else {
                continue;
            };

            // Try unification
            if let Ok(mgu) = mrs_unify::unify(&t1, &t2) {
                // Build resolvent: all literals except the resolved pair
                let mut lits: Vec<Literal> = Vec::new();
                for (k, lit) in c1.literals.iter().enumerate() {
                    if k != i {
                        lits.push(mgu.apply_literal(lit));
                    }
                }
                for (k, lit) in c2r.literals.iter().enumerate() {
                    if k != j {
                        lits.push(mgu.apply_literal(lit));
                    }
                }

                resolvents.push(Clause::new(
                    id_gen.next(),
                    lits,
                    ClauseSource::Inference {
                        rule: "resolution".to_string(),
                        parents: vec![c1.id, c2.id],
                    },
                ));
            }
        }
    }

    resolvents
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::SymbolTable;
    use mrs_core::clause::{ClauseId, ClauseSource};
    use mrs_core::display::DisplayWithSymbols;

    fn input_clause(id: u64, lits: Vec<Literal>) -> Clause {
        Clause::new(
            ClauseId(id),
            lits,
            ClauseSource::Input {
                name: "test".into(),
                role: "axiom".into(),
            },
        )
    }

    #[test]
    fn resolve_ground_complement() {
        // {p(a)} resolved with {~p(a)} -> empty clause
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");

        let c1 = input_clause(
            0,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
        );
        let c2 = input_clause(
            1,
            vec![Literal::neg(Atom::pred(p, vec![Term::constant(a)]))],
        );

        let mut id_gen = ClauseIdGen::new();
        id_gen.next();
        id_gen.next(); // skip 0 and 1
        let resolvents = resolve(&c1, &c2, &mut id_gen);

        assert_eq!(resolvents.len(), 1);
        assert!(resolvents[0].is_empty()); // empty clause
    }

    #[test]
    fn resolve_with_unification() {
        // {~human(X), mortal(X)} resolved with {human(socrates)}
        // -> {mortal(socrates)}
        let mut syms = SymbolTable::new();
        let human = syms.intern("human");
        let mortal = syms.intern("mortal");
        let socrates = syms.intern("socrates");

        let c1 = input_clause(
            0,
            vec![
                Literal::neg(Atom::pred(human, vec![Term::var(0)])),
                Literal::pos(Atom::pred(mortal, vec![Term::var(0)])),
            ],
        );
        let c2 = input_clause(
            1,
            vec![Literal::pos(Atom::pred(
                human,
                vec![Term::constant(socrates)],
            ))],
        );

        let mut id_gen = ClauseIdGen::new();
        id_gen.next();
        id_gen.next();
        let resolvents = resolve(&c1, &c2, &mut id_gen);

        assert_eq!(resolvents.len(), 1);
        assert_eq!(resolvents[0].len(), 1);
        let display = format!("{}", resolvents[0].display(&syms));
        assert_eq!(display, "mortal(socrates)");
    }

    #[test]
    fn resolve_no_complement() {
        // {p(a)} and {q(b)} -> no resolvents
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let a = syms.intern("a");
        let b = syms.intern("b");

        let c1 = input_clause(
            0,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
        );
        let c2 = input_clause(
            1,
            vec![Literal::pos(Atom::pred(q, vec![Term::constant(b)]))],
        );

        let mut id_gen = ClauseIdGen::new();
        let resolvents = resolve(&c1, &c2, &mut id_gen);
        assert!(resolvents.is_empty());
    }

    #[test]
    fn resolve_tracks_parents() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");

        let c1 = input_clause(
            5,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
        );
        let c2 = input_clause(
            7,
            vec![Literal::neg(Atom::pred(p, vec![Term::constant(a)]))],
        );

        let mut id_gen = ClauseIdGen::new();
        let resolvents = resolve(&c1, &c2, &mut id_gen);

        if let ClauseSource::Inference { rule, parents } = &resolvents[0].source {
            assert_eq!(rule, "resolution");
            assert_eq!(parents, &vec![ClauseId(5), ClauseId(7)]);
        } else {
            panic!("expected Inference source");
        }
    }

    #[test]
    fn resolve_multiple_resolvents() {
        // {p(X), q(X)} resolved with {~p(a), ~q(b)}
        // -> two resolvents:
        //   resolve on p: {q(a), ~q(b)}
        //   resolve on q: {p(b), ~p(a)}  (with X=b in c1)
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let a = syms.intern("a");
        let b = syms.intern("b");

        let c1 = input_clause(
            0,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(0)])),
                Literal::pos(Atom::pred(q, vec![Term::var(0)])),
            ],
        );
        let c2 = input_clause(
            1,
            vec![
                Literal::neg(Atom::pred(p, vec![Term::constant(a)])),
                Literal::neg(Atom::pred(q, vec![Term::constant(b)])),
            ],
        );

        let mut id_gen = ClauseIdGen::new();
        id_gen.next();
        id_gen.next();
        let resolvents = resolve(&c1, &c2, &mut id_gen);

        assert_eq!(resolvents.len(), 2);
    }

    #[test]
    fn resolve_variable_disjointness() {
        // {p(X)} resolved with {~p(X), q(X)}
        // Both use X=var(0), but c2 should get renamed.
        // Resolvent: {q(X')} where X' is c2's renamed var.
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");

        let c1 = input_clause(0, vec![Literal::pos(Atom::pred(p, vec![Term::var(0)]))]);
        let c2 = input_clause(
            1,
            vec![
                Literal::neg(Atom::pred(p, vec![Term::var(0)])),
                Literal::pos(Atom::pred(q, vec![Term::var(0)])),
            ],
        );

        let mut id_gen = ClauseIdGen::new();
        id_gen.next();
        id_gen.next();
        let resolvents = resolve(&c1, &c2, &mut id_gen);

        assert_eq!(resolvents.len(), 1);
        assert_eq!(resolvents[0].len(), 1); // just q(X')
        // The remaining literal should have q applied to a variable
        assert!(resolvents[0].literals[0].is_positive());
    }
}
