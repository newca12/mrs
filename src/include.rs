//! Include directive resolution for TPTP problems.
//!
//! TPTP problems can reference other files via `include('filename')` directives.
//! This module resolves those directives by reading, parsing, and lowering the
//! included formulas into the main `LoweredProblem`.
//!
//! Path resolution: if the `$TPTP` environment variable is set, included files
//! are resolved relative to that path. Otherwise, they're resolved relative to
//! the main problem file's directory.

use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use mrs_tptp::TPTPProblem;

use crate::lowering::{self, LoweredProblem};

/// Error during include resolution.
#[derive(Debug)]
pub enum IncludeError {
    /// The included file could not be found.
    FileNotFound { path: PathBuf },
    /// An I/O error reading the included file.
    IoError {
        path: PathBuf,
        error: std::io::Error,
    },
    /// A parse error in the included file.
    ParseError { path: PathBuf, message: String },
    /// Circular include detected.
    CircularInclude { path: PathBuf },
}

impl fmt::Display for IncludeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IncludeError::FileNotFound { path } => {
                write!(f, "Include file not found: {}", path.display())
            }
            IncludeError::IoError { path, error } => {
                write!(f, "Error reading {}: {}", path.display(), error)
            }
            IncludeError::ParseError { path, message } => {
                write!(f, "Parse error in {}: {}", path.display(), message)
            }
            IncludeError::CircularInclude { path } => {
                write!(f, "Circular include detected: {}", path.display())
            }
        }
    }
}

/// Resolves all include directives in a parsed TPTP problem, lowering their
/// formulas into the existing `LoweredProblem`.
///
/// This function reads each included file, parses it, converts its formulas
/// to core types via `lower_into`, and recursively resolves any nested includes.
///
/// `base_dir` is the directory of the main problem file.
/// `tptp_root` is the `$TPTP` environment variable path, if set.
pub fn resolve_and_lower(
    problem: &TPTPProblem<'_>,
    lowered: &mut LoweredProblem,
    base_dir: &Path,
    tptp_root: Option<&Path>,
) -> Result<(), IncludeError> {
    let mut visited = HashSet::new();
    resolve_includes_recursive(problem, lowered, base_dir, tptp_root, &mut visited)
}

/// Recursive helper for include resolution.
fn resolve_includes_recursive(
    problem: &TPTPProblem<'_>,
    lowered: &mut LoweredProblem,
    base_dir: &Path,
    tptp_root: Option<&Path>,
    visited: &mut HashSet<PathBuf>,
) -> Result<(), IncludeError> {
    for include in &problem.includes {
        let path = resolve_path(include.file_name, base_dir, tptp_root);

        // Circular include detection
        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
        if !visited.insert(canonical) {
            return Err(IncludeError::CircularInclude { path });
        }

        // Read the file
        if !path.exists() {
            return Err(IncludeError::FileNotFound { path: path.clone() });
        }

        let content = fs::read_to_string(&path).map_err(|e| IncludeError::IoError {
            path: path.clone(),
            error: e,
        })?;

        // Parse
        let inc_problem = mrs_tptp::parse_tptp(&content).map_err(|e| IncludeError::ParseError {
            path: path.clone(),
            message: format!("{}", e),
        })?;

        // Build selection filter: convert Option<Vec<Name>> to Option<Vec<&str>>
        let sel_strs: Option<Vec<&str>> = include
            .selection
            .as_ref()
            .map(|sel| sel.iter().map(|n| n.as_str()).collect());
        let sel_refs: Option<&[&str]> = sel_strs.as_deref();

        // Lower included formulas into the existing LoweredProblem
        lowering::lower_into(lowered, &inc_problem, sel_refs);

        // Recursively resolve any includes in the included file
        let inc_dir = path.parent().unwrap_or(Path::new("."));
        resolve_includes_recursive(&inc_problem, lowered, inc_dir, tptp_root, visited)?;
    }

    Ok(())
}

/// Resolves a TPTP include path.
///
/// If `$TPTP` is set, tries resolving relative to that root first.
/// Falls back to resolving relative to `base_dir` (the problem file's directory).
fn resolve_path(file_name: &str, base_dir: &Path, tptp_root: Option<&Path>) -> PathBuf {
    if let Some(root) = tptp_root {
        let path = root.join(file_name);
        if path.exists() {
            return path;
        }
    }
    base_dir.join(file_name)
}
