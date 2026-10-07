//! Term orderings for the superposition calculus.
//!
//! Two reduction orderings are supported:
//! - **KBO** (Knuth-Bendix Ordering): weight-based, fast, good for many problems.
//! - **LPO** (Lexicographic Path Ordering): precedence-based, better for some
//!   equational problems where KBO cannot orient key equalities.
//!
//! Both are used to orient equalities and determine which inferences are
//! necessary for completeness.

use crate::{HashMap, HashSet};
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
    ac_symbols: Option<Arc<HashSet<SymbolId>>>,
}

impl KBO {
    /// Creates a KBO with default weights (all symbols and variables have weight 1).
    pub fn new() -> Self {
        Self {
            config: Arc::new(SymbolConfig::default()),
            ac_symbols: None,
        }
    }

    pub fn with_config(config: Arc<SymbolConfig>) -> Self {
        Self {
            config,
            ac_symbols: None,
        }
    }

    pub fn with_ac(config: Arc<SymbolConfig>, ac_symbols: Arc<HashSet<SymbolId>>) -> Self {
        Self {
            config,
            ac_symbols: Some(ac_symbols),
        }
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

    fn weight_id(
        &self,
        t: mrs_core::term_bank::TermId,
        bank: &mrs_core::term_bank::TermBank,
    ) -> u32 {
        match bank.get(t) {
            mrs_core::term_bank::TermNode::Var(_) => self.config.w0,
            mrs_core::term_bank::TermNode::App(f, args) => {
                self.symbol_weight(*f) + args.iter().map(|&a| self.weight_id(a, bank)).sum::<u32>()
            }
        }
    }

    /// Counts occurrences of each variable in a term.
    fn var_counts(t: &Term) -> HashMap<VarId, i32> {
        let mut counts = HashMap::default();
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

    fn var_counts_id(
        t: mrs_core::term_bank::TermId,
        bank: &mrs_core::term_bank::TermBank,
    ) -> HashMap<VarId, i32> {
        let mut counts = HashMap::default();
        Self::collect_var_counts_id(t, bank, &mut counts);
        counts
    }

    fn collect_var_counts_id(
        t: mrs_core::term_bank::TermId,
        bank: &mrs_core::term_bank::TermBank,
        counts: &mut HashMap<VarId, i32>,
    ) {
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

    pub fn compare_id(
        &self,
        s: mrs_core::term_bank::TermId,
        t: mrs_core::term_bank::TermId,
        bank: &mrs_core::term_bank::TermBank,
    ) -> TermComparison {
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
            (
                mrs_core::term_bank::TermNode::App(f1, args1),
                mrs_core::term_bank::TermNode::App(f2, args2),
            ) => {
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

                if let Some(ac) = &self.ac_symbols
                    && ac.contains(f1)
                {
                    let mut s_args_flat = Vec::new();
                    let mut stack = vec![s];
                    while let Some(curr) = stack.pop() {
                        if let mrs_core::term_bank::TermNode::App(g, args) = bank.get(curr)
                            && g == f1
                        {
                            stack.extend(args.iter().copied());
                            continue;
                        }
                        s_args_flat.push(curr);
                    }

                    let mut t_args_flat = Vec::new();
                    let mut stack = vec![t];
                    while let Some(curr) = stack.pop() {
                        if let mrs_core::term_bank::TermNode::App(g, args) = bank.get(curr)
                            && g == f2
                        {
                            stack.extend(args.iter().copied());
                            continue;
                        }
                        t_args_flat.push(curr);
                    }

                    let mut i = 0;
                    while i < s_args_flat.len() {
                        if let Some(j) = t_args_flat.iter().position(|&x| x == s_args_flat[i]) {
                            s_args_flat.swap_remove(i);
                            t_args_flat.swap_remove(j);
                        } else {
                            i += 1;
                        }
                    }

                    if s_args_flat.is_empty() && t_args_flat.is_empty() {
                        return TermComparison::Equal;
                    }
                    if s_args_flat.is_empty() {
                        if t_ge_s_vars {
                            return TermComparison::Less;
                        } else {
                            return TermComparison::Incomparable;
                        }
                    }
                    if t_args_flat.is_empty() {
                        if s_ge_t_vars {
                            return TermComparison::Greater;
                        } else {
                            return TermComparison::Incomparable;
                        }
                    }

                    let s_gt_t = t_args_flat.iter().all(|&tj| {
                        s_args_flat
                            .iter()
                            .any(|&si| self.compare_id(si, tj, bank) == TermComparison::Greater)
                    });
                    let t_gt_s = s_args_flat.iter().all(|&si| {
                        t_args_flat
                            .iter()
                            .any(|&tj| self.compare_id(tj, si, bank) == TermComparison::Greater)
                    });

                    if s_gt_t && !t_gt_s && s_ge_t_vars {
                        return TermComparison::Greater;
                    }
                    if t_gt_s && !s_gt_t && t_ge_s_vars {
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

                if let Some(ac) = &self.ac_symbols
                    && ac.contains(f1)
                {
                    let mut s_args_flat = Vec::new();
                    let mut stack = vec![s];
                    while let Some(curr) = stack.pop() {
                        if let Term::App(g, args) = curr
                            && g == f1
                        {
                            stack.extend(args.iter());
                            continue;
                        }
                        s_args_flat.push(curr);
                    }

                    let mut t_args_flat = Vec::new();
                    let mut stack = vec![t];
                    while let Some(curr) = stack.pop() {
                        if let Term::App(g, args) = curr
                            && g == f2
                        {
                            stack.extend(args.iter());
                            continue;
                        }
                        t_args_flat.push(curr);
                    }

                    let mut i = 0;
                    while i < s_args_flat.len() {
                        if let Some(j) = t_args_flat.iter().position(|&x| x == s_args_flat[i]) {
                            s_args_flat.swap_remove(i);
                            t_args_flat.swap_remove(j);
                        } else {
                            i += 1;
                        }
                    }

                    if s_args_flat.is_empty() && t_args_flat.is_empty() {
                        return TermComparison::Equal;
                    }
                    if s_args_flat.is_empty() {
                        if t_ge_s_vars {
                            return TermComparison::Less;
                        } else {
                            return TermComparison::Incomparable;
                        }
                    }
                    if t_args_flat.is_empty() {
                        if s_ge_t_vars {
                            return TermComparison::Greater;
                        } else {
                            return TermComparison::Incomparable;
                        }
                    }

                    let s_gt_t = t_args_flat.iter().all(|&tj| {
                        s_args_flat
                            .iter()
                            .any(|&si| self.compare(si, tj) == TermComparison::Greater)
                    });
                    let t_gt_s = s_args_flat.iter().all(|&si| {
                        t_args_flat
                            .iter()
                            .any(|&tj| self.compare(tj, si) == TermComparison::Greater)
                    });

                    if s_gt_t && !t_gt_s && s_ge_t_vars {
                        return TermComparison::Greater;
                    }
                    if t_gt_s && !s_gt_t && t_ge_s_vars {
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

/// Memo of `lpo_gt`/`lpo_gt_id` results for one top-level comparison.
///
/// Case 2a re-descends into the arguments of `s` while Case 2b re-compares the
/// whole of `s` against each argument of `t`. Combined, that revisits the same
/// `(s, t)` pairs exponentially often in the term depth, which is reachable in
/// practice: on `GRP024-5` (casc-30 UEQ, strategy 8) the worker thread stays
/// inside `lpo_gt_id` far past its deadline, alternating between those two
/// cases until the harness kills the process.
///
/// Keying on `(s, t)` bounds the recursion by the number of distinct pairs
/// instead of the number of paths through them, which is what makes the
/// comparison polynomial. Every recursive call strictly decreases the sum of
/// the two term sizes, so the recursion is acyclic and no "in progress" marker
/// is needed: an entry is stored only once its result is known.
///
/// The memo is created per top-level `compare`/`compare_id` call rather than
/// kept on `LPO`, for two reasons: `TermOrdering` constructs a fresh `LPO` for
/// every comparison (`LPO::new().compare_id(..)`), so a field on `LPO` would
/// never be reused; and results depend on the `SymbolConfig` and `TermBank` in
/// scope, so a cache outliving one call would need config-identity keying and
/// would risk serving stale answers.
///
/// The `terms` map keys on term identity, which is only meaningful for the
/// duration of a single call: every term reachable during the recursion is
/// borrowed from the two terms the caller passed in, so those pointers stay
/// valid and distinct for as long as this memo lives.
///
/// # Step budget
///
/// The memo bounds work by the number of *distinct pairs*, which is still
/// `|subterms(s)| x |subterms(t)|`. That is finite but not bounded by the
/// deadline, and a single comparison can still exceed it, so `steps` caps the
/// work per call as well. The cap is a backstop against a term shape larger
/// than any seen so far, not the primary bound.
///
/// **Expiry cannot yield a comparison result.** LPO orients superposition
/// inferences, so an arbitrary answer on overflow would silently produce an
/// unsound proof. Instead the call aborts, `expired` is set, and
/// [`LpoBudget::expired`] lets the caller terminate the search. Callers must
/// treat expiry as a reason to stop, never as an answer — see
/// `mrs_search::lpo_budget` for the search-side wiring.
#[derive(Default)]
struct LpoMemo {
    ids: HashMap<(mrs_core::term_bank::TermId, mrs_core::term_bank::TermId), bool>,
    terms: HashMap<(usize, usize), bool>,
    steps: u64,
}

impl LpoMemo {
    /// Charges one node against this call's step budget.
    ///
    /// The budget is per top-level comparison, so a large term is capped no
    /// matter how many comparisons the search performs overall; the caller's
    /// wall-clock deadline remains the outer bound. Returns `None` when the
    /// budget is spent, which callers must propagate rather than read as a
    /// comparison result.
    fn charge(&mut self, limit: u64) -> Option<()> {
        if self.steps >= limit {
            return None;
        }
        self.steps += 1;
        Some(())
    }
}

/// Per-thread budget for LPO comparisons, armed by the search.
///
/// LPO cannot answer "I give up" with a comparison, so the budget only decides
/// when to *stop*: the search that armed it checks [`expired`] and returns
/// `Timeout`. Being thread-local matches the search's own structure — one
/// worker thread per strategy, each with its own deadline — and keeps the
/// hot-path comparison free of an extra parameter.
///
/// Unarmed, the budget is unlimited, so library callers outside a search
/// (`mrs-book-labs`, `certified.rs`'s own fixtures) are unaffected.
mod budget {
    use std::cell::Cell;

    thread_local! {
        static LIMIT: Cell<u64> = const { Cell::new(u64::MAX) };
        static EXPIRED: Cell<bool> = const { Cell::new(false) };
    }

    /// Arms the budget for the current thread. Returns the guard that restores
    /// the previous state on drop, so nested searches cannot inherit a spent
    /// budget.
    pub struct BudgetGuard {
        previous_limit: u64,
        previous_expired: bool,
    }

    impl BudgetGuard {
        pub fn arm(limit: u64) -> Self {
            let previous_limit = LIMIT.with(|l| l.replace(limit));
            let previous_expired = EXPIRED.with(|e| e.replace(false));
            Self {
                previous_limit,
                previous_expired,
            }
        }
    }

    impl Drop for BudgetGuard {
        fn drop(&mut self) {
            LIMIT.with(|l| l.set(self.previous_limit));
            EXPIRED.with(|e| e.set(self.previous_expired));
        }
    }

    /// The armed limit, or `u64::MAX` when no budget is active.
    pub fn limit() -> u64 {
        LIMIT.with(Cell::get)
    }

    /// Records that the budget ran out on this thread, and yields the value a
    /// comparison must return in that case.
    ///
    /// The value is `Incomparable` only so the signature stays total. It is
    /// **not** a comparison answer: callers that consume it must have already
    /// arranged to stop, via [`expired`].
    pub fn mark_expired() -> super::TermComparison {
        EXPIRED.with(|e| e.set(true));
        super::TermComparison::Incomparable
    }

    /// Whether an armed budget has run out since it was armed.
    ///
    /// The search must treat this as "stop and report a timeout": an aborted
    /// comparison set means completeness was never established, so it must not
    /// be reported as saturation.
    pub fn expired() -> bool {
        EXPIRED.with(Cell::get)
    }
}

pub use budget::BudgetGuard as LpoBudgetGuard;

/// Whether the current thread's armed LPO budget has run out.
///
/// The search checks this where it would otherwise report a status. Expiry must
/// force `Timeout`: an aborted comparison set means completeness was never
/// established, so reporting saturation would claim a completeness the run never
/// had.
pub fn lpo_budget_expired() -> bool {
    budget::expired()
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

    pub fn compare_id(
        &self,
        s: mrs_core::term_bank::TermId,
        t: mrs_core::term_bank::TermId,
        bank: &mrs_core::term_bank::TermBank,
    ) -> TermComparison {
        if s == t {
            return TermComparison::Equal;
        }
        let mut memo = LpoMemo::default();
        match self.lpo_gt_id(s, t, bank, &mut memo) {
            Some(true) => TermComparison::Greater,
            Some(false) => match self.lpo_gt_id(t, s, bank, &mut memo) {
                Some(true) => TermComparison::Less,
                Some(false) => TermComparison::Incomparable,
                None => budget::mark_expired(),
            },
            None => budget::mark_expired(),
        }
    }

    /// Returns `Some(s >_lpo t)`, memoizing on `(s, t)`.
    ///
    /// `None` means this call's step budget ran out. That is not a comparison
    /// result: the caller must stop, because an orientation decided from a
    /// partially explored recursion would be arbitrary and could orient a
    /// superposition inference wrongly.
    fn lpo_gt_id(
        &self,
        s: mrs_core::term_bank::TermId,
        t: mrs_core::term_bank::TermId,
        bank: &mrs_core::term_bank::TermBank,
        memo: &mut LpoMemo,
    ) -> Option<bool> {
        if let Some(&cached) = memo.ids.get(&(s, t)) {
            return Some(cached);
        }
        memo.charge(budget::limit())?;
        let result = self.lpo_gt_id_uncached(s, t, bank, memo)?;
        memo.ids.insert((s, t), result);
        Some(result)
    }

    fn lpo_gt_id_uncached(
        &self,
        s: mrs_core::term_bank::TermId,
        t: mrs_core::term_bank::TermId,
        bank: &mrs_core::term_bank::TermBank,
        memo: &mut LpoMemo,
    ) -> Option<bool> {
        // Case 1: t is a variable occurring in s (and s ≠ t)
        if let mrs_core::term_bank::TermNode::Var(v) = bank.get(t) {
            if s == t {
                return Some(false);
            }
            return Some(occurs_in_id(*v, s, bank));
        }

        match bank.get(s) {
            mrs_core::term_bank::TermNode::Var(_) => Some(false),
            mrs_core::term_bank::TermNode::App(f, s_args) => {
                // Case 2a: some si ≥_lpo t (subterm property)
                for &si in s_args {
                    if si == t {
                        return Some(true);
                    }
                    match self.lpo_gt_id(si, t, bank, memo) {
                        Some(true) => return Some(true),
                        // Propagate exhaustion rather than reading it as false.
                        None => return None,
                        Some(false) => {}
                    }
                }

                match bank.get(t) {
                    mrs_core::term_bank::TermNode::App(g, t_args) => {
                        for &tj in t_args {
                            match self.lpo_gt_id(s, tj, bank, memo) {
                                Some(true) => {}
                                None => return None,
                                Some(false) => return Some(false),
                            }
                        }

                        let prec_f = self.config.symbol_precedence(*f);
                        let prec_g = self.config.symbol_precedence(*g);

                        if prec_f > prec_g {
                            Some(true)
                        } else if prec_f == prec_g {
                            self.lex_gt_id(s_args, t_args, bank, memo)
                        } else {
                            Some(false)
                        }
                    }
                    mrs_core::term_bank::TermNode::Var(_) => {
                        unreachable!()
                    }
                }
            }
        }
    }

    fn lex_gt_id(
        &self,
        args_s: &[mrs_core::term_bank::TermId],
        args_t: &[mrs_core::term_bank::TermId],
        bank: &mrs_core::term_bank::TermBank,
        memo: &mut LpoMemo,
    ) -> Option<bool> {
        for (&si, &ti) in args_s.iter().zip(args_t.iter()) {
            if si == ti {
                continue;
            }
            return self.lpo_gt_id(si, ti, bank, memo);
        }
        Some(args_s.len() > args_t.len())
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
        let mut memo = LpoMemo::default();
        match self.lpo_gt(s, t, &mut memo) {
            Some(true) => TermComparison::Greater,
            Some(false) => match self.lpo_gt(t, s, &mut memo) {
                Some(true) => TermComparison::Less,
                Some(false) => TermComparison::Incomparable,
                None => budget::mark_expired(),
            },
            None => budget::mark_expired(),
        }
    }

    /// Returns `Some(s >_lpo t)`, memoizing on term identity.
    ///
    /// `None` means the step budget ran out, which the caller must not read as
    /// a comparison result.
    fn lpo_gt(&self, s: &Term, t: &Term, memo: &mut LpoMemo) -> Option<bool> {
        let key = (s as *const Term as usize, t as *const Term as usize);
        if let Some(&cached) = memo.terms.get(&key) {
            return Some(cached);
        }
        memo.charge(budget::limit())?;
        let result = self.lpo_gt_uncached(s, t, memo)?;
        memo.terms.insert(key, result);
        Some(result)
    }

    fn lpo_gt_uncached(&self, s: &Term, t: &Term, memo: &mut LpoMemo) -> Option<bool> {
        // Case 1: t is a variable occurring in s (and s ≠ t)
        if let Term::Var(v) = t {
            if s == t {
                return Some(false);
            }
            return Some(occurs_in(*v, s));
        }

        match s {
            Term::Var(_) => {
                // A variable is only greater than itself (handled by Equal above)
                // or if t is a variable in s. Since t is not a Var here (handled above),
                // a variable s cannot be greater than a non-variable t.
                Some(false)
            }
            Term::App(f, s_args) => {
                // Case 2a: some si ≥_lpo t (subterm property)
                for si in s_args {
                    if si == t {
                        return Some(true);
                    }
                    match self.lpo_gt(si, t, memo) {
                        Some(true) => return Some(true),
                        None => return None,
                        Some(false) => {}
                    }
                }

                match t {
                    Term::App(g, t_args) => {
                        // For cases 2b and 2c, we need s >_lpo all tj
                        for tj in t_args {
                            match self.lpo_gt(s, tj, memo) {
                                Some(true) => {}
                                None => return None,
                                Some(false) => return Some(false),
                            }
                        }

                        let prec_f = self.config.symbol_precedence(*f);
                        let prec_g = self.config.symbol_precedence(*g);

                        if prec_f > prec_g {
                            // Case 2b: f ≻ g and s >_lpo all tj
                            Some(true)
                        } else if prec_f == prec_g {
                            // Case 2c: same precedence, lexicographic comparison
                            // and s >_lpo all tj (already checked)
                            self.lex_gt(s_args, t_args, memo)
                        } else {
                            Some(false)
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
    fn lex_gt(&self, args_s: &[Term], args_t: &[Term], memo: &mut LpoMemo) -> Option<bool> {
        for (si, ti) in args_s.iter().zip(args_t.iter()) {
            if si == ti {
                continue;
            }
            // Remaining t args must all be less than s
            // (this is already ensured by the caller's s_gt_all_tj check)
            return self.lpo_gt(si, ti, memo);
        }
        // All compared args are equal. If s has more args, that's not standard LPO.
        // For same-arity symbols this means the terms are equal up to args — shouldn't happen
        // since we check s == t at the top.
        Some(false)
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

fn occurs_in_id(
    v: VarId,
    t: mrs_core::term_bank::TermId,
    bank: &mrs_core::term_bank::TermBank,
) -> bool {
    match bank.get(t) {
        mrs_core::term_bank::TermNode::Var(w) => v == *w,
        mrs_core::term_bank::TermNode::App(_, args) => {
            args.iter().any(|&a| occurs_in_id(v, a, bank))
        }
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
    /// KBO with custom configuration and AC symbols.
    CustomACKBO(Arc<SymbolConfig>, Arc<HashSet<SymbolId>>),
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
            TermOrdering::CustomACKBO(config, ac) => {
                KBO::with_ac(config.clone(), ac.clone()).compare(s, t)
            }
            TermOrdering::CustomLPO(config) => LPO::with_config(config.clone()).compare(s, t),
        }
    }

    pub fn compare_id(
        &self,
        s: mrs_core::term_bank::TermId,
        t: mrs_core::term_bank::TermId,
        bank: &mrs_core::term_bank::TermBank,
    ) -> TermComparison {
        match self {
            TermOrdering::KBO => KBO::new().compare_id(s, t, bank),
            TermOrdering::LPO => LPO::new().compare_id(s, t, bank),
            TermOrdering::CustomKBO(config) => {
                KBO::with_config(config.clone()).compare_id(s, t, bank)
            }
            TermOrdering::CustomACKBO(config, ac) => {
                KBO::with_ac(config.clone(), ac.clone()).compare_id(s, t, bank)
            }
            TermOrdering::CustomLPO(config) => {
                LPO::with_config(config.clone()).compare_id(s, t, bank)
            }
        }
    }

    /// Returns the symbol configuration used by this ordering.
    pub fn symbol_config(&self) -> Arc<SymbolConfig> {
        match self {
            TermOrdering::KBO | TermOrdering::LPO => Arc::new(SymbolConfig::default()),
            TermOrdering::CustomKBO(config)
            | TermOrdering::CustomACKBO(config, _)
            | TermOrdering::CustomLPO(config) => config.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::SymbolTable;
    use mrs_core::subst::Substitution;
    use std::sync::Arc;

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
    fn lpo_id_shared_deep_terms_compare_without_revisiting_pairs() {
        use mrs_core::term_bank::TermBank;

        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let mut bank = TermBank::new();
        let mut left = bank.intern_app(a, Vec::new());
        let mut right = bank.intern_app(b, Vec::new());
        // Both arms are interned, so the recursive LPO comparison repeatedly
        // encounters the same pair in its subterm and lexicographic cases.
        for _ in 0..80 {
            left = bank.intern_app(f, vec![left, left]);
            right = bank.intern_app(f, vec![right, right]);
        }
        let lpo = LPO::new();
        assert_eq!(lpo.compare_id(left, right, &bank), TermComparison::Less);
        assert_eq!(lpo.compare_id(right, left, &bank), TermComparison::Greater);
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

    #[test]
    fn kbo_is_strict_transitive_and_substitution_stable_on_bounded_terms() {
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let g = syms.intern("g");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let config = Arc::new(SymbolConfig {
            precedence: vec![4, 3, 2, 1],
            weights: vec![2, 2, 1, 1],
            w0: 1,
        });
        let kbo = KBO::with_config(config);
        let terms = vec![
            Term::var(0),
            Term::var(1),
            Term::constant(a),
            Term::constant(b),
            Term::app(f, vec![Term::var(0)]),
            Term::app(f, vec![Term::constant(a)]),
            Term::app(g, vec![Term::constant(b)]),
            Term::app(f, vec![Term::app(g, vec![Term::constant(a)])]),
        ];

        for term in &terms {
            assert_ne!(kbo.compare(term, term), TermComparison::Greater);
            assert_ne!(kbo.compare(term, term), TermComparison::Less);
        }
        for left in &terms {
            for middle in &terms {
                for right in &terms {
                    if kbo.compare(left, middle) == TermComparison::Greater
                        && kbo.compare(middle, right) == TermComparison::Greater
                    {
                        assert_eq!(kbo.compare(left, right), TermComparison::Greater);
                    }
                }
            }
        }

        let substitutions = [
            Substitution::singleton(0, Term::constant(a)),
            Substitution::singleton(0, Term::app(g, vec![Term::constant(b)])),
            {
                let mut substitution = Substitution::new();
                substitution.bind(0, Term::constant(a));
                substitution.bind(1, Term::app(f, vec![Term::constant(b)]));
                substitution
            },
        ];
        for left in &terms {
            for right in &terms {
                if kbo.compare(left, right) != TermComparison::Greater {
                    continue;
                }
                for substitution in &substitutions {
                    let left_instance = substitution.apply_term(left);
                    let right_instance = substitution.apply_term(right);
                    assert_eq!(
                        kbo.compare(&left_instance, &right_instance),
                        TermComparison::Greater,
                        "KBO lost strictness under substitution: {left:?} > {right:?}"
                    );
                }
            }
        }
    }

    /// Builds two `f`-nested terms with distinct leaves, so comparison cannot
    /// return early on structural equality and exercises LPO Case 2a/2b.
    fn nested_pair(syms: &mut SymbolTable, depth: usize) -> (Term, Term) {
        let f = syms.intern("f");
        let g = syms.intern("g");
        let c = syms.intern("c");
        let d = syms.intern("d");
        let mut left = Term::constant(c);
        let mut right = Term::constant(d);
        for _ in 0..depth {
            left = Term::app(f, vec![left, Term::constant(g)]);
            right = Term::app(f, vec![right, Term::constant(g)]);
        }
        (left, right)
    }

    /// Deeply nested terms must not send `lpo_gt` into superlinear work.
    ///
    /// Without memoization the Case 2a/Case 2b interaction in `lpo_gt`
    /// revisits the same `(s, t)` pairs exponentially often in the nesting
    /// depth. On `GRP024-5` (casc-30 UEQ, strategy 8) this wedged a worker
    /// thread past its deadline. The bound below is deliberately generous for
    /// the memoized cost and impossible for the unmemoized one, so the test
    /// fails by hanging rather than by a wrong answer.
    #[test]
    fn lpo_deeply_nested_comparison_terminates() {
        let mut syms = SymbolTable::new();
        let (left, right) = nested_pair(&mut syms, 24);
        let lpo = LPO::new();
        let verdict = lpo.compare(&left, &right);
        // d was interned after c, so the different leaves determine the order.
        assert_eq!(verdict, TermComparison::Less);
        assert_eq!(lpo.compare(&right, &left), TermComparison::Greater);
    }

    /// Builds the pair that makes `lpo_gt` work hardest per memo entry.
    ///
    /// `s` is wide and duplicates its own subterm, `t` is narrow, and the two use
    /// disjoint symbol sets so `si == t` in Case 2a cannot short-circuit. Case
    /// 2a therefore descends the whole of `s` at every node while Case 2b
    /// re-compares all of `s` against each argument of `t`. This is the shape
    /// from the `GRP024-5` investigation.
    fn wide_against_narrow(syms: &mut SymbolTable, depth: usize) -> (Term, Term) {
        let h = syms.intern("h");
        let i = syms.intern("i");
        let j = syms.intern("j");
        let k = syms.intern("k");
        let mut left = Term::constant(h);
        let mut right = Term::constant(j);
        for _ in 0..depth {
            left = Term::app(i, vec![left.clone(), left, Term::constant(h)]);
            right = Term::app(k, vec![right]);
        }
        (left, right)
    }

    /// An exhausted step budget must be reported, never answered.
    ///
    /// LPO orients superposition inferences. Returning a plausible comparison
    /// to stay inside a budget would orient inferences from a partially explored
    /// recursion, which is a soundness failure rather than a lost solve. So
    /// expiry returns `Incomparable` *and* sets the thread's expired flag; the
    /// search checks that flag and returns `Timeout`.
    #[test]
    fn lpo_budget_expiry_sets_expired_and_yields_no_answer() {
        let mut syms = SymbolTable::new();
        let lpo = LPO::new();

        // Unarmed: no limit, so the comparison completes on the memo alone.
        // Depth is modest here because this path is unbounded by design, and a
        // debug build runs the memoized work slowly.
        let (shallow_left, shallow_right) = wide_against_narrow(&mut syms, 10);
        assert!(!lpo_budget_expired(), "budget expired with nothing armed");
        let verdict = lpo.compare(&shallow_left, &shallow_right);
        assert_eq!(verdict, TermComparison::Less);
        assert!(
            !lpo_budget_expired(),
            "unarmed comparison must not expire a budget"
        );

        // Armed with a budget too small to finish: the call must give up and
        // mark the thread, rather than return a decision. Depth 20 would need
        // ~69M steps unbounded, so it can only complete if the budget bites.
        let (left, right) = wide_against_narrow(&mut syms, 20);
        {
            let _guard = LpoBudgetGuard::arm(64);
            let started = std::time::Instant::now();
            let verdict = lpo.compare(&left, &right);
            assert!(
                lpo_budget_expired(),
                "an exhausted budget must be observable by the caller"
            );
            // `Incomparable` here is a placeholder so the signature stays total;
            // what matters is that the caller can tell it must not trust it.
            assert_eq!(
                verdict,
                TermComparison::Incomparable,
                "expiry must not yield a directional answer"
            );
            assert!(
                started.elapsed() < std::time::Duration::from_secs(10),
                "expiry took {:?}; the budget is not bounding the work",
                started.elapsed()
            );
        }
    }

    /// The budget must be restored when its guard drops.
    ///
    /// A spent budget leaking into the next comparison on the same thread would
    /// abort every subsequent search on it.
    #[test]
    fn lpo_budget_guard_restores_previous_state() {
        let mut syms = SymbolTable::new();
        let lpo = LPO::new();
        let (deep_left, deep_right) = wide_against_narrow(&mut syms, 20);

        {
            let _guard = LpoBudgetGuard::arm(64);
            let _ = lpo.compare(&deep_left, &deep_right);
            assert!(lpo_budget_expired());
        }

        assert!(
            !lpo_budget_expired(),
            "expired flag survived the guard; later searches would abort immediately"
        );
        // Shallow depth, because with the budget restored this path is unbounded
        // again and must be kept cheap enough for a debug build.
        let (left, right) = wide_against_narrow(&mut syms, 8);
        assert_eq!(
            lpo.compare(&left, &right),
            TermComparison::Less,
            "comparison after the guard dropped should succeed"
        );
    }

    /// A budget large enough for the work must not change any verdict.
    ///
    /// This pins the interaction between the memo and the budget: with room to
    /// spare, the step counter must be invisible in the result.
    #[test]
    fn lpo_generous_budget_preserves_verdicts() {
        let mut syms = SymbolTable::new();
        let lpo = LPO::new();
        // Depth 16 is the widest shape measured in the GRP024-5 investigation
        // that still completes in milliseconds once memoized (~3.5M steps), so
        // the budget here has to exceed that to be "generous" in any useful
        // sense.
        for depth in [1usize, 4, 8, 12, 16] {
            let (left, right) = wide_against_narrow(&mut syms, depth);
            let expected = lpo.compare(&left, &right);
            let _guard = LpoBudgetGuard::arm(8_000_000);
            assert_eq!(
                lpo.compare(&left, &right),
                expected,
                "depth {depth}: armed budget changed the verdict"
            );
            assert!(
                !lpo_budget_expired(),
                "depth {depth}: generous budget expired"
            );
        }
    }

    /// The memo must not change any LPO verdict.
    ///
    /// Each pair is compared twice through the memoized path, and the
    /// antisymmetry of a strict reduction ordering is checked across a set of
    /// terms large enough that Case 2a and Case 2b both fire repeatedly.
    #[test]
    fn lpo_memo_preserves_verdicts() {
        let mut syms = SymbolTable::new();
        let mut terms = Vec::new();
        for depth in 1..=6usize {
            let (left, right) = nested_pair(&mut syms, depth);
            terms.push(left);
            terms.push(right);
            let f = syms.intern("f");
            terms.push(Term::app(f, vec![Term::constant(syms.intern("c"))]));
            terms.push(Term::constant(syms.intern("c")));
        }
        let lpo = LPO::new();
        for left in &terms {
            for right in &terms {
                let first = lpo.compare(left, right);
                let second = lpo.compare(left, right);
                assert_eq!(first, second, "memoized comparison is not stable");
                // A reduction ordering is antisymmetric on distinct terms.
                let reversed = lpo.compare(right, left);
                assert_eq!(
                    reversed,
                    match first {
                        TermComparison::Greater => TermComparison::Less,
                        TermComparison::Less => TermComparison::Greater,
                        other => other,
                    },
                    "comparison is not antisymmetric: {left:?} vs {right:?}"
                );
            }
        }
    }
}
