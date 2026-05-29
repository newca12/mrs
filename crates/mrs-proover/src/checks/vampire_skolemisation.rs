//! Structural check for Vampire's `skolemisation` inference rule.
//!
//! Vampire emits skolemisation in a single step that simultaneously
//! eliminates every existential subformula in its source formula, with
//! one Skolem axiom (`introduced(definition, [], [skolem_symbol_introduction])`)
//! per Skolem function symbol. The step's annotation lists those Skolems
//! in `new_symbols(skolem, [...])` and its parents are `[source,
//! sk_ax_1, ..., sk_ax_n]` in arbitrary order.
//!
//! Each Skolem axiom body has the shape
//!
//! ```text
//!   ∀U_1..U_k. (∃V. φ(U, V)) → φ(U, sK_i(t_1, ..., t_k))
//! ```
//!
//! where `t_j` are the (already-quantified) `U_j`. The axiom is sound by
//! definition (already accepted by `introduced_definition::try_skolem_axiom`
//! when that node was processed) and applying it to a formula that
//! contains the antecedent as a subformula yields an equisatisfiable
//! result with the consequent in its place.
//!
//! This check verifies that the step is exactly the result of applying
//! all `n` Skolem axioms to the source: starting from the source, for
//! each axiom we find a matching `∃V. φ(U, V)` subformula and replace
//! it with `φ(U, sK_i(t_j))`. If after applying all axioms the result
//! equals the step formula, the step is `Sound`.
//!
//! Returns `None` when the step does not match the expected shape so
//! the caller can fall back to the ATP.

use std::collections::HashMap;

use mrs_tptp::{
    AtomicWord, FOFAnnotated, FOFAtomicFormula, FOFFormula, FOFStatement, FOFTerm, Quantifier,
};

use crate::checks::skolemize::SkolemRegistry;
use crate::verdict::StepOutcome;

/// Try to verify a `skolemisation` step structurally.
///
/// `parents` is the ordered list of parent nodes (same order as the
/// annotation's `[name1, name2, ...]` list). The first parent is the
/// source; the rest are Skolem-axiom intro-defs in some order.
///
/// Returns:
///   - `Some(Sound)`   — the step was verified structurally.
///   - `Some(Unknown)` — the step has the right shape but the structural
///                       reproduction failed (likely benign, fall through
///                       to ATP would be wasteful; report Unknown).
///   - `None`          — the step does not have the expected skolemisation
///                       shape; the caller should try the ATP instead.
pub fn try_check<'p>(
    step: &'p FOFAnnotated<'p>,
    parents: &[&'p FOFAnnotated<'p>],
    registry: &mut SkolemRegistry,
) -> Option<StepOutcome> {
    let ann = step.annotations.as_ref()?;
    if ann.status() != Some("esa") {
        return None;
    }
    let declared_skolems = ann.new_symbols();
    if declared_skolems.is_empty() {
        return None;
    }
    if parents.is_empty() {
        return None;
    }

    // Identify the source (the parent that is not an introduced Skolem
    // axiom) and collect Skolem-axiom bodies.
    let mut source: Option<&FOFFormula<'p>> = None;
    let mut axioms: Vec<SkolemAxiom<'p>> = Vec::new();
    for p in parents {
        if is_skolem_intro(p) {
            if let Some(ax) = parse_skolem_axiom(p) {
                axioms.push(ax);
            } else {
                // A Skolem-axiom parent we cannot parse — abort and
                // let the ATP try.
                return None;
            }
        } else {
            // First non-intro parent is the source.
            if source.is_some() {
                // Multiple non-Skolem parents — unexpected shape.
                return None;
            }
            source = Some(match &p.formula {
                FOFStatement::Logical(f) => f,
                _ => return None,
            });
        }
    }
    let source = source?;

    // The number of Skolem axioms should equal the number of declared
    // Skolem symbols; otherwise the shape is unexpected.
    if axioms.len() != declared_skolems.len() {
        return None;
    }
    let declared_set: std::collections::HashSet<&str> =
        declared_skolems.iter().copied().collect();
    for ax in &axioms {
        if !declared_set.contains(ax.skolem_symbol) {
            return None;
        }
    }

    // Freshness check: every declared Skolem must be absent from the
    // problem-symbol registry. (Same convention as Fix #6.)
    let stale: Vec<&str> = declared_skolems
        .iter()
        .copied()
        .filter(|s| registry.seen_symbols.contains(*s))
        .collect();
    if !stale.is_empty() {
        return Some(StepOutcome::Unknown(format!(
            "skolemisation: declared Skolem(s) {stale:?} clash with the problem's symbols"
        )));
    }

    // Apply each axiom to the source. Axioms have dependencies — the
    // antecedent of a later axiom may reference Skolems introduced by
    // an earlier one (e.g. LCL654's sK6-using axioms reference sK0).
    // Vampire emits parents in arbitrary order, so we use a fixpoint
    // loop: repeatedly scan the remaining axioms and apply any that
    // match. We're done when all axioms have been consumed; we fail
    // (return None) if a full pass finds no applicable axiom.
    //
    // Intermediate formulas live in an arena-style `Vec<Box<…>>` so
    // each `&'p` borrow handed to `apply_axiom` survives across loop
    // iterations.
    let mut arena: Vec<Box<FOFFormula<'p>>> = Vec::with_capacity(axioms.len() + 1);
    arena.push(Box::new(source.clone()));
    // Apply axioms outermost-first: a Skolem axiom whose antecedent
    // contains another axiom's antecedent as a subformula must be
    // applied first, otherwise the inner rewrite alters the outer
    // antecedent and the outer match silently fails.
    //
    // We approximate this by sorting axioms in descending order of
    // antecedent size (a "bigger" antecedent is more likely to be
    // outer). For each axiom we record its original parent index so
    // ordering remains deterministic.
    let mut remaining: Vec<usize> = (0..axioms.len()).collect();
    remaining.sort_by(|&i, &j| {
        formula_size(&axioms[j].antecedent).cmp(&formula_size(&axioms[i].antecedent))
    });
    while !remaining.is_empty() {
        let mut progressed = false;
        let mut i = 0;
        while i < remaining.len() {
            let ax_idx = remaining[i];
            // SAFETY: we never remove or mutate elements in the arena;
            // each boxed formula remains valid for the rest of this
            // function. We never mutate `axioms` either.
            let current: &FOFFormula<'p> = arena.last().unwrap();
            let current_ref: &'p FOFFormula<'p> =
                unsafe { &*(current as *const FOFFormula<'p>) };
            let ax_ref: &'p SkolemAxiom<'p> =
                unsafe { &*(&axioms[ax_idx] as *const SkolemAxiom<'p>) };
            match apply_axiom(current_ref, ax_ref) {
                Some(next) => {
                    arena.push(Box::new(next));
                    remaining.swap_remove(i);
                    progressed = true;
                }
                None => {
                    i += 1;
                }
            }
        }
        if !progressed {
            // No axiom in `remaining` could apply to the current
            // formula. Either the matcher is incomplete or the step
            // has an unexpected shape; defer to the ATP.
            return None;
        }
    }
    let current: &FOFFormula<'p> = arena.last().unwrap();

    let step_f = match &step.formula {
        FOFStatement::Logical(f) => f,
        _ => return None,
    };

    if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
        eprintln!("[skolem-dbg] all axioms applied; comparing");
        eprintln!("[skolem-dbg] current = {current:?}");
        eprintln!("[skolem-dbg] step    = {step_f:?}");
    }

    if formula_eq(current, step_f) {
        // Record the Skolems so downstream freshness checks see them as
        // taken (the `introduced_definition` Skolem-axiom check also
        // records them, but doing so here is idempotent and protects
        // against any topo-ordering surprise).
        for s in &declared_skolems {
            registry.record(s);
        }
        Some(StepOutcome::Sound)
    } else {
        // The shape matches but the rewrite did not reproduce the step.
        // Either our matcher is missing a case or Vampire emitted
        // something subtly different. Fall back to ATP.
        None
    }
}

/// Parsed Skolem axiom: `∀U_1..U_k. (∃V_1..V_m. body_phi) → consequent`.
///
/// `consequent` is `body_phi` with each `V_j` replaced by an application
/// of `skolem_symbol` to some terms over `universals`. We do not
/// pre-extract the substitution — it is recovered at match time.
struct SkolemAxiom<'p> {
    universals: Vec<&'p str>,
    existentials: Vec<&'p str>,
    antecedent: FOFFormula<'p>,
    consequent: FOFFormula<'p>,
    skolem_symbol: &'p str,
}

fn is_skolem_intro<'p>(node: &FOFAnnotated<'p>) -> bool {
    crate::checks::introduced_definition::is_skolem_symbol_introduction(
        node.annotations.as_ref(),
    )
}

/// Parse a Skolem-axiom body of shape `∀U. (∃V. φ) → ψ`.
fn parse_skolem_axiom<'p>(node: &'p FOFAnnotated<'p>) -> Option<SkolemAxiom<'p>> {
    let f = match &node.formula {
        FOFStatement::Logical(f) => f,
        _ => return None,
    };
    // Strip outer parens / universal prefix.
    let mut universals: Vec<&'p str> = Vec::new();
    let mut cur = strip_parens(f);
    while let FOFFormula::Quantified {
        quantifier: Quantifier::Forall,
        variables,
        formula,
    } = cur
    {
        for v in variables {
            universals.push(*v);
        }
        cur = strip_parens(formula);
    }
    // Expect an implication next.
    let (lhs, rhs) = match cur {
        FOFFormula::Binary {
            left,
            connective: mrs_tptp::BinaryConnective::Impl,
            right,
        } => (strip_parens(left), strip_parens(right)),
        _ => return None,
    };
    // LHS must be `∃V. φ`.
    let (existentials, body_phi) = match lhs {
        FOFFormula::Quantified {
            quantifier: Quantifier::Exists,
            variables,
            formula,
        } => (variables.clone(), strip_parens(formula).clone()),
        _ => return None,
    };
    // Identify the Skolem symbol: a function symbol that appears in rhs
    // but not in body_phi.
    let mut rhs_syms: std::collections::HashSet<&'p str> = Default::default();
    let mut phi_syms: std::collections::HashSet<&'p str> = Default::default();
    crate::checks::introduced_definition::collect_fun_syms(rhs, &mut rhs_syms);
    crate::checks::introduced_definition::collect_fun_syms(&body_phi, &mut phi_syms);
    let new_syms: Vec<&'p str> = rhs_syms.difference(&phi_syms).copied().collect();
    if new_syms.len() != 1 {
        return None;
    }
    Some(SkolemAxiom {
        universals,
        existentials,
        antecedent: body_phi,
        consequent: rhs.clone(),
        skolem_symbol: new_syms[0],
    })
}

/// Try to apply a Skolem axiom to a formula. Returns the rewritten
/// formula if a matching subformula was found and replaced; otherwise
/// `None`.
///
/// We walk the formula looking for a subformula `∃V_1..V_m. φ'` such
/// that there is a substitution σ on the axiom's universals making
/// φ'(V) α-equivalent (modulo existential variable renaming) to
/// `axiom.antecedent(σ(U), V)`. When found, replace the subformula with
/// `axiom.consequent(σ(U))[V := skolem_term]`.
fn apply_axiom<'p>(f: &'p FOFFormula<'p>, axiom: &'p SkolemAxiom<'p>) -> Option<FOFFormula<'p>> {
    let mut found = false;
    let result = apply_axiom_walk(f, axiom, &mut found);
    if found { Some(result) } else { None }
}

fn apply_axiom_walk<'p>(
    f: &'p FOFFormula<'p>,
    axiom: &'p SkolemAxiom<'p>,
    found: &mut bool,
) -> FOFFormula<'p> {
    if *found {
        // Only apply the axiom once per call: each `skolem_symbol_introduction`
        // axiom is intended to eliminate a single existential occurrence.
        return f.clone();
    }
    // Try to match this whole subformula against `∃V. axiom.antecedent`.
    if let FOFFormula::Quantified {
        quantifier: Quantifier::Exists,
        variables,
        formula,
    } = strip_parens(f)
    {
        if variables.len() == axiom.existentials.len() {
            let body = strip_parens(formula);
            // Find a universal binding σ such that
            // axiom.antecedent[U:=σ(U), V:=variables] α-equals body.
            if let Some(sigma) = match_with_universal_subst(
                &axiom.antecedent,
                body,
                &axiom.universals,
                &axiom.existentials,
                variables,
            ) {
                // Build the Skolem term from σ.
                let sk_args: Vec<FOFTerm<'p>> = axiom
                    .universals
                    .iter()
                    .map(|u| sigma.get(u).cloned().unwrap_or(FOFTerm::Variable(u)))
                    .collect();
                let sk_term = if sk_args.is_empty() {
                    FOFTerm::Function(AtomicWord::Lower(axiom.skolem_symbol), vec![])
                } else {
                    FOFTerm::Function(AtomicWord::Lower(axiom.skolem_symbol), sk_args)
                };
                // Substitute the existentials with sk_term in the
                // consequent (with universals first σ-substituted).
                let mut new_body = subst_universals(&axiom.consequent, &sigma);
                for (v_ax, v_local) in axiom.existentials.iter().zip(variables.iter()) {
                    // The consequent has the skolem term already in
                    // place of the existential variable; but in case it
                    // still mentions the existential name (when not
                    // bound away), we substitute too.
                    new_body = subst_var(&new_body, v_ax, &sk_term);
                    // Also substitute the local existential name so any
                    // residual references are removed.
                    new_body = subst_var(&new_body, v_local, &sk_term);
                }
                *found = true;
                return new_body;
            }
        }
    }
    // Otherwise recurse into children.
    match f {
        FOFFormula::Atomic(_) | FOFFormula::Equality(_, _) | FOFFormula::Inequality(_, _) => {
            f.clone()
        }
        FOFFormula::Negation(inner) => {
            FOFFormula::Negation(Box::new(apply_axiom_walk(inner, axiom, found)))
        }
        FOFFormula::Parens(inner) => {
            FOFFormula::Parens(Box::new(apply_axiom_walk(inner, axiom, found)))
        }
        FOFFormula::Binary {
            left,
            connective,
            right,
        } => {
            let new_left = apply_axiom_walk(left, axiom, found);
            let new_right = apply_axiom_walk(right, axiom, found);
            FOFFormula::Binary {
                left: Box::new(new_left),
                connective: *connective,
                right: Box::new(new_right),
            }
        }
        FOFFormula::Quantified {
            quantifier,
            variables,
            formula,
        } => FOFFormula::Quantified {
            quantifier: *quantifier,
            variables: variables.clone(),
            formula: Box::new(apply_axiom_walk(formula, axiom, found)),
        },
    }
}

/// Try to find a substitution on the axiom's universal variables that
/// makes `axiom_body[U:=σ(U), V:=local_exists]` structurally equal to
/// `local_body`. Returns `None` on mismatch.
///
/// `axiom_exists` and `local_exists` are renamed pointwise.
fn match_with_universal_subst<'p>(
    axiom_body: &'p FOFFormula<'p>,
    local_body: &'p FOFFormula<'p>,
    universals: &[&'p str],
    axiom_exists: &[&'p str],
    local_exists: &[&'p str],
) -> Option<HashMap<&'p str, FOFTerm<'p>>> {
    let mut sigma: HashMap<&'p str, FOFTerm<'p>> = HashMap::new();
    let univ_set: std::collections::HashSet<&'p str> = universals.iter().copied().collect();
    let ax_exists_set: std::collections::HashSet<&'p str> =
        axiom_exists.iter().copied().collect();
    let local_exists_set: std::collections::HashSet<&'p str> =
        local_exists.iter().copied().collect();
    // Build a renaming for existentials: when matching, axiom V_i is
    // considered equal to local V_i.
    let mut exists_renaming: HashMap<&'p str, &'p str> = HashMap::new();
    for (a, l) in axiom_exists.iter().zip(local_exists.iter()) {
        exists_renaming.insert(*a, *l);
    }
    if match_formula(
        axiom_body,
        local_body,
        &univ_set,
        &ax_exists_set,
        &local_exists_set,
        &exists_renaming,
        &mut sigma,
    ) {
        Some(sigma)
    } else {
        None
    }
}

fn match_formula<'p>(
    a: &'p FOFFormula<'p>,
    b: &'p FOFFormula<'p>,
    universals: &std::collections::HashSet<&'p str>,
    a_exists: &std::collections::HashSet<&'p str>,
    b_exists: &std::collections::HashSet<&'p str>,
    exists_renaming: &HashMap<&'p str, &'p str>,
    sigma: &mut HashMap<&'p str, FOFTerm<'p>>,
) -> bool {
    let a = strip_parens(a);
    let b = strip_parens(b);
    match (a, b) {
        (FOFFormula::Atomic(ax), FOFFormula::Atomic(bx)) => {
            match_atomic(ax, bx, universals, exists_renaming, sigma)
        }
        (FOFFormula::Negation(ax), FOFFormula::Negation(bx)) => match_formula(
            ax,
            bx,
            universals,
            a_exists,
            b_exists,
            exists_renaming,
            sigma,
        ),
        (FOFFormula::Equality(la, ra), FOFFormula::Equality(lb, rb))
        | (FOFFormula::Inequality(la, ra), FOFFormula::Inequality(lb, rb)) => {
            match_term(la, lb, universals, exists_renaming, sigma)
                && match_term(ra, rb, universals, exists_renaming, sigma)
        }
        (
            FOFFormula::Binary {
                left: la,
                connective: ca,
                right: ra,
            },
            FOFFormula::Binary {
                left: lb,
                connective: cb,
                right: rb,
            },
        ) => {
            ca == cb
                && match_formula(
                    la,
                    lb,
                    universals,
                    a_exists,
                    b_exists,
                    exists_renaming,
                    sigma,
                )
                && match_formula(
                    ra,
                    rb,
                    universals,
                    a_exists,
                    b_exists,
                    exists_renaming,
                    sigma,
                )
        }
        (
            FOFFormula::Quantified {
                quantifier: qa,
                variables: va,
                formula: fa,
            },
            FOFFormula::Quantified {
                quantifier: qb,
                variables: vb,
                formula: fb,
            },
        ) => {
            // For quantifiers under the matched antecedent, we expect
            // the variable lists to be of the same length and we treat
            // bound variables positionally.
            if qa != qb || va.len() != vb.len() {
                return false;
            }
            let mut new_renaming = exists_renaming.clone();
            for (a_v, b_v) in va.iter().zip(vb.iter()) {
                new_renaming.insert(*a_v, *b_v);
            }
            match_formula(
                fa,
                fb,
                universals,
                a_exists,
                b_exists,
                &new_renaming,
                sigma,
            )
        }
        _ => false,
    }
}

fn match_atomic<'p>(
    a: &'p FOFAtomicFormula<'p>,
    b: &'p FOFAtomicFormula<'p>,
    universals: &std::collections::HashSet<&'p str>,
    renaming: &HashMap<&'p str, &'p str>,
    sigma: &mut HashMap<&'p str, FOFTerm<'p>>,
) -> bool {
    use FOFAtomicFormula::*;
    match (a, b) {
        (Plain(wa, aa), Plain(wb, ab)) => {
            wa == wb
                && aa.len() == ab.len()
                && aa
                    .iter()
                    .zip(ab.iter())
                    .all(|(x, y)| match_term(x, y, universals, renaming, sigma))
        }
        (Defined(wa, aa), Defined(wb, ab)) => {
            wa == wb
                && aa.len() == ab.len()
                && aa
                    .iter()
                    .zip(ab.iter())
                    .all(|(x, y)| match_term(x, y, universals, renaming, sigma))
        }
        (System(wa, aa), System(wb, ab)) => {
            wa == wb
                && aa.len() == ab.len()
                && aa
                    .iter()
                    .zip(ab.iter())
                    .all(|(x, y)| match_term(x, y, universals, renaming, sigma))
        }
        (True, True) | (False, False) => true,
        _ => false,
    }
}

fn match_term<'p>(
    a: &'p FOFTerm<'p>,
    b: &'p FOFTerm<'p>,
    universals: &std::collections::HashSet<&'p str>,
    renaming: &HashMap<&'p str, &'p str>,
    sigma: &mut HashMap<&'p str, FOFTerm<'p>>,
) -> bool {
    match (a, b) {
        (FOFTerm::Variable(va), _) if universals.contains(va) => {
            // Universal — bind to b. If already bound, must be consistent.
            if let Some(existing) = sigma.get(va) {
                term_eq_with_renaming(existing, b, renaming)
            } else {
                sigma.insert(*va, b.clone());
                true
            }
        }
        (FOFTerm::Variable(va), FOFTerm::Variable(vb)) => {
            if let Some(mapped) = renaming.get(va) {
                mapped == vb
            } else {
                va == vb
            }
        }
        (FOFTerm::Function(wa, aa), FOFTerm::Function(wb, ab)) => {
            wa == wb
                && aa.len() == ab.len()
                && aa
                    .iter()
                    .zip(ab.iter())
                    .all(|(x, y)| match_term(x, y, universals, renaming, sigma))
        }
        (FOFTerm::DefinedFunction(wa, aa), FOFTerm::DefinedFunction(wb, ab)) => {
            wa == wb
                && aa.len() == ab.len()
                && aa
                    .iter()
                    .zip(ab.iter())
                    .all(|(x, y)| match_term(x, y, universals, renaming, sigma))
        }
        (FOFTerm::SystemFunction(wa, aa), FOFTerm::SystemFunction(wb, ab)) => {
            wa == wb
                && aa.len() == ab.len()
                && aa
                    .iter()
                    .zip(ab.iter())
                    .all(|(x, y)| match_term(x, y, universals, renaming, sigma))
        }
        (FOFTerm::Number(x), FOFTerm::Number(y)) => x.as_str() == y.as_str(),
        (FOFTerm::DistinctObject(x), FOFTerm::DistinctObject(y)) => x == y,
        _ => false,
    }
}

fn term_eq_with_renaming<'p>(
    a: &FOFTerm<'p>,
    b: &FOFTerm<'p>,
    renaming: &HashMap<&'p str, &'p str>,
) -> bool {
    match (a, b) {
        (FOFTerm::Variable(va), FOFTerm::Variable(vb)) => {
            if let Some(mapped) = renaming.get(va) {
                mapped == vb
            } else {
                va == vb
            }
        }
        (FOFTerm::Function(wa, aa), FOFTerm::Function(wb, ab)) => {
            wa == wb
                && aa.len() == ab.len()
                && aa
                    .iter()
                    .zip(ab.iter())
                    .all(|(x, y)| term_eq_with_renaming(x, y, renaming))
        }
        (FOFTerm::DefinedFunction(wa, aa), FOFTerm::DefinedFunction(wb, ab)) => {
            wa == wb
                && aa.len() == ab.len()
                && aa
                    .iter()
                    .zip(ab.iter())
                    .all(|(x, y)| term_eq_with_renaming(x, y, renaming))
        }
        (FOFTerm::SystemFunction(wa, aa), FOFTerm::SystemFunction(wb, ab)) => {
            wa == wb
                && aa.len() == ab.len()
                && aa
                    .iter()
                    .zip(ab.iter())
                    .all(|(x, y)| term_eq_with_renaming(x, y, renaming))
        }
        (FOFTerm::Number(x), FOFTerm::Number(y)) => x.as_str() == y.as_str(),
        (FOFTerm::DistinctObject(x), FOFTerm::DistinctObject(y)) => x == y,
        _ => false,
    }
}

fn subst_universals<'p>(
    f: &FOFFormula<'p>,
    sigma: &HashMap<&'p str, FOFTerm<'p>>,
) -> FOFFormula<'p> {
    match f {
        FOFFormula::Atomic(a) => FOFFormula::Atomic(subst_in_atomic_uni(a, sigma)),
        FOFFormula::Negation(inner) => FOFFormula::Negation(Box::new(subst_universals(inner, sigma))),
        FOFFormula::Parens(inner) => FOFFormula::Parens(Box::new(subst_universals(inner, sigma))),
        FOFFormula::Equality(l, r) => {
            FOFFormula::Equality(subst_in_term_uni(l, sigma), subst_in_term_uni(r, sigma))
        }
        FOFFormula::Inequality(l, r) => {
            FOFFormula::Inequality(subst_in_term_uni(l, sigma), subst_in_term_uni(r, sigma))
        }
        FOFFormula::Binary {
            left,
            connective,
            right,
        } => FOFFormula::Binary {
            left: Box::new(subst_universals(left, sigma)),
            connective: *connective,
            right: Box::new(subst_universals(right, sigma)),
        },
        FOFFormula::Quantified {
            quantifier,
            variables,
            formula,
        } => FOFFormula::Quantified {
            quantifier: *quantifier,
            variables: variables.clone(),
            formula: Box::new(subst_universals(formula, sigma)),
        },
    }
}

fn subst_in_atomic_uni<'p>(
    a: &FOFAtomicFormula<'p>,
    sigma: &HashMap<&'p str, FOFTerm<'p>>,
) -> FOFAtomicFormula<'p> {
    match a {
        FOFAtomicFormula::Plain(w, args) => FOFAtomicFormula::Plain(
            w.clone(),
            args.iter().map(|t| subst_in_term_uni(t, sigma)).collect(),
        ),
        FOFAtomicFormula::Defined(w, args) => FOFAtomicFormula::Defined(
            w.clone(),
            args.iter().map(|t| subst_in_term_uni(t, sigma)).collect(),
        ),
        FOFAtomicFormula::System(w, args) => FOFAtomicFormula::System(
            w.clone(),
            args.iter().map(|t| subst_in_term_uni(t, sigma)).collect(),
        ),
        FOFAtomicFormula::True => FOFAtomicFormula::True,
        FOFAtomicFormula::False => FOFAtomicFormula::False,
    }
}

fn subst_in_term_uni<'p>(
    t: &FOFTerm<'p>,
    sigma: &HashMap<&'p str, FOFTerm<'p>>,
) -> FOFTerm<'p> {
    match t {
        FOFTerm::Variable(v) => sigma.get(v).cloned().unwrap_or(FOFTerm::Variable(v)),
        FOFTerm::Function(w, args) => FOFTerm::Function(
            w.clone(),
            args.iter().map(|a| subst_in_term_uni(a, sigma)).collect(),
        ),
        FOFTerm::DefinedFunction(w, args) => FOFTerm::DefinedFunction(
            w.clone(),
            args.iter().map(|a| subst_in_term_uni(a, sigma)).collect(),
        ),
        FOFTerm::SystemFunction(w, args) => FOFTerm::SystemFunction(
            w.clone(),
            args.iter().map(|a| subst_in_term_uni(a, sigma)).collect(),
        ),
        FOFTerm::Number(_) | FOFTerm::DistinctObject(_) => t.clone(),
    }
}

fn subst_var<'p>(f: &FOFFormula<'p>, var: &str, replacement: &FOFTerm<'p>) -> FOFFormula<'p> {
    match f {
        FOFFormula::Atomic(a) => FOFFormula::Atomic(subst_in_atomic_var(a, var, replacement)),
        FOFFormula::Negation(inner) => {
            FOFFormula::Negation(Box::new(subst_var(inner, var, replacement)))
        }
        FOFFormula::Parens(inner) => {
            FOFFormula::Parens(Box::new(subst_var(inner, var, replacement)))
        }
        FOFFormula::Equality(l, r) => FOFFormula::Equality(
            subst_in_term_var(l, var, replacement),
            subst_in_term_var(r, var, replacement),
        ),
        FOFFormula::Inequality(l, r) => FOFFormula::Inequality(
            subst_in_term_var(l, var, replacement),
            subst_in_term_var(r, var, replacement),
        ),
        FOFFormula::Binary {
            left,
            connective,
            right,
        } => FOFFormula::Binary {
            left: Box::new(subst_var(left, var, replacement)),
            connective: *connective,
            right: Box::new(subst_var(right, var, replacement)),
        },
        FOFFormula::Quantified {
            quantifier,
            variables,
            formula,
        } => {
            if variables.contains(&var) {
                f.clone()
            } else {
                FOFFormula::Quantified {
                    quantifier: *quantifier,
                    variables: variables.clone(),
                    formula: Box::new(subst_var(formula, var, replacement)),
                }
            }
        }
    }
}

fn subst_in_atomic_var<'p>(
    a: &FOFAtomicFormula<'p>,
    var: &str,
    replacement: &FOFTerm<'p>,
) -> FOFAtomicFormula<'p> {
    match a {
        FOFAtomicFormula::Plain(w, args) => FOFAtomicFormula::Plain(
            w.clone(),
            args.iter()
                .map(|t| subst_in_term_var(t, var, replacement))
                .collect(),
        ),
        FOFAtomicFormula::Defined(w, args) => FOFAtomicFormula::Defined(
            w.clone(),
            args.iter()
                .map(|t| subst_in_term_var(t, var, replacement))
                .collect(),
        ),
        FOFAtomicFormula::System(w, args) => FOFAtomicFormula::System(
            w.clone(),
            args.iter()
                .map(|t| subst_in_term_var(t, var, replacement))
                .collect(),
        ),
        FOFAtomicFormula::True => FOFAtomicFormula::True,
        FOFAtomicFormula::False => FOFAtomicFormula::False,
    }
}

fn subst_in_term_var<'p>(
    t: &FOFTerm<'p>,
    var: &str,
    replacement: &FOFTerm<'p>,
) -> FOFTerm<'p> {
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
                .map(|a| subst_in_term_var(a, var, replacement))
                .collect(),
        ),
        FOFTerm::DefinedFunction(w, args) => FOFTerm::DefinedFunction(
            w.clone(),
            args.iter()
                .map(|a| subst_in_term_var(a, var, replacement))
                .collect(),
        ),
        FOFTerm::SystemFunction(w, args) => FOFTerm::SystemFunction(
            w.clone(),
            args.iter()
                .map(|a| subst_in_term_var(a, var, replacement))
                .collect(),
        ),
        FOFTerm::Number(_) | FOFTerm::DistinctObject(_) => t.clone(),
    }
}

fn strip_parens<'p>(f: &'p FOFFormula<'p>) -> &'p FOFFormula<'p> {
    let mut cur = f;
    while let FOFFormula::Parens(inner) = cur {
        cur = inner;
    }
    cur
}

/// Count formula nodes (atoms, connectives, quantifiers) — used as a
/// proxy for "outerness" when ordering Skolem axioms.
fn formula_size<'p>(f: &FOFFormula<'p>) -> usize {
    match f {
        FOFFormula::Atomic(_) | FOFFormula::Equality(_, _) | FOFFormula::Inequality(_, _) => 1,
        FOFFormula::Negation(inner) | FOFFormula::Parens(inner) => 1 + formula_size(inner),
        FOFFormula::Binary { left, right, .. } => 1 + formula_size(left) + formula_size(right),
        FOFFormula::Quantified { formula, .. } => 1 + formula_size(formula),
    }
}

fn formula_eq<'p>(a: &FOFFormula<'p>, b: &FOFFormula<'p>) -> bool {
    let a = strip_parens(a);
    let b = strip_parens(b);
    match (a, b) {
        (FOFFormula::Atomic(ax), FOFFormula::Atomic(bx)) => atomic_eq(ax, bx),
        (FOFFormula::Negation(ax), FOFFormula::Negation(bx)) => formula_eq(ax, bx),
        (FOFFormula::Equality(la, ra), FOFFormula::Equality(lb, rb))
        | (FOFFormula::Inequality(la, ra), FOFFormula::Inequality(lb, rb)) => {
            term_eq(la, lb) && term_eq(ra, rb)
        }
        (
            FOFFormula::Binary {
                left: la,
                connective: ca,
                right: ra,
            },
            FOFFormula::Binary {
                left: lb,
                connective: cb,
                right: rb,
            },
        ) => ca == cb && formula_eq(la, lb) && formula_eq(ra, rb),
        (
            FOFFormula::Quantified {
                quantifier: qa,
                variables: va,
                formula: fa,
            },
            FOFFormula::Quantified {
                quantifier: qb,
                variables: vb,
                formula: fb,
            },
        ) => qa == qb && va == vb && formula_eq(fa, fb),
        _ => false,
    }
}

fn atomic_eq<'p>(a: &FOFAtomicFormula<'p>, b: &FOFAtomicFormula<'p>) -> bool {
    use FOFAtomicFormula::*;
    match (a, b) {
        (Plain(wa, aa), Plain(wb, ab)) => {
            wa == wb
                && aa.len() == ab.len()
                && aa.iter().zip(ab.iter()).all(|(x, y)| term_eq(x, y))
        }
        (Defined(wa, aa), Defined(wb, ab)) => {
            wa == wb
                && aa.len() == ab.len()
                && aa.iter().zip(ab.iter()).all(|(x, y)| term_eq(x, y))
        }
        (System(wa, aa), System(wb, ab)) => {
            wa == wb
                && aa.len() == ab.len()
                && aa.iter().zip(ab.iter()).all(|(x, y)| term_eq(x, y))
        }
        (True, True) | (False, False) => true,
        _ => false,
    }
}

fn term_eq<'p>(a: &FOFTerm<'p>, b: &FOFTerm<'p>) -> bool {
    match (a, b) {
        (FOFTerm::Variable(x), FOFTerm::Variable(y)) => x == y,
        (FOFTerm::Function(wa, aa), FOFTerm::Function(wb, ab)) => {
            wa == wb
                && aa.len() == ab.len()
                && aa.iter().zip(ab.iter()).all(|(x, y)| term_eq(x, y))
        }
        (FOFTerm::DefinedFunction(wa, aa), FOFTerm::DefinedFunction(wb, ab)) => {
            wa == wb
                && aa.len() == ab.len()
                && aa.iter().zip(ab.iter()).all(|(x, y)| term_eq(x, y))
        }
        (FOFTerm::SystemFunction(wa, aa), FOFTerm::SystemFunction(wb, ab)) => {
            wa == wb
                && aa.len() == ab.len()
                && aa.iter().zip(ab.iter()).all(|(x, y)| term_eq(x, y))
        }
        (FOFTerm::Number(x), FOFTerm::Number(y)) => x.as_str() == y.as_str(),
        (FOFTerm::DistinctObject(x), FOFTerm::DistinctObject(y)) => x == y,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_tptp::parse_tptp;

    /// Parse a TPTP problem string and return references to its FOF
    /// annotated formulas. Leaks the parsed problem so the borrows
    /// live for the entire test.
    fn parse_fofs(input: &'static str) -> Vec<&'static FOFAnnotated<'static>> {
        let problem = Box::leak(Box::new(parse_tptp(input).expect("parse")));
        let mut out: Vec<&'static FOFAnnotated<'static>> = Vec::new();
        for f in &problem.formulas {
            if let mrs_tptp::AnnotatedFormula::FOF(a) = f {
                let a_static: &'static FOFAnnotated<'static> =
                    unsafe { &*(a as *const FOFAnnotated<'_> as *const FOFAnnotated<'static>) };
                out.push(a_static);
            }
        }
        out
    }

    fn run(
        input: &'static str,
        step_name: &str,
        parent_names: &[&str],
    ) -> Option<StepOutcome> {
        let fofs = parse_fofs(input);
        let by_name: HashMap<&str, &FOFAnnotated<'static>> = fofs
            .iter()
            .map(|a| (a.name.as_str(), *a))
            .collect();
        let step = *by_name.get(step_name).expect("step not found");
        let parents: Vec<&FOFAnnotated<'static>> = parent_names
            .iter()
            .map(|n| *by_name.get(*n).expect("parent not found"))
            .collect();
        let mut reg = SkolemRegistry::new();
        // Seed the registry only from formulas that represent the
        // original problem input (axioms/conjectures without an
        // inference annotation), matching what the live verifier does.
        for p in &fofs {
            if p.annotations.is_some() {
                continue;
            }
            if let FOFStatement::Logical(f) = &p.formula {
                let mut syms: std::collections::HashSet<&str> =
                    std::collections::HashSet::new();
                crate::checks::introduced_definition::collect_fun_syms(f, &mut syms);
                for s in syms {
                    reg.record(s);
                }
            }
        }
        try_check(step, &parents, &mut reg)
    }

    #[test]
    fn accepts_single_nullary_skolem() {
        let input = "\
fof(src, plain, (? [X0] : (sorti1(X0) & ! [X1] : (op1(X1,X1) = X0 | ~sorti1(X1)))), \
    inference(ennf_transformation, [], [])).
fof(ax, plain, \
    (? [X0] : (sorti1(X0) & ! [X1] : (op1(X1,X1) = X0 | ~sorti1(X1))) \
     => (sorti1(sK0) & ! [X1] : (op1(X1,X1) = sK0 | ~sorti1(X1)))), \
    introduced(definition, [], [skolem_symbol_introduction])).
fof(step, plain, (sorti1(sK0) & ! [X1] : (op1(X1,X1) = sK0 | ~sorti1(X1))), \
    inference(skolemisation, [status(esa), new_symbols(skolem, [sK0])], [src, ax])).
";
        let outcome = run(input, "step", &["src", "ax"]);
        assert!(matches!(outcome, Some(StepOutcome::Sound)),
            "expected Sound, got {outcome:?}");
    }

    #[test]
    fn accepts_single_unary_skolem() {
        let input = "\
fof(src, plain, (! [X0] : (~sorti2(X0) | ? [X1] : (op2(X1,X1) != X0 & sorti2(X1)))), \
    inference(ennf_transformation, [], [])).
fof(ax, plain, \
    (! [X0] : (? [X1] : (op2(X1,X1) != X0 & sorti2(X1)) \
     => (op2(sK1(X0),sK1(X0)) != X0 & sorti2(sK1(X0))))), \
    introduced(definition, [], [skolem_symbol_introduction])).
fof(step, plain, \
    (! [X0] : (~sorti2(X0) | (op2(sK1(X0),sK1(X0)) != X0 & sorti2(sK1(X0))))), \
    inference(skolemisation, [status(esa), new_symbols(skolem, [sK1])], [src, ax])).
";
        let outcome = run(input, "step", &["src", "ax"]);
        assert!(matches!(outcome, Some(StepOutcome::Sound)),
            "expected Sound, got {outcome:?}");
    }

    #[test]
    fn shape_mismatch_returns_none() {
        let input = "\
fof(src, plain, (? [X0] : p(X0)), inference(ennf_transformation, [], [])).
fof(ax, plain, ((? [X0] : p(X0)) => p(sK0)), \
    introduced(definition, [], [skolem_symbol_introduction])).
fof(step, plain, (q(sK0)), \
    inference(skolemisation, [status(esa), new_symbols(skolem, [sK0])], [src, ax])).
";
        let outcome = run(input, "step", &["src", "ax"]);
        assert!(outcome.is_none(), "expected None, got {outcome:?}");
    }

    #[test]
    fn stale_skolem_returns_unknown() {
        let input = "\
fof(prob, axiom, p(sK0)).
fof(src, plain, (? [X0] : p(X0)), inference(ennf_transformation, [], [])).
fof(ax, plain, ((? [X0] : p(X0)) => p(sK0)), \
    introduced(definition, [], [skolem_symbol_introduction])).
fof(step, plain, (p(sK0)), \
    inference(skolemisation, [status(esa), new_symbols(skolem, [sK0])], [src, ax])).
";
        let outcome = run(input, "step", &["src", "ax"]);
        assert!(
            matches!(outcome, Some(StepOutcome::Unknown(_))),
            "expected Unknown, got {outcome:?}"
        );
    }

    #[test]
    fn no_skolem_axiom_parents_returns_none() {
        let input = "\
fof(src, plain, (? [X0] : p(X0)), inference(ennf_transformation, [], [])).
fof(step, plain, (p(sK0)), \
    inference(skolemisation, [status(esa), new_symbols(skolem, [sK0])], [src])).
";
        let outcome = run(input, "step", &["src"]);
        assert!(outcome.is_none(), "expected None, got {outcome:?}");
    }

    #[test]
    fn axiom_order_independent() {
        let input = "\
fof(src, plain, ((? [X0] : p(X0)) & (? [X1] : q(X1))), \
    inference(ennf_transformation, [], [])).
fof(ax1, plain, ((? [X0] : p(X0)) => p(sK0)), \
    introduced(definition, [], [skolem_symbol_introduction])).
fof(ax2, plain, ((? [X1] : q(X1)) => q(sK1)), \
    introduced(definition, [], [skolem_symbol_introduction])).
fof(step, plain, (p(sK0) & q(sK1)), \
    inference(skolemisation, [status(esa), new_symbols(skolem, [sK0, sK1])], [ax2, src, ax1])).
";
        let outcome = run(input, "step", &["ax2", "src", "ax1"]);
        assert!(matches!(outcome, Some(StepOutcome::Sound)),
            "expected Sound, got {outcome:?}");
    }

    /// Dependent axioms (LCL654-style): an inner Skolem axiom's
    /// antecedent mentions the outer Skolem symbol. The fixpoint loop
    /// must apply the outer axiom first even though it appears later
    /// in the parent list.
    #[test]
    fn dependent_axioms_reverse_order() {
        let input = "\
fof(src, plain, (? [X0] : (p(X0) & ? [X1] : r(X0, X1))), \
    inference(ennf_transformation, [], [])).
fof(ax_outer, plain, \
    ((? [X0] : (p(X0) & ? [X1] : r(X0, X1))) => (p(sK0) & ? [X1] : r(sK0, X1))), \
    introduced(definition, [], [skolem_symbol_introduction])).
fof(ax_inner, plain, \
    ((? [X1] : r(sK0, X1)) => r(sK0, sK1)), \
    introduced(definition, [], [skolem_symbol_introduction])).
fof(step, plain, (p(sK0) & r(sK0, sK1)), \
    inference(skolemisation, [status(esa), new_symbols(skolem, [sK0, sK1])], \
              [src, ax_inner, ax_outer])).
";
        let outcome = run(input, "step", &["src", "ax_inner", "ax_outer"]);
        assert!(matches!(outcome, Some(StepOutcome::Sound)),
            "expected Sound, got {outcome:?}");
    }

    /// Multi-axiom nested skolemisation (LCL654-style minimal): the
    /// source has an outer `∃X0.…` and an inner `∃X1.…` under a `∀`.
    /// One axiom Skolemises the outer ∃, another Skolemises the inner
    /// ∃ (now under sK0 in its antecedent). The outer axiom MUST be
    /// applied first; descending antecedent-size ordering achieves it.
    #[test]
    fn outer_and_inner_skolems_require_size_ordering() {
        let input = "\
fof(src, plain, \
    (? [X0] : (p(X0) & ! [Y] : (q(X0, Y) | ? [X1] : r(Y, X1)))), \
    inference(ennf_transformation, [], [])).
fof(ax_outer, plain, \
    ((? [X0] : (p(X0) & ! [Y] : (q(X0, Y) | ? [X1] : r(Y, X1)))) \
     => (p(sK0) & ! [Y] : (q(sK0, Y) | ? [X1] : r(Y, X1)))), \
    introduced(definition, [], [skolem_symbol_introduction])).
fof(ax_inner, plain, \
    (! [Y] : ((? [X1] : r(Y, X1)) => r(Y, sK1(Y)))), \
    introduced(definition, [], [skolem_symbol_introduction])).
fof(step, plain, \
    (p(sK0) & ! [Y] : (q(sK0, Y) | r(Y, sK1(Y)))), \
    inference(skolemisation, [status(esa), new_symbols(skolem, [sK0, sK1])], \
              [src, ax_inner, ax_outer])).
";
        let outcome = run(input, "step", &["src", "ax_inner", "ax_outer"]);
        assert!(matches!(outcome, Some(StepOutcome::Sound)),
            "expected Sound, got {outcome:?}");
    }
}
