//! Top-level verification loop. Orchestrates structural checks, per-rule
//! internal checks, and (later) ATP calls.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use mrs_core::{Formula, SymbolTable};
use mrs_tptp::{AnnotatedFormula, FormulaRole};

use crate::atp::{Atp, AtpVerdict, NoopAtp};
use crate::checks::{
    axiom_leaf, introduced_definition, neg_conjecture, skolemize, trivial, vampire_skolemisation,
};
use crate::dag::{self, Dag};
use crate::load::LoadedJob;
use crate::lower::{LowerCtx, lower_annotated_formula};
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

    if let Err(e) = crate::checks::introduced_definition::check_cycles(&dag) {
        return Verdict::FailedVerified(format!("structural: {e}"));
    }

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

    // 3) Pass 1 (serial): run every cheap internal check and *prepare* each
    //    ATP step (lowering premises/conclusion, then the structural
    //    fast-paths). This pass is the only one that mutates `symbols` and
    //    `sk_reg`, so it must run in topo order on a single thread. Structural
    //    steps are decided here; genuine ATP steps are collected as jobs.
    let mut names: Vec<&str> = Vec::with_capacity(dag.topo.len());
    let mut rules: Vec<&str> = Vec::with_capacity(dag.topo.len());
    let mut outcomes: Vec<Option<StepOutcome>> = Vec::with_capacity(dag.topo.len());
    let mut jobs: Vec<AtpJob> = Vec::new();

    for &idx in &dag.topo {
        let slot = outcomes.len();
        names.push(dag.nodes[idx].name);
        rules.push(dag.nodes[idx].inference_rule.unwrap_or("-"));
        match check_node_prepare(&dag, idx, job, &mut symbols, &mut sk_reg) {
            Prepared::Resolved(oc) => outcomes.push(Some(oc)),
            Prepared::NeedsAtp(step) => {
                outcomes.push(None);
                jobs.push(AtpJob {
                    slot,
                    is_skolemisation: dag.nodes[idx].inference_rule == Some("skolemisation"),
                    step,
                });
            }
        }
    }

    // 4) Pass 2 (parallel): run the collected ATP jobs across all cores. After
    //    Pass 1 the symbol table is complete and immutable, so `&symbols` is
    //    shared read-only and `atp.check_step` (an external process spawn) is
    //    a pure function of its inputs. Each job computes its budget from a
    //    shared deadline, keeping the total wall time within `total_budget`
    //    regardless of scheduling. Outcomes are written back to topo slots, so
    //    the aggregated verdict (and its reason string) stay deterministic.
    run_atp_jobs(&jobs, atp, &symbols, &mut outcomes, started, settings);

    if settings.verbose {
        for i in 0..names.len() {
            eprintln!(
                "% step {} [rule={}] -> {:?}",
                names[i], rules[i], outcomes[i]
            );
        }
    }

    aggregate(
        names
            .iter()
            .zip(outcomes)
            .map(|(n, o)| (*n, o.expect("every step resolved in pass 1 or pass 2"))),
    )
}

/// A single ATP-bound step queued by Pass 1 for parallel execution in Pass 2.
struct AtpJob {
    /// Index into the topo-ordered `outcomes` vector this job's result fills.
    slot: usize,
    /// Vampire `skolemisation` steps get a larger per-step budget cap.
    is_skolemisation: bool,
    step: AtpStep,
}

/// Everything needed to run one ATP entailment query and interpret its result,
/// captured during Pass 1 so Pass 2 needs no access to the DAG or mutable state.
struct AtpStep {
    premises: Vec<Formula>,
    conclusion: Formula,
    /// `esa` (equisatisfiability) steps: a counter-model is expected and must
    /// never be reported as `Unsound`.
    esa: bool,
    /// Inference-rule name, for diagnostic messages only.
    rule: Option<String>,
}

/// Outcome of preparing a step in Pass 1.
enum Prepared {
    /// Decided without the ATP (internal check or structural fast-path).
    Resolved(StepOutcome),
    /// Needs an external ATP query; deferred to Pass 2.
    NeedsAtp(AtpStep),
}

/// Run the queued ATP jobs in parallel and write each result back into its
/// topo slot in `outcomes`.
fn run_atp_jobs(
    jobs: &[AtpJob],
    atp: &dyn Atp,
    symbols: &SymbolTable,
    outcomes: &mut [Option<StepOutcome>],
    started: Instant,
    settings: &Settings,
) {
    if jobs.is_empty() {
        return;
    }

    let deadline = started + settings.total_budget;
    let per_step = settings.per_step_budget;
    let n_workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(jobs.len());

    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<(usize, StepOutcome)>> = Mutex::new(Vec::with_capacity(jobs.len()));

    std::thread::scope(|scope| {
        for _ in 0..n_workers {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= jobs.len() {
                        break;
                    }
                    let job = &jobs[i];
                    // `jobs.len() - i` is the number of not-yet-claimed jobs
                    // (including this one); used to share the remaining wall
                    // budget fairly while accounting for parallel execution.
                    let jobs_left = jobs.len() - i;
                    let budget = step_budget(
                        deadline,
                        per_step,
                        n_workers,
                        jobs_left,
                        job.is_skolemisation,
                    );
                    let oc = finish_atp(atp, symbols, &job.step, budget);
                    results.lock().expect("results mutex").push((job.slot, oc));
                }
            });
        }
    });

    for (slot, oc) in results.into_inner().expect("results mutex") {
        outcomes[slot] = Some(oc);
    }
}

/// Budget for one parallel ATP job.
///
/// With `w` workers and `jobs_left` queued jobs, roughly `w` run at once, so a
/// job may claim up to `remaining * min(w, jobs_left) / jobs_left` of the wall
/// clock — i.e. the full remaining time when there are no more jobs than
/// workers, and a fair parallel share otherwise. Capped by the per-step ceiling
/// (raised for `skolemisation`) and floored to 1s when meaningful time remains,
/// so we never waste a step on a sub-second query that is bound to time out.
fn step_budget(
    deadline: Instant,
    per_step: Duration,
    n_workers: usize,
    jobs_left: usize,
    is_skolemisation: bool,
) -> Duration {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Duration::ZERO;
    }
    let parallel = n_workers.min(jobs_left).max(1) as u32;
    let share = remaining * parallel / jobs_left.max(1) as u32;
    let cap = if is_skolemisation {
        per_step.max(Duration::from_secs(25))
    } else {
        per_step
    };
    let b = cap.min(share);
    if b < Duration::from_secs(1) && remaining >= Duration::from_secs(1) {
        Duration::from_secs(1)
    } else {
        b
    }
}

/// Former name-trust list, now replaced by structural verification.
///
/// Previously these preprocessing/clausification rules were accepted as
/// `Sound` purely on their inference-rule name — the single largest
/// adversarial soundness hole, since the attacker controls the label. They
/// now route through [`crate::checks::trivial`], which only accepts on a
/// *checked* structural proof (NNF-canonical equivalence or conjunct
/// projection) and otherwise falls through to the ATP. See that module for
/// the soundness argument.
///
/// No longer drives budgeting (Pass 1 prepares every step unconditionally);
/// retained as the documented routing predicate exercised by the tests below.
#[cfg_attr(not(test), allow(dead_code))]
fn step_needs_atp(node: &dag::Node<'_>) -> bool {
    // Leaves don't need ATP. A node is a "leaf" if it has a `file(...)`
    // provenance annotation AND its role is one a problem may legitimately
    // declare as a starting fact. Routing matches `is_premise_role` in the
    // leaf check so `check_node` and this function agree.
    if is_premise_role(node.role)
        && node
            .formula
            .annotations()
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
        .formula
        .annotations()
        .is_some_and(introduced_definition::is_introduced_definition)
    {
        return false;
    }
    // Former `TRIVIAL_RULES` steps are intentionally *not* short-circuited
    // here: they are attempted by the structural `trivial` verifier inside
    // `check_node` (essentially free), and only those it cannot confirm fall
    // through to the ATP. Counting them as ATP steps guarantees such
    // fall-throughs still receive a real budget instead of being starved to a
    // spurious `Unknown`.
    true
}

/// Roles a proof leaf may legitimately re-import from the linked problem.
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

fn check_node_prepare<'p>(
    dag: &Dag<'p>,
    idx: usize,
    job: &LoadedJob,
    symbols: &mut SymbolTable,
    sk_reg: &mut skolemize::SkolemRegistry,
) -> Prepared {
    let node = &dag.nodes[idx];

    // --- Role / status routing --------------------------------------------

    // Leaf: any premise role brought in from the problem file via a
    // `file(...)` source. We delegate to `axiom_leaf::check_leaf`, which
    // handles both the named-axiom and the Vampire-style anonymous
    // (`file(_, unknown)`) cases.
    if is_premise_role(node.role)
        && node
            .formula
            .annotations()
            .and_then(|a| a.file_source())
            .is_some()
    {
        return Prepared::Resolved(axiom_leaf::check_leaf(
            node.formula,
            job.problem.as_ref().map(|p| p.problem()),
            symbols,
        ));
    }

    // A conjecture MUST come from the problem file. If it lacks a file source,
    // it's an adversarial fake conjecture.
    if node.role == FormulaRole::Conjecture {
        return Prepared::Resolved(StepOutcome::Unsound(
            "conjecture step lacks file source annotation".into(),
        ));
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
                .and_then(|p| dag.by_name.get(p).map(|&i| dag.nodes[i].formula));
            return Prepared::Resolved(neg_conjecture::check(node.formula, parent_fof, symbols));
        }
        // Otherwise fall through to whatever other handling applies.
    }

    // plain `esa` rule=skolemize
    if node.inference_rule == Some("skolemize") {
        let parent_fof = node
            .parents
            .first()
            .and_then(|p| dag.by_name.get(p).map(|&i| dag.nodes[i].formula));
        return Prepared::Resolved(skolemize::check(node.formula, parent_fof, sk_reg));
    }

    // introduced(definition): predicate-definition introduction, sound as a
    // conservative extension when the head predicate is fresh.
    if node
        .formula
        .annotations()
        .is_some_and(introduced_definition::is_introduced_definition)
    {
        return Prepared::Resolved(introduced_definition::check(node.formula, sk_reg));
    }

    // Vampire `skolemisation`: try the structural check before falling
    // back to the ATP. The structural check is much faster than the ATP
    // and handles the multi-Skolem rewrites that often time out the ATP.
    if node.inference_rule == Some("skolemisation") {
        let parents: Vec<&AnnotatedFormula<'_>> = node
            .parents
            .iter()
            .filter_map(|p| dag.by_name.get(p).map(|&i| dag.nodes[i].formula))
            .collect();
        if let Some(outcome) = vampire_skolemisation::try_check(node.formula, &parents, sk_reg) {
            return Prepared::Resolved(outcome);
        }
        // Fall through to ATP if the structural check could not apply.
    }

    // Former `TRIVIAL_RULES`: instead of trusting the rule *name*, attempt a
    // structural verification (NNF-canonical equivalence for rewriting rules,
    // conjunct projection for `split_conjunct`). The check accepts only on a
    // checked structural proof and otherwise returns `None`, so adversarial
    // mislabelling cannot earn a blind pass — anything unconfirmed falls
    // through to the real entailment check below. `$false`-concluding steps
    // are excluded by `trivial::try_check` itself and always reach the ATP.
    if trivial::is_trivial_rule(node.inference_rule) {
        let parents: Vec<&AnnotatedFormula<'_>> = node
            .parents
            .iter()
            .filter_map(|p| dag.by_name.get(p).map(|&i| dag.nodes[i].formula))
            .collect();
        if let Some(outcome) = trivial::try_check(node, &parents, symbols) {
            return Prepared::Resolved(outcome);
        }
        // Fall through to ATP if the structural check could not confirm.
    }

    // Other plain/thm/cth steps → prepare an ATP query for Pass 2.
    prepare_atp_step(dag, idx, symbols)
}

/// Decide a step via the ATP, keeping the prepare/finish split internal.
///
/// Retained for the unit tests that exercise the esa/thm verdict mapping with
/// a synchronous mock backend; the production loop uses `prepare_atp_step` +
/// `finish_atp` directly so the (expensive) `finish_atp` half can run in
/// parallel across steps.
#[cfg_attr(not(test), allow(dead_code))]
fn delegate_to_atp<'p>(
    dag: &Dag<'p>,
    idx: usize,
    symbols: &mut SymbolTable,
    atp: &dyn Atp,
    budget: Duration,
) -> StepOutcome {
    match prepare_atp_step(dag, idx, symbols) {
        Prepared::Resolved(oc) => oc,
        Prepared::NeedsAtp(step) => finish_atp(atp, symbols, &step, budget),
    }
}

/// Lower a step's premises and conclusion and run the structural fast-paths.
/// Returns `Resolved` when a fast-path decides the step, otherwise `NeedsAtp`
/// with the lowered formulas captured for a deferred ATP query. Mutates
/// `symbols` (interning during lowering); must run in Pass 1.
fn prepare_atp_step<'p>(dag: &Dag<'p>, idx: usize, symbols: &mut SymbolTable) -> Prepared {
    let node = &dag.nodes[idx];
    // SZS status routing. An `esa` step asserts *equisatisfiability*, not
    // logical entailment: e.g. Skolemization introduces a fresh symbol whose
    // value the premises do not pin down, so the conclusion need not be a
    // logical consequence. A counter-model to the entailment query is
    // therefore *expected* for a sound esa step and is NOT evidence of a
    // fault. Reporting `Unsound` (→ FailedVerified, −1) on a good esa step
    // would be a scoring error, and becomes an outright hazard once a
    // counter-model finder is in the ladder. So for esa steps we accept only
    // positive `Sound` confirmations and downgrade every refutation to
    // `Unknown` (NotVerified, 0). `thm`/`cth`/plain steps are genuine
    // entailments and keep full refutation power.
    let esa = node.status == Some("esa");
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
    // Parallel to `premises`: whether each premise is an *introduced
    // definition* (E `predicate_definition_introduction` or Vampire
    // `avatar_definition`) as opposed to a source/original formula. The
    // structural folding check uses this to decide which premises to
    // unfold; a source that happens to be a biconditional (e.g. an
    // original `cUnsatisfiable(X) <=> …` definition carried through
    // `flattening`) must not be mistaken for a fresh-symbol definition.
    let mut premise_is_def = Vec::with_capacity(node.parents.len());
    for (i, p) in node.parents.iter().enumerate() {
        if let Some(&pi) = dag.by_name.get(p) {
            ctx.reset_vars();
            let mut f = lower_annotated_formula(&mut ctx, dag.nodes[pi].formula);
            let negated = node.negated_parents.get(i).copied().unwrap_or(false);
            if negated && dag.nodes[pi].role != FormulaRole::Conjecture {
                return Prepared::Resolved(StepOutcome::Unsound(format!(
                    "parent '{}' is wrapped in `assume_negation` but is not a conjecture",
                    p
                )));
            }
            let parent_ann = dag.nodes[pi].formula.annotations();
            // A definition introduces fresh symbols via
            // `new_symbols(naming, [..])`; a source never does. Negated
            // parents go through `assume_negation` and are sources.
            let is_def =
                !negated && !introduced_definition::declared_new_symbols_opt(parent_ann).is_empty();
            if negated {
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
                if introduced_definition::is_predicate_definition_introduction(parent_ann)
                    && let Some(extended) =
                        complete_definition_iff(&f, parent_ann.unwrap(), ctx.symbols)
                {
                    f = extended;
                }
            }
            premises.push(f);
            premise_is_def.push(is_def);
        }
    }
    ctx.reset_vars();
    let conclusion = lower_annotated_formula(&mut ctx, node.formula);

    // Structural definition_folding: when Vampire emits a step whose
    // sole non-def parent is the unfolded source and the rest are
    // `predicate_definition_introduction` axioms (now iff-completed by
    // the loop above), we can decide the entailment by syntactic
    // unfolding + alpha-equivalence. This sidesteps the saturation
    // search that even multi-second-budget ATPs fail on. The check is
    // bounded by a small work budget so worst case it falls through
    // quickly to the generic ATP ladder.
    //
    // The same shape applies to vampire's `avatar_split_clause`: its
    // premises are one original clause + several `avatar_definition`
    // iff axioms (`spl <=> body`), and the conclusion is the
    // propositional disjunction of the `spl` symbols. Unfolding all
    // `spl` symbols in the conclusion yields a formula α-equivalent
    // to the original clause, exactly as for `definition_folding`.
    if matches!(
        node.inference_rule,
        Some("definition_folding") | Some("avatar_split_clause")
    ) && let Some(outcome) =
        crate::checks::definition_folding::try_check(&premises, &premise_is_def, &conclusion)
    {
        return Prepared::Resolved(outcome);
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
        return Prepared::Resolved(match outcome {
            crate::checks::propositional_sat::PropOutcome::Sound => StepOutcome::Sound,
            crate::checks::propositional_sat::PropOutcome::Unsound if esa => StepOutcome::Unknown(
                "esa step: propositional refutation is not evidence of a faulty \
                 equisatisfiability step"
                    .into(),
            ),
            crate::checks::propositional_sat::PropOutcome::Unsound => StepOutcome::Unsound(
                "propositional SAT solver refuted entailment by premises".into(),
            ),
        });
    }

    // Propositional-abstraction fast-path: treat every argumented atom (and
    // equalities) as an opaque boolean and ask the SAT solver whether the
    // step is *propositionally* valid. Sound over-approximation — an UNSAT
    // abstraction means the step holds in every FOL model, so we accept it.
    // A satisfiable abstraction proves nothing and falls through to the ATP
    // ladder; this path never reports unsoundness. This decides Vampire's
    // `avatar_component_clause` (`spl <=> body` ⊢ `¬body ∨ spl`) and similar
    // CNF-of-iff extractions that the FOL ATPs stall on.
    if crate::checks::propositional_sat::try_propositional_abstraction(&premises, &conclusion) {
        return Prepared::Resolved(StepOutcome::Sound);
    }

    // No fast-path applied: defer the genuine entailment query to Pass 2.
    Prepared::NeedsAtp(AtpStep {
        premises,
        conclusion,
        esa,
        rule: node.inference_rule.map(str::to_owned),
    })
}

/// Run one prepared ATP query and map the verdict to a `StepOutcome`,
/// honouring the esa downgrade rule. A zero budget yields `Unknown`.
fn finish_atp(
    atp: &dyn Atp,
    symbols: &SymbolTable,
    step: &AtpStep,
    budget: Duration,
) -> StepOutcome {
    if budget.is_zero() {
        return StepOutcome::Unknown(format!("ATP budget exhausted (rule={:?})", step.rule));
    }
    match atp.check_step(symbols, &step.premises, &step.conclusion, budget) {
        AtpVerdict::Sound => StepOutcome::Sound,
        AtpVerdict::Unsound if step.esa => StepOutcome::Unknown(format!(
            "esa step: ATP `{}` found a counter-model, but equisatisfiability \
             steps are not entailments, so this is not a fault",
            atp.name()
        )),
        AtpVerdict::Unsound => StepOutcome::Unsound(format!(
            "ATP `{}` refuted entailment by premises",
            atp.name()
        )),
        AtpVerdict::Unknown => {
            StepOutcome::Unknown(format!("no ATP could decide step (rule={:?})", step.rule))
        }
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
        .filter(|&(i, _)| i != p_idx)
        .map(|(_, d)| (*d).clone())
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
    let other_side = if p_positive { Formula::neg(rest) } else { rest };
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

#[cfg(test)]
mod false_guard_tests {
    use super::*;
    use mrs_tptp::parse_tptp;

    fn first_fof(src: &'static str) -> &'static AnnotatedFormula<'static> {
        let prob = Box::leak(Box::new(parse_tptp(src).expect("parse")));
        &prob.formulas[0]
    }

    fn false_node() -> dag::Node<'static> {
        // A `$false`-concluding step tagged with an otherwise-trusted
        // trivial rule and *no* parents — the disconnected_root / trivial_
        // rule_trust shape.
        dag::Node {
            name: "s2",
            role: FormulaRole::Plain,
            parents: vec![],
            negated_parents: vec![],
            inference_rule: Some("fof_simplification"),
            status: None,
            is_false: true,
            formula: first_fof("fof(s2, plain, $false, inference(fof_simplification, [], []))."),
        }
    }

    #[test]
    fn false_concluding_trivial_step_requires_atp() {
        // A `$false`-concluding step tagged with a former trivial rule must
        // reach the entailment check, never a structural shortcut.
        assert!(
            step_needs_atp(&false_node()),
            "a $false-concluding trivial step must require the entailment check"
        );
    }

    #[test]
    fn ordinary_trivial_step_is_budgeted_for_atp() {
        // Under structural verification, former trivial rules are no longer
        // auto-skipped for budgeting: the structural check is tried first in
        // `check_node` (free), but if it cannot confirm the step the ATP must
        // still receive a real budget. So `step_needs_atp` reports `true`.
        let node = dag::Node {
            is_false: false,
            formula: first_fof("fof(s2, plain, p(a), inference(fof_simplification, [], [a]))."),
            ..false_node()
        };
        assert!(step_needs_atp(&node));
    }
}

#[cfg(test)]
mod esa_guard_tests {
    use super::*;
    use crate::atp::{Atp, AtpVerdict};
    use mrs_core::Formula;

    /// A backend that always refutes the entailment — stands in for a
    /// counter-model finder hitting a step whose conclusion is not a logical
    /// consequence of its premises.
    struct AlwaysUnsound;
    impl Atp for AlwaysUnsound {
        fn name(&self) -> &'static str {
            "always_unsound"
        }
        fn check_step(
            &self,
            _: &SymbolTable,
            _: &[Formula],
            _: &Formula,
            _: Duration,
        ) -> AtpVerdict {
            AtpVerdict::Unsound
        }
    }

    fn build_dag(src: &'static str) -> dag::Dag<'static> {
        let prob = Box::leak(Box::new(mrs_tptp::parse_tptp(src).expect("parse")));
        dag::build(prob).expect("dag")
    }

    /// `delegate_to_atp` on the named step with an always-Unsound backend.
    fn outcome_for(src: &'static str, step: &str) -> StepOutcome {
        let dag = build_dag(src);
        let idx = *dag.by_name.get(step).expect("step");
        let mut symbols = SymbolTable::new();
        delegate_to_atp(
            &dag,
            idx,
            &mut symbols,
            &AlwaysUnsound,
            Duration::from_secs(1),
        )
    }

    #[test]
    fn esa_step_downgrades_unsound_to_unknown() {
        // An esa step refuted by the backend must NOT be reported Unsound:
        // equisatisfiability steps are not entailments, so a counter-model is
        // expected and is not a fault. Scoring: 0 (NotVerified), never −1.
        let src = "fof(a, axiom, p(a)).\n\
                   fof(s1, plain, q(b), inference(some_rule, [status(esa)], [a])).\n\
                   fof(s2, plain, $false, inference(some_rule, [status(thm)], [s1])).\n";
        assert!(
            matches!(outcome_for(src, "s1"), StepOutcome::Unknown(_)),
            "esa refutation must downgrade to Unknown"
        );
    }

    #[test]
    fn thm_step_keeps_unsound() {
        // A thm step is a genuine entailment; a refutation IS a fault and must
        // be reported Unsound (→ FailedVerified, +2 on a bad proof).
        let src = "fof(a, axiom, p(a)).\n\
                   fof(s1, plain, q(b), inference(some_rule, [status(thm)], [a])).\n\
                   fof(s2, plain, $false, inference(some_rule, [status(thm)], [s1])).\n";
        assert!(
            matches!(outcome_for(src, "s1"), StepOutcome::Unsound(_)),
            "thm refutation must stay Unsound"
        );
    }
}

#[cfg(test)]
mod budget_tests {
    use super::*;

    #[test]
    fn zero_when_deadline_passed() {
        let past = Instant::now() - Duration::from_secs(1);
        assert_eq!(
            step_budget(past, Duration::from_secs(8), 8, 4, false),
            Duration::ZERO
        );
    }

    #[test]
    fn full_per_step_cap_when_jobs_fit_in_workers() {
        // jobs_left <= workers ⇒ share ≈ full remaining (~30s), capped at the
        // per-step ceiling of 8s.
        let deadline = Instant::now() + Duration::from_secs(30);
        assert_eq!(
            step_budget(deadline, Duration::from_secs(8), 8, 3, false),
            Duration::from_secs(8)
        );
    }

    #[test]
    fn parallel_fair_share_when_many_jobs() {
        // 100 jobs / 8 workers / ~30s ⇒ share ≈ 30·8/100 = 2.4s, under the cap.
        let deadline = Instant::now() + Duration::from_secs(30);
        let b = step_budget(deadline, Duration::from_secs(8), 8, 100, false);
        assert!(
            b >= Duration::from_millis(2200) && b <= Duration::from_millis(2400),
            "got {b:?}"
        );
    }

    #[test]
    fn skolemisation_gets_raised_cap() {
        // One skolemisation job ⇒ share ≈ full remaining, capped at max(8s,25s).
        let deadline = Instant::now() + Duration::from_secs(30);
        assert_eq!(
            step_budget(deadline, Duration::from_secs(8), 8, 1, true),
            Duration::from_secs(25)
        );
    }

    #[test]
    fn floors_sub_second_share_to_one_second() {
        // 1000 jobs / 8 workers / ~30s ⇒ share ≈ 0.24s, floored to 1s because
        // meaningful wall time remains.
        let deadline = Instant::now() + Duration::from_secs(30);
        assert_eq!(
            step_budget(deadline, Duration::from_secs(8), 8, 1000, false),
            Duration::from_secs(1)
        );
    }
}
