//! Regression coverage for the adversarial CASC-J13 ProoVer corpus.
//!
//! One `#[test]` per proof ID so the libtest harness runs them in parallel
//! across all available cores (`--test-threads` defaults to the core count).
//! Each proof still verifies with `workers: 1`: the mock ATP backend answers
//! immediately, so inner parallelism would only add thread-spawn overhead
//! while outer parallelism over proofs is where the wall-time win is.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use mrs_core::{Formula, SymbolTable};
use mrs_proover::atp::{Atp, AtpVerdict, NoopAtp};
use mrs_proover::load::load;
use mrs_proover::verdict::Verdict;
use mrs_proover::verify::{Settings, verify_with};

struct AlwaysUnsound;

impl Atp for AlwaysUnsound {
    fn name(&self) -> &'static str {
        "regression-unsound"
    }

    fn check_step(
        &self,
        _symbols: &SymbolTable,
        _premises: &[Formula],
        _conclusion: &Formula,
        _budget: Duration,
        _cancel: &AtomicBool,
    ) -> AtpVerdict {
        AtpVerdict::Unsound
    }
}

fn corpus_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../mrs-bench/proover-corpus/Proover2026")
}

fn settings() -> Settings {
    Settings {
        total_budget: Duration::from_secs(30),
        per_step_budget: Duration::from_secs(3),
        verbose: false,
        workers: 1,
        strict: false,
    }
}

fn run(root: &Path, id: &str) -> Verdict {
    let proof = root.join("Proofs").join(format!("{id}.s"));
    let job = load(&proof, Some(root)).expect("corpus proof should load");
    let settings = settings();
    verify_with(&job, &settings, &AlwaysUnsound)
}

fn check_evil_rejected(id: &str) {
    let root = corpus_root();
    let verdict = run(&root, id);
    assert!(
        matches!(verdict, Verdict::VerifiedBad(_)),
        "evil regression {id} must be positively rejected: {verdict:?}"
    );
}

/// Reject an evil proof with *no* ATP at all.
///
/// `run` uses a mock ATP that refutes everything, so it only proves the
/// verdict policy is fail-closed. These tests pin the stronger property: the
/// verifier's own structural checks — leaf role/provenance, Skolem freshness
/// against the problem signature, definition non-circularity — reject the
/// proof unaided.
fn check_evil_rejected_without_atp(id: &str) {
    let root = corpus_root();
    let proof = root.join("Proofs").join(format!("{id}.s"));
    let job = load(&proof, Some(&root)).expect("corpus proof should load");
    let verdict = verify_with(&job, &settings(), &NoopAtp);
    assert!(
        matches!(verdict, Verdict::VerifiedBad(_)),
        "evil regression {id} must be rejected by the structural checks alone: {verdict:?}"
    );
}

/// A proof whose `% Proof :` link cannot be resolved must never be certified.
///
/// PRV051+1 declares its axiom with the problem's `conjecture` role and
/// PRV074+1 reuses a problem function symbol as a Skolem witness. Both are
/// caught only by comparing against the problem file, so before the
/// fail-closed policy they came back `VerifiedGood` from any working
/// directory that did not happen to contain `Problems/`.
fn assert_unresolvable_problem_never_certifies(id: &str) {
    let root = corpus_root();
    let text = std::fs::read_to_string(root.join("Proofs").join(format!("{id}.s")))
        .expect("corpus proof should be readable");
    let dir = std::env::temp_dir().join(format!("mrs-proover-noproblem-{id}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir should be creatable");
    let proof = dir.join(format!("{id}.s"));
    std::fs::write(&proof, &text).expect("proof should be copied");

    let job = load(&proof, None).expect("proof itself should still parse");
    assert!(
        job.problem.is_none(),
        "{id}: fixture is only meaningful when the problem cannot be resolved"
    );
    let verdict = verify_with(&job, &settings(), &AlwaysUnsound);
    assert!(
        !matches!(verdict, Verdict::VerifiedGood),
        "{id} must not be certified without its problem file: {verdict:?}"
    );
    // With the problem reachable the same proof must be positively rejected,
    // which pins both directions of the policy.
    let linked = load(&proof, Some(&root)).expect("proof should load with a problems root");
    assert!(linked.problem.is_some());
    let verdict = verify_with(&linked, &settings(), &NoopAtp);
    assert!(
        matches!(verdict, Verdict::VerifiedBad(_)),
        "{id} must be rejected once its problem file is available: {verdict:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

macro_rules! evil_regression_test {
    ($test_name:ident, $id:literal) => {
        #[test]
        fn $test_name() {
            check_evil_rejected($id);
        }
    };
}

macro_rules! structural_evil_regression_test {
    ($test_name:ident, $id:literal) => {
        #[test]
        fn $test_name() {
            check_evil_rejected_without_atp($id);
        }
    };
}

macro_rules! unresolvable_problem_test {
    ($test_name:ident, $id:literal) => {
        #[test]
        fn $test_name() {
            assert_unresolvable_problem_never_certifies($id);
        }
    };
}

evil_regression_test!(prv006_rejected, "PRV006+1");
evil_regression_test!(prv008_rejected, "PRV008+1");
evil_regression_test!(prv056_rejected, "PRV056+1");
evil_regression_test!(prv057_rejected, "PRV057+1");
evil_regression_test!(prv068_rejected, "PRV068+1");
evil_regression_test!(prv072_rejected, "PRV072+1");
evil_regression_test!(prv075_rejected, "PRV075+1");
evil_regression_test!(prv077_rejected, "PRV077+1");
evil_regression_test!(prv090_rejected, "PRV090+1");
evil_regression_test!(prv094_rejected, "PRV094+1");

// The two fixtures whose only evidence lives in the problem file: a leaf whose
// role disagrees with the problem's declared role, and a Skolem witness that
// reuses a problem symbol. They are the regression for "verify modulo
// assumptions" being reachable whenever the `% Proof :` link does not resolve.
evil_regression_test!(prv051_rejected, "PRV051+1");
evil_regression_test!(prv074_rejected, "PRV074+1");
structural_evil_regression_test!(prv006_rejected_without_atp, "PRV006+1");
structural_evil_regression_test!(prv008_rejected_without_atp, "PRV008+1");
structural_evil_regression_test!(prv051_rejected_without_atp, "PRV051+1");
structural_evil_regression_test!(prv057_rejected_without_atp, "PRV057+1");
structural_evil_regression_test!(prv072_rejected_without_atp, "PRV072+1");
structural_evil_regression_test!(prv074_rejected_without_atp, "PRV074+1");
structural_evil_regression_test!(prv075_rejected_without_atp, "PRV075+1");
structural_evil_regression_test!(prv077_rejected_without_atp, "PRV077+1");
structural_evil_regression_test!(prv090_rejected_without_atp, "PRV090+1");
structural_evil_regression_test!(prv094_rejected_without_atp, "PRV094+1");
unresolvable_problem_test!(prv051_not_certified_without_problem, "PRV051+1");
unresolvable_problem_test!(prv074_not_certified_without_problem, "PRV074+1");
