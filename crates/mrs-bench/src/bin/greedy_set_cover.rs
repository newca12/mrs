//! greedy_set_cover — Select the most complementary portfolio of strategies.
//!
//! Usage:
//!     greedy_set_cover <run.csv> [K] [--division DIVISION]
//!
//! Arguments:
//!     <run.csv>             Path to run.csv produced by casc.sh
//!     [K]                   Portfolio size to select (default: 8 = CASC core count)
//!     -d / --division NAME  Only consider rows for this CASC division
//!                           (e.g. fne, feq, ueq, eps, epu — case-insensitive)
//!
//! Input CSV Schema (produced by casc.sh):
//!     edition,division,problem,system,szs_status,expected,verdict,wall_time_s[,failure_detail]
//!
//! The `system` column is used as the strategy identifier.  To analyse
//! individual mrs strategies (rather than whole solvers), run each strategy
//! as a separate named "system" via the mrs-strategy bench wrapper and feed
//! the combined CSV to this tool:
//!
//!     # Generate per-strategy benchmark data (see run_strategy_sweep.sh):
//!     casc.sh --systems mrs-s01,mrs-s02,...,mrs-s15 --divisions fne --time 30 ...
//!
//!     # Find the 8 most complementary strategies for FNE:
//!     greedy_set_cover run.csv 8 --division fne
//!
//! The algorithm is greedy: at each step it picks the strategy that maximises
//! the number of *newly* covered problems.  Ties are broken alphabetically by
//! strategy name for deterministic output.
//!
//! Why K=8?  CASC competition hardware has exactly 8 cores.  The optimal
//! per-division portfolio for CASC is the 8-strategy set identified by this
//! tool run with K=8 and --division matching the CASC division under study.
//! See AGENTS.md §"CASC Hardware & --casc Decision Rule" for the full workflow.

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

fn print_usage(prog: &str) {
    eprintln!("Usage: {prog} <run.csv> [K] [-d/--division DIVISION]");
    eprintln!("  K defaults to 8 (CASC core count).");
    eprintln!("  --division filters to a single CASC division (e.g. fne, feq, ueq, eps, epu).");
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let prog = args
        .first()
        .map(String::as_str)
        .unwrap_or("greedy_set_cover");

    if args.len() < 2 {
        print_usage(prog);
        std::process::exit(1);
    }

    let mut csv_path: Option<String> = None;
    let mut k: usize = 8;
    let mut filter_division: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-d" | "--division" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --division requires a value");
                    print_usage(prog);
                    std::process::exit(1);
                }
                filter_division = Some(args[i].to_ascii_lowercase());
            }
            "--help" | "-h" => {
                print_usage(prog);
                std::process::exit(0);
            }
            arg if !arg.starts_with('-') => {
                if csv_path.is_none() {
                    csv_path = Some(arg.to_string());
                } else {
                    k = arg.parse().unwrap_or_else(|_| {
                        eprintln!(
                            "error: portfolio size K must be a positive integer, got {arg:?}"
                        );
                        std::process::exit(1);
                    });
                }
            }
            arg => {
                eprintln!("error: unknown argument: {arg}");
                print_usage(prog);
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let csv_path = csv_path.unwrap_or_else(|| {
        print_usage(prog);
        std::process::exit(1);
    });

    let file = File::open(&csv_path)?;
    let reader = BufReader::new(file);

    // Map: strategy → set of solved problems (within the selected division if any).
    let mut strategy_solved: HashMap<String, HashSet<String>> = HashMap::new();
    // All problems solved by at least one strategy in scope.
    let mut all_solved_problems: HashSet<String> = HashSet::new();

    let mut lines = reader.lines();
    let header = match lines.next() {
        Some(h) => h?,
        None => {
            eprintln!("error: CSV file is empty");
            std::process::exit(1);
        }
    };

    let cols: Vec<&str> = header.split(',').map(str::trim).collect();
    let prob_idx = cols.iter().position(|&s| s == "problem");
    let sys_idx = cols.iter().position(|&s| s == "system");
    let status_idx = cols.iter().position(|&s| s == "szs_status");
    let div_idx = cols.iter().position(|&s| s == "division");

    let (p_i, s_i, st_i) = match (prob_idx, sys_idx, status_idx) {
        (Some(p), Some(s), Some(st)) => (p, s, st),
        _ => {
            eprintln!(
                "error: CSV header must contain 'problem', 'system', and 'szs_status' columns."
            );
            std::process::exit(1);
        }
    };

    for line in lines {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() <= p_i || parts.len() <= s_i || parts.len() <= st_i {
            continue;
        }

        // Apply division filter if requested.
        if let Some(ref target) = filter_division {
            match div_idx {
                Some(d_i) if parts.len() > d_i => {
                    if parts[d_i].trim().to_ascii_lowercase() != *target {
                        continue;
                    }
                }
                _ => {} // no division column — don't filter
            }
        }

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

    if strategy_solved.is_empty() {
        if let Some(ref div) = filter_division {
            println!("No solved problems found in division '{div}'.");
        } else {
            println!("No solved problems found in the CSV.");
        }
        return Ok(());
    }

    // Header line.
    if let Some(ref div) = filter_division {
        println!("Division filter : {}", div.to_uppercase());
    }
    println!(
        "Total unique solved problems in scope: {}",
        all_solved_problems.len()
    );
    println!("Portfolio size  : {k}");
    println!("------------------------------------------------------------");

    // ── Greedy Set Cover ─────────────────────────────────────────────────────
    let mut selected_strategies: Vec<String> = Vec::new();
    let mut covered_problems: HashSet<String> = HashSet::new();

    for step in 1..=k {
        let mut best_strategy: Option<String> = None;
        let mut best_new_solves: usize = 0;

        // Sort alphabetically so ties are broken deterministically.
        let mut candidates: Vec<&String> = strategy_solved.keys().collect();
        candidates.sort_unstable();

        for strategy in candidates {
            if selected_strategies.contains(strategy) {
                continue;
            }
            let new_solves = strategy_solved[strategy]
                .difference(&covered_problems)
                .count();
            // Strictly greater: first alphabetically wins on ties.
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
                "Step {:02}: {:<20} | +{:<4} new  | {:<4} / {} covered  ({:.1}%)",
                step,
                strategy,
                best_new_solves,
                total_solved,
                all_solved_problems.len(),
                pct
            );
        } else {
            println!("No further strategies cover new problems. Stopping at step {step}.");
            break;
        }
    }

    println!("------------------------------------------------------------");
    println!(
        "Selected portfolio ({} strategies):",
        selected_strategies.len()
    );
    for (i, s) in selected_strategies.iter().enumerate() {
        println!("  {:2}. {}", i + 1, s);
    }

    Ok(())
}
