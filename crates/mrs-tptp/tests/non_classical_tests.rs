//! Non-Classical Logic Parser Test Suite
//!
//! This suite tests parsing of non-classical logic problems from the TPTP library.
//! Based on the scala-tptp-parser TPTPNCLTestSuite.
//!
//! Supports:
//! - Modal logic specifications: $modal == [...], $alethic_modal == [...], $epistemic_modal == [...]
//! - Short box/diamond connectives: [.], <.>, [#name], <#name>
//! - Alternative short connectives: /.\\, \\./
//! - Long connectives: {$box}, {#dia}, {$knows(#agent)}

use mrs_tptp::parse_tptp;
use std::fs;
use std::path::PathBuf;

/// Get the path to the non-classical test resources directory
fn get_non_classical_resources_path() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");
    path.push("resources");
    path.push("non-classical");
    path
}

/// Parse a non-classical test file and print the result
fn parse_and_print(filename: &str) {
    let path = get_non_classical_resources_path().join(filename);

    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read file {}: {}", path.display(), e));

    match parse_tptp(&content) {
        Ok(problem) => {
            println!("Successfully parsed {}", filename);
            println!("  Formulae: {}", problem.formulas.len());
            println!("  Includes: {}", problem.includes.len());
        }
        Err(e) => {
            panic!("Parse error in {}: {:?}", filename, e);
        }
    }
}

#[test]
fn test_ncl_correct_specifications() {
    parse_and_print("CorrectSpecifications.p");
}

#[test]
fn test_ncl_krs272_1() {
    parse_and_print("KRS272~1.p");
}

#[test]
fn test_ncl_lcl870_1() {
    parse_and_print("LCL870#1.p");
}

#[test]
fn test_ncl_lcl871_1() {
    parse_and_print("LCL871#1.p");
}

#[test]
fn test_ncl_lcl871_cone() {
    parse_and_print("LCL871-cone.p");
}

#[test]
fn test_ncl_puz087_1_hash() {
    parse_and_print("PUZ087#1.p");
}

#[test]
fn test_ncl_puz087_1_underscore() {
    parse_and_print("PUZ087_1.p");
}

#[test]
fn test_ncl_puz087_1_tilde() {
    parse_and_print("PUZ087~1.p");
}

#[test]
fn test_ncl_puz087_2_hash() {
    parse_and_print("PUZ087#2.p");
}

#[test]
fn test_ncl_puz087_2_underscore() {
    parse_and_print("PUZ087_2.p");
}

#[test]
fn test_ncl_puz087_2_tilde() {
    parse_and_print("PUZ087~2.p");
}

#[test]
fn test_ncl_puz087_3_underscore() {
    parse_and_print("PUZ087_3.p");
}

#[test]
fn test_ncl_puz087_3_tilde() {
    parse_and_print("PUZ087~3.p");
}

#[test]
fn test_ncl_puz149_1_hash() {
    parse_and_print("PUZ149#1.p");
}

#[test]
fn test_ncl_puz149_1_tilde() {
    parse_and_print("PUZ149~1.p");
}
