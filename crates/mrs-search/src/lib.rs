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
//! assert!(matches!(result, SearchResult::Saturated(..)));
//! ```

pub(crate) use rustc_hash::FxHashMap as HashMap;
pub(crate) use rustc_hash::FxHashSet as HashSet;

pub mod avatar;
pub mod cwa;
pub mod der;
pub mod fvo;
pub mod given_clause;
pub mod goal_distance;
pub mod instgen;
pub mod preprocessing;
pub mod select;
pub mod sine;
pub mod state;
pub mod strategy;
pub mod symbol_config;
pub mod unprocessed;
pub mod weight;

use std::time::Duration;

use mrs_core::clause::{Clause, ClauseId};

pub use goal_distance::GoalDistanceMap;

pub use mrs_calculus::literal_selection::LiteralSelection;
pub use mrs_calculus::ordering::TermOrdering;
pub use mrs_cnf::goal_transform::GoalTransformMode;
pub use preprocessing::{PreprocessingConfig, PreprocessingStats, preprocess_clauses};
pub use select::{QueueType, SelectionStrategy};
pub use symbol_config::{PrecedenceScheme, SymbolWeightScheme, compute_symbol_config};

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
    /// Shared unit-equality chains published by this strategy.
    pub shared_published: u64,
    /// Shared unit-equality chains imported by this strategy.
    pub shared_imported: u64,
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
    /// Number of search workers actually spawned for the schedule.
    pub workers: usize,
    /// Wall-clock duration of the schedule run in milliseconds.
    pub elapsed_ms: u64,
    pub strategies: Vec<StrategyReport>,
}

impl ScheduleReport {
    /// Return stable machine-readable telemetry for one completed schedule.
    pub fn telemetry_detail(&self, result: &str) -> String {
        let total_processed: u64 = self.strategies.iter().map(|s| s.stats.processed).sum();
        let total_generated: u64 = self.strategies.iter().map(|s| s.stats.generated).sum();
        let total_passive: u64 = self.strategies.iter().map(|s| s.stats.passive_size).sum();
        let total_weight_discarded: u64 = self
            .strategies
            .iter()
            .map(|s| s.stats.weight_discarded)
            .sum();
        let total_lrs_discarded: u64 = self.strategies.iter().map(|s| s.stats.lrs_discarded).sum();
        let total_forward_subsumed: u64 = self
            .strategies
            .iter()
            .map(|s| s.stats.forward_subsumed)
            .sum();
        let total_shared_published: u64 = self
            .strategies
            .iter()
            .map(|s| s.stats.shared_published)
            .sum();
        let total_shared_imported: u64 = self
            .strategies
            .iter()
            .map(|s| s.stats.shared_imported)
            .sum();
        let timeout = self
            .strategies
            .iter()
            .filter(|s| matches!(s.result, SearchResult::Timeout))
            .count();
        let saturated = self
            .strategies
            .iter()
            .filter(|s| matches!(s.result, SearchResult::Saturated(..)))
            .count();

        format!(
            "strategies={} workers={} result={} elapsed_ms={} timeout={} saturated={} \
             processed={} generated={} passive={} weight_discarded={} lrs_discarded={} \
             fwd_subsumed={} shared_published={} shared_imported={}",
            self.strategies.len(),
            self.workers,
            result,
            self.elapsed_ms,
            timeout,
            saturated,
            total_processed,
            total_generated,
            total_passive,
            total_weight_discarded,
            total_lrs_discarded,
            total_forward_subsumed,
            total_shared_published,
            total_shared_imported,
        )
    }

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
            .filter(|s| matches!(s.result, SearchResult::Saturated(..)))
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

/// Detailed reason why a search strategy or run is refutationally incomplete and cannot claim
/// `SearchResult::Saturated`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IncompletenessReason {
    /// Max term weight cap is set and generated clauses were discarded.
    MaxTermWeightDiscarded {
        /// The max term weight cap configured.
        cap: u32,
        /// The number of generated clauses discarded because they exceeded the cap.
        discarded: u64,
    },
    /// Set-of-Support (SOS) depth restriction is active.
    SosRestricted(u32),
    /// Unit-only resolution restriction is active.
    UnitOnlyResolution,
    /// SInE axiom filtering dropped input axioms.
    SineFiltered,
    /// ML premise pruning discarded input axioms.
    MlPremisePruned,
    /// Non-standard clause weight function can alter simplification order.
    NonStandardWeightFn,
    /// Incomplete literal selection strategy.
    IncompleteLiteralSelection,
    /// Passive queue pruned by LRS (Limited Resource Strategy).
    LrsDiscarded(u64),
    /// Unsound or incomplete model abstraction.
    IncompleteModelAbstraction,
}

/// High-level justification for why a saturation result is sound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaturationReason {
    /// Purely ground clause set saturated without contradictions.
    Ground,
    /// Unpruned saturation under standard complete first-order superposition/resolution calculus.
    FirstOrderSuperposition,
    /// Verified finite Herbrand model.
    FiniteModel,
}

/// A witness certifying that a saturation result was derived by a sound and refutationally
/// complete calculus/search configuration without pruning or incomplete heuristics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletenessWitness {
    reason: SaturationReason,
}

impl CompletenessWitness {
    /// Create a witness for a verified ground saturation.
    pub fn ground() -> Self {
        Self {
            reason: SaturationReason::Ground,
        }
    }

    /// Create a witness for first-order superposition saturation under a complete configuration.
    pub fn first_order_superposition() -> Self {
        Self {
            reason: SaturationReason::FirstOrderSuperposition,
        }
    }

    /// Create a witness for a verified finite model.
    pub fn finite_model() -> Self {
        Self {
            reason: SaturationReason::FiniteModel,
        }
    }

    /// The specific justification category for this completeness witness.
    pub fn reason(&self) -> SaturationReason {
        self.reason
    }
}

/// Result of a proof search.
#[derive(Clone, Debug)]
pub enum SearchResult {
    /// A refutation was found. Contains the ID of the empty clause and the proof TSTP string.
    Refutation(ClauseId, String),
    /// All clauses were processed without finding a contradiction, certified by a CompletenessWitness.
    Saturated(CompletenessWitness),
    /// The time limit was exceeded.
    Timeout,
    /// The search gave up (e.g. saturated with an incomplete strategy).
    GaveUp,
}

/// Policy used by the Limited Resource Strategy (LRS) passive-queue pruner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LrsPolicy {
    /// Estimate remaining iterations from wall-clock throughput.
    #[default]
    WallClock,
    /// Use a fixed logical iteration budget, independent of CPU scheduling.
    FixedIterations {
        /// Total logical iterations available to the search.
        budget: u64,
    },
}

/// A proof-preserving unit-equality chain shared between portfolio workers.
#[derive(Clone, Debug)]
pub struct SharedClauseChain {
    /// Logical epoch in which the chain was published.
    pub epoch: u64,
    /// Stable content key used for deterministic ordering and deduplication.
    pub key: String,
    /// Ancestor chain, with the shared unit equality last.
    pub chain: Vec<Clause>,
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
    /// Goal distance: scales the clause weight by its distance from the conjecture
    /// in the symbol reachability graph and derivation DAG.
    GoalDistance,
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
    /// LRS target calculation policy.
    pub lrs_policy: LrsPolicy,
    /// Optional Twee-style goal-directed preprocessing transformation.
    pub goal_transformation: Option<GoalTransformMode>,
    /// Number of given-clause iterations between shared-pool polls.
    /// `0` disables cross-strategy clause sharing.
    pub shared_pool_poll_interval: u64,
    /// Scheme used to compute problem-specific symbol precedence for reduction orderings.
    pub precedence_scheme: PrecedenceScheme,
    /// Scheme used to compute symbol weights for reduction orderings (KBO).
    pub symbol_weight_scheme: SymbolWeightScheme,
}

impl SearchConfig {
    /// Checks whether this configuration together with the current search state is refutationally
    /// complete and capable of producing a sound `CompletenessWitness`.
    ///
    /// If any incomplete pruning, heuristic filtering, or non-standard ordering is active,
    /// returns `Err(IncompletenessReason)`.
    pub fn check_completeness(
        &self,
        weight_discarded: u64,
        lrs_discarded: u64,
        ml_pruned: bool,
    ) -> Result<CompletenessWitness, IncompletenessReason> {
        if let Some(cap) = self.max_term_weight
            && weight_discarded > 0
        {
            return Err(IncompletenessReason::MaxTermWeightDiscarded {
                cap,
                discarded: weight_discarded,
            });
        }
        if self.sos_depth < u32::MAX {
            return Err(IncompletenessReason::SosRestricted(self.sos_depth));
        }
        if self.unit_only_resolution {
            return Err(IncompletenessReason::UnitOnlyResolution);
        }
        if self.sine_tolerance.is_some() {
            return Err(IncompletenessReason::SineFiltered);
        }
        if ml_pruned {
            return Err(IncompletenessReason::MlPremisePruned);
        }
        if self.weight_fn != ClauseWeightFn::Standard {
            return Err(IncompletenessReason::NonStandardWeightFn);
        }
        if matches!(
            self.literal_selection,
            LiteralSelection::MaxNegativeOrMaxPositive
        ) {
            return Err(IncompletenessReason::IncompleteLiteralSelection);
        }
        if lrs_discarded > 0 {
            return Err(IncompletenessReason::LrsDiscarded(lrs_discarded));
        }
        Ok(CompletenessWitness::first_order_superposition())
    }
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
            goal_transformation: None,
            lrs_policy: LrsPolicy::WallClock,
            shared_pool_poll_interval: 500,
            precedence_scheme: PrecedenceScheme::InvFreq,
            symbol_weight_scheme: SymbolWeightScheme::Uniform,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_completeness_clean_configuration() {
        let config = SearchConfig {
            max_term_weight: None,
            ..SearchConfig::default()
        };
        let witness = config.check_completeness(0, 0, false);
        assert!(witness.is_ok());
        assert_eq!(
            witness.unwrap().reason(),
            SaturationReason::FirstOrderSuperposition
        );
    }

    #[test]
    fn test_check_completeness_max_term_weight() {
        let config = SearchConfig {
            max_term_weight: Some(100),
            ..SearchConfig::default()
        };
        // Zero discarded: not incomplete
        assert!(config.check_completeness(0, 0, false).is_ok());

        // Clauses discarded: incomplete
        let res = config.check_completeness(3, 0, false);
        assert_eq!(
            res,
            Err(IncompletenessReason::MaxTermWeightDiscarded {
                cap: 100,
                discarded: 3
            })
        );
    }

    #[test]
    fn test_check_completeness_sos_restricted() {
        let config = SearchConfig {
            sos_depth: 5,
            ..SearchConfig::default()
        };
        assert_eq!(
            config.check_completeness(0, 0, false),
            Err(IncompletenessReason::SosRestricted(5))
        );
    }

    #[test]
    fn test_check_completeness_unit_only_resolution() {
        let config = SearchConfig {
            unit_only_resolution: true,
            ..SearchConfig::default()
        };
        assert_eq!(
            config.check_completeness(0, 0, false),
            Err(IncompletenessReason::UnitOnlyResolution)
        );
    }

    #[test]
    fn test_check_completeness_sine_filtered() {
        let config = SearchConfig {
            sine_tolerance: Some(1.2),
            ..SearchConfig::default()
        };
        assert_eq!(
            config.check_completeness(0, 0, false),
            Err(IncompletenessReason::SineFiltered)
        );
    }

    #[test]
    fn test_check_completeness_ml_pruned() {
        let config = SearchConfig::default();
        assert_eq!(
            config.check_completeness(0, 0, true),
            Err(IncompletenessReason::MlPremisePruned)
        );
    }

    #[test]
    fn test_check_completeness_non_standard_weight_fn() {
        let config = SearchConfig {
            weight_fn: ClauseWeightFn::HornHeuristic,
            ..SearchConfig::default()
        };
        assert_eq!(
            config.check_completeness(0, 0, false),
            Err(IncompletenessReason::NonStandardWeightFn)
        );
    }

    #[test]
    fn test_check_completeness_incomplete_literal_selection() {
        let config = SearchConfig {
            literal_selection: LiteralSelection::MaxNegativeOrMaxPositive,
            ..SearchConfig::default()
        };
        assert_eq!(
            config.check_completeness(0, 0, false),
            Err(IncompletenessReason::IncompleteLiteralSelection)
        );
    }

    #[test]
    fn test_check_completeness_lrs_discarded() {
        let config = SearchConfig::default();
        assert_eq!(
            config.check_completeness(0, 42, false),
            Err(IncompletenessReason::LrsDiscarded(42))
        );
    }

    #[test]
    fn test_witness_constructors() {
        assert_eq!(
            CompletenessWitness::ground().reason(),
            SaturationReason::Ground
        );
        assert_eq!(
            CompletenessWitness::first_order_superposition().reason(),
            SaturationReason::FirstOrderSuperposition
        );
        assert_eq!(
            CompletenessWitness::finite_model().reason(),
            SaturationReason::FiniteModel
        );
    }
}
