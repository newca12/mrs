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

use std::collections::HashMap;
use std::time::{Duration, Instant};

use mrs_cadical::{SolveResult, Solver};
use mrs_core::clause::Clause;
use mrs_core::formula::Atom;

use crate::certified::{CertificationFailure, CertifiedGroundReport, trace_certify};
use crate::{CompletenessWitness, SearchResult, SearchStats};

/// Encode a grounded clause set as signed-integer SAT clauses under a
/// deterministic atom ordering. Tautologies are skipped (valid in any
/// model); an empty clause means the set is unsatisfiable, which this tier
/// cannot certify — that fails closed via the SAT-only rule.
pub(crate) fn encode_sat(
    grounded: &[Clause],
    atoms: &[Atom],
) -> Result<(Vec<Vec<i32>>, usize), CertificationFailure> {
    let mut ordered: Vec<&Atom> = atoms.iter().collect();
    ordered.sort_by_key(|atom| format!("{atom:?}"));
    let mut var_of: HashMap<&Atom, i32> = HashMap::with_capacity(ordered.len());
    for (index, atom) in ordered.iter().enumerate() {
        var_of.insert(atom, index as i32 + 1);
    }
    let mut encoded = Vec::with_capacity(grounded.len());
    for clause in grounded {
        if clause.is_empty() {
            return Err(CertificationFailure::Unsupported(
                "sat-backed tier certifies satisfiability only",
            ));
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
        encoded.push(lits);
    }
    Ok((encoded, ordered.len()))
}

/// Independently re-verify a solver model: every encoded clause must have a
/// literal the model assigns true. `value` takes a *variable* id (matching
/// `mrs_cadical::Solver::value`, which reports the variable assignment even
/// for negative arguments — passing a signed literal through directly would
/// invert negative literals) and the polarity is compared here. Unassigned
/// (`None`) variables never satisfy — a partial model that leaves a clause
/// uncovered is rejected (fail closed), never accepted.
pub(crate) fn verify_model(encoded: &[Vec<i32>], value: &dyn Fn(i32) -> Option<bool>) -> bool {
    encoded.iter().all(|clause| {
        clause.iter().any(|&literal| {
            let var = literal.abs();
            match value(var) {
                Some(assigned) => assigned == (literal > 0),
                None => false,
            }
        })
    })
}

/// Decide a large grounding with CaDiCaL and certify satisfiability via a
/// verified model. See the module docs for the SAT-only contract.
pub(crate) fn certify_sat_backed(
    grounded: &[Clause],
    atoms: &[Atom],
    time_limit: Duration,
) -> Result<CertifiedGroundReport, CertificationFailure> {
    let deadline = Instant::now() + time_limit;
    let (encoded, var_count) = encode_sat(grounded, atoms)?;
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
        });
    }
    let mut solver = Solver::new();
    for clause in &encoded {
        if Instant::now() >= deadline {
            trace_certify("sat_outcome=add_timeout".to_string());
            return Err(CertificationFailure::Limit(
                "certification time limit exceeded",
            ));
        }
        solver.add_clause(clause);
    }
    match solver.solve_until(deadline) {
        SolveResult::Sat => {
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
            Ok(CertifiedGroundReport {
                result: SearchResult::Saturated(CompletenessWitness::sat_backed_grounding()),
                stats: SearchStats {
                    processed: grounded.len() as u64,
                    ..SearchStats::default()
                },
            })
        }
        SolveResult::Unsat => {
            trace_certify(format!(
                "sat_outcome=unsat vars={var_count} clauses={}",
                encoded.len(),
            ));
            Err(CertificationFailure::Unsupported(
                "sat-backed tier certifies satisfiability only",
            ))
        }
        SolveResult::Unknown => {
            trace_certify("sat_outcome=unknown".to_string());
            Err(CertificationFailure::Limit(
                "sat solver did not decide within budget",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::clause::{ClauseIdGen, ClauseSource, Literal};
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

    fn unsat_fixture() -> (Vec<Clause>, Vec<Atom>) {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let a = symbols.intern("a");
        let mut ids = ClauseIdGen::new();
        let pa = Atom::pred(p, vec![Term::constant(a)]);
        let clauses = vec![
            input_clause(&mut ids, vec![Literal::pos(pa.clone())]),
            input_clause(&mut ids, vec![Literal::neg(pa.clone())]),
        ];
        (clauses, vec![pa])
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
        let (first, vars) = encode_sat(&clauses, &atoms).expect("encodable");
        let (second, _) = encode_sat(&clauses, &atoms).expect("encodable");
        assert_eq!(first, second, "encoding must be deterministic");
        assert_eq!(vars, 2);
        assert_eq!(
            first.len(),
            2,
            "tautology must be skipped, leaving two unit clauses"
        );
        for clause in &first {
            assert_eq!(clause.len(), 1);
            assert!(clause[0] > 0, "both units are positive");
        }
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
            encode_sat(std::slice::from_ref(&empty), &[]),
            Err(CertificationFailure::Unsupported(
                "sat-backed tier certifies satisfiability only"
            ))
        ));
    }

    #[test]
    fn model_verifier_accepts_valid_and_rejects_broken_models() {
        // (p(a)) & (~p(a) | q(b)): variable assignment {1=T, 2=T} verifies.
        // Closures take variable ids (matching Solver::value semantics).
        let encoded = vec![vec![1], vec![-1, 2]];
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
        assert!(verify_model(&[vec![-1]], &negative));
        assert!(!verify_model(&[vec![1]], &negative));
        // Empty clause can never verify (but encode_sat never emits one).
        assert!(!verify_model(&[vec![]], &good));
        // Vacuous input verifies.
        assert!(verify_model(&[], &|_| None));
    }

    #[test]
    fn sat_path_certifies_small_sat_grounding() {
        let (clauses, atoms, _) = sat_fixture();
        let report = certify_sat_backed(&clauses, &atoms, Duration::from_secs(5))
            .expect("tiny SAT grounding must certify");
        assert!(matches!(
            report.result,
            SearchResult::Saturated(witness)
                if witness.reason() == crate::SaturationReason::SatBackedGrounding
        ));
    }

    #[test]
    fn sat_path_fails_closed_on_unsat() {
        // Documents the Tier-2 asymmetry: unsatisfiable groundings are
        // GaveUp here (no FRAT-to-TSTP elaborator), never refutations.
        let (clauses, atoms) = unsat_fixture();
        assert!(matches!(
            certify_sat_backed(&clauses, &atoms, Duration::from_secs(5)),
            Err(CertificationFailure::Unsupported(
                "sat-backed tier certifies satisfiability only"
            ))
        ));
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
        let tier2 = certify_sat_backed(&sat_clauses, &sat_atoms, Duration::from_secs(5))
            .expect("tier 2 must decide tiny SAT");
        assert!(matches!(tier2.result, SearchResult::Saturated(_)));

        let (unsat_clauses, unsat_atoms) = unsat_fixture();
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
        // Tier 2 fails closed on UNSAT by design (see above test).
        assert!(certify_sat_backed(&unsat_clauses, &unsat_atoms, Duration::from_secs(5)).is_err());
    }
}
