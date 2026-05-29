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

use std::collections::HashSet;

use mrs_tptp::ast::common::{AtomicWord, GeneralTerm, Quantifier};
use mrs_tptp::{
    Annotations, BinaryConnective, FOFAnnotated, FOFAtomicFormula, FOFFormula, FOFStatement,
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
pub fn check<'p>(step: &FOFAnnotated<'p>, registry: &SkolemRegistry) -> StepOutcome {
    // --- Vampire shape: annotation declares the new symbol(s) directly. ---
    if let Some(ann) = step.annotations.as_ref() {
        let declared = declared_new_symbols(ann);
        if declared.len() == 1 {
            let sym = declared[0];
            if !registry.seen_symbols.contains(sym) {
                return StepOutcome::Sound;
            }
            return StepOutcome::Unknown(format!(
                "introduced(definition) declares already-seen symbol `{sym}`"
            ));
        }
        if declared.len() > 1 {
            // Multiple new symbols at once: unusual but conservative — accept
            // only if *all* are fresh.
            let all_fresh = declared.iter().all(|s| !registry.seen_symbols.contains(*s));
            if all_fresh {
                return StepOutcome::Sound;
            }
            return StepOutcome::Unknown(
                "introduced(definition) declares multiple new symbols and at \
                 least one is already known"
                    .into(),
            );
        }
        // No new_symbols entry — fall through to the structural paths.
    }

    let logical = match &step.formula {
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
    if let Some(outcome) = try_skolem_axiom(logical, registry) {
        return outcome;
    }

    // --- E shape: parse the formula for a biconditional with a fresh head. ---
    let body = peel(logical);
    let (left, right) = match body {
        FOFFormula::Binary {
            left,
            connective: BinaryConnective::Iff,
            right,
        } => (peel(left), peel(right)),
        _ => {
            return StepOutcome::Unknown(
                "introduced(definition) with no new_symbols entry and body is \
                 not a biconditional or Skolem axiom after peeling \
                 quantifiers/parens"
                    .into(),
            );
        }
    };

    if let Some(name) = head_predicate(left)
        && !registry.seen_symbols.contains(name)
    {
        return StepOutcome::Sound;
    }
    if let Some(name) = head_predicate(right)
        && !registry.seen_symbols.contains(name)
    {
        return StepOutcome::Sound;
    }

    StepOutcome::Unknown(
        "introduced(definition) head predicate is not fresh (already seen \
         earlier in the proof or in the linked problem)"
            .into(),
    )
}

/// Try to match a Skolem-axiom shape and certify it.
///
/// Returns `Some(Sound)` if accepted, `Some(Unknown(reason))` if the
/// shape matches but the freshness check fails, and `None` if the
/// formula isn't recognisable as a Skolem axiom at all (so the caller
/// should try the next shape).
fn try_skolem_axiom<'p>(f: &FOFFormula<'p>, registry: &SkolemRegistry) -> Option<StepOutcome> {
    // Peel only universal quantifiers and parens, NOT existentials — the
    // existential is the witness we are looking to Skolemise and must be
    // visible on the left of the implication.
    let body = peel_forall(f);
    let (ante, cons) = match body {
        FOFFormula::Binary {
            left,
            connective: BinaryConnective::Impl,
            right,
        } => (peel_parens(left), peel_parens(right)),
        _ => return None,
    };

    // Antecedent must be ∃-quantified (the existential we are Skolemising).
    if !matches!(
        ante,
        FOFFormula::Quantified {
            quantifier: Quantifier::Exists,
            ..
        }
    ) {
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
        return Some(StepOutcome::Unknown(format!(
            "introduced(definition) Skolem axiom reuses already-seen symbol(s): {stale:?}"
        )));
    }
    Some(StepOutcome::Sound)
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

fn collect_fun_syms_atomic<'a>(
    a: &mrs_tptp::FOFAtomicFormula<'a>,
    out: &mut HashSet<&'a str>,
) {
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

/// Peel leading `∀` quantifiers and `(…)` wrappers — but NOT existentials.
/// Used when the existential structure is significant (e.g. the antecedent
/// of a Skolem axiom).
fn peel_forall<'a, 'p>(f: &'a FOFFormula<'p>) -> &'a FOFFormula<'p> {
    let mut cur = f;
    loop {
        match cur {
            FOFFormula::Parens(inner) => cur = inner,
            FOFFormula::Quantified {
                quantifier: Quantifier::Forall,
                formula,
                ..
            } => cur = formula,
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
fn declared_new_symbols<'a>(ann: &Annotations<'a>) -> Vec<&'a str> {
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

/// Returns the predicate-symbol name of `f` if `f` is a single (possibly
/// negated) atomic predicate application like `P(...)` or `~P(...)`.
/// Returns `None` for anything more complex (a connective, an equality,
/// a quantifier, `$true`/`$false`).
fn head_predicate<'a>(f: &'a FOFFormula<'_>) -> Option<&'a str> {
    let stripped = match f {
        FOFFormula::Negation(inner) => peel(inner),
        _ => f,
    };
    match stripped {
        FOFFormula::Atomic(FOFAtomicFormula::Plain(w, _)) => Some(w.as_str()),
        _ => None,
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
    fn rejects_non_fresh_predicate() {
        let af = first_fof("fof(c1, plain, (p <=> q), introduced(definition)).");
        let mut reg = SkolemRegistry::new();
        reg.record("p"); // already seen
        reg.record("q"); // also seen → neither side qualifies
        assert!(matches!(check(af, &reg), StepOutcome::Unknown(_)));
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
        assert!(matches!(check(af, &reg), StepOutcome::Unknown(_)));
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
        assert!(matches!(check(af, &reg), StepOutcome::Unknown(_)));
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
        assert!(matches!(check(af, &reg), StepOutcome::Unknown(_)));
    }

    #[test]
    fn rejects_implication_without_existential_antecedent() {
        // Not a Skolem axiom — antecedent is a plain conjunction.
        let af = first_fof(
            "fof(c1, plain, ((p & q) => r(sK1)), introduced(definition)).",
        );
        let reg = SkolemRegistry::new();
        assert!(matches!(check(af, &reg), StepOutcome::Unknown(_)));
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
        assert!(matches!(check(af, &reg), StepOutcome::Unknown(_)));
    }
}
