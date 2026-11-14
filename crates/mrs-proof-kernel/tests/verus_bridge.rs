//! Executed against the artifact built by the formal verification job.
use mrs_proof_kernel::verus::{
    ShadowVerdict, VerusConfig, check_tptp_dag, check_tptp_step, verify_with_shadow,
    verify_with_verus_gate,
};
use mrs_proof_kernel::{KernelVerdict, VerificationLimits, verify_strict_with_source};
use sha2::{Digest, Sha256};
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

fn first_input_name(tptp: &str) -> Option<&str> {
    tptp.lines().find_map(|line| {
        let declaration = line
            .trim()
            .strip_prefix("cnf(")
            .or_else(|| line.trim().strip_prefix("fof("))?;
        let (name, rest) = declaration.split_once(',')?;
        let (role, _) = rest.split_once(',')?;
        matches!(role.trim(), "axiom" | "negated_conjecture").then_some(name.trim())
    })
}

fn is_empty_cnf_clause(formula: &mrs_tptp::AnnotatedFormula<'_>) -> bool {
    fn empty_body(formula: &mrs_tptp::CNFFormula<'_>) -> bool {
        match formula {
            mrs_tptp::CNFFormula::Parens(inner) => empty_body(inner),
            mrs_tptp::CNFFormula::Disjunction(literals) => {
                literals.len() == 1
                    && matches!(
                        literals.first(),
                        Some(mrs_tptp::CNFLiteral::Positive(
                            mrs_tptp::CNFAtomicFormula::False
                        ))
                    )
            }
        }
    }

    formula.as_cnf().is_some_and(|cnf| match &cnf.formula {
        mrs_tptp::CNFStatement::Logical(body) => empty_body(body),
    })
}

fn first_cited_parent_range(tptp: &str) -> Option<(usize, usize)> {
    let inference_start = tptp.find("inference(")?;
    let status_close = inference_start + tptp[inference_start..].find(']')?;
    let parents_open = status_close + 1 + tptp[status_close + 1..].find('[')?;
    let first_parent = parents_open + 1;
    let first_parent = first_parent
        + tptp[first_parent..]
            .char_indices()
            .find(|(_, character)| !character.is_whitespace())?
            .0;
    let parent_end = first_parent + tptp[first_parent..].find([',', ']'])?;
    (first_parent < parent_end).then_some((first_parent, parent_end))
}

fn replace_first_cited_parent(tptp: &str, replacement: &str) -> String {
    let (start, end) = first_cited_parent_range(tptp).expect("first inference has a cited parent");
    format!("{}{}{}", &tptp[..start], replacement, &tptp[end..])
}

fn replace_first_inference_rule(tptp: &str, replacement: &str) -> String {
    let rule_start = tptp
        .find("inference(")
        .map(|start| start + "inference(".len())
        .expect("proof has an inference");
    let rule_end = rule_start
        + tptp[rule_start..]
            .find([',', ')'])
            .expect("inference has a rule");
    format!(
        "{}{}{}",
        &tptp[..rule_start],
        replacement,
        &tptp[rule_end..]
    )
}

#[test]
fn archived_prover9_syn343_proof_certifies_in_the_strict_kernel() {
    let problem_text = include_str!("../../../formal/superposition-checker/fixtures/SYN343-1.p");
    let proof_text =
        include_str!("../../../formal/superposition-checker/fixtures/SYN343-1-Prover9-2026-6A.s");
    let actual_sha256 = Sha256::digest(proof_text.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let manifest: serde_json::Value = serde_json::from_str(include_str!(
        "../../../formal/superposition-checker/supported-rules.json"
    ))
    .unwrap();
    let archive_case = &manifest["limits"]["supported_dag_fixture_corpus_archived_positive_proofs"]
        ["SYN343-1-Prover9-2026-6A.s"];
    assert_eq!(
        archive_case["fixture_sha256"].as_str(),
        Some(actual_sha256.as_str())
    );
    assert_eq!(
        archive_case["archive_sha256"],
        "a39372f9b113e504401adb071a32940660919e7f0f86ddd1423004e8ae890df9"
    );
    assert_eq!(
        archive_case["fixture_normalization"],
        "remove_trailing_ascii_horizontal_whitespace_per_line"
    );
    assert!(proof_text.contains("% Archive SHA-256 before repository normalization:"));
    let problem = mrs_tptp::parse_tptp(problem_text).unwrap();
    let proof = mrs_tptp::parse_tptp(proof_text).unwrap();
    let verdict = verify_strict_with_source(
        &problem,
        &proof,
        Some("theBenchmark.p"),
        VerificationLimits::default(),
    );
    assert_eq!(verdict, KernelVerdict::Certified);
}

#[test]
fn strict_leaf_role_mapping_accepts_negated_conjectures_but_not_goals() {
    let proof = mrs_tptp::parse_tptp(
        "cnf(a,axiom,p,file('input',a)). \
         cnf(b,axiom,~p,file('input',b)). \
         cnf(bot,plain,$false,inference(resolve,[status(thm)],[a,b])).",
    )
    .unwrap();
    let negated_conjecture =
        mrs_tptp::parse_tptp("cnf(a,negated_conjecture,p). cnf(b,axiom,~p).").unwrap();
    assert_eq!(
        verify_strict_with_source(
            &negated_conjecture,
            &proof,
            Some("input"),
            VerificationLimits::default(),
        ),
        KernelVerdict::Certified
    );

    let conjecture = mrs_tptp::parse_tptp("cnf(a,conjecture,p). cnf(b,axiom,~p).").unwrap();
    assert_ne!(
        verify_strict_with_source(
            &conjecture,
            &proof,
            Some("input"),
            VerificationLimits::default(),
        ),
        KernelVerdict::Certified
    );
}

#[test]
#[ignore = "requires the pinned compiled Verus artifact; run by formal CI"]
fn archived_step_and_real_tptp_dags_traverse_compiled_checker() {
    let cfg = config();
    let archived_context = include_str!(
        "../../../formal/superposition-checker/fixtures/SWV406+1-c502-proof-context.p"
    );
    let archived_context_sha256 = Sha256::digest(archived_context.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(
        archived_context_sha256, "5710ac765546ac4f889430dc4fe9bbfdf0cb15cc24e3340e38ce4420959baaf4",
        "archived c502 context fixture changed"
    );
    let support_manifest: serde_json::Value = serde_json::from_str(include_str!(
        "../../../formal/superposition-checker/supported-rules.json"
    ))
    .unwrap();
    let limits = &support_manifest["limits"];
    assert_eq!(
        limits["archived_c502_context_fixture_sha256"].as_str(),
        Some(archived_context_sha256.as_str())
    );
    assert_eq!(
        limits["archived_c502_context_source_sha256"].as_str(),
        Some("5a8ea239cc6ef13d630bdfdfaa319c48b10785fad58e0238178ce5675646e822")
    );
    let parsed_context = mrs_tptp::parse_tptp(archived_context).unwrap();
    assert_eq!(parsed_context.formulas.len(), 9);
    let context_names: Vec<_> = parsed_context
        .formulas
        .iter()
        .map(|formula| formula.name())
        .collect();
    assert_eq!(
        context_names,
        [
            "c24", "c36", "c25", "c37", "c26", "c38", "c27", "c39", "c502"
        ]
    );
    let archived_report = check_tptp_step(archived_context, "c27", "c39", "c502", &cfg);
    assert_eq!(
        archived_report.verdict,
        ShadowVerdict::AcceptedStepIr,
        "{archived_report:?}"
    );
    assert!(!archived_report.production_certification);
    let forged_context = archived_context.replace(
        "X2 = X5 | remove_slb(remove_slb",
        "X2 = bogus | remove_slb(remove_slb",
    );
    assert!(matches!(
        check_tptp_step(&forged_context, "c27", "c39", "c502", &cfg).verdict,
        ShadowVerdict::Unknown(_)
    ));

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
fn archived_swv406_leo_iii_thf_proof_fails_closed_before_lowering() {
    const MANIFEST: &str = include_str!(
        "../../../formal/superposition-checker/fixtures/archived-unsupported-proofs.tsv"
    );
    let rows = MANIFEST
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect::<Vec<_>>();
    assert_eq!(
        rows.len(),
        1,
        "archived unsupported-proof inventory changed"
    );
    let fields = rows[0].split_whitespace().collect::<Vec<_>>();
    assert_eq!(fields.len(), 7, "malformed archived proof row");
    let [
        id,
        proof_file,
        source_url,
        expected_sha256,
        expected_formulas,
        expected_types,
        outcome,
    ] = fields.as_slice()
    else {
        unreachable!("field count checked above")
    };
    assert_eq!(*id, "swv406-leo-iii");
    assert!(source_url.starts_with("https://seetptp.tptp.org/"));
    assert_eq!(*outcome, "type-declaration-fail-closed");

    let path = std::path::Path::new(proof_file);
    assert_eq!(path.components().count(), 1, "unsafe archive fixture path");
    let fixture_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../formal/superposition-checker/fixtures");
    let proof_text = std::fs::read_to_string(fixture_dir.join(path)).unwrap();
    let actual_sha256 = Sha256::digest(proof_text.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual_sha256, *expected_sha256, "{id} source drifted");

    let proof = mrs_tptp::parse_tptp(&proof_text)
        .unwrap_or_else(|error| panic!("{id} archived proof did not parse: {error}"));
    assert_eq!(
        proof.formulas.len(),
        expected_formulas.parse::<usize>().unwrap()
    );
    assert_eq!(
        proof
            .formulas
            .iter()
            .filter(|formula| formula.role().is_type_declaration())
            .count(),
        expected_types.parse::<usize>().unwrap()
    );

    // The full independent THF proof is deliberately rejected at the explicit
    // typed-syntax boundary, before any input-leaf or inference lowering.
    let empty_problem = mrs_tptp::parse_tptp("").unwrap();
    let cfg = VerusConfig {
        executable: "unused-checker".into(),
        expected_sha256: "0".repeat(64),
        timeout: Duration::from_secs(1),
    };
    let report = check_tptp_dag(
        &empty_problem,
        &proof,
        None,
        VerificationLimits::default(),
        &cfg,
    );
    assert!(
        matches!(&report.verdict, ShadowVerdict::Unknown(reason)
            if reason.contains("type declarations in proofs are unsupported")),
        "{id}: {report:?}"
    );
    assert_eq!(
        report.artifact_sha256, None,
        "{id} must fail before runtime"
    );
}

#[test]
#[ignore = "requires the pinned compiled Verus artifact; run by formal CI"]
fn supported_dag_fixture_corpus_certifies_with_both_checkers() {
    const CORPUS: &str =
        include_str!("../../../formal/superposition-checker/fixtures/supported-dag-corpus.tsv");

    let cfg = config();
    let fixture_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../formal/superposition-checker/fixtures");
    let assert_mutation_rejected =
        |name: &str, problem_text: &str, proof_text: &str, source: Option<&str>| {
            let problem = mrs_tptp::parse_tptp(problem_text)
                .unwrap_or_else(|error| panic!("{name} mutated problem did not parse: {error}"));
            let proof = mrs_tptp::parse_tptp(proof_text)
                .unwrap_or_else(|error| panic!("{name} mutated proof did not parse: {error}"));
            let (verdict, report) = verify_with_verus_gate(
                &problem,
                &proof,
                source,
                VerificationLimits::default(),
                &cfg,
            );
            assert_ne!(
                verdict,
                KernelVerdict::Certified,
                "{name} mutation must not certify: {report:?}"
            );
            assert_ne!(
                report.verdict,
                ShadowVerdict::CertifiedDagIr,
                "{name} mutation must not pass the Verus DAG gate: {report:?}"
            );
        };
    let load_fixture = |name: &str| {
        let path = std::path::Path::new(name);
        assert_eq!(path.components().count(), 1, "unsafe corpus path: {name}");
        std::fs::read_to_string(fixture_dir.join(path))
            .unwrap_or_else(|error| panic!("failed to read corpus fixture {name}: {error}"))
    };

    let corpus_manifest: serde_json::Value = serde_json::from_str(include_str!(
        "../../../formal/superposition-checker/supported-rules.json"
    ))
    .expect("supported-rules manifest should be valid JSON");
    let mut cases = 0;
    let mut hand_authored_cases = 0;
    let mut synthetic_generated_cases = 0;
    let mut tptp_generated_cases = 0;
    let mut archived_positive_cases = 0;
    for (line_number, line) in CORPUS.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split_whitespace().collect::<Vec<_>>();
        assert_eq!(fields.len(), 6, "malformed corpus row {}", line_number + 1);
        let [
            name,
            origin,
            problem_file,
            proof_file,
            source,
            expected_nodes,
        ] = fields.as_slice()
        else {
            unreachable!("field count checked above")
        };
        match *origin {
            "hand-authored" => hand_authored_cases += 1,
            "mrs-generated-synthetic-0.3.0" => synthetic_generated_cases += 1,
            "mrs-generated-tptp-v10.0.0" => tptp_generated_cases += 1,
            "tptp-archive-prover9-2026-6a" => archived_positive_cases += 1,
            other => panic!("{name} has unrecognized corpus origin {other:?}"),
        }
        let problem_text = load_fixture(problem_file);
        let proof_text = load_fixture(proof_file);
        if *origin == "mrs-generated-tptp-v10.0.0" {
            let tptp_case = &corpus_manifest["limits"]["supported_dag_fixture_corpus_real_tptp_problems"]
                [*problem_file];
            let expected_sha256 = tptp_case["sha256"]
                .as_str()
                .expect("manifest should pin each generated TPTP problem hash");
            let actual_sha256 = Sha256::digest(problem_text.as_bytes())
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            assert_eq!(
                actual_sha256, expected_sha256,
                "{name} problem source drifted"
            );
            let expected_proof_sha256 = tptp_case["proof_sha256"]
                .as_str()
                .expect("manifest should pin each generated TPTP proof hash");
            let actual_proof_sha256 = Sha256::digest(proof_text.as_bytes())
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            assert_eq!(
                actual_proof_sha256, expected_proof_sha256,
                "{name} generated proof source drifted"
            );
            assert_eq!(tptp_case["tptp_release"], "10.0.0", "{name}");
            assert_eq!(
                tptp_case["checked_nodes"],
                expected_nodes.parse::<usize>().unwrap(),
                "{name} node-count manifest drifted"
            );
        } else if *origin == "tptp-archive-prover9-2026-6a" {
            let archive_case = &corpus_manifest["limits"]["supported_dag_fixture_corpus_archived_positive_proofs"]
                [*proof_file];
            let actual_problem_sha256 = Sha256::digest(problem_text.as_bytes())
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            let actual_proof_sha256 = Sha256::digest(proof_text.as_bytes())
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            assert_eq!(archive_case["problem_file"], *problem_file, "{name}");
            assert_eq!(
                archive_case["problem_sha256"], actual_problem_sha256,
                "{name}"
            );
            assert_eq!(
                archive_case["fixture_sha256"], actual_proof_sha256,
                "{name}"
            );
            assert_eq!(
                archive_case["archive_sha256"],
                "a39372f9b113e504401adb071a32940660919e7f0f86ddd1423004e8ae890df9",
                "{name} raw archive hash drifted"
            );
            assert_eq!(
                archive_case["fixture_normalization"],
                "remove_trailing_ascii_horizontal_whitespace_per_line",
                "{name} fixture normalization drifted"
            );
            assert_eq!(archive_case["system"], "Prover9", "{name}");
            assert_eq!(archive_case["system_version"], "2026-6A", "{name}");
            assert_eq!(
                archive_case["proof_metadata_tptp_release"], "9.3.1",
                "{name}"
            );
            assert_eq!(
                archive_case["problem_fixture_tptp_release"], "10.0.0",
                "{name}"
            );
            assert_eq!(
                archive_case["checked_nodes"],
                expected_nodes.parse::<usize>().unwrap(),
                "{name} node-count manifest drifted"
            );
            assert_eq!(
                archive_case["proof_url"],
                "https://seetptp.tptp.org/file/TPTPWorld/TSTP/Solutions/SYN/SYN343-1/Prover9---2026-6A.UNS-CRf.s",
                "{name} archive provenance drifted"
            );
        }
        let problem = mrs_tptp::parse_tptp(&problem_text)
            .unwrap_or_else(|error| panic!("{name} problem did not parse: {error}"));
        let proof = mrs_tptp::parse_tptp(&proof_text)
            .unwrap_or_else(|error| panic!("{name} proof did not parse: {error}"));
        let expected_nodes = expected_nodes
            .parse::<usize>()
            .unwrap_or_else(|error| panic!("{name} has invalid node count: {error}"));
        let source = (*source != "-").then_some(*source);
        let (verdict, report) = verify_with_verus_gate(
            &problem,
            &proof,
            source,
            VerificationLimits::default(),
            &cfg,
        );
        assert_eq!(verdict, KernelVerdict::Certified, "{name}: {report:?}");
        assert_eq!(
            report.verdict,
            ShadowVerdict::CertifiedDagIr,
            "{name}: {report:?}"
        );
        assert_eq!(report.checked_nodes, expected_nodes, "{name}: {report:?}");
        assert!(!report.production_certification);
        let support_manifest = report
            .support_manifest
            .as_ref()
            .unwrap_or_else(|| panic!("{name} report omitted support manifest: {report:?}"));
        assert_eq!(support_manifest["schema_version"], 1, "{name}: {report:?}");
        assert_eq!(
            support_manifest["checker_status"], "prototype_non_certifying",
            "{name}: {report:?}"
        );
        assert_eq!(
            report.support_manifest_sha256.as_deref().map(str::len),
            Some(64),
            "{name}: {report:?}"
        );
        assert_eq!(
            report.build_evidence_sha256.as_deref().map(str::len),
            Some(64),
            "{name}: {report:?}"
        );
        let serialized_report = serde_json::to_value(&report).unwrap();
        assert_eq!(
            serialized_report["support_manifest"]["checker_status"],
            "prototype_non_certifying"
        );

        let empty_roots = proof
            .formulas
            .iter()
            .filter(|formula| {
                formula.role() == mrs_tptp::FormulaRole::Plain && is_empty_cnf_clause(formula)
            })
            .count();
        assert_eq!(
            empty_roots, 1,
            "{name} must have exactly one empty-clause root"
        );
        assert_eq!(
            proof_text.matches("$false").count(),
            1,
            "{name} must have a unique textual empty-clause token for mutation"
        );
        let forged_root_text = proof_text.replacen("$false", "p", 1);
        let forged_root = mrs_tptp::parse_tptp(&forged_root_text)
            .unwrap_or_else(|error| panic!("{name} forged root did not parse: {error}"));
        let (forged_verdict, forged_report) = verify_with_verus_gate(
            &problem,
            &forged_root,
            source,
            VerificationLimits::default(),
            &cfg,
        );
        assert_ne!(
            forged_verdict,
            KernelVerdict::Certified,
            "{name} forged root: {forged_report:?}"
        );
        assert_ne!(
            forged_report.verdict,
            ShadowVerdict::CertifiedDagIr,
            "{name} forged root: {forged_report:?}"
        );

        let unsupported_rule_proof =
            replace_first_inference_rule(&proof_text, "unsupported_formal_corpus_rule");
        assert_mutation_rejected(
            &format!("{name} unsupported rule"),
            &problem_text,
            &unsupported_rule_proof,
            source,
        );

        let (parent_start, parent_end) = first_cited_parent_range(&proof_text)
            .unwrap_or_else(|| panic!("{name} has no cited inference parent"));
        let original_parent = &proof_text[parent_start..parent_end];
        let wrong_parent = proof_text
            .lines()
            .filter_map(first_input_name)
            .find(|candidate| *candidate != original_parent)
            .unwrap_or_else(|| panic!("{name} has no alternate input parent"));
        let wrong_parent_proof = replace_first_cited_parent(&proof_text, wrong_parent);
        assert_mutation_rejected(
            &format!("{name} wrong parent"),
            &problem_text,
            &wrong_parent_proof,
            source,
        );

        let disconnected_problem_line =
            "cnf(mrs_formal_disconnected,axiom,mrs_formal_disconnected_pred).";
        let disconnected_proof_line = format!(
            "cnf(mrs_formal_disconnected,axiom,mrs_formal_disconnected_pred,file('{}',mrs_formal_disconnected)).",
            source.unwrap_or("input")
        );
        assert!(
            !problem_text.contains("mrs_formal_disconnected")
                && !proof_text.contains("mrs_formal_disconnected"),
            "{name} collides with the disconnected-node mutation"
        );
        let disconnected_problem = format!("{problem_text}\n{disconnected_problem_line}\n");
        let first_inference = proof_text
            .find("inference(")
            .unwrap_or_else(|| panic!("{name} has no inference to anchor the inserted input"));
        let insertion = [
            proof_text[..first_inference]
                .rfind("\ncnf(")
                .map(|start| start + 1)
                .or_else(|| proof_text.starts_with("cnf(").then_some(0)),
            proof_text[..first_inference]
                .rfind("\nfof(")
                .map(|start| start + 1)
                .or_else(|| proof_text.starts_with("fof(").then_some(0)),
        ]
        .into_iter()
        .flatten()
        .max()
        .unwrap_or_else(|| panic!("{name} has no annotated formula before its first inference"));
        let disconnected_proof = format!(
            "{}{}\n{}",
            &proof_text[..insertion],
            disconnected_proof_line,
            &proof_text[insertion..]
        );
        assert_mutation_rejected(
            &format!("{name} disconnected input"),
            &disconnected_problem,
            &disconnected_proof,
            source,
        );
        cases += 1;
    }
    assert_eq!(cases, 9, "supported DAG corpus inventory changed");
    assert_eq!(
        hand_authored_cases, 4,
        "hand-authored corpus inventory changed"
    );
    assert_eq!(
        synthetic_generated_cases, 1,
        "synthetic generated corpus inventory changed"
    );
    assert_eq!(
        tptp_generated_cases, 3,
        "TPTP generated corpus inventory changed"
    );
    assert_eq!(
        archived_positive_cases, 1,
        "archived positive inventory changed"
    );
    assert_eq!(
        corpus_manifest["limits"]["supported_dag_fixture_corpus_entries"],
        cases
    );
    assert_eq!(
        corpus_manifest["limits"]["supported_dag_fixture_corpus_hand_authored_cases"],
        hand_authored_cases
    );
    assert_eq!(
        corpus_manifest["limits"]["supported_dag_fixture_corpus_synthetic_generated_cases"],
        synthetic_generated_cases
    );
    assert_eq!(
        corpus_manifest["limits"]["supported_dag_fixture_corpus_mrs_generated_cases"],
        synthetic_generated_cases + tptp_generated_cases
    );
    assert_eq!(
        corpus_manifest["limits"]["supported_dag_fixture_corpus_real_tptp_generated_cases"],
        tptp_generated_cases
    );
    assert_eq!(
        corpus_manifest["limits"]["supported_dag_fixture_corpus_archived_positive_cases"],
        archived_positive_cases
    );
    assert_eq!(
        corpus_manifest["limits"]["supported_dag_fixture_corpus_has_external_archived_positive"],
        true
    );
}

#[test]
#[ignore = "requires the pinned compiled Verus artifact; run by formal CI"]
fn tptp_10_type_declaration_roles_fail_closed_in_problem_and_proof() {
    let cfg = config();
    let problem_text = "cnf(a,axiom,p). cnf(b,axiom,~p).";
    let proof_text = "cnf(a,axiom,p,file('input',a)).
        cnf(b,axiom,~p,file('input',b)).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[a,b])).";
    let problem = mrs_tptp::parse_tptp(problem_text).unwrap();
    let proof = mrs_tptp::parse_tptp(proof_text).unwrap();
    let declarations = [
        "tff(type_decl,type,nat: $tType).",
        "tff(datatype_decl,type-datatype,nat: $tType).",
        "tff(codatatype_decl,type-codatatype,stream: $tType).",
        "tff(datatype_constructor,type-datatype_constructor,s: nat > nat).",
        "tff(codatatype_constructor,type-codatatype_constructor,stream_cons: $i > stream).",
    ];

    for declaration in declarations {
        let typed_proof_text = format!("{proof_text} {declaration}");
        let typed_proof = mrs_tptp::parse_tptp(&typed_proof_text)
            .expect("TPTP 10 type declaration in proof should parse");
        let (verdict, report) = verify_with_verus_gate(
            &problem,
            &typed_proof,
            Some("input"),
            VerificationLimits::default(),
            &cfg,
        );
        assert_ne!(
            verdict,
            KernelVerdict::Certified,
            "{declaration}: {report:?}"
        );
        assert!(
            matches!(report.verdict, ShadowVerdict::Unknown(_)),
            "proof-side {declaration} must be unsupported: {report:?}"
        );
        assert!(!report.production_certification);

        let typed_problem_text = format!("{problem_text} {declaration}");
        let typed_problem = mrs_tptp::parse_tptp(&typed_problem_text)
            .expect("TPTP 10 type declaration in problem should parse");
        let (verdict, report) = verify_with_verus_gate(
            &typed_problem,
            &proof,
            Some("input"),
            VerificationLimits::default(),
            &cfg,
        );
        assert_ne!(
            verdict,
            KernelVerdict::Certified,
            "{declaration}: {report:?}"
        );
        assert!(
            matches!(report.verdict, ShadowVerdict::Unknown(_)),
            "problem-side {declaration} must be unsupported: {report:?}"
        );
        assert!(!report.production_certification);
    }
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
    let zero_refl_text = "cnf(r,plain,a=a,inference(reflexivity,[status(thm)],[])).
        cnf(a,axiom,p(a),file('input',a)).
        cnf(b,axiom,a!=a|~p(a),file('input',b)).
        cnf(m1,plain,~p(a),inference(resolution,[status(thm)],[r,b])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[m1,a])).";
    let zero_refl_proof = mrs_tptp::parse_tptp(zero_refl_text).unwrap();
    let (kernel, report) = verify_with_verus_gate(
        &zero_refl_problem,
        &zero_refl_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified, "{report:?}");
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");

    // 11. Zero-parent excluded middle inference:
    let zero_em_problem = mrs_tptp::parse_tptp("cnf(a,axiom,p). cnf(b,axiom,~p).").unwrap();
    let zero_em_text = "cnf(em,plain,p|~p,inference(excluded_middle,[status(thm)],[])).
        cnf(a,axiom,p,file('input',a)).
        cnf(b,axiom,~p,file('input',b)).
        cnf(m1,plain,~p,inference(resolution,[status(thm)],[em,b])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[a,m1])).";
    let zero_em_proof = mrs_tptp::parse_tptp(zero_em_text).unwrap();
    let (kernel, report) = verify_with_verus_gate(
        &zero_em_problem,
        &zero_em_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified, "{report:?}");
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");

    // 12. A zero-parent tautology is also certified by both independent paths.
    let zero_taut_text = "cnf(tau,plain,p|~p,inference(tautology,[status(thm)],[])).
        cnf(a,axiom,p,file('input',a)).
        cnf(b,axiom,~p,file('input',b)).
        cnf(m1,plain,~p,inference(resolution,[status(thm)],[tau,b])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[a,m1])).";
    let zero_taut_proof = mrs_tptp::parse_tptp(zero_taut_text).unwrap();
    let (kernel, report) = verify_with_verus_gate(
        &zero_em_problem,
        &zero_taut_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified, "{report:?}");
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");

    // 13. Multi-parent Horn forward chain (3 parents: r2, fact, r1 -> r(a)):
    let horn_problem = mrs_tptp::parse_tptp(
        "cnf(r1,axiom,~p(X)|q(X)). cnf(r2,axiom,~q(X)|r(X)). cnf(fact,axiom,p(a)). cnf(nr,axiom,~r(a)).",
    )
    .unwrap();
    let horn_text = "cnf(r1,axiom,~p(X)|q(X),file('input',r1)).
        cnf(r2,axiom,~q(X)|r(X),file('input',r2)).
        cnf(fact,axiom,p(a),file('input',fact)).
        cnf(h,plain,r(a),inference(horn,[status(thm)],[r2,fact,r1])).
        cnf(nr,axiom,~r(a),file('input',nr)).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[h,nr])).";
    let horn_proof = mrs_tptp::parse_tptp(horn_text).unwrap();
    let (kernel, report) = verify_with_verus_gate(
        &horn_problem,
        &horn_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified, "{report:?}");
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");

    // 14. Multi-parent Horn chaining standardizes variables apart and accepts
    // an alpha-renamed universally quantified conclusion.
    let horn_alpha_problem = mrs_tptp::parse_tptp(
        "cnf(r1,axiom,~p(X)|q(X)). cnf(r2,axiom,~q(X)|r(X)). cnf(fact,axiom,p(Y)). cnf(nr,axiom,~r(Y)).",
    )
    .unwrap();
    let horn_alpha_text = "cnf(r1,axiom,~p(X)|q(X),file('input',r1)).
        cnf(r2,axiom,~q(X)|r(X),file('input',r2)).
        cnf(fact,axiom,p(Y),file('input',fact)).
        cnf(h,plain,r(Y),inference(horn,[status(thm)],[r2,fact,r1])).
        cnf(nr,axiom,~r(Y),file('input',nr)).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[h,nr])).";
    let horn_alpha_proof = mrs_tptp::parse_tptp(horn_alpha_text).unwrap();
    let (kernel, report) = verify_with_verus_gate(
        &horn_alpha_problem,
        &horn_alpha_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified, "{report:?}");
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");
    let contra_alpha_text = horn_alpha_text.replace("inference(horn,", "inference(contrapositive,");
    let contra_alpha_proof = mrs_tptp::parse_tptp(&contra_alpha_text).unwrap();
    let (kernel, report) = verify_with_verus_gate(
        &horn_alpha_problem,
        &contra_alpha_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified, "{report:?}");
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");

    // 15. Multi-parent consequence resolution chain (3 parents: a, b, c -> r):
    let conseq_chain_problem = mrs_tptp::parse_tptp(
        "cnf(a,axiom,~p|~q|r). cnf(b,axiom,p). cnf(c,axiom,q). cnf(d,axiom,~r).",
    )
    .unwrap();
    let conseq_chain_text = "cnf(a,axiom,~p|~q|r,file('input',a)).
        cnf(b,axiom,p,file('input',b)).
        cnf(c,axiom,q,file('input',c)).
        cnf(d,axiom,~r,file('input',d)).
        cnf(res,plain,r,inference(consequence,[status(thm)],[a,b,c])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[res,d])).";
    let conseq_chain_proof = mrs_tptp::parse_tptp(conseq_chain_text).unwrap();
    let (kernel, report) = verify_with_verus_gate(
        &conseq_chain_problem,
        &conseq_chain_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified, "{report:?}");
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");

    // 16. Contrapositive rule alias:
    let contra_problem =
        mrs_tptp::parse_tptp("cnf(rule,axiom,~p(X)|q(X)). cnf(nq,axiom,~q(a)). cnf(p,axiom,p(a)).")
            .unwrap();
    let contra_text = "cnf(rule,axiom,~p(X)|q(X),file('input',rule)).
        cnf(nq,axiom,~q(a),file('input',nq)).
        cnf(p,axiom,p(a),file('input',p)).
        cnf(np,plain,~p(a),inference(contrapositive,[status(thm)],[rule,nq])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[np,p])).";
    let contra_proof = mrs_tptp::parse_tptp(contra_text).unwrap();
    let (kernel, report) = verify_with_verus_gate(
        &contra_problem,
        &contra_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified, "{report:?}");
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");

    // 17. Equality factoring with commuted literal:
    let eq_fact_commuted_problem = mrs_tptp::parse_tptp(
        "cnf(a,axiom,p(a)=p(b)|q(c)=p(a)). cnf(b,axiom,p(a)!=p(b)). cnf(c,axiom,p(b)=q(c)).",
    )
    .unwrap();
    let eq_fact_commuted_text = "cnf(a,axiom,p(a)=p(b)|q(c)=p(a),file('input',a)).
        cnf(b,axiom,p(a)!=p(b),file('input',b)).
        cnf(c,axiom,p(b)=q(c),file('input',c)).
        cnf(ef,plain,p(a)=p(b)|p(b)!=q(c),inference(equality_factoring,[status(thm)],[a])).
        cnf(r1,plain,p(b)!=q(c),inference(resolution,[status(thm)],[ef,b])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[c,r1])).";
    let eq_fact_commuted_proof = mrs_tptp::parse_tptp(eq_fact_commuted_text).unwrap();
    let (kernel, report) = verify_with_verus_gate(
        &eq_fact_commuted_problem,
        &eq_fact_commuted_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified, "{report:?}");
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");

    // 18. Exact resolution accepts equality pivots modulo equality symmetry.
    let symmetric_eq_problem =
        mrs_tptp::parse_tptp("cnf(eq,axiom,a=b). cnf(neq,axiom,b!=a).").unwrap();
    let symmetric_eq_text = "cnf(eq,axiom,a=b,file('input',eq)).
        cnf(neq,axiom,b!=a,file('input',neq)).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[eq,neq])).";
    let symmetric_eq_proof = mrs_tptp::parse_tptp(symmetric_eq_text).unwrap();
    let (kernel, report) = verify_with_verus_gate(
        &symmetric_eq_problem,
        &symmetric_eq_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified, "{report:?}");
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");

    // 19. Standardized first-order resolution also unifies swapped equality
    // pivots, then the ordinary resolution tail closes the refutation.
    let symmetric_fo_problem = mrs_tptp::parse_tptp(
        "cnf(eq,axiom,f(X)=a|p(X)). cnf(neq,axiom,a!=f(a)). cnf(np,axiom,~p(a)).",
    )
    .unwrap();
    let symmetric_fo_text = "cnf(eq,axiom,f(X)=a|p(X),file('input',eq)).
        cnf(neq,axiom,a!=f(a),file('input',neq)).
        cnf(np,axiom,~p(a),file('input',np)).
        cnf(pos,plain,p(a),inference(resolution,[status(thm)],[eq,neq])).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[pos,np])).";
    let symmetric_fo_proof = mrs_tptp::parse_tptp(symmetric_fo_text).unwrap();
    let (kernel, report) = verify_with_verus_gate(
        &symmetric_fo_problem,
        &symmetric_fo_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_eq!(kernel, KernelVerdict::Certified, "{report:?}");
    assert_eq!(report.verdict, ShadowVerdict::CertifiedDagIr, "{report:?}");

    // 20. Forged Horn conclusion fail-closed regression:
    let forged_horn_text = "cnf(r1,axiom,~p(X)|q(X),file('input',r1)).
        cnf(r2,axiom,~q(X)|r(X),file('input',r2)).
        cnf(fact,axiom,p(a),file('input',fact)).
        cnf(h,plain,s(a),inference(horn,[status(thm)],[r2,fact,r1])).
        cnf(nr,axiom,~r(a),file('input',nr)).
        cnf(bot,plain,$false,inference(resolution,[status(thm)],[h,nr])).";
    let forged_horn_proof = mrs_tptp::parse_tptp(forged_horn_text).unwrap();
    let (kernel, report) = verify_with_verus_gate(
        &horn_problem,
        &forged_horn_proof,
        Some("input"),
        VerificationLimits::default(),
        &cfg,
    );
    assert_ne!(kernel, KernelVerdict::Certified);
    assert_ne!(report.verdict, ShadowVerdict::CertifiedDagIr);
}
