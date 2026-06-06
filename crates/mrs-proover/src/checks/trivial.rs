//! Structural verifiers for "trivial" preprocessing/clausification rules.
//!
//! Historically `verify.rs` carried a `TRIVIAL_RULES` allow-list that accepted
//! a step as `Sound` *purely because of its inference-rule name*, with no check
//! that the named transformation actually held. That is the single largest
//! soundness liability in an adversarial setting (ProoVer 2026 `evil-proofs`):
//! an attacker controls the rule label, so they can tag a non-equivalence
//! step `[A] ⊢ B` (where `B` does not follow from `A`) with e.g. `fof_nnf`
//! and earn a blind pass — a catastrophic `Verified`-on-bad (−10) outcome.
//!
//! This module replaces *name-trust* with *name-dispatched structural
//! verification*. Each former trivial rule routes to a verifier that returns
//! `Some(StepOutcome::Sound)` **only** on a positive structural proof that the
//! transformation holds, and `None` otherwise (so the caller falls through to
//! the ATP ladder). Acceptance therefore never rests on the attacker-controlled
//! label — it always rests on a checked structural fact.
//!
//! Soundness of the two primitives used here:
//!
//!   * **Equivalence rules** (`fof_nnf`, `nnf_transformation`,
//!     `variable_rename`, `flattening`, `remove_duplicate_literals`, …):
//!     accepted when `canon(nnf(parent))` is α-equivalent to
//!     `canon(nnf(conclusion))`. `to_nnf` preserves logical equivalence; the
//!     `canon` pass only flattens/sorts/dedups associative–commutative–
//!     idempotent `And`/`Or` (all equivalence-preserving); and α-equivalence
//!     implies logical equivalence. So a positive match proves
//!     `parent ≡ conclusion`, hence `parent ⊨ conclusion`. The check is
//!     deliberately *partial* (it does not, e.g., distribute ∨ over ∧); any
//!     miss simply falls through to the ATP, never to an unsound accept.
//!
//!   * **Conjunct projection** (`split_conjunct`): `(A ∧ B) ⊢ A`. Accepted
//!     when, after peeling a shared universal prefix from `nnf(parent)`, the
//!     body is a conjunction one of whose conjuncts (re-wrapped in the prefix)
//!     is α-equivalent to `nnf(conclusion)`. Sound because `∀x.(A ∧ B) ⊨
//!     ∀x.A`.
//!
//! Competition proofs that use ProoVer's own coarse rule names
//! (`consequence`, `instantiate`, `horn`, `deduction`, …) match none of these
//! verifiers and route to the ATP unchanged — this module is a pure soundness
//! win with no behavioural regression on legitimate proofs.

use mrs_cnf::nnf::to_nnf;
use mrs_core::alpha::alpha_equiv;
use mrs_core::{Formula, SymbolTable, VarId};
use mrs_tptp::AnnotatedFormula;

use crate::dag::Node;
use crate::lower::{LowerCtx, lower_annotated_formula};
use crate::verdict::StepOutcome;

/// Single-premise rules whose conclusion the prover claims is *logically
/// equivalent* to the (sole) premise.
const EQUIV_RULES: &[&str] = &[
    "assume_negation", // handled by neg_conjecture too; harmless here for non-conjecture parents
    "rectify",
    "true_and_iff_removal",
    "fof_simplification",
    "trivial_inequality_removal",
    "evaluation",
    "remove_duplicate_literals",
    "fof_nnf",
    "distribute",
    "variable_rename",
    "duplicate_literal_removal",
    "flattening",
    "nnf_transformation",
    "ennf_transformation",
    "cnf_transformation",
];

/// Single-premise rules that project one conjunct out of a conjunction.
const PROJECTION_RULES: &[&str] = &["split_conjunct"];

/// Returns `true` if `rule` is one this module knows how to *attempt*.
/// (It may still fail the structural check and fall through to the ATP.)
pub fn is_trivial_rule(rule: Option<&str>) -> bool {
    matches!(rule, Some(r) if EQUIV_RULES.contains(&r) || PROJECTION_RULES.contains(&r))
}

/// Attempt a structural verification of a former "trivial" step.
///
/// Returns `Some(StepOutcome::Sound)` only when the named transformation is
/// structurally confirmed; `None` to fall through to the ATP. Never returns
/// `Unsound`: a failed structural match is not positive evidence of a faulty
/// step (it may just be outside this module's partial decision power), so we
/// stay conservative and defer to the entailment checker.
pub fn try_check<'p>(
    node: &Node<'p>,
    parents: &[&AnnotatedFormula<'p>],
    symbols: &mut SymbolTable,
) -> Option<StepOutcome> {
    let rule = node.inference_rule?;

    // A `$false`-concluding step is never a sound equivalence/projection of
    // consistent (or absent) premises; route it to the entailment check.
    if node.is_false {
        return None;
    }

    // Both primitives operate on a single premise.
    let [parent] = parents else {
        return None;
    };

    let mut ctx = LowerCtx::new(symbols);
    ctx.reset_vars();
    let parent_f = lower_annotated_formula(&mut ctx, parent);
    ctx.reset_vars();
    let concl_f = lower_annotated_formula(&mut ctx, node.formula);

    if EQUIV_RULES.contains(&rule) && equiv(&parent_f, &concl_f) {
        return Some(StepOutcome::Sound);
    }
    if PROJECTION_RULES.contains(&rule) && projects_conjunct(&parent_f, &concl_f) {
        return Some(StepOutcome::Sound);
    }
    None
}

/// Sound (partial) logical-equivalence test: `a ≡ b` confirmed via
/// `canon(nnf(a)) =α= canon(nnf(b))`.
fn equiv(a: &Formula, b: &Formula) -> bool {
    let ca = canon(&to_nnf(a));
    let cb = canon(&to_nnf(b));
    alpha_equiv(&ca, &cb)
}

/// `(∀x.(A ∧ B)) ⊨ ∀x.A`: confirm the conclusion is α-equivalent to the
/// universal closure of one top-level conjunct of the parent's NNF.
fn projects_conjunct(parent: &Formula, concl: &Formula) -> bool {
    let parent_nnf = to_nnf(parent);
    // Peel a shared universal prefix.
    let mut binders: Vec<VarId> = Vec::new();
    let mut body = &parent_nnf;
    while let Formula::Forall(v, inner) = body {
        binders.push(*v);
        body = inner;
    }
    let conjuncts: Vec<&Formula> = match body {
        Formula::And(cs) => cs.iter().collect(),
        _ => return false,
    };
    let concl_canon = canon(&to_nnf(concl));
    for c in conjuncts {
        // Re-wrap the conjunct in the peeled universal prefix.
        let mut wrapped = c.clone();
        for &v in binders.iter().rev() {
            wrapped = Formula::forall(v, wrapped);
        }
        if alpha_equiv(&canon(&wrapped), &concl_canon) {
            return true;
        }
    }
    false
}

/// Canonicalise an NNF formula by flattening, sorting, and de-duplicating
/// associative–commutative–idempotent `And`/`Or` nodes. Equivalence-preserving.
/// Sorting/dedup use the derived `Debug` rendering as a deterministic key —
/// adequate because any false negative merely defers to the ATP.
fn canon(f: &Formula) -> Formula {
    match f {
        Formula::And(_) => {
            let mut parts = flatten(f, true);
            parts = parts.iter().map(canon).collect();
            dedup_sort(&mut parts);
            match parts.len() {
                0 => Formula::True,
                1 => parts.into_iter().next().unwrap(),
                _ => Formula::And(parts),
            }
        }
        Formula::Or(_) => {
            let mut parts = flatten(f, false);
            parts = parts.iter().map(canon).collect();
            dedup_sort(&mut parts);
            match parts.len() {
                0 => Formula::False,
                1 => parts.into_iter().next().unwrap(),
                _ => Formula::Or(parts),
            }
        }
        Formula::Neg(inner) => Formula::neg(canon(inner)),
        Formula::Forall(v, body) => Formula::forall(*v, canon(body)),
        Formula::Exists(v, body) => Formula::exists(*v, canon(body)),
        Formula::Atom(_) | Formula::True | Formula::False => f.clone(),
        // NNF removes these, but stay total for safety.
        Formula::Implies(a, b) => Formula::implies(canon(a), canon(b)),
        Formula::Iff(a, b) => Formula::iff(canon(a), canon(b)),
    }
}

/// Recursively flatten nested same-kind connectives into a flat operand list.
fn flatten(f: &Formula, want_and: bool) -> Vec<Formula> {
    let mut out = Vec::new();
    let children = match (f, want_and) {
        (Formula::And(cs), true) => cs,
        (Formula::Or(cs), false) => cs,
        _ => return vec![f.clone()],
    };
    for c in children {
        out.extend(flatten(c, want_and));
    }
    out
}

/// Sort by `Debug` key and drop duplicates (idempotence of ∧/∨).
fn dedup_sort(parts: &mut Vec<Formula>) {
    parts.sort_by_key(|p| format!("{p:?}"));
    parts.dedup_by_key(|p| format!("{p:?}"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_tptp::{AnnotatedFormula, parse_tptp};

    fn fof(src: &'static str) -> &'static FOFAnnotated<'static> {
        let prob = Box::leak(Box::new(parse_tptp(src).expect("parse")));
        match prob.formulas.first().expect("formula") {
            AnnotatedFormula::FOF(f) => f,
            _ => panic!("expected FOF"),
        }
    }

    fn node_for(
        concl: &'static FOFAnnotated<'static>,
        rule: &'static str,
        is_false: bool,
    ) -> Node<'static> {
        Node {
            name: "s",
            role: mrs_tptp::FormulaRole::Plain,
            parents: vec!["p"],
            negated_parents: vec![false],
            inference_rule: Some(rule),
            status: None,
            is_false,
            fof: concl,
        }
    }

    fn run(
        parent_src: &'static str,
        concl_src: &'static str,
        rule: &'static str,
    ) -> Option<StepOutcome> {
        let parent = fof(parent_src);
        let concl = fof(concl_src);
        let node = node_for(concl, rule, false);
        let mut syms = SymbolTable::new();
        try_check(&node, &[parent], &mut syms)
    }

    #[test]
    fn nnf_equivalence_accepted() {
        // (p => q)  vs  (~p | q): logically equivalent under NNF.
        let oc = run(
            "fof(p, plain, (p => q)).",
            "fof(s, plain, (~p | q), inference(fof_nnf, [], [p])).",
            "fof_nnf",
        );
        assert!(matches!(oc, Some(StepOutcome::Sound)), "got {oc:?}");
    }

    #[test]
    fn ac_reorder_and_dedup_accepted() {
        // (a & b & a)  vs  (b & a): commutativity + idempotence.
        let oc = run(
            "fof(p, plain, (a & b & a)).",
            "fof(s, plain, (b & a), inference(flattening, [], [p])).",
            "flattening",
        );
        assert!(matches!(oc, Some(StepOutcome::Sound)), "got {oc:?}");
    }

    #[test]
    fn variable_rename_accepted() {
        let oc = run(
            "fof(p, plain, ![X]: q(X)).",
            "fof(s, plain, ![Y]: q(Y), inference(variable_rename, [], [p])).",
            "variable_rename",
        );
        assert!(matches!(oc, Some(StepOutcome::Sound)), "got {oc:?}");
    }

    #[test]
    fn split_conjunct_projection_accepted() {
        let oc = run(
            "fof(p, plain, (a & b)).",
            "fof(s, plain, a, inference(split_conjunct, [], [p])).",
            "split_conjunct",
        );
        assert!(matches!(oc, Some(StepOutcome::Sound)), "got {oc:?}");
    }

    #[test]
    fn split_conjunct_under_forall_accepted() {
        let oc = run(
            "fof(p, plain, ![X]: (a(X) & b(X))).",
            "fof(s, plain, ![X]: a(X), inference(split_conjunct, [], [p])).",
            "split_conjunct",
        );
        assert!(matches!(oc, Some(StepOutcome::Sound)), "got {oc:?}");
    }

    #[test]
    fn non_equivalent_step_falls_through() {
        // p  vs  q tagged fof_nnf: NOT equivalent — must NOT be accepted.
        // (This is exactly the adversarial name-trust bypass.)
        let oc = run(
            "fof(p, plain, p).",
            "fof(s, plain, q, inference(fof_nnf, [], [p])).",
            "fof_nnf",
        );
        assert!(oc.is_none(), "must fall through to ATP, got {oc:?}");
    }

    #[test]
    fn weakening_not_accepted_as_equivalence() {
        // (a & b) ⊢ a is a projection, not an equivalence: under fof_nnf
        // (an equivalence rule) it must fall through, not be accepted.
        let oc = run(
            "fof(p, plain, (a & b)).",
            "fof(s, plain, a, inference(fof_nnf, [], [p])).",
            "fof_nnf",
        );
        assert!(oc.is_none(), "weakening is not equivalence; got {oc:?}");
    }

    #[test]
    fn false_conclusion_falls_through() {
        let concl = fof("fof(s, plain, $false, inference(fof_simplification, [], [p])).");
        let parent = fof("fof(p, plain, a).");
        let node = node_for(concl, "fof_simplification", true);
        let mut syms = SymbolTable::new();
        assert!(try_check(&node, &[parent], &mut syms).is_none());
    }

    #[test]
    fn multi_premise_falls_through() {
        let parent1 = fof("fof(p, plain, a).");
        let parent2 = fof("fof(q, plain, b).");
        let concl = fof("fof(s, plain, a, inference(fof_nnf, [], [p, q])).");
        let node = Node {
            parents: vec!["p", "q"],
            negated_parents: vec![false, false],
            ..node_for(concl, "fof_nnf", false)
        };
        let mut syms = SymbolTable::new();
        assert!(try_check(&node, &[parent1, parent2], &mut syms).is_none());
    }
}
