//! MRS - Mechanical Reasoning System
//!
//! An automated theorem prover targeting the CASC competition.

mod include;
mod lowering;
mod sine;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::time::Instant;

use std::time::Duration;

use mrs_core::Formula;
use mrs_core::clause::Clause;
use mrs_search::SearchResult;
use mrs_search::strategy::{StrategySchedule, run_schedule};
use mrs_szs::{SzsStatus, szs_output_end, szs_output_start, szs_status_line};

fn main() {
    let start = Instant::now();

    let mut path: Option<String> = None;
    let mut time_secs: u64 = 30;
    let mut schedule_name: Option<String> = None;
    let mut log_ml_data: Option<String> = None;
    let mut ml_log_csv = false;
    let mut ml_weights: Option<String> = None;
    let mut workers: Option<usize> = None;

    #[cfg(feature = "proover")]
    let mut quiet = false;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--time" => {
                let val = args.next().unwrap_or_else(|| {
                    eprintln!("Usage: mrs [--time <seconds>] <file.p>");
                    process::exit(1);
                });
                time_secs = val.parse().unwrap_or_else(|_| {
                    eprintln!("Error: --time requires a positive integer, got {:?}", val);
                    process::exit(1);
                });
            }
            "--workers" => {
                let val = args.next().unwrap_or_else(|| {
                    eprintln!("Usage: mrs [--workers <N>] <file.p>");
                    process::exit(1);
                });
                workers = Some(val.parse().unwrap_or_else(|_| {
                    eprintln!(
                        "Error: --workers requires a positive integer, got {:?}",
                        val
                    );
                    process::exit(1);
                }));
            }
            "--schedule" => {
                let val = args.next().unwrap_or_else(|| {
                    eprintln!(
                        "Error: --schedule requires a name (one of: {})",
                        mrs_search::strategy::named::ALL.join(", ")
                    );
                    process::exit(1);
                });
                schedule_name = Some(val);
            }
            "--log-ml-data" => {
                let val = args.next().unwrap_or_else(|| {
                    eprintln!("Error: --log-ml-data requires a directory path");
                    process::exit(1);
                });
                log_ml_data = Some(val);
            }
            "--ml-log-csv" => {
                ml_log_csv = true;
            }
            "--ml-weights" => {
                let val = args.next().unwrap_or_else(|| {
                    eprintln!("Error: --ml-weights requires a file path");
                    process::exit(1);
                });
                ml_weights = Some(val);
                // If ml weights are provided but no schedule is selected, default to the `ml` schedule.
                if schedule_name.is_none() {
                    schedule_name = Some("ml".to_string());
                }
            }
            // Deprecated alias: --fast is now --schedule fast.
            "--fast" => {
                schedule_name = Some("fast".to_string());
            }
            "--list-schedules" => {
                for name in mrs_search::strategy::named::ALL {
                    println!("{name}");
                }
                process::exit(0);
            }
            #[cfg(feature = "proover")]
            "--quiet" => quiet = true,
            _ => {
                if path.is_some() {
                    eprintln!("Usage: mrs [--time <seconds>] [--schedule NAME] <file.p>");
                    process::exit(1);
                }
                path = Some(arg);
            }
        }
    }
    let Some(path) = path else {
        eprintln!("Usage: mrs [--time <seconds>] [--schedule NAME] <file.p>");
        eprintln!("  An automated theorem prover for TPTP problems.");
        eprintln!(
            "  Schedules: {} (default: casc)",
            mrs_search::strategy::named::ALL.join(", ")
        );
        process::exit(1);
    };

    // Helper macro: print informational stderr unless --quiet is in effect.
    // In default builds, `quiet` does not exist, so the macro reduces to a
    // plain `eprintln!`.
    #[cfg(feature = "proover")]
    macro_rules! info {
        ($($arg:tt)*) => { if !quiet { eprintln!($($arg)*); } };
    }
    #[cfg(not(feature = "proover"))]
    macro_rules! info {
        ($($arg:tt)*) => { eprintln!($($arg)*); };
    }

    let problem_name = if path == "-" {
        "stdin"
    } else {
        Path::new(&path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
    };

    // Read the input. With the `proover` feature, `-` means stdin.
    let input = if path == "-" {
        #[cfg(feature = "proover")]
        {
            use std::io::Read;
            let mut buf = String::new();
            if let Err(e) = std::io::stdin().read_to_string(&mut buf) {
                eprintln!("Error reading stdin: {}", e);
                println!("{}", szs_status_line(SzsStatus::Error, problem_name));
                process::exit(1);
            }
            buf
        }
        #[cfg(not(feature = "proover"))]
        {
            eprintln!("Error: `-` (stdin) requires the `proover` feature");
            println!("{}", szs_status_line(SzsStatus::Error, problem_name));
            process::exit(1);
        }
    } else {
        match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error reading {}: {}", path, e);
                println!("{}", szs_status_line(SzsStatus::Error, problem_name));
                process::exit(1);
            }
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

        // Use $TPTP as a hint for the root directory.  Even if it is wrong
        // (e.g. pointing at Problems/ instead of TPTP-v9.2.1/), resolve_path
        // will also auto-detect the root by walking up from base_dir looking
        // for an ancestor that contains Axioms/.
        let tptp_root: Option<PathBuf> = env::var("TPTP").ok().map(PathBuf::from);

        match include::resolve_and_lower(&problem, &mut lowered, base_dir, tptp_root.as_deref()) {
            Ok(()) => {
                info!("% Resolved {} include directive(s)", problem.includes.len());
            }
            Err(e) => {
                info!("Warning: include resolution failed: {}", e);
            }
        }
    }

    let has_conjecture = !lowered.conjectures.is_empty();

    // --- SInE Filtering ---
    // Backup formulas in case SInE over-prunes and we need to retry.
    let backup_axioms = lowered.axioms.clone();
    let backup_conjectures = lowered.conjectures.clone();
    let backup_cnf_clauses = lowered.cnf_clauses.clone();
    let backup_id_gen = lowered.id_gen.clone();

    // In problems with massive axiomatizations, use SInE to filter.
    // If there are more than 100 axioms, try filtering.
    let mut sine_triggered = false;
    if lowered.axioms.len() + lowered.cnf_clauses.len() > 100 {
        let tolerance = 2.0;
        let depth_limit = Some(5);

        let before_axioms = lowered.axioms.len();
        let before_cnf = lowered.cnf_clauses.len();

        let mut all_items: Vec<sine::SineItemWrapper> = Vec::new();
        for axiom in lowered.axioms {
            all_items.push(sine::SineItemWrapper::Formula(axiom));
        }
        for conj in lowered.conjectures {
            all_items.push(sine::SineItemWrapper::Formula(conj));
        }
        for clause in lowered.cnf_clauses {
            all_items.push(sine::SineItemWrapper::Clause(clause));
        }

        let filtered = sine::filter_items(&all_items, tolerance, depth_limit);

        if filtered.len() < all_items.len() {
            sine_triggered = true;
        }

        lowered.axioms = Vec::new();
        lowered.conjectures = Vec::new();
        lowered.cnf_clauses = Vec::new();

        for item in filtered {
            match item {
                sine::SineItemWrapper::Formula(lf) => {
                    if lf.role == "conjecture" || lf.role == "negated_conjecture" {
                        lowered.conjectures.push(lf);
                    } else {
                        lowered.axioms.push(lf);
                    }
                }
                sine::SineItemWrapper::Clause(c) => lowered.cnf_clauses.push(c),
            }
        }

        info!(
            "% SInE filtered axioms: {} -> {}, cnf: {} -> {}",
            before_axioms,
            lowered.axioms.len(),
            before_cnf,
            lowered.cnf_clauses.len()
        );
    }

    // Display input summary
    let cnf_count = lowered.cnf_clauses.len();
    info!(
        "% Problem: {} ({} axioms, {} conjectures, {} cnf clauses)",
        problem_name,
        lowered.axioms.len(),
        lowered.conjectures.len(),
        cnf_count
    );

    let total_budget = Duration::from_secs(time_secs);
    let mut final_result = SearchResult::GaveUp;
    let mut final_status = SzsStatus::GaveUp;

    // We run the search up to 2 times: once with SInE (if triggered), and once without if it saturated prematurely.
    let mut attempt = 0;
    while attempt < 2 {
        attempt += 1;

        // --- Clausification ---
        let mut id_gen = lowered.id_gen.clone();
        let mut all_clauses: Vec<Clause> = lowered
            .cnf_clauses
            .clone()
            .into_iter()
            .map(|c| c.with_distance(100))
            .collect();

        // Clausify axioms directly
        for f in &lowered.axioms {
            let clauses = mrs_cnf::clausify(
                &f.formula,
                &mut lowered.symbols,
                &mut id_gen,
                &f.name,
                &f.role,
            );
            all_clauses.extend(clauses.into_iter().map(|c| c.with_distance(100)));
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
            all_clauses.extend(clauses.into_iter().map(|c| c.with_distance(0)));
        }

        let elapsed = start.elapsed();
        if elapsed >= total_budget {
            final_status = SzsStatus::Timeout;
            final_result = SearchResult::Timeout;
            break;
        }

        let actual_workers = workers.unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1)
        });

        let search_budget = total_budget - elapsed;
        let schedule = match schedule_name.as_deref() {
            None => StrategySchedule::default_schedule(search_budget, actual_workers),
            Some(name) => {
                match mrs_search::strategy::named::by_name(name, search_budget, actual_workers) {
                    Some(s) => s,
                    None => {
                        eprintln!(
                            "Error: unknown schedule {:?} (known: {})",
                            name,
                            mrs_search::strategy::named::ALL.join(", "),
                        );
                        process::exit(1);
                    }
                }
            }
        };

        let search_start = std::time::Instant::now();
        let result = run_schedule(
            &all_clauses,
            id_gen,
            &schedule,
            &lowered.symbols,
            mrs_search::strategy::MlOptions {
                log_dir: log_ml_data.clone(),
                log_csv: ml_log_csv,
                weights: ml_weights.clone(),
            },
            workers,
        );
        let search_elapsed = search_start.elapsed();

        let status = match &result {
            SearchResult::Refutation(..) => {
                if has_conjecture {
                    SzsStatus::Theorem
                } else {
                    SzsStatus::Unsatisfiable
                }
            }
            SearchResult::Saturated => {
                if sine_triggered {
                    // If SInE dropped axioms, saturation is incomplete for the full problem.
                    SzsStatus::GaveUp
                } else if has_conjecture {
                    SzsStatus::CounterSatisfiable
                } else {
                    SzsStatus::Satisfiable
                }
            }
            SearchResult::Timeout => SzsStatus::Timeout,
            SearchResult::GaveUp => SzsStatus::GaveUp,
        };

        final_result = result;
        final_status = status;

        // SInE Fallback check
        if attempt == 1
            && sine_triggered
            && matches!(final_status, SzsStatus::GaveUp)
            && search_elapsed < Duration::from_secs(1)
        {
            info!(
                "% SInE over-pruning suspected (saturated in {:.3}s). Restarting without SInE.",
                search_elapsed.as_secs_f64()
            );
            sine_triggered = false;
            lowered.axioms = backup_axioms.clone();
            lowered.conjectures = backup_conjectures.clone();
            lowered.cnf_clauses = backup_cnf_clauses.clone();
            lowered.id_gen = backup_id_gen.clone();
            continue;
        }

        // Otherwise, break out of loop
        break;
    }

    let status = final_status;
    let result = final_result;

    println!("{}", szs_status_line(status, problem_name));

    // Output proof if refutation found (skip in quiet mode: mrs-proover only
    // cares about the SZS line).
    #[cfg(feature = "proover")]
    let emit_extras = !quiet;
    #[cfg(not(feature = "proover"))]
    let emit_extras = true;

    if emit_extras {
        if let SearchResult::Refutation(_, tstp_proof) = &result {
            println!("{}", szs_output_start("Proof", problem_name));
            println!("{}", tstp_proof);
            println!("{}", szs_output_end("Proof", problem_name));
        }

        print_statistics(status, start.elapsed());
    }
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
        SzsStatus::Timeout | SzsStatus::ResourceOut => "Timeout",
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
