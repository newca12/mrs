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
//! assert!(matches!(result, SearchResult::GaveUp));
//! ```

pub(crate) use rustc_hash::FxHashMap as HashMap;
pub(crate) use rustc_hash::FxHashSet as HashSet;

pub mod avatar;
pub(crate) mod certified;
pub(crate) mod certified_eq;
pub(crate) mod certified_sat;
pub mod cwa;
pub mod der;
pub mod fvo;
pub mod given_clause;
pub mod goal_distance;
pub mod instgen;
pub mod preprocessing;
pub mod resource;
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

pub use instgen::{
    InstGenTelemetry, classify_epr_profile, is_epr, is_pure_relational_epr, try_instgen_epr,
    try_instgen_epr_with_telemetry,
};
pub use mrs_calculus::literal_selection::LiteralSelection;
pub use mrs_calculus::ordering::TermOrdering;
pub use mrs_cnf::goal_transform::GoalTransformMode;
pub use preprocessing::{PreprocessingConfig, PreprocessingStats, preprocess_clauses};
pub use resource::{ResourceLimits, current_memory_mb, system_memory_limit_mb};
pub use select::{QueueType, SelectionStrategy};
pub use strategy::{CandidateReceiver, CandidateRefutation, run_schedule_with_candidate_receiver};
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
    /// Base strategy ID (1-based) when this schedule came from the named CASC
    /// portfolio; zero for ad-hoc schedules that do not assign one.
    pub strategy_id: usize,
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
    /// Telemetry collected from the InstGen pre-pass, if run.
    pub instgen: Option<InstGenTelemetry>,
    /// Certification tier that produced this schedule's result (`1`, `2`,
    /// or `3`), set only by the `--certify-ordered` path on success.
    /// Lets benchmark harnesses attribute coverage without TRACE output.
    pub cert_tier: Option<String>,
    /// Ordering the certifier ran under (`kbo`, `lpo`, or `ac-kbo`),
    /// recorded whenever the `--certify-ordered` path runs — including
    /// fail-closed runs, so remote analysis can tabulate attempts.
    pub cert_ordering: Option<String>,
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
            .filter(|s| matches!(s.result, SearchResult::Saturated(_)))
            .count();

        let mut detail = format!(
            "strategies={} workers={} strategy_ids={} result={} elapsed_ms={} timeout={} saturated={} \
             processed={} generated={} passive={} weight_discarded={} lrs_discarded={} \
             fwd_subsumed={} shared_published={} shared_imported={}",
            self.strategies.len(),
            self.workers,
            {
                let mut strategy_ids = self
                    .strategies
                    .iter()
                    .map(|s| (s.strategy_idx, s.strategy_id))
                    .collect::<Vec<_>>();
                strategy_ids.sort_unstable_by_key(|(slot, _)| *slot);
                strategy_ids
                    .into_iter()
                    .map(|(slot, id)| format!("{slot}:{id}"))
                    .collect::<Vec<_>>()
                    .join(";")
            },
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
        );

        if let Some(ig) = &self.instgen {
            detail.push_str(&format!(
                " instgen_route={} instgen_rounds={} instgen_instances={} instgen_vars={} instgen_clauses={} instgen_ms={}",
                ig.route, ig.rounds, ig.generated_instances, ig.sat_vars, ig.sat_clauses, ig.elapsed_ms
            ));
            if let Some(reason) = ig.fallback_reason {
                detail.push_str(&format!(" instgen_fallback={}", reason));
            }
        }

        if let Some(tier) = &self.cert_tier {
            detail.push_str(&format!(" cert_tier={tier}"));
        }
        if let Some(ordering) = &self.cert_ordering {
            detail.push_str(&format!(" cert_ordering={ordering}"));
        }

        detail
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
        let n_resource_out = self
            .strategies
            .iter()
            .filter(|s| matches!(s.result, SearchResult::ResourceOut))
            .count();
        let n_saturated = self
            .strategies
            .iter()
            .filter(|s| matches!(s.result, SearchResult::Saturated(_)))
            .count();

        Some(format!(
            "strategies={} timeout={} resource_out={} saturated={} \
             processed={} generated={} passive={} weight_discarded={} lrs_discarded={} fwd_subsumed={}",
            self.strategies.len(),
            n_timeout,
            n_resource_out,
            n_saturated,
            total_processed,
            total_generated,
            total_passive,
            total_wt_disc,
            total_lrs_disc,
            total_fwd_sub,
        ))
    }

    /// Returns the raw search result seen across all strategies in the schedule,
    /// before any post-search certification filtering.
    pub fn raw_search_result(&self) -> SearchResult {
        for s in &self.strategies {
            if matches!(s.result, SearchResult::Refutation(..)) {
                return s.result.clone();
            }
        }
        for s in &self.strategies {
            if matches!(s.result, SearchResult::Saturated(_)) {
                return s.result.clone();
            }
        }
        for s in &self.strategies {
            if matches!(s.result, SearchResult::ResourceOut) {
                return s.result.clone();
            }
        }
        for s in &self.strategies {
            if matches!(s.result, SearchResult::GaveUp) {
                return s.result.clone();
            }
        }
        SearchResult::Timeout
    }
}

/// Detailed reason why a search configuration cannot soundly claim saturation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IncompletenessReason {
    /// Generated clauses were discarded by a maximum term-weight cap.
    MaxTermWeightDiscarded {
        /// Configured maximum clause weight.
        cap: u32,
        /// Number of discarded generated clauses.
        discarded: u64,
    },
    /// Set-of-Support restricts inference generation.
    SosRestricted(u32),
    /// Only unit-resolution inferences are generated.
    UnitOnlyResolution,
    /// SInE removed input axioms.
    SineFiltered,
    /// ML premise pruning removed input axioms.
    MlPremisePruned,
    /// A non-standard clause weight changes the simplification/search order.
    NonStandardWeightFn,
    /// Literal selection does not preserve complete inference generation.
    IncompleteLiteralSelection,
    /// LRS discarded passive clauses.
    LrsDiscarded(u64),
    /// Ordered maximal-literal restriction is not certified for this engine.
    OrderedInferenceRestriction,
}

/// Category of completeness evidence carried by a saturation result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaturationReason {
    /// A purely ground clause set was checked by the exact ground model path.
    Ground,
    /// A finite ground ordered-resolution closure agreed with its unrestricted reference closure.
    GroundOrderedResolution,
    /// A large EPR grounding was decided satisfiable by CaDiCaL and the
    /// model was independently re-verified clause by clause (Tier 2,
    /// SAT-direction only; unsatisfiable outcomes fail closed).
    SatBackedGrounding,
}

/// Evidence that a saturation result was produced by a complete search path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletenessWitness {
    reason: SaturationReason,
}

impl CompletenessWitness {
    /// Create evidence for the exact ground model path.
    pub(crate) fn ground() -> Self {
        Self {
            reason: SaturationReason::Ground,
        }
    }

    /// Create evidence for the bounded ground ordered-resolution certificate.
    pub(crate) fn ground_ordered_resolution() -> Self {
        Self {
            reason: SaturationReason::GroundOrderedResolution,
        }
    }

    /// Create evidence for the SAT-backed Tier-2 satisfiability certificate.
    pub(crate) fn sat_backed_grounding() -> Self {
        Self {
            reason: SaturationReason::SatBackedGrounding,
        }
    }

    /// Return the evidence category.
    pub fn reason(&self) -> SaturationReason {
        self.reason
    }
}

/// Result of a proof search.
#[derive(Clone, Debug)]
pub enum SearchResult {
    /// A refutation was found. Contains the ID of the empty clause and the proof TSTP string.
    Refutation(ClauseId, String),
    /// All clauses were processed without finding a contradiction and the
    /// search path supplied completeness evidence.
    Saturated(CompletenessWitness),
    /// The time limit was exceeded.
    Timeout,
    /// The search gave up (e.g. saturated with an incomplete strategy).
    GaveUp,
    /// The search exceeded resource limits (memory watchdog, clause or term ceiling).
    ResourceOut,
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
    /// Disable LRS passive queue pruning completely.
    Disabled,
}

/// A proof-preserving unit-equality chain shared between portfolio workers.
#[derive(Clone, Debug)]
pub struct SharedClauseChain {
    /// Logical epoch in which the chain was published.
    pub epoch: u64,
    /// Stable content key used for deterministic ordering and deduplication.
    pub key: String,
    /// Publisher symbol names in `SymbolId` index order.  Workers own private
    /// symbol tables, so raw symbol IDs cannot be shared without this mapping.
    pub symbol_names: Vec<String>,
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
    /// Base strategy identity for benchmark telemetry; zero means ad hoc.
    pub strategy_id: usize,
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
    /// Run the bounded, independently cross-checked ground ordered-resolution
    /// certifier instead of the heuristic given-clause path. Unsupported
    /// non-ground/equality inputs fail closed as `GaveUp`.
    pub certify_ordered_inferences: bool,
    /// SInE tolerance level. `None` means SInE is disabled.
    pub sine_tolerance: Option<f64>,
    /// SInE depth limit.
    pub sine_depth_limit: Option<usize>,
    /// LRS target calculation policy.
    pub lrs_policy: LrsPolicy,
    /// Optional Twee-style goal-directed preprocessing transformation.
    pub goal_transformation: Option<GoalTransformMode>,
    /// Number of given-clause iterations between shared-pool polls.
    /// `0` disables cross-strategy clause sharing (the default: every named
    /// `casc_*` schedule inherits this unless it sets an explicit interval).
    /// Set `MRS_SHARED_POOL_INTERVAL=<N>` to re-enable sharing experimentally.
    pub shared_pool_poll_interval: u64,
    /// Scheme used to compute problem-specific symbol precedence for reduction orderings.
    pub precedence_scheme: PrecedenceScheme,
    /// Scheme used to compute symbol weights for reduction orderings (KBO).
    pub symbol_weight_scheme: SymbolWeightScheme,
    /// Resource containment limits (clause ceilings, term bank ceiling, memory watchdog).
    pub resource_limits: ResourceLimits,
}

impl SearchConfig {
    /// Whether ordered inference is enabled for this search, including the
    /// process-wide diagnostic override.
    pub fn ordered_inferences_enabled(&self) -> bool {
        effective_ordered_inferences(
            self.ordered_inferences,
            std::env::var_os("MRS_ORDERED").is_some(),
        )
    }

    /// Check whether this configuration avoids known incomplete restrictions.
    /// This is only a configuration audit; it is not by itself a completeness
    /// proof for the full given-clause implementation.
    pub fn check_completeness(
        &self,
        weight_discarded: u64,
        lrs_discarded: u64,
        ml_pruned: bool,
    ) -> Result<(), IncompletenessReason> {
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
        if self.ordered_inferences_enabled() {
            return Err(IncompletenessReason::OrderedInferenceRestriction);
        }
        Ok(())
    }
}

fn effective_ordered_inferences(configured: bool, environment_override: bool) -> bool {
    configured || environment_override
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            strategy_id: 0,
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
            // Ordered maximal-literal inference remains available for
            // refutation search, but is not certified for positive results.
            ordered_inferences: false,
            certify_ordered_inferences: false,
            sine_tolerance: None,
            sine_depth_limit: None,
            goal_transformation: None,
            lrs_policy: LrsPolicy::WallClock,
            // Default: no cross-strategy clause sharing. Sharing stays
            // available opt-in via `MRS_SHARED_POOL_INTERVAL=<N>` or an
            // explicit per-schedule `shared_pool_poll_interval`.
            shared_pool_poll_interval: 0,
            precedence_scheme: PrecedenceScheme::InvFreq,
            symbol_weight_scheme: SymbolWeightScheme::Uniform,
            resource_limits: ResourceLimits::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completeness_audit_accepts_unpruned_default_configuration() {
        let config = SearchConfig::default();
        config
            .check_completeness(0, 0, false)
            .expect("default configuration should pass the configuration audit");
    }

    #[test]
    fn completeness_audit_rejects_ordered_inference() {
        let config = SearchConfig {
            ordered_inferences: true,
            ..SearchConfig::default()
        };
        assert_eq!(
            config.check_completeness(0, 0, false),
            Err(IncompletenessReason::OrderedInferenceRestriction)
        );
    }

    #[test]
    fn effective_ordered_inference_includes_environment_override() {
        assert!(!effective_ordered_inferences(false, false));
        assert!(effective_ordered_inferences(true, false));
        assert!(effective_ordered_inferences(false, true));
        assert!(effective_ordered_inferences(true, true));
    }

    #[test]
    fn completeness_audit_rejects_each_known_pruning_source() {
        let base = SearchConfig {
            max_term_weight: Some(10),
            ..SearchConfig::default()
        };
        assert_eq!(
            base.check_completeness(1, 0, false),
            Err(IncompletenessReason::MaxTermWeightDiscarded {
                cap: 10,
                discarded: 1,
            })
        );

        let sos = SearchConfig {
            sos_depth: 10,
            ..base.clone()
        };
        assert_eq!(
            sos.check_completeness(0, 0, false),
            Err(IncompletenessReason::SosRestricted(10))
        );

        let unit_only = SearchConfig {
            unit_only_resolution: true,
            ..base.clone()
        };
        assert_eq!(
            unit_only.check_completeness(0, 0, false),
            Err(IncompletenessReason::UnitOnlyResolution)
        );

        let sine = SearchConfig {
            sine_tolerance: Some(2.0),
            ..base.clone()
        };
        assert_eq!(
            sine.check_completeness(0, 0, false),
            Err(IncompletenessReason::SineFiltered)
        );

        assert_eq!(
            base.check_completeness(0, 0, true),
            Err(IncompletenessReason::MlPremisePruned)
        );
        assert_eq!(
            base.check_completeness(0, 42, false),
            Err(IncompletenessReason::LrsDiscarded(42))
        );
        assert_eq!(
            SearchConfig {
                weight_fn: ClauseWeightFn::FunctionDepth,
                ..base.clone()
            }
            .check_completeness(0, 0, false),
            Err(IncompletenessReason::NonStandardWeightFn)
        );
        assert_eq!(
            SearchConfig {
                literal_selection: LiteralSelection::MaxNegativeOrMaxPositive,
                ..base
            }
            .check_completeness(0, 0, false),
            Err(IncompletenessReason::IncompleteLiteralSelection)
        );
    }
}
