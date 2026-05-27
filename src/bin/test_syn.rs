use mrs_core::Formula;
use std::collections::HashMap;
use std::fs;

#[path = "../include.rs"]
mod include;
#[path = "../lowering.rs"]
mod lowering;
#[path = "../sine.rs"]
mod sine;

fn main() {
    let path = "crates/mrs-bench/problems/casc-30/FNE/SYN457+1.p";
    let input = fs::read_to_string(path).unwrap();
    let problem = mrs_tptp::parse_tptp(&input).unwrap();
    let mut lowered = lowering::lower_problem(&problem);

    let mut id_gen = lowered.id_gen;
    let mut all_clauses = Vec::new();

    for f in &lowered.conjectures {
        let negated = Formula::neg(f.formula.clone());
        let clauses = mrs_cnf::clausify(
            &negated,
            &mut lowered.symbols,
            &mut id_gen,
            &f.name,
            "negated_conjecture",
        );
        all_clauses.extend(clauses);
    }

    let mut var_map = HashMap::new();
    let mut next_var = 1;

    let mut solver = varisat::Solver::new();
    use varisat::ExtendFormula;

    for c in &all_clauses {
        let mut sat_clause = Vec::new();
        for lit in &c.literals {
            use mrs_core::display::DisplayWithSymbols;
            let name = format!("{}", lit.atom.display(&lowered.symbols));
            let v = *var_map.entry(name).or_insert_with(|| {
                let id = next_var;
                next_var += 1;
                id
            });
            let var = varisat::Var::from_dimacs(v as isize);
            sat_clause.push(varisat::Lit::from_var(var, lit.positive));
        }
        solver.add_clause(&sat_clause);
    }

    let result = solver.solve().unwrap();
    println!("VARISAT RESULT on purely propositional clauses: {}", result);
}
