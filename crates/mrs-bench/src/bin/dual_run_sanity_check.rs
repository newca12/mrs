//! Dual-Run Sanity Check for Automated Soundness and Metamorphic Auditing.
//!
//! Evaluates first-order problems against core soundness invariants:
//! 1. Dual-Polarity Invariant: A problem Φ = (A, C) and its dual Φ* = (A, ¬C)
//!    cannot BOTH yield refutations (Theorem) unless the axioms A themselves
//!    are provably unsatisfiable.
//! 2. Metamorphic Invariant: Running with 1 worker (deterministic baseline) vs
//!    N workers (concurrent portfolio with clause sharing) must never produce
//!    contradictory definitive SZS statuses.
//! 3. Contradiction Sensitivity: A set claiming Satisfiable must not remain
//!    Satisfiable when an explicit contradiction ($false) is injected.
//! 4. Canary Integrity: CASC division canaries must not suffer include-drift
//!    contamination or unexpected status flips.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use tempfile::Builder;
use wait_timeout::ChildExt;

use mrs_szs::SzsStatus;
use mrs_tptp::ast::cnf::{CNFFormula, CNFLiteral, CNFStatement};
use mrs_tptp::ast::fof::{FOFFormula, FOFStatement};
use mrs_tptp::ast::tff::{TFFFormula, TFFStatement};
use mrs_tptp::ast::thf::{THFFormula, THFStatement};
use mrs_tptp::ast::{
    AnnotatedFormula, CNFAnnotated, FOFAnnotated, FormulaRole, TFFAnnotated, THFAnnotated,
};
use mrs_tptp::parse_tptp;

// =============================================================================
// CLI Arguments and Configuration
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckMode {
    Polarity,
    Metamorphic,
    Both,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub problems: Vec<PathBuf>,
    pub check_canaries: bool,
    pub time_limit: u64,
    pub workers: usize,
    pub schedule: Option<String>,
    pub mrs_bin: PathBuf,
    pub mode: CheckMode,
    pub fail_fast: bool,
    pub quiet: bool,
    pub color: bool,
    pub check_drift: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            problems: Vec::new(),
            check_canaries: false,
            time_limit: 5,
            workers: 4,
            schedule: None,
            mrs_bin: PathBuf::new(),
            mode: CheckMode::Both,
            fail_fast: false,
            quiet: false,
            color: true,
            check_drift: false,
        }
    }
}

// =============================================================================
// Prover Run Results and Invariant Violations
// =============================================================================

#[derive(Debug, Clone)]
pub struct RunResult {
    pub status: SzsStatus,
    pub raw_status: String,
    pub elapsed_ms: u64,
    pub processed: u64,
    pub generated: u64,
    pub lrs_discarded: u64,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone)]
pub enum InvariantViolation {
    ContradictoryTheorems {
        problem: String,
        orig_status: SzsStatus,
        dual_status: SzsStatus,
        axioms_status: Option<SzsStatus>,
        detail: String,
    },
    MetamorphicDisagreement {
        problem: String,
        workers_1_status: SzsStatus,
        workers_n_status: SzsStatus,
        workers_n: usize,
        detail: String,
    },
    ContradictionBlindness {
        problem: String,
        orig_status: SzsStatus,
        contra_status: SzsStatus,
        detail: String,
    },
    CanaryContamination {
        problem: String,
        canary_name: String,
        reason: String,
    },
}

impl InvariantViolation {
    pub fn description(&self) -> String {
        match self {
            Self::ContradictoryTheorems {
                problem,
                orig_status,
                dual_status,
                axioms_status,
                detail,
            } => {
                let ax_str = match axioms_status {
                    Some(s) => format!("{s}"),
                    None => "not run".to_string(),
                };
                format!(
                    "FATAL UNSOUNDNESS: Problem '{problem}' proved both conjecture ({orig_status}) \
                     and its dual ({dual_status})! Axioms status: {ax_str}. {detail}"
                )
            }
            Self::MetamorphicDisagreement {
                problem,
                workers_1_status,
                workers_n_status,
                workers_n,
                detail,
            } => {
                format!(
                    "FATAL DISAGREEMENT: Problem '{problem}' gave contradictory definitive results \
                     under different worker counts: 1 worker = {workers_1_status}, {workers_n} workers = {workers_n_status}. {detail}"
                )
            }
            Self::ContradictionBlindness {
                problem,
                orig_status,
                contra_status,
                detail,
            } => {
                format!(
                    "FATAL UNSOUNDNESS: Problem '{problem}' reported {orig_status}, but remained \
                     {contra_status} after injecting explicit $false contradiction! {detail}"
                )
            }
            Self::CanaryContamination {
                problem,
                canary_name,
                reason,
            } => {
                format!("CANARY CONTAMINATION: Canary '{canary_name}' ({problem}): {reason}")
            }
        }
    }
}

// =============================================================================
// ANSI Colors
// =============================================================================

struct Colors {
    enabled: bool,
}

impl Colors {
    fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    fn red(&self, s: &str) -> String {
        if self.enabled {
            format!("\x1b[0;31m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }

    fn green(&self, s: &str) -> String {
        if self.enabled {
            format!("\x1b[0;32m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }

    fn yellow(&self, s: &str) -> String {
        if self.enabled {
            format!("\x1b[0;33m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }

    fn cyan(&self, s: &str) -> String {
        if self.enabled {
            format!("\x1b[0;36m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }

    fn bold(&self, s: &str) -> String {
        if self.enabled {
            format!("\x1b[1m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }
}

// =============================================================================
// Dual Problem Generation
// =============================================================================

pub struct ProblemDuals {
    pub has_conjecture: bool,
    pub dual_content: Option<String>,
    pub axioms_only_content: Option<String>,
    pub contradiction_injected_content: String,
}

fn negate_cnf_literal<'a>(lit: &CNFLiteral<'a>) -> CNFLiteral<'a> {
    match lit {
        CNFLiteral::Positive(atom) => CNFLiteral::Negative(atom.clone()),
        CNFLiteral::Negative(atom) => CNFLiteral::Positive(atom.clone()),
        CNFLiteral::Equality(l, r) => CNFLiteral::Inequality(l.clone(), r.clone()),
        CNFLiteral::Inequality(l, r) => CNFLiteral::Equality(l.clone(), r.clone()),
    }
}

pub fn generate_problem_duals(file_content: &str) -> Result<ProblemDuals, String> {
    let ast = parse_tptp(file_content).map_err(|e| format!("Failed to parse TPTP: {e}"))?;

    let has_conjecture = ast
        .formulas
        .iter()
        .any(|f| f.role() == FormulaRole::Conjecture || f.role() == FormulaRole::NegatedConjecture);

    let mut dual_buf = String::new();
    let mut ax_buf = String::new();
    let mut contra_buf = String::new();

    // Include directives are preserved across all variants
    for inc in &ast.includes {
        let inc_str = format!("{inc}\n");
        dual_buf.push_str(&inc_str);
        ax_buf.push_str(&inc_str);
        contra_buf.push_str(&inc_str);
    }

    let mut dual_possible = has_conjecture;

    for f in &ast.formulas {
        let is_conj = f.role() == FormulaRole::Conjecture;
        let is_neg_conj = f.role() == FormulaRole::NegatedConjecture;

        // Populate contradiction-injected problem with all formulas
        contra_buf.push_str(&format!("{f}\n"));

        // Axioms-only problem excludes conjecture formulas
        if !is_conj && !is_neg_conj {
            ax_buf.push_str(&format!("{f}\n"));
        }

        // Dual problem generation
        if is_conj {
            match f {
                AnnotatedFormula::FOF(ann) => match &ann.formula {
                    FOFStatement::Logical(inner) => {
                        let negated = FOFStatement::Logical(FOFFormula::Negation(Box::new(
                            FOFFormula::Parens(Box::new(inner.clone())),
                        )));
                        let dual_fof = FOFAnnotated {
                            name: ann.name,
                            role: FormulaRole::Conjecture,
                            formula: negated,
                            annotations: ann.annotations.clone(),
                        };
                        dual_buf.push_str(&format!("{dual_fof}\n"));
                    }
                    FOFStatement::Sequent(..) => {
                        dual_possible = false;
                    }
                },
                AnnotatedFormula::TFF(ann) => match &ann.formula {
                    TFFStatement::Logical(inner) => {
                        let negated = TFFStatement::Logical(TFFFormula::Negation(Box::new(
                            TFFFormula::Parens(Box::new(inner.clone())),
                        )));
                        let dual_tff = TFFAnnotated {
                            name: ann.name,
                            role: FormulaRole::Conjecture,
                            formula: negated,
                            annotations: ann.annotations.clone(),
                        };
                        dual_buf.push_str(&format!("{dual_tff}\n"));
                    }
                    _ => {
                        dual_possible = false;
                    }
                },
                AnnotatedFormula::THF(ann) => match &ann.formula {
                    THFStatement::Logical(inner) => {
                        let negated = THFStatement::Logical(THFFormula::Negation(Box::new(
                            THFFormula::Parens(Box::new(inner.clone())),
                        )));
                        let dual_thf = THFAnnotated {
                            name: ann.name,
                            role: FormulaRole::Conjecture,
                            formula: negated,
                            annotations: ann.annotations.clone(),
                        };
                        dual_buf.push_str(&format!("{dual_thf}\n"));
                    }
                    _ => {
                        dual_possible = false;
                    }
                },
                AnnotatedFormula::CNF(ann) => {
                    match &ann.formula {
                        CNFStatement::Logical(CNFFormula::Disjunction(lits)) if lits.len() == 1 => {
                            let flipped = negate_cnf_literal(&lits[0]);
                            let dual_cnf = CNFAnnotated {
                                name: ann.name,
                                role: FormulaRole::Conjecture,
                                formula: CNFStatement::Logical(CNFFormula::Disjunction(vec![
                                    flipped,
                                ])),
                                annotations: ann.annotations.clone(),
                            };
                            dual_buf.push_str(&format!("{dual_cnf}\n"));
                        }
                        _ => {
                            // Non-unit CNF clause cannot be inverted into a single clause
                            dual_possible = false;
                        }
                    }
                }
                _ => {
                    dual_possible = false;
                }
            }
        } else if is_neg_conj {
            match f {
                AnnotatedFormula::CNF(ann) => match &ann.formula {
                    CNFStatement::Logical(CNFFormula::Disjunction(lits)) if lits.len() == 1 => {
                        let flipped = negate_cnf_literal(&lits[0]);
                        let dual_cnf = CNFAnnotated {
                            name: ann.name,
                            role: FormulaRole::NegatedConjecture,
                            formula: CNFStatement::Logical(CNFFormula::Disjunction(vec![flipped])),
                            annotations: ann.annotations.clone(),
                        };
                        dual_buf.push_str(&format!("{dual_cnf}\n"));
                    }
                    _ => {
                        dual_possible = false;
                    }
                },
                _ => {
                    dual_possible = false;
                }
            }
        } else {
            dual_buf.push_str(&format!("{f}\n"));
        }
    }

    // Append explicit contradiction probe to contra_buf
    contra_buf.push_str("fof(dual_sanity_contradiction, axiom, $false).\n");

    Ok(ProblemDuals {
        has_conjecture,
        dual_content: if dual_possible { Some(dual_buf) } else { None },
        axioms_only_content: if has_conjecture { Some(ax_buf) } else { None },
        contradiction_injected_content: contra_buf,
    })
}

// =============================================================================
// Prover Invocation
// =============================================================================

fn run_prover(
    mrs_bin: &Path,
    problem_path: &Path,
    time_limit: u64,
    workers: usize,
    schedule: Option<&str>,
    tptp_root: Option<&Path>,
) -> io::Result<RunResult> {
    let mut command = Command::new(mrs_bin);
    command
        .arg("--time")
        .arg(time_limit.to_string())
        .arg("--workers")
        .arg(workers.to_string());

    if let Some(sched) = schedule {
        command.arg("--schedule").arg(sched);
    }

    command.arg(problem_path);

    // Guarantee stack headroom matching invoke.sh
    command.env("RUST_MIN_STACK", "67108864");

    if let Some(tptp) = tptp_root {
        command.env("TPTP", tptp);
    } else if env::var("TPTP").is_err() {
        // Fallback to local casc-30 directory if available
        let default_tptp = PathBuf::from("crates/mrs-bench/problems/casc-30");
        if default_tptp.is_dir() {
            command.env("TPTP", &default_tptp);
        }
    }

    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = command.spawn()?;
    let timeout = Duration::from_secs(time_limit.saturating_add(5));

    let exit_status = match child.wait_timeout(timeout)? {
        Some(status) => status,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(RunResult {
                status: SzsStatus::Timeout,
                raw_status: "Timeout".to_string(),
                elapsed_ms: time_limit * 1000,
                processed: 0,
                generated: 0,
                lrs_discarded: 0,
                exit_code: 124,
                stdout: String::new(),
                stderr: "Killed after timeout".to_string(),
            });
        }
    };

    let output = child.wait_with_output()?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    let raw_status = stdout
        .lines()
        .find_map(|line| line.strip_prefix("% SZS status "))
        .and_then(|line| line.split_whitespace().next())
        .unwrap_or("Unknown")
        .to_string();

    let status = SzsStatus::parse(&raw_status).unwrap_or(SzsStatus::Unknown);

    let detail_line = stderr
        .lines()
        .find_map(|line| line.strip_prefix("% SZS detail "))
        .unwrap_or("");

    let details = parse_detail(detail_line);
    let elapsed_ms = details
        .get("elapsed_ms")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let processed = details
        .get("processed")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let generated = details
        .get("generated")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let lrs_discarded = details
        .get("lrs_discarded")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let exit_code = exit_status.code().unwrap_or(1);

    Ok(RunResult {
        status,
        raw_status,
        elapsed_ms,
        processed,
        generated,
        lrs_discarded,
        exit_code,
        stdout,
        stderr,
    })
}

fn parse_detail(detail: &str) -> BTreeMap<String, String> {
    detail
        .split_whitespace()
        .filter_map(|field| field.split_once('='))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

// =============================================================================
// Division and Canary Specifications
// =============================================================================

#[derive(Debug, Clone)]
struct CanarySpec {
    division: &'static str,
    name: &'static str,
    file_rel: &'static str,
    max_lrs_discarded: u64,
    max_processed: u64,
}

const CANARIES: &[CanarySpec] = &[
    CanarySpec {
        division: "EPS",
        name: "HWC004-1",
        file_rel: "EPS/HWC004-1.p",
        max_lrs_discarded: 1000,
        max_processed: u64::MAX,
    },
    CanarySpec {
        division: "FNE",
        name: "CSR026+3",
        file_rel: "FNE/CSR026+3.p",
        max_lrs_discarded: u64::MAX,
        max_processed: 1000,
    },
    CanarySpec {
        division: "FEQ",
        name: "AGT005+1",
        file_rel: "FEQ/AGT005+1.p",
        max_lrs_discarded: u64::MAX,
        max_processed: 1000,
    },
    CanarySpec {
        division: "UEQ",
        name: "ALG212-10",
        file_rel: "UEQ/ALG212-10.p",
        max_lrs_discarded: u64::MAX,
        max_processed: 1000,
    },
];

fn resolve_canary_path(spec: &CanarySpec, tptp_root: Option<&Path>) -> Option<PathBuf> {
    if let Some(root) = tptp_root {
        let p = root.join(spec.file_rel);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Ok(env_tptp) = env::var("TPTP") {
        let p = Path::new(&env_tptp).join(spec.file_rel);
        if p.is_file() {
            return Some(p);
        }
    }
    let local = Path::new("crates/mrs-bench/problems/casc-30").join(spec.file_rel);
    if local.is_file() {
        return Some(local);
    }
    None
}

// =============================================================================
// Core Sanity Checker Runner
// =============================================================================

pub struct ProblemReport {
    pub problem_name: String,
    pub path: PathBuf,
    pub original_result: RunResult,
    pub dual_result: Option<RunResult>,
    pub axioms_result: Option<RunResult>,
    pub worker_1_result: Option<RunResult>,
    pub contra_result: Option<RunResult>,
    pub violations: Vec<InvariantViolation>,
}

fn detect_division_schedule(path: &Path) -> &'static str {
    for comp in path.components() {
        if let Some(s) = comp.as_os_str().to_str() {
            match s.to_ascii_uppercase().as_str() {
                "FNE" => return "casc_fne",
                "FEQ" => return "casc_feq",
                "UEQ" => return "casc_ueq",
                "EPS" => return "casc_eps",
                "EPU" => return "casc_epu",
                "ICU" => return "casc_icu",
                _ => {}
            }
        }
    }
    "casc"
}

fn check_problem(
    problem_path: &Path,
    config: &Config,
    tptp_root: Option<&Path>,
    canary_spec: Option<&CanarySpec>,
) -> io::Result<ProblemReport> {
    let file_content = fs::read_to_string(problem_path)?;
    let problem_name = problem_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    let duals = generate_problem_duals(&file_content)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let problem_dir = problem_path.parent().unwrap_or_else(|| Path::new("."));

    let effective_sched = match &config.schedule {
        Some(s) => s.as_str(),
        None => detect_division_schedule(problem_path),
    };

    // 1. Run Original Problem with N workers
    let orig_res = run_prover(
        &config.mrs_bin,
        problem_path,
        config.time_limit,
        config.workers,
        Some(effective_sched),
        tptp_root,
    )?;

    let mut violations = Vec::new();
    let mut dual_result = None;
    let mut axioms_result = None;
    let mut worker_1_result = None;
    let mut contra_result = None;

    // 2. Dual-Polarity Check (if mode enables and dual is possible)
    if (config.mode == CheckMode::Polarity || config.mode == CheckMode::Both)
        && duals.has_conjecture
        && let Some(dual_code) = &duals.dual_content
    {
        let mut dual_temp = Builder::new()
            .prefix(".dual_sanity_")
            .suffix(".p")
            .tempfile_in(problem_dir)?;
        dual_temp.write_all(dual_code.as_bytes())?;
        dual_temp.flush()?;

        let d_res = run_prover(
            &config.mrs_bin,
            dual_temp.path(),
            config.time_limit,
            config.workers,
            Some(effective_sched),
            tptp_root,
        )?;

        // Invariant: Non-Contradiction
        let orig_is_refutation = matches!(
            orig_res.status,
            SzsStatus::Theorem | SzsStatus::Unsatisfiable
        );
        let dual_is_refutation =
            matches!(d_res.status, SzsStatus::Theorem | SzsStatus::Unsatisfiable);

        if orig_is_refutation && dual_is_refutation {
            // Both claimed refutation! Check axioms alone to rule out inconsistent background axioms
            let ax_res = if let Some(ax_code) = &duals.axioms_only_content {
                let mut ax_temp = Builder::new()
                    .prefix(".ax_sanity_")
                    .suffix(".p")
                    .tempfile_in(problem_dir)?;
                ax_temp.write_all(ax_code.as_bytes())?;
                ax_temp.flush()?;

                let res = run_prover(
                    &config.mrs_bin,
                    ax_temp.path(),
                    config.time_limit,
                    config.workers,
                    Some(effective_sched),
                    tptp_root,
                )?;
                Some(res)
            } else {
                None
            };

            let axioms_are_unsat = ax_res
                .as_ref()
                .map(|r| r.status == SzsStatus::Unsatisfiable)
                .unwrap_or(false);

            if !axioms_are_unsat {
                violations.push(InvariantViolation::ContradictoryTheorems {
                    problem: problem_name.clone(),
                    orig_status: orig_res.status,
                    dual_status: d_res.status,
                    axioms_status: ax_res.as_ref().map(|r| r.status),
                    detail: format!(
                        "Both problem and its negation produced refutations within {}ms and {}ms, \
                         while axioms alone yielded {:?}.",
                        orig_res.elapsed_ms,
                        d_res.elapsed_ms,
                        ax_res.as_ref().map(|r| r.status)
                    ),
                });
            }
            axioms_result = ax_res;
        }

        dual_result = Some(d_res);
    }

    // 3. Metamorphic / Dual-Worker Check (1 worker vs N workers)
    if config.mode == CheckMode::Metamorphic || config.mode == CheckMode::Both {
        let w1_res = run_prover(
            &config.mrs_bin,
            problem_path,
            config.time_limit,
            1,
            Some(effective_sched),
            tptp_root,
        )?;

        if w1_res.status.is_success()
            && orig_res.status.is_success()
            && w1_res.status != orig_res.status
        {
            violations.push(InvariantViolation::MetamorphicDisagreement {
                problem: problem_name.clone(),
                workers_1_status: w1_res.status,
                workers_n_status: orig_res.status,
                workers_n: config.workers,
                detail: format!(
                    "Sequential search (w=1) produced {}, while parallel portfolio (w={}) \
                     produced {}.",
                    w1_res.status, config.workers, orig_res.status
                ),
            });
        }
        worker_1_result = Some(w1_res);
    }

    // 4. Contradiction Injection Check for SAT claims
    if matches!(
        orig_res.status,
        SzsStatus::Satisfiable | SzsStatus::CounterSatisfiable
    ) {
        let mut contra_temp = Builder::new()
            .prefix(".contra_sanity_")
            .suffix(".p")
            .tempfile_in(problem_dir)?;
        contra_temp.write_all(duals.contradiction_injected_content.as_bytes())?;
        contra_temp.flush()?;

        let c_res = run_prover(
            &config.mrs_bin,
            contra_temp.path(),
            config.time_limit,
            config.workers,
            Some(effective_sched),
            tptp_root,
        )?;

        if matches!(c_res.status, SzsStatus::Satisfiable) {
            violations.push(InvariantViolation::ContradictionBlindness {
                problem: problem_name.clone(),
                orig_status: orig_res.status,
                contra_status: c_res.status,
                detail: "Explicit $false contradiction failed to refute; problem was reported Satisfiable."
                    .to_string(),
            });
        }
        contra_result = Some(c_res);
    }

    // 5. Canary Drift / Bloat Audit
    if config.check_drift
        && let Some(spec) = canary_spec
    {
        if orig_res.lrs_discarded > spec.max_lrs_discarded {
            violations.push(InvariantViolation::CanaryContamination {
                problem: problem_name.clone(),
                canary_name: spec.name.to_string(),
                reason: format!(
                    "LRS discards {} exceeded threshold {} (Axioms include-drift)",
                    orig_res.lrs_discarded, spec.max_lrs_discarded
                ),
            });
        }
        if orig_res.processed > spec.max_processed {
            violations.push(InvariantViolation::CanaryContamination {
                problem: problem_name.clone(),
                canary_name: spec.name.to_string(),
                reason: format!(
                    "Processed clauses {} exceeded threshold {}",
                    orig_res.processed, spec.max_processed
                ),
            });
        }
    }

    Ok(ProblemReport {
        problem_name,
        path: problem_path.to_path_buf(),
        original_result: orig_res,
        dual_result,
        axioms_result,
        worker_1_result,
        contra_result,
        violations,
    })
}

// =============================================================================
// CLI Parsing and Main Execution
// =============================================================================

fn print_usage() {
    println!(
        "dual_run_sanity_check: Automated Dual-Polarity & Metamorphic Soundness Auditing\n\n\
        Usage: dual_run_sanity_check [OPTIONS] [PROBLEMS...]\n\n\
        Options:\n\
          -p, --problem <FILE>      Check a specific TPTP problem file\n\
          -c, --canaries            Run checks against all canonical CASC division canaries\n\
          -l, --problems <FILE>     File containing list of TPTP problem paths to check\n\
          -t, --time <SECS>         Time limit in seconds per prover execution (default: 5)\n\
          -w, --workers <N>         Worker count for multi-worker run (default: 4)\n\
          -s, --schedule <NAME>     Schedule name to pass to mrs (default: auto)\n\
              --mrs <PATH>          Path to mrs binary (default: searches target/release/mrs, target/debug/mrs)\n\
          -m, --mode <MODE>         Mode: polarity, metamorphic, or both (default: both)\n\
              --fail-fast           Abort immediately on first invariant violation\n\
              --no-color            Disable ANSI color output\n\
              --check-drift         Fail if canary processed clauses or LRS discards exceed thresholds\n\
          -q, --quiet               Suppress per-step details, report summary only\n\
          -h, --help                Print this help message\n"
    );
}

fn locate_mrs_binary() -> io::Result<PathBuf> {
    let candidates = [
        "target/release/mrs",
        "target/debug/mrs",
        "../target/release/mrs",
        "../../target/release/mrs",
        "../../../target/release/mrs",
    ];
    for cand in &candidates {
        let p = PathBuf::from(cand);
        if p.is_file() {
            return Ok(p);
        }
    }
    // Search current executable's directory
    if let Ok(mut exe) = env::current_exe() {
        exe.pop();
        let p = exe.join("mrs");
        if p.is_file() {
            return Ok(p);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "mrs binary not found. Build with 'cargo build --release' or specify --mrs <PATH>.",
    ))
}

fn parse_cli_args() -> io::Result<Config> {
    let mut config = Config::default();
    let mut args = env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-p" | "--problem" => {
                let val = args.next().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "--problem requires a file path",
                    )
                })?;
                config.problems.push(PathBuf::from(val));
            }
            "-c" | "--canaries" => {
                config.check_canaries = true;
            }
            "-l" | "--problems" => {
                let list_file = args.next().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "--problems requires a file path containing paths",
                    )
                })?;
                let content = fs::read_to_string(&list_file)?;
                for line in content.lines() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() && !trimmed.starts_with('#') {
                        config.problems.push(PathBuf::from(trimmed));
                    }
                }
            }
            "-t" | "--time" => {
                let val = args.next().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "--time requires a positive integer",
                    )
                })?;
                config.time_limit = val.parse::<u64>().map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidInput, "invalid --time integer")
                })?;
            }
            "-w" | "--workers" => {
                let val = args.next().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "--workers requires a positive integer",
                    )
                })?;
                config.workers = val.parse::<usize>().map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidInput, "invalid --workers integer")
                })?;
            }
            "-s" | "--schedule" => {
                let val = args.next().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "--schedule requires a name")
                })?;
                config.schedule = Some(val);
            }
            "--mrs" => {
                let val = args.next().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "--mrs requires a binary path")
                })?;
                config.mrs_bin = PathBuf::from(val);
            }
            "-m" | "--mode" => {
                let val = args.next().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "--mode requires 'polarity', 'metamorphic', or 'both'",
                    )
                })?;
                match val.to_lowercase().as_str() {
                    "polarity" => config.mode = CheckMode::Polarity,
                    "metamorphic" => config.mode = CheckMode::Metamorphic,
                    "both" => config.mode = CheckMode::Both,
                    other => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("unknown --mode: {other}"),
                        ));
                    }
                }
            }
            "--fail-fast" => {
                config.fail_fast = true;
            }
            "--no-color" => {
                config.color = false;
            }
            "--check-drift" => {
                config.check_drift = true;
            }
            "-q" | "--quiet" => {
                config.quiet = true;
            }
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            pos if !pos.starts_with('-') => {
                config.problems.push(PathBuf::from(pos));
            }
            unknown => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown option '{unknown}'. Use --help for usage."),
                ));
            }
        }
    }

    if config.mrs_bin.as_os_str().is_empty() {
        config.mrs_bin = locate_mrs_binary()?;
    } else if !config.mrs_bin.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "specified mrs binary not found: {}",
                config.mrs_bin.display()
            ),
        ));
    }

    Ok(config)
}

fn main() {
    let config = match parse_cli_args() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    let colors = Colors::new(config.color);

    println!(
        "{}",
        colors.cyan("=== MRS Dual-Run & Metamorphic Sanity Checker ===")
    );
    println!(
        "Prover Binary : {}",
        colors.yellow(&config.mrs_bin.display().to_string())
    );
    println!("Per-Run Time  : {}s", config.time_limit);
    println!(
        "Workers       : 1 (deterministic) vs {} (portfolio)",
        config.workers
    );
    println!("Check Mode    : {:?}", config.mode);
    println!("---------------------------------------------------------");

    let tptp_root = env::var("TPTP").ok().map(PathBuf::from);

    let mut total_evaluated = 0;
    let mut total_passed = 0;
    let mut total_violations = Vec::new();

    // 1. Process Canary Suite if requested
    if config.check_canaries {
        println!("{}", colors.bold("Executing CASC Division Canaries:"));
        for spec in CANARIES {
            let canary_path = match resolve_canary_path(spec, tptp_root.as_deref()) {
                Some(p) => p,
                None => {
                    eprintln!(
                        "  [{}] {} : {}",
                        spec.division,
                        spec.name,
                        colors.yellow("SKIPPED (file not found)")
                    );
                    continue;
                }
            };

            total_evaluated += 1;
            match check_problem(&canary_path, &config, tptp_root.as_deref(), Some(spec)) {
                Ok(report) => {
                    let has_err = !report.violations.is_empty();
                    if has_err {
                        println!(
                            "  [{}] {:<10} : {}",
                            spec.division,
                            spec.name,
                            colors.red("VIOLATION")
                        );
                        for v in &report.violations {
                            println!("    * {}", colors.red(&v.description()));
                        }
                        total_violations.extend(report.violations);
                        if config.fail_fast {
                            eprintln!(
                                "\n{}",
                                colors.red("Aborting immediately (--fail-fast enabled).")
                            );
                            std::process::exit(2);
                        }
                    } else {
                        println!(
                            "  [{}] {:<10} : {} (orig={}, time={:.3}s, processed={}, lrs={})",
                            spec.division,
                            spec.name,
                            colors.green("OK"),
                            report.original_result.status,
                            report.original_result.elapsed_ms as f64 / 1000.0,
                            report.original_result.processed,
                            report.original_result.lrs_discarded,
                        );
                        total_passed += 1;
                    }
                }
                Err(e) => {
                    eprintln!(
                        "  [{}] {:<10} : {} ({})",
                        spec.division,
                        spec.name,
                        colors.red("ERROR"),
                        e
                    );
                }
            }
        }
        println!("---------------------------------------------------------");
    }

    // 2. Process custom / user problems
    if !config.problems.is_empty() {
        println!("{}", colors.bold("Executing Problem Suite:"));
        for prob in &config.problems {
            total_evaluated += 1;
            let prob_name = prob.file_name().and_then(|s| s.to_str()).unwrap_or("prob");
            match check_problem(prob, &config, tptp_root.as_deref(), None) {
                Ok(report) => {
                    let has_err = !report.violations.is_empty();
                    if has_err {
                        println!("  {:<25} : {}", prob_name, colors.red("VIOLATION"));
                        for v in &report.violations {
                            println!("    * {}", colors.red(&v.description()));
                        }
                        total_violations.extend(report.violations);
                        if config.fail_fast {
                            eprintln!(
                                "\n{}",
                                colors.red("Aborting immediately (--fail-fast enabled).")
                            );
                            std::process::exit(2);
                        }
                    } else {
                        let dual_info = match &report.dual_result {
                            Some(d) => format!(", dual={}", d.status),
                            None => "".to_string(),
                        };
                        let w1_info = match &report.worker_1_result {
                            Some(w1) => format!(", w1={}", w1.status),
                            None => "".to_string(),
                        };
                        println!(
                            "  {:<25} : {} (orig={}{}{}, time={:.3}s)",
                            prob_name,
                            colors.green("OK"),
                            report.original_result.status,
                            dual_info,
                            w1_info,
                            report.original_result.elapsed_ms as f64 / 1000.0,
                        );
                        total_passed += 1;
                    }
                }
                Err(e) => {
                    eprintln!("  {:<25} : {} ({})", prob_name, colors.red("ERROR"), e);
                }
            }
        }
        println!("---------------------------------------------------------");
    }

    if total_evaluated == 0 {
        println!(
            "{}",
            colors.yellow(
                "No problems evaluated. Pass problems or --canaries. Use --help for usage."
            )
        );
        std::process::exit(0);
    }

    println!("Summary:");
    println!("  Total Evaluated : {total_evaluated}");
    println!("  Passed Invariants: {total_passed}");
    println!("  Violations      : {}", total_violations.len());

    if !total_violations.is_empty() {
        println!();
        println!(
            "{}",
            colors.red("Result: KO - SOUNDNESS INVARIANT VIOLATIONS DETECTED")
        );
        for (idx, v) in total_violations.iter().enumerate() {
            println!("  [{}] {}", idx + 1, colors.red(&v.description()));
        }
        std::process::exit(2);
    } else {
        println!();
        println!(
            "{}",
            colors.green("Result: OK - ALL SOUNDNESS & METAMORPHIC INVARIANTS SATISFIED")
        );
        std::process::exit(0);
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_problem_duals_fof() {
        let tptp = "\
fof(ax1, axiom, human(socrates) => mortal(socrates)).
fof(ax2, axiom, human(socrates)).
fof(goal, conjecture, mortal(socrates)).
";
        let duals = generate_problem_duals(tptp).expect("should parse");
        assert!(duals.has_conjecture);
        let dual = duals.dual_content.expect("dual must exist");
        assert!(dual.contains("axiom"));
        assert!(dual.contains("conjecture"));
        assert!(dual.contains('~'));
        assert!(dual.contains("mortal(socrates)"));

        let ax = duals.axioms_only_content.expect("axioms only must exist");
        assert!(ax.contains("ax1"));
        assert!(ax.contains("ax2"));
        assert!(!ax.contains("goal"));
    }

    #[test]
    fn test_generate_problem_duals_cnf_unit() {
        let tptp = "\
cnf(c1, axiom, p | q).
cnf(c2, negated_conjecture, ~p).
";
        let duals = generate_problem_duals(tptp).expect("should parse");
        assert!(duals.has_conjecture);
        let dual = duals.dual_content.expect("dual must exist");
        assert!(dual.contains("negated_conjecture"));
        assert!(dual.contains("p"));
    }

    #[test]
    fn test_generate_problem_duals_axioms_only() {
        let tptp = "\
include('Axioms/TEST.ax').
cnf(c1, axiom, p).
cnf(c2, axiom, ~p).
";
        let duals = generate_problem_duals(tptp).expect("should parse");
        assert!(!duals.has_conjecture);
        assert!(duals.dual_content.is_none());
        assert!(duals.axioms_only_content.is_none());
        assert!(duals.contradiction_injected_content.contains("$false"));
    }

    #[test]
    fn test_detect_division_schedule() {
        assert_eq!(
            detect_division_schedule(Path::new("problems/casc-30/FNE/CSR026+3.p")),
            "casc_fne"
        );
        assert_eq!(
            detect_division_schedule(Path::new("problems/casc-30/FEQ/AGT005+1.p")),
            "casc_feq"
        );
        assert_eq!(
            detect_division_schedule(Path::new("problems/casc-30/UEQ/ALG212-10.p")),
            "casc_ueq"
        );
        assert_eq!(
            detect_division_schedule(Path::new("problems/casc-30/EPS/HWC004-1.p")),
            "casc_eps"
        );
        assert_eq!(
            detect_division_schedule(Path::new("problems/casc-30/EPU/problem.p")),
            "casc_epu"
        );
        assert_eq!(
            detect_division_schedule(Path::new("problems/casc-30/ICU/problem.p")),
            "casc_icu"
        );
        assert_eq!(
            detect_division_schedule(Path::new("problems/socrates.p")),
            "casc"
        );
    }
}
