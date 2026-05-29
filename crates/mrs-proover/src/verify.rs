//! Top-level verification loop. Orchestrates structural checks, per-rule
//! internal checks, and (later) ATP calls.

use std::time::{Duration, Instant};

use mrs_core::SymbolTable;
use mrs_tptp::FormulaRole;

use crate::atp::{Atp, AtpVerdict, NoopAtp};
use crate::checks::{axiom_leaf, introduced_definition, neg_conjecture, skolemize};
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
            let b = std::cmp::min(settings.per_step_budget, share);
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
            }
            premises.push(f);
        }
    }
    ctx.reset_vars();
    let conclusion = lower_fof_statement(&mut ctx, &node.fof.formula);

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
