//! greedy_set_cover — Select the most complementary portfolio of strategies.
//!
//! Usage:
//!     greedy_set_cover <run.csv> [portfolio_size_K]
//!
//! Input CSV Schema (produced by casc.sh):
//!     edition,division,problem,system,szs_status,expected,verdict,wall_time_s[,failure_detail]
//!
//! The `system` column is used as the strategy identifier.  To analyse
//! individual mrs strategies rather than whole solvers, run each strategy
//! as a separate named "system" by setting MRS_SINGLE_STRATEGY=N and
//! naming the invocation wrapper accordingly.  Then feed the combined
//! CSV to this tool.
//!
//! Example — comparing multi-system portfolios:
//!     casc.sh --systems mrs,eprover,vampire ...   # produces run.csv
//!     greedy_set_cover run.csv 3                  # best 3-system portfolio
//!
//! The algorithm is greedy: at each step it picks the strategy that
//! maximises the number of *newly* covered problems.  Ties are broken
//! alphabetically by strategy name for deterministic output.
//!
//! Output:
//!     One line per selected strategy showing how many new problems it
//!     added, the cumulative covered count, and the running percentage.
//!     The final block lists the selected portfolio in order.

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

    // Map: strategy → Set of solved problems
    let mut strategy_solved: HashMap<String, HashSet<String>> = HashMap::new();
    // Set of all problems solved by at least one strategy
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
                            .or_default()
                            .insert(problem.clone());
                        all_solved_problems.insert(problem);
                    }
                }
            }
        } else {
            eprintln!(
                "Error: CSV header must contain 'problem', 'system', and 'szs_status' columns."
            );
            std::process::exit(1);
        }
    }

    if strategy_solved.is_empty() {
        println!("No solved problems found in the CSV.");
        return Ok(());
    }

    println!(
        "Total unique solved problems in dataset: {}",
        all_solved_problems.len()
    );
    println!("------------------------------------------------------------");

    // Greedy Set Cover
    let mut selected_strategies: Vec<String> = Vec::new();
    let mut covered_problems: HashSet<String> = HashSet::new();

    for step in 1..=k {
        let mut best_strategy: Option<String> = None;
        let mut best_new_solves: usize = 0;

        // Collect and sort by name for deterministic tiebreaking.
        let mut candidates: Vec<&String> = strategy_solved.keys().collect();
        candidates.sort();

        for strategy in candidates {
            if selected_strategies.contains(strategy) {
                continue;
            }
            let new_solves = strategy_solved[strategy]
                .difference(&covered_problems)
                .count();
            // Strictly greater: ties broken by alphabetical order from the
            // sorted iteration above (first alphabetically wins).
            if new_solves > best_new_solves {
                best_new_solves = new_solves;
                best_strategy = Some(strategy.clone());
            }
        }

        if let Some(strategy) = best_strategy {
            if let Some(solved) = strategy_solved.get(&strategy) {
                covered_problems.extend(solved.iter().cloned());
            }
            selected_strategies.push(strategy.clone());

            let total_solved = covered_problems.len();
            let pct = (total_solved as f64 / all_solved_problems.len() as f64) * 100.0;
            println!(
                "Step {:02}: Pick {:<15} | Unique solves added: {:<4} | \
                 Cumulative solved: {:<4} ({:.2}%)",
                step, strategy, best_new_solves, total_solved, pct
            );
        } else {
            println!("No further strategies can cover any new problems. Stopping.");
            break;
        }
    }

    println!("------------------------------------------------------------");
    println!(
        "Optimal complementary portfolio of size {}:",
        selected_strategies.len()
    );
    for (i, s) in selected_strategies.iter().enumerate() {
        println!("  {}. {}", i + 1, s);
    }

    Ok(())
}
