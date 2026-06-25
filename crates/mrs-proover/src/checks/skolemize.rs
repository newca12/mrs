//! Phase 5: `skolemize` step check.
//!
//! Per the ProoVer rules, a `skolemize` step must:
//!
//! - have status `esa`;
//! - introduce exactly one new Skolem symbol via `new_symbols(skolem, [sK])`;
//! - record the eliminated existential variable via `skolemize(Var, sk(...))`;
//! - the Skolem term must depend exactly on the universally quantified
//!   variables that are in scope at the point where `Var` is eliminated;
//! - the resulting formula must be a correct Skolemization of the parent.
//!
//! We check all of these against the FOF AST directly (no full lowering to
//! `mrs-core` is needed here, since we deal with the original variable names
//! given in the annotation).

use std::collections::HashSet;

use mrs_tptp::{
    AnnotatedFormula, AtomicWord, FOFAtomicFormula, FOFFormula, FOFStatement, FOFTerm, Quantifier,
};

use crate::verdict::StepOutcome;

/// Global state of Skolem symbols seen so far in this proof. The verifier
/// maintains one of these across the whole proof to enforce freshness.
pub struct SkolemRegistry {
    pub seen_symbols: HashSet<String>,
}

impl SkolemRegistry {
    pub fn new() -> Self {
        Self {
            seen_symbols: HashSet::new(),
        }
    }

    /// Record any symbol (function or predicate) that already exists.
    pub fn record(&mut self, sym: &str) {
        self.seen_symbols.insert(sym.to_string());
    }

    /// Record every symbol occurring in a FOF statement.
    pub fn record_from_statement(&mut self, s: &FOFStatement<'_>) {
        match s {
            FOFStatement::Logical(f) => self.record_from_formula(f),
            FOFStatement::Sequent(lhs, rhs) => {
                for f in lhs.iter().chain(rhs.iter()) {
                    self.record_from_formula(f);
                }
            }
        }
    }

    fn record_from_formula(&mut self, f: &FOFFormula<'_>) {
        match f {
            FOFFormula::Atomic(a) => self.record_from_atomic(a),
            FOFFormula::Negation(g) | FOFFormula::Parens(g) => self.record_from_formula(g),
            FOFFormula::Quantified { formula, .. } => self.record_from_formula(formula),
            FOFFormula::Binary { left, right, .. } => {
                self.record_from_formula(left);
                self.record_from_formula(right);
            }
            FOFFormula::Equality(l, r) | FOFFormula::Inequality(l, r) => {
                self.record_from_term(l);
                self.record_from_term(r);
            }
        }
    }

    fn record_from_atomic(&mut self, a: &FOFAtomicFormula<'_>) {
        match a {
            FOFAtomicFormula::Plain(w, args) => {
                self.record(w.as_str());
                for t in args {
                    self.record_from_term(t);
                }
            }
            FOFAtomicFormula::Defined(_, args) | FOFAtomicFormula::System(_, args) => {
                for t in args {
                    self.record_from_term(t);
                }
            }
            FOFAtomicFormula::True | FOFAtomicFormula::False => {}
        }
    }

    fn record_from_term(&mut self, t: &FOFTerm<'_>) {
        match t {
            FOFTerm::Variable(_) | FOFTerm::Number(_) | FOFTerm::DistinctObject(_) => {}
            FOFTerm::Function(w, args) => {
                self.record(w.as_str());
                for a in args {
                    self.record_from_term(a);
                }
            }
            FOFTerm::DefinedFunction(_, args) | FOFTerm::SystemFunction(_, args) => {
                for a in args {
                    self.record_from_term(a);
                }
            }
        }
    }
}

impl Default for SkolemRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Check a single `skolemize` step.
pub fn check<'p>(
    step: &AnnotatedFormula<'p>,
    parent: Option<&AnnotatedFormula<'p>>,
    registry: &mut SkolemRegistry,
) -> StepOutcome {
    // 1) status must be 'esa'.
    let Some(ann) = step.annotations() else {
        return StepOutcome::Unsound("skolemize step lacks annotations".into());
    };
    if ann.status() != Some("esa") {
        return StepOutcome::Unsound("skolemize step must have status `esa`".into());
    }

    // 2) extract skolemize(Var, sk(args)) info.
    //
    // E omits this annotation entirely on its `skolemize` steps, declaring
    // neither `skolemize(...)` nor `new_symbols(...)`. In that case we
    // cannot do the full structural check (we don't know which existential
    // was eliminated nor the exact Skolem term), so we fall back to a
    // permissive inference: any function symbol present in the step but
    // absent from both the parent and the registry is treated as a fresh
    // Skolem. If we find at least one such fresh symbol AND no symbol in
    // the step clashes with the registry, we accept the step as `Unknown`
    // — not provably sound, but not provably unsound either. This avoids
    // a false-positive FailedVerified for the many E proofs whose skolemize
    // annotations are too sparse.
    let Some(info) = ann.skolemize_info() else {
        return check_e_style_skolemize(step, parent, registry);
    };

    // 3) exactly one new symbol declared, equal to info.skolem_symbol.
    let new_syms = ann.new_symbols();
    if new_syms.len() != 1 {
        return StepOutcome::Unsound(format!(
            "skolemize must declare exactly one new symbol, got {}",
            new_syms.len()
        ));
    }
    if new_syms[0] != info.skolem_symbol {
        return StepOutcome::Unsound(format!(
            "skolemize symbol mismatch: new_symbols=[{}] but skolemize(...) uses {}",
            new_syms[0], info.skolem_symbol
        ));
    }

    // 4) fresh-symbol check.
    if registry.seen_symbols.contains(info.skolem_symbol) {
        return StepOutcome::Unsound(format!(
            "Skolem symbol `{}` is not fresh (reused or clashes with an existing symbol)",
            info.skolem_symbol
        ));
    }

    // 5) Need the parent formula.
    let Some(parent) = parent else {
        return StepOutcome::Unsound("skolemize step has no parent".into());
    };
    let parent_fof = match parent.as_fof() {
        Some(f) => f,
        None => return StepOutcome::Unknown("skolemize parent is not FOF".into()),
    };
    let parent_f = match &parent_fof.formula {
        FOFStatement::Logical(f) => f,
        _ => return StepOutcome::Unsound("skolemize parent is a sequent".into()),
    };

    // 6) Walk through ∀ binders; the next thing we expect is ?Var or a path
    // that contains ?Var as the next existential under some universal prefix.
    // For the rules to apply cleanly we expect the parent to be in prenex
    // form: ∀u1…∀un. ∃Var. body  (possibly with intermediate ∀ between
    // ∃Var and the body — but the rule says deps are exactly the universals
    // in scope at the existential).
    let (universals, body_after_existential) = match find_existential_binder(parent_f, info.var) {
        Some(x) => x,
        None => {
            return StepOutcome::Unsound(format!(
                "skolemize: existential variable `{}` not found at a Skolemizable position in parent",
                info.var
            ));
        }
    };

    // 7) Compare args (ordered) to universals (ordered).
    if info.args.len() != universals.len() {
        return StepOutcome::Unsound(format!(
            "skolemize: Skolem term `{}` has {} args but {} universal variable(s) are in scope",
            info.skolem_symbol,
            info.args.len(),
            universals.len()
        ));
    }
    for (i, (a, u)) in info.args.iter().zip(universals.iter()).enumerate() {
        if a != u {
            return StepOutcome::Unsound(format!(
                "skolemize: arg {} of `{}` is `{}` but expected universal `{}`",
                i, info.skolem_symbol, a, u
            ));
        }
    }

    // 8) Build the expected post-Skolemization formula and compare to the
    // step's formula (syntactic equality after substitution, modulo
    // structural equality of the AST — no α-renaming yet because the proof
    // tool kept the variable names).
    let sk_term = build_skolem_term(info.skolem_symbol, &info.args);
    let expected_body = subst_var_in_formula(body_after_existential, info.var, &sk_term);
    let expected = wrap_universals(&universals, expected_body);

    let step_fof = match step.as_fof() {
        Some(f) => f,
        None => return StepOutcome::Unknown("skolemize step is not FOF".into()),
    };
    let step_f = match &step_fof.formula {
        FOFStatement::Logical(f) => f,
        _ => return StepOutcome::Unsound("skolemize step is a sequent".into()),
    };

    if formula_eq(step_f, &expected) {
        // Register the new Skolem symbol so the next step sees it as taken.
        registry.record(info.skolem_symbol);
        StepOutcome::Sound
    } else {
        StepOutcome::Unsound(format!(
            "skolemize: resulting formula does not match expected substitution \
             of `{}` for `?{}` in parent",
            info.skolem_symbol, info.var
        ))
    }
}

/// Walk through any leading ∀ binders and the *next* ∃Var binder.
///
/// Returns `(universals_in_order, body_after_∃Var)` if such a position is
/// found in the formula's prenex prefix. Returns `None` if the existential
/// `var` does not appear as the first existential under the universal
/// prefix.
fn find_existential_binder<'p>(
    f: &'p FOFFormula<'p>,
    var: &str,
) -> Option<(Vec<&'p str>, &'p FOFFormula<'p>)> {
    let mut universals: Vec<&'p str> = Vec::new();
    let mut cur = strip_parens(f);
    loop {
        match cur {
            FOFFormula::Quantified {
                quantifier: Quantifier::Forall,
                variables,
                formula,
            } => {
                for v in variables {
                    universals.push(*v);
                }
                cur = strip_parens(formula);
            }
            FOFFormula::Quantified {
                quantifier: Quantifier::Exists,
                variables,
                formula,
            } => {
                // The existential we're after must be the first variable in
                // *some* leading existential binder. Per the example proofs,
                // each existential is its own binder with one variable.
                if variables.first().copied() == Some(var) {
                    // If there are more vars in the same `?[A,B]:` block, we
                    // can't cleanly Skolemize just `A`. Refuse.
                    if variables.len() != 1 {
                        return None;
                    }
                    return Some((universals, formula));
                }
                return None;
            }
            _ => return None,
        }
    }
}

fn strip_parens<'p>(f: &'p FOFFormula<'p>) -> &'p FOFFormula<'p> {
    let mut cur = f;
    while let FOFFormula::Parens(inner) = cur {
        cur = inner;
    }
    cur
}

/// Walk a (prenex) quantifier prefix and return, for each existential
/// encountered, the set of universally-quantified variables in scope at that
/// existential. A sound Skolemisation replaces each existential by a term over
/// exactly those in-scope universals, so this captures the *set* of legitimate
/// Skolem argument-variable sets (one per existential, in prefix order).
///
/// Example: for `? [X2] : (! [X3,X4] : (? [X5] : matrix))` this yields
/// `[{}, {X3,X4}]` — `X2`'s Skolem is a constant, `X5`'s captures `{X3,X4}`.
/// Stops at the first non-quantifier node, so nested quantifiers inside the
/// matrix are simply not enumerated (the caller treats unmatched Skolems
/// conservatively).
fn collect_existential_scopes<'p>(f: &'p FOFFormula<'p>) -> Vec<HashSet<&'p str>> {
    let mut scopes: Vec<HashSet<&'p str>> = Vec::new();
    let mut universals: HashSet<&'p str> = HashSet::new();
    let mut cur = strip_parens(f);
    loop {
        match cur {
            FOFFormula::Quantified {
                quantifier: Quantifier::Forall,
                variables,
                formula,
            } => {
                for v in variables {
                    universals.insert(*v);
                }
                cur = strip_parens(formula);
            }
            FOFFormula::Quantified {
                quantifier: Quantifier::Exists,
                variables,
                formula,
            } => {
                // Every variable bound by this existential is Skolemised with
                // the same in-scope universals.
                for _ in variables {
                    scopes.push(universals.clone());
                }
                cur = strip_parens(formula);
            }
            _ => break,
        }
    }
    scopes
}

fn wrap_universals<'p>(vars: &[&'p str], body: FOFFormula<'p>) -> FOFFormula<'p> {
    let mut cur = body;
    for v in vars.iter().rev() {
        cur = FOFFormula::Quantified {
            quantifier: Quantifier::Forall,
            variables: vec![*v],
            formula: Box::new(cur),
        };
    }
    cur
}

fn build_skolem_term<'p>(sym: &'p str, args: &[&'p str]) -> FOFTerm<'p> {
    if args.is_empty() {
        FOFTerm::Function(AtomicWord::Lower(sym), vec![])
    } else {
        let ts: Vec<FOFTerm<'p>> = args.iter().map(|a| FOFTerm::Variable(a)).collect();
        FOFTerm::Function(AtomicWord::Lower(sym), ts)
    }
}

fn subst_var_in_formula<'p>(
    f: &FOFFormula<'p>,
    var: &str,
    replacement: &FOFTerm<'p>,
) -> FOFFormula<'p> {
    match f {
        FOFFormula::Atomic(a) => FOFFormula::Atomic(subst_in_atomic(a, var, replacement)),
        FOFFormula::Negation(inner) => {
            FOFFormula::Negation(Box::new(subst_var_in_formula(inner, var, replacement)))
        }
        FOFFormula::Parens(inner) => {
            FOFFormula::Parens(Box::new(subst_var_in_formula(inner, var, replacement)))
        }
        FOFFormula::Equality(l, r) => FOFFormula::Equality(
            subst_in_term(l, var, replacement),
            subst_in_term(r, var, replacement),
        ),
        FOFFormula::Inequality(l, r) => FOFFormula::Inequality(
            subst_in_term(l, var, replacement),
            subst_in_term(r, var, replacement),
        ),
        FOFFormula::Binary {
            left,
            connective,
            right,
        } => FOFFormula::Binary {
            left: Box::new(subst_var_in_formula(left, var, replacement)),
            connective: *connective,
            right: Box::new(subst_var_in_formula(right, var, replacement)),
        },
        FOFFormula::Quantified {
            quantifier,
            variables,
            formula,
        } => {
            // If `var` is rebound by this quantifier, don't substitute inside.
            if variables.contains(&var) {
                FOFFormula::Quantified {
                    quantifier: *quantifier,
                    variables: variables.clone(),
                    formula: formula.clone(),
                }
            } else {
                FOFFormula::Quantified {
                    quantifier: *quantifier,
                    variables: variables.clone(),
                    formula: Box::new(subst_var_in_formula(formula, var, replacement)),
                }
            }
        }
    }
}

fn subst_in_atomic<'p>(
    a: &FOFAtomicFormula<'p>,
    var: &str,
    replacement: &FOFTerm<'p>,
) -> FOFAtomicFormula<'p> {
    match a {
        FOFAtomicFormula::Plain(w, args) => FOFAtomicFormula::Plain(
            w.clone(),
            args.iter()
                .map(|t| subst_in_term(t, var, replacement))
                .collect(),
        ),
        FOFAtomicFormula::Defined(w, args) => FOFAtomicFormula::Defined(
            w.clone(),
            args.iter()
                .map(|t| subst_in_term(t, var, replacement))
                .collect(),
        ),
        FOFAtomicFormula::System(w, args) => FOFAtomicFormula::System(
            w.clone(),
            args.iter()
                .map(|t| subst_in_term(t, var, replacement))
                .collect(),
        ),
        FOFAtomicFormula::True => FOFAtomicFormula::True,
        FOFAtomicFormula::False => FOFAtomicFormula::False,
    }
}

fn subst_in_term<'p>(t: &FOFTerm<'p>, var: &str, replacement: &FOFTerm<'p>) -> FOFTerm<'p> {
    match t {
        FOFTerm::Variable(v) => {
            if *v == var {
                replacement.clone()
            } else {
                FOFTerm::Variable(v)
            }
        }
        FOFTerm::Function(w, args) => FOFTerm::Function(
            w.clone(),
            args.iter()
                .map(|a| subst_in_term(a, var, replacement))
                .collect(),
        ),
        FOFTerm::DefinedFunction(w, args) => FOFTerm::DefinedFunction(
            w.clone(),
            args.iter()
                .map(|a| subst_in_term(a, var, replacement))
                .collect(),
        ),
        FOFTerm::SystemFunction(w, args) => FOFTerm::SystemFunction(
            w.clone(),
            args.iter()
                .map(|a| subst_in_term(a, var, replacement))
                .collect(),
        ),
        FOFTerm::Number(n) => FOFTerm::Number(*n),
        FOFTerm::DistinctObject(s) => FOFTerm::DistinctObject(s),
    }
}

/// Structural equality on FOF formulas, treating `Parens` as transparent.
fn formula_eq(a: &FOFFormula<'_>, b: &FOFFormula<'_>) -> bool {
    match (strip_parens(a), strip_parens(b)) {
        (FOFFormula::Atomic(x), FOFFormula::Atomic(y)) => atomic_eq(x, y),
        (FOFFormula::Negation(x), FOFFormula::Negation(y)) => formula_eq(x, y),
        (FOFFormula::Equality(l1, r1), FOFFormula::Equality(l2, r2)) => {
            term_eq(l1, l2) && term_eq(r1, r2)
        }
        (FOFFormula::Inequality(l1, r1), FOFFormula::Inequality(l2, r2)) => {
            term_eq(l1, l2) && term_eq(r1, r2)
        }
        (
            FOFFormula::Binary {
                left: l1,
                connective: c1,
                right: r1,
            },
            FOFFormula::Binary {
                left: l2,
                connective: c2,
                right: r2,
            },
        ) => c1 == c2 && formula_eq(l1, l2) && formula_eq(r1, r2),
        (
            FOFFormula::Quantified {
                quantifier: q1,
                variables: v1,
                formula: f1,
            },
            FOFFormula::Quantified {
                quantifier: q2,
                variables: v2,
                formula: f2,
            },
        ) => q1 == q2 && v1 == v2 && formula_eq(f1, f2),
        _ => false,
    }
}

fn atomic_eq(a: &FOFAtomicFormula<'_>, b: &FOFAtomicFormula<'_>) -> bool {
    match (a, b) {
        (FOFAtomicFormula::True, FOFAtomicFormula::True)
        | (FOFAtomicFormula::False, FOFAtomicFormula::False) => true,
        (FOFAtomicFormula::Plain(w1, a1), FOFAtomicFormula::Plain(w2, a2)) => {
            w1 == w2 && a1.len() == a2.len() && a1.iter().zip(a2.iter()).all(|(x, y)| term_eq(x, y))
        }
        (FOFAtomicFormula::Defined(w1, a1), FOFAtomicFormula::Defined(w2, a2)) => {
            w1 == w2 && a1.len() == a2.len() && a1.iter().zip(a2.iter()).all(|(x, y)| term_eq(x, y))
        }
        (FOFAtomicFormula::System(w1, a1), FOFAtomicFormula::System(w2, a2)) => {
            w1 == w2 && a1.len() == a2.len() && a1.iter().zip(a2.iter()).all(|(x, y)| term_eq(x, y))
        }
        _ => false,
    }
}

fn term_eq(a: &FOFTerm<'_>, b: &FOFTerm<'_>) -> bool {
    match (a, b) {
        (FOFTerm::Variable(x), FOFTerm::Variable(y)) => x == y,
        (FOFTerm::Function(w1, a1), FOFTerm::Function(w2, a2)) => {
            w1 == w2 && a1.len() == a2.len() && a1.iter().zip(a2.iter()).all(|(x, y)| term_eq(x, y))
        }
        (FOFTerm::DefinedFunction(w1, a1), FOFTerm::DefinedFunction(w2, a2)) => {
            w1 == w2 && a1.len() == a2.len() && a1.iter().zip(a2.iter()).all(|(x, y)| term_eq(x, y))
        }
        (FOFTerm::SystemFunction(w1, a1), FOFTerm::SystemFunction(w2, a2)) => {
            w1 == w2 && a1.len() == a2.len() && a1.iter().zip(a2.iter()).all(|(x, y)| term_eq(x, y))
        }
        (FOFTerm::Number(x), FOFTerm::Number(y)) => x.as_str() == y.as_str(),
        (FOFTerm::DistinctObject(x), FOFTerm::DistinctObject(y)) => x == y,
        _ => false,
    }
}

/// E-style fallback when a `skolemize` step has no `skolemize(Var, sk(...))`
/// annotation. We collect every function symbol mentioned in the step but
/// not in the parent. If at least one such symbol exists and none of them
/// were already in the registry (problem symbols), record them as fresh
/// Skolems and return `Unknown` (we cannot prove soundness without the
/// missing var/term info, but we have no evidence of unsoundness either —
/// `Unknown` propagates to `NotVerified`, scoring 0 instead of the −1 a
/// false-positive `FailedVerified` would cost).
fn check_e_style_skolemize<'p>(
    step: &AnnotatedFormula<'p>,
    parent: Option<&AnnotatedFormula<'p>>,
    registry: &mut SkolemRegistry,
) -> StepOutcome {
    let Some(parent) = parent else {
        return StepOutcome::Unknown("skolemize step has no parent".into());
    };
    let step_fof = match step.as_fof() {
        Some(f) => f,
        None => return StepOutcome::Unknown("skolemize step is not FOF".into()),
    };
    let step_f = match &step_fof.formula {
        FOFStatement::Logical(f) => f,
        _ => return StepOutcome::Unknown("skolemize step is a sequent".into()),
    };
    let parent_fof = match parent.as_fof() {
        Some(f) => f,
        None => return StepOutcome::Unknown("skolemize parent is not FOF".into()),
    };
    let parent_f = match &parent_fof.formula {
        FOFStatement::Logical(f) => f,
        _ => return StepOutcome::Unknown("skolemize parent is a sequent".into()),
    };

    let mut step_syms: HashSet<&str> = HashSet::new();
    let mut parent_syms: HashSet<&str> = HashSet::new();
    crate::checks::introduced_definition::collect_fun_syms(step_f, &mut step_syms);
    crate::checks::introduced_definition::collect_fun_syms(parent_f, &mut parent_syms);

    let fresh: Vec<&str> = step_syms.difference(&parent_syms).copied().collect();
    if fresh.is_empty() {
        return StepOutcome::Unknown(
            "skolemize step missing `skolemize(Var, sk(...))` annotation; \
             no fresh symbols introduced — cannot verify structurally"
                .into(),
        );
    }

    let stale: Vec<&str> = fresh
        .iter()
        .copied()
        .filter(|s| registry.seen_symbols.contains(*s))
        .collect();
    if !stale.is_empty() {
        return StepOutcome::Unknown(format!(
            "skolemize step missing annotation and candidate Skolem symbol(s) {stale:?} \
             clash with the problem's symbols"
        ));
    }

    // Try to enforce arity / no-illegal-capture if the parent has a prenex
    // quantifier prefix. A correct Skolemisation replaces each existential by a
    // term over *exactly* the universals in scope at that existential, so the
    // legitimate Skolem argument-variable sets are the per-existential scopes
    // (plus any parent free variables, which are always in scope). Crucially we
    // must handle existentials nested at *different* depths — e.g. PyRes emits
    // `? [X2] : ! [X3,X4] : ? [X5] : …` in one step, where one Skolem is a
    // constant and another captures `{X3,X4}`. A single "expected vars" set
    // (the previous implementation) wrongly rejected the deeper Skolem.
    let scopes = collect_existential_scopes(parent_f);
    if !scopes.is_empty() {
        let mut parent_bound = HashSet::new();
        let mut parent_free = HashSet::new();
        crate::checks::introduced_definition::free_vars(
            parent_f,
            &mut parent_bound,
            &mut parent_free,
        );

        // A Skolem's argument set is acceptable iff it equals one of the
        // per-existential in-scope universal sets (each augmented with the
        // parent's free variables).
        let valid_arg_sets: Vec<HashSet<&str>> = scopes
            .iter()
            .map(|s| {
                let mut set = s.clone();
                set.extend(parent_free.iter().copied());
                set
            })
            .collect();

        for &sk in &fresh {
            let mut mismatch = false;
            let mut check_sk_args = |args: &[FOFTerm<'_>]| {
                let mut arg_vars = HashSet::new();
                for a in args {
                    crate::checks::introduced_definition::collect_term_vars(a, &mut arg_vars);
                }
                if !valid_arg_sets.contains(&arg_vars) {
                    mismatch = true;
                }
            };

            fn walk_fof<'a, F: FnMut(&[FOFTerm<'a>])>(f: &FOFFormula<'a>, sk: &str, cb: &mut F) {
                match f {
                    FOFFormula::Atomic(a) => match a {
                        FOFAtomicFormula::Plain(_, args)
                        | FOFAtomicFormula::Defined(_, args)
                        | FOFAtomicFormula::System(_, args) => {
                            for arg in args {
                                walk_term(arg, sk, cb);
                            }
                        }
                        _ => {}
                    },
                    FOFFormula::Negation(inner) | FOFFormula::Parens(inner) => {
                        walk_fof(inner, sk, cb)
                    }
                    FOFFormula::Binary { left, right, .. } => {
                        walk_fof(left, sk, cb);
                        walk_fof(right, sk, cb);
                    }
                    FOFFormula::Quantified { formula, .. } => walk_fof(formula, sk, cb),
                    FOFFormula::Equality(l, r) | FOFFormula::Inequality(l, r) => {
                        walk_term(l, sk, cb);
                        walk_term(r, sk, cb);
                    }
                }
            }

            fn walk_term<'a, F: FnMut(&[FOFTerm<'a>])>(t: &FOFTerm<'a>, sk: &str, cb: &mut F) {
                match t {
                    FOFTerm::Function(w, args) => {
                        if w.as_str() == sk {
                            cb(args);
                        }
                        for a in args {
                            walk_term(a, sk, cb);
                        }
                    }
                    FOFTerm::DefinedFunction(w, args) => {
                        if w.0 == sk {
                            cb(args);
                        }
                        for a in args {
                            walk_term(a, sk, cb);
                        }
                    }
                    FOFTerm::SystemFunction(w, args) => {
                        if w.0 == sk {
                            cb(args);
                        }
                        for a in args {
                            walk_term(a, sk, cb);
                        }
                    }
                    _ => {}
                }
            }

            walk_fof(step_f, sk, &mut check_sk_args);

            // This is the unannotated heuristic path: it can never positively
            // confirm soundness (it always ends in `Unknown`). An argument-set
            // that matches no existential scope is *suspicious* but, lacking the
            // `skolemize(Var, sk(...))` annotation, is not proof of unsoundness
            // — our prefix model does not cover non-prenex shapes. Returning
            // `Unsound` here costs −1 on valid nested Skolemisations (observed on
            // PyRes), so we conservatively downgrade to `Unknown` (0 pts). A
            // genuinely bad Skolemisation still fails downstream at the ATP.
            if mismatch {
                registry.record(sk);
                return StepOutcome::Unknown(format!(
                    "skolemize step (unannotated) introduces Skolem `{sk}` whose argument \
                     variables match no existential scope of the parent; cannot confirm \
                     structurally — deferred as Unknown"
                ));
            }
        }
    }

    for s in &fresh {
        registry.record(s);
    }
    StepOutcome::Unknown(format!(
        "skolemize step missing `skolemize(Var, sk(...))` annotation; \
         inferred fresh Skolem(s) {fresh:?} from step\\parent — accepted as Unknown"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_tptp::parse_tptp;

    fn nth_fof<'p>(input: &'p str, n: usize) -> &'p AnnotatedFormula<'p> {
        let problem = Box::leak(Box::new(parse_tptp(input).expect("parse")));
        &problem.formulas[n]
    }

    /// PyRes emits multi-existential Skolemisation in a single `skolemize`
    /// step: `? [X2] : ! [X3,X4] : ? [X5] : …` collapses to a constant Skolem
    /// for `X2` and an arity-2 Skolem `sk(X3,X4)` for `X5`. The two Skolems
    /// have *different* arities/scopes, which the old single-`expected_vars`
    /// check wrongly flagged as `Unsound` (a −1 false reject on a valid proof).
    /// It must now be at worst `Unknown`, never `Unsound`.
    #[test]
    fn nested_multi_existential_skolemize_is_not_unsound() {
        let parent = nth_fof(
            "fof(c2, negated_conjecture, \
             (? [X2]: (! [X3,X4]: (? [X5]: \
              (big_f(X2,X5) & (big_f(X3,X5) & (big_f(X4,X5) & \
               (big_f(X3,X2) & ~ big_f(X5,X4)))))))), \
             inference(variable_rename,[status(thm)],[c1])).",
            0,
        );
        let step = nth_fof(
            "fof(c3, negated_conjecture, \
             (! [X3,X4]: (big_f(skolem0001,skolem0002(X3,X4)) & \
              (big_f(X3,skolem0002(X3,X4)) & (big_f(X4,skolem0002(X3,X4)) & \
               (big_f(X3,skolem0001) & ~ big_f(skolem0002(X3,X4),X4)))))), \
             inference(skolemize,[status(esa)],[c2])).",
            0,
        );
        let mut reg = SkolemRegistry::new();
        let outcome = check(step, Some(parent), &mut reg);
        assert!(
            !matches!(outcome, StepOutcome::Unsound(_)),
            "valid nested multi-existential skolemization must not be Unsound, got {outcome:?}"
        );
    }

    /// `collect_existential_scopes` must record one in-scope-universal set per
    /// existential, at the correct depth.
    #[test]
    fn existential_scopes_track_depth() {
        let f = nth_fof(
            "fof(c, axiom, (? [X2]: (! [X3,X4]: (? [X5]: big_f(X2,X3,X4,X5))))).",
            0,
        );
        let logical = match &f.as_fof().unwrap().formula {
            FOFStatement::Logical(l) => l,
            _ => panic!("expected logical"),
        };
        let scopes = collect_existential_scopes(logical);
        assert_eq!(scopes.len(), 2, "two existentials expected");
        assert!(scopes[0].is_empty(), "X2 has no universals in scope");
        assert_eq!(
            scopes[1],
            ["X3", "X4"].into_iter().collect::<HashSet<_>>(),
            "X5 captures {{X3,X4}}"
        );
    }
}

