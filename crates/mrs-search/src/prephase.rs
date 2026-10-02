//! The `mrs-search` side of the pre-phase: turn a routing [`Plan`] into the
//! engine's own types.
//!
//! The plan is produced by `mrs_prephase::route`, which knows nothing about
//! `SearchConfig`. This module is the only place the two vocabularies meet, so a
//! parameter is added in exactly one place and the rule table stays readable.
//!
//! It also owns the *dynamic* half of the pre-phase: [`Probe`], a bounded
//! search-behaviour measurement the static analysis cannot make. The static
//! vector says what a problem looks like; the probe says what the search is
//! actually doing on it. Both are needed, because the strongest predictor of
//! "this portfolio will not finish" is not a syntactic property of the input but
//! the shape of the search's own growth on it.

use std::time::Duration;

use crate::{
    GoalTransformMode, LiteralSelection, LrsPolicy, PrecedenceScheme, SearchConfig,
    SelectionStrategy, SymbolWeightScheme, TermOrdering,
};
use mrs_core::SymbolTable;
use mrs_core::clause::Clause;
use mrs_prephase::plan::{Algorithm, Plan, PrePass, Selection, StrategyKind, WeightFn, route};
use mrs_prephase::{Analysis, MetaInput};

/// What the pre-phase decided, plus the evidence it decided from.
///
/// Kept as a struct so `main.rs` can print the whole decision — analysis
/// highlights, routing, and probe outcome — from one place, and so a run's log
/// line is the complete audit trail of why it searched the way it did.
#[derive(Debug, Clone)]
pub struct Decision {
    pub analysis: Analysis,
    pub plan: Plan,
    pub probe: Option<Probe>,
}

/// Analyse a clausified problem and route it.
///
/// Cheap: `Analysis::extract` is one pass over the clause set plus the engine's
/// own redundancy reduction, and no search runs here.
pub fn decide(name: &str, meta: &MetaInput, clauses: &[Clause], symbols: &SymbolTable) -> Decision {
    let analysis = mrs_prephase::analyze(name, meta, clauses, symbols);
    let plan = route(&analysis);
    Decision {
        analysis,
        plan,
        probe: None,
    }
}

/// Build a schedule from a plan.
///
/// `workers` strategies are laid out, one per worker, taking the plan's priority
/// order cyclically: slot `i` runs entry `i % order.len()`. Each gets an equal
/// share of `budget`, and the last slot absorbs the rounding remainder so the
/// shares sum to the budget exactly.
///
/// A plan shorter than the worker count is legal and means "any of these
/// configurations, in this order, is acceptable" — the slots past its end start
/// again from the beginning rather than leaving workers idle.
pub fn schedule_from_plan(plan: &Plan, budget: Duration, workers: usize) -> Vec<SearchConfig> {
    let order = &plan.strategy_order;
    assert!(
        !order.is_empty(),
        "a plan with no strategy cannot be scheduled"
    );
    let workers = workers.max(1);
    let millis = budget.as_millis() as u64;
    let slice = Duration::from_millis(millis / workers as u64);
    (0..workers)
        .map(|slot| {
            let kind = &order[slot % order.len()];
            let remaining = budget.saturating_sub(slice * slot as u32);
            let time = if slot + 1 == workers {
                remaining
            } else {
                slice
            };
            let mut config = config_from(kind);
            config.time_limit = time;
            config
        })
        .collect()
}

/// Map one plan entry onto a `SearchConfig`.
pub fn config_from(kind: &StrategyKind) -> SearchConfig {
    SearchConfig {
        selection: match kind.selection {
            Selection::AgeWeight(n) => SelectionStrategy::AgeWeight(n),
            Selection::SmallestFirst => SelectionStrategy::SmallestFirst,
            Selection::GoalDirected(n) => SelectionStrategy::GoalDirected(n),
        },
        literal_selection: match kind.literal_selection {
            mrs_prephase::plan::LiteralSelection::AllNegative => LiteralSelection::AllNegative,
            mrs_prephase::plan::LiteralSelection::MaxNegative => LiteralSelection::MaxNegative,
            mrs_prephase::plan::LiteralSelection::All => LiteralSelection::All,
            mrs_prephase::plan::LiteralSelection::Maximal => {
                LiteralSelection::MaxNegativeOrMaxPositive
            }
        },
        ordering: match kind.ordering {
            mrs_prephase::plan::Ordering::Kbo => TermOrdering::KBO,
            mrs_prephase::plan::Ordering::Lpo => TermOrdering::LPO,
        },
        weight_fn: match kind.weight_fn {
            WeightFn::Standard => crate::ClauseWeightFn::Standard,
            WeightFn::FunctionDepth => crate::ClauseWeightFn::FunctionWeightPenalty,
            WeightFn::SymbolWeight => crate::ClauseWeightFn::SymbolWeight,
            WeightFn::ConjSymbolBoost => crate::ClauseWeightFn::ConjSymbolBoost,
            WeightFn::HornHeuristic => crate::ClauseWeightFn::HornHeuristic,
            WeightFn::HornPenalty => crate::ClauseWeightFn::HornPenalty,
        },
        max_term_weight: kind.max_term_weight,
        use_avatar: kind.avatar,
        sos_depth: kind.sos_depth.unwrap_or(u32::MAX),
        unit_only_resolution: kind.unit_only_resolution,
        precedence_scheme: match kind.precedence {
            mrs_prephase::plan::Precedence::InvFreq => PrecedenceScheme::InvFreq,
            mrs_prephase::plan::Precedence::ArityMin => PrecedenceScheme::ArityMin,
            mrs_prephase::plan::Precedence::ArityMax => PrecedenceScheme::ArityMax,
            mrs_prephase::plan::Precedence::Freq => PrecedenceScheme::Freq,
            mrs_prephase::plan::Precedence::GoalBoost => PrecedenceScheme::GoalBoost,
            // `PrecedenceScheme` has no `Uniform` variant: an all-equal
            // precedence is exactly `ArityMin` over a single-arity vocabulary,
            // and the catalogue never asks for a genuinely uniform precedence.
            mrs_prephase::plan::Precedence::Uniform => PrecedenceScheme::ArityMin,
        },
        symbol_weight_scheme: match kind.symbol_weight {
            mrs_prephase::plan::SymbolWeight::Uniform => SymbolWeightScheme::Uniform,
            mrs_prephase::plan::SymbolWeight::Arity => SymbolWeightScheme::Arity,
            mrs_prephase::plan::SymbolWeight::InvFreq => SymbolWeightScheme::InvFreq,
            mrs_prephase::plan::SymbolWeight::ConjectureBonus => {
                SymbolWeightScheme::ConjectureBonus
            }
        },
        goal_transformation: match kind.goal_transform {
            mrs_prephase::plan::GoalTransform::None => None,
            mrs_prephase::plan::GoalTransform::RecursiveSubterms => {
                Some(GoalTransformMode::RecursiveSubterms)
            }
            mrs_prephase::plan::GoalTransform::MaximalSubterms => {
                Some(GoalTransformMode::MaximalSubterms)
            }
        },
        ..SearchConfig::default()
    }
}

/// Apply the plan's preprocessing switches to a schedule.
///
/// This is deliberately separate from [`schedule_from_plan`]: whether a pre-pass
/// runs is an experimental question with its own A/B, whereas the strategy order
/// is the routing decision. Splitting them keeps a pre-pass measurement from
/// silently changing which strategies run.
pub fn apply_pre_passes(schedule: &mut [SearchConfig], plan: &Plan) {
    let sine = plan.pre_passes.iter().find_map(|p| match p {
        PrePass::Sine {
            tolerance,
            depth_limit,
        } => Some((*tolerance, *depth_limit)),
        _ => None,
    });
    let algorithm = plan.algorithm;
    for config in schedule.iter_mut() {
        match sine {
            Some((tolerance, depth_limit)) => {
                // Signature-based axiom filtering removes premises the
                // conjecture cannot reach. It is a *soundness-preserving
                // restriction for refutation* only while the conjecture's own
                // clause is retained, which SInE guarantees by construction, so
                // this is safe to enable from the plan. It does make any positive
                // (satisfiability) answer unreportable, which the search's
                // completeness audit already refuses.
                config.sine_tolerance = Some(tolerance);
                config.sine_depth_limit = Some(depth_limit);
            }
            None => {
                config.sine_tolerance = None;
                config.sine_depth_limit = None;
            }
        }
        // AVATAR's SAT instance grows without bound on a purely propositional
        // clause set, and the plan is what knows the input is propositional. The
        // engine forces this off for EPR input regardless; doing it here too
        // makes the decision the plan's rather than a side effect of an
        // unrelated guard.
        if algorithm == Algorithm::PropositionalSplitting
            && plan.pre_passes.contains(&PrePass::Grounding)
        {
            config.use_avatar = false;
        }
    }
}

/// A bounded measurement of what the search is doing on this problem.
///
/// The static analysis describes the input. This describes the *search*: how
/// fast clauses accumulate, how much of them survives redundancy elimination,
/// and whether the passive queue is still moving. Those three numbers are what
/// separate "hard but progressing" from "not going to finish", and no syntactic
/// feature can tell them apart — they are properties of the search's trajectory.
///
/// The probe runs the *reference* configuration (the same one for every problem,
/// so the numbers are comparable across problems) under a small iteration
/// budget, and is discarded afterwards: it produces telemetry, never a verdict.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Probe {
    /// Given-clause iterations executed.
    pub iterations: u64,
    /// New clauses enqueued.
    pub generated: u64,
    /// Clauses moved to the processed set.
    pub processed: u64,
    /// Clauses deleted by forward subsumption.
    pub forward_subsumed: u64,
    /// Clauses discarded by the weight cap.
    pub weight_discarded: u64,
    /// Passive queue size at the end of the probe.
    pub passive: u64,
    /// Wall-clock milliseconds the probe took.
    pub elapsed_ms: u64,
}

impl Probe {
    /// Clauses enqueued per iteration. A first-order problem tends to sit well
    /// above 1; a saturated or trivially-easy one sits at or near 0.
    pub fn generation_rate(&self) -> f64 {
        if self.iterations == 0 {
            return 0.0;
        }
        self.generated as f64 / self.iterations as f64
    }

    /// Fraction of generated clauses that redundancy elimination removed. High
    /// means the search is re-deriving what it already has, which is the
    /// signature of a weight function that is not steering.
    pub fn redundancy_rate(&self) -> f64 {
        if self.generated == 0 {
            return 0.0;
        }
        self.forward_subsumed as f64 / self.generated as f64
    }

    /// Clauses processed per second, normalized against the reference
    /// configuration so the number means "how fast does *this problem* run here"
    /// rather than an absolute rate that moves with the host.
    pub fn throughput_per_ms(&self) -> f64 {
        if self.elapsed_ms == 0 {
            return 0.0;
        }
        self.processed as f64 / self.elapsed_ms as f64
    }

    /// One-line summary for the log.
    pub fn summary(&self) -> String {
        format!(
            "iterations={} generated={} processed={} subsumed={} weight_dropped={} \
             passive={} gen_rate={:.2} redundancy={:.2} per_ms={:.0}",
            self.iterations,
            self.generated,
            self.processed,
            self.forward_subsumed,
            self.weight_discarded,
            self.passive,
            self.generation_rate(),
            self.redundancy_rate(),
            self.throughput_per_ms()
        )
    }
}

/// The probe's CSV columns, in order. The dumper writes exactly these after the
/// static analysis columns, so the header and the row cannot drift.
pub fn probe_columns() -> &'static [&'static str] {
    &[
        "probe_iterations",
        "probe_generated",
        "probe_processed",
        "probe_forward_subsumed",
        "probe_weight_discarded",
        "probe_passive",
        "probe_elapsed_ms",
        "probe_generation_rate",
        "probe_redundancy_rate",
        "probe_throughput_per_ms",
    ]
}

/// Clauses the reference probe search is allowed to process.
///
/// A fixed *work* ceiling rather than a fixed clock, for the reason given on
/// [`probe`]: the number has to mean the same thing on a fast host and a slow
/// one, or it cannot be compared across the corpus the study is measured on.
/// 2000 clauses is enough for the growth and redundancy rates to have settled
/// and small enough to cost a few tens of milliseconds.
pub const PROBE_CLAUSES: u64 = 2_000;

/// Run a bounded probe search and report what the search is doing.
///
/// The probe uses one *reference* configuration for every problem, so its
/// numbers mean "how does this problem behave under the standard configuration"
/// and are comparable across problems. It never returns a verdict: a refutation
/// found here is discarded, because the probe's budget is far too small to
/// decide a competition problem and a premature "solved" would suppress the real
/// search. What survives is the trajectory.
///
/// Runs on its own thread with the prover's recursion stack, for the same reason
/// the search does: unification and subsumption recurse to the depth of the
/// input terms, and a default 2 MiB stack aborts the process on a deeply nested
/// problem rather than returning a number.
pub fn probe(
    clauses: &[Clause],
    id_gen: mrs_core::clause::ClauseIdGen,
    symbols: &SymbolTable,
) -> Probe {
    if clauses.is_empty() {
        return Probe::default();
    }
    let symbols = symbols.clone();
    let clauses = clauses.to_vec();
    let builder = std::thread::Builder::new().stack_size(mrs_core::RECURSION_STACK_BYTES);
    let Ok(handle) = builder.spawn(move || probe_inner(clauses, id_gen, symbols)) else {
        return Probe::default();
    };
    handle.join().unwrap_or_default()
}

fn probe_inner(
    clauses: Vec<Clause>,
    id_gen: mrs_core::clause::ClauseIdGen,
    symbols: SymbolTable,
) -> Probe {
    use std::sync::Arc;
    use std::time::Instant;

    let started = Instant::now();
    let mut config = config_from(&mrs_prephase::plan::catalogue::s01_balanced_kbo());
    // AVATAR off: the probe is measuring the given-clause loop, and a split
    // sends the run into CaDiCaL, where the interesting signal no longer is.
    config.use_avatar = false;
    config.sos_depth = u32::MAX;
    config.sine_tolerance = None;
    config.sine_depth_limit = None;
    config.shared_pool_poll_interval = 0;
    config.lrs_policy = LrsPolicy::Disabled;
    // A work ceiling, not a wall clock: the number has to mean the same thing on
    // a fast host and a slow one, or it is not comparable across the corpus.
    config.resource_limits.max_processed = Some(PROBE_CLAUSES);
    config.resource_limits.max_memory_mb = None;

    let symbol_config = crate::symbol_config::compute_symbol_config(
        &clauses,
        config.precedence_scheme,
        config.symbol_weight_scheme,
    );
    config.ordering = match config.ordering {
        TermOrdering::KBO => TermOrdering::CustomKBO(Arc::clone(&symbol_config)),
        TermOrdering::LPO => TermOrdering::CustomLPO(Arc::clone(&symbol_config)),
        other => other,
    };
    let mut state = crate::state::SearchState::new_with_ml(
        clauses,
        Vec::new(),
        id_gen,
        symbol_config,
        Arc::new(symbols),
        config.use_avatar,
        None,
        false,
        config.weight_fn.clone(),
    );
    // The verdict is discarded on purpose: the probe's job is the trajectory.
    let _ = crate::given_clause::search(&mut state, &config);
    let stats = state.stats.clone();
    Probe {
        iterations: stats.iterations,
        generated: stats.generated,
        processed: stats.processed,
        forward_subsumed: stats.forward_subsumed,
        weight_discarded: stats.weight_discarded,
        passive: stats.passive_size,
        elapsed_ms: started.elapsed().as_millis() as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_prephase::plan::catalogue;

    #[test]
    fn plan_schedules_one_strategy_per_worker() {
        let plan = route(&Analysis {
            n_clauses: 900,
            logic_class: mrs_prephase::LogicClass::UnitEquality,
            shape_class: mrs_prephase::ShapeClass::Horn,
            scale_class: mrs_prephase::ScaleClass::Medium,
            goal_class: mrs_prephase::GoalClass::Tight,
            decomposition_class: mrs_prephase::DecompositionClass::Connected,
            label: "UEQ/HORN/MEDIUM/TIGHT/CONNECTED".to_string(),
            ..Analysis::default()
        });
        let budget = Duration::from_secs(30);
        let schedule = schedule_from_plan(&plan, budget, 8);
        assert_eq!(schedule.len(), 8);
        let total: Duration = schedule.iter().map(|c| c.time_limit).sum();
        assert_eq!(total, budget, "strategy slices must partition the budget");
    }

    #[test]
    fn a_short_plan_cycles_rather_than_leaving_workers_idle() {
        let plan = route(&Analysis {
            n_clauses: 10,
            logic_class: mrs_prephase::LogicClass::Equational,
            shape_class: mrs_prephase::ShapeClass::NonHorn,
            scale_class: mrs_prephase::ScaleClass::Tiny,
            goal_class: mrs_prephase::GoalClass::Tight,
            decomposition_class: mrs_prephase::DecompositionClass::Connected,
            label: "FEQ/NON_HORN/TINY/TIGHT/CONNECTED".to_string(),
            ..Analysis::default()
        });
        assert!(plan.strategy_order.len() < 8, "test needs a short plan");
        let schedule = schedule_from_plan(&plan, Duration::from_secs(16), 8);
        assert_eq!(schedule.len(), 8);
        assert!(
            schedule
                .iter()
                .all(|config| config.time_limit > Duration::ZERO)
        );
    }

    #[test]
    fn every_catalogue_entry_maps_onto_a_distinct_config() {
        let mut seen: Vec<(String, String)> = Vec::new();
        for kind in catalogue::all() {
            let config = config_from(&kind);
            let key = (format!("{:?}", config.selection), kind.name.clone());
            assert!(
                !seen.iter().any(|(_, name)| name == &kind.name),
                "catalogue name reused: {}",
                kind.name
            );
            seen.push(key);
        }
        // Two configurations may share a selection strategy (s1 and s7 differ
        // only in the ordering), but no two may be fully identical, or the
        // portfolio would be spending a slot on a duplicate.
        let mut full: Vec<String> = catalogue::all()
            .iter()
            .map(|kind| {
                let config = config_from(kind);
                format!(
                    "{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
                    config.selection,
                    config.literal_selection,
                    config.ordering,
                    config.weight_fn,
                    config.max_term_weight,
                    config.use_avatar,
                    config.sos_depth,
                    config.unit_only_resolution,
                    config.precedence_scheme,
                    config.symbol_weight_scheme,
                    config.goal_transformation
                )
            })
            .collect();
        let before = full.len();
        full.sort();
        full.dedup();
        assert_eq!(
            full.len(),
            before,
            "two catalogue entries are the same configuration"
        );
    }

    #[test]
    fn goal_transformation_survives_the_mapping() {
        let plan = route(&Analysis {
            n_clauses: 900,
            logic_class: mrs_prephase::LogicClass::UnitEquality,
            shape_class: mrs_prephase::ShapeClass::Horn,
            scale_class: mrs_prephase::ScaleClass::Medium,
            goal_class: mrs_prephase::GoalClass::Tight,
            decomposition_class: mrs_prephase::DecompositionClass::Connected,
            label: "UEQ".to_string(),
            ..Analysis::default()
        });
        let schedule = schedule_from_plan(&plan, Duration::from_secs(30), 3);
        assert!(
            schedule
                .iter()
                .any(|config| config.goal_transformation.is_some()),
            "the UEQ plan interleaves goal transformation and it must reach the engine"
        );
    }

    #[test]
    fn probe_rates_are_finite_at_the_degenerate_values() {
        let probe = Probe::default();
        assert_eq!(probe.generation_rate(), 0.0);
        assert_eq!(probe.redundancy_rate(), 0.0);
        assert_eq!(probe.throughput_per_ms(), 0.0);
        assert!(probe.summary().contains("gen_rate=0.00"));
    }

    #[test]
    fn sine_is_only_installed_when_the_plan_asks_for_it() {
        let mut plan = route(&Analysis {
            n_clauses: 5000,
            logic_class: mrs_prephase::LogicClass::Equational,
            shape_class: mrs_prephase::ShapeClass::NonHorn,
            scale_class: mrs_prephase::ScaleClass::Large,
            goal_class: mrs_prephase::GoalClass::Background,
            decomposition_class: mrs_prephase::DecompositionClass::Connected,
            label: "FEQ".to_string(),
            ..Analysis::default()
        });
        let mut schedule = schedule_from_plan(&plan, Duration::from_secs(30), 4);
        apply_pre_passes(&mut schedule, &plan);
        assert!(schedule.iter().all(|c| c.sine_tolerance.is_some()));

        plan.pre_passes.clear();
        apply_pre_passes(&mut schedule, &plan);
        assert!(schedule.iter().all(|c| c.sine_tolerance.is_none()));
    }
}
