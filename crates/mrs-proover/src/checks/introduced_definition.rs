//! Structural check for `introduced(definition)` clauses.
//!
//! E (and a few other ATPs) introduce auxiliary predicate symbols to keep
//! intermediate clauses compact. Such a step has source `introduced(definition)`
//! and asserts a biconditional of the form
//!
//! ```text
//! ! [X1, ..., Xn] : ( P(t1, ..., tk)  <=>  phi )
//! ```
//!
//! or — with the negation pushed to the head side —
//!
//! ```text
//! ! [X1, ..., Xn] : ( ~P(t1, ..., tk)  <=>  phi )
//! ```
//!
//! or (most commonly with a fresh nullary predicate) the unquantified
//!
//! ```text
//! P  <=>  phi      /     ~P  <=>  phi
//! ```
//!
//! A predicate definition `P :⇔ φ` is a *conservative extension* whenever
//! `P` is fresh — i.e. does not occur in any earlier proof node or in the
//! linked problem. Conservativity means no new theorem about the original
//! signature is derivable, so the introduction is sound by construction and
//! does not need to be checked against an ATP.
//!
//! Soundness requirements implemented here:
//!
//! 1. Source is `introduced(definition)` (or `introduced(definition, _)`).
//! 2. The formula has the structural shape above.
//! 3. The "head" predicate symbol `P` is *not* already in the
//!    [`SkolemRegistry`] (which tracks every symbol seen so far). The
//!    registry is updated by the main verify loop after each node, so
//!    "earlier" here means "in any prior node or the linked problem".
//!
//! Anything else falls through to the ATP path.

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
/// [`StepOutcome::Sound`] iff the formula is a biconditional whose head
/// predicate is fresh w.r.t. the registry; otherwise an outcome that
/// surfaces the reason for rejection.
///
/// The caller is responsible for invoking this only when
/// [`is_introduced_definition`] returns true for the step's annotation.
pub fn check<'p>(step: &FOFAnnotated<'p>, registry: &SkolemRegistry) -> StepOutcome {
    let logical = match &step.formula {
        FOFStatement::Logical(f) => f,
        FOFStatement::Sequent(..) => {
            return StepOutcome::Unknown(
                "introduced(definition) on a sequent — unhandled shape".into(),
            );
        }
    };

    // Peel off a leading universal quantifier (and any number of nested
    // parentheses) to expose the biconditional.
    let body = peel(logical);

    let (left, right) = match body {
        FOFFormula::Binary {
            left,
            connective: BinaryConnective::Iff,
            right,
        } => (peel(left), peel(right)),
        _ => {
            return StepOutcome::Unknown(
                "introduced(definition) is not a biconditional after peeling \
                 leading quantifiers/parens"
                    .into(),
            );
        }
    };

    // Either side may be the "head" side carrying the fresh predicate
    // symbol. Try both. A side is a candidate head iff it is an atomic
    // predicate application (or its single negation).
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
        // q must also be considered for freshness; mark it too so neither side
        // qualifies as the fresh head.
        reg.record("q");
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
    fn rejects_non_biconditional() {
        let af = first_fof("fof(c1, plain, (epred1_0 => q), introduced(definition)).");
        let reg = SkolemRegistry::new();
        assert!(matches!(check(af, &reg), StepOutcome::Unknown(_)));
    }
}
