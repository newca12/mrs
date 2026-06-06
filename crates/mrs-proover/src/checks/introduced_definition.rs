//! Structural check for `introduced(definition)` clauses.
//!
//! Two ATP families emit such clauses with slightly different shapes:
//!
//! ## E (full biconditional)
//!
//! ```text
//! fof(c1, plain, (epred1_0 <=> phi), introduced(definition)).
//! fof(c2, plain, (! [X] : (P(X) <=> phi(X))), introduced(definition)).
//! ```
//!
//! Here a single clause asserts the entire biconditional `P :iff phi`.
//!
//! ## Vampire (directional definition fragments, with declared new symbol)
//!
//! ```text
//! fof(f18, plain, ( phi | ~sP2 ),
//!     introduced(definition, [new_symbols(naming, [sP2])],
//!                            [predicate_definition_introduction])).
//! ```
//!
//! Vampire's AVATAR splits the biconditional `sP2 :iff phi` into the two
//! clausal halves (`sP2 -> phi` written as `phi | ~sP2`, and `phi -> sP2`
//! written as `~phi | sP2`) and emits each as its own
//! `introduced(definition, ...)` clause. The fresh predicate symbol is
//! announced in the `new_symbols(naming, [...])` info entry.
//!
//! ## Soundness
//!
//! Either way the introduction is a *conservative extension* of the
//! theory: the new predicate symbol does not appear earlier in the proof
//! or in the linked problem, so any model can be extended with an
//! interpretation of the new symbol that satisfies the introduced
//! clause(s). Therefore no theorem about the original signature can be
//! falsified.
//!
//! ## What we accept
//!
//! 1. **E-style explicit biconditional** — the formula's body (after
//!    peeling leading universal quantifiers and parentheses) is
//!    `lhs <=> rhs` and one side's "head" predicate symbol is fresh
//!    w.r.t. the [`SkolemRegistry`].
//!
//! 2. **Vampire-style declared new symbol** — the annotation carries a
//!    `new_symbols(naming, [...])` entry naming a single new predicate
//!    symbol that is fresh w.r.t. the [`SkolemRegistry`]. The formula's
//!    shape is not further constrained; the freshness of the symbol is
//!    what makes the clause sound regardless of which directional
//!    fragment is being asserted.
//!
//! In both cases "fresh" means: not present in the registry, which the
//! main verify loop seeds from the linked problem and updates after each
//! prior node.

use std::collections::{HashMap, HashSet};

use mrs_tptp::ast::common::{AtomicWord, GeneralTerm, Quantifier};
use mrs_tptp::{
    AnnotatedFormula, Annotations, BinaryConnective, FOFAtomicFormula, FOFFormula, FOFStatement,
    FOFTerm,
};

use crate::checks::skolemize::SkolemRegistry;
use crate::verdict::StepOutcome;

/// Returns `true` iff the annotation source is `introduced(definition[, ...])`.
pub fn is_introduced_definition(ann: &Annotations<'_>) -> bool {
    match &ann.source {
        GeneralTerm::Function(AtomicWord::Lower("introduced"), args) if !args.is_empty() => {
            matches!(
                &args[0],
                GeneralTerm::Word(AtomicWord::Lower("definition"))
                    | GeneralTerm::Word(AtomicWord::SingleQuoted("definition"))
            )
        }
        // `introduced(definition)` with zero further args is sometimes
        // serialised as a bare word too; handle that defensively.
        GeneralTerm::Word(AtomicWord::Lower("introduced")) => true,
        _ => false,
    }
}

/// Returns `true` iff the annotation looks like a Vampire Skolem-axiom
/// introduction (`introduced(definition, _, [skolem_symbol_introduction])`).
/// Checks the third argument (intro list) for the `skolem_symbol_introduction`
/// marker. We do not require the second argument (info list) to be empty —
/// future Vampire versions may add metadata there.
pub fn is_skolem_symbol_introduction(ann: Option<&Annotations<'_>>) -> bool {
    let Some(ann) = ann else { return false };
    let GeneralTerm::Function(AtomicWord::Lower("introduced"), args) = &ann.source else {
        return false;
    };
    if args.len() < 3 {
        return false;
    }
    let GeneralTerm::List(intro_list) = &args[2] else {
        return false;
    };
    intro_list.iter().any(|t| {
        matches!(
            t,
            GeneralTerm::Word(AtomicWord::Lower("skolem_symbol_introduction"))
                | GeneralTerm::Word(AtomicWord::SingleQuoted("skolem_symbol_introduction"))
        )
    })
}

/// Returns `true` iff the annotation looks like an E-style
/// `predicate_definition_introduction`
/// (`introduced(definition, _, [predicate_definition_introduction])`).
/// Mirror of [`is_skolem_symbol_introduction`].
pub fn is_predicate_definition_introduction(ann: Option<&Annotations<'_>>) -> bool {
    let Some(ann) = ann else { return false };
    let GeneralTerm::Function(AtomicWord::Lower("introduced"), args) = &ann.source else {
        return false;
    };
    if args.len() < 3 {
        return false;
    }
    let GeneralTerm::List(intro_list) = &args[2] else {
        return false;
    };
    intro_list.iter().any(|t| {
        matches!(
            t,
            GeneralTerm::Word(AtomicWord::Lower("predicate_definition_introduction"))
                | GeneralTerm::Word(AtomicWord::SingleQuoted(
                    "predicate_definition_introduction"
                ))
        )
    })
}

/// Verify one `introduced(definition)` step. Returns
/// [`StepOutcome::Sound`] iff:
///
/// * the annotation declares a single new predicate symbol via
///   `new_symbols(naming, [P])` and `P` is fresh (Vampire shape), **or**
/// * the body is a Skolem axiom — universally closed
///   `(∃X. φ) → φ[X := sk(...)]` — whose Skolem function symbol does
///   not appear in any earlier proof step (Vampire's
///   `skolem_symbol_introduction` shape), **or**
/// * the formula body is a biconditional one of whose sides has a fresh
///   head predicate symbol (E shape).
///
/// Otherwise returns [`StepOutcome::Unknown`] with the reason — never
/// [`StepOutcome::Unsound`], because `introduced(definition)` carries no
/// claim that an attacker could break: an *unsupported* shape means we
/// can't certify, not that the step is wrong.
///
/// The caller is responsible for invoking this only when
/// [`is_introduced_definition`] returns true.
pub fn check<'p>(step: &AnnotatedFormula<'p>, registry: &SkolemRegistry) -> StepOutcome {
    // --- Vampire shape: annotation declares the new symbol(s) directly. ---
    let step_fof = match step.as_fof() {
        Some(f) => f,
        None => return StepOutcome::Unknown("introduced(definition) step is not FOF".into()),
    };
    if let Some(ann) = step.annotations() {
        let declared = declared_new_symbols(ann);
        if declared.len() == 1 {
            let sym = declared[0];
            if registry.seen_symbols.contains(sym) {
                return StepOutcome::Unsound(format!(
                    "introduced(definition) declares already-seen symbol `{sym}`"
                ));
            }

            // To prevent definition laundering (evil_definition_false, etc.),
            // we must structurally validate that the formula is a valid naming fragment.
            // A valid naming fragment is a clause containing the fresh symbol.
            let logical = match &step_fof.formula {
                FOFStatement::Logical(f) => f,
                FOFStatement::Sequent(..) => {
                    return StepOutcome::Unknown("Sequent not supported".into());
                }
            };

            let (_, body) = collect_forall(&logical);
            if is_naming_clause(body, sym) {
                return StepOutcome::Sound;
            }

            // If it's not a naming clause, fall through to try_skolem_axiom and try_distinctness_axiom
        } else if declared.len() > 1 {
            let all_fresh = declared.iter().all(|s| !registry.seen_symbols.contains(*s));
            if !all_fresh {
                return StepOutcome::Unsound(
                    "introduced(definition) declares multiple new symbols and at \
                     least one is already known"
                        .into(),
                );
            }

            let logical = match &step_fof.formula {
                FOFStatement::Logical(f) => f,
                FOFStatement::Sequent(..) => {
                    return StepOutcome::Unknown("Sequent not supported".into());
                }
            };

            let (_, body) = collect_forall(&logical);
            let all_valid = declared.iter().all(|&sym| is_naming_clause(body, sym));
            if all_valid {
                return StepOutcome::Sound;
            }
        }
    }

    let logical = match &step_fof.formula {
        FOFStatement::Logical(f) => f,
        FOFStatement::Sequent(..) => {
            return StepOutcome::Unknown(
                "introduced(definition) on a sequent — unhandled shape".into(),
            );
        }
    };

    // --- Vampire `skolem_symbol_introduction`: a Skolem axiom. ---
    //
    // Vampire emits these with the body
    //   ! [params] : ((? [X] : phi(X, params)) => phi(sk(params), params))
    // and an empty info list (no `new_symbols` entry). The witness symbol
    // `sk` appears in the consequent but is absent from the antecedent.
    // It is a sound conservative extension iff `sk` is fresh — i.e. does
    // not appear in any earlier step of the proof or the linked problem.
    //
    // We detect the shape structurally rather than by label: any body of
    // the form `(∃ X. φ) → ψ` whose ψ introduces at least one function
    // symbol absent from φ is accepted, provided every such new symbol
    // is fresh in the registry. This is strictly sound (the symbol is
    // fresh, so the formula is satisfiable in any model that interprets
    // it as a choice function) and admits the slight generalisations
    // where the implication is wrapped in additional universal binders
    // or where φ/ψ are themselves quantified.
    if let Some(outcome) = try_skolem_axiom(&logical, registry) {
        return outcome;
    }

    // --- Distinctness axiom between distinct $distinct_object constants. ---
    //
    // Vampire emits this as `introduced(definition,[],[distinctness_axiom])`
    // with a body like `"Apple" != "Microsoft"`. TPTP semantics
    // (Sutcliffe, JAR 2009 §3.1) guarantees that any two syntactically
    // distinct double-quoted constants are pairwise unequal, so the
    // inequality is a tautology requiring no further justification.
    //
    // We accept any body that, after peeling parens/quantifiers, is an
    // `Inequality(DistinctObject(a), DistinctObject(b))` with `a != b`.
    // Anything else — equality, inequality of plain terms, etc. — is
    // rejected and falls through to the iff check below.
    if let Some(outcome) = try_distinctness_axiom(&logical) {
        return outcome;
    }

    // --- E shape: parse the formula for a biconditional with a fresh head. ---
    let (universals, body) = collect_forall(&logical);
    let (left, right) = match body {
        FOFFormula::Binary {
            left,
            connective: BinaryConnective::Iff,
            right,
        } => (peel(left), peel(right)),
        _ => {
            return StepOutcome::Unsound(
                "introduced(definition) with no new_symbols entry and body is \
                 not a biconditional or Skolem axiom after peeling \
                 quantifiers/parens"
                    .into(),
            );
        }
    };

    let mut is_sound = false;
    if let Some((name, args)) = head_predicate_with_args(left)
        && !registry.seen_symbols.contains(name)
        && check_free_vars(right, args, &universals)
    {
        is_sound = true;
    }
    if !is_sound
        && let Some((name, args)) = head_predicate_with_args(right)
        && !registry.seen_symbols.contains(name)
        && check_free_vars(left, args, &universals)
    {
        is_sound = true;
    }

    if is_sound {
        return StepOutcome::Sound;
    }

    StepOutcome::Unsound(
        "introduced(definition) head predicate is not fresh, OR its arguments \
         do not capture all free variables of the body"
            .into(),
    )
}

/// Returns true if the formula is a valid naming clause: a disjunction of literals
/// (or a single literal) where at least one literal's predicate is exactly `sym`.
fn is_naming_clause(f: &FOFFormula<'_>, sym: &str) -> bool {
    fn is_literal_with_sym(f: &FOFFormula<'_>, sym: &str) -> (bool, bool) {
        let peeled = peel_parens(f);
        match peeled {
            FOFFormula::Atomic(FOFAtomicFormula::Plain(
                AtomicWord::Lower(p) | AtomicWord::SingleQuoted(p),
                _,
            )) => (true, *p == sym),
            FOFFormula::Atomic(FOFAtomicFormula::Defined(w, _)) => (true, w.0 == sym),
            FOFFormula::Atomic(FOFAtomicFormula::System(w, _)) => (true, w.0 == sym),
            FOFFormula::Negation(inner) => {
                let inner_peeled = peel_parens(inner);
                match inner_peeled {
                    FOFFormula::Atomic(FOFAtomicFormula::Plain(
                        AtomicWord::Lower(p) | AtomicWord::SingleQuoted(p),
                        _,
                    )) => (true, *p == sym),
                    FOFFormula::Atomic(FOFAtomicFormula::Defined(w, _)) => (true, w.0 == sym),
                    FOFFormula::Atomic(FOFAtomicFormula::System(w, _)) => (true, w.0 == sym),
                    _ => (false, false),
                }
            }
            _ => (false, false),
        }
    }

    let mut stack = vec![peel_parens(f)];
    let mut found_sym = false;

    while let Some(curr) = stack.pop() {
        let peeled = peel_parens(curr);
        match peeled {
            FOFFormula::Binary {
                left,
                connective: BinaryConnective::Or,
                right,
            } => {
                stack.push(left);
                stack.push(right);
            }
            _ => {
                let (is_lit, has_sym) = is_literal_with_sym(peeled, sym);
                if !is_lit {
                    return false; // Not a clause!
                }
                if has_sym {
                    found_sym = true;
                }
            }
        }
    }

    found_sym
}

/// Try to match a Skolem-axiom shape and certify it.
///
/// Returns `Some(Sound)` if accepted, `Some(Unknown(reason))` if the
/// shape matches but the freshness check fails, and `None` if the
/// formula isn't recognisable as a Skolem axiom at all (so the caller
/// should try the next shape).
///
/// A genuine Vampire Skolem axiom has the body
///
/// ```text
/// ! [params] : ( (? [X1..Xk] : phi) => phi[X1:=t1, .., Xk:=tk] )
/// ```
///
/// where each `ti = sk_i(params)` is a fresh Skolem term. Soundness rests
/// on **two** independent conditions, *both* of which are checked here:
///
///  1. **Structural witnessing** — the consequent must be exactly the
///     antecedent matrix `phi` with each existential variable replaced by
///     some witness term (consistent across occurrences). This is what
///     makes `(∃X. phi) → phi[X:=t]` a logical consequence of the choice
///     of `t`. Without this check an adversary can assert e.g.
///     `(∃X. $true) → ($false & bad(sK))`: the antecedent is valid, the
///     consequent is *not* the antecedent matrix, and `sK` being fresh
///     does nothing to rescue the embedded `$false`.
///  2. **Freshness** — every function symbol the consequent introduces
///     over the antecedent (the Skolem witnesses) must be absent from the
///     registry, so the extension is conservative.
fn try_skolem_axiom<'p>(f: &FOFFormula<'p>, registry: &SkolemRegistry) -> Option<StepOutcome> {
    // Free variable check: an introduced Skolem axiom must not contain free variables.
    // In TPTP, free variables are implicitly universally quantified at the top level,
    // which could allow an attacker to bypass explicit arity checks.
    let mut bound = HashSet::new();
    let mut free = HashSet::new();
    free_vars(f, &mut bound, &mut free);
    if !free.is_empty() {
        return Some(StepOutcome::Unsound(format!(
            "introduced(definition) Skolem axiom contains free variables: {:?}",
            free
        )));
    }

    // Peel only universal quantifiers and parens, NOT existentials — the
    // existential is the witness we are looking to Skolemise and must be
    // visible on the left of the implication.
    let (universals, body) = collect_forall(f);
    let (ante, cons) = match body {
        FOFFormula::Binary {
            left,
            connective: BinaryConnective::Impl,
            right,
        } => (peel_parens(left), peel_parens(right)),
        _ => return None,
    };

    // Antecedent must be ∃-quantified (the existential we are Skolemising).
    // Collect the existential variables (the "holes") and the matrix they
    // quantify.
    let (exists_vars, matrix) = collect_existentials(ante)?;

    // Structural witnessing: the consequent must be `matrix` with each
    // existential variable instantiated by a witness term. If it is not,
    // this is not a Skolem axiom we can certify (it might be an outright
    // unsound assertion dressed up as one) — fall through to the other
    // shapes, ending in `Unknown` rather than a spurious `Sound`.
    let mut subst: HashMap<&str, FOFTerm<'_>> = HashMap::new();
    if !match_formula(matrix, cons, &exists_vars, &mut subst) {
        return None;
    }

    // Collect function symbols on each side and find the new ones.
    let mut ante_syms = HashSet::new();
    collect_fun_syms(ante, &mut ante_syms);
    let mut cons_syms = HashSet::new();
    collect_fun_syms(cons, &mut cons_syms);

    let introduced: Vec<&str> = cons_syms.difference(&ante_syms).copied().collect();
    if introduced.is_empty() {
        // Consequent introduces no new function symbol — not a Skolem
        // axiom (or a degenerate one we can't distinguish from the
        // identity rewrite).
        return None;
    }

    // Arity drop check: Every witness term assigned to an existential
    // must contain all universal variables of the axiom.
    for term in subst.values() {
        let mut term_vars = HashSet::new();
        collect_term_vars(term, &mut term_vars);
        for u in &universals {
            if !term_vars.contains(u) {
                return Some(StepOutcome::Unsound(format!(
                    "introduced(definition) Skolem term drops universal variable `{}`",
                    u
                )));
            }
        }
    }

    // Every newly-introduced symbol must be fresh in the registry. If
    // any is already known, refuse (it would mean Vampire reused an
    // earlier symbol as a Skolem witness, which our soundness argument
    // forbids).
    let stale: Vec<&str> = introduced
        .iter()
        .copied()
        .filter(|s| registry.seen_symbols.contains(*s))
        .collect();
    if !stale.is_empty() {
        return Some(StepOutcome::Unsound(format!(
            "introduced(definition) Skolem axiom reuses already-seen symbol(s): {stale:?}"
        )));
    }
    Some(StepOutcome::Sound)
}

/// Peel leading existential quantifiers (and parens), collecting their
/// bound variable names, and return the variable set together with the
/// matrix they quantify. Returns `None` if there is no leading
/// existential (so the formula is not a Skolem-axiom antecedent).
fn collect_existentials<'a, 'p>(
    f: &'a FOFFormula<'p>,
) -> Option<(HashSet<&'p str>, &'a FOFFormula<'p>)> {
    let mut vars: HashSet<&'p str> = HashSet::new();
    let mut cur = f;
    loop {
        match cur {
            FOFFormula::Parens(inner) => cur = inner,
            FOFFormula::Quantified {
                quantifier: Quantifier::Exists,
                variables,
                formula,
            } => {
                for v in variables {
                    vars.insert(*v);
                }
                cur = formula;
            }
            _ => break,
        }
    }
    if vars.is_empty() {
        None
    } else {
        Some((vars, cur))
    }
}

/// Structural match: is `conc` equal to the pattern `pat` with each
/// `exists` variable replaced by a (consistent) witness term?
///
/// Connective/quantifier/predicate structure must correspond exactly.
/// At term positions a pattern variable in `exists` binds to whatever
/// term the consequent has there (and must bind consistently across all
/// its occurrences); any other variable must match an identical variable.
fn match_formula<'p>(
    pat: &FOFFormula<'p>,
    conc: &FOFFormula<'p>,
    exists: &HashSet<&str>,
    subst: &mut HashMap<&'p str, FOFTerm<'p>>,
) -> bool {
    match (pat, conc) {
        (FOFFormula::Parens(a), _) => match_formula(a, conc, exists, subst),
        (_, FOFFormula::Parens(b)) => match_formula(pat, b, exists, subst),
        (FOFFormula::Atomic(a), FOFFormula::Atomic(b)) => match_atomic(a, b, exists, subst),
        (FOFFormula::Negation(a), FOFFormula::Negation(b)) => match_formula(a, b, exists, subst),
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
        ) => q1 == q2 && v1 == v2 && match_formula(f1, f2, exists, subst),
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
            c1 == c2 && match_formula(l1, l2, exists, subst) && match_formula(r1, r2, exists, subst)
        }
        (FOFFormula::Equality(a, b), FOFFormula::Equality(c, d)) => {
            match_term(a, c, exists, subst) && match_term(b, d, exists, subst)
        }
        (FOFFormula::Inequality(a, b), FOFFormula::Inequality(c, d)) => {
            match_term(a, c, exists, subst) && match_term(b, d, exists, subst)
        }
        _ => false,
    }
}

fn match_atomic<'p>(
    pat: &FOFAtomicFormula<'p>,
    conc: &FOFAtomicFormula<'p>,
    exists: &HashSet<&str>,
    subst: &mut HashMap<&'p str, FOFTerm<'p>>,
) -> bool {
    use FOFAtomicFormula::*;
    match (pat, conc) {
        (Plain(n1, a1), Plain(n2, a2)) => n1 == n2 && match_term_lists(a1, a2, exists, subst),
        (Defined(n1, a1), Defined(n2, a2)) => n1 == n2 && match_term_lists(a1, a2, exists, subst),
        (System(n1, a1), System(n2, a2)) => n1 == n2 && match_term_lists(a1, a2, exists, subst),
        (True, True) | (False, False) => true,
        _ => false,
    }
}

fn match_term_lists<'p>(
    pat: &[FOFTerm<'p>],
    conc: &[FOFTerm<'p>],
    exists: &HashSet<&str>,
    subst: &mut HashMap<&'p str, FOFTerm<'p>>,
) -> bool {
    pat.len() == conc.len()
        && pat
            .iter()
            .zip(conc)
            .all(|(a, b)| match_term(a, b, exists, subst))
}

fn match_term<'p>(
    pat: &FOFTerm<'p>,
    conc: &FOFTerm<'p>,
    exists: &HashSet<&str>,
    subst: &mut HashMap<&'p str, FOFTerm<'p>>,
) -> bool {
    match pat {
        // Existential variable: binds to the witness term (consistently).
        FOFTerm::Variable(v) if exists.contains(v) => match subst.get(v) {
            Some(prev) => prev == conc,
            None => {
                subst.insert(v, conc.clone());
                true
            }
        },
        // Any other variable (e.g. an outer universal `param`): must be
        // the identical variable in the consequent.
        FOFTerm::Variable(v) => matches!(conc, FOFTerm::Variable(w) if w == v),
        FOFTerm::Function(n, args) => match conc {
            FOFTerm::Function(n2, a2) => n == n2 && match_term_lists(args, a2, exists, subst),
            _ => false,
        },
        FOFTerm::DefinedFunction(n, args) => match conc {
            FOFTerm::DefinedFunction(n2, a2) => {
                n == n2 && match_term_lists(args, a2, exists, subst)
            }
            _ => false,
        },
        FOFTerm::SystemFunction(n, args) => match conc {
            FOFTerm::SystemFunction(n2, a2) => n == n2 && match_term_lists(args, a2, exists, subst),
            _ => false,
        },
        FOFTerm::Number(_) | FOFTerm::DistinctObject(_) => pat == conc,
    }
}

pub(crate) fn free_vars<'a>(f: &FOFFormula<'a>, bound: &mut HashSet<&'a str>, free: &mut HashSet<&'a str>) {
    match f {
        FOFFormula::Atomic(a) => match a {
            FOFAtomicFormula::Plain(_, args)
            | FOFAtomicFormula::Defined(_, args)
            | FOFAtomicFormula::System(_, args) => {
                for t in args {
                    free_vars_term(t, bound, free);
                }
            }
            FOFAtomicFormula::True | FOFAtomicFormula::False => {}
        },
        FOFFormula::Negation(inner) | FOFFormula::Parens(inner) => {
            free_vars(inner, bound, free);
        }
        FOFFormula::Binary { left, right, .. } => {
            free_vars(left, bound, free);
            free_vars(right, bound, free);
        }
        FOFFormula::Equality(l, r) | FOFFormula::Inequality(l, r) => {
            free_vars_term(l, bound, free);
            free_vars_term(r, bound, free);
        }
        FOFFormula::Quantified {
            variables, formula, ..
        } => {
            let mut newly_bound = Vec::new();
            for v in variables {
                if bound.insert(*v) {
                    newly_bound.push(*v);
                }
            }
            free_vars(formula, bound, free);
            for v in newly_bound {
                bound.remove(v);
            }
        }
    }
}

fn free_vars_term<'a>(t: &FOFTerm<'a>, bound: &HashSet<&'a str>, free: &mut HashSet<&'a str>) {
    match t {
        FOFTerm::Variable(v) => {
            if !bound.contains(v) {
                free.insert(*v);
            }
        }
        FOFTerm::Function(_, args)
        | FOFTerm::DefinedFunction(_, args)
        | FOFTerm::SystemFunction(_, args) => {
            for a in args {
                free_vars_term(a, bound, free);
            }
        }
        FOFTerm::Number(_) | FOFTerm::DistinctObject(_) => {}
    }
}

pub(crate) fn collect_term_vars<'a>(t: &FOFTerm<'a>, out: &mut HashSet<&'a str>) {
    match t {
        FOFTerm::Variable(v) => {
            out.insert(*v);
        }
        FOFTerm::Function(_, args)
        | FOFTerm::DefinedFunction(_, args)
        | FOFTerm::SystemFunction(_, args) => {
            for a in args {
                collect_term_vars(a, out);
            }
        }
        FOFTerm::Number(_) | FOFTerm::DistinctObject(_) => {}
    }
}

fn collect_forall<'a, 'p>(f: &'a FOFFormula<'p>) -> (HashSet<&'p str>, &'a FOFFormula<'p>) {
    let mut vars = HashSet::new();
    let mut cur = f;
    loop {
        match cur {
            FOFFormula::Parens(inner) => cur = inner,
            FOFFormula::Quantified {
                quantifier: Quantifier::Forall,
                variables,
                formula,
            } => {
                for v in variables {
                    vars.insert(*v);
                }
                cur = formula;
            }
            _ => break,
        }
    }
    (vars, cur)
}

fn check_free_vars<'a>(
    body: &FOFFormula<'a>,
    head_args: &[FOFTerm<'a>],
    _universals: &HashSet<&'a str>,
) -> bool {
    let mut bound = HashSet::new();
    let mut free = HashSet::new();
    free_vars(body, &mut bound, &mut free);

    let mut head_vars = HashSet::new();
    for a in head_args {
        collect_term_vars(a, &mut head_vars);
    }

    for v in free {
        if !head_vars.contains(v) {
            return false;
        }
    }
    true
}

fn head_predicate_with_args<'a>(f: &'a FOFFormula<'a>) -> Option<(&'a str, &'a [FOFTerm<'a>])> {
    let stripped = match f {
        FOFFormula::Negation(inner) => peel(inner),
        _ => f,
    };
    match stripped {
        FOFFormula::Atomic(FOFAtomicFormula::Plain(w, args)) => Some((w.as_str(), args)),
        _ => None,
    }
}

/// Collect all function symbol names appearing in a formula.
pub(crate) fn collect_fun_syms<'a>(f: &FOFFormula<'a>, out: &mut HashSet<&'a str>) {
    match f {
        FOFFormula::Atomic(a) => collect_fun_syms_atomic(a, out),
        FOFFormula::Negation(inner) | FOFFormula::Parens(inner) => collect_fun_syms(inner, out),
        FOFFormula::Quantified { formula, .. } => collect_fun_syms(formula, out),
        FOFFormula::Binary { left, right, .. } => {
            collect_fun_syms(left, out);
            collect_fun_syms(right, out);
        }
        FOFFormula::Equality(a, b) | FOFFormula::Inequality(a, b) => {
            collect_fun_syms_term(a, out);
            collect_fun_syms_term(b, out);
        }
    }
}

/// If `f` is (after peeling quantifiers/parens) an inequality between two
/// syntactically distinct `$distinct_object` constants, return
/// `Some(Sound)`. TPTP semantics treats distinct double-quoted constants
/// as pairwise unequal, so the inequality is a tautology and the
/// `introduced(definition,[],[distinctness_axiom])` step is sound.
///
/// Returns `None` on any other shape so the caller can fall through to
/// the iff-style structural check.
fn try_distinctness_axiom<'p>(f: &FOFFormula<'p>) -> Option<StepOutcome> {
    let body = peel(f);
    let (lhs, rhs) = match body {
        FOFFormula::Inequality(l, r) => (l, r),
        _ => return None,
    };
    match (lhs, rhs) {
        (FOFTerm::DistinctObject(a), FOFTerm::DistinctObject(b)) if a != b => {
            Some(StepOutcome::Sound)
        }
        _ => None,
    }
}

fn collect_fun_syms_atomic<'a>(a: &mrs_tptp::FOFAtomicFormula<'a>, out: &mut HashSet<&'a str>) {
    use mrs_tptp::FOFAtomicFormula::*;
    match a {
        Plain(_pred, args) => {
            // Predicates are not function symbols for our purposes — only
            // their arguments are. (A "Skolem function" we care about is
            // one that appears as a function term in the consequent but
            // not the antecedent.)
            for t in args {
                collect_fun_syms_term(t, out);
            }
        }
        Defined(_, args) | System(_, args) => {
            for t in args {
                collect_fun_syms_term(t, out);
            }
        }
        True | False => {}
    }
}

fn collect_fun_syms_term<'a>(t: &mrs_tptp::FOFTerm<'a>, out: &mut HashSet<&'a str>) {
    use mrs_tptp::FOFTerm::*;
    match t {
        Function(name, args) => {
            out.insert(name.as_str());
            for a in args {
                collect_fun_syms_term(a, out);
            }
        }
        DefinedFunction(_, args) | SystemFunction(_, args) => {
            for a in args {
                collect_fun_syms_term(a, out);
            }
        }
        Variable(_) | Number(_) | DistinctObject(_) => {}
    }
}

/// Peel leading universal quantifiers and `(…)` wrappers, yielding the
/// innermost formula. Returns a reference into the input AST.
fn peel<'a, 'p>(f: &'a FOFFormula<'p>) -> &'a FOFFormula<'p> {
    let mut cur = f;
    loop {
        match cur {
            FOFFormula::Parens(inner) => cur = inner,
            FOFFormula::Quantified { formula, .. } => cur = formula,
            _ => return cur,
        }
    }
}

/// Peel only `(…)` wrappers; leave quantifiers in place.
fn peel_parens<'a, 'p>(f: &'a FOFFormula<'p>) -> &'a FOFFormula<'p> {
    let mut cur = f;
    while let FOFFormula::Parens(inner) = cur {
        cur = inner;
    }
    cur
}

/// Extract `new_symbols(kind, [s1, s2, …])` symbol names from an
/// `introduced(definition, [info_items], [extras])` annotation source.
///
/// Unlike [`Annotations::new_symbols`], which only looks inside
/// `inference(/3)`, this walks the `introduced(/N)` info list directly.
/// Returns the symbol names declared in the
/// `introduced(definition, [new_symbols(naming, [...])], ...)` annotation,
/// or an empty vector if no such entry is present. Useful for callers
/// that need to identify the freshly-defined symbol(s) (e.g. to extend a
/// one-directional `predicate_definition_introduction` premise into its
/// biconditional closure before handing it to an ATP).
pub(crate) fn declared_new_symbols<'a>(ann: &Annotations<'a>) -> Vec<&'a str> {
    let info = match &ann.source {
        GeneralTerm::Function(AtomicWord::Lower("introduced"), args) if args.len() >= 2 => {
            match &args[1] {
                GeneralTerm::List(items) => items.as_slice(),
                _ => return Vec::new(),
            }
        }
        _ => return Vec::new(),
    };
    for it in info {
        if let GeneralTerm::Function(AtomicWord::Lower("new_symbols"), inner) = it
            && inner.len() == 2
            && let GeneralTerm::List(items) = &inner[1]
        {
            return items
                .iter()
                .filter_map(|g| match g {
                    GeneralTerm::Word(AtomicWord::Lower(s)) => Some(*s),
                    GeneralTerm::Word(AtomicWord::SingleQuoted(s)) => Some(*s),
                    _ => None,
                })
                .collect();
        }
    }
    Vec::new()
}

/// As [`declared_new_symbols`], but accepts an optional annotation and
/// returns an empty vector when absent. A non-empty result means the
/// step introduces fresh symbol(s) (E `predicate_definition_introduction`
/// or Vampire `avatar_definition`), which marks it as a *definition*
/// rather than a source/original formula.
pub(crate) fn declared_new_symbols_opt<'a>(ann: Option<&Annotations<'a>>) -> Vec<&'a str> {
    match ann {
        Some(a) => declared_new_symbols(a),
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_tptp::parse_tptp;

    fn first_fof<'p>(input: &'p str) -> &'p FOFAnnotated<'p> {
        let problem = Box::leak(Box::new(parse_tptp(input).expect("parse")));
        match &problem.formulas[0] {
            mrs_tptp::AnnotatedFormula::FOF(f) => f,
            _ => panic!("expected FOF"),
        }
    }

    #[test]
    fn detects_source_keyword() {
        let af = first_fof("fof(c1, plain, (p0 <=> q), introduced(definition)).");
        let ann = af.annotations.as_ref().unwrap();
        assert!(is_introduced_definition(ann));
    }

    #[test]
    fn detects_source_with_extras() {
        let af = first_fof(
            "fof(c1, plain, (q | ~sP2), \
             introduced(definition,[new_symbols(naming,[sP2])],\
                                   [predicate_definition_introduction])).",
        );
        let ann = af.annotations.as_ref().unwrap();
        assert!(is_introduced_definition(ann));
    }

    #[test]
    fn rejects_non_definition_source() {
        let af = first_fof("fof(c1, plain, (p0 <=> q), inference(rw, [status(thm)], [a])).");
        let ann = af.annotations.as_ref().unwrap();
        assert!(!is_introduced_definition(ann));
    }

    #[test]
    fn accepts_fresh_nullary_definition() {
        let af = first_fof("fof(c1, plain, (epred1_0 <=> q), introduced(definition)).");
        let reg = SkolemRegistry::new();
        assert!(matches!(check(af, &reg), StepOutcome::Sound));
    }

    #[test]
    fn accepts_fresh_negated_head_definition() {
        let af = first_fof("fof(c1, plain, (~epred1_0 <=> q), introduced(definition)).");
        let reg = SkolemRegistry::new();
        assert!(matches!(check(af, &reg), StepOutcome::Sound));
    }

    #[test]
    fn accepts_fresh_quantified_definition() {
        let af = first_fof("fof(c1, plain, (! [X] : (p(X) <=> q(X))), introduced(definition)).");
        let reg = SkolemRegistry::new();
        assert!(matches!(check(af, &reg), StepOutcome::Sound));
    }

    #[test]
    fn accepts_distinctness_between_distinct_objects() {
        // Vampire emits `"Apple" != "Microsoft"` as a tautology under
        // TPTP's distinct-object semantics; see SYO561+1 in the corpus.
        let af = first_fof(
            "fof(f3, plain, (\"Apple\" != \"Microsoft\"), \
             introduced(definition,[],[distinctness_axiom])).",
        );
        let reg = SkolemRegistry::new();
        assert!(matches!(check(af, &reg), StepOutcome::Sound));
    }

    #[test]
    fn rejects_distinctness_between_equal_objects() {
        // Same literal twice is not a tautology — defer to ATPs.
        let af = first_fof(
            "fof(f3, plain, (\"Apple\" != \"Apple\"), \
             introduced(definition,[],[distinctness_axiom])).",
        );
        let reg = SkolemRegistry::new();
        assert!(matches!(check(af, &reg), StepOutcome::Unsound(_)));
    }

    #[test]
    fn rejects_distinctness_between_plain_terms() {
        // Plain function symbols are not distinct objects; defer.
        let af =
            first_fof("fof(f3, plain, (a != b), introduced(definition,[],[distinctness_axiom])).");
        let reg = SkolemRegistry::new();
        assert!(matches!(check(af, &reg), StepOutcome::Unsound(_)));
    }

    #[test]
    fn rejects_non_fresh_predicate() {
        let af = first_fof("fof(c1, plain, (p <=> q), introduced(definition)).");
        let mut reg = SkolemRegistry::new();
        reg.record("p"); // already seen
        reg.record("q"); // also seen → neither side qualifies
        assert!(matches!(check(af, &reg), StepOutcome::Unsound(_)));
    }

    #[test]
    fn accepts_when_either_side_is_fresh() {
        let af = first_fof("fof(c1, plain, (q <=> epred1_0), introduced(definition)).");
        let mut reg = SkolemRegistry::new();
        reg.record("q"); // q is known; epred1_0 is the fresh head
        assert!(matches!(check(af, &reg), StepOutcome::Sound));
    }

    #[test]
    fn rejects_non_biconditional_without_new_symbols() {
        let af = first_fof("fof(c1, plain, (epred1_0 => q), introduced(definition)).");
        let reg = SkolemRegistry::new();
        assert!(matches!(check(af, &reg), StepOutcome::Unsound(_)));
    }

    #[test]
    fn accepts_vampire_directional_fragment_via_new_symbols() {
        // The body is NOT a biconditional — it's `phi | ~sP2`, one of the
        // two clausal halves of `sP2 <=> phi`. Acceptance comes from the
        // declared `new_symbols(naming, [sP2])` entry.
        let af = first_fof(
            "fof(f18, plain, (q | ~sP2), \
             introduced(definition,[new_symbols(naming,[sP2])],\
                                   [predicate_definition_introduction])).",
        );
        let reg = SkolemRegistry::new();
        assert!(matches!(check(af, &reg), StepOutcome::Sound));
    }

    #[test]
    fn rejects_vampire_fragment_with_known_new_symbol() {
        let af = first_fof(
            "fof(f18, plain, (q | ~sP2), \
             introduced(definition,[new_symbols(naming,[sP2])],\
                                   [predicate_definition_introduction])).",
        );
        let mut reg = SkolemRegistry::new();
        reg.record("sP2");
        assert!(matches!(check(af, &reg), StepOutcome::Unsound(_)));
    }

    // --- Vampire skolem_symbol_introduction (Skolem axiom shape) ---

    #[test]
    fn accepts_parametric_skolem_axiom() {
        // Body: ! [X0] : ((? [X1] : (op2(X1,X1) != X0 & sorti2(X1)))
        //                  => (op2(sK1(X0),sK1(X0)) != X0 & sorti2(sK1(X0))))
        let af = first_fof(
            "fof(f15, plain, \
             ( ! [X0] : ( ( ? [X1] : (op2(X1,X1) != X0 & sorti2(X1)) ) \
                          => (op2(sK1(X0),sK1(X0)) != X0 & sorti2(sK1(X0))) ) ), \
             introduced(definition,[],[skolem_symbol_introduction])).",
        );
        let reg = SkolemRegistry::new();
        assert!(matches!(check(af, &reg), StepOutcome::Sound));
    }

    #[test]
    fn accepts_nullary_skolem_axiom() {
        // Body: (? [X0] : (sorti2(X0) & p(X0))) => (sorti2(sK1) & p(sK1))
        let af = first_fof(
            "fof(f16, plain, \
             ( ( ? [X0] : (sorti2(X0) & p(X0)) ) => (sorti2(sK1) & p(sK1)) ), \
             introduced(definition,[],[skolem_symbol_introduction])).",
        );
        let reg = SkolemRegistry::new();
        assert!(matches!(check(af, &reg), StepOutcome::Sound));
    }

    #[test]
    fn rejects_skolem_axiom_reusing_known_symbol() {
        let af = first_fof(
            "fof(f16, plain, \
             ( ( ? [X0] : (sorti2(X0) & p(X0)) ) => (sorti2(sK1) & p(sK1)) ), \
             introduced(definition,[],[skolem_symbol_introduction])).",
        );
        let mut reg = SkolemRegistry::new();
        reg.record("sK1"); // already seen elsewhere → must not reuse
        assert!(matches!(check(af, &reg), StepOutcome::Unsound(_)));
    }

    #[test]
    fn rejects_implication_without_existential_antecedent() {
        // Not a Skolem axiom — antecedent is a plain conjunction.
        let af = first_fof("fof(c1, plain, ((p & q) => r(sK1)), introduced(definition)).");
        let reg = SkolemRegistry::new();
        assert!(matches!(check(af, &reg), StepOutcome::Unsound(_)));
    }

    #[test]
    fn rejects_skolem_axiom_with_no_new_symbol() {
        // Implication with ∃ antecedent but consequent introduces no
        // new function symbol — refuse (we can't see the witness).
        let af = first_fof(
            "fof(c1, plain, \
             ( ( ? [X] : p(X) ) => p(c0) ), \
             introduced(definition,[],[skolem_symbol_introduction])).",
        );
        let mut reg = SkolemRegistry::new();
        reg.record("c0"); // c0 already known → consequent has no new sym
        assert!(matches!(check(af, &reg), StepOutcome::Unsound(_)));
    }

    // --- Adversarial shapes (evil-proofs corpus) -----------------------

    #[test]
    fn rejects_skolem_injection_unsound_consequent() {
        // `(? [X] : $true) => ($false & bad_sym(sK))`: the antecedent is
        // valid and `sK` is fresh, but the consequent is NOT the
        // antecedent matrix (`$true`) witnessed — it embeds `$false`, so
        // the implication is unsound. The structural-witnessing check must
        // refuse to certify it. (evil-proofs/skolem_injection)
        let af = first_fof(
            "fof(s2, plain, \
             ( ( ? [X] : $true ) => ( $false & bad_sym(sK) ) ), \
             introduced(definition,[],[skolem_symbol_introduction])).",
        );
        let reg = SkolemRegistry::new();
        assert!(
            matches!(check(af, &reg), StepOutcome::Unsound(_)),
            "must NOT certify an unsound disguised Skolem axiom"
        );
    }

    #[test]
    fn rejects_skolem_axiom_with_altered_consequent() {
        // Antecedent matrix `p(X) & q(X)` but consequent `p(sK) & r(sK)`
        // (q swapped for r): the consequent is not the witnessed matrix,
        // so it must be rejected even though sK is fresh.
        let af = first_fof(
            "fof(s2, plain, \
             ( ( ? [X] : (p(X) & q(X)) ) => (p(sK) & r(sK)) ), \
             introduced(definition,[],[skolem_symbol_introduction])).",
        );
        let reg = SkolemRegistry::new();
        assert!(matches!(check(af, &reg), StepOutcome::Unsound(_)));
    }

    #[test]
    #[test]
    fn rejects_skolem_axiom_inconsistent_witness() {
        // Same existential variable witnessed by two *different* terms in
        // the consequent (`sK1` vs `sK2`): not a valid Skolemisation.
        let af = first_fof(
            "fof(s2, plain, \
             ( ( ? [X] : (p(X) & q(X)) ) => (p(sK1) & q(sK2)) ), \
             introduced(definition,[],[skolem_symbol_introduction])).",
        );
        let reg = SkolemRegistry::new();
        assert!(matches!(check(af, &reg), StepOutcome::Unsound(_)));
    }
}

pub fn check_cycles(dag: &crate::dag::Dag<'_>) -> Result<(), String> {
    use std::collections::{HashMap, HashSet};

    let mut defs: HashMap<&str, HashSet<&str>> = HashMap::new();

    // Collect all introduced definitions and the symbols they define.
    for node in &dag.nodes {
        if let Some(ann) = node.formula.annotations() {
            if is_introduced_definition(ann) {
                let declared = declared_new_symbols(ann);
                let step_fof = match node.formula.as_fof() {
                    Some(f) => f,
                    None => continue,
                };
                let mut body_syms = HashSet::new();
                if let FOFStatement::Logical(form) = &step_fof.formula {
                    collect_fun_syms(form, &mut body_syms);
                }
                // Also collect predicate symbols for the dependency graph
                collect_pred_syms(&step_fof.formula, &mut body_syms);

                for d in declared {
                    let mut deps = body_syms.clone();
                    deps.remove(d);
                    defs.insert(d, deps);
                }
            }
        }
    }

    // DFS for cycle detection
    #[derive(PartialEq)]
    enum Mark {
        Visiting,
        Done,
    }
    let mut marks: HashMap<&str, Mark> = HashMap::new();

    fn dfs<'a>(
        v: &'a str,
        defs: &HashMap<&'a str, HashSet<&'a str>>,
        marks: &mut HashMap<&'a str, Mark>,
    ) -> Result<(), String> {
        match marks.get(v) {
            Some(Mark::Visiting) => return Err(format!("cyclic definition detected involving `{}`", v)),
            Some(Mark::Done) => return Ok(()),
            None => {}
        }
        marks.insert(v, Mark::Visiting);
        if let Some(deps) = defs.get(v) {
            for &dep in deps {
                dfs(dep, defs, marks)?;
            }
        }
        marks.insert(v, Mark::Done);
        Ok(())
    }

    for &v in defs.keys() {
        dfs(v, &defs, &mut marks)?;
    }

    Ok(())
}

fn collect_pred_syms<'a>(f: &FOFStatement<'a>, out: &mut HashSet<&'a str>) {
    if let FOFStatement::Logical(form) = f {
        collect_pred_syms_formula(form, out);
    }
}

fn collect_pred_syms_formula<'a>(f: &FOFFormula<'a>, out: &mut HashSet<&'a str>) {
    match f {
        FOFFormula::Atomic(a) => {
            match a {
                FOFAtomicFormula::Plain(w, _) => {
                    out.insert(w.as_str());
                }
                FOFAtomicFormula::Defined(w, _) => {
                    out.insert(w.0);
                }
                FOFAtomicFormula::System(w, _) => {
                    out.insert(w.0);
                }
                _ => {}
            }
        }
        FOFFormula::Negation(inner) | FOFFormula::Parens(inner) => collect_pred_syms_formula(inner, out),
        FOFFormula::Quantified { formula, .. } => collect_pred_syms_formula(formula, out),
        FOFFormula::Binary { left, right, .. } => {
            collect_pred_syms_formula(left, out);
            collect_pred_syms_formula(right, out);
        }
        FOFFormula::Equality(_, _) | FOFFormula::Inequality(_, _) => {}
    }
}
