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
        // s1-s9: proven baseline portfolio (88% combined)
        // s10-s15: new strategies exploiting distance propagation, SOS, and
        //          goal-symbol heuristics (~12% combined)
        let t10 = Duration::from_millis(ms * 5 / 100); //  5% SOS
        let t11 = Duration::from_millis(ms * 5 / 100); //  5% ConjSymbolBoost
        let t12 = Duration::from_millis(ms * 3 / 100); //  3% HornPenalty (FNE)
        let t13 = Duration::from_millis(ms * 2 / 100); //  2% SOS + GoalDirected + LPO
        let t14 = Duration::from_millis(ms * 2 / 100); //  2% ConjSymbolBoost + All (FEQ)
        // t15 absorbs rounding remainder (~1% at 30 s) — FunctionDepth + LPO
        let t15 = total_time
            .saturating_sub(t1)
            .saturating_sub(t2)
            .saturating_sub(t3)
            .saturating_sub(t4)
            .saturating_sub(t5)
            .saturating_sub(t6)
            .saturating_sub(t7)
            .saturating_sub(t8)
            .saturating_sub(t9)
            .saturating_sub(t10)
            .saturating_sub(t11)
            .saturating_sub(t12)
            .saturating_sub(t13)
            .saturating_sub(t14);

        StrategySchedule {
            strategies: vec![
                // ── KBO strategies ──────────────────────────────────────────
                // s1: balanced exploration
                (
                    SearchConfig {
                        time_limit: t1,
                        selection: SelectionStrategy::AgeWeight(3),
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
                        ..SearchConfig::default()
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
                        selection: SelectionStrategy::AgeWeight(8),
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
                        selection: SelectionStrategy::AgeWeight(10),
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
                        ..SearchConfig::default()
                    },
                    t6,
                ),
                // ── LPO strategies ──────────────────────────────────────────
                // s7: LPO balanced exploration
                (
                    SearchConfig {
                        time_limit: t7,
                        selection: SelectionStrategy::AgeWeight(3),
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
                // ── New heuristic strategies ─────────────────────────────────
                // s10: SOS-restricted + AgeWeight(12) + AllNegative + KBO
                // Set-of-Support: weight picks only return goal-connected clauses
                // (distance < 100), steering resolution toward the conjecture.
                // Based on the audit showing E finds proofs with 10-100x fewer
                // clauses; SOS is its key mechanism on FNE/FEQ problems.
                (
                    SearchConfig {
                        time_limit: t10,
                        selection: SelectionStrategy::AgeWeight(12),
                        literal_selection: LiteralSelection::AllNegative,
                        ordering: TermOrdering::KBO,
                        sos_depth: 100,
                        ..SearchConfig::default()
                    },
                    t10,
                ),
                // s11: ConjSymbolBoost + AgeWeight(6) + AllNegative + KBO
                // Symbols not appearing in the conjecture closure cost 3x.
                // Approximates E's 'prefer goal-relevant symbols' weight function,
                // which is the single most effective heuristic in E's portfolio.
                (
                    SearchConfig {
                        time_limit: t11,
                        selection: SelectionStrategy::AgeWeight(6),
                        literal_selection: LiteralSelection::AllNegative,
                        ordering: TermOrdering::KBO,
                        weight_fn: crate::ClauseWeightFn::ConjSymbolBoost,
                        ..SearchConfig::default()
                    },
                    t11,
                ),
                // s12: HornHeuristic + AgeWeight(5) + AllNegative + KBO, no AVATAR
                // FNE problems are mostly Horn; progressive multiplier (pos_count×)
                // on non-Horn clauses keeps the proof search Horn-focused.
                // Milder than HornPenalty's fixed 3× — better calibrated for
                // problems with 2-3 positive literals.  No AVATAR to avoid
                // SAT-solver overhead on purely Horn problems.
                (
                    SearchConfig {
                        time_limit: t12,
                        selection: SelectionStrategy::AgeWeight(5),
                        literal_selection: LiteralSelection::AllNegative,
                        ordering: TermOrdering::KBO,
                        weight_fn: crate::ClauseWeightFn::HornHeuristic,
                        max_term_weight: None,
                        use_avatar: false,
                        unit_only_resolution: false,
                        ..SearchConfig::default()
                    },
                    t12,
                ),
                // s13: SOS (inference-level) + FunctionWeightPenalty + AgeWeight(5) + KBO
                // Combines inference-level SOS (only infer with at least one
                // goal-connected parent) with quadratic depth weighting to
                // suppress term-tower growth.  SOS keeps focus near the conjecture;
                // FunctionWeightPenalty discards deep superposition chains early.
                (
                    SearchConfig {
                        time_limit: t13,
                        selection: SelectionStrategy::AgeWeight(5),
                        literal_selection: LiteralSelection::AllNegative,
                        ordering: TermOrdering::KBO,
                        weight_fn: crate::ClauseWeightFn::FunctionWeightPenalty,
                        sos_depth: 100,
                        ..SearchConfig::default()
                    },
                    t13,
                ),
                // s14: ConjSymbolBoost + SmallestFirst + All + KBO, no AVATAR
                // FEQ variant: All selection + conjecture-symbol boost + tight weight.
                // No AVATAR: equational chains need unrestricted paramodulation.
                (
                    SearchConfig {
                        time_limit: t14,
                        selection: SelectionStrategy::SmallestFirst,
                        literal_selection: LiteralSelection::All,
                        ordering: TermOrdering::KBO,
                        weight_fn: crate::ClauseWeightFn::ConjSymbolBoost,
                        max_term_weight: Some(100),
                        use_avatar: false,
                        unit_only_resolution: false,
                        ..SearchConfig::default()
                    },
                    t14,
                ),
                // s15: SymbolWeight + AgeWeight(4) + AllNegative + KBO, no AVATAR
                // Precedence-based cost: rare symbols cost more (higher precedence).
                // Encourages the prover to prefer clauses with common, low-precedence
                // symbols — those interact well with demodulation rules already in
                // the active set.  Complementary to ConjSymbolBoost.
                // Absorbs rounding remainder (~1% at 30 s).
                (
                    SearchConfig {
                        time_limit: t15,
                        selection: SelectionStrategy::AgeWeight(4),
                        literal_selection: LiteralSelection::AllNegative,
                        ordering: TermOrdering::KBO,
                        weight_fn: crate::ClauseWeightFn::SymbolWeight,
                        max_term_weight: None,
                        use_avatar: false,
                        unit_only_resolution: false,
                        ..SearchConfig::default()
                    },
                    t15,
                ),
                // ── Diagnostic strategy (zero time in normal runs) ────────────
                // s16: SmallestFirst + All + max_weight=15 + AVATAR
                // AVATAR splits the 46-literal all-positive main clause into 46 independent
                // branches; with SmallestFirst+All+weight=15 each branch refutation is fast
                // (small sub-problem, tight weight keeps passive compact).  BUR output is
                // never weight-filtered (see given_clause.rs).
                // (zero time in normal runs; testable via MRS_SINGLE_STRATEGY=16)
                (
                    SearchConfig {
                        time_limit: Duration::ZERO,
                        selection: SelectionStrategy::SmallestFirst,
                        literal_selection: LiteralSelection::All,
                        ordering: TermOrdering::KBO,
                        max_term_weight: Some(15),
                        use_avatar: true,
                        unit_only_resolution: false,
                        ..SearchConfig::default()
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
/// Options controlling ML data logging and ML-guided selection.
///
/// Plain data in every build; only acted upon when the `ml-guidance`
/// feature is enabled (except `log_dir`, which also drives offline trace
/// collection). `MlOptions::default()` disables everything.
#[derive(Debug, Clone, Default)]
pub struct MlOptions {
    /// Directory to write labeled clause-feature traces to after a refutation.
    pub log_dir: Option<String>,
    /// Write traces as CSV instead of wincode.
    pub log_csv: bool,
    /// Path to trained model weights for ML-guided clause selection.
    pub weights: Option<String>,
}

pub fn run_schedule(
    clauses: &[Clause],
    id_gen: ClauseIdGen,
    schedule: &StrategySchedule,
    symbols: &SymbolTable,
    ml: MlOptions,
    workers: Option<usize>,
) -> (SearchResult, crate::ScheduleReport) {
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
            return (result, crate::ScheduleReport::default());
        }
    }

    // Clone the symbol table into an Arc once; threads and the componentwise
    // pre-pass share it without further copies.
    let symbols_arc = std::sync::Arc::new(symbols.clone());

    // Componentwise AVATAR pre-pass: for problems produced by definitional CNF
    // on a top-level conjunction, the input contains a single large positive
    // disjunction `def_1(X̄₁) ∨ ... ∨ def_N(X̄_N)` with distinct predicate
    // symbols, plus definition clauses encoding each conjunct.  Refute every
    // branch independently.
    {
        let mut cwa_id_gen = id_gen.clone();
        if let Some(result) = try_componentwise_refute(
            &clauses_owned,
            &mut cwa_id_gen,
            symbols_arc.clone(),
            config.clone(),
        ) {
            return (result, crate::ScheduleReport::default());
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
    let (tx, rx) = mpsc::channel::<(usize, SearchResult, crate::SearchStats, u64)>();

    let available_cores = workers.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    });
    let num_workers = available_cores.min(actual_configs.len());
    let next_strategy = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let has_epr = epr_ground_cache.is_some();

    #[cfg(not(feature = "ml-guidance"))]
    if ml.weights.is_some() {
        eprintln!(
            "Warning: ML weights provided but this build lacks the `ml-guidance` feature; ignoring."
        );
    }
    #[cfg(feature = "ml-guidance")]
    let ml_model = ml.weights.as_ref().and_then(|path| {
        use burn::backend::NdArray;
        use burn::module::Module;
        use burn::record::{BinFileRecorder, Recorder};
        use mrs_core::ml::model::ClauseClassifier;

        match BinFileRecorder::<burn::record::HalfPrecisionSettings>::default()
            .load(path.into(), &Default::default())
        {
            Ok(record) => {
                let model =
                    ClauseClassifier::<NdArray>::new(&Default::default()).load_record(record);
                Some(Arc::new(model))
            }
            Err(e) => {
                eprintln!("Failed to load ML weights from {}: {:?}", path, e);
                None
            }
        }
    });

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
            let symbols_thread = symbols_arc.clone();
            let actual_configs_ref = &actual_configs;

            #[cfg(feature = "ml-guidance")]
            let ml_model_thread = ml_model.clone();
            let log_ml_data_thread = ml.log_dir.clone();

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

                    let mut state = SearchState::new_with_ml(
                        clauses_for_thread.clone(),
                        id_gen_thread.clone(),
                        config_thread.clone(),
                        symbols_thread.clone(),
                        sc.use_avatar,
                        log_ml_data_thread.clone(),
                        ml.log_csv,
                        sc.weight_fn.clone(),
                    );
                    #[cfg(feature = "ml-guidance")]
                    {
                        state.ml_model = ml_model_thread.clone();
                    }
                    state.stop_flag = Some(Arc::clone(&stop));
                    state.shared_pool = Some(Arc::clone(&pool));

                    let strategy_start = std::time::Instant::now();
                    let raw = search(&mut state, &sc);
                    let elapsed_ms = strategy_start.elapsed().as_millis() as u64;
                    // Capture passive size after search (unprocessed set is still live).
                    state.stats.passive_size = state.unprocessed.active_count() as u64;

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
                            #[cfg(feature = "ml-guidance")]
                            if let Some(log_dir) = &state.log_ml_data {
                                let positive_ids = mrs_proof::extract::extract_proof_ids(id, &state.clause_store);
                                let pos_set: std::collections::HashSet<_> = positive_ids.iter().copied().collect();

                                let mut all_samples = Vec::new();

                                for (&cid, clause) in &state.clause_store {
                                    let is_pos = pos_set.contains(&cid);

                                    // Negative subsampling: keep all positives, sample ~10% of negatives
                                    if !is_pos && rand::random::<f32>() > 0.1 {
                                        continue;
                                    }

                                    let label = if is_pos { 1.0 } else { 0.0 };
                                    let weight = crate::weight::clause_weight_id(clause, &state.term_bank, &state.config) as f32;
                                    let feats = mrs_core::ml::features::extract(clause, &state.term_bank, symbols, weight);
                                    all_samples.push(mrs_core::ml::sample::LabeledSample { label, feats });
                                }

                                let problem_name = std::env::var("PROBLEM_NAME").unwrap_or_else(|_| "unknown_problem".to_string());
                                let file_stem = format!("{}_{}_{}", problem_name, strategy_idx, id.0);
                                let log_path = std::path::Path::new(log_dir);
                                std::fs::create_dir_all(log_path).ok();

                                if state.ml_log_csv {
                                    if let Ok(mut w) = std::fs::File::create(log_path.join(format!("{}.csv", file_stem))) {
                                        use std::io::Write;
                                        for s in &all_samples {
                                            let feats_str = s.feats.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(",");
                                            let _ = writeln!(w, "{},{}", s.label, feats_str);
                                        }
                                    }
                                } else {
                                    if let Ok(mut w) = std::fs::File::create(log_path.join(format!("{}.wincode", file_stem))) {
                                        let mut std_write = wincode::io::std_write::WriteAdapter::new(&mut w);
                                        for s in &all_samples {
                                            let _ = wincode::serialize_into(&mut std_write, s);
                                        }
                                    }
                                }
                            }

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
                        // SOS is refutationally incomplete: a strategy with sos_depth set
                        // cannot distinguish "no proof exists" from "proof exists but is
                        // unreachable under SOS restrictions".  Saturation from an
                        // SOS-restricted strategy must therefore be GaveUp, not Saturated.
                        // Without this, the stop flag fires and the entire portfolio is
                        // killed, producing a false CounterSatisfiable on Theorem problems.
                        SearchResult::Saturated if sc.sos_depth < u32::MAX => SearchResult::GaveUp,
                        // Unit-only resolution is incomplete: a clause set may be
                        // unsatisfiable yet require non-unit resolvents to find the proof.
                        SearchResult::Saturated if sc.unit_only_resolution => SearchResult::GaveUp,
                        // Non-Standard weight functions affect the ORDER in which clauses
                        // are selected, which in turn changes which clauses are generated
                        // and which are simplified away.  This interaction between
                        // ordering and simplification (forward subsumption, condensation)
                        // can make saturation incomplete even when passive=0: a proof-
                        // relevant clause may have been forward-subsumed earlier than it
                        // would have been with Standard ordering.  Treat saturation from
                        // any non-Standard weight strategy as GaveUp to avoid false
                        // Satisfiable/CounterSatisfiable verdicts.
                        SearchResult::Saturated
                            if sc.weight_fn != crate::ClauseWeightFn::Standard =>
                        {
                            SearchResult::GaveUp
                        }
                        other => other,
                    };

                    if matches!(
                        result,
                        SearchResult::Refutation(..) | SearchResult::Saturated
                    ) {
                        stop.store(true, Ordering::Relaxed);
                    }

                    let _ = tx.send((strategy_idx, result, state.stats.clone(), elapsed_ms));
                }
            });
        }

        // Drop the main sender so the channel closes when all threads finish.
        drop(tx);

        // Collect results: track the best definitive answer seen.
        // Priority: Refutation > Saturated > GaveUp > Timeout
        let mut best: SearchResult = SearchResult::GaveUp;
        let mut report = crate::ScheduleReport::default();

        for (idx, res, stats, elapsed_ms) in rx.into_iter() {
            report.strategies.push(crate::StrategyReport {
                strategy_idx: idx,
                result: res.clone(),
                stats,
                elapsed_ms,
            });
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
        (best, report)
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
        let (result, _) = run_schedule(
            &[c1, c2],
            id_gen,
            &schedule,
            &syms,
            MlOptions::default(),
            None,
        );
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
        let (result, _) = run_schedule(
            &[c1, c2],
            id_gen,
            &schedule,
            &syms,
            MlOptions::default(),
            None,
        );
        // After EPR preprocessing, a saturated ground search is demoted to
        // GaveUp (conservative: avoids outputting a wrong Satisfiable).
        assert!(
            matches!(result, SearchResult::Saturated | SearchResult::GaveUp),
            "expected Saturated or GaveUp, got {result:?}"
        );
    }
}
