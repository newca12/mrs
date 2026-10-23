//! Measure one `mrs` base strategy per (problem, strategy) pair and record the
//! outcome matrix the pre-phase study learns from.
//!
//! One row per run:
//!
//! ```text
//! problem,path,strategy,schedule,status,verdict,elapsed_s,wall_s,detail
//! ```
//!
//! * `elapsed_s` is the search's own `elapsed_ms` from the `% SZS detail` line,
//!   which is the quantity a portfolio's time allocation actually competes on.
//! * `verdict` is `refutation` / `saturation` / `unsolved`, collapsing the SZS
//!   vocabulary to the three outcomes that matter for portfolio design. EPS-style
//!   positive answers are `saturation`; everything else that fails is
//!   `unsolved`, and the raw `status` is kept alongside so a run can be audited.
//! * `detail` carries the whole `% SZS detail` payload, so generated/processed/
//!   passive counters come for free and can be joined onto the feature table.
//!
//! The runs are genuinely solo: `--workers 1`, sharing off, one strategy for the
//! whole budget. That matches `systems/mrs-sNN`, so the numbers are comparable
//! with anything `run_strategy_sweep.sh` already produced.
//!
//! ```text
//! prephase_sweep --paths-list sample.txt --strategies 1-15 --time 3 \
//!                --out labels.csv --jobs 2
//! prephase_sweep --features features.csv --sample 40 --seed 7 --time 3 --out labels.csv
//! ```

use std::collections::HashMap;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};

#[derive(Clone)]
struct Config {
    binary: PathBuf,
    time_secs: u64,
    schedule: String,
    tptp: Option<PathBuf>,
    timeout_margin: u64,
}

struct Outcome {
    problem: String,
    path: String,
    strategy: usize,
    schedule: String,
    status: String,
    verdict: String,
    elapsed_s: f64,
    wall_s: f64,
    detail: String,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut paths_list: Option<PathBuf> = None;
    let mut features: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut time_secs = 3u64;
    let mut jobs = num_cpus::get().max(1);
    let mut sample: Option<usize> = None;
    let mut seed = 1u64;
    let mut strategies: Vec<usize> = (1..=15).collect();
    let mut schedule = "casc".to_string();
    let mut limit: Option<usize> = None;
    let mut resume = false;

    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--paths-list" => {
                paths_list = Some(PathBuf::from(args[index + 1].clone()));
                index += 2;
            }
            "--features" => {
                features = Some(PathBuf::from(args[index + 1].clone()));
                index += 2;
            }
            "--out" => {
                out = Some(PathBuf::from(args[index + 1].clone()));
                index += 2;
            }
            "--time" => {
                time_secs = args[index + 1].parse().expect("--time needs seconds");
                index += 2;
            }
            "--jobs" => {
                jobs = args[index + 1].parse().expect("--jobs needs a count");
                index += 2;
            }
            "--sample" => {
                sample = Some(args[index + 1].parse().expect("--sample needs a count"));
                index += 2;
            }
            "--seed" => {
                seed = args[index + 1].parse().expect("--seed needs a number");
                index += 2;
            }
            "--strategies" => {
                strategies = parse_range(&args[index + 1]);
                index += 2;
            }
            // An explicit `--schedule` overrides the per-division default for
            // every problem, which is what a control arm needs.
            "--schedule" => {
                schedule = args[index + 1].clone();
                index += 2;
            }
            "--limit" => {
                limit = Some(args[index + 1].parse().expect("--limit needs a count"));
                index += 2;
            }
            "--resume" => {
                resume = true;
                index += 1;
            }
            "--binary" => {
                // Handled below via an env override.
                index += 2;
            }
            "--help" | "-h" => {
                println!(
                    "Usage: prephase_sweep (--paths-list FILE | --features FILE) --out CSV \\\n\
                     \x20              [--time S] [--jobs N] [--sample N] [--seed N] \\\n\
                     \x20              [--strategies 1-15|1,3,5] [--schedule NAME] [--limit N] [--resume]"
                );
                return;
            }
            other => {
                eprintln!("Unknown argument {other}");
                std::process::exit(1);
            }
        }
    }

    rayon::ThreadPoolBuilder::new()
        .num_threads(jobs)
        .stack_size(mrs_core::RECURSION_STACK_BYTES)
        .build_global()
        .ok();

    let binary = std::env::var("MRS_BINARY")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("target/release/mrs"));
    if !binary.exists() {
        eprintln!("mrs binary not found at {}", binary.display());
        std::process::exit(1);
    }
    let Some(out) = out else {
        eprintln!("--out is required");
        std::process::exit(1);
    };

    let all_paths: Vec<(String, String)> = if let Some(list) = &paths_list {
        read_path_list(list)
    } else if let Some(features) = &features {
        read_feature_paths(features)
    } else {
        eprintln!("one of --paths-list or --features is required");
        std::process::exit(1);
    };
    let mut paths = all_paths;
    if let Some(limit) = limit {
        paths.truncate(limit);
    }

    // Stratified sample: round-robin across parent directories so a division or
    // a source domain cannot dominate by count. The alternative — shuffle and
    // take a prefix — biases towards whichever file walk listed things first.
    if let Some(sample) = sample
        && sample < paths.len()
    {
        paths = stratified_sample(paths, sample, seed);
    }
    paths.sort();
    paths.dedup();

    eprintln!(
        "[sweep] {} problems x {} strategies @ {}s, jobs={}",
        paths.len(),
        strategies.len(),
        time_secs,
        jobs
    );

    let mut done: HashMap<(String, usize), String> = HashMap::new();
    if resume && out.exists() {
        if let Ok(text) = fs::read_to_string(&out) {
            for line in text.lines().skip(1) {
                let fields: Vec<&str> = line.split(',').collect();
                if fields.len() < 4 {
                    continue;
                }
                let Some(strategy) = fields[2].parse::<usize>().ok() else {
                    continue;
                };
                done.insert((fields[0].to_string(), strategy), line.to_string());
            }
        }
        eprintln!("[sweep] resuming, {} runs already recorded", done.len());
    }

    let config = Config {
        binary,
        time_secs,
        schedule,
        tptp: std::env::var("TPTP").ok().map(PathBuf::from),
        timeout_margin: 20,
    };

    let tasks: Vec<(String, String, usize)> = paths
        .par_iter()
        .flat_map(|(problem, path)| {
            strategies
                .iter()
                .map(move |strategy| (problem.clone(), path.clone(), *strategy))
                .collect::<Vec<_>>()
        })
        .filter(|task| !done.contains_key(&(task.0.clone(), task.2)))
        .collect();
    eprintln!("[sweep] {} runs to execute", tasks.len());

    let collected: Mutex<Vec<Outcome>> = Mutex::new(Vec::new());
    let completed = Mutex::new(0usize);

    tasks.par_iter().for_each(|task| {
        let outcome = run_one(&config, &task.0, &task.1, task.2);
        collected.lock().unwrap().push(outcome);
        let mut done_count = completed.lock().unwrap();
        *done_count += 1;
        if (*done_count).is_multiple_of(50) {
            eprintln!("[sweep] {done_count}/{} runs", tasks.len());
        }
    });

    let mut rows = collected.into_inner().unwrap();
    rows.sort_by(|a, b| a.problem.cmp(&b.problem).then(a.strategy.cmp(&b.strategy)));

    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).ok();
    }
    let mut file = fs::File::create(&out).expect("cannot create output");
    let mut sink = BufWriter::new(&mut file);
    if !resume {
        writeln!(
            sink,
            "problem,path,strategy,schedule,status,verdict,elapsed_s,wall_s,detail"
        )
        .unwrap();
    }
    for row in &rows {
        writeln!(
            sink,
            "{},{},{},{},{},{},{:.3},{:.3},{}",
            row.problem,
            row.path,
            row.strategy,
            row.schedule,
            row.status,
            row.verdict,
            row.elapsed_s,
            row.wall_s,
            sanitize(&row.detail)
        )
        .unwrap();
    }
    // Resumed runs go back in after the new ones so the file stays complete.
    for line in done.values() {
        writeln!(sink, "{line}").unwrap();
    }
    sink.flush().unwrap();
    eprintln!("[sweep] wrote {} new rows to {}", rows.len(), out.display());
}

fn sanitize(text: &str) -> String {
    text.replace(',', ";").replace('\n', " ")
}

fn parse_range(spec: &str) -> Vec<usize> {
    let mut out = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((lo, hi)) = part.split_once('-') {
            let lo: usize = lo.parse().expect("bad strategy range");
            let hi: usize = hi.parse().expect("bad strategy range");
            out.extend(lo..=hi);
        } else {
            out.push(part.parse().expect("bad strategy id"));
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn read_path_list(path: &Path) -> Vec<(String, String)> {
    let text = fs::read_to_string(path).expect("cannot read --paths-list");
    text.lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let name = Path::new(line)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();
            (name, line.to_string())
        })
        .collect()
}

/// Read `path` column 0 out of a `prephase_dump` CSV.
fn read_feature_paths(path: &Path) -> Vec<(String, String)> {
    let text = fs::read_to_string(path).expect("cannot read --features");
    let mut out = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if index == 0 || line.is_empty() {
            continue;
        }
        let Some(path) = line.split(',').next() else {
            continue;
        };
        if path.is_empty() {
            continue;
        }
        let name = Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        out.push((name, path.to_string()));
    }
    out
}

/// Take `count` paths, round-robin over their parent directories.
///
/// Problems arrive grouped by division (`casc-30/FEQ/...`) and by TPTP source
/// domain (`Problems/GRP/GRS/...`), and those groups differ in size by orders of
/// magnitude. Round-robin keeps the sample's division mix proportional to what
/// exists without letting the walk order decide.
fn stratified_sample(
    paths: Vec<(String, String)>,
    count: usize,
    seed: u64,
) -> Vec<(String, String)> {
    use std::collections::HashMap;
    let mut buckets: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for entry in paths {
        let key = Path::new(&entry.1)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        buckets.entry(key).or_default().push(entry);
    }
    // Deterministic bucket order so the same seed reproduces the same sample.
    let mut keys: Vec<String> = buckets.keys().cloned().collect();
    keys.sort();
    for key in keys.iter_mut() {
        let mut bucket = buckets.remove(key).expect("bucket present");
        // Rotate by the seed so a rerun with a different seed is a genuinely
        // different sample rather than a prefix of the same ordering.
        if !bucket.is_empty() {
            let shift = (seed as usize) % bucket.len();
            bucket.rotate_left(shift);
        }
        buckets.insert(key.clone(), bucket);
    }
    let mut out = Vec::with_capacity(count);
    let mut round = 0usize;
    while out.len() < count {
        let mut progressed = false;
        for key in &keys {
            if let Some(bucket) = buckets.get_mut(key)
                && round < bucket.len()
            {
                out.push(bucket[round].clone());
                progressed = true;
                if out.len() == count {
                    break;
                }
            }
        }
        if !progressed {
            break;
        }
        round += 1;
    }
    out
}

/// The CASC division of a problem is its parent directory, which is exactly how
/// `casc.sh` and `systems/mrs-sNN` read it. An explicit `--schedule` is folded in
/// by the caller through `config.schedule` being the fallback.
fn schedule_for(config: &Config, path: &str) -> String {
    let division = Path::new(path)
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match division.as_str() {
        "feq" => "casc_feq".to_string(),
        "fne" => "casc_fne".to_string(),
        "ueq" => "casc_ueq".to_string(),
        "eps" => "casc_eps".to_string(),
        "epu" => "casc_epu".to_string(),
        "icu" => "casc_icu".to_string(),
        "epr" => "casc_epr".to_string(),
        _ => config.schedule.clone(),
    }
}

fn run_one(config: &Config, problem: &str, path: &str, strategy: usize) -> Outcome {
    let schedule = schedule_for(config, path);
    let mut command = Command::new(&config.binary);
    command
        .arg("--time")
        .arg(config.time_secs.to_string())
        .arg("--workers")
        .arg("1")
        .arg("--schedule")
        .arg(&schedule)
        .arg("--strategy")
        .arg(strategy.to_string())
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Solo means solo: no cross-strategy sharing, and no ambient override
        // that would make this run unlike `systems/mrs-sNN`.
        .env("MRS_SHARED_POOL_INTERVAL", "0")
        .env("MRS_SINGLE_STRATEGY", "");
    if let Some(tptp) = &config.tptp {
        command.env("TPTP", tptp);
    }
    // Proof output is not part of the label and dominates stdout volume.
    command.env("RUST_MIN_STACK", "67108864");

    let started = Instant::now();
    let status;
    let mut elapsed_s = f64::NAN;
    let mut detail = String::new();
    let limit = Duration::from_secs(config.time_secs + config.timeout_margin);

    match spawn_with_timeout(&mut command, limit) {
        Ok(Some((code, stdout, stderr))) => {
            status = Some(parse_status(&stdout).unwrap_or_else(|| format!("exit{code}")));
            if let Some(text) = stderr
                .lines()
                .find_map(|line| line.strip_prefix("% SZS detail "))
            {
                detail = text.to_string();
                if let Some(ms) = field(&detail, "elapsed_ms").and_then(|v| v.parse::<f64>().ok()) {
                    elapsed_s = ms / 1000.0;
                }
            }
            if elapsed_s.is_nan() {
                elapsed_s = started.elapsed().as_secs_f64();
            }
        }
        Ok(None) => {
            status = Some("hard_timeout".to_string());
            elapsed_s = started.elapsed().as_secs_f64();
        }
        Err(error) => {
            status = Some(format!("spawn_error:{error}"));
            elapsed_s = started.elapsed().as_secs_f64();
        }
    }
    let status = status.unwrap_or_else(|| "no_status".to_string());

    let verdict = match status.as_str() {
        "Theorem" | "Unsatisfiable" => "refutation",
        "Satisfiable" | "CounterSatisfiable" => "saturation",
        _ => "unsolved",
    };
    Outcome {
        problem: problem.to_string(),
        path: path.to_string(),
        strategy,
        schedule,
        status,
        verdict: verdict.to_string(),
        elapsed_s,
        wall_s: started.elapsed().as_secs_f64(),
        detail,
    }
}

fn spawn_with_timeout(
    command: &mut Command,
    limit: Duration,
) -> std::io::Result<Option<(i32, String, String)>> {
    let mut child = command.spawn()?;
    let deadline = Instant::now() + limit;
    loop {
        match child.try_wait()? {
            Some(_) => break,
            None => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Ok(None);
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
    let output = child.wait_with_output()?;
    Ok(Some((
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )))
}

fn parse_status(stdout: &str) -> Option<String> {
    stdout.lines().find_map(|line| {
        line.strip_prefix("% SZS status ")
            .map(|rest| rest.split_whitespace().next().unwrap_or("").to_string())
            .filter(|value| !value.is_empty())
    })
}

fn field(detail: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=");
    let start = detail.find(&needle)? + needle.len();
    let rest = &detail[start..];
    let end = rest.find(|c: char| c.is_whitespace()).unwrap_or(rest.len());
    Some(rest[..end].to_string())
}
