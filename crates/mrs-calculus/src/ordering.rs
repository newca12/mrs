//! Term orderings for the superposition calculus.
//!
//! Two reduction orderings are supported:
//! - **KBO** (Knuth-Bendix Ordering): weight-based, fast, good for many problems.
//! - **LPO** (Lexicographic Path Ordering): precedence-based, better for some
//!   equational problems where KBO cannot orient key equalities.
//!
//! Both are used to orient equalities and determine which inferences are
//! necessary for completeness.

use std::collections::HashMap;
use std::sync::Arc;

use mrs_core::SymbolId;
use mrs_core::term::{Term, VarId};

/// Configuration for symbol precedence and weights.
/// This can be generated based on the problem's symbol frequencies.
#[derive(Clone, Debug)]
pub struct SymbolConfig {
    /// Maps SymbolId.0 to its precedence. Higher is greater.
    pub precedence: Vec<u32>,
    /// Maps SymbolId.0 to its weight.
    pub weights: Vec<u32>,
    /// Default weight for variables and unknown symbols.
    pub w0: u32,
}

impl Default for SymbolConfig {
    fn default() -> Self {
        Self {
            precedence: Vec::new(),
            weights: Vec::new(),
            w0: 1,
        }
    }
}

impl SymbolConfig {
    pub fn symbol_weight(&self, s: SymbolId) -> u32 {
        self.weights
            .get(s.index() as usize)
            .copied()
            .unwrap_or(self.w0)
    }

    pub fn symbol_precedence(&self, s: SymbolId) -> u32 {
        self.precedence
            .get(s.index() as usize)
            .copied()
            .unwrap_or(s.index())
    }
}

/// Result of comparing two terms under a reduction ordering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TermComparison {
    /// The first term is strictly greater.
    Greater,
    /// The first term is strictly less.
    Less,
    /// The terms are equal.
    Equal,
    /// The terms are incomparable (neither is greater).
    Incomparable,
}

/// Knuth-Bendix Ordering (KBO).
///
/// A simplification ordering on terms defined by:
/// - A weight function assigning a positive integer to each function symbol
/// - A precedence (total order) on function symbols
/// - Variable weight `w0` (must be the minimum weight of any symbol)
///
/// Default configuration: all symbols have weight 1, precedence is by `SymbolId` value.
#[derive(Clone, Debug)]
pub struct KBO {
    config: Arc<SymbolConfig>,
}

impl KBO {
    /// Creates a KBO with default weights (all symbols and variables have weight 1).
    pub fn new() -> Self {
        Self {
            config: Arc::new(SymbolConfig::default()),
        }
    }

    pub fn with_config(config: Arc<SymbolConfig>) -> Self {
        Self { config }
    }

    /// Returns the weight of a function symbol.
    fn symbol_weight(&self, s: SymbolId) -> u32 {
        self.config.symbol_weight(s)
    }

    /// Computes the total weight of a term.
    fn weight(&self, t: &Term) -> u32 {
        match t {
            Term::Var(_) => self.config.w0,
            Term::App(f, args) => {
                self.symbol_weight(*f) + args.iter().map(|a| self.weight(a)).sum::<u32>()
            }
        }
    }

    fn weight_id(&self, t: mrs_core::term_bank::TermId, bank: &mrs_core::term_bank::TermBank) -> u32 {
        match bank.get(t) {
            mrs_core::term_bank::TermNode::Var(_) => self.config.w0,
            mrs_core::term_bank::TermNode::App(f, args) => {
                self.symbol_weight(*f) + args.iter().map(|&a| self.weight_id(a, bank)).sum::<u32>()
            }
        }
    }

    /// Counts occurrences of each variable in a term.
    fn var_counts(t: &Term) -> HashMap<VarId, i32> {
        let mut counts = HashMap::new();
        Self::collect_var_counts(t, &mut counts);
        counts
    }

    fn collect_var_counts(t: &Term, counts: &mut HashMap<VarId, i32>) {
        match t {
            Term::Var(v) => {
                *counts.entry(*v).or_insert(0) += 1;
            }
            Term::App(_, args) => {
                for arg in args {
                    Self::collect_var_counts(arg, counts);
                }
            }
        }
    }

    fn var_counts_id(t: mrs_core::term_bank::TermId, bank: &mrs_core::term_bank::TermBank) -> HashMap<VarId, i32> {
        let mut counts = HashMap::new();
        Self::collect_var_counts_id(t, bank, &mut counts);
        counts
    }

    fn collect_var_counts_id(t: mrs_core::term_bank::TermId, bank: &mrs_core::term_bank::TermBank, counts: &mut HashMap<VarId, i32>) {
        match bank.get(t) {
            mrs_core::term_bank::TermNode::Var(v) => {
                *counts.entry(*v).or_insert(0) += 1;
            }
            mrs_core::term_bank::TermNode::App(_, args) => {
                for &arg in args {
                    Self::collect_var_counts_id(arg, bank, counts);
                }
            }
        }
    }

    pub fn compare_id(&self, s: mrs_core::term_bank::TermId, t: mrs_core::term_bank::TermId, bank: &mrs_core::term_bank::TermBank) -> TermComparison {
        if s == t {
            return TermComparison::Equal;
        }

        let s_counts = Self::var_counts_id(s, bank);
        let t_counts = Self::var_counts_id(t, bank);

        let s_ge_t_vars = t_counts
            .iter()
            .all(|(v, &ct)| s_counts.get(v).copied().unwrap_or(0) >= ct);
        let t_ge_s_vars = s_counts
            .iter()
            .all(|(v, &cs)| t_counts.get(v).copied().unwrap_or(0) >= cs);

        let ws = self.weight_id(s, bank);
        let wt = self.weight_id(t, bank);

        if ws > wt && s_ge_t_vars {
            return TermComparison::Greater;
        }
        if wt > ws && t_ge_s_vars {
            return TermComparison::Less;
        }
        if ws != wt {
            return TermComparison::Incomparable;
        }

        match (bank.get(s), bank.get(t)) {
            (mrs_core::term_bank::TermNode::App(f1, args1), mrs_core::term_bank::TermNode::App(f2, args2)) => {
                if f1 != f2 {
                    let prec1 = self.config.symbol_precedence(*f1);
                    let prec2 = self.config.symbol_precedence(*f2);
                    if s_ge_t_vars && prec1 > prec2 {
                        return TermComparison::Greater;
                    }
                    if t_ge_s_vars && prec2 > prec1 {
                        return TermComparison::Less;
                    }
                    return TermComparison::Incomparable;
                }

                for (&a1, &a2) in args1.iter().zip(args2.iter()) {
                    let cmp = self.compare_id(a1, a2, bank);
                    match cmp {
                        TermComparison::Equal => continue,
                        TermComparison::Greater => {
                            if s_ge_t_vars {
                                return TermComparison::Greater;
                            } else {
                                return TermComparison::Incomparable;
                            }
                        }
                        TermComparison::Less => {
                            if t_ge_s_vars {
                                return TermComparison::Less;
                            } else {
                                return TermComparison::Incomparable;
                            }
                        }
                        TermComparison::Incomparable => return TermComparison::Incomparable,
                    }
                }
                TermComparison::Equal
            }
            _ => TermComparison::Incomparable,
        }
    }

    /// Compares two terms under KBO.
    ///
    /// Returns `Greater` if `s > t`, `Less` if `s < t`,
    /// `Equal` if `s = t`, or `Incomparable` if neither is greater.
    ///
    /// KBO rules:
    /// 1. Variable condition: every variable in `t` must occur at least
    ///    as many times in `s` (for `s > t`).
    /// 2. If `weight(s) > weight(t)` and variable condition holds: `s > t`.
    /// 3. If `weight(s) = weight(t)` and same top symbol: compare args lexicographically.
    /// 4. If `weight(s) = weight(t)` and different top symbols: compare by precedence.
    pub fn compare(&self, s: &Term, t: &Term) -> TermComparison {
        if s == t {
            return TermComparison::Equal;
        }

        let s_counts = Self::var_counts(s);
        let t_counts = Self::var_counts(t);

        // Check variable condition in both directions
        let s_ge_t_vars = t_counts
            .iter()
            .all(|(v, &ct)| s_counts.get(v).copied().unwrap_or(0) >= ct);
        let t_ge_s_vars = s_counts
            .iter()
            .all(|(v, &cs)| t_counts.get(v).copied().unwrap_or(0) >= cs);

        let ws = self.weight(s);
        let wt = self.weight(t);

        if ws > wt && s_ge_t_vars {
            return TermComparison::Greater;
        }
        if wt > ws && t_ge_s_vars {
            return TermComparison::Less;
        }
        if ws != wt {
            // Weights differ but variable condition fails
            return TermComparison::Incomparable;
        }

        // Equal weights — compare by top symbol and then arguments
        match (s, t) {
            (Term::App(f1, args1), Term::App(f2, args2)) => {
                if f1 != f2 {
                    let prec1 = self.config.symbol_precedence(*f1);
                    let prec2 = self.config.symbol_precedence(*f2);
                    // Precedence comparison: higher = higher precedence
                    if s_ge_t_vars && prec1 > prec2 {
                        return TermComparison::Greater;
                    }
                    if t_ge_s_vars && prec2 > prec1 {
                        return TermComparison::Less;
                    }
                    return TermComparison::Incomparable;
                }

                // Same symbol: lexicographic comparison of arguments
                for (a1, a2) in args1.iter().zip(args2.iter()) {
                    let cmp = self.compare(a1, a2);
                    match cmp {
                        TermComparison::Equal => continue,
                        TermComparison::Greater if s_ge_t_vars => return TermComparison::Greater,
                        TermComparison::Less if t_ge_s_vars => return TermComparison::Less,
                        _ => return TermComparison::Incomparable,
                    }
                }
                // All args equal but terms aren't equal (shouldn't happen if lengths match)
                TermComparison::Incomparable
            }
            // A variable vs non-variable with equal weight is incomparable
            // (unless it's a unary symbol with the var as argument,
            //  but variable condition prevents that from being Greater)
            _ => TermComparison::Incomparable,
        }
    }
}

impl Default for KBO {
    fn default() -> Self {
        Self::new()
    }
}

/// Lexicographic Path Ordering (LPO).
///
/// A simplification ordering based purely on a precedence over function symbols.
/// Unlike KBO, LPO does not use weights, which allows it to orient equalities
/// that KBO cannot (e.g., when both sides have the same weight).
///
/// LPO is particularly effective for equational problems involving
/// associativity, commutativity, and distributivity.
///
/// Precedence: by configured `SymbolConfig` or default (SymbolId value).
#[derive(Clone, Debug)]
pub struct LPO {
    config: Arc<SymbolConfig>,
}

impl LPO {
    /// Creates an LPO with default precedence (by SymbolId value).
    pub fn new() -> Self {
        Self {
            config: Arc::new(SymbolConfig::default()),
        }
    }

    pub fn with_config(config: Arc<SymbolConfig>) -> Self {
        Self { config }
    }

    pub fn compare_id(&self, s: mrs_core::term_bank::TermId, t: mrs_core::term_bank::TermId, bank: &mrs_core::term_bank::TermBank) -> TermComparison {
        if s == t {
            return TermComparison::Equal;
        }
        if self.lpo_gt_id(s, t, bank) {
            TermComparison::Greater
        } else if self.lpo_gt_id(t, s, bank) {
            TermComparison::Less
        } else {
            TermComparison::Incomparable
        }
    }

    /// Returns true if s >_lpo t.
    fn lpo_gt_id(&self, s: mrs_core::term_bank::TermId, t: mrs_core::term_bank::TermId, bank: &mrs_core::term_bank::TermBank) -> bool {
        // Case 1: t is a variable occurring in s (and s ≠ t)
        if let mrs_core::term_bank::TermNode::Var(v) = bank.get(t) {
            if s == t {
                return false;
            }
            return occurs_in_id(*v, s, bank);
        }

        match bank.get(s) {
            mrs_core::term_bank::TermNode::Var(_) => {
                false
            }
            mrs_core::term_bank::TermNode::App(f, s_args) => {
                // Case 2a: some si ≥_lpo t (subterm property)
                for &si in s_args {
                    if si == t || self.lpo_gt_id(si, t, bank) {
                        return true;
                    }
                }

                match bank.get(t) {
                    mrs_core::term_bank::TermNode::App(g, t_args) => {
                        let s_gt_all_tj = t_args.iter().all(|&tj| self.lpo_gt_id(s, tj, bank));
                        if !s_gt_all_tj {
                            return false;
                        }

                        let prec_f = self.config.symbol_precedence(*f);
                        let prec_g = self.config.symbol_precedence(*g);

                        if prec_f > prec_g {
                            true
                        } else if prec_f == prec_g {
                            self.lex_gt_id(s_args, t_args, bank)
                        } else {
                            false
                        }
                    }
                    mrs_core::term_bank::TermNode::Var(_) => {
                        unreachable!()
                    }
                }
            }
        }
    }

    fn lex_gt_id(&self, args_s: &[mrs_core::term_bank::TermId], args_t: &[mrs_core::term_bank::TermId], bank: &mrs_core::term_bank::TermBank) -> bool {
        for (&si, &ti) in args_s.iter().zip(args_t.iter()) {
            if si == ti {
                continue;
            }
            return self.lpo_gt_id(si, ti, bank);
        }
        args_s.len() > args_t.len()
    }

    /// Compares two terms under LPO.
    ///
    /// s >_lpo t iff:
    /// 1. t is a variable occurring in s (and s ≠ t), or
    /// 2. s = f(s1,...,sn) and:
    ///    a. some si ≥_lpo t (subterm property), or
    ///    b. t = g(t1,...,tm) and f ≻ g and s >_lpo all tj, or
    ///    c. t = f(t1,...,tm) and (s1,...,sn) >_lpo_lex (t1,...,tm)
    ///    and s >_lpo all tj.
    pub fn compare(&self, s: &Term, t: &Term) -> TermComparison {
        if s == t {
            return TermComparison::Equal;
        }
        if self.lpo_gt(s, t) {
            TermComparison::Greater
        } else if self.lpo_gt(t, s) {
            TermComparison::Less
        } else {
            TermComparison::Incomparable
        }
    }

    /// Returns true if s >_lpo t.
    fn lpo_gt(&self, s: &Term, t: &Term) -> bool {
        // Case 1: t is a variable occurring in s (and s ≠ t)
        if let Term::Var(v) = t {
            if s == t {
                return false;
            }
            return occurs_in(*v, s);
        }

        match s {
            Term::Var(_) => {
                // A variable is only greater than itself (handled by Equal above)
                // or if t is a variable in s. Since t is not a Var here (handled above),
                // a variable s cannot be greater than a non-variable t.
                false
            }
            Term::App(f, s_args) => {
                // Case 2a: some si ≥_lpo t (subterm property)
                for si in s_args {
                    if si == t || self.lpo_gt(si, t) {
                        return true;
                    }
                }

                match t {
                    Term::App(g, t_args) => {
                        // For cases 2b and 2c, we need s >_lpo all tj
                        let s_gt_all_tj = t_args.iter().all(|tj| self.lpo_gt(s, tj));
                        if !s_gt_all_tj {
                            return false;
                        }

                        let prec_f = self.config.symbol_precedence(*f);
                        let prec_g = self.config.symbol_precedence(*g);

                        if prec_f > prec_g {
                            // Case 2b: f ≻ g and s >_lpo all tj
                            true
                        } else if prec_f == prec_g {
                            // Case 2c: same precedence, lexicographic comparison
                            // and s >_lpo all tj (already checked)
                            self.lex_gt(s_args, t_args)
                        } else {
                            false
                        }
                    }
                    Term::Var(_) => {
                        // Already handled above in the t match
                        unreachable!()
                    }
                }
            }
        }
    }

    /// Lexicographic comparison of argument lists.
    /// Returns true if args_s >_lex args_t (first differing position has si > ti).
    /// Also requires that s >_lpo all remaining tj (which the caller ensures via s_gt_all_tj).
    fn lex_gt(&self, args_s: &[Term], args_t: &[Term]) -> bool {
        for (si, ti) in args_s.iter().zip(args_t.iter()) {
            if si == ti {
                continue;
            }
            if self.lpo_gt(si, ti) {
                // Remaining t args must all be less than s
                // (this is already ensured by the caller's s_gt_all_tj check)
                return true;
            }
            return false;
        }
        // All compared args are equal. If s has more args, that's not standard LPO.
        // For same-arity symbols this means the terms are equal up to args — shouldn't happen
        // since we check s == t at the top.
        false
    }
}

impl Default for LPO {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns true if variable `v` occurs in term `t`.
fn occurs_in(v: VarId, t: &Term) -> bool {
    match t {
        Term::Var(w) => v == *w,
        Term::App(_, args) => args.iter().any(|a| occurs_in(v, a)),
    }
}

fn occurs_in_id(v: VarId, t: mrs_core::term_bank::TermId, bank: &mrs_core::term_bank::TermBank) -> bool {
    match bank.get(t) {
        mrs_core::term_bank::TermNode::Var(w) => v == *w,
        mrs_core::term_bank::TermNode::App(_, args) => args.iter().any(|&a| occurs_in_id(v, a, bank)),
    }
}

/// A term ordering: either KBO or LPO.
///
/// Wraps both orderings in an enum so that the search engine can be
/// configured to use either one without trait objects or generics.
#[derive(Clone, Debug, Default)]
pub enum TermOrdering {
    /// Knuth-Bendix Ordering.
    #[default]
    KBO,
    /// Lexicographic Path Ordering.
    LPO,
    /// KBO with custom configuration.
    CustomKBO(Arc<SymbolConfig>),
    /// LPO with custom configuration.
    CustomLPO(Arc<SymbolConfig>),
}

impl TermOrdering {
    /// Compares two terms under the configured ordering.
    pub fn compare(&self, s: &Term, t: &Term) -> TermComparison {
        match self {
            TermOrdering::KBO => KBO::new().compare(s, t),
            TermOrdering::LPO => LPO::new().compare(s, t),
            TermOrdering::CustomKBO(config) => KBO::with_config(config.clone()).compare(s, t),
            TermOrdering::CustomLPO(config) => LPO::with_config(config.clone()).compare(s, t),
        }
    }

    pub fn compare_id(&self, s: mrs_core::term_bank::TermId, t: mrs_core::term_bank::TermId, bank: &mrs_core::term_bank::TermBank) -> TermComparison {
        match self {
            TermOrdering::KBO => KBO::new().compare_id(s, t, bank),
            TermOrdering::LPO => LPO::new().compare_id(s, t, bank),
            TermOrdering::CustomKBO(config) => KBO::with_config(config.clone()).compare_id(s, t, bank),
            TermOrdering::CustomLPO(config) => LPO::with_config(config.clone()).compare_id(s, t, bank),
        }
    }

    /// Returns the symbol configuration used by this ordering.
    pub fn symbol_config(&self) -> Arc<SymbolConfig> {
        match self {
            TermOrdering::KBO | TermOrdering::LPO => Arc::new(SymbolConfig::default()),
            TermOrdering::CustomKBO(config) | TermOrdering::CustomLPO(config) => config.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::SymbolTable;

    #[test]
    fn compare_identical() {
        let mut syms = SymbolTable::new();
        let a = syms.intern("a");
        let kbo = KBO::new();
        assert_eq!(
            kbo.compare(&Term::constant(a), &Term::constant(a)),
            TermComparison::Equal
        );
    }

    #[test]
    fn compare_constants_by_precedence() {
        let mut syms = SymbolTable::new();
        let a = syms.intern("a");
        let b = syms.intern("b");
        let kbo = KBO::new();
        // b has higher SymbolId than a, so b > a
        assert_eq!(
            kbo.compare(&Term::constant(b), &Term::constant(a)),
            TermComparison::Greater
        );
        assert_eq!(
            kbo.compare(&Term::constant(a), &Term::constant(b)),
            TermComparison::Less
        );
    }

    #[test]
    fn compare_by_weight() {
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let a = syms.intern("a");
        let kbo = KBO::new();
        // f(a) has weight 2, a has weight 1 → f(a) > a
        assert_eq!(
            kbo.compare(&Term::app(f, vec![Term::constant(a)]), &Term::constant(a)),
            TermComparison::Greater
        );
    }

    #[test]
    fn compare_variable_incomparable() {
        let mut syms = SymbolTable::new();
        let a = syms.intern("a");
        let kbo = KBO::new();
        // X vs a: same weight (1), but X is a variable → incomparable
        // (variable condition: a has no variables, X does, so a !>= X for vars)
        assert_eq!(
            kbo.compare(&Term::var(0), &Term::constant(a)),
            TermComparison::Incomparable
        );
    }

    #[test]
    fn compare_function_greater_than_var() {
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let kbo = KBO::new();
        // f(X) has weight 2, X has weight 1 → f(X) > X
        // Variable condition: X in RHS occurs in LHS ✓
        assert_eq!(
            kbo.compare(&Term::app(f, vec![Term::var(0)]), &Term::var(0)),
            TermComparison::Greater
        );
    }

    #[test]
    fn compare_different_vars_incomparable() {
        let kbo = KBO::new();
        // X vs Y: same weight, but variable condition fails both ways
        assert_eq!(
            kbo.compare(&Term::var(0), &Term::var(1)),
            TermComparison::Incomparable
        );
    }

    #[test]
    fn compare_lexicographic() {
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let kbo = KBO::new();
        // f(b) vs f(a): same weight, same top symbol → compare args: b > a
        assert_eq!(
            kbo.compare(
                &Term::app(f, vec![Term::constant(b)]),
                &Term::app(f, vec![Term::constant(a)])
            ),
            TermComparison::Greater
        );
    }

    #[test]
    fn compare_var_condition_failure() {
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let kbo = KBO::new();
        // f(X) vs f(Y): same weight, same top symbol, but X!=Y → incomparable
        assert_eq!(
            kbo.compare(
                &Term::app(f, vec![Term::var(0)]),
                &Term::app(f, vec![Term::var(1)])
            ),
            TermComparison::Incomparable
        );
    }

    #[test]
    fn compare_nested_weight() {
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let g = syms.intern("g");
        let a = syms.intern("a");
        let kbo = KBO::new();
        // f(g(a)) has weight 3, g(a) has weight 2 → f(g(a)) > g(a)
        assert_eq!(
            kbo.compare(
                &Term::app(f, vec![Term::app(g, vec![Term::constant(a)])]),
                &Term::app(g, vec![Term::constant(a)])
            ),
            TermComparison::Greater
        );
    }

    // --- LPO tests ---

    #[test]
    fn lpo_identical() {
        let mut syms = SymbolTable::new();
        let a = syms.intern("a");
        let lpo = LPO::new();
        assert_eq!(
            lpo.compare(&Term::constant(a), &Term::constant(a)),
            TermComparison::Equal
        );
    }

    #[test]
    fn lpo_constants_by_precedence() {
        let mut syms = SymbolTable::new();
        let a = syms.intern("a");
        let b = syms.intern("b");
        let lpo = LPO::new();
        // b has higher SymbolId → b > a
        assert_eq!(
            lpo.compare(&Term::constant(b), &Term::constant(a)),
            TermComparison::Greater
        );
        assert_eq!(
            lpo.compare(&Term::constant(a), &Term::constant(b)),
            TermComparison::Less
        );
    }

    #[test]
    fn lpo_function_greater_than_var() {
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let lpo = LPO::new();
        // f(X) >_lpo X because X occurs in f(X)
        assert_eq!(
            lpo.compare(&Term::app(f, vec![Term::var(0)]), &Term::var(0)),
            TermComparison::Greater
        );
    }

    #[test]
    fn lpo_different_vars_incomparable() {
        let lpo = LPO::new();
        // X vs Y: neither occurs in the other → incomparable
        assert_eq!(
            lpo.compare(&Term::var(0), &Term::var(1)),
            TermComparison::Incomparable
        );
    }

    #[test]
    fn lpo_higher_precedence_function() {
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let g = syms.intern("g");
        let a = syms.intern("a");
        let lpo = LPO::new();
        // g has higher SymbolId than f, so g(a) >_lpo f(a)
        assert_eq!(
            lpo.compare(
                &Term::app(g, vec![Term::constant(a)]),
                &Term::app(f, vec![Term::constant(a)])
            ),
            TermComparison::Greater
        );
    }

    #[test]
    fn lpo_lexicographic() {
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let lpo = LPO::new();
        // f(b) vs f(a): same top symbol → compare args: b > a → f(b) > f(a)
        assert_eq!(
            lpo.compare(
                &Term::app(f, vec![Term::constant(b)]),
                &Term::app(f, vec![Term::constant(a)])
            ),
            TermComparison::Greater
        );
    }

    #[test]
    fn lpo_subterm_property() {
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let g = syms.intern("g");
        let a = syms.intern("a");
        let lpo = LPO::new();
        // f(g(a)) >_lpo g(a) because g(a) is a subterm of f(g(a))
        assert_eq!(
            lpo.compare(
                &Term::app(f, vec![Term::app(g, vec![Term::constant(a)])]),
                &Term::app(g, vec![Term::constant(a)])
            ),
            TermComparison::Greater
        );
    }

    #[test]
    fn lpo_var_incomparable_with_constant() {
        let mut syms = SymbolTable::new();
        let a = syms.intern("a");
        let lpo = LPO::new();
        // X vs a: X is not in a, a is not in X → incomparable
        assert_eq!(
            lpo.compare(&Term::var(0), &Term::constant(a)),
            TermComparison::Incomparable
        );
    }

    // --- TermOrdering enum tests ---

    #[test]
    fn term_ordering_kbo() {
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let a = syms.intern("a");
        let ord = TermOrdering::KBO;
        assert_eq!(
            ord.compare(&Term::app(f, vec![Term::constant(a)]), &Term::constant(a)),
            TermComparison::Greater
        );
    }

    #[test]
    fn term_ordering_lpo() {
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let a = syms.intern("a");
        let ord = TermOrdering::LPO;
        assert_eq!(
            ord.compare(&Term::app(f, vec![Term::constant(a)]), &Term::constant(a)),
            TermComparison::Greater
        );
    }
}
