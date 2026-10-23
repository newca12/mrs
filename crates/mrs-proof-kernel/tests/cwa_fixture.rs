use std::path::PathBuf;

use mrs_proof_kernel::{KernelVerdict, VerificationLimits, verify_strict_with_source};
use mrs_tptp::parse_tptp;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("resources")
        .join(name)
}

fn read_fixture(name: &str) -> String {
    std::fs::read_to_string(fixture_path(name)).expect("fixture reads")
}

#[test]
fn certifies_linked_cwa_fixture() {
    let problem_text = read_fixture("cwa_fixture_problem.p");
    let proof_text = read_fixture("cwa_fixture_proof.p");
    let problem = parse_tptp(&problem_text).expect("problem parses");
    let proof = parse_tptp(&proof_text).expect("proof parses");
    assert_eq!(
        verify_strict_with_source(
            &problem,
            &proof,
            Some("tests/resources/cwa_fixture_problem.p"),
            VerificationLimits::default(),
        ),
        KernelVerdict::Certified
    );
}

#[test]
fn rejects_mutated_cwa_branch_polarity() {
    let problem_text = read_fixture("cwa_fixture_problem.p");
    let proof_text = read_fixture("cwa_fixture_proof.p")
        .replace("fof(branch_true, plain, p,", "fof(branch_true, plain, q,");
    let problem = parse_tptp(&problem_text).expect("problem parses");
    let proof = parse_tptp(&proof_text).expect("mutated proof parses");
    assert!(matches!(
        verify_strict_with_source(
            &problem,
            &proof,
            Some("tests/resources/cwa_fixture_problem.p"),
            VerificationLimits::default(),
        ),
        KernelVerdict::Rejected(_) | KernelVerdict::Inconclusive(_)
    ));
}
