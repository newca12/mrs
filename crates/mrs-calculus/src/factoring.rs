//! Factoring: merge unifiable same-polarity literals within a clause.
//!
//! Given a clause with two literals L1 and L2 of the same polarity,
//! if their atoms unify with MGU σ, the factor is:
//!
//!   σ(C \ {L2})
//!
//! Factoring is needed for completeness of binary resolution.

use mrs_core::clause::{Clause, ClauseIdGen, ClauseSource};
use mrs_core::term::Term;
use mrs_core::{Atom, Literal};

/// Converts an atom to a term for unification purposes.
fn atom_to_term(atom: &Atom) -> Option<Term> {
    match atom {
        Atom::Pred(p, args) => Some(Term::app(*p, args.clone())),
        Atom::Eq(_, _) => None,
    }
}

/// Produces all binary factors of a clause.
///
/// For each pair of same-polarity literals whose atoms unify,
/// produces a factor with one literal merged and the MGU applied.
pub fn factor(clause: &Clause, id_gen: &mut ClauseIdGen) -> Vec<Clause> {
    let mut factors = Vec::new();

    for i in 0..clause.literals.len() {
        for j in (i + 1)..clause.literals.len() {
            let l1 = &clause.literals[i];
            let l2 = &clause.literals[j];

            // Same polarity required
            if l1.positive != l2.positive {
                continue;
            }

            let Some(t1) = atom_to_term(&l1.atom) else {
                continue;
            };
            let Some(t2) = atom_to_term(&l2.atom) else {
                continue;
            };

            if let Ok(mgu) = mrs_unify::unify(&t1, &t2) {
                // Remove literal j (keep i), apply MGU to all remaining
                let mut lits: Vec<Literal> = Vec::new();
                for (k, lit) in clause.literals.iter().enumerate() {
                    if k != j {
                        lits.push(mgu.apply_literal(lit));
                    }
                }

                factors.push(Clause::new_avatar(
                    id_gen.next(),
                    lits,
                    ClauseSource::Inference {
                        rule: "factoring".into(),
                        parents: vec![clause.id],
                    },
                    clause.avatar.clone(),
                ));
            }
        }
    }

    factors
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::SymbolTable;
    use mrs_core::clause::{ClauseId, ClauseSource};

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
    fn factor_unifiable_same_polarity() {
        // {p(X), p(a), q(Y)} -> {p(a), q(Y)} with X=a
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let a = syms.intern("a");

        let c = input_clause(
            0,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(0)])),
                Literal::pos(Atom::pred(p, vec![Term::constant(a)])),
                Literal::pos(Atom::pred(q, vec![Term::var(1)])),
            ],
        );

        let mut id_gen = ClauseIdGen::new();
        id_gen.next(); // skip 0
        let factors = factor(&c, &mut id_gen);

        assert_eq!(factors.len(), 1);
        assert_eq!(factors[0].len(), 2); // p(a) and q(Y)
    }

    #[test]
    fn factor_different_polarity_no_result() {
        // {p(X), ~p(a)} -> no factors (different polarity)
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");

        let c = input_clause(
            0,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(0)])),
                Literal::neg(Atom::pred(p, vec![Term::constant(a)])),
            ],
        );

        let mut id_gen = ClauseIdGen::new();
        let factors = factor(&c, &mut id_gen);
        assert!(factors.is_empty());
    }

    #[test]
    fn factor_different_predicates_no_result() {
        // {p(a), q(b)} -> no factors (different predicates)
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let a = syms.intern("a");
        let b = syms.intern("b");

        let c = input_clause(
            0,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::constant(a)])),
                Literal::pos(Atom::pred(q, vec![Term::constant(b)])),
            ],
        );

        let mut id_gen = ClauseIdGen::new();
        let factors = factor(&c, &mut id_gen);
        assert!(factors.is_empty());
    }

    #[test]
    fn factor_tracks_parent() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");

        let c = input_clause(
            5,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(0)])),
                Literal::pos(Atom::pred(p, vec![Term::constant(a)])),
            ],
        );

        let mut id_gen = ClauseIdGen::new();
        let factors = factor(&c, &mut id_gen);

        if let ClauseSource::Inference { rule, parents } = &factors[0].source {
            assert_eq!(rule, "factoring");
            assert_eq!(parents, &vec![ClauseId(5)]);
        } else {
            panic!("expected Inference source");
        }
    }

    #[test]
    fn factor_single_literal_no_result() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");

        let c = input_clause(
            0,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
        );

        let mut id_gen = ClauseIdGen::new();
        let factors = factor(&c, &mut id_gen);
        assert!(factors.is_empty());
    }
}
