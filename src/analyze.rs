use std::fs;
use std::path::Path;

use mrs_core::{Clause, InputMetadata, ProblemProfile, SymbolTable};
use mrs_tptp::{AnnotatedFormula, TPTPProblem};

/// Analyzes a lowered problem and returns its deep `ProblemProfile`.
pub fn analyze_problem(
    path: &str,
    problem: &TPTPProblem<'_>,
    symbols: &SymbolTable,
    all_clauses: &[Clause],
) -> ProblemProfile {
    let name = if path == "-" {
        "stdin"
    } else {
        Path::new(path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
    };

    let file_size_bytes = if path != "-" {
        fs::metadata(path).ok().map(|m| m.len())
    } else {
        None
    };

    // Analyze dialects present in AST
    let mut dialect_counts = std::collections::HashMap::new();
    for f in &problem.formulas {
        let dialect = match f {
            AnnotatedFormula::THF(_) => "THF",
            AnnotatedFormula::TFF(_) => "TFF",
            AnnotatedFormula::FOF(_) => "FOF",
            AnnotatedFormula::TCF(_) => "TCF",
            AnnotatedFormula::CNF(_) => "CNF",
            AnnotatedFormula::TPI(_) => "TPI",
        };
        *dialect_counts.entry(dialect).or_insert(0usize) += 1;
    }

    let predominant_dialect = dialect_counts
        .into_iter()
        .max_by_key(|(_, cnt)| *cnt)
        .map(|(d, _)| d.to_string())
        .unwrap_or_else(|| "CNF".to_string());

    let (header_status, header_rating) = if path != "-" {
        parse_tptp_headers(path)
    } else {
        (None, None)
    };

    let meta = InputMetadata {
        dialect: Some(predominant_dialect),
        raw_formulas_count: problem.formulas.len(),
        includes_count: problem.includes.len(),
        file_size_bytes,
        header_status,
        header_rating,
    };

    ProblemProfile::extract(name, Some(&meta), all_clauses, symbols)
}

/// Print human-readable deep problem profile report.
pub fn analyze_and_print(
    path: &str,
    problem: &TPTPProblem<'_>,
    symbols: &SymbolTable,
    all_clauses: &[Clause],
) {
    let profile = analyze_problem(path, problem, symbols, all_clauses);
    println!("{}", profile);
}

/// Print machine-readable JSON problem profile.
pub fn analyze_and_print_json(
    path: &str,
    problem: &TPTPProblem<'_>,
    symbols: &SymbolTable,
    all_clauses: &[Clause],
) {
    let profile = analyze_problem(path, problem, symbols, all_clauses);
    match serde_json::to_string_pretty(&profile) {
        Ok(json) => println!("{}", json),
        Err(e) => eprintln!("% Error serializing problem profile to JSON: {}", e),
    }
}

/// Scan header comments in a TPTP problem for Status and Rating fields.
fn parse_tptp_headers(path: &str) -> (Option<String>, Option<f32>) {
    let Ok(content) = fs::read_to_string(path) else {
        return (None, None);
    };

    let mut status = None;
    let mut rating = None;

    for line in content.lines().take(100) {
        let trimmed = line.trim();
        if !trimmed.starts_with('%') {
            if !trimmed.is_empty() {
                // Past the header comments
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
