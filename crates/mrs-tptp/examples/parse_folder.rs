//! Example: Recursively parse all TPTP files in a folder.
//!
//! This example mimics the behavior of ParserTestSuite.scala from scala-tptp-parser,
//! recursively finding and parsing all .p files in a given directory.
//!
//! Uses parallel processing with rayon for faster execution on multi-core systems.
//!
//! Run with: cargo run --example parse_folder [folder_path] [--timeout <ms>] [--threads <n>] [--verbose]
//!
//! Options:
//!   --timeout <ms>  Abort parsing of any file that takes longer than <ms> milliseconds
//!   --threads <n>   Set the number of threads (default: min(system threads, 8))
//!   --verbose       Show detailed list of timed out files (by default only count is shown)
//!
//! If no folder is provided, it defaults to tests/resources

use mrs_tptp::{AnnotatedFormula, parse_tptp};
#[cfg(feature = "cancellation")]
use mrs_tptp::{clear_cancel_flag, set_cancel_flag};
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

/// Recursively collect all .p files in a directory
fn collect_tptp_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    if dir.is_dir()
        && let Ok(entries) = fs::read_dir(dir)
    {
        {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    files.extend(collect_tptp_files(&path));
                } else if path.extension().is_some_and(|ext| ext == "p") {
                    files.push(path);
                }
            }
        }
    }

    files.sort();
    files
}

/// Result of parsing a single file
#[derive(Debug)]
enum ParseResult {
    Success {
        formula_count: usize,
        include_count: usize,
        read_ms: f64,
        elapsed_ms: f64,
    },
    RoundtripFail {
        formula_count: usize,
        read_ms: f64,
        elapsed_ms: f64,
        error: String,
    },
    Failed {
        read_ms: f64,
        elapsed_ms: f64,
        error: String,
    },
    ReadError {
        read_ms: f64,
        error: String,
    },
    Timeout {
        timeout_ms: u64,
    },
}

/// A job sent to the watchdog thread: fires `cancelled` at `deadline`.
struct WatchdogJob {
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
}

/// Parse a single file and return the result
fn parse_file(
    file: &Path,
    folder: &Path,
    timeout_ms: Option<u64>,
    watchdog_sender: Option<&mpsc::SyncSender<WatchdogJob>>,
) -> (PathBuf, String, ParseResult) {
    let relative_path = file
        .strip_prefix(folder)
        .unwrap_or(file)
        .display()
        .to_string();

    let read_start = Instant::now();
    let content = match fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            return (
                file.to_path_buf(),
                relative_path,
                ParseResult::ReadError {
                    read_ms: read_start.elapsed().as_micros() as f64 / 1000.0,
                    error: format!("Read error: {}", e),
                },
            );
        }
    };
    let read_ms = read_start.elapsed().as_micros() as f64 / 1000.0;

    // If timeout is specified, register with the shared watchdog thread and parse directly
    // in this Rayon worker — no per-file OS thread spawn, so N workers stay N workers.
    // The watchdog fires `cancelled` at the deadline; with the "cancellation" feature the
    // parser checks it internally and returns early.  Without that feature a runaway file
    // will block this worker until the parse finishes naturally (deadlines in the TPTP set
    // are rarely hit, so this is acceptable).
    if let Some(timeout) = timeout_ms {
        let cancelled = Arc::new(AtomicBool::new(false));
        let start = Instant::now();
        let deadline = start + Duration::from_millis(timeout);

        // Register with the shared watchdog thread
        if let Some(sender) = watchdog_sender {
            let _ = sender.send(WatchdogJob {
                deadline,
                cancelled: cancelled.clone(),
            });
        }

        // Bind the cancellation flag to this thread so the parser can check it
        #[cfg(feature = "cancellation")]
        set_cancel_flag(&cancelled);

        // Parse directly in this Rayon worker thread
        let result = parse_tptp(&content);
        let elapsed_ms = start.elapsed().as_micros() as f64 / 1000.0;

        #[cfg(feature = "cancellation")]
        clear_cancel_flag();

        // If the watchdog fired, report Timeout
        if cancelled.load(Ordering::Relaxed) {
            return (
                file.to_path_buf(),
                relative_path,
                ParseResult::Timeout {
                    timeout_ms: timeout,
                },
            );
        }

        match result {
            Ok(p) => {
                let formula_count = p.formulas.len();
                let include_count = p.includes.len();

                let mut roundtrip_error: Option<String> = None;
                for formula in &p.formulas {
                    // Check if the watchdog fired during the roundtrip pass
                    if cancelled.load(Ordering::Relaxed) {
                        return (
                            file.to_path_buf(),
                            relative_path,
                            ParseResult::Timeout {
                                timeout_ms: timeout,
                            },
                        );
                    }
                    let name = formula.name().to_string();
                    let pretty = formula.to_string();
                    match parse_tptp(&pretty) {
                        Ok(reparsed_problem) => {
                            if reparsed_problem.formulas.len() == 1 {
                                let reparsed_pretty = reparsed_problem.formulas[0].to_string();
                                if pretty != reparsed_pretty {
                                    roundtrip_error = Some(format!(
                                        "Round-trip mismatch for '{}':\n  Original:  {}\n  Reparsed:  {}",
                                        name, pretty, reparsed_pretty
                                    ));
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            roundtrip_error = Some(format!(
                                "Failed to reparse '{}': {}\n  Pretty: {}",
                                name, e, pretty
                            ));
                            break;
                        }
                    }
                }

                match roundtrip_error {
                    Some(error) => (
                        file.to_path_buf(),
                        relative_path,
                        ParseResult::RoundtripFail {
                            formula_count,
                            read_ms,
                            elapsed_ms,
                            error,
                        },
                    ),
                    None => (
                        file.to_path_buf(),
                        relative_path,
                        ParseResult::Success {
                            formula_count,
                            include_count,
                            read_ms,
                            elapsed_ms,
                        },
                    ),
                }
            }
            Err(e) => (
                file.to_path_buf(),
                relative_path,
                ParseResult::Failed {
                    read_ms,
                    elapsed_ms,
                    error: e.to_string(),
                },
            ),
        }
    } else {
        // No timeout - original behavior
        let start = Instant::now();
        let result = parse_tptp(&content);
        let elapsed = start.elapsed();
        let elapsed_ms = elapsed.as_micros() as f64 / 1000.0;

        match result {
            Ok(problem) => {
                let formula_count = problem.formulas.len();
                let include_count = problem.includes.len();

                // Parsing-reparsing comparison test (Pollack-consistency)
                for formula in &problem.formulas {
                    let pretty = formula.to_string();
                    match parse_single_formula(&pretty) {
                        Ok(reparsed) => {
                            let reparsed_pretty = reparsed.to_string();
                            if pretty != reparsed_pretty {
                                return (
                                    file.to_path_buf(),
                                    relative_path,
                                    ParseResult::RoundtripFail {
                                        formula_count,
                                        read_ms,
                                        elapsed_ms,
                                        error: format!(
                                            "Round-trip mismatch for '{}':\n  Original:  {}\n  Reparsed:  {}",
                                            formula.name(),
                                            pretty,
                                            reparsed_pretty
                                        ),
                                    },
                                );
                            }
                        }
                        Err(e) => {
                            return (
                                file.to_path_buf(),
                                relative_path,
                                ParseResult::RoundtripFail {
                                    formula_count,
                                    read_ms,
                                    elapsed_ms,
                                    error: format!(
                                        "Failed to reparse '{}': {}\n  Pretty: {}",
                                        formula.name(),
                                        e,
                                        pretty
                                    ),
                                },
                            );
                        }
                    }
                }

                (
                    file.to_path_buf(),
                    relative_path,
                    ParseResult::Success {
                        formula_count,
                        include_count,
                        read_ms,
                        elapsed_ms,
                    },
                )
            }
            Err(e) => (
                file.to_path_buf(),
                relative_path,
                ParseResult::Failed {
                    read_ms,
                    elapsed_ms,
                    error: e.to_string(),
                },
            ),
        }
    }
}

/// Parse a single annotated formula from a string
fn parse_single_formula(input: &str) -> Result<AnnotatedFormula<'_>, String> {
    match parse_tptp(input) {
        Ok(problem) => {
            if problem.formulas.len() == 1 {
                Ok(problem.formulas.into_iter().next().unwrap())
            } else {
                Err(format!(
                    "Expected 1 formula, got {}",
                    problem.formulas.len()
                ))
            }
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Background watchdog thread: receives `WatchdogJob` entries and sets each `cancelled` flag
/// when its deadline passes.  Exits when the sender side of the channel is dropped.
fn run_watchdog(receiver: mpsc::Receiver<WatchdogJob>) {
    // Pending jobs: (deadline, cancelled_arc).  At most N entries (one per Rayon worker).
    let mut pending: Vec<(Instant, Arc<AtomicBool>)> = Vec::new();
    loop {
        // Sleep until the next deadline (or 60 s if idle)
        let wait = if pending.is_empty() {
            Duration::from_secs(60)
        } else {
            let now = Instant::now();
            let min_deadline = pending.iter().map(|(d, _)| *d).min().unwrap();
            min_deadline
                .checked_duration_since(now)
                .unwrap_or(Duration::ZERO)
        };
        match receiver.recv_timeout(wait) {
            Ok(job) => {
                pending.push((job.deadline, job.cancelled));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        // Fire any expired deadlines
        let now = Instant::now();
        pending.retain(|(deadline, cancelled)| {
            if *deadline <= now {
                cancelled.store(true, Ordering::Relaxed);
                false
            } else {
                true
            }
        });
    }
}

fn main() {
    // Parse arguments first to get thread count
    let args: Vec<String> = env::args().collect();
    let num_threads = parse_thread_count(&args);

    // Configure rayon with larger stack size and limited parallelism to control memory
    let pool = ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .stack_size(64 * 1024 * 1024) // 64 MB stack per thread
        .build()
        .expect("Failed to build rayon thread pool");

    // Run everything inside the custom pool
    pool.install(main_inner);
}

/// Parse the --threads argument from command line args
fn parse_thread_count(args: &[String]) -> usize {
    let default_threads = std::cmp::min(rayon::current_num_threads(), 8);

    let mut i = 1;
    while i < args.len() {
        if args[i] == "--threads" && i + 1 < args.len() {
            return args[i + 1].parse().unwrap_or_else(|_| {
                eprintln!(
                    "Warning: Invalid --threads value, using default ({})",
                    default_threads
                );
                default_threads
            });
        }
        i += 1;
    }
    default_threads
}

fn main_inner() {
    let args: Vec<String> = env::args().collect();

    // Parse arguments
    let mut folder = PathBuf::from("tests/resources");
    let mut timeout_ms: Option<u64> = None;
    let mut verbose = false;

    let mut i = 1;
    while i < args.len() {
        if args[i] == "--timeout" {
            if i + 1 < args.len() {
                timeout_ms = Some(args[i + 1].parse().expect("Invalid timeout value"));
                i += 2;
            } else {
                eprintln!("Error: --timeout requires a value in milliseconds");
                std::process::exit(1);
            }
        } else if args[i] == "--threads" {
            // Already parsed in main(), skip here
            if i + 1 < args.len() {
                i += 2;
            } else {
                eprintln!("Error: --threads requires a value");
                std::process::exit(1);
            }
        } else if args[i] == "--verbose" || args[i] == "-v" {
            verbose = true;
            i += 1;
        } else if !args[i].starts_with('-') {
            folder = PathBuf::from(&args[i]);
            i += 1;
        } else {
            eprintln!("Unknown option: {}", args[i]);
            std::process::exit(1);
        }
    }

    if !folder.exists() {
        eprintln!("Error: folder '{}' does not exist", folder.display());
        std::process::exit(1);
    }

    println!("###################################");
    println!("TPTP Parser Test Suite (Parallel)");
    println!("###################################");
    println!();
    println!("Scanning folder: {}", folder.display());
    if let Some(timeout) = timeout_ms {
        println!("Timeout per file: {}ms", timeout);
    }
    println!();

    let files = collect_tptp_files(&folder);

    if files.is_empty() {
        println!("No .p files found in {}", folder.display());
        return;
    }

    let total_files = files.len();
    println!("Found {} TPTP files to parse", total_files);
    println!("Using {} threads", rayon::current_num_threads());
    println!();
    println!("{}", "=".repeat(70));

    // Progress counter for parallel execution
    let progress = AtomicUsize::new(0);

    // Start the single watchdog thread when a timeout is configured.
    // It replaces the old per-file polling OS threads: N Rayon workers + 1 watchdog
    // instead of N Rayon workers + N parser threads + N polling threads.
    let watchdog = timeout_ms.map(|_| {
        let (sender, receiver) = mpsc::sync_channel::<WatchdogJob>(256);
        let handle = thread::Builder::new()
            .name("tptp-watchdog".to_string())
            .spawn(move || run_watchdog(receiver))
            .expect("Failed to spawn watchdog thread");
        (sender, handle)
    });
    // Clone the sender so the parallel closure can share it without moving `watchdog`
    let watchdog_sender_opt: Option<mpsc::SyncSender<WatchdogJob>> =
        watchdog.as_ref().map(|(s, _)| s.clone());

    let start_time = Instant::now();

    // Parse all files in parallel
    let results: Vec<_> = files
        .par_iter()
        .map(|file| {
            let result = parse_file(file, &folder, timeout_ms, watchdog_sender_opt.as_ref());
            let current = progress.fetch_add(1, Ordering::Relaxed) + 1;

            // Print progress (may be out of order due to parallelism)
            let status = match &result.2 {
                ParseResult::Success {
                    formula_count,
                    include_count,
                    read_ms,
                    elapsed_ms,
                } => format!(
                    "OK (io:{:.2}ms parse:{:.2}ms, {} formulas, {} includes)",
                    read_ms, elapsed_ms, formula_count, include_count
                ),
                ParseResult::RoundtripFail {
                    formula_count,
                    read_ms,
                    elapsed_ms,
                    ..
                } => format!(
                    "ROUNDTRIP FAIL (io:{:.2}ms parse:{:.2}ms, {} formulas)",
                    read_ms, elapsed_ms, formula_count
                ),
                ParseResult::Failed {
                    read_ms,
                    elapsed_ms,
                    error,
                } => {
                    format!(
                        "FAILED (io:{:.2}ms parse:{:.2}ms)\n        Error: {}",
                        read_ms, elapsed_ms, error
                    )
                }
                ParseResult::ReadError { read_ms, error } => {
                    format!("FAILED (io:{:.2}ms, {})", read_ms, error)
                }
                ParseResult::Timeout { timeout_ms } => format!("TIMEOUT (>{}ms)", timeout_ms),
            };

            println!(
                "[{:>4}/{}] Parsing {} ... {}",
                current, total_files, result.1, status
            );

            result
        })
        .collect();

    let total_time = start_time.elapsed();

    // Shut down the watchdog: drop both sender ends so the thread exits, then join it
    drop(watchdog_sender_opt);
    if let Some((sender, handle)) = watchdog {
        drop(sender);
        let _ = handle.join();
    }

    // Collect statistics
    let mut successful = 0;
    let mut failed = 0;
    let mut roundtrip_failures = 0;
    let mut timeouts = 0;
    let mut total_formulas = 0;
    let mut total_includes = 0;
    let mut total_read_time_ms: f64 = 0.0;
    let mut total_parse_time_ms: f64 = 0.0;
    // (rel_path, read_ms, elapsed_ms, status_string) — used to print top-10 in the summary
    let mut top_entries: Vec<(String, f64, f64, String)> = Vec::new();
    let mut failed_files: Vec<(PathBuf, String)> = Vec::new();
    let mut roundtrip_failed_files: Vec<(PathBuf, String, String)> = Vec::new();
    let mut timeout_files: Vec<(PathBuf, u64)> = Vec::new();

    for (file, rel_path, result) in results {
        match result {
            ParseResult::Success {
                formula_count,
                include_count,
                read_ms,
                elapsed_ms,
            } => {
                successful += 1;
                total_formulas += formula_count;
                total_includes += include_count;
                total_read_time_ms += read_ms;
                total_parse_time_ms += elapsed_ms;
                top_entries.push((
                    rel_path,
                    read_ms,
                    elapsed_ms,
                    format!(
                        "OK (io:{:.2}ms parse:{:.2}ms, {} formulas, {} includes)",
                        read_ms, elapsed_ms, formula_count, include_count
                    ),
                ));
            }
            ParseResult::RoundtripFail {
                formula_count,
                read_ms,
                elapsed_ms,
                error,
            } => {
                successful += 1; // Parsing succeeded, roundtrip failed
                roundtrip_failures += 1;
                total_formulas += formula_count;
                total_read_time_ms += read_ms;
                total_parse_time_ms += elapsed_ms;
                top_entries.push((
                    rel_path.clone(),
                    read_ms,
                    elapsed_ms,
                    format!(
                        "ROUNDTRIP FAIL (io:{:.2}ms parse:{:.2}ms, {} formulas)",
                        read_ms, elapsed_ms, formula_count
                    ),
                ));
                roundtrip_failed_files.push((file, rel_path, error));
            }
            ParseResult::Failed {
                read_ms,
                elapsed_ms,
                error,
            } => {
                failed += 1;
                total_read_time_ms += read_ms;
                total_parse_time_ms += elapsed_ms;
                top_entries.push((
                    rel_path,
                    read_ms,
                    elapsed_ms,
                    format!("FAILED (io:{:.2}ms parse:{:.2}ms)", read_ms, elapsed_ms),
                ));
                failed_files.push((file, error));
            }
            ParseResult::ReadError { read_ms, error } => {
                failed += 1;
                total_read_time_ms += read_ms;
                failed_files.push((file, error));
            }
            ParseResult::Timeout { timeout_ms } => {
                timeouts += 1;
                timeout_files.push((file, timeout_ms));
            }
        }
    }

    println!();
    println!("{}", "=".repeat(70));
    println!();
    println!("###################################");
    println!("Summary");
    println!("###################################");
    println!();
    println!("Total files:      {}", total_files);
    println!(
        "Successful:       {} ({:.1}%)",
        successful,
        (successful as f64 / total_files as f64) * 100.0
    );
    println!(
        "Failed:           {} ({:.1}%)",
        failed,
        (failed as f64 / total_files as f64) * 100.0
    );
    println!(
        "Timeouts:         {} ({:.1}%)",
        timeouts,
        (timeouts as f64 / total_files as f64) * 100.0
    );
    println!();
    println!("Total formulas:   {}", total_formulas);
    println!("Total includes:   {}", total_includes);
    println!("Roundtrip fails:  {}", roundtrip_failures);
    println!();
    println!("Wall time:        {:.2}s", total_time.as_secs_f64());
    println!("I/O time (sum):   {:.2}ms", total_read_time_ms);
    println!("Parse time (sum): {:.2}ms", total_parse_time_ms);

    if total_files > 0 {
        println!(
            "Avg I/O/file:     {:.2}ms",
            total_read_time_ms / total_files as f64
        );
        println!(
            "Avg parse/file:   {:.2}ms",
            total_parse_time_ms / total_files as f64
        );
        println!(
            "Throughput:       {:.1} files/sec",
            total_files as f64 / total_time.as_secs_f64()
        );
    }

    if !top_entries.is_empty() {
        println!();
        println!("###################################");
        println!("Top 10 Slowest Files");
        println!("###################################");
        println!();
        top_entries.sort_by(|a, b| {
            (b.1 + b.2)
                .partial_cmp(&(a.1 + a.2))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for (i, (rel_path, _, _, status)) in top_entries.iter().take(10).enumerate() {
            println!("[{:>2}/10] Parsing {} ... {}", i + 1, rel_path, status);
        }
    }

    if !failed_files.is_empty() {
        println!();
        println!("###################################");
        println!("Parse Failed Files");
        println!("###################################");
        println!();
        for (file, error) in &failed_files {
            println!("  {} ", file.display());
            println!("    Error: {}", error);
        }
    }

    if !roundtrip_failed_files.is_empty() {
        println!();
        println!("###################################");
        println!("Roundtrip Failed Files");
        println!("###################################");
        println!();
        for (file, _rel_path, error) in &roundtrip_failed_files {
            println!("  {} ", file.display());
            println!("    {}", error);
            println!();
        }
    }

    if !timeout_files.is_empty() && verbose {
        println!();
        println!("###################################");
        println!("Timed Out Files");
        println!("###################################");
        println!();
        for (file, timeout) in &timeout_files {
            println!("  {}  (>{}ms)", file.display(), timeout);
        }
    }

    if !failed_files.is_empty() || !roundtrip_failed_files.is_empty() || !timeout_files.is_empty() {
        std::process::exit(1);
    } else {
        println!();
        println!("✓ All files parsed successfully!");
        println!("✓ Parsing-reparsing comparison successful for all files.");
    }
}
