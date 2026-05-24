//! Abstract Syntax Tree types for TPTP formulas.
//!
//! This module defines all the data structures representing parsed TPTP content,
//! organized by dialect (CNF, FOF, TFF, TCF, THF) with shared common types.

pub mod cnf;
pub mod common;
mod display;
pub mod fof;
pub mod tcf;
pub mod tff;
pub mod thf;

pub use cnf::*;
pub use common::*;
pub use fof::*;
pub use tcf::*;
pub use tff::*;
pub use thf::*;

use std::collections::HashMap;

/// A parsed TPTP problem — the top-level result of [`parse_tptp`].
///
/// Holds the complete content of one TPTP file: include directives, annotated
/// formulas, and any comments that were associated with named formulas.
///
/// # Filtering helpers
///
/// Use the convenience iterators rather than iterating `formulas` manually:
///
/// ```
/// use mrs_tptp::parse_tptp;
///
/// let input = "fof(ax, axiom, p).  fof(goal, conjecture, q).";
/// let problem = parse_tptp(input).unwrap();
/// assert_eq!(problem.axioms().count(), 1);
/// assert_eq!(problem.conjectures().count(), 1);
/// ```
///
/// [`parse_tptp`]: crate::parse_tptp
#[derive(Debug, Clone, PartialEq)]
pub struct TPTPProblem<'a> {
    /// Include directives (`include('filename', [sel1, sel2]).`).
    pub includes: Vec<Include<'a>>,
    /// All annotated formulas, in source order.
    pub formulas: Vec<AnnotatedFormula<'a>>,
    /// Comments associated with each formula, keyed by formula name.
    ///
    /// Only comments that appear immediately before a named formula are
    /// captured here.  Use [`comments_for`](TPTPProblem::comments_for) for
    /// convenient access.
    pub formula_comments: HashMap<&'a str, Vec<Comment<'a>>>,
}

impl<'a> Default for TPTPProblem<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> TPTPProblem<'a> {
    /// Create an empty [`TPTPProblem`].
    pub fn new() -> Self {
        TPTPProblem {
            includes: Vec::new(),
            formulas: Vec::new(),
            formula_comments: HashMap::new(),
        }
    }

    /// Iterate over all formulas with the given role.
    pub fn formulas_by_role(
        &self,
        role: FormulaRole,
    ) -> impl Iterator<Item = &AnnotatedFormula<'a>> + '_ {
        self.formulas.iter().filter(move |f| f.role() == role)
    }

    /// Iterate over all formulas with role [`FormulaRole::Axiom`].
    pub fn axioms(&self) -> impl Iterator<Item = &AnnotatedFormula<'a>> + '_ {
        self.formulas_by_role(FormulaRole::Axiom)
    }

    /// Iterate over all formulas with role [`FormulaRole::Conjecture`].
    pub fn conjectures(&self) -> impl Iterator<Item = &AnnotatedFormula<'a>> + '_ {
        self.formulas_by_role(FormulaRole::Conjecture)
    }

    /// Iterate over the [`FOFAnnotated`] formulas in this problem.
    pub fn fof_formulas(&self) -> impl Iterator<Item = &FOFAnnotated<'a>> + '_ {
        self.formulas.iter().filter_map(|f| f.as_fof())
    }

    /// Iterate over the [`TFFAnnotated`] formulas in this problem.
    pub fn tff_formulas(&self) -> impl Iterator<Item = &TFFAnnotated<'a>> + '_ {
        self.formulas.iter().filter_map(|f| f.as_tff())
    }

    /// Iterate over the [`THFAnnotated`] formulas in this problem.
    pub fn thf_formulas(&self) -> impl Iterator<Item = &THFAnnotated<'a>> + '_ {
        self.formulas.iter().filter_map(|f| f.as_thf())
    }

    /// Iterate over the [`CNFAnnotated`] formulas in this problem.
    pub fn cnf_formulas(&self) -> impl Iterator<Item = &CNFAnnotated<'a>> + '_ {
        self.formulas.iter().filter_map(|f| f.as_cnf())
    }

    /// Iterate over the [`TCFAnnotated`] formulas in this problem.
    pub fn tcf_formulas(&self) -> impl Iterator<Item = &TCFAnnotated<'a>> + '_ {
        self.formulas.iter().filter_map(|f| f.as_tcf())
    }

    /// Return the comments associated with the formula named `name`, if any.
    pub fn comments_for(&self, name: &str) -> &[Comment<'a>] {
        self.formula_comments
            .get(name)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

/// An include directive: `include('Axioms/SET001+0.ax', [sel1, sel2]).`
#[derive(Debug, Clone, PartialEq)]
pub struct Include<'a> {
    /// The file path (without surrounding quotes).
    pub file_name: &'a str,
    /// Optional formula-name selection list.  `None` means "include all".
    pub selection: Option<Vec<common::Name<'a>>>,
}

/// A comment in a TPTP file (`% line comment` or `/* block comment */`).
#[derive(Debug, Clone, PartialEq)]
pub struct Comment<'a> {
    /// The comment text, without delimiters.
    pub content: &'a str,
    /// `true` for `/* … */` block comments, `false` for `% …` line comments.
    pub is_block: bool,
}

/// A top-level annotated formula — one of the six TPTP dialects.
///
/// Use [`as_fof`](AnnotatedFormula::as_fof), [`as_tff`](AnnotatedFormula::as_tff),
/// etc. to downcast, or the `is_*` predicates to test the variant.
#[derive(Debug, Clone, PartialEq)]
pub enum AnnotatedFormula<'a> {
    /// `thf(name, role, formula [, annotations]).`
    THF(THFAnnotated<'a>),
    /// `tff(name, role, formula [, annotations]).`
    TFF(TFFAnnotated<'a>),
    /// `fof(name, role, formula [, annotations]).`
    FOF(FOFAnnotated<'a>),
    /// `tcf(name, role, formula [, annotations]).`
    TCF(TCFAnnotated<'a>),
    /// `cnf(name, role, formula [, annotations]).`
    CNF(CNFAnnotated<'a>),
    /// `tpi(name, role, formula [, annotations]).`
    TPI(TPIAnnotated<'a>),
}

impl<'a> AnnotatedFormula<'a> {
    /// Get the name of this formula
    pub fn name(&self) -> &'a str {
        match self {
            AnnotatedFormula::THF(f) => f.name.as_str(),
            AnnotatedFormula::TFF(f) => f.name.as_str(),
            AnnotatedFormula::FOF(f) => f.name.as_str(),
            AnnotatedFormula::TCF(f) => f.name.as_str(),
            AnnotatedFormula::CNF(f) => f.name.as_str(),
            AnnotatedFormula::TPI(f) => f.name.as_str(),
        }
    }

    /// Get the role of this formula
    pub fn role(&self) -> FormulaRole {
        match self {
            AnnotatedFormula::THF(f) => f.role,
            AnnotatedFormula::TFF(f) => f.role,
            AnnotatedFormula::FOF(f) => f.role,
            AnnotatedFormula::TCF(f) => f.name_to_role(),
            AnnotatedFormula::CNF(f) => f.role,
            AnnotatedFormula::TPI(f) => f.role,
        }
    }

    /// Return the annotations attached to this formula, if any.
    pub fn annotations(&self) -> Option<&Annotations<'a>> {
        match self {
            AnnotatedFormula::THF(f) => f.annotations.as_ref(),
            AnnotatedFormula::TFF(f) => f.annotations.as_ref(),
            AnnotatedFormula::FOF(f) => f.annotations.as_ref(),
            AnnotatedFormula::TCF(f) => f.annotations.as_ref(),
            AnnotatedFormula::CNF(f) => f.annotations.as_ref(),
            AnnotatedFormula::TPI(f) => f.annotations.as_ref(),
        }
    }

    /// Return a reference to the inner [`FOFAnnotated`], or `None` if this is
    /// a different dialect.
    pub fn as_fof(&self) -> Option<&FOFAnnotated<'a>> {
        if let AnnotatedFormula::FOF(f) = self {
            Some(f)
        } else {
            None
        }
    }

    /// Return a reference to the inner [`TFFAnnotated`], or `None`.
    pub fn as_tff(&self) -> Option<&TFFAnnotated<'a>> {
        if let AnnotatedFormula::TFF(f) = self {
            Some(f)
        } else {
            None
        }
    }

    /// Return a reference to the inner [`THFAnnotated`], or `None`.
    pub fn as_thf(&self) -> Option<&THFAnnotated<'a>> {
        if let AnnotatedFormula::THF(f) = self {
            Some(f)
        } else {
            None
        }
    }

    /// Return a reference to the inner [`CNFAnnotated`], or `None`.
    pub fn as_cnf(&self) -> Option<&CNFAnnotated<'a>> {
        if let AnnotatedFormula::CNF(f) = self {
            Some(f)
        } else {
            None
        }
    }

    /// Return a reference to the inner [`TCFAnnotated`], or `None`.
    pub fn as_tcf(&self) -> Option<&TCFAnnotated<'a>> {
        if let AnnotatedFormula::TCF(f) = self {
            Some(f)
        } else {
            None
        }
    }

    /// Return a reference to the inner [`TPIAnnotated`], or `None`.
    pub fn as_tpi(&self) -> Option<&TPIAnnotated<'a>> {
        if let AnnotatedFormula::TPI(f) = self {
            Some(f)
        } else {
            None
        }
    }

    /// Return `true` if this is a FOF formula.
    pub fn is_fof(&self) -> bool {
        matches!(self, AnnotatedFormula::FOF(_))
    }

    /// Return `true` if this is a TFF formula.
    pub fn is_tff(&self) -> bool {
        matches!(self, AnnotatedFormula::TFF(_))
    }

    /// Return `true` if this is a THF formula.
    pub fn is_thf(&self) -> bool {
        matches!(self, AnnotatedFormula::THF(_))
    }

    /// Return `true` if this is a CNF formula.
    pub fn is_cnf(&self) -> bool {
        matches!(self, AnnotatedFormula::CNF(_))
    }

    /// Return `true` if this is a TCF formula.
    pub fn is_tcf(&self) -> bool {
        matches!(self, AnnotatedFormula::TCF(_))
    }
}

/// A `thf(…)` annotated formula.
#[derive(Debug, Clone, PartialEq)]
pub struct THFAnnotated<'a> {
    /// Formula name.
    pub name: common::Name<'a>,
    /// Formula role (axiom, conjecture, type, …).
    pub role: FormulaRole,
    /// The THF formula body.
    pub formula: thf::THFStatement<'a>,
    /// Optional source/useful-info annotations.
    pub annotations: Option<Annotations<'a>>,
}

/// A `tff(…)` annotated formula.
#[derive(Debug, Clone, PartialEq)]
pub struct TFFAnnotated<'a> {
    /// Formula name.
    pub name: common::Name<'a>,
    /// Formula role (axiom, conjecture, type, …).
    pub role: FormulaRole,
    /// The TFF formula body.
    pub formula: tff::TFFStatement<'a>,
    /// Optional source/useful-info annotations.
    pub annotations: Option<Annotations<'a>>,
}

/// A `fof(…)` annotated formula.
#[derive(Debug, Clone, PartialEq)]
pub struct FOFAnnotated<'a> {
    /// Formula name.
    pub name: common::Name<'a>,
    /// Formula role (axiom, conjecture, …).
    pub role: FormulaRole,
    /// The FOF formula body.
    pub formula: fof::FOFStatement<'a>,
    /// Optional source/useful-info annotations.
    pub annotations: Option<Annotations<'a>>,
}

/// A `tcf(…)` annotated formula.
#[derive(Debug, Clone, PartialEq)]
pub struct TCFAnnotated<'a> {
    /// Formula name.
    pub name: common::Name<'a>,
    /// Formula role (axiom, conjecture, …).
    pub role: FormulaRole,
    /// The TCF formula body.
    pub formula: tcf::TCFStatement<'a>,
    /// Optional source/useful-info annotations.
    pub annotations: Option<Annotations<'a>>,
}

impl<'a> TCFAnnotated<'a> {
    fn name_to_role(&self) -> FormulaRole {
        self.role
    }
}

/// A `cnf(…)` annotated formula.
#[derive(Debug, Clone, PartialEq)]
pub struct CNFAnnotated<'a> {
    /// Formula name.
    pub name: common::Name<'a>,
    /// Formula role (axiom, negated_conjecture, …).
    pub role: FormulaRole,
    /// The CNF clause body.
    pub formula: cnf::CNFStatement<'a>,
    /// Optional source/useful-info annotations.
    pub annotations: Option<Annotations<'a>>,
}

/// A `tpi(…)` annotated formula (TPTP Process Instruction).
#[derive(Debug, Clone, PartialEq)]
pub struct TPIAnnotated<'a> {
    /// Formula name.
    pub name: common::Name<'a>,
    /// Formula role.
    pub role: FormulaRole,
    /// The formula body (FOF grammar).
    pub formula: fof::FOFStatement<'a>,
    /// Optional source/useful-info annotations.
    pub annotations: Option<Annotations<'a>>,
}

/// The role of an annotated formula (the second positional argument in TPTP).
///
/// Use the predicate methods — [`is_axiom`](FormulaRole::is_axiom),
/// [`is_conjecture`](FormulaRole::is_conjecture),
/// [`is_premise`](FormulaRole::is_premise), etc. — instead of pattern-matching
/// on variants to future-proof your code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FormulaRole {
    /// `axiom` — an unconditional background fact.
    Axiom,
    /// `axiom-local` — an axiom local to its include file.
    AxiomLocal,
    /// `hypothesis` — an assumed fact for the current problem.
    Hypothesis,
    /// `definition` — definitional axiom.
    Definition,
    /// `assumption` — assumed (possibly temporarily) for a sub-proof.
    Assumption,
    /// `lemma` — a derived fact used as a step.
    Lemma,
    /// `theorem` — a proved result.
    Theorem,
    /// `corollary` — a result following from a theorem.
    Corollary,
    /// `conjecture` — the proof goal to be established.
    Conjecture,
    /// `negated_conjecture` — the negation of the conjecture (for refutation).
    NegatedConjecture,
    /// `plain` — no special role.
    Plain,
    /// `type` — a type declaration (`p: $i > $o`).
    Type,
    /// `interpretation` — a model interpretation.
    Interpretation,
    /// `fi_domain` — finite interpretation domain.
    FiDomain,
    /// `fi_functors` — finite interpretation functors.
    FiFunctors,
    /// `fi_predicates` — finite interpretation predicates.
    FiPredicates,
    /// `logic` — a logic specification.
    Logic,
    /// `unknown` — role not recognised.
    Unknown,
}

impl FormulaRole {
    /// Parse a [`FormulaRole`] from its TPTP keyword string.
    ///
    /// Returns `None` for unrecognised strings.
    pub fn parse(s: &str) -> Option<FormulaRole> {
        match s {
            "axiom" => Some(FormulaRole::Axiom),
            "axiom-local" => Some(FormulaRole::AxiomLocal),
            "hypothesis" => Some(FormulaRole::Hypothesis),
            "definition" => Some(FormulaRole::Definition),
            "assumption" => Some(FormulaRole::Assumption),
            "lemma" => Some(FormulaRole::Lemma),
            "theorem" => Some(FormulaRole::Theorem),
            "corollary" => Some(FormulaRole::Corollary),
            "conjecture" => Some(FormulaRole::Conjecture),
            "negated_conjecture" => Some(FormulaRole::NegatedConjecture),
            "plain" => Some(FormulaRole::Plain),
            "type" => Some(FormulaRole::Type),
            "interpretation" => Some(FormulaRole::Interpretation),
            "fi_domain" => Some(FormulaRole::FiDomain),
            "fi_functors" => Some(FormulaRole::FiFunctors),
            "fi_predicates" => Some(FormulaRole::FiPredicates),
            "logic" => Some(FormulaRole::Logic),
            "unknown" => Some(FormulaRole::Unknown),
            _ => None,
        }
    }

    /// Return the TPTP keyword string for this role (e.g. `"axiom"`).
    pub fn as_str(&self) -> &'static str {
        match self {
            FormulaRole::Axiom => "axiom",
            FormulaRole::AxiomLocal => "axiom-local",
            FormulaRole::Hypothesis => "hypothesis",
            FormulaRole::Definition => "definition",
            FormulaRole::Assumption => "assumption",
            FormulaRole::Lemma => "lemma",
            FormulaRole::Theorem => "theorem",
            FormulaRole::Corollary => "corollary",
            FormulaRole::Conjecture => "conjecture",
            FormulaRole::NegatedConjecture => "negated_conjecture",
            FormulaRole::Plain => "plain",
            FormulaRole::Type => "type",
            FormulaRole::Interpretation => "interpretation",
            FormulaRole::FiDomain => "fi_domain",
            FormulaRole::FiFunctors => "fi_functors",
            FormulaRole::FiPredicates => "fi_predicates",
            FormulaRole::Logic => "logic",
            FormulaRole::Unknown => "unknown",
        }
    }

    // -----------------------------------------------------------------------
    // Single-role predicates
    // -----------------------------------------------------------------------

    /// `true` for `axiom` or `axiom-local`.
    pub fn is_axiom(self) -> bool {
        matches!(self, FormulaRole::Axiom | FormulaRole::AxiomLocal)
    }

    /// `true` for `hypothesis`.
    pub fn is_hypothesis(self) -> bool {
        self == FormulaRole::Hypothesis
    }

    /// `true` for `assumption`.
    pub fn is_assumption(self) -> bool {
        self == FormulaRole::Assumption
    }

    /// `true` for `definition`.
    pub fn is_definition(self) -> bool {
        self == FormulaRole::Definition
    }

    /// `true` for `lemma`.
    pub fn is_lemma(self) -> bool {
        self == FormulaRole::Lemma
    }

    /// `true` for `theorem`.
    pub fn is_theorem(self) -> bool {
        self == FormulaRole::Theorem
    }

    /// `true` for `corollary`.
    pub fn is_corollary(self) -> bool {
        self == FormulaRole::Corollary
    }

    /// `true` for `conjecture`.
    pub fn is_conjecture(self) -> bool {
        self == FormulaRole::Conjecture
    }

    /// `true` for `negated_conjecture`.
    pub fn is_negated_conjecture(self) -> bool {
        self == FormulaRole::NegatedConjecture
    }

    /// `true` for `type` declarations (e.g. `p: $i > $o`).
    pub fn is_type_declaration(self) -> bool {
        self == FormulaRole::Type
    }

    // -----------------------------------------------------------------------
    // Group predicates
    // -----------------------------------------------------------------------

    /// `true` for `conjecture` or `negated_conjecture` — the formula is a proof goal.
    pub fn is_goal(self) -> bool {
        matches!(
            self,
            FormulaRole::Conjecture | FormulaRole::NegatedConjecture
        )
    }

    /// `true` for roles that act as premises in a proof:
    /// `axiom`, `axiom-local`, `hypothesis`, `assumption`, `definition`,
    /// `lemma`, `theorem`, and `corollary`.
    pub fn is_premise(self) -> bool {
        matches!(
            self,
            FormulaRole::Axiom
                | FormulaRole::AxiomLocal
                | FormulaRole::Hypothesis
                | FormulaRole::Assumption
                | FormulaRole::Definition
                | FormulaRole::Lemma
                | FormulaRole::Theorem
                | FormulaRole::Corollary
        )
    }

    /// `true` for derived facts: `lemma`, `theorem`, `corollary`.
    pub fn is_derived(self) -> bool {
        matches!(
            self,
            FormulaRole::Lemma | FormulaRole::Theorem | FormulaRole::Corollary
        )
    }
}

/// Optional annotations on an annotated formula — source and useful_info.
///
/// In TPTP syntax: `fof(name, role, formula, source, useful_info).`
#[derive(Debug, Clone, PartialEq)]
pub struct Annotations<'a> {
    /// The `source` general term (e.g. `file('foo.p', axiom_name)`).
    pub source: GeneralTerm<'a>,
    /// Optional `useful_info` list.
    pub useful_info: Option<Vec<GeneralTerm<'a>>>,
}
