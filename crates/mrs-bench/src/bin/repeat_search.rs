//! Repeat one MRS search and emit machine-readable scheduler telemetry.
//!
//! This is intentionally a subprocess harness: each run gets a fresh process,
//! allocator state, thread pool, and wall-clock origin. It is useful for
//! comparing verdict/counter variance under different worker and scheduler
//! policies without involving the benchmark database.

use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use wait_timeout::ChildExt;

const FIELDS: &[&str] = &[
    "strategies",
    "workers",
    "result",
    "elapsed_ms",
    "timeout",
    "saturated",
    "processed",
    "generated",
    "passive",
    "weight_discarded",
    "lrs_discarded",
    "fwd_subsumed",
    "shared_published",
    "shared_imported",
];

struct Args {
    problem: PathBuf,
    runs: usize,
    time: u64,
    workers: usize,
    schedule: Option<String>,
    lrs_fixed: Option<u64>,
    shared_interval: Option<u64>,
    mrs: PathBuf,
}

fn main() -> io::Result<()> {
    let args = parse_args()?;
    println!("run,status,{},exit_code", FIELDS.join(","));
    for run in 1..=args.runs {
        let (status, detail, exit_code) = run_once(&args)?;
        let values = parse_detail(&detail);
        let fields = FIELDS
            .iter()
            .map(|field| csv_escape(values.get(*field).map(String::as_str).unwrap_or("")))
            .collect::<Vec<_>>();
        println!(
            "{run},{},{},{}",
            csv_escape(&status),
            fields.join(","),
            exit_code
        );
    }
    Ok(())
}

fn parse_args() -> io::Result<Args> {
    let mut args = std::env::args().skip(1);
    let mut problem = None;
    let mut runs = 5;
    let mut time = 10;
    let mut workers = 1;
    let mut schedule = None;
    let mut lrs_fixed = None;
    let mut shared_interval = None;
    let mut mrs = PathBuf::from("target/release/mrs");

    while let Some(arg) = args.next() {
        let value = |name: &str, args: &mut std::iter::Skip<std::env::Args>| {
            args.next().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("{name} requires a value"),
                )
            })
        };
        match arg.as_str() {
            "--problem" => problem = Some(PathBuf::from(value("--problem", &mut args)?)),
            "--runs" => runs = parse_positive(&value("--runs", &mut args)?, "--runs")?,
            "--time" => time = parse_positive(&value("--time", &mut args)?, "--time")? as u64,
            "--workers" => workers = parse_positive(&value("--workers", &mut args)?, "--workers")?,
            "--schedule" => schedule = Some(value("--schedule", &mut args)?),
            "--lrs-fixed" => {
                lrs_fixed = Some(parse_nonnegative(
                    &value("--lrs-fixed", &mut args)?,
                    "--lrs-fixed",
                )?)
            }
            "--shared-pool-interval" => {
                shared_interval = Some(parse_nonnegative(
                    &value("--shared-pool-interval", &mut args)?,
                    "--shared-pool-interval",
                )?)
            }
            "--mrs" => mrs = PathBuf::from(value("--mrs", &mut args)?),
            "-h" | "--help" => {
                println!(
                    "repeat_search --problem FILE [--runs N] [--time SECS] [--workers N] \
                     [--schedule NAME] [--lrs-fixed N] [--shared-pool-interval N] [--mrs PATH]"
                );
                std::process::exit(0);
            }
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown argument: {other}"),
                ));
            }
        }
    }

    let problem = problem
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "--problem is required"))?;
    if !problem.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("problem file not found: {}", problem.display()),
        ));
    }
    if !mrs.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("mrs binary not found: {}", mrs.display()),
        ));
    }

    Ok(Args {
        problem,
        runs,
        time,
        workers,
        schedule,
        lrs_fixed,
        shared_interval,
        mrs,
    })
}

fn parse_positive(value: &str, name: &str) -> io::Result<usize> {
    let parsed = value.parse::<usize>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid {name}: {value}"),
        )
    })?;
    if parsed == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} must be positive"),
        ));
    }
    Ok(parsed)
}

fn parse_nonnegative(value: &str, name: &str) -> io::Result<u64> {
    value.parse::<u64>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid {name}: {value}"),
        )
    })
}

fn run_once(args: &Args) -> io::Result<(String, String, i32)> {
    let mut command = Command::new(&args.mrs);
    command
        .arg("--time")
        .arg(args.time.to_string())
        .arg("--workers")
        .arg(args.workers.to_string());
    if let Some(schedule) = &args.schedule {
        command.arg("--schedule").arg(schedule);
    }
    command
        .arg(&args.problem)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(budget) = args.lrs_fixed {
        command.env("MRS_LRS_FIXED_ITERATIONS", budget.to_string());
    }
    if let Some(interval) = args.shared_interval {
        command.env("MRS_SHARED_POOL_INTERVAL", interval.to_string());
    }

    let mut child = command.spawn()?;
    let timeout = Duration::from_secs(args.time.saturating_add(5));
    let exit_status = match child.wait_timeout(timeout)? {
        Some(status) => status,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(("Timeout".into(), "result=Timeout".into(), 124));
        }
    };
    let output = child.wait_with_output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let status = stdout
        .lines()
        .find_map(|line| line.strip_prefix("% SZS status "))
        .and_then(|line| line.split_whitespace().next())
        .unwrap_or("Unknown")
        .to_string();
    let detail = stderr
        .lines()
        .find_map(|line| line.strip_prefix("% SZS detail "))
        .unwrap_or("result=Unknown")
        .to_string();
    let code = status_code(&status, exit_status.success());
    Ok((status, detail, code))
}

fn status_code(status: &str, success: bool) -> i32 {
    if success {
        0
    } else if status == "Timeout" {
        124
    } else {
        1
    }
}

fn csv_escape(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn parse_detail(detail: &str) -> BTreeMap<String, String> {
    detail
        .split_whitespace()
        .filter_map(|field| field.split_once('='))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}
