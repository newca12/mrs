use std::path::PathBuf;
use std::process::Command;

#[test]
fn shared_goal_definitions_preserve_skolem_introduction_order() {
    let problem = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("problems/group.p");
    let output = Command::new(env!("CARGO_BIN_EXE_mrs"))
        .args([
            "--self-check",
            "--time",
            "10",
            "--workers",
            "2",
            "--schedule",
            "casc_ueq",
        ])
        .env("MRS_SHARED_POOL_INTERVAL", "1")
        .arg(problem)
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("% SZS status Theorem for group"),
        "{stdout}\n{stderr}"
    );
    assert!(stderr.contains("self_check=Certified"), "{stderr}");
}

#[test]
fn isolated_checker_replays_actual_parents_and_refuses_forged_conclusion() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("problem.p"),
        "cnf(a,axiom,p). cnf(b,axiom,~p).",
    )
    .unwrap();
    std::fs::write(directory.path().join("request.json"), serde_json::to_vec(&serde_json::json!({
        "problem_path": "problem.p", "include_root": null, "has_includes": false, "budget_ms": 5000,
    })).unwrap()).unwrap();
    let proof = "cnf(a,axiom,p,file('problem.p',a)). cnf(b,axiom,~p,file('problem.p',b)). cnf(c,plain,$false,inference(resolution,[status(thm)],[a,b])).";
    std::fs::write(directory.path().join("proof.p"), proof).unwrap();
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_mrs"))
            .arg("--internal-certify")
            .arg(directory.path())
            .output()
            .unwrap()
    };
    let good = run();
    assert!(good.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&good.stdout).unwrap(),
        "Certified"
    );
    std::fs::write(
        directory.path().join("proof.p"),
        proof.replace("[a,b]", "[a,a]"),
    )
    .unwrap();
    let bad = run();
    assert!(bad.status.success());
    assert_ne!(
        serde_json::from_slice::<serde_json::Value>(&bad.stdout).unwrap(),
        "Certified"
    );
}

#[test]
fn async_certified_flag_with_adequate_budget_certifies() {
    let problem = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("problems/socrates.p");
    let output = Command::new(env!("CARGO_BIN_EXE_mrs"))
        .args([
            "--certified",
            "--time",
            "10",
            "--workers",
            "2",
            "--cert-reserve-worker",
            "--schedule",
            "fast",
        ])
        .arg(&problem)
        .output()
        .expect("mrs CLI should run");

    assert!(
        output.status.success(),
        "mrs exited with {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stdout.contains("% SZS status Theorem for socrates"));
    assert!(stdout.contains("% SZS output start Proof for socrates"));
    assert!(stdout.contains("% Candidate certification audit:"));
    assert!(stdout.contains("Candidates received:"));
    assert!(stdout.contains("Certified candidate index: 1"));

    assert!(
        stderr.contains("self_check=Certified"),
        "stderr did not contain self_check=Certified:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(stderr.contains("cert_oversubscribed=false"));
    assert!(stderr.contains("cert_search_workers=1"));
    assert!(stderr.contains("cert_workers=1"));
    assert!(stderr.contains("cert_idx=1"));
}

#[test]
fn async_certified_short_budget_records_rejection_telemetry() {
    let problem = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("problems/socrates.p");
    let output = Command::new(env!("CARGO_BIN_EXE_mrs"))
        .args([
            "--certified",
            "--time",
            "1",
            "--workers",
            "2",
            "--schedule",
            "fast",
        ])
        .arg(&problem)
        .output()
        .expect("mrs CLI should run");

    assert!(
        output.status.success(),
        "mrs exited with {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // One-second budget leaves < 2s reserve, so candidates are rejected due to reserve
    assert!(stdout.contains("% SZS status GaveUp for socrates"));
    assert!(stdout.contains("% Candidate certification audit:"));
    assert!(stdout.contains("Candidates rejected:"));
    assert!(stdout.contains("insufficient time remains"));

    assert!(stderr.contains("result=Refutation"));
    assert!(stderr.contains("self_check=Rejected"));
    assert!(stderr.contains("candidates="));
    assert!(stderr.contains("rejections="));
    assert!(stderr.contains("cert_oversubscribed=true"));
}
