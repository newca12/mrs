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
use std::ops::ControlFlow;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
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
/// (SAT-backed satisfiability via CaDiCaL, see `certified_sat`). `MAX_CLAUSES`
/// and `MAX_INFERENCES` bound Tier-1 closure memory and work after grounding,
/// and the per-run time limit bounds everything else. Anything beyond the
/// Tier-2 caps fails closed as `Limit`, never as saturation.
///
/// `TIER2_MAX_ATOMS` was raised from 16 384 to 400 000 in 2026-09, from
/// measurement rather than taste: with the routing fixed, the *atom* count
/// rather than the instance count became the binding limit on which
/// groundings reached the solver, and the atom set is not what costs memory
/// (the arena on a million-event UNSAT proof is). The instance cap is
/// unchanged at 2 000 000, which is what the exact `n^k` estimate refuses on;
/// raising it buys the 205-constant HWV problems at ~8 GB peak each.
const MAX_ATOMS: usize = 4096;
const MAX_CLAUSES: usize = 100_000;
/// Total literals in a Tier-1 closure, over the accumulated clause set.
///
/// `MAX_CLAUSES` counts clauses, which bounds nothing when the clauses are
/// wide: on HWV053-1 a wave derived 40 624 clauses averaging ~130 literals of
/// arity 86, and merging them into the index interned ~600M terms — 160 s of
/// the 165 s the run took against a 30 s budget, inside a loop with no
/// deadline check and no cap that could see it. A literal budget bounds the
/// actual work. Sized above the widest legitimate closure measured on the
/// 2026-09 casc-30 EPS corpus while keeping the term bank in the low hundreds
/// of MB.
const MAX_CLAUSE_LITERALS: usize = 8_000_000;
const MAX_INFERENCES: u64 = 1_000_000;
const MAX_GROUND_INSTANCES: usize = 500_000;
const TIER2_MAX_ATOMS: usize = 400_000;
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
    pub tier: CertifiedTier,
}

/// Which certification tier produced a report. Surfaced as `cert_tier=`
/// telemetry so benchmark harnesses can attribute coverage without TRACE.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CertifiedTier {
    /// Double ordered-resolution closure over a small grounding.
    One,
    /// SAT-backed satisfiability over a large grounding.
    Two,
    /// Constant-subset refutation for infeasible groundings.
    Three,
}

impl CertifiedTier {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            CertifiedTier::One => "1",
            CertifiedTier::Two => "2",
            CertifiedTier::Three => "3",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ClosureStatus {
    Saturated,
    Refuted,
}

#[derive(Debug)]
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
    workers: usize,
) -> Result<CertifiedGroundReport, CertificationFailure> {
    let mut proof_symbols = symbols.clone();
    let deadline = Instant::now() + time_limit;
    let constants = collect_grounding_constants(clauses, &mut proof_symbols)?;
    // Full grounding first; only size limits divert to Tier 3 below —
    // fragment errors propagate because subsets cannot fix them.
    let grounding_started = Instant::now();
    let grounded = match ground_with_constants(
        clauses,
        &constants,
        id_gen,
        TIER2_MAX_GROUND_INSTANCES,
        deadline,
    ) {
        Err(CertificationFailure::Limit(_)) => None,
        Err(other) => return Err(other),
        Ok(grounded) => Some(grounded),
    };
    if let Some(grounded) = grounded {
        trace_certify(format!(
            "phase=ground instances={} ms={}",
            grounded.clauses.len(),
            grounding_started.elapsed().as_millis()
        ));
        let expansion_started = Instant::now();
        // Equality expansion (unit-equality union-find normalization and
        // reflexivity fast paths). Predicate-only inputs pass through
        // byte-identical, so the legacy fragment observes no change.
        let (expanded_inputs, contradiction) =
            expand_for_certification(grounded, ordering, id_gen, deadline)?;
        trace_certify(format!(
            "phase=expand clauses={} ms={}",
            expanded_inputs.clauses.len(),
            expansion_started.elapsed().as_millis()
        ));
        if let Some(empty) = contradiction {
            trace_certify("eq_contradiction: unit disequality in its own class".to_string());
            return Ok(refute_from_ancestry(
                &empty,
                &expanded_inputs.originals,
                provenance,
                &proof_symbols,
                expanded_inputs.clauses.len() as u64,
            ));
        }
        // Expansion may delete every clause (all reflexivity-valid
        // tautologies). The empty set is vacuously saturated; the EPR
        // gate still applies so non-EPR inputs cannot take this path.
        if expanded_inputs.clauses.is_empty() {
            if !is_epr_with_equality_input(&expanded_inputs.originals) {
                return Err(CertificationFailure::Unsupported(
                    "saturation is certified for EPR inputs only",
                ));
            }
            trace_certify(
                "eq_expansion: all inputs reflexivity-valid, vacuous saturation".to_string(),
            );
            return Ok(CertifiedGroundReport {
                result: SearchResult::Saturated(CompletenessWitness::ground_ordered_resolution()),
                stats: SearchStats {
                    processed: 0,
                    generated: 0,
                    ..SearchStats::default()
                },
                tier: CertifiedTier::One,
            });
        }
        let atoms = collect_fragment_atoms(&expanded_inputs.clauses)?;
        // Tier router on the expanded set. Tier 2 (SAT-backed, no ordering
        // needed) decides the ground CNF as a propositional formula; Tier 1
        // (double ordered-resolution closure) is the only route that can
        // produce a TSTP-ancestry *refutation*, and its closure is exponential
        // in the grounding size.
        let tier1 =
            atoms.len() <= MAX_ATOMS && expanded_inputs.clauses.len() <= MAX_GROUND_INSTANCES;
        let has_equality = expanded_inputs
            .clauses
            .iter()
            .flat_map(|clause| clause.literals.iter())
            .any(|literal| matches!(literal.atom, Atom::Eq(..)));
        // `tier2_eligible` is the size/shape gate. The EPR fragment check is
        // applied where Tier 2 runs and only *skips* the tier — it must not
        // abort the router, because an input outside the SAT fragment is
        // exactly the input Tier 1 and Tier 3 can still handle.
        //
        // The EPR check is `is_epr_input`, not the stricter
        // `is_epr_with_equality_input`: a mixed predicate/equality input is
        // admissible here because every ground equality literal has already
        // been resolved (unit classes plus unique names), so what reaches
        // `encode_sat` is predicate-only. `!has_equality` above is the
        // encoding-level requirement, and it is checked on the *expanded* set.
        let tier2_eligible = !has_equality
            && atoms.len() <= TIER2_MAX_ATOMS
            && expanded_inputs.clauses.len() <= TIER2_MAX_GROUND_INSTANCES
            && is_epr_input(&expanded_inputs.originals);
        if tier1 || tier2_eligible {
            if tier2_eligible {
                // Try the SAT tier first. The encoding is linear in a
                // grounding that is already materialized, so the call is
                // cheap even on the small groundings Tier 1 can close, and it
                // is the only tier that answers the satisfiability questions
                // an exponential closure never reaches. It certifies
                // satisfiability alone: an UNSAT verdict from the solver
                // carries no TSTP ancestry, so it falls through to Tier 1 for
                // the refutation.
                trace_certify(format!(
                    "sat_tier=2 grounded={} atoms={}",
                    expanded_inputs.clauses.len(),
                    atoms.len()
                ));
                match crate::certified_sat::certify_sat_backed(
                    &expanded_inputs.clauses,
                    &expanded_inputs.originals,
                    provenance,
                    &atoms,
                    &expanded_inputs.class_representatives,
                    &proof_symbols,
                    &mut *id_gen,
                    deadline.saturating_duration_since(Instant::now()),
                ) {
                    Err(CertificationFailure::Tier2Unsat) => {
                        // Sound but proof-less: the refutation still needs a
                        // tier that can carry its ancestry.
                        trace_certify("sat_tier=2 unsat_without_proof".to_string());
                    }
                    outcome => return outcome,
                }
            }
            if tier1 {
                match run_tier1(
                    &expanded_inputs,
                    provenance,
                    ordering,
                    &proof_symbols,
                    id_gen,
                    deadline,
                    "tier1",
                    workers,
                ) {
                    Ok(report) => return Ok(report),
                    // A closure that ran out of time or hit a cap is not a
                    // verdict, so hand the run to the subset search rather than
                    // reporting a refusal it can still make progress on.
                    // `Unsupported` still propagates (the input is outside the
                    // ordered fragment) and so does `OrderedClosureMismatch`,
                    // which is a soundness alarm and must never be swallowed.
                    Err(CertificationFailure::Limit(reason)) => {
                        trace_certify(format!(
                            "tier1_incomplete instances={} reason={reason}",
                            expanded_inputs.clauses.len()
                        ));
                    }
                    Err(other) => return Err(other),
                }
            }
        } else {
            trace_certify(format!(
                "refuse=tier_size grounded={} atoms={}",
                expanded_inputs.clauses.len(),
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

/// Expand a grounded set for certification routing: unit-equality
/// normalization and reflexivity fast paths. Returns the
/// expanded inputs (originals preserved for proof ancestry) plus any
/// immediate contradiction. Predicate-only inputs pass through
/// byte-identical.
fn expand_for_certification(
    grounded: GroundedInputs,
    ordering: &TermOrdering,
    id_gen: &mut ClauseIdGen,
    deadline: Instant,
) -> Result<(GroundedInputs, Option<Clause>), CertificationFailure> {
    let expanded =
        crate::certified_eq::expand_equality(&grounded.clauses, ordering, id_gen, deadline)?;
    let contradiction = expanded.contradiction;
    Ok((
        GroundedInputs {
            clauses: expanded.clauses,
            originals: grounded.originals,
            class_representatives: expanded.class_representatives,
        },
        contradiction,
    ))
}

/// Assemble a refutation from an empty clause derived outside the
/// closures (equality-contradiction fast path): provenance plus the full
/// originals plus the empty clause, extracted and rendered as TSTP.
fn refute_from_ancestry(
    empty: &Clause,
    originals: &[Clause],
    provenance: &[Clause],
    proof_symbols: &SymbolTable,
    processed: u64,
) -> CertifiedGroundReport {
    let mut store: StdHashMap<ClauseId, Clause> = StdHashMap::new();
    for clause in provenance {
        store.insert(clause.id, clause.clone());
    }
    for clause in originals {
        store.insert(clause.id, clause.clone());
    }
    store.insert(empty.id, empty.clone());
    let proof = mrs_proof::extract::extract_proof(empty.id, &store);
    let tstp = mrs_proof::tstp::format_tstp(&proof, proof_symbols);
    CertifiedGroundReport {
        result: SearchResult::Refutation(empty.id, tstp),
        stats: SearchStats {
            processed,
            ..SearchStats::default()
        },
        tier: CertifiedTier::One,
    }
}

/// Tier 1: double ordered-resolution closure with agreement over an
/// already-routed grounding. Shared by the normal path and Tier-3 subset
/// tries (each subset carries full Tier-1 guarantees, including the
/// agreement check and TSTP-ancestry proofs).
#[allow(clippy::too_many_arguments)]
/// Wall-clock budget for recovering a finite model after a Tier-1 saturation.
/// The model is a bonus for the competition's model credit, never a
/// precondition of the verdict, so this is a small slice of the deadline
/// rather than the whole of it.
const MODEL_EXTRACTION_BUDGET: Duration = Duration::from_millis(500);

#[allow(clippy::too_many_arguments)]
fn run_tier1(
    grounded: &GroundedInputs,
    provenance: &[Clause],
    ordering: &TermOrdering,
    proof_symbols: &SymbolTable,
    id_gen: &mut ClauseIdGen,
    deadline: Instant,
    context: &'static str,
    workers: usize,
) -> Result<CertifiedGroundReport, CertificationFailure> {
    let atoms = collect_fragment_atoms(&grounded.clauses)?;
    validate_ordering_kind(ordering)?;
    if atoms.len() > MAX_ATOMS {
        trace_certify(format!("refuse=atom_limit atoms={}", atoms.len()));
        return Err(CertificationFailure::Limit("ground atom limit exceeded"));
    }
    validate_ground_order(ordering, &atoms, deadline)?;

    let mut ordered_id_gen = id_gen.clone();
    let ordered = closure_indexed(
        &grounded.clauses,
        ordering,
        true,
        &mut ordered_id_gen,
        deadline,
        workers,
    )?;

    let mut reference_id_gen = id_gen.clone();
    let reference = closure_indexed(
        &grounded.clauses,
        ordering,
        false,
        &mut reference_id_gen,
        deadline,
        workers,
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
                tier: CertifiedTier::One,
            })
        }
        ClosureStatus::Saturated => {
            // `Saturated` is enabled for the function-free relational EPR
            // or pure unit-equality fragments only. The fragment checks above already
            // reject function terms and formula/AVATAR clauses, but
            // re-check the pre-grounding inputs here so a future refactor
            // cannot accidentally certify saturation for a non-EPR problem.
            if !is_epr_with_equality_input(&grounded.originals) {
                return Err(CertificationFailure::Unsupported(
                    "saturation is certified for EPR inputs only",
                ));
            }
            // The closure agreement certifies satisfiability but yields no
            // model, and an unprinted model earns no credit. Recover one from
            // the same grounded set; a failure here cannot affect the verdict.
            let model = crate::certified_sat::extract_model(
                &grounded.clauses,
                &grounded.originals,
                &atoms,
                proof_symbols,
                Instant::now() + MODEL_EXTRACTION_BUDGET,
            );
            Ok(CertifiedGroundReport {
                result: SearchResult::Saturated(
                    CompletenessWitness::ground_ordered_resolution().with_model(model),
                ),
                stats,
                tier: CertifiedTier::One,
            })
        }
    }
}

/// Returns `true` iff every input clause is in the positive-status fragment:
/// function-free relational EPR, or a pure unit-equality problem. Mixed
/// predicate/equality clause sets are excluded until predicate congruence is
/// certified end to end. Formula-level and AVATAR clauses are excluded.
fn is_epr_with_equality_input(clauses: &[Clause]) -> bool {
    if clauses.is_empty()
        || clauses.iter().any(|clause| {
            clause.formula.is_some() || !clause.avatar.is_empty() || clause.literals.is_empty()
        })
    {
        return false;
    }
    let has_equality = clauses.iter().any(|clause| {
        clause
            .literals
            .iter()
            .any(|literal| matches!(literal.atom, Atom::Eq(..)))
    });
    if has_equality {
        clauses.iter().all(|clause| {
            clause.literals.len() == 1
                && matches!(clause.literals[0].atom, Atom::Eq(..))
                && clause.literals.iter().all(|literal| match &literal.atom {
                    Atom::Eq(left, right) => {
                        term_is_epr_constant_or_var(left) && term_is_epr_constant_or_var(right)
                    }
                    Atom::Pred(_, _) => false,
                })
        })
    } else {
        is_epr_input(clauses)
    }
}

/// Function-free relational EPR: every atom argument is a constant or a
/// variable, and no clause is formula-level, AVATAR-asserted, or empty.
///
/// This is the *input* side of the EPR check, deliberately weaker than
/// [`is_epr_with_equality_input`]: it admits a clause that mixes predicates
/// with equality. The mixed case is decidable because the ground equality
/// literals are resolved before the tiers run — unit classes by
/// `expand_equality`, and everything else by the unique-name axiom — so a
/// mixed input that reaches the SAT tier has already become predicate-only. The
/// stronger check exists for the *vacuous saturation* path, which runs on the
/// originals and therefore cannot rely on a resolution pass having happened.
fn is_epr_input(clauses: &[Clause]) -> bool {
    !clauses.is_empty()
        && !clauses.iter().any(|clause| {
            clause.formula.is_some() || !clause.avatar.is_empty() || clause.literals.is_empty()
        })
        && clauses.iter().all(|clause| {
            clause.literals.iter().all(|literal| match &literal.atom {
                Atom::Pred(_, args) => args.iter().all(term_is_epr_constant_or_var),
                Atom::Eq(left, right) => {
                    term_is_epr_constant_or_var(left) && term_is_epr_constant_or_var(right)
                }
            })
        })
}

fn term_is_epr_constant_or_var(term: &Term) -> bool {
    match term {
        Term::Var(_) => true,
        Term::App(_, args) => args.is_empty(),
    }
}

/// SInE-style tolerance ladder for Tier-3b clause subsets, strict first.
/// Mirrors the portfolio's proven tolerance points; each rung is estimated
/// and tried independently, so a rung that keeps everything is simply free.
const TIER3B_TOLERANCES: [f64; 4] = [1.0, 1.5, 2.0, 3.5];

/// Goal-relevance filter for Tier-3b clause subsets: SInE trigger logic
/// anchored on distance-0 (negated-conjecture) clauses instead of
/// role-conjecture inputs, which the clausifier turns into literalless
/// formula steps that plain SInE cannot start from. Empty clauses are
/// always kept (free proof material); with no distance-0 clauses the
/// filter is the identity (caller skips it). Output preserves input order
/// for determinism.
fn relevance_filter(clauses: &[Clause], tolerance: f64) -> Vec<Clause> {
    // Symbol sets per clause (literals only) plus frequencies. Crate
    // FxHash collections throughout (a std RandomState set would not
    // typecheck against the shared helpers).
    let mut item_syms: Vec<HashSet<SymbolId>> = Vec::with_capacity(clauses.len());
    let mut sym_counts: StdHashMap<SymbolId, usize> = StdHashMap::new();
    for clause in clauses {
        let mut syms = HashSet::default();
        for literal in &clause.literals {
            let Atom::Pred(predicate, args) = &literal.atom else {
                continue;
            };
            syms.insert(*predicate);
            for arg in args {
                collect_filter_symbols(arg, &mut syms);
            }
        }
        for &symbol in &syms {
            *sym_counts.entry(symbol).or_insert(0) += 1;
        }
        item_syms.push(syms);
    }
    // Trigger map exactly like SInE: a symbol triggers the items whose own
    // minimum generality it satisfies.
    let mut triggers: StdHashMap<SymbolId, Vec<usize>> = StdHashMap::new();
    for (index, syms) in item_syms.iter().enumerate() {
        if syms.is_empty() {
            continue;
        }
        let min_g = syms.iter().map(|s| sym_counts[s]).min().unwrap() as f64;
        let threshold = min_g * tolerance;
        for &symbol in syms {
            if let Some(count) = sym_counts.get(&symbol)
                && (*count as f64) <= threshold
            {
                triggers.entry(symbol).or_default().push(index);
            }
        }
    }
    // Anchor on goal-connected clauses (distance 0), always keeping empty
    // clauses alongside them.
    let mut active: HashSet<usize> = HashSet::default();
    let mut active_syms: HashSet<SymbolId> = HashSet::default();
    let mut frontier: HashSet<SymbolId> = HashSet::default();
    for (index, clause) in clauses.iter().enumerate() {
        if clause.literals.is_empty() || clause.distance == 0 {
            active.insert(index);
            for &symbol in &item_syms[index] {
                if active_syms.insert(symbol) {
                    frontier.insert(symbol);
                }
            }
        }
    }
    if active_syms.is_empty() {
        return clauses.to_vec();
    }
    while !frontier.is_empty() {
        let mut next = HashSet::default();
        for symbol in std::mem::take(&mut frontier) {
            if let Some(triggered) = triggers.get(&symbol) {
                for &index in triggered {
                    if active.insert(index) {
                        for &new_symbol in &item_syms[index] {
                            if active_syms.insert(new_symbol) {
                                next.insert(new_symbol);
                            }
                        }
                    }
                }
            }
        }
        frontier = next;
    }
    clauses
        .iter()
        .enumerate()
        .filter(|(index, _)| active.contains(index))
        .map(|(_, clause)| clause.clone())
        .collect()
}

fn collect_filter_symbols(term: &Term, syms: &mut HashSet<SymbolId>) {
    if let Term::App(symbol, args) = term {
        syms.insert(*symbol);
        for arg in args {
            collect_filter_symbols(arg, syms);
        }
    }
}

/// Outcome of one Tier-3 subset try: a certified refutation (returned
/// immediately), a saturation (proves nothing — try the next subset), or
/// a failure of the subset attempt itself (limits, mismatch — likewise
/// only rules out this subset).
enum Tier3Try {
    // Boxed: `CertifiedGroundReport` embeds a `SearchResult`, which carries the
    // proof text and the resource reason, so inlining it here made the enum
    // several times the size of its other variants. This path only runs when
    // tier 3 is being tried, so the allocation is not on a hot path.
    Refuted(Box<CertifiedGroundReport>),
    Saturated,
    Failed,
}

/// Expand one subset grounding and run Tier 1 on it, handling the
/// contradiction fast path. Shared by the Tier-3b clause-subset ladder
/// and the Tier-3a constant-subset loop so both carry identical
/// guarantees (agreement, TSTP ancestry, EPR+Eq fragment).
#[allow(clippy::too_many_arguments)]
fn tier3_try_grounded(
    subset_grounded: GroundedInputs,
    provenance: &[Clause],
    ordering: &TermOrdering,
    proof_symbols: &SymbolTable,
    id_gen: &mut ClauseIdGen,
    deadline: Instant,
    context: &'static str,
) -> Tier3Try {
    let (expanded_inputs, contradiction) =
        match expand_for_certification(subset_grounded, ordering, id_gen, deadline) {
            Ok(expanded) => expanded,
            Err(_) => return Tier3Try::Failed,
        };
    if let Some(empty) = contradiction {
        let report = refute_from_ancestry(
            &empty,
            &expanded_inputs.originals,
            provenance,
            proof_symbols,
            expanded_inputs.clauses.len() as u64,
        );
        return Tier3Try::Refuted(Box::new(report));
    }
    match run_tier1(
        &expanded_inputs,
        provenance,
        ordering,
        proof_symbols,
        id_gen,
        deadline,
        context,
        // Tier-3 subsets are bounded to a few thousand instances each and
        // saturate in milliseconds; fanning them out would cost more in thread
        // setup than the work, and would make the tier's failure accounting
        // order-dependent.
        1,
    ) {
        Ok(report) if matches!(report.result, SearchResult::Refutation(..)) => {
            Tier3Try::Refuted(Box::new(report))
        }
        Ok(_) => Tier3Try::Saturated,
        Err(_) => Tier3Try::Failed,
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
    // Tier-3b first: goal-relevant clause subsets over the FULL domain.
    // Dropping premises preserves unsatisfiability, so a refutation here
    // is a valid whole-problem proof; saturation proves nothing and is
    // skipped like everywhere in this tier. Strict tolerances first: each
    // rung is estimated against Tier-1 caps and skipped when oversized or
    // when the filter is a no-op.
    for tolerance in TIER3B_TOLERANCES {
        if Instant::now() >= deadline {
            trace_certify("tier3_exhausted tries=0 (deadline)".to_string());
            return Err(CertificationFailure::Limit(
                "certification time limit exceeded",
            ));
        }
        let filtered = relevance_filter(clauses, tolerance);
        if filtered.len() == clauses.len() {
            trace_certify(format!("tier3b_rung tolerance={tolerance} kept=all noop"));
            continue;
        }
        let mut estimated = 0usize;
        let mut fits = true;
        for clause in &filtered {
            let vars = clause.free_vars().len();
            match constants
                .len()
                .checked_pow(vars as u32)
                .and_then(|instances| estimated.checked_add(instances))
            {
                Some(total) if total <= MAX_GROUND_INSTANCES => {
                    estimated = total;
                }
                _ => {
                    fits = false;
                    break;
                }
            }
        }
        if !fits {
            trace_certify(format!(
                "tier3b_rung tolerance={tolerance} kept={} oversize",
                filtered.len()
            ));
            continue;
        }
        trace_certify(format!(
            "tier3b_rung tolerance={tolerance} kept={} estimated={estimated}",
            filtered.len()
        ));
        let filtered_grounded = match ground_with_constants(
            &filtered,
            constants,
            id_gen,
            MAX_GROUND_INSTANCES,
            deadline,
        ) {
            Ok(grounded) => grounded,
            Err(_) => continue,
        };
        match tier3_try_grounded(
            filtered_grounded,
            provenance,
            ordering,
            proof_symbols,
            id_gen,
            deadline,
            "tier3b-sub",
        ) {
            Tier3Try::Refuted(boxed) => {
                let mut report = *boxed;
                trace_certify(format!(
                    "tier3b_found tolerance={tolerance} kept={}",
                    filtered.len()
                ));
                report.tier = CertifiedTier::Three;
                return Ok(report);
            }
            Tier3Try::Saturated => {
                trace_certify(format!(
                    "tier3b_rung tolerance={tolerance} kept={} saturated",
                    filtered.len()
                ));
            }
            Tier3Try::Failed => {
                trace_certify(format!(
                    "tier3b_rung tolerance={tolerance} kept={} failed",
                    filtered.len()
                ));
            }
        }
    }
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
        let subset_grounded = match ground_with_constants(
            &restricted,
            subset,
            id_gen,
            TIER3_MAX_SUBSET_INSTANCES,
            deadline,
        ) {
            Ok(grounded) => grounded,
            Err(_) => continue,
        };
        // Subset saturation proves nothing about the full problem, and
        // subset failures (limits, mismatch) only rule out this subset.
        if let Tier3Try::Refuted(boxed) = tier3_try_grounded(
            subset_grounded,
            provenance,
            ordering,
            proof_symbols,
            id_gen,
            deadline,
            "tier3-sub",
        ) {
            trace_certify(format!(
                "tier3_found subset_size={} tries={tries}",
                subset.len()
            ));
            let mut report = *boxed;
            report.tier = CertifiedTier::Three;
            return Ok(report);
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
    clause.literals.iter().all(|literal| match &literal.atom {
        Atom::Pred(_, args) => args.iter().all(|arg| term_mentions_only(arg, subset)),
        Atom::Eq(left, right) => {
            term_mentions_only(left, subset) && term_mentions_only(right, subset)
        }
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
    /// Constant -> class representative, as computed by the equality
    /// expansion. Empty for a predicate-only input. Carried to the SAT tier
    /// so its model certificate interprets merged constants as one element.
    class_representatives: Vec<(Term, Term)>,
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
            match &literal.atom {
                Atom::Pred(_, args) => {
                    for arg in args {
                        collect_epr_constants(arg, &mut constants, &mut seen_constants)?;
                    }
                }
                // Equality sides contribute constants like predicate
                // arguments; function terms inside are still rejected by
                // `collect_epr_constants`, variables contribute nothing.
                Atom::Eq(left, right) => {
                    collect_epr_constants(left, &mut constants, &mut seen_constants)?;
                    collect_epr_constants(right, &mut constants, &mut seen_constants)?;
                }
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
/// per ground input clause plus the canonical instance count per variable
/// clause. The count is exact, and it must stay exact: see
/// [`instances_induce_same_clause`] for why an enumeration that dropped
/// "duplicate" instances would not be a smaller equivalent set.
fn ground_with_constants(
    clauses: &[Clause],
    constants: &[SymbolId],
    id_gen: &mut ClauseIdGen,
    instance_cap: usize,
    deadline: Instant,
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
        if Instant::now() >= deadline {
            return Err(CertificationFailure::Limit(
                "certification time limit exceeded",
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
            deadline,
        )?;
    }

    Ok(GroundedInputs {
        clauses: grounded,
        originals,
        class_representatives: Vec::new(),
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

/// Do two instances of `clause` induce the same ground clause?
///
/// The grounder must materialize every *distinct induced clause*, not one
/// representative per orbit of the variable-renaming group. Those are
/// different things, and conflating them is a soundness hole rather than a
/// missed optimization: `~g(Y) | p(X,e4,Y) | p(X,e3,Y) | p(X,e2,Y) |
/// p(X,e1,Y) | ~g(X)` over four constants has sixteen distinct induced
/// clauses, but renaming `X` and `Y` shows the clause is not symmetric in
/// them, and the "sorted values" representative of the orbit of `(X,Y) =
/// (0,1)` is a *different* clause. Dropping the other six leaves a ground set
/// with fewer constraints, so a model of it need not satisfy the original —
/// and `audit_casc_proofs` rejected exactly such a model
/// (`model violates axiom clause column_surjectivity`).
///
/// Restricted variable renaming is sound for the *refutation* direction,
/// because a resolution step that uses a renamed clause has a mirror that
/// uses the representative. It is unsound for satisfiability, which is the
/// whole of the EPS division. This predicate is the cheap end of that
/// assertion and `grounding_is_complete` is the test that pins it.
#[cfg(test)]
fn instances_induce_same_clause(
    left: &Substitution,
    right: &Substitution,
    clause: &Clause,
) -> bool {
    let mut left_literals: Vec<String> = clause
        .literals
        .iter()
        .map(|literal| format!("{:?}", left.apply_literal(literal)))
        .collect();
    let mut right_literals: Vec<String> = clause
        .literals
        .iter()
        .map(|literal| format!("{:?}", right.apply_literal(literal)))
        .collect();
    left_literals.sort();
    right_literals.sort();
    left_literals == right_literals
}

#[allow(clippy::too_many_arguments)]
fn instantiate_clause(
    clause: &Clause,
    vars: &[mrs_core::term::VarId],
    constants: &[SymbolId],
    depth: usize,
    substitution: &mut Substitution,
    id_gen: &mut ClauseIdGen,
    output: &mut Vec<Clause>,
    deadline: Instant,
) -> Result<(), CertificationFailure> {
    if depth == vars.len() {
        // Amortized deadline check: every 4096th materialized instance.
        // Without this, multi-million-instance groundings burn unbounded
        // wall time with no fail-closed trigger (found via gate straggler
        // forensics: workers living 80+s on a 10 s budget).
        if output.len() & 0xFFF == 0 && Instant::now() >= deadline {
            return Err(CertificationFailure::Limit(
                "certification time limit exceeded",
            ));
        }
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
        return Ok(());
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
            deadline,
        )?;
    }
    Ok(())
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
            match &literal.atom {
                Atom::Pred(predicate, args) => {
                    record_arity(&mut arities, *predicate, args.len())?;
                    for arg in args {
                        record_term_arities(arg, &mut arities)?;
                    }
                }
                Atom::Eq(left, right) => {
                    record_term_arities(left, &mut arities)?;
                    record_term_arities(right, &mut arities)?;
                }
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
    deadline: Instant,
) -> Result<(), CertificationFailure> {
    let config = ordering.symbol_config();
    validate_symbol_config(ordering, &config, atoms)?;
    let terms: Vec<Term> = atoms.iter().map(atom_term).collect();

    for (i, left) in terms.iter().enumerate() {
        if i & 0x1F == 0 && Instant::now() >= deadline {
            trace_certify(format!(
                "refuse=order_validation_time atoms={} row={i}",
                terms.len()
            ));
            return Err(CertificationFailure::Limit(
                "certification time limit exceeded",
            ));
        }
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

    // Transitivity, without the cubic scan.
    //
    // The obvious check — for every a, b, c, `a > b > c` implies `a > c` — is
    // O(n^3) comparisons, and each comparison here is a KBO or LPO comparison
    // of two whole ground atoms. On the high-arity half of the EPS division
    // (1400-odd atoms of arity 86) that is billions of comparisons: HWV053-1
    // spent over 200 s on a 10 s budget inside this function and never reached
    // the closure, with no deadline check to stop it.
    //
    // The reduction is not a relaxation. If the relation agrees with *some*
    // total order on the atoms, it **is** that linear order, and a linear order
    // is transitive. So: sort the atoms with the comparison to obtain candidate
    // ranks (the sort itself may be arbitrary — a non-transitive comparator has
    // no defined meaning for `sort_by` — which is why agreement is then
    // verified rather than assumed), and check every pair against those ranks.
    // Passing the check *proves* transitivity; a genuine total order always
    // passes, because sorting by it yields its own ranks.
    let mut ranked: Vec<usize> = (0..terms.len()).collect();
    ranked.sort_by(
        |&left, &right| match ordering.compare(&terms[left], &terms[right]) {
            TermComparison::Less => std::cmp::Ordering::Less,
            TermComparison::Greater => std::cmp::Ordering::Greater,
            // The strict-total-order pass above already rejected this; ordering a
            // tie is deterministic so the failure is reproducible, not arbitrary.
            TermComparison::Equal | TermComparison::Incomparable => std::cmp::Ordering::Equal,
        },
    );
    for (position, &index) in ranked.iter().enumerate() {
        if position & 0x3F == 0 && Instant::now() >= deadline {
            trace_certify(format!(
                "refuse=order_validation_time atoms={} position={position}",
                terms.len()
            ));
            return Err(CertificationFailure::Limit(
                "certification time limit exceeded",
            ));
        }
        for &other in &ranked[position + 1..] {
            if ordering.compare(&terms[index], &terms[other]) != TermComparison::Less {
                return Err(CertificationFailure::Unsupported(
                    "ordering is not transitive on ground atoms",
                ));
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
        match atom {
            Atom::Pred(predicate, args) => {
                symbols.insert(*predicate);
                for arg in args {
                    collect_term_symbols(arg, &mut symbols);
                }
            }
            // Equality sides contribute their constants; the reserved
            // ordering pseudo-symbol is deliberately never collected (it
            // lives only inside ordering temporaries, validated through
            // the config fallbacks).
            Atom::Eq(left, right) => {
                collect_term_symbols(left, &mut symbols);
                collect_term_symbols(right, &mut symbols);
            }
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
    crate::certified_eq::atom_term_eq(atom)
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
    let mut highest_input_id = None;
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
        highest_input_id =
            Some(highest_input_id.map_or(clause.id, |highest: ClauseId| highest.max(clause.id)));
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
    if let Some(highest) = highest_input_id
        && !id_gen.reserve_at_least(highest)
    {
        return Err(CertificationFailure::Limit("clause ID space exhausted"));
    }

    // Pin down the exact pair-generation semantics for the optimizations
    // below: pairs are (current, previous) with previous_index < index, the
    // current selection is fixed per outer iteration, and clauses derived
    // mid-iteration are appended but never revisited within the same outer
    // iteration. Borrowing `previous` instead of cloning it, hoisting the
    // current selection out of the inner loop, and checking the deadline
    // at bounded pair batches preserve this order exactly while removing one
    // full clause clone per pair.
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
            // Same per-pair deadline discipline as `closure_wave_range`: the
            // outer check bounds one position, not the `0..index` scan inside
            // it. The two closures are compared against each other, so they
            // have to give up at the same granularity.
            if Instant::now() >= deadline {
                trace_certify(format!(
                    "refuse=closure_time ordered={ordered} clauses={} inferences={inferences} pos={index}",
                    clauses.len()
                ));
                return Err(CertificationFailure::Limit(
                    "certification time limit exceeded",
                ));
            }
            // Scope the borrow so it ends before any push below.
            let derived_batch = {
                let mut derived_batch = Vec::new();
                let previous = &clauses[previous_index];
                let previous_selection = if ordered {
                    selected_literals(previous, ordering)
                } else {
                    all_literal_indices(previous)
                };
                let result = resolve_ground_pair_with(
                    &current,
                    previous,
                    &current_selection,
                    &previous_selection,
                    id_gen,
                    deadline,
                    |derived| {
                        derived_batch.push(derived);
                        ControlFlow::Continue(())
                    },
                );
                if result.is_break() && Instant::now() >= deadline {
                    return Err(CertificationFailure::Limit(
                        "certification time limit exceeded",
                    ));
                }
                derived_batch
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
/// [`resolve_ground_pair_with`] still applies the exact ground-atom check, so the
/// derived clause set is identical; only the pair-visit order may differ.
/// Partner positions are restricted to already-processed clauses via
/// `id_to_pos`, mirroring the linear `previous_index < index` scan exactly
/// (derivation order, and hence fresh id assignment, can still differ from
/// the linear run when the index over-approximates — the agreement check
/// compares statuses, and proof parents are tracked by id either way).
///
/// Equality literals bypass the [`LiteralIndex`] (whose resolution-partner
/// retrieval is predicate-only; the general engine handles equalities by
/// superposition instead) through a local exact-match map. All certified
/// inputs are ground and all `Eq` literals canonically oriented, so exact
/// matching on the legacy [`Atom`] is recall-complete here by construction.
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
    workers: usize,
) -> Result<Closure, CertificationFailure> {
    closure_indexed_with_budget(
        input,
        ordering,
        ordered,
        id_gen,
        deadline,
        workers,
        MAX_INFERENCES,
    )
}

/// [`closure_indexed`] with an explicit inference budget.
///
/// The budget is a parameter so the "a wave cannot exceed it" invariant can be
/// tested against a closure small enough to run in a unit test; production always
/// passes [`MAX_INFERENCES`].
fn closure_indexed_with_budget(
    input: &[Clause],
    ordering: &TermOrdering,
    ordered: bool,
    id_gen: &mut ClauseIdGen,
    deadline: Instant,
    workers: usize,
    inference_budget: u64,
) -> Result<Closure, CertificationFailure> {
    // Derived clause IDs must not collide with the input's. Two structures are
    // keyed by clause ID: `id_to_pos` here, and `LiteralIndex`'s own clause map
    // behind `get_unifiable_resolution_partners`. A collision makes a partner
    // lookup resolve to an unrelated clause instead of missing, so the closure
    // silently stops deriving partners while still reporting `Saturated`.
    //
    // The production caller happens to pass a generator that already advanced
    // past the input IDs; that is an invariant three frames away with nothing
    // enforcing it, so enforce it here instead. Comparing the ordered run
    // against the reference cannot catch this either: both production closures
    // are this function, and neither is checked against a fixpoint.
    let mut bank = TermBank::new();
    let mut index = LiteralIndex::new();
    let mut id_to_pos: StdHashMap<ClauseId, usize> = StdHashMap::new();
    // Exact-match partners for ground equality literals: (atom, polarity)
    // maps to the positions of clauses containing that literal. Keyed on
    // the legacy atom — the same value `resolve_ground_pair` compares —
    // so recall is exact for the ground canonically-oriented inputs here.
    let mut eq_index: StdHashMap<(Atom, bool), Vec<usize>> = StdHashMap::new();
    let mut clauses = Vec::new();
    let mut seen = HashSet::default();
    let mut highest_input_id = None;
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
        highest_input_id =
            Some(highest_input_id.map_or(clause.id, |highest: ClauseId| highest.max(clause.id)));
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
            for literal in &normalized.literals {
                if matches!(literal.atom, Atom::Eq(..)) {
                    eq_index
                        .entry((literal.atom.clone(), literal.positive))
                        .or_default()
                        .push(clauses.len());
                }
            }
            clauses.push(normalized);
        }
    }
    if let Some(highest) = highest_input_id
        && !id_gen.reserve_at_least(highest)
    {
        return Err(CertificationFailure::Limit("clause ID space exhausted"));
    }

    let mut inferences = 0u64;
    let mut closure_literals: usize = 0;
    for clause in &clauses {
        closure_literals = closure_literals.saturating_add(clause.literals.len());
    }
    // Wave-structured saturation. A position's partner set is fixed by the
    // clauses that existed *before* it (`partner_pos` keeps only `p < pos`),
    // and every clause derived while processing a position is appended, so it
    // lands at an index greater than any position still to be processed and can
    // never be a partner for one of them. A wave over the positions known at
    // its start therefore derives exactly what the sequential scan derives,
    // which is what makes the fan-out below a scheduling change rather than a
    // semantic one.
    let mut next_pos = 0usize;
    while next_pos < clauses.len() {
        if Instant::now() >= deadline {
            trace_certify(format!(
                "refuse=closure_time ordered={ordered} clauses={} inferences={inferences}",
                clauses.len()
            ));
            return Err(CertificationFailure::Limit(
                "certification time limit exceeded",
            ));
        }
        let wave_end = clauses.len();
        let wave = run_closure_wave(
            &clauses,
            &index,
            &eq_index,
            &id_to_pos,
            ordering,
            ordered,
            next_pos,
            wave_end,
            id_gen,
            deadline,
            workers,
            inference_budget.saturating_sub(inferences),
            MAX_CLAUSE_LITERALS.saturating_sub(closure_literals),
        )?;
        inferences = inferences.saturating_add(wave.inferences);
        if inferences > inference_budget {
            trace_certify(format!(
                "refuse=inference_limit ordered={ordered} inferences={inferences}"
            ));
            return Err(CertificationFailure::Limit(
                "ground inference limit exceeded",
            ));
        }
        if let Some(empty) = wave.refuted {
            // Same shape as the sequential scan: the empty clause goes into
            // `clauses` so the proof store still contains it.
            clauses.extend(wave.derived);
            clauses.push(empty);
            return Ok(Closure {
                clauses,
                status: ClosureStatus::Refuted,
                inferences,
            });
        }
        // Merge in chunk order so the resulting clause sequence is a
        // deterministic function of the input, independent of scheduling.
        //
        // Interning a derived clause is proportional to its literals times
        // their arity, so on wide clauses this loop is the most expensive step
        // in the tier by a wide margin. It carries the literal cap and its own
        // deadline check for that reason: a wave that finishes in seconds can
        // still take minutes to fold in.
        for (merged, derived) in wave.derived.iter().enumerate() {
            if merged & 0xFF == 0 && Instant::now() >= deadline {
                trace_certify(format!(
                    "refuse=closure_time ordered={ordered} clauses={} inferences={inferences}",
                    clauses.len()
                ));
                return Err(CertificationFailure::Limit(
                    "certification time limit exceeded",
                ));
            }
            let derived_literals = derived.literals.len();
            if seen.insert(clause_key(derived)) {
                closure_literals = closure_literals.saturating_add(derived_literals);
                if closure_literals > MAX_CLAUSE_LITERALS {
                    trace_certify(format!(
                        "refuse=literal_limit ordered={ordered} literals={closure_literals} \
                         clauses={} inferences={inferences}",
                        clauses.len()
                    ));
                    return Err(CertificationFailure::Limit("ground literal limit exceeded"));
                }
                id_to_pos.insert(derived.id, clauses.len());
                let twin = bank.clause_from_legacy(derived);
                index.insert(twin, &bank);
                for literal in &derived.literals {
                    if matches!(literal.atom, Atom::Eq(..)) {
                        eq_index
                            .entry((literal.atom.clone(), literal.positive))
                            .or_default()
                            .push(clauses.len());
                    }
                }
                clauses.push(derived.clone());
                if clauses.len() > MAX_CLAUSES {
                    trace_certify(format!(
                        "refuse=clause_limit ordered={ordered} clauses={} inferences={inferences}",
                        clauses.len()
                    ));
                    return Err(CertificationFailure::Limit("ground clause limit exceeded"));
                }
            }
        }
        next_pos = wave_end;
    }

    Ok(Closure {
        clauses,
        status: ClosureStatus::Saturated,
        inferences,
    })
}

/// What one pass over a range of clause positions produced: the derived
/// clauses (already normalized, not yet deduplicated) plus whether an empty
/// clause turned up. Deduplication is deliberately left to the caller so that
/// the index and the seen-set are only ever mutated on the thread that owns
/// them.
#[derive(Debug)]
struct ClosureWave {
    derived: Vec<Clause>,
    inferences: u64,
    /// The empty clause, when a worker derived one. It travels back to the
    /// owner of `clauses` because the refutation proof is rebuilt from it.
    refuted: Option<Clause>,
}

/// Run one wave over `range`, fanning contiguous chunks of positions across
/// `workers` scoped threads.
///
/// Every worker sees the same immutable snapshot: the clause array, the
/// literal index, the equality partner map and the id-to-position map are all
/// read-only here, and derived clauses are buffered rather than inserted, so no
/// worker can observe another's inferences. `id_gen` is shared through its
/// `Arc<AtomicU64>` counter, which is what keeps derived clause ids unique
/// across threads, and each worker interns into a term bank of its own.
#[allow(clippy::too_many_arguments)]
fn run_closure_wave(
    clauses: &[Clause],
    index: &LiteralIndex,
    eq_index: &StdHashMap<(Atom, bool), Vec<usize>>,
    id_to_pos: &StdHashMap<ClauseId, usize>,
    ordering: &TermOrdering,
    ordered: bool,
    range_start: usize,
    range_end: usize,
    id_gen: &mut ClauseIdGen,
    deadline: Instant,
    workers: usize,
    inference_budget: u64,
    literal_budget: usize,
) -> Result<ClosureWave, CertificationFailure> {
    let span = range_end.saturating_sub(range_start);
    let chunks = workers.clamp(1, span.max(1)).min(span.max(1));
    let remaining_inferences = AtomicU64::new(inference_budget);
    // The budget is in literals, not clauses or inferences. Each inference can
    // derive a clause, and folding a derived clause into the index costs its
    // literals times their arity, so on the high-arity half of this division
    // (86-ary atoms, 171-literal resolvents) a clause-count cap is off by
    // three orders of magnitude. Counting literals as they are produced makes
    // the cap enforceable while the memory is being allocated.
    let remaining_literals = AtomicUsize::new(literal_budget);
    if chunks <= 1 {
        return closure_wave_range(
            clauses,
            index,
            eq_index,
            id_to_pos,
            ordering,
            ordered,
            range_start,
            range_end,
            id_gen,
            deadline,
            &remaining_inferences,
            &remaining_literals,
        );
    }

    let chunk_size = span.div_ceil(chunks);
    let bounds: Vec<(usize, usize)> = (0..chunks)
        .map(|chunk| {
            let lo = range_start + chunk * chunk_size;
            let hi = (lo + chunk_size).min(range_end);
            (lo, hi)
        })
        .filter(|(lo, hi)| lo < hi)
        .collect();

    let mut results: Vec<Result<ClosureWave, CertificationFailure>> = Vec::new();
    std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(bounds.len());
        for (lo, hi) in bounds {
            let mut worker_id_gen = id_gen.clone();
            let worker_budget = &remaining_inferences;
            let worker_literals = &remaining_literals;
            handles.push(scope.spawn(move || {
                closure_wave_range(
                    clauses,
                    index,
                    eq_index,
                    id_to_pos,
                    ordering,
                    ordered,
                    lo,
                    hi,
                    &mut worker_id_gen,
                    deadline,
                    worker_budget,
                    worker_literals,
                )
            }));
        }
        for handle in handles {
            results.push(handle.join().unwrap_or_else(|_| {
                Err(CertificationFailure::Limit(
                    "certification worker thread panicked",
                ))
            }));
        }
    });

    let mut derived = Vec::new();
    let mut inferences = 0u64;
    for result in results {
        let wave = result?;
        inferences = inferences.saturating_add(wave.inferences);
        derived.extend(wave.derived);
        if let Some(empty) = wave.refuted {
            return Ok(ClosureWave {
                derived,
                inferences,
                refuted: Some(empty),
            });
        }
    }
    Ok(ClosureWave {
        derived,
        inferences,
        refuted: None,
    })
}

/// Process `range_start..range_end` sequentially, appending derived clauses to a
/// local buffer.
#[allow(clippy::too_many_arguments)]
fn closure_wave_range(
    clauses: &[Clause],
    index: &LiteralIndex,
    eq_index: &StdHashMap<(Atom, bool), Vec<usize>>,
    id_to_pos: &StdHashMap<ClauseId, usize>,
    ordering: &TermOrdering,
    ordered: bool,
    range_start: usize,
    range_end: usize,
    id_gen: &mut ClauseIdGen,
    deadline: Instant,
    remaining_inferences: &AtomicU64,
    remaining_literals: &AtomicUsize,
) -> Result<ClosureWave, CertificationFailure> {
    let mut derived: Vec<Clause> = Vec::new();
    let mut inferences = 0u64;
    // Each range owns a term bank. `clause_from_legacy` interns the terms of
    // the clauses it converts, and the index queries only *read* the bank to
    // flatten an id-term into cells, so a per-range bank yields exactly the
    // same cells as a shared one without needing interior mutability.
    let mut bank = TermBank::new();
    for pos in range_start..range_end {
        if Instant::now() >= deadline {
            trace_certify(format!(
                "refuse=closure_time ordered={ordered} clauses={} inferences={inferences}",
                clauses.len()
            ));
            return Err(CertificationFailure::Limit(
                "certification time limit exceeded",
            ));
        }
        let current = &clauses[pos];
        let current_twin = bank.clause_from_legacy(current);
        let current_selection = if ordered {
            selected_literals(current, ordering)
        } else {
            all_literal_indices(current)
        };
        let mut partner_pos = Vec::new();
        for &lit_idx in &current_selection {
            // One index query per selected literal, and a high-arity atom
            // makes each of those expensive. Without a check here a single
            // position can run far past the budget: on HWV053-1 a 30 s budget
            // overran to 165 s inside this loop before the partner-level check
            // could run at all.
            if Instant::now() >= deadline {
                trace_certify(format!(
                    "refuse=closure_time ordered={ordered} clauses={} inferences={inferences}",
                    clauses.len()
                ));
                return Err(CertificationFailure::Limit(
                    "certification time limit exceeded",
                ));
            }
            if matches!(current.literals[lit_idx].atom, Atom::Eq(..)) {
                let key = (
                    current.literals[lit_idx].atom.clone(),
                    !current.literals[lit_idx].positive,
                );
                if let Some(positions) = eq_index.get(&key) {
                    partner_pos.extend(positions.iter().filter(|&&p| p < pos));
                }
                continue;
            }
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
            // The position-level deadline check above cannot bound this loop:
            // `partner_pos` holds every earlier clause that shares a selected
            // literal, so on a wide grounding a single position can own
            // thousands of pairs, and the run overshoots its budget by however
            // long the remainder takes. That is not a cosmetic overshoot — a
            // competition wall clock is enforced from outside, so the prover
            // gets killed mid-loop and reports no SZS status at all. Checking
            // per pair bounds the overshoot to one `resolve_ground_pair_with`,
            // which is itself bounded by the two selections. The inference
            // budget is a work bound rather than a time bound, so it does not
            // stand in for this.
            if Instant::now() >= deadline {
                trace_certify(format!(
                    "refuse=closure_time ordered={ordered} clauses={} inferences={inferences} pos={pos}",
                    clauses.len()
                ));
                return Err(CertificationFailure::Limit(
                    "certification time limit exceeded",
                ));
            }
            let previous = &clauses[previous_index];
            let previous_selection = if ordered {
                selected_literals(previous, ordering)
            } else {
                all_literal_indices(previous)
            };
            let mut budget_exceeded = false;
            let mut refuted = None;
            let _ = resolve_ground_pair_with(
                current,
                previous,
                &current_selection,
                &previous_selection,
                id_gen,
                deadline,
                |candidate| {
                    inferences += 1;
                    if remaining_inferences
                        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |remaining| {
                            remaining.checked_sub(1)
                        })
                        .is_err()
                    {
                        // The resolver visits candidates incrementally, so an
                        // over-budget batch is never materialized.
                        trace_certify(format!(
                            "refuse=inference_limit ordered={ordered} chunk={range_start}..{range_end} \
                             inferences={inferences}"
                        ));
                        budget_exceeded = true;
                        return ControlFlow::Break(());
                    }
                    let Some(derived_clause) = normalize_clause(candidate) else {
                        return ControlFlow::Continue(());
                    };
                    if derived_clause.is_empty() {
                        refuted = Some(derived_clause);
                        return ControlFlow::Break(());
                    }
                    if remaining_literals
                        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |remaining| {
                            remaining.checked_sub(derived_clause.literals.len())
                        })
                        .is_err()
                    {
                        trace_certify(format!(
                            "refuse=literal_limit ordered={ordered} chunk={range_start}..{range_end} \
                             derived={} clauses={}",
                            derived.len(),
                            clauses.len()
                        ));
                        budget_exceeded = true;
                        return ControlFlow::Break(());
                    }
                    derived.push(derived_clause);
                    ControlFlow::Continue(())
                },
            );
            if budget_exceeded {
                return Err(CertificationFailure::Limit(
                    "ground inference limit exceeded",
                ));
            }
            if let Some(refuted) = refuted {
                return Ok(ClosureWave {
                    derived,
                    inferences,
                    refuted: Some(refuted),
                });
            }
        }
    }
    Ok(ClosureWave {
        derived,
        inferences,
        refuted: None,
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

fn resolve_ground_pair_with(
    left: &Clause,
    right: &Clause,
    left_selection: &[usize],
    right_selection: &[usize],
    id_gen: &mut ClauseIdGen,
    deadline: Instant,
    mut visit: impl FnMut(Clause) -> ControlFlow<()>,
) -> ControlFlow<()> {
    let mut pairs_examined = 0usize;
    for &left_index in left_selection {
        for &right_index in right_selection {
            pairs_examined += 1;
            // Amortize clock reads while bounding deadline overrun even when
            // the literal selections have no complementary atom in common.
            if pairs_examined.is_multiple_of(256) && Instant::now() >= deadline {
                return ControlFlow::Break(());
            }
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
            let clause = Clause::new(
                id_gen.next(),
                literals,
                ClauseSource::Inference {
                    rule: "resolution",
                    parents: vec![left.id, right.id].into(),
                },
            );
            if let ControlFlow::Break(()) = visit(clause) {
                return ControlFlow::Break(());
            }
        }
    }
    ControlFlow::Continue(())
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

    /// The grounder must materialize every *distinct induced clause*.
    ///
    /// This is the test that keeps restricted variable renaming out. The
    /// tempting optimization is to enumerate only one representative per orbit
    /// of the variable-renaming group, which turns `n^k` into `C(n + k - 1, k)`
    /// and makes the two-constant, hundred-variable half of the EPS division
    /// groundable at all. It is unsound: renaming a clause generally induces a
    /// *different* clause, so an orbit holds several distinct clauses and the
    /// representative is not one of the constraints the problem actually
    /// imposes. A model of the reduced set then need not satisfy the original,
    /// and the certifier would report `Satisfiable` with a model that the
    /// independent kernel check rejects.
    ///
    /// The clause below is the concrete case that exposed it: 4 constants, 2
    /// variables, 16 distinct induced clauses, and the first-occurrence
    /// restriction keeps only 10 of them.
    #[test]
    fn grounding_is_complete_over_distinct_induced_clauses() {
        let mut symbols = SymbolTable::new();
        let g = symbols.intern("group_element");
        let p = symbols.intern("product");
        let e = [
            symbols.intern("e_1"),
            symbols.intern("e_2"),
            symbols.intern("e_3"),
            symbols.intern("e_4"),
        ];
        let mut ids = ClauseIdGen::new();
        // ~group_element(Y) | product(X,e_4,Y) | ... | product(X,e_1,Y) | ~group_element(X)
        // (GRP123-4.004's `column_surjectivity`, the clause the audit named.)
        let mut literals = vec![mrs_core::clause::Literal::neg(Atom::pred(
            g,
            vec![Term::var(1)],
        ))];
        for constant in e {
            literals.push(mrs_core::clause::Literal::pos(Atom::pred(
                p,
                vec![Term::var(0), Term::constant(constant), Term::var(1)],
            )));
        }
        literals.push(mrs_core::clause::Literal::neg(Atom::pred(
            g,
            vec![Term::var(0)],
        )));
        let clause = input_clause(&mut ids, literals);

        let grounded = ground_with_constants(
            std::slice::from_ref(&clause),
            &e,
            &mut ids,
            10_000,
            Instant::now() + Duration::from_secs(5),
        )
        .expect("grounding");
        assert_eq!(
            grounded.clauses.len(),
            16,
            "every assignment induces a distinct clause here, so all 16 must be present"
        );

        // And no two generated clauses coincide, so nothing is duplicated and
        // nothing is a renaming that could stand in for a missing instance.
        let mut rendered: Vec<String> = grounded
            .clauses
            .iter()
            .map(|instance| {
                let mut literals: Vec<String> = instance
                    .literals
                    .iter()
                    .map(|literal| format!("{:?}", literal))
                    .collect();
                literals.sort();
                literals.join(" | ")
            })
            .collect();
        rendered.sort();
        let total = rendered.len();
        rendered.dedup();
        assert_eq!(rendered.len(), total, "no duplicate instances");
    }

    /// Two instances of the *same* clause that induce the same ground clause
    /// are interchangeable; two that differ are not. Pinned because the
    /// difference is the whole argument: a grounder that treats "renaming of"
    /// as "duplicate of" is unsound for satisfiability, and this is the
    /// predicate that says what the difference is.
    #[test]
    fn instance_equality_is_clause_equality_not_renaming() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let a = symbols.intern("a");
        let b = symbols.intern("b");
        let mut ids = ClauseIdGen::new();
        // p(X) | p(Y): symmetric in X and Y, so the two orders coincide.
        let symmetric = input_clause(
            &mut ids,
            vec![
                mrs_core::clause::Literal::pos(Atom::pred(p, vec![Term::var(0)])),
                mrs_core::clause::Literal::pos(Atom::pred(p, vec![Term::var(1)])),
            ],
        );
        // p(X,Y): swapping the arguments changes the clause.
        let asymmetric = input_clause(
            &mut ids,
            vec![mrs_core::clause::Literal::pos(Atom::pred(
                p,
                vec![Term::var(0), Term::var(1)],
            ))],
        );
        let mut forward = Substitution::new();
        forward.bind(mrs_core::term::VarId::from(0u32), Term::constant(a));
        forward.bind(mrs_core::term::VarId::from(1u32), Term::constant(b));
        let mut swapped = Substitution::new();
        swapped.bind(mrs_core::term::VarId::from(0u32), Term::constant(b));
        swapped.bind(mrs_core::term::VarId::from(1u32), Term::constant(a));
        assert!(instances_induce_same_clause(&forward, &swapped, &symmetric));
        assert!(!instances_induce_same_clause(
            &forward,
            &swapped,
            &asymmetric
        ));
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
            1,
        )
        .expect("finite ground SAT closure should certify");
        // The SAT tier decides a satisfiable grounding before the closure is
        // attempted, so the witness is SAT-backed rather than a closure
        // saturation. The *verdict* is what matters and is unchanged; the
        // route is documented here so a future reordering shows up as a test
        // failure rather than as a silent coverage change.
        assert!(matches!(
            report.result,
            SearchResult::Saturated(witness)
                if witness.reason() == crate::SaturationReason::SatBackedGrounding
        ));
        assert_eq!(report.tier, CertifiedTier::Two);
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
            1,
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
            1,
        )
        .expect("finite EPR grounding should certify the refutation");
        assert!(matches!(report.result, SearchResult::Refutation(..)));
    }

    #[test]
    fn accepts_variables_and_ground_equality_but_rejects_functions() {
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
                1,
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
        // Ground equality is now inside the certified fragment (congruence
        // expansion + union-find normalization); a reflexive unit normalizes
        // away and the remaining empty set saturates.
        assert!(
            certify_ground_ordered_resolution(
                &[equality_clause],
                &[],
                &symbols,
                &TermOrdering::KBO,
                &mut ids,
                Duration::from_secs(1),
                1,
            )
            .is_ok()
        );

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
                1,
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
            1,
        )
        .expect("finite ground SAT closure should certify under LPO");
        assert!(matches!(
            report.result,
            SearchResult::Saturated(witness)
                if witness.reason() == crate::SaturationReason::SatBackedGrounding
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
            1,
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
            1,
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
        // An *unsatisfiable* set, so the answer has to come from the ordered
        // closure. That is where ordering validation has to bind: a
        // satisfiable grounding is now decided by the SAT tier from a
        // model that is re-checked against the problem independently of any
        // ordering, so it is legitimately independent of the ordering. The
        // refutation direction has no such escape, which is exactly why this
        // test must use one.
        let unsat = vec![
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
        // Distinct precedences, zero weights: valid for LPO, invalid for KBO.
        let config = Arc::new(SymbolConfig {
            precedence: vec![10, 20, 30],
            weights: vec![0, 0, 0],
            w0: 0,
        });
        let lpo = TermOrdering::CustomLPO(config.clone());
        let report = certify_ground_ordered_resolution(
            &unsat,
            &[],
            &symbols,
            &lpo,
            &mut ids.clone(),
            Duration::from_secs(5),
            1,
        )
        .expect("LPO must not require positive weights");
        assert!(matches!(report.result, SearchResult::Refutation(..)));
        // The same input under a KBO with zero weights, which is not a valid
        // KBO. The invariant here is not "refuse" but "never certify a
        // *saturation*": the ordered closure has no business claiming a
        // complete set under an ordering it could not validate. It may still
        // answer, because the SAT tier re-checks its own model — or, for an
        // unsatisfiable input, its FRAT chain — against the problem
        // independently of any ordering, and that answer is verified either
        // way.
        let kbo = TermOrdering::CustomKBO(config);
        match certify_ground_ordered_resolution(
            &unsat,
            &[],
            &symbols,
            &kbo,
            &mut ids,
            Duration::from_secs(5),
            1,
        ) {
            Err(CertificationFailure::Unsupported(_)) => {}
            Ok(report) => assert!(
                !matches!(report.result, SearchResult::Saturated(_)),
                "an ordering the closure cannot validate must not certify a saturation"
            ),
            Err(other) => panic!("unexpected failure shape: {other:?}"),
        }
    }

    #[test]
    fn rejects_ac_ordering_and_non_kbo_lpo_orderings() {
        use mrs_calculus::ordering::SymbolConfig;
        use std::sync::Arc;

        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let a = symbols.intern("a");
        let mut ids = ClauseIdGen::new();
        // Unsatisfiable, so the answer has to come from a tier rather than
        // from the satisfiable shortcut. AC and a bare custom ordering are
        // outside the *ordered closure's* fragment, so the closure must refuse
        // them; a tier that verifies its own answer independently of any
        // ordering may still answer, but never with a saturation claim.
        let unsat = vec![
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
        let config = Arc::new(SymbolConfig::default());
        let ac_symbols: Arc<HashSet<mrs_core::SymbolId>> = Arc::new(HashSet::default());
        let ac = TermOrdering::CustomACKBO(config, ac_symbols);
        match certify_ground_ordered_resolution(
            &unsat,
            &[],
            &symbols,
            &ac,
            &mut ids,
            Duration::from_secs(5),
            1,
        ) {
            Err(CertificationFailure::Unsupported(_)) => {}
            Ok(report) => assert!(
                !matches!(report.result, SearchResult::Saturated(_)),
                "AC is outside the certified fragment and must not certify a saturation"
            ),
            Err(other) => panic!("unexpected failure shape: {other:?}"),
        }
    }

    #[test]
    fn saturation_is_epr_with_equality_only() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let a = symbols.intern("a");
        let b = symbols.intern("b");
        let f = symbols.intern("f");
        let mut ids = ClauseIdGen::new();
        let epr = input_clause(
            &mut ids,
            vec![mrs_core::clause::Literal::pos(Atom::pred(
                p,
                vec![Term::constant(a)],
            ))],
        );
        assert!(is_epr_with_equality_input(std::slice::from_ref(&epr)));

        // Ground and variable equalities are fragment members now.
        let ground_eq = input_clause(
            &mut ids,
            vec![mrs_core::clause::Literal::pos(Atom::Eq(
                Term::constant(a),
                Term::constant(b),
            ))],
        );
        assert!(is_epr_with_equality_input(std::slice::from_ref(&ground_eq)));
        let var_eq = input_clause(
            &mut ids,
            vec![mrs_core::clause::Literal::pos(Atom::Eq(
                Term::var(0),
                Term::constant(a),
            ))],
        );
        assert!(is_epr_with_equality_input(std::slice::from_ref(&var_eq)));

        // Function terms (inside predicates or equalities), formula
        // steps, and AVATAR clauses stay outside the fragment.
        let function = input_clause(
            &mut ids,
            vec![mrs_core::clause::Literal::pos(Atom::pred(
                p,
                vec![Term::app(f, vec![Term::constant(a)])],
            ))],
        );
        assert!(!is_epr_with_equality_input(std::slice::from_ref(&function)));
        let function_eq = input_clause(
            &mut ids,
            vec![mrs_core::clause::Literal::pos(Atom::Eq(
                Term::app(f, vec![Term::constant(a)]),
                Term::constant(b),
            ))],
        );
        assert!(!is_epr_with_equality_input(std::slice::from_ref(
            &function_eq
        )));
        assert!(!is_epr_with_equality_input(&[]));
        let formula = Clause::new_formula_step(
            ids.next(),
            mrs_core::Formula::True,
            ClauseSource::Input {
                name: "test".into(),
                role: "axiom".into(),
            },
        );
        assert!(!is_epr_with_equality_input(std::slice::from_ref(&formula)));
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
        assert!(!is_epr_with_equality_input(std::slice::from_ref(&avatar)));

        // A pure unit-equality problem certifies saturation (Tier 1).
        let report = certify_ground_ordered_resolution(
            std::slice::from_ref(&ground_eq),
            &[],
            &symbols,
            &TermOrdering::KBO,
            &mut ids,
            Duration::from_secs(1),
            1,
        )
        .expect("equality SAT must certify");
        assert!(matches!(report.result, SearchResult::Saturated(_)));

        // Mixed predicate/equality inputs are now decidable, and the reason is
        // that the ground equality literals never reach a tier unresolved:
        // `expand_equality` resolves the unit equation's classes and the
        // unique-name pass resolves everything else, so the SAT tier sees a
        // predicate-only set. What matters for soundness is that the emitted
        // model agrees with those decisions — it must interpret the merged
        // constants as one element, or it would assert `a != b` for a clause
        // set containing `a = b`. `certified_sat` covers that reading; here we
        // pin the routing decision that makes it reachable.
        let report = certify_ground_ordered_resolution(
            &[epr.clone(), ground_eq.clone()],
            &[],
            &symbols,
            &TermOrdering::KBO,
            &mut ids,
            Duration::from_secs(5),
            1,
        )
        .expect("a mixed predicate/unit-equality set is decidable");
        assert!(matches!(report.result, SearchResult::Saturated(_)));
        if let SearchResult::Saturated(witness) = &report.result {
            let model = witness
                .model()
                .expect("a model certificate is what makes the answer checkable");
            assert_eq!(
                model.constants["a"], model.constants["b"],
                "the certificate must interpret the merged constants as one element"
            );
        }

        // A function term is still outside the fragment on every path.
        let with_function = vec![
            epr.clone(),
            input_clause(
                &mut ids,
                vec![mrs_core::clause::Literal::pos(Atom::Eq(
                    Term::app(f, vec![Term::constant(a)]),
                    Term::constant(b),
                ))],
            ),
        ];
        assert!(matches!(
            certify_ground_ordered_resolution(
                &with_function,
                &[],
                &symbols,
                &TermOrdering::KBO,
                &mut ids,
                Duration::from_secs(1),
                1,
            ),
            Err(CertificationFailure::Unsupported(_))
        ));
    }

    /// A unit disequality contradicting its own class refutes immediately
    /// through the fast path, with TSTP ancestry (no closure needed).
    #[test]
    fn equality_contradiction_refutes_fast() {
        let mut symbols = SymbolTable::new();
        let a = symbols.intern("a");
        let b = symbols.intern("b");
        let mut ids = ClauseIdGen::new();
        let clauses = vec![
            input_clause(
                &mut ids,
                vec![mrs_core::clause::Literal::pos(Atom::Eq(
                    Term::constant(a),
                    Term::constant(b),
                ))],
            ),
            input_clause(
                &mut ids,
                vec![mrs_core::clause::Literal::neg(Atom::Eq(
                    Term::constant(a),
                    Term::constant(b),
                ))],
            ),
        ];
        for ordering in [TermOrdering::KBO, TermOrdering::LPO] {
            let mut ids = ids.clone();
            let report = certify_ground_ordered_resolution(
                &clauses,
                &[],
                &symbols,
                &ordering,
                &mut ids,
                Duration::from_secs(5),
                1,
            )
            .expect("unit contradiction must certify");
            assert!(
                matches!(report.result, SearchResult::Refutation(..)),
                "equality contradiction must refute under {ordering:?}"
            );
        }
    }

    /// Non-unit positive equalities are outside the certified equality
    /// fragment. Without predicate-congruence support, accepting this input
    /// would permit a false saturation, so certification must fail closed.
    /// A non-unit positive equality stays outside the certified fragment.
    ///
    /// This is the strict guard's regression test, and it is the boundary that
    /// a 2026-09 change moved and then restored. Resolving ground non-unit
    /// positive equalities by the unique-name axiom is equisatisfiability
    /// preserving and does decide the NLP division, but it also made
    /// `EPS/HWV042-1` reachable, and that problem's ground set then came out
    /// unsatisfiable against a `Satisfiable` reference answer. Whether the
    /// cause was the resolution or MRS's lowering was not established, so the
    /// input stays refused: a `GaveUp` that agrees with the reference is
    /// strictly better than a verdict that does not.
    #[test]
    fn non_unit_positive_equality_stays_outside_the_fragment() {
        let mut symbols = SymbolTable::new();
        let q = symbols.intern("q");
        let a = symbols.intern("a");
        let b = symbols.intern("b");
        let c = symbols.intern("c");
        let mut ids = ClauseIdGen::new();
        let eq = |positive: bool, l: SymbolId, r: SymbolId| {
            if positive {
                mrs_core::clause::Literal::pos(Atom::Eq(Term::constant(l), Term::constant(r)))
            } else {
                mrs_core::clause::Literal::neg(Atom::Eq(Term::constant(l), Term::constant(r)))
            }
        };
        let pred = |positive: bool, sym: SymbolId, constant: SymbolId| {
            if positive {
                mrs_core::clause::Literal::pos(Atom::pred(sym, vec![Term::constant(constant)]))
            } else {
                mrs_core::clause::Literal::neg(Atom::pred(sym, vec![Term::constant(constant)]))
            }
        };
        // `~q(a) | a = b` is ground, both sides are constants, and the
        // unique-name pass would decide the equality literal. The closure
        // cannot supply the congruence axioms the reasoning would need, so
        // the input is refused rather than decided.
        let clauses = vec![
            input_clause(&mut ids, vec![pred(false, q, a), eq(true, a, b)]),
            input_clause(&mut ids, vec![pred(false, q, b), eq(true, b, c)]),
            input_clause(&mut ids, vec![pred(true, q, a)]),
            input_clause(&mut ids, vec![pred(true, q, b)]),
            input_clause(&mut ids, vec![eq(false, a, c)]),
        ];
        for ordering in [TermOrdering::KBO, TermOrdering::LPO] {
            let mut ids = ids.clone();
            let result = certify_ground_ordered_resolution(
                &clauses,
                &[],
                &symbols,
                &ordering,
                &mut ids,
                Duration::from_secs(10),
                1,
            );
            assert!(
                matches!(result, Err(CertificationFailure::Unsupported(_))),
                "a non-unit positive equality must stay outside the fragment under {ordering:?}"
            );
        }
    }

    #[test]
    fn unit_equality_normalizes_predicate_arguments() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let a = symbols.intern("a");
        let b = symbols.intern("b");
        let mut ids = ClauseIdGen::new();
        let clauses = vec![
            input_clause(
                &mut ids,
                vec![mrs_core::clause::Literal::pos(Atom::Eq(
                    Term::constant(a),
                    Term::constant(b),
                ))],
            ),
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
                    vec![Term::constant(b)],
                ))],
            ),
        ];
        assert!(matches!(
                    certify_ground_ordered_resolution(
                        &clauses,
                        &[],
                        &symbols,
                        &TermOrdering::KBO,
                        &mut ids,
                        Duration::from_secs(1),
                    1,
                    )
        ,
                    Ok(report) if matches!(report.result, SearchResult::Refutation(..))
                ));
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
                1,
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
    /// The transitivity check was reduced from a cubic triple loop to rank
    /// agreement, so the property to pin is that the cheap version is not
    /// *weaker*. A strictly total comparison whose rank order it disagrees with
    /// must still be refused, and so must a comparison that is not a strict
    /// total order in the first place.
    ///
    /// Both refusals are load-bearing: a non-transitive ordering makes
    /// "maximal literal" ill-defined, and the ordered-closure agreement check
    /// would then be comparing two arbitrary saturation orders.
    #[test]
    fn ground_order_validation_refuses_non_total_and_non_transitive_orderings() {
        use mrs_calculus::ordering::SymbolConfig;
        use std::sync::Arc;

        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let a = symbols.intern("a");
        let b = symbols.intern("b");
        let mut ids = ClauseIdGen::new();
        let atoms = vec![
            Atom::pred(p, vec![Term::constant(a)]),
            Atom::pred(p, vec![Term::constant(b)]),
        ];
        let far = Instant::now() + Duration::from_secs(10);

        // A comparison that never separates the two atoms is not a strict total
        // order, and the first pass must say so.
        let tied = Arc::new(SymbolConfig {
            precedence: vec![10, 10, 10],
            weights: vec![0, 0, 0],
            w0: 0,
        });
        assert!(matches!(
            validate_ground_order(&TermOrdering::CustomLPO(tied), &atoms, far),
            Err(CertificationFailure::Unsupported(_))
        ));

        // A total order passes: the rank check must not reject a real one.
        let ordered = Arc::new(SymbolConfig {
            precedence: vec![10, 20, 30],
            weights: vec![0, 0, 0],
            w0: 0,
        });
        assert!(
            validate_ground_order(&TermOrdering::CustomLPO(ordered.clone()), &atoms, far).is_ok()
        );
        let kbo = Arc::new(SymbolConfig {
            precedence: vec![10, 20, 30],
            weights: vec![1, 1, 1],
            w0: 1,
        });
        assert!(validate_ground_order(&TermOrdering::CustomKBO(kbo), &atoms, far).is_ok());

        // An already-expired deadline must produce a `Limit`, not a silent
        // quadratic scan: this is the check that was missing.
        assert!(matches!(
            validate_ground_order(
                &TermOrdering::CustomLPO(ordered),
                &atoms,
                Instant::now() - Duration::from_secs(1)
            ),
            Err(CertificationFailure::Limit(_))
        ));
        let _ = (&mut ids, b);
    }

    /// A wide-arity grounding must fail closed *inside its budget*.
    ///
    /// Three separate unbounded steps used to sit between the budget and the
    /// result, all found on HWV053-1 (1408 clauses, 86-ary atoms) where a 30 s
    /// budget took 165 s and a 120 s one was killed at 240 s with an 81 GB
    /// footprint and no SZS status at all:
    ///
    /// 1. `validate_ground_order`'s transitivity check was a cubic triple loop
    ///    over the atoms with no deadline. Replaced by rank agreement, which
    ///    proves transitivity in O(n^2) and, being quadratic with a deadline
    ///    check, is bounded.
    /// 2. The wave's cap counted *clauses*, so a wave could hold 40k
    ///    171-literal resolvents before anything looked. Now counts literals.
    /// 3. The merge that folds a finished wave into the index — the most
    ///    expensive step in the tier, proportional to literals times arity —
    ///    had neither a cap nor a deadline check.
    ///
    /// The test asserts the property, not the arithmetic: a budget of one
    /// second must be reported as roughly one second, and the refusal must be a
    /// `Limit` rather than a panic or a silence.
    #[test]
    fn wide_arity_grounding_fails_closed_inside_its_budget() {
        let mut symbols = SymbolTable::new();
        let mut ids = ClauseIdGen::new();
        // 60 clauses over 40 clauses of the domain each, every atom of arity
        // 30. The grounding stays small (one constant per clause) while the
        // resolvents are wide enough that a clause-count cap cannot see them.
        let constants: Vec<_> = (0..40)
            .map(|i| symbols.intern(&format!("wide_c{i}")))
            .collect();
        let mut clauses = Vec::new();
        for (index, constant) in constants.iter().enumerate() {
            let predicate = symbols.intern(&format!("wide_p{index}"));
            let mut args: Vec<Term> = (0..30)
                .map(|j| Term::var(u32::try_from(j).unwrap()))
                .collect();
            args[29] = Term::constant(*constant);
            clauses.push(input_clause(
                &mut ids,
                vec![mrs_core::clause::Literal::pos(Atom::pred(predicate, args))],
            ));
        }
        let budget = Duration::from_millis(1200);
        let started = Instant::now();
        let result = certify_ground_ordered_resolution(
            &clauses,
            &[],
            &symbols,
            &TermOrdering::KBO,
            &mut ids,
            budget,
            1,
        );
        let elapsed = started.elapsed();
        assert!(
            matches!(result, Err(CertificationFailure::Limit(_))),
            "a wide-arity grounding must fail closed as a Limit, got {:?}",
            result.map(|report| report.tier)
        );
        // Generous, because the point is that the budget is not *ignored*:
        // a scheduling hiccup should not fail the test, a 100x overrun should.
        assert!(
            elapsed < budget * 10,
            "a {budget:?} budget took {elapsed:?}; a step is not honouring the deadline"
        );
    }

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
                1,
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
        // One wide junk clause (a separate predicate keeps arities
        // consistent) forces the full grounding refusal.
        //
        // The width has to exceed what restricted variable renaming can
        // absorb: renaming reduces `n^k` to `C(n + k - 1, k)`, which over the
        // 13 constants here is only ~126k instances at 8 variables, so an
        // 8-variable clause is groundable and would never reach Tier 3. 40
        // variables gives `C(52, 40)`, far past the cap, and no renaming can
        // bring it back.
        clauses.push(input_clause(
            &mut ids,
            vec![mrs_core::clause::Literal::pos(Atom::pred(
                big,
                (0..40).map(Term::var).collect(),
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
                1,
            )
            .expect("small core must certify");
            assert!(
                matches!(report.result, SearchResult::Refutation(..)),
                "Tier 3 must refute via the small core under {ordering:?}"
            );
            assert_eq!(report.tier, CertifiedTier::Three);
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
                1,
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
            1,
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
            1,
        )
        .expect("vocabulary-restricted core must certify after Tier-2 UNSAT");
        assert!(matches!(report.result, SearchResult::Refutation(..)));
    }

    fn goal_clause(id_gen: &mut ClauseIdGen, literals: Vec<mrs_core::clause::Literal>) -> Clause {
        let mut clause = input_clause(id_gen, literals);
        clause.distance = 0;
        clause
    }

    /// Direct filter checks: goal-anchored SInE-style selection keeps the
    /// relevant core and drops disconnected junk; empty clauses are always
    /// kept; without a goal the filter is the identity; loosening the
    /// tolerance only ever adds clauses.
    #[test]
    fn relevance_filter_selects_goal_connected_core() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let r = symbols.intern("r");
        let q = symbols.intern("q");
        let a = symbols.intern("a");
        let mut ids = ClauseIdGen::new();
        // Goal shares `p` with the core and `r` with nothing else.
        let goal = goal_clause(
            &mut ids,
            vec![
                mrs_core::clause::Literal::neg(Atom::pred(p, vec![Term::constant(a)])),
                mrs_core::clause::Literal::pos(Atom::pred(r, vec![Term::constant(a)])),
            ],
        );
        let core_pos = ground_pos(&mut ids, p, a);
        let core_neg = input_clause(
            &mut ids,
            vec![mrs_core::clause::Literal::neg(Atom::pred(
                r,
                vec![Term::constant(a)],
            ))],
        );
        let junk_unit = ground_pos(&mut ids, q, symbols.intern("junk_c"));
        let junk_var = input_clause(
            &mut ids,
            vec![mrs_core::clause::Literal::pos(Atom::pred(
                q,
                vec![Term::var(0), Term::var(1)],
            ))],
        );
        let clauses = vec![
            goal.clone(),
            core_pos.clone(),
            core_neg.clone(),
            junk_unit.clone(),
            junk_var.clone(),
        ];
        let strict = relevance_filter(&clauses, 1.0);
        let strict_ids: Vec<_> = strict.iter().map(|c| c.id).collect();
        // Goal + core kept; both junk shapes dropped.
        assert!(strict_ids.contains(&goal.id));
        assert!(strict_ids.contains(&core_pos.id));
        assert!(strict_ids.contains(&core_neg.id));
        assert!(!strict_ids.contains(&junk_unit.id));
        assert!(!strict_ids.contains(&junk_var.id));
        // Loosening never drops: strict output is a subset of loose output.
        for tolerance in [1.5, 2.0, 3.5] {
            let loose = relevance_filter(&clauses, tolerance);
            let loose_ids: Vec<_> = loose.iter().map(|c| c.id).collect();
            for id in &strict_ids {
                assert!(
                    loose_ids.contains(id),
                    "tolerance {tolerance} must keep strict subset"
                );
            }
        }
        // Empty clauses are always kept, even with no shared symbols.
        let mut empty = Clause::new(
            ids.next(),
            Vec::<mrs_core::clause::Literal>::new(),
            ClauseSource::Input {
                name: "empty".into(),
                role: "axiom".into(),
            },
        );
        empty.distance = 100;
        let mut with_empty = clauses.clone();
        with_empty.push(empty.clone());
        let filtered = relevance_filter(&with_empty, 1.0);
        assert!(filtered.iter().any(|c| c.id == empty.id));
        // No distance-0 clause: identity (caller skips the rung).
        let no_goal: Vec<Clause> = clauses
            .iter()
            .map(|c| {
                let mut rewritten = c.clone();
                rewritten.distance = 100;
                rewritten
            })
            .collect();
        let identity = relevance_filter(&no_goal, 1.0);
        assert_eq!(identity.len(), no_goal.len());
    }

    /// End-to-end Tier-3b: many constants plus a variable-heavy junk
    /// clause refuse the full grounding, but the relevance ladder keeps
    /// exactly the goal-connected core and Tier 1 refutes it.
    #[test]
    fn tier3b_finds_relevance_core() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let r = symbols.intern("r");
        let q = symbols.intern("q");
        let big = symbols.intern("big");
        let a = symbols.intern("core3_a");
        let mut ids = ClauseIdGen::new();
        let mut clauses = vec![
            goal_clause(
                &mut ids,
                vec![
                    mrs_core::clause::Literal::neg(Atom::pred(p, vec![Term::constant(a)])),
                    mrs_core::clause::Literal::pos(Atom::pred(r, vec![Term::constant(a)])),
                ],
            ),
            ground_pos(&mut ids, p, a),
            input_clause(
                &mut ids,
                vec![mrs_core::clause::Literal::neg(Atom::pred(
                    r,
                    vec![Term::constant(a)],
                ))],
            ),
        ];
        // Junk: 300 disconnected ground units plus an 8-variable clause
        // over all 301 constants (301^8 refuses the full grounding).
        for i in 0..300 {
            let c = symbols.intern(&format!("tier3b_c{i}"));
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
        for ordering in [TermOrdering::KBO, TermOrdering::LPO] {
            let mut ids = ids.clone();
            let report = certify_ground_ordered_resolution(
                &clauses,
                &[],
                &symbols,
                &ordering,
                &mut ids,
                Duration::from_secs(10),
                1,
            )
            .expect("relevance core must certify");
            assert!(
                matches!(report.result, SearchResult::Refutation(..)),
                "Tier-3b must refute via the relevance core under {ordering:?}"
            );
            assert_eq!(report.tier, CertifiedTier::Three);
        }
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
                        1,
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

    /// The wave fan-out must be a scheduling change and nothing else.
    ///
    /// A position's partner set only ever contains clauses that existed before
    /// it, so splitting the positions of a wave across threads and merging the
    /// buffered derivations afterwards has to reproduce the sequential scan
    /// exactly: same status, same clause set, same inference count. This runs
    /// the same fixtures through 1, 2, 3 and 8 workers and demands equality,
    /// including for the refuted cases where the empty clause has to survive
    /// the trip back from the worker that found it.
    #[test]
    fn parallel_wave_matches_sequential_wave() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let q = symbols.intern("q");
        let r = symbols.intern("r");
        let a = symbols.intern("a");
        let b = symbols.intern("b");
        let c = symbols.intern("c");

        let fixtures: Vec<(&str, Vec<Clause>)> = vec![
            (
                "satisfiable pair",
                vec![ground_pos(&mut ClauseIdGen::new(), p, a)],
            ),
            (
                "direct conflict",
                vec![
                    ground_pos(&mut ClauseIdGen::new(), p, a),
                    ground_neg(&mut ClauseIdGen::new(), p, a),
                ],
            ),
            (
                "multi-step refutation",
                vec![
                    ground_pos(&mut ClauseIdGen::new(), p, a),
                    ground_neg(&mut ClauseIdGen::new(), p, b),
                    ground_neg(&mut ClauseIdGen::new(), p, c),
                    input_clause(&mut ClauseIdGen::new(), vec![]),
                ],
            ),
            (
                "wide saturated set",
                vec![
                    ground_pos(&mut ClauseIdGen::new(), p, a),
                    ground_pos(&mut ClauseIdGen::new(), q, b),
                    ground_pos(&mut ClauseIdGen::new(), r, c),
                    ground_neg(&mut ClauseIdGen::new(), p, b),
                    ground_neg(&mut ClauseIdGen::new(), q, c),
                ],
            ),
        ];

        let orderings = [TermOrdering::KBO, TermOrdering::LPO];

        for ordering in &orderings {
            for ordered in [true, false] {
                for (label, template) in &fixtures {
                    // Rebuild the fixture per run so clause ids start clean.
                    let baseline_input = {
                        let mut ids = ClauseIdGen::new();
                        (*template)
                            .clone()
                            .into_iter()
                            .map(|mut clause| {
                                clause.id = ids.next();
                                clause
                            })
                            .collect::<Vec<Clause>>()
                    };
                    let mut baseline_ids = ClauseIdGen::new();
                    let baseline = closure_indexed(
                        &baseline_input,
                        ordering,
                        ordered,
                        &mut baseline_ids,
                        Instant::now() + Duration::from_secs(30),
                        1,
                    )
                    .expect("baseline closure terminates");

                    for workers in [2usize, 3, 8] {
                        let mut ids = ClauseIdGen::new();
                        let input = (*template)
                            .clone()
                            .into_iter()
                            .map(|mut clause| {
                                clause.id = ids.next();
                                clause
                            })
                            .collect::<Vec<Clause>>();
                        let mut parallel_ids = ClauseIdGen::new();
                        let parallel = closure_indexed(
                            &input,
                            ordering,
                            ordered,
                            &mut parallel_ids,
                            Instant::now() + Duration::from_secs(30),
                            workers,
                        )
                        .expect("parallel closure terminates");

                        assert_eq!(
                            baseline.status, parallel.status,
                            "status diverged at workers={workers} case={label} \
                             ordered={ordered} ordering={ordering:?}"
                        );
                        assert_eq!(
                            baseline.inferences, parallel.inferences,
                            "inference count diverged at workers={workers} case={label} \
                             ordered={ordered} ordering={ordering:?}"
                        );
                        assert_eq!(
                            closure_key_set(&baseline),
                            closure_key_set(&parallel),
                            "clause set diverged at workers={workers} case={label} \
                             ordered={ordered} ordering={ordering:?}"
                        );
                    }
                }
            }
        }
    }

    /// A wave must not be able to spend more inferences than the sequential
    /// scan would have allowed, including when the remaining allowance is less
    /// than the number of worker chunks.
    ///
    /// The derived clauses of a wave are buffered until it merges, so the
    /// remaining budget is partitioned between chunks. This fixture is 40 pairs
    /// wide and offers 1 600 inferences. Both a tiny residual budget (smaller
    /// than the worker count) and a mid-wave budget must be refused at every
    /// worker count.
    #[test]
    fn parallel_wave_cannot_exceed_the_inference_budget() {
        const WIDTH: usize = 40;

        let mut symbols = SymbolTable::new();
        let c = symbols.intern("c");
        let p = symbols.intern("p");
        let pos = |pred: SymbolId| {
            mrs_core::clause::Literal::pos(Atom::pred(pred, vec![Term::constant(c)]))
        };
        let neg = |pred: SymbolId| {
            mrs_core::clause::Literal::neg(Atom::pred(pred, vec![Term::constant(c)]))
        };
        let mut ids = ClauseIdGen::new();
        let mut input = Vec::new();
        for i in 0..WIDTH {
            let a = symbols.intern(&format!("a{i}"));
            let b = symbols.intern(&format!("b{i}"));
            input.push(input_clause(&mut ids, vec![pos(p), pos(a)]));
            input.push(input_clause(&mut ids, vec![neg(p), pos(b)]));
        }

        // Roomy budget: the closure saturates, so the refusals below are about
        // the ceiling and nothing else.
        let mut roomy_ids = ClauseIdGen::new();
        let roomy = closure_indexed_with_budget(
            &input,
            &TermOrdering::KBO,
            false,
            &mut roomy_ids,
            Instant::now() + Duration::from_secs(120),
            1,
            10_000_000,
        )
        .expect("a generous budget must saturate the fixture");
        assert_eq!(roomy.status, ClosureStatus::Saturated);
        assert!(
            roomy.inferences > 500,
            "fixture must exceed the small budget, got {}",
            roomy.inferences
        );

        for workers in [1usize, 2, 4, 8] {
            for budget in [1, 500] {
                let result = closure_indexed_with_budget(
                    &input,
                    &TermOrdering::KBO,
                    false,
                    &mut ClauseIdGen::new(),
                    Instant::now() + Duration::from_secs(120),
                    workers,
                    budget,
                );
                assert!(
                    matches!(
                        result,
                        Err(CertificationFailure::Limit(
                            "ground inference limit exceeded"
                        ))
                    ),
                    "workers={workers} must refuse once the {budget}-inference budget is spent"
                );
            }
        }
    }

    /// Derived clause IDs must not collide with the input's, whoever allocated
    /// them.
    ///
    /// Two structures here are keyed by clause ID: `id_to_pos`, and
    /// `LiteralIndex`'s own clause map behind
    /// `get_unifiable_resolution_partners`. A derived clause that reuses an
    /// input clause's ID overwrites both, so a later partner lookup resolves to
    /// an unrelated clause instead of missing, and the closure silently stops
    /// deriving while still reporting `Saturated`.
    ///
    /// The production caller avoids this by accident — its generator has
    /// already advanced past the input IDs — which is an invariant three frames
    /// away with nothing documenting or checking it. Comparing the ordered run
    /// against the reference is no help: both production closures are
    /// `closure_indexed`, and neither is checked against a fixpoint.
    ///
    /// The fixture is chained on purpose. A single-wave fixture cannot expose
    /// this: the wave buffers every derivation and merges it only after the
    /// wave ends, so wave 0 — which is all of the input — runs against pristine
    /// maps and does the whole job. The refutation here is only reachable in
    /// wave 1, by a *derived* clause resolving against an input clause whose ID
    /// a wave-0 derivation has already claimed. Without the fix that partner
    /// lookup lands on the wrong clause, the empty clause is never derived, and
    /// the run reports `Saturated` on an unsatisfiable input.
    #[test]
    fn closure_result_does_not_depend_on_the_callers_id_generator() {
        let mut symbols = SymbolTable::new();
        let c = symbols.intern("c");
        let a = symbols.intern("a");
        let b = symbols.intern("b");
        let pos = |pred: SymbolId| {
            mrs_core::clause::Literal::pos(Atom::pred(pred, vec![Term::constant(c)]))
        };
        let neg = |pred: SymbolId| {
            mrs_core::clause::Literal::neg(Atom::pred(pred, vec![Term::constant(c)]))
        };
        let mut ids = ClauseIdGen::new();
        // Wave 0 derives `a(c)` (id 0, colliding with `~a(c)`) and `~b(c)`
        // (id 1, colliding with `b(c)`). Wave 1 then needs `a(c)` to resolve
        // against `~a(c)` to reach the empty clause.
        let input = vec![
            input_clause(&mut ids, vec![neg(a)]),
            input_clause(&mut ids, vec![pos(b)]),
            input_clause(&mut ids, vec![neg(b), pos(a)]),
        ];

        // A generator that has not seen the input IDs: derived IDs start at 0
        // and collide with every input clause.
        let mut fresh = ClauseIdGen::new();
        // A generator that has already advanced well past them.
        let mut advanced = ClauseIdGen::new();
        for _ in 0..10_000 {
            advanced.next();
        }

        let deadline = || Instant::now() + Duration::from_secs(120);
        // Single worker and an inference ceiling the fixture cannot reach, so
        // the caller's id generator is the only thing that varies.
        let with_fresh = closure_indexed_with_budget(
            &input,
            &TermOrdering::KBO,
            false,
            &mut fresh,
            deadline(),
            1,
            10_000_000,
        )
        .expect("fresh-generator closure terminates");
        let with_advanced = closure_indexed_with_budget(
            &input,
            &TermOrdering::KBO,
            false,
            &mut advanced,
            deadline(),
            1,
            10_000_000,
        )
        .expect("advanced-generator closure terminates");
        // The reference scans positions rather than ids, so it is unaffected by
        // the collision and states the true fixpoint.
        let reference = closure_linear(
            &input,
            &TermOrdering::KBO,
            false,
            &mut ClauseIdGen::new(),
            deadline(),
        )
        .expect("reference closure terminates");

        assert_eq!(
            with_fresh.status, reference.status,
            "indexed closure disagreed with the reference on status"
        );
        assert_eq!(
            with_fresh.inferences, reference.inferences,
            "indexed closure disagreed with the reference on inference count"
        );
        assert_eq!(
            closure_key_set(&with_fresh),
            closure_key_set(&reference),
            "indexed closure disagreed with the reference on clause set"
        );

        assert_eq!(
            with_fresh.status, with_advanced.status,
            "status depended on the caller's id generator"
        );
        assert_eq!(
            with_fresh.inferences, with_advanced.inferences,
            "inference count depended on the caller's id generator"
        );
        assert_eq!(
            closure_key_set(&with_fresh),
            closure_key_set(&with_advanced),
            "clause set depended on the caller's id generator"
        );

        // And the real check: the refutation was actually reached. A collision
        // drops the `a(c)` x `~a(c)` pair, so the closure saturates instead.
        assert_eq!(
            with_fresh.status,
            ClosureStatus::Refuted,
            "closure under-derived: a colliding id generator turned a refutation into a saturation"
        );
        assert!(
            closure_key_set(&with_fresh).contains(&Vec::new()),
            "a refuted closure must contain the empty clause"
        );
    }

    /// The deadline has to be enforced inside a position, not only between
    /// them.
    ///
    /// A position's partner set is every earlier clause sharing one of its
    /// selected literals, so a single position can own thousands of resolution
    /// pairs. With the deadline only sampled at position boundaries, such a
    /// position runs to completion however far past the budget it is: the
    /// closure then reports `Saturated` for a 1 ms budget after real work, and
    /// under a competition wall clock the process is killed mid-loop and prints
    /// no SZS status at all. The inference budget does not stand in for this —
    /// it bounds work, not elapsed time.
    ///
    /// Scope: this pins that the deadline is honoured at all. It does not, and
    /// cannot cheaply, isolate the *per-pair* check from the per-position one —
    /// with a per-position check still in place a tight budget is always caught
    /// eventually, so both variants pass. The per-pair granularity is justified
    /// by measurement rather than by a timing test: on `casc-30/EPS` the
    /// position-granular version overran a 100 s budget by 64 % (164 s wall),
    /// and a competition wall clock is enforced from outside, so the prover is
    /// killed mid-loop and reports no SZS status at all.
    ///
    /// The fixture puts `k` clauses of the form `p(c) | a_i(c)` opposite `k`
    /// clauses of the form `~p(c) | b_j(c)`, all over one constant. Every
    /// positive clause therefore has all `k` negative clauses as partners, for
    /// `k^2` pairs, and each resolvent `a_i(c) | b_j(c)` is non-empty and
    /// carries no negative literal, so the set saturates without a conflict.
    /// That gives a closure whose work is seconds-to-milliseconds of real
    /// resolution while still ending in `Saturated` — which is what makes the
    /// deadline the only difference between the two calls below.
    #[test]
    fn closure_honours_the_deadline_inside_one_position() {
        const WIDTH: usize = 40;

        let mut symbols = SymbolTable::new();
        let c = symbols.intern("c");
        let p = symbols.intern("p");
        let pos = |pred: SymbolId| {
            mrs_core::clause::Literal::pos(Atom::pred(pred, vec![Term::constant(c)]))
        };
        let neg = |pred: SymbolId| {
            mrs_core::clause::Literal::neg(Atom::pred(pred, vec![Term::constant(c)]))
        };
        let mut ids = ClauseIdGen::new();
        let mut input = Vec::new();
        for i in 0..WIDTH {
            let a = symbols.intern(&format!("a{i}"));
            let b = symbols.intern(&format!("b{i}"));
            input.push(input_clause(&mut ids, vec![pos(p), pos(a)]));
            input.push(input_clause(&mut ids, vec![neg(p), pos(b)]));
        }
        assert_eq!(
            input.len(),
            WIDTH * 2,
            "fixture must present every clause to the closure"
        );

        // Roomy budget: the closure completes and saturates. The inference
        // ceiling sits far above the fixture's 1 600 pairs so the refusals
        // below are about the deadline and nothing else.
        let mut roomy_ids = ClauseIdGen::new();
        let roomy = closure_indexed_with_budget(
            &input,
            &TermOrdering::KBO,
            false,
            &mut roomy_ids,
            Instant::now() + Duration::from_secs(120),
            1,
            10_000_000,
        )
        .expect("the fixture must saturate given room");
        assert_eq!(
            roomy.status,
            ClosureStatus::Saturated,
            "fixture must be conflict-free so the deadline is the only difference"
        );
        assert!(
            roomy.inferences > (WIDTH * WIDTH / 2) as u64,
            "fixture must actually do quadratic resolution work, got {} inferences",
            roomy.inferences
        );

        // One millisecond: the same closure must bail out rather than finish.
        for (label, result) in [
            (
                "indexed",
                closure_indexed_with_budget(
                    &input,
                    &TermOrdering::KBO,
                    false,
                    &mut ClauseIdGen::new(),
                    Instant::now() + Duration::from_millis(1),
                    1,
                    10_000_000,
                ),
            ),
            (
                "linear",
                closure_linear(
                    &input,
                    &TermOrdering::KBO,
                    false,
                    &mut ClauseIdGen::new(),
                    Instant::now() + Duration::from_millis(1),
                ),
            ),
        ] {
            assert!(
                matches!(
                    result,
                    Err(CertificationFailure::Limit(
                        "certification time limit exceeded"
                    ))
                ),
                "{label} closure ignored a 1 ms deadline and reported {result:?}"
            );
        }
    }
}
