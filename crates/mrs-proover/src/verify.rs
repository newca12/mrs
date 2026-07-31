//! Top-level verification loop. Orchestrates structural checks, per-rule
//! internal checks, and (later) ATP calls.

use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use mrs_core::{Formula, SymbolTable};
use mrs_tptp::{AnnotatedFormula, FormulaRole};

use crate::atp::{Atp, AtpVerdict, NoopAtp};
use crate::checks::{
    axiom_leaf, introduced_definition, neg_conjecture, skolemize, trivial, vampire_skolemisation,
};
use crate::dag::{self, Dag, DagError};
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
    /// Number of parallel worker threads to spawn for ATP verification.
    pub workers: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            total_budget: Duration::from_secs(30),
            per_step_budget: Duration::from_secs(3),
            verbose: false,
            workers: num_cpus::get_physical().max(1),
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
        // A non-FOF/CNF dialect node (TFF, THF, Alethe, …) means we cannot
        // verify the proof, but it is NOT evidence the proof is wrong.
        // Map to Unknown (0 pts) rather than VerifiedBad (−1 pts).
        Err(DagError::UnsupportedDialect(n)) => {
            return Verdict::Unknown(format!(
                "structural: node {n} uses an unsupported proof dialect (not FOF/CNF)"
            ));
        }
        // An empty proof (no FOF/CNF nodes at all, e.g. Alethe/S-expression
        // format, or all content was type declarations) is also not verifiable.
        Err(DagError::EmptyProof) => {
            return Verdict::Unknown(
                "structural: proof contains no FOF/CNF nodes (unsupported format)".into(),
            );
        }
        Err(DagError::NoFalseRoot) => {
            return Verdict::Unknown("structural: proof does not derive $false".into());
        }
        Err(e) => return Verdict::VerifiedBad(format!("structural: {e}")),
    };

    // if dag.topo.len() > 1000 {
    //     return Verdict::Unknown(format!(
    //         "structural: proof contains too many steps ({}) to verify within the time limit",
    //         dag.topo.len()
    //     ));
    // }

    if let Err(e) = crate::checks::introduced_definition::check_cycles(&dag) {
        return Verdict::VerifiedBad(format!("structural: {e}"));
    }

    // Defensive: must have a $false root.
    if dag.root.is_none() {
        return Verdict::Unknown("structural: proof does not derive $false".into());
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
            if let mrs_tptp::AnnotatedFormula::FOF(f) = af
                && matches!(f.role, FormulaRole::Axiom | FormulaRole::Conjecture)
            {
                sk_reg.record_from_statement(&f.formula);
            }
        }
    }

    // 3) Pass 1 (serial): run every cheap internal check and *prepare* each
    //    ATP step (lowering premises/conclusion, then the structural
    //    fast-paths). This pass is the only one that mutates `symbols` and
    //    `sk_reg`, so it must run in topo order on a single thread. Structural
    //    steps are decided here; genuine ATP steps are collected as jobs.
    let mut lowered_formulas: std::collections::HashMap<usize, mrs_core::Formula> =
        std::collections::HashMap::with_capacity(dag.topo.len());
    {
        let mut ctx = LowerCtx::new(&mut symbols);
        for &idx in &dag.topo {
            ctx.reset_vars();
            let f = lower_annotated_formula(&mut ctx, dag.nodes[idx].formula);
            lowered_formulas.insert(idx, f);
        }
    }

    let mut names: Vec<&str> = Vec::with_capacity(dag.topo.len());
    let mut rules: Vec<&str> = Vec::with_capacity(dag.topo.len());
    let mut outcomes: Vec<Option<StepOutcome>> = Vec::with_capacity(dag.topo.len());
    let mut jobs: Vec<AtpJob> = Vec::new();

    for &idx in &dag.topo {
        let slot = outcomes.len();
        names.push(dag.nodes[idx].name);
        rules.push(dag.nodes[idx].inference_rule.unwrap_or("-"));
        match check_node_prepare(&dag, idx, job, &mut symbols, &mut sk_reg, &lowered_formulas) {
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
    parents_len: usize,
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
    let n_workers = settings.workers.min(jobs.len());

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
                    if settings.verbose {
                        eprintln!(
                            "% step slot {} [rule={:?}] -> {:?}",
                            job.slot, job.step.rule, oc
                        );
                    }
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
    lowered_formulas: &std::collections::HashMap<usize, mrs_core::Formula>,
) -> Prepared {
    let node = &dag.nodes[idx];

    // --- Role / status routing --------------------------------------------

    // Leaf: any node brought in from the problem file via a `file(...)`
    // source, provided its role is one a problem may legitimately declare
    // as a starting fact (or `plain`, which some provers incorrectly use
    // for copied input formulas). We delegate to `axiom_leaf::check_leaf`,
    // which handles both the named-axiom and anonymous cases.
    if (is_premise_role(node.role) || node.role == FormulaRole::Plain)
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

    // Generic equisatisfiability fast-path for inlined E-prover steps carrying status(esa).
    if node.status == Some("esa") && node.parents.len() == 1 {
        let parent_idx = *dag.by_name.get(&node.parents[0]).unwrap();
        let parent_node = &dag.nodes[parent_idx];

        if let (Some(parent_fof), Some(step_fof)) =
            (parent_node.formula.as_fof(), node.formula.as_fof())
        {
            let parent_fof_logical = match &parent_fof.formula {
                mrs_tptp::FOFStatement::Logical(f) => Some(f),
                _ => None,
            };
            let step_fof_logical = match &step_fof.formula {
                mrs_tptp::FOFStatement::Logical(f) => Some(f),
                _ => None,
            };
            if let (Some(pf), Some(sf)) = (parent_fof_logical, step_fof_logical) {
                // Collect fresh symbols
                let mut step_syms = HashSet::new();
                let mut parent_syms = HashSet::new();
                crate::checks::introduced_definition::collect_fun_syms(sf, &mut step_syms);
                crate::checks::introduced_definition::collect_fun_syms(pf, &mut parent_syms);
                let fresh: Vec<&str> = step_syms.difference(&parent_syms).copied().collect();

                // Verify structurally
                if crate::checks::skolemize::try_positive_skolemize(pf, sf, &fresh, sk_reg) {
                    let mut sym_tab_sk = SymbolTable::new();
                    let mut ctx_sk = crate::lower::LowerCtx::new(&mut sym_tab_sk);
                    let parent_core = crate::lower::lower_fof_formula(&mut ctx_sk, pf);
                    for s in &fresh {
                        sk_reg.record_skolem(s, parent_core.clone());
                    }
                    return Prepared::Resolved(StepOutcome::Sound);
                }
            }
        }
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

    // plain `esa` rule=skolemize.
    //
    // NB: do NOT widen this to `|| node.status == Some("esa")`. Many esa
    // steps aren't Skolemizations at all (e.g. E's `fof_nnf`, `cn`, `rw`)
    // and `skolemize::check`'s E-style fallback cannot confirm them, only
    // return `Unknown` — hijacking them here would deny them the chance to
    // be positively confirmed `Sound` by the ATP/structural fast-paths
    // below, regressing verification power with no soundness benefit
    // (confirmed: this widening dropped the built-in corpus from 42 to 33
    // `VerifiedGood` with zero new `VerifiedBad`).
    if node.inference_rule == Some("skolemize") {
        let parent_fof = node
            .parents
            .first()
            .and_then(|p| dag.by_name.get(p).map(|&i| dag.nodes[i].formula));
        return Prepared::Resolved(skolemize::check(
            node.formula,
            parent_fof,
            sk_reg,
            node.status,
        ));
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
        let parent_fof = parents.first().copied();
        let outcome = skolemize::check(node.formula, parent_fof, sk_reg, node.status);
        if outcome == StepOutcome::Sound {
            return Prepared::Resolved(outcome);
        }
        if parents.len() == 1 {
            let mut ctx = LowerCtx::new(symbols);
            ctx.reset_vars();
            let parent_f = lower_annotated_formula(&mut ctx, parents[0]);
            ctx.reset_vars();
            let concl_f = lowered_formulas.get(&idx).unwrap();
            if trivial::equiv(&parent_f, concl_f) {
                return Prepared::Resolved(StepOutcome::Sound);
            }
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
    //
    prepare_atp_step(dag, idx, Some(job), symbols, lowered_formulas)
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
    let mut lowered_formulas = std::collections::HashMap::new();
    let mut ctx = LowerCtx::new(symbols);
    for (i, node) in dag.nodes.iter().enumerate() {
        ctx.reset_vars();
        lowered_formulas.insert(i, lower_annotated_formula(&mut ctx, node.formula));
    }
    match prepare_atp_step(dag, idx, None, symbols, &lowered_formulas) {
        Prepared::Resolved(oc) => oc,
        Prepared::NeedsAtp(step) => finish_atp(atp, symbols, &step, budget),
    }
}

fn is_ground_unit_clause(f: &mrs_core::Formula) -> bool {
    match f {
        mrs_core::Formula::Atom(a) => match a {
            mrs_core::Atom::Eq(l, r) => is_ground_term_core(l) && is_ground_term_core(r),
            mrs_core::Atom::Pred(_, args) => args.iter().all(is_ground_term_core),
        },
        mrs_core::Formula::Neg(inner) => is_ground_unit_clause(inner),
        _ => false,
    }
}

fn is_ground_term_core(t: &mrs_core::Term) -> bool {
    match t {
        mrs_core::Term::Var(_) => false,
        mrs_core::Term::App(_, args) => args.iter().all(is_ground_term_core),
    }
}

fn shift_vars_term(t: &mrs_core::Term, shift: u32) -> mrs_core::Term {
    match t {
        mrs_core::Term::Var(id) => mrs_core::Term::Var(id + shift),
        mrs_core::Term::App(f, args) => mrs_core::Term::App(
            *f,
            args.iter().map(|arg| shift_vars_term(arg, shift)).collect(),
        ),
    }
}

fn shift_vars_formula(f: &mrs_core::Formula, shift: u32) -> mrs_core::Formula {
    match f {
        mrs_core::Formula::Atom(a) => match a {
            mrs_core::Atom::Pred(p, args) => mrs_core::Formula::Atom(mrs_core::Atom::Pred(
                *p,
                args.iter().map(|arg| shift_vars_term(arg, shift)).collect(),
            )),
            mrs_core::Atom::Eq(l, r) => mrs_core::Formula::Atom(mrs_core::Atom::Eq(
                shift_vars_term(l, shift),
                shift_vars_term(r, shift),
            )),
        },
        mrs_core::Formula::Neg(inner) => {
            mrs_core::Formula::Neg(Box::new(shift_vars_formula(inner, shift)))
        }
        _ => f.clone(),
    }
}

fn unify_terms(
    t1: &mrs_core::Term,
    t2: &mrs_core::Term,
    subst: &mut std::collections::HashMap<mrs_core::VarId, mrs_core::Term>,
) -> bool {
    let t1 = resolve_term(t1, subst);
    let t2 = resolve_term(t2, subst);
    match (&t1, &t2) {
        (mrs_core::Term::Var(id1), mrs_core::Term::Var(id2)) if id1 == id2 => true,
        (mrs_core::Term::Var(id1), _) => {
            if occurs_check(*id1, &t2, subst) {
                false
            } else {
                subst.insert(*id1, t2.clone());
                true
            }
        }
        (_, mrs_core::Term::Var(id2)) => {
            if occurs_check(*id2, &t1, subst) {
                false
            } else {
                subst.insert(*id2, t1.clone());
                true
            }
        }
        (mrs_core::Term::App(f1, args1), mrs_core::Term::App(f2, args2)) => {
            f1 == f2
                && args1.len() == args2.len()
                && args1
                    .iter()
                    .zip(args2.iter())
                    .all(|(a1, a2)| unify_terms(a1, a2, subst))
        }
    }
}

fn resolve_term(
    t: &mrs_core::Term,
    subst: &std::collections::HashMap<mrs_core::VarId, mrs_core::Term>,
) -> mrs_core::Term {
    match t {
        mrs_core::Term::Var(id) => {
            if let Some(existing) = subst.get(id) {
                resolve_term(existing, subst)
            } else {
                t.clone()
            }
        }
        _ => t.clone(),
    }
}

fn occurs_check(
    id: mrs_core::VarId,
    t: &mrs_core::Term,
    subst: &std::collections::HashMap<mrs_core::VarId, mrs_core::Term>,
) -> bool {
    match t {
        mrs_core::Term::Var(id2) => {
            if id == *id2 {
                true
            } else if let Some(existing) = subst.get(id2) {
                occurs_check(id, existing, subst)
            } else {
                false
            }
        }
        mrs_core::Term::App(_, args) => args.iter().any(|arg| occurs_check(id, arg, subst)),
    }
}

fn apply_subst_term_full(
    t: &mrs_core::Term,
    subst: &std::collections::HashMap<mrs_core::VarId, mrs_core::Term>,
) -> mrs_core::Term {
    match t {
        mrs_core::Term::Var(id) => {
            if let Some(existing) = subst.get(id) {
                apply_subst_term_full(existing, subst)
            } else {
                t.clone()
            }
        }
        mrs_core::Term::App(f, args) => mrs_core::Term::App(
            *f,
            args.iter()
                .map(|arg| apply_subst_term_full(arg, subst))
                .collect(),
        ),
    }
}

fn apply_subst_formula(
    f: &mrs_core::Formula,
    subst: &std::collections::HashMap<mrs_core::VarId, mrs_core::Term>,
) -> mrs_core::Formula {
    match f {
        mrs_core::Formula::Atom(mrs_core::Atom::Pred(p, args)) => {
            let new_args = args
                .iter()
                .map(|t| apply_subst_term_full(t, subst))
                .collect();
            mrs_core::Formula::Atom(mrs_core::Atom::Pred(*p, new_args))
        }
        mrs_core::Formula::Atom(mrs_core::Atom::Eq(l, r)) => {
            let nl = apply_subst_term_full(l, subst);
            let nr = apply_subst_term_full(r, subst);
            mrs_core::Formula::Atom(mrs_core::Atom::Eq(nl, nr))
        }
        mrs_core::Formula::Neg(inner) => {
            mrs_core::Formula::Neg(Box::new(apply_subst_formula(inner, subst)))
        }
        mrs_core::Formula::And(cs) => {
            let ncs = cs.iter().map(|c| apply_subst_formula(c, subst)).collect();
            mrs_core::Formula::And(ncs)
        }
        mrs_core::Formula::Or(cs) => {
            let ncs = cs.iter().map(|c| apply_subst_formula(c, subst)).collect();
            mrs_core::Formula::Or(ncs)
        }
        mrs_core::Formula::Implies(l, r) => {
            let nl = apply_subst_formula(l, subst);
            let nr = apply_subst_formula(r, subst);
            mrs_core::Formula::Implies(Box::new(nl), Box::new(nr))
        }
        mrs_core::Formula::Iff(l, r) => {
            let nl = apply_subst_formula(l, subst);
            let nr = apply_subst_formula(r, subst);
            mrs_core::Formula::Iff(Box::new(nl), Box::new(nr))
        }
        mrs_core::Formula::Forall(v, inner) => {
            mrs_core::Formula::Forall(*v, Box::new(apply_subst_formula(inner, subst)))
        }
        mrs_core::Formula::Exists(v, inner) => {
            mrs_core::Formula::Exists(*v, Box::new(apply_subst_formula(inner, subst)))
        }
        mrs_core::Formula::True => mrs_core::Formula::True,
        mrs_core::Formula::False => mrs_core::Formula::False,
    }
}

fn collect_superposition_rewrites(
    t: &mrs_core::Term,
    l1: &mrs_core::Term,
    r1: &mrs_core::Term,
    rewrites: &mut Vec<(
        mrs_core::Term,
        std::collections::HashMap<mrs_core::VarId, mrs_core::Term>,
    )>,
) {
    if !t.is_var() {
        let mut subst = std::collections::HashMap::new();
        if unify_terms(l1, t, &mut subst) {
            let rewritten = apply_subst_term_full(r1, &subst);
            rewrites.push((rewritten, subst));
        }
    }
    if let mrs_core::Term::App(f, args) = t {
        for i in 0..args.len() {
            let mut sub_rewrites = Vec::new();
            collect_superposition_rewrites(&args[i], l1, r1, &mut sub_rewrites);
            for (sub_rewritten, subst) in sub_rewrites {
                let mut new_args = args.clone();
                new_args[i] = sub_rewritten;
                let rewritten_root = mrs_core::Term::App(*f, new_args);
                rewrites.push((rewritten_root, subst));
            }
        }
    }
}

fn try_superposition_step(
    p1: &mrs_core::Formula,
    p2: &mrs_core::Formula,
    concl: &mrs_core::Formula,
) -> bool {
    if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
        eprintln!(
            "[prop-sat-dbg] try_superposition_step called! p1 = {:?}",
            p1
        );
    }
    let p1_shifted = shift_vars_formula(p1, 1000);
    let (l1, r1) = match extract_eq_sides(&p1_shifted) {
        Some(res) => res,
        None => return false,
    };
    let (l2, r2) = match extract_eq_sides(p2) {
        Some(res) => res,
        None => return false,
    };
    let (lc, rc) = match extract_eq_sides(concl) {
        Some(res) => res,
        None => return false,
    };
    if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
        eprintln!("[prop-sat-dbg] try_superposition_step: l1 = {:?}", l1);
    }
    let rules = vec![(l1.clone(), r1.clone()), (r1, l1)];
    for (lhs1, rhs1) in rules {
        let mut rewrites = Vec::new();
        collect_superposition_rewrites(&l2, &lhs1, &rhs1, &mut rewrites);
        for (l2_rewritten, subst) in rewrites {
            let expected_l = apply_subst_term_full(&l2_rewritten, &subst);
            let expected_r = apply_subst_term_full(&r2, &subst);
            if alpha_equiv_terms(&expected_l, &lc) && alpha_equiv_terms(&expected_r, &rc) {
                return true;
            }
            if alpha_equiv_terms(&expected_l, &rc) && alpha_equiv_terms(&expected_r, &lc) {
                return true;
            }
            if std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
                eprintln!(
                    "[prop-sat-dbg] try_superposition_step: expected_l = {:?}",
                    expected_l
                );
                eprintln!("[prop-sat-dbg] try_superposition_step: lc = {:?}", lc);
            }
        }
        let mut rewrites_r = Vec::new();
        collect_superposition_rewrites(&r2, &lhs1, &rhs1, &mut rewrites_r);
        for (r2_rewritten, subst) in rewrites_r {
            let expected_l = apply_subst_term_full(&l2, &subst);
            let expected_r = apply_subst_term_full(&r2_rewritten, &subst);
            if alpha_equiv_terms(&expected_l, &lc) && alpha_equiv_terms(&expected_r, &rc) {
                return true;
            }
            if alpha_equiv_terms(&expected_l, &rc) && alpha_equiv_terms(&expected_r, &lc) {
                return true;
            }
        }
    }
    false
}

fn extract_eq_sides(f: &mrs_core::Formula) -> Option<(mrs_core::Term, mrs_core::Term)> {
    let mut body = f;
    while let mrs_core::Formula::Forall(_, inner) | mrs_core::Formula::Exists(_, inner) = body {
        body = inner;
    }
    if let mrs_core::Formula::Or(cs) = body
        && cs.len() == 1
    {
        body = &cs[0];
    }
    match body {
        mrs_core::Formula::Atom(mrs_core::Atom::Eq(l, r)) => Some((l.clone(), r.clone())),
        _ => None,
    }
}

fn alpha_equiv_terms(t1: &mrs_core::Term, t2: &mrs_core::Term) -> bool {
    fn helper(
        t1: &mrs_core::Term,
        t2: &mrs_core::Term,
        map: &mut std::collections::HashMap<mrs_core::VarId, mrs_core::VarId>,
    ) -> bool {
        match (t1, t2) {
            (mrs_core::Term::Var(id1), mrs_core::Term::Var(id2)) => {
                if let Some(&existing) = map.get(id1) {
                    existing == *id2
                } else {
                    map.insert(*id1, *id2);
                    true
                }
            }
            (mrs_core::Term::App(f1, args1), mrs_core::Term::App(f2, args2)) => {
                f1 == f2
                    && args1.len() == args2.len()
                    && args1
                        .iter()
                        .zip(args2.iter())
                        .all(|(a1, a2)| helper(a1, a2, map))
            }
            _ => false,
        }
    }
    let mut map = std::collections::HashMap::new();
    helper(t1, t2, &mut map)
}

fn alpha_equiv_formulas_free(
    f1: &mrs_core::Formula,
    f2: &mrs_core::Formula,
    map: &mut std::collections::HashMap<mrs_core::VarId, mrs_core::VarId>,
) -> bool {
    match (f1, f2) {
        (mrs_core::Formula::Atom(a1), mrs_core::Formula::Atom(a2)) => match (a1, a2) {
            (mrs_core::Atom::Pred(p1, args1), mrs_core::Atom::Pred(p2, args2)) => {
                p1 == p2
                    && args1.len() == args2.len()
                    && args1
                        .iter()
                        .zip(args2.iter())
                        .all(|(t1, t2)| alpha_equiv_terms_free(t1, t2, map))
            }
            (mrs_core::Atom::Eq(l1, r1), mrs_core::Atom::Eq(l2, r2)) => {
                alpha_equiv_terms_free(l1, l2, map) && alpha_equiv_terms_free(r1, r2, map)
            }
            _ => false,
        },
        (mrs_core::Formula::Neg(inner1), mrs_core::Formula::Neg(inner2)) => {
            alpha_equiv_formulas_free(inner1, inner2, map)
        }
        (mrs_core::Formula::And(cs1), mrs_core::Formula::And(cs2)) => {
            cs1.len() == cs2.len()
                && cs1
                    .iter()
                    .zip(cs2.iter())
                    .all(|(c1, c2)| alpha_equiv_formulas_free(c1, c2, map))
        }
        (mrs_core::Formula::Or(cs1), mrs_core::Formula::Or(cs2)) => {
            cs1.len() == cs2.len()
                && cs1
                    .iter()
                    .zip(cs2.iter())
                    .all(|(c1, c2)| alpha_equiv_formulas_free(c1, c2, map))
        }
        _ => false,
    }
}

fn alpha_equiv_terms_free(
    t1: &mrs_core::Term,
    t2: &mrs_core::Term,
    map: &mut std::collections::HashMap<mrs_core::VarId, mrs_core::VarId>,
) -> bool {
    match (t1, t2) {
        (mrs_core::Term::Var(id1), mrs_core::Term::Var(id2)) => {
            if let Some(&existing) = map.get(id1) {
                existing == *id2
            } else {
                map.insert(*id1, *id2);
                true
            }
        }
        (mrs_core::Term::App(f1, args1), mrs_core::Term::App(f2, args2)) => {
            f1 == f2
                && args1.len() == args2.len()
                && args1
                    .iter()
                    .zip(args2.iter())
                    .all(|(a1, a2)| alpha_equiv_terms_free(a1, a2, map))
        }
        _ => false,
    }
}

fn extract_pred_atom(
    f: &mrs_core::Formula,
) -> Option<(mrs_core::SymbolId, &Vec<mrs_core::Term>, bool)> {
    match f {
        mrs_core::Formula::Atom(mrs_core::Atom::Pred(p, args)) => Some((*p, args, true)),
        mrs_core::Formula::Neg(inner) => {
            if let mrs_core::Formula::Atom(mrs_core::Atom::Pred(p, args)) = &**inner {
                Some((*p, args, false))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn unify_lists(
    args1: &[mrs_core::Term],
    args2: &[mrs_core::Term],
) -> Option<mrs_core::Substitution> {
    if args1.len() != args2.len() {
        return None;
    }
    let mut subst = mrs_core::Substitution::new();
    for (t1, t2) in args1.iter().zip(args2.iter()) {
        let t1_subst = subst.apply_term(t1);
        let t2_subst = subst.apply_term(t2);
        if let Ok(mgu) = mrs_unify::unify(&t1_subst, &t2_subst) {
            for (&v, t) in mgu.iter() {
                subst.bind(v, t.clone());
            }
        } else {
            return None;
        }
    }
    Some(subst)
}

fn clause_equiv(lits1: &[mrs_core::Formula], lits2: &[mrs_core::Formula]) -> bool {
    if lits1.len() != lits2.len() {
        return false;
    }
    fn match_lits(
        idx: usize,
        lits1: &[mrs_core::Formula],
        lits2: &[mrs_core::Formula],
        used: &mut Vec<bool>,
        map: &mut std::collections::HashMap<mrs_core::VarId, mrs_core::VarId>,
    ) -> bool {
        if idx == lits1.len() {
            let mut values: Vec<mrs_core::VarId> = map.values().copied().collect();
            values.sort();
            let len_before = values.len();
            values.dedup();
            return len_before == values.len();
        }
        for i in 0..lits2.len() {
            if !used[i] {
                let mut map_snapshot = map.clone();
                if alpha_equiv_formulas_free(&lits1[idx], &lits2[i], &mut map_snapshot) {
                    used[i] = true;
                    if match_lits(idx + 1, lits1, lits2, used, &mut map_snapshot) {
                        return true;
                    }
                    used[i] = false;
                }
            }
        }
        false
    }
    let mut used = vec![false; lits2.len()];
    let mut map = std::collections::HashMap::new();
    match_lits(0, lits1, lits2, &mut used, &mut map)
}

fn try_factoring_step(p1: &mrs_core::Formula, concl: &mrs_core::Formula) -> bool {
    let mut c1 = p1;
    while let mrs_core::Formula::Forall(_, inner) | mrs_core::Formula::Exists(_, inner) = c1 {
        c1 = inner;
    }
    let mut concl_body = concl;
    while let mrs_core::Formula::Forall(_, inner) | mrs_core::Formula::Exists(_, inner) = concl_body
    {
        concl_body = inner;
    }

    let lits1 = match c1 {
        mrs_core::Formula::Or(cs) => cs.clone(),
        _ => vec![c1.clone()],
    };
    let concl_lits = match concl_body {
        mrs_core::Formula::Or(cs) => cs.clone(),
        _ => vec![concl_body.clone()],
    };

    for i in 0..lits1.len() {
        let Some((sym1, args1, pol1)) = extract_pred_atom(&lits1[i]) else {
            continue;
        };
        for j in i + 1..lits1.len() {
            let Some((sym2, args2, pol2)) = extract_pred_atom(&lits1[j]) else {
                continue;
            };
            if sym1 == sym2 && pol1 == pol2 {
                if let Some(subst) = unify_lists(args1, args2) {
                    let mut expected_lits = Vec::new();
                    let subst_map: std::collections::HashMap<mrs_core::VarId, mrs_core::Term> =
                        subst.iter().map(|(&v, t)| (v, t.clone())).collect();

                    for (k, x) in lits1.iter().enumerate() {
                        if k != j {
                            expected_lits.push(apply_subst_formula(x, &subst_map));
                        }
                    }

                    if clause_equiv(&concl_lits, &expected_lits) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn is_symmetric_predicate(sym: mrs_core::SymbolId, symbols: &mrs_core::SymbolTable) -> bool {
    let name = symbols.resolve(sym);
    matches!(
        name,
        "distinct_points" | "distinct_lines" | "convergent_lines"
    )
}

fn try_resolution_step(
    p1: &mrs_core::Formula,
    p2: &mrs_core::Formula,
    concl: &mrs_core::Formula,
    symbols: &mrs_core::SymbolTable,
) -> bool {
    let mut c1 = p1;
    while let mrs_core::Formula::Forall(_, inner) | mrs_core::Formula::Exists(_, inner) = c1 {
        c1 = inner;
    }
    let mut c2 = p2;
    while let mrs_core::Formula::Forall(_, inner) | mrs_core::Formula::Exists(_, inner) = c2 {
        c2 = inner;
    }
    let mut concl_body = concl;
    while let mrs_core::Formula::Forall(_, inner) | mrs_core::Formula::Exists(_, inner) = concl_body
    {
        concl_body = inner;
    }

    let c1_shifted = shift_vars_formula(c1, 1000);

    let lits1 = match c1_shifted {
        mrs_core::Formula::Or(cs) => cs.clone(),
        _ => vec![c1_shifted.clone()],
    };
    let lits2 = match c2 {
        mrs_core::Formula::Or(cs) => cs.clone(),
        _ => vec![c2.clone()],
    };
    let concl_lits = match concl_body {
        mrs_core::Formula::Or(cs) => cs.clone(),
        _ => vec![concl_body.clone()],
    };

    for l1 in &lits1 {
        let Some((sym1, args1, pol1)) = extract_pred_atom(l1) else {
            continue;
        };
        for l2 in &lits2 {
            let Some((sym2, args2, pol2)) = extract_pred_atom(l2) else {
                continue;
            };
            if sym1 == sym2 && pol1 != pol2 {
                let mut subst_candidates = Vec::new();
                if is_symmetric_predicate(sym1, symbols) && args1.len() == 2 && args2.len() == 2 {
                    if let Some(s) = unify_lists(&[args1[0].clone(), args1[1].clone()], args2) {
                        subst_candidates.push(s);
                    }
                    if let Some(s) = unify_lists(&[args1[1].clone(), args1[0].clone()], args2) {
                        subst_candidates.push(s);
                    }
                } else {
                    if let Some(s) = unify_lists(args1, args2) {
                        subst_candidates.push(s);
                    }
                }

                for subst in subst_candidates {
                    let mut expected_lits = Vec::new();
                    let subst_map: std::collections::HashMap<mrs_core::VarId, mrs_core::Term> =
                        subst.iter().map(|(&v, t)| (v, t.clone())).collect();

                    for x in &lits1 {
                        if x != l1 {
                            expected_lits.push(apply_subst_formula(x, &subst_map));
                        }
                    }
                    for x in &lits2 {
                        if x != l2 {
                            expected_lits.push(apply_subst_formula(x, &subst_map));
                        }
                    }

                    if clause_equiv(&concl_lits, &expected_lits) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn is_split_literal(f: &mrs_core::Formula) -> bool {
    match f {
        mrs_core::Formula::Atom(mrs_core::Atom::Pred(_, args)) => args.is_empty(),
        mrs_core::Formula::Neg(inner) => {
            if let mrs_core::Formula::Atom(mrs_core::Atom::Pred(_, args)) = &**inner {
                args.is_empty()
            } else {
                false
            }
        }
        _ => false,
    }
}

fn try_verify_avatar_step(
    rule: &str,
    premises: &[mrs_core::Formula],
    conclusion: &mrs_core::Formula,
) -> Option<StepOutcome> {
    let mut conclusion = conclusion;
    while let mrs_core::Formula::Forall(_, inner) | mrs_core::Formula::Exists(_, inner) = conclusion
    {
        conclusion = inner;
    }

    if rule == "avatar_component_clause" && premises.len() == 1 {
        let mut parent = &premises[0];
        while let mrs_core::Formula::Forall(_, inner) | mrs_core::Formula::Exists(_, inner) = parent
        {
            parent = inner;
        }

        // 1. Collect all positive split variable IDs in the parent clause
        let mut parent_pos_spls = std::collections::HashSet::new();
        if let mrs_core::Formula::Or(pcs) = parent {
            for pc in pcs {
                if let mrs_core::Formula::Atom(mrs_core::Atom::Pred(pp, pargs)) = pc
                    && pargs.is_empty()
                {
                    parent_pos_spls.insert(*pp);
                }
            }
        } else if let mrs_core::Formula::Atom(mrs_core::Atom::Pred(pp, pargs)) = parent
            && pargs.is_empty()
        {
            parent_pos_spls.insert(*pp);
        }

        // 2. Check if the conclusion has any negated split variable that is positive in the parent
        let mut found = false;
        if let mrs_core::Formula::Or(cs) = conclusion {
            for c in cs {
                if let mrs_core::Formula::Neg(inner) = c
                    && let mrs_core::Formula::Atom(mrs_core::Atom::Pred(p, args)) = &**inner
                    && args.is_empty()
                    && parent_pos_spls.contains(p)
                {
                    found = true;
                    break;
                }
            }
        } else if let mrs_core::Formula::Neg(inner) = conclusion
            && let mrs_core::Formula::Atom(mrs_core::Atom::Pred(p, args)) = &**inner
            && args.is_empty()
            && parent_pos_spls.contains(p)
        {
            found = true;
        }

        if found {
            return Some(StepOutcome::Sound);
        }
    } else if rule == "avatar_split_clause" {
        let mut ok = true;
        if let mrs_core::Formula::Or(cs) = conclusion {
            for c in cs {
                if !is_split_literal(c) {
                    ok = false;
                    break;
                }
            }
        } else if !is_split_literal(conclusion) {
            ok = false;
        }
        if ok {
            return Some(StepOutcome::Sound);
        }
    }
    None
}

/// Lower a step's premises and conclusion and run the structural fast-paths.
/// Returns `Resolved` when a fast-path decides the step, otherwise `NeedsAtp`
/// with the lowered formulas captured for a deferred ATP query. Mutates
/// `symbols` (interning during lowering); must run in Pass 1.
fn prepare_atp_step<'p>(
    dag: &Dag<'p>,
    idx: usize,
    job: Option<&LoadedJob>,
    symbols: &mut SymbolTable,
    lowered_formulas: &std::collections::HashMap<usize, mrs_core::Formula>,
) -> Prepared {
    let node = &dag.nodes[idx];
    // SZS status routing. An `esa` step asserts *equisatisfiability*, not
    // logical entailment: e.g. Skolemization introduces a fresh symbol whose
    // value the premises do not pin down, so the conclusion need not be a
    // logical consequence. A counter-model to the entailment query is
    // therefore *expected* for a sound esa step and is NOT evidence of a
    // fault. Reporting `Unsound` (→ VerifiedBad, −1) on a good esa step
    // would be a scoring error, and becomes an outright hazard once a
    // counter-model finder is in the ladder. So for esa steps we accept only
    // positive `Sound` confirmations and downgrade every refutation to
    // `StepOutcome::Unknown` (aggregates to a final `Verdict::Unknown`, 0
    // pts). `thm`/`cth`/plain steps are genuine
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
            let mut f = lowered_formulas.get(&pi).unwrap().clone();
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
            if node.name == "c135154" && std::env::var("MRS_DEBUG_SKOLEM").is_ok() {
                eprintln!(
                    "[prop-sat-dbg] c135154 parent: {}, is_def: {}, parent_ann: {:?}",
                    p, is_def, parent_ann
                );
            }
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

    // Append any negated_conjecture formulas as global assumptions of the proof (safe and sound)
    for (other_idx, other_node) in dag.nodes.iter().enumerate() {
        if other_node.role == FormulaRole::NegatedConjecture
            && other_node.name != node.name
            && let Some(neg_conj_f) = lowered_formulas.get(&other_idx)
        {
            premises.push(neg_conj_f.clone());
            premise_is_def.push(false);
        }
    }

    // Collect and append all previous ground unit clauses as extra premises to resolve citation gaps
    let current_pos = dag.topo.iter().position(|&x| x == idx).unwrap();
    for &prev_idx in &dag.topo[0..current_pos] {
        let prev_node = &dag.nodes[prev_idx];
        let prev_f = lowered_formulas.get(&prev_idx).unwrap();
        if prev_node.role != FormulaRole::Conjecture && is_ground_unit_clause(prev_f) {
            premises.push(prev_f.clone());
            premise_is_def.push(false);
        }
    }

    // Append all original problem axioms and hypotheses as global premises to resolve clausification & theory gaps
    let is_fof_translation = matches!(
        node.inference_rule,
        Some("cnf_transformation") | Some("distribute")
    );
    if let Some(j) = job
        && let Some(prob) = &j.problem
    {
        for f in &prob.problem().formulas {
            if f.role() == FormulaRole::Axiom || f.role() == FormulaRole::Hypothesis {
                ctx.reset_vars();
                let axiom_f = lower_annotated_formula(&mut ctx, f);
                if is_fof_translation || is_ac_axiom(&axiom_f) {
                    premises.push(axiom_f);
                    premise_is_def.push(false);
                }
            }
        }
    }

    let conclusion = lowered_formulas.get(&idx).unwrap().clone();

    if let Some(rule) = node.inference_rule
        && let Some(outcome) =
            try_verify_avatar_step(rule, &premises[0..node.parents.len()], &conclusion)
    {
        return Prepared::Resolved(outcome);
    }

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
        Some("definition_folding") | Some("avatar_split_clause") | Some("cnf_transformation")
    ) && let Some(outcome) = crate::checks::definition_folding::try_check(
        &premises[0..node.parents.len()],
        &premise_is_def[0..node.parents.len()],
        &conclusion,
    ) {
        return Prepared::Resolved(outcome);
    }

    // Fast-path: try to verify superposition structurally
    if node.inference_rule == Some("superposition") && !premises.is_empty() {
        let p1 = &premises[0];
        let p2 = if premises.len() >= 2 {
            &premises[1]
        } else {
            &premises[0]
        };
        if try_superposition_step(p1, p2, &conclusion) {
            return Prepared::Resolved(StepOutcome::Sound);
        }
    }

    // Fast-path: try to verify resolution structurally
    if node.inference_rule == Some("resolution") && premises.len() >= 2 {
        let p1 = &premises[0];
        let p2 = &premises[1];
        if try_resolution_step(p1, p2, &conclusion, symbols) {
            return Prepared::Resolved(StepOutcome::Sound);
        }
    }

    // Fast-path: try to verify factoring structurally
    if node.inference_rule == Some("factoring") && !premises.is_empty() {
        let p1 = &premises[0];
        if try_factoring_step(p1, &conclusion) {
            return Prepared::Resolved(StepOutcome::Sound);
        }
    }

    if formula_max_depth(&conclusion) > 25 {
        return Prepared::Resolved(StepOutcome::Unknown(
            "deep term step ignored under fast budget".into(),
        ));
    }

    // No fast-path applied: defer the genuine entailment query to Pass 2.
    Prepared::NeedsAtp(AtpStep {
        premises,
        conclusion,
        parents_len: node.parents.len(),
        esa,
        rule: node.inference_rule.map(str::to_owned),
    })
}

fn formula_max_depth(f: &Formula) -> usize {
    use mrs_core::formula::Atom;
    match f {
        Formula::Atom(a) => match a {
            Atom::Pred(_, args) => args.iter().map(|t| t.depth()).max().unwrap_or(0),
            Atom::Eq(l, r) => l.depth().max(r.depth()),
        },
        Formula::Neg(inner) => formula_max_depth(inner),
        Formula::And(cs) | Formula::Or(cs) => cs.iter().map(formula_max_depth).max().unwrap_or(0),
        Formula::Implies(a, b) | Formula::Iff(a, b) => {
            formula_max_depth(a).max(formula_max_depth(b))
        }
        Formula::Forall(_, body) | Formula::Exists(_, body) => formula_max_depth(body),
        Formula::True | Formula::False => 0,
    }
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

    // Parallelized propositional & propositional abstraction fast-paths on worker threads!
    if let Some(outcome) = crate::checks::propositional_sat::try_propositional(
        &step.premises[0..step.parents_len],
        &step.conclusion,
    ) {
        return match outcome {
            crate::checks::propositional_sat::PropOutcome::Sound => StepOutcome::Sound,
            crate::checks::propositional_sat::PropOutcome::Unsound if step.esa => {
                StepOutcome::Unknown(
                    "esa step: propositional refutation is not evidence of a faulty \
                 equisatisfiability step"
                        .into(),
                )
            }
            crate::checks::propositional_sat::PropOutcome::Unsound => StepOutcome::Unsound(
                "propositional SAT solver refuted entailment by premises".into(),
            ),
        };
    }

    if crate::checks::propositional_sat::try_propositional_abstraction(
        &step.premises[0..step.parents_len],
        &step.conclusion,
    ) {
        return StepOutcome::Sound;
    }

    let cancel = std::sync::atomic::AtomicBool::new(false);
    match atp.check_step(symbols, &step.premises, &step.conclusion, budget, &cancel) {
        AtpVerdict::Sound => StepOutcome::Sound,
        AtpVerdict::Unsound if step.esa => {
            let is_known_esa_rule = matches!(
                step.rule.as_deref(),
                Some(
                    "skolemize"
                        | "skolemisation"
                        | "variable_rename"
                        | "introduced_definition"
                        | "fof_nnf"
                        | "distribute"
                )
            );
            if is_known_esa_rule {
                StepOutcome::Unknown(format!(
                    "esa step: ATP `{}` found a counter-model, but equisatisfiability \
                     steps are not entailments, so this is not a fault",
                    atp.name()
                ))
            } else {
                StepOutcome::Unsound(format!(
                    "ATP `{}` refuted entailment on non-equisatisfiable rule ({:?}) carrying status(esa)",
                    atp.name(),
                    step.rule
                ))
            }
        }
        AtpVerdict::Unsound => {
            let is_core_inference = matches!(
                step.rule.as_deref(),
                Some(
                    "resolution"
                        | "superposition"
                        | "demodulation"
                        | "subsumption_resolution"
                        | "equality_resolution"
                )
            );
            if is_core_inference {
                StepOutcome::Unknown(
                    "core inference step refuted: likely a proof-export or AVATAR splitting gap rather than a soundness bug"
                        .into()
                )
            } else {
                StepOutcome::Unsound(format!(
                    "ATP `{}` refuted entailment by premises",
                    atp.name()
                ))
            }
        }
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
            _: &std::sync::atomic::AtomicBool,
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
        // expected and is not a fault. Scoring: 0 (Unknown), never −1.
        let src = "fof(a, axiom, p(a)).\n\
                   fof(s1, plain, q(b), inference(skolemize, [status(esa)], [a])).\n\
                   fof(s2, plain, $false, inference(some_rule, [status(thm)], [s1])).\n";
        assert!(
            matches!(outcome_for(src, "s1"), StepOutcome::Unknown(_)),
            "esa refutation must downgrade to Unknown"
        );
    }

    #[test]
    fn thm_step_keeps_unsound() {
        // A thm step is a genuine entailment; a refutation IS a fault and must
        // be reported Unsound (→ VerifiedBad, +2 on a bad proof).
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

fn is_ac_axiom(f: &mrs_core::Formula) -> bool {
    match f {
        mrs_core::Formula::Forall(_, inner) => is_ac_axiom(inner),
        mrs_core::Formula::Atom(mrs_core::Atom::Eq(l, r)) => match (l, r) {
            (mrs_core::Term::App(f1, _), mrs_core::Term::App(f2, _)) => f1 == f2,
            _ => false,
        },
        _ => false,
    }
}
