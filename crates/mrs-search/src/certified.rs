//! Bounded certification for the function-free EPR ordered-resolution fragment.
//!
//! This module deliberately certifies a small fragment instead of making a
//! completeness claim about the full given-clause engine.  The supported
//! fragment is:
//!
//! - function-free relational EPR clauses, exhaustively grounded over their
//!   finite constant domain (`Saturated` is enabled for this EPR fragment
//!   only; anything else fails closed as `GaveUp`);
//! - predicate atoms only (no equality);
//! - no AVATAR assertions or formula-level search nodes;
//! - KBO (with a validated positive-weight, total precedence) or LPO (with a
//!   validated total precedence), applied in that implementation order; and
//! - no pruning or heuristic simplification.
//!
//! For that finite fragment, ordered resolution cannot introduce new terms or
//! atoms.  We compute both the ordered closure and an independent unrestricted
//! ground-resolution closure (the double-closure reference is retained by
//! design and is not replaced by a trace replay of the given-clause loop).
//! A positive result is returned only when both closures agree: either both
//! derive the empty clause (refutation) or both saturate without it.  If the
//! ordered closure ever disagrees with the unrestricted reference,
//! certification fails closed.

use std::collections::HashMap as StdHashMap;
use std::time::{Duration, Instant};

use crate::{CompletenessWitness, HashSet, SearchResult, SearchStats, TermOrdering};
use mrs_calculus::ordering::{SymbolConfig, TermComparison};
use mrs_core::clause::{Clause, ClauseId, ClauseIdGen, ClauseSource};
use mrs_core::formula::Atom;
use mrs_core::subst::Substitution;
use mrs_core::symbol::{SymbolId, SymbolTable};
use mrs_core::term::Term;
use mrs_core::term_bank::TermBank;
use mrs_index::literal_index::LiteralIndex;

/// Resource caps for the bounded certifier. `MAX_ATOMS` and
/// `MAX_GROUND_INSTANCES` bound Tier 1 (double ordered-resolution closure);
/// `TIER2_MAX_ATOMS` and `TIER2_MAX_GROUND_INSTANCES` bound Tier 2
/// (SAT-backed satisfiability via CaDiCaL, see `certified_sat`). Tier-2
/// bounds were sized from TRACE_CERTIFY measurements: they admit the next
/// slice of closure-bound groundings while keeping the materialized vec and
/// the solver arena within a few hundred MB. `MAX_CLAUSES` and
/// `MAX_INFERENCES` bound Tier-1 closure memory and work after grounding,
/// and the per-run time limit bounds everything else. Anything beyond the
/// Tier-2 caps fails closed as `Limit`, never as saturation.
const MAX_ATOMS: usize = 4096;
const MAX_CLAUSES: usize = 100_000;
const MAX_INFERENCES: u64 = 1_000_000;
const MAX_GROUND_INSTANCES: usize = 500_000;
const TIER2_MAX_ATOMS: usize = 16_384;
const TIER2_MAX_GROUND_INSTANCES: usize = 2_000_000;
/// Tier-3 constant-subset search bounds: subset sizes, per-size try caps,
/// and the per-subset grounding cap. Subsets stay small so each Tier-1 run
/// is milliseconds; everything shares the run deadline.
const TIER3_MAX_SUBSET_SIZE: usize = 3;
const TIER3_MAX_SINGLETON_TRIES: usize = 200;
const TIER3_MAX_PAIR_TRIES: usize = 100;
const TIER3_MAX_TRIPLE_TRIES: usize = 30;
const TIER3_MAX_SUBSET_INSTANCES: usize = 50_000;

/// Diagnostic logging for certification sizing, gated on `TRACE_CERTIFY=1`.
/// Follows the `TRACE_LRS` / `TRACE_BCE` precedent: refusal reasons plus
/// the problem sizes that triggered them, so cap changes stay data-driven.
pub(crate) fn trace_certify(message: String) {
    if std::env::var_os("TRACE_CERTIFY").is_some() {
        eprintln!("[CERTIFY] {message}");
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CertificationFailure {
    Unsupported(&'static str),
    Limit(&'static str),
    OrderedClosureMismatch,
    /// Tier-2 SAT solving proved unsatisfiability: sound but uncertifiable
    /// in the SAT-only tier (no FRAT-to-TSTP elaborator). The router may
    /// still try Tier-3 subset search for a TSTP-ancestry refutation.
    Tier2Unsat,
}

pub(crate) struct CertifiedGroundReport {
    pub result: SearchResult,
    pub stats: SearchStats,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClosureStatus {
    Saturated,
    Refuted,
}

struct Closure {
    clauses: Vec<Clause>,
    status: ClosureStatus,
    inferences: u64,
}

/// Certify and run ordered resolution for the bounded ground fragment.
pub(crate) fn certify_ground_ordered_resolution(
    clauses: &[Clause],
    provenance: &[Clause],
    symbols: &SymbolTable,
    ordering: &TermOrdering,
    id_gen: &mut ClauseIdGen,
    time_limit: Duration,
) -> Result<CertifiedGroundReport, CertificationFailure> {
    let mut proof_symbols = symbols.clone();
    let deadline = Instant::now() + time_limit;
    let constants = collect_grounding_constants(clauses, &mut proof_symbols)?;
    // Full grounding first; only size limits divert to Tier 3 below —
    // fragment errors propagate because subsets cannot fix them.
    let grounded =
        match ground_with_constants(clauses, &constants, id_gen, TIER2_MAX_GROUND_INSTANCES) {
            Err(CertificationFailure::Limit(_)) => None,
            Err(other) => return Err(other),
            Ok(grounded) => Some(grounded),
        };
    if let Some(grounded) = grounded {
        let atoms = collect_fragment_atoms(&grounded.clauses)?;
        // Tier router. Tier 1 (double ordered-resolution closure) handles
        // small groundings; Tier 2 (SAT-backed satisfiability, no ordering
        // needed) handles the next size slice.
        let tier1 = atoms.len() <= MAX_ATOMS && grounded.clauses.len() <= MAX_GROUND_INSTANCES;
        let tier2 =
            atoms.len() <= TIER2_MAX_ATOMS && grounded.clauses.len() <= TIER2_MAX_GROUND_INSTANCES;
        if tier1 {
            return run_tier1(
                &grounded,
                provenance,
                ordering,
                &proof_symbols,
                id_gen,
                deadline,
                "tier1",
            );
        }
        if tier2 {
            if !is_pure_relational_epr_input(&grounded.originals) {
                return Err(CertificationFailure::Unsupported(
                    "saturation is certified for EPR inputs only",
                ));
            }
            trace_certify(format!(
                "sat_tier=2 grounded={} atoms={}",
                grounded.clauses.len(),
                atoms.len()
            ));
            match crate::certified_sat::certify_sat_backed(
                &grounded.clauses,
                &atoms,
                deadline.saturating_duration_since(Instant::now()),
            ) {
                Err(CertificationFailure::Tier2Unsat) => {
                    // Sound but proof-less: a small constant core may still
                    // refute with TSTP ancestry via Tier 3 below.
                }
                outcome => return outcome,
            }
        } else {
            trace_certify(format!(
                "refuse=tier_size grounded={} atoms={}",
                grounded.clauses.len(),
                atoms.len()
            ));
        }
    }
    // Tier 3: the full grounding is infeasible or proved UNSAT without a
    // proof — search small constant subsets for a certified refutation.
    tier3_subset_unsat(
        clauses,
        provenance,
        &constants,
        ordering,
        &proof_symbols,
        id_gen,
        deadline,
    )
}

/// Tier 1: double ordered-resolution closure with agreement over an
/// already-routed grounding. Shared by the normal path and Tier-3 subset
/// tries (each subset carries full Tier-1 guarantees, including the
/// agreement check and TSTP-ancestry proofs).
#[allow(clippy::too_many_arguments)]
fn run_tier1(
    grounded: &GroundedInputs,
    provenance: &[Clause],
    ordering: &TermOrdering,
    proof_symbols: &SymbolTable,
    id_gen: &mut ClauseIdGen,
    deadline: Instant,
    context: &'static str,
) -> Result<CertifiedGroundReport, CertificationFailure> {
    let atoms = collect_fragment_atoms(&grounded.clauses)?;
    validate_ordering_kind(ordering)?;
    if atoms.len() > MAX_ATOMS {
        trace_certify(format!("refuse=atom_limit atoms={}", atoms.len()));
        return Err(CertificationFailure::Limit("ground atom limit exceeded"));
    }
    validate_ground_order(ordering, &atoms)?;

    let mut ordered_id_gen = id_gen.clone();
    let ordered = closure_indexed(
        &grounded.clauses,
        ordering,
        true,
        &mut ordered_id_gen,
        deadline,
    )?;

    let mut reference_id_gen = id_gen.clone();
    let reference = closure_indexed(
        &grounded.clauses,
        ordering,
        false,
        &mut reference_id_gen,
        deadline,
    )?;

    if ordered.status != reference.status {
        return Err(CertificationFailure::OrderedClosureMismatch);
    }

    trace_certify(format!(
        "certified {context} status={:?} grounded={} atoms={} ordered_clauses={} ordered_inferences={} reference_clauses={} reference_inferences={}",
        ordered.status,
        grounded.clauses.len(),
        atoms.len(),
        ordered.clauses.len(),
        ordered.inferences,
        reference.clauses.len(),
        reference.inferences,
    ));

    *id_gen = ordered_id_gen;
    let stats = SearchStats {
        processed: ordered.clauses.len() as u64,
        generated: ordered.inferences,
        ..SearchStats::default()
    };

    match ordered.status {
        ClosureStatus::Refuted => {
            let empty = ordered
                .clauses
                .last()
                .filter(|clause| clause.is_empty())
                .ok_or(CertificationFailure::Unsupported(
                    "ordered closure lost its empty clause",
                ))?;
            let mut store: StdHashMap<ClauseId, Clause> = StdHashMap::new();
            for clause in provenance {
                store.insert(clause.id, clause.clone());
            }
            for clause in &grounded.originals {
                store.insert(clause.id, clause.clone());
            }
            for clause in &ordered.clauses {
                store.insert(clause.id, clause.clone());
            }
            let proof = mrs_proof::extract::extract_proof(empty.id, &store);
            let tstp = mrs_proof::tstp::format_tstp(&proof, proof_symbols);
            Ok(CertifiedGroundReport {
                result: SearchResult::Refutation(empty.id, tstp),
                stats,
            })
        }
        ClosureStatus::Saturated => {
            // `Saturated` is enabled for the EPR fragment only. The fragment
            // checks above already reject equality, function terms, and
            // formula/AVATAR clauses, but re-check the pre-grounding inputs
            // here so a future refactor cannot accidentally certify
            // saturation for a non-EPR problem.
            if !is_pure_relational_epr_input(&grounded.originals) {
                return Err(CertificationFailure::Unsupported(
                    "saturation is certified for EPR inputs only",
                ));
            }
            Ok(CertifiedGroundReport {
                result: SearchResult::Saturated(CompletenessWitness::ground_ordered_resolution()),
                stats,
            })
        }
    }
}

/// Returns `true` iff every input clause is pure relational EPR: predicate
/// atoms only, with each argument a variable or a constant. Formula-level
/// and AVATAR clauses are not EPR for certification purposes.
fn is_pure_relational_epr_input(clauses: &[Clause]) -> bool {
    !clauses.is_empty()
        && clauses.iter().all(|clause| {
            clause.formula.is_none()
                && clause.avatar.is_empty()
                && !clause.literals.is_empty()
                && clause.literals.iter().all(|literal| match &literal.atom {
                    Atom::Pred(_, args) => args.iter().all(term_is_epr_constant_or_var),
                    Atom::Eq(_, _) => false,
                })
        })
}

fn term_is_epr_constant_or_var(term: &Term) -> bool {
    match term {
        Term::Var(_) => true,
        Term::App(_, args) => args.is_empty(),
    }
}

/// Tier 3: constant-subset unsatisfiability search for groundings that are
/// infeasible in full (size refusals) or proved UNSAT without a proof
/// (Tier-2 SAT outcomes).
///
/// Soundness: subset instances are a subset of full instances, so an empty
/// clause derived from a subset — via the full Tier-1 double closure with
/// agreement and TSTP ancestry — is a valid refutation of the whole
/// problem. Subset *saturation* proves nothing and is skipped over: this
/// tier can only refute, never certify satisfiability. That asymmetry is
/// load-bearing and unit-tested.
///
/// Completeness is explicitly absent: unsatisfiability may genuinely need
/// many constants (pigeonhole-style), and tries are bounded. Exhaustion
/// fails closed.
///
/// Outcome encoding: `Ok(report)` is a certified refutation (returned);
/// `Err(())` means "try the next subset"; `Err` with `timed_out` set means
/// the shared deadline expired (fail closed immediately).
#[allow(clippy::too_many_arguments)]
fn tier3_subset_unsat(
    clauses: &[Clause],
    provenance: &[Clause],
    constants: &[SymbolId],
    ordering: &TermOrdering,
    proof_symbols: &SymbolTable,
    id_gen: &mut ClauseIdGen,
    deadline: Instant,
) -> Result<CertifiedGroundReport, CertificationFailure> {
    // Ordering applies to every Tier-1 subset run: reject once here instead
    // of once per subset.
    validate_ordering_kind(ordering)?;
    let biased = bias_order_constants(clauses, constants);
    // Per-clause variable counts, computed once: subset estimates stay
    // arithmetic, never materialized speculatively.
    let var_counts: Vec<usize> = clauses
        .iter()
        .map(|clause| clause.free_vars().len())
        .collect();

    // k = 1: every constant alone (bias order), then bounded biased k = 2, 3.
    let mut subsets: Vec<Vec<SymbolId>> = Vec::new();
    for (index, constant) in biased.iter().enumerate() {
        if index >= TIER3_MAX_SINGLETON_TRIES {
            break;
        }
        subsets.push(vec![*constant]);
    }
    'pairs: for (i, a) in biased.iter().enumerate() {
        for b in biased.iter().skip(i + 1) {
            if subsets.len() >= TIER3_MAX_SINGLETON_TRIES + TIER3_MAX_PAIR_TRIES {
                break 'pairs;
            }
            subsets.push(vec![*a, *b]);
        }
    }
    'triples: for (i, a) in biased.iter().enumerate() {
        for (j, b) in biased.iter().enumerate().skip(i + 1) {
            for c in biased.iter().skip(j + 1) {
                if subsets.len()
                    >= TIER3_MAX_SINGLETON_TRIES + TIER3_MAX_PAIR_TRIES + TIER3_MAX_TRIPLE_TRIES
                {
                    break 'triples;
                }
                subsets.push(vec![*a, *b, *c]);
            }
        }
    }
    debug_assert!(subsets.iter().all(|s| s.len() <= TIER3_MAX_SUBSET_SIZE));

    let mut tries = 0u64;
    for subset in &subsets {
        if Instant::now() >= deadline {
            trace_certify(format!("tier3_exhausted tries={tries} (deadline)"));
            return Err(CertificationFailure::Limit(
                "certification time limit exceeded",
            ));
        }
        // Arithmetic pre-check: skip subsets whose grounding alone would
        // exceed the per-subset budget.
        let mut estimated = 0usize;
        let mut fits = true;
        for &vars in &var_counts {
            match subset.len().checked_pow(vars as u32) {
                Some(instances)
                    if estimated
                        .checked_add(instances)
                        .is_some_and(|total| total <= TIER3_MAX_SUBSET_INSTANCES) =>
                {
                    estimated += instances;
                }
                _ => {
                    fits = false;
                    break;
                }
            }
        }
        if !fits {
            continue;
        }
        tries += 1;
        // Vocabulary restriction: keep only clauses whose constants all
        // lie in the subset (variable-only clauses are always kept — they
        // ground over the subset). Dropping premises is sound for the
        // refutation direction: a proof from fewer premises stays valid
        // for the full problem. It also keeps each try cheap when the
        // input is ground-heavy.
        let restricted: Vec<Clause> = clauses
            .iter()
            .filter(|clause| clause_mentions_only(clause, subset))
            .cloned()
            .collect();
        if restricted.is_empty() {
            continue;
        }
        let subset_grounded =
            match ground_with_constants(&restricted, subset, id_gen, TIER3_MAX_SUBSET_INSTANCES) {
                Ok(grounded) => grounded,
                Err(_) => continue,
            };
        match run_tier1(
            &subset_grounded,
            provenance,
            ordering,
            proof_symbols,
            id_gen,
            deadline,
            "tier3-sub",
        ) {
            Ok(report) if matches!(report.result, SearchResult::Refutation(..)) => {
                trace_certify(format!(
                    "tier3_found subset_size={} tries={tries}",
                    subset.len()
                ));
                return Ok(report);
            }
            // Subset saturation proves nothing about the full problem, and
            // subset failures (limits, mismatch) only rule out this subset.
            _ => {}
        }
    }
    trace_certify(format!("tier3_exhausted tries={tries}"));
    if Instant::now() >= deadline {
        return Err(CertificationFailure::Limit(
            "certification time limit exceeded",
        ));
    }
    Err(CertificationFailure::Limit(
        "subset instantiation budget exhausted",
    ))
}

/// Order constants for subset enumeration: goal-connected constants
/// (`distance == 0`, i.e. negated-conjecture descendants) first, then by
/// decreasing occurrence frequency, stably by first-seen position. Small
/// unsatisfiable cores usually involve the goal vocabulary, so bias order
/// is hit-rate, not soundness: every subset is decided independently.
fn bias_order_constants(clauses: &[Clause], constants: &[SymbolId]) -> Vec<SymbolId> {
    let mut position: StdHashMap<SymbolId, usize> = StdHashMap::new();
    for (index, constant) in constants.iter().enumerate() {
        position.insert(*constant, index);
    }
    let mut goal = HashSet::default();
    let mut frequency: StdHashMap<SymbolId, u64> = StdHashMap::new();
    for clause in clauses {
        for literal in &clause.literals {
            let Atom::Pred(_, args) = &literal.atom else {
                continue;
            };
            for arg in args {
                count_constants(arg, &mut frequency, clause.distance == 0, &mut goal);
            }
        }
    }
    let mut biased = constants.to_vec();
    biased.sort_by_key(|constant| {
        let goal_rank = if goal.contains(constant) { 0 } else { 1 };
        let frequency = frequency.get(constant).copied().unwrap_or(0);
        (
            goal_rank,
            std::cmp::Reverse(frequency),
            position.get(constant).copied().unwrap_or(usize::MAX),
        )
    });
    biased
}

fn count_constants(
    term: &Term,
    frequency: &mut StdHashMap<SymbolId, u64>,
    is_goal: bool,
    goal: &mut HashSet<SymbolId>,
) {
    if let Term::App(symbol, args) = term {
        if args.is_empty() {
            *frequency.entry(*symbol).or_default() += 1;
            if is_goal {
                goal.insert(*symbol);
            }
        } else {
            for arg in args {
                count_constants(arg, frequency, is_goal, goal);
            }
        }
    }
}

/// Returns `true` iff every constant occurring in `clause` is a member of
/// `subset` (variables are unrestricted). Used for Tier-3 vocabulary
/// restriction: kept clauses ground over the subset using only subset
/// constants, so their instances are a subset of the full instances.
fn clause_mentions_only(clause: &Clause, subset: &[SymbolId]) -> bool {
    clause.literals.iter().all(|literal| {
        let args: &[Term] = match &literal.atom {
            Atom::Pred(_, args) => args,
            Atom::Eq(..) => return false,
        };
        args.iter().all(|arg| term_mentions_only(arg, subset))
    })
}

fn term_mentions_only(term: &Term, subset: &[SymbolId]) -> bool {
    match term {
        Term::Var(_) => true,
        Term::App(symbol, args) => {
            if args.is_empty() {
                subset.contains(symbol)
            } else {
                // Fragment-clean inputs have no function terms; treat any
                // nested term conservatively as outside the subset.
                false
            }
        }
    }
}

struct GroundedInputs {
    clauses: Vec<Clause>,
    originals: Vec<Clause>,
}

/// Collect the finite constant domain of EPR clauses: fragment checks plus
/// the sorted distinct constants (or one fresh domain constant when the
/// input has none). Tier 3 reuses this to enumerate constant subsets.
fn collect_grounding_constants(
    clauses: &[Clause],
    symbols: &mut SymbolTable,
) -> Result<Vec<SymbolId>, CertificationFailure> {
    let mut constants = Vec::new();
    let mut seen_constants = HashSet::default();
    for clause in clauses {
        if clause.formula.is_some() || !clause.avatar.is_empty() {
            return Err(CertificationFailure::Unsupported(
                "formula-level or AVATAR clause in ground certification",
            ));
        }
        for literal in &clause.literals {
            let Atom::Pred(_, args) = &literal.atom else {
                return Err(CertificationFailure::Unsupported(
                    "equality is outside the certified fragment",
                ));
            };
            for arg in args {
                collect_epr_constants(arg, &mut constants, &mut seen_constants)?;
            }
        }
    }
    if constants.is_empty() {
        constants.push(symbols.fresh_symbol("$cert_domain"));
    }
    constants.sort_unstable();
    Ok(constants)
}

/// Exhaustively instantiate clauses over `constants`, refusing past
/// `instance_cap` before materializing. The estimate is exact: one output
/// per ground input clause plus `constants^vars` per variable clause.
fn ground_with_constants(
    clauses: &[Clause],
    constants: &[SymbolId],
    id_gen: &mut ClauseIdGen,
    instance_cap: usize,
) -> Result<GroundedInputs, CertificationFailure> {
    let originals = clauses.to_vec();
    let mut grounded = Vec::new();
    let mut estimated_instances = 0usize;
    for clause in clauses {
        let mut vars: Vec<_> = clause.free_vars().into_iter().collect();
        vars.sort_unstable();
        if vars.is_empty() {
            estimated_instances = estimated_instances.saturating_add(1);
            grounded.push(clause.clone());
            continue;
        }
        let Some(instances) = constants.len().checked_pow(vars.len() as u32) else {
            trace_certify(format!(
                "refuse=instance_count_overflow vars={} constants={}",
                vars.len(),
                constants.len()
            ));
            return Err(CertificationFailure::Limit(
                "ground instance count overflow",
            ));
        };
        estimated_instances = estimated_instances.saturating_add(instances);
        if estimated_instances > instance_cap {
            trace_certify(format!(
                "refuse=instance_limit estimated={estimated_instances} vars={} constants={}",
                vars.len(),
                constants.len()
            ));
            return Err(CertificationFailure::Limit(
                "ground instance limit exceeded",
            ));
        }
        let mut substitution = Substitution::new();
        instantiate_clause(
            clause,
            &vars,
            constants,
            0,
            &mut substitution,
            id_gen,
            &mut grounded,
        );
    }

    Ok(GroundedInputs {
        clauses: grounded,
        originals,
    })
}

fn collect_epr_constants(
    term: &Term,
    constants: &mut Vec<SymbolId>,
    seen: &mut HashSet<SymbolId>,
) -> Result<(), CertificationFailure> {
    let Term::App(symbol, args) = term else {
        return Ok(());
    };
    if !args.is_empty() {
        return Err(CertificationFailure::Unsupported(
            "function terms are outside the certified EPR fragment",
        ));
    }
    if seen.insert(*symbol) {
        constants.push(*symbol);
    }
    Ok(())
}

fn instantiate_clause(
    clause: &Clause,
    vars: &[mrs_core::term::VarId],
    constants: &[SymbolId],
    depth: usize,
    substitution: &mut Substitution,
    id_gen: &mut ClauseIdGen,
    output: &mut Vec<Clause>,
) {
    if depth == vars.len() {
        let literals = clause
            .literals
            .iter()
            .map(|literal| substitution.apply_literal(literal))
            .collect::<Vec<_>>();
        let mut grounded = Clause::new(
            id_gen.next(),
            literals,
            ClauseSource::Inference {
                rule: "instantiation",
                parents: vec![clause.id].into(),
            },
        );
        grounded.distance = clause.distance;
        output.push(grounded);
        return;
    }

    for &constant in constants {
        substitution.bind(vars[depth], Term::constant(constant));
        instantiate_clause(
            clause,
            vars,
            constants,
            depth + 1,
            substitution,
            id_gen,
            output,
        );
    }
}

fn is_kbo(ordering: &TermOrdering) -> bool {
    matches!(ordering, TermOrdering::KBO | TermOrdering::CustomKBO(_))
}

fn is_lpo(ordering: &TermOrdering) -> bool {
    matches!(ordering, TermOrdering::LPO | TermOrdering::CustomLPO(_))
}

fn validate_ordering_kind(ordering: &TermOrdering) -> Result<(), CertificationFailure> {
    if matches!(ordering, TermOrdering::CustomACKBO(_, _)) {
        return Err(CertificationFailure::Unsupported(
            "AC ordering is outside the certified fragment",
        ));
    }
    if !(is_kbo(ordering) || is_lpo(ordering)) {
        return Err(CertificationFailure::Unsupported(
            "only KBO and LPO are certified for ordered ground resolution",
        ));
    }
    Ok(())
}

/// Fragment checks shared by both tiers: non-empty input, no
/// formula/AVATAR clauses, predicate atoms only, consistent arities.
/// Returns the distinct ground atoms. Ordering validation and size caps are
/// tier-specific and applied by the caller.
fn collect_fragment_atoms(clauses: &[Clause]) -> Result<Vec<Atom>, CertificationFailure> {
    if clauses.is_empty() {
        return Err(CertificationFailure::Unsupported("empty input clause set"));
    }
    let mut atoms = HashSet::default();
    let mut arities = StdHashMap::<SymbolId, usize>::new();
    for clause in clauses {
        if clause.formula.is_some() || !clause.avatar.is_empty() {
            return Err(CertificationFailure::Unsupported(
                "formula-level or AVATAR clause in ground certification",
            ));
        }
        for literal in &clause.literals {
            let Atom::Pred(predicate, args) = &literal.atom else {
                return Err(CertificationFailure::Unsupported(
                    "equality is outside the certified fragment",
                ));
            };
            record_arity(&mut arities, *predicate, args.len())?;
            for arg in args {
                record_term_arities(arg, &mut arities)?;
            }
            atoms.insert(literal.atom.clone());
        }
    }
    Ok(atoms.into_iter().collect())
}

fn record_arity(
    arities: &mut StdHashMap<SymbolId, usize>,
    symbol: SymbolId,
    arity: usize,
) -> Result<(), CertificationFailure> {
    if let Some(previous) = arities.insert(symbol, arity)
        && previous != arity
    {
        return Err(CertificationFailure::Unsupported(
            "symbol used with inconsistent arity",
        ));
    }
    Ok(())
}

fn record_term_arities(
    term: &Term,
    arities: &mut StdHashMap<SymbolId, usize>,
) -> Result<(), CertificationFailure> {
    let Term::App(symbol, args) = term else {
        return Ok(());
    };
    record_arity(arities, *symbol, args.len())?;
    for arg in args {
        record_term_arities(arg, arities)?;
    }
    Ok(())
}

fn validate_ground_order(
    ordering: &TermOrdering,
    atoms: &[Atom],
) -> Result<(), CertificationFailure> {
    let config = ordering.symbol_config();
    validate_symbol_config(ordering, &config, atoms)?;
    let terms: Vec<Term> = atoms.iter().map(atom_term).collect();

    for (i, left) in terms.iter().enumerate() {
        for (j, right) in terms.iter().enumerate() {
            if i == j {
                continue;
            }
            if !matches!(
                ordering.compare(left, right),
                TermComparison::Greater | TermComparison::Less
            ) {
                return Err(CertificationFailure::Unsupported(
                    "ordering is not a strict total order on ground atoms",
                ));
            }
        }
    }

    for a in &terms {
        for b in &terms {
            for c in &terms {
                if ordering.compare(a, b) == TermComparison::Greater
                    && ordering.compare(b, c) == TermComparison::Greater
                    && ordering.compare(a, c) != TermComparison::Greater
                {
                    return Err(CertificationFailure::Unsupported(
                        "ordering is not transitive on ground atoms",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validate_symbol_config(
    ordering: &TermOrdering,
    config: &SymbolConfig,
    atoms: &[Atom],
) -> Result<(), CertificationFailure> {
    // KBO is validated first, then LPO: KBO needs positive weights plus a
    // total precedence, while LPO needs only the total precedence (weights
    // are irrelevant to LPO comparison).
    if is_kbo(ordering) && config.w0 == 0 {
        return Err(CertificationFailure::Unsupported(
            "KBO variable weight must be positive",
        ));
    }
    let mut symbols = HashSet::default();
    for atom in atoms {
        let Atom::Pred(predicate, args) = atom else {
            unreachable!("ground validation rejects equality before ordering validation")
        };
        symbols.insert(*predicate);
        for arg in args {
            collect_term_symbols(arg, &mut symbols);
        }
    }
    let mut precedence = HashSet::default();
    for symbol in symbols {
        if is_kbo(ordering) && config.symbol_weight(symbol) == 0 {
            return Err(CertificationFailure::Unsupported(
                "KBO symbol weight must be positive",
            ));
        }
        if !precedence.insert(config.symbol_precedence(symbol)) {
            return Err(CertificationFailure::Unsupported(
                "precedence must be total on the input signature",
            ));
        }
    }
    Ok(())
}

fn collect_term_symbols(term: &Term, symbols: &mut HashSet<SymbolId>) {
    if let Term::App(symbol, args) = term {
        symbols.insert(*symbol);
        for arg in args {
            collect_term_symbols(arg, symbols);
        }
    }
}

fn atom_term(atom: &Atom) -> Term {
    let Atom::Pred(predicate, args) = atom else {
        unreachable!("ground validation rejects equality before atom ordering")
    };
    Term::app(*predicate, args.clone())
}

/// Linear all-pairs closure. Retained as the independent reference for the
/// indexed-vs-linear equivalence unit tests; the certification path uses
/// [`closure_indexed`] for both closures.
#[cfg(test)]
fn closure_linear(
    input: &[Clause],
    ordering: &TermOrdering,
    ordered: bool,
    id_gen: &mut ClauseIdGen,
    deadline: Instant,
) -> Result<Closure, CertificationFailure> {
    let mut clauses = Vec::new();
    let mut seen = HashSet::default();
    for clause in input {
        // The input normalization pass is linear but unbounded in the input
        // size, so it honors the same deadline as the pair loop below.
        if Instant::now() >= deadline {
            trace_certify(format!(
                "refuse=closure_time ordered={ordered} clauses={} inferences=0",
                clauses.len()
            ));
            return Err(CertificationFailure::Limit(
                "certification time limit exceeded",
            ));
        }
        let Some(normalized) = normalize_clause(clause.clone()) else {
            continue;
        };
        let key = clause_key(&normalized);
        if seen.insert(key) {
            if normalized.is_empty() {
                clauses.push(normalized);
                return Ok(Closure {
                    clauses,
                    status: ClosureStatus::Refuted,
                    inferences: 0,
                });
            }
            clauses.push(normalized);
        }
    }

    // Pin down the exact pair-generation semantics for the optimizations
    // below: pairs are (current, previous) with previous_index < index, the
    // current selection is fixed per outer iteration, and clauses derived
    // mid-iteration are appended but never revisited within the same outer
    // iteration. Borrowing `previous` instead of cloning it, hoisting the
    // current selection out of the inner loop, and checking the deadline
    // once per outer iteration preserve this order exactly while removing
    // one full clause clone per pair.
    let mut inferences = 0;
    let mut index = 0;
    while index < clauses.len() {
        if Instant::now() >= deadline {
            trace_certify(format!(
                "refuse=closure_time ordered={ordered} clauses={} inferences={inferences}",
                clauses.len()
            ));
            return Err(CertificationFailure::Limit(
                "certification time limit exceeded",
            ));
        }
        let current = clauses[index].clone();
        let current_selection = if ordered {
            selected_literals(&current, ordering)
        } else {
            all_literal_indices(&current)
        };
        for previous_index in 0..index {
            // Scope the borrow so it ends before any push below.
            let derived_batch = {
                let previous = &clauses[previous_index];
                let previous_selection = if ordered {
                    selected_literals(previous, ordering)
                } else {
                    all_literal_indices(previous)
                };
                resolve_ground_pair(
                    &current,
                    previous,
                    &current_selection,
                    &previous_selection,
                    id_gen,
                )
            };
            for derived in derived_batch {
                inferences += 1;
                if inferences > MAX_INFERENCES {
                    trace_certify(format!(
                        "refuse=inference_limit ordered={ordered} inferences={inferences}"
                    ));
                    return Err(CertificationFailure::Limit(
                        "ground inference limit exceeded",
                    ));
                }
                let Some(derived) = normalize_clause(derived) else {
                    continue;
                };
                if derived.is_empty() {
                    clauses.push(derived);
                    return Ok(Closure {
                        clauses,
                        status: ClosureStatus::Refuted,
                        inferences,
                    });
                }
                if seen.insert(clause_key(&derived)) {
                    clauses.push(derived);
                    if clauses.len() > MAX_CLAUSES {
                        trace_certify(format!(
                            "refuse=clause_limit ordered={ordered} clauses={} inferences={inferences}",
                            clauses.len()
                        ));
                        return Err(CertificationFailure::Limit("ground clause limit exceeded"));
                    }
                }
            }
        }
        index += 1;
    }
    Ok(Closure {
        clauses,
        status: ClosureStatus::Saturated,
        inferences,
    })
}

/// Indexed closure: same fixpoint as [`closure_linear`], but inference
/// partners come from a [`LiteralIndex`] over hash-consed clause twins
/// instead of an all-pairs scan. Retrieval is a superset of the exact
/// partners (recall is pinned by `tests/index_equivalence.rs`), and
/// [`resolve_ground_pair`] still applies the exact ground-atom check, so the
/// derived clause set is identical; only the pair-visit order may differ.
/// Partner positions are restricted to already-processed clauses via
/// `id_to_pos`, mirroring the linear `previous_index < index` scan exactly
/// (derivation order, and hence fresh id assignment, can still differ from
/// the linear run when the index over-approximates — the agreement check
/// compares statuses, and proof parents are tracked by id either way).
///
/// Both the ordered and the reference closure use this implementation: the
/// linear scan provably cannot close mid-size groundings on practical
/// budgets (see the cap-sizing experiment in
/// `docs/ORDERED_INFERENCE_CERTIFICATION.md`), so keeping one side linear
/// would cap coverage at the linear side. Correlated index misses would
/// surface as agreeing false saturations and are guarded empirically by the
/// EPU canary gate (any saturation on EPU fails loudly).
fn closure_indexed(
    input: &[Clause],
    ordering: &TermOrdering,
    ordered: bool,
    id_gen: &mut ClauseIdGen,
    deadline: Instant,
) -> Result<Closure, CertificationFailure> {
    let mut bank = TermBank::new();
    let mut index = LiteralIndex::new();
    let mut id_to_pos: StdHashMap<ClauseId, usize> = StdHashMap::new();
    let mut clauses = Vec::new();
    let mut seen = HashSet::default();
    for clause in input {
        // The input normalization pass is linear but unbounded in the input
        // size, so it honors the same deadline as the pair loop below.
        if Instant::now() >= deadline {
            trace_certify(format!(
                "refuse=closure_time ordered={ordered} clauses={} inferences=0",
                clauses.len()
            ));
            return Err(CertificationFailure::Limit(
                "certification time limit exceeded",
            ));
        }
        let Some(normalized) = normalize_clause(clause.clone()) else {
            continue;
        };
        let key = clause_key(&normalized);
        if seen.insert(key) {
            if normalized.is_empty() {
                clauses.push(normalized);
                return Ok(Closure {
                    clauses,
                    status: ClosureStatus::Refuted,
                    inferences: 0,
                });
            }
            id_to_pos.insert(normalized.id, clauses.len());
            let twin = bank.clause_from_legacy(&normalized);
            index.insert(twin, &bank);
            clauses.push(normalized);
        }
    }

    let mut inferences = 0;
    let mut pos = 0;
    while pos < clauses.len() {
        if Instant::now() >= deadline {
            trace_certify(format!(
                "refuse=closure_time ordered={ordered} clauses={} inferences={inferences}",
                clauses.len()
            ));
            return Err(CertificationFailure::Limit(
                "certification time limit exceeded",
            ));
        }
        let current = clauses[pos].clone();
        let current_twin = bank.clause_from_legacy(&current);
        let current_selection = if ordered {
            selected_literals(&current, ordering)
        } else {
            all_literal_indices(&current)
        };
        // Collect already-processed partner positions via the index. Sorting
        // ascending reproduces the linear scan order.
        let mut partner_pos = Vec::new();
        for &lit_idx in &current_selection {
            let query = &current_twin.literals[lit_idx];
            for hit in index.get_unifiable_resolution_partners(&query.atom, query.positive, &bank) {
                if let Some(&p) = id_to_pos.get(&hit.id)
                    && p < pos
                {
                    partner_pos.push(p);
                }
            }
        }
        partner_pos.sort_unstable();
        partner_pos.dedup();
        for previous_index in partner_pos {
            // Scope the borrow so it ends before any push below.
            let derived_batch = {
                let previous = &clauses[previous_index];
                let previous_selection = if ordered {
                    selected_literals(previous, ordering)
                } else {
                    all_literal_indices(previous)
                };
                resolve_ground_pair(
                    &current,
                    previous,
                    &current_selection,
                    &previous_selection,
                    id_gen,
                )
            };
            for derived in derived_batch {
                inferences += 1;
                if inferences > MAX_INFERENCES {
                    trace_certify(format!(
                        "refuse=inference_limit ordered={ordered} inferences={inferences}"
                    ));
                    return Err(CertificationFailure::Limit(
                        "ground inference limit exceeded",
                    ));
                }
                let Some(derived) = normalize_clause(derived) else {
                    continue;
                };
                if derived.is_empty() {
                    clauses.push(derived);
                    return Ok(Closure {
                        clauses,
                        status: ClosureStatus::Refuted,
                        inferences,
                    });
                }
                if seen.insert(clause_key(&derived)) {
                    id_to_pos.insert(derived.id, clauses.len());
                    let twin = bank.clause_from_legacy(&derived);
                    index.insert(twin, &bank);
                    clauses.push(derived);
                    if clauses.len() > MAX_CLAUSES {
                        trace_certify(format!(
                            "refuse=clause_limit ordered={ordered} clauses={} inferences={inferences}",
                            clauses.len()
                        ));
                        return Err(CertificationFailure::Limit("ground clause limit exceeded"));
                    }
                }
            }
        }
        pos += 1;
    }
    Ok(Closure {
        clauses,
        status: ClosureStatus::Saturated,
        inferences,
    })
}

fn all_literal_indices(clause: &Clause) -> Vec<usize> {
    (0..clause.literals.len()).collect()
}

fn selected_literals(clause: &Clause, ordering: &TermOrdering) -> Vec<usize> {
    let terms: Vec<_> = clause
        .literals
        .iter()
        .map(|literal| atom_term(&literal.atom))
        .collect();
    (0..terms.len())
        .filter(|&index| {
            !(0..terms.len()).any(|other| {
                other != index
                    && ordering.compare(&terms[other], &terms[index]) == TermComparison::Greater
            })
        })
        .collect()
}

fn resolve_ground_pair(
    left: &Clause,
    right: &Clause,
    left_selection: &[usize],
    right_selection: &[usize],
    id_gen: &mut ClauseIdGen,
) -> Vec<Clause> {
    let mut results = Vec::new();
    for &left_index in left_selection {
        for &right_index in right_selection {
            let left_literal = &left.literals[left_index];
            let right_literal = &right.literals[right_index];
            if left_literal.positive == right_literal.positive
                || left_literal.atom != right_literal.atom
            {
                continue;
            }
            let literals = left
                .literals
                .iter()
                .enumerate()
                .filter_map(|(index, literal)| (index != left_index).then_some(literal.clone()))
                .chain(
                    right
                        .literals
                        .iter()
                        .enumerate()
                        .filter_map(|(index, literal)| {
                            (index != right_index).then_some(literal.clone())
                        }),
                )
                .collect::<Vec<_>>();
            results.push(Clause::new(
                id_gen.next(),
                literals,
                ClauseSource::Inference {
                    rule: "resolution",
                    parents: vec![left.id, right.id].into(),
                },
            ));
        }
    }
    results
}

fn normalize_clause(mut clause: Clause) -> Option<Clause> {
    clause.deduplicate();
    if clause.is_tautology() {
        return None;
    }
    clause
        .literals
        .sort_by_key(|literal| format!("{literal:?}"));
    Some(clause)
}

fn clause_key(clause: &Clause) -> Vec<String> {
    clause
        .literals
        .iter()
        .map(|literal| format!("{literal:?}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::clause::ClauseIdGen;

    fn input_clause(id_gen: &mut ClauseIdGen, literals: Vec<mrs_core::clause::Literal>) -> Clause {
        Clause::new(
            id_gen.next(),
            literals,
            ClauseSource::Input {
                name: "test".into(),
                role: "axiom".into(),
            },
        )
    }

    #[test]
    fn certifies_ground_satisfiable_ordered_closure() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let q = symbols.intern("q");
        let a = symbols.intern("a");
        let mut ids = ClauseIdGen::new();
        let clauses = vec![
            input_clause(
                &mut ids,
                vec![mrs_core::clause::Literal::pos(Atom::pred(
                    p,
                    vec![Term::constant(a)],
                ))],
            ),
            input_clause(
                &mut ids,
                vec![mrs_core::clause::Literal::pos(Atom::pred(
                    q,
                    vec![Term::constant(a)],
                ))],
            ),
        ];
        let report = certify_ground_ordered_resolution(
            &clauses,
            &[],
            &symbols,
            &TermOrdering::KBO,
            &mut ids,
            Duration::from_secs(1),
        )
        .expect("finite ground SAT closure should certify");
        assert!(matches!(
            report.result,
            SearchResult::Saturated(witness)
                if witness.reason() == crate::SaturationReason::GroundOrderedResolution
        ));
    }

    #[test]
    fn certifies_ground_refutation_with_ordered_resolution() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let a = symbols.intern("a");
        let mut ids = ClauseIdGen::new();
        let clauses = vec![
            input_clause(
                &mut ids,
                vec![mrs_core::clause::Literal::pos(Atom::pred(
                    p,
                    vec![Term::constant(a)],
                ))],
            ),
            input_clause(
                &mut ids,
                vec![mrs_core::clause::Literal::neg(Atom::pred(
                    p,
                    vec![Term::constant(a)],
                ))],
            ),
        ];
        let report = certify_ground_ordered_resolution(
            &clauses,
            &[],
            &symbols,
            &TermOrdering::KBO,
            &mut ids,
            Duration::from_secs(1),
        )
        .expect("finite ground UNSAT closure should certify");
        assert!(matches!(report.result, SearchResult::Refutation(..)));
    }

    #[test]
    fn certifies_variable_epr_refutation_after_finite_grounding() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let a = symbols.intern("a");
        let mut ids = ClauseIdGen::new();
        let clauses = vec![
            input_clause(
                &mut ids,
                vec![mrs_core::clause::Literal::pos(Atom::pred(
                    p,
                    vec![Term::var(0)],
                ))],
            ),
            input_clause(
                &mut ids,
                vec![mrs_core::clause::Literal::neg(Atom::pred(
                    p,
                    vec![Term::constant(a)],
                ))],
            ),
        ];
        let report = certify_ground_ordered_resolution(
            &clauses,
            &[],
            &symbols,
            &TermOrdering::KBO,
            &mut ids,
            Duration::from_secs(1),
        )
        .expect("finite EPR grounding should certify the refutation");
        assert!(matches!(report.result, SearchResult::Refutation(..)));
    }

    #[test]
    fn accepts_variables_but_rejects_equality_and_functions() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let a = symbols.intern("a");
        let mut ids = ClauseIdGen::new();
        let variable_clause = input_clause(
            &mut ids,
            vec![mrs_core::clause::Literal::pos(Atom::pred(
                p,
                vec![Term::var(0)],
            ))],
        );
        assert!(
            certify_ground_ordered_resolution(
                &[variable_clause],
                &[],
                &symbols,
                &TermOrdering::KBO,
                &mut ids,
                Duration::from_secs(1),
            )
            .is_ok()
        );

        let equality_clause = input_clause(
            &mut ids,
            vec![mrs_core::clause::Literal::pos(Atom::Eq(
                Term::constant(a),
                Term::constant(a),
            ))],
        );
        assert!(matches!(
            certify_ground_ordered_resolution(
                &[equality_clause],
                &[],
                &symbols,
                &TermOrdering::KBO,
                &mut ids,
                Duration::from_secs(1),
            ),
            Err(CertificationFailure::Unsupported(
                "equality is outside the certified fragment"
            ))
        ));

        let f = symbols.intern("f");
        let function_clause = input_clause(
            &mut ids,
            vec![mrs_core::clause::Literal::pos(Atom::pred(
                p,
                vec![Term::app(f, vec![Term::constant(a)])],
            ))],
        );
        assert!(matches!(
            certify_ground_ordered_resolution(
                &[function_clause],
                &[],
                &symbols,
                &TermOrdering::KBO,
                &mut ids,
                Duration::from_secs(1),
            ),
            Err(CertificationFailure::Unsupported(
                "function terms are outside the certified EPR fragment"
            ))
        ));
    }

    #[test]
    fn certifies_ground_satisfiable_ordered_closure_lpo() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let q = symbols.intern("q");
        let a = symbols.intern("a");
        let mut ids = ClauseIdGen::new();
        let clauses = vec![
            input_clause(
                &mut ids,
                vec![mrs_core::clause::Literal::pos(Atom::pred(
                    p,
                    vec![Term::constant(a)],
                ))],
            ),
            input_clause(
                &mut ids,
                vec![mrs_core::clause::Literal::pos(Atom::pred(
                    q,
                    vec![Term::constant(a)],
                ))],
            ),
        ];
        let report = certify_ground_ordered_resolution(
            &clauses,
            &[],
            &symbols,
            &TermOrdering::LPO,
            &mut ids,
            Duration::from_secs(1),
        )
        .expect("finite ground SAT closure should certify under LPO");
        assert!(matches!(
            report.result,
            SearchResult::Saturated(witness)
                if witness.reason() == crate::SaturationReason::GroundOrderedResolution
        ));
    }

    #[test]
    fn certifies_ground_refutation_lpo() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let a = symbols.intern("a");
        let mut ids = ClauseIdGen::new();
        let clauses = vec![
            input_clause(
                &mut ids,
                vec![mrs_core::clause::Literal::pos(Atom::pred(
                    p,
                    vec![Term::constant(a)],
                ))],
            ),
            input_clause(
                &mut ids,
                vec![mrs_core::clause::Literal::neg(Atom::pred(
                    p,
                    vec![Term::constant(a)],
                ))],
            ),
        ];
        let report = certify_ground_ordered_resolution(
            &clauses,
            &[],
            &symbols,
            &TermOrdering::LPO,
            &mut ids,
            Duration::from_secs(1),
        )
        .expect("finite ground UNSAT closure should certify under LPO");
        assert!(matches!(report.result, SearchResult::Refutation(..)));
    }

    #[test]
    fn certifies_variable_epr_refutation_lpo_after_finite_grounding() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let a = symbols.intern("a");
        let mut ids = ClauseIdGen::new();
        let clauses = vec![
            input_clause(
                &mut ids,
                vec![mrs_core::clause::Literal::pos(Atom::pred(
                    p,
                    vec![Term::var(0)],
                ))],
            ),
            input_clause(
                &mut ids,
                vec![mrs_core::clause::Literal::neg(Atom::pred(
                    p,
                    vec![Term::constant(a)],
                ))],
            ),
        ];
        let report = certify_ground_ordered_resolution(
            &clauses,
            &[],
            &symbols,
            &TermOrdering::LPO,
            &mut ids,
            Duration::from_secs(1),
        )
        .expect("finite EPR grounding should certify the refutation under LPO");
        assert!(matches!(report.result, SearchResult::Refutation(..)));
    }

    #[test]
    fn lpo_ignores_weights_while_kbo_requires_them() {
        use mrs_calculus::ordering::SymbolConfig;
        use std::sync::Arc;

        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let q = symbols.intern("q");
        let a = symbols.intern("a");
        let mut ids = ClauseIdGen::new();
        let clauses = vec![
            input_clause(
                &mut ids,
                vec![mrs_core::clause::Literal::pos(Atom::pred(
                    p,
                    vec![Term::constant(a)],
                ))],
            ),
            input_clause(
                &mut ids,
                vec![mrs_core::clause::Literal::pos(Atom::pred(
                    q,
                    vec![Term::constant(a)],
                ))],
            ),
        ];
        // Distinct precedences, zero weights: valid for LPO, invalid for KBO.
        let config = Arc::new(SymbolConfig {
            precedence: vec![10, 20, 30],
            weights: vec![0, 0, 0],
            w0: 0,
        });
        let lpo = TermOrdering::CustomLPO(config.clone());
        certify_ground_ordered_resolution(
            &clauses,
            &[],
            &symbols,
            &lpo,
            &mut ids.clone(),
            Duration::from_secs(1),
        )
        .expect("LPO must not require positive weights");
        let kbo = TermOrdering::CustomKBO(config);
        assert!(matches!(
            certify_ground_ordered_resolution(
                &clauses,
                &[],
                &symbols,
                &kbo,
                &mut ids,
                Duration::from_secs(1),
            ),
            Err(CertificationFailure::Unsupported(_))
        ));
    }

    #[test]
    fn rejects_ac_ordering_and_non_kbo_lpo_orderings() {
        use mrs_calculus::ordering::SymbolConfig;
        use std::sync::Arc;

        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let a = symbols.intern("a");
        let mut ids = ClauseIdGen::new();
        let clauses = vec![input_clause(
            &mut ids,
            vec![mrs_core::clause::Literal::pos(Atom::pred(
                p,
                vec![Term::constant(a)],
            ))],
        )];
        let config = Arc::new(SymbolConfig::default());
        let ac_symbols: Arc<HashSet<mrs_core::SymbolId>> = Arc::new(HashSet::default());
        let ac = TermOrdering::CustomACKBO(config, ac_symbols);
        assert!(matches!(
            certify_ground_ordered_resolution(
                &clauses,
                &[],
                &symbols,
                &ac,
                &mut ids,
                Duration::from_secs(1),
            ),
            Err(CertificationFailure::Unsupported(
                "AC ordering is outside the certified fragment"
            ))
        ));
    }

    #[test]
    fn saturation_is_epr_only() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let a = symbols.intern("a");
        let f = symbols.intern("f");
        let mut ids = ClauseIdGen::new();
        let epr = input_clause(
            &mut ids,
            vec![mrs_core::clause::Literal::pos(Atom::pred(
                p,
                vec![Term::constant(a)],
            ))],
        );
        assert!(is_pure_relational_epr_input(std::slice::from_ref(&epr)));
        assert!(is_pure_relational_epr_input(&[epr.clone(), epr.clone()]));

        let with_var = Clause::new(
            ids.next(),
            vec![mrs_core::clause::Literal::pos(Atom::pred(
                p,
                vec![Term::var(0)],
            ))],
            ClauseSource::Input {
                name: "test".into(),
                role: "axiom".into(),
            },
        );
        assert!(is_pure_relational_epr_input(std::slice::from_ref(
            &with_var
        )));

        let equality = input_clause(
            &mut ids,
            vec![mrs_core::clause::Literal::pos(Atom::Eq(
                Term::constant(a),
                Term::constant(a),
            ))],
        );
        assert!(!is_pure_relational_epr_input(std::slice::from_ref(
            &equality
        )));

        let function = input_clause(
            &mut ids,
            vec![mrs_core::clause::Literal::pos(Atom::pred(
                p,
                vec![Term::app(f, vec![Term::constant(a)])],
            ))],
        );
        assert!(!is_pure_relational_epr_input(std::slice::from_ref(
            &function
        )));

        assert!(!is_pure_relational_epr_input(&[]));

        let formula = Clause::new_formula_step(
            ids.next(),
            mrs_core::Formula::True,
            ClauseSource::Input {
                name: "test".into(),
                role: "axiom".into(),
            },
        );
        assert!(!is_pure_relational_epr_input(std::slice::from_ref(
            &formula
        )));

        let avatar = Clause::new_avatar(
            ids.next(),
            vec![mrs_core::clause::Literal::pos(Atom::pred(
                p,
                vec![Term::constant(a)],
            ))],
            ClauseSource::Input {
                name: "test".into(),
                role: "axiom".into(),
            },
            vec![1],
        );
        assert!(!is_pure_relational_epr_input(std::slice::from_ref(&avatar)));

        // Non-EPR inputs must fail closed, never saturate.
        let equality_saturation = certify_ground_ordered_resolution(
            &[equality],
            &[],
            &symbols,
            &TermOrdering::KBO,
            &mut ids,
            Duration::from_secs(1),
        );
        assert!(
            !matches!(
                equality_saturation,
                Ok(ref report) if matches!(
                    report.result,
                    SearchResult::Saturated(_)
                )
            ),
            "equality input must never certify saturation"
        );
    }

    /// Regression canary for the historical false-`Satisfiable` shape
    /// (SYN861/862/866): an unsatisfiable clause set containing an
    /// all-positive clause must REFUTE under ordered resolution, never
    /// saturate — under both certified orderings.
    #[test]
    fn all_positive_epr_unsat_refutes_under_kbo_and_lpo() {
        for ordering in [TermOrdering::KBO, TermOrdering::LPO] {
            let mut symbols = SymbolTable::new();
            let p = symbols.intern("p");
            let q = symbols.intern("q");
            let a = symbols.intern("a");
            let mut ids = ClauseIdGen::new();
            // p(a) | q(a) , ~p(a) , ~q(a) — unsatisfiable.
            let clauses = vec![
                input_clause(
                    &mut ids,
                    vec![
                        mrs_core::clause::Literal::pos(Atom::pred(p, vec![Term::constant(a)])),
                        mrs_core::clause::Literal::pos(Atom::pred(q, vec![Term::constant(a)])),
                    ],
                ),
                input_clause(
                    &mut ids,
                    vec![mrs_core::clause::Literal::neg(Atom::pred(
                        p,
                        vec![Term::constant(a)],
                    ))],
                ),
                input_clause(
                    &mut ids,
                    vec![mrs_core::clause::Literal::neg(Atom::pred(
                        q,
                        vec![Term::constant(a)],
                    ))],
                ),
            ];
            let report = certify_ground_ordered_resolution(
                &clauses,
                &[],
                &symbols,
                &ordering,
                &mut ids,
                Duration::from_secs(1),
            )
            .expect("all-positive UNSAT EPR must certify");
            assert!(
                matches!(report.result, SearchResult::Refutation(..)),
                "all-positive UNSAT EPR must refute under {ordering:?}, got {:?}",
                report.result
            );
        }
    }

    /// Resource canary: a finite EPR grounding that exceeds the instance
    /// budget must fail closed with `Limit`, never with a saturation claim.
    #[test]
    fn grounding_blowup_fails_closed_with_limit() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let q = symbols.intern("q");
        let mut ids = ClauseIdGen::new();
        // Twelve distinct constants plus one 6-variable clause:
        // 12^6 = 2_985_984 instances exceeds TIER2_MAX_GROUND_INSTANCES.
        let mut clauses = Vec::new();
        for i in 0..12 {
            let c = symbols.intern(&format!("blowup_c{i}"));
            clauses.push(input_clause(
                &mut ids,
                vec![mrs_core::clause::Literal::pos(Atom::pred(
                    q,
                    vec![Term::constant(c)],
                ))],
            ));
        }
        clauses.push(input_clause(
            &mut ids,
            vec![mrs_core::clause::Literal::pos(Atom::pred(
                p,
                vec![
                    Term::var(0),
                    Term::var(1),
                    Term::var(2),
                    Term::var(3),
                    Term::var(4),
                    Term::var(5),
                ],
            ))],
        ));
        for ordering in [TermOrdering::KBO, TermOrdering::LPO] {
            let mut ids = ids.clone();
            let result = certify_ground_ordered_resolution(
                &clauses,
                &[],
                &symbols,
                &ordering,
                &mut ids,
                Duration::from_secs(1),
            );
            // Full grounding is refused (2.98M > Tier-2 cap); Tier-3 subset
            // tries find only all-positive saturations and exhaust. Either
            // Limit fails closed — never a saturation claim.
            assert!(
                matches!(result, Err(CertificationFailure::Limit(_))),
                "grounding blowup must fail closed with Limit under {ordering:?}"
            );
        }
    }

    /// Tier 3 finds a small unsatisfiable core: p(a) / ~p(a) hide among
    /// junk whose full grounding (10^8) is refused, but the {a} singleton
    /// grounds trivially and Tier 1 refutes with agreement.
    #[test]
    fn tier3_finds_small_unsatisfiable_core() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let q = symbols.intern("q");
        let big = symbols.intern("big");
        let a = symbols.intern("core_a");
        let mut ids = ClauseIdGen::new();
        let mut clauses = vec![ground_pos(&mut ids, p, a), ground_neg(&mut ids, p, a)];
        for i in 0..10 {
            let c = symbols.intern(&format!("junk_c{i}"));
            clauses.push(ground_pos(&mut ids, q, c));
        }
        // One 8-variable junk clause (separate predicate keeps arities
        // consistent) forces the full grounding refusal.
        clauses.push(input_clause(
            &mut ids,
            vec![mrs_core::clause::Literal::pos(Atom::pred(
                big,
                vec![
                    Term::var(0),
                    Term::var(1),
                    Term::var(2),
                    Term::var(3),
                    Term::var(4),
                    Term::var(5),
                    Term::var(6),
                    Term::var(7),
                ],
            ))],
        ));
        for ordering in [TermOrdering::KBO, TermOrdering::LPO] {
            let mut ids = ids.clone();
            let report = certify_ground_ordered_resolution(
                &clauses,
                &[],
                &symbols,
                &ordering,
                &mut ids,
                Duration::from_secs(5),
            )
            .expect("small core must certify");
            assert!(
                matches!(report.result, SearchResult::Refutation(..)),
                "Tier 3 must refute via the small core under {ordering:?}"
            );
        }
    }

    /// Tier 3 never certifies satisfiability: an all-positive problem whose
    /// full grounding is refused saturates every subset and exhausts.
    #[test]
    fn tier3_never_claims_saturation_from_subsets() {
        let mut symbols = SymbolTable::new();
        let r = symbols.intern("r");
        let mut ids = ClauseIdGen::new();
        // r(X1..X6) over 20 occurring constants: 64M instances, refused in
        // full. Every subset grounding is all-positive, hence saturating —
        // which must never become a saturation claim.
        let mut clauses = Vec::new();
        for i in 0..20 {
            let c = symbols.intern(&format!("sat_c{i}"));
            clauses.push(ground_pos(&mut ids, r, c));
        }
        clauses.push(input_clause(
            &mut ids,
            vec![mrs_core::clause::Literal::pos(Atom::pred(
                r,
                vec![
                    Term::var(0),
                    Term::var(1),
                    Term::var(2),
                    Term::var(3),
                    Term::var(4),
                    Term::var(5),
                ],
            ))],
        ));
        for ordering in [TermOrdering::KBO, TermOrdering::LPO] {
            let mut ids = ids.clone();
            let result = certify_ground_ordered_resolution(
                &clauses,
                &[],
                &symbols,
                &ordering,
                &mut ids,
                Duration::from_secs(5),
            );
            assert!(
                !matches!(
                    result,
                    Ok(ref report) if matches!(
                        report.result,
                        SearchResult::Saturated(_)
                    )
                ),
                "Tier 3 must never saturate from subsets under {ordering:?}"
            );
        }
    }

    /// Tier 3 respects a zero budget: it fails closed immediately instead of
    /// enumerating subsets without time.
    #[test]
    fn tier3_zero_budget_fails_closed_fast() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let q = symbols.intern("q");
        let big = symbols.intern("big");
        let mut ids = ClauseIdGen::new();
        let mut clauses = vec![
            ground_pos(&mut ids, p, symbols.intern("z_a")),
            ground_neg(&mut ids, p, symbols.intern("z_a")),
        ];
        for i in 0..10 {
            let c = symbols.intern(&format!("z_c{i}"));
            clauses.push(ground_pos(&mut ids, q, c));
        }
        clauses.push(input_clause(
            &mut ids,
            vec![mrs_core::clause::Literal::pos(Atom::pred(
                big,
                vec![
                    Term::var(0),
                    Term::var(1),
                    Term::var(2),
                    Term::var(3),
                    Term::var(4),
                    Term::var(5),
                    Term::var(6),
                    Term::var(7),
                ],
            ))],
        ));
        let start = Instant::now();
        let result = certify_ground_ordered_resolution(
            &clauses,
            &[],
            &symbols,
            &TermOrdering::KBO,
            &mut ids,
            Duration::ZERO,
        );
        assert!(result.is_err(), "zero budget must fail closed");
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "zero budget must not enumerate subsets"
        );
    }

    /// Goal bias: constants from distance-0 (negated-conjecture) clauses
    /// sort before frequent non-goal constants.
    #[test]
    fn bias_order_puts_goal_constants_first() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let goal_c = symbols.intern("goal_c");
        let freq_c = symbols.intern("freq_c");
        let rare_c = symbols.intern("rare_c");
        let mut ids = ClauseIdGen::new();
        let mut goal_clause = ground_pos(&mut ids, p, goal_c);
        goal_clause.distance = 0;
        let clauses = vec![
            goal_clause,
            ground_pos(&mut ids, p, freq_c),
            ground_pos(&mut ids, p, freq_c),
            ground_pos(&mut ids, p, freq_c),
            ground_pos(&mut ids, p, rare_c),
        ];
        let constants = vec![freq_c, goal_c, rare_c];
        let biased = bias_order_constants(&clauses, &constants);
        assert_eq!(biased.first(), Some(&goal_c));
        assert_eq!(biased.get(1), Some(&freq_c));
        assert_eq!(biased.get(2), Some(&rare_c));
    }

    /// Tier-2-UNSAT routes into Tier 3 and refutes: ~4100 ground units
    /// exceed the Tier-1 atom cap (landing in Tier-2 range with few
    /// instances), the solver proves UNSAT without a proof, and Tier 3
    /// finds the TSTP refutation — vocabulary restriction drops the 4100
    /// off-core units from the {a} subset, leaving just the core pair.
    #[test]
    fn tier2_unsat_falls_through_to_tier3_refutation() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let a = symbols.intern("core2_a");
        let mut ids = ClauseIdGen::new();
        let mut clauses = vec![ground_pos(&mut ids, p, a), ground_neg(&mut ids, p, a)];
        for i in 0..4100 {
            let c = symbols.intern(&format!("wide_c{i}"));
            clauses.push(ground_pos(&mut ids, p, c));
        }
        let report = certify_ground_ordered_resolution(
            &clauses,
            &[],
            &symbols,
            &TermOrdering::KBO,
            &mut ids,
            Duration::from_secs(30),
        )
        .expect("vocabulary-restricted core must certify after Tier-2 UNSAT");
        assert!(matches!(report.result, SearchResult::Refutation(..)));
    }

    fn ground_pos(id_gen: &mut ClauseIdGen, pred: SymbolId, constant: SymbolId) -> Clause {
        input_clause(
            id_gen,
            vec![mrs_core::clause::Literal::pos(Atom::pred(
                pred,
                vec![Term::constant(constant)],
            ))],
        )
    }

    fn ground_neg(id_gen: &mut ClauseIdGen, pred: SymbolId, constant: SymbolId) -> Clause {
        input_clause(
            id_gen,
            vec![mrs_core::clause::Literal::neg(Atom::pred(
                pred,
                vec![Term::constant(constant)],
            ))],
        )
    }

    fn closure_key_set(closure: &Closure) -> Vec<Vec<String>> {
        let mut keys: Vec<Vec<String>> = closure.clauses.iter().map(clause_key).collect();
        keys.sort();
        keys
    }

    /// The indexed closure must compute the same fixpoint as the linear
    /// all-pairs closure: same status always, and the same derived clause
    /// set (as content keys — fresh id assignment may differ when the index
    /// over-approximates retrieval) whenever a run saturates. Refuted runs
    /// return at the first empty clause, so their truncated clause sets may
    /// legitimately differ by pair-visit order; only the status must agree.
    /// This mirrors the production agreement check, which compares statuses.
    #[test]
    fn indexed_closure_matches_linear_closure() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let q = symbols.intern("q");
        let a = symbols.intern("a");
        let b = symbols.intern("b");

        let build_inputs = |ids: &mut ClauseIdGen| -> Vec<Vec<Clause>> {
            // 0: SAT pair. 1: direct UNSAT pair.
            // 2: all-positive UNSAT (needs two resolution steps).
            // 3: full binary tree over p(a), q(a) (UNSAT, multi-step).
            let sat = vec![ground_pos(ids, p, a), ground_pos(ids, q, a)];
            let unsat = vec![ground_pos(ids, p, a), ground_neg(ids, p, a)];
            let all_pos = vec![
                input_clause(
                    ids,
                    vec![
                        mrs_core::clause::Literal::pos(Atom::pred(p, vec![Term::constant(a)])),
                        mrs_core::clause::Literal::pos(Atom::pred(q, vec![Term::constant(a)])),
                    ],
                ),
                ground_neg(ids, p, a),
                ground_neg(ids, q, a),
            ];
            let tree = vec![
                input_clause(
                    ids,
                    vec![
                        mrs_core::clause::Literal::pos(Atom::pred(p, vec![Term::constant(a)])),
                        mrs_core::clause::Literal::pos(Atom::pred(q, vec![Term::constant(a)])),
                    ],
                ),
                input_clause(
                    ids,
                    vec![
                        mrs_core::clause::Literal::neg(Atom::pred(p, vec![Term::constant(a)])),
                        mrs_core::clause::Literal::pos(Atom::pred(q, vec![Term::constant(a)])),
                    ],
                ),
                input_clause(
                    ids,
                    vec![
                        mrs_core::clause::Literal::pos(Atom::pred(p, vec![Term::constant(b)])),
                        mrs_core::clause::Literal::neg(Atom::pred(q, vec![Term::constant(a)])),
                    ],
                ),
                input_clause(
                    ids,
                    vec![
                        mrs_core::clause::Literal::neg(Atom::pred(p, vec![Term::constant(b)])),
                        mrs_core::clause::Literal::neg(Atom::pred(q, vec![Term::constant(a)])),
                    ],
                ),
            ];
            vec![sat, unsat, all_pos, tree]
        };

        for ordering in [TermOrdering::KBO, TermOrdering::LPO] {
            for ordered in [true, false] {
                let mut ids = ClauseIdGen::new();
                for (case, input) in build_inputs(&mut ids).into_iter().enumerate() {
                    let mut linear_ids = ClauseIdGen::new();
                    let linear = closure_linear(
                        &input,
                        &ordering,
                        ordered,
                        &mut linear_ids,
                        Instant::now() + Duration::from_secs(5),
                    )
                    .expect("linear closure must terminate on tiny inputs");
                    let mut indexed_ids = ClauseIdGen::new();
                    let indexed = closure_indexed(
                        &input,
                        &ordering,
                        ordered,
                        &mut indexed_ids,
                        Instant::now() + Duration::from_secs(5),
                    )
                    .expect("indexed closure must terminate on tiny inputs");
                    assert_eq!(
                        linear.status, indexed.status,
                        "status mismatch case={case} ordered={ordered} ordering={ordering:?}"
                    );
                    if matches!(linear.status, ClosureStatus::Saturated) {
                        assert_eq!(
                            closure_key_set(&linear),
                            closure_key_set(&indexed),
                            "clause-set mismatch case={case} ordered={ordered} ordering={ordering:?}"
                        );
                    } else {
                        assert!(
                            closure_key_set(&indexed).contains(&Vec::new()),
                            "refuted indexed closure must contain the empty clause"
                        );
                    }
                }
            }
        }
    }
}
