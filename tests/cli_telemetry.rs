use std::path::PathBuf;
use std::process::Command;

#[test]
fn self_check_rejection_preserves_search_result_in_telemetry() {
    let problem = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("problems/socrates.p");
    let output = Command::new(env!("CARGO_BIN_EXE_mrs"))
        .args([
            "--self-check",
            "--time",
            "1",
            "--workers",
            "1",
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

    // The search found a refutation, but the one-second budget deliberately
    // leaves less than the two-second self-check reserve. The public SZS
    // result is therefore GaveUp while telemetry preserves both facts.
    assert!(stdout.contains("% SZS status GaveUp for socrates"));
    assert!(stderr.contains("result=Refutation"));
    assert!(stderr.contains("self_check=Rejected"));
}
