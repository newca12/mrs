//! Structural check for Vampire's `definition_folding` inference.
//!
//! `definition_folding` replaces body-subformulas of one or more `sP_i`
//! definitions with `sP_i(args)`. Given the parent iff-definitions (after
//! our iff-completion pass) and the unfolded source, the step is sound
//! iff unfolding all `sP_i` in the conclusion yields a formula
//! alpha-equivalent to (unfolding all `sP_i` in) the source.
//!
//! ## Conservativeness
//!
//! This check exists because pure FOL ATPs routinely time out on this
//! rule. We want it to be **fast** (microseconds for typical inputs) and
//! **safe** (never hang the whole verifier, never refute incorrectly).
//! Every recursive walk is bounded by a shared step counter; if any
//! input exceeds a small cap or the work counter overflows, the check
//! bails to `None` and the caller falls through to the ATP ladder. This
//! is strictly no-worse than the pre-check baseline.
//!
//! Returns `Some(true)` on a confirmed structural match; never returns
//! `Some(false)` — structural mismatch means we cannot decide
//! (alpha-equivalence is sound but not complete for "different concrete
//! syntax means different formula"), so `None` is the honest answer.

use std::cell::Cell;
use std::collections::HashMap;

use mrs_core::alpha::alpha_equiv;
use mrs_core::{Atom, Formula, SymbolId, Term, VarId};

/// Maximum nodes in any single input formula (premise or conclusion).
/// Anything larger short-circuits to `None`. A typical Vampire-emitted
/// def-folding step has 20-200 nodes per formula; 2000 is generous.
const MAX_INPUT_NODES: usize = 2_000;

/// Maximum number of iff-definitions we will consider. Steps with more
/// than this many parent defs are vanishingly rare and not worth the
/// risk of blowup; defer to ATP.
const MAX_DEFS: usize = 8;

/// Total work budget across `unfold_once` + `mentions_symbol` +
/// `count_nodes` calls within a single `try_check` invocation. One unit
/// = one recursive call. Bounds wall time to roughly sub-millisecond.
const WORK_BUDGET: u64 = 200_000;

/// Per-formula recursion depth cap. Vampire formulas are shallow
/// (quantifier nesting rarely exceeds 20); 256 is a safe hard wall.
const MAX_DEPTH: u32 = 256;

/// Shared bail-state for one `try_check` call.
#[derive(Default)]
struct Guard {
    work: Cell<u64>,
    bailed: Cell<bool>,
}

impl Guard {
    fn step(&self) -> bool {
        if self.bailed.get() {
            return false;
        }
        let w = self.work.get() + 1;
        if w > WORK_BUDGET {
            self.bailed.set(true);
            return false;
        }
        self.work.set(w);
        true
    }
}

/// Try to verify a `definition_folding` step by structural unfolding.
///
/// `premises` are the lowered + iff-completed parent formulas.
/// `conclusion` is the lowered folded formula.
///
/// Returns `Some(true)` on a confirmed structural match; `None`
/// otherwise. Never returns `Some(false)`.
pub fn try_check(premises: &[Formula], conclusion: &Formula) -> Option<bool> {
    let g = Guard::default();

    // Pre-flight size gate: cheap walk, bounded by MAX_INPUT_NODES.
    if !size_ok(conclusion, &g) {
        return None;
    }
    for p in premises {
        if !size_ok(p, &g) {
            return None;
        }
    }

    let mut defs: HashMap<SymbolId, (Vec<VarId>, Formula)> = HashMap::new();
    let mut sources: Vec<&Formula> = Vec::new();

    for p in premises {
        if let Some((sym, params, body)) = parse_iff_def(p) {
            if mentions_symbol(&body, sym, 0, &g) {
                return None;
            }
            if defs.insert(sym, (params, body)).is_some() {
                return None;
            }
            if defs.len() > MAX_DEFS {
                return None;
            }
        } else {
            sources.push(p);
        }
    }

    if sources.len() != 1 || defs.is_empty() {
        return None;
    }

    if has_dependency_cycle(&defs) {
        return None;
    }

    let source_unfolded = unfold_all(sources[0], &defs, &g)?;
    let conclusion_unfolded = unfold_all(conclusion, &defs, &g)?;

    if g.bailed.get() {
        return None;
    }

    if alpha_equiv(&source_unfolded, &conclusion_unfolded) {
        Some(true)
    } else {
        None
    }
}

/// Walk `f` and confirm it has at most `MAX_INPUT_NODES` nodes. Returns
/// `false` (and trips `g.bailed`) on overrun.
fn size_ok(f: &Formula, g: &Guard) -> bool {
    fn rec(f: &Formula, g: &Guard, remaining: &mut isize) -> bool {
        if !g.step() {
            return false;
        }
        *remaining -= 1;
        if *remaining < 0 {
            return false;
        }
        match f {
            Formula::Atom(_) | Formula::True | Formula::False => true,
            Formula::Neg(x) | Formula::Forall(_, x) | Formula::Exists(_, x) => rec(x, g, remaining),
            Formula::And(xs) | Formula::Or(xs) => xs.iter().all(|x| rec(x, g, remaining)),
            Formula::Implies(a, b) | Formula::Iff(a, b) => {
                rec(a, g, remaining) && rec(b, g, remaining)
            }
        }
    }
    let mut remaining = MAX_INPUT_NODES as isize;
    rec(f, g, &mut remaining)
}

/// Cheap cycle detection over the def dependency graph: edge `a -> b`
/// iff `a`'s body mentions symbol `b`. Returns true if any def
/// participates in a cycle.
fn has_dependency_cycle(defs: &HashMap<SymbolId, (Vec<VarId>, Formula)>) -> bool {
    use std::collections::HashSet;
    let g = Guard::default();
    let symbols: Vec<SymbolId> = defs.keys().copied().collect();
    let mut edges: HashMap<SymbolId, Vec<SymbolId>> = HashMap::new();
    for (a, (_, body)) in defs {
        let mut out = Vec::new();
        for &b in &symbols {
            if a != &b && mentions_symbol(body, b, 0, &g) {
                out.push(b);
            }
        }
        edges.insert(*a, out);
    }
    if g.bailed.get() {
        return true; // be conservative
    }
    // DFS cycle detection.
    enum Mark {
        Visiting,
        Done,
    }
    let mut marks: HashMap<SymbolId, Mark> = HashMap::new();
    fn dfs(
        v: SymbolId,
        edges: &HashMap<SymbolId, Vec<SymbolId>>,
        marks: &mut HashMap<SymbolId, Mark>,
    ) -> bool {
        match marks.get(&v) {
            Some(Mark::Visiting) => return true,
            Some(Mark::Done) => return false,
            None => {}
        }
        marks.insert(v, Mark::Visiting);
        if let Some(succ) = edges.get(&v) {
            for &w in succ {
                if dfs(w, edges, marks) {
                    return true;
                }
            }
        }
        marks.insert(v, Mark::Done);
        false
    }
    let _ = HashSet::<SymbolId>::new(); // silence unused-import warning if any
    for &v in &symbols {
        if dfs(v, &edges, &mut marks) {
            return true;
        }
    }
    false
}

/// If `f` matches `∀X⃗. (sP(X⃗) ↔ body)` or `∀X⃗. (body ↔ sP(X⃗))`,
/// return `(sP, X⃗, body)`. The Forall prefix may be empty.
fn parse_iff_def(f: &Formula) -> Option<(SymbolId, Vec<VarId>, Formula)> {
    let mut binders: Vec<VarId> = Vec::new();
    let mut body: &Formula = f;
    while let Formula::Forall(v, inner) = body {
        binders.push(*v);
        body = inner;
    }
    let (a, b) = match body {
        Formula::Iff(a, b) => (a.as_ref(), b.as_ref()),
        _ => return None,
    };

    if let Some((sym, args)) = head_pred_app(a) {
        let params = args_as_vars(args)?;
        if !same_var_set(&binders, &params) {
            return None;
        }
        return Some((sym, params, b.clone()));
    }
    if let Some((sym, args)) = head_pred_app(b) {
        let params = args_as_vars(args)?;
        if !same_var_set(&binders, &params) {
            return None;
        }
        return Some((sym, params, a.clone()));
    }
    None
}

fn head_pred_app(f: &Formula) -> Option<(SymbolId, &[Term])> {
    if let Formula::Atom(Atom::Pred(sym, args)) = f {
        Some((*sym, args))
    } else {
        None
    }
}

fn args_as_vars(args: &[Term]) -> Option<Vec<VarId>> {
    let mut out = Vec::with_capacity(args.len());
    for t in args {
        match t {
            Term::Var(v) => out.push(*v),
            _ => return None,
        }
    }
    let mut seen = std::collections::HashSet::new();
    for &v in &out {
        if !seen.insert(v) {
            return None;
        }
    }
    Some(out)
}

fn same_var_set(a: &[VarId], b: &[VarId]) -> bool {
    let sa: std::collections::HashSet<_> = a.iter().copied().collect();
    let sb: std::collections::HashSet<_> = b.iter().copied().collect();
    sa == sb
}

/// True if `f` syntactically mentions a predicate application of `sym`.
/// Walks under the work guard; returns `false` (without recursing
/// further) once the guard trips.
fn mentions_symbol(f: &Formula, sym: SymbolId, depth: u32, g: &Guard) -> bool {
    if !g.step() || depth >= MAX_DEPTH {
        return false;
    }
    match f {
        Formula::Atom(Atom::Pred(s, _)) => *s == sym,
        Formula::Atom(Atom::Eq(..)) => false,
        Formula::Neg(x) | Formula::Forall(_, x) | Formula::Exists(_, x) => {
            mentions_symbol(x, sym, depth + 1, g)
        }
        Formula::And(xs) | Formula::Or(xs) => {
            xs.iter().any(|x| mentions_symbol(x, sym, depth + 1, g))
        }
        Formula::Implies(a, b) | Formula::Iff(a, b) => {
            mentions_symbol(a, sym, depth + 1, g) || mentions_symbol(b, sym, depth + 1, g)
        }
        Formula::True | Formula::False => false,
    }
}

/// Iteratively unfold all defined symbols in `f` until none remain.
/// Bounded by `WORK_BUDGET` (via the guard) and by `defs.len() + 4`
/// outer iterations (definitions are non-recursive so each pass must
/// shrink the active symbol set).
fn unfold_all(
    f: &Formula,
    defs: &HashMap<SymbolId, (Vec<VarId>, Formula)>,
    g: &Guard,
) -> Option<Formula> {
    let mut current = unfold_once(f, defs, 0, g)?;
    for _ in 0..(defs.len() + 4) {
        if g.bailed.get() {
            return None;
        }
        // Check size after each pass: a definition whose body has many
        // call-sites can blow up the formula exponentially even though
        // each individual unfold is sound. Bail before doing more work.
        let mut rem = MAX_INPUT_NODES as isize * 4;
        fn count(f: &Formula, rem: &mut isize) -> bool {
            *rem -= 1;
            if *rem < 0 {
                return false;
            }
            match f {
                Formula::Atom(_) | Formula::True | Formula::False => true,
                Formula::Neg(x) | Formula::Forall(_, x) | Formula::Exists(_, x) => count(x, rem),
                Formula::And(xs) | Formula::Or(xs) => xs.iter().all(|x| count(x, rem)),
                Formula::Implies(a, b) | Formula::Iff(a, b) => count(a, rem) && count(b, rem),
            }
        }
        if !count(&current, &mut rem) {
            return None;
        }
        let needs_more = defs.keys().any(|s| mentions_symbol(&current, *s, 0, g));
        if g.bailed.get() {
            return None;
        }
        if !needs_more {
            return Some(current);
        }
        current = unfold_once(&current, defs, 0, g)?;
    }
    None
}

fn unfold_once(
    f: &Formula,
    defs: &HashMap<SymbolId, (Vec<VarId>, Formula)>,
    depth: u32,
    g: &Guard,
) -> Option<Formula> {
    if !g.step() || depth >= MAX_DEPTH {
        return None;
    }
    Some(match f {
        Formula::Atom(Atom::Pred(sym, args)) => {
            if let Some((params, body)) = defs.get(sym) {
                if params.len() != args.len() {
                    return None;
                }
                substitute_vars(body, params, args)
            } else {
                f.clone()
            }
        }
        Formula::Atom(Atom::Eq(..)) => f.clone(),
        Formula::Neg(x) => Formula::Neg(Box::new(unfold_once(x, defs, depth + 1, g)?)),
        Formula::And(xs) => {
            let mut out = Vec::with_capacity(xs.len());
            for x in xs {
                out.push(unfold_once(x, defs, depth + 1, g)?);
            }
            Formula::And(out)
        }
        Formula::Or(xs) => {
            let mut out = Vec::with_capacity(xs.len());
            for x in xs {
                out.push(unfold_once(x, defs, depth + 1, g)?);
            }
            Formula::Or(out)
        }
        Formula::Implies(a, b) => Formula::Implies(
            Box::new(unfold_once(a, defs, depth + 1, g)?),
            Box::new(unfold_once(b, defs, depth + 1, g)?),
        ),
        Formula::Iff(a, b) => Formula::Iff(
            Box::new(unfold_once(a, defs, depth + 1, g)?),
            Box::new(unfold_once(b, defs, depth + 1, g)?),
        ),
        Formula::Forall(v, x) => Formula::Forall(*v, Box::new(unfold_once(x, defs, depth + 1, g)?)),
        Formula::Exists(v, x) => Formula::Exists(*v, Box::new(unfold_once(x, defs, depth + 1, g)?)),
        Formula::True | Formula::False => f.clone(),
    })
}

/// Substitute each `params[i] → args[i]` simultaneously throughout `body`.
///
/// Uses a local single-pass substituter rather than
/// [`mrs_core::Substitution::apply_term`], because the latter follows
/// variable chains iteratively and infinite-loops in release builds on
/// substitutions that contain cycles (e.g. `X0 → Var(X1)`,
/// `X1 → Var(X0)`). For `definition_folding` it is normal for the def's
/// formal parameters and the call-site arguments to share VarIds (the
/// def `∀X0,X1. sP(X1,X0) ↔ …` and the call site `sP(X1, X0)` use the
/// same VarIds), which produces exactly this kind of cycle.
///
/// Capture-avoidance: bound variables inside `body` are *not*
/// substituted. We follow `Substitution::apply_formula`'s convention of
/// not renaming inner binders; in practice Vampire emits definitions
/// whose inner bound variables (e.g. X2 in `∀X2. …`) differ from the
/// formal parameters, so capture does not arise.
fn substitute_vars(body: &Formula, params: &[VarId], args: &[Term]) -> Formula {
    let map: HashMap<VarId, &Term> = params.iter().copied().zip(args.iter()).collect();
    sub_formula(body, &map)
}

fn sub_formula(f: &Formula, map: &HashMap<VarId, &Term>) -> Formula {
    match f {
        Formula::Atom(a) => Formula::Atom(sub_atom(a, map)),
        Formula::Neg(g) => Formula::Neg(Box::new(sub_formula(g, map))),
        Formula::And(gs) => Formula::And(gs.iter().map(|g| sub_formula(g, map)).collect()),
        Formula::Or(gs) => Formula::Or(gs.iter().map(|g| sub_formula(g, map)).collect()),
        Formula::Implies(a, b) => {
            Formula::Implies(Box::new(sub_formula(a, map)), Box::new(sub_formula(b, map)))
        }
        Formula::Iff(a, b) => {
            Formula::Iff(Box::new(sub_formula(a, map)), Box::new(sub_formula(b, map)))
        }
        Formula::Forall(v, g) => {
            if map.contains_key(v) {
                // Bound variable shadows a substituted parameter: do
                // not substitute inside. (Same convention as
                // mrs_core::Substitution::apply_formula.)
                Formula::Forall(*v, g.clone())
            } else {
                Formula::Forall(*v, Box::new(sub_formula(g, map)))
            }
        }
        Formula::Exists(v, g) => {
            if map.contains_key(v) {
                Formula::Exists(*v, g.clone())
            } else {
                Formula::Exists(*v, Box::new(sub_formula(g, map)))
            }
        }
        Formula::True | Formula::False => f.clone(),
    }
}

fn sub_atom(a: &Atom, map: &HashMap<VarId, &Term>) -> Atom {
    match a {
        Atom::Pred(p, args) => Atom::Pred(*p, args.iter().map(|t| sub_term(t, map)).collect()),
        Atom::Eq(l, r) => Atom::Eq(sub_term(l, map), sub_term(r, map)),
    }
}

/// Single-pass term substitution: each variable is replaced at most once
/// by its mapped term, with no recursive re-substitution.
fn sub_term(t: &Term, map: &HashMap<VarId, &Term>) -> Term {
    match t {
        Term::Var(v) => match map.get(v) {
            Some(&replacement) => replacement.clone(),
            None => t.clone(),
        },
        Term::App(f, args) => Term::App(*f, args.iter().map(|a| sub_term(a, map)).collect()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::{Atom, Formula, SymbolTable, Term};

    #[test]
    fn rejects_when_no_sources() {
        let mut s = SymbolTable::new();
        let p_sym = s.intern("sP1");
        let q_sym = s.intern("q");
        let p_app = Formula::Atom(Atom::Pred(p_sym, vec![Term::var(0)]));
        let body = Formula::Atom(Atom::Pred(q_sym, vec![Term::var(0)]));
        let def = Formula::forall(0, Formula::iff(p_app, body));
        let concl = Formula::Atom(Atom::Pred(q_sym, vec![Term::var(0)]));
        assert_eq!(try_check(&[def], &concl), None);
    }

    #[test]
    fn rejects_when_multiple_sources() {
        let mut s = SymbolTable::new();
        let q_sym = s.intern("q");
        let r_sym = s.intern("r");
        let src1 = Formula::Atom(Atom::Pred(q_sym, vec![]));
        let src2 = Formula::Atom(Atom::Pred(r_sym, vec![]));
        let concl = src1.clone();
        assert_eq!(try_check(&[src1, src2], &concl), None);
    }

    #[test]
    fn folds_single_atom_definition() {
        let mut s = SymbolTable::new();
        let p_sym = s.intern("sP1");
        let q_sym = s.intern("q");
        let a_sym = s.intern("a");
        let p_app = Formula::Atom(Atom::Pred(p_sym, vec![Term::var(0)]));
        let q_app = Formula::Atom(Atom::Pred(q_sym, vec![Term::var(0)]));
        let def = Formula::forall(0, Formula::iff(p_app, q_app));
        let src = Formula::Atom(Atom::Pred(q_sym, vec![Term::constant(a_sym)]));
        let concl = Formula::Atom(Atom::Pred(p_sym, vec![Term::constant(a_sym)]));
        assert_eq!(try_check(&[def, src], &concl), Some(true));
    }

    #[test]
    fn folds_compound_body() {
        let mut s = SymbolTable::new();
        let p_sym = s.intern("sP1");
        let q_sym = s.intern("q");
        let r_sym = s.intern("r");
        let a_sym = s.intern("a");
        let p_app = Formula::Atom(Atom::Pred(p_sym, vec![Term::var(0)]));
        let body = Formula::And(vec![
            Formula::Atom(Atom::Pred(q_sym, vec![Term::var(0)])),
            Formula::Atom(Atom::Pred(r_sym, vec![Term::var(0)])),
        ]);
        let def = Formula::forall(0, Formula::iff(p_app, body));
        let src = Formula::And(vec![
            Formula::Atom(Atom::Pred(q_sym, vec![Term::constant(a_sym)])),
            Formula::Atom(Atom::Pred(r_sym, vec![Term::constant(a_sym)])),
        ]);
        let concl = Formula::Atom(Atom::Pred(p_sym, vec![Term::constant(a_sym)]));
        assert_eq!(try_check(&[def, src], &concl), Some(true));
    }

    #[test]
    fn folds_multiple_definitions_in_chain() {
        let mut s = SymbolTable::new();
        let p1 = s.intern("sP1");
        let p2 = s.intern("sP2");
        let q = s.intern("q");
        let r = s.intern("r");
        let a = s.intern("a");
        let def1 = Formula::forall(
            0,
            Formula::iff(
                Formula::Atom(Atom::Pred(p1, vec![Term::var(0)])),
                Formula::Atom(Atom::Pred(q, vec![Term::var(0)])),
            ),
        );
        let def2 = Formula::forall(
            0,
            Formula::iff(
                Formula::Atom(Atom::Pred(p2, vec![Term::var(0)])),
                Formula::Or(vec![
                    Formula::Atom(Atom::Pred(p1, vec![Term::var(0)])),
                    Formula::Atom(Atom::Pred(r, vec![Term::var(0)])),
                ]),
            ),
        );
        let src = Formula::Or(vec![
            Formula::Atom(Atom::Pred(q, vec![Term::constant(a)])),
            Formula::Atom(Atom::Pred(r, vec![Term::constant(a)])),
        ]);
        let concl = Formula::Atom(Atom::Pred(p2, vec![Term::constant(a)]));
        assert_eq!(try_check(&[def1, def2, src], &concl), Some(true));
    }

    #[test]
    fn rejects_recursive_definition() {
        let mut s = SymbolTable::new();
        let p = s.intern("sP1");
        let q = s.intern("q");
        let def = Formula::forall(
            0,
            Formula::iff(
                Formula::Atom(Atom::Pred(p, vec![Term::var(0)])),
                Formula::And(vec![
                    Formula::Atom(Atom::Pred(q, vec![Term::var(0)])),
                    Formula::Atom(Atom::Pred(p, vec![Term::var(0)])),
                ]),
            ),
        );
        let src = Formula::Atom(Atom::Pred(q, vec![]));
        let concl = src.clone();
        assert_eq!(try_check(&[def, src], &concl), None);
    }

    #[test]
    fn rejects_unrelated_source_and_conclusion() {
        let mut s = SymbolTable::new();
        let p = s.intern("sP1");
        let q = s.intern("q");
        let r = s.intern("r");
        let a = s.intern("a");
        let b = s.intern("b");
        let def = Formula::forall(
            0,
            Formula::iff(
                Formula::Atom(Atom::Pred(p, vec![Term::var(0)])),
                Formula::Atom(Atom::Pred(q, vec![Term::var(0)])),
            ),
        );
        let src = Formula::Atom(Atom::Pred(q, vec![Term::constant(a)]));
        let concl = Formula::Atom(Atom::Pred(r, vec![Term::constant(b)]));
        assert_eq!(try_check(&[def, src], &concl), None);
    }

    #[test]
    fn folds_under_outer_quantifiers() {
        let mut s = SymbolTable::new();
        let p = s.intern("sP1");
        let q = s.intern("q");
        let def = Formula::forall(
            0,
            Formula::iff(
                Formula::Atom(Atom::Pred(p, vec![Term::var(0)])),
                Formula::Atom(Atom::Pred(q, vec![Term::var(0)])),
            ),
        );
        let src = Formula::forall(1, Formula::Atom(Atom::Pred(q, vec![Term::var(1)])));
        let concl = Formula::forall(1, Formula::Atom(Atom::Pred(p, vec![Term::var(1)])));
        assert_eq!(try_check(&[def, src], &concl), Some(true));
    }

    /// Stress test: even if a pathological input were fed in, the
    /// function must return quickly (well under a second) rather than
    /// hang. We construct a deep nested formula that exceeds
    /// MAX_INPUT_NODES; the size pre-check must fire and return None.
    #[test]
    fn bails_on_oversized_input() {
        let mut s = SymbolTable::new();
        let p = s.intern("sP1");
        let q = s.intern("q");
        let def = Formula::forall(
            0,
            Formula::iff(
                Formula::Atom(Atom::Pred(p, vec![Term::var(0)])),
                Formula::Atom(Atom::Pred(q, vec![Term::var(0)])),
            ),
        );
        // Build an And with 5000 leaves.
        let leaf = Formula::Atom(Atom::Pred(q, vec![Term::var(0)]));
        let huge = Formula::And(vec![leaf; 5000]);
        let concl = Formula::Atom(Atom::Pred(p, vec![Term::var(0)]));
        let start = std::time::Instant::now();
        let result = try_check(&[def, huge], &concl);
        let elapsed = start.elapsed();
        assert_eq!(result, None);
        assert!(
            elapsed.as_millis() < 100,
            "oversized input took {:?}",
            elapsed
        );
    }
}
