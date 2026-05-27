use std::fs;
use mrs_core::Formula;
use mrs_search::state::SearchState;
use std::sync::Arc;

#[path = "src/lowering.rs"]
mod lowering;
#[path = "src/include.rs"]
mod include;
#[path = "src/sine.rs"]
mod sine;

fn main() {
    let path = "crates/mrs-bench/problems/casc-30/FNE/SYO606+1.p";
    let input = fs::read_to_string(path).unwrap();
    let problem = mrs_tptp::parse_tptp(&input).unwrap();
    let mut lowered = lowering::lower_problem(&problem);
    
    let mut id_gen = lowered.id_gen;
    let mut all_clauses = Vec::new();

    for f in &lowered.axioms {
        let clauses = mrs_cnf::clausify(&f.formula, &mut lowered.symbols, &mut id_gen, &f.name, &f.role);
        for c in &clauses {
            println!("AXIOM: {}", c.display(&lowered.symbols));
        }
        all_clauses.extend(clauses);
    }

    for f in &lowered.conjectures {
        let negated = Formula::neg(f.formula.clone());
        let clauses = mrs_cnf::clausify(&negated, &mut lowered.symbols, &mut id_gen, &f.name, "negated_conjecture");
        for c in &clauses {
            println!("CONJ: {}", c.display(&lowered.symbols));
        }
        all_clauses.extend(clauses);
    }
}
