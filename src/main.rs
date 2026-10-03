//! MRS - Mechanical Reasoning System
//!
//! An automated theorem prover targeting the CASC competition.

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[allow(dead_code)]
mod analyze;
mod coordinator;
mod include;
mod lowering;
mod pipeline;
mod sine;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::time::Instant;

use std::time::Duration;

use mrs_proof_kernel::model::ModelEvaluation;
use mrs_search::strategy::{StrategySchedule, run_schedule};
use mrs_search::{ScheduleReport, SearchResult};
use mrs_szs::{SzsStatus, szs_output_end, szs_output_start, szs_status_line};

fn main() {
    let start = Instant::now();
    let mut time_secs: u64 = 30;
    let mut schedule_name: Option<String> = None;
    let mut path: Option<String> = None;
    let mut log_ml_data: Option<String> = None;
    let mut ml_log_csv = false;
    let mut ml_weights: Option<String> = None;
    #[cfg(feature = "parent-guidance")]
    let mut parent_weights: Option<String> = None;
    #[cfg(feature = "parent-guidance")]
    let mut parent_threshold: Option<f32> = None;
    let mut workers: Option<usize> = None;
    let mut hardware: Option<mrs_search::HardwareMode> = None;
    // casc-sim only: multiple of the CASC wall clock to keep searching after.
    // `0` means "until a resource cap", so a memory-bound failure surfaces
    // instead of reading as a timeout.
    let mut sim_time_factor: f64 = 2.0;
    let mut auto_schedule = false;
    let mut exact_strategy: Option<usize> = None;
    let mut portfolio: Option<Vec<usize>> = None;
    let mut ml_prune_ratio: Option<f32> = None;
    let mut self_check = false;
    let mut cert_reserve_worker = false;
    let mut include_root: Option<PathBuf> = None;
    let mut stats_mode = false;
    let mut profile_json_mode = false;
    let mut goal_transform: Option<mrs_cnf::GoalTransformMode> = None;
    let mut certify_ordered = false;
    // Pre-phase: measure the problem, then route the portfolio from the
    // measurement. Off unless asked for, so this stays a measurement rather than
    // a silent behaviour change.
    let mut pre_phase = false;
    let mut pre_phase_only = false;
    let mut pre_phase_probe = false;
    /// What casc-sim does with the wall clock. `Inherit` is every other mode.
    enum SimBudget {
        Inherit,
        Scaled(Duration),
        UntilResourceCap,
    }
    // Default 8 MiB: below the smallest output allowance CASC has stated
    // (10MB per system in CASC-23), so a runaway AVATAR certificate cannot
    // get the process killed before the SZS status line is flushed.
    let mut proof_bytes_limit: usize = 8 * 1024 * 1024;
    #[cfg(feature = "ml")]
    let mut ml_premise_weights: Option<String> = None;

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
                if time_secs == 0 {
                    eprintln!("Error: --time requires a positive integer");
                    process::exit(1);
                }
            }
            "--workers" => {
                let val = args.next().unwrap_or_else(|| {
                    eprintln!("Usage: mrs [--workers <N>] <file.p>");
                    process::exit(1);
                });
                let parsed = val.parse().unwrap_or_else(|_| {
                    eprintln!(
                        "Error: --workers requires a positive integer, got {:?}",
                        val
                    );
                    process::exit(1);
                });
                if parsed == 0 {
                    eprintln!("Error: --workers requires a positive integer");
                    process::exit(1);
                }
                workers = Some(parsed);
            }
            "--hardware" => {
                let val = args.next().unwrap_or_else(|| {
                    eprintln!("Usage: mrs [--hardware <adaptive|casc|casc-sim>] <file.p>");
                    process::exit(1);
                });
                hardware = Some(mrs_search::HardwareMode::parse(&val).unwrap_or_else(|| {
                    eprintln!("Error: --hardware expects adaptive, casc, or casc-sim, got {val:?}");
                    process::exit(1);
                }));
            }
            "--sim-time-factor" => {
                let val = args.next().unwrap_or_else(|| {
                    eprintln!("Usage: mrs --sim-time-factor <mult|0|unbounded>");
                    process::exit(1);
                });
                sim_time_factor = match val.trim().to_ascii_lowercase().as_str() {
                    "0" | "unbounded" | "none" | "inf" => 0.0,
                    other => match other.parse::<f64>() {
                        Ok(f) if f > 0.0 => f,
                        _ => {
                            eprintln!(
                                "Error: --sim-time-factor expects a positive multiplier, or 0/unbounded for no wall-clock limit; got {val:?}"
                            );
                            process::exit(1);
                        }
                    },
                };
            }
            "--strategy" => {
                let val = args.next().unwrap_or_else(|| {
                    eprintln!("Error: --strategy requires an ID in the range 1..15");
                    process::exit(1);
                });
                let parsed = val.parse().unwrap_or_else(|_| {
                    eprintln!("Error: --strategy requires an integer, got {:?}", val);
                    process::exit(1);
                });
                if !(1..=15).contains(&parsed) {
                    eprintln!("Error: --strategy requires an ID in the range 1..15");
                    process::exit(1);
                }
                exact_strategy = Some(parsed);
            }
            "--portfolio" => {
                let val = args.next().unwrap_or_else(|| {
                    eprintln!("Error: --portfolio requires comma-separated strategy IDs");
                    process::exit(1);
                });
                let ids = val
                    .split(',')
                    .map(|id| id.parse::<usize>())
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap_or_else(|_| {
                        eprintln!("Error: --portfolio contains a non-integer strategy ID");
                        process::exit(1);
                    });
                if ids.is_empty() || ids.iter().any(|id| !(1..=15).contains(id)) {
                    eprintln!("Error: --portfolio strategy IDs must be in the range 1..15");
                    process::exit(1);
                }
                portfolio = Some(ids);
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
            "--parent-guidance-weights" => {
                #[cfg(feature = "parent-guidance")]
                {
                    parent_weights = Some(args.next().unwrap_or_else(|| {
                        eprintln!("--parent-guidance-weights requires a JSON file");
                        process::exit(1);
                    }));
                }
                #[cfg(not(feature = "parent-guidance"))]
                {
                    eprintln!("Build with --features parent-guidance to use this option");
                    process::exit(1);
                }
            }
            "--parent-guidance-threshold" => {
                #[cfg(feature = "parent-guidance")]
                {
                    let raw = args.next().unwrap_or_else(|| {
                        eprintln!("--parent-guidance-threshold requires a finite logit");
                        process::exit(1);
                    });
                    let value = raw
                        .parse::<f32>()
                        .ok()
                        .filter(|v| v.is_finite())
                        .unwrap_or_else(|| {
                            eprintln!("Invalid finite logit: {raw}");
                            process::exit(1);
                        });
                    parent_threshold = Some(value);
                }
                #[cfg(not(feature = "parent-guidance"))]
                {
                    eprintln!("Build with --features parent-guidance to use this option");
                    process::exit(1);
                }
            }
            "--auto-schedule" => {
                auto_schedule = true;
            }
            "--stats" | "--info" | "--analyze" | "--profile" => {
                stats_mode = true;
            }
            "--profile-json" => {
                profile_json_mode = true;
            }
            "--self-check" | "--certified" => {
                self_check = true;
            }
            "--cert-reserve-worker" => {
                cert_reserve_worker = true;
            }
            "--include-root" => {
                let value = args.next().unwrap_or_else(|| {
                    eprintln!("Error: --include-root requires a directory path");
                    process::exit(1);
                });
                include_root = Some(PathBuf::from(value));
            }
            // Deprecated: the ML schedule classifier is retired (degenerate
            // majority-class model + label mismatch; see docs/BENCHMARKS.md).
            // --ml-schedule now maps to the rule-based --auto-schedule.
            "--ml-schedule" => {
                eprintln!(
                    "Warning: --ml-schedule is deprecated; using rule-based --auto-schedule instead."
                );
                auto_schedule = true;
            }
            "--ml-schedule-weights" => {
                let _ = args.next().unwrap_or_else(|| {
                    eprintln!("Error: --ml-schedule-weights requires a file path");
                    std::process::exit(1);
                });
                eprintln!(
                    "Warning: --ml-schedule-weights is deprecated and ignored (schedule selection is rule-based)."
                );
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
            "--ml-premise-weights" => {
                let val = args.next().unwrap_or_else(|| {
                    eprintln!("Error: --ml-premise-weights requires a file path");
                    std::process::exit(1);
                });
                #[cfg(feature = "ml")]
                {
                    ml_premise_weights = Some(val);
                }
                #[cfg(not(feature = "ml"))]
                {
                    let _ = val;
                }
            }
            "--goal-transform" => {
                let val = args.next().unwrap_or_else(|| {
                    eprintln!(
                        "Error: --goal-transform requires a mode (recursive, maximal, or none)"
                    );
                    process::exit(1);
                });
                match val.as_str() {
                    "recursive" | "all" => {
                        goal_transform = Some(mrs_cnf::GoalTransformMode::RecursiveSubterms)
                    }
                    "maximal" | "top" => {
                        goal_transform = Some(mrs_cnf::GoalTransformMode::MaximalSubterms)
                    }
                    "none" | "off" => goal_transform = None,
                    _ => {
                        eprintln!(
                            "Error: unknown --goal-transform mode {:?} (expected: recursive, maximal, none)",
                            val
                        );
                        process::exit(1);
                    }
                }
            }
            "--certify-ordered" => {
                certify_ordered = true;
            }
            "--pre-phase" => {
                pre_phase = true;
            }
            "--pre-phase-only" => {
                pre_phase = true;
                pre_phase_only = true;
            }
            "--pre-phase-probe" => {
                pre_phase = true;
                pre_phase_probe = true;
            }
            "--proof-bytes-limit" => {
                let val = args.next().unwrap_or_else(|| {
                    eprintln!("Error: --proof-bytes-limit requires a byte count");
                    process::exit(1);
                });
                proof_bytes_limit = val.parse().unwrap_or_else(|_| {
                    eprintln!(
                        "Error: --proof-bytes-limit requires a byte count, got {:?}",
                        val
                    );
                    process::exit(1);
                });
                if proof_bytes_limit == 0 {
                    eprintln!("Error: --proof-bytes-limit must be greater than zero");
                    process::exit(1);
                }
            }
            // Deprecated alias: --fast is now --schedule fast.
            "--fast" => {
                schedule_name = Some("fast".to_string());
            }
            "--list-rules" => {
                println!("{}", mrs_prephase::plan::describe_rules());
                process::exit(0);
            }
            "--list-schedules" => {
                for name in mrs_search::strategy::named::ALL {
                    println!("{name}");
                }
                process::exit(0);
            }
            "--no-bce" => unsafe {
                std::env::set_var("MRS_NO_BCE", "1");
            },
            "--no-ple" => unsafe {
                std::env::set_var("MRS_NO_PLE", "1");
            },
            "--trace-bce" => unsafe {
                std::env::set_var("TRACE_BCE", "1");
            },
            "--no-instgen" => unsafe {
                std::env::set_var("MRS_NO_INSTGEN", "1");
            },
            "--trace-instgen" => unsafe {
                std::env::set_var("TRACE_INSTGEN", "1");
            },
            "--no-lrs" => unsafe {
                std::env::set_var("MRS_NO_LRS", "1");
            },
            "--trace-lrs" => unsafe {
                std::env::set_var("TRACE_LRS", "1");
            },
            "--no-sharing" => unsafe {
                std::env::set_var("MRS_SHARED_POOL_INTERVAL", "0");
            },
            #[cfg(feature = "proover")]
            "--quiet" => quiet = true,
            _ => {
                if path.is_some() {
                    eprintln!(
                        "Usage: mrs [--time <seconds>] [--schedule NAME] [--workers N] [--strategy N|--portfolio IDS] [--goal-transform MODE] [--certify-ordered] [--pre-phase] [--proof-bytes-limit N] [--no-bce] [--no-ple] [--no-instgen] [--no-lrs] [--no-sharing] [--self-check] [--stats|--profile] [--profile-json] [--include-root DIR] <file.p>"
                    );
                    process::exit(1);
                }
                path = Some(arg);
            }
        }
    }
    let Some(path) = path else {
        eprintln!(
            "Usage: mrs [--time <seconds>] [--schedule NAME] [--workers N] [--strategy N|--portfolio IDS] [--goal-transform MODE] [--certify-ordered] [--pre-phase] [--proof-bytes-limit N] [--no-bce] [--no-ple] [--no-instgen] [--no-lrs] [--no-sharing] [--self-check] [--stats|--profile] [--profile-json] [--include-root DIR] <file.p>"
        );
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

    // Record the real invocation path so that proof output's `file(...)`
    // leaf annotations point at a path GDV-style checkers can actually
    // re-open (e.g. the StarExec sandbox path at competition time), rather
    // than a placeholder string. Left unset for stdin input (no real path).
    if path != "-" {
        mrs_proof::tstp::set_problem_path(path.clone());
    }

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

    // Lower to core types, resolve includes, and clausify. Shared with the
    // offline pre-phase dumper so both see exactly the same clause set.
    let input_bytes = if path == "-" {
        None
    } else {
        fs::metadata(&path).ok().map(|m| m.len())
    };
    let resolved_includes = problem.includes.len();
    let prepared = pipeline::prepare(&problem, Some(&path), input_bytes);
    let lowered = prepared.lowered;
    let prephase_meta = prepared.meta;
    let all_clauses = prepared.clauses;
    let provenance = prepared.provenance;
    // The clause-id generator handed to the scheduler is the lowered one, not
    // the generator the clausification walked: inference steps get ids from it
    // after every input clause is already numbered.
    let id_gen = lowered.id_gen.clone();
    if resolved_includes > 0 {
        info!("% Resolved {} include directive(s)", resolved_includes);
    }

    let has_logical_formulas = !problem.includes.is_empty()
        || problem.formulas.iter().any(|f| {
            match f {
                mrs_tptp::AnnotatedFormula::FOF(_) => true,
                mrs_tptp::AnnotatedFormula::CNF(_) => true,
                mrs_tptp::AnnotatedFormula::TFF(tff) => {
                    !matches!(tff.formula, mrs_tptp::TFFStatement::Typing(_))
                }
                mrs_tptp::AnnotatedFormula::TCF(_) => true,
                _ => true, // THF, TPI are logical
            }
        });

    if has_logical_formulas
        && lowered.axioms.is_empty()
        && lowered.conjectures.is_empty()
        && lowered.cnf_clauses.is_empty()
    {
        info!("% Warning: No supported logical formulas could be lowered");
        println!("{}", szs_status_line(SzsStatus::GaveUp, problem_name));
        process::exit(0);
    }

    let has_conjecture = !lowered.conjectures.is_empty();

    // SInE is now performed per portfolio strategy in parallel (with threshold tuning),
    // so we do not run a single global pre-filter on LoweredFormulas anymore.
    //
    // Hardware mode decides the worker count, the memory ceiling and (for
    // casc-sim) the CPU set. `MRS_HARDWARE` lets the benchmark harness select a
    // mode for a whole run without rewriting each invocation, and an explicit
    // `--workers` / `MRS_MAX_MEMORY_MB` still wins for that dimension.
    let hardware_mode = hardware
        .or_else(|| {
            std::env::var("MRS_HARDWARE")
                .ok()
                .and_then(|raw| mrs_search::HardwareMode::parse(&raw))
        })
        .unwrap_or(mrs_search::HardwareMode::Adaptive);
    let explicit_memory_mb = std::env::var("MRS_MAX_MEMORY_MB")
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok());
    let hardware_profile = mrs_search::resolve_profile(hardware_mode, workers, explicit_memory_mb);

    // In casc-sim the CASC wall clock stays the reference verdict, and the run is
    // allowed to continue past it so a memory-bound failure surfaces instead of
    // being recorded as a timeout. `--sim-time-factor 0` (or `unbounded`) means
    // "until a resource cap" rather than any multiple of the limit.
    let casc_limit = Duration::from_secs(time_secs);
    // Whether the search will run past the CASC wall clock, decided before the
    // schedule exists.
    let sim_budget_is_extended =
        matches!(hardware_mode, mrs_search::HardwareMode::CascSim) && sim_time_factor != 1.0;
    let sim_budget = if hardware_mode == mrs_search::HardwareMode::CascSim {
        if sim_time_factor == 0.0 {
            SimBudget::UntilResourceCap
        } else {
            SimBudget::Scaled(Duration::from_secs(
                (time_secs as f64 * sim_time_factor).ceil() as u64,
            ))
        }
    } else {
        SimBudget::Inherit
    };
    // `SearchConfig::time_limit` is a plain `Duration`, so "no wall-clock limit"
    // is expressed as a sentinel far beyond any real run. One year cannot be
    // reached: the memory, term-bank and clause ceilings still apply, which is
    // the whole point of this mode.
    const YEAR: Duration = Duration::from_secs(31_536_000);
    let total_budget = match sim_budget {
        SimBudget::Inherit => casc_limit,
        SimBudget::Scaled(limit) => limit,
        SimBudget::UntilResourceCap => YEAR,
    };

    for warning in &hardware_profile.warnings {
        eprintln!("% Hardware warning: {warning}");
    }

    // casc-sim makes the constraint real rather than nominal. Pinning happens
    // before any worker thread exists, so every thread inherits the mask.
    // Both guards are held for the life of `main`; dropping them restores the
    // inherited limits, which matters because this is a library.
    let _cpu_pinning = if hardware_mode == mrs_search::HardwareMode::CascSim {
        match mrs_search::pin_to_physical_cores(mrs_search::CASC_PHYSICAL_CORES) {
            Ok(pin) => {
                eprintln!(
                    "% Hardware: pinned to {} logical CPU(s) across {} physical core(s)",
                    pin.cpus().len(),
                    mrs_search::CASC_PHYSICAL_CORES
                );
                Some(pin)
            }
            Err(reason) => {
                eprintln!(
                    "% Hardware warning: casc-sim could not pin to {} physical cores ({reason}); \
                     the run is not CPU-constrained",
                    mrs_search::CASC_PHYSICAL_CORES
                );
                None
            }
        }
    } else {
        None
    };
    let _address_space_limit = if hardware_mode == mrs_search::HardwareMode::CascSim {
        let address_space_budget_mb = hardware_profile
            .memory_budget_mb
            .unwrap_or(mrs_search::CASC_MEMORY_MB);
        match mrs_search::limit_address_space_mb(address_space_budget_mb) {
            Ok(limit) => {
                let applied = limit.applied_mb();
                if applied == u64::MAX {
                    eprintln!(
                        "% Hardware warning: no address-space ceiling could be applied; the \
                         inherited limit is unlimited, so only the RSS watchdog bounds memory"
                    );
                } else if applied < address_space_budget_mb {
                    eprintln!(
                        "% Hardware warning: address-space ceiling clamped to {applied} MB by the \
                         inherited hard limit, below the requested {address_space_budget_mb} MB allowance"
                    );
                }
                Some(limit)
            }
            Err(reason) => {
                eprintln!(
                    "% Hardware warning: casc-sim could not set an address-space ceiling \
                     ({reason}); only the RSS watchdog applies"
                );
                None
            }
        }
    } else {
        None
    };
    let mut sim_budget_note = match sim_budget {
        SimBudget::Inherit => String::new(),
        SimBudget::Scaled(limit) => {
            format!(
                " casc_limit_s={} sim_limit_s={}",
                casc_limit.as_secs(),
                limit.as_secs()
            )
        }
        SimBudget::UntilResourceCap => {
            format!(
                " casc_limit_s={} sim_limit_s=unbounded",
                casc_limit.as_secs()
            )
        }
    };
    if let Some(pin) = _cpu_pinning.as_ref() {
        sim_budget_note.push_str(&format!(" pinned_cpus={}", pin.cpus().len()));
    }
    if let Some(limit) = _address_space_limit.as_ref()
        && limit.applied_mb() != u64::MAX
    {
        sim_budget_note.push_str(&format!(" address_space_mb={}", limit.applied_mb()));
    }
    info!(
        "% Hardware: {}{sim_budget_note}",
        hardware_profile.describe()
    );

    // Display input summary
    let cnf_count = lowered.cnf_clauses.len();
    info!(
        "% Problem: {} ({} axioms, {} conjectures, {} cnf clauses)",
        problem_name,
        lowered.axioms.len(),
        lowered.conjectures.len(),
        cnf_count
    );

    // Clausification already happened in `pipeline::prepare`; the clause set
    // the search sees is `all_clauses`, and `provenance` holds the FOF-level
    // NNF / Skolemization / conjecture-negation steps that document the
    // translation in the TSTP proof (CASC's evaluation criteria require the
    // FOF-to-CNF translation to be documented). Those steps never enter the
    // live search — see `Clause::formula`'s doc comment for why.

    if profile_json_mode {
        analyze::analyze_and_print_json_with_counts(
            &path,
            &problem,
            &lowered.symbols,
            &all_clauses,
            lowered.input_axioms_count,
            lowered.input_conjectures_count,
        );
        process::exit(0);
    }

    if stats_mode {
        analyze::analyze_and_print_with_counts(
            &path,
            &problem,
            &lowered.symbols,
            &all_clauses,
            lowered.input_axioms_count,
            lowered.input_conjectures_count,
        );
        process::exit(0);
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
                if let Ok(mut w) =
                    std::fs::File::create(log_path.join(format!("{}.csv", file_stem)))
                {
                    use std::io::Write;
                    let feats_str = feats
                        .iter()
                        .map(|f| f.to_string())
                        .collect::<Vec<_>>()
                        .join(",");
                    let _ = writeln!(w, "0,{}", feats_str);
                }
            } else {
                if let Ok(mut w) =
                    std::fs::File::create(log_path.join(format!("{}.wincode", file_stem)))
                {
                    let mut std_write = wincode::io::std_write::WriteAdapter::new(&mut w);
                    let _ = wincode::serialize_into(&mut std_write, &sample);
                }
            }
        }
    }

    // ML premise keep-set (clause ids). Applied per worker inside the
    // scheduler on a minority of strategies; `None` disables pruning.
    let mut premise_keep: Option<
        std::sync::Arc<std::collections::HashSet<mrs_core::clause::ClauseId>>,
    > = None;
    #[cfg(not(feature = "ml"))]
    {
        let _ = &mut premise_keep; // silence unused_mut without the feature
    }

    #[cfg(feature = "ml")]
    {
        if let Some(ratio) = ml_prune_ratio {
            use burn::backend::ndarray::NdArrayDevice;
            use mrs_core::ml::premise_selector::PremiseSelector;
            use mrs_core::term_bank::TermBank;
            let mut bank = TermBank::new();
            let mut id_clauses = Vec::with_capacity(all_clauses.len());
            for c in &all_clauses {
                id_clauses.push(bank.clause_from_legacy(c));
            }

            let device = NdArrayDevice::Cpu;
            let Some(weights) = &ml_premise_weights else {
                eprintln!(
                    "Error: --ml-prune requires --ml-premise-weights; refusing to prune with a randomly initialized model."
                );
                std::process::exit(1);
            };
            let selector = match PremiseSelector::<burn::backend::ndarray::NdArray>::load_from_file(
                weights, &device,
            ) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Failed to load premise weights from {}: {}", weights, e);
                    std::process::exit(1);
                }
            };

            let mut conjectures = Vec::new();
            let mut axioms = Vec::new();
            for c in &id_clauses {
                if c.distance == 0 {
                    conjectures.push(c.clone());
                } else {
                    axioms.push(c.clone());
                }
            }
            let axiom_count = axioms.len();

            // Below this size, ML premise selection is not worth its own
            // overhead: `select_premises` floors the keep count at 10
            // (mrs_core::ml::premise_selector), so small problems barely get
            // pruned anyway, while still paying the cost of feature
            // extraction + a model forward pass. Mirrors the analogous
            // `len > 100` size guard SInE uses in strategy.rs.
            const ML_PRUNE_MIN_AXIOMS: usize = 100;

            if axiom_count < ML_PRUNE_MIN_AXIOMS {
                info!(
                    "% ML Premise Selection: skipped ({} axioms < {} minimum)",
                    axiom_count, ML_PRUNE_MIN_AXIOMS
                );
            } else {
                // Preprocessing time is not subtracted from the search budget
                // (see `elapsed`/`total_budget` below), so it is measured and
                // logged here purely as a diagnostic: it should be negligible
                // now that scoring is batched into a single forward pass
                // (mrs_core::ml::premise_selector::PremiseSelector::
                // evaluate_scores_batch) instead of one tensor per axiom.
                let scoring_start = Instant::now();
                let pruned_axioms =
                    selector.select_premises(axioms, &conjectures, ratio, &bank, &lowered.symbols);
                let scoring_ms = scoring_start.elapsed().as_millis();

                info!(
                    "% ML Premise Selection: kept {} / {} axioms in {}ms (applied per worker)",
                    pruned_axioms.len(),
                    axiom_count,
                    scoring_ms
                );

                // Only install a keep-set if pruning actually removes clauses.
                // It is applied per worker inside run_schedule; the full clause
                // set is still handed to the scheduler untouched.
                if pruned_axioms.len() < axiom_count {
                    use std::collections::HashSet;
                    let kept_ids: HashSet<_> = pruned_axioms
                        .iter()
                        .map(|c| c.id)
                        .chain(conjectures.iter().map(|c| c.id))
                        .collect();
                    premise_keep = Some(std::sync::Arc::new(kept_ids));
                }
            }
        }
    }

    #[cfg(not(feature = "ml"))]
    {
        if ml_prune_ratio.is_some() {
            eprintln!(
                "Warning: --ml-prune used but prover compiled without 'ml' feature. Flag ignored."
            );
        }
    }

    // Rule-based schedule auto-detection (replaces the retired ML schedule
    // classifier; see docs/research/ml.md). An explicit --schedule always
    // wins. Works in any build.
    if auto_schedule && schedule_name.is_none() {
        let assigned = mrs_search::strategy::auto_schedule_name(&all_clauses);
        schedule_name = Some(assigned.to_string());
        info!("% Auto schedule: chose portfolio '{}'", assigned);
    }

    let elapsed = start.elapsed();
    let (final_result, final_status, final_report, final_cert_telemetry) = if elapsed
        >= total_budget
    {
        (
            SearchResult::Timeout,
            SzsStatus::Timeout,
            ScheduleReport::default(),
            None,
        )
    } else {
        // One worker per usable physical core by default, bounded by what memory
        // can support; `--hardware casc`/`casc-sim` pin it to the CASC count
        // instead. An explicit `--workers` already won inside the profile.
        let actual_workers = hardware_profile.workers;
        // `--certify-ordered` still needs `--strategy N`, because the certified
        // fragment commits to one ordering. It no longer needs a single worker:
        // the ordered-resolution closure is a wave-structured saturation whose
        // positions have provably fixed partner sets, so it fans out across
        // workers and reproduces the sequential scan exactly. That fan-out is
        // where the certified fragment spends its time — on the 2026-09-26
        // `casc-30/EPS` run every refusal was a closure timeout, not a grounding
        // or SAT limit.
        if certify_ordered && exact_strategy.is_none() {
            eprintln!("Error: --certify-ordered requires --strategy N");
            process::exit(1);
        }
        let (search_workers, cert_oversubscribed) = if self_check && cert_reserve_worker {
            ((actual_workers.saturating_sub(1)).max(1), false)
        } else {
            (actual_workers, true)
        };

        let search_budget = total_budget - elapsed;
        if exact_strategy.is_some() && portfolio.is_some() {
            eprintln!("Error: --strategy and --portfolio are mutually exclusive");
            process::exit(1);
        }
        let selected_schedule = schedule_name.as_deref().unwrap_or("casc");
        let mut schedule = match (exact_strategy, portfolio.as_deref()) {
            (Some(id), None) => {
                match mrs_search::strategy::named::single_strategy(
                    selected_schedule,
                    search_budget,
                    id,
                ) {
                    Some(s) => s,
                    None => {
                        eprintln!(
                            "Error: --strategy requires a CASC division schedule (casc, casc_feq, casc_fne, casc_ueq, casc_epr, casc_eps, casc_epu, or casc_icu)"
                        );
                        process::exit(1);
                    }
                }
            }
            (None, Some(ids)) => {
                match mrs_search::strategy::named::with_portfolio(
                    selected_schedule,
                    search_budget,
                    search_workers,
                    ids,
                ) {
                    Some(s) => s,
                    None => {
                        eprintln!(
                            "Error: --portfolio requires a CASC division schedule and strategy IDs in 1..15"
                        );
                        process::exit(1);
                    }
                }
            }
            (None, None) if schedule_name.is_none() => {
                StrategySchedule::default_schedule(search_budget, search_workers)
            }
            (None, None) => match mrs_search::strategy::named::by_name(
                selected_schedule,
                search_budget,
                search_workers,
            ) {
                Some(s) => s,
                None => {
                    eprintln!(
                        "Error: unknown schedule {:?} (known: {})",
                        selected_schedule,
                        mrs_search::strategy::named::ALL.join(", "),
                    );
                    process::exit(1);
                }
            },
            (Some(_), Some(_)) => unreachable!(),
        };
        if self_check {
            for (config, _) in &mut schedule.strategies {
                config.emit_avatar_trace = true;
            }
        }
        // The pre-phase replaces the portfolio only when no schedule was named
        // explicitly. An explicit `--schedule` / `--strategy` / `--portfolio` is
        // a measurement instruction, and silently overriding it would make every
        // A/B of this feature unreadable.
        let prephase_enabled = pre_phase || std::env::var("MRS_PREPHASE").is_ok_and(|v| v != "0");
        let mut decision: Option<mrs_search::prephase::Decision> = None;
        if prephase_enabled {
            let mut route = mrs_search::prephase::decide(
                problem_name,
                &prephase_meta,
                &all_clauses,
                &lowered.symbols,
            );
            if pre_phase_probe {
                route.probe = Some(mrs_search::prephase::probe(
                    &all_clauses,
                    lowered.id_gen.clone(),
                    &lowered.symbols,
                ));
                let summary = route
                    .probe
                    .as_ref()
                    .map(|probe| probe.summary())
                    .unwrap_or_default();
                info!("% Pre-phase probe: {summary}");
            }
            info!("% Pre-phase: {}", route.plan.summary());
            info!(
                "% Pre-phase analysis: label={} clauses={} literals={} logic={} shape={} \
                 goal_reachable={:.2} components={} redundancy={:.2} max_depth={} skolems={} \
                 abstraction_atoms={} feasibility={}",
                route.analysis.label,
                route.analysis.n_clauses,
                route.analysis.n_literals,
                route.analysis.logic(),
                route.analysis.shape_class.as_str(),
                route.analysis.goal_reachable_ratio,
                route.analysis.n_components,
                route.analysis.redundant_ratio,
                route.analysis.max_term_depth,
                route.analysis.n_skolems,
                route.analysis.abstraction_atoms,
                route.analysis.feasibility.as_str(),
            );
            if pre_phase_only {
                println!("{}", szs_status_line(SzsStatus::GaveUp, problem_name));
                process::exit(0);
            }
            if schedule_name.is_none() && exact_strategy.is_none() && portfolio.is_none() {
                let mut configs = mrs_search::prephase::schedule_from_plan(
                    &route.plan,
                    search_budget,
                    search_workers,
                );
                mrs_search::prephase::apply_pre_passes(&mut configs, &route.plan);
                schedule = mrs_search::strategy::StrategySchedule {
                    strategies: configs
                        .into_iter()
                        .map(|config| {
                            let time = config.time_limit;
                            (config, time)
                        })
                        .collect(),
                };
                info!(
                    "% Pre-phase: portfolio replaced with {} routed strategies",
                    schedule.strategies.len()
                );
            } else {
                info!(
                    "% Pre-phase: an explicit schedule was requested, so the routing decision \
                     is reported but not applied"
                );
            }
            decision = Some(route);
        }
        let _ = &decision;
        if let Some(gt) = goal_transform {
            for (config, _) in &mut schedule.strategies {
                config.goal_transformation = Some(gt);
            }
        }
        // The memory ceiling the watchdog enforces, applied to every strategy.
        // Without this the watchdog would keep using `ResourceLimits::default()`,
        // which reads the ambient policy and so ignores the mode entirely.
        //
        // This is the *effective* ceiling, not the mode's nominal allowance. A
        // casc-shaped mode asks for 128 GB, but when the host has less the OS
        // OOM-killer gets there first and the run dies with no SZS status at
        // all — the benchmark records a kill instead of a resource limit, which
        // is the opposite of what casc-sim exists to surface. Enforcing the
        // effective ceiling instead makes the run fail closed and say so. On a
        // host that can represent the allowance the two are equal, so a
        // competition-shaped run is unaffected.
        if let Some(limit_mb) = hardware_profile.effective_memory_mb {
            for (config, _) in &mut schedule.strategies {
                config.resource_limits.max_memory_mb = Some(limit_mb);
            }
        }
        // Only casc-sim searches past the CASC wall clock, so only casc-sim needs
        // the reference limit recorded inside the search.
        if sim_budget_is_extended {
            for (config, _) in &mut schedule.strategies {
                config.casc_reference_limit = Some(casc_limit);
            }
        }
        if certify_ordered {
            if schedule.strategies.len() != 1 {
                eprintln!(
                    "Error: --certify-ordered requires one strategy; use --workers 1 --strategy 1 or a single explicit schedule"
                );
                process::exit(1);
            }
            let (config, _) = &mut schedule.strategies[0];
            config.certify_ordered_inferences = true;
            config.ordered_inferences = true;
            config.max_term_weight = None;
            config.use_avatar = false;
            config.literal_selection = mrs_search::LiteralSelection::All;
            config.sos_depth = u32::MAX;
            config.unit_only_resolution = false;
            config.weight_fn = mrs_search::ClauseWeightFn::Standard;
            config.sine_tolerance = None;
            config.sine_depth_limit = None;
            config.lrs_policy = mrs_search::LrsPolicy::Disabled;
            config.shared_pool_poll_interval = 0;
        }

        let (result, schedule_report, cert_telemetry) = if self_check {
            let coordinator =
                coordinator::AsyncCoordinator::new(coordinator::AsyncCoordinatorConfig {
                    time_limit: Duration::from_secs(time_secs),
                    self_check_reserve: Duration::from_secs(2),
                    problem_path: path.clone(),
                    input_text: input.clone(),
                    include_root: include_root.clone(),
                    has_includes: !problem.includes.is_empty(),
                    cert_oversubscribed,
                    search_workers,
                    cert_workers: 1,
                    start_time: start,
                });
            let (res, rep) = mrs_search::strategy::run_schedule_with_candidate_receiver(
                &all_clauses,
                &provenance,
                id_gen,
                &schedule,
                &lowered.symbols,
                mrs_search::strategy::MlOptions {
                    log_dir: log_ml_data.clone(),
                    log_csv: ml_log_csv,
                    weights: ml_weights.clone(),
                    premise_keep: premise_keep.clone(),
                    #[cfg(feature = "parent-guidance")]
                    parent_weights: parent_weights.clone(),
                    #[cfg(feature = "parent-guidance")]
                    parent_threshold,
                },
                Some(search_workers),
                Some(coordinator.clone()),
            );
            let tele = coordinator.finish();
            let mut res = coordinator.certified_result().unwrap_or(res);
            if tele.coordinator_error.is_some()
                && matches!(
                    res,
                    SearchResult::Refutation(..) | SearchResult::Saturated(_)
                )
            {
                res = SearchResult::GaveUp;
            }
            if matches!(res, SearchResult::Timeout)
                && rep
                    .strategies
                    .iter()
                    .any(|s| matches!(s.result, SearchResult::Refutation(..)))
            {
                res = SearchResult::GaveUp;
            }
            (res, rep, Some(tele))
        } else {
            let (res, rep) = run_schedule(
                &all_clauses,
                &provenance,
                id_gen,
                &schedule,
                &lowered.symbols,
                mrs_search::strategy::MlOptions {
                    log_dir: log_ml_data.clone(),
                    log_csv: ml_log_csv,
                    weights: ml_weights.clone(),
                    premise_keep: premise_keep.clone(),
                    #[cfg(feature = "parent-guidance")]
                    parent_weights: parent_weights.clone(),
                    #[cfg(feature = "parent-guidance")]
                    parent_threshold,
                },
                Some(actual_workers),
            );
            (res, rep, None)
        };

        let status = match &result {
            SearchResult::Refutation(..) => {
                if has_conjecture {
                    SzsStatus::Theorem
                } else {
                    SzsStatus::Unsatisfiable
                }
            }
            SearchResult::Saturated(_) => {
                // Positive saturation is emitted only by an independently
                // cross-checked certification path. Ordinary portfolio search
                // demotes saturation to GaveUp before it reaches this match.
                if has_conjecture {
                    SzsStatus::CounterSatisfiable
                } else {
                    SzsStatus::Satisfiable
                }
            }
            SearchResult::Timeout => SzsStatus::Timeout,
            SearchResult::GaveUp => SzsStatus::GaveUp,
            SearchResult::ResourceOut(_) => SzsStatus::ResourceOut,
        };

        (result, status, schedule_report, cert_telemetry)
    };

    let mut status = final_status;
    let result = final_result;

    // A search saturation is not a certified model.  The strict release path
    // certifies only the two completeness witnesses produced by the ordered
    // and SAT-backed tiers; an ordinary portfolio saturation is demoted before
    // it can reach a positive SZS status.  Never turn an incomplete or
    // heuristic saturation into a positive SZS model result.
    if self_check
        && matches!(
            result,
            SearchResult::Saturated(ref witness)
                if !matches!(
                    witness.reason(),
                    mrs_search::SaturationReason::GroundOrderedResolution
                        | mrs_search::SaturationReason::SatBackedGrounding
                )
        )
    {
        status = SzsStatus::GaveUp;
    }

    // Model certificates are emitted by the certifying tier and, under
    // --self-check, re-validated here before the status line claims a model:
    // the kernel is the only thing in the process that can confirm a finite
    // interpretation really satisfies the input.
    let model_certificate = match &result {
        SearchResult::Saturated(witness) => witness.model().cloned(),
        _ => None,
    };
    if self_check
        && let Some(certificate) = &model_certificate
        && matches!(
            status,
            SzsStatus::Satisfiable | SzsStatus::CounterSatisfiable
        )
    {
        let expected = if has_conjecture {
            Some("CounterSatisfiable")
        } else {
            Some("Satisfiable")
        };
        match certificate.validate(&problem, expected) {
            mrs_proof_kernel::model::ModelVerdict::Certified { .. } => {}
            mrs_proof_kernel::model::ModelVerdict::Rejected(reason) => {
                eprintln!("% Model certificate rejected by the strict kernel: {reason}");
                status = SzsStatus::GaveUp;
            }
            mrs_proof_kernel::model::ModelVerdict::Inconclusive(reason) => {
                eprintln!("% Model certificate inconclusive: {reason}");
                status = SzsStatus::GaveUp;
            }
        }
    }

    #[cfg(feature = "ml")]
    if let Some(log_dir) = &log_ml_data
        && matches!(status, SzsStatus::Theorem | SzsStatus::Unsatisfiable)
        && let Some(winning_strategy) = final_report
            .strategies
            .iter()
            .find(|s| matches!(s.result, SearchResult::Refutation(..)))
    {
        let elapsed = winning_strategy.elapsed_ms as f64 / 1000.0;
        let processed = winning_strategy.stats.processed;

        if elapsed >= 0.5 && processed >= 100 {
            use mrs_core::term_bank::TermBank;
            let mut bank = TermBank::new();
            let mut id_clauses = Vec::with_capacity(all_clauses.len());
            for c in &all_clauses {
                id_clauses.push(bank.clause_from_legacy(c));
            }
            let feats = mrs_core::ml::schedule_classifier::extract_schedule_features(
                &id_clauses,
                &bank,
                &lowered.symbols,
            );
            let sample = mrs_core::ml::sample::ScheduleSample {
                label_idx: winning_strategy.strategy_idx as u32,
                feats,
            };

            let log_path = std::path::Path::new(log_dir).join("schedule");
            std::fs::create_dir_all(&log_path).ok();
            let file_stem = format!("{}_schedule", problem_name);

            if !ml_log_csv
                && let Ok(mut w) =
                    std::fs::File::create(log_path.join(format!("{}.wincode", file_stem)))
            {
                let mut std_write = wincode::io::std_write::WriteAdapter::new(&mut w);
                let _ = wincode::serialize_into(&mut std_write, &sample);
            }
        }
    }

    // --- Asynchronous candidate certification check -----------------------
    let proof_certified = if self_check {
        matches!(result, SearchResult::Refutation(..))
    } else {
        true
    };

    if self_check
        && !proof_certified
        && matches!(status, SzsStatus::Theorem | SzsStatus::Unsatisfiable)
    {
        status = SzsStatus::GaveUp;
    }

    println!("{}", szs_status_line(status, problem_name));

    // Output proof if refutation found (skip in quiet mode: mrs-proover only
    // cares about the SZS line).
    #[cfg(feature = "proover")]
    let emit_extras = !quiet;
    #[cfg(not(feature = "proover"))]
    let emit_extras = true;

    // Proof-size budget. CASC limits how much output a system may produce ("a
    // limit, dependent on the disk space available, is imposed on the amount
    // of stdout and stderr output"; at least 10MB per system in CASC-23), and
    // an over-budget proof can get the process killed before it ever flushes
    // its SZS status line — losing the solve as well as the proof. So an
    // oversized proof is dropped with a diagnostic instead: the status line is
    // the part the competition scores.
    let proof_bytes = match &result {
        SearchResult::Refutation(_, tstp) => tstp.len(),
        _ => 0,
    };
    let proof_nodes = match &result {
        SearchResult::Refutation(_, tstp) => count_proof_nodes(tstp),
        _ => 0,
    };

    if emit_extras
        && let Some(certificate) = &model_certificate
        && matches!(
            status,
            SzsStatus::Satisfiable | SzsStatus::CounterSatisfiable
        )
    {
        // A `% Proof :` link inside the block, so a checker holding only this
        // output can find the problem the model has to satisfy.
        let model_block = format!(
            "% Proof : {}\n{}",
            path,
            certificate.to_szs_block(problem_name)
        );
        if model_block.len() > proof_bytes_limit {
            eprintln!(
                "% Model certificate omitted: {} bytes exceeds the --proof-bytes-limit of {} bytes",
                model_block.len(),
                proof_bytes_limit
            );
        } else {
            print!("{model_block}");
        }
    } else if emit_extras
        && matches!(
            status,
            SzsStatus::Satisfiable | SzsStatus::CounterSatisfiable
        )
    {
        eprintln!(
            "% No model certificate: this satisfiability result rests on the completeness \
             argument alone and earns no model credit."
        );
    }

    if emit_extras && proof_certified && proof_bytes > proof_bytes_limit {
        eprintln!(
            "% Proof omitted: {} bytes / {} nodes exceeds the --proof-bytes-limit of {} bytes",
            proof_bytes, proof_nodes, proof_bytes_limit
        );
    } else if emit_extras
        && proof_certified
        && let SearchResult::Refutation(_, tstp_proof) = &result
    {
        println!("{}", szs_output_start("Proof", problem_name));
        println!("{}", tstp_proof);
        println!("{}", szs_output_end("Proof", problem_name));
    }

    if emit_extras {
        print_statistics(
            status,
            start.elapsed(),
            &final_report,
            final_cert_telemetry.as_ref(),
            self_check,
            proof_certified,
            &result,
            (proof_nodes, proof_bytes, proof_bytes > proof_bytes_limit),
        );
    }
}

/// Counts the TSTP steps in a formatted proof, for telemetry.
fn count_proof_nodes(tstp: &str) -> usize {
    tstp.lines()
        .filter(|line| {
            let line = line.trim_start();
            line.starts_with("cnf(") || line.starts_with("fof(")
        })
        .count()
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
#[allow(clippy::too_many_arguments)]
fn print_statistics(
    status: SzsStatus,
    elapsed: Duration,
    report: &mrs_search::ScheduleReport,
    cert_telemetry: Option<&coordinator::CertificationTelemetry>,
    self_check: bool,
    proof_certified: bool,
    final_result: &mrs_search::SearchResult,
    (proof_nodes, proof_bytes, proof_omitted): (usize, usize, bool),
) {
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
    // Say *which* limit fired, and with what numbers. A run that stops on the
    // memory watchdog is a statement about the hardware; one that stops on the
    // clause ceiling is a statement about the search, and the two need different
    // responses. Reporting a bare "ResourceOut" makes them indistinguishable in
    // a benchmark sweep.
    if let mrs_search::SearchResult::ResourceOut(reason) = final_result {
        println!("% Resource limit: {}", reason.describe());
    }
    println!("% Time elapsed: {:.3} s", elapsed.as_secs_f64());
    if proof_bytes > 0 {
        println!(
            "% Proof: {} nodes, {} bytes{}",
            proof_nodes,
            proof_bytes,
            if proof_omitted {
                " (omitted: over the --proof-bytes-limit)"
            } else {
                ""
            }
        );
    }
    if let Some(mb) = peak_memory_mb() {
        println!("% Peak memory usage: {} MB", mb);
    }
    if let Some(tele) = cert_telemetry {
        println!("% Candidate certification audit:");
        println!("%   Candidates received: {}", tele.candidate_count);
        println!(
            "%   Candidates rejected: {}",
            tele.candidate_rejection_count
        );
        if let Some(idx) = tele.certified_candidate_index {
            println!("%   Certified candidate index: {}", idx);
            println!("%   Proof nodes: {}", tele.proof_nodes);
            println!("%   Proof bytes: {}", tele.proof_bytes);
            println!(
                "%   Time remaining at discovery: {:.3} s",
                tele.time_remaining_at_discovery.as_secs_f64()
            );
        }
        println!(
            "%   Elaboration time: {:.3} ms",
            tele.total_elaboration_time.as_secs_f64() * 1000.0
        );
        println!(
            "%   Strict kernel time: {:.3} ms",
            tele.total_strict_kernel_time.as_secs_f64() * 1000.0
        );
        println!(
            "%   Search workers: {}, Cert workers: {}, Oversubscribed: {}",
            tele.search_workers, tele.cert_workers, tele.cert_oversubscribed
        );
        if !tele.candidate_reasons.is_empty() {
            println!("%   Rejection reasons:");
            for reason in &tele.candidate_reasons {
                println!("%     - {}", reason);
            }
        }
    }
    println!("% ------------------------------");

    // Emit structured failure detail to stderr so the benchmark harness can
    // classify unsolved problems without re-parsing stdout.
    // Format: "% SZS detail <key=value> ..."
    // Always emitted (even on success) so casc.sh can parse it uniformly.
    let raw_search_result = report.raw_search_result();
    // Prefer the post-coordinator result when it is a refutation: the
    // async path (and --certify-ordered) may leave GaveUp in the schedule
    // report even after a candidate was strictly certified. Fall back to
    // the raw schedule result so a rejected candidate still reports
    // `result=Refutation` with `self_check=Rejected`.
    let final_is_refutation = matches!(final_result, SearchResult::Refutation(..));
    let search_result_name = if final_is_refutation {
        "Refutation"
    } else {
        match &raw_search_result {
            SearchResult::Refutation(..) => "Refutation",
            SearchResult::Saturated(_) => "Saturation",
            SearchResult::GaveUp => "GaveUp",
            SearchResult::Timeout => "Timeout",
            SearchResult::ResourceOut(_) => "ResourceOut",
        }
    };

    let mut detail_str = report.telemetry_detail(search_result_name);
    // Which limit fired goes in the detail line, which is what the benchmark
    // harness reads as `failure_detail` and grades. A bare `ResourceOut` makes
    // a memory-bound run indistinguishable from one that hit the clause ceiling,
    // and only the first one says anything about the hardware.
    if let mrs_search::SearchResult::ResourceOut(reason) = final_result {
        detail_str = format!(
            "{detail_str} resource_reason={} resource_detail=\"{}\"",
            reason.as_str(),
            reason.describe()
        );
    }
    if proof_bytes > 0 {
        detail_str = format!(
            "{detail_str} proof_nodes={proof_nodes} proof_bytes={proof_bytes} proof_emitted={}",
            !proof_omitted
        );
    }
    if self_check {
        let self_check_status = if final_is_refutation && proof_certified {
            "Certified"
        } else if matches!(raw_search_result, SearchResult::Refutation(..)) {
            "Rejected"
        } else {
            "Unchecked"
        };
        if let Some(tele) = cert_telemetry {
            detail_str = format!(
                "{} self_check={} candidates={} rejections={} cert_oversubscribed={} cert_search_workers={} cert_workers={}",
                detail_str,
                self_check_status,
                tele.candidate_count,
                tele.candidate_rejection_count,
                tele.cert_oversubscribed,
                tele.search_workers,
                tele.cert_workers,
            );
            if let Some(idx) = tele.certified_candidate_index {
                detail_str = format!("{} cert_idx={}", detail_str, idx);
            }
            detail_str = format!(
                "{} cert_elab_ms={} cert_kernel_ms={} cert_proof_nodes={} cert_proof_bytes={}",
                detail_str,
                tele.total_elaboration_time.as_millis(),
                tele.total_strict_kernel_time.as_millis(),
                tele.proof_nodes,
                tele.proof_bytes,
            );
        } else {
            detail_str = format!("{} self_check={}", detail_str, self_check_status);
        }
    }
    eprintln!("% SZS detail {}", detail_str);
}
