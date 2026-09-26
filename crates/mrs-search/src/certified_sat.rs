//! SAT-backed Tier-2 certification for large EPR groundings.
//!
//! Tier 1 (double ordered-resolution closure in `certified.rs`) cannot close
//! mid-size groundings on practical budgets: closures grow past 100k clauses
//! and hit inference/time caps. This tier instead encodes the grounded
//! clause set propositionally and asks CaDiCaL to decide it:
//!
//! - `Sat` plus an independently re-verified model yields `Saturated`. The
//!   model *is* the certificate; soundness never depends on trusting the
//!   solver, only on the fragment checks (shared with Tier 1) and the
//!   [`verify_model`] re-check below.
//! - `Unsat`, `Unknown`, trace/solver errors, and failed model re-checks all
//!   fail closed as `GaveUp`. This tier deliberately certifies
//!   satisfiability only: there is no FRAT-to-TSTP elaborator for UNSAT
//!   proofs, so unsatisfiable large groundings stay `GaveUp` (Tier 1 still
//!   refutes small ones with full TSTP proofs).
//!
//! Fragment and size gating happen in the Tier router
//! (`certified::certify_ground_ordered_resolution`); ordering validation is
//! skipped here by design — model checking needs no ordering.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::{Duration, Instant};

use mrs_cadical::{
    ProofEvent, ProofTrace, SolveResult, Solver, TraceConfig, check_proof_trace, encode_frat_ascii,
};
use mrs_core::clause::{
    AvatarSatTrace, Clause, ClauseCertificate, ClauseId, ClauseIdGen, ClauseSource,
    avatar_sat_trace_digest,
};
use mrs_core::formula::Atom;
use mrs_core::model::{EqualitySemantics, ModelCertificate, PredicateTable};
use mrs_core::symbol::SymbolTable;

use crate::certified::{CertificationFailure, CertifiedGroundReport, trace_certify};
use crate::{CompletenessWitness, SearchResult, SearchStats};

/// Cap on CaDiCaL proof-trace events per Tier-2 run. The tracer stops
/// appending past the cap (memory stays bounded) and disconnect reports
/// overflow; both fail closed. Sized from measurement: the small PHP
/// fixture already emits ~1.2M events (mostly originals + deletions).
const SAT_TRACE_MAX_EVENTS: usize = 16_000_000;

/// Emission caps for Tier-2 UNSAT proofs: the certificate embeds every
/// manifest clause plus the FRAT bytes, so emission (and kernel
/// verification) is bounded separately from capture. Anything bigger
/// stays `Tier2Unsat` → Tier-3 → usually exhausted → `GaveUp`. Sized
/// generously against the small-PHP measurement (615k manifest entries);
/// the kernel enforces matching Tier-2 limits on its side.
const SAT_EMIT_MAX_MANIFEST: usize = 2_500_000;
const SAT_EMIT_MAX_TRACE_BYTES: usize = 256 * 1024 * 1024;
/// Avoid allocating a dense finite-model table whose size is not bounded by
/// the SAT atom/ground-clause caps (for example, many predicates of arity 8).
const MODEL_TABLE_MAX_ENTRIES: usize = mrs_core::model::MAX_MODEL_TABLE_ENTRIES;

/// One propositionally encoded ground clause plus its source clause id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EncodedSatClause {
    pub source: ClauseId,
    pub lits: Vec<i32>,
}

/// Outcome of capturing and independently re-checking a solver UNSAT proof.
pub(crate) struct UnsatTraceReport {
    /// Total recorded events.
    pub events: usize,
    /// Original-clause events (should match the encoded input count).
    pub originals: usize,
    /// Derived-clause events.
    pub derived: usize,
    /// Result of the independent RUP-chain re-check (`Err` carries the
    /// checker's display string: RAT witnesses and malformed steps land
    /// here and fail closed downstream).
    pub check: Result<(), String>,
    /// The captured event stream, moved along for emission.
    pub trace: ProofTrace,
}

/// Disconnect a traced solver after UNSAT and run the independent RUP
/// re-check. Capture problems (disconnect failure, event-limit overflow)
/// return `Err` and fail closed; a failed re-check is RECORDED, not
/// returned as error, because the caller maps every UNSAT outcome to
/// `Tier2Unsat` identically (capture-first measurement must not change
/// verdicts — only TRACE gains lines).
pub(crate) fn capture_and_check(
    solver: &mut Solver,
) -> Result<UnsatTraceReport, CertificationFailure> {
    let trace = solver.disconnect_trace().map_err(|e| {
        if matches!(e, mrs_cadical::TraceError::EventLimitExceeded) {
            CertificationFailure::Limit("sat trace event limit exceeded")
        } else {
            CertificationFailure::Limit("sat trace capture failed")
        }
    })?;
    let mut originals = 0usize;
    let mut derived = 0usize;
    for event in &trace.events {
        match event {
            ProofEvent::OriginalClause { .. } => originals += 1,
            ProofEvent::DerivedClause { .. } => derived += 1,
            _ => {}
        }
    }
    let check = check_proof_trace(&trace).map_err(|e| format!("{e}"));
    let events = trace.events.len();
    Ok(UnsatTraceReport {
        events,
        originals,
        derived,
        check,
        trace,
    })
}

/// Encode a grounded clause set as signed-integer SAT clauses under a
/// deterministic atom ordering. Tautologies are skipped (valid in any
/// model); an empty clause means the set is unsatisfiable, which this tier
/// cannot certify — that fails closed via the SAT-only rule.
/// Canonical sort key for a predicate ground atom: predicate name plus
/// argument constant names. Equality atoms are deliberately rejected here;
/// Tier 2 remains predicate-only until equality encoding and kernel replay are
/// supported end to end. Name strings (not interning indices or `Debug` output)
/// keep the var numbering stable across runs and checkable by the kernel,
/// which re-derives the identical ordering from TSTP text alone. Ground
/// atoms only (Tier-2 inputs are fully grounded); anything else fails the
/// encoding loudly instead of silently misordering.
pub(crate) fn atom_sort_key(atom: &Atom, symbols: &SymbolTable) -> Option<(String, Vec<String>)> {
    match atom {
        Atom::Pred(predicate, args) => {
            let mut arg_names = Vec::with_capacity(args.len());
            for arg in args {
                let mrs_core::term::Term::App(symbol, inner) = arg else {
                    return None;
                };
                if !inner.is_empty() {
                    return None;
                }
                arg_names.push(symbols.resolve(*symbol).to_string());
            }
            Some((symbols.resolve(*predicate).to_string(), arg_names))
        }
        Atom::Eq(_, _) => None,
    }
}

pub(crate) fn encode_sat(
    grounded: &[Clause],
    atoms: &[Atom],
    symbols: &SymbolTable,
    deadline: Instant,
) -> Result<(Vec<EncodedSatClause>, Vec<Atom>), CertificationFailure> {
    let mut ordered: Vec<&Atom> = atoms.iter().collect();
    let mut keys: HashMap<&Atom, (String, Vec<String>)> = HashMap::new();
    for atom in &ordered {
        let Some(key) = atom_sort_key(atom, symbols) else {
            return Err(CertificationFailure::Unsupported(
                "non-ground atom outside the sat-backed fragment",
            ));
        };
        keys.insert(atom, key);
    }
    ordered.sort_by_key(|atom| keys.get(atom).cloned());
    let mut var_of: HashMap<&Atom, i32> = HashMap::with_capacity(ordered.len());
    for (index, atom) in ordered.iter().enumerate() {
        var_of.insert(atom, index as i32 + 1);
    }
    let mut encoded = Vec::with_capacity(grounded.len());
    for (index, clause) in grounded.iter().enumerate() {
        // Amortized deadline check: encoding millions of clauses must not
        // burn unbounded wall time past the budget.
        if index & 0xFFF == 0 && Instant::now() >= deadline {
            return Err(CertificationFailure::Limit(
                "certification time limit exceeded",
            ));
        }
        if clause.is_empty() {
            // An empty grounded clause means unsatisfiability without a
            // TSTP-ancestry proof: Tier-3-eligible, like solver UNSAT.
            return Err(CertificationFailure::Tier2Unsat);
        }
        if clause.is_tautology() {
            continue;
        }
        let mut lits = Vec::with_capacity(clause.literals.len());
        for literal in &clause.literals {
            let Some(var) = var_of.get(&literal.atom) else {
                return Err(CertificationFailure::Unsupported(
                    "ground atom missing from fragment atom set",
                ));
            };
            lits.push(if literal.positive { *var } else { -*var });
        }
        encoded.push(EncodedSatClause {
            source: clause.id,
            lits,
        });
    }
    Ok((encoded, ordered.into_iter().cloned().collect()))
}

/// Independently re-verify a solver model: every encoded clause must have a
/// literal the model assigns true. `value` takes a *variable* id (matching
/// `mrs_cadical::Solver::value`, which reports the variable assignment even
/// for negative arguments — passing a signed literal through directly would
/// invert negative literals) and the polarity is compared here. Unassigned
/// (`None`) variables never satisfy — a partial model that leaves a clause
/// uncovered is rejected (fail closed), never accepted.
pub(crate) fn verify_model(
    encoded: &[EncodedSatClause],
    value: &dyn Fn(i32) -> Option<bool>,
) -> bool {
    encoded.iter().all(|clause| {
        clause.lits.iter().any(|&literal| {
            let var = literal.abs();
            match value(var) {
                Some(assigned) => assigned == (literal > 0),
                None => false,
            }
        })
    })
}

/// Decide a large grounding with CaDiCaL: satisfiability via a verified
/// model, unsatisfiability via an emitted FRAT-backed TSTP refutation.
/// Every failure mode fails closed (Tier-2-eligible for Tier-3 subset
/// search, otherwise `GaveUp` upstream).
#[allow(clippy::too_many_arguments)]
pub(crate) fn certify_sat_backed(
    grounded: &[Clause],
    originals: &[Clause],
    provenance: &[Clause],
    atoms: &[Atom],
    symbols: &SymbolTable,
    id_gen: &mut ClauseIdGen,
    time_limit: Duration,
) -> Result<CertifiedGroundReport, CertificationFailure> {
    let deadline = Instant::now() + time_limit;
    let (encoded, ordered_atoms) = encode_sat(grounded, atoms, symbols, deadline)?;
    let var_count = ordered_atoms.len();
    trace_certify(format!(
        "sat_encoded vars={var_count} clauses={} skipped={}",
        encoded.len(),
        grounded.len().saturating_sub(encoded.len()),
    ));
    if encoded.is_empty() {
        // Tautology-only input: valid in every model, still EPR-checked.
        trace_certify("sat_outcome=vacuous_sat".to_string());
        return Ok(CertifiedGroundReport {
            result: SearchResult::Saturated(CompletenessWitness::sat_backed_grounding()),
            stats: SearchStats {
                processed: grounded.len() as u64,
                ..SearchStats::default()
            },
            tier: crate::certified::CertifiedTier::Two,
        });
    }
    let mut solver = Solver::new();
    // Always-trace (deterministic, simpler): the SAT path pays the tracing
    // overhead too, and the UNSAT path needs the events. Capture failures
    // fail closed; they never change the verdict mapping below.
    if solver
        .connect_trace(TraceConfig {
            antecedents: true,
            finalize_clauses: true,
            max_events: SAT_TRACE_MAX_EVENTS,
        })
        .is_err()
    {
        return Err(CertificationFailure::Limit("sat trace capture failed"));
    }
    for clause in &encoded {
        if Instant::now() >= deadline {
            trace_certify("sat_outcome=add_timeout".to_string());
            return Err(CertificationFailure::Limit(
                "certification time limit exceeded",
            ));
        }
        solver.add_clause(&clause.lits);
    }
    match solver.solve_until(deadline) {
        SolveResult::Sat => {
            // The trace is unneeded on the SAT path (the verified model is
            // the certificate); disconnect errors are irrelevant here.
            let _ = solver.disconnect_trace();
            let model_ok = verify_model(&encoded, &|literal| solver.value(literal));
            trace_certify(format!(
                "sat_outcome=sat vars={var_count} clauses={} model_ok={model_ok}",
                encoded.len(),
            ));
            if !model_ok {
                return Err(CertificationFailure::Unsupported(
                    "sat model failed independent verification",
                ));
            }
            // The re-verified model is the certificate, so hand it out: a
            // satisfiability claim with no model is worth nothing to a
            // competition, and the kernel already knows how to re-check one.
            // A model we cannot express completely is simply not reported —
            // the saturation verdict does not depend on it.
            let certificate =
                build_model_certificate(&ordered_atoms, originals, symbols, &|variable| {
                    solver.value(variable)
                });
            trace_certify(format!(
                "sat_model_certificate={}",
                if certificate.is_some() { "yes" } else { "no" }
            ));
            Ok(CertifiedGroundReport {
                result: SearchResult::Saturated(
                    CompletenessWitness::sat_backed_grounding().with_model(certificate),
                ),
                stats: SearchStats {
                    processed: grounded.len() as u64,
                    ..SearchStats::default()
                },
                tier: crate::certified::CertifiedTier::Two,
            })
        }
        SolveResult::Unsat => {
            trace_certify(format!(
                "sat_outcome=unsat vars={var_count} clauses={}",
                encoded.len(),
            ));
            // Capture, re-check, and emit a FRAT-backed TSTP refutation.
            // Every failure below maps to Tier2Unsat (fail closed, Tier-3
            // may still find a small-core proof).
            let report = match capture_and_check(&mut solver) {
                Ok(report) => report,
                Err(CertificationFailure::Limit(reason)) => {
                    trace_certify(format!("sat_trace_capture failed:{reason}"));
                    return Err(CertificationFailure::Tier2Unsat);
                }
                Err(other) => return Err(other),
            };
            let check = match &report.check {
                Ok(()) => "ok".to_string(),
                Err(reason) => format!("rejected:{reason}"),
            };
            trace_certify(format!(
                "sat_trace_capture events={} originals={} derived={} check={check}",
                report.events, report.originals, report.derived
            ));
            if report.check.is_err() {
                return Err(CertificationFailure::Tier2Unsat);
            }
            // Emission maps every failure to Tier2Unsat internally, so any
            // error here still routes to Tier-3 subset search.
            let (empty_id, tstp) = match emit_sat_refutation(
                grounded,
                originals,
                provenance,
                &encoded,
                &report.trace,
                var_count as u32,
                symbols,
                id_gen,
            ) {
                Ok(emitted) => emitted,
                Err(_) => return Err(CertificationFailure::Tier2Unsat),
            };
            Ok(CertifiedGroundReport {
                result: SearchResult::Refutation(empty_id, tstp),
                stats: SearchStats {
                    processed: grounded.len() as u64,
                    generated: report.derived as u64,
                    ..SearchStats::default()
                },
                tier: crate::certified::CertifiedTier::Two,
            })
        }
        SolveResult::Unknown => {
            trace_certify("sat_outcome=unknown".to_string());
            Err(CertificationFailure::Limit(
                "sat solver did not decide within budget",
            ))
        }
    }
}

/// Emit a FRAT-backed TSTP refutation for a solver-proved UNSAT grounding.
///
/// The manifest lists every trace original in event order; each entry is
/// mapped back to its grounded clause by multiset-normalized content (both
/// sides sorted — CaDiCaL may reorder or deduplicate literals internally,
/// so exact-sequence matching would spuriously fail). Tautologies skipped
/// by the encoder never appear in the trace and need no mapping. The final
/// empty clause cites every distinct mapped instance; proof extraction
/// prunes to that ancestry and the renderer emits `sat_backed_refutation`
/// with the trace payload for the kernel.
///
/// Every failure maps to `Tier2Unsat` (fail closed → Tier-3 subset search);
/// the only success is a complete, renderable refutation.
#[allow(clippy::too_many_arguments)]
fn emit_sat_refutation(
    grounded: &[Clause],
    originals: &[Clause],
    provenance: &[Clause],
    encoded: &[EncodedSatClause],
    trace: &ProofTrace,
    var_count: u32,
    symbols: &SymbolTable,
    id_gen: &mut ClauseIdGen,
) -> Result<(ClauseId, String), CertificationFailure> {
    use std::collections::HashMap as Map;
    // Content map: normalized encoded literals -> source clause id.
    let mut content_map: Map<Vec<i32>, ClauseId> = Map::new();
    for clause in encoded {
        let mut key = clause.lits.clone();
        key.sort_unstable();
        content_map.entry(key).or_insert(clause.source);
    }
    // Manifest in trace-original order, each entry mapped to a source.
    let mut manifest: Vec<Vec<i32>> = Vec::new();
    let mut original_ids: Vec<i64> = Vec::new();
    let mut cited_sources: Vec<ClauseId> = Vec::new();
    for event in &trace.events {
        let ProofEvent::OriginalClause { id, clause, .. } = event else {
            continue;
        };
        let mut key = clause.clone();
        key.sort_unstable();
        let Some(source) = content_map.get(&key).copied() else {
            trace_certify(format!("sat_emit=fail:unmapped_original frat_id={id}"));
            return Err(CertificationFailure::Tier2Unsat);
        };
        original_ids.push(*id);
        manifest.push(clause.clone());
        if !cited_sources.contains(&source) {
            cited_sources.push(source);
        }
    }
    if manifest.len() > SAT_EMIT_MAX_MANIFEST {
        trace_certify(format!(
            "sat_emit=fail:manifest_too_large entries={}",
            manifest.len()
        ));
        return Err(CertificationFailure::Tier2Unsat);
    }
    let trace_bytes = match encode_frat_ascii(trace) {
        Ok(bytes) => bytes,
        Err(_) => {
            trace_certify("sat_emit=fail:frat_encode".to_string());
            return Err(CertificationFailure::Tier2Unsat);
        }
    };
    if trace_bytes.len() > SAT_EMIT_MAX_TRACE_BYTES {
        trace_certify(format!(
            "sat_emit=fail:trace_too_large bytes={}",
            trace_bytes.len()
        ));
        return Err(CertificationFailure::Tier2Unsat);
    }
    let cited_indices: Vec<usize> = (0..manifest.len()).collect();
    let digest = avatar_sat_trace_digest(
        "frat-lrat",
        var_count,
        &original_ids,
        &cited_indices,
        &manifest,
        &trace_bytes,
    );
    let empty_id = id_gen.next();
    let mut empty = Clause::new(
        empty_id,
        Vec::<mrs_core::clause::Literal>::new(),
        ClauseSource::Inference {
            rule: "sat_backed_refutation",
            parents: cited_sources.clone().into(),
        },
    );
    empty.certificate = Some(ClauseCertificate::SatBackedRefutation {
        inputs: cited_sources,
        sat_trace: Some(AvatarSatTrace {
            format: "frat-lrat",
            variables: var_count,
            original_ids,
            cited_indices,
            clauses: manifest,
            trace: trace_bytes,
            digest,
        }),
    });
    let mut store: HashMap<ClauseId, Clause> = HashMap::new();
    for clause in provenance {
        store.insert(clause.id, clause.clone());
    }
    // Originals FIRST so input leaves resolve: instance nodes cite them
    // as instantiation parents, and extraction drops any parent missing
    // from the store (omitting this surfaced as a bogus "missing parent"
    // topological failure on large proofs, where the missing leaf was an
    // original far from the cited set).
    for clause in originals {
        store.insert(clause.id, clause.clone());
    }
    for clause in grounded {
        store.insert(clause.id, clause.clone());
    }
    store.insert(empty_id, empty);
    let proof = mrs_proof::extract::extract_proof(empty_id, &store);
    let tstp = mrs_proof::tstp::format_tstp(&proof, symbols);
    if tstp.is_empty() {
        trace_certify("sat_emit=fail:empty_tstp".to_string());
        return Err(CertificationFailure::Tier2Unsat);
    }
    trace_certify(format!(
        "sat_emit=ok parents={} tstp_bytes={}",
        proof.len(),
        tstp.len()
    ));
    Ok((empty_id, tstp))
}

/// Extract a finite model of an already-grounded, satisfiable clause set.
///
/// The ordered-closure tier proves satisfiability by *agreement* of two
/// closures, which yields no model: a competition satisfiability answer is only
/// credited when a model is printed. The grounded set is finite and known to be
/// satisfiable, so asking CaDiCaL for one and re-verifying it clause by clause
/// is both cheap relative to the closures and the same evidence the SAT-backed
/// tier already relies on. Any failure returns `None`: the tier's verdict does
/// not depend on the model, it only decides whether one can be printed.
pub(crate) fn extract_model(
    grounded: &[Clause],
    originals: &[Clause],
    atoms: &[Atom],
    symbols: &SymbolTable,
    deadline: Instant,
) -> Option<ModelCertificate> {
    let (encoded, ordered_atoms) = encode_sat(grounded, atoms, symbols, deadline).ok()?;
    let mut solver = Solver::new();
    for clause in &encoded {
        if Instant::now() >= deadline {
            return None;
        }
        solver.add_clause(&clause.lits);
    }
    if solver.solve_until(deadline) != SolveResult::Sat {
        return None;
    }
    if !verify_model(&encoded, &|literal| solver.value(literal)) {
        return None;
    }
    build_model_certificate(&ordered_atoms, originals, symbols, &|variable| {
        solver.value(variable)
    })
}

/// Turn a re-verified SAT assignment into a complete finite model certificate.
///
/// The domain is one element per distinct input constant, which is exactly the
/// grounding the certifier used, and every predicate of the input gets a full
/// table over that domain. Atoms the solver left unconstrained are false,
/// which is sound: the model only has to satisfy the problem, and the encoder
/// re-check already proved it does.
///
/// Returns `None` rather than a partial certificate — a missing interpretation
/// would be rejected by the kernel anyway, and a model is only worth emitting
/// if it is complete.
pub(crate) fn build_model_certificate(
    ordered_atoms: &[Atom],
    originals: &[Clause],
    symbols: &SymbolTable,
    value: &dyn Fn(i32) -> Option<bool>,
) -> Option<ModelCertificate> {
    // Signature of the input: every constant and every predicate with its
    // arity, whether or not the grounding mentions it.
    let mut constants: BTreeSet<String> = BTreeSet::new();
    let mut predicates: BTreeMap<String, usize> = BTreeMap::new();
    for clause in originals {
        for literal in &clause.literals {
            match &literal.atom {
                Atom::Pred(predicate, args) => {
                    let name = symbols.resolve(*predicate).to_string();
                    match predicates.get(&name) {
                        Some(arity) if *arity != args.len() => return None,
                        _ => {
                            predicates.insert(name, args.len());
                        }
                    }
                    for arg in args {
                        collect_constant_name(arg, symbols, &mut constants)?;
                    }
                }
                // Equality is expanded by congruence before this tier, and a
                // non-ground or function term is outside the fragment.
                Atom::Eq(left, right) => {
                    collect_constant_name(left, symbols, &mut constants)?;
                    collect_constant_name(right, symbols, &mut constants)?;
                }
            }
        }
    }
    // The domain must cover every ground atom the solver reasoned about, not
    // only the constants the input clauses mention: an atom mentioning a
    // constant outside the domain has no interpretation at all.
    for atom in ordered_atoms {
        let Atom::Pred(predicate, args) = atom else {
            return None;
        };
        // A predicate the solver reasoned about but the input clauses do not
        // name still needs a table: the assignment refers to it. Extra entries
        // are harmless to a checker, a missing one is not.
        let name = symbols.resolve(*predicate).to_string();
        match predicates.get(&name) {
            Some(arity) if *arity != args.len() => return None,
            _ => {
                predicates.insert(name, args.len());
            }
        }
        for arg in args {
            collect_constant_name(arg, symbols, &mut constants)?;
        }
    }
    let domain: Vec<String> = constants.iter().cloned().collect();
    if domain.is_empty() {
        return None;
    }
    let domain_size = domain.len();
    let positions: BTreeMap<&str, usize> = domain
        .iter()
        .enumerate()
        .map(|(index, name)| (name.as_str(), index))
        .collect();

    // Seed every table with `false`, then set the atoms the solver assigned.
    let mut total_entries = 0usize;
    let mut tables = BTreeMap::new();
    for (name, arity) in &predicates {
        let length = if *arity == 0 {
            1
        } else {
            domain_size.checked_pow(u32::try_from(*arity).ok()?)?
        };
        total_entries = total_entries.checked_add(length)?;
        if total_entries > MODEL_TABLE_MAX_ENTRIES {
            return None;
        }
        tables.insert(
            name.clone(),
            PredicateTable {
                arity: *arity,
                table: vec![false; length],
            },
        );
    }

    for (index, atom) in ordered_atoms.iter().enumerate() {
        let Atom::Pred(predicate, args) = atom else {
            return None;
        };
        let variable = index as i32 + 1;
        // An unset variable is unconstrained; `false` is a sound choice for a
        // model that only has to satisfy the problem.
        if value(variable) != Some(true) {
            continue;
        }
        let name = symbols.resolve(*predicate);
        let mut tuple = Vec::with_capacity(args.len());
        for arg in args {
            let mrs_core::term::Term::App(constant, inner) = arg else {
                return None;
            };
            if !inner.is_empty() {
                return None;
            }
            tuple.push(*positions.get(symbols.resolve(*constant))?);
        }
        let table = tables.get_mut(name)?;
        let index = table_index(table.arity, domain_size, &tuple)?;
        table.table[index] = true;
    }

    let mut certificate = ModelCertificate {
        domain_size,
        constants: domain
            .iter()
            .enumerate()
            .map(|(index, name)| (name.clone(), index))
            .collect(),
        functions: BTreeMap::new(),
        predicates: tables,
        equality: EqualitySemantics::StrictIdentity,
        digest: String::new(),
    };
    certificate.digest = certificate.compute_digest();
    Some(certificate)
}

fn collect_constant_name(
    term: &mrs_core::term::Term,
    symbols: &SymbolTable,
    constants: &mut BTreeSet<String>,
) -> Option<()> {
    match term {
        mrs_core::term::Term::App(symbol, args) if args.is_empty() => {
            constants.insert(symbols.resolve(*symbol).to_string());
            Some(())
        }
        mrs_core::term::Term::Var(_) => Some(()),
        _ => None,
    }
}

fn table_index(_arity: usize, domain_size: usize, tuple: &[usize]) -> Option<usize> {
    let mut index = 0usize;
    for value in tuple {
        index = index.checked_mul(domain_size)?.checked_add(*value)?;
    }
    Some(index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::clause::{ClauseId, ClauseIdGen, ClauseSource, Literal};
    use mrs_core::symbol::SymbolTable;
    use mrs_core::term::Term;

    fn input_clause(id_gen: &mut ClauseIdGen, literals: Vec<Literal>) -> Clause {
        Clause::new(
            id_gen.next(),
            literals,
            ClauseSource::Input {
                name: "test".into(),
                role: "axiom".into(),
            },
        )
    }

    fn sat_fixture() -> (Vec<Clause>, Vec<Atom>, SymbolTable) {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let q = symbols.intern("q");
        let a = symbols.intern("a");
        let b = symbols.intern("b");
        let mut ids = ClauseIdGen::new();
        let pa = Atom::pred(p, vec![Term::constant(a)]);
        let qb = Atom::pred(q, vec![Term::constant(b)]);
        let clauses = vec![
            input_clause(&mut ids, vec![Literal::pos(pa.clone())]),
            input_clause(&mut ids, vec![Literal::pos(qb.clone())]),
        ];
        (clauses, vec![pa, qb], symbols)
    }

    fn unsat_fixture() -> (Vec<Clause>, Vec<Atom>, SymbolTable) {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let a = symbols.intern("a");
        let mut ids = ClauseIdGen::new();
        let pa = Atom::pred(p, vec![Term::constant(a)]);
        let clauses = vec![
            input_clause(&mut ids, vec![Literal::pos(pa.clone())]),
            input_clause(&mut ids, vec![Literal::neg(pa.clone())]),
        ];
        (clauses, vec![pa], symbols)
    }

    #[test]
    fn encoding_is_deterministic_and_skips_tautologies() {
        let (mut clauses, atoms, mut symbols) = sat_fixture();
        let p = symbols.intern("p");
        let a = symbols.intern("a");
        let mut ids = ClauseIdGen::new();
        // Tautology p(a) | ~p(a): valid in every model, must be skipped.
        clauses.push(input_clause(
            &mut ids,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::constant(a)])),
                Literal::neg(Atom::pred(p, vec![Term::constant(a)])),
            ],
        ));
        let far = Instant::now() + Duration::from_secs(5);
        let (first, vars) = encode_sat(&clauses, &atoms, &symbols, far).expect("encodable");
        let (second, _) = encode_sat(&clauses, &atoms, &symbols, far).expect("encodable");
        assert_eq!(first, second, "encoding must be deterministic");
        assert_eq!(vars.len(), 2);
        assert_eq!(
            first.len(),
            2,
            "tautology must be skipped, leaving two unit clauses"
        );
        for clause in &first {
            assert_eq!(clause.lits.len(), 1);
            assert!(clause.lits[0] > 0, "both units are positive");
        }
    }

    #[test]
    fn encoding_is_stable_across_interning_orders() {
        // The atom ordering must not depend on interning order (SymbolId
        // values differ across runs/tables): the kernel re-derives it from
        // TSTP names alone, so two tables interning in opposite orders must
        // encode identically.
        let build = |p_first: bool| {
            let mut symbols = SymbolTable::new();
            let (p, q) = if p_first {
                (symbols.intern("p"), symbols.intern("q"))
            } else {
                let q = symbols.intern("q");
                let p = symbols.intern("p");
                (p, q)
            };
            let a = symbols.intern("a");
            let b = symbols.intern("b");
            let mut ids = ClauseIdGen::new();
            let clauses = vec![
                input_clause(
                    &mut ids,
                    vec![Literal::pos(Atom::pred(q, vec![Term::constant(b)]))],
                ),
                input_clause(
                    &mut ids,
                    vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
                ),
            ];
            let atoms = vec![
                Atom::pred(q, vec![Term::constant(b)]),
                Atom::pred(p, vec![Term::constant(a)]),
            ];
            let far = Instant::now() + Duration::from_secs(5);
            encode_sat(&clauses, &atoms, &symbols, far)
                .expect("encodable")
                .0
                .into_iter()
                .map(|clause| clause.lits)
                .collect::<Vec<_>>()
        };
        assert_eq!(build(true), build(false));
    }

    #[test]
    fn encoding_rejects_empty_clause_as_sat_only() {
        let mut ids = ClauseIdGen::new();
        let empty = Clause::new(
            ids.next(),
            Vec::<Literal>::new(),
            ClauseSource::Input {
                name: "test".into(),
                role: "axiom".into(),
            },
        );
        assert!(matches!(
            encode_sat(
                std::slice::from_ref(&empty),
                &[],
                &SymbolTable::new(),
                Instant::now() + Duration::from_secs(5)
            ),
            Err(CertificationFailure::Tier2Unsat)
        ));
    }

    #[test]
    fn solver_value_reports_variable_assignment_regardless_of_sign() {
        // Locks the FFI convention verify_model depends on: value()
        // reports the VARIABLE assignment even for negative arguments
        // (value(-1) is Some(true) when v1 is true, although the literal
        // ¬v1 is false). Hence verify_model must compare polarity itself —
        // passing signed literals straight through would invert every
        // negative literal. Forces v1=true, v2=false.
        let mut solver = mrs_cadical::Solver::new();
        solver.add_clause([1]);
        solver.add_clause([-2]);
        assert_eq!(solver.solve(), mrs_cadical::SolveResult::Sat);
        assert_eq!(solver.value(1), Some(true));
        assert_eq!(solver.value(-1), Some(true));
        assert_eq!(solver.value(2), Some(false));
        assert_eq!(solver.value(-2), Some(false));
    }

    fn encoded_unit(source: ClauseId, lit: i32) -> EncodedSatClause {
        EncodedSatClause {
            source,
            lits: vec![lit],
        }
    }

    #[test]
    fn model_verifier_accepts_valid_and_rejects_broken_models() {
        // (p(a)) & (~p(a) | q(b)): variable assignment {1=T, 2=T} verifies.
        // Closures take variable ids (matching Solver::value semantics).
        let encoded = vec![
            encoded_unit(ClauseId(1), 1),
            EncodedSatClause {
                source: ClauseId(2),
                lits: vec![-1, 2],
            },
        ];
        let good = |var: i32| match var {
            1 | 2 => Some(true),
            _ => None,
        };
        assert!(verify_model(&encoded, &good));
        // Flipped q ({1=T, 2=F}): second clause uncovered.
        let flipped = |var: i32| match var {
            1 => Some(true),
            2 => Some(false),
            _ => None,
        };
        assert!(!verify_model(&encoded, &flipped));
        // Partial model leaving q unassigned: uncovered.
        let partial = |var: i32| match var {
            1 => Some(true),
            _ => None,
        };
        assert!(!verify_model(&encoded, &partial));
        // A model satisfying only via a negative literal: {1=F} covers -1.
        let negative = |var: i32| match var {
            1 => Some(false),
            _ => None,
        };
        assert!(verify_model(&[encoded_unit(ClauseId(1), -1)], &negative));
        assert!(!verify_model(&[encoded_unit(ClauseId(1), 1)], &negative));
        // Empty clause can never verify (but encode_sat never emits one).
        assert!(!verify_model(
            &[EncodedSatClause {
                source: ClauseId(1),
                lits: vec![],
            }],
            &good
        ));
        // Vacuous input verifies.
        assert!(verify_model(&[], &|_| None));
    }

    #[test]
    fn sat_path_certifies_small_sat_grounding() {
        let (clauses, atoms, symbols) = sat_fixture();
        let report = certify_sat_backed(
            &clauses,
            &clauses,
            &[],
            &atoms,
            &symbols,
            &mut ClauseIdGen::new(),
            Duration::from_secs(5),
        )
        .expect("tiny SAT grounding must certify");
        assert!(matches!(
            report.result,
            SearchResult::Saturated(witness)
                if witness.reason() == crate::SaturationReason::SatBackedGrounding
        ));
        assert_eq!(report.tier, crate::certified::CertifiedTier::Two);
    }

    #[test]
    fn unsat_capture_records_checkable_proof() {
        // Capture-first contract: a tiny UNSAT solve yields originals plus
        // an independently re-checked proof (no RAT witnesses from a plain
        // unit-conflict solve).
        let mut solver = mrs_cadical::Solver::new();
        solver
            .connect_trace(mrs_cadical::TraceConfig {
                antecedents: true,
                finalize_clauses: true,
                max_events: 1024,
            })
            .expect("trace must connect");
        solver.add_clause([1]);
        solver.add_clause([-1]);
        assert_eq!(solver.solve(), mrs_cadical::SolveResult::Unsat);
        let report = capture_and_check(&mut solver).expect("capture must succeed");
        assert_eq!(report.originals, 2);
        assert!(
            report.check.is_ok(),
            "plain unit-conflict proof must re-check, got {:?}",
            report.check
        );
    }

    #[test]
    fn sat_path_emits_refutation_on_unsat() {
        // Tier-2 UNSAT now emits a FRAT-backed TSTP refutation (emission
        // phase): the tiny unit conflict captures, re-checks, and renders.
        let (clauses, atoms, symbols) = unsat_fixture();
        // Advance past the fixture's input ids: production always passes
        // the live post-grounding generator, so the emitted empty clause
        // id must not collide with an input id (a collision would make
        // the empty clause its own parent and fail topological sort).
        let mut ids = ClauseIdGen::new();
        ids.next();
        ids.next();
        let report = certify_sat_backed(
            &clauses,
            &clauses,
            &[],
            &atoms,
            &symbols,
            &mut ids,
            Duration::from_secs(5),
        )
        .expect("tiny UNSAT grounding must emit a refutation");
        match report.result {
            SearchResult::Refutation(_, ref tstp) => {
                assert!(
                    tstp.contains("sat_backed_refutation"),
                    "refutation must carry the sat-backed rule"
                );
                assert!(tstp.contains("$false"), "refutation must conclude $false");
            }
            other => panic!("expected a refutation, got {other:?}"),
        }
    }

    #[test]
    fn sat_verdict_agrees_with_closure_on_tier1_fixtures() {
        // Differential cross-validation: where both tiers run, the SAT
        // verdict must agree with the double-closure verdict.
        let (sat_clauses, sat_atoms, symbols) = sat_fixture();
        let tier1 = crate::certified::certify_ground_ordered_resolution(
            &sat_clauses,
            &[],
            &symbols,
            &crate::TermOrdering::KBO,
            &mut ClauseIdGen::new(),
            Duration::from_secs(5),
        )
        .expect("tier 1 must decide tiny SAT");
        assert!(matches!(tier1.result, SearchResult::Saturated(_)));
        let tier2 = certify_sat_backed(
            &sat_clauses,
            &sat_clauses,
            &[],
            &sat_atoms,
            &symbols,
            &mut ClauseIdGen::new(),
            Duration::from_secs(5),
        )
        .expect("tier 2 must decide tiny SAT");
        assert!(matches!(tier2.result, SearchResult::Saturated(_)));

        let (unsat_clauses, unsat_atoms, _) = unsat_fixture();
        let tier1 = crate::certified::certify_ground_ordered_resolution(
            &unsat_clauses,
            &[],
            &symbols,
            &crate::TermOrdering::KBO,
            &mut ClauseIdGen::new(),
            Duration::from_secs(5),
        )
        .expect("tier 1 must decide tiny UNSAT");
        assert!(matches!(tier1.result, SearchResult::Refutation(..)));
        // Agreement extends to UNSAT now that Tier 2 emits refutations
        // (ids advanced past the fixture inputs, as production guarantees).
        let mut ids = ClauseIdGen::new();
        ids.next();
        ids.next();
        let tier2 = certify_sat_backed(
            &unsat_clauses,
            &unsat_clauses,
            &[],
            &unsat_atoms,
            &symbols,
            &mut ids,
            Duration::from_secs(5),
        )
        .expect("tier 2 must decide tiny UNSAT");
        assert!(matches!(tier2.result, SearchResult::Refutation(..)));
    }

    #[test]
    fn model_certificate_is_complete_over_the_input_signature() {
        // p(a), ~p(b), and a unary q the problem mentions nowhere: the
        // certificate still has to interpret every predicate the input uses,
        // because a partial interpretation is not a model.
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let r = syms.intern("r");
        syms.intern("unused_symbol");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let pa = Atom::pred(p, vec![Term::constant(a)]);
        let pb = Atom::pred(p, vec![Term::constant(b)]);
        let ra = Atom::pred(r, vec![Term::constant(a)]);

        let originals = vec![Clause::new(
            ClauseId(1),
            vec![Literal::pos(pa.clone())],
            ClauseSource::Input {
                name: "a1".into(),
                role: "axiom".into(),
            },
        )];

        // Variable order: 1 -> p(a), 2 -> p(b), 3 -> r(a). Only p(a) is set.
        let ordered = vec![pa.clone(), pb.clone(), ra.clone()];
        let certificate = build_model_certificate(&ordered, &originals, &syms, &|variable| {
            (variable == 1).then_some(true)
        })
        .expect("certificate is produced");

        assert_eq!(certificate.domain_size, 2);
        assert_eq!(certificate.constants.get("a"), Some(&0));
        assert_eq!(certificate.constants.get("b"), Some(&1));
        // A symbol the input never uses must not be interpreted either.
        assert!(!certificate.predicates.contains_key("unused_symbol"));
        let p_table = &certificate.predicates["p"];
        assert_eq!(p_table.arity, 1);
        assert_eq!(p_table.table, vec![true, false]);
        // Tables are complete over the whole domain, not just the tuples the
        // solver happened to assign.
        let r_table = &certificate.predicates["r"];
        assert_eq!(r_table.arity, 1);
        assert_eq!(r_table.table, vec![false, false]);
        assert_eq!(
            certificate.digest,
            certificate.compute_digest(),
            "the digest must describe the certificate it ships with"
        );
    }

    #[test]
    fn model_certificate_refuses_incomplete_signatures() {
        // A predicate used with two different arities is not a model of
        // anything; refuse rather than emit a certificate the kernel rejects.
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let pa = Atom::pred(p, vec![Term::constant(a)]);
        let pab = Atom::pred(p, vec![Term::constant(a), Term::constant(b)]);
        let originals = vec![Clause::new(
            ClauseId(1),
            vec![Literal::pos(pa.clone()), Literal::pos(pab)],
            ClauseSource::Input {
                name: "a1".into(),
                role: "axiom".into(),
            },
        )];
        assert!(build_model_certificate(&[pa], &originals, &syms, &|_| Some(true)).is_none());
    }

    #[test]
    fn model_certificate_is_absent_when_the_solver_leaves_an_atom_unknown() {
        // Unset variables are modelled as false, so a model is still produced;
        // what must never happen is a *partial* table.
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let pa = Atom::pred(p, vec![Term::constant(a)]);
        let originals = vec![Clause::new(
            ClauseId(1),
            vec![Literal::pos(pa.clone())],
            ClauseSource::Input {
                name: "a1".into(),
                role: "axiom".into(),
            },
        )];
        let certificate =
            build_model_certificate(&[pa], &originals, &syms, &|_| None).expect("certificate");
        assert_eq!(certificate.predicates["p"].table, vec![false]);
    }
}
