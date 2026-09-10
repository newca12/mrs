use clap::{Parser, ValueEnum};
use crossbeam_channel::{Receiver, unbounded};
use mrs_tptp::ast::cnf::*;
use mrs_tptp::ast::fof::*;
use mrs_tptp::ast::*;
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;
use regex::Regex;
use rusqlite::{Connection, Result as SqliteResult, params};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
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
    /// Directory containing TPTP files (required for benchmarking or folder profiling)
    folder: Option<PathBuf>,

    /// Path to the SQLite database file
    #[arg(long, default_value = "codex.db")]
    db: PathBuf,

    /// Ingest CASC benchmark results from a run.csv file or results directory
    #[arg(long)]
    import_casc: Option<PathBuf>,

    /// Path to the competition problems root directory (default: auto-detected under crates/mrs-bench/problems/<edition>)
    #[arg(long)]
    problems_dir: Option<PathBuf>,

    /// Explicit corpus name (e.g. casc-30, casc-j13, tptp-v9.3.0). Auto-detected if omitted.
    #[arg(long)]
    corpus: Option<String>,

    /// Skip extracting problem profiles during CASC import (imports run results only)
    #[arg(long)]
    skip_profiles: bool,

    /// Name of the prover system (e.g., mrs-0.2.1)
    #[arg(long, default_value = "")]
    system: String,

    /// Description of the hardware (auto-detected if not provided)
    #[arg(long)]
    hardware: Option<String>,

    /// Timeout in seconds
    #[arg(long, default_value_t = 30)]
    timeout: u64,

    /// Command template. Must include {file}. Can optionally include {timeout}.
    /// Example: "vampire --mode casc --time_limit {timeout} {file}"
    #[arg(long, default_value = "")]
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

    /// Extract and store deep problem profiles only (does not run provers)
    #[arg(long)]
    profile_only: bool,

    /// Overwrite existing problem profiles in codex database
    #[arg(long)]
    overwrite_profiles: bool,
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
    corpus: String,
    canonical_name: String,
    is_competition: bool,
    expected: Option<String>,
    verdict: Option<String>,
    peak_memory_mb: Option<f64>,
    failure_detail: Option<String>,
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
    profile: Option<mrs_core::ProblemProfile>,
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
            corpus TEXT NOT NULL DEFAULT 'tptp',
            canonical_name TEXT,
            is_competition INTEGER NOT NULL DEFAULT 0,
            expected TEXT,
            verdict TEXT,
            peak_memory_mb REAL,
            failure_detail TEXT,
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
    let _ = conn.execute(
        "ALTER TABLE results ADD COLUMN corpus TEXT DEFAULT 'tptp'",
        [],
    );
    let _ = conn.execute("ALTER TABLE results ADD COLUMN canonical_name TEXT", []);
    let _ = conn.execute(
        "ALTER TABLE results ADD COLUMN is_competition INTEGER DEFAULT 0",
        [],
    );
    let _ = conn.execute("ALTER TABLE results ADD COLUMN expected TEXT", []);
    let _ = conn.execute("ALTER TABLE results ADD COLUMN verdict TEXT", []);
    let _ = conn.execute("ALTER TABLE results ADD COLUMN peak_memory_mb REAL", []);
    let _ = conn.execute("ALTER TABLE results ADD COLUMN failure_detail TEXT", []);
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

    conn.execute(
        "CREATE TABLE IF NOT EXISTS problem_profiles (
            problem_name TEXT PRIMARY KEY NOT NULL,
            domain TEXT,
            dialect TEXT,
            header_status TEXT,
            header_rating REAL,
            archetype TEXT NOT NULL,
            casc_division TEXT NOT NULL,
            recommended_schedule TEXT,
            recommended_engine TEXT,
            recommended_avatar INTEGER,
            recommended_sine INTEGER,
            num_clauses INTEGER,
            num_literals INTEGER,
            num_axioms INTEGER,
            num_conjectures INTEGER,
            num_variables INTEGER,
            avg_vars_per_clause REAL,
            max_vars_per_clause INTEGER,
            unit_ratio REAL,
            horn_ratio REAL,
            definite_ratio REAL,
            goal_clause_ratio REAL,
            ground_ratio REAL,
            equality_literal_ratio REAL,
            non_linear_var_ratio REAL,
            max_clause_len INTEGER,
            avg_clause_len REAL,
            max_pos_literals INTEGER,
            max_term_depth INTEGER,
            avg_term_depth REAL,
            max_term_size INTEGER,
            avg_term_size REAL,
            num_predicates INTEGER,
            num_functions INTEGER,
            num_constants INTEGER,
            max_fun_arity INTEGER,
            max_pred_arity INTEGER,
            skolem_symbols_count INTEGER,
            is_ueq INTEGER,
            is_peq INTEGER,
            is_fne INTEGER,
            is_feq INTEGER,
            is_epr INTEGER,
            is_fvo INTEGER,
            is_large_theory INTEGER,
            has_ac_symbols INTEGER,
            ac_symbols TEXT,
            has_identity_axiom INTEGER,
            has_inverse_axiom INTEGER,
            has_idempotence INTEGER,
            has_conjecture INTEGER,
            conjecture_clauses INTEGER,
            conjecture_literals INTEGER,
            conjecture_max_depth INTEGER,
            conjecture_symbol_overlap REAL,
            unique_conjecture_symbols TEXT,
            profile_complete INTEGER NOT NULL DEFAULT 1,
            raw_profile_json TEXT,
            corpus TEXT NOT NULL DEFAULT 'tptp',
            canonical_name TEXT,
            is_competition INTEGER NOT NULL DEFAULT 0
        )",
        [],
    )?;

    // Schema migration for profiles created before completeness or isolation was tracked.
    let _ = conn.execute(
        "ALTER TABLE problem_profiles ADD COLUMN profile_complete INTEGER NOT NULL DEFAULT 1",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE problem_profiles ADD COLUMN corpus TEXT DEFAULT 'tptp'",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE problem_profiles ADD COLUMN canonical_name TEXT",
        [],
    );
    let _ = conn.execute(
        "ALTER TABLE problem_profiles ADD COLUMN is_competition INTEGER DEFAULT 0",
        [],
    );

    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_problem_profiles_archetype ON problem_profiles(archetype)",
        [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_problem_profiles_division ON problem_profiles(casc_division)",
        [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_problem_profiles_domain ON problem_profiles(domain)",
        [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_problem_profiles_corpus ON problem_profiles(corpus)",
        [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_problem_profiles_canonical ON problem_profiles(canonical_name)",
        [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_problem_profiles_is_competition ON problem_profiles(is_competition)",
        [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_results_corpus ON results(corpus)",
        [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_results_canonical ON results(canonical_name)",
        [],
    );
    let _ = conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_results_is_competition ON results(is_competition)",
        [],
    );

    Ok(())
}

fn save_problem_profile(
    conn: &Connection,
    profile: &mrs_core::ProblemProfile,
    corpus: &str,
    canonical_name: &str,
    is_competition: bool,
) -> SqliteResult<()> {
    let ac_syms_json = serde_json::to_string(&profile.ac_symbols).unwrap_or_else(|_| "[]".into());
    let uniq_conj_syms_json =
        serde_json::to_string(&profile.unique_conjecture_symbols).unwrap_or_else(|_| "[]".into());
    let raw_json = serde_json::to_string(profile).unwrap_or_else(|_| "{}".into());

    conn.execute(
        "INSERT OR REPLACE INTO problem_profiles (
            problem_name, domain, dialect, header_status, header_rating,
            archetype, casc_division, recommended_schedule, recommended_engine,
            recommended_avatar, recommended_sine, num_clauses, num_literals,
            num_axioms, num_conjectures, num_variables, avg_vars_per_clause,
            max_vars_per_clause, unit_ratio, horn_ratio, definite_ratio,
            goal_clause_ratio, ground_ratio, equality_literal_ratio,
            non_linear_var_ratio, max_clause_len, avg_clause_len,
            max_pos_literals, max_term_depth, avg_term_depth, max_term_size,
            avg_term_size, num_predicates, num_functions, num_constants,
            max_fun_arity, max_pred_arity, skolem_symbols_count, is_ueq,
            is_peq, is_fne, is_feq, is_epr, is_fvo, is_large_theory,
            has_ac_symbols, ac_symbols, has_identity_axiom, has_inverse_axiom,
            has_idempotence, has_conjecture, conjecture_clauses,
            conjecture_literals, conjecture_max_depth, conjecture_symbol_overlap,
            unique_conjecture_symbols, profile_complete, raw_profile_json,
            corpus, canonical_name, is_competition
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
            ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28,
            ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37, ?38, ?39, ?40, ?41,
            ?42, ?43, ?44, ?45, ?46, ?47, ?48, ?49, ?50, ?51, ?52, ?53, ?54,
            ?55, ?56, ?57, ?58, ?59, ?60, ?61
        )",
        params![
            profile.problem_name,
            profile.domain,
            profile.dialect,
            profile.header_status,
            profile.header_rating,
            profile.archetype.as_str(),
            profile.casc_division,
            profile.recommended_schedule,
            profile.recommended_engine,
            profile.recommended_avatar as i64,
            profile.recommended_sine as i64,
            profile.num_clauses as i64,
            profile.num_literals as i64,
            profile.num_axioms as i64,
            profile.num_conjectures as i64,
            profile.num_variables as i64,
            profile.avg_vars_per_clause as f64,
            profile.max_vars_per_clause as i64,
            profile.unit_ratio as f64,
            profile.horn_ratio as f64,
            profile.definite_ratio as f64,
            profile.goal_clause_ratio as f64,
            profile.ground_ratio as f64,
            profile.equality_literal_ratio as f64,
            profile.non_linear_var_ratio as f64,
            profile.max_clause_len as i64,
            profile.avg_clause_len as f64,
            profile.max_pos_literals as i64,
            profile.max_term_depth as i64,
            profile.avg_term_depth as f64,
            profile.max_term_size as i64,
            profile.avg_term_size as f64,
            profile.num_predicates as i64,
            profile.num_functions as i64,
            profile.num_constants as i64,
            profile.max_fun_arity as i64,
            profile.max_pred_arity as i64,
            profile.skolem_symbols_count as i64,
            profile.is_ueq as i64,
            profile.is_peq as i64,
            profile.is_fne as i64,
            profile.is_feq as i64,
            profile.is_epr as i64,
            profile.is_fvo as i64,
            profile.is_large_theory as i64,
            profile.has_ac_symbols as i64,
            ac_syms_json,
            profile.has_identity_axiom as i64,
            profile.has_inverse_axiom as i64,
            profile.has_idempotence as i64,
            profile.has_conjecture as i64,
            profile.conjecture_clauses as i64,
            profile.conjecture_literals as i64,
            profile.conjecture_max_depth as i64,
            profile.conjecture_symbol_overlap as f64,
            uniq_conj_syms_json,
            profile.profile_complete as i64,
            raw_json,
            corpus,
            canonical_name,
            is_competition as i64,
        ],
    )?;
    Ok(())
}

fn fetch_profiled_problems(conn: &Connection) -> SqliteResult<HashSet<String>> {
    let mut stmt =
        conn.prepare("SELECT problem_name FROM problem_profiles WHERE profile_complete = 1")?;
    let rows = stmt.query_map([], |row| row.get(0))?;
    let mut names = HashSet::new();
    for name in rows {
        names.insert(name?);
    }
    Ok(names)
}

fn read_header_prefix(path: &Path) -> String {
    let Ok(file) = std::fs::File::open(path) else {
        return String::new();
    };
    let mut prefix = String::new();
    let _ = file.take(64 * 1024).read_to_string(&mut prefix);
    prefix
}

fn extract_domain_from_name(name: &str) -> String {
    let base = Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(name);
    if base.len() >= 3 && base.chars().take(3).all(|c| c.is_ascii_alphabetic()) {
        return base[0..3].to_uppercase();
    }
    if let Some((dir, _)) = name.split_once('/')
        && dir.len() >= 3
        && dir.chars().take(3).all(|c| c.is_ascii_alphabetic())
    {
        return dir[0..3].to_uppercase();
    }
    "UNK".to_string()
}

/// Normalizes any TPTP problem identifier to its canonical "DOMAIN/NAME.p" representation.
/// E.g.: "AGT005+1.p" -> "AGT/AGT005+1.p", "casc-30/FEQ/AGT005+1.p" -> "AGT/AGT005+1.p",
/// "GRP123-4.004" -> "GRP/GRP123-4.004.p".
fn canonical_tptp_name(raw_name: &str) -> String {
    let filename = Path::new(raw_name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(raw_name);
    let with_ext = if filename.ends_with(".p") {
        filename.to_string()
    } else {
        format!("{}.p", filename)
    };
    if with_ext.len() >= 3 {
        let prefix = &with_ext[..3];
        if prefix.chars().all(|c| c.is_ascii_alphabetic()) {
            return format!("{}/{}", prefix.to_ascii_uppercase(), with_ext);
        }
    }
    with_ext
}

/// Detects whether a folder is a CASC competition folder (e.g. casc-30, casc-j13) or general TPTP,
/// returning (corpus_name, is_competition, detected_tptp_root).
fn detect_corpus_and_competition(
    folder: &Path,
    explicit_corpus: Option<&str>,
) -> (String, bool, PathBuf) {
    if let Some(c) = explicit_corpus {
        let is_comp = c.to_ascii_lowercase().starts_with("casc");
        let detected_tptp = if folder.join("Axioms").is_dir() {
            folder.to_path_buf()
        } else if folder.join("TPTP-v9.3.0").is_dir() {
            folder.join("TPTP-v9.3.0")
        } else {
            folder.to_path_buf()
        };
        return (c.to_string(), is_comp, detected_tptp);
    }

    let folder_str = folder.to_string_lossy().to_ascii_lowercase();
    let file_name = folder
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let is_casc_folder = file_name.starts_with("casc-")
        || file_name.starts_with("casc_")
        || folder_str.contains("/problems/casc-")
        || folder_str.contains("/problems/casc_");

    if is_casc_folder {
        let edition = if let Some(pos) = folder_str.rfind("casc-") {
            let sub = &folder_str[pos..];
            sub.split('/').next().unwrap_or("casc")
        } else if let Some(pos) = folder_str.rfind("casc_") {
            let sub = &folder_str[pos..];
            sub.split('/').next().unwrap_or("casc")
        } else {
            &file_name
        };
        (edition.to_string(), true, folder.to_path_buf())
    } else {
        let detected_tptp = if folder.join("TPTP-v9.3.0").is_dir() {
            folder.join("TPTP-v9.3.0")
        } else if folder.join("../TPTP-v9.3.0").is_dir() {
            folder.join("../TPTP-v9.3.0")
        } else {
            folder.to_path_buf()
        };
        ("tptp".to_string(), false, detected_tptp)
    }
}

/// Simple CSV line splitter handling quoted strings with commas and escaped quotes.
fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '"' {
            if in_quotes && chars.peek() == Some(&'"') {
                chars.next();
                current.push('"');
            } else {
                in_quotes = !in_quotes;
            }
        } else if c == ',' && !in_quotes {
            fields.push(current.trim().to_string());
            current = String::new();
        } else {
            current.push(c);
        }
    }
    fields.push(current.trim().to_string());
    fields
}

/// Parses a companion run.log file to extract edition, division time limits, and TPTP root.
fn parse_companion_run_log(
    log_path: &Path,
) -> (
    Option<String>,
    std::collections::HashMap<String, u64>,
    Option<PathBuf>,
) {
    let mut edition = None;
    let mut time_limits = std::collections::HashMap::new();
    let mut tptp_root = None;

    if let Ok(content) = std::fs::read_to_string(log_path) {
        for line in content.lines() {
            let line = line.trim();
            if let Some(val) = line.strip_prefix("[casc] Edition:").map(|s| s.trim()) {
                edition = Some(val.to_string());
            } else if let Some(val) = line.strip_prefix("[casc] TPTP:").map(|s| s.trim()) {
                tptp_root = Some(PathBuf::from(val));
            } else if let Some(val) = line.strip_prefix("[casc] Time limits:").map(|s| s.trim()) {
                for part in val.split(',') {
                    if let Some((div, time_str)) = part.trim().split_once('=') {
                        let clean_time = time_str.trim_end_matches('s').trim();
                        if let Ok(secs) = clean_time.parse::<u64>() {
                            time_limits.insert(div.trim().to_ascii_lowercase(), secs);
                        }
                    }
                }
            }
        }
    }

    (edition, time_limits, tptp_root)
}

/// Standard CASC division timeouts (seconds).
fn default_casc_timeout(division: &str, edition: &str) -> u64 {
    let div = division.to_ascii_lowercase();
    let is_j13 = edition.to_ascii_lowercase().contains("j13");
    match div.as_str() {
        "fne" | "feq" | "ueq" | "tne" | "teq" | "fnn" | "fnq" => {
            if is_j13 {
                180
            } else {
                240
            }
        }
        "eps" | "epu" | "tfi" | "tfe" | "tfn" => 120,
        "icu" => 480,
        "slh" => 15,
        _ => 120,
    }
}

/// Locates the competition problem directory containing division subdirectories and Axioms/.
fn find_competition_problems_dir(
    edition: &str,
    cli_dir: Option<&Path>,
    log_tptp: Option<&Path>,
) -> Option<PathBuf> {
    if let Some(dir) = cli_dir
        && dir.is_dir()
    {
        return Some(dir.to_path_buf());
    }
    if let Some(dir) = log_tptp
        && dir.is_dir()
    {
        return Some(dir.to_path_buf());
    }
    let candidates = [
        format!("crates/mrs-bench/problems/{}", edition),
        format!("../mrs-bench/problems/{}", edition),
        format!("problems/{}", edition),
        format!(
            "/home/fr22192/EDLA/git/mrs/crates/mrs-bench/problems/{}",
            edition
        ),
    ];
    for c in &candidates {
        let p = PathBuf::from(c);
        if p.is_dir() {
            return Some(p);
        }
    }
    None
}

fn parse_headers_from_str(content: &str) -> (Option<String>, Option<f32>) {
    let mut status = None;
    let mut rating = None;

    for line in content.lines().take(120) {
        let trimmed = line.trim();
        if !trimmed.starts_with('%') {
            if !trimmed.is_empty() {
                break;
            }
            continue;
        }

        let without_pct = trimmed.trim_start_matches('%').trim();
        if without_pct.starts_with("Status")
            && let Some((_, val)) = without_pct.split_once(':')
            && let Some(token) = val.split_whitespace().next()
            && !token.is_empty()
        {
            status = Some(token.to_string());
        } else if without_pct.starts_with("Rating")
            && let Some((_, val)) = without_pct.split_once(':')
            && let Some(token) = val.split_whitespace().next()
            && let Ok(r) = token.parse::<f32>()
        {
            rating = Some(r);
        }
    }

    (status, rating)
}

fn extract_problem_profile_from_file(
    path: &Path,
    problem_name: &str,
    tptp_root: Option<&Path>,
) -> Option<mrs_core::ProblemProfile> {
    let file_size = std::fs::metadata(path).ok().map(|m| m.len()).unwrap_or(0);
    if file_size > 20 * 1024 * 1024 {
        let content_prefix = read_header_prefix(path);
        let (header_status, header_rating) = parse_headers_from_str(&content_prefix);
        let domain = extract_domain_from_name(problem_name);
        let profile = mrs_core::ProblemProfile::incomplete(
            problem_name,
            domain,
            "Unknown".to_string(),
            header_status,
            header_rating,
        );
        return Some(profile);
    }

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return None,
    };
    let problem = match mrs_tptp::parse_tptp(&content) {
        Ok(p) => p,
        Err(_) => {
            let (header_status, header_rating) = parse_headers_from_str(&content);
            let domain = extract_domain_from_name(problem_name);
            let profile = mrs_core::ProblemProfile::incomplete(
                problem_name,
                domain,
                "Unknown".to_string(),
                header_status,
                header_rating,
            );
            return Some(profile);
        }
    };
    let mut lowered = mrs::lowering::lower_problem(&problem);
    if !problem.includes.is_empty() {
        let base_dir = path.parent().unwrap_or(Path::new("."));
        // Priority for include resolution:
        // 1. Explicit tptp_root (if provided)
        // 2. An ancestor of base_dir that contains an Axioms/ subdirectory (ensures sliced CASC or local axioms are used)
        // 3. Fallback to $TPTP environment variable only if no local Axioms/ directory exists
        let local_ancestor_root = {
            let mut cur = base_dir.to_path_buf();
            let mut found = None;
            loop {
                if cur.join("Axioms").is_dir() {
                    found = Some(cur);
                    break;
                }
                if !cur.pop() {
                    break;
                }
            }
            found
        };
        let env_tptp = std::env::var("TPTP").ok().map(PathBuf::from);
        let effective_tptp = tptp_root
            .map(Path::to_path_buf)
            .or(local_ancestor_root)
            .or(env_tptp);
        let _ = mrs::include::resolve_and_lower(
            &problem,
            &mut lowered,
            base_dir,
            effective_tptp.as_deref(),
        );
    }
    if lowered.axioms.len() > 30_000 {
        let (header_status, header_rating) = parse_headers_from_str(&content);
        let domain = extract_domain_from_name(problem_name);
        let mut profile = mrs_core::ProblemProfile::incomplete(
            problem_name,
            domain,
            "Unknown".to_string(),
            header_status,
            header_rating,
        );
        profile.num_axioms = lowered.input_axioms_count;
        profile.num_conjectures = lowered.input_conjectures_count;
        profile.is_large_theory = true;
        profile.archetype = mrs_core::ProblemArchetype::LargeTheory;
        profile.recommended_sine = true;
        return Some(profile);
    }

    let mut id_gen = lowered.id_gen.clone();
    let mut all_clauses = lowered.cnf_clauses;
    if all_clauses.len() > 50_000 {
        let domain = extract_domain_from_name(problem_name);
        let (header_status, header_rating) = parse_headers_from_str(&content);
        let mut profile = mrs_core::ProblemProfile::incomplete(
            problem_name,
            domain,
            "Unknown".to_string(),
            header_status,
            header_rating,
        );
        profile.num_axioms = lowered.input_axioms_count;
        profile.num_conjectures = lowered.input_conjectures_count;
        return Some(profile);
    }
    for f in lowered.axioms {
        let (_, clauses) = mrs_cnf::clausify_with_provenance(
            &f.formula,
            &mut lowered.symbols,
            &mut id_gen,
            &f.name,
            mrs_core::clause::ClauseSource::Input {
                name: f.name.clone(),
                role: f.role.clone(),
            },
            None,
        );
        all_clauses.extend(clauses);
        if all_clauses.len() > 50_000 {
            let domain = extract_domain_from_name(problem_name);
            let (header_status, header_rating) = parse_headers_from_str(&content);
            let mut profile = mrs_core::ProblemProfile::incomplete(
                problem_name,
                domain,
                "FOF".to_string(),
                header_status,
                header_rating,
            );
            profile.num_axioms = lowered.input_axioms_count;
            profile.num_conjectures = lowered.input_conjectures_count;
            return Some(profile);
        }
    }
    for f in lowered.conjectures {
        let negated = mrs_core::Formula::neg(f.formula.clone());
        let (_, clauses) = mrs_cnf::clausify_with_provenance(
            &negated,
            &mut lowered.symbols,
            &mut id_gen,
            &f.name,
            mrs_core::clause::ClauseSource::Inference {
                rule: "negated_conjecture",
                parents: vec![mrs_core::clause::ClauseId(0)].into(),
            },
            None,
        );
        all_clauses.extend(clauses.into_iter().map(|c| c.with_distance(0)));
    }
    let mut profile = mrs::analyze::analyze_problem_with_counts(
        &path.to_string_lossy(),
        &problem,
        &lowered.symbols,
        &all_clauses,
        lowered.input_axioms_count,
        lowered.input_conjectures_count,
    );
    profile.problem_name = problem_name.to_string();
    Some(profile)
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

fn fetch_all_results_problems(conn: &Connection) -> SqliteResult<HashSet<String>> {
    let mut stmt = conn.prepare("SELECT DISTINCT problem_name FROM results")?;
    let problem_names = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut names = HashSet::new();
    for name in problem_names {
        names.insert(name?);
    }
    Ok(names)
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
        if let Some(profile) = &result.profile
            && let Err(e) = save_problem_profile(
                &conn,
                profile,
                &result.corpus,
                &result.canonical_name,
                result.is_competition,
            )
        {
            eprintln!("Error saving profile for {}: {}", result.problem_name, e);
        }

        let res = conn.execute(
            "INSERT OR REPLACE INTO results 
             (problem_name, division, system_id, hardware_id, parameter_id, timeout, time_to_solve, status,
              corpus, canonical_name, is_competition, expected, verdict, peak_memory_mb, failure_detail,
              proover_validated, starexec_validated, time_to_verify,
              kernel_validated, kernel_time, mrs_validated, mrs_verify_time, competition_validated, competition_time, external_atp_validated, external_atp_time)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)",
            params![
                result.problem_name,
                result.division,
                result.system_id,
                result.hardware_id,
                result.parameter_id,
                result.timeout as i64,
                result.time_to_solve,
                result.status,
                result.corpus,
                result.canonical_name,
                result.is_competition as i64,
                result.expected,
                result.verdict,
                result.peak_memory_mb,
                result.failure_detail,
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
            eprintln!("Error inserting result for {}: {}", result.problem_name, e);
        }
    }
}

fn run_profile_only(args: &Args) {
    let folder = args
        .folder
        .as_ref()
        .expect("Folder is required for profiling");
    let conn = Connection::open(&args.db).expect("Failed to open SQLite database");
    init_db(&conn).expect("Failed to initialize database schema");

    let profiled_problems = if args.overwrite_profiles {
        HashSet::new()
    } else {
        fetch_profiled_problems(&conn).expect("Failed to fetch profiled problems")
    };
    drop(conn);

    println!(
        "Found {} already profiled problems in {}.",
        profiled_problems.len(),
        args.db.display()
    );

    let (corpus, is_competition, detected_tptp) =
        detect_corpus_and_competition(folder, args.corpus.as_deref());

    // Determine the base folder to scan
    let base_folder = if is_competition {
        folder.clone()
    } else if folder.join("TPTP-v9.3.0/Problems").is_dir() {
        folder.join("TPTP-v9.3.0/Problems")
    } else if folder.join("Problems").is_dir() {
        folder.join("Problems")
    } else {
        folder.clone()
    };

    println!("Scanning {} for .p files...", base_folder.display());

    let mut pending_files = Vec::new();
    for entry in WalkDir::new(&base_folder)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.path().is_file() && entry.path().extension().is_some_and(|ext| ext == "p") {
            let relative_path = entry
                .path()
                .strip_prefix(&base_folder)
                .unwrap_or(entry.path());
            let problem_name = if is_competition {
                format!("{}/{}", corpus, relative_path.to_string_lossy())
            } else {
                relative_path.to_string_lossy().to_string()
            };

            if !profiled_problems.contains(&problem_name) {
                pending_files.push((problem_name, entry.path().to_path_buf()));
            }
        }
    }

    pending_files.sort_by(|a, b| a.0.cmp(&b.0));
    let total_pending = pending_files.len();
    println!("Found {} files to profile.", total_pending);
    if total_pending == 0 {
        println!("All files are already profiled.");
        return;
    }

    let num_threads = args.jobs.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    });
    let pool = ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .stack_size(64 * 1024 * 1024)
        .build()
        .expect("Failed to build rayon thread pool");

    let (sender, receiver) = unbounded::<(mrs_core::ProblemProfile, String, String, bool)>();

    let db_path = args.db.clone();
    let writer_handle = thread::spawn(move || {
        let mut conn =
            Connection::open(db_path).expect("Failed to open SQLite database in writer thread");
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA cache_size = -64000;",
        )
        .expect("Failed to set PRAGMAs");

        let batch_size: usize = 250;
        let mut count: usize = 0;
        let mut tx = conn.transaction().expect("Failed to start transaction");

        for (profile, p_corpus, p_canonical, p_is_comp) in receiver {
            if let Err(e) = save_problem_profile(&tx, &profile, &p_corpus, &p_canonical, p_is_comp)
            {
                eprintln!("Error saving profile for {}: {}", profile.problem_name, e);
            }
            count += 1;
            if count.is_multiple_of(batch_size) {
                tx.commit().expect("Failed to commit batch transaction");
                tx = conn
                    .transaction()
                    .expect("Failed to start next transaction");
            }
        }
        tx.commit().expect("Failed to commit final transaction");
    });

    let progress = Arc::new(AtomicUsize::new(0));
    let start_time = Instant::now();

    pool.install(|| {
        pending_files
            .par_iter()
            .for_each(|(problem_name, file_path)| {
                let profile_opt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    extract_problem_profile_from_file(file_path, problem_name, Some(&detected_tptp))
                }))
                .unwrap_or(None);

                if let Some(mut profile) = profile_opt {
                    profile.problem_name = problem_name.clone();
                    let filename = file_path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or(problem_name);
                    let canon_name = canonical_tptp_name(filename);
                    sender
                        .send((profile, corpus.clone(), canon_name, is_competition))
                        .expect("Failed to send profile to writer thread");
                } else {
                    eprintln!("Warning: Failed to extract profile for {}", problem_name);
                }

                let current = progress.fetch_add(1, Ordering::Relaxed) + 1;
                if current.is_multiple_of(250) || current == total_pending {
                    let elapsed = start_time.elapsed().as_secs_f64();
                    let rate = current as f64 / elapsed.max(0.001);
                    let remaining = total_pending.saturating_sub(current);
                    let eta_secs = remaining as f64 / rate.max(0.001);
                    println!(
                        "[{:>5}/{}] ({:5.1}%) Profiled at {:.1} probs/s | ETA: {:.0}s",
                        current,
                        total_pending,
                        (current as f64 / total_pending as f64) * 100.0,
                        rate,
                        eta_secs,
                    );
                }
            });
    });

    drop(sender);
    writer_handle
        .join()
        .expect("Profile writer thread panicked");
    println!(
        "Profiling complete in {:.2}s. Saved profiles to {}.",
        start_time.elapsed().as_secs_f64(),
        args.db.display()
    );
}

/// Imports CASC competition results from a run.csv file (or directory containing run.csv) into the database.
fn run_import_casc(args: &Args, import_path: &Path) {
    let (csv_path, log_path) = if import_path.is_dir() {
        (import_path.join("run.csv"), import_path.join("run.log"))
    } else {
        let parent = import_path.parent().unwrap_or(Path::new("."));
        (import_path.to_path_buf(), parent.join("run.log"))
    };

    if !csv_path.exists() {
        eprintln!(
            "Error: CASC results CSV '{}' not found.",
            csv_path.display()
        );
        std::process::exit(1);
    }

    println!("Importing CASC results from '{}'...", csv_path.display());
    let (log_edition, log_time_limits, log_tptp) = parse_companion_run_log(&log_path);

    let csv_content = std::fs::read_to_string(&csv_path).expect("Failed to read CSV file");
    let mut lines = csv_content.lines();
    let header_line = match lines.next() {
        Some(h) => h,
        None => {
            eprintln!("CSV file is empty.");
            return;
        }
    };

    let headers: Vec<String> = parse_csv_line(header_line)
        .into_iter()
        .map(|s| s.to_ascii_lowercase())
        .collect();

    let col_idx = |name: &str| -> Option<usize> { headers.iter().position(|h| h == name) };

    let idx_edition = col_idx("edition");
    let idx_division = col_idx("division").or_else(|| col_idx("div"));
    let idx_problem = col_idx("problem");
    let idx_system = col_idx("system");
    let idx_status = col_idx("szs_status").or_else(|| col_idx("status"));
    let idx_expected = col_idx("expected");
    let idx_verdict = col_idx("verdict");
    let idx_wall_time = col_idx("wall_time_s")
        .or_else(|| col_idx("wall_time"))
        .or_else(|| col_idx("time"));
    let idx_peak_mem = col_idx("peak_memory_mb").or_else(|| col_idx("memory_mb"));
    let idx_failure = col_idx("failure_detail").or_else(|| col_idx("failure"));

    if idx_problem.is_none() || idx_system.is_none() || idx_status.is_none() {
        eprintln!(
            "Error: CSV missing required columns (problem, system, szs_status). Found headers: {:?}",
            headers
        );
        std::process::exit(1);
    }

    let default_edition = log_edition
        .as_deref()
        .or(args.corpus.as_deref())
        .unwrap_or("casc");

    let competition_problems_dir = find_competition_problems_dir(
        default_edition,
        args.problems_dir.as_deref(),
        log_tptp.as_deref(),
    );

    if let Some(dir) = &competition_problems_dir {
        println!("Using competition problems directory: {}", dir.display());
    } else if !args.skip_profiles {
        println!(
            "Note: Competition problems directory for edition '{}' not found. Profile extraction will be skipped unless files are found.",
            default_edition
        );
    }

    let conn = Connection::open(&args.db).expect("Failed to open SQLite database");
    init_db(&conn).expect("Failed to initialize database");

    // Cache hardware ID
    let hardware_desc = args.hardware.clone().unwrap_or_else(detect_hardware);
    let hardware_id = get_or_create_id(&conn, "hardware", "description", &hardware_desc)
        .expect("Failed to get hardware ID");

    // Cache existing profiles
    let profiled_problems = fetch_profiled_problems(&conn).unwrap_or_default();
    drop(conn);

    struct ParsedCascRow {
        edition: String,
        division: String,
        problem: String,
        system: String,
        szs_status: String,
        expected: Option<String>,
        verdict: Option<String>,
        wall_time_s: Option<f64>,
        peak_memory_mb: Option<f64>,
        failure_detail: Option<String>,
    }

    let mut rows = Vec::new();
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let fields = parse_csv_line(line);
        let get = |opt_idx: Option<usize>| -> Option<String> {
            opt_idx
                .and_then(|i| fields.get(i).cloned())
                .filter(|s| !s.is_empty())
        };

        let edition = get(idx_edition).unwrap_or_else(|| default_edition.to_string());
        let division = get(idx_division)
            .unwrap_or_else(|| "Other".to_string())
            .to_ascii_uppercase();
        let problem = match get(idx_problem) {
            Some(p) => p,
            None => continue,
        };
        let system = match get(idx_system) {
            Some(s) => s,
            None => continue,
        };
        let szs_status = match get(idx_status) {
            Some(st) => st,
            None => continue,
        };
        let expected = get(idx_expected);
        let verdict = get(idx_verdict);
        let wall_time_s = get(idx_wall_time).and_then(|s| s.parse::<f64>().ok());
        let peak_memory_mb = get(idx_peak_mem).and_then(|s| s.parse::<f64>().ok());
        let failure_detail = get(idx_failure);

        rows.push(ParsedCascRow {
            edition,
            division,
            problem,
            system,
            szs_status,
            expected,
            verdict,
            wall_time_s,
            peak_memory_mb,
            failure_detail,
        });
    }

    println!("Parsed {} run records from CSV.", rows.len());

    // Connect and insert results
    let mut conn = Connection::open(&args.db).expect("Failed to open SQLite database for writing");
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA cache_size = -64000;",
    )
    .expect("Failed to set PRAGMAs");

    let mut system_id_map: HashMap<String, i64> = HashMap::new();
    let mut param_id_map: HashMap<String, i64> = HashMap::new();

    // Map unique problems to extract profiles for
    let mut problems_to_profile: HashMap<String, (String, String, PathBuf)> = HashMap::new();

    let tx = conn.transaction().expect("Failed to begin transaction");
    let mut inserted_results = 0;

    for row in &rows {
        let sys_id = match system_id_map.get(&row.system) {
            Some(&id) => id,
            None => {
                let id = get_or_create_id(&tx, "systems", "name", &row.system)
                    .expect("Failed to get system ID");
                system_id_map.insert(row.system.clone(), id);
                id
            }
        };

        let param_cmd = format!(
            "casc.sh --edition {} --division {}",
            row.edition, row.division
        );
        let param_id = match param_id_map.get(&param_cmd) {
            Some(&id) => id,
            None => {
                let id = get_or_create_id(&tx, "parameters", "command_template", &param_cmd)
                    .expect("Failed to get param ID");
                param_id_map.insert(param_cmd.clone(), id);
                id
            }
        };

        let div_lower = row.division.to_ascii_lowercase();
        let timeout = log_time_limits
            .get(&div_lower)
            .copied()
            .unwrap_or_else(|| default_casc_timeout(&row.division, &row.edition));

        let filename = if row.problem.ends_with(".p") {
            row.problem.clone()
        } else {
            format!("{}.p", row.problem)
        };

        // Strict isolation: Problem name in competition results is always prefixed by edition and division
        let problem_name = format!("{}/{}/{}", row.edition, row.division, filename);
        let canonical_name = canonical_tptp_name(&filename);
        let is_competition = 1i64;
        let corpus = row.edition.clone();

        tx.execute(
            "INSERT OR REPLACE INTO results 
             (problem_name, division, system_id, hardware_id, parameter_id, timeout, time_to_solve, status,
              corpus, canonical_name, is_competition, expected, verdict, peak_memory_mb, failure_detail)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                problem_name,
                row.division,
                sys_id,
                hardware_id,
                param_id,
                timeout as i64,
                row.wall_time_s,
                row.szs_status,
                corpus,
                canonical_name,
                is_competition,
                row.expected,
                row.verdict,
                row.peak_memory_mb,
                row.failure_detail,
            ],
        )
        .expect("Failed to insert run result");

        inserted_results += 1;

        if !args.skip_profiles
            && !profiled_problems.contains(&problem_name)
            && let Some(comp_dir) = &competition_problems_dir
        {
            let prob_path = comp_dir.join(&row.division).join(&filename);
            if prob_path.is_file() {
                problems_to_profile.entry(problem_name.clone()).or_insert((
                    corpus,
                    canonical_name,
                    prob_path,
                ));
            }
        }
    }

    tx.commit().expect("Failed to commit results transaction");
    println!(
        "Successfully inserted/updated {} results.",
        inserted_results
    );

    // Profile any unprofiled competition problems in parallel
    if !problems_to_profile.is_empty() {
        println!(
            "Extracting profiles for {} new competition problems...",
            problems_to_profile.len()
        );
        let pending: Vec<(String, String, String, PathBuf)> = problems_to_profile
            .into_iter()
            .map(|(pname, (corpus, cname, path))| (pname, corpus, cname, path))
            .collect();

        let num_threads = args.jobs.unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1)
        });
        let pool = ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .stack_size(64 * 1024 * 1024)
            .build()
            .expect("Failed to build rayon thread pool");

        let (sender, receiver) = unbounded::<(mrs_core::ProblemProfile, String, String, bool)>();
        let db_path = args.db.clone();
        let writer_handle = thread::spawn(move || {
            let mut conn =
                Connection::open(db_path).expect("Failed to open SQLite database for profiles");
            conn.execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = NORMAL;
                 PRAGMA cache_size = -64000;",
            )
            .expect("Failed to set PRAGMAs");

            let batch_size: usize = 250;
            let mut count: usize = 0;
            let mut tx = conn.transaction().expect("Failed to start transaction");

            for (profile, corpus, cname, is_comp) in receiver {
                if let Err(e) = save_problem_profile(&tx, &profile, &corpus, &cname, is_comp) {
                    eprintln!("Error saving profile for {}: {}", profile.problem_name, e);
                }
                count += 1;
                if count.is_multiple_of(batch_size) {
                    tx.commit().expect("Failed to commit batch transaction");
                    tx = conn
                        .transaction()
                        .expect("Failed to start next transaction");
                }
            }
            tx.commit()
                .expect("Failed to commit final profile transaction");
        });

        let tptp_root_for_profile = competition_problems_dir.clone();
        pool.install(|| {
            pending
                .par_iter()
                .for_each(|(pname, corpus, cname, file_path)| {
                    let profile_opt =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            extract_problem_profile_from_file(
                                file_path,
                                pname,
                                tptp_root_for_profile.as_deref(),
                            )
                        }))
                        .unwrap_or(None);

                    if let Some(mut profile) = profile_opt {
                        profile.problem_name = pname.to_string();
                        sender
                            .send((profile, corpus.to_string(), cname.to_string(), true))
                            .expect("Failed to send profile to writer");
                    } else {
                        eprintln!("Warning: Failed to extract profile for {}", pname);
                    }
                });
        });

        drop(sender);
        writer_handle
            .join()
            .expect("Profile writer thread panicked");
        println!("Profile extraction complete.");
    }
}

fn main() {
    let args = Args::parse();

    if args.timeout > i64::MAX as u64 {
        eprintln!("Error: --timeout must not exceed {} seconds.", i64::MAX);
        std::process::exit(1);
    }

    if let Some(import_path) = &args.import_casc {
        run_import_casc(&args, import_path);
        return;
    }

    let folder = match &args.folder {
        Some(f) => f,
        None => {
            eprintln!("Error: <folder> or --import-casc is required.");
            std::process::exit(1);
        }
    };

    if !folder.exists() {
        eprintln!("Error: Directory '{}' does not exist.", folder.display());
        std::process::exit(1);
    }

    if args.profile_only {
        run_profile_only(&args);
        return;
    }

    if args.system.is_empty() {
        eprintln!("Error: --system is required when running benchmarks.");
        std::process::exit(1);
    }

    if args.cmd.is_empty() {
        eprintln!("Error: --cmd is required when running benchmarks.");
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

    let allowed_problems =
        fetch_all_results_problems(&conn).expect("Failed to fetch allowed problems");

    // We don't need the connection anymore in the main thread
    drop(conn);

    println!(
        "Found {} already completed problems for this configuration.",
        completed_problems.len()
    );
    if !allowed_problems.is_empty() {
        println!(
            "Database contains {} total target problems; restricting scan only to them.",
            allowed_problems.len()
        );
    }
    println!("Proof verification mode: {:?}", args.verify_mode);
    println!("Scanning {} for .p files...", folder.display());

    let (corpus, is_competition, detected_tptp) =
        detect_corpus_and_competition(folder, args.corpus.as_deref());

    let mut pending_files = Vec::new();
    for entry in WalkDir::new(folder).into_iter().filter_map(|e| e.ok()) {
        if entry.path().is_file() && entry.path().extension().is_some_and(|ext| ext == "p") {
            let relative_path = entry.path().strip_prefix(folder).unwrap_or(entry.path());
            let problem_name = if is_competition {
                format!("{}/{}", corpus, relative_path.to_string_lossy())
            } else {
                relative_path.to_string_lossy().to_string()
            };

            if !allowed_problems.is_empty() && !allowed_problems.contains(&problem_name) {
                continue;
            }

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
        .stack_size(64 * 1024 * 1024) // 64 MiB stack size (prevents parsing stack overflow)
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
                let profile = extract_problem_profile_from_file(
                    file_path,
                    problem_name,
                    Some(&detected_tptp),
                );
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

                let filename = file_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(problem_name);
                let canonical_name = canonical_tptp_name(filename);

                let result = RunResult {
                    problem_name: problem_name.clone(),
                    division: division.clone(),
                    system_id,
                    parameter_id,
                    hardware_id,
                    timeout: args.timeout,
                    time_to_solve,
                    status: status_str.clone(),
                    corpus: corpus.clone(),
                    canonical_name,
                    is_competition,
                    expected: None,
                    verdict: None,
                    peak_memory_mb: None,
                    failure_detail: None,
                    proover_validated: proover_validated.clone(),
                    starexec_validated: starexec_validated.clone(),
                    time_to_verify,
                    kernel_validated,
                    kernel_time,
                    mrs_validated: mrs_validated.clone(),
                    mrs_verify_time,
                    competition_validated,
                    competition_time,
                    external_atp_validated,
                    external_atp_time,
                    profile,
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
                            format!(" | Kernel: {} ({:.2}s)", status, kernel_time.unwrap_or(0.0))
                        })
                        .unwrap_or_default(),
                    VerifyMode::Competition => format!(
                        " | ProoVer: {} ({:.2}s) | StarExec: {} ({:.2}s)",
                        mrs_validated.as_deref().unwrap_or("N/A"),
                        mrs_verify_time.unwrap_or(0.0),
                        starexec_validated.as_deref().unwrap_or("N/A"),
                        external_atp_time.unwrap_or(0.0)
                    ),
                    VerifyMode::None => String::new(),
                };

                println!(
                    "[{}/{}] Problem: {} | Status: {} | Time: {}{}",
                    current, total_pending, problem_name, status_str, time_disp, val_disp
                );
            });
    });

    // Close the sender channel so the writer thread terminates after processing all messages
    drop(sender);

    writer_handle.join().expect("Writer thread panicked");

    println!("Benchmark completed successfully.");
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

    #[test]
    fn test_profile_only_args_parse() {
        let args = Args::try_parse_from(["mrs-codex", "problems", "--profile-only"])
            .expect("profile-only arguments parse");
        assert!(args.profile_only);
        assert_eq!(args.system, "");
        assert_eq!(args.cmd, "");
    }

    #[test]
    fn test_problem_profiles_schema_and_save() {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();

        let profile = mrs_core::ProblemProfile {
            problem_name: "GRP001-1.p".to_string(),
            domain: "GRP".to_string(),
            dialect: "CNF".to_string(),
            header_status: Some("Unsatisfiable".to_string()),
            header_rating: Some(0.12),
            profile_complete: true,
            num_clauses: 4,
            num_literals: 4,
            num_axioms: 3,
            num_conjectures: 1,
            num_variables: 3,
            avg_vars_per_clause: 1.5,
            max_vars_per_clause: 3,
            unit_ratio: 1.0,
            horn_ratio: 1.0,
            definite_ratio: 0.75,
            goal_clause_ratio: 0.25,
            ground_ratio: 0.25,
            equality_literal_ratio: 1.0,
            non_linear_var_ratio: 0.5,
            max_clause_len: 1,
            avg_clause_len: 1.0,
            max_pos_literals: 1,
            max_term_depth: 3,
            avg_term_depth: 2.1,
            max_term_size: 5,
            avg_term_size: 3.2,
            num_predicates: 1,
            num_functions: 2,
            num_constants: 1,
            max_fun_arity: 2,
            max_pred_arity: 2,
            skolem_symbols_count: 0,
            is_ueq: true,
            is_peq: true,
            is_fne: false,
            is_feq: false,
            is_epr: false,
            is_fvo: false,
            is_large_theory: false,
            has_ac_symbols: true,
            ac_symbols: vec!["multiply".to_string()],
            has_identity_axiom: true,
            has_inverse_axiom: true,
            has_idempotence: false,
            has_conjecture: true,
            conjecture_clauses: 1,
            conjecture_literals: 1,
            conjecture_max_depth: 2,
            conjecture_symbol_overlap: 1.0,
            unique_conjecture_symbols: vec![],
            archetype: mrs_core::ProblemArchetype::PureUnitEquality,
            casc_division: "UEQ".to_string(),
            recommended_schedule: "casc_ueq".to_string(),
            recommended_engine: "Superposition".to_string(),
            recommended_avatar: false,
            recommended_sine: false,
        };

        save_problem_profile(&conn, &profile, "tptp", "GRP/GRP001-1.p", false).unwrap();

        // Query back from DB
        let mut stmt = conn
            .prepare("SELECT problem_name, archetype, casc_division, is_ueq, has_ac_symbols FROM problem_profiles WHERE problem_name = ?1")
            .unwrap();
        let row = stmt
            .query_row(params!["GRP001-1.p"], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })
            .unwrap();

        assert_eq!(row.0, "GRP001-1.p");
        assert_eq!(row.1, "PureUnitEquality");
        assert_eq!(row.2, "UEQ");
        assert_eq!(row.3, 1);
        assert_eq!(row.4, 1);
    }

    #[test]
    fn test_extract_problem_profile_from_file() {
        let content = "cnf(comm, axiom, multiply(X, Y) = multiply(Y, X)).\ncnf(goal, negated_conjecture, multiply(a, b) != multiply(b, a)).";
        let mut file = tempfile::Builder::new().suffix(".p").tempfile().unwrap();
        file.write_all(content.as_bytes()).unwrap();

        let profile = extract_problem_profile_from_file(file.path(), "test.p", None)
            .expect("profile extracted");
        assert_eq!(
            profile.archetype,
            mrs_core::ProblemArchetype::PureUnitEquality
        );
        assert_eq!(profile.casc_division, "UEQ");
        assert!(profile.is_ueq);
        assert_eq!(profile.num_clauses, 2);
    }

    #[test]
    fn profile_only_preserves_status_aware_division() {
        let content =
            "% Status: Satisfiable\nfof(a1, axiom, p(f(a))). fof(c1, conjecture, ~p(f(b))).";
        let mut file = tempfile::Builder::new().suffix(".p").tempfile().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        let ast = mrs_tptp::parse_tptp(content).unwrap();
        let status = extract_ground_truth_status(content);
        assert_eq!(
            determine_division_from_ast(&ast, status.as_deref(), content),
            "FNN"
        );

        let profile = extract_problem_profile_from_file(file.path(), "FOO001.p", None)
            .expect("profile extracted");
        assert!(profile.profile_complete);
        assert_eq!(profile.casc_division, "FNE");
    }

    #[test]
    fn oversized_profile_is_incomplete_without_reading_all_statistics() {
        let mut file = tempfile::Builder::new().suffix(".p").tempfile().unwrap();
        file.write_all(b"% Status: Theorem\n% Rating: 0.1\n")
            .unwrap();
        file.as_file_mut().set_len(20 * 1024 * 1024 + 1).unwrap();

        let profile = extract_problem_profile_from_file(file.path(), "FOO001.p", None)
            .expect("profile extracted");
        assert!(!profile.profile_complete);
        assert_eq!(profile.dialect, "Unknown");
        assert_eq!(profile.casc_division, "Unknown");
        assert_eq!(profile.header_status.as_deref(), Some("Theorem"));
    }

    #[test]
    fn test_canonical_tptp_name() {
        assert_eq!(canonical_tptp_name("AGT005+1.p"), "AGT/AGT005+1.p");
        assert_eq!(
            canonical_tptp_name("casc-30/FEQ/AGT005+1.p"),
            "AGT/AGT005+1.p"
        );
        assert_eq!(
            canonical_tptp_name("Problems/GRP/GRP001-1.p"),
            "GRP/GRP001-1.p"
        );
        assert_eq!(canonical_tptp_name("GRP123-4.004"), "GRP/GRP123-4.004.p");
        assert_eq!(canonical_tptp_name("123.p"), "123.p");
    }

    #[test]
    fn test_detect_corpus_and_competition() {
        let p1 = Path::new("/home/user/mrs/crates/mrs-bench/problems/casc-30");
        let (corpus1, is_comp1, _) = detect_corpus_and_competition(p1, None);
        assert_eq!(corpus1, "casc-30");
        assert!(is_comp1);

        let p2 = Path::new("/home/user/TPTP-v9.3.0/Problems");
        let (corpus2, is_comp2, _) = detect_corpus_and_competition(p2, None);
        assert_eq!(corpus2, "tptp");
        assert!(!is_comp2);

        let (corpus3, is_comp3, _) = detect_corpus_and_competition(p2, Some("custom_corpus"));
        assert_eq!(corpus3, "custom_corpus");
        assert!(!is_comp3);
    }

    #[test]
    fn test_no_conflation_between_competition_and_general_tptp() {
        let conn = Connection::open_in_memory().unwrap();
        init_db(&conn).unwrap();

        let mut tptp_profile = mrs_core::ProblemProfile::empty(
            "AGT/AGT005+1.p",
            "AGT".to_string(),
            "FOF".to_string(),
            Some("Theorem".to_string()),
            Some(0.1),
        );
        tptp_profile.num_clauses = 646;

        let mut casc_profile = mrs_core::ProblemProfile::empty(
            "casc-30/FEQ/AGT005+1.p",
            "AGT".to_string(),
            "FOF".to_string(),
            Some("Theorem".to_string()),
            Some(0.1),
        );
        casc_profile.num_clauses = 20;

        save_problem_profile(&conn, &tptp_profile, "tptp", "AGT/AGT005+1.p", false).unwrap();
        save_problem_profile(&conn, &casc_profile, "casc-30", "AGT/AGT005+1.p", true).unwrap();

        // 1. Verify both records exist without overwriting each other
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM problem_profiles", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);

        // 2. Query master TPTP profile
        let (tptp_name, tptp_clauses, tptp_is_comp, tptp_corpus): (String, i64, i64, String) = conn
            .query_row(
                "SELECT problem_name, num_clauses, is_competition, corpus FROM problem_profiles WHERE is_competition = 0",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(tptp_name, "AGT/AGT005+1.p");
        assert_eq!(tptp_clauses, 646);
        assert_eq!(tptp_is_comp, 0);
        assert_eq!(tptp_corpus, "tptp");

        // 3. Query competition profile
        let (casc_name, casc_clauses, casc_is_comp, casc_corpus): (String, i64, i64, String) = conn
            .query_row(
                "SELECT problem_name, num_clauses, is_competition, corpus FROM problem_profiles WHERE is_competition = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(casc_name, "casc-30/FEQ/AGT005+1.p");
        assert_eq!(casc_clauses, 20);
        assert_eq!(casc_is_comp, 1);
        assert_eq!(casc_corpus, "casc-30");

        // 4. Query using shared canonical TPTP key
        let canonical_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM problem_profiles WHERE canonical_name = 'AGT/AGT005+1.p'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(canonical_count, 2);
    }

    #[test]
    fn test_import_casc_csv_and_isolation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test_casc.db");

        // Create a mock run.csv and run.log
        let csv_path = temp_dir.path().join("run.csv");
        let log_path = temp_dir.path().join("run.log");

        let csv_data = "edition,division,problem,system,szs_status,expected,verdict,wall_time_s,failure_detail\n\
casc-30,FEQ,AGT005+1.p,vampire,Theorem,Theorem,CORRECT,1.23,\n\
casc-30,FEQ,AGT005+1.p,mrs,Theorem,Theorem,CORRECT,0.05,\n\
casc-30,UEQ,GRP001-1.p,mrs,Unsatisfiable,Unsatisfiable,CORRECT,0.12,\n";
        std::fs::write(&csv_path, csv_data).unwrap();

        let log_data = "[casc] Edition: casc-30\n\
[casc] Time limits: eps=120s,epu=120s,feq=240s,fne=240s,ueq=240s\n\
[casc] TPTP: /nonexistent/path\n";
        std::fs::write(&log_path, log_data).unwrap();

        let args = Args {
            folder: None,
            db: db_path.clone(),
            system: "dummy".to_string(),
            cmd: "dummy".to_string(),
            params: None,
            hardware: Some("Test Hardware".to_string()),
            timeout: 300,
            jobs: Some(1),
            verify_mode: VerifyMode::None,
            profile_only: false,
            overwrite_profiles: false,
            import_casc: Some(csv_path.clone()),
            problems_dir: None,
            corpus: None,
            skip_profiles: true,
        };

        run_import_casc(&args, &csv_path);

        let conn = Connection::open(&db_path).unwrap();

        // Verify results table
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM results", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 3);

        // Verify competition isolation in results
        let mut stmt = conn
            .prepare("SELECT problem_name, division, timeout, status, corpus, canonical_name, is_competition, time_to_solve FROM results ORDER BY system_id, problem_name")
            .unwrap();

        let rows: Vec<(String, String, i64, String, String, String, i64, f64)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                ))
            })
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        for r in &rows {
            assert_eq!(r.4, "casc-30"); // corpus
            assert_eq!(r.6, 1); // is_competition
            assert!(r.0.starts_with("casc-30/")); // problem_name namespaced
        }

        // Check canonical names and division timeouts
        let agt_rows: Vec<_> = rows.iter().filter(|r| r.5 == "AGT/AGT005+1.p").collect();
        assert_eq!(agt_rows.len(), 2);
        assert_eq!(agt_rows[0].2, 240); // FEQ timeout from log
    }
}
