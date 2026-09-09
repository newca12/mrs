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
    /// Directory containing TPTP files
    folder: PathBuf,

    /// Path to the SQLite database file
    #[arg(long, default_value = "codex.db")]
    db: PathBuf,

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
            raw_profile_json TEXT
        )",
        [],
    )?;

    // Schema migration for profiles created before completeness was tracked.
    let _ = conn.execute(
        "ALTER TABLE problem_profiles ADD COLUMN profile_complete INTEGER NOT NULL DEFAULT 1",
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

    Ok(())
}

fn save_problem_profile(conn: &Connection, profile: &mrs_core::ProblemProfile) -> SqliteResult<()> {
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
            unique_conjecture_symbols, profile_complete, raw_profile_json
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
            ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28,
            ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37, ?38, ?39, ?40, ?41,
            ?42, ?43, ?44, ?45, ?46, ?47, ?48, ?49, ?50, ?51, ?52, ?53, ?54,
            ?55, ?56, ?57, ?58
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
    if let Some((dir, _)) = name.split_once('/')
        && dir.len() >= 3
        && dir.chars().take(3).all(|c| c.is_ascii_alphabetic())
    {
        return dir[0..3].to_uppercase();
    }
    let base = Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(name);
    if base.len() >= 3 && base.chars().take(3).all(|c| c.is_ascii_alphabetic()) {
        base[0..3].to_uppercase()
    } else {
        "UNK".to_string()
    }
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
        let env_tptp = std::env::var("TPTP").ok().map(PathBuf::from);
        let effective_tptp = tptp_root.or(env_tptp.as_deref());
        let _ = mrs::include::resolve_and_lower(&problem, &mut lowered, base_dir, effective_tptp);
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
            && let Err(e) = save_problem_profile(&conn, profile)
        {
            eprintln!("Error saving profile for {}: {}", result.problem_name, e);
        }

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

fn run_profile_only(args: &Args) {
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

    // Auto-detect TPTP root if not set
    let detected_tptp = std::env::var("TPTP").ok().map(PathBuf::from).or_else(|| {
        if args.folder.join("TPTP-v9.3.0").is_dir() {
            Some(args.folder.join("TPTP-v9.3.0"))
        } else if args.folder.join("../TPTP-v9.3.0").is_dir() {
            Some(args.folder.join("../TPTP-v9.3.0"))
        } else {
            None
        }
    });

    // Determine the base folder to scan
    let base_folder = if args.folder.join("TPTP-v9.3.0/Problems").is_dir() {
        args.folder.join("TPTP-v9.3.0/Problems")
    } else if args.folder.join("Problems").is_dir() {
        args.folder.join("Problems")
    } else {
        args.folder.clone()
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
            let problem_name = relative_path.to_string_lossy().to_string();

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

    let (sender, receiver) = unbounded::<mrs_core::ProblemProfile>();

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

        for profile in receiver {
            if let Err(e) = save_problem_profile(&tx, &profile) {
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
                    extract_problem_profile_from_file(
                        file_path,
                        problem_name,
                        detected_tptp.as_deref(),
                    )
                }))
                .unwrap_or(None);

                if let Some(profile) = profile_opt {
                    sender
                        .send(profile)
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
                let profile = extract_problem_profile_from_file(file_path, problem_name, None);
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

        save_problem_profile(&conn, &profile).unwrap();

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
}
