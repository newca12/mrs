use clap::Parser;
use crossbeam_channel::{Receiver, unbounded};
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;
use regex::Regex;
use rusqlite::{Connection, Result as SqliteResult, params};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use sysinfo::System;
use wait_timeout::ChildExt;
use walkdir::WalkDir;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Directory containing TPTP files
    folder: PathBuf,

    /// Path to the SQLite database file
    #[arg(long, default_value = "codex.db")]
    db: PathBuf,

    /// Name of the prover system (e.g., mrs-0.1.9)
    #[arg(long)]
    system: String,

    /// Description of the hardware (auto-detected if not provided)
    #[arg(long)]
    hardware: Option<String>,

    /// Timeout in seconds
    #[arg(long, default_value_t = 30)]
    timeout: u64,

    /// Command template. Must include {file}. Can optionally include {timeout}.
    /// Example: "vampire --mode casc --time_limit {timeout} {file}"
    #[arg(long)]
    cmd: String,

    /// Parameters string to store in the database (defaults to the cmd string if not provided)
    #[arg(long)]
    params: Option<String>,

    /// Number of parallel jobs
    #[arg(short, long)]
    jobs: Option<usize>,
}

#[derive(Debug, Clone)]
struct RunResult {
    problem_name: String,
    system_id: i64,
    parameter_id: i64,
    hardware_id: i64,
    timeout: u64,
    time_to_solve: Option<f64>,
    status: String,
}

fn init_db(conn: &Connection) -> SqliteResult<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS systems (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT UNIQUE NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS hardware (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            description TEXT UNIQUE NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS parameters (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            command_template TEXT UNIQUE NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS results (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            problem_name TEXT NOT NULL,
            system_id INTEGER NOT NULL,
            hardware_id INTEGER NOT NULL,
            parameter_id INTEGER NOT NULL,
            timeout INTEGER NOT NULL,
            time_to_solve REAL,
            status TEXT NOT NULL,
            FOREIGN KEY(system_id) REFERENCES systems(id),
            FOREIGN KEY(hardware_id) REFERENCES hardware(id),
            FOREIGN KEY(parameter_id) REFERENCES parameters(id),
            UNIQUE(problem_name, system_id, hardware_id, parameter_id, timeout)
        )",
        [],
    )?;
    Ok(())
}

fn get_or_create_id(
    conn: &Connection,
    table: &str,
    column: &str,
    value: &str,
) -> SqliteResult<i64> {
    let insert_sql = format!("INSERT OR IGNORE INTO {} ({}) VALUES (?1)", table, column);
    conn.execute(&insert_sql, params![value])?;

    let select_sql = format!("SELECT id FROM {} WHERE {} = ?1", table, column);
    let mut stmt = conn.prepare(&select_sql)?;
    let id: i64 = stmt.query_row(params![value], |row| row.get(0))?;

    Ok(id)
}

fn fetch_completed_problems(
    conn: &Connection,
    system_id: i64,
    parameter_id: i64,
    hardware_id: i64,
    timeout: u64,
) -> SqliteResult<HashSet<String>> {
    let mut stmt = conn.prepare(
        "SELECT problem_name FROM results 
         WHERE system_id = ?1 AND parameter_id = ?2 
         AND hardware_id = ?3 AND timeout = ?4",
    )?;

    let problem_names = stmt.query_map(
        params![system_id, parameter_id, hardware_id, timeout],
        |row| row.get::<_, String>(0),
    )?;

    let mut completed = HashSet::new();
    for name in problem_names {
        completed.insert(name?);
    }
    Ok(completed)
}

fn extract_szs_status(output: &str) -> Option<String> {
    // Looks for things like `% SZS status Theorem` or `SZS status Unsatisfiable`
    let re = Regex::new(r"(?i)%?\s*SZS status\s+([A-Za-z0-9_]+)").unwrap();
    if let Some(caps) = re.captures(output) {
        return Some(caps.get(1).unwrap().as_str().to_string());
    }
    None
}

fn parse_cmd_template(template: &str, file: &Path, timeout: u64) -> Vec<String> {
    let file_str = file.to_string_lossy().to_string();
    let timeout_str = timeout.to_string();

    let replaced = template
        .replace("{file}", &file_str)
        .replace("{timeout}", &timeout_str);

    shlex::split(&replaced).unwrap_or_else(|| {
        eprintln!("Warning: failed to parse command template as a shell string, falling back to split_whitespace");
        replaced.split_whitespace().map(|s| s.to_string()).collect()
    })
}

fn detect_hardware() -> String {
    let mut sys = System::new_all();
    sys.refresh_all();

    let cpus = sys.cpus();
    let cpu_name = cpus
        .first()
        .map(|c| c.brand())
        .unwrap_or("Unknown CPU")
        .trim();
    let cores = System::physical_core_count().unwrap_or(cpus.len());
    let memory_gb = (sys.total_memory() as f64 / (1024.0 * 1024.0 * 1024.0)).round();
    let os = System::name().unwrap_or_else(|| "Unknown OS".to_string());
    let os_ver = System::os_version().unwrap_or_default();

    format!(
        "{} ({} cores, {} GB RAM, {} {})",
        cpu_name, cores, memory_gb, os, os_ver
    )
}

fn writer_thread(db_path: PathBuf, receiver: Receiver<RunResult>) {
    let conn = Connection::open(db_path).expect("Failed to open SQLite database in writer thread");
    // Optimize SQLite for bulk inserts
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;",
    )
    .expect("Failed to set PRAGMAs");

    for result in receiver {
        let res = conn.execute(
            "INSERT OR REPLACE INTO results 
             (problem_name, system_id, hardware_id, parameter_id, timeout, time_to_solve, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                result.problem_name,
                result.system_id,
                result.hardware_id,
                result.parameter_id,
                result.timeout,
                result.time_to_solve,
                result.status
            ],
        );

        if let Err(e) = res {
            eprintln!("Error saving result for {}: {}", result.problem_name, e);
        }
    }
}

fn main() {
    let args = Args::parse();

    if !args.folder.exists() {
        eprintln!(
            "Error: Directory '{}' does not exist.",
            args.folder.display()
        );
        std::process::exit(1);
    }

    if !args.cmd.contains("{file}") {
        eprintln!("Error: --cmd must contain the '{{file}}' placeholder.");
        std::process::exit(1);
    }

    let parameters = args.params.clone().unwrap_or_else(|| args.cmd.clone());
    let hardware = args.hardware.unwrap_or_else(detect_hardware);

    let conn = Connection::open(&args.db).expect("Failed to open SQLite database");
    init_db(&conn).expect("Failed to initialize database schema");

    let system_id = get_or_create_id(&conn, "systems", "name", &args.system)
        .expect("Failed to get/create system ID");
    let parameter_id = get_or_create_id(&conn, "parameters", "command_template", &parameters)
        .expect("Failed to get/create parameter ID");
    let hardware_id = get_or_create_id(&conn, "hardware", "description", &hardware)
        .expect("Failed to get/create hardware ID");

    let completed_problems = fetch_completed_problems(
        &conn,
        system_id,
        parameter_id,
        hardware_id,
        args.timeout,
    )
    .expect("Failed to fetch completed problems");

    // We don't need the connection anymore in the main thread
    drop(conn);

    println!(
        "Found {} already completed problems for this configuration.",
        completed_problems.len()
    );
    println!("Scanning {} for .p files...", args.folder.display());

    let mut pending_files = Vec::new();
    for entry in WalkDir::new(&args.folder)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.path().is_file() && entry.path().extension().is_some_and(|ext| ext == "p") {
            let relative_path = entry
                .path()
                .strip_prefix(&args.folder)
                .unwrap_or(entry.path());
            let problem_name = relative_path.to_string_lossy().to_string();

            if !completed_problems.contains(&problem_name) {
                pending_files.push((problem_name, entry.path().to_path_buf()));
            }
        }
    }

    // Sort to be deterministic
    pending_files.sort_by(|a, b| a.0.cmp(&b.0));

    let total_pending = pending_files.len();
    println!("Found {} files to process.", total_pending);

    if total_pending == 0 {
        println!("All files are already processed.");
        return;
    }

    let num_threads = args.jobs.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    });
    let pool = ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build()
        .expect("Failed to build rayon thread pool");

    let (sender, receiver) = unbounded::<RunResult>();

    let db_path = args.db.clone();
    let writer_handle = thread::spawn(move || {
        writer_thread(db_path, receiver);
    });

    let progress = Arc::new(AtomicUsize::new(0));

    pool.install(|| {
        pending_files
            .par_iter()
            .for_each(|(problem_name, file_path)| {
                let cmd_args = parse_cmd_template(&args.cmd, file_path, args.timeout);
                if cmd_args.is_empty() {
                    eprintln!("Error: Command template is empty.");
                    return;
                }

                let mut command = Command::new(&cmd_args[0]);
                if cmd_args.len() > 1 {
                    command.args(&cmd_args[1..]);
                }
                command.stdout(Stdio::piped());
                command.stderr(Stdio::piped());

                let start_time = Instant::now();

                let mut status_str = "Error".to_string();
                let mut time_to_solve = None;

                match command.spawn() {
                    Ok(mut child) => {
                        let timeout_duration = Duration::from_secs(args.timeout);
                        match child.wait_timeout(timeout_duration) {
                            Ok(Some(status)) => {
                                // Process exited before timeout
                                let elapsed = start_time.elapsed().as_secs_f64();
                                time_to_solve = Some(elapsed);

                                // Try to read stdout and stderr
                                if let Ok(output) = child.wait_with_output() {
                                    let stdout = String::from_utf8_lossy(&output.stdout);
                                    let stderr = String::from_utf8_lossy(&output.stderr);

                                    if let Some(szs) = extract_szs_status(&stdout)
                                        .or_else(|| extract_szs_status(&stderr))
                                    {
                                        status_str = szs;
                                    } else {
                                        if status.success() {
                                            status_str = "SuccessNoSZS".to_string();
                                        } else {
                                            status_str = "Error".to_string();
                                        }
                                    }
                                }
                            }
                            Ok(None) => {
                                // Process timed out
                                let _ = child.kill();
                                // Wait for the child to actually terminate
                                let _ = child.wait();
                                status_str = "Timeout".to_string();
                                time_to_solve = Some(args.timeout as f64);
                            }
                            Err(e) => {
                                eprintln!("Error waiting for process: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to spawn prover for {}: {}", problem_name, e);
                        status_str = "SpawnError".to_string();
                    }
                }

                let result = RunResult {
                    problem_name: problem_name.clone(),
                    system_id,
                    parameter_id,
                    hardware_id,
                    timeout: args.timeout,
                    time_to_solve,
                    status: status_str.clone(),
                };

                sender
                    .send(result)
                    .expect("Failed to send result to writer thread");

                let current = progress.fetch_add(1, Ordering::Relaxed) + 1;
                let time_disp = time_to_solve
                    .map(|t| format!("{:.2}s", t))
                    .unwrap_or_else(|| "N/A".to_string());
                println!(
                    "[{:>5}/{}] {} ... {} ({})",
                    current, total_pending, problem_name, status_str, time_disp
                );
            });
    });

    // Close the sender channel so the writer thread terminates after processing all messages
    drop(sender);

    writer_handle.join().expect("Writer thread panicked");

    println!("Processing complete.");
}
