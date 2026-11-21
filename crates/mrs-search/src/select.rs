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

/// Parse the bounded selection-expression syntax used by experimental runs.
///
/// Supported forms are `fifo`, `smallest`, `age:N`, `goal:N`, and
/// `ml:N:ALPHA`, or `queue:age=N,weight=N,goal=N,unit=N,horn=N,sos=N`. Queue
/// entries are kept in the supplied order because that order defines the phase
/// of the interleaving. Zero-frequency and duplicate queues are rejected rather
/// than silently changing the requested selector.
pub fn parse_selection_strategy(value: &str) -> Result<SelectionStrategy, String> {
    let value = value.trim();
    let lower = value.to_ascii_lowercase();
    match lower.as_str() {
        "fifo" => return Ok(SelectionStrategy::Fifo),
        "smallest" | "smallest-first" => return Ok(SelectionStrategy::SmallestFirst),
        _ => {}
    }

    if let Some(raw_ratio) = lower.strip_prefix("age:") {
        let ratio = raw_ratio
            .parse::<u32>()
            .ok()
            .filter(|ratio| *ratio > 0)
            .ok_or_else(|| "age: requires a positive integer ratio".to_string())?;
        if ratio == 1 {
            return Ok(SelectionStrategy::Fifo);
        }
        return Ok(SelectionStrategy::AgeWeight(ratio));
    }
    if let Some(raw_ratio) = lower.strip_prefix("goal:") {
        let ratio = raw_ratio
            .parse::<u32>()
            .ok()
            .filter(|ratio| *ratio > 0)
            .ok_or_else(|| "goal: requires a positive integer ratio".to_string())?;
        if ratio == 1 {
            return Ok(SelectionStrategy::Fifo);
        }
        return Ok(SelectionStrategy::GoalDirected(ratio));
    }
    if let Some(raw_ml) = lower.strip_prefix("ml:") {
        let Some((raw_ratio, raw_alpha)) = raw_ml.split_once(':') else {
            return Err("ml: requires a positive ratio and an alpha in 0..=1".into());
        };
        let ratio = raw_ratio
            .parse::<u32>()
            .ok()
            .filter(|ratio| *ratio > 0)
            .ok_or_else(|| "ml: requires a positive integer ratio".to_string())?;
        let alpha = raw_alpha
            .parse::<f32>()
            .ok()
            .filter(|alpha| alpha.is_finite() && (0.0..=1.0).contains(alpha))
            .ok_or_else(|| "ml: alpha must be finite and in 0..=1".to_string())?;
        return Ok(SelectionStrategy::MlGuided { ratio, alpha });
    }

    let Some(raw_queues) = lower.strip_prefix("queue:") else {
        return Err(format!(
            "unknown selection {value:?}; expected fifo, smallest, age:N, goal:N, ml:N:ALPHA, or queue:..."
        ));
    };
    if raw_queues.is_empty() {
        return Err("queue: requires at least one queue=frequency entry".into());
    }

    let mut queues = Vec::new();
    let mut seen = [false; 6];
    let mut total = 0u32;
    for entry in raw_queues.split(',') {
        let Some((name, raw_weight)) = entry.split_once('=') else {
            return Err(format!(
                "invalid queue entry {entry:?}; expected queue=positive_integer"
            ));
        };
        let queue = match name.trim() {
            "age" => QueueType::Age,
            "weight" => QueueType::Weight,
            "goal" => QueueType::Goal,
            "unit" => QueueType::Unit,
            "horn" => QueueType::Horn,
            "sos" => QueueType::Sos,
            _ => return Err(format!("unknown queue {name:?}")),
        };
        let index = match queue {
            QueueType::Age => 0,
            QueueType::Weight => 1,
            QueueType::Goal => 2,
            QueueType::Unit => 3,
            QueueType::Horn => 4,
            QueueType::Sos => 5,
        };
        if seen[index] {
            return Err(format!("queue {name:?} appears more than once"));
        }
        seen[index] = true;
        let weight = raw_weight
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|weight| *weight > 0)
            .ok_or_else(|| format!("queue {name:?} requires a positive frequency"))?;
        total = total
            .checked_add(weight)
            .ok_or_else(|| "sum of queue frequencies exceeds u32::MAX".to_string())?;
        queues.push((queue, weight));
    }
    if total == 0 {
        return Err("queue: requires a positive total frequency".into());
    }

    Ok(SelectionStrategy::MultiQueue(queues))
}

/// Format a selection strategy in the syntax accepted by
/// [`parse_selection_strategy`].
pub fn format_selection_strategy(strategy: &SelectionStrategy) -> String {
    match strategy {
        SelectionStrategy::Fifo => "fifo".into(),
        SelectionStrategy::SmallestFirst => "smallest".into(),
        SelectionStrategy::AgeWeight(ratio) => format!("age:{ratio}"),
        SelectionStrategy::GoalDirected(ratio) => format!("goal:{ratio}"),
        SelectionStrategy::MlGuided { ratio, alpha } => {
            format!("ml:{ratio}:{alpha}")
        }
        SelectionStrategy::MultiQueue(queues) => format!(
            "queue:{}",
            queues
                .iter()
                .map(|(queue, weight)| {
                    let name = match queue {
                        QueueType::Age => "age",
                        QueueType::Weight => "weight",
                        QueueType::Goal => "goal",
                        QueueType::Unit => "unit",
                        QueueType::Horn => "horn",
                        QueueType::Sos => "sos",
                    };
                    format!("{name}={weight}")
                })
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

/// Selects and removes a clause ID from the unprocessed set.
///
/// `sos_depth`: if `< u32::MAX`, the weight-based pop uses SOS restriction
/// (only returns clauses with `distance < sos_depth`).
///
/// Returns `None` **only** when the set is empty. `None` is the given-clause
/// loop's end-of-search signal (`given_clause.rs`: `None => break`), so a
/// strategy arm that can report "no clause here" must fall back to another
/// queue, and `GoalDirected` provably does not need one:
///
/// - `UnprocessedSet::push` inserts every clause into *every* queue, and the
///   priority pops delete lazily, skipping ids that are no longer active. A
///   queue is therefore physically empty only after every clause it held has
///   been returned to the caller or deactivated, so `pop_goal_directed`
///   cannot report "goal queue drained" while a still-active clause is
///   missing from it — the goal queue is drained only when no clause remains
///   at all. Any *other* queue holding entries at that point holds tombstones.
/// - `UnprocessedSet::prune` (the LRS path) rebuilds the goal queue from the
///   active set, so pruning cannot strand a surviving clause outside it.
///
/// The `debug_assert` below pins that invariant: it is free in release builds
/// and is checked on every selection in `cargo test` and debug runs.
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

    let chosen = match strategy {
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
                // No age fallback is needed: the goal queue holds an entry for
                // every active clause, so it cannot drain ahead of the set.
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
                unprocessed.pop_age()
            } else {
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
    };

    debug_assert!(
        chosen.is_some() || unprocessed.is_empty(),
        "select returned None with {} unprocessed clause(s) left under {strategy:?}",
        unprocessed.active_count(),
    );
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::clause::{Clause, ClauseSource};
    use mrs_core::term_bank::TermBank;
    use mrs_core::{Atom, Literal, SymbolTable, Term};

    #[test]
    fn selection_expression_round_trips_supported_policies() {
        for expression in [
            "fifo",
            "smallest",
            "age:5",
            "goal:8",
            "ml:5:0.3",
            "queue:age=1,weight=3,goal=2,unit=1,horn=2,sos=1",
        ] {
            let parsed = parse_selection_strategy(expression).unwrap();
            assert_eq!(format_selection_strategy(&parsed), expression);
        }
    }

    #[test]
    fn selection_expression_rejects_bad_ratios_and_queue_specs() {
        for expression in [
            "age:0",
            "goal:nope",
            "ml:5:NaN",
            "queue:",
            "queue:age=0,weight=1",
            "queue:age=1,age=2",
            "queue:other=1",
            "queue:age=4294967295,weight=1",
        ] {
            assert!(
                parse_selection_strategy(expression).is_err(),
                "accepted invalid selector {expression:?}"
            );
        }
    }

    #[test]
    fn one_in_n_age_or_goal_selection_canonicalizes_to_fifo() {
        assert!(matches!(
            parse_selection_strategy("age:1"),
            Ok(SelectionStrategy::Fifo)
        ));
        assert!(matches!(
            parse_selection_strategy("goal:1"),
            Ok(SelectionStrategy::Fifo)
        ));
    }

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
        unproc.push(&c, bank, w, None, None);
    }

    fn new_set() -> UnprocessedSet {
        UnprocessedSet::new(std::sync::Arc::new(
            mrs_calculus::ordering::SymbolConfig::default(),
        ))
    }

    /// Pushes a clause with an explicit goal distance, so the goal queue orders
    /// differently from the weight queue.
    fn push_clause_with_goal_distance(
        id: u64,
        num_lits: usize,
        goal_distance: Option<u8>,
        bank: &mut TermBank,
        unproc: &mut UnprocessedSet,
    ) -> ClauseId {
        let c = make_id_clause(id, num_lits, bank);
        let w = crate::weight::clause_weight_id(
            &c,
            bank,
            &mrs_calculus::ordering::SymbolConfig::default(),
        );
        unproc.push(&c, bank, w, goal_distance, None);
        c.id
    }

    /// A set whose queues genuinely disagree: units (unit queue only), wide
    /// clauses (weight/age only), and mixed goal distances (goal queue).
    fn mixed_set(bank: &mut TermBank) -> (UnprocessedSet, Vec<ClauseId>) {
        let mut unproc = new_set();
        let mut ids = Vec::new();
        for (id, lits, goal_distance) in [
            (0u64, 1usize, Some(0u8)), // unit, closest to the goal
            (1, 3, None),              // wide pure axiom: heaviest goal weight
            (2, 1, Some(2)),           // unit, mid distance
            (3, 2, Some(1)),           //
            (4, 4, None),              // widest, unit-free, farthest
            (5, 1, Some(3)),           // unit, farthest reachable
        ] {
            ids.push(push_clause_with_goal_distance(
                id,
                lits,
                goal_distance,
                bank,
                &mut unproc,
            ));
        }
        (unproc, ids)
    }

    /// UI-9: the goal queue must never report "drained" while clauses are
    /// still active. The goal queue holds an entry for every active clause, so
    /// draining it removes exactly the clauses that are still selectable —
    /// `select` reports `None` only once the set itself is empty.
    #[test]
    fn goal_directed_signals_end_of_search_only_when_the_set_is_empty() {
        let mut bank = TermBank::new();
        let (mut unproc, ids) = mixed_set(&mut bank);
        let strat = SelectionStrategy::GoalDirected(10);

        let mut selected = Vec::new();
        let mut iteration = 0u64;
        loop {
            match select(&mut unproc, &strat, iteration, u32::MAX) {
                Some(id) => selected.push(id),
                None => {
                    assert!(
                        unproc.is_empty(),
                        "select reported end of search with {} clause(s) still active: {:?}",
                        unproc.active_count(),
                        unproc.iter().collect::<Vec<_>>(),
                    );
                    break;
                }
            }
            iteration += 1;
        }

        // Every clause was handed out exactly once before `None`.
        selected.sort();
        let mut expected = ids;
        expected.sort();
        assert_eq!(selected, expected);
        assert_eq!(iteration, expected.len() as u64);

        // The other queues still hold entries at that point — they are
        // tombstones for the clauses the goal queue handed out, which is why
        // they cannot be popped again.
        assert!(unproc.pop_age().is_none());
        assert!(unproc.pop_weight().is_none());
        assert!(unproc.pop_unit().is_none());
        assert!(unproc.pop_horn().is_none());
        assert!(unproc.pop_sos().is_none());
    }

    /// The same property after LRS pruning, which is the only path that
    /// rebuilds the goal queue rather than lazily draining it.
    #[test]
    fn goal_directed_still_selects_everything_that_survived_pruning() {
        let mut bank = TermBank::new();
        let (mut unproc, _) = mixed_set(&mut bank);
        // Six more clauses, so pruning has something to choose between.
        for id in 6..12u64 {
            push_clause_with_goal_distance(id, (id % 3) as usize + 1, None, &mut bank, &mut unproc);
        }
        assert_eq!(unproc.active_count(), 12);

        // The LRS prune: keep the oldest quarter plus the lightest of the rest.
        let discarded = unproc.prune(4, 25);
        assert_eq!(discarded, 8);
        let survivors: Vec<ClauseId> = unproc.iter().collect();
        assert_eq!(survivors.len(), 4);

        let strat = SelectionStrategy::GoalDirected(10);
        let mut selected = Vec::new();
        for iteration in 0.. {
            match select(&mut unproc, &strat, iteration, u32::MAX) {
                Some(id) => {
                    assert!(
                        survivors.contains(&id),
                        "{id:?} was not in the surviving active set after pruning"
                    );
                    selected.push(id);
                }
                None => {
                    assert!(unproc.is_empty());
                    break;
                }
            }
        }
        selected.sort();
        let mut expected = survivors;
        expected.sort();
        assert_eq!(selected, expected);
    }

    /// A genuinely empty set still terminates normally, on every selector.
    #[test]
    fn empty_set_terminates_for_every_strategy() {
        let strategies = [
            SelectionStrategy::Fifo,
            SelectionStrategy::SmallestFirst,
            SelectionStrategy::AgeWeight(5),
            SelectionStrategy::GoalDirected(10),
            SelectionStrategy::GoalDirected(0),
            SelectionStrategy::MlGuided {
                ratio: 5,
                alpha: 0.3,
            },
            SelectionStrategy::MultiQueue(vec![(QueueType::Goal, 3), (QueueType::Age, 1)]),
            SelectionStrategy::MultiQueue(vec![]),
        ];
        for strategy in strategies {
            for iteration in 0..4u64 {
                let mut unproc = new_set();
                assert!(select(&mut unproc, &strategy, iteration, u32::MAX).is_none());
                assert!(unproc.is_empty());
            }
        }
    }

    /// Every strategy, every queue type: `None` implies empty. Checked over a
    /// mixed set driven to exhaustion through interleaved operations, which is
    /// the state the given-clause loop observes at the `None => break` site.
    #[test]
    fn no_strategy_reports_none_while_clauses_remain() {
        let strategies = [
            SelectionStrategy::Fifo,
            SelectionStrategy::SmallestFirst,
            SelectionStrategy::AgeWeight(5),
            SelectionStrategy::AgeWeight(1),
            SelectionStrategy::GoalDirected(10),
            SelectionStrategy::GoalDirected(2),
            SelectionStrategy::GoalDirected(0),
            SelectionStrategy::MlGuided {
                ratio: 5,
                alpha: 0.3,
            },
            SelectionStrategy::MultiQueue(vec![
                (QueueType::Goal, 3),
                (QueueType::Unit, 1),
                (QueueType::Sos, 2),
                (QueueType::Horn, 1),
                (QueueType::Weight, 1),
                (QueueType::Age, 1),
            ]),
            SelectionStrategy::MultiQueue(vec![(QueueType::Goal, 1)]),
            SelectionStrategy::MultiQueue(vec![(QueueType::Sos, 1)]),
        ];
        for strategy in strategies {
            let mut bank = TermBank::new();
            let (mut unproc, _) = mixed_set(&mut bank);
            for id in 6..14u64 {
                push_clause_with_goal_distance(
                    id,
                    (id % 4) as usize,
                    if id % 2 == 0 { None } else { Some(1) },
                    &mut bank,
                    &mut unproc,
                );
            }
            for iteration in 0..200u64 {
                if iteration % 5 == 4 {
                    // Keep LRS pruning in the mix so the queue rebuilds run.
                    unproc.prune(3, 50);
                }
                if select(&mut unproc, &strategy, iteration, u32::MAX).is_none() {
                    assert!(
                        unproc.is_empty(),
                        "{strategy:?} reported end of search with {} clause(s) left",
                        unproc.active_count(),
                    );
                    break;
                }
            }
            assert!(
                unproc.is_empty(),
                "{strategy:?} did not drain the set within 200 iterations"
            );
        }
    }

    /// `GoalDirected` with SOS enabled exercises the other pop that can report
    /// "nothing available here": `pop_weight_sos` skips goal-ineligible clauses
    /// in a bounded window, which is why `pop_weight` keeps its age fallback.
    #[test]
    fn goal_directed_with_sos_depth_drains_the_set() {
        let mut bank = TermBank::new();
        let (mut unproc, ids) = mixed_set(&mut bank);
        let strat = SelectionStrategy::GoalDirected(10);
        let mut selected = 0;
        for iteration in 0..100u64 {
            match select(&mut unproc, &strat, iteration, 100) {
                Some(_) => selected += 1,
                None => {
                    assert!(unproc.is_empty());
                    break;
                }
            }
        }
        assert_eq!(selected, ids.len());
        assert!(unproc.is_empty());
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
