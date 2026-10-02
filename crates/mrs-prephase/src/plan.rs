//! From a measured [`Analysis`] to a concrete search plan.
//!
//! A [`Plan`] is plain data: an ordered priority list of [`StrategyKind`]
//! (fully specified search configurations, described in terms neutral to
//! `mrs-search` so this crate stays independent of it), plus the pre-pass and
//! preprocessing switches that belong with them. `mrs-search` turns a plan into
//! a `StrategySchedule`; nothing here knows what a `SearchConfig` is.
//!
//! # Why a rule table and not a model
//!
//! Two reasons, and the second is the important one.
//!
//! 1. A wrong routing decision costs a competition run, and a rule table can be
//!    read, argued with and audited. A 40-coefficient logistic regression can
//!    only be trusted.
//! 2. More decisively: the decision this table has to make is *which of fifteen
//!    configurations to try first*, and the measurement that justifies it is
//!    conditional coverage — "on problems with these structural properties,
//!    configuration X solves strictly more than configuration Y". That evidence
//!    is a rule. A model would fit it worse while looking better.
//!
//! # Calibration status
//!
//! Every threshold in [`route`] is a `NOTE(calibrated)` marker naming the
//! measurement that sets it. Rules whose marker reads `uncalibrated` are
//! *priors* — they encode what the engine's own comments and the existing
//! `casc_*` schedules already believe — and they are flagged so that a reader
//! can tell a measurement from an assumption without reading the whole file.

use serde::{Deserialize, Serialize};

use crate::{Analysis, GoalClass, LogicClass, ScaleClass, ShapeClass};

/// Which clause-selection policy the passive-queue pop uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Selection {
    /// Every n-th pop is by age (FIFO), the rest by weight. Larger `n` is more
    /// weight-biased; smaller `n` explores more broadly.
    AgeWeight(u32),
    /// Always the lightest clause.
    SmallestFirst,
    /// Age, but every n-th pop prefers the clause closest to the conjecture.
    GoalDirected(u32),
}

/// Which literals may participate in an inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LiteralSelection {
    /// Only negative literals. The standard complete choice on equality-free
    /// input.
    AllNegative,
    /// At most one negative literal, chosen as maximal. Refutationally
    /// complete, and it removes the all-negative resolution blow-up on FNE.
    MaxNegative,
    /// Every literal, positive and negative.
    All,
    /// One maximal literal of either polarity. Faster, and incomplete for
    /// positive answers.
    Maximal,
}

/// Reduction ordering for orienting equations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ordering {
    /// Knuth–Bendix, with problem-specific weights.
    Kbo,
    /// Lexicographic path ordering.
    Lpo,
}

/// Clause-weighting function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeightFn {
    /// Every symbol costs 1.
    Standard,
    /// Quadratically heavier for deeply nested terms.
    FunctionDepth,
    /// Heavier for rare (high-precedence) symbols.
    SymbolWeight,
    /// Symbols absent from the conjecture closure cost 3x.
    ConjSymbolBoost,
    /// Non-Horn clauses pay a multiplier equal to their positive literal count.
    HornHeuristic,
    /// Non-Horn clauses pay 3x.
    HornPenalty,
}

/// How problem-specific symbol precedence is derived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Precedence {
    InvFreq,
    ArityMin,
    ArityMax,
    Freq,
    GoalBoost,
    Uniform,
}

/// How problem-specific symbol weights are derived (KBO only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolWeight {
    Uniform,
    Arity,
    InvFreq,
    ConjectureBonus,
}

/// Twee-style goal-directed term flattening.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GoalTransform {
    None,
    RecursiveSubterms,
    MaximalSubterms,
}

/// One fully specified search configuration.
///
/// This is a description, not a `SearchConfig`: the adapter in
/// `mrs-search::prephase` maps it onto the engine's own types. Keeping the
/// description here means the routing rules can be read, tested and printed
/// without the search crate, and means the rule table is the only thing that has
/// to change when a strategy's parameters are retuned.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategyKind {
    /// Stable slug used in reports and in `--pre-phase-trace` output.
    pub name: String,
    pub selection: Selection,
    pub literal_selection: LiteralSelection,
    pub ordering: Ordering,
    pub weight_fn: WeightFn,
    pub max_term_weight: Option<u32>,
    pub avatar: bool,
    /// `Some(d)` restricts weight-based pops to clauses within inference level
    /// `d` of the conjecture (set-of-support).
    pub sos_depth: Option<u32>,
    pub unit_only_resolution: bool,
    pub precedence: Precedence,
    pub symbol_weight: SymbolWeight,
    pub goal_transform: GoalTransform,
}

impl StrategyKind {
    /// The default constructor, used by the catalogue below.
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            selection: Selection::AgeWeight(5),
            literal_selection: LiteralSelection::AllNegative,
            ordering: Ordering::Kbo,
            weight_fn: WeightFn::Standard,
            max_term_weight: Some(200),
            avatar: true,
            sos_depth: None,
            unit_only_resolution: false,
            precedence: Precedence::InvFreq,
            symbol_weight: SymbolWeight::Uniform,
            goal_transform: GoalTransform::None,
        }
    }
}

/// A pre-pass or preprocessing switch the plan turns on.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PrePass {
    /// Signature-based axiom filtering (SInE), with a tolerance and depth limit.
    Sine { tolerance: f64, depth_limit: usize },
    /// Twee-style goal transformation (carried per strategy in
    /// [`StrategyKind::goal_transform`] as well, because it is a per-strategy
    /// choice in the existing schedules).
    GoalTransform,
    /// Bounded grounded expansion, for propositional / EPR input.
    Grounding,
    /// Componentwise refutation, for clause sets that split into symbol
    /// components.
    Componentwise,
    /// Propositional-skeleton resolution, for clause sets whose predicate
    /// arguments are all variables.
    PropositionalSkeleton,
    /// Condensed detachment, for the compact modal-logic fragment.
    CondensedDetachment,
}

/// The coarse algorithm family a problem is best attacked with.
///
/// This is the answer to "which algorithm", as distinct from "which
/// parameters": the families are searched by different means, so a misrouted
/// family costs the whole budget rather than a slice of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Algorithm {
    /// Ordered resolution + superposition over the given-clause loop.
    Superposition,
    /// Saturation of the propositional core, with the first-order structure
    /// handled by instantiation. The natural mode for EPR input.
    PropositionalSplitting,
    /// Unit-equation completion (ordered rewriting plus narrowing). The natural
    /// mode when every clause is a single equality.
    UnitEqualityCompletion,
    /// Bounded exhaustive grounding, followed by a SAT decision. Only for
    /// genuinely ground clause sets.
    GroundSat,
    /// Nothing in the current repertoire fits; fall back to the portfolio.
    Portfolio,
}

impl Algorithm {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Superposition => "superposition",
            Self::PropositionalSplitting => "propositional_splitting",
            Self::UnitEqualityCompletion => "unit_equality_completion",
            Self::GroundSat => "ground_sat",
            Self::Portfolio => "portfolio",
        }
    }
}

/// A complete routing decision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    /// The `logic/shape/scale/goal/decomposition` slug this plan came from.
    pub label: String,
    /// The CASC division the analysis believes the problem belongs to.
    pub division: String,
    pub algorithm: Algorithm,
    /// Priority order. `mrs-search` cycles it across workers, one strategy per
    /// worker, so slot *i* takes entry `i % len`.
    pub strategy_order: Vec<StrategyKind>,
    /// Pre-pass switches, in the order they should be attempted.
    pub pre_passes: Vec<PrePass>,
    /// One line naming the evidence this plan rests on. Printed by
    /// `--pre-phase`, so a run's routing decision is auditable from its log.
    pub rationale: String,
}

impl Plan {
    /// The plan as a single line, for logs.
    pub fn summary(&self) -> String {
        let order: Vec<&str> = self
            .strategy_order
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        format!(
            "division={} algorithm={} label={} order=[{}] pre=[{}] because: {}",
            self.division,
            self.algorithm.as_str(),
            self.label,
            order.join(","),
            self.pre_passes
                .iter()
                .map(pre_pass_slug)
                .collect::<Vec<_>>()
                .join(","),
            self.rationale
        )
    }
}

/// The configuration catalogue.
///
/// Entries mirror the fifteen base strategies in
/// `mrs_search::StrategySchedule::_all_strategies`, because those are the
/// configurations whose relative strength this study measures. Routing is a
/// *permutation* of them, not an invention of new ones: the measurement
/// available is "which of these fifteen covers which problems", and a rule that
/// selected a sixteenth untested configuration would be a guess wearing a
/// measurement's clothes.
pub mod catalogue {
    use super::*;

    macro_rules! strategy {
        ($name:literal, $field:ident = $value:expr) => {{
            let mut base = StrategyKind::new($name);
            base.$field = $value;
            base
        }};
    }

    /// s1 — balanced KBO exploration.
    pub fn s01_balanced_kbo() -> StrategyKind {
        let mut s = StrategyKind::new("s01_age3_kbo_allneg");
        s.selection = Selection::AgeWeight(3);
        s
    }
    /// s2 — deep chains, no weight cap, no AVATAR.
    pub fn s02_smallest_kbo() -> StrategyKind {
        let mut s = StrategyKind::new("s02_smallest_kbo_nocap");
        s.selection = Selection::SmallestFirst;
        s.max_term_weight = None;
        s.avatar = false;
        s.precedence = Precedence::ArityMin;
        s
    }
    /// s3 — pure best-first.
    pub fn s03_smallest_kbo_arity() -> StrategyKind {
        let mut s = StrategyKind::new("s03_smallest_kbo_arity");
        s.selection = Selection::SmallestFirst;
        s.symbol_weight = SymbolWeight::Arity;
        s
    }
    /// s4 — aggressive selection, one maximal negative literal.
    pub fn s04_age8_kbo_maxneg() -> StrategyKind {
        let mut s = StrategyKind::new("s04_age8_kbo_maxneg");
        s.selection = Selection::AgeWeight(8);
        s.literal_selection = LiteralSelection::MaxNegative;
        s.precedence = Precedence::ArityMax;
        s
    }
    /// s5 — every literal eligible.
    pub fn s05_age5_kbo_all() -> StrategyKind {
        let mut s = StrategyKind::new("s05_age5_kbo_all");
        s.selection = Selection::AgeWeight(5);
        s.literal_selection = LiteralSelection::All;
        s.precedence = Precedence::Freq;
        s
    }
    /// s6 — definitional-CNF / FNE oriented, no weight cap, no AVATAR.
    pub fn s06_age10_kbo_all_nocap() -> StrategyKind {
        let mut s = StrategyKind::new("s06_age10_kbo_all_nocap");
        s.selection = Selection::AgeWeight(10);
        s.literal_selection = LiteralSelection::All;
        s.max_term_weight = None;
        s.avatar = false;
        s.symbol_weight = SymbolWeight::InvFreq;
        s
    }
    /// s7 — balanced LPO exploration.
    pub fn s07_age3_lpo_allneg() -> StrategyKind {
        let mut s = StrategyKind::new("s07_age3_lpo_allneg");
        s.selection = Selection::AgeWeight(3);
        s.ordering = Ordering::Lpo;
        s
    }
    /// s8 — LPO goal-directed.
    pub fn s08_goal10_lpo() -> StrategyKind {
        let mut s = StrategyKind::new("s08_goal10_lpo");
        s.selection = Selection::GoalDirected(10);
        s.ordering = Ordering::Lpo;
        s.precedence = Precedence::GoalBoost;
        s.symbol_weight = SymbolWeight::ConjectureBonus;
        s
    }
    /// s9 — LPO best-first.
    pub fn s09_smallest_lpo() -> StrategyKind {
        let mut s = StrategyKind::new("s09_smallest_lpo");
        s.selection = Selection::SmallestFirst;
        s.ordering = Ordering::Lpo;
        s.precedence = Precedence::ArityMin;
        s
    }
    /// s10 — set-of-support on inference level.
    pub fn s10_sos_kbo() -> StrategyKind {
        let mut s = StrategyKind::new("s10_sos_age12_kbo");
        s.selection = Selection::AgeWeight(12);
        s.sos_depth = Some(100);
        s.precedence = Precedence::GoalBoost;
        s.symbol_weight = SymbolWeight::ConjectureBonus;
        s
    }
    /// s11 — conjecture-symbol boost.
    pub fn s11_conjboost_kbo() -> StrategyKind {
        let mut s = StrategyKind::new("s11_conjboost_kbo");
        s.selection = Selection::AgeWeight(6);
        s.weight_fn = WeightFn::ConjSymbolBoost;
        s.precedence = Precedence::GoalBoost;
        s
    }
    /// s12 — Horn preference.
    pub fn s12_horn_kbo() -> StrategyKind {
        let mut s = StrategyKind::new("s12_horn_age5_kbo");
        s.weight_fn = WeightFn::HornHeuristic;
        s.max_term_weight = None;
        s.avatar = false;
        s.precedence = Precedence::ArityMin;
        s.symbol_weight = SymbolWeight::Arity;
        s
    }
    /// s13 — set-of-support plus a depth penalty.
    pub fn s13_sos_depthpen_kbo() -> StrategyKind {
        let mut s = StrategyKind::new("s13_sos_depthpen_kbo");
        s.weight_fn = WeightFn::FunctionDepth;
        s.sos_depth = Some(100);
        s.precedence = Precedence::ArityMax;
        s.symbol_weight = SymbolWeight::InvFreq;
        s
    }
    /// s14 — conjecture-symbol boost with every literal and a tight cap.
    pub fn s14_conjboost_smallest_all() -> StrategyKind {
        let mut s = StrategyKind::new("s14_conjboost_smallest_all");
        s.selection = Selection::SmallestFirst;
        s.literal_selection = LiteralSelection::All;
        s.weight_fn = WeightFn::ConjSymbolBoost;
        s.max_term_weight = Some(100);
        s.avatar = false;
        s.precedence = Precedence::GoalBoost;
        s.symbol_weight = SymbolWeight::ConjectureBonus;
        s
    }
    /// s15 — rare-symbol penalty.
    pub fn s15_symbolweight_kbo() -> StrategyKind {
        let mut s = StrategyKind::new("s15_symbolweight_kbo");
        s.selection = Selection::AgeWeight(4);
        s.weight_fn = WeightFn::SymbolWeight;
        s.max_term_weight = None;
        s.avatar = false;
        s.precedence = Precedence::InvFreq;
        s.symbol_weight = SymbolWeight::Arity;
        s
    }

    /// The fifteen configurations, in base-strategy order. Index `i` is base
    /// strategy `i + 1`.
    pub fn all() -> Vec<StrategyKind> {
        vec![
            s01_balanced_kbo(),
            s02_smallest_kbo(),
            s03_smallest_kbo_arity(),
            s04_age8_kbo_maxneg(),
            s05_age5_kbo_all(),
            s06_age10_kbo_all_nocap(),
            s07_age3_lpo_allneg(),
            s08_goal10_lpo(),
            s09_smallest_lpo(),
            s10_sos_kbo(),
            s11_conjboost_kbo(),
            s12_horn_kbo(),
            s13_sos_depthpen_kbo(),
            s14_conjboost_smallest_all(),
            s15_symbolweight_kbo(),
        ]
    }

    #[allow(unused)]
    fn _keep_macro_used() -> StrategyKind {
        strategy!("placeholder", selection = Selection::AgeWeight(1))
    }
}

/// Route an [`Analysis`] to a [`Plan`].
///
/// NOTE(uncalibrated): every branch below is a prior until the sweep in
/// `crates/mrs-bench/prephase/` says otherwise. Branches are ordered from the
/// most specific structural fact to the least, because the specific facts are
/// the ones the engine's own pre-passes key on and therefore the ones a routing
/// error is most expensive to get wrong.
pub fn route(analysis: &Analysis) -> Plan {
    // ── 0. Nothing to route ───────────────────────────────────────────────
    if analysis.n_clauses == 0 {
        return Plan {
            label: analysis.label.clone(),
            division: "NONE".to_string(),
            algorithm: Algorithm::Portfolio,
            strategy_order: catalogue::all(),
            pre_passes: Vec::new(),
            rationale: "no clause survived lowering; the input is outside the \
                        supported fragment and no search configuration applies"
                .to_string(),
        };
    }

    // ── 1. Ground propositional input ──────────────────────────────────────
    // Everything is a constant and there is no equality: the problem is a finite
    // propositional instance, so the right move is to decide it as one.
    // NOTE(uncalibrated)
    if analysis.ground_ratio >= 1.0 && analysis.logic_class != LogicClass::UnitEquality {
        return Plan {
            label: analysis.label.clone(),
            division: "GROUND".to_string(),
            algorithm: Algorithm::GroundSat,
            strategy_order: vec![
                catalogue::s05_age5_kbo_all(),
                catalogue::s14_conjboost_smallest_all(),
            ],
            pre_passes: vec![PrePass::Grounding],
            rationale: format!(
                "every clause is ground ({} clauses, {} predicates); a bounded \
                 grounding decides it directly",
                analysis.n_clauses, analysis.n_predicates
            ),
        };
    }

    // ── 2. Unit equality ───────────────────────────────────────────────────
    // NOTE(prior, from `casc_ueq`): every clause a single equality, so the proof
    // is a rewrite/completion argument and the productive knobs are goal
    // transformation, a maximal-literal restriction, and dropping AVATAR (whose
    // SAT instance grows without bound on a purely equational clause set).
    if analysis.logic_class == LogicClass::UnitEquality {
        let mut order = vec![
            catalogue::s04_age8_kbo_maxneg(),
            catalogue::s08_goal10_lpo(),
            catalogue::s12_horn_kbo(),
            catalogue::s11_conjboost_kbo(),
            catalogue::s02_smallest_kbo(),
            catalogue::s14_conjboost_smallest_all(),
            catalogue::s15_symbolweight_kbo(),
            catalogue::s01_balanced_kbo(),
            catalogue::s05_age5_kbo_all(),
        ];
        // Interleave goal transformation across the slots the way `casc_ueq`
        // does: it is a per-slot choice, not a global one, and which slots it
        // suits is itself unresolved.
        for (index, strategy) in order.iter_mut().enumerate() {
            strategy.goal_transform = match index % 3 {
                0 => GoalTransform::None,
                1 => GoalTransform::RecursiveSubterms,
                _ => GoalTransform::MaximalSubterms,
            };
        }
        return Plan {
            label: analysis.label.clone(),
            division: "UEQ".to_string(),
            algorithm: Algorithm::UnitEqualityCompletion,
            strategy_order: order,
            pre_passes: vec![PrePass::GoalTransform],
            rationale: format!(
                "every clause is a unit equality ({} clauses, {} function symbols, \
                 max depth {}); completion-style configurations first",
                analysis.n_clauses, analysis.n_functions, analysis.max_term_depth
            ),
        };
    }

    // ── 3. Effectively propositional (EPR) ─────────────────────────────────
    // No function symbol of arity >= 1, so the problem is propositional up to
    // instantiation. AVATAR's splitting is exactly the right instrument; the
    // all-literal and maximal-literal restrictions matter because every
    // instance is trivially resolvable.
    // NOTE(prior, from `casc_epu` / `casc_eps`)
    if analysis.logic_class == LogicClass::EffectivelyPropositional {
        let order = if analysis.goal_class == GoalClass::NoGoal {
            // Satisfiability (EPS). Nothing is goal-connected, so the question
            // is whether saturation closes, and the configurations that close
            // fast matter more than the ones that find deep proofs.
            vec![
                catalogue::s06_age10_kbo_all_nocap(),
                catalogue::s02_smallest_kbo(),
                catalogue::s01_balanced_kbo(),
                catalogue::s03_smallest_kbo_arity(),
                catalogue::s05_age5_kbo_all(),
                catalogue::s14_conjboost_smallest_all(),
                catalogue::s12_horn_kbo(),
            ]
        } else {
            vec![
                catalogue::s01_balanced_kbo(),
                catalogue::s06_age10_kbo_all_nocap(),
                catalogue::s14_conjboost_smallest_all(),
                catalogue::s11_conjboost_kbo(),
                catalogue::s04_age8_kbo_maxneg(),
                catalogue::s05_age5_kbo_all(),
                catalogue::s02_smallest_kbo(),
            ]
        };
        return Plan {
            label: analysis.label.clone(),
            division: if analysis.goal_class == GoalClass::NoGoal {
                "EPS".to_string()
            } else {
                "EPR".to_string()
            },
            algorithm: Algorithm::PropositionalSplitting,
            strategy_order: order,
            pre_passes: vec![PrePass::Grounding, PrePass::Componentwise],
            rationale: format!(
                "no function symbol of arity >= 1 over {} clauses; propositional \
                 core with a {} abstraction",
                analysis.n_clauses, analysis.abstraction_atoms
            ),
        };
    }

    // ── 4. Large background theories ───────────────────────────────────────
    // The goal reaches only a small slice of a big clause set. Two independent
    // filters apply: signature-based axiom filtering (SInE) to shrink the
    // premise set, and set-of-support to keep the search near the goal.
    // NOTE(prior: SInE is only worth its cost above ~150 axioms, which is where
    // `Profile::is_large_theory` already draws the line)
    if analysis.goal_class == GoalClass::Background && analysis.n_clauses > 150 {
        let mut first = catalogue::s11_conjboost_kbo();
        first.sos_depth = Some(100);
        return Plan {
            label: analysis.label.clone(),
            division: division_of(analysis),
            algorithm: Algorithm::Superposition,
            strategy_order: vec![
                first,
                catalogue::s10_sos_kbo(),
                catalogue::s08_goal10_lpo(),
                catalogue::s06_age10_kbo_all_nocap(),
                catalogue::s01_balanced_kbo(),
                catalogue::s13_sos_depthpen_kbo(),
                catalogue::s12_horn_kbo(),
            ],
            pre_passes: vec![
                PrePass::Sine {
                    tolerance: 2.0,
                    depth_limit: 5,
                },
                PrePass::GoalTransform,
            ],
            rationale: format!(
                "goal reaches only {:.0}% of {} non-goal clauses; premise filtering \
                 plus set-of-support",
                100.0 * analysis.goal_reachable_ratio,
                analysis.n_clauses
            ),
        };
    }

    // ── 5. Horn input ──────────────────────────────────────────────────────
    // Every clause has at most one positive literal, so there is a Horn
    // refutation procedure and the search should be biased towards unit chains:
    // no AVATAR (the SAT instance is pure overhead), and a preference weight
    // that keeps Horn clauses cheap.
    // NOTE(prior, from strategy s12)
    if analysis.shape_class == ShapeClass::Horn || analysis.shape_class == ShapeClass::Stratified {
        return Plan {
            label: analysis.label.clone(),
            division: division_of(analysis),
            algorithm: Algorithm::Superposition,
            strategy_order: vec![
                catalogue::s12_horn_kbo(),
                catalogue::s11_conjboost_kbo(),
                catalogue::s04_age8_kbo_maxneg(),
                catalogue::s06_age10_kbo_all_nocap(),
                catalogue::s10_sos_kbo(),
                catalogue::s01_balanced_kbo(),
                catalogue::s08_goal10_lpo(),
            ],
            pre_passes: pre_passes_for_large(analysis),
            rationale: format!(
                "horn_ratio={:.2} over {} clauses; unit-chain biased configurations",
                analysis.horn_ratio, analysis.n_clauses
            ),
        };
    }

    // ── 6. Non-equational first-order ──────────────────────────────────────
    // NOTE(prior, from `casc_fne`): no equality anywhere, so all-negative
    // literal selection is the resolution blow-up generator the engine already
    // mitigates dynamically by switching to a single maximal negative literal.
    // The plan says so up front rather than relying on that runtime patch.
    if analysis.logic_class == LogicClass::NonEquational {
        let order: Vec<StrategyKind> = vec![
            catalogue::s11_conjboost_kbo(),
            catalogue::s08_goal10_lpo(),
            catalogue::s04_age8_kbo_maxneg(),
            catalogue::s15_symbolweight_kbo(),
            catalogue::s10_sos_kbo(),
            catalogue::s03_smallest_kbo_arity(),
            catalogue::s12_horn_kbo(),
            catalogue::s01_balanced_kbo(),
            catalogue::s06_age10_kbo_all_nocap(),
        ];
        return Plan {
            label: analysis.label.clone(),
            division: "FNE".to_string(),
            algorithm: Algorithm::Superposition,
            strategy_order: order,
            pre_passes: pre_passes_for_large(analysis),
            rationale: format!(
                "no equality in {} clauses (avg width {:.2}, max depth {}); \
                 resolution-oriented configurations",
                analysis.n_clauses, analysis.avg_clause_len, analysis.max_term_depth
            ),
        };
    }

    // ── 7. Deep equational input ───────────────────────────────────────────
    // Equality plus deeply nested terms: the binding constraint is term growth,
    // so the configurations that cap or penalise weight come first.
    // NOTE(prior: `DeepEquational` uses a depth threshold of 7)
    if analysis.max_term_depth >= 7 {
        return Plan {
            label: analysis.label.clone(),
            division: "FEQ".to_string(),
            algorithm: Algorithm::Superposition,
            strategy_order: vec![
                catalogue::s13_sos_depthpen_kbo(),
                catalogue::s14_conjboost_smallest_all(),
                catalogue::s11_conjboost_kbo(),
                catalogue::s06_age10_kbo_all_nocap(),
                catalogue::s10_sos_kbo(),
                catalogue::s01_balanced_kbo(),
                catalogue::s15_symbolweight_kbo(),
            ],
            pre_passes: pre_passes_for_large(analysis),
            rationale: format!(
                "max term depth {} over {} clauses; weight-capped configurations \
                 first",
                analysis.max_term_depth, analysis.n_clauses
            ),
        };
    }

    // ── 8. Small generic problems ──────────────────────────────────────────
    // Under a few hundred clauses the portfolio's per-slot setup cost is a real
    // fraction of the budget, so a short order is strictly better than a long
    // one; the two most broadly strong configurations lead.
    // NOTE(prior: `mini` exists for exactly this reason)
    let mut order = vec![
        catalogue::s01_balanced_kbo(),
        catalogue::s02_smallest_kbo(),
        catalogue::s11_conjboost_kbo(),
        catalogue::s06_age10_kbo_all_nocap(),
        catalogue::s10_sos_kbo(),
        catalogue::s08_goal10_lpo(),
        catalogue::s12_horn_kbo(),
    ];
    if analysis.scale_class == ScaleClass::Tiny {
        order.truncate(3);
    }
    Plan {
        label: analysis.label.clone(),
        division: division_of(analysis),
        algorithm: Algorithm::Superposition,
        strategy_order: order,
        pre_passes: pre_passes_for_large(analysis),
        rationale: format!(
            "general first-order with equality, {} clauses ({})",
            analysis.n_clauses,
            analysis.scale_class.as_str()
        ),
    }
}

fn pre_passes_for_large(analysis: &Analysis) -> Vec<PrePass> {
    // Signature-based axiom filtering earns its cost only once the premise set
    // is big enough for the signature chain to say something useful, and the
    // tolerance has to be looser when the goal reaches a small slice of the
    // clause set (a tighter tolerance would drop axioms the goal needs).
    // NOTE(prior: 150 axioms, matching `Profile::is_large_theory`)
    let mut out = Vec::new();
    if analysis.n_clauses > 150 {
        let tolerance = if analysis.goal_class == GoalClass::Background {
            3.5
        } else {
            2.0
        };
        out.push(PrePass::Sine {
            tolerance,
            depth_limit: if tolerance > 3.0 { 8 } else { 5 },
        });
    }
    if analysis.goal_class == GoalClass::Loose {
        out.push(PrePass::GoalTransform);
    }
    out
}

/// A short slug for a pre-pass, for log lines.
pub fn pre_pass_slug(pre_pass: &PrePass) -> String {
    match pre_pass {
        PrePass::Sine {
            tolerance,
            depth_limit,
        } => {
            format!("sine(t{tolerance},d{depth_limit})")
        }
        PrePass::GoalTransform => "goal_transform".to_string(),
        PrePass::Grounding => "grounding".to_string(),
        PrePass::Componentwise => "componentwise".to_string(),
        PrePass::PropositionalSkeleton => "propositional_skeleton".to_string(),
        PrePass::CondensedDetachment => "condensed_detachment".to_string(),
    }
}

/// The CASC division this analysis believes the problem belongs to.
pub fn division_of(analysis: &Analysis) -> String {
    match analysis.logic_class {
        LogicClass::UnitEquality => "UEQ".to_string(),
        LogicClass::PropositionalEquality => "PEQ".to_string(),
        LogicClass::EffectivelyPropositional => {
            if analysis.goal_class == GoalClass::NoGoal {
                "EPS".to_string()
            } else {
                "EPR".to_string()
            }
        }
        LogicClass::NonEquational => "FNE".to_string(),
        LogicClass::Equational => "FEQ".to_string(),
        LogicClass::Empty => "NONE".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn analysis(clauses: usize) -> Analysis {
        Analysis {
            n_clauses: clauses,
            label: "TEST".to_string(),
            ..Analysis::default()
        }
    }

    #[test]
    fn catalogue_has_fifteen_distinct_configurations() {
        let all = catalogue::all();
        assert_eq!(all.len(), 15);
        let mut names: Vec<String> = all.iter().map(|s| s.name.clone()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 15, "catalogue names must be unique");
    }

    #[test]
    fn every_plan_names_at_least_one_strategy() {
        for clauses in [0usize, 1, 10, 200, 5000] {
            let plan = route(&analysis(clauses));
            assert!(
                !plan.strategy_order.is_empty(),
                "a plan with no strategy cannot run (n_clauses={clauses})"
            );
        }
    }

    #[test]
    fn empty_input_is_not_routed_to_a_search() {
        let plan = route(&analysis(0));
        assert_eq!(plan.division, "NONE");
        assert!(plan.rationale.contains("outside the supported fragment"));
    }

    #[test]
    fn ground_input_goes_to_the_sat_path() {
        let a = Analysis {
            n_clauses: 400,
            ground_ratio: 1.0,
            label: "GROUND".to_string(),
            ..Analysis::default()
        };
        let plan = route(&a);
        assert_eq!(plan.algorithm, Algorithm::GroundSat);
        assert!(plan.pre_passes.contains(&PrePass::Grounding));
    }

    #[test]
    fn unit_equality_interleaves_goal_transformation() {
        let a = Analysis {
            n_clauses: 900,
            logic_class: LogicClass::UnitEquality,
            label: "UEQ".to_string(),
            ..Analysis::default()
        };
        let plan = route(&a);
        assert_eq!(plan.algorithm, Algorithm::UnitEqualityCompletion);
        let transforms: Vec<GoalTransform> = plan
            .strategy_order
            .iter()
            .map(|s| s.goal_transform)
            .collect();
        assert!(transforms.contains(&GoalTransform::RecursiveSubterms));
        assert!(transforms.contains(&GoalTransform::None));
        assert_ne!(
            transforms[0], transforms[1],
            "adjacent slots must not run the same goal transformation twice"
        );
    }

    #[test]
    fn large_background_problem_gets_premise_filtering() {
        let a = Analysis {
            n_clauses: 4000,
            logic_class: LogicClass::Equational,
            goal_class: GoalClass::Background,
            label: "FEQ/NON_HORN/LARGE/BACKGROUND/CONNECTED".to_string(),
            ..Analysis::default()
        };
        let plan = route(&a);
        assert!(
            plan.pre_passes
                .iter()
                .any(|p| matches!(p, PrePass::Sine { .. })),
            "a 4000-clause problem with a distant goal must be premise-filtered"
        );
    }

    #[test]
    fn small_problems_get_a_short_order() {
        // `scale_class` is what the branch reads, so set it the way
        // `Analysis::extract` would rather than relying on the field defaults.
        let generic = |clauses, scale| Analysis {
            n_clauses: clauses,
            logic_class: LogicClass::Equational,
            shape_class: ShapeClass::NonHorn,
            scale_class: scale,
            goal_class: GoalClass::Tight,
            max_fun_arity: 2,
            label: "FEQ/NON_HORN".to_string(),
            ..Analysis::default()
        };
        let long = route(&generic(400, ScaleClass::Small));
        let short = route(&generic(10, ScaleClass::Tiny));
        assert!(
            short.strategy_order.len() < long.strategy_order.len(),
            "a 10-clause problem must not pay for a 7-slot portfolio"
        );
    }

    #[test]
    fn every_strategy_in_every_plan_exists_in_the_catalogue() {
        let catalogue = catalogue::all();
        for clauses in [1usize, 30, 300, 3000, 30000] {
            for strategy in route(&analysis(clauses)).strategy_order {
                assert!(
                    catalogue.contains(&strategy),
                    "plan named a configuration the catalogue does not define: {}",
                    strategy.name
                );
            }
        }
    }
}
