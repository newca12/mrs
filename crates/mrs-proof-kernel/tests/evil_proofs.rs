use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use mrs_proof_kernel::{KernelVerdict, VerificationLimits, verify_strict};
use mrs_tptp::parse_tptp;

/// Outcome of checking one committed evil-proof case.
struct CaseOutcome {
    name: String,
    result: Result<bool, String>,
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
    // Every thread below parses TPTP with a recursive-descent parser and then
    // replays the proof through the kernel, both of which recurse to the depth
    // of the input. A spawned thread's stack defaults to 2 MiB, which these
    // deliberately nasty cases are known to exceed, and an overflow aborts the
    // process with no output rather than failing one case. Set it here: this
    // crate depends only on sha2/serde so it cannot share the constant the
    // prover crates use, so the two must be kept in step deliberately.
    const RECURSION_STACK: usize = 64 * 1024 * 1024;

    let n_workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(inputs.len())
        .max(1);
    let next = AtomicUsize::new(0);
    let outcomes: Mutex<Vec<CaseOutcome>> = Mutex::new(Vec::with_capacity(inputs.len()));
    std::thread::scope(|scope| {
        for worker_id in 0..n_workers {
            std::thread::Builder::new()
                .name(format!("evil-proof-{worker_id}"))
                .stack_size(RECURSION_STACK)
                .spawn_scoped(scope, || {
                    let mut local = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        if i >= inputs.len() {
                            break;
                        }
                        let (name, problem_text, proof_text) = &inputs[i];
                        let result = (|| {
                            let problem = parse_tptp(problem_text)
                                .map_err(|error| format!("problem parse failed: {error}"))?;
                            let proof = parse_tptp(proof_text)
                                .map_err(|error| format!("proof parse failed: {error}"))?;
                            let verdict =
                                verify_strict(&problem, &proof, VerificationLimits::default());
                            Ok(matches!(verdict, KernelVerdict::Certified))
                        })();
                        local.push(CaseOutcome {
                            name: name.clone(),
                            result,
                        });
                    }
                    if !local.is_empty() {
                        outcomes.lock().expect("outcomes mutex").extend(local);
                    }
                })
                .unwrap_or_else(|error| panic!("could not spawn evil-proof worker: {error}"));
        }
    });

    let outcomes = outcomes.into_inner().expect("outcomes mutex");
    let mut certified = Vec::new();
    let mut failures = Vec::new();
    for CaseOutcome { name, result } in outcomes {
        match result {
            Ok(true) => certified.push(name),
            Ok(false) => {}
            Err(error) => failures.push(format!("{name}: {error}")),
        }
    }
    assert!(
        failures.is_empty(),
        "committed evil proof cases failed to parse: {}",
        failures.join("; ")
    );
    assert!(
        certified.is_empty(),
        "strict kernel certified committed evil proofs: {}",
        certified.join(", ")
    );
}
