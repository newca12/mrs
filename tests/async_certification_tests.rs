use std::path::PathBuf;
use std::process::Command;

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

    assert!(stderr.contains("self_check=Certified"));
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
