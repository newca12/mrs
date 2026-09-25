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
    let mut time_secs: u64 = 30;
    let mut schedule_name: Option<String> = None;
    let mut path: Option<String> = None;
    let mut log_ml_data: Option<String> = None;
    let mut ml_log_csv = false;
    let mut ml_weights: Option<String> = None;
    let mut workers: Option<usize> = None;
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
                        "Usage: mrs [--time <seconds>] [--schedule NAME] [--workers N] [--strategy N|--portfolio IDS] [--goal-transform MODE] [--certify-ordered] [--no-bce] [--no-ple] [--no-instgen] [--no-lrs] [--no-sharing] [--self-check] [--stats|--profile] [--profile-json] [--include-root DIR] <file.p>"
                    );
                    process::exit(1);
                }
                path = Some(arg);
            }
        }
    }
    let Some(path) = path else {
        eprintln!(
            "Usage: mrs [--time <seconds>] [--schedule NAME] [--workers N] [--strategy N|--portfolio IDS] [--goal-transform MODE] [--certify-ordered] [--no-bce] [--no-ple] [--no-instgen] [--no-lrs] [--no-sharing] [--self-check] [--stats|--profile] [--profile-json] [--include-root DIR] <file.p>"
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

    // Non-clausal FOF-level proof steps (NNF conversion, Skolemization, and
    // the explicit conjecture-negation step) produced alongside `all_clauses`.
    // These document the FOF-to-CNF translation for the final proof (see
    // CASC's evaluation criteria: "Translations from one form to another...
    // must be adequately documented"). They are never added to the live
    // given-clause search — see `Clause::formula`'s doc comment for why.
    let mut provenance: Vec<Clause> = Vec::new();

    // Clausify axioms directly
    for f in &lowered.axioms {
        let leaf_source = ClauseSource::Input {
            name: f.name.clone(),
            role: f.role.clone(),
        };
        let (steps, clauses) = mrs_cnf::clausify_with_provenance(
            &f.formula,
            &mut lowered.symbols,
            &mut id_gen,
            &f.name,
            leaf_source,
            None,
        );
        provenance.extend(steps);
        all_clauses.extend(clauses.into_iter().map(|c| c.with_distance(100)));
    }

    // Negate conjectures for refutation-based proving:
    // To prove P, we show that axioms ∧ ¬P is unsatisfiable.
    for f in &lowered.conjectures {
        // Explicit leaf citing the original (non-negated) conjecture.
        let conj_leaf_id = id_gen.next();
        provenance.push(Clause::new_formula_step(
            conj_leaf_id,
            f.formula.clone(),
            ClauseSource::Input {
                name: f.name.clone(),
                role: "conjecture".to_string(),
            },
        ));

        // The negation step itself is explicitly cited (status cth, single
        // parent = the conjecture leaf), per the CASC evaluation criteria:
        // "Proofs that negate the conjecture must correctly annotate the
        // step as status(cth) and have a single parent with the role
        // conjecture."
        let negated = Formula::neg(f.formula.clone());
        let (steps, clauses) = mrs_cnf::clausify_with_provenance(
            &negated,
            &mut lowered.symbols,
            &mut id_gen,
            &f.name,
            ClauseSource::Inference {
                rule: "negated_conjecture",
                parents: vec![conj_leaf_id].into(),
            },
            None,
        );
        provenance.extend(steps);
        all_clauses.extend(clauses.into_iter().map(|c| c.with_distance(0)));
    }

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
    // classifier). An explicit --schedule always wins. Works in any build.
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
        // Default to one worker per physical core, bounded by what the
        // available memory can support: every worker holds its own term
        // bank and indexes, so one-per-core is the wrong default on a small
        // host.
        let actual_workers =
            workers.unwrap_or_else(|| mrs_search::default_worker_count(num_cpus::get_physical()));
        if certify_ordered && (actual_workers != 1 || exact_strategy.is_none()) {
            eprintln!("Error: --certify-ordered requires --workers 1 and --strategy N");
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
        if let Some(gt) = goal_transform {
            for (config, _) in &mut schedule.strategies {
                config.goal_transformation = Some(gt);
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
                    problem_name: problem_name.to_string(),
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
                },
                Some(search_workers),
                Some(coordinator.clone()),
            );
            let tele = coordinator.finish();
            let mut res = coordinator.certified_result().unwrap_or(res);
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
            SearchResult::ResourceOut => SzsStatus::ResourceOut,
        };

        (result, status, schedule_report, cert_telemetry)
    };

    let mut status = final_status;
    let result = final_result;

    // A search saturation is not a certified model.  The strict release path
    // currently certifies refutations only; model certificates are validated
    // by a separate tool and are not produced by this binary.  Never turn an
    // incomplete or heuristic saturation into a positive SZS model result.
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

    if emit_extras
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
        );
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
fn print_statistics(
    status: SzsStatus,
    elapsed: Duration,
    report: &mrs_search::ScheduleReport,
    cert_telemetry: Option<&coordinator::CertificationTelemetry>,
    self_check: bool,
    proof_certified: bool,
    final_result: &mrs_search::SearchResult,
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
    println!("% Time elapsed: {:.3} s", elapsed.as_secs_f64());
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
            SearchResult::ResourceOut => "ResourceOut",
        }
    };

    let mut detail_str = report.telemetry_detail(search_result_name);
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
