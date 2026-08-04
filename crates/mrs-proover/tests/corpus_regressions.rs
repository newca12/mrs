//! Regression coverage for the adversarial CASC-J13 ProoVer corpus.

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

#[test]
fn formerly_unsoundly_accepted_evil_proofs_are_not_verified_good() {
    let root = corpus_root();
    for id in [
        "PRV006+1", "PRV008+1", "PRV056+1", "PRV057+1", "PRV068+1", "PRV072+1", "PRV075+1",
        "PRV077+1", "PRV090+1", "PRV094+1",
    ] {
        let verdict = run(&root, id);
        assert!(
            matches!(verdict, Verdict::VerifiedBad(_)),
            "evil regression {id} must be positively rejected: {verdict:?}"
        );
    }
}
