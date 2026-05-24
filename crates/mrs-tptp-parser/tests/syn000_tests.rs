//! SYN000 Parser Test Suite
//!
//! This suite tests parsing of the SYN000-sample problems from the TPTP library.
//! A test succeeds if the parser can successfully parse the input and
//! fails otherwise (i.e. if a parse error occurs).
//!
//! Based on the scala-tptp-parser test suite by Alexander Steen.

use mrs_tptp::parse_tptp;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

/// Get the path to the test resources directory
fn get_test_resources_path() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");
    path.push("resources");
    path.push("SYN000");
    path
}

/// Helper function to measure parsing time
fn time_parse(
    input: &str,
) -> (
    std::time::Duration,
    Result<mrs_tptp::TPTPProblem<'_>, String>,
) {
    let start = Instant::now();
    let result = parse_tptp(input).map_err(|e| format!("{:?}", e));
    let duration = start.elapsed();
    (duration, result)
}

/// Parse a test file and report results
fn parse_test_file(filename: &str, description: &str) {
    println!("###################################");
    println!("Parsing test for {} ...", description);
    println!("###################################");

    let path = get_test_resources_path().join(filename);
    print!("Parsing {} ...", filename);

    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read file {}: {}", path.display(), e));

    let (duration, result) = time_parse(&content);

    match result {
        Ok(problem) => {
            println!("done ({}ms).", duration.as_millis());
            println!(
                "Parsed {} formulae and {} include statements.",
                problem.formulas.len(),
                problem.includes.len()
            );
        }
        Err(e) => {
            println!("\nParse error: {}", e);
            panic!("Failed to parse {}", filename);
        }
    }
}

// SYN000 Test Cases - CNF
#[test]
fn test_syn000_cnf_basic() {
    parse_test_file("SYN000-1.p", "TPTP CNF basic syntax features");
}

#[test]
fn test_syn000_cnf_advanced() {
    parse_test_file("SYN000-2.p", "TPTP CNF advanced syntax features");
}

// SYN000 Test Cases - TCF
#[test]
fn test_syn000_tcf_basic() {
    parse_test_file("SYN000-1-TCF.p", "TPTP TCF basic syntax (improvised)");
}

#[test]
fn test_syn000_tcf_advanced() {
    parse_test_file("SYN000-2-TCF.p", "TPTP TCF advanced syntax (improvised)");
}

// SYN000 Test Cases - FOF
#[test]
fn test_syn000_fof_basic() {
    parse_test_file("SYN000+1.p", "TPTP FOF basic syntax features");
}

#[test]
fn test_syn000_fof_advanced() {
    parse_test_file("SYN000+2.p", "TPTP FOF advanced syntax features");
}

// SYN000 Test Cases - TFF
#[test]
fn test_syn000_tff_basic() {
    parse_test_file("SYN000_1.p", "TPTP TF0 basic syntax features");
}

#[test]
fn test_syn000_tff_advanced() {
    parse_test_file("SYN000_2.p", "TPTP TF0 advanced syntax features");
}

#[test]
fn test_syn000_tf1_syntax() {
    parse_test_file("SYN000_3.p", "TPTP TF1 syntax features");
}

#[test]
fn test_syn000_tfx_syntax() {
    parse_test_file("SYN000_4.p", "TPTP TFX syntax features");
}

// SYN000 Test Cases - THF
#[test]
fn test_syn000_thf_basic() {
    parse_test_file("SYN000^1.p", "TPTP THF basic syntax features");
}

#[test]
fn test_syn000_thf_advanced() {
    parse_test_file("SYN000^2.p", "TPTP THF advanced syntax features");
}

#[test]
fn test_syn000_th1_syntax() {
    parse_test_file("SYN000^3.p", "TPTP TH1 syntax features");
}

// SYN000 Test Cases - TFA (with arithmetic)
#[test]
fn test_syn000_tfa_arithmetic() {
    parse_test_file(
        "SYN000=2.p",
        "TPTP TFA with arithmetic advanced syntax features",
    );
}

// SYN000 Test Cases - Modal/Non-classical
#[test]
fn test_syn000_modal_thf() {
    parse_test_file("SYN000~1.p", "Modal THF format with logic specification");
}
