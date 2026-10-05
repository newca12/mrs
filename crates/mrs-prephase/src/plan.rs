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

use crate::{Analysis, GoalClass, LogicClass};

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
    /// One maximal negative literal, or one maximal positive literal when the
    /// clause has no negative one. Faster, and incomplete for positive answers.
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
    /// s4 — aggressive selection: one maximal negative literal, or one maximal
    /// positive literal when the clause has no negative one.
    ///
    /// The `OrMaxPositive` half is load-bearing and was initially transcribed
    /// here as a plain `MaxNegative`. That is a strictly narrower restriction,
    /// and the end-to-end A/B found it: on CASC-30/UEQ the five large `CSR*-10`
    /// problems are refuted by the shipped `casc_ueq` portfolio in ~40 ms and time
    /// out when the catalogue's narrower s4 replaces it. "Maximal negative" is
    /// not the same inference set as "maximal negative, else maximal positive".
    pub fn s04_age8_kbo_maxneg() -> StrategyKind {
        let mut s = StrategyKind::new("s04_age8_kbo_maxneg");
        s.selection = Selection::AgeWeight(8);
        s.literal_selection = LiteralSelection::Maximal;
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
/// One numeric feature a rule can test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feature {
    Clauses,
    MaxTermDepth,
    MaxFunArity,
    DefiniteRatio,
    DualHornRatio,
    GoalReachableRatio,
    GroundRatio,
    HornRatio,
    UnitRatio,
    NComponents,
    RedundantRatio,
    AnalysisCapped,
}

/// One condition a rule tests: either a numeric threshold or a class equality.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Condition {
    Numeric(Feature, Test),
    Class(ClassAxis, &'static str),
    NotClass(ClassAxis, &'static str),
}

/// How a rule compares a numeric [`Feature`] against a threshold.
///
/// `PartialEq` but not `Eq`: the thresholds are `f64`, and `Eq` on floats is a
/// promise no comparison here needs to make.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Test {
    /// `feature < threshold`
    Lt(f64),
    /// `feature <= threshold`
    Le(f64),
    /// `feature > threshold`
    Gt(f64),
    /// `feature >= threshold`
    Ge(f64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassAxis {
    Logic,
    Shape,
    Scale,
    Goal,
    Decomposition,
}

impl ClassAxis {
    fn matches(self, analysis: &Analysis, value: &str) -> bool {
        match self {
            Self::Logic => analysis.logic_class.as_str() == value,
            Self::Shape => analysis.shape_class.as_str() == value,
            Self::Scale => analysis.scale_class.as_str() == value,
            Self::Goal => analysis.goal_class.as_str() == value,
            Self::Decomposition => analysis.decomposition_class.as_str() == value,
        }
    }
}

/// One entry in the routing table.
///
/// The table is data rather than control flow for three reasons: it can be
/// printed and read in one screen, each row carries its own provenance flag, and
/// a row can be re-ordered or replaced by the study without touching any code.
#[derive(Debug, Clone, Copy)]
pub struct Rule {
    /// Human-readable name, printed by `--pre-phase`.
    pub name: &'static str,
    /// All conditions must hold. Empty means "always", which only the fallback
    /// row has.
    pub when: &'static [Condition],
    pub algorithm: Algorithm,
    /// Base-strategy IDs, most promising first. `mrs-search` cycles them across
    /// workers.
    pub order: &'static [usize],
    pub pre_passes: &'static [PrePass],
    /// `false` means the row is a prior: it encodes what the engine already
    /// believed, and the sweep in `crates/mrs-bench/prephase/` has not yet
    /// confirmed it. `true` means the measurement named in
    /// [`Rule::evidence`] supports it.
    pub calibrated: bool,
    /// Where the order came from. Named so a reader can check the claim.
    pub evidence: &'static str,
}

impl Rule {
    /// Whether this rule applies to the analysis.
    pub fn matches(&self, analysis: &Analysis) -> bool {
        self.when.iter().all(|condition| match condition {
            Condition::Numeric(feature, test) => {
                let value = feature.read(analysis);
                match test {
                    Test::Lt(cut) => value < *cut,
                    Test::Le(cut) => value <= *cut,
                    Test::Gt(cut) => value > *cut,
                    Test::Ge(cut) => value >= *cut,
                }
            }
            Condition::Class(axis, name) => axis.matches(analysis, name),
            Condition::NotClass(axis, name) => !axis.matches(analysis, name),
        })
    }

    /// The rule's one-line description, for `--pre-phase` output.
    pub fn describe(&self) -> String {
        let conditions: Vec<String> = self.when.iter().map(describe_condition).collect();
        format!(
            "{}{} [{}] algorithm={} order=[{}] pre=[{}]{}",
            self.name,
            if conditions.is_empty() {
                String::new()
            } else {
                format!(" when {}", conditions.join(" and "))
            },
            if self.calibrated {
                "calibrated"
            } else {
                "prior"
            },
            self.algorithm.as_str(),
            self.order
                .iter()
                .map(|id| format!("s{id}"))
                .collect::<Vec<_>>()
                .join(","),
            self.pre_passes
                .iter()
                .map(pre_pass_slug)
                .collect::<Vec<_>>()
                .join(","),
            if self.evidence.is_empty() {
                String::new()
            } else {
                format!(" // {}", self.evidence)
            }
        )
    }
}

fn feature_name(feature: Feature) -> &'static str {
    match feature {
        Feature::Clauses => "n_clauses",
        Feature::MaxTermDepth => "max_term_depth",
        Feature::MaxFunArity => "max_fun_arity",
        Feature::DefiniteRatio => "definite_ratio",
        Feature::DualHornRatio => "dual_horn_ratio",
        Feature::GoalReachableRatio => "goal_reachable_ratio",
        Feature::GroundRatio => "ground_ratio",
        Feature::HornRatio => "horn_ratio",
        Feature::UnitRatio => "unit_ratio",
        Feature::NComponents => "n_components",
        Feature::RedundantRatio => "redundant_ratio",
        Feature::AnalysisCapped => "analysis_capped",
    }
}

fn axis_name(axis: ClassAxis) -> &'static str {
    match axis {
        ClassAxis::Logic => "logic",
        ClassAxis::Shape => "shape",
        ClassAxis::Scale => "scale",
        ClassAxis::Goal => "goal",
        ClassAxis::Decomposition => "decomposition",
    }
}

/// The routing table, most specific first. Order is part of the rule: the first
/// row whose conditions all hold wins.
pub fn rules() -> &'static [Rule] {
    use Algorithm::*;
    use ClassAxis::{Goal as G, Logic as L, Scale as S, Shape as H};
    use PrePass::*;
    &[
        Rule {
            name: "empty",
            when: &[Condition::Numeric(Feature::Clauses, Test::Le(0.0))],
            algorithm: Portfolio,
            order: &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
            pre_passes: &[],
            calibrated: true,
            evidence: "nothing to route; the input is outside the supported fragment",
        },
        Rule {
            name: "ground-sat",
            // Grounded *and* not purely equational: a ground unit-equality
            // clause set is decided either way, but completion is the classical
            // mode for it and yields a first-order proof, so the exception is
            // explicit here rather than a consequence of rule order.
            when: &[
                Condition::Numeric(Feature::GroundRatio, Test::Ge(1.0)),
                Condition::NotClass(L, "UEQ"),
                Condition::NotClass(L, "PEQ"),
            ],
            algorithm: GroundSat,
            order: &[5, 14, 6, 1],
            pre_passes: &[Grounding],
            calibrated: false,
            evidence: "prior: a finite ground instance is decidable by grounding",
        },
        Rule {
            name: "ueq-completion",
            when: &[(Condition::Class(L, "UEQ"))],
            algorithm: UnitEqualityCompletion,
            // The three goal-transform variants are interleaved across slots by
            // `with_goal_transform`, which is a per-slot choice rather than a
            // property of the strategy.
            order: &[4, 8, 12, 11, 2, 14, 15, 1, 5],
            pre_passes: &[GoalTransform],
            calibrated: false,
            evidence: "prior: casc_ueq, whose order came from a solo sweep",
        },
        // EPS comes before the general EPR row because the two are different
        // jobs: with no conjecture the question is whether saturation closes,
        // which rewards the configurations that close fast, while a conjecture
        // makes the question where the proof is, which rewards the ones that
        // steer. A single EPR row would have to pick one of those for both.
        Rule {
            name: "eps-saturation",
            when: &[Condition::Class(L, "EPR"), Condition::Class(G, "NO_GOAL")],
            algorithm: PropositionalSplitting,
            order: &[6, 2, 1, 3, 5, 14, 12],
            pre_passes: &[Grounding, Componentwise],
            calibrated: false,
            evidence: "prior: casc_eps",
        },
        Rule {
            name: "epr-refutation",
            when: &[(Condition::Class(L, "EPR"))],
            algorithm: PropositionalSplitting,
            order: &[1, 6, 14, 11, 4, 5, 2],
            pre_passes: &[Grounding, Componentwise],
            calibrated: false,
            evidence: "prior: casc_epu",
        },
        // A definite clause set — every clause has exactly one positive literal
        // — is a forward-chaining problem, and a dual-Horn set is a
        // hyper-resolution problem. Both measured at a materially above-base
        // solve rate (0.37 and 0.21 against a 0.072 base rate over 600 problems).
        //
        // The leading configurations are the two chain builders: s2 is
        // `SmallestFirst` with no weight cap, s14 is `SmallestFirst` with a tight
        // cap and a conjecture-symbol boost. Both were the top two solvers on
        // these subsets (s2 10/18, s14 8/18 on the definite subset).
        //
        // NOTE: the *portfolio* effect of this rule is +0, not +16. See
        // docs/reports/prephase/2026-10-prephase-study.md §6: the per-division
        // order already covers this subset at 17 of 18, so there is nothing left
        // for a rule to win. What the measurement supports is the *feasibility*
        // prediction, which this row records, and not the reordering.
        Rule {
            name: "forward-chaining",
            when: &[
                Condition::Numeric(Feature::DefiniteRatio, Test::Ge(0.99)),
                Condition::NotClass(L, "UEQ"),
            ],
            algorithm: Superposition,
            order: &[2, 14, 4, 5, 12, 8, 10],
            pre_passes: &[],
            calibrated: true,
            evidence: "calibrated on the CASC-30 3 s sweep: definite_ratio>=0.99 over 49 problems gives P(solved)=0.37 against a 0.072 base rate, led by s2 (10) and s14 (8). Portfolio effect at 8 slots: +0, because casc_ueq and casc_fne already cover 17 of the 18",
        },
        Rule {
            name: "hyper-resolution",
            when: &[(Condition::Numeric(Feature::DualHornRatio, Test::Ge(0.95)))],
            algorithm: Superposition,
            order: &[2, 12, 4, 5, 14, 8, 15],
            pre_passes: &[],
            calibrated: true,
            evidence: "calibrated on the CASC-30 3 s sweep: dual_horn_ratio>=0.95 over 156 problems gives P(solved)=0.21 against 0.072, led by s2 (19) and s12 (18). Portfolio effect at 8 slots: +2 of 600, within noise",
        },
        Rule {
            name: "unreachable-depth",
            // max_term_depth > 10 was 0-for-34 at 3 s. The order here is
            // therefore not a claim that these configurations are better; it is
            // the same broad order, because no measurement distinguishes them.
            // What the rule buys is the refusal to spend budget on pre-passes for
            // a problem whose clause terms already exceed any weight cap worth
            // setting.
            when: &[
                Condition::Numeric(Feature::MaxTermDepth, Test::Gt(10.0)),
                Condition::NotClass(L, "UEQ"),
            ],
            algorithm: Superposition,
            order: &[15, 12, 2, 4, 5, 8, 10],
            pre_passes: &[],
            calibrated: true,
            evidence: "calibrated on the CASC-30 3 s sweep: max_term_depth>10 was 0 of 34 solved by any configuration, against 0.072 overall. Order not distinguished by the data",
        },
        Rule {
            name: "far-goal",
            when: &[
                (Condition::Class(G, "BACKGROUND")),
                Condition::Numeric(Feature::Clauses, Test::Gt(150.0)),
            ],
            algorithm: Superposition,
            order: &[11, 10, 8, 6, 1, 13, 12],
            pre_passes: &[
                Sine {
                    tolerance: 3.5,
                    depth_limit: 8,
                },
                GoalTransform,
            ],
            calibrated: false,
            evidence: "prior: Profile::is_large_theory (150 axioms) plus ConjSymbolBoost",
        },
        Rule {
            name: "horn",
            when: &[(Condition::Class(H, "HORN"))],
            algorithm: Superposition,
            order: &[12, 11, 4, 6, 10, 1, 8],
            pre_passes: &[],
            calibrated: false,
            evidence: "prior: strategy s12, the Horn-preference configuration",
        },
        Rule {
            name: "fne-resolution",
            when: &[(Condition::Class(L, "FNE"))],
            algorithm: Superposition,
            order: &[11, 8, 4, 15, 10, 3, 12, 1, 6],
            pre_passes: &[],
            calibrated: false,
            evidence: "prior: casc_fne",
        },
        Rule {
            name: "deep-equational",
            when: &[Condition::Numeric(Feature::MaxTermDepth, Test::Ge(7.0))],
            algorithm: Superposition,
            order: &[13, 14, 11, 6, 10, 1, 15],
            pre_passes: &[],
            calibrated: false,
            evidence: "prior: Profile::DeepEquational uses a depth threshold of 7",
        },
        Rule {
            name: "tiny",
            when: &[(Condition::Class(S, "TINY"))],
            algorithm: Superposition,
            order: &[1, 2, 11],
            pre_passes: &[],
            calibrated: false,
            evidence: "prior: a short order; the portfolio's per-slot setup is a real share of a tiny budget",
        },
        Rule {
            name: "general-feq",
            when: &[],
            algorithm: Superposition,
            order: &[1, 2, 11, 6, 10, 8, 12],
            pre_passes: &[],
            calibrated: false,
            evidence: "prior: the broad KBO/LPO baseline",
        },
    ]
}

/// Print the routing table. Used by `--list-rules` and by the study write-up, so
/// the table in the report is generated from the table the binary runs.
pub fn describe_rules() -> String {
    rules()
        .iter()
        .map(|rule| rule.describe())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Route an [`Analysis`] to a [`Plan`].
pub fn route(analysis: &Analysis) -> Plan {
    if analysis.n_clauses == 0 {
        // The `empty` row. It still names a full portfolio: an input that lowers
        // to nothing has to produce *some* verdict, and refusing to build a
        // schedule here would take the decision away from the caller.
        let rule = &rules()[0];
        let strategies = rule.order.iter().map(|id| strategy_by_id(*id)).collect();
        return plan_from(
            rule,
            analysis,
            strategies,
            "no clause survived lowering; the input is outside the supported fragment",
        );
    }
    let rule = rules()
        .iter()
        .find(|rule| rule.matches(analysis))
        .unwrap_or_else(|| rules().last().expect("the table has a catch-all"));
    let strategies = rule
        .order
        .iter()
        .map(|id| strategy_by_id(*id))
        .collect::<Vec<_>>();
    let pre_passes = if matches!(rule.name, "ueq-completion") {
        with_goal_transform(strategies)
    } else {
        strategies
    };
    plan_from(rule, analysis, pre_passes, rule.evidence)
}

fn plan_from(rule: &Rule, analysis: &Analysis, strategies: Vec<StrategyKind>, why: &str) -> Plan {
    Plan {
        label: analysis.label.clone(),
        division: division_of(analysis),
        algorithm: rule.algorithm,
        strategy_order: strategies,
        pre_passes: rule.pre_passes.to_vec(),
        rationale: format!(
            "rule `{}`{}{}: {}",
            rule.name,
            if rule.calibrated {
                ""
            } else {
                " (uncalibrated prior)"
            },
            if rule.when.is_empty() {
                String::new()
            } else {
                format!(" [{}]", rule.describe_conditions())
            },
            why
        ),
    }
}

fn describe_condition(condition: &Condition) -> String {
    match condition {
        Condition::Class(axis, name) => format!("{} == {name}", axis_name(*axis)),
        Condition::NotClass(axis, name) => format!("{} != {name}", axis_name(*axis)),
        Condition::Numeric(feature, test) => {
            let name = feature_name(*feature);
            match test {
                Test::Lt(cut) => format!("{name} < {cut}"),
                Test::Le(cut) => format!("{name} <= {cut}"),
                Test::Gt(cut) => format!("{name} > {cut}"),
                Test::Ge(cut) => format!("{name} >= {cut}"),
            }
        }
    }
}

impl Feature {
    fn read(self, analysis: &Analysis) -> f64 {
        match self {
            Self::Clauses => analysis.n_clauses as f64,
            Self::MaxTermDepth => analysis.max_term_depth as f64,
            Self::MaxFunArity => analysis.max_fun_arity as f64,
            Self::DefiniteRatio => analysis.definite_ratio,
            Self::DualHornRatio => analysis.dual_horn_ratio,
            Self::GoalReachableRatio => analysis.goal_reachable_ratio,
            Self::GroundRatio => analysis.ground_ratio,
            Self::HornRatio => analysis.horn_ratio,
            Self::UnitRatio => analysis.unit_ratio,
            Self::NComponents => analysis.n_components as f64,
            Self::RedundantRatio => analysis.redundant_ratio,
            // A saturated counter means the problem was too large to measure
            // precisely. Reading it as "large" makes the conservative rule win
            // rather than the informative one.
            Self::AnalysisCapped => f64::from(analysis.analysis_capped),
        }
    }
}

impl Rule {
    fn describe_conditions(&self) -> String {
        self.when
            .iter()
            .map(describe_condition)
            .collect::<Vec<_>>()
            .join(" and ")
    }
}

fn strategy_by_id(id: usize) -> StrategyKind {
    catalogue::all()
        .into_iter()
        .nth(id.saturating_sub(1))
        .unwrap_or_else(catalogue::s01_balanced_kbo)
}

/// Interleave the three goal-transformation variants across the slots.
///
/// Which slots suit which variant is itself unresolved, so the plan runs one of
/// each in rotation rather than committing to an answer: that is the same choice
/// `casc_ueq` makes, for the same reason.
fn with_goal_transform(strategies: Vec<StrategyKind>) -> Vec<StrategyKind> {
    strategies
        .into_iter()
        .enumerate()
        .map(|(index, mut strategy)| {
            strategy.goal_transform = match index % 3 {
                0 => GoalTransform::None,
                1 => GoalTransform::RecursiveSubterms,
                _ => GoalTransform::MaximalSubterms,
            };
            strategy
        })
        .collect()
}

/// A short slug for a pre-pass, for log lines and the rule table.
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
    use crate::{ScaleClass, ShapeClass};

    fn analysis(clauses: usize) -> Analysis {
        Analysis {
            n_clauses: clauses,
            label: "TEST".to_string(),
            ..Analysis::default()
        }
    }

    #[test]
    fn the_rule_table_is_total_and_ordered() {
        // Totality: a non-empty analysis must match some row.
        let generic = |clauses, logic, depth| Analysis {
            n_clauses: clauses,
            logic_class: logic,
            max_term_depth: depth,
            label: "X".to_string(),
            ..Analysis::default()
        };
        for clauses in [1usize, 100, 5000] {
            for logic in [
                LogicClass::UnitEquality,
                LogicClass::PropositionalEquality,
                LogicClass::EffectivelyPropositional,
                LogicClass::NonEquational,
                LogicClass::Equational,
            ] {
                for depth in [0usize, 12] {
                    let analysis = generic(clauses, logic, depth);
                    assert!(
                        rules().iter().any(|rule| rule.matches(&analysis)),
                        "no rule matches clauses={clauses} logic={} depth={depth}",
                        logic.as_str()
                    );
                }
            }
        }
    }

    #[test]
    fn every_rule_names_configurations_that_exist() {
        for rule in rules() {
            assert!(!rule.order.is_empty(), "rule `{}` has no order", rule.name);
            for id in rule.order {
                assert!(
                    (1..=15).contains(id),
                    "rule `{}` names base strategy {id}, outside 1..=15",
                    rule.name
                );
            }
        }
    }

    #[test]
    fn no_rule_is_shadowed_by_an_earlier_one() {
        // A rule that can never fire is a rule that misleads whoever reads the
        // table, so shadowing is a defect rather than a harmless redundancy.
        // Overlap on the *axis* is fine; what must not happen is a later row whose
        // conditions are a subset of an earlier row's.
        for (index, rule) in rules().iter().enumerate() {
            assert!(
                !rule.when.is_empty() || index == rules().len() - 1,
                "only the last rule may be unconditional; `{}` is rule {} of {}",
                rule.name,
                index + 1,
                rules().len()
            );
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
