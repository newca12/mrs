//! Strategy scheduling: run multiple search strategies in sequence.
//!
//! Different clause selection strategies explore the search space differently.
//! Strategy scheduling tries multiple strategies with time slices, increasing
//! the chance of finding a proof within the overall time limit.
//!
//! This is inspired by systems like Vampire and E, which use strategy
//! portfolios for CASC competition.

pub mod named;

use crate::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use mrs_calculus::ordering::SymbolConfig;
use mrs_core::SymbolTable;
use mrs_core::clause::{Clause, ClauseIdGen};
use mrs_core::symbol::SymbolId;
use mrs_core::term::Term;
use mrs_proof::extract::extract_proof;
use mrs_proof::tstp::format_tstp;

use crate::cwa::try_componentwise_refute;
use crate::fvo::try_fvo_refutation;
use crate::given_clause::search;
use crate::instgen::is_epr;
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
    ///  1. AgeWeight(5) + AllNegative + KBO — balanced exploration (14%)
    ///  2. SmallestFirst + AllNegative + KBO + no weight limit + no AVATAR — deep chain proofs (10%)
    ///  3. SmallestFirst + AllNegative + KBO — pure best-first (10%)
    ///  4. AgeWeight(5) + MaxNegative + KBO — aggressive selection (9%)
    ///  5. AgeWeight(5) + All + KBO — unrestricted literal selection (9%)
    ///  6. AgeWeight(5) + All + KBO + no AVATAR — FNE/definitional CNF proofs (10%)
    ///  7. AgeWeight(5) + AllNegative + LPO — LPO balanced exploration (14%)
    ///  8. GoalDirected(10) + AllNegative + LPO — LPO goal-directed (9%)
    ///  9. SmallestFirst + AllNegative + LPO — LPO best-first (9%)
    /// 10. SmallestFirst + All + KBO + max_weight=30 + no AVATAR — FEQ (4%)
    /// 11. AgeWeight(5) + All + LPO + no weight limit + no AVATAR — FEQ (remainder ~2%)
    pub fn default_schedule(total_time: Duration, workers: usize) -> Self {
        // Allow overriding to a single strategy for diagnosis: MRS_SINGLE_STRATEGY=N
        // runs only strategy N (1-indexed) for the full time budget.
        if let Ok(val) = std::env::var("MRS_SINGLE_STRATEGY")
            && let Ok(n) = val.trim().parse::<usize>()
        {
            let full = StrategySchedule::_all_strategies(total_time, workers);
            if n >= 1 && n <= full.strategies.len() {
                let (mut cfg, _) = full.strategies[n - 1].clone();
                cfg.time_limit = total_time;
                return StrategySchedule {
                    strategies: vec![(cfg, total_time)],
                };
            }
        }
        Self::_all_strategies(total_time, workers)
    }

    fn _all_strategies(total_time: Duration, _workers: usize) -> Self {
        let ms = total_time.as_millis() as u64;
        // s1–s9: restored close to original proportions (92% combined for a 30 s budget)
        // s10–s11: small FEQ-targeted bonus budgets (8% combined)
        let t1 = Duration::from_millis(ms * 14 / 100); // 14% (was 15%)
        let t2 = Duration::from_millis(ms * 10 / 100); // 10%
        let t3 = Duration::from_millis(ms * 10 / 100); // 10%
        let t4 = Duration::from_millis(ms * 9 / 100); //   9% (was 10%)
        let t5 = Duration::from_millis(ms * 9 / 100); //   9% (was 10%)
        let t6 = Duration::from_millis(ms * 10 / 100); // 10%
        let t7 = Duration::from_millis(ms * 14 / 100); // 14% (was 15%)
        let t8 = Duration::from_millis(ms * 9 / 100); //   9% (was 10%)
        let t9 = Duration::from_millis(ms * 9 / 100); //   9% (was ~10%)
        // s1–s9 total: 94%
        let t10 = Duration::from_millis(ms * 4 / 100); //  4% FEQ KBO
        // t11 absorbs rounding remainder (~2% at 30 s) — FEQ LPO
        let t11 = total_time
            .saturating_sub(t1)
            .saturating_sub(t2)
            .saturating_sub(t3)
            .saturating_sub(t4)
            .saturating_sub(t5)
            .saturating_sub(t6)
            .saturating_sub(t7)
            .saturating_sub(t8)
            .saturating_sub(t9)
            .saturating_sub(t10);

        StrategySchedule {
            strategies: vec![
                // ── KBO strategies ──────────────────────────────────────────
                // s1: balanced exploration
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
                // s2: deep chain proofs
                (
                    SearchConfig {
                        time_limit: t2,
                        selection: SelectionStrategy::SmallestFirst,
                        literal_selection: LiteralSelection::AllNegative,
                        ordering: TermOrdering::KBO,
                        // No weight limit: allows proofs that require terms of unbounded
                        // size (e.g. SYN986+1.004 needs succ^65536(zero) as a witness).
                        max_term_weight: None,
                        // No AVATAR: avoids overhead and incorrect dormancy on chain proofs.
                        use_avatar: false,
                        unit_only_resolution: false,
                    },
                    t2,
                ),
                // s3: pure best-first
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
                // s4: aggressive selection
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
                // s5: unrestricted literal selection
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
                // s6: FNE/definitional CNF proofs
                (
                    SearchConfig {
                        time_limit: t6,
                        selection: SelectionStrategy::AgeWeight(5),
                        literal_selection: LiteralSelection::All,
                        ordering: TermOrdering::KBO,
                        // No weight limit: SYN938+1 has 185 clauses whose proof path
                        // requires resolving a 46-literal definitional main clause through
                        // multi-step chains; intermediate clauses can exceed 200 weight,
                        // so removing the cap allows the proof to go through.
                        max_term_weight: None,
                        // No AVATAR: AVATAR splits definition clauses (e.g. ~def_k | p(X))
                        // into separate components and can dormant the ~def_k unit under
                        // the SAT model, blocking key resolutions in FNE-style problems.
                        use_avatar: false,
                        unit_only_resolution: false,
                    },
                    t6,
                ),
                // ── LPO strategies ──────────────────────────────────────────
                // s7: LPO balanced exploration
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
                // s8: LPO goal-directed
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
                // s9: LPO best-first
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
                // ── FEQ-targeted strategies ──────────────────────────────────
                // s10: KBO with All selection and moderate weight cap — FEQ
                // All selection allows picking positive equality literals for
                // paramodulation, which is critical for equational problems.
                // Weight cap 30 keeps passive compact on large FEQ clause sets.
                // No AVATAR: component splits interfere with equational chaining.
                // Small budget (4%): provides FEQ coverage without stealing time
                // from the main s1-s9 portfolio.
                (
                    SearchConfig {
                        time_limit: t10,
                        selection: SelectionStrategy::SmallestFirst,
                        literal_selection: LiteralSelection::All,
                        ordering: TermOrdering::KBO,
                        max_term_weight: Some(30),
                        use_avatar: false,
                        unit_only_resolution: false,
                    },
                    t10,
                ),
                // s11: LPO with All selection and no weight cap — FEQ
                // LPO handles function-symbol-heavy equational theories differently
                // from KBO; removing the weight cap allows following long
                // paramodulation chains to their conclusion.
                // No AVATAR: same reason as s10.
                // Absorbs rounding remainder (~2% at 30 s).
                (
                    SearchConfig {
                        time_limit: t11,
                        selection: SelectionStrategy::AgeWeight(5),
                        literal_selection: LiteralSelection::All,
                        ordering: TermOrdering::LPO,
                        max_term_weight: None,
                        use_avatar: false,
                        unit_only_resolution: false,
                    },
                    t11,
                ),
                // ── Diagnostic strategy (zero time in normal runs) ────────────
                // s12: SmallestFirst + All + max_weight=15 + AVATAR
                // AVATAR splits the 46-literal all-positive main clause into 46 independent
                // branches; with SmallestFirst+All+weight=15 each branch refutation is fast
                // (small sub-problem, tight weight keeps passive compact).  BUR output is
                // never weight-filtered (see given_clause.rs).
                // (zero time in normal runs; testable via MRS_SINGLE_STRATEGY=12)
                (
                    SearchConfig {
                        time_limit: Duration::ZERO,
                        selection: SelectionStrategy::SmallestFirst,
                        literal_selection: LiteralSelection::All,
                        ordering: TermOrdering::KBO,
                        max_term_weight: Some(15),
                        use_avatar: true,
                        unit_only_resolution: false,
                    },
                    Duration::ZERO,
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
/// `cadical::Solver` is dropped.
pub fn run_schedule(
    clauses: &[Clause],
    id_gen: ClauseIdGen,
    schedule: &StrategySchedule,
    symbols: &SymbolTable,
    workers: Option<usize>,
) -> SearchResult {
    // 1. Analyze problem for symbol frequencies to configure KBO/LPO and weights.
    let mut sym_counts: HashMap<SymbolId, u32> = HashMap::default();
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
    syms_by_freq.sort_unstable_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0))); // lowest count first (rarest)

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

    // EPR pre-grounding is disabled: naive ground instance enumeration causes
    // OOM on large EPR problems (tens of thousands of ground clauses inflate
    // cadical's SAT instance beyond memory limits).  AVATAR handles EPR
    // structure lazily and correctly without pre-expansion.
    let epr_ground_cache: Option<Vec<Clause>> = None;

    // Detect EPR structure even when the full expansion exceeds MAX_INSTANCES.
    // EPR problems (only variables and ground terms, no function symbols of
    // arity ≥ 1) must run without AVATAR regardless of whether we succeeded in
    // expanding them: AVATAR's SAT instance grows without bound during a
    // resolution search over EPR clauses and can cause cadical to block for
    // minutes with no way to interrupt it.
    // NOTE: EPR grounding is disabled (epr_ground_cache is always None).
    //       is_epr is retained for future use (e.g. re-enabling AVATAR guard).
    let _ = is_epr(&clauses_owned);

    // FVO pre-pass: for clause sets where all predicate arguments are variables
    // (no equality, no function terms), the first-order problem is
    // propositionally equivalent.  Try a BFS propositional refutation first;
    // it solves problems like SYN938+1 in milliseconds where the regular
    // given-clause loop times out.
    {
        let mut fvo_id_gen = id_gen.clone();
        if let Some(result) = try_fvo_refutation(&clauses_owned, &mut fvo_id_gen, symbols) {
            return result;
        }
    }

    // Componentwise AVATAR pre-pass: for problems produced by definitional CNF
    // on a top-level conjunction, the input contains a single large positive
    // disjunction `def_1(X̄₁) ∨ ... ∨ def_N(X̄_N)` with distinct predicate
    // symbols, plus definition clauses encoding each conjunct.  Refute every
    // branch independently.
    {
        let mut cwa_id_gen = id_gen.clone();
        if let Some(result) =
            try_componentwise_refute(&clauses_owned, &mut cwa_id_gen, symbols, config.clone())
        {
            return result;
        }
    }

    // Total time budget = sum of all strategy slices.
    let total_budget: Duration = actual_configs.iter().map(|c| c.time_limit).sum();
    let schedule_start = Instant::now();

    // Run strategies in parallel.  Each non-zero strategy runs for the full
    // remaining budget instead of only its proportional slice — with N threads
    // running simultaneously the wall-clock time is bounded by the total budget
    // while we explore N different search directions at once.
    //
    // A shared stop-flag is set by the first thread that finds a Refutation or
    // a genuine Saturation; every other thread notices it on its next time-check
    // iteration and returns Timeout.
    //
    // SearchState (and the cadical::Solver it contains) is Send, but TermBank
    // is heavily thread-local (interning IDs are only valid locally). So each
    // thread constructs its own SearchState from the cloned clause data, but
    // we share a pool of globally discovered unit equalities via an RwLock.
    let stop_flag = Arc::new(AtomicBool::new(false));
    let shared_pool = Arc::new(std::sync::RwLock::new(Vec::new()));
    let (tx, rx) = mpsc::channel::<(usize, SearchResult)>();

    let available_cores = workers.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    });
    let num_workers = available_cores.min(actual_configs.len());
    let next_strategy = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let has_epr = epr_ground_cache.is_some();

    // Compute how much time remains after all pre-processing steps.
    let remaining_at_spawn = total_budget.saturating_sub(schedule_start.elapsed());

    std::thread::scope(|s| {
        for _ in 0..num_workers {
            let stop = Arc::clone(&stop_flag);
            let pool = Arc::clone(&shared_pool);
            let tx = tx.clone();
            let next_strategy = Arc::clone(&next_strategy);
            let clauses_for_thread = clauses_owned.clone();
            let id_gen_thread = id_gen.clone();
            let config_thread = Arc::clone(&config);
            let actual_configs_ref = &actual_configs;

            s.spawn(move || {
                loop {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let strategy_idx = next_strategy.fetch_add(1, Ordering::Relaxed);
                    if strategy_idx >= actual_configs_ref.len() {
                        break;
                    }

                    let search_config = &actual_configs_ref[strategy_idx];

                    if search_config.time_limit.is_zero() {
                        continue;
                    }
                    if remaining_at_spawn.is_zero() {
                        break;
                    }

                    let mut sc = search_config.clone();
                    // Scale the individual time slice by the number of workers, capped by wall-clock limit.
                    let scaled_ms = (sc.time_limit.as_millis() as u64).saturating_mul(num_workers as u64);
                    sc.time_limit = Duration::from_millis(scaled_ms).min(remaining_at_spawn);

                    if has_epr {
                        sc.max_term_weight = None;
                    }

                    let mut state = SearchState::new(
                        clauses_for_thread.clone(),
                        id_gen_thread.clone(),
                        config_thread.clone(),
                        sc.use_avatar,
                    );
                    state.stop_flag = Some(Arc::clone(&stop));
                    state.shared_pool = Some(Arc::clone(&pool));

                    let raw = search(&mut state, &sc);

                    if std::env::var("TRACE_SEARCH").is_ok() {
                        eprintln!(
                            "[TRACE] strategy {} ({:?}+{:?}+no_weight={}) result={:?} elapsed={:.2}s",
                            strategy_idx + 1,
                            sc.selection,
                            sc.literal_selection,
                            sc.max_term_weight.is_none(),
                            raw,
                            schedule_start.elapsed().as_secs_f64(),
                        );
                    }

                    let result = match raw {
                        SearchResult::Refutation(id, _) => {
                            let legacy_store: HashMap<_, _> = state
                                .clause_store
                                .iter()
                                .map(|(&cid, ic)| (cid, state.term_bank.clause_to_legacy(ic)))
                                .collect();
                            let proof = extract_proof(id, &legacy_store);
                            let tstp = format_tstp(&proof, symbols);
                            SearchResult::Refutation(id, tstp)
                        }
                        SearchResult::Saturated if sc.max_term_weight.is_some() => SearchResult::GaveUp,
                        other => other,
                    };

                    if matches!(
                        result,
                        SearchResult::Refutation(..) | SearchResult::Saturated
                    ) {
                        stop.store(true, Ordering::Relaxed);
                    }

                    let _ = tx.send((strategy_idx, result));
                }
            });
        }

        // Drop the main sender so the channel closes when all threads finish.
        drop(tx);

        // Collect results: track the best definitive answer seen.
        // Priority: Refutation > Saturated > GaveUp > Timeout
        let mut best: SearchResult = SearchResult::GaveUp;
        for (_idx, res) in rx.into_iter() {
            match &res {
                SearchResult::Refutation(..) => {
                    best = res;
                    // Keep draining the channel so threads can finish cleanly.
                }
                SearchResult::Saturated => {
                    if !matches!(best, SearchResult::Refutation(..)) {
                        best = res;
                    }
                }
                SearchResult::GaveUp => {
                    if matches!(best, SearchResult::Timeout) {
                        best = res;
                    }
                }
                SearchResult::Timeout => { /* lowest priority — keep existing best */ }
            }
        }
        best
    })
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

        let schedule = StrategySchedule::default_schedule(Duration::from_secs(5), 1);
        let result = run_schedule(&[c1, c2], id_gen, &schedule, &syms, None);
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

        let schedule = StrategySchedule::default_schedule(Duration::from_secs(5), 1);
        let result = run_schedule(&[c1, c2], id_gen, &schedule, &syms, None);
        // After EPR preprocessing, a saturated ground search is demoted to
        // GaveUp (conservative: avoids outputting a wrong Satisfiable).
        assert!(
            matches!(result, SearchResult::Saturated | SearchResult::GaveUp),
            "expected Saturated or GaveUp, got {result:?}"
        );
    }
}
