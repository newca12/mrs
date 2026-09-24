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

#[test]
fn ordinary_search_proves_ground_unit_equality() {
    // Regression test: BCE once deleted `p(a)` as "blocked" (a and b do not
    // unify, so no binary resolvent exists), ignoring the paramodulation
    // step through `a = b`. Ordinary search then gave up with generated=0.
    // BCE is now skipped whenever equality is present.
    let problem = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("problems/eq_unit_ground.p");
    let output = Command::new(env!("CARGO_BIN_EXE_mrs"))
        .args(["--workers", "1", "--strategy", "1", "--time", "10"])
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
    assert!(
        stdout.contains("% SZS status Theorem for eq_unit_ground"),
        "stdout: {stdout}"
    );
}

#[test]
fn certify_ordered_ground_eq_self_check_certifies() {
    let problem = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("problems/eq_unit_ground.p");
    let output = Command::new(env!("CARGO_BIN_EXE_mrs"))
        .args([
            "--self-check",
            "--certify-ordered",
            "--workers",
            "1",
            "--strategy",
            "1",
            "--time",
            "10",
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

    assert!(
        stdout.contains("% SZS status Theorem for eq_unit_ground"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("equality_normalization"),
        "proof must contain equality_normalization step: {stdout}"
    );
    assert!(
        stdout.contains("% SZS output start Proof for eq_unit_ground"),
        "stdout: {stdout}"
    );
    assert!(stderr.contains("result=Refutation"), "stderr: {stderr}");
    assert!(stderr.contains("self_check=Certified"), "stderr: {stderr}");
    assert!(stderr.contains("cert_idx=1"), "stderr: {stderr}");
}

#[test]
fn ordered_certifier_accepts_ground_sat_and_rejects_equality() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let sat = root.join("problems/certified_epr_sat.p");
    let sat_output = Command::new(env!("CARGO_BIN_EXE_mrs"))
        .args(["--certify-ordered", "--workers", "1", "--strategy", "1"])
        .arg(&sat)
        .output()
        .expect("mrs CLI should run");
    assert!(sat_output.status.success());
    assert!(
        String::from_utf8_lossy(&sat_output.stdout)
            .contains("% SZS status Satisfiable for certified_epr_sat")
    );

    let equality = root.join("problems/eq_simple.p");
    let equality_output = Command::new(env!("CARGO_BIN_EXE_mrs"))
        .args(["--certify-ordered", "--workers", "1", "--strategy", "1"])
        .arg(&equality)
        .output()
        .expect("mrs CLI should run");
    assert!(equality_output.status.success());
    assert!(
        String::from_utf8_lossy(&equality_output.stdout)
            .contains("% SZS status GaveUp for eq_simple")
    );
}

#[test]
fn raw_epr_sat_gives_up_only_after_instgen_falls_through() {
    let problem = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("problems/certified_epr_sat.p");
    let output = Command::new(env!("CARGO_BIN_EXE_mrs"))
        .args(["--workers", "1", "--schedule", "casc_eps", "--time", "1"])
        .env("MRS_NO_BCE", "1")
        .env("MRS_NO_PLE", "1")
        .arg(&problem)
        .output()
        .expect("mrs CLI should run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.contains("% SZS status GaveUp for certified_epr_sat"));
    assert!(stderr.contains("instgen_result=gaveup_variable_model"));
    assert!(stderr.contains("processed="));
}
