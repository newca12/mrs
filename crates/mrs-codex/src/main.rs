use clap::{Parser, ValueEnum};
use crossbeam_channel::{Receiver, unbounded};
use mrs_tptp::ast::cnf::*;
use mrs_tptp::ast::fof::*;
use mrs_tptp::ast::*;
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;
use regex::Regex;
use rusqlite::{Connection, Result as SqliteResult, params};
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use sysinfo::System;
use tempfile::NamedTempFile;
use wait_timeout::ChildExt;
use walkdir::WalkDir;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum VerifyMode {
    /// Run the independent strict proof kernel only.
    Kernel,
    /// Run the existing competition-oriented verification checks.
    Competition,
    /// Do not verify proof output.
    #[value(name = "none")]
    None,
}

impl VerifyMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Kernel => "kernel",
            Self::Competition => "competition",
            Self::None => "none",
        }
    }
}

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Directory containing TPTP files
    folder: PathBuf,

    /// Path to the SQLite database file
    #[arg(long, default_value = "codex.db")]
    db: PathBuf,

    /// Name of the prover system (e.g., mrs-0.2.1)
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

    /// Proof verification policy: kernel, competition, or none
    #[arg(long, value_enum, default_value_t = VerifyMode::Competition)]
    verify_mode: VerifyMode,
}

#[derive(Debug, Clone)]
struct RunResult {
    problem_name: String,
    division: String,
    system_id: i64,
    parameter_id: i64,
    hardware_id: i64,
    timeout: u64,
    time_to_solve: Option<f64>,
    status: String,
    proover_validated: Option<String>,
    starexec_validated: Option<String>,
    time_to_verify: Option<f64>,
    kernel_validated: Option<String>,
    kernel_time: Option<f64>,
    mrs_validated: Option<String>,
    mrs_verify_time: Option<f64>,
    competition_validated: Option<String>,
    competition_time: Option<f64>,
    external_atp_validated: Option<String>,
    external_atp_time: Option<f64>,
}

fn extract_ground_truth_status(content: &str) -> Option<String> {
    for line in content.lines() {
        if line.trim().starts_with("% Status") {
            return line.find(':').map(|pos| {
                let status = line[pos + 1..].trim();
                status.split('(').next().unwrap_or("").trim().to_string()
            });
        }
    }
    None
}

fn check_cnf_literal(lit: &CNFLiteral, has_functions: &mut bool) {
    match lit {
        CNFLiteral::Positive(CNFAtomicFormula::Plain(_, args))
        | CNFLiteral::Negative(CNFAtomicFormula::Plain(_, args)) => {
            for arg in args {
                check_term(arg, has_functions);
            }
        }
        CNFLiteral::Equality(t1, t2) | CNFLiteral::Inequality(t1, t2) => {
            check_term(t1, has_functions);
            check_term(t2, has_functions);
        }
        _ => {}
    }
}

fn check_fof_formula(formula: &FOFFormula, has_eq: &mut bool, has_funcs: &mut bool) {
    match formula {
        FOFFormula::Atomic(FOFAtomicFormula::Plain(_, args)) => {
            for arg in args {
                check_term(arg, has_funcs);
            }
        }
        FOFFormula::Equality(t1, t2) | FOFFormula::Inequality(t1, t2) => {
            *has_eq = true;
            check_term(t1, has_funcs);
            check_term(t2, has_funcs);
        }
        FOFFormula::Atomic(_) => {}
        FOFFormula::Negation(f) => check_fof_formula(f, has_eq, has_funcs),
        FOFFormula::Binary { left, right, .. } => {
            check_fof_formula(left, has_eq, has_funcs);
            check_fof_formula(right, has_eq, has_funcs);
        }
        FOFFormula::Quantified { formula: f, .. } => check_fof_formula(f, has_eq, has_funcs),
        FOFFormula::Parens(f) => check_fof_formula(f, has_eq, has_funcs),
    }
}

fn check_term(term: &FOFTerm, has_funcs: &mut bool) {
    if let FOFTerm::Function(_, args) = term {
        if !args.is_empty() {
            *has_funcs = true;
        }
        for arg in args {
            check_term(arg, has_funcs);
        }
    }
}

fn file_has_equality(content: &str) -> bool {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('%') {
            continue;
        }
        if trimmed.contains('=') {
            return true;
        }
    }
    false
}

fn determine_division_from_ast(ast: &TPTPProblem, status: Option<&str>, content: &str) -> String {
    let mut has_equality = false;
    let mut has_functions = false;
    let mut all_unit = true;
    let mut all_equalities = true;
    let mut is_cnf = true;
    let mut is_thf = false;

    for input in &ast.formulas {
        let role = input.role();
        if role == FormulaRole::Type || role == FormulaRole::Definition {
            continue; // Skip types
        }

        match input {
            AnnotatedFormula::CNF(cnf) => {
                let lits = match &cnf.formula {
                    CNFStatement::Logical(CNFFormula::Disjunction(lits)) => lits.clone(),
                    CNFStatement::Logical(CNFFormula::Parens(inner)) => {
                        if let CNFFormula::Disjunction(lits) = &**inner {
                            lits.clone()
                        } else {
                            return "Other".to_string();
                        }
                    }
                };

                if lits.len() != 1 {
                    all_unit = false;
                }

                for lit in lits {
                    match lit {
                        CNFLiteral::Equality(..) | CNFLiteral::Inequality(..) => {
                            has_equality = true;
                        }
                        CNFLiteral::Positive(_) | CNFLiteral::Negative(_) => {
                            all_equalities = false;
                        }
                    }

                    // Check for functions arity > 0
                    check_cnf_literal(&lit, &mut has_functions);
                }
            }
            AnnotatedFormula::FOF(fof) => {
                is_cnf = false;
                all_unit = false;
                all_equalities = false;
                match &fof.formula {
                    FOFStatement::Logical(f) => {
                        check_fof_formula(f, &mut has_equality, &mut has_functions)
                    }
                    FOFStatement::Sequent(..) => return "Other".to_string(),
                }
            }
            AnnotatedFormula::THF(_) => {
                is_thf = true;
            }
            AnnotatedFormula::TFF(_) => return "TFF".to_string(),
            AnnotatedFormula::TCF(_) => return "TCF".to_string(),
            _ => return "Other".to_string(),
        }
    }

    if is_thf {
        if file_has_equality(content) {
            return "TEQ".to_string();
        } else {
            return "TNE".to_string();
        }
    }

    // 1. Effectively Propositional (EPR)
    if !has_functions {
        match status {
            Some("Satisfiable") | Some("CounterSatisfiable") => return "EPS".to_string(),
            Some("Unsatisfiable") | Some("Theorem") => return "EPU".to_string(),
            _ => return "EPR".to_string(),
        }
    }

    // 2. Unit Equality (UEQ)
    if is_cnf && all_unit && all_equalities {
        return "UEQ".to_string();
    }

    // 3. First-order Non-theorems (FNT)
    let is_fnt = matches!(status, Some("Satisfiable") | Some("CounterSatisfiable"));

    if is_fnt {
        if has_equality {
            "FNQ".to_string()
        } else {
            "FNN".to_string()
        }
    } else {
        if has_equality {
            "FEQ".to_string()
        } else {
            "FNE".to_string()
        }
    }
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
            division TEXT,
            system_id INTEGER NOT NULL,
            hardware_id INTEGER NOT NULL,
            parameter_id INTEGER NOT NULL,
            timeout INTEGER NOT NULL,
            time_to_solve REAL,
            status TEXT NOT NULL,
            proover_validated TEXT,
            starexec_validated TEXT,
            time_to_verify REAL,
            kernel_validated TEXT,
            kernel_time REAL,
            mrs_validated TEXT,
            mrs_verify_time REAL,
            competition_validated TEXT,
            competition_time REAL,
            external_atp_validated TEXT,
            external_atp_time REAL,
            FOREIGN KEY(system_id) REFERENCES systems(id),
            FOREIGN KEY(hardware_id) REFERENCES hardware(id),
            FOREIGN KEY(parameter_id) REFERENCES parameters(id),
            UNIQUE(problem_name, system_id, hardware_id, parameter_id, timeout)
        )",
        [],
    )?;
    // Schema migrations for already-existing databases
    let _ = conn.execute("ALTER TABLE results ADD COLUMN division TEXT", []);
    let _ = conn.execute("ALTER TABLE results ADD COLUMN starexec_validated TEXT", []);
    let _ = conn.execute("ALTER TABLE results ADD COLUMN time_to_verify REAL", []);
    for (name, sql_type) in [
        ("kernel_validated", "TEXT"),
        ("kernel_time", "REAL"),
        ("mrs_validated", "TEXT"),
        ("mrs_verify_time", "REAL"),
        ("competition_validated", "TEXT"),
        ("competition_time", "REAL"),
        ("external_atp_validated", "TEXT"),
        ("external_atp_time", "REAL"),
    ] {
        let _ = conn.execute(
            &format!("ALTER TABLE results ADD COLUMN {name} {sql_type}"),
            [],
        );
    }
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
        params![system_id, parameter_id, hardware_id, timeout as i64],
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

/// Verifies a TSTP proof (given as `stdout` from a prover run) using
/// `mrs-proover --only-mrs`, restricted to the `mrs` ATP fallback.
/// Returns "VerifiedGood", "VerifiedBad", or "Unknown".
fn verify_proof_with_proover(stdout: &str) -> String {
    let run = || -> Option<String> {
        // Write stdout (which should contain the TSTP proof) to a temp file.
        let mut temp_file = NamedTempFile::new().ok()?;
        temp_file.write_all(stdout.as_bytes()).ok()?;

        // Determine the path to mrs-proover. Assumed to be in the same dir as the current executable.
        let current_exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("mrs-codex"));
        let proover_exe = current_exe
            .parent()
            .unwrap_or(Path::new("."))
            .join("mrs-proover");

        // Run the verifier forcing it to use only 'mrs' as the ATP fallback.
        // Limit to 1 worker thread to prevent thread explosion under parallel codex jobs.
        // Restrict total verification budget to 10 seconds.
        let mut proover_cmd = Command::new(&proover_exe);
        proover_cmd.arg("--only-mrs");
        proover_cmd.arg("--workers");
        proover_cmd.arg("1");
        proover_cmd.arg("--time");
        proover_cmd.arg("10");
        proover_cmd.arg(temp_file.path());

        let mut proover_child = proover_cmd.stdout(Stdio::piped()).spawn().ok()?;

        // We give the verifier at most 60 seconds to verify.
        match proover_child.wait_timeout(Duration::from_secs(60)) {
            Ok(Some(_)) => {
                let p_output = proover_child.wait_with_output().ok()?;
                let p_stdout = String::from_utf8_lossy(&p_output.stdout);
                match extract_szs_status(&p_stdout).as_deref() {
                    Some("VerifiedGood") => Some("VerifiedGood".to_string()),
                    Some("VerifiedBad") => Some("VerifiedBad".to_string()),
                    _ => Some("Unknown".to_string()),
                }
            }
            _ => {
                let _ = proover_child.kill();
                let _ = proover_child.wait();
                Some("Unknown".to_string())
            }
        }
    };
    run().unwrap_or_else(|| "Unknown".to_string())
}

/// Verify a proof using only the independent strict proof kernel.
fn verify_proof_with_kernel(stdout: &str) -> String {
    let run = || -> Option<String> {
        let mut temp_file = NamedTempFile::new().ok()?;
        temp_file.write_all(stdout.as_bytes()).ok()?;

        let current_exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("mrs-codex"));
        let proover_exe = current_exe
            .parent()
            .unwrap_or(Path::new("."))
            .join("mrs-proover");
        let mut command = Command::new(proover_exe);
        command.args(["--strict", "--workers", "1", "--time", "10"]);
        let mut child = command
            .arg(temp_file.path())
            .stdout(Stdio::piped())
            .spawn()
            .ok()?;

        match child.wait_timeout(Duration::from_secs(60)) {
            Ok(Some(_)) => {
                let output = child.wait_with_output().ok()?;
                let stdout = String::from_utf8_lossy(&output.stdout);
                match extract_szs_status(&stdout).as_deref() {
                    Some("VerifiedGood") => Some("VerifiedGood".to_string()),
                    Some("VerifiedBad") => Some("VerifiedBad".to_string()),
                    _ => Some("Unknown".to_string()),
                }
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                Some("Unknown".to_string())
            }
        }
    };
    run().unwrap_or_else(|| "Unknown".to_string())
}

/// Verifies a TSTP proof using the StarExec entrypoint script.
/// Returns the parsed status ("VerifiedGood", "VerifiedBad", "Unknown") and the duration.
fn verify_proof_with_starexec(stdout: &str) -> (String, f64) {
    let run = || -> Option<(String, f64)> {
        let mut temp_file = NamedTempFile::new().ok()?;
        temp_file.write_all(stdout.as_bytes()).ok()?;

        let current_exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("mrs-codex"));
        let workspace_root = current_exe
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .unwrap_or(Path::new("."));
        let starexec_script =
            workspace_root.join("crates/mrs-bench/systems/mrs-proover/starexec_run_default");

        let mut cmd = Command::new(&starexec_script);
        cmd.env("STAREXEC_WALLCLOCK_LIMIT", "300");
        cmd.env("PROOVER_WORKERS", "1");
        cmd.arg(temp_file.path());

        let start_time = Instant::now();
        let mut child = cmd.stdout(Stdio::piped()).spawn().ok()?;

        match child.wait_timeout(Duration::from_secs(310)) {
            Ok(Some(_)) => {
                let duration = start_time.elapsed().as_secs_f64();
                let p_output = child.wait_with_output().ok()?;
                let p_stdout = String::from_utf8_lossy(&p_output.stdout);
                let status = match extract_szs_status(&p_stdout).as_deref() {
                    Some("VerifiedGood") => "VerifiedGood".to_string(),
                    Some("VerifiedBad") => "VerifiedBad".to_string(),
                    _ => "Unknown".to_string(),
                };
                Some((status, duration))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                let duration = start_time.elapsed().as_secs_f64();
                Some(("Unknown".to_string(), duration))
            }
        }
    };
    run().unwrap_or_else(|| ("Unknown".to_string(), 0.0))
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
             (problem_name, division, system_id, hardware_id, parameter_id, timeout, time_to_solve, status, proover_validated, starexec_validated, time_to_verify,
              kernel_validated, kernel_time, mrs_validated, mrs_verify_time, competition_validated, competition_time, external_atp_validated, external_atp_time)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
            params![
                result.problem_name,
                result.division,
                result.system_id,
                result.hardware_id,
                result.parameter_id,
                result.timeout as i64,
                result.time_to_solve,
                result.status,
                result.proover_validated,
                result.starexec_validated,
                result.time_to_verify,
                result.kernel_validated,
                result.kernel_time,
                result.mrs_validated,
                result.mrs_verify_time,
                result.competition_validated,
                result.competition_time,
                result.external_atp_validated,
                result.external_atp_time,
            ],
        );

        if let Err(e) = res {
            eprintln!("Error saving result for {}: {}", result.problem_name, e);
        }
    }
}

fn main() {
    let args = Args::parse();

    if args.timeout > i64::MAX as u64 {
        eprintln!("Error: --timeout must not exceed {} seconds.", i64::MAX);
        std::process::exit(1);
    }

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

    let parameter_text = args.params.clone().unwrap_or_else(|| args.cmd.clone());
    // Verification policy changes both the work performed and the meaning of
    // the recorded validation columns, so make it part of the resumable
    // configuration key.
    let parameters = format!(
        "{parameter_text} [verify-mode={}]",
        args.verify_mode.as_str()
    );
    let hardware = args.hardware.unwrap_or_else(detect_hardware);

    let conn = Connection::open(&args.db).expect("Failed to open SQLite database");
    init_db(&conn).expect("Failed to initialize database schema");

    let system_id = get_or_create_id(&conn, "systems", "name", &args.system)
        .expect("Failed to get/create system ID");
    let parameter_id = get_or_create_id(&conn, "parameters", "command_template", &parameters)
        .expect("Failed to get/create parameter ID");
    let hardware_id = get_or_create_id(&conn, "hardware", "description", &hardware)
        .expect("Failed to get/create hardware ID");

    let completed_problems =
        fetch_completed_problems(&conn, system_id, parameter_id, hardware_id, args.timeout)
            .expect("Failed to fetch completed problems");

    // We don't need the connection anymore in the main thread
    drop(conn);

    println!(
        "Found {} already completed problems for this configuration.",
        completed_problems.len()
    );
    println!("Proof verification mode: {:?}", args.verify_mode);
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
                let content = std::fs::read_to_string(file_path).unwrap_or_default();
                let status = extract_ground_truth_status(&content);
                let division = match mrs_tptp::parse_tptp(&content) {
                    Ok(ast) => determine_division_from_ast(&ast, status.as_deref(), &content),
                    Err(_) => "Other".to_string(),
                };

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
                let mut proover_validated: Option<String> = None;
                let mut starexec_validated: Option<String> = None;
                let mut time_to_verify: Option<f64> = None;
                let mut kernel_validated: Option<String> = None;
                let mut kernel_time: Option<f64> = None;
                let mut mrs_validated: Option<String> = None;
                let mut mrs_verify_time: Option<f64> = None;
                let mut competition_validated: Option<String> = None;
                let mut competition_time: Option<f64> = None;
                let mut external_atp_validated: Option<String> = None;
                let mut external_atp_time: Option<f64> = None;

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
                                        status_str = szs.clone();

                                        // Verify proof output according to the explicit policy.
                                        if szs == "Theorem" || szs == "Unsatisfiable" {
                                            match args.verify_mode {
                                                VerifyMode::Kernel => {
                                                    let verify_start = Instant::now();
                                                    let verdict = verify_proof_with_kernel(&stdout);
                                                    let elapsed =
                                                        verify_start.elapsed().as_secs_f64();
                                                    kernel_validated = Some(verdict.clone());
                                                    kernel_time = Some(elapsed);
                                                    proover_validated = Some(verdict);
                                                    time_to_verify = Some(elapsed);
                                                }
                                                VerifyMode::Competition => {
                                                    let mrs_start = Instant::now();
                                                    let mrs_verdict =
                                                        verify_proof_with_proover(&stdout);
                                                    mrs_verify_time =
                                                        Some(mrs_start.elapsed().as_secs_f64());
                                                    mrs_validated = Some(mrs_verdict.clone());
                                                    proover_validated = Some(mrs_verdict);
                                                    let competition_start = Instant::now();
                                                    let (st_val, st_time) =
                                                        verify_proof_with_starexec(&stdout);
                                                    starexec_validated = Some(st_val);
                                                    competition_time = Some(
                                                        competition_start.elapsed().as_secs_f64(),
                                                    );
                                                    competition_validated =
                                                        starexec_validated.clone();
                                                    external_atp_validated =
                                                        competition_validated.clone();
                                                    external_atp_time = Some(st_time);
                                                    time_to_verify = competition_time;
                                                }
                                                VerifyMode::None => {}
                                            }
                                        }
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
                                // Error waiting for process
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
                    division: division.clone(),
                    system_id,
                    parameter_id,
                    hardware_id,
                    timeout: args.timeout,
                    time_to_solve,
                    status: status_str.clone(),
                    proover_validated: proover_validated.clone(),
                    starexec_validated: starexec_validated.clone(),
                    time_to_verify,
                    kernel_validated,
                    kernel_time,
                    mrs_validated,
                    mrs_verify_time,
                    competition_validated,
                    competition_time,
                    external_atp_validated,
                    external_atp_time,
                };

                sender
                    .send(result)
                    .expect("Failed to send result to writer thread");

                let current = progress.fetch_add(1, Ordering::Relaxed) + 1;
                let time_disp = time_to_solve
                    .map(|t| format!("{:.2}s", t))
                    .unwrap_or_else(|| "N/A".to_string());

                let val_disp = match args.verify_mode {
                    VerifyMode::Kernel => proover_validated
                        .as_deref()
                        .map(|status| {
                            format!(
                                " [Kernel: {status} (verify: {:.2}s)]",
                                time_to_verify.unwrap_or(0.0)
                            )
                        })
                        .unwrap_or_default(),
                    VerifyMode::Competition => match (&proover_validated, &starexec_validated) {
                        (Some(pv), Some(sv)) => format!(
                            " [Proover: {}, StarExec: {} (verify: {:.2}s)]",
                            pv,
                            sv,
                            time_to_verify.unwrap_or(0.0)
                        ),
                        _ => "".to_string(),
                    },
                    VerifyMode::None => String::new(),
                };

                println!(
                    "[{:>5}/{}] {} ... {} ({}){}",
                    current, total_pending, problem_name, status_str, time_disp, val_disp
                );
            });
    });

    // Close the sender channel so the writer thread terminates after processing all messages
    drop(sender);

    writer_handle.join().expect("Writer thread panicked");

    println!("Processing complete.");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_args() -> Vec<&'static str> {
        vec![
            "mrs-codex",
            "problems",
            "--system",
            "mrs",
            "--cmd",
            "mrs {file}",
        ]
    }

    #[test]
    fn verifier_mode_defaults_to_competition() {
        let args = Args::try_parse_from(base_args()).expect("default arguments parse");
        assert_eq!(args.verify_mode, VerifyMode::Competition);
    }

    #[test]
    fn verifier_modes_parse_explicitly() {
        for (value, expected) in [
            ("kernel", VerifyMode::Kernel),
            ("competition", VerifyMode::Competition),
            ("none", VerifyMode::None),
        ] {
            let mut argv = base_args();
            argv.extend(["--verify-mode", value]);
            let args = Args::try_parse_from(argv).expect("verification mode parses");
            assert_eq!(args.verify_mode, expected);
        }
    }

    #[test]
    fn test_extract_ground_truth_status() {
        let content = "% Status: Theorem\n% Some other comment";
        assert_eq!(
            extract_ground_truth_status(content),
            Some("Theorem".to_string())
        );

        let content_with_space = "% Status             : CounterSatisfiable (hard)\n% Comment";
        assert_eq!(
            extract_ground_truth_status(content_with_space),
            Some("CounterSatisfiable".to_string())
        );

        let no_status = "% No status here";
        assert_eq!(extract_ground_truth_status(no_status), None);
    }

    #[test]
    fn test_determine_division() {
        let content_fne = "fof(a1, axiom, p(f(a))). fof(c1, conjecture, ~p(f(b))).";
        let ast = mrs_tptp::parse_tptp(content_fne).unwrap();
        let div = determine_division_from_ast(&ast, Some("Theorem"), content_fne);
        assert_eq!(div, "FNE");

        let content_feq = "fof(a1, axiom, f(a) = f(b)). fof(c1, conjecture, f(b) != f(c)).";
        let ast = mrs_tptp::parse_tptp(content_feq).unwrap();
        let div = determine_division_from_ast(&ast, Some("Theorem"), content_feq);
        assert_eq!(div, "FEQ");

        let content_ueq = "cnf(a1, axiom, f(a) = f(b)). cnf(c1, negated_conjecture, f(b) != f(c)).";
        let ast = mrs_tptp::parse_tptp(content_ueq).unwrap();
        let div = determine_division_from_ast(&ast, Some("Theorem"), content_ueq);
        assert_eq!(div, "UEQ");

        let content_fnn = "fof(a1, axiom, p(f(a))). fof(c1, conjecture, ~p(f(b))).";
        let ast = mrs_tptp::parse_tptp(content_fnn).unwrap();
        let div = determine_division_from_ast(&ast, Some("Satisfiable"), content_fnn);
        assert_eq!(div, "FNN");

        let content_fnq = "fof(a1, axiom, f(a) = f(b)). fof(c1, conjecture, f(b) != f(c)).";
        let ast = mrs_tptp::parse_tptp(content_fnq).unwrap();
        let div = determine_division_from_ast(&ast, Some("CounterSatisfiable"), content_fnq);
        assert_eq!(div, "FNQ");
    }
}
