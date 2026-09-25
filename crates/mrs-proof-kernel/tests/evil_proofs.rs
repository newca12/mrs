use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use mrs_proof_kernel::{KernelVerdict, VerificationLimits, verify_strict};
use mrs_tptp::parse_tptp;

/// Outcome of checking one committed evil-proof case.
enum CaseOutcome {
    /// Problem or proof did not parse; skipped, as in the serial version.
    Skipped(String),
    /// Parsed and kernel-checked; carries whether it wrongly certified.
    Checked { name: String, certified: bool },
}

#[test]
fn committed_evil_proofs_never_certify() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("evil-proofs")
        .join("exploits");
    let mut cases = Vec::new();
    for entry in std::fs::read_dir(&root).expect("evil proof corpus reads") {
        let entry = entry.expect("evil proof directory entry reads");
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let problem = path.join("problem.p");
        let proof = path.join("proof.p");
        if problem.is_file() && proof.is_file() {
            cases.push((
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
                problem,
                proof,
            ));
        }
    }
    cases.sort_by(|left, right| left.0.cmp(&right.0));
    assert!(!cases.is_empty(), "evil proof corpus is empty");

    // Read all inputs up front so worker threads only borrow.
    let mut inputs = Vec::with_capacity(cases.len());
    for (name, problem_path, proof_path) in &cases {
        let problem_text = std::fs::read_to_string(problem_path).expect("problem reads");
        let proof_text = std::fs::read_to_string(proof_path).expect("proof reads");
        inputs.push((name.clone(), problem_text, proof_text));
    }

    // Verify cases in parallel across the available cores. `verify_strict`
    // is a pure function of its inputs (no global state), so scoped threads
    // borrowing the inputs are sound. Work-stealing via a shared index keeps
    // slow cases from straggling on one thread.
    let n_workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(inputs.len())
        .max(1);
    let next = AtomicUsize::new(0);
    let outcomes: Mutex<Vec<CaseOutcome>> = Mutex::new(Vec::with_capacity(inputs.len()));
    std::thread::scope(|scope| {
        for _ in 0..n_workers {
            scope.spawn(|| {
                let mut local = Vec::new();
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= inputs.len() {
                        break;
                    }
                    let (name, problem_text, proof_text) = &inputs[i];
                    let (Ok(problem), Ok(proof)) =
                        (parse_tptp(problem_text), parse_tptp(proof_text))
                    else {
                        local.push(CaseOutcome::Skipped(name.clone()));
                        continue;
                    };
                    let verdict = verify_strict(&problem, &proof, VerificationLimits::default());
                    local.push(CaseOutcome::Checked {
                        name: name.clone(),
                        certified: matches!(verdict, KernelVerdict::Certified),
                    });
                }
                if !local.is_empty() {
                    outcomes.lock().expect("outcomes mutex").extend(local);
                }
            });
        }
    });

    let outcomes = outcomes.into_inner().expect("outcomes mutex");
    let mut parsed_cases = 0;
    let mut certified = Vec::new();
    for outcome in outcomes {
        match outcome {
            CaseOutcome::Skipped(_) => {}
            CaseOutcome::Checked {
                name,
                certified: true,
            } => {
                parsed_cases += 1;
                certified.push(name);
            }
            CaseOutcome::Checked {
                certified: false, ..
            } => {
                parsed_cases += 1;
            }
        }
    }
    assert!(
        certified.is_empty(),
        "strict kernel certified committed evil proofs: {}",
        certified.join(", ")
    );
    assert!(
        parsed_cases > 0,
        "evil proof corpus contained no parseable cases"
    );
}
