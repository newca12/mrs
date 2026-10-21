//! Executed against the artifact built by the formal verification job.
use mrs_proof_kernel::verus::{
    ShadowVerdict, VerusConfig, check_tptp_step, verify_with_shadow, verify_with_verus_gate,
};
use mrs_proof_kernel::{KernelVerdict, VerificationLimits};
use std::time::Duration;

fn config() -> VerusConfig {
    VerusConfig {
        executable: std::env::var_os("MRS_VERUS_TEST_BINARY")
            .expect("compiled Verus binary")
            .into(),
        expected_sha256: std::env::var("MRS_VERUS_TEST_SHA256").expect("artifact SHA-256"),
        timeout: Duration::from_secs(10),
    }
}

#[test]
#[ignore = "requires the pinned compiled Verus artifact; run by formal CI"]
fn archived_step_and_real_tptp_dags_traverse_compiled_checker() {
    let cfg = config();
    let fixture =
        include_str!("../../../formal/superposition-checker/fixtures/SWV406+1-c502-clauses.p");
    let report = check_tptp_step(fixture, "c27", "c39", "c502_candidate", &cfg);
    assert_eq!(report.verdict, ShadowVerdict::AcceptedStepIr, "{report:?}");
    assert!(!report.production_certification);
    let forged = fixture.replace("X2 = X5", "X2 = bogus");
    assert!(matches!(
        check_tptp_step(&forged, "c27", "c39", "c502_candidate", &cfg).verdict,
        ShadowVerdict::Unknown(_)
    ));

    let problem =
        mrs_tptp::parse_tptp("cnf(a,axiom,p|q). cnf(b,axiom,~p). cnf(c,axiom,~q).").unwrap();
    let text = "cnf(a,axiom,p|q,file('input',a)). cnf(b,axiom,~p,file('input',b)).
        cnf(c,axiom,~q,file('input',c)).
        cnf(d,plain,q,inference(resolution,[status(thm)],[a,b])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[d,c])).";
    let proof = mrs_tptp::parse_tptp(text).unwrap();
    let (kernel, shadow) = verify_with_shadow(
        &problem,
        &proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified);
    assert_eq!(shadow.verdict, ShadowVerdict::CertifiedDagIr, "{shadow:?}");
    assert_eq!(shadow.checked_nodes, 5);
    let repeated = verify_with_shadow(
        &problem,
        &proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    )
    .1;
    assert_eq!(shadow.request_sha256, repeated.request_sha256);
    assert_eq!(shadow.artifact_sha256, repeated.artifact_sha256);
    // A complete proof where the superposition conclusion is essential, then
    // exact-atom resolution closes the refutation. No derived clause is an input.
    let sp_problem =
        mrs_tptp::parse_tptp("cnf(a,axiom,f(a)=b). cnf(t,axiom,p(f(a))). cnf(n,axiom,~p(b)).")
            .unwrap();
    let sp_text = "cnf(a,axiom,f(a)=b,file('input',a)).
        cnf(t,axiom,p(f(a)),file('input',t)). cnf(n,axiom,~p(b),file('input',n)).
        cnf(s,plain,p(b),inference(superposition,[status(thm)],[a,t])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[s,n])).";
    let sp_proof = mrs_tptp::parse_tptp(sp_text).unwrap();
    let (kernel, report) = verify_with_shadow(
        &sp_problem,
        &sp_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified);
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
    assert_eq!(
        verify_with_verus_gate(
            &sp_problem,
            &sp_proof,
            Some("input"),
            VerificationLimits::default(),
            &cfg
        )
        .0,
        KernelVerdict::Certified
    );
    for altered in [
        sp_text.replace("[s,n]", "[t,n]"),
        sp_text.replace("plain,p(b)", "plain,p(a)"),
        sp_text.replace("[a,t]", "[inference(copy,[status(thm)],[a]),t]"),
    ] {
        let proof = mrs_tptp::parse_tptp(&altered).unwrap();
        let (verdict, _) = verify_with_verus_gate(
            &sp_problem,
            &proof,
            Some("input"),
            VerificationLimits::default(),
            &cfg,
        );
        assert_ne!(verdict, KernelVerdict::Certified);
    }
    // Equality and binary-predicate exact pivots use the same proved rule.
    for (atom, negative) in [
        ("a=b", "a!=b"),
        ("p(a,b)", "~p(a,b)"),
        ("p(X,X)", "~p(X,X)"),
    ] {
        let ptext = format!("cnf(a,axiom,{atom}). cnf(b,axiom,{negative}).");
        let qtext = format!(
            "cnf(a,axiom,{atom},file('input',a)). cnf(b,axiom,{negative},file('input',b)).
            cnf(bot,plain,$false,inference(resolution,[status(thm)],[a,b]))."
        );
        let (p, q) = (
            mrs_tptp::parse_tptp(&ptext).unwrap(),
            mrs_tptp::parse_tptp(&qtext).unwrap(),
        );
        let (_, report) =
            verify_with_shadow(&p, &q, Some("input"), VerificationLimits::default(), &cfg);
        assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
    }
    for mutation in [
        text.replace("[d,c]", "[a,c]"),
        text.replace("[d,c]", "[bot,c]"),
        text.replace("plain,q,inference", "plain,p,inference"),
        text.replace("resolution", "untrusted_rule"),
    ] {
        let proof = mrs_tptp::parse_tptp(&mutation).unwrap();
        let (_, report) = verify_with_shadow(
            &problem,
            &proof,
            Some("input"),
            VerificationLimits::default(),
            &cfg,
        );
        assert_ne!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
    }
    let disconnected = format!("{text} cnf(extra,axiom,p|q,file('input',a)).");
    let proof = mrs_tptp::parse_tptp(&disconnected).unwrap();
    let (_, report) = verify_with_shadow(
        &problem,
        &proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_ne!(report.verdict, ShadowVerdict::CertifiedDagIr);

    // First-order resolution with variable unification across standardized parents:
    // a: p(X) | q(X)
    // b: ~p(a)
    // c: ~q(a)
    // r1: q(a) by resolution([a, b])
    // bot: $false by resolution([r1, c])
    let fo_problem =
        mrs_tptp::parse_tptp("cnf(a,axiom,p(X)|q(X)). cnf(b,axiom,~p(a)). cnf(c,axiom,~q(a)).")
            .unwrap();
    let fo_text = "cnf(a,axiom,p(X)|q(X),file('input',a)).
        cnf(b,axiom,~p(a),file('input',b)).
        cnf(c,axiom,~q(a),file('input',c)).
        cnf(r1,plain,q(a),inference(resolution,[status(thm)],[a,b])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[r1,c])).";
    let fo_proof = mrs_tptp::parse_tptp(fo_text).unwrap();
    let (kernel, report) = verify_with_shadow(
        &fo_problem,
        &fo_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified);
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
    assert_eq!(report.checked_nodes, 5);

    // Equality resolution (reflexivity resolution) on a negative equality literal:
    // a: X != a | p(X)
    // b: ~p(a)
    // r1: p(a) by equality_resolution([a])
    // bot: $false by resolution([r1, b])
    let eq_problem = mrs_tptp::parse_tptp("cnf(a,axiom,X!=a|p(X)). cnf(b,axiom,~p(a)).").unwrap();
    let eq_text = "cnf(a,axiom,X!=a|p(X),file('input',a)).
        cnf(b,axiom,~p(a),file('input',b)).
        cnf(r1,plain,p(a),inference(equality_resolution,[status(thm)],[a])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[r1,b])).";
    let eq_proof = mrs_tptp::parse_tptp(eq_text).unwrap();
    let (kernel, report) = verify_with_shadow(
        &eq_problem,
        &eq_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified);
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
    assert_eq!(report.checked_nodes, 4);

    let mut bad = cfg.clone();
    bad.expected_sha256 = "0".repeat(64);
    assert!(matches!(
        check_tptp_step(fixture, "c27", "c39", "c502_candidate", &bad).verdict,
        ShadowVerdict::Unknown(_)
    ));
}

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
        timeout: Duration::from_secs(10),
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

#[test]
fn unsupported_and_missing_checker_never_upgrade_kernel() {
    let problem = mrs_tptp::parse_tptp("cnf(a,axiom,p). cnf(b,axiom,~p).").unwrap();
    let proof = mrs_tptp::parse_tptp(
        "cnf(a,axiom,p,file('input',a)). cnf(b,axiom,~p,file('input',b)).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[a,b])).",
    )
    .unwrap();
    let cfg = VerusConfig {
        executable: "/nonexistent/mrs-verus".into(),
        expected_sha256: "0".repeat(64),
        timeout: Duration::from_millis(100),
    };
    let (kernel, shadow) = verify_with_shadow(
        &problem,
        &proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified);
    assert!(matches!(shadow.verdict, ShadowVerdict::Unknown(_)));
    assert!(matches!(
        verify_with_verus_gate(
            &problem,
            &proof,
            Some("input"),
            VerificationLimits::default(),
            &cfg
        )
        .0,
        KernelVerdict::Inconclusive(_)
    ));
}

#[test]
fn unsupported_tptp_constructs_are_not_erased_by_lowering() {
    let cfg = VerusConfig {
        executable: "/nonexistent/mrs-verus".into(),
        expected_sha256: "0".repeat(64),
        timeout: Duration::from_millis(100),
    };
    for atom in [
        "p(a,b,c)",
        "p($sum(a,b))",
        "p(1)",
        "p(\"object\")",
        "p|$false",
    ] {
        let text = format!("cnf(a,axiom,f(a)=b). cnf(t,axiom,{atom}). cnf(c,plain,p(b)).");
        let report = check_tptp_step(&text, "a", "t", "c", &cfg);
        assert!(matches!(report.verdict, ShadowVerdict::Unknown(_)));
        assert_eq!(
            report.request_sha256, None,
            "must decline before launching decoder"
        );
    }
}
