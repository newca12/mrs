use mrs_core::Formula;
use mrs_core::display::DisplayWithSymbols;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[path = "../include.rs"]
mod include;
#[path = "../lowering.rs"]
mod lowering;
#[path = "../sine.rs"]
mod sine;

fn main() {
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "crates/mrs-bench/problems/casc-30/FNE/SYN457+1.p".to_string());

    let input = fs::read_to_string(&path).expect("Failed to read problem file");
    let problem = mrs_tptp::parse_tptp(&input).expect("Failed to parse problem");
    let mut lowered = lowering::lower_problem(&problem);

    // Resolve includes
    if !problem.includes.is_empty() {
        let base_dir = Path::new(&path).parent().unwrap_or(Path::new("."));
        let tptp_root: Option<PathBuf> = env::var("TPTP").ok().map(PathBuf::from);
        match include::resolve_and_lower(&problem, &mut lowered, base_dir, tptp_root.as_deref()) {
            Ok(()) => eprintln!("Resolved {} include(s)", problem.includes.len()),
            Err(e) => eprintln!("Include warning: {}", e),
        }
    }

    let mut id_gen = lowered.id_gen;

    println!("--- Axioms ({}) ---", lowered.axioms.len());
    for f in &lowered.axioms {
        let clauses = mrs_cnf::clausify(
            &f.formula,
            &mut lowered.symbols,
            &mut id_gen,
            &f.name,
            &f.role,
        );
        for c in &clauses {
            println!("[{}] {}", f.name, c.display(&lowered.symbols));
        }
    }

    println!(
        "--- Negated Conjectures ({}) ---",
        lowered.conjectures.len()
    );
    for f in &lowered.conjectures {
        let negated = Formula::neg(f.formula.clone());
        let clauses = mrs_cnf::clausify(
            &negated,
            &mut lowered.symbols,
            &mut id_gen,
            &f.name,
            "negated_conjecture",
        );
        println!(
            "  ({} clauses from negated conjecture '{}')",
            clauses.len(),
            f.name
        );
        for c in &clauses {
            println!("[neg_{}] {}", f.name, c.display(&lowered.symbols));
        }
    }
}
