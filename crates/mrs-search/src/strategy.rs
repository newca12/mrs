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

use mrs_core::SymbolTable;
use mrs_core::clause::{Clause, ClauseIdGen};
use mrs_proof::extract::extract_proof;
use mrs_proof::tstp::format_tstp;

use crate::cwa::try_componentwise_refute;
use crate::fvo::try_fvo_refutation;
use crate::given_clause::search;
use crate::instgen::is_epr;
use crate::state::SearchState;
use crate::{
    LiteralSelection, SearchConfig, SearchResult, SelectionStrategy, SharedClauseChain,
    TermOrdering,
};

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

    pub(crate) fn _all_strategies(total_time: Duration, _workers: usize) -> Self {
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
                        precedence_scheme: crate::PrecedenceScheme::ArityMin,
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
                        symbol_weight_scheme: crate::SymbolWeightScheme::Arity,
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
                        precedence_scheme: crate::PrecedenceScheme::ArityMax,
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
                        precedence_scheme: crate::PrecedenceScheme::Freq,
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
                        symbol_weight_scheme: crate::SymbolWeightScheme::InvFreq,
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
                        precedence_scheme: crate::PrecedenceScheme::GoalBoost,
                        symbol_weight_scheme: crate::SymbolWeightScheme::ConjectureBonus,
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
                        precedence_scheme: crate::PrecedenceScheme::ArityMin,
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
                        precedence_scheme: crate::PrecedenceScheme::GoalBoost,
                        symbol_weight_scheme: crate::SymbolWeightScheme::ConjectureBonus,
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
                        precedence_scheme: crate::PrecedenceScheme::GoalBoost,
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
                        precedence_scheme: crate::PrecedenceScheme::ArityMin,
                        symbol_weight_scheme: crate::SymbolWeightScheme::Arity,
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
                        precedence_scheme: crate::PrecedenceScheme::ArityMax,
                        symbol_weight_scheme: crate::SymbolWeightScheme::InvFreq,
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
                        precedence_scheme: crate::PrecedenceScheme::GoalBoost,
                        symbol_weight_scheme: crate::SymbolWeightScheme::ConjectureBonus,
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
                        precedence_scheme: crate::PrecedenceScheme::InvFreq,
                        symbol_weight_scheme: crate::SymbolWeightScheme::Arity,
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

    /// Automatically applies parallel SInE threshold tuning across the portfolio strategies.
    pub fn apply_sine_threshold_tuning(&mut self) {
        let sine_configs = [
            (Some(1.5), Some(3)), // strict SInE
            (Some(2.0), Some(5)), // standard SInE
            (Some(3.5), Some(8)), // relaxed SInE
            (None, None),         // SInE disabled
        ];
        for (i, (cfg, _)) in self.strategies.iter_mut().enumerate() {
            let (sine_tol, sine_depth) = sine_configs[i % sine_configs.len()];
            cfg.sine_tolerance = sine_tol;
            cfg.sine_depth_limit = sine_depth;
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
/// `mrs_cadical::Solver` is dropped.
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
    /// ML premise selection keep-set: clause ids (axioms judged relevant plus
    /// all conjectures) that pruned workers restrict their input to.
    ///
    /// Pruning is applied **per worker**, on the last 2 strategy slots of the
    /// portfolio (`casc_*` schedules are greedy-set-cover ordered by
    /// decreasing marginal coverage, so the tail slots are the cheapest to
    /// put at risk); the rest of the portfolio runs on the full clause set
    /// unpruned. A worker that actually pruned clauses can never report
    /// `Saturated` (demoted to `GaveUp`), because saturating a subset says
    /// nothing about the full problem. `None` disables pruning.
    pub premise_keep: Option<Arc<std::collections::HashSet<mrs_core::clause::ClauseId>>>,
}

/// Pick a division portfolio from the syntactic shape of the clause set.
///
/// Rule-based replacement for the (frozen) ML schedule classifier: the CASC
/// division of a problem is deterministically computable from its clauses,
/// so no learned model is needed. Rules, in priority order:
///
/// 1. UEQ (every clause a unit equality literal)      → `casc_ueq`
/// 2. EPR (no function symbols of arity ≥ 1)          → `casc_epr`
/// 3. FNE (no equality anywhere)                      → `casc_fne`
/// 4. otherwise (first-order with equality)           → `casc_feq`
pub fn auto_schedule_name(clauses: &[Clause]) -> &'static str {
    // UEQ is checked before EPR so that constants-only unit-equality problems
    // go to the equational portfolio (EPR portfolios are tuned for
    // non-equational SAT-style splitting).
    let all_unit_eq = !clauses.is_empty()
        && clauses.iter().all(|c| {
            c.literals.len() == 1 && matches!(c.literals[0].atom, mrs_core::formula::Atom::Eq(_, _))
        });
    if all_unit_eq {
        return "casc_ueq";
    }
    if is_epr(clauses) {
        return "casc_epr";
    }
    let has_eq = clauses.iter().any(|c| {
        c.literals
            .iter()
            .any(|l| matches!(l.atom, mrs_core::formula::Atom::Eq(_, _)))
    });
    if !has_eq { "casc_fne" } else { "casc_feq" }
}

/// Number of trailing portfolio slots that ML premise pruning is allowed to
/// substitute a pruned clause view for.
pub const ML_PRUNE_LAST_SLOTS: usize = 2;

/// Whether `strategy_idx` (0-based) out of `total_strategies` portfolio
/// entries is one of the last [`ML_PRUNE_LAST_SLOTS`] slots eligible for ML
/// premise pruning.
///
/// `casc_*` portfolios are ordered by decreasing marginal coverage (greedy
/// set-cover, see AGENTS.md), so pruning the tail slots sacrifices the least
/// expected coverage if the pruned view doesn't help on a given problem.
fn is_ml_prune_slot(strategy_idx: usize, total_strategies: usize) -> bool {
    strategy_idx >= total_strategies.saturating_sub(ML_PRUNE_LAST_SLOTS)
}

pub fn run_schedule(
    clauses: &[Clause],
    provenance: &[Clause],
    id_gen: ClauseIdGen,
    schedule: &StrategySchedule,
    symbols: &SymbolTable,
    ml: MlOptions,
    workers: Option<usize>,
) -> (SearchResult, crate::ScheduleReport) {
    // 1. Compute default symbol configuration for pre-passes.
    let default_config = crate::symbol_config::compute_symbol_config(
        clauses,
        crate::PrecedenceScheme::InvFreq,
        crate::SymbolWeightScheme::Uniform,
    );

    let mut has_eq = false;
    for clause in clauses {
        if has_eq {
            break;
        }
        for lit in &clause.literals {
            if let mrs_core::formula::Atom::Eq(_, _) = lit.atom {
                has_eq = true;
                break;
            }
        }
    }
    let is_fne = !has_eq;
    let disable_single_neg = std::env::var("MRS_NO_SINGLE_NEG").is_ok();
    let apply_max_neg = is_fne && !disable_single_neg;

    let mut actual_configs = Vec::new();
    for (search_config, _) in &schedule.strategies {
        let mut actual_config = search_config.clone();
        if let Ok(value) = std::env::var("MRS_LRS_FIXED_ITERATIONS")
            && let Ok(budget) = value.parse::<u64>()
        {
            actual_config.lrs_policy = crate::LrsPolicy::FixedIterations { budget };
        }
        if let Ok(value) = std::env::var("MRS_SHARED_POOL_INTERVAL")
            && let Ok(interval) = value.parse::<u64>()
        {
            actual_config.shared_pool_poll_interval = interval;
        }
        if apply_max_neg
            && matches!(
                actual_config.literal_selection,
                LiteralSelection::AllNegative
            )
        {
            actual_config.literal_selection = LiteralSelection::MaxNegative;
        }
        let sym_config = crate::symbol_config::compute_symbol_config(
            clauses,
            search_config.precedence_scheme,
            search_config.symbol_weight_scheme,
        );
        actual_config.ordering = match search_config.ordering {
            TermOrdering::KBO => TermOrdering::CustomKBO(sym_config),
            TermOrdering::LPO => TermOrdering::CustomLPO(sym_config),
            ref other => other.clone(),
        };
        actual_configs.push(actual_config);
    }

    let clauses_owned = clauses.to_vec();

    // EPR pre-grounding is disabled: naive ground instance enumeration causes
    // OOM on large EPR problems (tens of thousands of ground clauses inflate
    // CaDiCaL's SAT instance beyond memory limits). AVATAR handles EPR
    // structure lazily and correctly without pre-expansion.

    // Detect EPR structure even when the full expansion exceeds MAX_INSTANCES.
    // EPR problems (only variables and ground terms, no function symbols of
    // arity ≥ 1) must run without AVATAR regardless of whether we succeeded in
    // expanding them: AVATAR's SAT instance grows without bound during a
    // resolution search over EPR clauses and can cause CaDiCaL to block for
    // minutes with no way to interrupt it.
    let is_problem_epr = is_epr(&clauses_owned);

    // FVO pre-pass: for clause sets where all predicate arguments are variables
    // (no equality, no function terms), the first-order problem is
    // propositionally equivalent.  Try a BFS propositional refutation first;
    // it solves problems like SYN938+1 in milliseconds where the regular
    // given-clause loop times out.
    {
        let mut fvo_id_gen = id_gen.clone();
        if let Some(result) =
            try_fvo_refutation(&clauses_owned, provenance, &mut fvo_id_gen, symbols)
        {
            return (
                result,
                crate::ScheduleReport {
                    workers: workers.unwrap_or_else(|| num_cpus::get_physical().max(1)),
                    ..crate::ScheduleReport::default()
                },
            );
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
        // Strict self-check requires a replayable SAT trace. CWA currently
        // emits a structural case-split certificate without the SAT manifest,
        // so let the regular AVATAR path produce the trace instead.
        let strict_trace = schedule
            .strategies
            .iter()
            .any(|(config, _)| config.emit_avatar_trace);
        let mut cwa_id_gen = id_gen.clone();
        if !strict_trace
            && let Some(result) = try_componentwise_refute(
                &clauses_owned,
                provenance,
                &mut cwa_id_gen,
                symbols_arc.clone(),
                default_config.clone(),
            )
        {
            return (
                result,
                crate::ScheduleReport {
                    workers: workers.unwrap_or_else(|| num_cpus::get_physical().max(1)),
                    ..crate::ScheduleReport::default()
                },
            );
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
    // SearchState (and the mrs_cadical::Solver it contains) is Send, but TermBank
    // is heavily thread-local (interning IDs are only valid locally). So each
    // thread constructs its own SearchState from the cloned clause data, but
    // we share a pool of globally discovered unit equalities via an RwLock.
    let stop_flag = Arc::new(AtomicBool::new(false));
    let shared_pool = Arc::new(std::sync::RwLock::new(Vec::<SharedClauseChain>::new()));
    let (tx, rx) = mpsc::channel::<(usize, SearchResult, crate::SearchStats, u64)>();

    // Default to the *physical* core count, matching the default that
    // `src/main.rs` uses when time-slicing the schedule itself
    // (`num_cpus::get_physical()`). Previously this fell back to
    // `std::thread::available_parallelism()` (logical/hyperthreaded core
    // count), which on any SMT-enabled machine disagreed with the
    // schedule's own assumed concurrency, causing more strategies to run
    // concurrently than intended and making wall-clock-sensitive search
    // heuristics (e.g. LRS pruning) non-reproducible between runs. Callers
    // that explicitly resolve and pass their own `workers` value (as
    // `src/main.rs` now does) are unaffected by this fallback.
    let available_cores = workers.unwrap_or_else(|| num_cpus::get_physical().max(1));
    let num_workers = available_cores.min(actual_configs.len());
    let next_strategy = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let has_epr = is_problem_epr;

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
            let provenance_for_thread = provenance.to_vec();
            let id_gen_thread = id_gen.clone();
            let config_thread = Arc::clone(&default_config);
            let symbols_thread = symbols_arc.clone();
            let actual_configs_ref = &actual_configs;

            #[cfg(feature = "ml-guidance")]
            let ml_model_thread = ml_model.clone();
            let log_ml_data_thread = ml.log_dir.clone();
            let premise_keep_thread = ml.premise_keep.clone();

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
                        sc.use_avatar = false;
                    }

                    let mut thread_clauses = clauses_for_thread.clone();
                    if let Some(tolerance) = sc.sine_tolerance {
                        if thread_clauses.len() > 100 {
                            let before_len = thread_clauses.len();
                            thread_clauses = crate::sine::filter_items(&thread_clauses, tolerance, sc.sine_depth_limit);
                            if thread_clauses.len() == before_len {
                                sc.sine_tolerance = None;
                            }
                        } else {
                            sc.sine_tolerance = None;
                        }
                    }

                    // Per-worker ML premise pruning: only the LAST
                    // ML_PRUNE_LAST_SLOTS strategy slots of the portfolio run
                    // on the ML-pruned axiom set; the rest run the full
                    // problem unpruned. See `is_ml_prune_slot` for rationale.
                    // Note this is a substitution, not a strict addition:
                    // with a fixed worker count equal to the portfolio size
                    // (CASC hardware is fixed at 8 cores), these slots do NOT
                    // also run unpruned elsewhere in the same schedule, so
                    // this does not guarantee mrs-ml can never solve fewer
                    // problems than the unpruned baseline — it only bounds
                    // how much coverage is put at risk.
                    let mut ml_pruned = false;
                    if let Some(keep) = &premise_keep_thread
                        && is_ml_prune_slot(strategy_idx, actual_configs_ref.len())
                    {
                        let before_len = thread_clauses.len();
                        thread_clauses.retain(|c| keep.contains(&c.id));
                        ml_pruned = thread_clauses.len() < before_len;
                    }

                    let mut thread_provenance = provenance_for_thread.clone();
                    let mut thread_symbols = (*symbols_thread).clone();
                    let mut thread_id_gen = id_gen_thread.clone();

                    if let Some(mode) = sc.goal_transformation {
                        let res = mrs_cnf::goal_transform::transform_goal_clauses(
                            &thread_clauses,
                            &mut thread_symbols,
                            &mut thread_id_gen,
                            mode,
                        );
                        if res.transformed {
                            thread_clauses = res.clauses;
                            thread_provenance.extend(res.provenance);
                        }
                    }

                    let thread_sym_config = match &sc.ordering {
                        TermOrdering::CustomKBO(cfg) | TermOrdering::CustomLPO(cfg) => cfg.clone(),
                        _ => config_thread.clone(),
                    };
                    let mut state = SearchState::new_with_ml(
                        thread_clauses,
                        thread_provenance,
                        thread_id_gen,
                        thread_sym_config,
                        Arc::new(thread_symbols),
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
                        SearchResult::Refutation(id, tstp_proof) => {
                            #[cfg(feature = "ml-guidance")]
                            if let Some(log_dir) = &state.log_ml_data {
                                let elapsed = schedule_start.elapsed().as_secs_f64();
                                if elapsed >= 0.5 && state.stats.processed >= 100 {
                                    let positive_ids = mrs_proof::extract::extract_proof_ids(id, &state.clause_store);
                                    let pos_set: std::collections::HashSet<_> = positive_ids.iter().copied().collect();

                                    let mut all_samples = Vec::new();

                                    for (&cid, clause) in &state.clause_store {
                                        let is_pos = pos_set.contains(&cid);

                                        // Negative subsampling: keep all positives, sample only from processed set
                                        if !is_pos
                                            && (state.unprocessed.contains(&cid)
                                                || rand::random::<f32>() > 0.1)
                                        {
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

                                // Paradigm B: Premise Selector Logging
                                    let conjectures: Vec<_> = state.clause_store.values()
                                        .filter(|c| matches!(c.source, mrs_core::clause::ClauseSource::Input { .. }) && c.distance == 0)
                                        .cloned()
                                        .collect();

                                    if !conjectures.is_empty() {
                                        let ctx = mrs_core::ml::premise_selector::ConjectureContext::new(&conjectures, &state.term_bank, symbols);
                                        let mut premise_samples = Vec::new();

                                        for axiom in state.clause_store.values() {
                                            if matches!(axiom.source, mrs_core::clause::ClauseSource::Input { .. }) && axiom.distance != 0 {
                                                let is_pos = pos_set.contains(&axiom.id);
                                                let label = if is_pos { 1.0 } else { 0.0 };
                                                let feats = mrs_core::ml::premise_selector::extract_premise_features(axiom, &ctx, &state.term_bank, symbols);
                                                premise_samples.push(mrs_core::ml::sample::PremiseSample { label, feats });
                                            }
                                        }

                                        let premise_file_stem = format!("{}_{}_premises", problem_name, strategy_idx);
                                        let premise_log_path = log_path.join("premise");
                                        std::fs::create_dir_all(&premise_log_path).ok();

                                        if state.ml_log_csv {
                                            if let Ok(mut w) = std::fs::File::create(premise_log_path.join(format!("{}.csv", premise_file_stem))) {
                                                use std::io::Write;
                                                for s in &premise_samples {
                                                    let feats_str = s.feats.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(",");
                                                    let _ = writeln!(w, "{},{}", s.label, feats_str);
                                                }
                                            }
                                        } else {
                                            if let Ok(mut w) = std::fs::File::create(premise_log_path.join(format!("{}.wincode", premise_file_stem))) {
                                                let mut std_write = wincode::io::std_write::WriteAdapter::new(&mut w);
                                                for s in &premise_samples {
                                                    let _ = wincode::serialize_into(&mut std_write, s);
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            let tstp = if tstp_proof.is_empty() {
                                let legacy_store: HashMap<_, _> = state
                                    .clause_store
                                    .iter()
                                    .map(|(&cid, ic)| (cid, state.term_bank.clause_to_legacy(ic)))
                                    .collect();
                                let proof = extract_proof(id, &legacy_store);
                                format_tstp(&proof, symbols)
                            } else {
                                tstp_proof
                            };
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
                        // SInE filtering drops axioms; saturating on a subset of the problem
                        // does not imply the full problem is satisfiable.
                        SearchResult::Saturated if sc.sine_tolerance.is_some() => SearchResult::GaveUp,
                        // Same for ML premise pruning: a worker that actually dropped
                        // axioms cannot claim Saturated for the full problem.
                        SearchResult::Saturated if ml_pruned => SearchResult::GaveUp,
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
        let mut report = crate::ScheduleReport {
            workers: num_workers,
            ..crate::ScheduleReport::default()
        };

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
        report.elapsed_ms = schedule_start.elapsed().as_millis() as u64;
        (best, report)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::clause::{ClauseIdGen, ClauseSource};
    use mrs_core::{Atom, Literal, SymbolTable, Term};

    #[test]
    fn ml_prune_slot_picks_last_two_of_eight() {
        // 8-strategy portfolio (the standard CASC 8-worker case): only
        // indices 6 and 7 (the last two, 0-based) are prune-eligible.
        for idx in 0..6 {
            assert!(
                !is_ml_prune_slot(idx, 8),
                "idx {idx} should not be a prune slot in an 8-strategy portfolio"
            );
        }
        assert!(is_ml_prune_slot(6, 8));
        assert!(is_ml_prune_slot(7, 8));
    }

    #[test]
    fn ml_prune_slot_handles_small_portfolios_without_underflow() {
        // Fewer strategies than ML_PRUNE_LAST_SLOTS must not panic/underflow;
        // every existing slot ends up eligible instead.
        assert!(is_ml_prune_slot(0, 1));
        assert!(is_ml_prune_slot(0, 2));
        assert!(is_ml_prune_slot(1, 2));
        // An empty portfolio has no valid strategy_idx to query in practice,
        // but the arithmetic must still not panic.
        assert!(is_ml_prune_slot(0, 0));
    }

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
    fn auto_schedule_detects_divisions() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let f = syms.intern("f");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();

        let fa = Term::app(f, vec![Term::constant(a)]);

        // EPR: predicates over constants/variables only.
        let epr = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
            "epr",
        );
        assert_eq!(auto_schedule_name(std::slice::from_ref(&epr)), "casc_epr");

        // UEQ: unit equalities with a real function term (not EPR).
        let ueq = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::Eq(fa.clone(), Term::constant(a)))],
            "ueq",
        );
        assert_eq!(auto_schedule_name(std::slice::from_ref(&ueq)), "casc_ueq");

        // Constants-only unit equality: UEQ wins over EPR.
        let ueq_const = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::Eq(Term::constant(a), Term::constant(a)))],
            "ueq_const",
        );
        assert_eq!(
            auto_schedule_name(std::slice::from_ref(&ueq_const)),
            "casc_ueq"
        );

        // FNE: function terms, no equality.
        let fne = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![fa.clone()]))],
            "fne",
        );
        assert_eq!(auto_schedule_name(std::slice::from_ref(&fne)), "casc_fne");

        // FEQ: mixed predicate + equality, non-unit.
        let feq = input_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::pred(p, vec![fa.clone()])),
                Literal::neg(Atom::Eq(fa, Term::constant(a))),
            ],
            "feq",
        );
        assert_eq!(auto_schedule_name(std::slice::from_ref(&feq)), "casc_feq");
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
            &[],
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
            &[],
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

    #[test]
    fn schedule_report_records_actual_worker_count() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();
        let c1 = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
            "ax1",
        );
        let schedule = StrategySchedule {
            strategies: vec![(
                SearchConfig {
                    time_limit: Duration::from_millis(50),
                    max_term_weight: None,
                    use_avatar: false,
                    ..SearchConfig::default()
                },
                Duration::from_millis(50),
            )],
        };
        let (_result, report) = run_schedule(
            &[c1],
            &[],
            id_gen,
            &schedule,
            &syms,
            MlOptions::default(),
            Some(1),
        );
        assert_eq!(report.workers, 1);
        assert!(report.elapsed_ms <= 1_000);
        assert_eq!(report.strategies.len(), 1);
    }
}
