//! Clause selection strategies.
//!
//! Determines which clause to process next from the unprocessed set.
//! Different strategies explore the search space differently:
//!
//! - **FIFO**: Breadth-first, complete, but may be slow
//! - **SmallestFirst**: Prefer shorter clauses, often finds proofs faster
//! - **AgeWeight**: Alternates between FIFO and smallest-first

use std::collections::VecDeque;

use mrs_core::clause::Clause;

use crate::weight::clause_weight;

/// A clause selection strategy.
#[derive(Clone, Debug)]
pub enum SelectionStrategy {
    /// First-in, first-out (breadth-first search).
    Fifo,
    /// Select the clause with the lowest weight (sum of symbol occurrences).
    SmallestFirst,
    /// Alternate: every `ratio`-th pick is by age (FIFO), rest by weight.
    AgeWeight(u32),
}

/// Selects and removes a clause from the unprocessed set.
///
/// Returns `None` if the set is empty.
pub fn select(
    unprocessed: &mut VecDeque<Clause>,
    strategy: &SelectionStrategy,
    iteration: u64,
) -> Option<Clause> {
    if unprocessed.is_empty() {
        return None;
    }

    match strategy {
        SelectionStrategy::Fifo => unprocessed.pop_front(),

        SelectionStrategy::SmallestFirst => {
            let min_idx = unprocessed
                .iter()
                .enumerate()
                .min_by_key(|(_, c)| clause_weight(c))
                .map(|(i, _)| i)
                .unwrap();
            unprocessed.remove(min_idx)
        }

        SelectionStrategy::AgeWeight(ratio) => {
            if *ratio == 0 || iteration.is_multiple_of(*ratio as u64) {
                // Age pick: FIFO
                unprocessed.pop_front()
            } else {
                // Weight pick: lightest clause
                let min_idx = unprocessed
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, c)| clause_weight(c))
                    .map(|(i, _)| i)
                    .unwrap();
                unprocessed.remove(min_idx)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::clause::{ClauseId, ClauseSource};
    use mrs_core::{Atom, Literal, SymbolTable, Term};

    fn make_clause(id: u64, num_lits: usize) -> Clause {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let lits = (0..num_lits)
            .map(|i| Literal::pos(Atom::pred(p, vec![Term::var(i as u32)])))
            .collect();
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
    fn fifo_returns_oldest() {
        let mut unproc = VecDeque::from(vec![
            make_clause(0, 3),
            make_clause(1, 1),
            make_clause(2, 2),
        ]);
        let selected = select(&mut unproc, &SelectionStrategy::Fifo, 0).unwrap();
        assert_eq!(selected.id, ClauseId(0));
    }

    #[test]
    fn smallest_returns_shortest() {
        let mut unproc = VecDeque::from(vec![
            make_clause(0, 3),
            make_clause(1, 1),
            make_clause(2, 2),
        ]);
        let selected = select(&mut unproc, &SelectionStrategy::SmallestFirst, 0).unwrap();
        assert_eq!(selected.id, ClauseId(1));
    }

    #[test]
    fn age_weight_alternates() {
        let mut unproc = VecDeque::from(vec![
            make_clause(0, 3), // oldest, largest
            make_clause(1, 1), // smallest
        ]);
        // ratio=2: iteration 0 -> age (FIFO), iteration 1 -> weight
        let s0 = select(&mut unproc, &SelectionStrategy::AgeWeight(2), 0).unwrap();
        assert_eq!(s0.id, ClauseId(0)); // FIFO pick
        let s1 = select(&mut unproc, &SelectionStrategy::AgeWeight(2), 1).unwrap();
        assert_eq!(s1.id, ClauseId(1)); // smallest pick (only one left)
    }

    #[test]
    fn empty_returns_none() {
        let mut unproc = VecDeque::new();
        assert!(select(&mut unproc, &SelectionStrategy::Fifo, 0).is_none());
    }
}
