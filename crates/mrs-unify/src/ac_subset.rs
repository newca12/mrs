//! AC subset matching: matching a pattern into a *part* of a target's AC arguments.
//!
//! # Why this exists
//!
//! Superposition rewrites a subterm `u` of the target clause with the equation's
//! right-hand side. Under an AC symbol `f`, `u` is a multiset
//! `A = flatten(u)`, and the ordinary rule consumes *all* of it: `σ(s) = A`.
//! That is the only shape [`crate::robinson::unify_ac_rigid_id`] can express, and
//! it is why rigid superposition cannot reach inferences such as rewriting two
//! of the five arguments of `f(a, b, c, d, e)` — see UI-1 in
//! `docs/policies/unresolved-issues.md`.
//!
//! This module adds the missing shape. A match is a **non-empty** sub-multiset
//! `S ⊆ A` together with a substitution `σ` with `σ(s) = S`; the caller then
//! rebuilds `u' = f((A \ S) ∪ {σ(t)})`. Non-emptiness is what makes the step a
//! rewrite rather than a fabrication: with `S = ∅` the step would add an
//! argument for free, so `f(a) = f(a, b)` would be derivable from `b = c`.
//!
//! # Soundness
//!
//! `u = f(A) = f(S ∪ R)` and `σ(s) = S`, and `s ≈ t` gives `σ(s) ≈ σ(t)`, so
//! `S ≈ σ(t)` and therefore `u ≈ f((A \ S) ∪ {σ(t)})`. Replacing a subterm of the
//! target by an equal one, then instantiating by `σ`, yields a logical
//! consequence of the two cited parents. The ordering condition `σ(s) ≻ σ(t)`
//! is the caller's to check; nothing here assumes it.
//!
//! # The rigid condition
//!
//! `σ` may only bind the *pattern's* variables — for superposition those are the
//! equation clause's. A target variable is never bound, at any depth: the
//! `[_, TermNode::Var(_)]` arm of [`match_element`] fails rather than binding,
//! and a pattern variable that is itself rigid fails rather than binding. No
//! target variable can therefore enter the range of `σ`, and the rewritten term
//! is exactly the target's terms with `σ`'s effect confined to the pattern.
//!
//! # Budget
//!
//! Matching a `k`-element pattern into an `m`-element target has `m!/(m-k)!`
//! candidate pairings, so the search is explicitly bounded by
//! [`AcSubsetBudget`]. The budget is a plain counter decremented at every
//! candidate pairing, every element comparison and every permutation trial, in
//! a fixed order, so the search is deterministic. Running out is
//! [`UnifyError::BudgetExhausted`] — a non-success result, never a partial
//! match.

use std::collections::HashSet;
use std::hash::BuildHasher;

use mrs_core::SymbolId;
use mrs_core::term::VarId;
use mrs_core::term_bank::{IdSubstitution, TermBank, TermId, TermNode};

use crate::UnifyError;

/// Deterministic ceiling on the pairing search of one AC subset match.
///
/// The counter is decremented at every candidate pairing, element comparison and
/// permutation trial, in a fixed order, so a given `(pattern, target)` always
/// either finds the same first match or fails the same way.
#[derive(Clone, Debug)]
pub struct AcSubsetBudget {
    remaining: u32,
}

impl AcSubsetBudget {
    pub const fn new(limit: u32) -> Self {
        Self { remaining: limit }
    }

    /// `false` once the budget is gone. Exhaustion is never a success.
    fn consume(&mut self) -> bool {
        if self.remaining == 0 {
            return false;
        }
        self.remaining -= 1;
        true
    }
}

/// A successful AC subset match.
#[derive(Clone, Debug)]
pub struct AcSubsetMatch {
    /// Binds pattern (equation-clause) variables only.
    pub subst: IdSubstitution,
    /// Indices into the flattened argument list of the target subterm that the
    /// match consumed, in ascending order. Never empty.
    pub consumed: Vec<usize>,
}

impl AcSubsetMatch {
    /// The flattened arguments of the target subterm this matched into.
    ///
    /// The caller rebuilds the rewritten term from these, so the arguments the
    /// match did *not* consume survive into the conclusion.
    pub fn consumed_len(&self) -> usize {
        self.consumed.len()
    }
}

/// Matches `pattern` into a non-empty sub-multiset of `target`'s AC arguments.
///
/// `target` must be an application of a symbol that is both commutative and
/// associative and have at least two flattened arguments; anything else is
/// [`UnifyError::UnsupportedShape`] and is a non-success. `pattern` must be an
/// application of the same AC symbol with at least two flattened arguments. A
/// unary `f(X)` is not a matching representation of the operand `X`:
/// associativity cannot remove the outer `f`, so accepting it as a one-operand
/// subset would turn `f(X) = t` into the unsound rewrite `X → t`.
///
/// The first match found in the fixed enumeration order is returned; there is no
/// backtracking across *different* matches, because superposition takes the one
/// the ordering check admits.
pub fn match_ac_subset_rigid_id<S: BuildHasher>(
    pattern: TermId,
    target: TermId,
    bank: &TermBank,
    comm: &HashSet<SymbolId, S>,
    assoc: &HashSet<SymbolId, S>,
    rigid: Option<&HashSet<VarId, S>>,
    budget: &mut AcSubsetBudget,
) -> Result<AcSubsetMatch, UnifyError> {
    let TermNode::App(symbol, _) = bank.get(target) else {
        return Err(UnifyError::UnsupportedShape);
    };
    if !comm.contains(symbol) || !assoc.contains(symbol) {
        return Err(UnifyError::UnsupportedShape);
    }

    let mut subst = IdSubstitution::new();
    let target_args = flatten_ac(target, *symbol, &subst, bank);
    // A single argument carries no subset to choose from: the only non-empty
    // subset is the whole list, which is the ordinary whole-subterm match that
    // the caller has already tried.
    if target_args.len() < 2 {
        return Err(UnifyError::UnsupportedShape);
    }

    let TermNode::App(pattern_symbol, _) = bank.get(pattern) else {
        return Err(UnifyError::UnsupportedShape);
    };
    if pattern_symbol != symbol {
        return Err(UnifyError::UnsupportedShape);
    }
    let pattern_args = flatten_ac(pattern, *symbol, &subst, bank);
    if pattern_args.len() < 2 || pattern_args.len() > target_args.len() {
        if pattern_args.len() > target_args.len() {
            return Err(UnifyError::ArityMismatch {
                expected: pattern_args.len(),
                found: target_args.len(),
            });
        }
        return Err(UnifyError::UnsupportedShape);
    }

    let mut used = vec![false; target_args.len()];
    let mut chosen: Vec<usize> = Vec::with_capacity(pattern_args.len());
    let mut ctx = MatchCtx {
        bank,
        comm,
        assoc,
        rigid,
        budget,
    };
    enumerate(
        &mut ctx,
        &pattern_args,
        &target_args,
        0,
        &mut used,
        &mut chosen,
        &mut subst,
    )?;
    if chosen.is_empty() {
        // Unreachable: `enumerate` only returns Ok at depth `pattern_args.len()`,
        // and an empty pattern was rejected above. Fail closed regardless.
        return Err(UnifyError::UnsupportedShape);
    }
    chosen.sort_unstable();
    Ok(AcSubsetMatch {
        subst,
        consumed: chosen,
    })
}

struct MatchCtx<'a, S: BuildHasher> {
    bank: &'a TermBank,
    comm: &'a HashSet<SymbolId, S>,
    assoc: &'a HashSet<SymbolId, S>,
    rigid: Option<&'a HashSet<VarId, S>>,
    budget: &'a mut AcSubsetBudget,
}

/// Enumerates injective pairings of `pattern[index..]` into unused `target`
/// arguments, in increasing index order at every depth.
fn enumerate<S: BuildHasher>(
    ctx: &mut MatchCtx<'_, S>,
    pattern: &[TermId],
    target: &[TermId],
    index: usize,
    used: &mut [bool],
    chosen: &mut Vec<usize>,
    subst: &mut IdSubstitution,
) -> Result<(), UnifyError> {
    if index == pattern.len() {
        return Ok(());
    }
    for candidate in 0..target.len() {
        if used[candidate] {
            continue;
        }
        if !ctx.budget.consume() {
            return Err(UnifyError::BudgetExhausted);
        }
        let saved = subst.clone();
        used[candidate] = true;
        chosen.push(candidate);

        if let Err(err) = match_element(ctx, pattern[index], target[candidate], subst) {
            chosen.pop();
            used[candidate] = false;
            *subst = saved;
            // A rejected pairing is a reason to try the next candidate here;
            // only budget exhaustion and an unsupported shape abort the search.
            if fatal(&err) {
                return Err(err);
            }
            continue;
        }

        match enumerate(ctx, pattern, target, index + 1, used, chosen, subst) {
            Ok(()) => return Ok(()),
            Err(err) => {
                chosen.pop();
                used[candidate] = false;
                *subst = saved;
                if fatal(&err) {
                    return Err(err);
                }
            }
        }
    }
    Err(UnifyError::NoAcSubsetMatch)
}

/// Failures that mean "stop searching", as opposed to "this candidate does not
/// work". Budget exhaustion and an unsupported shape both have to propagate, or
/// the caller would read a truncated search as a refutation of the match.
fn fatal(err: &UnifyError) -> bool {
    matches!(
        err,
        UnifyError::BudgetExhausted | UnifyError::UnsupportedShape
    )
}

/// Matches one pattern element against one target element, rigidly.
fn match_element<S: BuildHasher>(
    ctx: &mut MatchCtx<'_, S>,
    pattern: TermId,
    target: TermId,
    subst: &mut IdSubstitution,
) -> Result<(), UnifyError> {
    if !ctx.budget.consume() {
        return Err(UnifyError::BudgetExhausted);
    }
    let pattern = deref_id(pattern, subst, ctx.bank);
    let target = deref_id(target, subst, ctx.bank);
    if pattern == target {
        return Ok(());
    }

    match (ctx.bank.get(pattern), ctx.bank.get(target)) {
        (TermNode::Var(var), _) => {
            let var = *var;
            if ctx.rigid.is_some_and(|rigid| rigid.contains(&var)) {
                return Err(UnifyError::RigidVariable { var });
            }
            if contains_var_id(target, var, subst, ctx.bank) {
                return Err(UnifyError::OccursCheck { var });
            }
            subst.bind(var, target);
            Ok(())
        }
        // A variable on the target side is either already bound (and would have
        // been dereferenced above) or rigid. Either way it is never bound here.
        (_, TermNode::Var(var)) => Err(UnifyError::RigidVariable { var: *var }),
        (TermNode::App(left_symbol, left_args), TermNode::App(right_symbol, right_args)) => {
            if left_symbol != right_symbol {
                return Err(UnifyError::SymbolClash {
                    left: format!("{left_symbol:?}"),
                    right: format!("{right_symbol:?}"),
                });
            }

            let saved = subst.clone();
            if ctx.assoc.contains(left_symbol) {
                let left_flat = flatten_ac(pattern, *left_symbol, subst, ctx.bank);
                let right_flat = flatten_ac(target, *left_symbol, subst, ctx.bank);
                if left_flat.len() == right_flat.len() && ctx.comm.contains(left_symbol) {
                    let mut used = vec![false; right_flat.len()];
                    let outcome =
                        match_flat_permutations(ctx, &left_flat, &right_flat, 0, &mut used, subst);
                    if outcome.is_err() {
                        *subst = saved;
                    }
                    return outcome;
                }
                if left_flat.len() != right_flat.len() {
                    // Associativity alone cannot align different numbers of
                    // arguments: the only sound alignment is positional, and it
                    // exists exactly when the counts agree.
                    return Err(UnifyError::ArityMismatch {
                        expected: left_flat.len(),
                        found: right_flat.len(),
                    });
                }
                for (left_arg, right_arg) in left_flat.iter().zip(right_flat.iter()) {
                    match_element(ctx, *left_arg, *right_arg, subst)?;
                }
                return Ok(());
            }

            if left_args.len() != right_args.len() {
                return Err(UnifyError::ArityMismatch {
                    expected: left_args.len(),
                    found: right_args.len(),
                });
            }
            let positional: Result<(), UnifyError> = (|| {
                for (left_arg, right_arg) in left_args.iter().zip(right_args.iter()) {
                    match_element(ctx, *left_arg, *right_arg, subst)?;
                }
                Ok(())
            })();
            if positional.is_ok() {
                return Ok(());
            }
            if ctx.comm.contains(left_symbol) && left_args.len() == 2 {
                let mut swapped = saved.clone();
                let swap_ok = (|| {
                    match_element(ctx, left_args[0], right_args[1], &mut swapped)?;
                    match_element(ctx, left_args[1], right_args[0], &mut swapped)
                })();
                if swap_ok.is_ok() {
                    *subst = swapped;
                    return Ok(());
                }
            }
            *subst = saved;
            Err(UnifyError::SymbolClash {
                left: "element mismatch".into(),
                right: "element mismatch".into(),
            })
        }
    }
}

/// Matches two equal-length flattened AC argument lists under a commutative
/// symbol, trying every permutation under the caller's budget.
fn match_flat_permutations<S: BuildHasher>(
    ctx: &mut MatchCtx<'_, S>,
    left: &[TermId],
    right: &[TermId],
    index: usize,
    used: &mut [bool],
    subst: &mut IdSubstitution,
) -> Result<(), UnifyError> {
    if index == left.len() {
        return Ok(());
    }
    let mut last: Option<UnifyError> = None;
    for candidate in 0..right.len() {
        if used[candidate] {
            continue;
        }
        if !ctx.budget.consume() {
            return Err(UnifyError::BudgetExhausted);
        }
        let saved = subst.clone();
        used[candidate] = true;
        if let Err(err) = match_element(ctx, left[index], right[candidate], subst) {
            used[candidate] = false;
            *subst = saved;
            if fatal(&err) {
                return Err(err);
            }
            last = Some(err);
            continue;
        }
        if let Err(err) = match_flat_permutations(ctx, left, right, index + 1, used, subst) {
            used[candidate] = false;
            *subst = saved;
            if fatal(&err) {
                return Err(err);
            }
            last = Some(err);
            continue;
        }
        return Ok(());
    }
    Err(last.unwrap_or(UnifyError::NoAcSubsetMatch))
}

/// Flattens nested applications of `symbol` under associativity, dereferencing
/// the substitution as it goes.
pub fn flatten_ac(
    term: TermId,
    symbol: SymbolId,
    subst: &IdSubstitution,
    bank: &TermBank,
) -> Vec<TermId> {
    let mut result = Vec::new();
    let mut stack = vec![term];
    while let Some(current) = stack.pop() {
        let current = deref_id(current, subst, bank);
        if let TermNode::App(head, args) = bank.get(current)
            && *head == symbol
        {
            for arg in args.iter().rev() {
                stack.push(*arg);
            }
            continue;
        }
        result.push(current);
    }
    result
}

fn deref_id(mut term: TermId, subst: &IdSubstitution, bank: &TermBank) -> TermId {
    let mut steps = 0;
    loop {
        if let TermNode::Var(v) = bank.get(term)
            && let Some(next) = subst.get(*v)
        {
            term = next;
            steps += 1;
            debug_assert!(steps < 100_000, "deref_id cycle");
            continue;
        }
        break;
    }
    term
}

fn contains_var_id(term: TermId, var: VarId, subst: &IdSubstitution, bank: &TermBank) -> bool {
    let term = deref_id(term, subst, bank);
    match bank.get(term) {
        TermNode::Var(v) => *v == var,
        TermNode::App(_, args) => args.iter().any(|&a| contains_var_id(a, var, subst, bank)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::SymbolTable;

    struct Fixture {
        bank: TermBank,
        symbols: SymbolTable,
        comm: HashSet<SymbolId>,
        assoc: HashSet<SymbolId>,
    }

    impl Fixture {
        fn new(ac: &[&str]) -> Self {
            let mut symbols = SymbolTable::new();
            for name in ["f", "g", "h", "a", "b", "c", "e", "inv"] {
                symbols.intern(name);
            }
            let mut comm = HashSet::new();
            let mut assoc = HashSet::new();
            for name in ac {
                let id = symbols.resolve_name(name).expect(name);
                comm.insert(id);
                assoc.insert(id);
            }
            Self {
                bank: TermBank::new(),
                symbols,
                comm,
                assoc,
            }
        }

        fn f(&mut self, args: Vec<TermId>) -> TermId {
            let id = self.symbols.resolve_name("f").unwrap();
            self.bank.intern_app(id, args)
        }

        fn const_(&mut self, name: &str) -> TermId {
            let id = self.symbols.resolve_name(name).unwrap();
            self.bank.intern_app(id, Vec::new())
        }

        fn var(&mut self, v: u32) -> TermId {
            self.bank.intern_var(v)
        }

        fn render(&self, term: TermId) -> String {
            render(&self.bank, &self.symbols, term)
        }
    }

    fn render(bank: &TermBank, symbols: &SymbolTable, term: TermId) -> String {
        match bank.get(term) {
            TermNode::Var(v) => format!("X{v}"),
            TermNode::App(symbol, args) => format!(
                "{}({})",
                symbols.resolve(*symbol),
                args.iter()
                    .map(|a| render(bank, symbols, *a))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }

    #[test]
    fn matches_a_subset_and_leaves_the_rest_untouched() {
        // f(X0, X1)  into  f(a, b, c):  X0 := a, X1 := b, {c} untouched.
        let mut fx = Fixture::new(&["f"]);
        let x0 = fx.var(0);
        let x1 = fx.var(1);
        let a = fx.const_("a");
        let b = fx.const_("b");
        let c = fx.const_("c");
        let pattern = fx.f(vec![x0, x1]);
        let target = fx.f(vec![a, b, c]);

        let mut budget = AcSubsetBudget::new(1024);
        let found = match_ac_subset_rigid_id(
            pattern,
            target,
            &fx.bank,
            &fx.comm,
            &fx.assoc,
            None,
            &mut budget,
        )
        .expect("a subset match exists");

        assert_eq!(found.consumed, vec![0, 1]);
        let applied = found.subst.apply_term(fx.bank.intern_var(0), &mut fx.bank);
        assert_eq!(fx.render(applied), "a()");
    }

    #[test]
    fn unary_pattern_cannot_erase_its_outer_symbol() {
        // f(X0) cannot match the operand X0 as a one-element subset: that would
        // erase the outer f and turn f(X0)=t into the unsound rewrite X0 → t.
        let mut fx = Fixture::new(&["f"]);
        let x0 = fx.var(0);
        let pattern = fx.f(vec![x0]);
        let a = fx.const_("a");
        let target = fx.f(vec![a, a]);
        let mut budget = AcSubsetBudget::new(1024);
        let result = match_ac_subset_rigid_id(
            pattern,
            target,
            &fx.bank,
            &fx.comm,
            &fx.assoc,
            None,
            &mut budget,
        );
        assert!(result.is_err(), "unary f(X) must not match operand X");
    }

    #[test]
    fn rigid_target_variable_is_never_in_the_substitution() {
        // f(X0, X1) into f(a, Y): X1 may be bound *to* Y — Y itself still does not
        // move. The invariant is that a rigid variable is never in the domain
        // of σ, only in its range.
        let mut fx = Fixture::new(&["f"]);
        let x0 = fx.var(0);
        let x1 = fx.var(1);
        let y = fx.var(7);
        let a = fx.const_("a");
        let pattern = fx.f(vec![x0, x1]);
        let target = fx.f(vec![a, y]);

        let rigid: HashSet<u32> = HashSet::from([7u32]);
        let mut budget = AcSubsetBudget::new(1024);
        let found = match_ac_subset_rigid_id(
            pattern,
            target,
            &fx.bank,
            &fx.comm,
            &fx.assoc,
            Some(&rigid),
            &mut budget,
        )
        .expect("matching X0 := a and X1 := Y binds no rigid variable");

        assert_eq!(found.consumed, vec![0, 1]);
        assert!(
            found.subst.get(7).is_none(),
            "the rigid variable must never be bound"
        );
    }

    #[test]
    fn rigid_target_variable_is_refused_when_it_is_the_only_pairing() {
        // f(g(X0)) into f(Y, a): making g(X0) equal Y would bind Y, so there is
        // no pairing at all. Failing is the only correct outcome.
        let mut fx = Fixture::new(&["f"]);
        let x0 = fx.var(0);
        let g = fx.symbols.resolve_name("g").unwrap();
        let y = fx.var(7);
        let a = fx.const_("a");
        let g_x0 = fx.bank.intern_app(g, vec![x0]);
        let pattern = fx.f(vec![g_x0]);
        let target = fx.f(vec![y, a]);

        let rigid: HashSet<u32> = HashSet::from([7u32]);
        let mut budget = AcSubsetBudget::new(1024);
        let outcome = match_ac_subset_rigid_id(
            pattern,
            target,
            &fx.bank,
            &fx.comm,
            &fx.assoc,
            Some(&rigid),
            &mut budget,
        );
        assert!(
            outcome.is_err(),
            "a match that can only be had by binding the rigid variable must be refused"
        );
    }

    #[test]
    fn rigid_target_variable_is_refused_when_it_is_the_only_pairing_nested() {
        // f(X0, g(X1)) into f(a, Y): the second argument can only be reached by
        // binding Y.
        let mut fx = Fixture::new(&["f"]);
        let x0 = fx.var(0);
        let x1 = fx.var(1);
        let g = fx.symbols.resolve_name("g").unwrap();
        let y = fx.var(7);
        let a = fx.const_("a");
        let g_x1 = fx.bank.intern_app(g, vec![x1]);
        let pattern = fx.f(vec![x0, g_x1]);
        let target = fx.f(vec![a, y]);

        let rigid: HashSet<u32> = HashSet::from([7u32]);
        let mut budget = AcSubsetBudget::new(1024);
        let outcome = match_ac_subset_rigid_id(
            pattern,
            target,
            &fx.bank,
            &fx.comm,
            &fx.assoc,
            Some(&rigid),
            &mut budget,
        );
        assert!(
            outcome.is_err(),
            "a match that can only be had by binding the rigid variable must be refused"
        );
    }

    #[test]
    fn budget_exhaustion_fails_closed() {
        let mut fx = Fixture::new(&["f"]);
        let x0 = fx.var(0);
        let x1 = fx.var(1);
        let a = fx.const_("a");
        let b = fx.const_("b");
        let c = fx.const_("c");
        let pattern = fx.f(vec![x0, x1]);
        let target = fx.f(vec![a, b, c]);
        let mut budget = AcSubsetBudget::new(0);
        let err = match_ac_subset_rigid_id(
            pattern,
            target,
            &fx.bank,
            &fx.comm,
            &fx.assoc,
            None,
            &mut budget,
        )
        .expect_err("an exhausted budget must not produce a match");
        assert_eq!(err, UnifyError::BudgetExhausted);
    }

    #[test]
    fn non_ac_symbol_is_an_unsupported_shape() {
        let mut fx = Fixture::new(&[]);
        let a = fx.const_("a");
        let b = fx.const_("b");
        let pattern = fx.f(vec![a]);
        let target = fx.f(vec![a, b]);
        let mut budget = AcSubsetBudget::new(1024);
        let err = match_ac_subset_rigid_id(
            pattern,
            target,
            &fx.bank,
            &fx.comm,
            &fx.assoc,
            None,
            &mut budget,
        )
        .expect_err("a non-AC symbol cannot carry an AC subset match");
        assert_eq!(err, UnifyError::UnsupportedShape);
    }

    #[test]
    fn nested_commutative_match_uses_distinct_target_arguments() {
        let mut fx = Fixture::new(&["f", "g"]);
        let a = fx.const_("a");
        let b = fx.const_("b");
        let c = fx.const_("c");
        let f = fx.symbols.resolve_name("f").unwrap();
        let g = fx.symbols.resolve_name("g").unwrap();
        let pattern_inner = fx.bank.intern_app(g, vec![a, b]);
        let target_inner = fx.bank.intern_app(g, vec![a, a]);
        let pattern = fx.bank.intern_app(f, vec![pattern_inner, a]);
        let target = fx.bank.intern_app(f, vec![target_inner, a, c]);
        let comm = HashSet::from([f, g]);
        let assoc = HashSet::from([f, g]);
        let mut budget = AcSubsetBudget::new(4096);

        assert!(matches!(
            match_ac_subset_rigid_id(pattern, target, &fx.bank, &comm, &assoc, None, &mut budget,),
            Err(UnifyError::NoAcSubsetMatch)
        ));
    }

    #[test]
    fn nested_commutative_match_backtracks_over_a_real_permutation() {
        let mut fx = Fixture::new(&["f", "g"]);
        let a = fx.const_("a");
        let b = fx.const_("b");
        let c = fx.const_("c");
        let f = fx.symbols.resolve_name("f").unwrap();
        let g = fx.symbols.resolve_name("g").unwrap();
        let pattern_inner = fx.bank.intern_app(g, vec![a, b]);
        let target_inner = fx.bank.intern_app(g, vec![b, a]);
        let pattern = fx.bank.intern_app(f, vec![pattern_inner, a]);
        let target = fx.bank.intern_app(f, vec![target_inner, a, c]);
        let comm = HashSet::from([f, g]);
        let assoc = HashSet::from([f, g]);
        let mut budget = AcSubsetBudget::new(4096);

        assert!(
            match_ac_subset_rigid_id(pattern, target, &fx.bank, &comm, &assoc, None, &mut budget,)
                .is_ok()
        );
    }
}
