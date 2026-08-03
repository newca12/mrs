//! Proof search engine with the given-clause loop.
//!
//! This crate implements the Otter-style given-clause algorithm:
//!
//! 1. Select a clause from the unprocessed set
//! 2. Generate all inferences with the processed set
//! 3. Add the clause to the processed set
//! 4. Add new clauses to the unprocessed set
//! 5. Repeat until empty clause found, saturated, or resource limit hit
//!
//! # Example
//!
//! ```
//! use std::sync::Arc;
//! use mrs_core::clause::ClauseIdGen;
//! use mrs_calculus::ordering::SymbolConfig;
//! use mrs_core::SymbolTable;
//! use mrs_search::{SearchConfig, SearchResult, SelectionStrategy};
//! use mrs_search::state::SearchState;
//! use mrs_search::given_clause::search;
//!
//! let id_gen = ClauseIdGen::new();
//! let config_arc = Arc::new(SymbolConfig::default());
//! let symbols_arc = Arc::new(SymbolTable::new());
//! let mut state = SearchState::new(vec![], id_gen, config_arc, symbols_arc, true);
//! let config = SearchConfig::default();
//! let result = search(&mut state, &config);
//! assert!(matches!(result, SearchResult::Saturated));
//! ```

pub(crate) use rustc_hash::FxHashMap as HashMap;
pub(crate) use rustc_hash::FxHashSet as HashSet;

pub mod avatar;
pub mod cwa;
pub mod fvo;
pub mod given_clause;
pub mod instgen;
pub mod select;
pub mod sine;
pub mod state;
pub mod strategy;
pub mod unprocessed;
pub mod weight;

use std::time::Duration;

use mrs_core::clause::ClauseId;

pub use mrs_calculus::literal_selection::LiteralSelection;
pub use mrs_calculus::ordering::TermOrdering;
pub use select::SelectionStrategy;

/// Per-strategy counters for failure diagnosis and throughput analysis.
///
/// All counters are for the **single** strategy that ran; `ScheduleReport`
/// aggregates them across the whole portfolio run.
#[derive(Clone, Debug, Default)]
pub struct SearchStats {
    /// Total given-clause loop iterations (includes skips).
    pub iterations: u64,
    /// Clauses added to the processed set.
    pub processed: u64,
    /// New clauses enqueued into the unprocessed set.
    pub generated: u64,
    /// Clauses rejected by the `max_term_weight` filter.
    pub weight_discarded: u64,
    /// Clauses deleted by forward subsumption.
    pub forward_subsumed: u64,
    /// Clauses remaining in the passive (unprocessed) queue when search ended.
    pub passive_size: u64,
    /// Clauses deleted by backward subsumption/demodulation.
    pub backward_deleted: u64,
    /// Clauses discarded by the Limited Resource Strategy (LRS) passive pruning.
    pub lrs_discarded: u64,
}

/// Summary for one strategy in the portfolio run.
#[derive(Clone, Debug)]
pub struct StrategyReport {
    /// Zero-based strategy index within the schedule.
    pub strategy_idx: usize,
    /// The result of this strategy's search.
    pub result: SearchResult,
    /// Counters collected during the search.
    pub stats: SearchStats,
    /// Wall-clock time this strategy ran (milliseconds).
    pub elapsed_ms: u64,
}

/// Aggregate report returned by [`strategy::run_schedule`] alongside
/// the winning `SearchResult`.
///
/// Contains one entry per strategy that actually ran (strategies that were
/// never launched because a winner was found first are absent).
#[derive(Clone, Debug, Default)]
pub struct ScheduleReport {
    pub strategies: Vec<StrategyReport>,
}

impl ScheduleReport {
    /// Human-readable one-line summary of the failure mode seen across all
    /// strategies.  Returns `None` when the search succeeded (Refutation).
    ///
    /// Used by `main.rs` to emit a `% SZS detail` line on stderr.
    pub fn failure_reason(&self) -> Option<String> {
        if self.strategies.is_empty() {
            return None;
        }
        // If any strategy found a refutation, there is no failure.
        if self
            .strategies
            .iter()
            .any(|s| matches!(s.result, SearchResult::Refutation(..)))
        {
            return None;
        }

        let total_processed: u64 = self.strategies.iter().map(|s| s.stats.processed).sum();
        let total_generated: u64 = self.strategies.iter().map(|s| s.stats.generated).sum();
        let total_passive: u64 = self.strategies.iter().map(|s| s.stats.passive_size).sum();
        let total_wt_disc: u64 = self
            .strategies
            .iter()
            .map(|s| s.stats.weight_discarded)
            .sum();
        let total_lrs_disc: u64 = self.strategies.iter().map(|s| s.stats.lrs_discarded).sum();
        let total_fwd_sub: u64 = self
            .strategies
            .iter()
            .map(|s| s.stats.forward_subsumed)
            .sum();

        // Count how many strategies reached each final state.
        let n_timeout = self
            .strategies
            .iter()
            .filter(|s| matches!(s.result, SearchResult::Timeout))
            .count();
        let n_saturated = self
            .strategies
            .iter()
            .filter(|s| matches!(s.result, SearchResult::Saturated))
            .count();

        Some(format!(
            "strategies={} timeout={} saturated={} \
             processed={} generated={} passive={} weight_discarded={} lrs_discarded={} fwd_subsumed={}",
            self.strategies.len(),
            n_timeout,
            n_saturated,
            total_processed,
            total_generated,
            total_passive,
            total_wt_disc,
            total_lrs_disc,
            total_fwd_sub,
        ))
    }
}

/// Result of a proof search.
#[derive(Clone, Debug)]
pub enum SearchResult {
    /// A refutation was found. Contains the ID of the empty clause and the proof TSTP string.
    Refutation(ClauseId, String),
    /// All clauses were processed without finding a contradiction.
    Saturated,
    /// The time limit was exceeded.
    Timeout,
    /// The search gave up (e.g. saturated with an incomplete strategy).
    GaveUp,
}

/// How clause weights are computed for the passive-queue priority heaps.
///
/// All variants are sums over all symbol occurrences in all literals; they
/// differ in how individual occurrences are weighted:
///
/// * `Standard`     — every symbol costs 1, every variable costs `w0`.
///   This is the default and reproduces the historical behaviour.
///
/// * `FunctionDepth` — terms are weighted by `symbol_weight * (depth + 1)`.
///   Deeply nested terms become heavier, discouraging the prover from
///   building tall term towers during superposition chains.
///
/// * `HornPenalty`  — same as Standard, but clauses with more than one
///   positive literal pay a 3× multiplier penalty.  Horn clauses (≤1
///   positive literal) are preferred, which helps on FNE / mixed problems.
///
/// * `ConjSymbolBoost` — counts symbols that *also appear in a
///   goal-connected clause* (distance < 100) as weight 1; symbols that
///   do not appear in any goal clause are penalised (weight 3).
///   This approximates E's "prefer clauses that share symbols with the
///   conjecture" heuristic.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ClauseWeightFn {
    /// Every symbol costs 1, variable costs `w0`.  (default)
    #[default]
    Standard,
    /// Depth-weighted: heavier for deeply nested terms.
    FunctionDepth,
    /// Quadratic depth-weighted: heavier for deeply nested terms with quadratic scaling.
    FunctionWeightPenalty,
    /// Exponential depth-weighted: extremely heavy for deeply nested terms with exponential scaling.
    FunctionWeightPenaltyExp,
    /// Horn preference: non-Horn clauses pay a 3× multiplier.
    HornPenalty,
    /// Horn progressive multiplier: non-Horn clauses pay a multiplier equal to positive literals.
    HornHeuristic,
    /// Horn exponential multiplier: non-Horn clauses pay a 2^(pos_count - 1) multiplier.
    HornHeuristicExp,
    /// Goal-symbol boost: symbols not in the conjecture closure are 3×.
    ConjSymbolBoost,
    /// Precedence-based symbol weight: each symbol's cost equals its KBO/LPO
    /// precedence rank.  Rare symbols have higher precedence and therefore cost
    /// more, so clauses that contain many rare symbols are treated as heavier
    /// and processed later.  This nudges the prover toward clauses whose
    /// vocabulary is dominated by common (low-precedence) symbols, which tend
    /// to interact well with the rewrite rules already in the active set.
    ///
    /// Note: the effect is complementary to `ConjSymbolBoost`.  Whereas
    /// `ConjSymbolBoost` rewards goal-symbol overlap, `SymbolWeight` penalises
    /// rare symbols regardless of whether they appear in the conjecture.
    SymbolWeight,
}

/// Configuration for the search engine.
#[derive(Clone, Debug)]
pub struct SearchConfig {
    /// Maximum wall-clock time for the search.
    pub time_limit: Duration,
    /// Clause selection strategy.
    pub selection: SelectionStrategy,
    /// Literal selection strategy for inference restriction.
    pub literal_selection: LiteralSelection,
    /// Term ordering for orienting equalities.
    pub ordering: TermOrdering,
    /// Maximum total symbol weight of any generated clause.
    ///
    /// Inferred clauses whose total weight (sum of all symbol occurrences
    /// across all literals) exceeds this limit are discarded immediately.
    /// This prevents unbounded term growth during superposition.
    /// `None` means no limit.
    pub max_term_weight: Option<u32>,
    /// Whether to enable AVATAR clause splitting via an embedded SAT solver.
    pub use_avatar: bool,
    /// Emit a replayable CaDiCaL SAT trace for the final AVATAR certificate.
    /// Disabled by default because trace generation is a verification cost.
    pub emit_avatar_trace: bool,
    /// If true, only generate resolvents where at least one parent is a unit
    /// (single-literal clause).  This restricts the inference to unit resolution,
    /// which dramatically reduces passive-set growth on FNE-encoded problems whose
    /// proofs consist entirely of unit-chain derivations.  The restriction is
    /// incomplete for general clause sets but correct (sound) everywhere.
    pub unit_only_resolution: bool,
    /// Weight function used when inserting clauses into the passive-queue heaps.
    ///
    /// Defaults to `Standard` (unchanged historical behaviour).
    pub weight_fn: ClauseWeightFn,
    /// Set-of-Support (SOS) restriction.
    ///
    /// When `true`, the weight-based priority queue only offers clauses whose
    /// `distance` is below this threshold for the *weight* pop.  Age picks
    /// (FIFO) are unrestricted.  Setting this to `u32::MAX` disables SOS
    /// (equivalent to `false`).
    ///
    /// Suggested value: `100` — keeps all conjecture descendants (distance 0–99)
    /// in the SOS.  Axiom-only clauses (distance ≥ 100) are still reachable via
    /// the age queue slot of AgeWeight/GoalDirected strategies.
    pub sos_depth: u32,
    /// Enable the ordered-inference maximal-literal restriction. EXPERIMENTAL
    /// and **off by default**: the current implementation is refutationally
    /// INCOMPLETE (it caused false `Satisfiable` verdicts on EPR problems,
    /// e.g. SYN861/862/866), because the predicate-atom ordering it uses is not
    /// a sound literal ordering. Kept behind the flag for future, correct work.
    /// Enable for experiments via the `MRS_ORDERED` env var.
    pub ordered_inferences: bool,
    /// SInE tolerance level. `None` means SInE is disabled.
    pub sine_tolerance: Option<f64>,
    /// SInE depth limit.
    pub sine_depth_limit: Option<usize>,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            time_limit: Duration::from_secs(5),
            selection: SelectionStrategy::AgeWeight(5),
            literal_selection: LiteralSelection::AllNegative,
            ordering: TermOrdering::KBO,
            max_term_weight: Some(200),
            use_avatar: true,
            emit_avatar_trace: false,
            unit_only_resolution: false,
            weight_fn: ClauseWeightFn::Standard,
            sos_depth: u32::MAX, // disabled
            ordered_inferences: true,
            sine_tolerance: None,
            sine_depth_limit: None,
        }
    }
}
