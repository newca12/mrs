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

    // Clause factoring on same-polarity literals under a unifier:
    // a: p(X) | p(a) | q(X)
    // b: ~p(a)
    // c: ~q(a)
    // f1: p(a) | q(a) by factoring([a])
    // r1: q(a) by resolution([f1, b])
    // bot: $false by resolution([r1, c])
    let fact_problem = mrs_tptp::parse_tptp(
        "cnf(a,axiom,p(X)|p(a)|q(X)). cnf(b,axiom,~p(a)). cnf(c,axiom,~q(a)).",
    )
    .unwrap();
    let fact_text = "cnf(a,axiom,p(X)|p(a)|q(X),file('input',a)).
        cnf(b,axiom,~p(a),file('input',b)).
        cnf(c,axiom,~q(a),file('input',c)).
        cnf(f1,plain,p(a)|q(a),inference(factoring,[status(thm)],[a])).
        cnf(r1,plain,q(a),inference(resolution,[status(thm)],[f1,b])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[r1,c])).";
    let fact_proof = mrs_tptp::parse_tptp(fact_text).unwrap();
    let (kernel, report) = verify_with_shadow(
        &fact_problem,
        &fact_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified);
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
    assert_eq!(report.checked_nodes, 6);

    // Equality factoring on positive equality literals under a unifier:
    // a: f(X) = a | f(b) = c | p(X)
    // b: ~(f(b) = a)
    // c: a = c
    // d: ~p(b)
    // ef: f(b) = a | ~(a = c) | p(b) by equality_factoring([a])
    // r1: ~(a = c) | p(b) by resolution([ef, b])
    // r2: p(b) by resolution([c, r1])
    // bot: $false by resolution([r2, d])
    let eq_fact_problem = mrs_tptp::parse_tptp(
        "cnf(a,axiom,f(X)=a|f(b)=c|p(X)). cnf(b,axiom,f(b)!=a). cnf(c,axiom,a=c). cnf(d,axiom,~p(b)).",
    )
    .unwrap();
    let eq_fact_text = "cnf(a,axiom,f(X)=a|f(b)=c|p(X),file('input',a)).
        cnf(b,axiom,f(b)!=a,file('input',b)).
        cnf(c,axiom,a=c,file('input',c)).
        cnf(d,axiom,~p(b),file('input',d)).
        cnf(ef,plain,f(b)=a|a!=c|p(b),inference(equality_factoring,[status(thm)],[a])).
        cnf(r1,plain,a!=c|p(b),inference(resolution,[status(thm)],[ef,b])).
        cnf(r2,plain,p(b),inference(resolution,[status(thm)],[c,r1])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[r2,d])).";
    let eq_fact_proof = mrs_tptp::parse_tptp(eq_fact_text).unwrap();
    let (kernel, report) = verify_with_shadow(
        &eq_fact_problem,
        &eq_fact_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified);
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
    assert_eq!(report.checked_nodes, 8);

    // Subsumption resolution:
    // target (a): ~p(a) | q(a) | r(a)
    // active (b): p(X) | q(X)
    // c: ~q(a)
    // d: ~r(a)
    // sr: q(a) | r(a) by subsumption_resolution([a, b])
    // r1: r(a) by resolution([sr, c])
    // bot: $false by resolution([r1, d])
    let sr_problem = mrs_tptp::parse_tptp(
        "cnf(a,axiom,~p(a)|q(a)|r(a)). cnf(b,axiom,p(X)|q(X)). cnf(c,axiom,~q(a)). cnf(d,axiom,~r(a)).",
    )
    .unwrap();
    let sr_text = "cnf(a,axiom,~p(a)|q(a)|r(a),file('input',a)).
        cnf(b,axiom,p(X)|q(X),file('input',b)).
        cnf(c,axiom,~q(a),file('input',c)).
        cnf(d,axiom,~r(a),file('input',d)).
        cnf(sr,plain,q(a)|r(a),inference(subsumption_resolution,[status(thm)],[a,b])).
        cnf(r1,plain,r(a),inference(resolution,[status(thm)],[sr,c])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[r1,d])).";
    let sr_proof = mrs_tptp::parse_tptp(sr_text).unwrap();
    let (kernel, report) = verify_with_shadow(
        &sr_problem,
        &sr_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified);
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
    assert_eq!(report.checked_nodes, 7);

    // Paramodulation (bidirectional superposition):
    let pm_problem =
        mrs_tptp::parse_tptp("cnf(a,axiom,f(a)=b). cnf(t,axiom,p(f(a))). cnf(n,axiom,~p(b)).")
            .unwrap();
    let pm_text = "cnf(a,axiom,f(a)=b,file('input',a)).
        cnf(t,axiom,p(f(a)),file('input',t)). cnf(n,axiom,~p(b),file('input',n)).
        cnf(s,plain,p(b),inference(paramodulation,[status(thm)],[t,a])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[s,n])).";
    let pm_proof = mrs_tptp::parse_tptp(pm_text).unwrap();
    let (kernel, report) = verify_with_shadow(
        &pm_problem,
        &pm_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified);
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
    assert_eq!(report.checked_nodes, 5);

    // Demodulation (unit rewriting):
    let dm_problem =
        mrs_tptp::parse_tptp("cnf(a,axiom,f(a)=b). cnf(t,axiom,p(f(a))). cnf(n,axiom,~p(b)).")
            .unwrap();
    let dm_text = "cnf(a,axiom,f(a)=b,file('input',a)).
        cnf(t,axiom,p(f(a)),file('input',t)). cnf(n,axiom,~p(b),file('input',n)).
        cnf(s,plain,p(b),inference(demodulation,[status(thm)],[t,a])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[s,n])).";
    let dm_proof = mrs_tptp::parse_tptp(dm_text).unwrap();
    let (kernel, report) = verify_with_shadow(
        &dm_problem,
        &dm_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified);
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
    assert_eq!(report.checked_nodes, 5);

    // Condensation (factoring):
    let cond_problem = mrs_tptp::parse_tptp(
        "cnf(a,axiom,p(X)|q(X)|p(Y)). cnf(b,axiom,~p(a)). cnf(c,axiom,~q(a)).",
    )
    .unwrap();
    let cond_text = "cnf(a,axiom,p(X)|q(X)|p(Y),file('input',a)).
        cnf(b,axiom,~p(a),file('input',b)).
        cnf(c,axiom,~q(a),file('input',c)).
        cnf(f1,plain,p(X)|q(X),inference(condensation,[status(thm)],[a])).
        cnf(r1,plain,q(a),inference(resolution,[status(thm)],[f1,b])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[r1,c])).";
    let cond_proof = mrs_tptp::parse_tptp(cond_text).unwrap();
    let (kernel, report) = verify_with_shadow(
        &cond_problem,
        &cond_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified);
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
    assert_eq!(report.checked_nodes, 6);

    // Destructive equality resolution (equality resolution on reflexive inequality):
    let ineq_problem = mrs_tptp::parse_tptp("cnf(a,axiom,a!=a|p(a)). cnf(b,axiom,~p(a)).").unwrap();
    let ineq_text = "cnf(a,axiom,a!=a|p(a),file('input',a)).
        cnf(b,axiom,~p(a),file('input',b)).
        cnf(s,plain,p(a),inference(destructive_equality_resolution,[status(thm)],[a])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[s,b])).";
    let ineq_proof = mrs_tptp::parse_tptp(ineq_text).unwrap();
    let (kernel, report) = verify_with_shadow(
        &ineq_problem,
        &ineq_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified);
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
    assert_eq!(report.checked_nodes, 4);

    // Clause weakening:
    let wk_problem =
        mrs_tptp::parse_tptp("cnf(a,axiom,p(a)). cnf(b,axiom,~p(a)). cnf(c,axiom,~q(a)).").unwrap();
    let wk_text = "cnf(a,axiom,p(a),file('input',a)).
        cnf(b,axiom,~p(a),file('input',b)).
        cnf(c,axiom,~q(a),file('input',c)).
        cnf(w,plain,p(a)|q(a),inference(weaken,[status(thm)],[a])).
        cnf(r1,plain,q(a),inference(resolution,[status(thm)],[w,b])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[r1,c])).";
    let wk_proof = mrs_tptp::parse_tptp(wk_text).unwrap();
    let (kernel, report) = verify_with_shadow(
        &wk_problem,
        &wk_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified);
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
    assert_eq!(report.checked_nodes, 6);

    // Demodulation:
    let dmod_problem =
        mrs_tptp::parse_tptp("cnf(a,axiom,p(f(a))). cnf(b,axiom,a=b). cnf(c,axiom,~p(f(b))).")
            .unwrap();
    let dmod_text = "cnf(a,axiom,p(f(a)),file('input',a)).
        cnf(b,axiom,a=b,file('input',b)).
        cnf(c,axiom,~p(f(b)),file('input',c)).
        cnf(d,plain,p(f(b)),inference(demodulation,[status(thm)],[a,b])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[d,c])).";
    let dmod_proof = mrs_tptp::parse_tptp(dmod_text).unwrap();
    let (kernel, report) = verify_with_shadow(
        &dmod_problem,
        &dmod_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified);
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
    assert_eq!(report.checked_nodes, 5);

    // Instantiation:
    let inst_problem =
        mrs_tptp::parse_tptp("cnf(a,axiom,p(X)|q(X)). cnf(b,axiom,~p(a)). cnf(c,axiom,~q(a)).")
            .unwrap();
    let inst_text = "cnf(a,axiom,p(X)|q(X),file('input',a)).
        cnf(b,axiom,~p(a),file('input',b)).
        cnf(c,axiom,~q(a),file('input',c)).
        cnf(i,plain,p(a)|q(a),inference(instantiate,[status(thm)],[a])).
        cnf(r1,plain,q(a),inference(resolution,[status(thm)],[i,b])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[r1,c])).";
    let inst_proof = mrs_tptp::parse_tptp(inst_text).unwrap();
    let (kernel, report) = verify_with_shadow(
        &inst_problem,
        &inst_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified);
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
    assert_eq!(report.checked_nodes, 6);

    // Instantiation may rename a universally quantified variable to another
    // variable; the rule is not limited to grounding substitutions.
    let variable_inst_problem =
        mrs_tptp::parse_tptp("cnf(a,axiom,p(X)|q(Z)). cnf(b,axiom,~p(Y)). cnf(c,axiom,~q(Y)).")
            .unwrap();
    let variable_inst_text = "cnf(a,axiom,p(X)|q(Z),file('input',a)).
        cnf(b,axiom,~p(Y),file('input',b)).
        cnf(c,axiom,~q(Y),file('input',c)).
        cnf(i,plain,p(Y)|q(Y),inference(instantiate,[status(thm)],[a])).
        cnf(r1,plain,q(Y),inference(resolution,[status(thm)],[i,b])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[r1,c])).";
    let variable_inst_proof = mrs_tptp::parse_tptp(variable_inst_text).unwrap();
    let (kernel, report) = verify_with_shadow(
        &variable_inst_problem,
        &variable_inst_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified);
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
    assert_eq!(report.checked_nodes, 6);

    // Condensation:
    let cond_problem = mrs_tptp::parse_tptp(
        "cnf(a,axiom,p(X)|q(X)|p(Y)). cnf(b,axiom,~p(a)). cnf(c,axiom,~q(a)).",
    )
    .unwrap();
    let cond_text = "cnf(a,axiom,p(X)|q(X)|p(Y),file('input',a)).
        cnf(b,axiom,~p(a),file('input',b)).
        cnf(c,axiom,~q(a),file('input',c)).
        cnf(cond,plain,p(X)|q(X),inference(condensation,[status(thm)],[a])).
        cnf(r1,plain,q(a),inference(resolution,[status(thm)],[cond,b])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[r1,c])).";
    let cond_proof = mrs_tptp::parse_tptp(cond_text).unwrap();
    let (kernel, report) = verify_with_shadow(
        &cond_problem,
        &cond_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified);
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
    assert_eq!(report.checked_nodes, 6);

    // Duplicate literal removal via condensation:
    let dup_problem = mrs_tptp::parse_tptp("cnf(a,axiom,p(a)|p(a)). cnf(b,axiom,~p(a)).").unwrap();
    let dup_text = "cnf(a,axiom,p(a)|p(a),file('input',a)).
        cnf(b,axiom,~p(a),file('input',b)).
        cnf(cond,plain,p(a),inference(condensation,[status(thm)],[a])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[cond,b])).";
    let dup_proof = mrs_tptp::parse_tptp(dup_text).unwrap();
    let (kernel, report) = verify_with_shadow(
        &dup_problem,
        &dup_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified);
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
    assert_eq!(report.checked_nodes, 4);

    // Reflexivity:
    let refl_problem = mrs_tptp::parse_tptp("cnf(a,axiom,p(a)). cnf(b,axiom,a!=a|~p(a)).").unwrap();
    let refl_text = "cnf(a,axiom,p(a),file('input',a)).
        cnf(b,axiom,a!=a|~p(a),file('input',b)).
        cnf(r,plain,a=a,inference(reflexivity,[status(thm)],[a])).
        cnf(m1,plain,~p(a),inference(resolution,[status(thm)],[r,b])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[m1,a])).";
    let refl_proof = mrs_tptp::parse_tptp(refl_text).unwrap();
    let (kernel, report) = verify_with_shadow(
        &refl_problem,
        &refl_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified);
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
    assert_eq!(report.checked_nodes, 5);

    // Transitivity:
    let trans_problem =
        mrs_tptp::parse_tptp("cnf(a,axiom,a=b). cnf(b,axiom,b=c). cnf(c,axiom,a!=c).").unwrap();
    let trans_text = "cnf(a,axiom,a=b,file('input',a)).
        cnf(b,axiom,b=c,file('input',b)).
        cnf(c,axiom,a!=c,file('input',c)).
        cnf(t,plain,a=c,inference(transitivity,[status(thm)],[a,b])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[t,c])).";
    let trans_proof = mrs_tptp::parse_tptp(trans_text).unwrap();
    let (kernel, report) = verify_with_shadow(
        &trans_problem,
        &trans_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified);
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
    assert_eq!(report.checked_nodes, 5);

    // Symmetry:
    let symm_problem = mrs_tptp::parse_tptp("cnf(a,axiom,a=b). cnf(b,axiom,b!=a).").unwrap();
    let symm_text = "cnf(a,axiom,a=b,file('input',a)).
        cnf(b,axiom,b!=a,file('input',b)).
        cnf(s,plain,b=a,inference(symmetry,[status(thm)],[a])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[s,b])).";
    let symm_proof = mrs_tptp::parse_tptp(symm_text).unwrap();
    let (kernel, report) = verify_with_shadow(
        &symm_problem,
        &symm_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified);
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
    assert_eq!(report.checked_nodes, 4);

    // Multi-literal clause symmetry:
    let multi_symm_problem =
        mrs_tptp::parse_tptp("cnf(a,axiom,p(c)|a=b). cnf(b,axiom,~p(c)). cnf(c,axiom,b!=a).")
            .unwrap();
    let multi_symm_text = "cnf(a,axiom,p(c)|a=b,file('input',a)).
        cnf(b,axiom,~p(c),file('input',b)).
        cnf(c,axiom,b!=a,file('input',c)).
        cnf(s,plain,p(c)|b=a,inference(symmetry,[status(thm)],[a])).
        cnf(r1,plain,b=a,inference(resolution,[status(thm)],[s,b])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[r1,c])).";
    let multi_symm_proof = mrs_tptp::parse_tptp(multi_symm_text).unwrap();
    let (kernel, report) = verify_with_verus_gate(
        &multi_symm_problem,
        &multi_symm_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified, "{report:?}");
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");

    // Multi-literal clause congruence:
    let multi_cong_problem =
        mrs_tptp::parse_tptp("cnf(a,axiom,p(c)|a=b). cnf(b,axiom,~p(c)). cnf(c,axiom,f(a)!=f(b)).")
            .unwrap();
    let multi_cong_text = "cnf(a,axiom,p(c)|a=b,file('input',a)).
        cnf(b,axiom,~p(c),file('input',b)).
        cnf(c,axiom,f(a)!=f(b),file('input',c)).
        cnf(cg,plain,p(c)|f(a)=f(b),inference(congruence,[status(thm)],[a])).
        cnf(r1,plain,f(a)=f(b),inference(resolution,[status(thm)],[cg,b])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[r1,c])).";
    let multi_cong_proof = mrs_tptp::parse_tptp(multi_cong_text).unwrap();
    let (kernel, report) = verify_with_verus_gate(
        &multi_cong_problem,
        &multi_cong_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified, "{report:?}");
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
    let rename_problem = mrs_tptp::parse_tptp("cnf(a,axiom,p(X)). cnf(b,axiom,~p(Y)).").unwrap();
    let rename_text = "cnf(a,axiom,p(X),file('input',a)).
        cnf(b,axiom,~p(Y),file('input',b)).
        cnf(r,plain,p(Z),inference(copy,[status(thm)],[a])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[r,b])).";
    let rename_proof = mrs_tptp::parse_tptp(rename_text).unwrap();
    let (kernel, report) = verify_with_shadow(
        &rename_problem,
        &rename_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified);
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
    assert_eq!(report.checked_nodes, 4);

    // Disjunctive syllogism via resolution:
    let ds_problem =
        mrs_tptp::parse_tptp("cnf(a,axiom,p|q). cnf(b,axiom,~p). cnf(c,axiom,~q).").unwrap();
    let ds_text = "cnf(a,axiom,p|q,file('input',a)).
        cnf(b,axiom,~p,file('input',b)).
        cnf(c,axiom,~q,file('input',c)).
        cnf(d,plain,q,inference(disjunctive_syllogism,[status(thm)],[a,b])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[d,c])).";
    let ds_proof = mrs_tptp::parse_tptp(ds_text).unwrap();
    let (kernel, report) = verify_with_shadow(
        &ds_problem,
        &ds_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified);
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
    assert_eq!(report.checked_nodes, 5);

    // Excluded middle:
    let em_problem = mrs_tptp::parse_tptp("cnf(a,axiom,p). cnf(b,axiom,~p).").unwrap();
    let em_text = "cnf(a,axiom,p,file('input',a)).
        cnf(b,axiom,~p,file('input',b)).
        cnf(em,plain,p|~p,inference(excluded_middle,[status(thm)],[a])).
        cnf(m1,plain,~p,inference(resolution,[status(thm)],[em,b])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[a,m1])).";
    let em_proof = mrs_tptp::parse_tptp(em_text).unwrap();
    let (kernel, report) = verify_with_shadow(
        &em_problem,
        &em_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified);
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
    assert_eq!(report.checked_nodes, 5);

    // Tautology alias:
    let tau_problem = mrs_tptp::parse_tptp("cnf(a,axiom,p). cnf(b,axiom,~p).").unwrap();
    let tau_text = "cnf(a,axiom,p,file('input',a)).
        cnf(b,axiom,~p,file('input',b)).
        cnf(tau,plain,p|~p,inference(tautology,[status(thm)],[a])).
        cnf(m1,plain,~p,inference(resolution,[status(thm)],[tau,b])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[a,m1])).";
    let tau_proof = mrs_tptp::parse_tptp(tau_text).unwrap();
    let (_, report) = verify_with_shadow(
        &tau_problem,
        &tau_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");

    // Resolve and consequence aliases:
    let res_alias_problem =
        mrs_tptp::parse_tptp("cnf(a,axiom,p|q). cnf(b,axiom,~p). cnf(c,axiom,~q).").unwrap();
    let res_alias_text = "cnf(a,axiom,p|q,file('input',a)).
        cnf(b,axiom,~p,file('input',b)).
        cnf(c,axiom,~q,file('input',c)).
        cnf(d,plain,q,inference(resolve,[status(thm)],[a,b])).
        cnf(bot,plain,$false,inference(consequence,[status(thm)],[d,c])).";
    let res_alias_proof = mrs_tptp::parse_tptp(res_alias_text).unwrap();
    let (kernel, report) = verify_with_shadow(
        &res_alias_problem,
        &res_alias_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified);
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
    assert_eq!(report.checked_nodes, 5);

    // Congruence:
    let cong_problem = mrs_tptp::parse_tptp("cnf(a,axiom,a=b). cnf(b,axiom,f(a)!=f(b)).").unwrap();
    let cong_text = "cnf(a,axiom,a=b,file('input',a)).
        cnf(b,axiom,f(a)!=f(b),file('input',b)).
        cnf(c,plain,f(a)=f(b),inference(congruence,[status(thm)],[a])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[c,b])).";
    let cong_proof = mrs_tptp::parse_tptp(cong_text).unwrap();
    let (kernel, report) = verify_with_shadow(
        &cong_problem,
        &cong_proof,
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
fn nested_congruence_refutations_and_deep_mutations() {
    let cfg = config();
    for (parent, left, right) in [
        ("a=b", "h(g(c,f(a)),d)", "h(g(c,f(b)),d)"),
        ("b=a", "h(g(c,f(a)),d)", "h(g(c,f(b)),d)"),
        ("a=b", "h(g(c,f(b)),d)", "h(g(c,f(a)),d)"),
        ("f(X)=g(X)", "h(k(f(X),c))", "h(k(g(X),c))"),
    ] {
        let problem_text = format!("cnf(a,axiom,{parent}). cnf(b,axiom,{left}!={right}).");
        let problem = mrs_tptp::parse_tptp(&problem_text).unwrap();
        for rule in ["congruence", "equality_congruence"] {
            let text = format!(
                "cnf(a,axiom,{parent},file('input',a)).
                 cnf(b,axiom,{left}!={right},file('input',b)).
                 cnf(c,plain,{left}={right},inference({rule},[status(thm)],[a])).
                 cnf(bot,plain,$false,inference(resolution,[status(thm)],[c,b]))."
            );
            let proof = mrs_tptp::parse_tptp(&text).unwrap();
            let (kernel, report) = verify_with_verus_gate(
                &problem,
                &proof,
                Some("input"),
                VerificationLimits::default(),
                &cfg,
            );
            assert_eq!(kernel, KernelVerdict::Certified, "{report:?}");
            assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
        }
    }

    for right in [
        "h(g(c,f(e)),d)", // Deep replacement not justified by a=b.
        "h(g(c,k(b)),d)", // Deep function symbol mismatch.
        "h(g(e,f(b)),d)", // Inner sibling changed.
        "h(g(c,f(b)),e)", // Outer sibling changed.
        "h(g(b,f(b)),d)", // Two holes require separate replay steps.
    ] {
        let problem_text = format!("cnf(a,axiom,a=b). cnf(b,axiom,h(g(c,f(a)),d)!={right}).");
        let problem = mrs_tptp::parse_tptp(&problem_text).unwrap();
        let text = format!(
            "cnf(a,axiom,a=b,file('input',a)).
             cnf(b,axiom,h(g(c,f(a)),d)!={right},file('input',b)).
             cnf(c,plain,h(g(c,f(a)),d)={right},inference(congruence,[status(thm)],[a])).
             cnf(bot,plain,$false,inference(resolution,[status(thm)],[c,b]))."
        );
        let proof = mrs_tptp::parse_tptp(&text).unwrap();
        let (kernel, report) = verify_with_verus_gate(
            &problem,
            &proof,
            Some("input"),
            VerificationLimits::default(),
            &cfg,
        );
        assert_ne!(kernel, KernelVerdict::Certified);
        assert_ne!(report.verdict, ShadowVerdict::CertifiedDagIr);
    }
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

#[test]
#[ignore = "requires the pinned compiled Verus artifact; run by formal CI"]
fn clause_tautology_refutations_and_mutations() {
    let cfg = config();
    for rule in ["tautology", "clause_tautology"] {
        // Complementary pair tautology:
        let tau_problem = mrs_tptp::parse_tptp("cnf(a,axiom,p). cnf(b,axiom,~p).").unwrap();
        let tau_text = format!(
            "cnf(a,axiom,p,file('input',a)).
             cnf(b,axiom,~p,file('input',b)).
             cnf(tau,plain,p|~p,inference({rule},[status(thm)],[a])).
             cnf(m1,plain,~p,inference(resolution,[status(thm)],[tau,b])).
             cnf(bot,plain,$false,inference(resolution,[status(thm)],[a,m1]))."
        );
        let tau_proof = mrs_tptp::parse_tptp(&tau_text).unwrap();
        let (kernel, report) = verify_with_verus_gate(
            &tau_problem,
            &tau_proof,
            Some("input"),
            VerificationLimits::default(),
            &cfg,
        );
        assert_eq!(kernel, KernelVerdict::Certified, "{report:?}");
        assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");

        // Multi-literal tautology with reflexive equality:
        let eq_problem =
            mrs_tptp::parse_tptp("cnf(a,axiom,p). cnf(b,axiom,~p). cnf(c,axiom,a!=a).").unwrap();
        let eq_text = format!(
            "cnf(a,axiom,p,file('input',a)).
             cnf(b,axiom,~p,file('input',b)).
             cnf(c,axiom,a!=a,file('input',c)).
             cnf(tau,plain,a=a|p,inference({rule},[status(thm)],[a])).
             cnf(res1,plain,p,inference(resolution,[status(thm)],[tau,c])).
             cnf(bot,plain,$false,inference(resolution,[status(thm)],[res1,b]))."
        );
        let eq_proof = mrs_tptp::parse_tptp(&eq_text).unwrap();
        let (kernel, report) = verify_with_verus_gate(
            &eq_problem,
            &eq_proof,
            Some("input"),
            VerificationLimits::default(),
            &cfg,
        );
        assert_eq!(kernel, KernelVerdict::Certified, "{report:?}");
        assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
    }

    // Fail-closed mutations: non-tautology clauses
    for non_tau in ["p|q", "a=b", "p|p"] {
        let problem = mrs_tptp::parse_tptp("cnf(a,axiom,p). cnf(b,axiom,~p).").unwrap();
        let text = format!(
            "cnf(a,axiom,p,file('input',a)).
             cnf(b,axiom,~p,file('input',b)).
             cnf(tau,plain,{non_tau},inference(tautology,[status(thm)],[a])).
             cnf(bot,plain,$false,inference(resolution,[status(thm)],[a,b]))."
        );
        let proof = mrs_tptp::parse_tptp(&text).unwrap();
        let (kernel, report) = verify_with_verus_gate(
            &problem,
            &proof,
            Some("input"),
            VerificationLimits::default(),
            &cfg,
        );
        assert_ne!(kernel, KernelVerdict::Certified);
        assert_ne!(report.verdict, ShadowVerdict::CertifiedDagIr);
    }
}

#[test]
#[ignore = "requires the pinned compiled Verus artifact; run by formal CI"]
fn multi_step_and_multi_parent_demodulation_refutations() {
    let cfg = config();

    // 1. Multi-step demodulation with single equality parent rewriting 2 positions:
    let multi_step_problem = mrs_tptp::parse_tptp(
        "cnf(a,axiom,f(a)=b). cnf(t,axiom,p(f(a),f(a))). cnf(n,axiom,~p(b,b)).",
    )
    .unwrap();
    let multi_step_text = "cnf(a,axiom,f(a)=b,file('input',a)).
        cnf(t,axiom,p(f(a),f(a)),file('input',t)).
        cnf(n,axiom,~p(b,b),file('input',n)).
        cnf(d,plain,p(b,b),inference(demodulation,[status(thm)],[t,a])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[d,n])).";
    let multi_step_proof = mrs_tptp::parse_tptp(multi_step_text).unwrap();
    let (kernel, report) = verify_with_verus_gate(
        &multi_step_problem,
        &multi_step_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified, "{report:?}");
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");

    // 2. Multi-parent demodulation with 2 equality parents (3 parents total):
    let multi_parent_problem =
        mrs_tptp::parse_tptp("cnf(a,axiom,f(a)=b). cnf(c,axiom,g(c)=d). cnf(t,axiom,p(f(a),g(c))). cnf(n,axiom,~p(b,d)).").unwrap();
    let multi_parent_text = "cnf(a,axiom,f(a)=b,file('input',a)).
        cnf(c,axiom,g(c)=d,file('input',c)).
        cnf(t,axiom,p(f(a),g(c)),file('input',t)).
        cnf(n,axiom,~p(b,d),file('input',n)).
        cnf(d,plain,p(b,d),inference(demodulation,[status(thm)],[t,a,c])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[d,n])).";
    let multi_parent_proof = mrs_tptp::parse_tptp(multi_parent_text).unwrap();
    let (kernel, report) = verify_with_verus_gate(
        &multi_parent_problem,
        &multi_parent_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified, "{report:?}");
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");

    // 3. Equality normalization with multiple equality parents:
    let eq_norm_problem = mrs_tptp::parse_tptp(
        "cnf(a,axiom,a=b). cnf(c,axiom,c=d). cnf(t,axiom,p(a,c)). cnf(n,axiom,~p(b,d)).",
    )
    .unwrap();
    let eq_norm_text = "cnf(a,axiom,a=b,file('input',a)).
        cnf(c,axiom,c=d,file('input',c)).
        cnf(t,axiom,p(a,c),file('input',t)).
        cnf(n,axiom,~p(b,d),file('input',n)).
        cnf(d,plain,p(b,d),inference(equality_normalization,[status(thm)],[t,a,c])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[d,n])).";
    let eq_norm_proof = mrs_tptp::parse_tptp(eq_norm_text).unwrap();
    let (kernel, report) = verify_with_verus_gate(
        &eq_norm_problem,
        &eq_norm_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified, "{report:?}");
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");

    // 4. Equality normalization to empty clause via demodulation + equality resolution:
    let eq_norm_empty_problem =
        mrs_tptp::parse_tptp("cnf(ab,axiom,a=b). cnf(neq,axiom,a!=b).").unwrap();
    let eq_norm_empty_text = "cnf(ab,axiom,a=b,file('input',ab)).
        cnf(neq,axiom,a!=b,file('input',neq)).
        cnf(bot,plain,$false,inference(equality_normalization,[status(thm)],[neq,ab])).";
    let eq_norm_empty_proof = mrs_tptp::parse_tptp(eq_norm_empty_text).unwrap();
    let (kernel, report) = verify_with_verus_gate(
        &eq_norm_empty_problem,
        &eq_norm_empty_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified, "{report:?}");
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");

    // 5. Ex falso and disjunctive syllogism rule aliases routed to resolution:
    let mp_problem =
        mrs_tptp::parse_tptp("cnf(a,axiom,p|q). cnf(b,axiom,~p). cnf(c,axiom,~q).").unwrap();
    let mp_text = "cnf(a,axiom,p|q,file('input',a)).
        cnf(b,axiom,~p,file('input',b)).
        cnf(c,axiom,~q,file('input',c)).
        cnf(d,plain,q,inference(disjunctive_syllogism,[status(thm)],[a,b])).
        cnf(bot,plain,$false,inference(ex_falso,[status(thm)],[d,c])).";
    let mp_proof = mrs_tptp::parse_tptp(mp_text).unwrap();
    let (kernel, report) = verify_with_verus_gate(
        &mp_problem,
        &mp_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified, "{report:?}");
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");

    // 6. Multi-parent consequence with redundant parent:
    let conseq_problem = mrs_tptp::parse_tptp(
        "cnf(a,axiom,p|q). cnf(b,axiom,~p). cnf(c,axiom,r). cnf(d,axiom,~q|~r).",
    )
    .unwrap();
    let conseq_text = "cnf(a,axiom,p|q,file('input',a)).
        cnf(b,axiom,~p,file('input',b)).
        cnf(c,axiom,r,file('input',c)).
        cnf(d,axiom,~q|~r,file('input',d)).
        cnf(q_clause,plain,q,inference(consequence,[status(thm)],[a,b,c])).
        cnf(d2,plain,~r,inference(resolution,[status(thm)],[q_clause,d])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[d2,c])).";
    let conseq_proof = mrs_tptp::parse_tptp(conseq_text).unwrap();
    let (kernel, report) = verify_with_verus_gate(
        &conseq_problem,
        &conseq_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified, "{report:?}");
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");

    // 7. cnf_transformation via instantiation:
    let cnf_problem = mrs_tptp::parse_tptp("cnf(a,axiom,p(X)). cnf(b,axiom,~p(c)).").unwrap();
    let cnf_text = "cnf(a,axiom,p(X),file('input',a)).
        cnf(b,axiom,~p(c),file('input',b)).
        cnf(c1,plain,p(X),inference(cnf_transformation,[status(thm)],[a])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[c1,b])).";
    let cnf_proof = mrs_tptp::parse_tptp(cnf_text).unwrap();
    let (kernel, report) = verify_with_verus_gate(
        &cnf_problem,
        &cnf_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified, "{report:?}");
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");

    // 8. Multi-literal clause transitivity:
    let multi_trans_problem = mrs_tptp::parse_tptp(
        "cnf(a,axiom,p(c)|a=b). cnf(b,axiom,b=d). cnf(c,axiom,~p(c)). cnf(d,axiom,a!=d).",
    )
    .unwrap();
    let multi_trans_text = "cnf(a,axiom,p(c)|a=b,file('input',a)).
        cnf(b,axiom,b=d,file('input',b)).
        cnf(c,axiom,~p(c),file('input',c)).
        cnf(d,axiom,a!=d,file('input',d)).
        cnf(tr,plain,p(c)|a=d,inference(transitivity,[status(thm)],[a,b])).
        cnf(r1,plain,a=d,inference(resolution,[status(thm)],[tr,c])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[r1,d])).";
    let multi_trans_proof = mrs_tptp::parse_tptp(multi_trans_text).unwrap();
    let (kernel, report) = verify_with_verus_gate(
        &multi_trans_problem,
        &multi_trans_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified, "{report:?}");
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");

    // 9. Multi-parent transitivity chain (3 parents: a=b, b=c, c=d -> a=d):
    let chain_trans_problem = mrs_tptp::parse_tptp(
        "cnf(a,axiom,a=b). cnf(b,axiom,b=c). cnf(c,axiom,c=d). cnf(d,axiom,a!=d).",
    )
    .unwrap();
    let chain_trans_text = "cnf(a,axiom,a=b,file('input',a)).
        cnf(b,axiom,b=c,file('input',b)).
        cnf(c,axiom,c=d,file('input',c)).
        cnf(d,axiom,a!=d,file('input',d)).
        cnf(tr,plain,a=d,inference(transitivity,[status(thm)],[a,b,c])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[tr,d])).";
    let chain_trans_proof = mrs_tptp::parse_tptp(chain_trans_text).unwrap();
    let (kernel, report) = verify_with_verus_gate(
        &chain_trans_problem,
        &chain_trans_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified, "{report:?}");
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");

    // 10. Zero-parent reflexivity inference:
    let zero_refl_problem =
        mrs_tptp::parse_tptp("cnf(a,axiom,p(a)). cnf(b,axiom,a!=a|~p(a)).").unwrap();
    let zero_refl_text = "cnf(a,axiom,p(a),file('input',a)).
        cnf(b,axiom,a!=a|~p(a),file('input',b)).
        cnf(r,plain,a=a,inference(reflexivity,[status(thm)],[])).
        cnf(m1,plain,~p(a),inference(resolution,[status(thm)],[r,b])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[m1,a])).";
    let zero_refl_proof = mrs_tptp::parse_tptp(zero_refl_text).unwrap();
    let (_, report) = verify_with_shadow(
        &zero_refl_problem,
        &zero_refl_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");

    // 11. Zero-parent excluded middle inference:
    let zero_em_problem = mrs_tptp::parse_tptp("cnf(a,axiom,p). cnf(b,axiom,~p).").unwrap();
    let zero_em_text = "cnf(a,axiom,p,file('input',a)).
        cnf(b,axiom,~p,file('input',b)).
        cnf(em,plain,p|~p,inference(excluded_middle,[status(thm)],[])).
        cnf(m1,plain,~p,inference(resolution,[status(thm)],[em,b])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[a,m1])).";
    let zero_em_proof = mrs_tptp::parse_tptp(zero_em_text).unwrap();
    let (_, report) = verify_with_shadow(
        &zero_em_problem,
        &zero_em_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
}
