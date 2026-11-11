use mrs_proof_kernel::verus::{ShadowVerdict, VerusConfig};
use mrs_proof_kernel::{KernelVerdict, VerificationLimits};

#[test]
#[ignore = "requires the pinned compiled Verus artifact; run by formal CI"]
fn loaded_job_with_selected_include_closure_reaches_the_formal_checker() {
    use mrs_proover::load::load_text;
    use std::path::Path;

    let root = std::env::temp_dir().join(format!("mrs-verus-include-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("selected.p"),
        "cnf(left,axiom,p). cnf(right,axiom,~p). cnf(unselected,axiom,q).\n",
    )
    .unwrap();
    std::fs::write(
        root.join("included.p"),
        "%include('selected.p',[left,right]).\n",
    )
    .unwrap();
    let problem = "include('selected.p',[left,right]).\n".to_owned();
    let proof = "% Proof : included.p\n\
        cnf(left,axiom,p,file('included.p',left)).\n\
        cnf(right,axiom,~p,file('included.p',right)).\n\
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[left,right])).\n"
        .to_owned();
    let job = load_text(problem, proof, Path::new(&root)).unwrap();
    let loaded_problem = job.problem.as_ref().unwrap().problem();
    assert!(loaded_problem.includes.is_empty());
    assert!(
        !loaded_problem
            .formulas
            .iter()
            .any(|f| f.name() == "unselected")
    );
    let config = VerusConfig {
        executable: std::env::var_os("MRS_VERUS_TEST_BINARY").unwrap().into(),
        expected_sha256: std::env::var("MRS_VERUS_TEST_SHA256").unwrap(),
        timeout: std::time::Duration::from_secs(10),
    };
    let (kernel, report) = mrs_proover::strict::verify_loaded_job_with_verus_gate(
        &job,
        VerificationLimits::default(),
        &config,
    )
    .unwrap();
    assert_eq!(kernel, KernelVerdict::Certified);
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
    let tampered = format!(
        "{} cnf(unselected,axiom,q).",
        job.problem.as_ref().unwrap().source()
    );
    let tampered = mrs_tptp::parse_tptp(&tampered).unwrap();
    let proof = job.proof.problem();
    let report = mrs_proof_kernel::verus::check_tptp_dag(
        &tampered,
        proof,
        Some("included.p"),
        VerificationLimits::default(),
        &config,
    );
    assert!(
        matches!(report.verdict, ShadowVerdict::Unknown(_)),
        "{report:?}"
    );
    std::fs::remove_dir_all(root).unwrap();
}
