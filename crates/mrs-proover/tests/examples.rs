//! Integration tests against the ProoVer 2026 example proofs.
//!
//! These do NOT require any external ATP: the `Verdict` we expect from each
//! example is reachable by the structural / specified-rule checks alone (for
//! evil examples) or is at worst `NotVerified` (for good examples, which need
//! ATP support that ships in phase 6+).

use std::path::PathBuf;

use mrs_proover::load::load;
use mrs_proover::verdict::Verdict;
use mrs_proover::verify::{Settings, verify};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn run(proof: &str) -> Verdict {
    let dir = fixtures_dir();
    let proof_path = dir.join(proof);
    let job = load(&proof_path, Some(&dir)).expect("load failed");
    let settings = Settings::default();
    verify(&job, &settings)
}

// ----------------------------- Evil proofs --------------------------------
// All four must be reported as FailedVerified by the internal checks alone.

#[test]
fn example1_e_negated_conjecture_wrong() {
    let v = run("example1_e_proof.p");
    assert!(matches!(v, Verdict::FailedVerified(_)), "got {v:?}");
}

#[test]
fn example3_e_reused_skolem_symbol() {
    let v = run("example3_e_proof.p");
    assert!(matches!(v, Verdict::FailedVerified(_)), "got {v:?}");
}

#[test]
fn example4_e_wrong_skolemization_shape() {
    let v = run("example4_e_proof.p");
    assert!(matches!(v, Verdict::FailedVerified(_)), "got {v:?}");
}

// example2_e: bad `deduction` step. Without an ATP backend yet, we cannot
// detect this. We accept `NotVerified` for now (will become `FailedVerified`
// once the ATP bridge is wired in phase 6/7).
#[test]
fn example2_e_bad_deduction_acceptable_unknown() {
    let v = run("example2_e_proof.p");
    assert!(
        matches!(v, Verdict::NotVerified(_) | Verdict::FailedVerified(_)),
        "got {v:?}"
    );
}

// ---------------------------- Good proofs ---------------------------------
// In phases 1–5 (before the ATP bridge), all-internal checks pass but some
// `plain/thm` steps remain undecided. The expected verdict is therefore
// `NotVerified` for now. Once ATP is wired, this should turn into `Verified`.

#[test]
fn example1_c_good_proof_not_unsound() {
    let v = run("example1_c_proof.p");
    assert!(
        matches!(v, Verdict::Verified | Verdict::NotVerified(_)),
        "good proof must not be FailedVerified, got {v:?}"
    );
}

#[test]
fn example2_c_good_proof_not_unsound() {
    let v = run("example2_c_proof.p");
    assert!(
        matches!(v, Verdict::Verified | Verdict::NotVerified(_)),
        "good proof must not be FailedVerified, got {v:?}"
    );
}

#[test]
fn example3_c_good_proof_not_unsound() {
    let v = run("example3_c_proof.p");
    assert!(
        matches!(v, Verdict::Verified | Verdict::NotVerified(_)),
        "good proof must not be FailedVerified, got {v:?}"
    );
}
