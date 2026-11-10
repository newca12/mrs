//! Compare decidable closed arithmetic files with their TPTP status headers.
//! Unsupported files (including includes and quantified arithmetic) are counted,
//! not claimed as solved. This is semantic regression testing, not a benchmark.

use mrs_arithmetic::Evaluator;
use mrs_tptp::{FormulaRole, TPTPInput, TPTPIterator};
use std::path::{Path, PathBuf};

fn files(path: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if path.is_file() {
        out.push(path.to_path_buf());
    } else {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                files(&entry.path(), out)?;
            } else if entry
                .path()
                .extension()
                .is_some_and(|ext| ext == "p" || ext == "ax")
            {
                out.push(entry.path());
            }
        }
    }
    Ok(())
}

fn decide(source: &str) -> Option<&'static str> {
    let mut premises = true;
    let mut goals = None;
    let mut logical_formulas = 0;
    for item in TPTPIterator::new(source) {
        let TPTPInput::Formula(formula) = item.ok()? else {
            return None;
        };
        let role = formula.role();
        if role.is_type_declaration() {
            continue;
        }
        let value = Evaluator::default().evaluate(&formula).ok()?;
        logical_formulas += 1;
        if role == FormulaRole::Conjecture {
            goals = Some(goals.unwrap_or(true) && value);
        } else if role.is_premise()
            || matches!(role, FormulaRole::Plain | FormulaRole::NegatedConjecture)
        {
            premises &= value;
        } else {
            return None;
        }
    }
    if logical_formulas == 0 {
        return None;
    }
    Some(match (premises, goals) {
        (false, Some(_)) | (true, Some(true)) => "Theorem",
        (false, None) => "Unsatisfiable",
        (true, None) => "Satisfiable",
        (true, Some(false)) => "CounterSatisfiable",
    })
}

fn check(root: PathBuf) -> Result<(), String> {
    let mut paths = Vec::new();
    files(&root, &mut paths).map_err(|error| error.to_string())?;
    paths.sort();
    let mut evaluated = 0;
    let mut mismatches = 0;
    for path in &paths {
        let source = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
        let expected = source.lines().find_map(|line| {
            line.strip_prefix("% Status")?
                .split_once(':')
                .map(|(_, status)| status.trim())
        });
        let Some(expected) = expected else { continue };
        let Some(actual) = decide(&source) else {
            continue;
        };
        evaluated += 1;
        if actual != expected {
            mismatches += 1;
            eprintln!(
                "{}: expected {expected}, evaluated {actual}",
                path.display()
            );
        }
    }
    println!(
        "Files: {}, evaluated: {evaluated}, unsupported/unclassified: {}, mismatches: {mismatches}",
        paths.len(),
        paths.len() - evaluated
    );
    if mismatches == 0 {
        Ok(())
    } else {
        Err("arithmetic status disagreements".into())
    }
}

fn main() {
    let Some(root) = std::env::args_os().nth(1) else {
        eprintln!("usage: check_corpus TPTP/Problems/ARI");
        std::process::exit(2);
    };
    // The parser still recursively descends files that evaluation will refuse.
    let result = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || check(PathBuf::from(root)))
        .expect("parser thread")
        .join()
        .expect("parser panic");
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
