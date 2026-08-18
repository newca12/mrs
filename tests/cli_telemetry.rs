use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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

#[test]
fn unsupported_formula_in_include_is_not_reported_satisfiable() {
    let root = std::env::temp_dir().join(format!(
        "mrs-unsupported-include-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after the Unix epoch")
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).expect("temporary include directory should be created");
    let problem = root.join("problem.p");
    let included = root.join("unsupported.ax");
    std::fs::write(&problem, "include('unsupported.ax').\n")
        .expect("temporary problem should be written");
    std::fs::write(&included, "thf(unsupported, axiom, $true).\n")
        .expect("temporary include should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_mrs"))
        .args(["--time", "1", "--workers", "1"])
        .arg(&problem)
        .output()
        .expect("mrs CLI should run");
    let _ = std::fs::remove_dir_all(&root);

    assert!(
        output.status.success(),
        "mrs exited with {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("% SZS status GaveUp for problem"),
        "unsupported included formulas must not be reported satisfiable: {stdout}"
    );
    assert!(!stdout.contains("% SZS status Satisfiable"));
}
