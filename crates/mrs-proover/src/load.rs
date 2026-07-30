//! Load and parse a proof file together with its linked problem file.

use std::fs;
use std::path::{Path, PathBuf};

use mrs_tptp::owned::{OwnedTPTPProblem, parse_tptp_file};
use mrs_tptp::proover::proof_header_link;

/// All inputs for one verification job.
pub struct LoadedJob {
    /// Path of the proof file (as provided).
    pub proof_path: PathBuf,
    /// Raw text of the proof file (kept for header parsing).
    pub proof_text: String,
    /// Parsed proof.
    pub proof: OwnedTPTPProblem,
    /// Path of the linked problem file, if any was found.
    pub problem_path: Option<PathBuf>,
    /// Parsed problem (if found and parsable).
    pub problem: Option<OwnedTPTPProblem>,
    /// Included problem files (kept alive for lifetime purposes).
    pub problem_includes: Vec<OwnedTPTPProblem>,
}

/// Errors that prevent any verification from starting.
#[derive(Debug)]
pub enum LoadError {
    /// Could not read the proof file.
    ReadProof(String),
    /// Could not parse the proof file.
    ParseProof(String),
    /// The proof file did not contain a `% Proof : …` header.
    MissingProofHeader,
    /// Could not read the linked problem file.
    ReadProblem(String, PathBuf),
    /// Could not parse the linked problem file.
    ParseProblem(String, PathBuf),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::ReadProof(e) => write!(f, "cannot read proof file: {e}"),
            LoadError::ParseProof(e) => write!(f, "cannot parse proof file: {e}"),
            LoadError::MissingProofHeader => write!(f, "no `% Proof :` header in proof file"),
            LoadError::ReadProblem(e, p) => {
                write!(f, "cannot read problem file {}: {e}", p.display())
            }
            LoadError::ParseProblem(e, p) => {
                write!(f, "cannot parse problem file {}: {e}", p.display())
            }
        }
    }
}

/// Load the proof and, if it references a problem file, parse that too.
///
/// `problems_root` is consulted if the header path is relative and the file is
/// not found next to the proof. This mirrors the typical competition layout:
/// proofs live in one folder and `Problems/foo.p` siblings live in another.
/// Load the proof and, if it references a problem file, parse that too.
///
/// `problems_root` is consulted if the header path is relative and the file is
/// not found next to the proof. This mirrors the typical competition layout:
/// proofs live in one folder and `Problems/foo.p` siblings live in another.
pub fn load(proof_path: &Path, problems_root: Option<&Path>) -> Result<LoadedJob, LoadError> {
    let proof_text =
        fs::read_to_string(proof_path).map_err(|e| LoadError::ReadProof(e.to_string()))?;

    let proof = parse_tptp_file(proof_path).map_err(|e| LoadError::ParseProof(e.to_string()))?;

    let header = proof_header_link(&proof_text);

    let mut problem_includes = Vec::new();
    let (problem_path, problem) = if let Some(rel) = header {
        let candidate =
            resolve_problem_path_with_base(proof_path, problems_root, Path::new("."), rel);
        match candidate {
            Some(path) => {
                let parsed = parse_tptp_file(&path)
                    .map_err(|e| LoadError::ParseProblem(e.to_string(), path.clone()))?;

                let base_dir = path.parent().unwrap_or(Path::new("."));
                resolve_includes_recursive(
                    proof_path,
                    problems_root,
                    &parsed.problem().includes,
                    base_dir,
                    &mut problem_includes,
                )?;

                (Some(path), Some(parsed))
            }
            None => (None, None),
        }
    } else {
        (None, None)
    };

    let mut job = LoadedJob {
        proof_path: proof_path.to_path_buf(),
        proof_text,
        proof,
        problem_path,
        problem,
        problem_includes,
    };

    // Merge included formulas into the main problem
    if let Some(ref mut prob) = job.problem {
        for inc in &job.problem_includes {
            let inc_tptp: &mrs_tptp::ast::TPTPProblem<'static> = inc;
            prob.append_formulas(&inc_tptp.formulas);
        }
    }

    Ok(job)
}

fn resolve_includes_recursive(
    proof_path: &Path,
    problems_root: Option<&Path>,
    includes: &[mrs_tptp::ast::Include<'_>],
    base_dir: &Path,
    problem_includes: &mut Vec<OwnedTPTPProblem>,
) -> Result<(), LoadError> {
    for inc in includes {
        let rel = inc.file_name;
        let candidate = resolve_problem_path_with_base(proof_path, problems_root, base_dir, rel);
        if let Some(path) = candidate {
            let parsed = parse_tptp_file(&path)
                .map_err(|e| LoadError::ParseProblem(e.to_string(), path.clone()))?;

            let inc_dir = path.parent().unwrap_or(Path::new("."));
            resolve_includes_recursive(
                proof_path,
                problems_root,
                &parsed.problem().includes,
                inc_dir,
                problem_includes,
            )?;

            problem_includes.push(parsed);
        } else {
            return Err(LoadError::ReadProblem(
                "included file not found".into(),
                PathBuf::from(rel),
            ));
        }
    }
    Ok(())
}

fn resolve_problem_path_with_base(
    proof_path: &Path,
    problems_root: Option<&Path>,
    base_dir: &Path,
    rel: &str,
) -> Option<PathBuf> {
    let mut v = vec![PathBuf::from(rel)];
    v.push(base_dir.join(rel));
    if let Some(parent) = proof_path.parent() {
        v.push(parent.join(rel));
        v.push(parent.join(base_dir).join(rel));
    }
    if let Some(root) = problems_root {
        v.push(root.join(rel));
        v.push(root.join(base_dir).join(rel));
        if let Some(name) = Path::new(rel).file_name() {
            v.push(root.join(name));
        }
    }
    v.into_iter().find(|p| p.is_file())
}
