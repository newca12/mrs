//! Deterministic Rust implementation of the MRS independent-proof audit.

use rayon::prelude::*;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tempfile::NamedTempFile;
use wait_timeout::ChildExt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VerifyMode {
    Kernel,
    Competition,
}

#[derive(Clone, Debug)]
struct Args {
    list: PathBuf,
    tptp: PathBuf,
    mrs: PathBuf,
    proover: PathBuf,
    timeout: u64,
    workers: usize,
    jobs: usize,
    mode: VerifyMode,
    output: PathBuf,
    raw_dir: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Class {
    CertifiedGood,
    ConfirmedBad,
    UnknownUnverified,
    MrsTimeout,
    ParseError,
    VerifierTimeout,
    InfrastructureFailure,
    NonRefutation,
}

impl Class {
    fn as_str(self) -> &'static str {
        match self {
            Self::CertifiedGood => "certified_good",
            Self::ConfirmedBad => "confirmed_bad",
            Self::UnknownUnverified => "unknown_unverified",
            Self::MrsTimeout => "mrs_timeout",
            Self::ParseError => "parse_error",
            Self::VerifierTimeout => "verifier_timeout",
            Self::InfrastructureFailure => "infrastructure_failure",
            Self::NonRefutation => "non_refutation",
        }
    }
}

#[derive(Debug)]
struct Record {
    problem: String,
    mrs_status: String,
    mrs_time_s: f64,
    verifier_status: String,
    verifier_time_s: f64,
    class: Class,
    detail: String,
}

fn main() {
    let args = match parse_args(std::env::args().skip(1).collect()) {
        Ok(args) => args,
        Err(error) => fail(&error),
    };
    for (name, path, directory) in [
        ("list", &args.list, false),
        ("tptp", &args.tptp, true),
        ("mrs", &args.mrs, false),
        ("proover", &args.proover, false),
    ] {
        if (directory && !path.is_dir()) || (!directory && !path.is_file()) {
            fail(&format!("{name} path does not exist: {}", path.display()));
        }
    }
    if let Some(raw_dir) = &args.raw_dir {
        fs::create_dir_all(raw_dir)
            .unwrap_or_else(|error| fail(&format!("create raw directory: {error}")));
    }
    let problems = read_list(&args.list).unwrap_or_else(|error| fail(&error));
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(args.jobs)
        .build()
        .unwrap_or_else(|error| fail(&format!("create audit pool: {error}")));
    let mut records = pool.install(|| {
        problems
            .par_iter()
            .map(|problem| audit_one(problem, &args))
            .collect::<Vec<_>>()
    });
    records.sort_by(|left, right| left.problem.cmp(&right.problem));
    write_report(&args.output, &records).unwrap_or_else(|error| fail(&error));
    print_summary(&records, args.mode);

    if records
        .iter()
        .any(|record| record.class == Class::InfrastructureFailure)
    {
        std::process::exit(1);
    }
    if records
        .iter()
        .any(|record| record.class == Class::ConfirmedBad)
    {
        std::process::exit(2);
    }
    if records
        .iter()
        .any(|record| record.class == Class::ParseError)
    {
        std::process::exit(4);
    }
    if records.iter().any(|record| {
        matches!(
            record.class,
            Class::UnknownUnverified | Class::MrsTimeout | Class::VerifierTimeout
        )
    }) {
        std::process::exit(3);
    }
}

fn parse_args(args: Vec<String>) -> Result<Args, String> {
    let mut list = None;
    let mut tptp = std::env::var_os("TPTP").map(PathBuf::from);
    let mut mrs = PathBuf::from("target/release/mrs");
    let mut proover = PathBuf::from("target/release/mrs-proover");
    let mut timeout = 30;
    let mut workers = 8;
    let mut jobs = 1;
    let mut mode = VerifyMode::Competition;
    let mut output = None;
    let mut raw_dir = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        let next = |iter: &mut std::vec::IntoIter<String>, name: &str| {
            iter.next()
                .ok_or_else(|| format!("{name} requires a value"))
        };
        match arg.as_str() {
            "--list" => list = Some(PathBuf::from(next(&mut iter, "--list")?)),
            "--tptp" => tptp = Some(PathBuf::from(next(&mut iter, "--tptp")?)),
            "--mrs" => mrs = PathBuf::from(next(&mut iter, "--mrs")?),
            "--proover" => proover = PathBuf::from(next(&mut iter, "--proover")?),
            "--timeout" => timeout = parse_positive(&next(&mut iter, "--timeout")?, "--timeout")?,
            "--workers" => workers = parse_positive(&next(&mut iter, "--workers")?, "--workers")?,
            "--jobs" => jobs = parse_positive(&next(&mut iter, "--jobs")?, "--jobs")?,
            "--kernel" => mode = VerifyMode::Kernel,
            "--competition" => mode = VerifyMode::Competition,
            "--output" => output = Some(PathBuf::from(next(&mut iter, "--output")?)),
            "--raw-dir" => raw_dir = Some(PathBuf::from(next(&mut iter, "--raw-dir")?)),
            "--help" | "-h" => {
                println!(
                    "audit_proover --list LIST --tptp ROOT --output REPORT.csv [--kernel|--competition] [--mrs PATH] [--proover PATH] [--timeout SECS] [--workers N] [--jobs N] [--raw-dir DIR]"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(Args {
        list: list.ok_or_else(|| "--list is required".to_string())?,
        tptp: tptp.ok_or_else(|| "--tptp or TPTP is required".to_string())?,
        mrs,
        proover,
        timeout,
        workers: usize::try_from(workers).map_err(|_| "--workers is too large")?,
        jobs: usize::try_from(jobs).map_err(|_| "--jobs is too large")?,
        mode,
        output: output.ok_or_else(|| "--output is required".to_string())?,
        raw_dir,
    })
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

fn audit_one(problem: &str, args: &Args) -> Record {
    let path = args.tptp.join(problem);
    if !path.is_file() {
        return record(
            problem,
            "Missing",
            0.0,
            "NotRun",
            0.0,
            Class::InfrastructureFailure,
            "problem file missing",
        );
    }
    let start = Instant::now();
    let workers = args.workers.to_string();
    let timeout = args.timeout.to_string();
    let path_text = path.to_string_lossy().into_owned();
    let child = Command::new(&args.mrs)
        .args([
            "--schedule",
            "casc",
            "--workers",
            &workers,
            "--time",
            &timeout,
            &path_text,
        ])
        .env("TPTP", &args.tptp)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(error) => {
            return record(
                problem,
                "SpawnError",
                start.elapsed().as_secs_f64(),
                "NotRun",
                0.0,
                Class::InfrastructureFailure,
                &format!("spawn MRS: {error}"),
            );
        }
    };
    let output = match child.wait_timeout(Duration::from_secs(args.timeout + 5)) {
        Ok(Some(_)) => child.wait_with_output().ok(),
        Ok(None) => {
            let _ = child.kill();
            let _ = child.wait();
            return record(
                problem,
                "Timeout",
                start.elapsed().as_secs_f64(),
                "NotRun",
                0.0,
                Class::MrsTimeout,
                "MRS exceeded wall-clock limit",
            );
        }
        Err(_) => None,
    };
    let Some(output) = output else {
        return record(
            problem,
            "Unknown",
            start.elapsed().as_secs_f64(),
            "NotRun",
            0.0,
            Class::InfrastructureFailure,
            "could not collect MRS output",
        );
    };
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    save_raw(args.raw_dir.as_deref(), problem, "mrs.stdout", &stdout);
    save_raw(args.raw_dir.as_deref(), problem, "mrs.stderr", &stderr);
    let mrs_time = start.elapsed().as_secs_f64();
    let Some(mrs_status) = szs_status(&stdout).or_else(|| szs_status(&stderr)) else {
        let class = if stderr.to_ascii_lowercase().contains("parse") {
            Class::ParseError
        } else if output.status.success() {
            Class::UnknownUnverified
        } else {
            Class::InfrastructureFailure
        };
        return record(
            problem,
            "NoSzsStatus",
            mrs_time,
            "NotRun",
            0.0,
            class,
            stderr.lines().last().unwrap_or("MRS emitted no SZS status"),
        );
    };
    if !matches!(mrs_status.as_str(), "Theorem" | "Unsatisfiable") {
        return record(
            problem,
            &mrs_status,
            mrs_time,
            "NotRun",
            0.0,
            Class::NonRefutation,
            "MRS did not emit a refutation status",
        );
    }
    let mut proof = match NamedTempFile::new() {
        Ok(file) => file,
        Err(error) => {
            return record(
                problem,
                &mrs_status,
                mrs_time,
                "NotRun",
                0.0,
                Class::InfrastructureFailure,
                &format!("create proof temporary file: {error}"),
            );
        }
    };
    if let Err(error) = proof.write_all(stdout.as_bytes()) {
        return record(
            problem,
            &mrs_status,
            mrs_time,
            "NotRun",
            0.0,
            Class::InfrastructureFailure,
            &format!("write proof temporary file: {error}"),
        );
    }
    let verifier_start = Instant::now();
    let verifier_time = args.timeout.clamp(1, 10).to_string();
    let root_text = args.tptp.to_string_lossy().into_owned();
    let proof_text = proof.path().to_string_lossy().into_owned();
    let mut verifier_args = Vec::new();
    if args.mode == VerifyMode::Kernel {
        verifier_args.push("--strict".to_owned());
    }
    verifier_args.extend([
        "--workers".into(),
        "1".into(),
        "--time".into(),
        verifier_time,
        "--problems-dir".into(),
        root_text,
        proof_text,
    ]);
    let mut verifier = match Command::new(&args.proover)
        .args(verifier_args.drain(..))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            return record(
                problem,
                &mrs_status,
                mrs_time,
                "SpawnError",
                verifier_start.elapsed().as_secs_f64(),
                Class::InfrastructureFailure,
                &format!("spawn verifier: {error}"),
            );
        }
    };
    let verifier_output =
        match verifier.wait_timeout(Duration::from_secs(args.timeout.clamp(1, 10) + 5)) {
            Ok(Some(_)) => verifier.wait_with_output().ok(),
            Ok(None) => {
                let _ = verifier.kill();
                let _ = verifier.wait();
                return record(
                    problem,
                    &mrs_status,
                    mrs_time,
                    "Timeout",
                    verifier_start.elapsed().as_secs_f64(),
                    Class::VerifierTimeout,
                    "verifier exceeded wall-clock limit",
                );
            }
            Err(_) => None,
        };
    let Some(verifier_output) = verifier_output else {
        return record(
            problem,
            &mrs_status,
            mrs_time,
            "Unknown",
            verifier_start.elapsed().as_secs_f64(),
            Class::InfrastructureFailure,
            "could not collect verifier output",
        );
    };
    let verifier_stdout = String::from_utf8_lossy(&verifier_output.stdout).into_owned();
    let verifier_stderr = String::from_utf8_lossy(&verifier_output.stderr).into_owned();
    save_raw(
        args.raw_dir.as_deref(),
        problem,
        "verifier.stdout",
        &verifier_stdout,
    );
    save_raw(
        args.raw_dir.as_deref(),
        problem,
        "verifier.stderr",
        &verifier_stderr,
    );
    let verifier_status = szs_status(&verifier_stdout).unwrap_or_else(|| "Unknown".into());
    let class = match verifier_status.as_str() {
        "VerifiedGood" => Class::CertifiedGood,
        "VerifiedBad" => Class::ConfirmedBad,
        _ => Class::UnknownUnverified,
    };
    record(
        problem,
        &mrs_status,
        mrs_time,
        &verifier_status,
        verifier_start.elapsed().as_secs_f64(),
        class,
        verifier_stderr.lines().last().unwrap_or_default(),
    )
}

fn record(
    problem: &str,
    mrs_status: &str,
    mrs_time_s: f64,
    verifier_status: &str,
    verifier_time_s: f64,
    class: Class,
    detail: &str,
) -> Record {
    Record {
        problem: problem.into(),
        mrs_status: mrs_status.into(),
        mrs_time_s,
        verifier_status: verifier_status.into(),
        verifier_time_s,
        class,
        detail: detail.into(),
    }
}

fn read_list(path: &Path) -> Result<Vec<String>, String> {
    let text = fs::read_to_string(path).map_err(|error| format!("read list: {error}"))?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect())
}

fn write_report(path: &Path, records: &[Record]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create report directory {}: {error}", parent.display()))?;
    }
    let mut output = String::from(
        "problem,mrs_status,mrs_time_s,verifier_status,verifier_time_s,class,detail\n",
    );
    for record in records {
        let detail = record.detail.replace([',', '\r', '\n'], ";");
        output.push_str(&format!(
            "{},{},{:.3},{},{:.3},{},{}\n",
            record.problem,
            record.mrs_status,
            record.mrs_time_s,
            record.verifier_status,
            record.verifier_time_s,
            record.class.as_str(),
            detail
        ));
    }
    fs::write(path, output).map_err(|error| format!("write report {}: {error}", path.display()))
}

fn print_summary(records: &[Record], mode: VerifyMode) {
    println!("verification_mode={mode:?}");
    for class in [
        Class::CertifiedGood,
        Class::ConfirmedBad,
        Class::UnknownUnverified,
        Class::MrsTimeout,
        Class::ParseError,
        Class::VerifierTimeout,
        Class::InfrastructureFailure,
        Class::NonRefutation,
    ] {
        println!(
            "{}={}",
            class.as_str(),
            records
                .iter()
                .filter(|record| record.class == class)
                .count()
        );
    }
}

fn szs_status(output: &str) -> Option<String> {
    output
        .lines()
        .find(|line| line.contains("% SZS status"))
        .and_then(|line| line.split_whitespace().nth(3))
        .map(str::to_owned)
}

fn save_raw(dir: Option<&Path>, problem: &str, suffix: &str, content: &str) {
    let Some(dir) = dir else { return };
    let safe = problem.replace(['/', '\\'], "_");
    let _ = fs::write(dir.join(format!("{safe}.{suffix}")), content);
}

fn fail(message: &str) -> ! {
    eprintln!("audit_proover: {message}");
    std::process::exit(1)
}
