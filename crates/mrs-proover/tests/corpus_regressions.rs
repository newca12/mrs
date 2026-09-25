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
use mrs_proover::atp::{Atp, AtpVerdict};
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

fn run(root: &Path, id: &str) -> Verdict {
    let proof = root.join("Proofs").join(format!("{id}.s"));
    let job = load(&proof, Some(root)).expect("corpus proof should load");
    let settings = Settings {
        total_budget: Duration::from_secs(30),
        per_step_budget: Duration::from_secs(3),
        verbose: false,
        workers: 1,
        strict: false,
    };
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

macro_rules! evil_regression_test {
    ($test_name:ident, $id:literal) => {
        #[test]
        fn $test_name() {
            check_evil_rejected($id);
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
