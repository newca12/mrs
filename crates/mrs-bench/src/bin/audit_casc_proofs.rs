//! Replay archived CASC prover output through independently selectable proof
//! verification policies. This tool never invokes MRS: it consumes the raw
//! stdout archived by `casc.sh` and runs one or more checks on that artifact.

use rayon::prelude::*;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use wait_timeout::ChildExt;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Check {
    Strict,
    Mrs,
    Ladder,
}

impl Check {
    fn as_str(self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::Mrs => "mrs",
            Self::Ladder => "ladder",
        }
    }
}

#[derive(Clone, Debug)]
struct Checks {
    strict: bool,
    mrs: bool,
    ladder: bool,
}

impl Checks {
    fn all() -> Self {
        Self {
            strict: true,
            mrs: true,
            ladder: true,
        }
    }

    fn contains(&self, check: Check) -> bool {
        match check {
            Check::Strict => self.strict,
            Check::Mrs => self.mrs,
            Check::Ladder => self.ladder,
        }
    }

    fn names(&self) -> String {
        [Check::Strict, Check::Mrs, Check::Ladder]
            .into_iter()
            .filter(|check| self.contains(*check))
            .map(Check::as_str)
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[derive(Clone, Debug)]
struct Args {
    run_csv: PathBuf,
    problems_dir: PathBuf,
    output: PathBuf,
    proover: PathBuf,
    eprover: Option<PathBuf>,
    vampire: Option<PathBuf>,
    checks: Checks,
    strict_time: u64,
    mrs_time: u64,
    ladder_time: u64,
    mrs_workers: usize,
    ladder_workers: usize,
    jobs: usize,
    force: bool,
}

#[derive(Clone, Debug)]
struct RunRow {
    edition: String,
    division: String,
    problem: String,
    system: String,
    timeout: u64,
    generation_status: String,
    generation_detail: String,
    raw_stdout_path: PathBuf,
    raw_stderr_path: PathBuf,
    expected_stdout_sha256: String,
    expected_stderr_sha256: String,
}

#[derive(Clone, Debug, Default)]
struct CheckResult {
    status: String,
    time_s: f64,
    detail: String,
}

impl CheckResult {
    fn not_run() -> Self {
        Self {
            status: "not_run".to_string(),
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug)]
struct AuditRow {
    edition: String,
    division: String,
    problem: String,
    system: String,
    timeout: u64,
    raw_stdout_path: String,
    raw_stderr_path: String,
    raw_stdout_sha256: String,
    raw_stderr_sha256: String,
    proof_path: String,
    proof_sha256: String,
    generation_status: String,
    generation_detail: String,
    strict: CheckResult,
    mrs: CheckResult,
    ladder: CheckResult,
    checks: String,
    audit_time_s: f64,
}

impl AuditRow {
    fn key(&self) -> String {
        row_key(&self.edition, &self.division, &self.problem, &self.system)
    }

    fn from_run(run: &RunRow) -> Self {
        Self {
            edition: run.edition.clone(),
            division: run.division.clone(),
            problem: run.problem.clone(),
            system: run.system.clone(),
            timeout: run.timeout,
            raw_stdout_path: absolute_path(&run.raw_stdout_path)
                .to_string_lossy()
                .into_owned(),
            raw_stderr_path: absolute_path(&run.raw_stderr_path)
                .to_string_lossy()
                .into_owned(),
            raw_stdout_sha256: String::new(),
            raw_stderr_sha256: String::new(),
            proof_path: String::new(),
            proof_sha256: String::new(),
            generation_status: run.generation_status.clone(),
            generation_detail: run.generation_detail.clone(),
            strict: CheckResult::not_run(),
            mrs: CheckResult::not_run(),
            ladder: CheckResult::not_run(),
            checks: String::new(),
            audit_time_s: 0.0,
        }
    }
}

fn main() {
    let args = match parse_args(std::env::args().skip(1).collect()) {
        Ok(args) => args,
        Err(error) => fail(&error),
    };

    if !args.run_csv.is_file() {
        fail(&format!(
            "run CSV does not exist: {}",
            args.run_csv.display()
        ));
    }
    if !args.problems_dir.is_dir() {
        fail(&format!(
            "problems directory does not exist: {}",
            args.problems_dir.display()
        ));
    }
    if !args.proover.is_file() {
        fail(&format!(
            "mrs-proover binary does not exist: {}",
            args.proover.display()
        ));
    }

    let runs = load_run_csv(&args.run_csv).unwrap_or_else(|error| fail(&error));
    if runs.is_empty() {
        fail("run CSV contains no benchmark rows");
    }

    fs::create_dir_all(&args.output)
        .unwrap_or_else(|error| fail(&format!("create output directory: {error}")));
    let report_path = args.output.join("audit.csv");
    let previous = if report_path.is_file() {
        load_audit_csv(&report_path).unwrap_or_else(|error| fail(&error))
    } else {
        HashMap::new()
    };

    let jobs = runs
        .iter()
        .map(|run| {
            let mut row = AuditRow::from_run(run);
            if let Some(old) = previous.get(&row.key()) {
                // The raw-artifact hash check below decides whether these
                // cached check results are still usable.
                row.strict = old.strict.clone();
                row.mrs = old.mrs.clone();
                row.ladder = old.ladder.clone();
                row.proof_path = old.proof_path.clone();
                row.proof_sha256 = old.proof_sha256.clone();
                row.raw_stdout_sha256 = old.raw_stdout_sha256.clone();
                row.raw_stderr_sha256 = old.raw_stderr_sha256.clone();
                row.checks = old.checks.clone();
            }
            (run.clone(), row)
        })
        .collect::<Vec<_>>();

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(args.jobs)
        .build()
        .unwrap_or_else(|error| fail(&format!("create audit thread pool: {error}")));
    let mut rows = pool.install(|| {
        jobs.par_iter()
            .map(|(run, row)| audit_one(run, row.clone(), &args))
            .collect::<Vec<_>>()
    });
    rows.sort_by_key(AuditRow::key);

    write_audit_csv(&report_path, &rows).unwrap_or_else(|error| fail(&error));
    let summary_path = args.output.join("audit-summary.txt");
    let summary = render_summary(&rows, &args.checks, &report_path, &summary_path);
    fs::write(&summary_path, &summary)
        .unwrap_or_else(|error| fail(&format!("write {}: {error}", summary_path.display())));
    print!("{summary}");
}

fn parse_args(args: Vec<String>) -> Result<Args, String> {
    let mut run = None;
    let mut problems_dir = None;
    let mut output = None;
    let mut proover = None;
    let mut eprover = None;
    let mut vampire = None;
    let mut checks = Checks::all();
    let mut strict_time = 30;
    let mut mrs_time = 10;
    let mut ladder_time = 30;
    let mut mrs_workers = 1;
    let mut ladder_workers = 8;
    let mut jobs = 1;
    let mut force = false;
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        let next = |iter: &mut std::vec::IntoIter<String>, name: &str| {
            iter.next()
                .ok_or_else(|| format!("{name} requires a value"))
        };
        match arg.as_str() {
            "--run" => run = Some(PathBuf::from(next(&mut iter, "--run")?)),
            "--problems-dir" => {
                problems_dir = Some(PathBuf::from(next(&mut iter, "--problems-dir")?))
            }
            "--output" => output = Some(PathBuf::from(next(&mut iter, "--output")?)),
            "--proover" => proover = Some(PathBuf::from(next(&mut iter, "--proover")?)),
            "--eprover" => eprover = Some(PathBuf::from(next(&mut iter, "--eprover")?)),
            "--vampire" => vampire = Some(PathBuf::from(next(&mut iter, "--vampire")?)),
            "--checks" => checks = parse_checks(&next(&mut iter, "--checks")?)?,
            "--strict-time" => {
                strict_time = parse_positive(&next(&mut iter, "--strict-time")?, "--strict-time")?
            }
            "--mrs-time" => {
                mrs_time = parse_positive(&next(&mut iter, "--mrs-time")?, "--mrs-time")?
            }
            "--ladder-time" => {
                ladder_time = parse_positive(&next(&mut iter, "--ladder-time")?, "--ladder-time")?
            }
            "--mrs-workers" => {
                mrs_workers =
                    parse_usize_positive(&next(&mut iter, "--mrs-workers")?, "--mrs-workers")?
            }
            "--ladder-workers" => {
                ladder_workers =
                    parse_usize_positive(&next(&mut iter, "--ladder-workers")?, "--ladder-workers")?
            }
            "--jobs" => jobs = parse_usize_positive(&next(&mut iter, "--jobs")?, "--jobs")?,
            "--force" => force = true,
            "--help" | "-h" => {
                println!(
                    "audit_casc_proofs --run RUN_DIR|run.csv --problems-dir DIR [options]\n\
                     options: --output DIR --checks strict,mrs,ladder --strict-time SEC\n\
                              --mrs-time SEC --ladder-time SEC --mrs-workers N\n\
                              --ladder-workers N --jobs N --proover PATH\n\
                              --eprover PATH --vampire PATH --force"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    let run_path = run.ok_or_else(|| "--run is required".to_string())?;
    let run_dir = if run_path.is_dir() {
        run_path.clone()
    } else {
        run_path.parent().unwrap_or(Path::new(".")).to_path_buf()
    };
    let run_csv = if run_path.is_dir() {
        run_path.join("run.csv")
    } else {
        run_path
    };
    let output = output.unwrap_or_else(|| run_dir.join("proof-audit"));
    let default_proover = current_exe_sibling("mrs-proover");
    let root = workspace_root();
    let default_eprover = root.join("crates/mrs-bench/systems/eprover/bin/eprover");
    let default_vampire = root.join("crates/mrs-bench/systems/vampire/bin/vampire");

    Ok(Args {
        run_csv,
        problems_dir: problems_dir.ok_or_else(|| "--problems-dir is required".to_string())?,
        output,
        proover: proover.unwrap_or(default_proover),
        eprover: eprover.or_else(|| default_eprover.is_file().then_some(default_eprover)),
        vampire: vampire.or_else(|| default_vampire.is_file().then_some(default_vampire)),
        checks,
        strict_time,
        mrs_time,
        ladder_time,
        mrs_workers,
        ladder_workers,
        jobs,
        force,
    })
}

fn parse_checks(value: &str) -> Result<Checks, String> {
    let mut checks = Checks {
        strict: false,
        mrs: false,
        ladder: false,
    };
    for name in value
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        match name.to_ascii_lowercase().as_str() {
            "strict" | "kernel" => checks.strict = true,
            "mrs" | "mrs-only" => checks.mrs = true,
            "ladder" | "competition" => checks.ladder = true,
            other => return Err(format!("unknown check `{other}`; use strict,mrs,ladder")),
        }
    }
    if !checks.strict && !checks.mrs && !checks.ladder {
        return Err("--checks must select at least one check".to_string());
    }
    Ok(checks)
}

fn parse_positive(value: &str, name: &str) -> Result<u64, String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| format!("invalid value for {name}"))?;
    if parsed == 0 {
        return Err(format!("{name} must be positive"));
    }
    Ok(parsed)
}

fn parse_usize_positive(value: &str, name: &str) -> Result<usize, String> {
    let parsed = parse_positive(value, name)?;
    usize::try_from(parsed).map_err(|_| format!("{name} is too large"))
}

fn load_run_csv(path: &Path) -> Result<Vec<RunRow>, String> {
    let text =
        fs::read_to_string(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let mut lines = text.lines();
    let header = lines
        .next()
        .ok_or_else(|| format!("run CSV is empty: {}", path.display()))?;
    let headers = parse_csv_line(header);
    let col = |name: &str| {
        headers
            .iter()
            .position(|header| header == name)
            .ok_or_else(|| format!("run CSV is missing `{name}`"))
    };
    let edition = col("edition")?;
    let division = col("division")?;
    let problem = col("problem")?;
    let system = col("system")?;
    let status = col("szs_status")?;
    let detail = headers.iter().position(|header| header == "failure_detail");
    let timeout = headers.iter().position(|header| header == "timeout");
    let stdout_path = headers
        .iter()
        .position(|header| header == "raw_stdout_path");
    let stderr_path = headers
        .iter()
        .position(|header| header == "raw_stderr_path");
    let stdout_hash = headers
        .iter()
        .position(|header| header == "raw_stdout_sha256");
    let stderr_hash = headers
        .iter()
        .position(|header| header == "raw_stderr_sha256");
    let run_dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();

    let mut rows = Vec::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let fields = parse_csv_line(line);
        let get = |index: usize| fields.get(index).cloned().unwrap_or_default();
        let edition_value = get(edition);
        let division_value = get(division);
        let problem_value = get(problem);
        let system_value = get(system);
        let stdout_rel = stdout_path
            .and_then(|index| fields.get(index).cloned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| {
                format!("raw/{system_value}/{division_value}/{problem_value}.stdout")
            });
        let stderr_rel = stderr_path
            .and_then(|index| fields.get(index).cloned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| {
                format!("raw/{system_value}/{division_value}/{problem_value}.stderr")
            });
        let timeout_value = timeout
            .and_then(|index| fields.get(index))
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);
        rows.push(RunRow {
            edition: edition_value,
            division: division_value,
            problem: problem_value,
            system: system_value,
            timeout: timeout_value,
            generation_status: get(status),
            generation_detail: detail.map(get).unwrap_or_default(),
            raw_stdout_path: resolve_path(&run_dir, Path::new(&stdout_rel)),
            raw_stderr_path: resolve_path(&run_dir, Path::new(&stderr_rel)),
            expected_stdout_sha256: stdout_hash.map(get).unwrap_or_default(),
            expected_stderr_sha256: stderr_hash.map(get).unwrap_or_default(),
        });
    }
    Ok(rows)
}

fn audit_one(run: &RunRow, mut row: AuditRow, args: &Args) -> AuditRow {
    let start = Instant::now();
    let stdout_path = &run.raw_stdout_path;
    let stderr_path = &run.raw_stderr_path;
    let raw_stdout = match fs::read(stdout_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            mark_selected_error(
                &mut row,
                &args.checks,
                "artifact_error",
                &format!("cannot read raw stdout: {error}"),
            );
            row.audit_time_s = start.elapsed().as_secs_f64();
            return row;
        }
    };
    let raw_stderr = fs::read(stderr_path).unwrap_or_default();
    let stdout_hash = sha256_bytes(&raw_stdout);
    let stderr_hash = sha256_bytes(&raw_stderr);
    row.raw_stdout_path = absolute_path(stdout_path).to_string_lossy().into_owned();
    row.raw_stderr_path = absolute_path(stderr_path).to_string_lossy().into_owned();
    row.raw_stdout_sha256 = stdout_hash.clone();
    row.raw_stderr_sha256 = stderr_hash.clone();

    if (!run.expected_stdout_sha256.is_empty() && run.expected_stdout_sha256 != stdout_hash)
        || (!run.expected_stderr_sha256.is_empty() && run.expected_stderr_sha256 != stderr_hash)
    {
        mark_selected_error(
            &mut row,
            &args.checks,
            "artifact_mismatch",
            "raw artifact hash does not match run.csv",
        );
        row.audit_time_s = start.elapsed().as_secs_f64();
        return row;
    }

    if (!row.raw_stdout_sha256.is_empty() && row.raw_stdout_sha256 != stdout_hash)
        || (!row.raw_stderr_sha256.is_empty() && row.raw_stderr_sha256 != stderr_hash)
    {
        row.strict = CheckResult::not_run();
        row.mrs = CheckResult::not_run();
        row.ladder = CheckResult::not_run();
        row.proof_path.clear();
        row.proof_sha256.clear();
    }

    let stdout_text = String::from_utf8_lossy(&raw_stdout).into_owned();
    let status = extract_szs_status(&stdout_text).unwrap_or_else(|| run.generation_status.clone());
    row.generation_status = status.clone();
    if status != "Theorem" && status != "Unsatisfiable" {
        if (status == "Satisfiable" || status == "CounterSatisfiable")
            && let Ok(cert) =
                mrs_proof_kernel::model::ModelCertificate::extract_from_text(&stdout_text)
            && let Some(problem_path) =
                find_problem(&args.problems_dir, &run.division, &run.problem)
            && let Ok(problem_text) = fs::read_to_string(&problem_path)
            && let Ok(problem) = mrs_tptp::parse_tptp(&problem_text)
        {
            let verdict = cert.validate(&problem, Some(&status));
            match verdict {
                mrs_proof_kernel::model::ModelVerdict::Certified {
                    domain_size,
                    digest,
                    ..
                } => {
                    let res = CheckResult {
                        status: "VerifiedGood".to_string(),
                        time_s: start.elapsed().as_secs_f64(),
                        detail: format!("certified_model:domain={domain_size},digest={digest}"),
                    };
                    row.strict = res.clone();
                    row.mrs = res.clone();
                    row.ladder = res;
                    row.checks = available_check_names(&row);
                    row.audit_time_s = start.elapsed().as_secs_f64();
                    return row;
                }
                mrs_proof_kernel::model::ModelVerdict::Rejected(reason) => {
                    mark_selected_error(&mut row, &args.checks, "invalid_model", &reason);
                    row.audit_time_s = start.elapsed().as_secs_f64();
                    return row;
                }
                mrs_proof_kernel::model::ModelVerdict::Inconclusive(reason) => {
                    mark_selected_error(&mut row, &args.checks, "inconclusive_model", &reason);
                    row.audit_time_s = start.elapsed().as_secs_f64();
                    return row;
                }
            }
        }
        mark_selected_error(
            &mut row,
            &args.checks,
            "non_refutation",
            "archived MRS output is not a refutation",
        );
        row.audit_time_s = start.elapsed().as_secs_f64();
        return row;
    }
    if !stdout_text.contains("$false") {
        mark_selected_error(
            &mut row,
            &args.checks,
            "no_proof",
            "refutation status has no $false proof root",
        );
        row.audit_time_s = start.elapsed().as_secs_f64();
        return row;
    }

    let problem_path = find_problem(&args.problems_dir, &run.division, &run.problem);
    let Some(problem_path) = problem_path else {
        mark_selected_error(
            &mut row,
            &args.checks,
            "problem_missing",
            "matching CASC problem file was not found",
        );
        row.audit_time_s = start.elapsed().as_secs_f64();
        return row;
    };
    let proof_dir = args
        .output
        .join("proofs")
        .join(safe_component(&run.system))
        .join(safe_component(&run.division));
    let proof_path = proof_dir.join(format!("{}.s", safe_component(&run.problem)));
    let proof_problem_dir = proof_dir.join("Problems");
    if let Err(error) = fs::create_dir_all(&proof_problem_dir) {
        mark_selected_error(
            &mut row,
            &args.checks,
            "artifact_error",
            &format!("create proof directory: {error}"),
        );
        row.audit_time_s = start.elapsed().as_secs_f64();
        return row;
    }
    let problem_filename = problem_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(&run.problem);
    let linked_problem = proof_problem_dir.join(problem_filename);
    if let Err(error) = fs::copy(&problem_path, &linked_problem) {
        mark_selected_error(
            &mut row,
            &args.checks,
            "artifact_error",
            &format!("copy linked problem: {error}"),
        );
        row.audit_time_s = start.elapsed().as_secs_f64();
        return row;
    }

    let proof_text = normalize_proof(&stdout_text, problem_filename);
    let proof_bytes = proof_text.as_bytes();
    let proof_hash = sha256_bytes(proof_bytes);
    let reusable = !args.force
        && row.proof_path == absolute_path(&proof_path).to_string_lossy()
        && row.proof_sha256 == proof_hash
        && proof_path.is_file();
    if !reusable && let Err(error) = fs::write(&proof_path, proof_bytes) {
        mark_selected_error(
            &mut row,
            &args.checks,
            "artifact_error",
            &format!("write normalized proof: {error}"),
        );
        row.audit_time_s = start.elapsed().as_secs_f64();
        return row;
    }
    row.proof_path = absolute_path(&proof_path).to_string_lossy().into_owned();
    row.proof_sha256 = proof_hash;

    let check_output_dir = args.output.join("checks");
    if let Err(error) = fs::create_dir_all(&check_output_dir) {
        mark_selected_error(
            &mut row,
            &args.checks,
            "artifact_error",
            &format!("create check output directory: {error}"),
        );
        row.audit_time_s = start.elapsed().as_secs_f64();
        return row;
    }
    for check in [Check::Strict, Check::Mrs, Check::Ladder] {
        if !args.checks.contains(check) {
            continue;
        }
        if !args.force && !check_result_is_pending(check_result(&row, check)) {
            continue;
        }
        let result = run_check(check, &proof_path, &proof_dir, &check_output_dir, args);
        *check_result_mut(&mut row, check) = result;
    }
    row.checks = available_check_names(&row);
    row.audit_time_s = start.elapsed().as_secs_f64();
    row
}

fn run_check(
    check: Check,
    proof_path: &Path,
    proof_dir: &Path,
    output_dir: &Path,
    args: &Args,
) -> CheckResult {
    let (limit, workers) = match check {
        Check::Strict => (args.strict_time, 1),
        Check::Mrs => (args.mrs_time, args.mrs_workers),
        Check::Ladder => (args.ladder_time, args.ladder_workers),
    };
    let mut command = Command::new(&args.proover);
    command.env("TPTP", &args.problems_dir);
    command.arg("--problems-dir").arg(proof_dir);
    command.arg("--workers").arg(workers.to_string());
    command.arg("--time").arg(limit.to_string());
    match check {
        Check::Strict => {
            command.arg("--strict");
        }
        Check::Mrs => {
            command.arg("--only-mrs");
        }
        Check::Ladder => {
            if let Some(path) = &args.eprover {
                command.arg("--eprover").arg(path);
            }
            if let Some(path) = &args.vampire {
                command.arg("--vampire").arg(path);
            }
        }
    }
    command.arg(proof_path);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let start = Instant::now();
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return CheckResult {
                status: "infrastructure_error".to_string(),
                time_s: start.elapsed().as_secs_f64(),
                detail: format!("spawn verifier: {error}"),
            };
        }
    };
    let output = match child.wait_timeout(Duration::from_secs(limit.saturating_add(5))) {
        Ok(Some(_)) => child.wait_with_output(),
        Ok(None) => {
            let _ = child.kill();
            let _ = child.wait();
            return CheckResult {
                status: "Timeout".to_string(),
                time_s: start.elapsed().as_secs_f64(),
                detail: format!("verifier exceeded {limit}s wall-clock limit"),
            };
        }
        Err(error) => Err(error),
    };
    let elapsed = start.elapsed().as_secs_f64();
    let output = match output {
        Ok(output) => output,
        Err(error) => {
            return CheckResult {
                status: "infrastructure_error".to_string(),
                time_s: elapsed,
                detail: format!("collect verifier output: {error}"),
            };
        }
    };
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let prefix = format!(
        "{}-{}",
        safe_component(proof_path.to_string_lossy().as_ref()),
        check.as_str()
    );
    let _ = fs::write(output_dir.join(format!("{prefix}.stdout")), &stdout);
    let _ = fs::write(output_dir.join(format!("{prefix}.stderr")), &stderr);
    let status = extract_szs_status(&stdout)
        .or_else(|| extract_szs_status(&stderr))
        .unwrap_or_else(|| "Unknown".to_string());
    let detail = extract_status_detail(&stdout)
        .or_else(|| extract_status_detail(&stderr))
        .or_else(|| {
            stderr
                .lines()
                .rev()
                .find(|line| !line.trim().is_empty())
                .map(str::to_string)
        })
        .unwrap_or_default();
    CheckResult {
        status,
        time_s: elapsed,
        detail,
    }
}

fn mark_selected_error(row: &mut AuditRow, checks: &Checks, status: &str, detail: &str) {
    for check in [Check::Strict, Check::Mrs, Check::Ladder] {
        if checks.contains(check) {
            *check_result_mut(row, check) = CheckResult {
                status: status.to_string(),
                time_s: 0.0,
                detail: detail.to_string(),
            };
        }
    }
    row.checks = available_check_names(row);
}

fn available_check_names(row: &AuditRow) -> String {
    [
        (Check::Strict, &row.strict),
        (Check::Mrs, &row.mrs),
        (Check::Ladder, &row.ladder),
    ]
    .into_iter()
    .filter(|(_, result)| !check_result_is_pending(result))
    .map(|(check, _)| check.as_str())
    .collect::<Vec<_>>()
    .join(",")
}

fn check_result(row: &AuditRow, check: Check) -> &CheckResult {
    match check {
        Check::Strict => &row.strict,
        Check::Mrs => &row.mrs,
        Check::Ladder => &row.ladder,
    }
}

fn check_result_mut(row: &mut AuditRow, check: Check) -> &mut CheckResult {
    match check {
        Check::Strict => &mut row.strict,
        Check::Mrs => &mut row.mrs,
        Check::Ladder => &mut row.ladder,
    }
}

fn check_result_is_pending(result: &CheckResult) -> bool {
    result.status.is_empty() || result.status == "not_run"
}

fn find_problem(root: &Path, division: &str, problem: &str) -> Option<PathBuf> {
    let filename = if problem.ends_with(".p") {
        problem.to_string()
    } else {
        format!("{problem}.p")
    };
    [
        root.join(division).join(&filename),
        root.join(division.to_ascii_uppercase()).join(&filename),
        root.join(division.to_ascii_lowercase()).join(&filename),
        root.join("Problems").join(division).join(&filename),
        root.join("Problems")
            .join(division.to_ascii_uppercase())
            .join(&filename),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

fn normalize_proof(text: &str, problem_filename: &str) -> String {
    let source = format!("Problems/{problem_filename}");
    let mut output = String::with_capacity(text.len());
    for original_line in text.lines() {
        let mut line = original_line.to_string();
        if line.trim_start().starts_with("% Proof :") {
            line = format!("% Proof : {source}");
        }
        let mut search_from = 0;
        while let Some(relative) = line[search_from..].find("file('") {
            let start = search_from + relative + "file('".len();
            let Some(end_relative) = line[start..].find("',") else {
                break;
            };
            let end = start + end_relative;
            line.replace_range(start..end, &source);
            search_from = start + source.len();
        }
        output.push_str(&line);
        output.push('\n');
    }
    output
}

fn load_audit_csv(path: &Path) -> Result<HashMap<String, AuditRow>, String> {
    let text =
        fs::read_to_string(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let mut lines = text.lines();
    let header = lines
        .next()
        .ok_or_else(|| format!("audit CSV is empty: {}", path.display()))?;
    let headers = parse_csv_line(header);
    let col = |name: &str| headers.iter().position(|header| header == name);
    let required = |name: &str| col(name).ok_or_else(|| format!("audit CSV is missing `{name}`"));
    let edition = required("edition")?;
    let division = required("division")?;
    let problem = required("problem")?;
    let system = required("system")?;
    let mut records = HashMap::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let fields = parse_csv_line(line);
        let get = |name: &str| {
            col(name)
                .and_then(|index| fields.get(index).cloned())
                .unwrap_or_default()
        };
        let row = AuditRow {
            edition: get_at(&fields, edition),
            division: get_at(&fields, division),
            problem: get_at(&fields, problem),
            system: get_at(&fields, system),
            timeout: get("timeout").parse().unwrap_or(0),
            raw_stdout_path: get("raw_stdout_path"),
            raw_stderr_path: get("raw_stderr_path"),
            raw_stdout_sha256: get("raw_stdout_sha256"),
            raw_stderr_sha256: get("raw_stderr_sha256"),
            proof_path: get("proof_path"),
            proof_sha256: get("proof_sha256"),
            generation_status: get("generation_status"),
            generation_detail: get("generation_detail"),
            strict: read_check_result(&get, "strict"),
            mrs: read_check_result(&get, "mrs"),
            ladder: read_check_result(&get, "ladder"),
            checks: get("checks"),
            audit_time_s: get("audit_time_s").parse().unwrap_or(0.0),
        };
        records.insert(row.key(), row);
    }
    Ok(records)
}

fn read_check_result(get: &impl Fn(&str) -> String, name: &str) -> CheckResult {
    CheckResult {
        status: get(&format!("{name}_status")),
        time_s: get(&format!("{name}_time_s")).parse().unwrap_or(0.0),
        detail: get(&format!("{name}_detail")),
    }
}

fn write_audit_csv(path: &Path, rows: &[AuditRow]) -> Result<(), String> {
    let mut output = String::from(
        "edition,division,problem,system,timeout,raw_stdout_path,raw_stderr_path,raw_stdout_sha256,raw_stderr_sha256,proof_path,proof_sha256,generation_status,generation_detail,strict_status,strict_time_s,strict_detail,mrs_status,mrs_time_s,mrs_detail,ladder_status,ladder_time_s,ladder_detail,checks,audit_time_s\n",
    );
    for row in rows {
        let values = [
            row.edition.clone(),
            row.division.clone(),
            row.problem.clone(),
            row.system.clone(),
            row.timeout.to_string(),
            row.raw_stdout_path.clone(),
            row.raw_stderr_path.clone(),
            row.raw_stdout_sha256.clone(),
            row.raw_stderr_sha256.clone(),
            row.proof_path.clone(),
            row.proof_sha256.clone(),
            row.generation_status.clone(),
            row.generation_detail.clone(),
            row.strict.status.clone(),
            format!("{:.6}", row.strict.time_s),
            row.strict.detail.clone(),
            row.mrs.status.clone(),
            format!("{:.6}", row.mrs.time_s),
            row.mrs.detail.clone(),
            row.ladder.status.clone(),
            format!("{:.6}", row.ladder.time_s),
            row.ladder.detail.clone(),
            row.checks.clone(),
            format!("{:.6}", row.audit_time_s),
        ];
        output.push_str(
            &values
                .iter()
                .map(|value| csv_escape(value))
                .collect::<Vec<_>>()
                .join(","),
        );
        output.push('\n');
    }
    fs::write(path, output).map_err(|error| format!("write {}: {error}", path.display()))
}

fn render_summary(
    rows: &[AuditRow],
    checks: &Checks,
    report: &Path,
    summary_path: &Path,
) -> String {
    let mut divisions = rows
        .iter()
        .map(|row| row.division.clone())
        .collect::<Vec<_>>();
    divisions.sort();
    divisions.dedup();

    let mut output = String::new();
    output.push_str(&format!("audit_report={}\n", report.display()));
    output.push_str(&format!("summary_report={}\n", summary_path.display()));
    output.push_str(&format!("checks=[{}]\n", checks.names()));

    for division in divisions {
        let division_rows = rows
            .iter()
            .filter(|row| row.division == division)
            .collect::<Vec<_>>();
        output.push('\n');
        output.push_str(&format!("{}\n", "=".repeat(80)));
        output.push_str(&format!(
            "Division: {division:<60} Rows: {}\n",
            division_rows.len()
        ));
        output.push_str(&format!("{}\n\n", "=".repeat(80)));

        output.push_str("Generation\n");
        output.push_str(&format_generation_table(&division_rows));
        output.push('\n');

        output.push_str("Verification (Refutations)\n");
        output.push_str(&format_verification_table(&division_rows, checks));

        let has_models = division_rows
            .iter()
            .any(|row| generation_scope(&row.generation_status) == GenerationScope::Model);
        if has_models {
            output.push('\n');
            output.push_str("Verification (Model Certificates)\n");
            output.push_str(&format_model_verification_table(&division_rows));
        }
    }
    output
}

fn format_model_verification_table(rows: &[&AuditRow]) -> String {
    const HEADERS: [&str; 4] = [
        "Total Models",
        "Certified Models",
        "Invalid Models",
        "Uncertified (N/A: Model)",
    ];
    let model_rows: Vec<_> = rows
        .iter()
        .filter(|row| generation_scope(&row.generation_status) == GenerationScope::Model)
        .collect();
    let total = model_rows.len();
    let certified = model_rows
        .iter()
        .filter(|row| row.strict.status == "VerifiedGood")
        .count();
    let invalid = model_rows
        .iter()
        .filter(|row| row.strict.status == "VerifiedBad")
        .count();
    let uncertified = total.saturating_sub(certified + invalid);

    let table_rows = vec![vec![
        total.to_string(),
        certified.to_string(),
        invalid.to_string(),
        uncertified.to_string(),
    ]];
    ascii_table(&HEADERS, &table_rows)
}

fn format_generation_table(rows: &[&AuditRow]) -> String {
    const HEADERS: [&str; 2] = ["Status", "Count"];
    let statuses = [
        "Theorem",
        "Unsatisfiable",
        "Satisfiable",
        "CounterSatisfiable",
        "GaveUp",
        "Timeout",
        "Error",
        "Other",
    ];
    let mut counts = HashMap::<&str, usize>::new();
    for row in rows {
        *counts.entry(row.generation_status.as_str()).or_default() += 1;
    }
    let mut output = String::new();
    output.push_str(&ascii_table(
        &HEADERS,
        &statuses
            .iter()
            .map(|status| {
                let count = counts.get(status).copied().unwrap_or(0);
                vec![status.to_string(), count.to_string()]
            })
            .collect::<Vec<_>>(),
    ));
    output
}

fn format_verification_table(rows: &[&AuditRow], checks: &Checks) -> String {
    const HEADERS: [&str; 10] = [
        "Mode",
        "Applicable",
        "VerifiedGood",
        "VerifiedBad",
        "Unknown",
        "Timeout",
        "N/A: Model",
        "N/A: Incomplete",
        "Error",
        "Other",
    ];
    let mut table_rows = Vec::new();
    for check in [Check::Strict, Check::Mrs, Check::Ladder] {
        if !checks.contains(check) {
            continue;
        }
        let mut counts = HashMap::<&str, usize>::new();
        let mut applicable = 0;
        for row in rows {
            let scope = generation_scope(&row.generation_status);
            let value = check_result_for(row, check).status.as_str();
            if scope == GenerationScope::Refutation {
                applicable += 1;
                *counts.entry(value).or_default() += 1;
            } else if scope == GenerationScope::Model {
                *counts.entry("N/A: Model").or_default() += 1;
            } else if scope == GenerationScope::Incomplete {
                *counts.entry("N/A: Incomplete").or_default() += 1;
            } else {
                *counts.entry("Error").or_default() += 1;
            }
        }
        table_rows.push(vec![
            check.as_str().to_string(),
            applicable.to_string(),
            count_string(&counts, "VerifiedGood"),
            count_string(&counts, "VerifiedBad"),
            count_string(&counts, "Unknown"),
            count_string(&counts, "Timeout"),
            count_string(&counts, "N/A: Model"),
            count_string(&counts, "N/A: Incomplete"),
            count_string(&counts, "Error"),
            count_string(&counts, "Other"),
        ]);
    }
    ascii_table(&HEADERS, &table_rows)
}

fn check_result_for(row: &AuditRow, check: Check) -> &CheckResult {
    match check {
        Check::Strict => &row.strict,
        Check::Mrs => &row.mrs,
        Check::Ladder => &row.ladder,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GenerationScope {
    Refutation,
    Model,
    Incomplete,
    Error,
}

fn generation_scope(status: &str) -> GenerationScope {
    match status {
        "Theorem" | "Unsatisfiable" => GenerationScope::Refutation,
        "Satisfiable" | "CounterSatisfiable" => GenerationScope::Model,
        "GaveUp" | "Timeout" | "ResourceOut" | "Unknown" => GenerationScope::Incomplete,
        _ => GenerationScope::Error,
    }
}

fn count_string(counts: &HashMap<&str, usize>, status: &str) -> String {
    counts.get(status).copied().unwrap_or(0).to_string()
}

fn ascii_table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut widths = headers
        .iter()
        .map(|header| header.len())
        .collect::<Vec<_>>();
    for row in rows {
        for (index, value) in row.iter().enumerate() {
            widths[index] = widths[index].max(value.len());
        }
    }
    let separator = format!(
        "+{}+\n",
        widths
            .iter()
            .map(|width| "-".repeat(width + 2))
            .collect::<Vec<_>>()
            .join("+")
    );
    let format_row = |values: &[String]| {
        format!(
            "|{}|\n",
            values
                .iter()
                .enumerate()
                .map(|(index, value)| format!(" {:>width$} ", value, width = widths[index]))
                .collect::<Vec<_>>()
                .join("|")
        )
    };
    let mut output = separator.clone();
    output.push_str(&format_row(
        &headers
            .iter()
            .map(|header| (*header).to_string())
            .collect::<Vec<_>>(),
    ));
    output.push_str(&separator);
    for row in rows {
        output.push_str(&format_row(row));
    }
    output.push_str(&separator);
    output
}

fn row_key(edition: &str, division: &str, problem: &str, system: &str) -> String {
    format!("{edition}\t{division}\t{problem}\t{system}")
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut chars = line.trim_end_matches('\r').chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '"' if quoted && chars.peek() == Some(&'"') => {
                chars.next();
                current.push('"');
            }
            '"' => quoted = !quoted,
            ',' if !quoted => {
                fields.push(current.trim().to_string());
                current.clear();
            }
            other => current.push(other),
        }
    }
    fields.push(current.trim().to_string());
    fields
}

fn csv_escape(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn get_at(fields: &[String], index: usize) -> String {
    fields.get(index).cloned().unwrap_or_default()
}

fn extract_szs_status(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        fields
            .iter()
            .position(|field| *field == "status")
            .and_then(|index| fields.get(index + 1))
            .map(|status| status.to_string())
    })
}

fn extract_status_detail(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        line.find(": ")
            .map(|index| line[index + 2..].trim().to_string())
    })
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}

fn resolve_path(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

fn absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

fn safe_component(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '+' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn current_exe_sibling(name: &str) -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join(name)))
        .unwrap_or_else(|| PathBuf::from(format!("target/release/{name}")))
}

fn workspace_root() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.parent()
                .and_then(Path::parent)
                .and_then(Path::parent)
                .map(Path::to_path_buf)
        })
        .unwrap_or_else(|| PathBuf::from("."))
}

fn fail(message: &str) -> ! {
    eprintln!("audit_casc_proofs: {message}");
    std::process::exit(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_selected_checks() {
        let checks = parse_checks("strict,ladder").unwrap();
        assert!(checks.strict);
        assert!(!checks.mrs);
        assert!(checks.ladder);
        assert_eq!(checks.names(), "strict,ladder");
    }

    #[test]
    fn normalizes_proof_provenance_without_changing_derivation() {
        let input = "% Proof : /tmp/problem.p\ncnf(a, axiom, p, file('/tmp/problem.p', a)).\n";
        let output = normalize_proof(input, "TEST.p");
        assert!(output.contains("% Proof : Problems/TEST.p"));
        assert!(output.contains("file('Problems/TEST.p', a)"));
        assert!(!output.contains("/tmp/problem.p"));
    }

    #[test]
    fn csv_round_trip_preserves_quoted_fields() {
        let line = ["a", "detail, with \"quotes\"", "c"]
            .into_iter()
            .map(csv_escape)
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(
            parse_csv_line(&line),
            vec!["a", "detail, with \"quotes\"", "c"]
        );
    }

    #[test]
    fn summary_separates_model_rows_from_refutation_checks() {
        let row = |status: &str| AuditRow {
            edition: "casc-30".to_string(),
            division: "EPS".to_string(),
            problem: "P.p".to_string(),
            system: "mrs".to_string(),
            timeout: 120,
            raw_stdout_path: String::new(),
            raw_stderr_path: String::new(),
            raw_stdout_sha256: String::new(),
            raw_stderr_sha256: String::new(),
            proof_path: String::new(),
            proof_sha256: String::new(),
            generation_status: status.to_string(),
            generation_detail: String::new(),
            strict: CheckResult::not_run(),
            mrs: CheckResult::not_run(),
            ladder: CheckResult::not_run(),
            checks: String::new(),
            audit_time_s: 0.0,
        };
        let rows = vec![row("Satisfiable"), row("GaveUp"), row("Timeout")];
        let summary = render_summary(
            &rows,
            &Checks::all(),
            Path::new("audit.csv"),
            Path::new("audit-summary.txt"),
        );
        assert!(summary.contains("Division: EPS"));
        assert!(summary.contains("Satisfiable"));
        assert!(summary.contains("strict"));
        assert!(summary.contains("N/A: Model"));
        assert!(summary.contains("N/A: Incomplete"));
        assert_eq!(generation_scope("Satisfiable"), GenerationScope::Model);
    }
}
