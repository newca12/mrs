use mrs_proof_kernel::model::{EqualitySemantics, ModelCertificate, ModelEvaluation, ModelVerdict};
use mrs_proof_kernel::{KernelVerdict, VerificationLimits, verify_strict};
use mrs_tptp::parse_tptp;

fn proof_check(problem: &str, proof: &str) -> KernelVerdict {
    verify_strict(
        &parse_tptp(problem).unwrap(),
        &parse_tptp(proof).unwrap(),
        VerificationLimits::default(),
    )
}

fn empty_model() -> ModelCertificate {
    let mut model = ModelCertificate {
        domain_size: 1,
        constants: Default::default(),
        functions: Default::default(),
        predicates: Default::default(),
        equality: EqualitySemantics::StrictIdentity,
        digest: String::new(),
    };
    model.digest = model.compute_digest();
    model
}

#[test]
fn proof_leaves_are_checked_against_exact_source_arithmetic() {
    let proof = "cnf(bot,axiom,$false,file('problem.p',a)).";
    for problem in [
        "fof(a,axiom,$less(1,1)).",
        "tff(a,axiom,$less(1/2,1/3)).",
        "tcf(a,axiom,$sum(1,2) != 3).",
    ] {
        assert_eq!(
            proof_check(problem, proof),
            KernelVerdict::Certified,
            "{problem}"
        );
    }
    for problem in [
        "fof(a,axiom,$less(1,2)).",
        "tff(a,axiom,$sum(1/3,1/6) = 1/2).",
        "tcf(a,axiom,$sum(1,2) != 4).",
        "fof(a,axiom,'$less'(a,a)).",
    ] {
        assert!(
            matches!(proof_check(problem, proof), KernelVerdict::Rejected(_)),
            "{problem}"
        );
    }
}

#[test]
fn true_arithmetic_conjecture_keeps_negation_provenance() {
    let problem = "tff(g,conjecture,$sum(1/3,1/6) = 1/2).";
    let proof = "fof(g,conjecture,$true,file('problem.p',g)).\n\
        fof(n,negated_conjecture,~$true,inference(negated_conjecture,[status(cth)],[g])).\n\
        cnf(bot,plain,$false,inference(cnf_transformation,[status(thm)],[n])).";
    assert_eq!(proof_check(problem, proof), KernelVerdict::Certified);
    let mutated = problem.replace("1/2", "2/3");
    assert!(matches!(
        proof_check(&mutated, proof),
        KernelVerdict::Rejected(_)
    ));
}

#[test]
fn unsupported_source_arithmetic_cannot_be_certified_as_false() {
    let proof = "cnf(bot,axiom,$false,file('problem.p',a)).";
    for problem in [
        "tff(a,axiom,![X:$int]:$less(X,1)).",
        "fof(a,axiom,$quotient(1,0) = 0/1).",
        "tff(a,axiom,$less(1,1/1)).",
        "fof(a,axiom,$less(1.0,1.0)).",
        "fof(a,axiom,$$unknown(a)).",
    ] {
        assert!(
            matches!(proof_check(problem, proof), KernelVerdict::Inconclusive(_)),
            "{problem}"
        );
    }
}

#[test]
fn models_use_standard_arithmetic_not_finite_predicate_tables() {
    let model = empty_model();
    for problem in [
        "tff(a,axiom,$less(1/3,1/2)).",
        "fof(a,axiom,$sum(340282366920938463463374607431768211456,1) = 340282366920938463463374607431768211457).",
        "tcf(a,axiom,$is_rat(1)).",
    ] {
        assert!(
            matches!(
                model.validate(&parse_tptp(problem).unwrap(), Some("Satisfiable")),
                ModelVerdict::Certified { .. }
            ),
            "{problem}"
        );
    }
    for problem in ["tff(a,axiom,$less(1,1)).", "fof(a,axiom,$sum(1,1) = 3)."] {
        assert!(
            matches!(
                model.validate(&parse_tptp(problem).unwrap(), Some("Satisfiable")),
                ModelVerdict::Rejected(_)
            ),
            "{problem}"
        );
    }
    let false_goal = parse_tptp("tff(g,conjecture,$to_int(-1/5) = 0).").unwrap();
    assert!(matches!(
        model.validate(&false_goal, Some("CounterSatisfiable")),
        ModelVerdict::Certified { .. }
    ));
    let true_goal = parse_tptp("tff(g,conjecture,$to_int(-1/5) = -1).").unwrap();
    assert!(matches!(
        model.validate(&true_goal, Some("CounterSatisfiable")),
        ModelVerdict::Rejected(_)
    ));
}

#[test]
fn unresolved_interpreted_formulas_never_use_finite_tables() {
    let model = empty_model();
    for problem in [
        "fof(a,axiom,$$unknown).",
        "fof(a,axiom,p(1)).",
        "fof(a,axiom,$less(a,b)).",
        "tff(a,axiom,![X:$int]:$less(X,1)).",
        "tff(a,axiom,$quotient(1,0) = 0/1).",
    ] {
        assert!(
            matches!(
                model.validate(&parse_tptp(problem).unwrap(), Some("Satisfiable")),
                ModelVerdict::Inconclusive(_)
            ),
            "{problem}"
        );
    }
}

#[test]
fn a_forged_less_table_cannot_override_standard_less() {
    let mut model = empty_model();
    model.predicates.insert(
        "$less".into(),
        mrs_proof_kernel::model::PredicateTable {
            arity: 2,
            table: vec![true],
        },
    );
    model.constants.insert("1".into(), 0);
    model.digest = model.compute_digest();
    let problem = parse_tptp("fof(a,axiom,$less(1,1)).").unwrap();
    assert!(
        matches!(model.validate(&problem, Some("Satisfiable")), ModelVerdict::Rejected(reason) if reason.contains("fixed arithmetic"))
    );
}

#[test]
fn model_include_resource_caps_and_namespaces_are_inconclusive() {
    let directory = tempfile::tempdir().unwrap();
    let model = empty_model();
    let oversized = directory.path().join("oversized.p");
    std::fs::File::create(&oversized)
        .unwrap()
        .set_len(16 * 1024 * 1024 + 1)
        .unwrap();
    assert!(
        matches!(model.validate_file(&oversized, Some("Satisfiable")), ModelVerdict::Inconclusive(reason) if reason.contains("byte limit"))
    );

    for depth in 0..=65 {
        let body = if depth == 65 {
            "tff(a,axiom,$true).".into()
        } else {
            format!("include('depth{}.p').", depth + 1)
        };
        std::fs::write(directory.path().join(format!("depth{depth}.p")), body).unwrap();
    }
    assert!(
        matches!(model.validate_file(&directory.path().join("depth0.p"), Some("Satisfiable")), ModelVerdict::Inconclusive(reason) if reason.contains("depth/file limit"))
    );
    let namespace = directory.path().join("namespace.p");
    std::fs::write(&namespace, "include('unused.ax',*,space).").unwrap();
    assert!(
        matches!(model.validate_file(&namespace, Some("Satisfiable")), ModelVerdict::Inconclusive(reason) if reason.contains("namespaces"))
    );
}

#[test]
fn file_model_validation_checks_arithmetic_in_includes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("problem.p");
    let axioms = directory.path().join("arithmetic.ax");
    std::fs::write(&path, "include('arithmetic.ax').").unwrap();
    std::fs::write(&axioms, "tff(a,axiom,$less(1,2)).").unwrap();
    let model = empty_model();
    assert!(matches!(
        model.validate_file(&path, Some("Satisfiable")),
        ModelVerdict::Certified { .. }
    ));
    std::fs::write(&axioms, "tff(a,axiom,$less(2,1)).").unwrap();
    assert!(matches!(
        model.validate_file(&path, Some("Satisfiable")),
        ModelVerdict::Rejected(_)
    ));
}

#[test]
fn unresolved_missing_and_circular_includes_fail_closed() {
    let model = empty_model();
    let unresolved = parse_tptp("include('missing.ax').").unwrap();
    assert!(matches!(
        model.validate(&unresolved, Some("Satisfiable")),
        ModelVerdict::Inconclusive(_)
    ));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("problem.p");
    std::fs::write(&path, "include('missing.ax').").unwrap();
    assert!(matches!(
        model.validate_file(&path, Some("Satisfiable")),
        ModelVerdict::Inconclusive(_)
    ));
    std::fs::write(&path, "include('problem.p').").unwrap();
    assert!(matches!(
        model.validate_file(&path, Some("Satisfiable")),
        ModelVerdict::Inconclusive(_)
    ));
}

#[test]
fn model_include_selections_keep_siblings_and_nested_constraints() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("problem.p");
    std::fs::write(
        directory.path().join("inner.ax"),
        "tff(good,axiom,$less(1,2)). tff(bad,axiom,$less(2,1)).",
    )
    .unwrap();
    std::fs::write(
        directory.path().join("outer.ax"),
        "include('inner.ax',[good]). tff(outer,axiom,$is_rat(1)).",
    )
    .unwrap();
    std::fs::write(
        &path,
        "include('outer.ax',[outer]). include('inner.ax',[good]).",
    )
    .unwrap();
    let model = empty_model();
    assert!(matches!(
        model.validate_file(&path, Some("Satisfiable")),
        ModelVerdict::Certified {
            formulas_evaluated: 3,
            ..
        }
    ));
    std::fs::write(
        &path,
        "include('inner.ax',[good]). include('inner.ax',[bad]).",
    )
    .unwrap();
    assert!(matches!(
        model.validate_file(&path, Some("Satisfiable")),
        ModelVerdict::Rejected(_)
    ));
}
