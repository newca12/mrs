//! Load and parse a proof file together with its linked problem file.

use std::collections::HashSet;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use mrs_tptp::owned::{OwnedTPTPProblem, parse_tptp_owned};
use mrs_tptp::proover::proof_header_link;

// Bound source bytes as well as include depth/count. These are source-size
// limits, not an exact bound on parser/AST memory amplification.
const MAX_INCLUDE_DEPTH: usize = 64;
const MAX_INCLUDE_FILES: usize = 1024;
const MAX_INPUT_FILE_BYTES: usize = 32 * 1024 * 1024;
const MAX_INPUT_TOTAL_BYTES: usize = 128 * 1024 * 1024;

#[derive(Default)]
struct IncludeResolutionState {
    active_paths: HashSet<PathBuf>,
    active_depth: usize,
    files_read: usize,
    total_bytes_read: usize,
}

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
    let proof_text = read_tptp_file(proof_path, MAX_INPUT_FILE_BYTES)
        .map_err(|e| LoadError::ReadProof(e.to_string()))?;

    let proof =
        parse_tptp_owned(proof_text.clone()).map_err(|e| LoadError::ParseProof(e.to_string()))?;

    let header = proof_header_link(&proof_text);

    let mut problem_includes = Vec::new();
    let (problem_path, problem) = if let Some(rel) = header {
        let candidate =
            resolve_problem_path_with_base(proof_path, problems_root, Path::new("."), rel);
        match candidate {
            Some(path) => {
                let problem_limit = MAX_INPUT_FILE_BYTES
                    .min(MAX_INPUT_TOTAL_BYTES.saturating_sub(proof_text.len()));
                let problem_source = read_tptp_file(&path, problem_limit)
                    .map_err(|e| LoadError::ReadProblem(e.to_string(), path.clone()))?;
                let mut include_state = IncludeResolutionState {
                    total_bytes_read: proof_text.len(),
                    ..IncludeResolutionState::default()
                };
                record_job_bytes(&mut include_state, &path, problem_source.len())?;
                let parsed = parse_tptp_owned(problem_source)
                    .map_err(|e| LoadError::ParseProblem(e.to_string(), path.clone()))?;

                let base_dir = path.parent().unwrap_or(Path::new("."));
                let canonical_problem_path = fs::canonicalize(&path).map_err(|error| {
                    LoadError::ReadProblem(
                        format!("cannot canonicalize linked problem file: {error}"),
                        path.clone(),
                    )
                })?;
                include_state.active_paths.insert(canonical_problem_path);
                resolve_includes_recursive(
                    proof_path,
                    problems_root,
                    &parsed.problem().includes,
                    base_dir,
                    &mut problem_includes,
                    &mut include_state,
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
        prob.clear_includes();
        for inc in &job.problem_includes {
            let inc_tptp: &mrs_tptp::ast::TPTPProblem<'static> = inc;
            prob.append_formulas(&inc_tptp.formulas);
        }
    }

    Ok(job)
}

/// Parse a problem and proof from memory and resolve `%include` directives
/// below an explicit root directory.
pub fn load_text(
    problem_text: String,
    proof_text: String,
    include_root: &Path,
) -> Result<LoadedJob, LoadError> {
    let problem_path = include_root.join("stdin.p");
    validate_input_lengths(proof_text.len(), problem_text.len()).map_err(|reason| {
        if proof_text.len() > MAX_INPUT_FILE_BYTES {
            LoadError::ReadProof(reason.to_owned())
        } else {
            LoadError::ReadProblem(reason.to_owned(), problem_path.clone())
        }
    })?;

    let initial_bytes = proof_text
        .len()
        .checked_add(problem_text.len())
        .ok_or_else(|| {
            LoadError::ReadProblem(
                "verification job input byte count overflow".into(),
                problem_path.clone(),
            )
        })?;
    let proof =
        parse_tptp_owned(proof_text.clone()).map_err(|e| LoadError::ParseProof(e.to_string()))?;
    let mut problem = parse_tptp_owned(problem_text)
        .map_err(|e| LoadError::ParseProblem(e.to_string(), problem_path.clone()))?;
    let virtual_proof_path = problem_path;
    let mut problem_includes = Vec::new();
    let mut include_state = IncludeResolutionState {
        total_bytes_read: initial_bytes,
        ..IncludeResolutionState::default()
    };
    resolve_includes_recursive(
        &virtual_proof_path,
        Some(include_root),
        &problem.problem().includes,
        include_root,
        &mut problem_includes,
        &mut include_state,
    )?;
    problem.clear_includes();
    for inc in &problem_includes {
        let inc_tptp: &mrs_tptp::ast::TPTPProblem<'static> = inc;
        problem.append_formulas(&inc_tptp.formulas);
    }
    Ok(LoadedJob {
        proof_path: virtual_proof_path,
        proof_text,
        proof,
        problem_path: None,
        problem: Some(problem),
        problem_includes,
    })
}

#[cfg(test)]
mod include_selection_tests {
    use super::*;

    #[test]
    fn include_selection_imports_only_named_formulas_and_clears_resolved_directives() {
        let root =
            std::env::temp_dir().join(format!("mrs_include_selection_{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("selected.p"),
            "cnf(wanted,axiom,p). cnf(unselected,axiom,q).\n",
        )
        .unwrap();
        let job = load_text(
            "include('selected.p',[wanted]).\n".into(),
            "% Proof : input\ncnf(wanted,axiom,p,file('input',wanted)).\n".to_owned(),
            &root,
        )
        .unwrap();
        let problem = job.problem.as_ref().unwrap().problem();
        assert!(problem.includes.is_empty());
        assert_eq!(
            problem
                .formulas
                .iter()
                .filter(|f| f.name() == "wanted")
                .count(),
            1
        );
        assert_eq!(
            problem
                .formulas
                .iter()
                .filter(|f| f.name() == "unselected")
                .count(),
            0
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn nested_include_selection_is_respected_at_each_level() {
        let root = std::env::temp_dir().join(format!(
            "mrs_nested_include_selection_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("outer.p"),
            "include('inner.p',[chosen]).\ncnf(outer_unselected,axiom,r).\n",
        )
        .unwrap();
        std::fs::write(
            root.join("inner.p"),
            "cnf(chosen,axiom,p). cnf(inner_unselected,axiom,q).\n",
        )
        .unwrap();
        let job = load_text(
            "include('outer.p',[outer_unselected]).\n".into(),
            "% Proof : input\ncnf(outer_unselected,axiom,r,file('input',outer_unselected)).\n"
                .into(),
            &root,
        )
        .unwrap();
        let names: Vec<_> = job
            .problem
            .as_ref()
            .unwrap()
            .problem()
            .formulas
            .iter()
            .map(|formula| formula.name())
            .collect();
        assert_eq!(names, vec!["chosen", "outer_unselected"]);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recursive_include_cycle_is_rejected() {
        let root = std::env::temp_dir().join(format!(
            "mrs_recursive_include_cycle_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("a.p"), "include('b.p').\n").unwrap();
        std::fs::write(root.join("b.p"), "include('a.p').\n").unwrap();

        let error = match load_text(
            "include('a.p').\n".into(),
            "% Proof : input\ncnf(root,axiom,p,file('input',root)).\n".into(),
            &root,
        ) {
            Err(error) => error,
            Ok(_) => panic!("cyclic include graph should be rejected"),
        };

        assert!(error.to_string().contains("include cycle"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn include_cycle_back_to_linked_problem_is_rejected() {
        let root = std::env::temp_dir().join(format!(
            "mrs_linked_problem_include_cycle_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("problem.p"),
            "include('child.p').\ncnf(root,axiom,p).\n",
        )
        .unwrap();
        std::fs::write(root.join("child.p"), "include('problem.p').\n").unwrap();
        std::fs::write(
            root.join("proof.p"),
            "% Proof : problem.p\ncnf(root,axiom,p,file('problem.p',root)).\n",
        )
        .unwrap();

        let error = match load(&root.join("proof.p"), None) {
            Err(error) => error,
            Ok(_) => panic!("include cycle back to the linked problem should be rejected"),
        };

        assert!(error.to_string().contains("include cycle"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn include_nesting_limit_is_enforced() {
        let root =
            std::env::temp_dir().join(format!("mrs_include_depth_limit_{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        for index in 0..=MAX_INCLUDE_DEPTH {
            let contents = if index < MAX_INCLUDE_DEPTH {
                format!("include('nested-{}.p').\n", index + 1)
            } else {
                "cnf(leaf,axiom,p).\n".to_owned()
            };
            std::fs::write(root.join(format!("nested-{index}.p")), contents).unwrap();
        }

        let error = match load_text(
            "include('nested-0.p').\n".into(),
            "% Proof : input\ncnf(root,axiom,p,file('input',root)).\n".into(),
            &root,
        ) {
            Err(error) => error,
            Ok(_) => panic!("include nesting beyond the configured limit should fail"),
        };

        assert!(
            error
                .to_string()
                .contains("include nesting exceeds the limit")
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn total_include_file_limit_is_enforced() {
        let root =
            std::env::temp_dir().join(format!("mrs_include_file_limit_{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("shared.p"), "cnf(shared,axiom,p).\n").unwrap();
        let problem = (0..=MAX_INCLUDE_FILES)
            .map(|_| "include('shared.p').\n")
            .collect::<String>();

        let error = match load_text(
            problem,
            "% Proof : input\ncnf(root,axiom,p,file('input',root)).\n".into(),
            &root,
        ) {
            Err(error) => error,
            Ok(_) => panic!("include count beyond the configured limit should fail"),
        };

        assert!(
            error
                .to_string()
                .contains("include file count exceeds the limit")
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn include_resolution_limits_match_formal_manifest() {
        let manifest: serde_json::Value = serde_json::from_str(include_str!(
            "../../../formal/superposition-checker/supported-rules.json"
        ))
        .unwrap();
        let limits = &manifest["limits"];

        assert_eq!(
            limits["include_resolution_max_active_depth"],
            MAX_INCLUDE_DEPTH
        );
        assert_eq!(
            limits["include_resolution_max_file_count"],
            MAX_INCLUDE_FILES
        );
        assert_eq!(
            limits["include_resolution_cycle_detection"],
            "canonicalized_active_paths"
        );
        assert_eq!(
            limits["include_resolution_max_file_bytes"],
            MAX_INPUT_FILE_BYTES
        );
        assert_eq!(
            limits["include_resolution_max_total_bytes"],
            MAX_INPUT_TOTAL_BYTES
        );
        assert_eq!(limits["include_resolution_file_bytes_bounded"], true);
        assert_eq!(
            limits["include_resolution_total_bytes_includes_proof"],
            true
        );
        assert_eq!(limits["include_resolver_verified"], false);
    }

    #[test]
    fn bounded_tptp_reader_rejects_an_oversized_file_before_parsing() {
        let root =
            std::env::temp_dir().join(format!("mrs_bounded_tptp_reader_{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("oversized.p");
        std::fs::write(&path, "123456789").unwrap();

        let error = read_tptp_file(&path, 8).unwrap_err();
        assert!(error.to_string().contains("byte limit"));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn include_bytes_share_the_job_wide_budget() {
        let path = PathBuf::from("large-include.p");
        let mut state = IncludeResolutionState {
            total_bytes_read: MAX_INPUT_TOTAL_BYTES - 1,
            ..IncludeResolutionState::default()
        };

        assert!(record_job_bytes(&mut state, &path, 1).is_ok());
        assert!(
            record_job_bytes(&mut state, &path, 1)
                .unwrap_err()
                .to_string()
                .contains("total input byte limit")
        );
    }

    #[test]
    fn initial_problem_and_proof_bytes_are_bounded_before_parsing() {
        assert!(validate_input_lengths_with_limits(8, 8, 8, 16).is_ok());
        assert!(
            validate_input_lengths_with_limits(8, 8, 8, 15)
                .unwrap_err()
                .contains("total input byte limit")
        );
        assert!(
            validate_input_lengths(MAX_INPUT_FILE_BYTES + 1, 0)
                .unwrap_err()
                .contains("proof file")
        );
        assert!(
            validate_input_lengths(0, MAX_INPUT_FILE_BYTES + 1)
                .unwrap_err()
                .contains("problem file")
        );
    }
}

fn read_tptp_file(path: &Path, max_bytes: usize) -> io::Result<String> {
    let mut source = String::new();
    fs::File::open(path)?
        .take((max_bytes as u64).saturating_add(1))
        .read_to_string(&mut source)?;
    if source.len() > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("TPTP source exceeds byte limit ({max_bytes})"),
        ));
    }
    Ok(source)
}

fn validate_input_lengths(proof_bytes: usize, problem_bytes: usize) -> Result<usize, String> {
    validate_input_lengths_with_limits(
        proof_bytes,
        problem_bytes,
        MAX_INPUT_FILE_BYTES,
        MAX_INPUT_TOTAL_BYTES,
    )
}

fn validate_input_lengths_with_limits(
    proof_bytes: usize,
    problem_bytes: usize,
    max_file_bytes: usize,
    max_total_bytes: usize,
) -> Result<usize, String> {
    if proof_bytes > max_file_bytes {
        return Err(format!("proof file exceeds byte limit ({max_file_bytes})"));
    }
    if problem_bytes > max_file_bytes {
        return Err(format!(
            "problem file exceeds byte limit ({max_file_bytes})"
        ));
    }
    let total_bytes = proof_bytes
        .checked_add(problem_bytes)
        .ok_or_else(|| "verification job input byte count overflow".to_owned())?;
    if total_bytes > max_total_bytes {
        return Err(format!(
            "verification job exceeds total input byte limit ({max_total_bytes})"
        ));
    }
    Ok(total_bytes)
}

fn record_job_bytes(
    state: &mut IncludeResolutionState,
    path: &Path,
    additional_bytes: usize,
) -> Result<(), LoadError> {
    let total_bytes = state
        .total_bytes_read
        .checked_add(additional_bytes)
        .ok_or_else(|| {
            LoadError::ReadProblem(
                "verification job input byte count overflow".into(),
                path.to_path_buf(),
            )
        })?;
    if total_bytes > MAX_INPUT_TOTAL_BYTES {
        return Err(LoadError::ReadProblem(
            format!("verification job exceeds total input byte limit ({MAX_INPUT_TOTAL_BYTES})"),
            path.to_path_buf(),
        ));
    }
    state.total_bytes_read = total_bytes;
    Ok(())
}

fn resolve_includes_recursive(
    proof_path: &Path,
    problems_root: Option<&Path>,
    includes: &[mrs_tptp::ast::Include<'_>],
    base_dir: &Path,
    problem_includes: &mut Vec<OwnedTPTPProblem>,
    state: &mut IncludeResolutionState,
) -> Result<(), LoadError> {
    for inc in includes {
        let rel = inc.file_name;
        let candidate = resolve_problem_path_with_base(proof_path, problems_root, base_dir, rel);
        if let Some(path) = candidate {
            let canonical_path = fs::canonicalize(&path).map_err(|error| {
                LoadError::ReadProblem(
                    format!("cannot canonicalize included file: {error}"),
                    path.clone(),
                )
            })?;
            if state.active_paths.contains(&canonical_path) {
                return Err(LoadError::ReadProblem(
                    "include cycle detected".into(),
                    path,
                ));
            }
            if state.active_depth >= MAX_INCLUDE_DEPTH {
                return Err(LoadError::ReadProblem(
                    format!("include nesting exceeds the limit of {MAX_INCLUDE_DEPTH}"),
                    path,
                ));
            }
            if state.files_read >= MAX_INCLUDE_FILES {
                return Err(LoadError::ReadProblem(
                    format!("include file count exceeds the limit of {MAX_INCLUDE_FILES}"),
                    path,
                ));
            }
            state.files_read += 1;

            let file_limit = MAX_INPUT_FILE_BYTES
                .min(MAX_INPUT_TOTAL_BYTES.saturating_sub(state.total_bytes_read));
            let source = read_tptp_file(&path, file_limit)
                .map_err(|e| LoadError::ReadProblem(e.to_string(), path.clone()))?;
            record_job_bytes(state, &path, source.len())?;
            let mut parsed = parse_tptp_owned(source)
                .map_err(|e| LoadError::ParseProblem(e.to_string(), path.clone()))?;

            state.active_paths.insert(canonical_path.clone());
            state.active_depth += 1;
            let inc_dir = path.parent().unwrap_or(Path::new("."));
            let nested_result = resolve_includes_recursive(
                proof_path,
                problems_root,
                &parsed.problem().includes,
                inc_dir,
                problem_includes,
                state,
            );
            state.active_depth -= 1;
            state.active_paths.remove(&canonical_path);
            nested_result?;

            parsed.retain_formula_names(inc.selection.as_deref());
            parsed.clear_includes();
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

/// Resolve the `% Proof :` header path to an existing problem file.
///
/// Candidate order matters: a relative header such as `Problems/foo.p` is
/// resolved *against the proof's own directory* first, then against an
/// explicit `--problems-dir` root. Only if neither matches do we fall back to
/// the bare relative path, i.e. against the process working directory. Putting
/// the cwd-relative candidate last means a proof verified from an unrelated
/// working directory cannot silently bind to a same-named `Problems/foo.p` in
/// that directory, and a genuinely missing problem is now reported as missing
/// (and refused by the verifier) instead of binding to whatever happens to
/// sit in the caller's cwd.
pub fn resolve_problem_path_with_base(
    proof_path: &Path,
    problems_root: Option<&Path>,
    base_dir: &Path,
    rel: &str,
) -> Option<PathBuf> {
    let mut v: Vec<PathBuf> = Vec::new();
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
    if let Ok(tptp_val) = std::env::var("TPTP") {
        let root = Path::new(&tptp_val);
        v.push(root.join(rel));
        v.push(root.join(base_dir).join(rel));
        if let Some(name) = Path::new(rel).file_name() {
            v.push(root.join(name));
        }
    }
    // Last resort: the header path as given, relative to the process cwd.
    v.push(PathBuf::from(rel));
    v.push(base_dir.join(rel));
    v.into_iter().find(|p| p.is_file())
}
