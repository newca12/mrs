//! Clause selection strategies.
//!
//! Determines which clause to process next from the unprocessed set.
//! Different strategies explore the search space differently:
//!
//! - **FIFO**: Breadth-first, complete, but may be slow
//! - **SmallestFirst**: Prefer shorter clauses, often finds proofs faster
//! - **AgeWeight**: Alternates between FIFO and smallest-first

use crate::unprocessed::UnprocessedSet;
use mrs_core::clause::ClauseId;

/// Individual priority queue types available for multi-queue selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum QueueType {
    /// Age (FIFO) - oldest clause first.
    Age,
    /// Lightest clause by symbol weight.
    Weight,
    /// Goal-directed: distance-penalized weight.
    Goal,
    /// Unit clauses: 1-literal clauses (ordered by weight).
    Unit,
    /// Horn clauses: at most 1 positive literal (ordered by weight).
    Horn,
    /// Set-of-Support: derived from conjecture (distance < 100, ordered by weight).
    Sos,
}

/// A clause selection strategy.
#[derive(Clone, Debug)]
pub enum SelectionStrategy {
    /// First-in, first-out (breadth-first search).
    Fifo,
    /// Select the clause with the lowest weight (sum of symbol occurrences).
    SmallestFirst,
    /// Alternate: every `ratio`-th pick is by age (FIFO), rest by weight.
    AgeWeight(u32),
    /// Alternate: every `ratio`-th pick is by age (FIFO), rest by distance-penalized weight.
    GoalDirected(u32),
    /// Alternate: every `ratio`-th pick is by age (FIFO), rest by ML-guided score blended with weight using `alpha`.
    MlGuided { ratio: u32, alpha: f32 },
    /// Multi-queue given-clause selection interleaving multiple priority queues with specified frequencies.
    MultiQueue(Vec<(QueueType, u32)>),
}

/// Selects and removes a clause ID from the unprocessed set.
///
/// `sos_depth`: if `< u32::MAX`, the weight-based pop uses SOS restriction
/// (only returns clauses with `distance < sos_depth`).
///
/// Returns `None` if the set is empty.
pub fn select(
    unprocessed: &mut UnprocessedSet,
    strategy: &SelectionStrategy,
    iteration: u64,
    sos_depth: u32,
) -> Option<ClauseId> {
    if unprocessed.is_empty() {
        return None;
    }

    let pop_weight = |u: &mut UnprocessedSet| {
        if sos_depth < u32::MAX {
            u.pop_weight_sos(sos_depth).or_else(|| u.pop_age()) // age fallback when no SOS clause is ready
        } else {
            u.pop_weight()
        }
    };

    match strategy {
        SelectionStrategy::Fifo => unprocessed.pop_age(),

        SelectionStrategy::SmallestFirst => pop_weight(unprocessed),

        SelectionStrategy::AgeWeight(ratio) => {
            if *ratio == 0 || iteration.is_multiple_of(*ratio as u64) {
                // Age pick: FIFO
                unprocessed.pop_age()
            } else {
                // Weight pick: lightest clause (SOS-restricted if enabled)
                pop_weight(unprocessed)
            }
        }

        SelectionStrategy::GoalDirected(ratio) => {
            if *ratio == 0 || iteration.is_multiple_of(*ratio as u64) {
                unprocessed.pop_age()
            } else {
                unprocessed.pop_goal_directed()
            }
        }

        SelectionStrategy::MlGuided { ratio, .. } => {
            if *ratio == 0 || iteration.is_multiple_of(*ratio as u64) {
                unprocessed.pop_age()
            } else {
                #[cfg(feature = "ml-guidance")]
                {
                    unprocessed.pop_ml()
                }
                #[cfg(not(feature = "ml-guidance"))]
                {
                    // Without the ml-guidance feature there is no ML queue;
                    // degrade gracefully to plain weight-based selection.
                    pop_weight(unprocessed)
                }
            }
        }

        SelectionStrategy::MultiQueue(queues) => {
            let total_weight: u32 = queues.iter().map(|(_, w)| *w).sum();
            if total_weight == 0 {
                return unprocessed.pop_age();
            }
            let mut step = (iteration % (total_weight as u64)) as u32;
            let mut chosen = QueueType::Weight;
            for (q_type, w) in queues {
                if step < *w {
                    chosen = *q_type;
                    break;
                }
                step -= *w;
            }

            match chosen {
                QueueType::Age => unprocessed.pop_age().or_else(|| pop_weight(unprocessed)),
                QueueType::Weight => pop_weight(unprocessed).or_else(|| unprocessed.pop_age()),
                QueueType::Goal => unprocessed
                    .pop_goal_directed()
                    .or_else(|| pop_weight(unprocessed))
                    .or_else(|| unprocessed.pop_age()),
                QueueType::Unit => unprocessed
                    .pop_unit()
                    .or_else(|| pop_weight(unprocessed))
                    .or_else(|| unprocessed.pop_age()),
                QueueType::Horn => unprocessed
                    .pop_horn()
                    .or_else(|| pop_weight(unprocessed))
                    .or_else(|| unprocessed.pop_age()),
                QueueType::Sos => unprocessed
                    .pop_sos()
                    .or_else(|| pop_weight(unprocessed))
                    .or_else(|| unprocessed.pop_age()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::clause::{Clause, ClauseSource};
    use mrs_core::term_bank::TermBank;
    use mrs_core::{Atom, Literal, SymbolTable, Term};

    fn make_id_clause(
        id: u64,
        num_lits: usize,
        bank: &mut TermBank,
    ) -> mrs_core::term_bank::IdClause {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let lits: Vec<Literal> = (0..num_lits)
            .map(|i| Literal::pos(Atom::pred(p, vec![Term::var(i as u32)])))
            .collect();
        let clause = Clause::new(
            ClauseId(id),
            lits,
            ClauseSource::Input {
                name: "test".into(),
                role: "axiom".into(),
            },
        );
        bank.clause_from_legacy(&clause)
    }

    fn push_clause(id: u64, num_lits: usize, bank: &mut TermBank, unproc: &mut UnprocessedSet) {
        let c = make_id_clause(id, num_lits, bank);
        let w = crate::weight::clause_weight_id(
            &c,
            bank,
            &mrs_calculus::ordering::SymbolConfig::default(),
        );
        unproc.push(&c, bank, w, None);
    }

    #[test]
    fn fifo_returns_oldest() {
        let mut bank = TermBank::new();
        let mut unproc = UnprocessedSet::new(std::sync::Arc::new(
            mrs_calculus::ordering::SymbolConfig::default(),
        ));
        push_clause(0, 3, &mut bank, &mut unproc);
        push_clause(1, 1, &mut bank, &mut unproc);
        push_clause(2, 2, &mut bank, &mut unproc);

        let selected = select(&mut unproc, &SelectionStrategy::Fifo, 0, u32::MAX).unwrap();
        assert_eq!(selected, ClauseId(0));
    }

    #[test]
    fn smallest_returns_shortest() {
        let mut bank = TermBank::new();
        let mut unproc = UnprocessedSet::new(std::sync::Arc::new(
            mrs_calculus::ordering::SymbolConfig::default(),
        ));
        push_clause(0, 3, &mut bank, &mut unproc);
        push_clause(1, 1, &mut bank, &mut unproc);
        push_clause(2, 2, &mut bank, &mut unproc);

        let selected = select(&mut unproc, &SelectionStrategy::SmallestFirst, 0, u32::MAX).unwrap();
        assert_eq!(selected, ClauseId(1));
    }

    #[test]
    fn age_weight_alternates() {
        let mut bank = TermBank::new();
        let mut unproc = UnprocessedSet::new(std::sync::Arc::new(
            mrs_calculus::ordering::SymbolConfig::default(),
        ));
        push_clause(0, 3, &mut bank, &mut unproc); // oldest, largest
        push_clause(1, 1, &mut bank, &mut unproc); // smallest

        // ratio=2: iteration 0 -> age (FIFO), iteration 1 -> weight
        let s0 = select(&mut unproc, &SelectionStrategy::AgeWeight(2), 0, u32::MAX).unwrap();
        assert_eq!(s0, ClauseId(0)); // FIFO pick
        let s1 = select(&mut unproc, &SelectionStrategy::AgeWeight(2), 1, u32::MAX).unwrap();
        assert_eq!(s1, ClauseId(1)); // smallest pick (only one left)
    }

    #[test]
    fn empty_returns_none() {
        let mut unproc = UnprocessedSet::new(std::sync::Arc::new(
            mrs_calculus::ordering::SymbolConfig::default(),
        ));
        assert!(select(&mut unproc, &SelectionStrategy::Fifo, 0, u32::MAX).is_none());
    }

    #[test]
    fn multi_queue_interleaves_and_falls_back() {
        let mut bank = TermBank::new();
        let mut unproc = UnprocessedSet::new(std::sync::Arc::new(
            mrs_calculus::ordering::SymbolConfig::default(),
        ));
        // Clause 0: 3 literals (weight high, not unit)
        push_clause(0, 3, &mut bank, &mut unproc);
        // Clause 1: 1 literal (unit, lightest)
        push_clause(1, 1, &mut bank, &mut unproc);
        // Clause 2: 2 literals (medium)
        push_clause(2, 2, &mut bank, &mut unproc);

        // Schedule: 1 Age, 2 Unit -> total 3.
        // iter 0: Age -> picks Clause 0 (oldest)
        // iter 1: Unit -> picks Clause 1 (unit)
        // iter 2: Unit -> unit queue empty -> falls back to Weight -> picks Clause 2
        let strat = SelectionStrategy::MultiQueue(vec![(QueueType::Age, 1), (QueueType::Unit, 2)]);

        let s0 = select(&mut unproc, &strat, 0, u32::MAX).unwrap();
        assert_eq!(s0, ClauseId(0));

        let s1 = select(&mut unproc, &strat, 1, u32::MAX).unwrap();
        assert_eq!(s1, ClauseId(1));

        let s2 = select(&mut unproc, &strat, 2, u32::MAX).unwrap();
        assert_eq!(s2, ClauseId(2));

        assert!(select(&mut unproc, &strat, 3, u32::MAX).is_none());
    }
}
