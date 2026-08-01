use std::path::PathBuf;

use mrs_proof_kernel::{KernelVerdict, VerificationLimits, verify_strict};
use mrs_tptp::parse_tptp;

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

    let mut parsed_cases = 0;
    for (name, problem_path, proof_path) in cases {
        let problem_text = std::fs::read_to_string(&problem_path).expect("problem reads");
        let proof_text = std::fs::read_to_string(&proof_path).expect("proof reads");
        let (Ok(problem), Ok(proof)) = (parse_tptp(&problem_text), parse_tptp(&proof_text)) else {
            continue;
        };
        parsed_cases += 1;
        let verdict = verify_strict(&problem, &proof, VerificationLimits::default());
        assert!(
            !matches!(verdict, KernelVerdict::Certified),
            "strict kernel certified committed evil proof `{name}`: {verdict}"
        );
    }
    assert!(
        parsed_cases > 0,
        "evil proof corpus contained no parseable cases"
    );
}
