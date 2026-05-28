//! ATP-enabled integration tests. These require either `eprover` or `vampire`
//! to be available; they are skipped (with an informational message) otherwise.

use std::path::PathBuf;

use mrs_proover::atp::{EProverAtp, LadderAtp, VampireAtp, find_eprover, find_vampire};
use mrs_proover::load::load;
use mrs_proover::verdict::Verdict;
use mrs_proover::verify::{Settings, verify_with};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn make_ladder() -> Option<LadderAtp> {
    let mut l = LadderAtp::new();
    if let Some(p) = find_eprover() {
        l = l.push(Box::new(EProverAtp::new(p)));
    }
    if let Some(p) = find_vampire() {
        l = l.push(Box::new(VampireAtp::new(p)));
    }
    if l.backends.is_empty() { None } else { Some(l) }
}

fn run_with_atp(proof: &str) -> Option<Verdict> {
    let ladder = make_ladder()?;
    let dir = fixtures_dir();
    let job = load(&dir.join(proof), Some(&dir)).expect("load failed");
    let settings = Settings::default();
    Some(verify_with(&job, &settings, &ladder))
}

#[test]
fn good_examples_all_verified_with_atp() {
    let Some(_) = make_ladder() else {
        eprintln!("skipping: no ATP backend found");
        return;
    };
    for proof in [
        "example1_c_proof.p",
        "example2_c_proof.p",
        "example3_c_proof.p",
    ] {
        let v = run_with_atp(proof).unwrap();
        assert_eq!(v, Verdict::Verified, "{proof}: got {v:?}");
    }
}

#[test]
fn evil_examples_all_failedverified_with_atp() {
    let Some(_) = make_ladder() else {
        eprintln!("skipping: no ATP backend found");
        return;
    };
    for proof in [
        "example1_e_proof.p",
        "example2_e_proof.p",
        "example3_e_proof.p",
        "example4_e_proof.p",
    ] {
        let v = run_with_atp(proof).unwrap();
        assert!(
            matches!(v, Verdict::FailedVerified(_)),
            "{proof}: got {v:?}"
        );
    }
}
