//! Strategy scheduling: run multiple search strategies in sequence.
//!
//! Different clause selection strategies explore the search space differently.
//! Strategy scheduling tries multiple strategies with time slices, increasing
//! the chance of finding a proof within the overall time limit.
//!
//! This is inspired by systems like Vampire and E, which use strategy
//! portfolios for CASC competition.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use mrs_calculus::ordering::SymbolConfig;
use mrs_core::SymbolTable;
use mrs_core::clause::{Clause, ClauseIdGen};
use mrs_core::symbol::SymbolId;
use mrs_core::term::Term;
use mrs_proof::extract::extract_proof;
use mrs_proof::tstp::format_tstp;

use crate::given_clause::search;
use crate::instgen::{is_epr, preprocess_epr};
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
                        selection: SelectionStrategy::AgeWeight(5),
                        literal_selection: LiteralSelection::AllNegative,
                        ordering: TermOrdering::KBO,
                        ..SearchConfig::default()
                    },
                    t1,
                ),
                (
                    SearchConfig {
                        time_limit: t2,
                        selection: SelectionStrategy::GoalDirected(5),
                        literal_selection: LiteralSelection::AllNegative,
                        ordering: TermOrdering::KBO,
                        ..SearchConfig::default()
                    },
                    t2,
                ),
                (
                    SearchConfig {
                        time_limit: t3,
                        selection: SelectionStrategy::SmallestFirst,
                        literal_selection: LiteralSelection::AllNegative,
                        ordering: TermOrdering::KBO,
                        ..SearchConfig::default()
                    },
                    t3,
                ),
                (
                    SearchConfig {
                        time_limit: t4,
                        selection: SelectionStrategy::AgeWeight(5),
                        literal_selection: LiteralSelection::MaxNegativeOrMaxPositive,
                        ordering: TermOrdering::KBO,
                        ..SearchConfig::default()
                    },
                    t4,
                ),
                (
                    SearchConfig {
                        time_limit: t5,
                        selection: SelectionStrategy::AgeWeight(5),
                        literal_selection: LiteralSelection::All,
                        ordering: TermOrdering::KBO,
                        ..SearchConfig::default()
                    },
                    t5,
                ),
                (
                    SearchConfig {
                        time_limit: t6,
                        selection: SelectionStrategy::Fifo,
                        literal_selection: LiteralSelection::AllNegative,
                        ordering: TermOrdering::KBO,
                        ..SearchConfig::default()
                    },
                    t6,
                ),
                // LPO strategies
                (
                    SearchConfig {
                        time_limit: t7,
                        selection: SelectionStrategy::AgeWeight(5),
                        literal_selection: LiteralSelection::AllNegative,
                        ordering: TermOrdering::LPO,
                        ..SearchConfig::default()
                    },
                    t7,
                ),
                (
                    SearchConfig {
                        time_limit: t8,
                        selection: SelectionStrategy::GoalDirected(10),
                        literal_selection: LiteralSelection::AllNegative,
                        ordering: TermOrdering::LPO,
                        ..SearchConfig::default()
                    },
                    t8,
                ),
                (
                    SearchConfig {
                        time_limit: t9,
                        selection: SelectionStrategy::SmallestFirst,
                        literal_selection: LiteralSelection::AllNegative,
                        ordering: TermOrdering::LPO,
                        ..SearchConfig::default()
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
///
/// `symbols` is required so that the TSTP proof string can be formatted inside the
/// worker thread (while `SearchState` is still live), before the non-`Send`
/// `varisat::Solver` is dropped.
pub fn run_schedule(
    clauses: &[Clause],
    id_gen: ClauseIdGen,
    schedule: &StrategySchedule,
    symbols: &SymbolTable,
) -> SearchResult {
    // 1. Analyze problem for symbol frequencies to configure KBO/LPO and weights.
    let mut sym_counts: HashMap<SymbolId, u32> = HashMap::new();
    for clause in clauses {
        for lit in &clause.literals {
            match &lit.atom {
                mrs_core::formula::Atom::Pred(p, args) => {
                    *sym_counts.entry(*p).or_insert(0) += 1;
                    let mut stack: Vec<&Term> = args.iter().collect();
                    while let Some(t) = stack.pop() {
                        if let Term::App(f, nested_args) = t {
                            *sym_counts.entry(*f).or_insert(0) += 1;
                            stack.extend(nested_args.iter());
                        }
                    }
                }
                mrs_core::formula::Atom::Eq(l, r) => {
                    let mut stack = vec![l, r];
                    while let Some(t) = stack.pop() {
                        if let Term::App(f, nested_args) = t {
                            *sym_counts.entry(*f).or_insert(0) += 1;
                            stack.extend(nested_args.iter());
                        }
                    }
                }
            }
        }
    }

    let mut syms_by_freq: Vec<(SymbolId, u32)> = sym_counts.into_iter().collect();
    syms_by_freq.sort_by_key(|&(_, count)| count); // lowest count first (rarest)

    // Rare symbols get HIGHER precedence to eliminate them quickly.
    let mut precedence = vec![
        0;
        syms_by_freq
            .iter()
            .map(|&(s, _)| s.index() as usize)
            .max()
            .unwrap_or(0)
            + 1
    ];
    for (i, &(sym, _)) in syms_by_freq.iter().rev().enumerate() {
        precedence[sym.index() as usize] = (syms_by_freq.len() - i) as u32;
    }

    // Config: dynamic weights and precedence driven by rarity
    let mut weights = vec![
        1;
        syms_by_freq
            .iter()
            .map(|&(s, _)| s.index() as usize)
            .max()
            .unwrap_or(0)
            + 1
    ];
    for (sym, _) in &syms_by_freq {
        // We could use arity or other heuristics, but for now we just make non-variable symbols weigh 2
        // KBO typically requires w(f) >= w0, and w(c) >= w0 for constants
        weights[sym.index() as usize] = 2;
    }

    let config = Arc::new(SymbolConfig {
        precedence,
        weights,
        w0: 1,
    });

    let mut actual_configs = Vec::new();
    for (search_config, _) in &schedule.strategies {
        let mut actual_config = search_config.clone();
        actual_config.ordering = match search_config.ordering {
            TermOrdering::KBO => TermOrdering::CustomKBO(config.clone()),
            TermOrdering::LPO => TermOrdering::CustomLPO(config.clone()),
            ref other => other.clone(),
        };
        actual_configs.push(actual_config);
    }

    let clauses_owned = clauses.to_vec();

    // Pre-compute EPR ground instances once.  EPR expansion can take several
    // seconds for large problems; running it once per strategy (up to 9 times)
    // would multiply that cost.  We generate the ground clauses a single time
    // and clone the result for each strategy that needs it.
    //
    // `epr_id_gen_base` is advanced past the IDs used for the ground clauses;
    // each strategy then clones it so newly derived clauses get unique IDs
    // that do not collide with the pre-computed ground ones.
    let mut epr_id_gen_base = id_gen.clone();
    let epr_ground_cache: Option<Vec<Clause>> =
        preprocess_epr(&clauses_owned, &mut epr_id_gen_base);

    // Detect EPR structure even when the full expansion exceeds MAX_INSTANCES.
    // EPR problems (only variables and ground terms, no function symbols of
    // arity ≥ 1) must run without AVATAR regardless of whether we succeeded in
    // expanding them: AVATAR's SAT instance grows without bound during a
    // resolution search over EPR clauses and can cause varisat to block for
    // minutes with no way to interrupt it.
    let epr_structure = epr_ground_cache.is_some() || is_epr(&clauses_owned);

    let mut last_result = SearchResult::GaveUp;

    // Total time budget = sum of all strategy slices.
    let total_budget: Duration = actual_configs.iter().map(|c| c.time_limit).sum();
    let schedule_start = Instant::now();

    // Run strategies serially.  Each strategy gets its own time slice; the
    // total wall-clock time across all strategies equals the full budget.
    for search_config in &actual_configs {
        // Guard: if the overall budget is already exhausted (e.g. because
        // prior strategies' preprocessing took longer than their slice),
        // stop launching new strategies.
        let elapsed = schedule_start.elapsed();
        if elapsed >= total_budget {
            break;
        }

        // Trim this strategy's time limit so that preprocessing + search
        // together never exceed the remaining overall budget.
        let remaining = total_budget - elapsed;
        let effective_limit = search_config.time_limit.min(remaining);
        let mut search_config = search_config.clone();
        search_config.time_limit = effective_limit;

        // Use the pre-computed EPR ground clauses (no per-strategy re-expansion).
        let (epr_applied, clauses_local, id_gen_local) = if let Some(ref ground) = epr_ground_cache
        {
            (true, ground.clone(), epr_id_gen_base.clone())
        } else {
            (false, clauses_owned.clone(), id_gen.clone())
        };

        // Disable AVATAR for EPR instances (expanded or not): the ground
        // enumeration may produce tens of thousands of clauses, and even when
        // the expansion is skipped the AVATAR SAT instance grows without bound
        // during a resolution search over EPR clauses (varisat has no interrupt).
        if epr_structure {
            search_config.use_avatar = false;
        }

        // Deduct any time already spent on preprocessing from this strategy's
        // time limit, so the search loop itself stays within the slice.
        let after_preprocess = schedule_start.elapsed();
        let used_in_preprocess = after_preprocess.saturating_sub(elapsed);
        search_config.time_limit = search_config.time_limit.saturating_sub(used_in_preprocess);

        let mut state = SearchState::new(
            clauses_local,
            id_gen_local,
            config.clone(),
            search_config.use_avatar,
        );

        // Deduct SearchState::new time (AVATAR splitting, etc.) as well.
        let after_init = schedule_start.elapsed();
        let used_in_init = after_init.saturating_sub(after_preprocess);
        search_config.time_limit = search_config.time_limit.saturating_sub(used_in_init);

        let result = search(&mut state, &search_config);

        let result = if let SearchResult::Refutation(id, _) = result {
            let proof = extract_proof(id, &state.clause_store);
            let tstp = format_tstp(&proof, symbols);
            SearchResult::Refutation(id, tstp)
        } else if epr_applied && matches!(result, SearchResult::Saturated) {
            // Conservative: EPR instance enumeration is sound but we have
            // observed false Saturated results (e.g. MSC024-1).  Demote to
            // GaveUp so we never output a wrong Satisfiable/CounterSatisfiable.
            SearchResult::GaveUp
        } else {
            result
        };

        match result {
            SearchResult::Refutation(..) => return result,
            other => last_result = other,
        }
    }

    last_result
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
        let result = run_schedule(&[c1, c2], id_gen, &schedule, &syms);
        assert!(matches!(result, SearchResult::Refutation(..)));
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
        let result = run_schedule(&[c1, c2], id_gen, &schedule, &syms);
        // After EPR preprocessing, a saturated ground search is demoted to
        // GaveUp (conservative: avoids outputting a wrong Satisfiable).
        assert!(
            matches!(result, SearchResult::Saturated | SearchResult::GaveUp),
            "expected Saturated or GaveUp, got {result:?}"
        );
    }
}
