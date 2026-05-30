//! Top-level verification loop. Orchestrates structural checks, per-rule
//! internal checks, and (later) ATP calls.

use std::time::{Duration, Instant};

use mrs_core::SymbolTable;
use mrs_tptp::{FOFAnnotated, FormulaRole};

use crate::atp::{Atp, AtpVerdict, NoopAtp};
use crate::checks::{
    axiom_leaf, introduced_definition, neg_conjecture, skolemize, vampire_skolemisation,
};
use crate::dag::{self, Dag};
use crate::load::LoadedJob;
use crate::lower::{LowerCtx, lower_fof_statement};
use crate::verdict::{StepOutcome, Verdict, aggregate};

/// Settings controlling the verification run.
pub struct Settings {
    /// Wall-clock budget for the whole proof.
    ///
    /// When ATP backends are used, this is split across the remaining
    /// unchecked steps (see `verify_with`).
    pub total_budget: Duration,
    /// Per-step ATP budget. Acts as an upper cap on each individual ATP
    /// invocation; the actual budget per step is
    /// `min(per_step_budget, remaining / remaining_steps)`.
    pub per_step_budget: Duration,
    /// If true, write a per-step report to stderr (`% step <name>: …`).
    pub verbose: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            total_budget: Duration::from_secs(30),
            per_step_budget: Duration::from_secs(8),
            verbose: false,
        }
    }
}

/// Run the verification pipeline on a loaded job. Returns the final verdict.
pub fn verify(job: &LoadedJob, settings: &Settings) -> Verdict {
    let noop = NoopAtp;
    verify_with(job, settings, &noop)
}

/// Run with a specific ATP backend.
pub fn verify_with(job: &LoadedJob, settings: &Settings, atp: &dyn Atp) -> Verdict {
    let started = Instant::now();

    // 1) Build the DAG.
    let dag = match dag::build(job.proof.problem()) {
        Ok(d) => d,
        Err(e) => return Verdict::FailedVerified(format!("structural: {e}")),
    };

    // Defensive: must have a $false root.
    if dag.root.is_none() {
        return Verdict::FailedVerified("structural: proof does not derive $false".into());
    }

    // 2) Set up shared symbol table and Skolem registry.
    let mut symbols = SymbolTable::new();
    let mut sk_reg = skolemize::SkolemRegistry::new();
    // Seed the registry with every symbol from the linked problem file so
    // that proofs cannot reuse problem symbols as Skolems. Per-step recording
    // is intentionally *not* done: Skolem freshness is defined w.r.t. the
    // original problem, not against peer proof steps that may legitimately
    // mention a Skolem introduced elsewhere in the proof (e.g. AVATAR
    // splitting definitions referencing a Skolem from a peer
    // `skolem_symbol_introduction` step). Steps that themselves introduce
    // Skolems (e.g. `skolemize`) record their new symbol explicitly.
    if let Some(problem) = job.problem.as_ref() {
        for af in &problem.problem().formulas {
            if let mrs_tptp::AnnotatedFormula::FOF(f) = af {
                sk_reg.record_from_statement(&f.formula);
            }
        }
    }

    // 3) Walk topo order, dispatching per node.
    let mut outcomes: Vec<(String, StepOutcome)> = Vec::with_capacity(dag.nodes.len());

    // Identify which steps may end up using the ATP (so we can prorate the
    // wall-clock budget across them). Internal checks are essentially free.
    let total_atp_steps = dag
        .topo
        .iter()
        .filter(|&&i| step_needs_atp(&dag.nodes[i]))
        .count();
    let mut atp_steps_remaining = total_atp_steps;

    for &idx in &dag.topo {
        let needs_atp = step_needs_atp(&dag.nodes[idx]);

        // Compute this step's ATP budget: the smaller of the per-step cap
        // and an even share of the remaining wall budget.
        let budget = if needs_atp {
            let elapsed = started.elapsed();
            let remaining = settings
                .total_budget
                .checked_sub(elapsed)
                .unwrap_or_default();
            let share = if atp_steps_remaining == 0 {
                Duration::ZERO
            } else {
                remaining / atp_steps_remaining as u32
            };
            // Vampire's `skolemisation` rule can rewrite many existentials
            // in one step against several Skolem-axiom premises. These
            // rewrites are sound by construction but combinatorially hard
            // for the ATP, so give them a bigger per-step cap. We still
            // honour the wall-clock remaining budget so other steps are
            // not entirely starved.
            let per_step_cap = if dag.nodes[idx].inference_rule == Some("skolemisation") {
                settings.per_step_budget.max(Duration::from_secs(25))
            } else {
                settings.per_step_budget
            };
            // Skolemisation steps may claim more than their fair share but
            // never more than the per-step cap or what remains on the wall.
            let upper = std::cmp::min(per_step_cap, remaining);
            let b = if dag.nodes[idx].inference_rule == Some("skolemisation") {
                upper
            } else {
                std::cmp::min(settings.per_step_budget, share)
            };
            // Avoid 0-duration ATP calls; require at least 1s if any time left.
            if b.is_zero() && !remaining.is_zero() {
                Duration::from_secs(1).min(remaining)
            } else {
                b
            }
        } else {
            Duration::ZERO
        };

        let oc = check_node(&dag, idx, job, &mut symbols, &mut sk_reg, atp, budget);
        let name = dag.nodes[idx].name.to_string();

        if needs_atp {
            atp_steps_remaining = atp_steps_remaining.saturating_sub(1);
        }

        if settings.verbose {
            let rule = dag.nodes[idx].inference_rule.unwrap_or("-");
            let kind = if needs_atp { "atp" } else { "internal" };
            eprintln!("% step {name} [{kind} rule={rule}] -> {oc:?}");
        }

        outcomes.push((name, oc));
    }

    aggregate(outcomes.iter().map(|(n, o)| (n.as_str(), o.clone())))
}

/// Inference rules that are structurally trivial enough that we declare them
/// `Sound` without bothering the ATP. They are sound *by definition* for any
/// reasonable presentation: each is either a tautological rearrangement of
/// the parent, or a syntactic reformulation, or a renaming of bound
/// variables, or a strict projection that is logically implied by the
/// parent (e.g. `(A & B) ⊢ A`).
///
/// Being wrong here costs us 10× more than playing it safe, so the list is
/// intentionally short and conservative. Anything not on the list still gets
/// dispatched to the ATP.
///
/// Categories represented:
///   * preprocessing / clausification (E, vampire):
///     `fof_simplification`, `fof_nnf`, `distribute`, `rectify`,
///     `variable_rename`, `true_and_iff_removal`, `evaluation`,
///     `trivial_inequality_removal`, `remove_duplicate_literals`,
///     `split_conjunct`
///   * negation step (covered separately by `neg_conjecture` check too):
///     `assume_negation`
///
/// Excluded on purpose (substantive first-order inferences that require
/// real entailment checks):
///   * E:        `spm`, `rw`, `cn`, `sr`, `pm`, `apply_def`
///   * Vampire:  `resolution`, `subsumption_resolution`, `superposition`,
///               `forward_subsumption_resolution`, `avatar_*`, etc.
const TRIVIAL_RULES: &[&str] = &[
    "assume_negation",
    "rectify",
    "true_and_iff_removal",
    "fof_simplification",
    "trivial_inequality_removal",
    "evaluation",
    "remove_duplicate_literals",
    // Added after the TPTP-v9 FOF corpus analysis (May 2026):
    "fof_nnf",         // negation normal form — logical equivalence
    "distribute",      // CNF distribution of ∨ over ∧ — logical equivalence
    "variable_rename", // α-renaming of bound variables — logical equivalence
    "split_conjunct",  // (A ∧ B) ⊢ A or B — sound projection
    // Added after the post-Fix#6 Vampire re-verify (May 2026):
    "duplicate_literal_removal", // Vampire alias of remove_duplicate_literals
    "flattening",                // (A ∧ B) ∧ C → A ∧ B ∧ C (assoc/commut)
    "nnf_transformation",        // Vampire alias of fof_nnf
    "ennf_transformation",       // eliminate <=> and =>; logically equivalent
    "cnf_transformation",        // FOF → CNF, equisatisfiable
];

fn is_trivial_rule(rule: Option<&str>) -> bool {
    matches!(rule, Some(r) if TRIVIAL_RULES.contains(&r))
}

fn step_needs_atp(node: &dag::Node<'_>) -> bool {
    // Leaves don't need ATP. A node is a "leaf" if it has a `file(...)`
    // provenance annotation AND its role is one a problem may legitimately
    // declare as a starting fact. Routing matches `is_premise_role` in the
    // leaf check so `check_node` and this function agree.
    if is_premise_role(node.role)
        && node
            .fof
            .annotations
            .as_ref()
            .and_then(|a| a.file_source())
            .is_some()
    {
        return false;
    }
    if node.role == FormulaRole::NegatedConjecture {
        let rule = node.inference_rule;
        let is_direct_negation =
            matches!(rule, Some("assume_negation") | Some("negated_conjecture"));
        if is_direct_negation || rule.is_none() {
            return false;
        }
    }
    if node.inference_rule == Some("skolemize") {
        return false;
    }
    // Predicate-definition introductions: handled by a dedicated structural
    // check, no ATP needed.
    if node
        .fof
        .annotations
        .as_ref()
        .is_some_and(introduced_definition::is_introduced_definition)
    {
        return false;
    }
    if is_trivial_rule(node.inference_rule) {
        return false;
    }
    true
}

/// Roles a proof leaf may legitimately re-import from the linked problem.
/// Mirrors `axiom_leaf::is_premise_role`; the two must stay in sync.
fn is_premise_role(r: FormulaRole) -> bool {
    matches!(
        r,
        FormulaRole::Axiom
            | FormulaRole::Hypothesis
            | FormulaRole::Assumption
            | FormulaRole::Definition
            | FormulaRole::Conjecture
            | FormulaRole::NegatedConjecture
            | FormulaRole::Lemma
            | FormulaRole::Theorem
            | FormulaRole::Corollary
    )
}

fn check_node<'p>(
    dag: &Dag<'p>,
    idx: usize,
    job: &LoadedJob,
    symbols: &mut SymbolTable,
    sk_reg: &mut skolemize::SkolemRegistry,
    atp: &dyn Atp,
    budget: Duration,
) -> StepOutcome {
    let node = &dag.nodes[idx];

    // --- Role / status routing --------------------------------------------

    // Leaf: any premise role brought in from the problem file via a
    // `file(...)` source. We delegate to `axiom_leaf::check_leaf`, which
    // handles both the named-axiom and the Vampire-style anonymous
    // (`file(_, unknown)`) cases.
    if is_premise_role(node.role)
        && node
            .fof
            .annotations
            .as_ref()
            .and_then(|a| a.file_source())
            .is_some()
    {
        return axiom_leaf::check_leaf(
            node.fof,
            job.problem.as_ref().map(|p| p.problem()),
            symbols,
        );
    }

    // negated_conjecture step — only the direct negation step (rule
    // `assume_negation`, `negated_conjecture`, or no rule with a conjecture
    // parent) gets the strict structural check. Downstream
    // `negated_conjecture`-tagged steps keep that role through normalization
    // (E does this); they are ordinary inferences and go to the ATP.
    if node.role == FormulaRole::NegatedConjecture {
        let rule = node.inference_rule;
        let is_direct_negation =
            matches!(rule, Some("assume_negation") | Some("negated_conjecture"));
        let parent_is_conjecture = node
            .parents
            .first()
            .and_then(|p| dag.by_name.get(p))
            .map(|&i| dag.nodes[i].role == FormulaRole::Conjecture)
            .unwrap_or(false);
        if is_direct_negation || (rule.is_none() && parent_is_conjecture) {
            let parent_fof = node
                .parents
                .first()
                .and_then(|p| dag.by_name.get(p).map(|&i| dag.nodes[i].fof));
            return neg_conjecture::check(node.fof, parent_fof, symbols);
        }
        // Otherwise fall through to whatever other handling applies.
    }

    // plain `esa` rule=skolemize
    if node.inference_rule == Some("skolemize") {
        let parent_fof = node
            .parents
            .first()
            .and_then(|p| dag.by_name.get(p).map(|&i| dag.nodes[i].fof));
        return skolemize::check(node.fof, parent_fof, sk_reg);
    }

    // introduced(definition): predicate-definition introduction, sound as a
    // conservative extension when the head predicate is fresh.
    if node
        .fof
        .annotations
        .as_ref()
        .is_some_and(introduced_definition::is_introduced_definition)
    {
        return introduced_definition::check(node.fof, sk_reg);
    }

    // Vampire `skolemisation`: try the structural check before falling
    // back to the ATP. The structural check is much faster than the ATP
    // and handles the multi-Skolem rewrites that often time out the ATP.
    if node.inference_rule == Some("skolemisation") {
        let parents: Vec<&FOFAnnotated<'_>> = node
            .parents
            .iter()
            .filter_map(|p| dag.by_name.get(p).map(|&i| dag.nodes[i].fof))
            .collect();
        if let Some(outcome) = vampire_skolemisation::try_check(node.fof, &parents, sk_reg) {
            return outcome;
        }
        // Fall through to ATP if the structural check could not apply.
    }

    // Trivial rules: accepted without ATP.
    if is_trivial_rule(node.inference_rule) {
        return StepOutcome::Sound;
    }

    // Other plain/thm/cth steps → delegate to ATP.
    delegate_to_atp(dag, idx, symbols, atp, budget)
}

fn delegate_to_atp<'p>(
    dag: &Dag<'p>,
    idx: usize,
    symbols: &mut SymbolTable,
    atp: &dyn Atp,
    budget: Duration,
) -> StepOutcome {
    let node = &dag.nodes[idx];
    if budget.is_zero() {
        return StepOutcome::Unknown(format!(
            "ATP budget exhausted (rule={:?})",
            node.inference_rule
        ));
    }
    // Build the conclusion and premise formulas in mrs-core form.
    //
    // Each parent's `negated_parents[i]` flag tells us whether the
    // pedigree wraps that parent in `assume_negation`. In that case the
    // *asserted* premise is `¬parent`, not `parent`, and we must
    // negate before handing it to the ATP — otherwise we'll feed
    // `co1 ∧ definitions ⊨ ¬co1` (genuinely unsatisfiable) and get back
    // a spurious `Unsound` verdict.
    let mut ctx = LowerCtx::new(symbols);
    let mut premises = Vec::with_capacity(node.parents.len());
    for (i, p) in node.parents.iter().enumerate() {
        if let Some(&pi) = dag.by_name.get(p) {
            ctx.reset_vars();
            let mut f = lower_fof_statement(&mut ctx, &dag.nodes[pi].fof.formula);
            if node.negated_parents.get(i).copied().unwrap_or(false) {
                f = mrs_core::Formula::Neg(Box::new(f));
            } else {
                // E emits `predicate_definition_introduction` axioms as
                // one half of the underlying iff, e.g.
                //   fof(f8, plain, (body | ~sP0),
                //       introduced(definition,
                //                  [new_symbols(naming, [sP0])],
                //                  [predicate_definition_introduction])).
                // Literally that says `sP0 -> ¬body`. The intended meaning
                // is `sP0 <-> ¬body`. When a `definition_folding` step
                // (or any later step) folds an occurrence of `¬body` into
                // `sP0`, it relies on the missing direction. We extend
                // the premise to the biconditional closure before
                // handing it to the ATP. Only applied when the parent's
                // annotation matches the E pattern; other premises pass
                // through unchanged.
                //
                // We do *not* touch negated parents (those go through
                // `assume_negation` and have their own polarity flip).
                let parent_ann = dag.nodes[pi].fof.annotations.as_ref();
                if introduced_definition::is_predicate_definition_introduction(parent_ann) {
                    if let Some(extended) =
                        complete_definition_iff(&f, parent_ann.unwrap(), ctx.symbols)
                    {
                        f = extended;
                    }
                }
            }
            premises.push(f);
        }
    }
    ctx.reset_vars();
    let conclusion = lower_fof_statement(&mut ctx, &node.fof.formula);

    // Structural definition_folding: when Vampire emits a step whose
    // sole non-def parent is the unfolded source and the rest are
    // `predicate_definition_introduction` axioms (now iff-completed by
    // the loop above), we can decide the entailment by syntactic
    // unfolding + alpha-equivalence. This sidesteps the saturation
    // search that even multi-second-budget ATPs fail on. The check is
    // bounded by a small work budget so worst case it falls through
    // quickly to the generic ATP ladder.
    if node.inference_rule == Some("definition_folding")
        && let Some(true) = crate::checks::definition_folding::try_check(&premises, &conclusion)
    {
        return StepOutcome::Sound;
    }

    // Propositional fast-path: when every premise and the conclusion are
    // built from 0-ary predicates only (the `spl0_N` avatar splits and
    // similar), the entailment check is a finite SAT problem solvable in
    // microseconds. Vampire's `rat`, `avatar_*`, and `sat_conversion`
    // rules all produce pure propositional steps, and FOL ATPs
    // (eprover/vampire/mrs) routinely time out on the larger ones.
    // Returning `None` here costs only a quick is_propositional walk and
    // falls through to the unchanged ATP ladder.
    if let Some(outcome) =
        crate::checks::propositional_sat::try_propositional(&premises, &conclusion)
    {
        return match outcome {
            crate::checks::propositional_sat::PropOutcome::Sound => StepOutcome::Sound,
            crate::checks::propositional_sat::PropOutcome::Unsound => StepOutcome::Unsound(
                "propositional SAT solver refuted entailment by premises".into(),
            ),
        };
    }

    match atp.check_step(symbols, &premises, &conclusion, budget) {
        AtpVerdict::Sound => StepOutcome::Sound,
        AtpVerdict::Unsound => StepOutcome::Unsound(format!(
            "ATP `{}` refuted entailment by premises",
            atp.name()
        )),
        AtpVerdict::Unknown => StepOutcome::Unknown(format!(
            "no ATP could decide step (rule={:?})",
            node.inference_rule
        )),
    }
}

/// Given a premise lowered from an E `predicate_definition_introduction`
/// axiom and its TPTP annotation, return the biconditional closure of
/// the premise when it has the canonical one-direction shape; return
/// `None` otherwise (premise passes through unchanged).
///
/// The canonical E shape is (modulo a possibly-empty `Forall` prefix):
///
/// ```text
/// rest_disjuncts ∨ ±P(args)
/// ```
///
/// where `±P(args)` is the unique disjunct mentioning the freshly-declared
/// predicate symbol `P` named in the annotation's `new_symbols(naming,
/// [P])` entry. The literal may be positive (`P(args)`) or negative
/// (`¬P(args)`), and the polarity determines which iff we produce:
///
/// * `rest ∨ ¬P` ≡ `P → rest`; completed by `rest → P`. Iff: `P ↔ rest`.
/// * `rest ∨ P`  ≡ `¬rest → P`; completed by `P → ¬rest`. Iff: `P ↔ ¬rest`.
///
/// E uses the negative-literal form in practice (see e.g. ALG021+1 f8:
/// `(commutativity_conj) | ~sP0`), which folds in `definition_folding`
/// as "if commutativity holds, sP0 holds". The literal premise alone
/// gives only one direction; without the iff completion the ATP rightly
/// refuses to conclude the folded form.
///
/// Shapes we deliberately do *not* extend (returning `None`):
///
/// * Already-biconditional bodies (`P ↔ φ` or `∀X. (P(X) ↔ φ(X))`) — the
///   premise already provides both halves.
/// * Disjunctions that do not contain a `±P` literal at the top level,
///   or contain `P` more than once, or where the predicate name is not
///   declared in the annotation.
/// * Sequents or anything not parseable as a quantifier-prefixed
///   disjunction (the rare exotic shape; conservative pass-through).
fn complete_definition_iff(
    lowered: &mrs_core::Formula,
    ann: &mrs_tptp::Annotations<'_>,
    symbols: &mut mrs_core::SymbolTable,
) -> Option<mrs_core::Formula> {
    use mrs_core::{Atom, Formula};

    let declared = introduced_definition::declared_new_symbols(ann);
    if declared.len() != 1 {
        return None;
    }
    let p_sym = symbols.intern(declared[0]);

    // Peel a (possibly empty) Forall prefix; remember the variables so we
    // can re-wrap the result.
    let mut binders: Vec<u32> = Vec::new();
    let mut body: &Formula = lowered;
    while let Formula::Forall(v, inner) = body {
        binders.push(*v);
        body = inner;
    }

    // If already a biconditional, no extension needed.
    if matches!(body, Formula::Iff(..)) {
        return None;
    }

    // Body must be a disjunction containing exactly one ±P(...) literal.
    let disjuncts: Vec<&Formula> = match body {
        Formula::Or(ds) => ds.iter().collect(),
        _ => return None,
    };

    let mut p_idx: Option<usize> = None;
    let mut p_args: Option<Vec<mrs_core::Term>> = None;
    let mut p_positive: bool = false;
    for (i, d) in disjuncts.iter().enumerate() {
        let (atom_opt, polarity_pos) = match d {
            Formula::Neg(inner) => match inner.as_ref() {
                Formula::Atom(a) => (Some(a), false),
                _ => (None, false),
            },
            Formula::Atom(a) => (Some(a), true),
            _ => (None, false),
        };
        if let Some(Atom::Pred(sid, args)) = atom_opt
            && *sid == p_sym
        {
            if p_idx.is_some() {
                // P appears more than once at top level — bail.
                return None;
            }
            p_idx = Some(i);
            p_args = Some(args.clone());
            p_positive = polarity_pos;
        }
    }

    let p_idx = p_idx?;
    let p_args = p_args?;

    // `rest` = disjunction of all other disjuncts (or False if none).
    let rest_owned: Vec<Formula> = disjuncts
        .iter()
        .enumerate()
        .filter_map(|(i, d)| (i != p_idx).then(|| (*d).clone()))
        .collect();
    let rest = match rest_owned.len() {
        0 => Formula::False,
        1 => rest_owned.into_iter().next().unwrap(),
        _ => Formula::Or(rest_owned),
    };

    // Polarity:
    //   * Negative literal: premise is `rest ∨ ¬P`, i.e. `P → rest`.
    //     iff completion adds `rest → P`. Combined: `P ↔ rest`.
    //   * Positive literal: premise is `rest ∨ P`, i.e. `¬rest → P`.
    //     iff completion adds `P → ¬rest`. Combined: `P ↔ ¬rest`.
    let p_atom = Formula::Atom(Atom::Pred(p_sym, p_args));
    let other_side = if p_positive {
        Formula::neg(rest)
    } else {
        rest
    };
    let iff = Formula::iff(p_atom, other_side);

    // Re-wrap in the original Forall prefix.
    let mut out = iff;
    for v in binders.into_iter().rev() {
        out = Formula::forall(v, out);
    }
    Some(out)
}

#[cfg(test)]
mod complete_definition_iff_tests {
    use super::*;
    use mrs_core::{Atom, Formula, SymbolTable, Term};
    use mrs_tptp::parse_tptp;

    fn ann_from(src: &str) -> mrs_tptp::Annotations<'_> {
        // Build a tiny fof annotated formula just to extract its annotation.
        let prob = Box::leak(Box::new(parse_tptp(src).expect("parse")));
        match prob.formulas.first().expect("formula") {
            mrs_tptp::AnnotatedFormula::FOF(f) => f.annotations.clone().expect("ann"),
            _ => panic!("expected FOF"),
        }
    }

    #[test]
    fn negative_literal_shape_completes_to_iff() {
        // Premise body: a | ~sP, with sP nullary.
        // a | ~sP ≡ sP → a, completed to sP ↔ a.
        let mut syms = SymbolTable::new();
        let a_sym = syms.intern("a");
        let p_sym = syms.intern("sP");
        let a_atom = Formula::Atom(Atom::Pred(a_sym, vec![]));
        let p_neg = Formula::neg(Formula::Atom(Atom::Pred(p_sym, vec![])));
        let lowered = Formula::Or(vec![a_atom.clone(), p_neg]);

        let ann = ann_from(
            "fof(f, plain, ($true),\n  introduced(definition,[new_symbols(naming,[sP])],[predicate_definition_introduction])).",
        );
        let extended = complete_definition_iff(&lowered, &ann, &mut syms).expect("extended");
        // Expect Iff(P, a) — NOT Iff(P, Neg(a)).
        match extended {
            Formula::Iff(l, r) => {
                match *l {
                    Formula::Atom(Atom::Pred(s, ref args)) => {
                        assert_eq!(s, p_sym);
                        assert!(args.is_empty());
                    }
                    other => panic!("lhs not P: {other:?}"),
                }
                match *r {
                    Formula::Atom(Atom::Pred(s, _)) => assert_eq!(s, a_sym),
                    other => panic!("rhs not a (got {other:?})"),
                }
            }
            other => panic!("not iff: {other:?}"),
        }
    }

    #[test]
    fn positive_literal_shape_completes_to_iff_with_neg_rest() {
        // Premise body: a | sP. ≡ ¬a → sP, completed to sP ↔ ¬a.
        let mut syms = SymbolTable::new();
        let a_sym = syms.intern("a");
        let p_sym = syms.intern("sP");
        let lowered = Formula::Or(vec![
            Formula::Atom(Atom::Pred(a_sym, vec![])),
            Formula::Atom(Atom::Pred(p_sym, vec![])),
        ]);
        let ann = ann_from(
            "fof(f, plain, ($true),\n  introduced(definition,[new_symbols(naming,[sP])],[predicate_definition_introduction])).",
        );
        let extended = complete_definition_iff(&lowered, &ann, &mut syms).expect("extended");
        match extended {
            Formula::Iff(_, r) => match *r {
                Formula::Neg(_) => {}
                other => panic!("rhs not Neg(.): {other:?}"),
            },
            other => panic!("not iff: {other:?}"),
        }
    }

    #[test]
    fn already_iff_is_unchanged() {
        let mut syms = SymbolTable::new();
        let a_sym = syms.intern("a");
        let p_sym = syms.intern("sP");
        let lowered = Formula::iff(
            Formula::Atom(Atom::Pred(p_sym, vec![])),
            Formula::Atom(Atom::Pred(a_sym, vec![])),
        );
        let ann = ann_from(
            "fof(f, plain, ($true),\n  introduced(definition,[new_symbols(naming,[sP])],[predicate_definition_introduction])).",
        );
        assert!(complete_definition_iff(&lowered, &ann, &mut syms).is_none());
    }

    #[test]
    fn forall_prefix_preserved() {
        // Body: ! [X] : (p(X) | ~sP(X))
        let mut syms = SymbolTable::new();
        let p_sym = syms.intern("p");
        let sp_sym = syms.intern("sP");
        let inner = Formula::Or(vec![
            Formula::Atom(Atom::Pred(p_sym, vec![Term::var(0)])),
            Formula::neg(Formula::Atom(Atom::Pred(sp_sym, vec![Term::var(0)]))),
        ]);
        let lowered = Formula::forall(0, inner);
        let ann = ann_from(
            "fof(f, plain, ($true),\n  introduced(definition,[new_symbols(naming,[sP])],[predicate_definition_introduction])).",
        );
        let extended = complete_definition_iff(&lowered, &ann, &mut syms).expect("extended");
        match extended {
            Formula::Forall(v, body) => {
                assert_eq!(v, 0);
                assert!(matches!(*body, Formula::Iff(..)));
            }
            other => panic!("not forall: {other:?}"),
        }
    }

    #[test]
    fn no_p_literal_returns_none() {
        // Body has no occurrence of declared symbol.
        let mut syms = SymbolTable::new();
        let a_sym = syms.intern("a");
        let _p_sym = syms.intern("sP");
        let lowered = Formula::Or(vec![
            Formula::Atom(Atom::Pred(a_sym, vec![])),
            Formula::Atom(Atom::Pred(a_sym, vec![])),
        ]);
        let ann = ann_from(
            "fof(f, plain, ($true),\n  introduced(definition,[new_symbols(naming,[sP])],[predicate_definition_introduction])).",
        );
        assert!(complete_definition_iff(&lowered, &ann, &mut syms).is_none());
    }

    #[test]
    fn multiple_p_occurrences_returns_none() {
        let mut syms = SymbolTable::new();
        let p_sym = syms.intern("sP");
        let lowered = Formula::Or(vec![
            Formula::Atom(Atom::Pred(p_sym, vec![])),
            Formula::neg(Formula::Atom(Atom::Pred(p_sym, vec![]))),
        ]);
        let ann = ann_from(
            "fof(f, plain, ($true),\n  introduced(definition,[new_symbols(naming,[sP])],[predicate_definition_introduction])).",
        );
        assert!(complete_definition_iff(&lowered, &ann, &mut syms).is_none());
    }
}
