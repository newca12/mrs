//! Reproducible scorer for the committed CASC-J13 ProoVer corpus.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use wait_timeout::ChildExt;

#[derive(Clone, Debug)]
struct Row {
    id: String,
    proof: String,
    category: String,
    accepted: String,
    max_score: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VerifyMode {
    Kernel,
    Competition,
}

#[derive(Debug)]
struct Outcome {
    status: String,
    detail: String,
    score: i32,
}

pub const PANEL_REMOVED_PROBLEMS: &[&str] = &[
    "PRV005+1", "PRV006+1", "PRV036+1", "PRV044+1", "PRV057+1", "PRV065+1", "PRV066+1", "PRV079+1",
    "PRV080+1",
];

fn main() {
    let mut args = std::env::args().skip(1);
    let root = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("crates/mrs-bench/proover-corpus/Proover2026"));
    let mut proover = None;
    let mut time = 10u64;
    let mut workers = 1usize;
    let mut mode = VerifyMode::Competition;
    let mut output = None;
    let mut official = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--proover" => proover = args.next().map(PathBuf::from),
            "--time" => time = parse_arg(&mut args, "--time"),
            "--workers" => workers = parse_arg(&mut args, "--workers"),
            "--kernel" => mode = VerifyMode::Kernel,
            "--competition" => mode = VerifyMode::Competition,
            "--output" => output = args.next().map(PathBuf::from),
            "--official" | "--official-91" => official = true,
            other => fail(&format!("unknown argument: {other}")),
        }
    }
    if time == 0 || workers == 0 {
        fail("--time and --workers must be positive");
    }
    let proover = proover.unwrap_or_else(|| PathBuf::from("target/release/mrs-proover"));
    let validation_root = root.join("Problems");
    if !validation_root.is_dir() {
        fail(&format!(
            "missing normalized Problems directory: {}",
            validation_root.display()
        ));
    }
    match score(
        &root,
        &proover,
        time,
        workers,
        mode,
        output.as_deref(),
        official,
    ) {
        Ok(summary) => {
            println!(
                "score={} good={} bad={} unknown={} false_rejection={} unsound={}",
                summary.score,
                summary.good,
                summary.bad,
                summary.unknown,
                summary.false_rejection,
                summary.unsound
            );
            if summary.unsound > 0 {
                std::process::exit(2);
            }
            if summary.false_rejection > 0 {
                std::process::exit(4);
            }
            if summary.unknown > 0 {
                std::process::exit(3);
            }
        }
        Err(error) => fail(&error),
    }
}

#[derive(Default)]
struct Summary {
    score: i32,
    good: usize,
    bad: usize,
    unknown: usize,
    false_rejection: usize,
    unsound: usize,
}

fn score(
    root: &Path,
    proover: &Path,
    time: u64,
    workers: usize,
    mode: VerifyMode,
    output: Option<&Path>,
    official: bool,
) -> Result<Summary, String> {
    let mut rows = read_manifest(&root.join("manifest.tsv"))?;
    if official {
        rows.retain(|row| !PANEL_REMOVED_PROBLEMS.contains(&row.id.as_str()));
    }
    let mut summary = Summary::default();
    let mut report = format!(
        "# ProoVer 2026 deterministic score report\n# mode={mode:?}\tofficial={official}\tproover={}\ttime={}\tworkers={}\n# id\tcategory\taccepted_verdicts\tmax_score\tstatus\tscore\tdetail\n",
        proover.display(),
        time,
        workers
    );
    for row in rows {
        let outcome = run_one(root, proover, &row, time, workers, mode)?;
        summary.score += outcome.score;
        match outcome.status.as_str() {
            "VerifiedGood" => summary.good += 1,
            "VerifiedBad" => summary.bad += 1,
            _ => summary.unknown += 1,
        }
        if row.category == "valid" && outcome.status == "VerifiedBad" {
            summary.false_rejection += 1;
        }
        if row.category == "evil" && outcome.status == "VerifiedGood" {
            summary.unsound += 1;
        }
        let detail = outcome.detail.replace(['\t', '\r', '\n'], " ");
        report.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            row.id,
            row.category,
            row.accepted,
            row.max_score,
            outcome.status,
            outcome.score,
            detail
        ));
    }
    if let Some(path) = output {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!("create report directory {}: {error}", parent.display())
            })?;
        }
        let mut file = fs::File::create(path)
            .map_err(|error| format!("create {}: {error}", path.display()))?;
        file.write_all(report.as_bytes())
            .map_err(|error| format!("write {}: {error}", path.display()))?;
    }
    Ok(summary)
}

fn run_one(
    root: &Path,
    proover: &Path,
    row: &Row,
    time: u64,
    workers: usize,
    mode: VerifyMode,
) -> Result<Outcome, String> {
    let proof = root.join(&row.proof);
    if !proof.is_file() {
        return Err(format!("missing proof file {}", proof.display()));
    }
    let workers_text = workers.to_string();
    let time_text = time.to_string();
    let root_text = root
        .to_str()
        .ok_or_else(|| "non-UTF-8 corpus path".to_string())?
        .to_owned();
    let proof_path = proof
        .to_str()
        .ok_or_else(|| format!("non-UTF-8 proof path for {}", row.id))?
        .to_owned();
    let mut child = Command::new(proover);
    if mode == VerifyMode::Kernel {
        child.arg("--strict");
    }
    let mut child = child
        .args([
            "--workers",
            &workers_text,
            "--time",
            &time_text,
            "--problems-dir",
            &root_text,
        ])
        .arg(&proof_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("spawn {}: {error}", proover.display()))?;
    let status = match child.wait_timeout(Duration::from_secs(time + 5)) {
        Ok(Some(_)) => {
            let output = child
                .wait_with_output()
                .map_err(|error| format!("collect {}: {error}", row.id))?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let status = stdout
                .lines()
                .find(|line| line.contains("% SZS status"))
                .and_then(|line| line.split_whitespace().nth(3))
                .unwrap_or("Unknown")
                .to_owned();
            let detail = stderr.lines().last().unwrap_or_default().to_owned();
            Outcome {
                score: score_outcome(&row.category, &status),
                status,
                detail,
            }
        }
        Ok(None) => {
            child
                .kill()
                .map_err(|error| format!("kill {}: {error}", row.id))?;
            let _ = child.wait();
            Outcome {
                status: "Unknown".into(),
                detail: "verifier timeout".into(),
                score: 0,
            }
        }
        Err(error) => return Err(format!("wait {}: {error}", row.id)),
    };
    Ok(status)
}

fn score_outcome(category: &str, status: &str) -> i32 {
    match (category, status) {
        ("valid", "VerifiedGood") => 1,
        ("valid", "VerifiedBad") => -1,
        ("locally_sound_evil", "VerifiedGood" | "VerifiedBad") => 2,
        ("evil", "VerifiedBad") => 2,
        ("evil", "VerifiedGood") => -10,
        _ => 0,
    }
}

fn read_manifest(path: &Path) -> Result<Vec<Row>, String> {
    let text =
        fs::read_to_string(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let mut rows = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 6 {
            return Err(format!("malformed manifest row: {line}"));
        }
        let row = Row {
            id: fields[0].into(),
            proof: fields[2].into(),
            category: fields[3].into(),
            accepted: fields[4].into(),
            max_score: fields[5]
                .parse()
                .map_err(|_| format!("invalid score: {line}"))?,
        };
        if !matches!(
            (row.category.as_str(), row.accepted.as_str(), row.max_score),
            ("valid", "VerifiedGood", 1)
                | ("evil", "VerifiedBad", 2)
                | ("locally_sound_evil", "VerifiedGood|VerifiedBad", 2)
        ) {
            return Err(format!("invalid scoring classification: {line}"));
        }
        rows.push(row);
    }
    if rows.len() != 100 {
        return Err(format!("expected 100 manifest rows, found {}", rows.len()));
    }
    Ok(rows)
}

fn parse_arg<T: std::str::FromStr>(args: &mut impl Iterator<Item = String>, name: &str) -> T {
    args.next()
        .unwrap_or_else(|| fail(&format!("{name} requires a value")))
        .parse()
        .unwrap_or_else(|_| fail(&format!("invalid value for {name}")))
}

fn fail(message: &str) -> ! {
    eprintln!("score_proover2026: {message}");
    std::process::exit(1)
}
