//! MRS - Mechanical Reasoning System
//!
//! An automated theorem prover targeting the CASC competition.

mod include;
mod lowering;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::time::Instant;

use std::time::Duration;

use mrs_core::Formula;
use mrs_core::clause::Clause;
use mrs_proof::extract::extract_proof;
use mrs_proof::tstp::format_tstp;
use mrs_search::SearchResult;
use mrs_search::strategy::{StrategySchedule, run_schedule};
use mrs_szs::{SzsStatus, szs_output_end, szs_output_start, szs_status_line};

fn main() {
    let start = Instant::now();

    let Some(path) = env::args().nth(1) else {
        eprintln!("Usage: mrs <file.p>");
        eprintln!("  An automated theorem prover for TPTP problems.");
        process::exit(1);
    };

    let problem_name = Path::new(&path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    // Read the file
    let input = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading {}: {}", path, e);
            println!("{}", szs_status_line(SzsStatus::Error, problem_name));
            process::exit(1);
        }
    };

    // Parse with the TPTP parser
    let problem = match mrs_tptp::parse_tptp(&input) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            println!("{}", szs_status_line(SzsStatus::Error, problem_name));
            process::exit(1);
        }
    };

    // Lower to core types
    let mut lowered = lowering::lower_problem(&problem);

    // Resolve include directives
    if !problem.includes.is_empty() {
        let base_dir = Path::new(&path).parent().unwrap_or(Path::new("."));

        // Prefer $TPTP env var; otherwise walk up from the problem directory
        // looking for the TPTP root (the first ancestor that contains Axioms/).
        let tptp_root: Option<PathBuf> = env::var("TPTP").ok().map(PathBuf::from).or_else(|| {
            let mut dir = base_dir.to_path_buf();
            loop {
                if dir.join("Axioms").is_dir() {
                    return Some(dir);
                }
                if !dir.pop() {
                    break;
                }
            }
            None
        });

        match include::resolve_and_lower(&problem, &mut lowered, base_dir, tptp_root.as_deref()) {
            Ok(()) => {
                eprintln!("% Resolved {} include directive(s)", problem.includes.len());
            }
            Err(e) => {
                eprintln!("Warning: include resolution failed: {}", e);
            }
        }
    }

    let has_conjecture = !lowered.conjectures.is_empty();

    // Display input summary
    let cnf_count = lowered.cnf_clauses.len();
    eprintln!(
        "% Problem: {} ({} axioms, {} conjectures, {} cnf clauses)",
        problem_name,
        lowered.axioms.len(),
        lowered.conjectures.len(),
        cnf_count
    );

    // --- Clausification ---
    let mut id_gen = lowered.id_gen;
    let mut all_clauses: Vec<Clause> = lowered.cnf_clauses;

    // Clausify axioms directly
    for f in &lowered.axioms {
        let clauses = mrs_cnf::clausify(
            &f.formula,
            &mut lowered.symbols,
            &mut id_gen,
            &f.name,
            &f.role,
        );
        all_clauses.extend(clauses);
    }

    // Negate conjectures for refutation-based proving:
    // To prove P, we show that axioms ∧ ¬P is unsatisfiable.
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

    // --- Proof search ---
    let schedule = StrategySchedule::default_schedule(Duration::from_secs(30));
    let (result, state) = run_schedule(&all_clauses, id_gen, &schedule);

    // --- Output result ---
    let status = match &result {
        SearchResult::Refutation(_) => {
            if has_conjecture {
                SzsStatus::Theorem
            } else {
                SzsStatus::Unsatisfiable
            }
        }
        SearchResult::Saturated => {
            if has_conjecture {
                SzsStatus::CounterSatisfiable
            } else {
                SzsStatus::Satisfiable
            }
        }
        SearchResult::Timeout => SzsStatus::Timeout,
        SearchResult::ResourceOut => SzsStatus::ResourceOut,
    };

    println!("{}", szs_status_line(status, problem_name));

    // Output proof if refutation found
    if let SearchResult::Refutation(empty_id) = result {
        let proof = extract_proof(empty_id, &state.clause_store);
        let tstp = format_tstp(&proof, &lowered.symbols);
        println!("{}", szs_output_start("Proof", problem_name));
        println!("{}", tstp);
        println!("{}", szs_output_end("Proof", problem_name));
    }

    print_statistics(status, start.elapsed());
}

/// Returns peak virtual memory in MB by reading /proc/self/status (Linux only).
fn peak_memory_mb() -> Option<u64> {
    let content = fs::read_to_string("/proc/self/status").ok()?;
    for line in content.lines() {
        if line.starts_with("VmPeak:") {
            let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
            return Some(kb / 1024);
        }
    }
    None
}

/// Prints a Vampire-style statistics block to stdout.
fn print_statistics(status: SzsStatus, elapsed: Duration) {
    let termination_reason = match status {
        SzsStatus::Theorem | SzsStatus::Unsatisfiable => "Refutation",
        SzsStatus::CounterSatisfiable | SzsStatus::Satisfiable => "Saturation",
        SzsStatus::Timeout => "Timeout",
        SzsStatus::ResourceOut => "ResourceOut",
        SzsStatus::GaveUp => "GaveUp",
        SzsStatus::Unknown | SzsStatus::Error => "Error",
    };
    println!("% ------------------------------");
    println!("% Version: mrs {}", env!("CARGO_PKG_VERSION"));
    println!("% Termination reason: {}", termination_reason);
    println!("% Time elapsed: {:.3} s", elapsed.as_secs_f64());
    if let Some(mb) = peak_memory_mb() {
        println!("% Peak memory usage: {} MB", mb);
    }
    println!("% ------------------------------");
}
