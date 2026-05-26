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
}
