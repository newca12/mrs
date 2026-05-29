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

use mrs_tptp::ast::common::{AtomicWord, GeneralTerm};
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

/// Verify one `introduced(definition)` step. Returns
/// [`StepOutcome::Sound`] iff:
///
/// * the annotation declares a single new predicate symbol via
///   `new_symbols(naming, [P])` and `P` is fresh (Vampire shape), **or**
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
        // No new_symbols entry — fall through to the E-style biconditional path.
    }

    // --- E shape: parse the formula for a biconditional with a fresh head. ---
    let logical = match &step.formula {
        FOFStatement::Logical(f) => f,
        FOFStatement::Sequent(..) => {
            return StepOutcome::Unknown(
                "introduced(definition) on a sequent — unhandled shape".into(),
            );
        }
    };

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
                 not a biconditional after peeling quantifiers/parens"
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
}
