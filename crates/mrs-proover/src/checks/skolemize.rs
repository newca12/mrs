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

use std::collections::{HashMap, HashSet};

use mrs_tptp::{
    AnnotatedFormula, AtomicWord, BinaryConnective, FOFAtomicFormula, FOFFormula, FOFStatement,
    FOFTerm, Quantifier,
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
///
/// `dag_status` is the DAG-computed status for this node (see
/// `dag::has_esa_in_term`), which already propagates `[status(esa)]` up
/// through nested E-prover inference chains like
/// `inference(fof_nnf,[status(thm)],[inference(skolemize,[status(esa)],[...])])`.
/// We deliberately reuse that single source of truth rather than
/// re-deriving it from `step`'s own (possibly outer, non-`esa`) annotation.
pub fn check<'p>(
    step: &AnnotatedFormula<'p>,
    parent: Option<&AnnotatedFormula<'p>>,
    registry: &mut SkolemRegistry,
    dag_status: Option<&str>,
) -> StepOutcome {
    // 1) status must be 'esa'.
    let Some(ann) = step.annotations() else {
        return StepOutcome::Unsound("skolemize step lacks annotations".into());
    };
    if dag_status != Some("esa") {
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
    // a false-positive VerifiedBad for the many E proofs whose skolemize
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

/// Collect every variable bound by a `!`/`?` quantifier anywhere in `f`,
/// partitioned into universals and existentials. After `variable_rename`
/// (which every prover applies before Skolemising) these names are globally
/// unique, so the partition is unambiguous.
fn collect_quantified_vars<'p>(
    f: &FOFFormula<'p>,
    univ: &mut HashSet<&'p str>,
    exist: &mut HashSet<&'p str>,
    in_scope: &mut Vec<&'p str>,
    exist_scope: &mut HashMap<&'p str, HashSet<&'p str>>,
) {
    match f {
        FOFFormula::Quantified {
            quantifier,
            variables,
            formula,
        } => {
            let is_forall = *quantifier == Quantifier::Forall;
            if is_forall {
                for v in variables {
                    univ.insert(*v);
                    in_scope.push(*v);
                }
            } else {
                for v in variables {
                    exist.insert(*v);
                    exist_scope.insert(*v, in_scope.iter().copied().collect());
                }
            }
            collect_quantified_vars(formula, univ, exist, in_scope, exist_scope);
            if is_forall {
                for _ in variables {
                    in_scope.pop();
                }
            }
        }
        FOFFormula::Negation(g) | FOFFormula::Parens(g) => {
            collect_quantified_vars(g, univ, exist, in_scope, exist_scope)
        }
        FOFFormula::Binary { left, right, .. } => {
            collect_quantified_vars(left, univ, exist, in_scope, exist_scope);
            collect_quantified_vars(right, univ, exist, in_scope, exist_scope);
        }
        FOFFormula::Atomic(_) | FOFFormula::Equality(..) | FOFFormula::Inequality(..) => {}
    }
}

/// Accumulated bindings discovered while matching a parent against its
/// Skolemised conclusion.
#[derive(Default, Clone)]
struct SkolemMatch<'p> {
    /// Parent universal variable → conclusion variable (a consistent renaming).
    uni_map: HashMap<&'p str, &'p str>,
    /// Existential variable → universals in scope at it (parent names).
    exist_scope: HashMap<&'p str, HashSet<&'p str>>,
    /// Existential variable → the (Skolem) term that replaced it.
    exist_term: HashMap<&'p str, FOFTerm<'p>>,
}

fn strip_parens_f<'a, 'p>(f: &'a FOFFormula<'p>) -> &'a FOFFormula<'p> {
    let mut cur = f;
    while let FOFFormula::Parens(inner) = cur {
        cur = inner;
    }
    cur
}

fn strip_quantifiers_f<'a, 'p>(f: &'a FOFFormula<'p>) -> &'a FOFFormula<'p> {
    let mut cur = f;
    loop {
        cur = strip_parens_f(cur);
        if let FOFFormula::Quantified { formula, .. } = cur {
            cur = formula;
        } else {
            break;
        }
    }
    cur
}

fn flatten_associative<'a, 'p>(
    f: &'a FOFFormula<'p>,
    conn: BinaryConnective,
) -> Vec<&'a FOFFormula<'p>> {
    let f = strip_quantifiers_f(f);
    let mut out = Vec::new();
    let mut stack = vec![f];
    while let Some(current) = stack.pop() {
        let current = strip_quantifiers_f(current);
        if let FOFFormula::Binary {
            left,
            connective,
            right,
        } = current
            && *connective == conn
        {
            stack.push(right);
            stack.push(left);
            continue;
        }
        out.push(current);
    }
    out
}

/// Match every element of `concs` against a *distinct* element of `pats`,
/// recording bindings in `m`.
///
/// This is a **bijective** multiset match: every pattern conjunct/disjunct
/// must be consumed too (checked by the caller via `pats.iter().all(|p|
/// p.is_none())` once this returns `true`, since `concs.len() ==
/// pats.len()` is enforced by every call site below). Do NOT relax this to
/// a subset match — a skolemize/esa step that structurally drops a parent
/// conjunct (or disjunct) without any Skolem witness for it is not a valid
/// Skolemisation and must not be accepted as `Sound` (see the regression
/// test `skolemize_subset_match_does_not_drop_conjuncts`).
fn match_multiset<'p>(
    pats: &mut [Option<&FOFFormula<'p>>],
    concs: &[&FOFFormula<'p>],
    univ: &HashSet<&str>,
    exist: &HashSet<&str>,
    m: &mut SkolemMatch<'p>,
) -> bool {
    if concs.is_empty() {
        return true;
    }
    let current_conc = concs[0];
    for i in 0..pats.len() {
        if let Some(current_pat) = pats[i] {
            let mut m_tentative = m.clone();
            if match_skolem_formula(current_pat, current_conc, univ, exist, &mut m_tentative) {
                pats[i] = None;
                if match_multiset(pats, &concs[1..], univ, exist, &mut m_tentative) {
                    *m = m_tentative;
                    return true;
                }
                pats[i] = Some(current_pat); // backtrack
            }
        }
    }
    false
}

/// Match a parent formula `pat` against its Skolemised conclusion `conc`.
///
/// Skolemisation removes every existential (replacing its variable by a Skolem
/// term over the universals in scope at it) and pulls the universals to the
/// front — possibly *regrouping* binders that were separated by the eliminated
/// existentials (e.g. `! X2 ? X3 ! X4 ? X5 . φ` becomes `! [X2,X4] . φ'`). We
/// therefore strip quantifiers from each side *independently*:
///   * stripping a parent `!`-binder records its variables as in-scope and
///     recurses (the conclusion binder is consumed separately);
///   * stripping a parent `?`-binder records each variable's in-scope universals
///     and recurses, leaving `conc` untouched (it has no matching binder);
///   * stripping a conclusion `!`-binder just recurses; a residual conclusion
///     `?`-binder means the step did not actually Skolemise — reject.
///
/// Variable occurrences then bind via [`match_skolem_term`]: universal pattern
/// variables to conclusion variables (a consistent renaming), existential ones
/// to their Skolem terms. `univ`/`exist` are the parent's quantified-variable
/// name sets.
fn match_skolem_formula<'p>(
    pat: &FOFFormula<'p>,
    conc: &FOFFormula<'p>,
    univ: &HashSet<&str>,
    exist: &HashSet<&str>,
    m: &mut SkolemMatch<'p>,
) -> bool {
    let pat = strip_quantifiers_f(pat);
    let conc = strip_quantifiers_f(conc);

    let res = match (pat, conc) {
        (FOFFormula::Atomic(a), FOFFormula::Atomic(b)) => match_skolem_atomic(a, b, univ, exist, m),
        (FOFFormula::Negation(a), FOFFormula::Negation(b)) => {
            match_skolem_formula(a, b, univ, exist, m)
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
        ) => {
            if c1 == c2 && c1.is_associative() {
                let pats = flatten_associative(pat, *c1);
                let concs = flatten_associative(conc, *c1);
                // Bijective multiset match: the flattened parent and step must
                // have the same number of conjuncts/disjuncts, and every one
                // of them must be paired off. A skolemize/esa step that drops
                // (or adds) a conjunct/disjunct relative to its parent is not a
                // valid Skolemisation, regardless of connective.
                let mut pats_opts: Vec<Option<&FOFFormula<'p>>> =
                    pats.iter().copied().map(Some).collect();
                pats.len() == concs.len() && match_multiset(&mut pats_opts, &concs, univ, exist, m)
            } else {
                c1 == c2
                    && match_skolem_formula(l1, l2, univ, exist, m)
                    && match_skolem_formula(r1, r2, univ, exist, m)
            }
        }
        (FOFFormula::Equality(a, b), FOFFormula::Equality(c, d))
        | (FOFFormula::Inequality(a, b), FOFFormula::Inequality(c, d)) => {
            match_skolem_term(a, c, univ, exist, m) && match_skolem_term(b, d, univ, exist, m)
        }
        _ => false,
    };
    if !res && std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
        eprintln!(
            "[skolem-dbg] match_skolem_formula failed for pat={:?}, conc={:?}",
            pat, conc
        );
    }
    res
}

fn match_skolem_atomic<'p>(
    pat: &FOFAtomicFormula<'p>,
    conc: &FOFAtomicFormula<'p>,
    univ: &HashSet<&str>,
    exist: &HashSet<&str>,
    m: &mut SkolemMatch<'p>,
) -> bool {
    use FOFAtomicFormula::*;
    match (pat, conc) {
        (True, True) | (False, False) => true,
        (Plain(w1, a1), Plain(w2, a2)) => {
            w1 == w2 && match_skolem_term_list(a1, a2, univ, exist, m)
        }
        (Defined(w1, a1), Defined(w2, a2)) => {
            w1 == w2 && match_skolem_term_list(a1, a2, univ, exist, m)
        }
        (System(w1, a1), System(w2, a2)) => {
            w1 == w2 && match_skolem_term_list(a1, a2, univ, exist, m)
        }
        _ => false,
    }
}

fn match_skolem_term_list<'p>(
    pat: &[FOFTerm<'p>],
    conc: &[FOFTerm<'p>],
    univ: &HashSet<&str>,
    exist: &HashSet<&str>,
    m: &mut SkolemMatch<'p>,
) -> bool {
    pat.len() == conc.len()
        && pat
            .iter()
            .zip(conc)
            .all(|(a, b)| match_skolem_term(a, b, univ, exist, m))
}

fn match_skolem_term<'p>(
    pat: &FOFTerm<'p>,
    conc: &FOFTerm<'p>,
    univ: &HashSet<&str>,
    exist: &HashSet<&str>,
    m: &mut SkolemMatch<'p>,
) -> bool {
    match pat {
        // Existential variable: binds to the (Skolem) term that replaced it,
        // consistently across all occurrences.
        FOFTerm::Variable(v) if exist.contains(v) => match m.exist_term.get(v) {
            Some(prev) => term_eq(prev, conc),
            None => {
                m.exist_term.insert(v, conc.clone());
                true
            }
        },
        // Universal variable: must map to a conclusion *variable*, consistently.
        FOFTerm::Variable(v) if univ.contains(v) => match conc {
            FOFTerm::Variable(w) => match m.uni_map.get(v) {
                Some(prev) => prev == w,
                None => {
                    m.uni_map.insert(v, w);
                    true
                }
            },
            _ => false,
        },
        // Any other variable (e.g. inner-bound): must be identical.
        FOFTerm::Variable(v) => matches!(conc, FOFTerm::Variable(w) if w == v),
        FOFTerm::Function(w, args) => match conc {
            FOFTerm::Function(w2, a2) => {
                w == w2 && match_skolem_term_list(args, a2, univ, exist, m)
            }
            _ => false,
        },
        FOFTerm::DefinedFunction(w, args) => match conc {
            FOFTerm::DefinedFunction(w2, a2) => {
                w == w2 && match_skolem_term_list(args, a2, univ, exist, m)
            }
            _ => false,
        },
        FOFTerm::SystemFunction(w, args) => match conc {
            FOFTerm::SystemFunction(w2, a2) => {
                w == w2 && match_skolem_term_list(args, a2, univ, exist, m)
            }
            _ => false,
        },
        FOFTerm::Number(_) | FOFTerm::DistinctObject(_) => term_eq(pat, conc),
    }
}

fn fof_to_nnf<'p>(f: &FOFFormula<'p>) -> FOFFormula<'p> {
    match f {
        FOFFormula::Atomic(_) | FOFFormula::Equality(..) | FOFFormula::Inequality(..) => f.clone(),
        FOFFormula::Parens(inner) => fof_to_nnf(inner),
        FOFFormula::Negation(inner) => match strip_parens_f(inner) {
            FOFFormula::Atomic(_) | FOFFormula::Equality(..) | FOFFormula::Inequality(..) => {
                f.clone()
            }
            FOFFormula::Parens(x) => fof_to_nnf(&FOFFormula::Negation(x.clone())),
            FOFFormula::Negation(x) => fof_to_nnf(x),
            FOFFormula::Binary {
                left,
                connective,
                right,
            } => match connective {
                BinaryConnective::And => FOFFormula::Binary {
                    left: Box::new(fof_to_nnf(&FOFFormula::Negation(left.clone()))),
                    connective: BinaryConnective::Or,
                    right: Box::new(fof_to_nnf(&FOFFormula::Negation(right.clone()))),
                },
                BinaryConnective::Or => FOFFormula::Binary {
                    left: Box::new(fof_to_nnf(&FOFFormula::Negation(left.clone()))),
                    connective: BinaryConnective::And,
                    right: Box::new(fof_to_nnf(&FOFFormula::Negation(right.clone()))),
                },
                BinaryConnective::Impl => FOFFormula::Binary {
                    left: Box::new(fof_to_nnf(left)),
                    connective: BinaryConnective::And,
                    right: Box::new(fof_to_nnf(&FOFFormula::Negation(right.clone()))),
                },
                BinaryConnective::Iff => {
                    let left_pos = fof_to_nnf(left);
                    let left_neg = fof_to_nnf(&FOFFormula::Negation(left.clone()));
                    let right_pos = fof_to_nnf(right);
                    let right_neg = fof_to_nnf(&FOFFormula::Negation(right.clone()));
                    FOFFormula::Binary {
                        left: Box::new(FOFFormula::Binary {
                            left: Box::new(left_pos),
                            connective: BinaryConnective::And,
                            right: Box::new(right_neg),
                        }),
                        connective: BinaryConnective::Or,
                        right: Box::new(FOFFormula::Binary {
                            left: Box::new(left_neg),
                            connective: BinaryConnective::And,
                            right: Box::new(right_pos),
                        }),
                    }
                }
                _ => f.clone(),
            },
            FOFFormula::Quantified {
                quantifier,
                variables,
                formula,
            } => {
                let dual_q = match quantifier {
                    Quantifier::Forall => Quantifier::Exists,
                    Quantifier::Exists => Quantifier::Forall,
                };
                FOFFormula::Quantified {
                    quantifier: dual_q,
                    variables: variables.clone(),
                    formula: Box::new(fof_to_nnf(&FOFFormula::Negation(formula.clone()))),
                }
            }
        },
        FOFFormula::Binary {
            left,
            connective,
            right,
        } => match connective {
            BinaryConnective::Impl => FOFFormula::Binary {
                left: Box::new(fof_to_nnf(&FOFFormula::Negation(left.clone()))),
                connective: BinaryConnective::Or,
                right: Box::new(fof_to_nnf(right)),
            },
            BinaryConnective::Iff => {
                let left_pos = fof_to_nnf(left);
                let left_neg = fof_to_nnf(&FOFFormula::Negation(left.clone()));
                let right_pos = fof_to_nnf(right);
                let right_neg = fof_to_nnf(&FOFFormula::Negation(right.clone()));
                FOFFormula::Binary {
                    left: Box::new(FOFFormula::Binary {
                        left: Box::new(left_neg),
                        connective: BinaryConnective::Or,
                        right: Box::new(right_pos),
                    }),
                    connective: BinaryConnective::And,
                    right: Box::new(FOFFormula::Binary {
                        left: Box::new(left_pos),
                        connective: BinaryConnective::Or,
                        right: Box::new(right_neg),
                    }),
                }
            }
            _ => FOFFormula::Binary {
                left: Box::new(fof_to_nnf(left)),
                connective: *connective,
                right: Box::new(fof_to_nnf(right)),
            },
        },
        FOFFormula::Quantified {
            quantifier,
            variables,
            formula,
        } => FOFFormula::Quantified {
            quantifier: *quantifier,
            variables: variables.clone(),
            formula: Box::new(fof_to_nnf(formula)),
        },
    }
}

fn rename_bound_variables<'p>(
    f: &FOFFormula<'p>,
    counter: &mut usize,
    subst: &mut HashMap<&'p str, &'p str>,
) -> FOFFormula<'p> {
    match f {
        FOFFormula::Atomic(a) => FOFFormula::Atomic(rename_in_atomic(a, subst)),
        FOFFormula::Negation(inner) => {
            FOFFormula::Negation(Box::new(rename_bound_variables(inner, counter, subst)))
        }
        FOFFormula::Parens(inner) => {
            FOFFormula::Parens(Box::new(rename_bound_variables(inner, counter, subst)))
        }
        FOFFormula::Equality(l, r) => {
            FOFFormula::Equality(rename_in_term(l, subst), rename_in_term(r, subst))
        }
        FOFFormula::Inequality(l, r) => {
            FOFFormula::Inequality(rename_in_term(l, subst), rename_in_term(r, subst))
        }
        FOFFormula::Binary {
            left,
            connective,
            right,
        } => FOFFormula::Binary {
            left: Box::new(rename_bound_variables(left, counter, subst)),
            connective: *connective,
            right: Box::new(rename_bound_variables(right, counter, subst)),
        },
        FOFFormula::Quantified {
            quantifier,
            variables,
            formula,
        } => {
            let mut new_subst = subst.clone();
            let mut new_vars = Vec::with_capacity(variables.len());
            for &v in variables {
                *counter += 1;
                let new_name: &'static str =
                    Box::leak(format!("skv_{}_{}", v, counter).into_boxed_str());
                new_subst.insert(v, new_name);
                new_vars.push(new_name);
            }
            FOFFormula::Quantified {
                quantifier: *quantifier,
                variables: new_vars,
                formula: Box::new(rename_bound_variables(formula, counter, &mut new_subst)),
            }
        }
    }
}

fn rename_in_atomic<'p>(
    a: &FOFAtomicFormula<'p>,
    subst: &HashMap<&'p str, &'p str>,
) -> FOFAtomicFormula<'p> {
    match a {
        FOFAtomicFormula::Plain(w, args) => FOFAtomicFormula::Plain(
            w.clone(),
            args.iter().map(|t| rename_in_term(t, subst)).collect(),
        ),
        FOFAtomicFormula::Defined(w, args) => FOFAtomicFormula::Defined(
            w.clone(),
            args.iter().map(|t| rename_in_term(t, subst)).collect(),
        ),
        FOFAtomicFormula::System(w, args) => FOFAtomicFormula::System(
            w.clone(),
            args.iter().map(|t| rename_in_term(t, subst)).collect(),
        ),
        FOFAtomicFormula::True => FOFAtomicFormula::True,
        FOFAtomicFormula::False => FOFAtomicFormula::False,
    }
}

fn rename_in_term<'p>(t: &FOFTerm<'p>, subst: &HashMap<&'p str, &'p str>) -> FOFTerm<'p> {
    match t {
        FOFTerm::Variable(v) => {
            if let Some(&new_v) = subst.get(v) {
                FOFTerm::Variable(new_v)
            } else {
                FOFTerm::Variable(v)
            }
        }
        FOFTerm::Function(w, args) => FOFTerm::Function(
            w.clone(),
            args.iter().map(|a| rename_in_term(a, subst)).collect(),
        ),
        FOFTerm::DefinedFunction(w, args) => FOFTerm::DefinedFunction(
            w.clone(),
            args.iter().map(|a| rename_in_term(a, subst)).collect(),
        ),
        FOFTerm::SystemFunction(w, args) => FOFTerm::SystemFunction(
            w.clone(),
            args.iter().map(|a| rename_in_term(a, subst)).collect(),
        ),
        FOFTerm::Number(n) => FOFTerm::Number(*n),
        FOFTerm::DistinctObject(s) => FOFTerm::DistinctObject(s),
    }
}

fn strip_all_quantifiers<'p>(f: &FOFFormula<'p>) -> FOFFormula<'p> {
    match f {
        FOFFormula::Atomic(_) | FOFFormula::Equality(..) | FOFFormula::Inequality(..) => f.clone(),
        FOFFormula::Parens(inner) => strip_all_quantifiers(inner),
        FOFFormula::Negation(inner) => FOFFormula::Negation(Box::new(strip_all_quantifiers(inner))),
        FOFFormula::Binary {
            left,
            connective,
            right,
        } => FOFFormula::Binary {
            left: Box::new(strip_all_quantifiers(left)),
            connective: *connective,
            right: Box::new(strip_all_quantifiers(right)),
        },
        FOFFormula::Quantified { formula, .. } => strip_all_quantifiers(formula),
    }
}

fn distribute_or_over_and<'p>(f: &FOFFormula<'p>) -> FOFFormula<'p> {
    match f {
        FOFFormula::Binary {
            left,
            connective: BinaryConnective::Or,
            right,
        } => {
            let l = distribute_or_over_and(left);
            let r = distribute_or_over_and(right);
            match (l, r) {
                (
                    FOFFormula::Binary {
                        left: l1,
                        connective: BinaryConnective::And,
                        right: r1,
                    },
                    r_val,
                ) => FOFFormula::Binary {
                    left: Box::new(distribute_or_over_and(&FOFFormula::Binary {
                        left: l1,
                        connective: BinaryConnective::Or,
                        right: Box::new(r_val.clone()),
                    })),
                    connective: BinaryConnective::And,
                    right: Box::new(distribute_or_over_and(&FOFFormula::Binary {
                        left: r1,
                        connective: BinaryConnective::Or,
                        right: Box::new(r_val),
                    })),
                },
                (
                    l_val,
                    FOFFormula::Binary {
                        left: l2,
                        connective: BinaryConnective::And,
                        right: r2,
                    },
                ) => FOFFormula::Binary {
                    left: Box::new(distribute_or_over_and(&FOFFormula::Binary {
                        left: Box::new(l_val.clone()),
                        connective: BinaryConnective::Or,
                        right: l2,
                    })),
                    connective: BinaryConnective::And,
                    right: Box::new(distribute_or_over_and(&FOFFormula::Binary {
                        left: Box::new(l_val),
                        connective: BinaryConnective::Or,
                        right: r2,
                    })),
                },
                (l_val, r_val) => FOFFormula::Binary {
                    left: Box::new(l_val),
                    connective: BinaryConnective::Or,
                    right: Box::new(r_val),
                },
            }
        }
        FOFFormula::Binary {
            left,
            connective,
            right,
        } => FOFFormula::Binary {
            left: Box::new(distribute_or_over_and(left)),
            connective: *connective,
            right: Box::new(distribute_or_over_and(right)),
        },
        FOFFormula::Negation(inner) => {
            FOFFormula::Negation(Box::new(distribute_or_over_and(inner)))
        }
        FOFFormula::Parens(inner) => distribute_or_over_and(inner),
        _ => f.clone(),
    }
}

/// Positively verify an unannotated `skolemize` step: confirm the conclusion is
/// exactly the parent with every existential (at any depth) replaced by a
/// distinct fresh Skolem term over precisely the universals in scope at it.
/// Returns `true` only on a fully-confirmed sound Skolemisation; `false` means
/// "could not confirm" and the caller falls back to the conservative path.
fn try_positive_skolemize<'p>(
    parent_f: &'p FOFFormula<'p>,
    step_f: &'p FOFFormula<'p>,
    fresh: &[&str],
    registry: &SkolemRegistry,
) -> bool {
    let mut counter = 0;
    let mut subst = HashMap::new();
    let parent_nnf = fof_to_nnf(parent_f);
    let parent_renamed = rename_bound_variables(&parent_nnf, &mut counter, &mut subst);
    let parent_stripped = strip_all_quantifiers(&parent_renamed);
    let parent_cnf = distribute_or_over_and(&parent_stripped);

    let step_nnf = fof_to_nnf(step_f);
    let step_stripped = strip_all_quantifiers(&step_nnf);
    let step_cnf = distribute_or_over_and(&step_stripped);

    let mut univ_set: HashSet<&str> = HashSet::new();
    let mut exist_set: HashSet<&str> = HashSet::new();
    let mut m = SkolemMatch::default();
    let mut in_scope: Vec<&str> = Vec::new();
    collect_quantified_vars(
        &parent_renamed,
        &mut univ_set,
        &mut exist_set,
        &mut in_scope,
        &mut m.exist_scope,
    );
    if exist_set.is_empty() {
        return false; // nothing to Skolemise this way
    }
    // A name bound both universally and existentially would make the analysis
    // ambiguous; such proofs are not produced after `variable_rename`.
    if univ_set.intersection(&exist_set).next().is_some() {
        return false;
    }

    if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
        eprintln!("[skolem-dbg] parent_f = {:?}", parent_f);
        eprintln!("[skolem-dbg] parent_cnf = {:?}", parent_cnf);
        eprintln!("[skolem-dbg] step_f = {:?}", step_f);
        eprintln!("[skolem-dbg] step_cnf = {:?}", step_cnf);
        eprintln!("[skolem-dbg] univ_set = {:?}", univ_set);
        eprintln!("[skolem-dbg] exist_set = {:?}", exist_set);
    }

    if !match_skolem_formula(&parent_cnf, &step_cnf, &univ_set, &exist_set, &mut m) {
        if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
            eprintln!("[skolem-dbg] match failed");
        }
        return false;
    }

    // The universal renaming must be injective.
    let mut seen_conc: HashSet<&str> = HashSet::new();
    for w in m.uni_map.values() {
        if !seen_conc.insert(*w) {
            if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
                eprintln!("[skolem-dbg] universal renaming not injective for {}", w);
            }
            return false;
        }
    }

    let fresh_set: HashSet<&str> = fresh.iter().copied().collect();
    let mut used_syms: HashSet<&str> = HashSet::new();
    for e in &exist_set {
        // Every existential must have been witnessed and have a recorded scope.
        let (Some(term), Some(scope)) = (m.exist_term.get(e), m.exist_scope.get(e)) else {
            if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
                eprintln!(
                    "[skolem-dbg] existential {} not witnessed or has no scope. term={:?}, scope={:?}",
                    e,
                    m.exist_term.get(e),
                    m.exist_scope.get(e)
                );
            }
            return false;
        };
        let (sym, args) = match term {
            FOFTerm::Function(w, args) => (w.as_str(), args),
            _ => {
                if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
                    eprintln!(
                        "[skolem-dbg] witness for {} is not a function: {:?}",
                        e, term
                    );
                }
                return false; // a Skolem witness must be a function/constant term
            }
        };
        if !fresh_set.contains(sym) || registry.seen_symbols.contains(sym) {
            if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
                eprintln!(
                    "[skolem-dbg] witness symbol {} not fresh. fresh_set={:?}, seen={:?}",
                    sym, fresh_set, registry.seen_symbols
                );
            }
            return false; // symbol not fresh
        }
        if !used_syms.insert(sym) {
            if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
                eprintln!("[skolem-dbg] witness symbol {} reused", sym);
            }
            return false; // same Skolem symbol reused for two existentials
        }
        // The Skolem arguments must be exactly the (renamed) in-scope universals
        // — distinct plain variables, no more, no fewer.
        let mut arg_vars: Vec<&str> = Vec::with_capacity(args.len());
        for a in args {
            match a {
                FOFTerm::Variable(v) => arg_vars.push(v),
                _ => {
                    if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
                        eprintln!(
                            "[skolem-dbg] argument in witness is not a variable: {:?}",
                            a
                        );
                    }
                    return false;
                }
            }
        }
        let arg_set: HashSet<&str> = arg_vars.iter().copied().collect();
        if arg_set.len() != arg_vars.len() {
            if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
                eprintln!(
                    "[skolem-dbg] duplicate arguments in witness: {:?}",
                    arg_vars
                );
            }
            return false; // duplicate argument
        }
        let mut expected: HashSet<&str> = HashSet::new();
        for u in scope {
            match m.uni_map.get(u) {
                Some(w) => {
                    expected.insert(*w);
                }
                // An in-scope universal never used in the body has no discovered
                // name; we cannot confirm dependency precisely, so bail safely.
                None => {
                    if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
                        eprintln!(
                            "[skolem-dbg] in-scope universal {} not found in uni_map: {:?}",
                            u, m.uni_map
                        );
                    }
                    return false;
                }
            }
        }
        if arg_set != expected {
            if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
                eprintln!(
                    "[skolem-dbg] argument set mismatch for {}. arg_set={:?}, expected={:?}",
                    e, arg_set, expected
                );
            }
            return false;
        }
    }

    true
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
/// `Unknown` propagates to a final `Verdict::Unknown`, scoring 0 instead of the −1 a
/// false-positive `VerifiedBad` would cost).
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

    // Positive verification: reconstruct the Skolemisation structurally and
    // confirm the conclusion is exactly the parent with each existential
    // replaced by a distinct fresh Skolem term over its in-scope universals.
    // On success this is a *confirmed* sound step (`Sound` → contributes to
    // `VerifiedGood`); the proofs the dataset emits without a
    // `skolemize(Var, sk(...))` annotation (e.g. PyRes) are handled here.
    if try_positive_skolemize(parent_f, step_f, &fresh, registry) {
        for s in &fresh {
            registry.record(s);
        }
        return StepOutcome::Sound;
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
    /// for `X2` and an arity-2 Skolem `sk(X3,X4)` for `X5`. With positive
    /// verification this is now confirmed `Sound` (→ `VerifiedGood`).
    #[test]
    fn nested_multi_existential_skolemize_is_sound() {
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
        let outcome = check(step, Some(parent), &mut reg, Some("esa"));
        assert!(
            matches!(outcome, StepOutcome::Sound),
            "valid nested multi-existential skolemization must be Sound, got {outcome:?}"
        );
    }

    /// Skolemisation pulls universals to the front, *regrouping* binders that
    /// were separated by eliminated existentials:
    /// `! X2 ? X3 ! X4 ? X5 . φ`  →  `! [X2,X4] . φ[X3:=sk1(X2), X5:=sk2(X2,X4)]`.
    /// The matcher must strip quantifiers from each side independently.
    #[test]
    fn regrouped_universals_skolemize_is_sound() {
        let parent = nth_fof(
            "fof(c2, negated_conjecture, \
             (! [X2]: (? [X3]: (! [X4]: (? [X5]: \
              (big_f(X2,X3) & (big_f(X4,X5) & big_f(X3,X5))))))), \
             inference(variable_rename,[status(thm)],[c1])).",
            0,
        );
        let step = nth_fof(
            "fof(c3, negated_conjecture, \
             (! [X2,X4]: (big_f(X2,skolem0001(X2)) & \
              (big_f(X4,skolem0002(X2,X4)) & big_f(skolem0001(X2),skolem0002(X2,X4))))), \
             inference(skolemize,[status(esa)],[c2])).",
            0,
        );
        let mut reg = SkolemRegistry::new();
        let outcome = check(step, Some(parent), &mut reg, Some("esa"));
        assert!(
            matches!(outcome, StepOutcome::Sound),
            "regrouped-universal skolemization must be Sound, got {outcome:?}"
        );
    }

    /// An existential nested *inside the matrix* (not in the leading prefix)
    /// must still be verified: `? X2 ! X3 ? X5 . (p(X2,X5) & ? X6. q(X3,X6))`.
    #[test]
    fn matrix_nested_existential_skolemize_is_sound() {
        let parent = nth_fof(
            "fof(c2, negated_conjecture, \
             (? [X2]: (! [X3]: (? [X5]: \
              (big_f(X2,X5) & (? [X6]: big_g(X3,X6)))))), \
             inference(variable_rename,[status(thm)],[c1])).",
            0,
        );
        let step = nth_fof(
            "fof(c3, negated_conjecture, \
             (! [X3]: (big_f(skolem0001,skolem0002(X3)) & big_g(X3,skolem0003(X3)))), \
             inference(skolemize,[status(esa)],[c2])).",
            0,
        );
        let mut reg = SkolemRegistry::new();
        let outcome = check(step, Some(parent), &mut reg, Some("esa"));
        assert!(
            matches!(outcome, StepOutcome::Sound),
            "matrix-nested existential skolemization must be Sound, got {outcome:?}"
        );
    }

    /// A Skolem term that *under-captures* its in-scope universals
    /// (`sk(X2,X4)` shrunk to `sk(X2)`) is an unsound dependency. The positive
    /// check must NOT confirm it (no false `Sound`/`VerifiedGood`); it falls back to
    /// the conservative `StepOutcome::Unknown`, aggregating to `Verdict::Unknown`.
    #[test]
    fn under_capturing_skolem_is_not_sound() {
        let parent = nth_fof(
            "fof(c2, negated_conjecture, \
             (! [X2]: (! [X4]: (? [X5]: big_f(X2,X4,X5)))), \
             inference(variable_rename,[status(thm)],[c1])).",
            0,
        );
        // X5's Skolem should depend on {X2,X4} but here only on {X2}.
        let step = nth_fof(
            "fof(c3, negated_conjecture, \
             (! [X2,X4]: big_f(X2,X4,skolem0001(X2))), \
             inference(skolemize,[status(esa)],[c2])).",
            0,
        );
        let mut reg = SkolemRegistry::new();
        let outcome = check(step, Some(parent), &mut reg, Some("esa"));
        assert!(
            !matches!(outcome, StepOutcome::Sound),
            "under-capturing skolem must not be confirmed Sound, got {outcome:?}"
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

    /// A Skolemisation step where the conjunction is re-associated / re-ordered
    /// (e.g. `(A & B) & C` became `A & (C & B)`) must be successfully matched and verified as Sound.
    #[test]
    fn ac_aware_skolemize_matching_is_sound() {
        let parent = nth_fof(
            "fof(c2, negated_conjecture, \
             (? [X2]: (big_a(X2) & (big_b(X2) & big_c(X2)))), \
             inference(variable_rename,[status(thm)],[c1])).",
            0,
        );
        // Notice the conjunction matrix is re-associated and re-ordered in c3!
        let step = nth_fof(
            "fof(c3, negated_conjecture, \
             ((big_b(skolem0001) & big_c(skolem0001)) & big_a(skolem0001)), \
             inference(skolemize,[status(esa)],[c2])).",
            0,
        );
        let mut reg = SkolemRegistry::new();
        let outcome = check(step, Some(parent), &mut reg, Some("esa"));
        assert!(
            matches!(outcome, StepOutcome::Sound),
            "AC-aware re-ordered / re-associated skolemization must be Sound, got {outcome:?}"
        );
    }

    /// Regression guard: the multiset matcher must be **bijective**, not a
    /// subset match. A step that Skolemises `X` but silently *drops* the
    /// unrelated conjunct `big_b(X2)` from the parent is not a valid
    /// Skolemisation — dropping a conjunct is an unsound step, not merely an
    /// unconfirmable one, but this positive-verification path can at worst
    /// return `Unknown` (never confirm it `Sound`).
    #[test]
    fn skolemize_subset_match_does_not_drop_conjuncts() {
        let parent = nth_fof(
            "fof(c2, negated_conjecture, \
             (big_a(c0) & (big_b(c0) & (? [X2]: big_r(X2)))), \
             inference(variable_rename,[status(thm)],[c1])).",
            0,
        );
        // `big_b(c0)` is dropped entirely — this must NOT be confirmed Sound.
        let step = nth_fof(
            "fof(c3, negated_conjecture, \
             (big_a(c0) & big_r(skolem0001)), \
             inference(skolemize,[status(esa)],[c2])).",
            0,
        );
        let mut reg = SkolemRegistry::new();
        let outcome = check(step, Some(parent), &mut reg, Some("esa"));
        assert!(
            !matches!(outcome, StepOutcome::Sound),
            "a skolemize step that drops a parent conjunct must never be confirmed Sound, got {outcome:?}"
        );
    }
}
