//! Literal selection for the superposition calculus.
//!
//! In the superposition calculus, inference rules are restricted to operate
//! only on "selected" literals. This reduces the number of generated clauses
//! while maintaining completeness.
//!
//! Strategies:
//! - `All`: No restriction — all literals eligible
//! - `AllNegative`: Select all negative literals (standard)
//! - `MaxNegative`: Select one negative literal with maximal weight (aggressive)

use mrs_core::clause::{Clause, Literal};
use mrs_core::formula::Atom;
use mrs_core::term::Term;
use mrs_core::term_bank::{IdAtom, IdClause, IdLiteral, TermBank, TermId, TermNode};

/// Literal selection strategy.
#[derive(Clone, Debug)]
pub enum LiteralSelection {
    /// All literals are eligible for inference (no restriction).
    All,
    /// Select all negative literals. If none exist, all literals are eligible.
    /// This is the standard completeness-preserving selection for superposition.
    AllNegative,
    /// Select the single negative literal with maximal weight.
    /// If no negative literals exist, all literals are eligible.
    MaxNegative,
    /// Select the single negative literal with maximal weight.
    /// If no negative literals exist, select the single positive literal with maximal weight.
    /// This is very aggressive and sacrifices completeness for speed.
    MaxNegativeOrMaxPositive,
}

/// Returns the indices of selected (eligible) literals in a clause.
///
/// Only selected literals participate in generating inferences (resolution,
/// superposition into target). This restricts the search space while
/// maintaining completeness of the superposition calculus.
pub fn selected_literals(clause: &Clause, strategy: &LiteralSelection) -> Vec<usize> {
    match strategy {
        LiteralSelection::All => (0..clause.len()).collect(),
        LiteralSelection::AllNegative => {
            let neg_indices: Vec<usize> = clause
                .literals
                .iter()
                .enumerate()
                .filter(|(_, lit)| lit.is_negative())
                .map(|(i, _)| i)
                .collect();
            if neg_indices.is_empty() {
                // All-positive clause: all literals eligible
                (0..clause.len()).collect()
            } else {
                neg_indices
            }
        }
        LiteralSelection::MaxNegative => {
            let neg_indices: Vec<usize> = clause
                .literals
                .iter()
                .enumerate()
                .filter(|(_, lit)| lit.is_negative())
                .map(|(i, _)| i)
                .collect();
            if neg_indices.is_empty() {
                (0..clause.len()).collect()
            } else {
                // Select the one negative literal with maximal weight
                let best = neg_indices
                    .iter()
                    .max_by_key(|&&i| literal_weight(&clause.literals[i]))
                    .unwrap();
                vec![*best]
            }
        }
        LiteralSelection::MaxNegativeOrMaxPositive => {
            let neg_indices: Vec<usize> = clause
                .literals
                .iter()
                .enumerate()
                .filter(|(_, lit)| lit.is_negative())
                .map(|(i, _)| i)
                .collect();
            if neg_indices.is_empty() {
                // No negative literals: select all maximal positive literals by weight
                // to be closer to complete (though technically still incomplete without term ordering)
                let max_weight = (0..clause.len())
                    .map(|i| literal_weight(&clause.literals[i]))
                    .max()
                    .unwrap_or(0);
                (0..clause.len())
                    .filter(|&i| literal_weight(&clause.literals[i]) == max_weight)
                    .collect()
            } else {
                // Select the one negative literal with maximal weight
                let best = neg_indices
                    .iter()
                    .max_by_key(|&&i| literal_weight(&clause.literals[i]))
                    .unwrap();
                vec![*best]
            }
        }
    }
}

/// Simple weight function for literal selection: counts symbol occurrences.
fn literal_weight(lit: &Literal) -> u32 {
    match &lit.atom {
        Atom::Pred(_, args) => 1 + args.iter().map(term_weight).sum::<u32>(),
        Atom::Eq(l, r) => term_weight(l) + term_weight(r),
    }
}

fn term_weight(term: &Term) -> u32 {
    match term {
        Term::Var(_) => 1,
        Term::App(_, args) => 1 + args.iter().map(term_weight).sum::<u32>(),
    }
}

// ── IdClause / TermBank variants ────────────────────────────────────────────

/// Returns the indices of selected (eligible) literals for an `IdClause`.
pub fn selected_literals_id(
    clause: &IdClause,
    strategy: &LiteralSelection,
    bank: &TermBank,
) -> Vec<usize> {
    match strategy {
        LiteralSelection::All => (0..clause.literals.len()).collect(),
        LiteralSelection::AllNegative => {
            let neg_indices: Vec<usize> = clause
                .literals
                .iter()
                .enumerate()
                .filter(|(_, lit)| !lit.positive)
                .map(|(i, _)| i)
                .collect();
            if neg_indices.is_empty() {
                (0..clause.literals.len()).collect()
            } else {
                neg_indices
            }
        }
        LiteralSelection::MaxNegative => {
            let neg_indices: Vec<usize> = clause
                .literals
                .iter()
                .enumerate()
                .filter(|(_, lit)| !lit.positive)
                .map(|(i, _)| i)
                .collect();
            if neg_indices.is_empty() {
                (0..clause.literals.len()).collect()
            } else {
                let best = neg_indices
                    .iter()
                    .max_by_key(|&&i| id_literal_weight(&clause.literals[i], bank))
                    .unwrap();
                vec![*best]
            }
        }
        LiteralSelection::MaxNegativeOrMaxPositive => {
            let neg_indices: Vec<usize> = clause
                .literals
                .iter()
                .enumerate()
                .filter(|(_, lit)| !lit.positive)
                .map(|(i, _)| i)
                .collect();
            if neg_indices.is_empty() {
                let max_w = (0..clause.literals.len())
                    .map(|i| id_literal_weight(&clause.literals[i], bank))
                    .max()
                    .unwrap_or(0);
                (0..clause.literals.len())
                    .filter(|&i| id_literal_weight(&clause.literals[i], bank) == max_w)
                    .collect()
            } else {
                let best = neg_indices
                    .iter()
                    .max_by_key(|&&i| id_literal_weight(&clause.literals[i], bank))
                    .unwrap();
                vec![*best]
            }
        }
    }
}

fn id_literal_weight(lit: &IdLiteral, bank: &TermBank) -> u32 {
    match &lit.atom {
        IdAtom::Pred(_, args) => 1 + args.iter().map(|&a| id_term_weight(a, bank)).sum::<u32>(),
        IdAtom::Eq(l, r) => id_term_weight(*l, bank) + id_term_weight(*r, bank),
    }
}

fn id_term_weight(term: TermId, bank: &TermBank) -> u32 {
    match bank.get(term) {
        TermNode::Var(_) => 1,
        TermNode::App(_, args) => 1 + args.iter().map(|&a| id_term_weight(a, bank)).sum::<u32>(),
    }
}

/// Ordered-inference restriction: for a clause that has **no negative literal**
/// and whose literals are **all predicate atoms** (no equality), return the
/// literals whose atom is maximal under the term ordering. This is the standard
/// maximal-literal restriction of ordered resolution/superposition and is
/// refutationally complete.
///
/// For any other clause shape (has a negative literal → selection already
/// restricts; or contains an equality literal → handled conservatively) the
/// input `base_selection` is returned unchanged.
///
/// IMPORTANT: this returns the **full** KBO-maximal set, NOT its intersection
/// with `base_selection`. Intersecting was a soundness bug: under selection
/// strategies that pick a single max-*weight* positive literal (e.g.
/// `MaxNegativeOrMaxPositive`), the intersection could drop the genuinely
/// KBO-maximal literal, leaving inferences unable to fire on it → refutational
/// incompleteness → premature saturation → false `Satisfiable` on unsatisfiable
/// EPR problems (SYN861/862/866). Always returning the maximal set fixes this.
///
/// Predicate atoms `p(t1,…,tn)` are compared by interning the synthetic term
/// `p(t1,…,tn)` and using the term ordering; maximality computed on the
/// unsubstituted clause is preserved under substitution because the ordering is
/// stable (`s > t ⇒ sσ > tσ`).
pub fn restrict_to_maximal_id(
    clause: &IdClause,
    base_selection: &[usize],
    ordering: &crate::ordering::TermOrdering,
    bank: &mut TermBank,
) -> Vec<usize> {
    let n = clause.literals.len();
    if n <= 1 {
        return base_selection.to_vec();
    }
    if clause
        .literals
        .iter()
        .any(|l| !l.positive || !matches!(l.atom, IdAtom::Pred(_, _)))
    {
        return base_selection.to_vec();
    }

    let atom_terms: Vec<TermId> = clause
        .literals
        .iter()
        .map(|l| match &l.atom {
            IdAtom::Pred(sym, args) => bank.intern_app(*sym, args.clone()),
            IdAtom::Eq(_, _) => unreachable!("guarded above"),
        })
        .collect();

    // A literal is maximal iff no other literal's atom is strictly greater.
    let maximal: Vec<usize> = (0..n)
        .filter(|&i| {
            !(0..n).any(|j| {
                j != i
                    && ordering.compare_id(atom_terms[j], atom_terms[i], bank)
                        == crate::ordering::TermComparison::Greater
            })
        })
        .collect();

    if maximal.is_empty() {
        base_selection.to_vec()
    } else {
        maximal
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::clause::{ClauseIdGen, ClauseSource};
    use mrs_core::{Atom, Literal, SymbolTable, Term};

    fn make_clause(id_gen: &mut ClauseIdGen, lits: Vec<Literal>) -> Clause {
        Clause::new(
            id_gen.next(),
            lits,
            ClauseSource::Input {
                name: "test".into(),
                role: "axiom".into(),
            },
        )
    }

    #[test]
    fn all_selects_everything() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();

        let clause = make_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::constant(a)])),
                Literal::neg(Atom::pred(p, vec![Term::constant(a)])),
            ],
        );

        let selected = selected_literals(&clause, &LiteralSelection::All);
        assert_eq!(selected, vec![0, 1]);
    }

    #[test]
    fn all_negative_selects_negatives() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();

        // p(a) | ~q(a) -> select index 1 (the negative literal)
        let clause = make_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::constant(a)])),
                Literal::neg(Atom::pred(q, vec![Term::constant(a)])),
            ],
        );

        let selected = selected_literals(&clause, &LiteralSelection::AllNegative);
        assert_eq!(selected, vec![1]);
    }

    #[test]
    fn all_negative_all_positive_clause() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();

        // p(a) | q(a) -> no negatives, so all are selected
        let clause = make_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::constant(a)])),
                Literal::pos(Atom::pred(q, vec![Term::constant(a)])),
            ],
        );

        let selected = selected_literals(&clause, &LiteralSelection::AllNegative);
        assert_eq!(selected, vec![0, 1]);
    }

    #[test]
    fn all_negative_multiple_negatives() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let r = syms.intern("r");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();

        // ~p(a) | q(a) | ~r(a) -> select indices 0 and 2
        let clause = make_clause(
            &mut id_gen,
            vec![
                Literal::neg(Atom::pred(p, vec![Term::constant(a)])),
                Literal::pos(Atom::pred(q, vec![Term::constant(a)])),
                Literal::neg(Atom::pred(r, vec![Term::constant(a)])),
            ],
        );

        let selected = selected_literals(&clause, &LiteralSelection::AllNegative);
        assert_eq!(selected, vec![0, 2]);
    }

    // --- restrict_to_maximal_id (ordered-inference) regression tests ---

    /// Regression for the EPR false-`Satisfiable` bug (SYN861/862/866):
    /// `restrict_to_maximal_id` must return the FULL KBO-maximal set, never the
    /// intersection with a restrictive `base_selection`. With clause
    /// `p(a) | p(f(a))`, the KBO-maximal literal is `p(f(a))` (index 1). Even if
    /// `base_selection` is the non-maximal `[0]`, the result must be `[1]` —
    /// otherwise resolution can never fire on the maximal literal and the search
    /// saturates prematurely (unsound on unsatisfiable problems).
    #[test]
    fn restrict_to_maximal_returns_full_maximal_set_not_intersection() {
        use mrs_core::term_bank::TermBank;

        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let f = syms.intern("f");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();

        // p(a) | p(f(a))  — all-positive, predicate-only.
        let clause = make_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::constant(a)])),
                Literal::pos(Atom::pred(p, vec![Term::app(f, vec![Term::constant(a)])])),
            ],
        );

        let mut bank = TermBank::new();
        let id_clause = bank.clause_from_legacy(&clause);
        let ordering = crate::ordering::TermOrdering::KBO;

        // KBO: p(f(a)) > p(a), so the maximal literal is index 1.
        // Even with a restrictive (non-maximal) base_selection of [0], the
        // result must be the maximal set [1] — NOT [0], and not empty.
        let restricted = restrict_to_maximal_id(&id_clause, &[0], &ordering, &mut bank);
        assert_eq!(
            restricted,
            vec![1],
            "must return the KBO-maximal literal, not the base_selection"
        );

        // With base_selection = all literals, same maximal set.
        let restricted_all = restrict_to_maximal_id(&id_clause, &[0, 1], &ordering, &mut bank);
        assert_eq!(restricted_all, vec![1]);
    }

    /// `restrict_to_maximal_id` is a no-op (returns `base_selection` unchanged)
    /// for clauses that have a negative literal, contain equality, or are units.
    #[test]
    fn restrict_to_maximal_is_noop_for_non_all_positive_predicate_clauses() {
        use mrs_core::term_bank::TermBank;

        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let f = syms.intern("f");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();
        let ordering = crate::ordering::TermOrdering::KBO;

        // Has a negative literal → unchanged.
        let with_neg = make_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::app(f, vec![Term::constant(a)])])),
                Literal::neg(Atom::pred(q, vec![Term::constant(a)])),
            ],
        );
        let mut bank = TermBank::new();
        let id_with_neg = bank.clause_from_legacy(&with_neg);
        assert_eq!(
            restrict_to_maximal_id(&id_with_neg, &[0, 1], &ordering, &mut bank),
            vec![0, 1]
        );

        // Contains an equality literal → unchanged (conservative).
        let with_eq = make_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::app(f, vec![Term::constant(a)])])),
                Literal::pos(Atom::eq(Term::constant(a), Term::constant(a))),
            ],
        );
        let id_with_eq = bank.clause_from_legacy(&with_eq);
        assert_eq!(
            restrict_to_maximal_id(&id_with_eq, &[0, 1], &ordering, &mut bank),
            vec![0, 1]
        );
    }
}
