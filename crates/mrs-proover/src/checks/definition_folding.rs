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
//! Returns `Some(crate::verdict::StepOutcome::Sound)` on a confirmed structural match; never returns
//! `Some(false)` — structural mismatch means we cannot decide
//! (alpha-equivalence is sound but not complete for "different concrete
//! syntax means different formula"), so `None` is the honest answer.

use std::cell::Cell;
use std::collections::HashMap;

use mrs_core::alpha::alpha_equiv;
use mrs_core::{Atom, Formula, SymbolId, SymbolTable, Term, VarId};

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

fn projects_conj_or_disj(parent: &Formula, concl: &Formula) -> bool {
    let mut p_body = parent;
    while let Formula::Forall(_, inner) | Formula::Exists(_, inner) = p_body {
        p_body = inner;
    }
    let mut c_body = concl;
    while let Formula::Forall(_, inner) | Formula::Exists(_, inner) = c_body {
        c_body = inner;
    }
    projects_conj_or_disj_core(p_body, c_body)
}

fn projects_conj_or_disj_core(parent: &Formula, concl: &Formula) -> bool {
    if mrs_core::alpha::alpha_equiv(parent, concl) {
        return true;
    }
    match parent {
        Formula::And(cs) => cs.iter().any(|c| projects_conj_or_disj_core(c, concl)),
        _ => false,
    }
}

/// Try to verify a `definition_folding` step by structural unfolding.
///
/// `premises` are the lowered + iff-completed parent formulas.
/// `conclusion` is the lowered folded formula.
///
/// Returns `Some(crate::verdict::StepOutcome::Sound)` on a confirmed structural match; `None`
/// otherwise. Never returns `Some(false)`.
pub fn try_check(
    premises: &[Formula],
    is_def: &[bool],
    conclusion: &Formula,
) -> Option<crate::verdict::StepOutcome> {
    if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
        eprintln!(
            "[prop-sat-dbg] definition_folding::try_check entered! premises len = {}, is_def = {:?}",
            premises.len(),
            is_def
        );
    }
    let g = Guard::default();

    // Pre-flight size gate: cheap walk, bounded by MAX_INPUT_NODES.
    if !size_ok(conclusion, &g) {
        if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
            eprintln!("[prop-sat-dbg] definition_folding early exit 1");
        }
        return None;
    }
    for p in premises {
        if !size_ok(p, &g) {
            if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
                eprintln!("[prop-sat-dbg] definition_folding early exit 2");
            }
            return None;
        }
    }

    let mut defs: HashMap<SymbolId, (Vec<VarId>, Formula)> = HashMap::new();
    let mut sources: Vec<&Formula> = Vec::new();

    for (i, p) in premises.iter().enumerate() {
        if is_def.get(i).copied().unwrap_or(false) {
            let parsed = parse_iff_def(p);
            if parsed.is_none() {
                if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
                    eprintln!(
                        "[prop-sat-dbg] definition_folding early exit 3 (failed to parse iff def for premise {})",
                        i
                    );
                }
                return None;
            }
            let (sym, params, body) = parsed.unwrap();
            if mentions_symbol(&body, sym, 0, &g) {
                if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
                    eprintln!("[prop-sat-dbg] definition_folding early exit 4");
                }
                return None;
            }
            if defs.insert(sym, (params, body)).is_some() {
                if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
                    eprintln!("[prop-sat-dbg] definition_folding early exit 5");
                }
                return None;
            }
            if defs.len() > MAX_DEFS {
                if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
                    eprintln!("[prop-sat-dbg] definition_folding early exit 6");
                }
                return None;
            }
        } else {
            sources.push(p);
        }
    }

    if sources.len() != 1 || defs.is_empty() {
        if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
            eprintln!(
                "[prop-sat-dbg] definition_folding early exit 7 (sources len = {}, defs empty = {})",
                sources.len(),
                defs.is_empty()
            );
        }
        return None;
    }

    if has_dependency_cycle(&defs) {
        return Some(crate::verdict::StepOutcome::Unsound(
            "recursive/cyclic definition unfolding".into(),
        ));
    }

    // Fresh-variable supply for capture-avoiding unfolding. Each premise
    // and the conclusion is lowered with an independently-reset variable
    // counter, so a definition body's *own* bound variables (e.g. the
    // `?[X]` inside `sP(a) <=> ?[X]. q(X,a)`) routinely reuse low VarIds
    // that also occur as call-site arguments or in the surrounding
    // formula. Substituting the body verbatim would capture those
    // variables. We therefore α-rename every body binder to a globally
    // fresh VarId (above any VarId occurring anywhere in the inputs) at
    // each expansion. Start above the max VarId in defs, source and
    // conclusion.
    let mut max_v: u32 = 0;
    max_var(conclusion, &mut max_v);
    for p in premises {
        max_var(p, &mut max_v);
    }
    for (_, body) in defs.values() {
        max_var(body, &mut max_v);
    }
    let fresh = Cell::new(max_v.wrapping_add(1));

    let source_unfolded = unfold_all(sources[0], &defs, &fresh, &g)?;
    let conclusion_unfolded = unfold_all(conclusion, &defs, &fresh, &g)?;

    if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
        eprintln!(
            "[prop-sat-dbg] definition_folding: source_unfolded = {:?}",
            source_unfolded
        );
        eprintln!(
            "[prop-sat-dbg] definition_folding: conclusion_unfolded = {:?}",
            conclusion_unfolded
        );
    }

    if g.bailed.get() {
        return None;
    }

    // Compare modulo α-renaming, associativity/commutativity of ∧/∨, and
    // symmetry of `=`. All three are logically valid equivalences, so a
    // match is a sound entailment (in fact equivalence). E routinely
    // permutes clause literals during `definition_folding`, so plain
    // structural α-equivalence is too strict; we canonicalise both sides
    // (De Bruijn binders + sorted ∧/∨ children + sorted `=` sides) and
    // compare for equality.
    if alpha_equiv(&source_unfolded, &conclusion_unfolded)
        || canon_eq(&source_unfolded, &conclusion_unfolded, None)
        || projects_conj_or_disj(&source_unfolded, &conclusion_unfolded)
    {
        Some(crate::verdict::StepOutcome::Sound)
    } else {
        None
    }
}

/// Canonical, order-insensitive form used to compare two formulas modulo
/// α-renaming, AC of ∧/∨, and symmetry of `=`.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum CTerm {
    /// De Bruijn index (distance from binder) of a bound variable.
    Bound(u32),
    /// Free variable, kept by original id.
    Free(VarId),
    App(SymbolId, Vec<CTerm>),
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum CForm {
    True,
    False,
    Pred(SymbolId, Vec<CTerm>),
    /// `=` with operands sorted (symmetry).
    Eq(Box<CTerm>, Box<CTerm>),
    Neg(Box<CForm>),
    /// children sorted (AC).
    And(Vec<CForm>),
    /// children sorted (AC).
    Or(Vec<CForm>),
    Implies(Box<CForm>, Box<CForm>),
    Iff(Box<CForm>, Box<CForm>),
    Forall(Box<CForm>),
    Exists(Box<CForm>),
}

/// True iff `a` and `b` have the same canonical form.
pub(crate) fn canon_eq(a: &Formula, b: &Formula, symbols: Option<&SymbolTable>) -> bool {
    canon_form(a, &mut Vec::new(), symbols) == canon_form(b, &mut Vec::new(), symbols)
}

fn collect_free_vars_term(t: &Term, vars: &mut Vec<VarId>) {
    match t {
        Term::Var(v) => {
            if !vars.contains(v) {
                vars.push(*v);
            }
        }
        Term::App(_, args) => {
            for a in args {
                collect_free_vars_term(a, vars);
            }
        }
    }
}

fn collect_free_vars(f: &Formula, vars: &mut Vec<VarId>) {
    match f {
        Formula::Atom(Atom::Pred(_, args)) => {
            for t in args {
                collect_free_vars_term(t, vars);
            }
        }
        Formula::Atom(Atom::Eq(l, r)) => {
            collect_free_vars_term(l, vars);
            collect_free_vars_term(r, vars);
        }
        Formula::Neg(inner) => collect_free_vars(inner, vars),
        Formula::And(cs) | Formula::Or(cs) => {
            for c in cs {
                collect_free_vars(c, vars);
            }
        }
        Formula::Implies(l, r) | Formula::Iff(l, r) => {
            collect_free_vars(l, vars);
            collect_free_vars(r, vars);
        }
        _ => {}
    }
}

fn apply_subst_term_fold(t: &Term, subst: &HashMap<VarId, Term>) -> Term {
    match t {
        Term::Var(id) => {
            if let Some(existing) = subst.get(id) {
                existing.clone()
            } else {
                t.clone()
            }
        }
        Term::App(f, args) => {
            let n_args = args
                .iter()
                .map(|arg| apply_subst_term_fold(arg, subst))
                .collect();
            Term::App(*f, n_args)
        }
    }
}

fn apply_subst_formula_fold(f: &Formula, subst: &HashMap<VarId, Term>) -> Formula {
    match f {
        Formula::Atom(Atom::Pred(p, args)) => {
            let n_args = args
                .iter()
                .map(|t| apply_subst_term_fold(t, subst))
                .collect();
            Formula::Atom(Atom::Pred(*p, n_args))
        }
        Formula::Atom(Atom::Eq(l, r)) => {
            let nl = apply_subst_term_fold(l, subst);
            let nr = apply_subst_term_fold(r, subst);
            Formula::Atom(Atom::Eq(nl, nr))
        }
        Formula::Neg(inner) => Formula::Neg(Box::new(apply_subst_formula_fold(inner, subst))),
        Formula::And(cs) => {
            let ncs = cs
                .iter()
                .map(|c| apply_subst_formula_fold(c, subst))
                .collect();
            Formula::And(ncs)
        }
        Formula::Or(cs) => {
            let ncs = cs
                .iter()
                .map(|c| apply_subst_formula_fold(c, subst))
                .collect();
            Formula::Or(ncs)
        }
        Formula::Implies(l, r) => {
            let nl = apply_subst_formula_fold(l, subst);
            let nr = apply_subst_formula_fold(r, subst);
            Formula::Implies(Box::new(nl), Box::new(nr))
        }
        Formula::Iff(l, r) => {
            let nl = apply_subst_formula_fold(l, subst);
            let nr = apply_subst_formula_fold(r, subst);
            Formula::Iff(Box::new(nl), Box::new(nr))
        }
        Formula::Forall(v, inner) => {
            Formula::Forall(*v, Box::new(apply_subst_formula_fold(inner, subst)))
        }
        Formula::Exists(v, inner) => {
            Formula::Exists(*v, Box::new(apply_subst_formula_fold(inner, subst)))
        }
        Formula::True => Formula::True,
        Formula::False => Formula::False,
    }
}

fn match_permutations(
    idx: usize,
    f_vars: &[VarId],
    p_vars: &[VarId],
    used: &mut Vec<bool>,
    current_map: &mut HashMap<VarId, VarId>,
    proof_f: &Formula,
    prob_canon: &CForm,
    symbols: Option<&SymbolTable>,
) -> bool {
    if idx == f_vars.len() {
        let mut map: HashMap<VarId, Term> = HashMap::new();
        for (&v, &pv) in current_map.iter() {
            map.insert(v, Term::Var(pv));
        }
        let mapped_f = apply_subst_formula_fold(proof_f, &map);
        let ca = canon_form(&mapped_f, &mut Vec::new(), symbols);
        return ca == *prob_canon;
    }
    for i in 0..p_vars.len() {
        if !used[i] {
            used[i] = true;
            current_map.insert(f_vars[idx], p_vars[i]);
            if match_permutations(
                idx + 1,
                f_vars,
                p_vars,
                used,
                current_map,
                proof_f,
                prob_canon,
                symbols,
            ) {
                return true;
            }
            current_map.remove(&f_vars[idx]);
            used[i] = false;
        }
    }
    false
}

pub(crate) fn canon_eq_free(a: &Formula, b: &Formula, symbols: Option<&SymbolTable>) -> bool {
    let mut a_body = a;
    while let Formula::Forall(_, inner) | Formula::Exists(_, inner) = a_body {
        a_body = inner;
    }
    let mut b_body = b;
    while let Formula::Forall(_, inner) | Formula::Exists(_, inner) = b_body {
        b_body = inner;
    }

    let mut a_vars = Vec::new();
    collect_free_vars(a_body, &mut a_vars);
    let mut b_vars = Vec::new();
    collect_free_vars(b_body, &mut b_vars);

    if a_vars.len() != b_vars.len() {
        return false;
    }

    let cb = canon_form(b_body, &mut Vec::new(), symbols);

    let mut used = vec![false; b_vars.len()];
    let mut map = HashMap::new();
    match_permutations(
        0, &a_vars, &b_vars, &mut used, &mut map, a_body, &cb, symbols,
    )
}

/// `scope` is the stack of bound variables (innermost last); a `Var(v)`
/// resolves to `Bound(index)` using the nearest enclosing binder of `v`,
/// else `Free(v)`.
fn canon_term(t: &Term, scope: &[VarId], symbols: Option<&SymbolTable>) -> CTerm {
    match t {
        Term::Var(v) => match scope.iter().rposition(|s| s == v) {
            Some(pos) => CTerm::Bound((scope.len() - 1 - pos) as u32),
            None => CTerm::Free(*v),
        },
        Term::App(f, args) => {
            let mut c_args: Vec<CTerm> =
                args.iter().map(|a| canon_term(a, scope, symbols)).collect();
            if let Some(symbols) = symbols {
                let name = symbols.resolve(*f);
                if name == "greatest_lower_bound"
                    || name == "least_upper_bound"
                    || name == "meet"
                    || name == "join"
                    || name == "+"
                    || name == "times"
                    || name == "*"
                {
                    c_args.sort();
                }
            }
            CTerm::App(*f, c_args)
        }
    }
}

fn canon_atom(a: &Atom, scope: &[VarId], symbols: Option<&SymbolTable>) -> CForm {
    match a {
        Atom::Pred(p, args) => CForm::Pred(
            *p,
            args.iter().map(|t| canon_term(t, scope, symbols)).collect(),
        ),
        Atom::Eq(l, r) => {
            let mut cl = canon_term(l, scope, symbols);
            let mut cr = canon_term(r, scope, symbols);
            if cr < cl {
                std::mem::swap(&mut cl, &mut cr);
            }
            CForm::Eq(Box::new(cl), Box::new(cr))
        }
    }
}

/// Flatten nested ∧ (or ∨) of the same kind into one sorted child list.
fn flatten_canon(
    f: &Formula,
    scope: &mut Vec<VarId>,
    is_and: bool,
    symbols: Option<&SymbolTable>,
) -> Vec<CForm> {
    let mut out = Vec::new();
    let kids: &[Formula] = match (is_and, f) {
        (true, Formula::And(xs)) | (false, Formula::Or(xs)) => xs,
        _ => return vec![canon_form(f, scope, symbols)],
    };
    for k in kids {
        let same = matches!(
            (is_and, k),
            (true, Formula::And(_)) | (false, Formula::Or(_))
        );
        if same {
            out.extend(flatten_canon(k, scope, is_and, symbols));
        } else {
            out.push(canon_form(k, scope, symbols));
        }
    }
    out.sort();
    out
}

fn canon_form(f: &Formula, scope: &mut Vec<VarId>, symbols: Option<&SymbolTable>) -> CForm {
    match f {
        Formula::True => CForm::True,
        Formula::False => CForm::False,
        Formula::Atom(a) => canon_atom(a, scope, symbols),
        Formula::Neg(x) => CForm::Neg(Box::new(canon_form(x, scope, symbols))),
        Formula::And(_) => CForm::And(flatten_canon(f, scope, true, symbols)),
        Formula::Or(_) => CForm::Or(flatten_canon(f, scope, false, symbols)),
        Formula::Implies(a, b) => CForm::Implies(
            Box::new(canon_form(a, scope, symbols)),
            Box::new(canon_form(b, scope, symbols)),
        ),
        Formula::Iff(a, b) => {
            let mut ca = canon_form(a, scope, symbols);
            let mut cb = canon_form(b, scope, symbols);
            // Iff is commutative; sort for canonicity.
            if cb < ca {
                std::mem::swap(&mut ca, &mut cb);
            }
            CForm::Iff(Box::new(ca), Box::new(cb))
        }
        Formula::Forall(v, x) => {
            scope.push(*v);
            let c = canon_form(x, scope, symbols);
            scope.pop();
            CForm::Forall(Box::new(c))
        }
        Formula::Exists(v, x) => {
            scope.push(*v);
            let c = canon_form(x, scope, symbols);
            scope.pop();
            CForm::Exists(Box::new(c))
        }
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
    if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
        eprintln!("[prop-sat-dbg] parse_iff_def entered with formula: {:?}", f);
    }
    let mut binders: Vec<VarId> = Vec::new();
    let mut body: &Formula = f;
    while let Formula::Forall(v, inner) = body {
        binders.push(*v);
        body = inner;
    }
    let (a, b) = match body {
        Formula::Iff(a, b) => (a.as_ref(), b.as_ref()),
        _ => {
            if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
                eprintln!(
                    "[prop-sat-dbg] parse_iff_def failed: not an Iff body, body = {:?}",
                    body
                );
            }
            return None;
        }
    };

    if let Some((sym, args)) = head_pred_app(a) {
        let params = args_as_vars(args);
        if params.is_none() {
            if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
                eprintln!("[prop-sat-dbg] parse_iff_def failed: a args not vars");
            }
            return None;
        }
        let params = params.unwrap();
        if !same_var_set(&binders, &params) {
            if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
                eprintln!(
                    "[prop-sat-dbg] parse_iff_def failed: binders {:?} != params {:?}",
                    binders, params
                );
            }
            return None;
        }
        return Some((sym, params, b.clone()));
    }
    if let Some((sym, args)) = head_pred_app(b) {
        let params = args_as_vars(args);
        if params.is_none() {
            if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
                eprintln!("[prop-sat-dbg] parse_iff_def failed: b args not vars");
            }
            return None;
        }
        let params = params.unwrap();
        if !same_var_set(&binders, &params) {
            if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
                eprintln!(
                    "[prop-sat-dbg] parse_iff_def failed: binders {:?} != params {:?}",
                    binders, params
                );
            }
            return None;
        }
        return Some((sym, params, a.clone()));
    }
    if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
        eprintln!("[prop-sat-dbg] parse_iff_def failed: neither side is head pred app");
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

/// Record the maximum VarId occurring anywhere in `f` (binders and term
/// variables) into `acc`.
fn max_var(f: &Formula, acc: &mut u32) {
    fn walk_t(t: &Term, acc: &mut u32) {
        match t {
            Term::Var(v) => *acc = (*acc).max(*v),
            Term::App(_, args) => args.iter().for_each(|a| walk_t(a, acc)),
        }
    }
    fn walk_a(at: &Atom, acc: &mut u32) {
        match at {
            Atom::Pred(_, args) => args.iter().for_each(|x| walk_t(x, acc)),
            Atom::Eq(l, r) => {
                walk_t(l, acc);
                walk_t(r, acc);
            }
        }
    }
    match f {
        Formula::Atom(at) => walk_a(at, acc),
        Formula::True | Formula::False => {}
        Formula::Neg(x) => max_var(x, acc),
        Formula::Forall(v, x) | Formula::Exists(v, x) => {
            *acc = (*acc).max(*v);
            max_var(x, acc);
        }
        Formula::And(xs) | Formula::Or(xs) => xs.iter().for_each(|x| max_var(x, acc)),
        Formula::Implies(l, r) | Formula::Iff(l, r) => {
            max_var(l, acc);
            max_var(r, acc);
        }
    }
}

/// Iteratively unfold all defined symbols in `f` until none remain.
/// Bounded by `WORK_BUDGET` (via the guard) and by `defs.len() + 4`
/// outer iterations (definitions are non-recursive so each pass must
/// shrink the active symbol set).
fn unfold_all(
    f: &Formula,
    defs: &HashMap<SymbolId, (Vec<VarId>, Formula)>,
    fresh: &Cell<u32>,
    g: &Guard,
) -> Option<Formula> {
    let mut current = unfold_once(f, defs, fresh, 0, g)?;
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
        current = unfold_once(&current, defs, fresh, 0, g)?;
    }
    None
}

fn unfold_once(
    f: &Formula,
    defs: &HashMap<SymbolId, (Vec<VarId>, Formula)>,
    fresh: &Cell<u32>,
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
                substitute_vars(body, params, args, fresh)
            } else {
                f.clone()
            }
        }
        Formula::Atom(Atom::Eq(..)) => f.clone(),
        Formula::Neg(x) => Formula::Neg(Box::new(unfold_once(x, defs, fresh, depth + 1, g)?)),
        Formula::And(xs) => {
            let mut out = Vec::with_capacity(xs.len());
            for x in xs {
                out.push(unfold_once(x, defs, fresh, depth + 1, g)?);
            }
            Formula::And(out)
        }
        Formula::Or(xs) => {
            let mut out = Vec::with_capacity(xs.len());
            for x in xs {
                out.push(unfold_once(x, defs, fresh, depth + 1, g)?);
            }
            Formula::Or(out)
        }
        Formula::Implies(a, b) => Formula::Implies(
            Box::new(unfold_once(a, defs, fresh, depth + 1, g)?),
            Box::new(unfold_once(b, defs, fresh, depth + 1, g)?),
        ),
        Formula::Iff(a, b) => Formula::Iff(
            Box::new(unfold_once(a, defs, fresh, depth + 1, g)?),
            Box::new(unfold_once(b, defs, fresh, depth + 1, g)?),
        ),
        Formula::Forall(v, x) => {
            Formula::Forall(*v, Box::new(unfold_once(x, defs, fresh, depth + 1, g)?))
        }
        Formula::Exists(v, x) => {
            Formula::Exists(*v, Box::new(unfold_once(x, defs, fresh, depth + 1, g)?))
        }
        Formula::True | Formula::False => f.clone(),
    })
}

/// Substitute each `params[i] → args[i]` simultaneously throughout
/// `body`, **capture-avoiding**: every binder inside `body` is α-renamed
/// to a globally-fresh VarId drawn from `fresh`.
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
/// ## Why capture-avoidance is required
///
/// Each premise (definition) and the source/conclusion are lowered with
/// an independently-reset variable counter, so a definition body's own
/// bound variables (e.g. the `?[X]` inside `sP(a) <=> ?[X]. q(X,a)`)
/// routinely reuse low VarIds that *also* occur as call-site arguments
/// or in the surrounding formula. Substituting the body verbatim would
/// capture those variables — the inner binder would swallow an outer
/// reference — yielding a structurally-wrong (and the α-check would
/// rightly reject it). We rename every body binder to a fresh VarId
/// above any VarId in the inputs, so neither the call-site arguments
/// (≤ max input VarId) nor the surrounding context can be captured.
fn substitute_vars(body: &Formula, params: &[VarId], args: &[Term], fresh: &Cell<u32>) -> Formula {
    let mut map: HashMap<VarId, Term> = params.iter().copied().zip(args.iter().cloned()).collect();
    sub_formula(body, &mut map, fresh)
}

fn sub_formula(f: &Formula, map: &mut HashMap<VarId, Term>, fresh: &Cell<u32>) -> Formula {
    match f {
        Formula::Atom(a) => Formula::Atom(sub_atom(a, map)),
        Formula::Neg(g) => Formula::Neg(Box::new(sub_formula(g, map, fresh))),
        Formula::And(gs) => Formula::And(gs.iter().map(|g| sub_formula(g, map, fresh)).collect()),
        Formula::Or(gs) => Formula::Or(gs.iter().map(|g| sub_formula(g, map, fresh)).collect()),
        Formula::Implies(a, b) => Formula::Implies(
            Box::new(sub_formula(a, map, fresh)),
            Box::new(sub_formula(b, map, fresh)),
        ),
        Formula::Iff(a, b) => Formula::Iff(
            Box::new(sub_formula(a, map, fresh)),
            Box::new(sub_formula(b, map, fresh)),
        ),
        Formula::Forall(v, g) => {
            let (nv, inner) = sub_binder(*v, g, map, fresh);
            Formula::Forall(nv, Box::new(inner))
        }
        Formula::Exists(v, g) => {
            let (nv, inner) = sub_binder(*v, g, map, fresh);
            Formula::Exists(nv, Box::new(inner))
        }
        Formula::True | Formula::False => f.clone(),
    }
}

/// Substitute under a binder for variable `v`: allocate a fresh VarId
/// `nv`, map `v → Var(nv)` for the scope of `g`, recurse, then restore
/// the previous mapping. Returns `(nv, substituted_body)`.
fn sub_binder(
    v: VarId,
    g: &Formula,
    map: &mut HashMap<VarId, Term>,
    fresh: &Cell<u32>,
) -> (VarId, Formula) {
    let nv = fresh.get();
    fresh.set(nv.wrapping_add(1));
    let prev = map.insert(v, Term::Var(nv));
    let inner = sub_formula(g, map, fresh);
    match prev {
        Some(t) => {
            map.insert(v, t);
        }
        None => {
            map.remove(&v);
        }
    }
    (nv, inner)
}

fn sub_atom(a: &Atom, map: &HashMap<VarId, Term>) -> Atom {
    match a {
        Atom::Pred(p, args) => Atom::Pred(*p, args.iter().map(|t| sub_term(t, map)).collect()),
        Atom::Eq(l, r) => Atom::Eq(sub_term(l, map), sub_term(r, map)),
    }
}

/// Single-pass term substitution: each variable is replaced at most once
/// by its mapped term, with no recursive re-substitution.
fn sub_term(t: &Term, map: &HashMap<VarId, Term>) -> Term {
    match t {
        Term::Var(v) => match map.get(v) {
            Some(replacement) => replacement.clone(),
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
        assert_eq!(try_check(&[def], &[true], &concl), None);
    }

    #[test]
    fn rejects_when_multiple_sources() {
        let mut s = SymbolTable::new();
        let q_sym = s.intern("q");
        let r_sym = s.intern("r");
        let src1 = Formula::Atom(Atom::Pred(q_sym, vec![]));
        let src2 = Formula::Atom(Atom::Pred(r_sym, vec![]));
        let concl = src1.clone();
        assert_eq!(try_check(&[src1, src2], &[false, false], &concl), None);
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
        assert_eq!(
            try_check(&[def, src], &[true, false], &concl),
            Some(crate::verdict::StepOutcome::Sound)
        );
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
        assert_eq!(
            try_check(&[def, src], &[true, false], &concl),
            Some(crate::verdict::StepOutcome::Sound)
        );
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
        assert_eq!(
            try_check(&[def1, def2, src], &[true, true, false], &concl),
            Some(crate::verdict::StepOutcome::Sound)
        );
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
        assert_eq!(try_check(&[def, src], &[true, false], &concl), None);
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
        assert_eq!(try_check(&[def, src], &[true, false], &concl), None);
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
        assert_eq!(
            try_check(&[def, src], &[true, false], &concl),
            Some(crate::verdict::StepOutcome::Sound)
        );
    }

    #[test]
    fn folds_avatar_propositional_definitions() {
        // Vampire's `avatar_split_clause` shape: each `spl<n>` is a 0-ary
        // predicate symbol defined by an iff with no outer quantifier
        // (e.g. `fof(f7225, definition, (spl150_1086 <=> sP119))`).
        // The conclusion of the split is a propositional disjunction
        // of the spl symbols, and the source is the original (already
        // propositional) clause whose disjuncts are named by the
        // spl symbols. Unfolding all spl symbols in the conclusion
        // must α-match the source.
        let mut s = SymbolTable::new();
        let spl_a = s.intern("spl_a");
        let spl_b = s.intern("spl_b");
        let spl_c = s.intern("spl_c");
        let q = s.intern("q");
        let r = s.intern("r");
        let t = s.intern("t");
        let def_a = Formula::iff(
            Formula::Atom(Atom::Pred(spl_a, vec![])),
            Formula::Atom(Atom::Pred(q, vec![])),
        );
        let def_b = Formula::iff(
            Formula::Atom(Atom::Pred(spl_b, vec![])),
            Formula::Atom(Atom::Pred(r, vec![])),
        );
        let def_c = Formula::iff(
            Formula::Atom(Atom::Pred(spl_c, vec![])),
            Formula::Atom(Atom::Pred(t, vec![])),
        );
        let src = Formula::Or(vec![
            Formula::Atom(Atom::Pred(q, vec![])),
            Formula::Atom(Atom::Pred(r, vec![])),
            Formula::Atom(Atom::Pred(t, vec![])),
        ]);
        let concl = Formula::Or(vec![
            Formula::Atom(Atom::Pred(spl_a, vec![])),
            Formula::Atom(Atom::Pred(spl_b, vec![])),
            Formula::Atom(Atom::Pred(spl_c, vec![])),
        ]);
        assert_eq!(
            try_check(
                &[def_a, def_b, def_c, src],
                &[true, true, true, false],
                &concl
            ),
            Some(crate::verdict::StepOutcome::Sound)
        );
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
        let result = try_check(&[def, huge], &[true, false], &concl);
        let elapsed = start.elapsed();
        assert_eq!(result, None);
        assert!(
            elapsed.as_millis() < 100,
            "oversized input took {:?}",
            elapsed
        );
    }
}
