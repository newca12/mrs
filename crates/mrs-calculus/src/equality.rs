//! Equality-specific inference rules.
//!
//! - **Equality Resolution**: From `s ≠ t ∨ C`, derive `σ(C)` where `σ = mgu(s, t)`.
//! - **Equality Factoring**: From `s = t ∨ s' = t' ∨ C`, derive
//!   `σ(s = t ∨ t ≠ t' ∨ C)` where `σ = mgu(s, s')`.

use mrs_core::Atom;
use mrs_core::clause::{Clause, ClauseIdGen, ClauseSource, Literal};
use mrs_unify::unify;

use crate::ordering::{TermComparison, TermOrdering};

/// Equality Resolution: from `s ≠ t ∨ C`, derive `σ(C)` where `σ = mgu(s, t)`.
///
/// For each negative equality literal `¬(s = t)` in the clause,
/// if `s` and `t` unify with mgu `σ`, produce clause `σ(C)` where `C` is
/// the remaining literals.
pub fn equality_resolve(clause: &Clause, id_gen: &mut ClauseIdGen) -> Vec<Clause> {
    let mut results = Vec::new();

    for (i, lit) in clause.literals.iter().enumerate() {
        // Only process negative equality literals: ¬(s = t)
        if lit.is_positive() {
            continue;
        }
        let (s, t) = match &lit.atom {
            Atom::Eq(s, t) => (s, t),
            _ => continue,
        };

        // Try to unify s and t
        let sigma = match unify(s, t) {
            Ok(s) => s,
            Err(_) => continue,
        };

        // Build result: σ applied to all literals except i
        let new_lits: Vec<Literal> = clause
            .literals
            .iter()
            .enumerate()
            .filter(|&(k, _)| k != i)
            .map(|(_, lit)| sigma.apply_literal(lit))
            .collect();

        results.push(Clause::new(
            id_gen.next(),
            new_lits,
            ClauseSource::Inference {
                rule: "equality_resolution".into(),
                parents: vec![clause.id],
            },
        ));
    }

    results
}

/// Equality Factoring: from `s = t ∨ s' = t' ∨ C`,
/// derive `σ(s = t ∨ t ≠ t' ∨ C)` where `σ = mgu(s, s')`.
///
/// For each pair of positive equality literals, if their "larger" sides
/// (under the ordering) unify, produce a factored clause that replaces
/// one equality with a disequality.
pub fn equality_factor(
    clause: &Clause,
    ordering: &TermOrdering,
    id_gen: &mut ClauseIdGen,
) -> Vec<Clause> {
    let mut results = Vec::new();

    for (i, lit1) in clause.literals.iter().enumerate() {
        if !lit1.is_positive() {
            continue;
        }
        let (s, t) = match &lit1.atom {
            Atom::Eq(s, t) => (s, t),
            _ => continue,
        };

        for (j, lit2) in clause.literals.iter().enumerate() {
            if j <= i || !lit2.is_positive() {
                continue;
            }
            let (s2, t2) = match &lit2.atom {
                Atom::Eq(s2, t2) => (s2, t2),
                _ => continue,
            };

            // Try all combinations of sides to unify
            for (left1, right1, left2, right2) in [
                (s, t, s2, t2),
                (s, t, t2, s2),
                (t, s, s2, t2),
                (t, s, t2, s2),
            ] {
                let sigma = match unify(left1, left2) {
                    Ok(s) => s,
                    Err(_) => continue,
                };

                // Check ordering: left1·σ ≻ right1·σ (the equality is oriented)
                let l1s = sigma.apply_term(left1);
                let r1s = sigma.apply_term(right1);
                if ordering.compare(&l1s, &r1s) != TermComparison::Greater {
                    continue;
                }

                // Build: σ(left1 = right1 ∨ right1 ≠ right2 ∨ C)
                // where C = all other literals
                let mut new_lits = Vec::new();

                // Keep the first equality (applied)
                new_lits.push(sigma.apply_literal(lit1));

                // Replace the second equality with a disequality: right1 ≠ right2
                new_lits.push(Literal::neg(Atom::eq(
                    sigma.apply_term(right1),
                    sigma.apply_term(right2),
                )));

                // Add all other literals
                for (k, lit) in clause.literals.iter().enumerate() {
                    if k != i && k != j {
                        new_lits.push(sigma.apply_literal(lit));
                    }
                }

                results.push(Clause::new(
                    id_gen.next(),
                    new_lits,
                    ClauseSource::Inference {
                        rule: "equality_factoring".into(),
                        parents: vec![clause.id],
                    },
                ));
            }
        }
    }

    results
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
    fn equality_resolve_simple() {
        // ¬(a = a) → empty clause
        let mut syms = SymbolTable::new();
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();

        let c = input_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::eq(Term::constant(a), Term::constant(a)))],
            "c1",
        );

        let results = equality_resolve(&c, &mut id_gen);
        assert_eq!(results.len(), 1);
        assert!(
            results[0].is_empty(),
            "should derive empty clause from ¬(a=a)"
        );
    }

    #[test]
    fn equality_resolve_with_remaining() {
        // ¬(X = X) ∨ p(a) → p(a)
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();

        let c = input_clause(
            &mut id_gen,
            vec![
                Literal::neg(Atom::eq(Term::var(0), Term::var(0))),
                Literal::pos(Atom::pred(p, vec![Term::constant(a)])),
            ],
            "c1",
        );

        let results = equality_resolve(&c, &mut id_gen);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].len(), 1);
        assert!(results[0].literals[0].is_positive());
    }

    #[test]
    fn equality_resolve_unifiable() {
        // ¬(f(X) = f(a)) ∨ p(X) → p(a) [with σ = {X=a}]
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let p = syms.intern("p");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();

        let c = input_clause(
            &mut id_gen,
            vec![
                Literal::neg(Atom::eq(
                    Term::app(f, vec![Term::var(0)]),
                    Term::app(f, vec![Term::constant(a)]),
                )),
                Literal::pos(Atom::pred(p, vec![Term::var(0)])),
            ],
            "c1",
        );

        let results = equality_resolve(&c, &mut id_gen);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].len(), 1);
        // The remaining literal should be p(a) after applying σ = {X→a}
        match &results[0].literals[0].atom {
            Atom::Pred(_, args) => {
                assert_eq!(args[0], Term::constant(a));
            }
            _ => panic!("expected predicate literal"),
        }
    }

    #[test]
    fn equality_resolve_no_match() {
        // ¬(a = b) → no resolution possible (a ≠ b don't unify)
        let mut syms = SymbolTable::new();
        let a = syms.intern("a");
        let b = syms.intern("b");
        let mut id_gen = ClauseIdGen::new();

        let c = input_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::eq(Term::constant(a), Term::constant(b)))],
            "c1",
        );

        let results = equality_resolve(&c, &mut id_gen);
        assert!(results.is_empty());
    }

    #[test]
    fn equality_resolve_positive_skipped() {
        // a = a (positive) → no equality resolution
        let mut syms = SymbolTable::new();
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();

        let c = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::eq(Term::constant(a), Term::constant(a)))],
            "c1",
        );

        let results = equality_resolve(&c, &mut id_gen);
        assert!(results.is_empty());
    }

    #[test]
    fn equality_factor_two_equalities() {
        // a = b ∨ a = c → a = b ∨ b ≠ c (if a > b under KBO)
        // But with default KBO, constants have same weight and precedence by SymbolId.
        // a is first interned, so a < b < c. So none of a > b, a > c holds.
        // Let's use f(a) instead to get a weight advantage.

        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let c = syms.intern("c");
        let mut id_gen = ClauseIdGen::new();
        let ordering = TermOrdering::KBO;

        // f(a) = b ∨ f(a) = c
        // Orientation: f(a) > b (weight 2 > 1), f(a) > c (weight 2 > 1)
        // Factor: unify(f(a), f(a)) = identity substitution
        // Result: f(a) = b ∨ b ≠ c
        let clause = input_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::eq(
                    Term::app(f, vec![Term::constant(a)]),
                    Term::constant(b),
                )),
                Literal::pos(Atom::eq(
                    Term::app(f, vec![Term::constant(a)]),
                    Term::constant(c),
                )),
            ],
            "c1",
        );

        let results = equality_factor(&clause, &ordering, &mut id_gen);
        assert!(
            !results.is_empty(),
            "should produce equality factoring result"
        );

        // Check that some result has a negative equality literal
        let has_diseq = results.iter().any(|clause| {
            clause
                .literals
                .iter()
                .any(|lit| lit.is_negative() && matches!(&lit.atom, Atom::Eq(_, _)))
        });
        assert!(has_diseq, "should contain a disequality literal");
    }
}
