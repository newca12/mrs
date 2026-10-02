//! Deep, cheap, deterministic problem analysis for the mrs pre-phase.
//!
//! # Why this crate exists
//!
//! `mrs` ships several *algorithms* around the given-clause loop — grounded
//! EPR expansion, componentwise refutation (CWA), propositional-skeleton
//! resolution (FVO), condensed detachment, and the superposition portfolio
//! itself — plus per-strategy preprocessing choices (SInE tolerance, AVATAR,
//! weight caps, literal selection, goal transformation). Every one of those
//! choices is currently made by a rule keyed on a coarse syntactic predicate
//! (see `mrs_search::strategy::auto_schedule_name` and
//! `mrs_core::profile::classify_problem`). This crate computes the evidence a
//! *data-derived* choice needs: one flat, fully serializable vector of
//! structural, goal-topological, decomposition, redundancy and signature
//! measurements over the clausified problem.
//!
//! Everything here is `O(size of the clause set)` with a small constant: no
//! search, no SAT solver, and no allocation that grows with the number of
//! inferred clauses. That is deliberate. The pre-phase may only spend a small,
//! bounded slice of a competition wall clock before the first inference.
//!
//! # Boundary: what is *not* here
//!
//! * Search-behaviour probes (clause-growth slope, subsumption rate,
//!   proof-depth extrapolation) live in `mrs-search`, because they need the
//!   search.
//! * The TPTP header `Rating:` field is deliberately **excluded** from the
//!   feature vector used for routing. It is present in the input file at
//!   competition time, but it is a community difficulty annotation authored
//!   with knowledge of other provers' performance. A router that reads it
//!   would score better than it reasons and would not generalize to unannotated
//!   input. It is captured separately in [`Analysis::header_rating`] so the
//!   study can quantify how much of the predictive power is community metadata
//!   rather than structure.
//!
//! # Boundary: what is *not* here
//!
//! This crate also does not depend on `mrs-tptp`. Dialect and role counts are
//! the only input-side information the analysis needs, and clausification
//! destroys them; the caller (the `mrs` binary, or the offline dumper in
//! `mrs-bench`) reads them off the AST and hands them over in [`MetaInput`].
//! That keeps this a pure clause-analysis library and lets `mrs-search` depend
//! on it without dragging the parser in.

use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use serde::{Deserialize, Serialize};

use mrs_core::SymbolTable;
use mrs_core::clause::{Clause, ClauseSource};
use mrs_core::formula::Atom;
use mrs_core::symbol::SymbolId;
use mrs_core::term::{Term, VarId};

pub mod plan;
pub mod preprocessing;

pub use plan::{Algorithm, Plan, PrePass, StrategyKind, route};

/// Maximum goal-distance BFS radius used for the reachability features.
///
/// Matches `mrs_search::goal_distance::MAX_GOAL_RADIUS`. Duplicated rather than
/// imported so this crate stays independent of `mrs-search`.
const GOAL_RADIUS: u8 = 5;

/// Sentinel distance for clauses the conjecture cannot reach at all.
const DISCONNECTED: u32 = 100;

/// Ceiling on the distinct-term / distinct-abstraction-atom sets.
///
/// These two counters are diagnostics, and both grow with the number of
/// subterms rather than with the size of the problem. A single 13 MB TPTP input
/// expands to millions of subterms, and an uncapped set of cloned `Term`s is
/// then the largest allocation in the pre-phase by a wide margin — enough to
/// take the process out with an OOM kill rather than produce a measurement. The
/// cap keeps the analysis `O(1)` in memory per problem at the cost of making
/// these two fields a lower bound above the cap; [`Analysis::analysis_capped`]
/// records when that happened, so a downstream reader never mistakes a saturated
/// count for a measured one.
const TERM_CAP: usize = 1_000_000;

/// Input-side metadata that clausification erases.
///
/// Every field has a zero/absent default, so a caller that only has clauses (a
/// unit test, a synthetic clause set) can pass [`MetaInput::default`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetaInput {
    /// Size of the source file in bytes.
    pub input_bytes: Option<u64>,
    /// Predominant input dialect (`CNF`, `FOF`, `TFF`, ...).
    pub dialect: String,
    /// Number of `%include` directives.
    pub n_includes: usize,
    /// Number of annotated input formulas.
    pub n_input_formulas: usize,
    /// Number of `conjecture` / `negated_conjecture` input formulas.
    pub n_conjecture_formulas: usize,
    /// Number of `type` input formulas.
    pub n_type_formulas: usize,
    /// Number of `definition` input formulas.
    pub n_definition_formulas: usize,
    /// Number of `cnf(...)` input clauses.
    pub n_input_cnf_clauses: usize,
    /// `% Status:` from the TPTP header.
    pub header_status: Option<String>,
    /// `% Rating:` from the TPTP header. Reported only; never routed on.
    pub header_rating: Option<f64>,
}

/// What the input problem *is*, in the coarse vocabulary CASC uses.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogicClass {
    /// Every clause is a single equality literal: the UEQ calculus.
    UnitEquality,
    /// Every clause is an equality literal, widths may exceed one: PEQ.
    PropositionalEquality,
    /// No function symbol of arity >= 1: propositional / EPR.
    EffectivelyPropositional,
    /// No equality anywhere: FNE.
    NonEquational,
    /// Both functions and equality present: FEQ.
    Equational,
    /// No clause survived lowering, or the input is unsupported/empty.
    #[default]
    Empty,
}

impl LogicClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnitEquality => "UEQ",
            Self::PropositionalEquality => "PEQ",
            Self::EffectivelyPropositional => "EPR",
            Self::NonEquational => "FNE",
            Self::Equational => "FEQ",
            Self::Empty => "EMPTY",
        }
    }
}

/// Polarity profile of the clause set.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeClass {
    /// Every clause has at most one positive literal.
    Horn,
    /// Every clause has at most one *negative* literal (definite programs and
    /// their duals, including the common `~p | q` encoding).
    DualHorn,
    /// Horn and dual-Horn clauses coexist with no other kind present.
    Stratified,
    /// Genuinely non-Horn clauses are present.
    NonHorn,
    /// No clause has any positive literal.
    GoalOnly,
    #[default]
    Empty,
}

impl ShapeClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Horn => "HORN",
            Self::DualHorn => "DUAL_HORN",
            Self::Stratified => "STRATIFIED",
            Self::NonHorn => "NON_HORN",
            Self::GoalOnly => "GOAL_ONLY",
            Self::Empty => "EMPTY",
        }
    }
}

/// Coarse size bucket. Boundaries sit where the engine's own heuristics change
/// behaviour, not at round numbers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScaleClass {
    /// <= 32 clauses: clausification and preprocessing dominate.
    Tiny,
    /// <= 256 clauses.
    Small,
    /// <= 2048 clauses: first size where SInE and BCE earn their cost.
    Medium,
    /// <= 16384 clauses.
    Large,
    /// > 16384 clauses.
    Huge,
    #[default]
    Empty,
}

impl ScaleClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tiny => "TINY",
            Self::Small => "SMALL",
            Self::Medium => "MEDIUM",
            Self::Large => "LARGE",
            Self::Huge => "HUGE",
            Self::Empty => "EMPTY",
        }
    }
}

/// How much of the clause set can participate in a refutation of the
/// conjecture, measured by symbol reachability rather than by size.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GoalClass {
    /// No conjecture: the task is satisfiability of the whole clause set.
    NoGoal,
    /// >= 80% of non-goal clauses are within the goal radius.
    Tight,
    /// >= 25% of non-goal clauses are within the goal radius.
    Loose,
    /// < 25%: the bulk of the clause set is background the goal cannot reach.
    Background,
    #[default]
    Empty,
}

impl GoalClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoGoal => "NO_GOAL",
            Self::Tight => "TIGHT",
            Self::Loose => "LOOSE",
            Self::Background => "BACKGROUND",
            Self::Empty => "EMPTY",
        }
    }
}

/// How the clause set splits along symbol sharing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecompositionClass {
    /// All clauses share a symbol: one connected component.
    Connected,
    /// Several components, and the conjecture touches all of them.
    DisconnectedMultiGoal,
    /// Several components, and the conjecture touches only some.
    DisconnectedSingleGoal,
    #[default]
    Empty,
}

impl DecompositionClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Connected => "CONNECTED",
            Self::DisconnectedMultiGoal => "MULTI_GOAL",
            Self::DisconnectedSingleGoal => "SINGLE_GOAL",
            Self::Empty => "EMPTY",
        }
    }
}

/// Per-clause intermediates gathered in the single pass over the clause set.
struct ClauseFacts {
    /// Every symbol occurring anywhere in the clause, predicates and function
    /// symbols alike, duplicates included (the incidence graph dedups).
    symbols: Vec<SymbolId>,
    is_goal: bool,
}

struct TermAcc {
    occurrences: usize,
    depth_sum: usize,
    depth_hist: Vec<usize>,
    arity_sum: usize,
    n_constants: usize,
    n_functions: usize,
    /// Arity per function symbol (arity 0 == constant).
    func_arities: HashMap<SymbolId, usize>,
    /// Distinct non-variable terms, bounded by [`TERM_CAP`].
    terms: HashSet<Term>,
    /// Set when [`Self::terms`] saturated.
    capped: bool,
}

impl Default for TermAcc {
    fn default() -> Self {
        Self {
            occurrences: 0,
            depth_sum: 0,
            depth_hist: vec![0; 65],
            arity_sum: 0,
            n_constants: 0,
            n_functions: 0,
            func_arities: HashMap::default(),
            terms: HashSet::default(),
            capped: false,
        }
    }
}

struct DisjointSet {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl DisjointSet {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
            rank: vec![0; n],
        }
    }
    fn find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }
    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra == rb {
            return;
        }
        match self.rank[ra].cmp(&self.rank[rb]) {
            std::cmp::Ordering::Less => self.parent[ra] = rb,
            std::cmp::Ordering::Greater => self.parent[rb] = ra,
            std::cmp::Ordering::Equal => {
                self.parent[rb] = ra;
                self.rank[ra] += 1;
            }
        }
    }
}

/// The complete static analysis of one problem.
///
/// Field names are the CSV column names emitted by [`Analysis::columns`] and
/// [`Analysis::csv_row`]; the two cannot drift because both iterate that one
/// list and `csv_row` looks each name up in [`Analysis::field`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Analysis {
    // ── Identity / provenance ───────────────────────────────────────────────
    /// Problem name (file stem).
    pub name: String,
    /// TPTP source-domain prefix (`GRP`, `LCL`, `SYN`, ...), `SYN` when absent.
    pub source_domain: String,
    /// Input size in bytes, when known.
    pub input_bytes: Option<u64>,
    /// Predominant input dialect (`CNF`, `FOF`, `TFF`, ...).
    pub dialect: String,
    /// Number of `%include` directives in the top-level file.
    pub n_includes: usize,
    /// Number of annotated input formulas.
    pub n_input_formulas: usize,
    /// Number of `conjecture`-role input formulas.
    pub n_conjecture_formulas: usize,
    /// Number of `type`-role input formulas.
    pub n_type_formulas: usize,
    /// Number of `definition`-role input formulas.
    pub n_definition_formulas: usize,

    // ── Scale ───────────────────────────────────────────────────────────────
    /// Clauses after clausification, before redundancy elimination.
    pub n_clauses: usize,
    /// Literals across all clauses.
    pub n_literals: usize,
    /// Clauses arriving from `cnf(...)` input (skolemized by the input, not by us).
    pub n_input_cnf_clauses: usize,
    /// `log10(n_clauses)`, the scale feature a model actually uses.
    pub log_n_clauses: f64,
    /// Average clause width.
    pub avg_clause_len: f64,
    /// Maximum clause width.
    pub max_clause_len: usize,
    /// Clauses of width exactly 1.
    pub n_width_1: usize,
    /// Clauses of width 2.
    pub n_width_2: usize,
    /// Clauses of width 3.
    pub n_width_3: usize,
    /// Clauses of width 4 or 5.
    pub n_width_4_5: usize,
    /// Clauses of width >= 6.
    pub n_width_6plus: usize,

    // ── Polarity shape ──────────────────────────────────────────────────────
    /// Fraction of clauses that are units.
    pub unit_ratio: f64,
    /// Fraction of clauses with at most one positive literal.
    pub horn_ratio: f64,
    /// Fraction of clauses with at most one negative literal.
    pub dual_horn_ratio: f64,
    /// Fraction of clauses with exactly one positive literal.
    pub definite_ratio: f64,
    /// Fraction of clauses with no positive literal.
    pub goal_clause_ratio: f64,
    /// Fraction of literals that are negative.
    pub negative_literal_ratio: f64,

    // ── Equality / variable structure ───────────────────────────────────────
    /// Fraction of literals that are equalities.
    pub equality_literal_ratio: f64,
    /// Number of equality literals.
    pub n_equality_literals: usize,
    /// Fraction of clauses that are ground.
    pub ground_ratio: f64,
    /// Fraction of clauses with a repeated variable inside one literal.
    pub nonlinear_ratio: f64,
    /// Fraction of predicate atoms whose arguments are all *distinct* variables.
    pub fvo_ratio: f64,
    /// Distinct variables across the clause set.
    pub n_variables: usize,
    /// Mean distinct variables per clause.
    pub avg_vars_per_clause: f64,
    /// Maximum distinct variables in one clause.
    pub max_vars_per_clause: usize,

    // ── Symbols ─────────────────────────────────────────────────────────────
    /// Distinct predicate symbols (any arity).
    pub n_predicates: usize,
    /// Distinct function symbols of arity >= 1.
    pub n_functions: usize,
    /// Distinct constants.
    pub n_constants: usize,
    /// Maximum function arity.
    pub max_fun_arity: usize,
    /// Mean arity over function symbols of arity >= 1.
    pub avg_fun_arity: f64,
    /// Maximum predicate arity.
    pub max_pred_arity: usize,
    /// Share of predicate-atom occurrences held by the ten most frequent
    /// predicates. High values mean a small vocabulary, which is what makes an
    /// AVATAR/CWA split profitable.
    pub symbol_concentration: f64,
    /// Gini coefficient of the predicate-atom occurrence distribution.
    pub symbol_gini: f64,
    /// Skolem symbols introduced by our clausification.
    pub n_skolems: usize,
    /// Maximum arity among skolem symbols.
    pub max_skolem_arity: usize,

    // ── Term shape ──────────────────────────────────────────────────────────
    /// Maximum term depth (a constant has depth 1).
    pub max_term_depth: usize,
    /// Mean term depth.
    pub avg_term_depth: f64,
    /// 90th percentile term depth.
    pub p90_term_depth: f64,
    /// Maximum term size in nodes.
    pub max_term_size: usize,
    /// Distinct non-variable terms.
    pub n_distinct_terms: usize,

    // ── Goal topology ───────────────────────────────────────────────────────
    /// Clauses originating from a conjecture.
    pub n_goal_clauses: usize,
    /// Literals in goal clauses.
    pub n_goal_literals: usize,
    /// Maximum term depth inside a goal clause.
    pub goal_max_depth: usize,
    /// Fraction of conjecture symbols that also occur in non-goal clauses.
    pub goal_symbol_overlap: f64,
    /// Conjecture symbols occurring in no non-goal clause.
    pub n_conjecture_only_symbols: usize,
    /// Fraction of non-goal clauses with at least one symbol inside the goal
    /// radius.
    pub goal_reachable_ratio: f64,
    /// Mean goal distance over non-goal clauses, `DISCONNECTED` for the rest.
    pub avg_goal_distance: f64,
    /// Fraction of non-goal clauses with no reachable symbol at all.
    pub goal_unreachable_clause_ratio: f64,

    // ── Decomposition ───────────────────────────────────────────────────────
    /// Connected components of the clause/symbol incidence graph.
    pub n_components: usize,
    /// Clauses in the largest component, as a fraction of all clauses.
    pub largest_component_ratio: f64,
    /// Components containing at least one goal clause.
    pub n_goal_components: usize,

    // ── Measured redundancy ─────────────────────────────────────────────────
    /// Clauses the engine's own preprocessing removes (tautology + PLE + BCE).
    pub n_redundant_removed: usize,
    /// `n_redundant_removed / n_clauses`.
    pub redundant_ratio: f64,
    /// Clauses that are tautologies in the input itself.
    pub n_input_tautologies: usize,
    /// Duplicate input clauses found by canonical form.
    pub n_input_duplicates: usize,

    // ── Signature / algebraic structure ─────────────────────────────────────
    /// Symbols detected as both associative and commutative from the unit axioms.
    pub n_ac_symbols: usize,
    /// An identity axiom `e * x = x` was found.
    pub has_identity_axiom: bool,
    /// An inverse axiom `x * inv(x) = e` was found.
    pub has_inverse_axiom: bool,
    /// An idempotence axiom `x * x = x` was found.
    pub has_idempotence_axiom: bool,
    /// Fraction of clauses that are a single positive equality between two
    /// compound terms — the rewrite-rule density.
    pub rewrite_rule_ratio: f64,

    // ── Propositional abstraction ───────────────────────────────────────────
    /// Distinct atoms after every non-variable subterm is collapsed to one
    /// fresh slot. This is the instance a SAT-based abstraction refiner sees.
    pub abstraction_atoms: usize,
    /// Clauses after the same collapse.
    pub abstraction_clauses: usize,
    /// Fraction of clauses the collapse leaves unchanged.
    pub abstraction_trivial_clause_ratio: f64,
    /// `true` when a bounded counter saturated at [`TERM_CAP`], so
    /// `n_distinct_terms` and `abstraction_atoms` are lower bounds rather than
    /// exact counts.
    pub analysis_capped: bool,

    // ── Header metadata (reported, never routed on) ─────────────────────────
    /// `% Status:` from the TPTP header, when present.
    pub header_status: Option<String>,
    /// `% Rating:` from the TPTP header, when present. **Never used for
    /// routing** — see the module docs.
    pub header_rating: Option<f64>,

    // ── Derived class labels ────────────────────────────────────────────────
    pub logic_class: LogicClass,
    pub shape_class: ShapeClass,
    pub scale_class: ScaleClass,
    pub goal_class: GoalClass,
    pub decomposition_class: DecompositionClass,
    /// A stable slug combining the classes, e.g. `FEQ/NON_HORN/MEDIUM/TIGHT/`.
    pub label: String,
}

impl Analysis {
    /// The coarse logic class as a string.
    pub fn logic(&self) -> &'static str {
        self.logic_class.as_str()
    }
}

/// Analyze a clausified problem.
///
/// `clauses` must be the post-clausification, pre-redundancy-elimination clause
/// set: the view the search engine actually sees, in which the conjecture is
/// already negated and Skolem symbols already exist.
pub fn analyze(
    name: &str,
    meta: &MetaInput,
    clauses: &[Clause],
    symbols: &SymbolTable,
) -> Analysis {
    let mut out = Analysis {
        name: name.to_string(),
        source_domain: source_domain(name),
        input_bytes: meta.input_bytes,
        dialect: if meta.dialect.is_empty() {
            "CNF".to_string()
        } else {
            meta.dialect.clone()
        },
        n_includes: meta.n_includes,
        n_input_formulas: meta.n_input_formulas,
        n_conjecture_formulas: meta.n_conjecture_formulas,
        n_type_formulas: meta.n_type_formulas,
        n_definition_formulas: meta.n_definition_formulas,
        n_input_cnf_clauses: meta.n_input_cnf_clauses,
        header_status: meta.header_status.clone(),
        header_rating: meta.header_rating,
        ..Analysis::default()
    };

    // ── Pass 1: one sweep over the clause set ──────────────────────────────
    let mut facts: Vec<ClauseFacts> = Vec::with_capacity(clauses.len());
    let mut term_acc = TermAcc::default();
    let mut pred_arities: HashMap<SymbolId, HashSet<usize>> = HashMap::default();
    let mut pred_freq: HashMap<SymbolId, usize> = HashMap::default();
    let mut all_vars: HashSet<VarId> = HashSet::default();
    let mut goal_symbols: HashSet<SymbolId> = HashSet::default();
    let mut non_goal_symbols: HashSet<SymbolId> = HashSet::default();
    let mut comm_syms: HashSet<SymbolId> = HashSet::default();
    let mut assoc_syms: HashSet<SymbolId> = HashSet::default();
    let (mut has_identity, mut has_inverse, mut has_idempotence) = (false, false, false);

    let mut n_atoms = 0usize;
    let mut n_fvo_atoms = 0usize;
    let mut total_literals = 0usize;
    let mut total_negative = 0usize;
    let mut total_eq_literals = 0usize;
    let mut sum_vars = 0usize;
    let mut sum_clause_len = 0usize;
    let mut max_vars = 0usize;
    let mut max_len = 0usize;
    let mut max_term_size = 0usize;
    let (mut n_width_1, mut n_width_2, mut n_width_3) = (0usize, 0usize, 0usize);
    let (mut n_width_4_5, mut n_width_6plus) = (0usize, 0usize);
    let (mut n_ground, mut n_nonlinear) = (0usize, 0usize);
    let (mut n_horn, mut n_dual_horn, mut n_definite, mut n_goal_only) =
        (0usize, 0usize, 0usize, 0usize);
    let (mut n_rewrite_rules, mut n_tautologies) = (0usize, 0usize);
    let (mut n_goal_clauses, mut n_goal_literals, mut goal_max_depth) = (0usize, 0usize, 0usize);
    let mut all_unit_eq = true;
    let mut all_eq_only = true;
    let mut any_eq = false;
    // Reused across literals to keep the pass allocation-free per literal:
    // `(symbol, depth, subtree node count)`.
    let mut buffer: Vec<(SymbolId, usize, usize)> = Vec::new();

    for clause in clauses {
        let width = clause.literals.len();
        total_literals += width;
        sum_clause_len += width;
        max_len = max_len.max(width);
        match width {
            0 => {}
            1 => n_width_1 += 1,
            2 => n_width_2 += 1,
            3 => n_width_3 += 1,
            4 | 5 => n_width_4_5 += 1,
            _ => n_width_6plus += 1,
        }
        if width != 1 {
            all_unit_eq = false;
        }
        let is_goal = is_goal_clause(clause);
        if is_goal {
            n_goal_clauses += 1;
            n_goal_literals += width;
        }
        if clause.is_tautology() {
            n_tautologies += 1;
        }

        let mut n_positive = 0usize;
        let mut n_negative = 0usize;
        let mut n_eq = 0usize;
        let mut equality_only = width > 0;
        let mut symbols: Vec<SymbolId> = Vec::with_capacity(4);
        let mut clause_vars: HashSet<VarId> = HashSet::default();
        let mut clause_max_depth = 0usize;
        let mut clause_nonlinear = false;

        for literal in &clause.literals {
            if literal.positive {
                n_positive += 1;
            } else {
                n_negative += 1;
            }
            clause_nonlinear |= literal_is_nonlinear(&literal.atom);
            match &literal.atom {
                Atom::Eq(left, right) => {
                    n_eq += 1;
                    total_eq_literals += 1;
                    any_eq = true;
                    equality_only = false;
                    collect_term_vars(left, &mut clause_vars);
                    collect_term_vars(right, &mut clause_vars);
                    for side in [left, right] {
                        buffer.clear();
                        record_term(side, 1, &mut term_acc, &mut buffer);
                        for &(symbol, depth, size) in buffer.iter() {
                            clause_max_depth = clause_max_depth.max(depth);
                            max_term_size = max_term_size.max(size);
                            if is_goal {
                                goal_symbols.insert(symbol);
                            } else {
                                non_goal_symbols.insert(symbol);
                            }
                            symbols.push(symbol);
                        }
                    }
                    if width == 1 && literal.positive {
                        detect_equational_axiom(
                            left,
                            right,
                            &mut comm_syms,
                            &mut assoc_syms,
                            &mut has_identity,
                            &mut has_inverse,
                            &mut has_idempotence,
                        );
                    }
                }
                Atom::Pred(symbol, args) => {
                    if !args.is_empty() {
                        equality_only = false;
                    }
                    n_atoms += 1;
                    *pred_freq.entry(*symbol).or_insert(0) += 1;
                    pred_arities.entry(*symbol).or_default().insert(args.len());
                    symbols.push(*symbol);
                    if is_goal {
                        goal_symbols.insert(*symbol);
                    } else {
                        non_goal_symbols.insert(*symbol);
                    }
                    let mut seen: HashSet<VarId> = HashSet::default();
                    let mut all_distinct_vars = true;
                    for arg in args {
                        match arg {
                            Term::Var(v) => {
                                if !seen.insert(*v) {
                                    all_distinct_vars = false;
                                }
                            }
                            Term::App(_, _) => all_distinct_vars = false,
                        }
                        collect_term_vars(arg, &mut clause_vars);
                        buffer.clear();
                        record_term(arg, 1, &mut term_acc, &mut buffer);
                        for &(symbol, depth, size) in buffer.iter() {
                            clause_max_depth = clause_max_depth.max(depth);
                            max_term_size = max_term_size.max(size);
                            if is_goal {
                                goal_symbols.insert(symbol);
                            } else {
                                non_goal_symbols.insert(symbol);
                            }
                            symbols.push(symbol);
                        }
                    }
                    if all_distinct_vars {
                        n_fvo_atoms += 1;
                    }
                }
            }
        }

        total_negative += n_negative;
        if n_positive <= 1 {
            n_horn += 1;
        }
        if n_negative <= 1 {
            n_dual_horn += 1;
        }
        if n_positive == 1 {
            n_definite += 1;
        }
        if n_positive == 0 {
            n_goal_only += 1;
        }
        if clause_vars.is_empty() {
            n_ground += 1;
        }
        if clause_nonlinear {
            n_nonlinear += 1;
        }
        if equality_only && n_eq == width && width > 0 {
            // keeps `all_eq_only` true
        } else {
            all_eq_only = false;
        }
        let is_rewrite_rule = width == 1 && n_eq == 1 && n_positive == 1;
        if is_rewrite_rule {
            n_rewrite_rules += 1;
        }
        if is_goal {
            goal_max_depth = goal_max_depth.max(clause_max_depth);
        }
        sum_vars += clause_vars.len();
        max_vars = max_vars.max(clause_vars.len());
        all_vars.extend(clause_vars.iter().copied());

        facts.push(ClauseFacts { symbols, is_goal });
    }

    // ── Scale / shape ratios ───────────────────────────────────────────────
    let n = clauses.len().max(1) as f64;
    out.n_clauses = clauses.len();
    out.n_literals = total_literals;
    out.log_n_clauses = if clauses.is_empty() {
        0.0
    } else {
        (clauses.len() as f64).log10()
    };
    out.avg_clause_len = sum_clause_len as f64 / n;
    out.max_clause_len = max_len;
    out.n_width_1 = n_width_1;
    out.n_width_2 = n_width_2;
    out.n_width_3 = n_width_3;
    out.n_width_4_5 = n_width_4_5;
    out.n_width_6plus = n_width_6plus;
    out.unit_ratio = n_width_1 as f64 / n;
    out.horn_ratio = n_horn as f64 / n;
    out.dual_horn_ratio = n_dual_horn as f64 / n;
    out.definite_ratio = n_definite as f64 / n;
    out.goal_clause_ratio = n_goal_only as f64 / n;
    out.negative_literal_ratio = ratio(total_negative, total_literals);
    out.equality_literal_ratio = ratio(total_eq_literals, total_literals);
    out.n_equality_literals = total_eq_literals;
    out.ground_ratio = n_ground as f64 / n;
    out.nonlinear_ratio = n_nonlinear as f64 / n;
    out.fvo_ratio = if n_atoms == 0 {
        0.0
    } else {
        n_fvo_atoms as f64 / n_atoms as f64
    };
    out.n_variables = all_vars.len();
    out.avg_vars_per_clause = sum_vars as f64 / n;
    out.max_vars_per_clause = max_vars;

    // ── Symbols ────────────────────────────────────────────────────────────
    out.n_predicates = pred_arities.len();
    out.max_pred_arity = pred_arities
        .values()
        .flat_map(|arities| arities.iter().copied())
        .max()
        .unwrap_or(0);
    let n_function_symbols = term_acc.func_arities.values().filter(|&&a| a >= 1).count();
    out.n_functions = n_function_symbols;
    out.n_constants = term_acc.n_constants;
    out.max_fun_arity = max_recorded_arity(&term_acc);
    out.avg_fun_arity = if n_function_symbols == 0 {
        0.0
    } else {
        term_acc.arity_sum as f64 / n_function_symbols as f64
    };

    let mut freq: Vec<usize> = pred_freq.values().copied().collect();
    freq.sort_unstable_by(|a, b| b.cmp(a));
    let freq_total: usize = freq.iter().sum();
    if freq_total > 0 {
        let top: usize = freq.iter().take(10).sum();
        out.symbol_concentration = top as f64 / freq_total as f64;
        let mut weighted = 0usize;
        for (index, count) in freq.iter().enumerate() {
            weighted += count * (index + 1);
        }
        let n_freq = freq.len() as f64;
        let sum = freq_total as f64;
        out.symbol_gini =
            (((2.0 * weighted as f64) / (n_freq * sum)) - ((n_freq + 1.0) / n_freq)).max(0.0);
    }

    let mut n_skolems = 0usize;
    let mut max_skolem_arity = 0usize;
    for (symbol, arity) in &term_acc.func_arities {
        // A symbol the caller never interned cannot be resolved; treat it as a
        // non-Skolem name rather than trusting the index.
        if symbol.index() as usize >= symbols.len() {
            continue;
        }
        if is_skolem_name(symbols.resolve(*symbol)) {
            n_skolems += 1;
            max_skolem_arity = max_skolem_arity.max(*arity);
        }
    }
    out.n_skolems = n_skolems;
    out.max_skolem_arity = max_skolem_arity;

    // ── Term shape ─────────────────────────────────────────────────────────
    let term_occ = term_acc.occurrences.max(1) as f64;
    out.avg_term_depth = term_acc.depth_sum as f64 / term_occ;
    let total_depth_hist: usize = term_acc.depth_hist.iter().sum();
    let p90_target = (total_depth_hist as f64 * 0.9).ceil() as usize;
    let mut cumulative = 0usize;
    for (depth, count) in term_acc.depth_hist.iter().enumerate() {
        cumulative += count;
        if cumulative >= p90_target {
            out.p90_term_depth = depth as f64;
            break;
        }
    }
    out.max_term_depth = term_acc
        .depth_hist
        .iter()
        .rposition(|&count| count > 0)
        .unwrap_or(0);
    out.max_term_size = max_term_size;
    out.n_distinct_terms = term_acc.terms.len();
    out.analysis_capped = term_acc.capped;

    // ── Goal topology ──────────────────────────────────────────────────────
    out.n_goal_clauses = n_goal_clauses;
    out.n_goal_literals = n_goal_literals;
    out.goal_max_depth = goal_max_depth;
    if goal_symbols.is_empty() {
        out.goal_symbol_overlap = 1.0;
    } else {
        let shared = goal_symbols
            .iter()
            .filter(|symbol| non_goal_symbols.contains(*symbol))
            .count();
        out.goal_symbol_overlap = shared as f64 / goal_symbols.len() as f64;
        out.n_conjecture_only_symbols = goal_symbols.len() - shared;
    }
    let distances = symbol_goal_distances(&facts, &goal_symbols);
    let (reachable, unreachable, distance_sum, non_goal_count) = reachability(&facts, &distances);
    if non_goal_count > 0 {
        out.goal_reachable_ratio = reachable as f64 / non_goal_count as f64;
        out.goal_unreachable_clause_ratio = unreachable as f64 / non_goal_count as f64;
        out.avg_goal_distance = distance_sum as f64 / non_goal_count as f64;
    }

    // ── Decomposition ──────────────────────────────────────────────────────
    let (components, largest, goal_components) = components(&facts);

    // ── Measured redundancy (the engine's own reducer) ─────────────────────
    let (removed, duplicates) = measure_redundancy(clauses);

    // ── Propositional abstraction ──────────────────────────────────────────
    let (abs_atoms, abs_clauses, abs_trivial, abs_capped) = propositional_abstraction(clauses);

    out.n_components = components;
    out.largest_component_ratio = largest;
    out.n_goal_components = goal_components;
    out.n_redundant_removed = removed;
    out.redundant_ratio = removed as f64 / n;
    out.n_input_tautologies = n_tautologies;
    out.n_input_duplicates = duplicates;
    out.n_ac_symbols = comm_syms
        .iter()
        .filter(|symbol| assoc_syms.contains(*symbol))
        .count();
    out.has_identity_axiom = has_identity;
    out.has_inverse_axiom = has_inverse;
    out.has_idempotence_axiom = has_idempotence;
    out.rewrite_rule_ratio = n_rewrite_rules as f64 / n;
    out.abstraction_atoms = abs_atoms;
    out.abstraction_clauses = abs_clauses;
    out.abstraction_trivial_clause_ratio = ratio(abs_trivial, abs_clauses);
    out.analysis_capped |= abs_capped;

    // ── Class labels ───────────────────────────────────────────────────────
    out.logic_class = if clauses.is_empty() {
        LogicClass::Empty
    } else if all_unit_eq {
        LogicClass::UnitEquality
    } else if all_eq_only {
        LogicClass::PropositionalEquality
    } else if out.max_fun_arity == 0 {
        LogicClass::EffectivelyPropositional
    } else if !any_eq {
        LogicClass::NonEquational
    } else {
        LogicClass::Equational
    };
    out.shape_class = if clauses.is_empty() {
        ShapeClass::Empty
    } else if n_horn == clauses.len() && n_goal_only == clauses.len() {
        ShapeClass::GoalOnly
    } else if n_horn == clauses.len() {
        ShapeClass::Horn
    } else if n_dual_horn == clauses.len() {
        ShapeClass::DualHorn
    } else if n_horn + n_dual_horn >= clauses.len() * 2 - 1 {
        // Every clause is Horn or dual-Horn: a stratified program. A Horn
        // clause with two or more positive literals fails both tests, so this
        // is the practical statement of "no clause is non-Horn in either
        // polarity".
        ShapeClass::Stratified
    } else {
        ShapeClass::NonHorn
    };
    out.scale_class = match clauses.len() {
        0 => ScaleClass::Empty,
        1..=32 => ScaleClass::Tiny,
        33..=256 => ScaleClass::Small,
        257..=2048 => ScaleClass::Medium,
        2049..=16_384 => ScaleClass::Large,
        _ => ScaleClass::Huge,
    };
    out.goal_class = if clauses.is_empty() {
        GoalClass::Empty
    } else if goal_symbols.is_empty() {
        GoalClass::NoGoal
    } else if out.goal_reachable_ratio >= 0.8 {
        GoalClass::Tight
    } else if out.goal_reachable_ratio >= 0.25 {
        GoalClass::Loose
    } else {
        GoalClass::Background
    };
    out.decomposition_class = if clauses.is_empty() {
        DecompositionClass::Empty
    } else if components <= 1 {
        DecompositionClass::Connected
    } else if goal_components >= 2 {
        DecompositionClass::DisconnectedMultiGoal
    } else {
        DecompositionClass::DisconnectedSingleGoal
    };
    out.label = format!(
        "{}/{}/{}/{}/{}",
        out.logic_class.as_str(),
        out.shape_class.as_str(),
        out.scale_class.as_str(),
        out.goal_class.as_str(),
        out.decomposition_class.as_str()
    );
    out
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn source_domain(name: &str) -> String {
    let file = name.rsplit('/').next().unwrap_or(name);
    let prefix: String = file
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();
    if prefix.is_empty() {
        "SYN".to_string()
    } else {
        prefix.to_uppercase()
    }
}

fn is_skolem_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.first() == Some(&b'$')
        || (bytes.len() >= 3
            && (name.starts_with("esk")
                || name.starts_with("skl")
                || name.starts_with("sK")
                || name.starts_with("sk")))
}

fn is_goal_clause(clause: &Clause) -> bool {
    if clause.distance == 0 {
        return true;
    }
    match &clause.source {
        ClauseSource::Input { role, .. } => {
            role.as_str() == "conjecture" || role.as_str() == "negated_conjecture"
        }
        ClauseSource::Inference { rule, .. } => *rule == "negated_conjecture",
        _ => false,
    }
}

fn collect_term_vars(term: &Term, out: &mut HashSet<VarId>) {
    let mut stack = vec![term];
    while let Some(current) = stack.pop() {
        match current {
            Term::Var(v) => {
                out.insert(*v);
            }
            Term::App(_, args) => stack.extend(args.iter()),
        }
    }
}

/// True when some variable occurs more than once inside one literal.
///
/// A variable occurring once in each of two literals also makes a clause
/// non-linear — that is exactly the propositional-skeleton case — so
/// accumulating across literals would erase the distinction this feature
/// exists to record.
fn literal_is_nonlinear(atom: &Atom) -> bool {
    let mut counts: HashMap<VarId, usize> = HashMap::default();
    match atom {
        Atom::Eq(left, right) => {
            count_term_vars(left, &mut counts);
            count_term_vars(right, &mut counts);
        }
        Atom::Pred(_, args) => {
            for arg in args {
                count_term_vars(arg, &mut counts);
            }
        }
    }
    counts.values().any(|&count| count > 1)
}

fn count_term_vars(term: &Term, out: &mut HashMap<VarId, usize>) {
    let mut stack = vec![term];
    while let Some(current) = stack.pop() {
        match current {
            Term::Var(v) => *out.entry(*v).or_insert(0) += 1,
            Term::App(_, args) => stack.extend(args.iter()),
        }
    }
}

/// Note one term occurrence: depth, arity, size, distinct-term identity, and
/// every function symbol it contains.
///
/// A variable consumes a depth slot (depth 1) so `avg_term_depth` counts
/// variables as depth-1 occurrences, matching how the ordering measures depth.
fn record_term(
    term: &Term,
    depth: usize,
    acc: &mut TermAcc,
    symbols: &mut Vec<(SymbolId, usize, usize)>,
) {
    acc.occurrences += 1;
    acc.depth_sum += depth;
    let bucket = depth.min(acc.depth_hist.len() - 1);
    acc.depth_hist[bucket] += 1;
    let Term::App(symbol, args) = term else {
        return;
    };
    let mut size = 1usize;
    for arg in args {
        size += subtree_size(arg);
    }
    symbols.push((*symbol, depth, size));
    acc.arity_sum += args.len();
    acc.func_arities.insert(*symbol, args.len());
    if args.is_empty() {
        acc.n_constants += 1;
    } else {
        acc.n_functions += 1;
    }
    if acc.terms.len() < TERM_CAP {
        acc.terms.insert(term.clone());
    } else {
        acc.capped = true;
    }
    for arg in args {
        record_term(arg, depth + 1, acc, symbols);
    }
}

fn subtree_size(term: &Term) -> usize {
    match term {
        Term::Var(_) => 1,
        Term::App(_, args) => 1 + args.iter().map(subtree_size).sum::<usize>(),
    }
}

/// Maximum function arity seen in the clause set.
fn max_recorded_arity(acc: &TermAcc) -> usize {
    acc.func_arities.values().copied().max().unwrap_or(0)
}

fn detect_equational_axiom(
    left: &Term,
    right: &Term,
    comm_syms: &mut HashSet<SymbolId>,
    assoc_syms: &mut HashSet<SymbolId>,
    has_identity: &mut bool,
    has_inverse: &mut bool,
    has_idempotence: &mut bool,
) {
    use Term::{App, Var};
    match (left, right) {
        // a * b = b * a
        (App(f1, a1), App(f2, a2))
            if f1 == f2 && a1.len() == 2 && a2.len() == 2 && a1[0] == a2[1] && a1[1] == a2[0] =>
        {
            comm_syms.insert(*f1);
        }
        // (a * b) * c = a * (b * c): the two sides spell the same three
        // factors under the same symbol, adjacent only. `a1`/`a2` are the two
        // outer argument pairs, so the middle factor is shared between
        // `a1[0].args[1]` and `a2[1].args[0]`.
        (App(f1, a1), App(f2, a2))
            if f1 == f2
                && a1.len() == 2
                && a2.len() == 2
                && matches!(
                    (&a1[0], &a2[1]),
                    (App(inner_left, left), App(inner_right, right))
                        if inner_left == f1
                            && inner_right == f1
                            && left.len() == 2
                            && right.len() == 2
                            && left[1] == right[0]
                            && left[0] == a2[0]
                            && right[1] == a1[1]
                ) =>
        {
            assoc_syms.insert(*f1);
        }
        _ => {}
    }
    // e * x = x  (either side)
    for (app, other) in [(left, right), (right, left)] {
        if let App(_, args) = app
            && args.len() == 2
            && args[1] == *other
        {
            *has_identity = true;
        }
    }
    // x * inv(x) = e
    for (app, _) in [(left, right), (right, left)] {
        if let App(_, args) = app
            && args.len() == 2
            && matches!(&args[0], Var(_))
            && matches!(&args[1], App(_, inner) if inner.len() == 1)
        {
            *has_inverse = true;
        }
    }
    // x * x = x
    for (app, other) in [(left, right), (right, left)] {
        if let App(_, args) = app
            && args.len() == 2
            && args[0] == *other
            && args[1] == *other
        {
            *has_idempotence = true;
        }
    }
}

/// BFS from the conjecture symbols over the clause/symbol incidence graph.
///
/// Mirrors `goal_distance`'s rule — a symbol at radius `r+1` shares a clause
/// with a symbol at radius `r` — with the same [`GOAL_RADIUS`] cutoff.
fn symbol_goal_distances(
    facts: &[ClauseFacts],
    goal_symbols: &HashSet<SymbolId>,
) -> HashMap<SymbolId, u8> {
    let mut distances: HashMap<SymbolId, u8> = HashMap::default();
    if goal_symbols.is_empty() {
        return distances;
    }
    for symbol in goal_symbols {
        distances.insert(*symbol, 0);
    }
    let mut radius = 0u8;
    loop {
        let mut grew = false;
        for fact in facts {
            if fact.is_goal {
                continue;
            }
            let mut best = u8::MAX;
            for symbol in &fact.symbols {
                if let Some(distance) = distances.get(symbol) {
                    best = best.min(*distance);
                }
            }
            if best == u8::MAX || best + 1 > GOAL_RADIUS {
                continue;
            }
            for symbol in &fact.symbols {
                match distances.get(symbol) {
                    Some(existing) if *existing <= best + 1 => {}
                    _ => {
                        distances.insert(*symbol, best + 1);
                        grew = true;
                    }
                }
            }
        }
        radius += 1;
        if !grew || radius > GOAL_RADIUS {
            break;
        }
    }
    distances
}

/// `(reachable, unreachable, distance_sum, non_goal_count)` over non-goal clauses.
fn reachability(
    facts: &[ClauseFacts],
    distances: &HashMap<SymbolId, u8>,
) -> (usize, usize, usize, usize) {
    let mut reachable = 0usize;
    let mut unreachable = 0usize;
    let mut distance_sum = 0usize;
    let mut non_goal = 0usize;
    for fact in facts {
        if fact.is_goal {
            continue;
        }
        non_goal += 1;
        let mut best = u8::MAX;
        for symbol in &fact.symbols {
            if let Some(distance) = distances.get(symbol) {
                best = best.min(*distance);
            }
        }
        if best == u8::MAX {
            unreachable += 1;
            distance_sum += DISCONNECTED as usize;
        } else {
            reachable += 1;
            distance_sum += best as usize;
        }
    }
    (reachable, unreachable, distance_sum, non_goal)
}

/// `(components, largest_component_ratio, goal_components)`.
fn components(facts: &[ClauseFacts]) -> (usize, f64, usize) {
    if facts.is_empty() {
        return (0, 0.0, 0);
    }
    let mut ds = DisjointSet::new(facts.len());
    let mut first_use: HashMap<SymbolId, usize> = HashMap::default();
    for (index, fact) in facts.iter().enumerate() {
        for symbol in &fact.symbols {
            match first_use.get(symbol) {
                Some(previous) => ds.union(*previous, index),
                None => {
                    first_use.insert(*symbol, index);
                }
            }
        }
    }
    let mut sizes: HashMap<usize, usize> = HashMap::default();
    let mut goal_roots: HashSet<usize> = HashSet::default();
    for (index, fact) in facts.iter().enumerate() {
        let root = ds.find(index);
        *sizes.entry(root).or_insert(0) += 1;
        if fact.is_goal {
            goal_roots.insert(root);
        }
    }
    (
        sizes.len(),
        sizes.values().copied().max().unwrap_or(0) as f64 / facts.len() as f64,
        goal_roots.len(),
    )
}

/// Run the engine's own redundancy elimination and count duplicate clauses.
///
/// Returns `(removed_by_preprocessing, duplicate_input_clauses)`. Duplicates are
/// counted on a canonical key so the count does not depend on internal variable
/// numbering beyond the renaming the clause set already carries.
fn measure_redundancy(clauses: &[Clause]) -> (usize, usize) {
    use std::hash::{Hash, Hasher};
    let config = preprocessing::PreprocessingConfig::default();
    let (remaining, stats) = preprocessing::preprocess_clauses(clauses, &config);
    let mut seen: HashSet<u64> = HashSet::default();
    let mut duplicates = 0usize;
    for clause in &remaining {
        let mut literals: Vec<u64> = clause
            .literals
            .iter()
            .map(|literal| {
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                literal.positive.hash(&mut hasher);
                canonical_atom(&literal.atom, &mut hasher);
                hasher.finish()
            })
            .collect();
        literals.sort_unstable();
        if seen.len() < TERM_CAP {
            seen.insert(hash_slice(&literals));
        } else if seen.contains(&hash_slice(&literals)) {
            duplicates += 1;
        }
    }
    (stats.total_removed, duplicates)
}

fn hash_slice(values: &[u64]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    values.hash(&mut hasher);
    hasher.finish()
}

/// Canonical hash of an atom. Variable *identity* is deliberately part of the
/// hash: two clauses differing only in which variable a position uses are not
/// duplicates, and folding them together would inflate the duplicate count on
/// exactly the propositional-skeleton inputs where it matters most.
fn canonical_atom(atom: &Atom, hasher: &mut impl std::hash::Hasher) {
    use std::hash::Hash;
    match atom {
        Atom::Pred(symbol, args) => {
            symbol.index().hash(hasher);
            for arg in args {
                canonical_term(arg, hasher);
            }
        }
        Atom::Eq(left, right) => {
            0u8.hash(hasher);
            canonical_term(left, hasher);
            canonical_term(right, hasher);
        }
    }
}

fn canonical_term(term: &Term, hasher: &mut impl std::hash::Hasher) {
    use std::hash::Hash;
    match term {
        Term::Var(v) => {
            1u8.hash(hasher);
            v.hash(hasher);
        }
        Term::App(symbol, args) => {
            2u8.hash(hasher);
            symbol.index().hash(hasher);
            for arg in args {
                canonical_term(arg, hasher);
            }
        }
    }
}

/// Collapse every non-variable subterm to a single fresh slot, then count the
/// resulting atoms and clauses.
///
/// This is the instance a SAT-based abstraction refiner operates on: when it
/// stays tiny, the bottleneck is first-order search rather than the propositional
/// core, which is the distinction a SAT-based pre-pass needs.
fn propositional_abstraction(clauses: &[Clause]) -> (usize, usize, usize, bool) {
    use std::hash::{Hash, Hasher};
    let mut slots: HashMap<&Term, usize> = HashMap::default();
    let mut atoms: HashSet<u64> = HashSet::default();
    let mut abstracted_clauses = 0usize;
    let mut trivial_clauses = 0usize;
    let mut capped = false;
    for clause in clauses {
        abstracted_clauses += 1;
        let mut changed = false;
        let mut signature: Vec<u64> = Vec::with_capacity(clause.literals.len());
        for literal in &clause.literals {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            match &literal.atom {
                Atom::Pred(symbol, args) => {
                    symbol.index().hash(&mut hasher);
                    for arg in args {
                        abstract_term(arg, &mut slots).hash(&mut hasher);
                    }
                    if args.iter().any(|arg| !matches!(arg, Term::Var(_))) {
                        changed = true;
                    }
                }
                Atom::Eq(left, right) => {
                    changed = true;
                    b'#'.hash(&mut hasher);
                    abstract_term(left, &mut slots).hash(&mut hasher);
                    abstract_term(right, &mut slots).hash(&mut hasher);
                }
            }
            // `!key` for a negative literal keeps `p` and `~p` distinct without
            // a second set.
            let key = if literal.positive {
                hasher.finish()
            } else {
                !hasher.finish()
            };
            if atoms.len() < TERM_CAP {
                atoms.insert(key);
            } else {
                capped = true;
            }
            signature.push(key);
        }
        signature.sort_unstable();
        let distinct = {
            let mut copy = signature.clone();
            copy.dedup();
            copy.len()
        };
        if distinct == clause.literals.len() && !changed {
            trivial_clauses += 1;
        }
    }
    (atoms.len(), abstracted_clauses, trivial_clauses, capped)
}

/// `0` for a variable (distinct arguments keep their identity); a stable slot
/// index for every non-variable subterm.
fn abstract_term<'a>(term: &'a Term, slots: &mut HashMap<&'a Term, usize>) -> usize {
    match term {
        Term::Var(_) => 0,
        Term::App(_, _) => {
            let next = slots.len();
            // Borrowed rather than cloned: a 13 MB input expands to millions of
            // subterms and cloning them all is the pre-phase's peak allocation.
            *slots.entry(term).or_insert(next)
        }
    }
}

/// Parse the `% Status:` / `% Rating:` header fields of a TPTP input.
///
/// Exposed separately from [`analyze`] so a caller can read the header without
/// a full parse — and, more importantly, so a caller can choose *not* to use
/// the result.
pub fn parse_header(text: &str) -> (Option<String>, Option<f64>) {
    let mut status = None;
    let mut rating = None;
    for line in text.lines().take(120) {
        let trimmed = line.trim();
        if !trimmed.starts_with('%') {
            if !trimmed.is_empty() {
                break;
            }
            continue;
        }
        let body = trimmed.trim_start_matches('%').trim();
        let Some((key, value)) = body.split_once(':') else {
            continue;
        };
        let token = value.split_whitespace().next().unwrap_or("");
        if token.is_empty() {
            continue;
        }
        match key.trim() {
            "Status" => status = Some(token.to_string()),
            "Rating" => rating = token.parse::<f64>().ok(),
            _ => {}
        }
    }
    (status, rating)
}

impl Analysis {
    /// The CSV column names, in order. `csv_row` emits exactly these.
    pub fn columns() -> &'static [&'static str] {
        Analysis::FIELDS
    }

    fn field(&self, column: &str) -> String {
        let num = |value: f64| format!("{value:.6}");
        match column {
            "name" => quote(&self.name),
            "source_domain" => quote(&self.source_domain),
            "input_bytes" => self.input_bytes.map(|v| v.to_string()).unwrap_or_default(),
            "dialect" => quote(&self.dialect),
            "n_includes" => self.n_includes.to_string(),
            "n_input_formulas" => self.n_input_formulas.to_string(),
            "n_conjecture_formulas" => self.n_conjecture_formulas.to_string(),
            "n_type_formulas" => self.n_type_formulas.to_string(),
            "n_definition_formulas" => self.n_definition_formulas.to_string(),
            "n_clauses" => self.n_clauses.to_string(),
            "n_literals" => self.n_literals.to_string(),
            "n_input_cnf_clauses" => self.n_input_cnf_clauses.to_string(),
            "log_n_clauses" => num(self.log_n_clauses),
            "avg_clause_len" => num(self.avg_clause_len),
            "max_clause_len" => self.max_clause_len.to_string(),
            "n_width_1" => self.n_width_1.to_string(),
            "n_width_2" => self.n_width_2.to_string(),
            "n_width_3" => self.n_width_3.to_string(),
            "n_width_4_5" => self.n_width_4_5.to_string(),
            "n_width_6plus" => self.n_width_6plus.to_string(),
            "unit_ratio" => num(self.unit_ratio),
            "horn_ratio" => num(self.horn_ratio),
            "dual_horn_ratio" => num(self.dual_horn_ratio),
            "definite_ratio" => num(self.definite_ratio),
            "goal_clause_ratio" => num(self.goal_clause_ratio),
            "negative_literal_ratio" => num(self.negative_literal_ratio),
            "equality_literal_ratio" => num(self.equality_literal_ratio),
            "n_equality_literals" => self.n_equality_literals.to_string(),
            "ground_ratio" => num(self.ground_ratio),
            "nonlinear_ratio" => num(self.nonlinear_ratio),
            "fvo_ratio" => num(self.fvo_ratio),
            "n_variables" => self.n_variables.to_string(),
            "avg_vars_per_clause" => num(self.avg_vars_per_clause),
            "max_vars_per_clause" => self.max_vars_per_clause.to_string(),
            "n_predicates" => self.n_predicates.to_string(),
            "n_functions" => self.n_functions.to_string(),
            "n_constants" => self.n_constants.to_string(),
            "max_fun_arity" => self.max_fun_arity.to_string(),
            "avg_fun_arity" => num(self.avg_fun_arity),
            "max_pred_arity" => self.max_pred_arity.to_string(),
            "symbol_concentration" => num(self.symbol_concentration),
            "symbol_gini" => num(self.symbol_gini),
            "n_skolems" => self.n_skolems.to_string(),
            "max_skolem_arity" => self.max_skolem_arity.to_string(),
            "max_term_depth" => self.max_term_depth.to_string(),
            "avg_term_depth" => num(self.avg_term_depth),
            "p90_term_depth" => num(self.p90_term_depth),
            "max_term_size" => self.max_term_size.to_string(),
            "n_distinct_terms" => self.n_distinct_terms.to_string(),
            "n_goal_clauses" => self.n_goal_clauses.to_string(),
            "n_goal_literals" => self.n_goal_literals.to_string(),
            "goal_max_depth" => self.goal_max_depth.to_string(),
            "goal_symbol_overlap" => num(self.goal_symbol_overlap),
            "n_conjecture_only_symbols" => self.n_conjecture_only_symbols.to_string(),
            "goal_reachable_ratio" => num(self.goal_reachable_ratio),
            "avg_goal_distance" => num(self.avg_goal_distance),
            "goal_unreachable_clause_ratio" => num(self.goal_unreachable_clause_ratio),
            "n_components" => self.n_components.to_string(),
            "largest_component_ratio" => num(self.largest_component_ratio),
            "n_goal_components" => self.n_goal_components.to_string(),
            "n_redundant_removed" => self.n_redundant_removed.to_string(),
            "redundant_ratio" => num(self.redundant_ratio),
            "n_input_tautologies" => self.n_input_tautologies.to_string(),
            "n_input_duplicates" => self.n_input_duplicates.to_string(),
            "n_ac_symbols" => self.n_ac_symbols.to_string(),
            "has_identity_axiom" => u8::from(self.has_identity_axiom).to_string(),
            "has_inverse_axiom" => u8::from(self.has_inverse_axiom).to_string(),
            "has_idempotence_axiom" => u8::from(self.has_idempotence_axiom).to_string(),
            "rewrite_rule_ratio" => num(self.rewrite_rule_ratio),
            "abstraction_atoms" => self.abstraction_atoms.to_string(),
            "abstraction_clauses" => self.abstraction_clauses.to_string(),
            "abstraction_trivial_clause_ratio" => num(self.abstraction_trivial_clause_ratio),
            "analysis_capped" => u8::from(self.analysis_capped).to_string(),
            "header_status" => self.header_status.clone().unwrap_or_default(),
            "header_rating" => self
                .header_rating
                .map(|v| v.to_string())
                .unwrap_or_default(),
            "logic_class" => self.logic_class.as_str().to_string(),
            "shape_class" => self.shape_class.as_str().to_string(),
            "scale_class" => self.scale_class.as_str().to_string(),
            "goal_class" => self.goal_class.as_str().to_string(),
            "decomposition_class" => self.decomposition_class.as_str().to_string(),
            "label" => quote(&self.label),
            other => panic!("unknown analysis column {other}"),
        }
    }

    /// The CSV row for this analysis, aligned with [`Analysis::columns`].
    pub fn csv_row(&self) -> String {
        Analysis::FIELDS
            .iter()
            .map(|column| self.field(column))
            .collect::<Vec<_>>()
            .join(",")
    }

    /// The single source of truth for the column order.
    const FIELDS: &'static [&'static str] = &[
        "name",
        "source_domain",
        "input_bytes",
        "dialect",
        "n_includes",
        "n_input_formulas",
        "n_conjecture_formulas",
        "n_type_formulas",
        "n_definition_formulas",
        "n_clauses",
        "n_literals",
        "n_input_cnf_clauses",
        "log_n_clauses",
        "avg_clause_len",
        "max_clause_len",
        "n_width_1",
        "n_width_2",
        "n_width_3",
        "n_width_4_5",
        "n_width_6plus",
        "unit_ratio",
        "horn_ratio",
        "dual_horn_ratio",
        "definite_ratio",
        "goal_clause_ratio",
        "negative_literal_ratio",
        "equality_literal_ratio",
        "n_equality_literals",
        "ground_ratio",
        "nonlinear_ratio",
        "fvo_ratio",
        "n_variables",
        "avg_vars_per_clause",
        "max_vars_per_clause",
        "n_predicates",
        "n_functions",
        "n_constants",
        "max_fun_arity",
        "avg_fun_arity",
        "max_pred_arity",
        "symbol_concentration",
        "symbol_gini",
        "n_skolems",
        "max_skolem_arity",
        "max_term_depth",
        "avg_term_depth",
        "p90_term_depth",
        "max_term_size",
        "n_distinct_terms",
        "n_goal_clauses",
        "n_goal_literals",
        "goal_max_depth",
        "goal_symbol_overlap",
        "n_conjecture_only_symbols",
        "goal_reachable_ratio",
        "avg_goal_distance",
        "goal_unreachable_clause_ratio",
        "n_components",
        "largest_component_ratio",
        "n_goal_components",
        "n_redundant_removed",
        "redundant_ratio",
        "n_input_tautologies",
        "n_input_duplicates",
        "n_ac_symbols",
        "has_identity_axiom",
        "has_inverse_axiom",
        "has_idempotence_axiom",
        "rewrite_rule_ratio",
        "abstraction_atoms",
        "abstraction_clauses",
        "abstraction_trivial_clause_ratio",
        "analysis_capped",
        "header_status",
        "header_rating",
        "logic_class",
        "shape_class",
        "scale_class",
        "goal_class",
        "decomposition_class",
        "label",
    ];
}

fn quote(value: &str) -> String {
    if value.contains(',') || value.contains('"') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::clause::{ClauseId, Literal};

    fn clause(id: u64, lits: Vec<Literal>, distance: u32) -> Clause {
        Clause::new(
            ClauseId(id),
            lits,
            ClauseSource::Input {
                name: format!("c{id}"),
                role: if distance == 0 {
                    "negated_conjecture".to_string()
                } else {
                    "axiom".to_string()
                },
            },
        )
        .with_distance(distance)
    }

    fn analyze_clauses(clauses: &[Clause]) -> Analysis {
        analyze(
            "TEST",
            &MetaInput::default(),
            clauses,
            &mrs_core::SymbolTable::new(),
        )
    }

    fn table() -> mrs_core::SymbolTable {
        mrs_core::SymbolTable::new()
    }

    #[test]
    fn csv_header_and_row_agree_on_width() {
        let analysis = analyze_clauses(&[]);
        assert_eq!(
            Analysis::columns().join(",").split(',').count(),
            analysis.csv_row().split(',').count(),
            "csv header and row must have the same column count"
        );
    }

    #[test]
    fn unit_equality_clause_set_is_ueq() {
        let symbols = table();
        let mut symbols = symbols;
        let f = symbols.intern("multiply");
        let x = symbols.intern("x");
        let y = symbols.intern("y");
        let c = clause(
            1,
            vec![Literal::pos(Atom::eq(
                Term::app(f, vec![Term::constant(x), Term::constant(y)]),
                Term::constant(x),
            ))],
            100,
        );
        let analysis = analyze("TEST", &MetaInput::default(), &[c], &symbols);
        assert_eq!(analysis.logic_class, LogicClass::UnitEquality);
        assert_eq!(analysis.shape_class, ShapeClass::Horn);
    }

    #[test]
    fn disconnected_background_is_detected() {
        let mut symbols = table();
        let p = symbols.intern("p");
        let q = symbols.intern("q");
        let r = symbols.intern("r");
        let x = Term::var(0);
        let clauses = vec![
            clause(1, vec![Literal::pos(Atom::pred(p, vec![x.clone()]))], 0),
            clause(2, vec![Literal::neg(Atom::pred(p, vec![x.clone()]))], 0),
            clause(3, vec![Literal::pos(Atom::pred(q, vec![x.clone()]))], 100),
            clause(4, vec![Literal::pos(Atom::pred(r, vec![x]))], 100),
        ];
        let analysis = analyze("TEST", &MetaInput::default(), &clauses, &symbols);
        // p-links the two goal clauses; q and r are each alone.
        assert_eq!(analysis.n_components, 3);
        assert_eq!(
            analysis.decomposition_class,
            DecompositionClass::DisconnectedSingleGoal
        );
        assert_eq!(analysis.goal_class, GoalClass::Background);
    }

    #[test]
    fn header_status_and_rating_are_read() {
        let text = "%----\n% File     : T\n% Status  : Theorem\n% Rating  : 0.900\n%----\nfof(a, axiom, p).\n";
        let (status, rating) = parse_header(text);
        assert_eq!(status.as_deref(), Some("Theorem"));
        assert_eq!(rating, Some(0.9));
    }

    #[test]
    fn source_domain_comes_from_the_file_name() {
        assert_eq!(source_domain("Problems/GRP/GRS/GRS001+1.p"), "GRS");
        assert_eq!(source_domain("SYN123-1.012"), "SYN");
        assert_eq!(source_domain("1234"), "SYN");
    }

    #[test]
    fn empty_clause_set_is_classified_as_empty() {
        let analysis = analyze_clauses(&[]);
        assert_eq!(analysis.logic_class, LogicClass::Empty);
        assert_eq!(analysis.scale_class, ScaleClass::Empty);
        assert_eq!(analysis.label, "EMPTY/EMPTY/EMPTY/EMPTY/EMPTY");
    }

    #[test]
    fn skolem_symbols_are_recognized() {
        assert!(is_skolem_name("$skolem1"));
        assert!(is_skolem_name("esk1_2"));
        assert!(is_skolem_name("sK3"));
        assert!(!is_skolem_name("multiply"));
        assert!(!is_skolem_name("p"));
    }

    #[test]
    fn commutative_and_associative_axioms_are_recognized() {
        let mut symbols = table();
        let mult = symbols.intern("mult");
        let a = symbols.intern("a");
        let b = symbols.intern("b");
        let c = symbols.intern("c");
        let comm = clause(
            1,
            vec![Literal::pos(Atom::eq(
                Term::app(mult, vec![Term::constant(a), Term::constant(b)]),
                Term::app(mult, vec![Term::constant(b), Term::constant(a)]),
            ))],
            100,
        );
        let assoc = clause(
            2,
            vec![Literal::pos(Atom::eq(
                Term::app(
                    mult,
                    vec![
                        Term::app(mult, vec![Term::constant(a), Term::constant(b)]),
                        Term::constant(c),
                    ],
                ),
                Term::app(
                    mult,
                    vec![
                        Term::constant(a),
                        Term::app(mult, vec![Term::constant(b), Term::constant(c)]),
                    ],
                ),
            ))],
            100,
        );
        let analysis = analyze("TEST", &MetaInput::default(), &[comm, assoc], &symbols);
        assert_eq!(analysis.n_ac_symbols, 1);
    }
}
