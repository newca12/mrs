//! Explicit opt-in command-line shadow check. The strict kernel remains authoritative.
use mrs_proof_kernel::VerificationLimits;
use mrs_proof_kernel::verus::{VerusConfig, verify_with_shadow, verify_with_verus_gate};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 5 || !matches!(args[0].as_str(), "shadow" | "gate") {
        return Err("usage: verus_shadow shadow|gate CHECKER SHA256 PROBLEM PROOF".into());
    }
    let problem = std::fs::read_to_string(&args[3])?;
    let proof = std::fs::read_to_string(&args[4])?;
    let parsed_problem = mrs_tptp::parse_tptp(&problem)?;
    let parsed_proof = mrs_tptp::parse_tptp(&proof)?;
    let config = VerusConfig {
        executable: args[1].clone().into(),
        expected_sha256: args[2].clone(),
        timeout: Duration::from_secs(10),
    };
    let verify = if args[0] == "gate" {
        verify_with_verus_gate
    } else {
        verify_with_shadow
    };
    let (kernel, shadow) = verify(
        &parsed_problem,
        &parsed_proof,
        mrs_tptp::proover::proof_header_link(&proof),
        VerificationLimits::default(),
        &config,
    );
    println!("strict_kernel={kernel}\nverus_shadow={shadow:?}");
    Ok(())
}
