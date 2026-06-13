//! greedy_set_cover — Select the most complementary portfolio of strategies.
//!
//! Usage:
//!     greedy_set_cover <run.csv> [portfolio_size_K]
//!
//! Input CSV Schema:
//!     edition,division,problem,system,szs_status,expected,verdict,wall_time_s
//!
//! Used to find the optimal combination of strategies that maximizes overall
//! problem coverage by selecting strategies with the highest unique solve counts.

use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader};

const SOLVED_STATUSES: &[&str] = &[
    "Theorem",
    "Unsatisfiable",
    "CounterSatisfiable",
    "Satisfiable",
];

fn is_solved(status: &str) -> bool {
    SOLVED_STATUSES.contains(&status)
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: greedy_set_cover <run.csv> [portfolio_size_K]");
        std::process::exit(1);
    }

    let csv_path = &args[1];
    let k: usize = if args.len() >= 3 {
        args[2].parse().unwrap_or(8)
    } else {
        8
    };

    let file = File::open(csv_path)?;
    let reader = BufReader::new(file);

    // Map: strategy -> Set of solved problems
    let mut strategy_solved: HashMap<String, HashSet<String>> = HashMap::new();
    // Set of all solved problems overall
    let mut all_solved_problems: HashSet<String> = HashSet::new();

    let mut lines = reader.lines();
    if let Some(header) = lines.next() {
        let header = header?;
        let cols: Vec<&str> = header.split(',').collect();
        let prob_idx = cols.iter().position(|&s| s == "problem");
        let sys_idx = cols.iter().position(|&s| s == "system");
        let status_idx = cols.iter().position(|&s| s == "szs_status");

        if let (Some(p_i), Some(s_i), Some(st_i)) = (prob_idx, sys_idx, status_idx) {
            for line in lines {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                let parts: Vec<&str> = line.split(',').collect();
                if parts.len() > p_i && parts.len() > s_i && parts.len() > st_i {
                    let problem = parts[p_i].trim().to_string();
                    let strategy = parts[s_i].trim().to_string();
                    let status = parts[st_i].trim();

                    if is_solved(status) {
                        strategy_solved
                            .entry(strategy)
                            .or_insert_with(HashSet::new)
                            .insert(problem.clone());
                        all_solved_problems.insert(problem);
                    }
                }
            }
        } else {
            eprintln!("Error: CSV header must contain 'problem', 'system', and 'szs_status' columns.");
            std::process::exit(1);
        }
    }

    if strategy_solved.is_empty() {
        println!("No solved problems found in the CSV.");
        return Ok(());
    }

    println!("Total unique solved problems in dataset: {}", all_solved_problems.len());
    println!("------------------------------------------------------------");

    // Greedy Set Cover
    let mut selected_strategies: Vec<String> = Vec::new();
    let mut covered_problems: HashSet<String> = HashSet::new();

    for step in 1..=k {
        let mut best_strategy: Option<String> = None;
        let mut best_new_solves: usize = 0;

        for (strategy, solved) in &strategy_solved {
            if selected_strategies.contains(strategy) {
                continue;
            }
            // Count how many new problems this strategy would cover
            let new_solves = solved.difference(&covered_problems).count();
            if new_solves > best_new_solves {
                best_new_solves = new_solves;
                best_strategy = Some(strategy.clone());
            }
        }

        if let Some(strategy) = best_strategy {
            // Update covered set
            if let Some(solved) = strategy_solved.get(&strategy) {
                for prob in solved {
                    covered_problems.insert(prob.clone());
                }
            }
            selected_strategies.push(strategy.clone());

            let total_solved = covered_problems.len();
            let pct = (total_solved as f64 / all_solved_problems.len() as f64) * 100.0;
            println!(
                "Step {:02}: Pick {:<15} | Unique solves added: {:<4} | Cumulative solved: {:<4} ({:.2}%)",
                step, strategy, best_new_solves, total_solved, pct
            );
        } else {
            println!("No further strategies can cover any new problems. Stopping cover.");
            break;
        }
    }

    println!("------------------------------------------------------------");
    println!("Optimal complementary portfolio of size {}:", selected_strategies.len());
    for (i, s) in selected_strategies.iter().enumerate() {
        println!("  {}. {}", i + 1, s);
    }

    Ok(())
}
