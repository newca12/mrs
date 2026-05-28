//! Include directive resolution for TPTP problems.
//!
//! TPTP problems can reference other files via `include('filename')` directives.
//! This module resolves those directives by reading, parsing, and lowering the
//! included formulas into the main `LoweredProblem`.
//!
//! Path resolution: if the `$TPTP` environment variable is set, included files
//! are resolved relative to that path. Otherwise, they're resolved relative to
//! the main problem file's directory.

// This module is shared across multiple binary targets via `#[path]`; not all
// binaries use every item, so suppress dead-code warnings here.
#![allow(dead_code)]

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
    FileNotFound {
        /// The requested include path (as written in the problem file).
        requested: String,
        /// Every absolute path that was tried.
        tried: Vec<PathBuf>,
    },
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
            IncludeError::FileNotFound { requested, tried } => {
                write!(f, "Include file not found: '{}'", requested)?;
                if !tried.is_empty() {
                    write!(f, "\n  Tried:")?;
                    for p in tried {
                        write!(f, "\n    {}", p.display())?;
                    }
                    write!(
                        f,
                        "\n  Hint: set TPTP to the root directory that contains Axioms/ \
                         (e.g. TPTP=/path/to/TPTP-v9.2.1)"
                    )?;
                }
                Ok(())
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
        let (path, tried) = resolve_path(include.file_name, base_dir, tptp_root);

        // Circular include detection
        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
        if !visited.insert(canonical) {
            return Err(IncludeError::CircularInclude { path });
        }

        // Read the file
        if !path.exists() {
            return Err(IncludeError::FileNotFound {
                requested: include.file_name.to_owned(),
                tried,
            });
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

/// Resolves a TPTP include path, trying multiple candidate roots.
///
/// Resolution order:
/// 1. `$TPTP/file_name`  (if the env-var root is provided)
/// 2. Walk up from `base_dir` looking for the first ancestor that contains
///    an `Axioms/` subdirectory, then try `<ancestor>/file_name`.  This
///    auto-detects the TPTP root even when `$TPTP` is set to a wrong value
///    (e.g. `.../Problems/` instead of `.../TPTP-v9.2.1/`).
/// 3. `base_dir/file_name`  (last resort)
///
/// Returns the best candidate path (the first existing one, or the last
/// candidate if none exist) together with the list of every path tried.
fn resolve_path(
    file_name: &str,
    base_dir: &Path,
    tptp_root: Option<&Path>,
) -> (PathBuf, Vec<PathBuf>) {
    let mut tried: Vec<PathBuf> = Vec::new();

    // 1. $TPTP/file_name
    if let Some(root) = tptp_root {
        let p = root.join(file_name);
        tried.push(p.clone());
        if p.exists() {
            return (p, tried);
        }
    }

    // 2. Walk up from base_dir, find the first ancestor with Axioms/
    let mut dir = base_dir.to_path_buf();
    loop {
        if dir.join("Axioms").is_dir() {
            let p = dir.join(file_name);
            if !tried.contains(&p) {
                tried.push(p.clone());
            }
            if p.exists() {
                return (p, tried);
            }
            break;
        }
        if !dir.pop() {
            break;
        }
    }

    // 3. base_dir/file_name
    let fallback = base_dir.join(file_name);
    if !tried.contains(&fallback) {
        tried.push(fallback.clone());
    }
    (fallback, tried)
}
