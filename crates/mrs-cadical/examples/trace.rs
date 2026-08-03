use std::path::PathBuf;

use mrs_cadical::{ProofFormat, SolveResult, Solver};

fn main() {
    let path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/cadical-frat.proof"));
    let mut solver = Solver::new();
    solver
        .start_file_trace(&path, ProofFormat::FratLrat)
        .expect("start FRAT trace");
    solver.add_clause(&[1]);
    solver.add_clause(&[-1]);
    assert_eq!(solver.solve(), SolveResult::Unsat);
    solver.close_file_trace();
    println!("{}", path.display());
}
