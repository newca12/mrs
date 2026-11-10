//! End-to-end regressions for arithmetic semantics, including certification.

use mrs_proof_kernel::{KernelVerdict, VerificationLimits, verify_strict};
use std::process::Command;

fn run_path(path: &std::path::Path, extra: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_mrs"))
        .args([
            "--hardware",
            "adaptive",
            "--workers",
            "1",
            "--strategy",
            "1",
            "--time",
            "5",
        ])
        .args(extra)
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn run(input: &str, extra: &[&str]) -> String {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("arithmetic.p");
    std::fs::write(&path, input).unwrap();
    run_path(&path, extra)
}

fn assert_status(output: &str, expected: &str) {
    assert!(
        output.contains(&format!("% SZS status {expected} for")),
        "{output}"
    );
}

#[test]
fn less_of_identical_integers_is_never_satisfiable() {
    for input in [
        "fof(a,axiom,$less(1,1)).",
        "tff(a,axiom,$less(1,1)).",
        "tff(t,type,a:$int). tff(a,axiom,$less(a,a)).",
    ] {
        for flags in [
            &[][..],
            &["--certify-ordered"][..],
            &["--certify-ordered", "--self-check"][..],
        ] {
            assert_status(&run(input, flags), "Unsatisfiable");
        }
    }
}

#[test]
fn exact_arithmetic_theorems_have_replayable_proofs() {
    for input in [
        "tff(g,conjecture,$sum(1/3,1/6) = 1/2).",
        "tff(g,conjecture,$sum(340282366920938463463374607431768211456,1) = 340282366920938463463374607431768211457).",
        "tff(g,conjecture,($quotient_e(-7,-3) = 3 & $remainder_e(-7,-3) = 2)).",
        "tff(g,conjecture,($round(-5/2) = -2/1 & $round(-7/2) = -4/1)).",
        "tff(g,conjecture,$quotient(3,2) = 3/2).",
    ] {
        let output = run(input, &["--self-check"]);
        assert_status(&output, "Theorem");
        let problem = mrs_tptp::parse_tptp(input).unwrap();
        let proof = mrs_tptp::parse_tptp(&output).unwrap();
        assert_eq!(
            verify_strict(&problem, &proof, VerificationLimits::default()),
            KernelVerdict::Certified
        );
    }
}

#[test]
fn arithmetic_models_are_checked_in_standard_sorts() {
    let flags = &["--certify-ordered", "--self-check"];
    let input = "tff(a,axiom,$less(1/3,1/2)).";
    let output = run(input, flags);
    assert_status(&output, "Satisfiable");
    assert!(output.contains("% SZS output start Model for"));
    assert!(!output.contains("FiniteInterpretation"));
    let model = mrs_core::model::ModelCertificate::extract_from_text(&output).unwrap();
    use mrs_proof_kernel::model::ModelEvaluation as _;
    assert!(matches!(
        model.validate(&mrs_tptp::parse_tptp(input).unwrap(), Some("Satisfiable")),
        mrs_proof_kernel::model::ModelVerdict::Certified { .. }
    ));
    assert_status(
        &run("tff(g,conjecture,$less(1,1)).", flags),
        "CounterSatisfiable",
    );
    // TPTP v10 ARI525_1: conversion floors negative nonintegral rationals.
    assert_status(
        &run("tff(g,conjecture,$greatereq($to_int(-1/5),0)).", flags),
        "CounterSatisfiable",
    );
}

#[test]
fn unevaluated_arithmetic_is_not_ordinary_first_order_input() {
    for input in [
        "tff(g,conjecture,![X:$int]:$less(X,2)).",
        "tff(a,axiom,p($sum(1,2))).",
        "tff(a,axiom,$quotient(1,0) = 0/1).",
        "fof(a,axiom,$$unknown(a)).",
        "tff(a,axiom,$less(1.0,2.0)).",
        "tff(a,axiom,$less(1,1/1)).",
    ] {
        assert_status(&run(input, &["--certify-ordered"]), "GaveUp");
    }
}

#[test]
fn include_selections_and_arithmetic_model_validation_agree() {
    let directory = tempfile::tempdir().unwrap();
    let main = directory.path().join("problem.p");
    std::fs::write(
        directory.path().join("arithmetic.ax"),
        "tff(good,axiom,$less(1,2)). tff(bad,axiom,$less(2,1)).",
    )
    .unwrap();
    std::fs::write(&main, "include('arithmetic.ax',[good]).").unwrap();
    assert_status(
        &run_path(&main, &["--certify-ordered", "--self-check"]),
        "Satisfiable",
    );
    std::fs::write(&main, "include('arithmetic.ax',[bad]).").unwrap();
    assert_status(
        &run_path(&main, &["--certify-ordered", "--self-check"]),
        "Unsatisfiable",
    );
}
