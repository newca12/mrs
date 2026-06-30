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
use mrs_core::clause::{Clause, ClauseSource};
use mrs_search::strategy::{StrategySchedule, run_schedule};
use mrs_search::{ScheduleReport, SearchResult};
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
    let mut ml_schedule = false;
    let mut ml_prune_ratio: Option<f32> = None;

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
            "--ml-schedule" => {
                ml_schedule = true;
            }
            "--ml-prune" => {
                let val = args.next().unwrap_or_else(|| {
                    eprintln!("Error: --ml-prune requires a ratio float (e.g. 0.6)");
                    process::exit(1);
                });
                ml_prune_ratio = Some(val.parse().unwrap_or_else(|_| {
                    eprintln!("Error: --ml-prune requires a float, got {:?}", val);
                    process::exit(1);
                }));
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

    // SInE is now performed per portfolio strategy in parallel (with threshold tuning),
    // so we do not run a single global pre-filter on LoweredFormulas anymore.
    let total_budget = Duration::from_secs(time_secs);

    // Display input summary
    let cnf_count = lowered.cnf_clauses.len();
    info!(
        "% Problem: {} ({} axioms, {} conjectures, {} cnf clauses)",
        problem_name,
        lowered.axioms.len(),
        lowered.conjectures.len(),
        cnf_count
    );

    // --- Clausification ---
    let mut id_gen = lowered.id_gen.clone();
    let mut all_clauses: Vec<Clause> = lowered
        .cnf_clauses
        .clone()
        .into_iter()
        .map(|c| {
            // CNF clauses with negated_conjecture role are already the
            // negated goal: give them distance=0 so SOS/GoalDirected
            // heuristics treat them as goal-connected.
            let is_nc = matches!(
                &c.source,
                ClauseSource::Input { role, .. } if role == "negated_conjecture"
            );
            c.with_distance(if is_nc { 0 } else { 100 })
        })
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

    #[cfg(feature = "ml")]
    {
        if let Some(log_dir) = &log_ml_data {
            use mrs_core::ml::schedule_classifier::extract_schedule_features;
            use mrs_core::term_bank::TermBank;
            let mut bank = TermBank::new();
            let mut id_clauses = Vec::with_capacity(all_clauses.len());
            for c in &all_clauses {
                id_clauses.push(bank.clause_from_legacy(c));
            }
            let feats = extract_schedule_features(&id_clauses, &bank, &lowered.symbols);
            let sample = mrs_core::ml::sample::ScheduleSample {
                label_idx: 0, // Mapped during offline training
                feats,
            };
            let log_path = std::path::Path::new(log_dir);
            std::fs::create_dir_all(log_path).ok();
            let file_stem = format!("{}_schedule", problem_name);
            if ml_log_csv {
                if let Ok(mut w) = std::fs::File::create(log_path.join(format!("{}.csv", file_stem))) {
                    use std::io::Write;
                    let feats_str = feats.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(",");
                    let _ = writeln!(w, "0,{}", feats_str);
                }
            } else {
                if let Ok(mut w) = std::fs::File::create(log_path.join(format!("{}.wincode", file_stem))) {
                    let mut std_write = wincode::io::std_write::WriteAdapter::new(&mut w);
                    let _ = wincode::serialize_into(&mut std_write, &sample);
                }
            }
        }
    }

    #[cfg(feature = "ml")]
    {
        if ml_schedule || ml_prune_ratio.is_some() {
            use mrs_core::term_bank::TermBank;
            let mut bank = TermBank::new();
            let mut id_clauses = Vec::with_capacity(all_clauses.len());
            for c in &all_clauses {
                id_clauses.push(bank.clause_from_legacy(c));
            }

            if let Some(ratio) = ml_prune_ratio {
                use burn::backend::ndarray::NdArrayDevice;
                use mrs_core::ml::premise_selector::PremiseSelector;
                let device = NdArrayDevice::Cpu;
                let selector = PremiseSelector::<burn::backend::ndarray::NdArray>::new(device);

                let mut conjectures = Vec::new();
                let mut axioms = Vec::new();
                for c in id_clauses {
                    if c.distance == 0 {
                        conjectures.push(c);
                    } else {
                        axioms.push(c);
                    }
                }

                let pruned_axioms =
                    selector.select_premises(axioms, &conjectures, ratio, &bank, &lowered.symbols);

                info!(
                    "% ML Premise Selection: kept {} / {} axioms",
                    pruned_axioms.len(),
                    all_clauses.len() - conjectures.len()
                );

                use std::collections::HashSet;
                let kept_ids: HashSet<_> = pruned_axioms
                    .iter()
                    .map(|c| c.id)
                    .chain(conjectures.iter().map(|c| c.id))
                    .collect();
                all_clauses.retain(|c| kept_ids.contains(&c.id));

                // Re-populate id_clauses for ScheduleClassifier
                id_clauses = Vec::with_capacity(all_clauses.len());
                for c in &all_clauses {
                    id_clauses.push(bank.clause_from_legacy(c));
                }
            }

            if ml_schedule && schedule_name.is_none() {
                use burn::backend::ndarray::NdArrayDevice;
                use mrs_core::ml::schedule_classifier::{
                    ScheduleClassifier, extract_schedule_features,
                };
                let device = NdArrayDevice::Cpu;
                let classifier = ScheduleClassifier::<burn::backend::ndarray::NdArray>::new(device);
                let feats = extract_schedule_features(&id_clauses, &bank, &lowered.symbols);
                let assigned = classifier.classify(feats);
                schedule_name = Some(assigned.to_string());
                info!("% ML Schedule Classifier: chose portfolio '{}'", assigned);
            }
        }
    }

    #[cfg(not(feature = "ml"))]
    {
        if ml_schedule || ml_prune_ratio.is_some() {
            eprintln!(
                "Warning: --ml-schedule or --ml-prune used but prover compiled without 'ml' feature. Flags ignored."
            );
        }
    }

    let elapsed = start.elapsed();
    let (final_result, final_status, final_report) = if elapsed >= total_budget {
        (
            SearchResult::Timeout,
            SzsStatus::Timeout,
            ScheduleReport::default(),
        )
    } else {
        let actual_workers = workers.unwrap_or_else(|| num_cpus::get_physical().max(1));

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

        let (result, schedule_report) = run_schedule(
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

        let status = match &result {
            SearchResult::Refutation(..) => {
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
            SearchResult::GaveUp => SzsStatus::GaveUp,
        };

        (result, status, schedule_report)
    };

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

        print_statistics(status, start.elapsed(), &final_report);
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
fn print_statistics(status: SzsStatus, elapsed: Duration, report: &mrs_search::ScheduleReport) {
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

    // Emit structured failure detail to stderr so the benchmark harness can
    // classify unsolved problems without re-parsing stdout.
    // Format: "% SZS detail <key=value> ..."
    // Always emitted (even on success) so casc.sh can parse it uniformly.
    if let Some(detail) = report.failure_reason() {
        eprintln!("% SZS detail {detail}");
    } else {
        // Summarise solved cases too (useful for throughput analysis).
        let total_processed: u64 = report.strategies.iter().map(|s| s.stats.processed).sum();
        let total_generated: u64 = report.strategies.iter().map(|s| s.stats.generated).sum();
        eprintln!(
            "% SZS detail strategies={} result=Refutation processed={} generated={}",
            report.strategies.len(),
            total_processed,
            total_generated,
        );
    }
}
