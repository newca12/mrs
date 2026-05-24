//! Strategy scheduling: run multiple search strategies in sequence.
//!
//! Different clause selection strategies explore the search space differently.
//! Strategy scheduling tries multiple strategies with time slices, increasing
//! the chance of finding a proof within the overall time limit.
//!
//! This is inspired by systems like Vampire and E, which use strategy
//! portfolios for CASC competition.

use std::time::Duration;

use mrs_core::clause::{Clause, ClauseIdGen};

use crate::given_clause::search;
use crate::state::SearchState;
use crate::{LiteralSelection, SearchConfig, SearchResult, SelectionStrategy, TermOrdering};

/// A strategy schedule: a sequence of (config, time_slice) pairs.
///
/// Each strategy is tried for its allocated time. If a proof is found,
/// it is returned immediately. If all strategies are exhausted without
/// success, the result from the last strategy is returned.
pub struct StrategySchedule {
    pub strategies: Vec<(SearchConfig, Duration)>,
}

impl StrategySchedule {
    /// Creates the default strategy schedule.
    ///
    /// 1. AgeWeight(5) + AllNegative + KBO — balanced exploration (15%)
    /// 2. AgeWeight(10) + AllNegative + KBO — prefer lighter clauses (10%)
    /// 3. SmallestFirst + AllNegative + KBO — pure best-first (10%)
    /// 4. AgeWeight(5) + MaxNegative + KBO — aggressive selection (10%)
    /// 5. AgeWeight(5) + All + KBO — unrestricted literal selection (10%)
    /// 6. Fifo + AllNegative + KBO — breadth-first saturation (10%)
    /// 7. AgeWeight(5) + AllNegative + LPO — LPO balanced exploration (15%)
    /// 8. AgeWeight(10) + AllNegative + LPO — LPO prefer lighter (10%)
    /// 9. SmallestFirst + AllNegative + LPO — LPO best-first (10%)
    pub fn default_schedule(total_time: Duration) -> Self {
        let ms = total_time.as_millis() as u64;
        let t1 = Duration::from_millis(ms * 15 / 100);
        let t2 = Duration::from_millis(ms * 10 / 100);
        let t3 = Duration::from_millis(ms * 10 / 100);
        let t4 = Duration::from_millis(ms * 10 / 100);
        let t5 = Duration::from_millis(ms * 10 / 100);
        let t6 = Duration::from_millis(ms * 10 / 100);
        let t7 = Duration::from_millis(ms * 15 / 100);
        let t8 = Duration::from_millis(ms * 10 / 100);
        let t9 = total_time
            .saturating_sub(t1)
            .saturating_sub(t2)
            .saturating_sub(t3)
            .saturating_sub(t4)
            .saturating_sub(t5)
            .saturating_sub(t6)
            .saturating_sub(t7)
            .saturating_sub(t8);

        StrategySchedule {
            strategies: vec![
                // KBO strategies
                (
                    SearchConfig {
                        time_limit: t1,
                        max_clauses: 50_000,
                        selection: SelectionStrategy::AgeWeight(5),
                        literal_selection: LiteralSelection::AllNegative,
                        ordering: TermOrdering::KBO,
                    },
                    t1,
                ),
                (
                    SearchConfig {
                        time_limit: t2,
                        max_clauses: 50_000,
                        selection: SelectionStrategy::AgeWeight(10),
                        literal_selection: LiteralSelection::AllNegative,
                        ordering: TermOrdering::KBO,
                    },
                    t2,
                ),
                (
                    SearchConfig {
                        time_limit: t3,
                        max_clauses: 50_000,
                        selection: SelectionStrategy::SmallestFirst,
                        literal_selection: LiteralSelection::AllNegative,
                        ordering: TermOrdering::KBO,
                    },
                    t3,
                ),
                (
                    SearchConfig {
                        time_limit: t4,
                        max_clauses: 50_000,
                        selection: SelectionStrategy::AgeWeight(5),
                        literal_selection: LiteralSelection::MaxNegative,
                        ordering: TermOrdering::KBO,
                    },
                    t4,
                ),
                (
                    SearchConfig {
                        time_limit: t5,
                        max_clauses: 50_000,
                        selection: SelectionStrategy::AgeWeight(5),
                        literal_selection: LiteralSelection::All,
                        ordering: TermOrdering::KBO,
                    },
                    t5,
                ),
                (
                    SearchConfig {
                        time_limit: t6,
                        max_clauses: 50_000,
                        selection: SelectionStrategy::Fifo,
                        literal_selection: LiteralSelection::AllNegative,
                        ordering: TermOrdering::KBO,
                    },
                    t6,
                ),
                // LPO strategies
                (
                    SearchConfig {
                        time_limit: t7,
                        max_clauses: 50_000,
                        selection: SelectionStrategy::AgeWeight(5),
                        literal_selection: LiteralSelection::AllNegative,
                        ordering: TermOrdering::LPO,
                    },
                    t7,
                ),
                (
                    SearchConfig {
                        time_limit: t8,
                        max_clauses: 50_000,
                        selection: SelectionStrategy::AgeWeight(10),
                        literal_selection: LiteralSelection::AllNegative,
                        ordering: TermOrdering::LPO,
                    },
                    t8,
                ),
                (
                    SearchConfig {
                        time_limit: t9,
                        max_clauses: 50_000,
                        selection: SelectionStrategy::SmallestFirst,
                        literal_selection: LiteralSelection::AllNegative,
                        ordering: TermOrdering::LPO,
                    },
                    t9,
                ),
            ],
        }
    }
}

/// Runs a strategy schedule on a set of input clauses.
///
/// Tries each strategy in sequence. Returns `Refutation` as soon as one is found.
/// Each strategy starts fresh with its own search state (no carryover from previous
/// attempts), since different strategies may benefit from different exploration orderings.
pub fn run_schedule(
    clauses: &[Clause],
    id_gen: ClauseIdGen,
    schedule: &StrategySchedule,
) -> (SearchResult, SearchState) {
    let mut last_result = SearchResult::Saturated;
    let mut last_state = SearchState::new(Vec::new(), ClauseIdGen::new());

    for (config, _) in &schedule.strategies {
        let mut state = SearchState::new(clauses.to_vec(), id_gen.clone());
        let result = search(&mut state, config);

        match &result {
            SearchResult::Refutation(_) => {
                return (result, state);
            }
            _ => {
                last_result = result;
                last_state = state;
            }
        }
    }

    (last_result, last_state)
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
    fn schedule_finds_refutation() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();

        let c1 = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
            "ax1",
        );
        let c2 = input_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::pred(p, vec![Term::constant(a)]))],
            "ax2",
        );

        let schedule = StrategySchedule::default_schedule(Duration::from_secs(5));
        let (result, _) = run_schedule(&[c1, c2], id_gen, &schedule);
        assert!(matches!(result, SearchResult::Refutation(_)));
    }

    #[test]
    fn schedule_saturates() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let mut id_gen = ClauseIdGen::new();

        let c1 = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
            "ax1",
        );
        let c2 = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(q, vec![Term::constant(b)]))],
            "ax2",
        );

        let schedule = StrategySchedule::default_schedule(Duration::from_secs(5));
        let (result, _) = run_schedule(&[c1, c2], id_gen, &schedule);
        assert!(matches!(result, SearchResult::Saturated));
    }
}
