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
pub fn load(proof_path: &Path, problems_root: Option<&Path>) -> Result<LoadedJob, LoadError> {
    let proof_text =
        fs::read_to_string(proof_path).map_err(|e| LoadError::ReadProof(e.to_string()))?;

    let proof = parse_tptp_file(proof_path).map_err(|e| LoadError::ParseProof(e.to_string()))?;

    let header = proof_header_link(&proof_text);

    let (problem_path, problem) = if let Some(rel) = header {
        let candidate = resolve_problem_path(proof_path, problems_root, rel);
        match candidate {
            Some(path) => {
                let parsed = parse_tptp_file(&path)
                    .map_err(|e| LoadError::ParseProblem(e.to_string(), path.clone()))?;
                (Some(path), Some(parsed))
            }
            None => (None, None),
        }
    } else {
        (None, None)
    };

    Ok(LoadedJob {
        proof_path: proof_path.to_path_buf(),
        proof_text,
        proof,
        problem_path,
        problem,
    })
}

fn resolve_problem_path(
    proof_path: &Path,
    problems_root: Option<&Path>,
    rel: &str,
) -> Option<PathBuf> {
    // Try (1) literal as-is, (2) relative to the proof's directory,
    // (3) relative to `problems_root` if given,
    // (4) `problems_root.join(basename)`.
    let candidates: Vec<PathBuf> = {
        let mut v = vec![PathBuf::from(rel)];
        if let Some(parent) = proof_path.parent() {
            v.push(parent.join(rel));
        }
        if let Some(root) = problems_root {
            v.push(root.join(rel));
            if let Some(name) = Path::new(rel).file_name() {
                v.push(root.join(name));
            }
        }
        v
    };
    candidates.into_iter().find(|p| p.is_file())
}
