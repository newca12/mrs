//! bench_report — Summarise a CASC benchmark run.
//!
//! Usage:
//!     bench_report <run.csv> [--min-systems <N>]
//!
//! Output:
//!     Per-division solved/avg-time table, cross-system disagreements, and
//!     polarity violations (wrong SZS polarity for a known division type).
//!
//! CSV schema (produced by crates/mrs-bench/casc.sh):
//!     edition,division,problem,system,szs_status,expected,verdict,wall_time_s
//!
//! (Columns are looked up by name, so additions are non-breaking.)
//!

use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const SOLVED_STATUSES: &[&str] = &[
    "Theorem",
    "Unsatisfiable",
    "CounterSatisfiable",
    "Satisfiable",
];

fn is_solved(s: &str) -> bool {
    SOLVED_STATUSES.contains(&s)
}

/// Returns `true` if SZS statuses `a` and `b` are logically contradictory.
fn are_contradictory(a: &str, b: &str) -> bool {
    matches!(
        (a, b),
        ("Theorem", "CounterSatisfiable")
            | ("Theorem", "Satisfiable")
            | ("CounterSatisfiable", "Theorem")
            | ("Satisfiable", "Theorem")
            | ("Unsatisfiable", "CounterSatisfiable")
            | ("Unsatisfiable", "Satisfiable")
            | ("CounterSatisfiable", "Unsatisfiable")
            | ("Satisfiable", "Unsatisfiable")
    )
}

/// Returns the set of acceptable solved statuses for divisions with a known
/// a-priori polarity, or `None` for open divisions.
fn division_polarity(div: &str) -> Option<&'static [&'static str]> {
    match div {
        "epu" => Some(&["Unsatisfiable"]),
        "ueq" => Some(&["Unsatisfiable"]),
        "eps" => Some(&["Satisfiable", "CounterSatisfiable"]),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

struct Row {
    edition: String,
    division: String,
    problem: String,
    system: String,
    szs_status: String,
    wall_time_s: f64,
}

// ---------------------------------------------------------------------------
// CSV loading
// ---------------------------------------------------------------------------

fn load_csv(path: &PathBuf) -> Result<Vec<Row>, String> {
    let file =
        std::fs::File::open(path).map_err(|e| format!("cannot open {}: {}", path.display(), e))?;
    let reader = io::BufReader::new(file);
    let mut lines = reader.lines();

    // Parse header line (simple split — no quoted fields in this schema).
    let header_line = lines
        .next()
        .ok_or_else(|| "file is empty".to_string())?
        .map_err(|e| format!("I/O error: {e}"))?;

    let headers: Vec<String> = header_line
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();

    let col = |name: &str| -> Result<usize, String> {
        headers
            .iter()
            .position(|h| h == name)
            .ok_or_else(|| format!("missing column: {name}"))
    };

    let col_edition = col("edition")?;
    let col_division = col("division")?;
    let col_problem = col("problem")?;
    let col_system = col("system")?;
    let col_szs = col("szs_status")?;
    let col_time = col("wall_time_s")?;

    let mut rows = Vec::new();
    for (line_no, line) in lines.enumerate() {
        let line = line.map_err(|e| format!("I/O error at line {}: {e}", line_no + 2))?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split(',').collect();
        let get = |idx: usize| fields.get(idx).copied().unwrap_or("").trim().to_string();

        let wall_time_s = fields
            .get(col_time)
            .and_then(|s| s.trim().parse::<f64>().ok())
            .unwrap_or(0.0);

        rows.push(Row {
            edition: get(col_edition),
            division: get(col_division),
            problem: get(col_problem),
            system: get(col_system),
            szs_status: get(col_szs),
            wall_time_s,
        });
    }

    Ok(rows)
}

// ---------------------------------------------------------------------------
// Date/time formatting (UTC, zero-dependency)
// ---------------------------------------------------------------------------

fn format_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;
    let hour = time_of_day / 3600;
    let minute = (time_of_day % 3600) / 60;

    let (year, month, day) = days_to_ymd(days_since_epoch);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}")
}

fn is_leap(year: u64) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970u64;
    loop {
        let in_year = if is_leap(year) { 366 } else { 365 };
        if days < in_year {
            break;
        }
        days -= in_year;
        year += 1;
    }
    let month_days: [u64; 12] = if is_leap(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1u64;
    for dm in &month_days {
        if days < *dm {
            break;
        }
        days -= dm;
        month += 1;
    }
    (year, month, days + 1)
}

// ---------------------------------------------------------------------------
// Argument parsing
// ---------------------------------------------------------------------------

struct Args {
    csv: PathBuf,
    /// Minimum number of systems that must have solved a problem before
    /// disagreements are reported.  Clamped to at least 2 (you need two
    /// systems to have a contradiction).
    min_systems: usize,
}

fn parse_args() -> Args {
    let argv: Vec<String> = std::env::args().collect();
    let prog = argv.first().map(String::as_str).unwrap_or("bench_report");

    let usage = || {
        eprintln!("Usage: {prog} <run.csv> [--min-systems N]");
        eprintln!("       {prog} --help");
    };

    let mut csv: Option<PathBuf> = None;
    let mut min_systems = 2usize;
    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--help" | "-h" => {
                println!("Usage: {prog} <run.csv> [--min-systems N]");
                println!();
                println!("Summarise a CASC benchmark run CSV.");
                println!();
                println!("Positional:");
                println!("  <run.csv>          Path to run.csv produced by bench/casc.sh");
                println!();
                println!("Options:");
                println!("  --min-systems N    Only report disagreements when at least N systems");
                println!("                     solved the problem (default: 2).");
                println!("  -h, --help         Show this help message.");
                std::process::exit(0);
            }
            "--min-systems" => {
                i += 1;
                if i >= argv.len() {
                    eprintln!("error: --min-systems requires a value");
                    usage();
                    std::process::exit(1);
                }
                min_systems = argv[i].parse().unwrap_or_else(|_| {
                    eprintln!("error: --min-systems must be a positive integer");
                    std::process::exit(1);
                });
                if min_systems < 2 {
                    min_systems = 2;
                }
            }
            arg if !arg.starts_with('-') => {
                if csv.is_some() {
                    eprintln!("error: unexpected positional argument: {arg}");
                    usage();
                    std::process::exit(1);
                }
                csv = Some(PathBuf::from(arg));
            }
            arg => {
                eprintln!("error: unknown argument: {arg}");
                usage();
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let csv = csv.unwrap_or_else(|| {
        usage();
        std::process::exit(1);
    });

    Args { csv, min_systems }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let args = parse_args();

    if !args.csv.exists() {
        eprintln!("Error: file not found: {}", args.csv.display());
        std::process::exit(1);
    }

    let rows = load_csv(&args.csv).unwrap_or_else(|e| {
        eprintln!("Error: {e}");
        std::process::exit(1);
    });

    if rows.is_empty() {
        eprintln!("CSV is empty.");
        std::process::exit(1);
    }

    let edition = rows[0].edition.clone();

    // ---- Build index --------------------------------------------------------
    // data[(div, prob, sys)] = (szs_status, wall_time_s)
    let mut data: HashMap<(String, String, String), (String, f64)> = HashMap::new();
    let mut systems_seen: HashSet<String> = HashSet::new();
    let mut divisions_seen: Vec<String> = Vec::new(); // insertion-order
    let mut div_order: HashMap<String, usize> = HashMap::new();
    let mut div_probs_set: HashMap<String, HashSet<String>> = HashMap::new();
    let mut div_problems: HashMap<String, Vec<String>> = HashMap::new(); // sorted below

    for row in &rows {
        let div = row.division.to_lowercase();
        let prob = row.problem.clone();
        let sys = row.system.clone();

        data.insert(
            (div.clone(), prob.clone(), sys.clone()),
            (row.szs_status.clone(), row.wall_time_s),
        );
        systems_seen.insert(sys);

        if !div_order.contains_key(&div) {
            div_order.insert(div.clone(), div_order.len());
            divisions_seen.push(div.clone());
        }

        let set = div_probs_set.entry(div.clone()).or_default();
        if set.insert(prob.clone()) {
            div_problems.entry(div).or_default().push(prob);
        }
    }

    let mut systems: Vec<String> = systems_seen.into_iter().collect();
    systems.sort();

    for probs in div_problems.values_mut() {
        probs.sort();
    }

    // ---- Per-division stats -------------------------------------------------
    // stats[(div, sys)] = (solved_count, total_time_of_solved)
    let mut stats: HashMap<(String, String), (u32, f64)> = HashMap::new();

    for div in &divisions_seen {
        let probs = div_problems.get(div).map(Vec::as_slice).unwrap_or(&[]);
        for prob in probs {
            for sys in &systems {
                let key = (div.clone(), prob.clone(), sys.clone());
                if let Some((szs, t)) = data.get(&key)
                    && is_solved(szs)
                {
                    let e = stats.entry((div.clone(), sys.clone())).or_insert((0, 0.0));
                    e.0 += 1;
                    e.1 += t;
                }
            }
        }
    }

    // ---- Disagreements ------------------------------------------------------
    type SolvedBy = Vec<(String, String)>;
    let mut disagreements: Vec<(String, String, SolvedBy)> = Vec::new();

    for div in &divisions_seen {
        let probs = div_problems.get(div).map(Vec::as_slice).unwrap_or(&[]);
        for prob in probs {
            let mut solved_by: Vec<(String, String)> = Vec::new();
            for sys in &systems {
                let key = (div.clone(), prob.clone(), sys.clone());
                if let Some((szs, _)) = data.get(&key)
                    && is_solved(szs)
                {
                    solved_by.push((sys.clone(), szs.clone()));
                }
            }
            if solved_by.len() < args.min_systems {
                continue;
            }
            // Check for any pairwise logical contradiction.
            let mut has_contradiction = false;
            'outer: for i in 0..solved_by.len() {
                for j in (i + 1)..solved_by.len() {
                    if are_contradictory(&solved_by[i].1, &solved_by[j].1) {
                        has_contradiction = true;
                        break 'outer;
                    }
                }
            }
            if has_contradiction {
                disagreements.push((div.clone(), prob.clone(), solved_by));
            }
        }
    }

    // ---- Polarity violations ------------------------------------------------
    let mut polarity_violations: Vec<(String, String, String, String, String)> = Vec::new();

    for div in &divisions_seen {
        if let Some(expected) = division_polarity(div) {
            let probs = div_problems.get(div).map(Vec::as_slice).unwrap_or(&[]);
            for prob in probs {
                for sys in &systems {
                    let key = (div.clone(), prob.clone(), sys.clone());
                    if let Some((szs, _)) = data.get(&key)
                        && is_solved(szs)
                        && !expected.contains(&szs.as_str())
                    {
                        let mut exp_sorted: Vec<&str> = expected.to_vec();
                        exp_sorted.sort_unstable();
                        let note = format!("expected one of {:?}", exp_sorted);
                        polarity_violations.push((
                            div.clone(),
                            prob.clone(),
                            sys.clone(),
                            szs.clone(),
                            note,
                        ));
                    }
                }
            }
        }
    }

    // ---- Render -------------------------------------------------------------
    let now = format_now();
    let total_problems: usize = div_problems.values().map(|v| v.len()).sum();
    let n_sys = systems.len();

    let header = format!(
        "{} Results \u{2014} {}  ({} problems \u{d7} {} systems)",
        edition.to_uppercase(),
        now,
        total_problems,
        n_sys
    );
    println!("{header}");
    println!("{}", "=".repeat(header.chars().count()));
    println!();

    // Column widths
    let div_w = divisions_seen
        .iter()
        .map(|d| d.len())
        .max()
        .unwrap_or(0)
        .max("Division".len());
    let prob_w = div_problems
        .values()
        .map(|v| v.len().to_string().len())
        .max()
        .unwrap_or(0)
        .max("Problems".len());
    let sys_col_w: usize = 18;

    // Header row
    let div_header = format!("{:<div_w$}  {:>prob_w$}", "Division", "Problems");
    let sys_headers: String = systems
        .iter()
        .map(|s| format!("  {s:<sys_col_w$}"))
        .collect::<Vec<_>>()
        .join("  ");
    println!("{div_header}  {sys_headers}");

    // Sub-header row (aligns under system name columns)
    let mut sub_header = " ".repeat(div_w + 2 + prob_w);
    for _ in &systems {
        sub_header.push_str(&format!("    {:>6}  {:>7}  ", "Solved", "Avg (s)"));
    }
    println!("{sub_header}");

    let sep = format!(
        "{}  {}",
        "-".repeat(div_w + 2 + prob_w),
        systems
            .iter()
            .map(|_| "-".repeat(sys_col_w + 2))
            .collect::<Vec<_>>()
            .join("  ")
    );
    println!("{sep}");

    let mut total_solved: HashMap<String, u32> = HashMap::new();
    let mut total_time: HashMap<String, f64> = HashMap::new();

    for div in &divisions_seen {
        let n_probs = div_problems.get(div).map(|v| v.len()).unwrap_or(0);
        let mut row_str = format!("{:<div_w$}  {:>prob_w$}", div.to_uppercase(), n_probs);
        for sys in &systems {
            let (cnt, t) = stats
                .get(&(div.clone(), sys.clone()))
                .copied()
                .unwrap_or((0, 0.0));
            let avg = if cnt > 0 { t / f64::from(cnt) } else { 0.0 };
            row_str.push_str(&format!("    {:>6}  {:>7.3}  ", cnt, avg));
            *total_solved.entry(sys.clone()).or_insert(0) += cnt;
            *total_time.entry(sys.clone()).or_insert(0.0) += t;
        }
        println!("{row_str}");
    }

    println!("{sep}");

    let total_probs_all: usize = div_problems.values().map(|v| v.len()).sum();
    let mut total_row = format!("{:<div_w$}  {:>prob_w$}", "TOTAL", total_probs_all);
    for sys in &systems {
        let cnt = *total_solved.get(sys).unwrap_or(&0);
        let t = *total_time.get(sys).unwrap_or(&0.0);
        let avg = if cnt > 0 { t / f64::from(cnt) } else { 0.0 };
        total_row.push_str(&format!("    {:>6}  {:>7.3}  ", cnt, avg));
    }
    println!("{total_row}");
    println!();

    // ---- Disagreements section ----------------------------------------------
    if disagreements.is_empty() {
        println!("DISAGREEMENTS \u{2014} none detected.");
    } else {
        println!(
            "DISAGREEMENTS \u{2014} {} problem(s) where systems gave contradictory answers:",
            disagreements.len()
        );
        for (div, prob, solved_by) in &disagreements {
            let parts: String = solved_by
                .iter()
                .map(|(s, szs)| format!("{s}={szs}"))
                .collect::<Vec<_>>()
                .join("  ");
            println!(
                "  {:<6}  {:<30}  {}  \u{26a0} SOUNDNESS",
                div.to_uppercase(),
                prob,
                parts
            );
        }
    }
    println!();

    // ---- Polarity violations section ----------------------------------------
    if polarity_violations.is_empty() {
        println!("POLARITY VIOLATIONS \u{2014} none detected.");
    } else {
        println!(
            "POLARITY VIOLATIONS \u{2014} {} case(s) of wrong SZS polarity:",
            polarity_violations.len()
        );
        for (div, prob, sys, szs, note) in &polarity_violations {
            println!(
                "  {:<6}  {:<30}  {sys}={szs}  ({note})  \u{26a0} UNSOUND",
                div.to_uppercase(),
                prob
            );
        }
    }
}
