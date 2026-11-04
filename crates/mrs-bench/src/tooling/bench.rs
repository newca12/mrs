use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use clap::Args;
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use regex::Regex;
use rusqlite::{Connection, OpenFlags};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use wait_timeout::ChildExt;

type ToolResult<T> = Result<T, Box<dyn Error>>;

#[derive(Args, Debug)]
pub struct PairedCampaignArgs {
    #[arg(long)]
    baseline: PathBuf,
    #[arg(long)]
    candidate: PathBuf,
    #[arg(long)]
    problems_dir: PathBuf,
    #[arg(long, value_parser = ["casc-30", "casc-j13"])]
    edition: String,
    #[arg(long, default_value = "fne,feq,ueq")]
    divisions: String,
    #[arg(long)]
    schedule: Option<String>,
    #[arg(long, default_value_t = 2)]
    reps: usize,
    #[arg(long, default_value_t = 42)]
    seed: u64,
    #[arg(long, default_value_t = 0)]
    sharing_interval: u64,
    #[arg(long, default_value_t = 120)]
    strict_time: u64,
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    auditor: Option<PathBuf>,
    #[arg(long)]
    proover: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct FixedWorkArgs {
    #[arg(long)]
    baseline: PathBuf,
    #[arg(long)]
    candidate: PathBuf,
    #[arg(long)]
    problem: PathBuf,
    #[arg(long, default_value = "casc_ueq")]
    schedule: String,
    #[arg(long, default_value_t = 4, value_parser = clap::value_parser!(u8).range(1..=15))]
    strategy: u8,
    #[arg(long, default_value_t = 1000)]
    processed: u64,
    #[arg(long, default_value_t = 120)]
    guard_seconds: u64,
    #[arg(long, default_value_t = 5)]
    reps: usize,
    #[arg(long, default_value_t = 42)]
    seed: u64,
    #[arg(long, default_value_t = false)]
    candidate_specialize: bool,
    #[arg(long, default_value_t = false)]
    candidate_compact: bool,
    #[arg(long)]
    output: PathBuf,
}

#[derive(Args, Debug)]
pub struct DashboardArgs {
    #[arg(required = true)]
    snapshots: Vec<PathBuf>,
}

#[derive(Args, Debug)]
pub struct CertificationSummaryArgs {
    /// Strict audit CSV produced by audit_casc_proofs.
    report: PathBuf,
    /// Run metadata, used to recover the resolved subset if one was recorded.
    run_meta: PathBuf,
}

#[derive(Args, Debug)]
pub struct CountVerifiedArgs {
    audit_csv: PathBuf,
}

#[derive(Args, Debug)]
pub struct RecordBinaryHashArgs {
    metadata: PathBuf,
    sha256: String,
}

#[derive(Args, Debug)]
pub struct ReadBinaryHashArgs {
    metadata: PathBuf,
}

#[derive(Args, Debug)]
pub struct EprTelemetryArgs {
    run_csv: PathBuf,
    output_dir: PathBuf,
}

#[derive(Args, Debug)]
pub struct PrephaseAbArgs {
    output_dir: PathBuf,
}

#[derive(Args, Debug)]
pub struct VarianceArgs {
    parent: PathBuf,
    reps: usize,
}

#[derive(Args, Debug)]
pub struct RunCsvSummaryArgs {
    run_csv: PathBuf,
    #[arg(long)]
    division: Option<String>,
}

#[derive(Args, Debug)]
pub struct FailureProfileArgs {
    run_csv: PathBuf,
    #[arg(long)]
    division: Option<String>,
}

#[derive(Args, Debug)]
pub struct ListMissingArgs {
    csv: PathBuf,
    #[arg(long, default_value = "failure_detail")]
    column: String,
    #[arg(long, default_value = "problem")]
    print_column: String,
}

#[derive(Args, Debug)]
pub struct CodexOverlapArgs {
    database: PathBuf,
    run_csv: PathBuf,
    #[arg(long, default_value = "%j13%")]
    corpus_like: String,
    #[arg(long, default_value = "UEQ")]
    division: String,
    #[arg(long, default_value = "Unsatisfiable")]
    codex_status: String,
    #[arg(long, default_value = "ok")]
    run_verdict: String,
}

#[derive(Args, Debug)]
pub struct CanaryDriftArgs {
    run_csv: PathBuf,
}

#[derive(Debug)]
pub struct CanaryContaminated;

impl std::fmt::Display for CanaryContaminated {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("canary audit found include-drift contamination")
    }
}

impl Error for CanaryContaminated {}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("mrs-bench is two levels below the workspace root")
        .to_path_buf()
}

fn absolute(path: &Path) -> ToolResult<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn create_output_directory(path: &Path) -> ToolResult<()> {
    if path.exists() {
        return Err(format!(
            "refusing to overwrite existing output directory {}",
            path.display()
        )
        .into());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(path)?;
    Ok(())
}

pub fn sha256_file(path: &Path) -> ToolResult<String> {
    let bytes = fs::read(path)?;
    Ok(hex_digest(&bytes))
}

fn hex_digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn verify_executable(path: &Path, label: &str) -> ToolResult<PathBuf> {
    let path = absolute(path)?;
    if !path.is_file() {
        return Err(format!("missing {label}: {}", path.display()).into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if fs::metadata(&path)?.permissions().mode() & 0o111 == 0 {
            return Err(format!("not executable ({label}): {}", path.display()).into());
        }
    }
    Ok(path)
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AuditKey {
    pub edition: String,
    pub division: String,
    pub problem: String,
    pub system: String,
}

fn certified_outcomes(path: &Path) -> ToolResult<BTreeMap<AuditKey, bool>> {
    let mut reader = csv::Reader::from_path(path)?;
    let headers = reader.headers()?.clone();
    let required = ["edition", "division", "problem", "system", "strict_status"];
    let indexes = required
        .iter()
        .map(|column| {
            headers
                .iter()
                .position(|field| field == *column)
                .ok_or_else(|| format!("{} is missing audit column {column}", path.display()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut outcomes = BTreeMap::new();
    for row in reader.records() {
        let row = row?;
        let key = AuditKey {
            edition: row.get(indexes[0]).unwrap_or_default().to_owned(),
            division: row.get(indexes[1]).unwrap_or_default().to_owned(),
            problem: row.get(indexes[2]).unwrap_or_default().to_owned(),
            system: row.get(indexes[3]).unwrap_or_default().to_owned(),
        };
        let status = row.get(indexes[4]).unwrap_or_default();
        if status == "VerifiedBad" {
            return Err(format!("rejected proof blocks promotion: {key:?}").into());
        }
        if outcomes
            .insert(key.clone(), status == "VerifiedGood")
            .is_some()
        {
            return Err(format!("duplicate audit row: {key:?}").into());
        }
    }
    if outcomes.is_empty() {
        return Err(format!("empty audit: {}", path.display()).into());
    }
    Ok(outcomes)
}

pub fn paired_delta(
    baseline: &BTreeMap<AuditKey, bool>,
    candidate: &BTreeMap<AuditKey, bool>,
) -> ToolResult<Value> {
    if baseline.keys().ne(candidate.keys()) {
        return Err("paired audits do not cover the same problem/system set".into());
    }
    let gains = baseline
        .iter()
        .filter(|(key, base)| !**base && candidate[*key])
        .count();
    let losses = baseline
        .iter()
        .filter(|(key, base)| **base && !candidate[*key])
        .count();
    let baseline_certified = baseline.values().filter(|&&value| value).count();
    let candidate_certified = candidate.values().filter(|&&value| value).count();
    Ok(json!({
        "baseline_certified": baseline_certified,
        "candidate_certified": candidate_certified,
        "gains": gains,
        "losses": losses,
        "delta": gains as isize - losses as isize,
        "problems": baseline.len(),
    }))
}

pub fn paired_campaign(args: PairedCampaignArgs) -> ToolResult<()> {
    if args.reps < 2 || args.strict_time == 0 {
        return Err("require reps >= 2 and strict-time > 0".into());
    }
    let root = repo_root();
    let baseline = verify_executable(&args.baseline, "baseline binary")?;
    let candidate = verify_executable(&args.candidate, "candidate binary")?;
    let auditor = verify_executable(
        args.auditor
            .as_deref()
            .unwrap_or(&root.join("target/release/audit_casc_proofs")),
        "proof auditor",
    )?;
    let proover = verify_executable(
        args.proover
            .as_deref()
            .unwrap_or(&root.join("target/release/mrs-proover")),
        "ProoVer binary",
    )?;
    let corpus = args.problems_dir.canonicalize()?;
    if !corpus.is_dir() {
        return Err(format!("corpus is not a directory: {}", corpus.display()).into());
    }
    let output = absolute(&args.output)?;
    create_output_directory(&output)?;

    let mut environment = std::env::vars().collect::<BTreeMap<_, _>>();
    environment.insert("MRS_HARDWARE".into(), "casc".into());
    environment.insert("MRS_WORKERS".into(), "8".into());
    environment.insert(
        "MRS_SHARED_POOL_INTERVAL".into(),
        args.sharing_interval.to_string(),
    );
    environment.insert("CASC_PROBLEMS_ROOT".into(), corpus.display().to_string());
    environment.insert("TPTP".into(), corpus.display().to_string());
    environment.insert(
        "MRS_TOOLS_BIN".into(),
        std::env::current_exe()?
            .canonicalize()?
            .display()
            .to_string(),
    );
    environment.remove("MRS_SCHEDULE");
    environment.remove("MRS_PORTFOLIO");
    if let Some(schedule) = &args.schedule {
        environment.insert("MRS_SCHEDULE".into(), schedule.clone());
    }

    let binaries = BTreeMap::from([
        ("baseline", baseline.clone()),
        ("candidate", candidate.clone()),
    ]);
    let mut rng = StdRng::seed_from_u64(args.seed);
    let mut orders = Vec::<Vec<String>>::new();
    for _ in 0..args.reps / 2 {
        orders.push(vec!["baseline".into(), "candidate".into()]);
        orders.push(vec!["candidate".into(), "baseline".into()]);
    }
    if args.reps % 2 == 1 {
        let mut order = vec!["baseline".into(), "candidate".into()];
        order.shuffle(&mut rng);
        orders.push(order);
    }
    orders.shuffle(&mut rng);

    let hashes = BTreeMap::from([
        ("baseline", sha256_file(&baseline)?),
        ("candidate", sha256_file(&candidate)?),
    ]);
    let tracked_environment = environment
        .iter()
        .filter(|(key, _)| {
            key.starts_with("MRS_")
                || key.starts_with("CASC_")
                || key.starts_with("OMP_")
                || *key == "TPTP"
        })
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    let manifest = json!({
        "schema": 1,
        "arguments": {
            "baseline": baseline,
            "candidate": candidate,
            "problems_dir": corpus,
            "edition": args.edition,
            "divisions": args.divisions,
            "schedule": args.schedule,
            "reps": args.reps,
            "seed": args.seed,
            "sharing_interval": args.sharing_interval,
            "strict_time": args.strict_time,
            "output": output,
            "auditor": auditor,
            "proover": proover,
        },
        "binary_sha256": hashes,
        "pair_orders": orders,
        "auditor_sha256": sha256_file(&auditor)?,
        "proover_sha256": sha256_file(&proover)?,
        "environment": tracked_environment,
    });
    fs::write(
        output.join("experiment.json"),
        format!("{}\n", serde_json::to_string_pretty(&manifest)?),
    )?;

    let mut pairs = Vec::<Value>::new();
    let mut corpus_digest: Option<String> = None;
    for (index, order) in orders.iter().enumerate() {
        let mut audits = HashMap::new();
        for arm in order {
            let binary = &binaries[arm.as_str()];
            if sha256_file(binary)? != hashes[arm.as_str()] {
                return Err(format!("binary changed during experiment: {arm}").into());
            }
            if sha256_file(&auditor)? != manifest["auditor_sha256"]
                || sha256_file(&proover)? != manifest["proover_sha256"]
            {
                return Err("audit tools changed during experiment".into());
            }
            let run = output.join(format!("pair-{}/{arm}", index + 1));
            let mut env = environment.clone();
            env.insert("MRS_BINARY".into(), binary.display().to_string());
            let status = Command::new("bash")
                .arg(root.join("crates/mrs-bench/casc.sh"))
                .args([
                    "--edition",
                    &args.edition,
                    "--systems",
                    "mrs",
                    "--divisions",
                    &args.divisions,
                    "--casc-times",
                    "--jobs",
                    "1",
                    "--output",
                ])
                .arg(&run)
                .envs(&env)
                .current_dir(&root)
                .status()?;
            if !status.success() {
                return Err(format!("casc.sh failed for pair {}, arm {arm}", index + 1).into());
            }
            let audit = run.join("strict-audit");
            let status = Command::new(&auditor)
                .args([
                    "--run",
                    run.join("run.csv").to_str().ok_or("non-UTF8 run path")?,
                    "--problems-dir",
                    corpus.to_str().ok_or("non-UTF8 corpus path")?,
                    "--proover",
                    proover.to_str().ok_or("non-UTF8 ProoVer path")?,
                    "--checks",
                    "strict",
                    "--strict-time",
                    &args.strict_time.to_string(),
                    "--jobs",
                    "1",
                    "--output",
                ])
                .arg(&audit)
                .envs(&env)
                .current_dir(&root)
                .status()?;
            if !status.success() {
                return Err(
                    format!("strict audit failed for pair {}, arm {arm}", index + 1).into(),
                );
            }
            let identity: Value =
                serde_json::from_slice(&fs::read(audit.join("audit-identity.json"))?)?;
            let digest = identity["corpus_sha256"]
                .as_str()
                .ok_or("audit identity has no corpus_sha256")?
                .to_owned();
            if let Some(expected) = &corpus_digest {
                if expected != &digest {
                    return Err("corpus/include contents changed between paired arms".into());
                }
            } else {
                corpus_digest = Some(digest);
            }
            audits.insert(arm.as_str(), certified_outcomes(&audit.join("audit.csv"))?);
        }
        let delta = paired_delta(&audits["baseline"], &audits["candidate"])?;
        let delta_number = delta["delta"]
            .as_i64()
            .ok_or("paired delta is not an integer")?;
        pairs.push(delta);
        let deltas = pairs
            .iter()
            .map(|pair| pair["delta"].as_i64().unwrap_or_default())
            .collect::<Vec<_>>();
        let summary = json!({
            "pairs": pairs,
            "mean_paired_delta": deltas.iter().sum::<i64>() as f64 / deltas.len() as f64,
            "observed_delta_range": [deltas.iter().min(), deltas.iter().max()],
            "interpretation": "descriptive paired evidence; two pairs are a screen, add pairs if outcomes disagree or the effect is marginal; no automatic promotion or division-wide noise extrapolation",
        });
        fs::write(
            output.join("paired-summary.json"),
            format!("{}\n", serde_json::to_string_pretty(&summary)?),
        )?;
        println!("pair {}: certified delta {delta_number:+}", index + 1);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchDetail(pub BTreeMap<String, String>);

pub fn parse_search_detail(stderr: &str) -> ToolResult<SearchDetail> {
    let lines = stderr
        .lines()
        .filter(|line| line.starts_with("% SZS detail "))
        .collect::<Vec<_>>();
    if lines.len() != 1 {
        return Err("expected exactly one final search detail line".into());
    }
    let tokens = shlex::split(&lines[0]["% SZS detail ".len()..])
        .ok_or("could not parse quoted % SZS detail fields")?;
    let mut values = BTreeMap::new();
    for token in tokens {
        if let Some((key, value)) = token.split_once('=') {
            values.insert(key.to_owned(), value.to_owned());
        }
    }
    Ok(SearchDetail(values))
}

pub fn equivalent_work(
    baseline: &BTreeMap<String, String>,
    candidate: &BTreeMap<String, String>,
    processed: u64,
) -> ToolResult<()> {
    let required = [
        "processed",
        "generated",
        "passive",
        "lrs_discarded",
        "weight_discarded",
        "trajectory_sha256",
        "resource_reason",
    ];
    for arm in [baseline, candidate] {
        if required
            .iter()
            .any(|key| arm.get(*key).is_none_or(String::is_empty))
        {
            return Err("incomplete work telemetry".into());
        }
        if arm.get("processed").map(String::as_str) != Some(&processed.to_string()) {
            return Err("run did not reach the declared fixed-work ceiling".into());
        }
        if arm.get("resource_reason").map(String::as_str) != Some("max_processed") {
            return Err("fixed-work run stopped for another reason".into());
        }
    }
    for key in [
        "trajectory_sha256",
        "generated",
        "passive",
        "lrs_discarded",
        "weight_discarded",
    ] {
        if baseline.get(key) != candidate.get(key) {
            return Err(format!("work trajectory differs: {key}").into());
        }
    }
    Ok(())
}

fn run_with_guard(
    command: &mut Command,
    timeout: Duration,
) -> ToolResult<(std::process::ExitStatus, String, String, f64)> {
    let stdout_file = tempfile::NamedTempFile::new()?;
    let stderr_file = tempfile::NamedTempFile::new()?;
    command
        .stdout(Stdio::from(stdout_file.reopen()?))
        .stderr(Stdio::from(stderr_file.reopen()?));
    let start = Instant::now();
    let mut child = command.spawn()?;
    let status = match child.wait_timeout(timeout)? {
        Some(status) => status,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "process exceeded guard timeout of {} seconds",
                timeout.as_secs()
            )
            .into());
        }
    };
    let elapsed = start.elapsed().as_secs_f64();
    let stdout = fs::read_to_string(stdout_file.path())?;
    let stderr = fs::read_to_string(stderr_file.path())?;
    Ok((status, stdout, stderr, elapsed))
}

pub fn fixed_work_compare(args: FixedWorkArgs) -> ToolResult<()> {
    if args.processed == 0 || args.guard_seconds == 0 || args.reps == 0 {
        return Err("work, guard, and repetition counts must be positive".into());
    }
    let baseline = verify_executable(&args.baseline, "baseline binary")?;
    let candidate = verify_executable(&args.candidate, "candidate binary")?;
    let problem = args.problem.canonicalize()?;
    if !problem.is_file() {
        return Err(format!("problem is not a file: {}", problem.display()).into());
    }
    let output = absolute(&args.output)?;
    create_output_directory(&output)?;
    let hashes = BTreeMap::from([
        ("baseline", sha256_file(&baseline)?),
        ("candidate", sha256_file(&candidate)?),
    ]);
    let problem_hash = sha256_file(&problem)?;
    let mut env = std::env::vars().collect::<BTreeMap<_, _>>();
    env.extend([
        ("MRS_NO_LRS".into(), "1".into()),
        ("MRS_SHARED_POOL_INTERVAL".into(), "0".into()),
        ("MRS_MAX_PROCESSED".into(), args.processed.to_string()),
        ("MRS_TRAJECTORY_DIGEST".into(), "1".into()),
        ("MRS_SPECIALIZE_QUEUES".into(), "0".into()),
        ("MRS_COMPACT_QUEUES".into(), "0".into()),
        ("MRS_PREPHASE".into(), "0".into()),
    ]);
    let mut rng = StdRng::seed_from_u64(args.seed);
    let mut orders = (0..args.reps)
        .map(|_| {
            let mut pair = vec!["baseline", "candidate"];
            pair.shuffle(&mut rng);
            pair
        })
        .collect::<Vec<_>>();
    let tracked_environment = env
        .iter()
        .filter(|(key, _)| key.starts_with("MRS_") || *key == "TPTP")
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    let manifest = json!({
        "schema": 1,
        "binary_sha256": hashes,
        "problem_sha256": problem_hash,
        "problem": problem,
        "arguments": {
            "baseline": baseline,
            "candidate": candidate,
            "output": output,
            "schedule": args.schedule,
            "strategy": args.strategy,
            "processed": args.processed,
            "guard_seconds": args.guard_seconds,
            "reps": args.reps,
            "seed": args.seed,
            "candidate_specialize": args.candidate_specialize,
            "candidate_compact": args.candidate_compact,
        },
        "orders": orders,
        "environment": tracked_environment,
    });
    fs::write(
        output.join("experiment.json"),
        format!("{}\n", serde_json::to_string_pretty(&manifest)?),
    )?;

    let binaries = BTreeMap::from([("baseline", baseline), ("candidate", candidate)]);
    let mut pairs = Vec::<Value>::new();
    let mut reference: Option<BTreeMap<String, String>> = None;
    for (index, order) in orders.drain(..).enumerate() {
        let mut arms = BTreeMap::<String, Value>::new();
        let mut details = BTreeMap::<String, BTreeMap<String, String>>::new();
        for arm in order {
            let binary = &binaries[arm];
            if sha256_file(binary)? != hashes[arm]
                || sha256_file(&problem)? != manifest["problem_sha256"]
            {
                return Err("binary or problem changed during measurement".into());
            }
            let mut arm_env = env.clone();
            if arm == "candidate" {
                arm_env.insert(
                    "MRS_SPECIALIZE_QUEUES".into(),
                    u8::from(args.candidate_specialize).to_string(),
                );
                arm_env.insert(
                    "MRS_COMPACT_QUEUES".into(),
                    u8::from(args.candidate_compact).to_string(),
                );
            }
            let mut command = Command::new(binary);
            command
                .args([
                    "--workers",
                    "1",
                    "--time",
                    &args.guard_seconds.to_string(),
                    "--schedule",
                    &args.schedule,
                    "--strategy",
                    &args.strategy.to_string(),
                ])
                .arg(&problem)
                .envs(&arm_env);
            let (status, stdout, stderr, elapsed) =
                run_with_guard(&mut command, Duration::from_secs(args.guard_seconds + 10))?;
            let prefix = output.join(format!("pair-{}-{arm}", index + 1));
            fs::write(prefix.with_extension("stdout"), stdout)?;
            fs::write(prefix.with_extension("stderr"), &stderr)?;
            if !status.success() {
                return Err(format!("{arm} exited with {status}").into());
            }
            let detail = parse_search_detail(&stderr)?.0;
            details.insert(arm.to_owned(), detail.clone());
            arms.insert(
                arm.to_owned(),
                json!({"elapsed_s": elapsed, "detail": detail}),
            );
        }
        equivalent_work(&details["baseline"], &details["candidate"], args.processed)?;
        if let Some(reference) = &reference {
            equivalent_work(reference, &details["baseline"], args.processed)?;
        }
        reference = Some(details["baseline"].clone());
        let baseline_elapsed = arms["baseline"]["elapsed_s"].as_f64().unwrap_or(0.0);
        let candidate_elapsed = arms["candidate"]["elapsed_s"].as_f64().unwrap_or(0.0);
        if baseline_elapsed <= 0.0 || candidate_elapsed <= 0.0 {
            return Err("fixed-work elapsed time must be positive for a ratio".into());
        }
        pairs.push(json!({
            "arms": arms,
            "candidate_over_baseline_elapsed": candidate_elapsed / baseline_elapsed,
        }));
        let comparison = json!({
            "pairs": pairs,
            "interpretation": "paired fixed-work diagnostics including hashing/startup; no automatic promotion",
        });
        fs::write(
            output.join("comparison.json"),
            format!("{}\n", serde_json::to_string_pretty(&comparison)?),
        )?;
        println!(
            "pair {}: equivalent selected work, elapsed ratio {:.3}",
            index + 1,
            candidate_elapsed / baseline_elapsed
        );
    }
    Ok(())
}

#[derive(Debug)]
struct SnapshotGroup {
    edition: String,
    division: String,
    system: String,
    statuses: BTreeMap<String, usize>,
}

fn read_snapshot(directory: &Path) -> ToolResult<(Value, String, Vec<SnapshotGroup>)> {
    let identity: Value =
        serde_json::from_slice(&fs::read(directory.join("audit-identity.json"))?)?;
    let data = fs::read(directory.join("audit.csv"))?;
    let digest = hex_digest(&data);
    if directory.file_name().and_then(|name| name.to_str()) != Some(&digest) {
        return Err(format!("snapshot content digest mismatch: {}", directory.display()).into());
    }
    for field in [
        "audit_binary_sha256",
        "proover_binary_sha256",
        "run_csv_sha256",
        "corpus_sha256",
    ] {
        if identity["schema"] != 1 || identity[field].as_str().is_none_or(str::is_empty) {
            return Err(format!("incomplete snapshot identity: {}", directory.display()).into());
        }
    }
    let mut reader = csv::Reader::from_reader(data.as_slice());
    let headers = reader.headers()?.clone();
    let required = ["edition", "division", "problem", "system", "strict_status"];
    let indexes = required
        .iter()
        .map(|column| {
            headers
                .iter()
                .position(|field| field == *column)
                .ok_or_else(|| format!("snapshot is missing column {column}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut rows = BTreeSet::new();
    let mut grouped = BTreeMap::<(String, String, String), BTreeMap<String, usize>>::new();
    for row in reader.records() {
        let row = row?;
        let key = (0..4)
            .map(|i| row.get(indexes[i]).unwrap_or_default().to_owned())
            .collect::<Vec<_>>();
        if !rows.insert(key.clone()) {
            return Err(format!("duplicate audit outcome: {key:?}").into());
        }
        let status = match row.get(indexes[4]).unwrap_or_default() {
            "" => "not_run",
            value => value,
        };
        *grouped
            .entry((key[0].clone(), key[1].clone(), key[3].clone()))
            .or_default()
            .entry(status.to_owned())
            .or_default() += 1;
    }
    if rows.is_empty() {
        return Err(format!("empty snapshot: {}", directory.display()).into());
    }
    let groups = grouped
        .into_iter()
        .map(|((edition, division, system), statuses)| SnapshotGroup {
            edition,
            division,
            system,
            statuses,
        })
        .collect();
    Ok((identity, digest, groups))
}

pub fn render_dashboard(directories: &[PathBuf]) -> ToolResult<String> {
    let mut lines = vec![
        "# Certification evidence dashboard".to_owned(),
        String::new(),
        "Each section is one immutable audit snapshot. Counts are not a HEAD baseline,".to_owned(),
        "and zero rejected proofs is not a global soundness theorem.".to_owned(),
        String::new(),
    ];
    let mut seen = BTreeSet::new();
    for directory in directories {
        let (identity, digest, groups) = read_snapshot(directory)?;
        let identity_key = serde_json::to_string(&identity)?;
        if !seen.insert((identity_key, digest.clone())) {
            return Err("duplicate evidence snapshot supplied".into());
        }
        lines.extend([
            format!("## Snapshot `{digest}`"),
            String::new(),
            format!("- Artifact directory: `{}`", directory.display()),
            format!(
                "- Run CSV: `{}`",
                identity["run_csv_sha256"].as_str().unwrap_or_default()
            ),
            format!(
                "- Producer metadata: `{}`",
                identity["producer_metadata_sha256"]
                    .as_str()
                    .filter(|v| !v.is_empty())
                    .unwrap_or("unavailable (legacy evidence)")
            ),
            format!(
                "- Auditor: `{}`",
                identity["audit_binary_sha256"].as_str().unwrap_or_default()
            ),
            format!(
                "- Checker: `{}`",
                identity["proover_binary_sha256"]
                    .as_str()
                    .unwrap_or_default()
            ),
            format!(
                "- Corpus/includes: `{}`",
                identity["corpus_sha256"].as_str().unwrap_or_default()
            ),
            format!(
                "- Strict budget: {} s; jobs: {}",
                identity["strict_time"]
                    .as_u64()
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "?".into()),
                identity["jobs"]
                    .as_u64()
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "?".into())
            ),
            String::new(),
            "| Edition | Division | System | Rows | Strict certified | Rejected | Other outcomes |"
                .into(),
            "|---|---|---|---:|---:|---:|---|".into(),
        ]);
        for group in groups {
            let total = group.statuses.values().sum::<usize>();
            let others = group
                .statuses
                .iter()
                .filter(|(status, _)| !matches!(status.as_str(), "VerifiedGood" | "VerifiedBad"))
                .map(|(status, count)| format!("{status}: {count}"))
                .collect::<Vec<_>>();
            lines.push(format!(
                "| {} | {} | {} | {} | {} | {} | {} |",
                group.edition,
                group.division,
                group.system,
                total,
                group
                    .statuses
                    .get("VerifiedGood")
                    .copied()
                    .unwrap_or_default(),
                group
                    .statuses
                    .get("VerifiedBad")
                    .copied()
                    .unwrap_or_default(),
                if others.is_empty() {
                    "—".to_owned()
                } else {
                    others.join("; ")
                },
            ));
        }
        lines.push(String::new());
    }
    Ok(lines.join("\n"))
}

pub fn certification_dashboard(args: DashboardArgs) -> ToolResult<()> {
    print!("{}", render_dashboard(&args.snapshots)?);
    Ok(())
}

fn csv_column(headers: &csv::StringRecord, name: &str) -> ToolResult<usize> {
    headers
        .iter()
        .position(|header| header == name)
        .ok_or_else(|| format!("CSV is missing required column {name}").into())
}

pub fn count_verified(args: CountVerifiedArgs) -> ToolResult<()> {
    let mut reader = csv::Reader::from_path(args.audit_csv)?;
    let headers = reader.headers()?.clone();
    let generation = csv_column(&headers, "generation_status")?;
    let strict = csv_column(&headers, "strict_status")?;
    let mut total = 0;
    let mut good = 0;
    for record in reader.records() {
        let record = record?;
        if matches!(
            record.get(generation).unwrap_or_default(),
            "Unsatisfiable" | "Theorem"
        ) {
            total += 1;
            if record.get(strict) == Some("VerifiedGood") {
                good += 1;
            }
        }
    }
    println!("{good}/{total}");
    Ok(())
}

pub fn record_binary_hash(args: RecordBinaryHashArgs) -> ToolResult<()> {
    let mut metadata: Value = serde_json::from_slice(&fs::read(&args.metadata)?)?;
    let object = metadata
        .as_object_mut()
        .ok_or("run metadata JSON must be an object")?;
    object.insert("binary_sha256".into(), Value::String(args.sha256));
    fs::write(
        args.metadata,
        format!("{}\n", serde_json::to_string_pretty(&metadata)?),
    )?;
    Ok(())
}

pub fn read_binary_hash(args: ReadBinaryHashArgs) -> ToolResult<()> {
    let metadata: Value = serde_json::from_slice(&fs::read(args.metadata)?)?;
    let digest = metadata
        .get("binary_sha256")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    println!("{digest}");
    Ok(())
}

pub fn epr_telemetry(args: EprTelemetryArgs) -> ToolResult<()> {
    let output_dir = absolute(&args.output_dir)?;
    let archive = output_dir.join("run/raw/mrs/epu");
    let mut reader = csv::Reader::from_path(args.run_csv)?;
    let headers = reader.headers()?.clone();
    let problem = csv_column(&headers, "problem")?;
    let status = csv_column(&headers, "szs_status")?;
    const KEYS: [&str; 11] = [
        "epr_attempted",
        "epr_route",
        "epr_generated",
        "epr_rounds",
        "epr_falsifying",
        "epr_full_grounding",
        "epr_clauses",
        "epr_vars",
        "epr_ms",
        "epr_result",
        "epr_fallback",
    ];
    let path = output_dir.join("epr_telemetry.csv");
    let mut writer = csv::Writer::from_path(path)?;
    let mut header = vec!["problem", "szs_status"];
    header.extend(KEYS);
    writer.write_record(header)?;
    for record in reader.records() {
        let record = record?;
        let problem_name = record.get(problem).unwrap_or_default();
        let stderr_path = archive.join(format!("{problem_name}.stderr"));
        let stderr = fs::read(stderr_path)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_default();
        let mut telemetry = BTreeMap::new();
        for token in stderr.split_whitespace() {
            if let Some((key, value)) = token.split_once('=')
                && key.starts_with("epr_")
            {
                telemetry.insert(key, value);
            }
        }
        let mut row = vec![
            problem_name.to_owned(),
            record.get(status).unwrap_or_default().to_owned(),
        ];
        row.extend(
            KEYS.iter()
                .map(|key| telemetry.get(key).copied().unwrap_or_default().to_owned()),
        );
        writer.write_record(row)?;
    }
    writer.flush()?;
    Ok(())
}

pub fn prephase_ab_summary(args: PrephaseAbArgs) -> ToolResult<()> {
    fn load(path: &Path) -> ToolResult<BTreeMap<String, String>> {
        let mut reader = csv::Reader::from_path(path)?;
        let headers = reader.headers()?.clone();
        let problem = csv_column(&headers, "problem")?;
        let status = csv_column(&headers, "szs_status")?;
        let mut statuses = BTreeMap::new();
        for row in reader.records() {
            let row = row?;
            let name = row.get(problem).unwrap_or_default().to_owned();
            if name != "problem" {
                statuses.insert(name, row.get(status).unwrap_or_default().to_owned());
            }
        }
        Ok(statuses)
    }
    const SOLVED: [&str; 4] = [
        "Theorem",
        "Unsatisfiable",
        "Satisfiable",
        "CounterSatisfiable",
    ];
    let root = absolute(&args.output_dir)?;
    let baseline = load(&root.join("baseline/run.csv"))?;
    let prephase = load(&root.join("prephase/run.csv"))?;
    let problems = baseline
        .keys()
        .chain(prephase.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let solved = |map: &BTreeMap<String, String>| {
        problems
            .iter()
            .filter(|problem| {
                map.get(*problem)
                    .is_some_and(|status| SOLVED.contains(&status.as_str()))
            })
            .count()
    };
    let baseline_count = solved(&baseline);
    let prephase_count = solved(&prephase);
    println!("  baseline: {baseline_count}/{}", problems.len());
    println!("  prephase: {prephase_count}/{}", problems.len());
    println!(
        "  delta:    {:+}",
        prephase_count as isize - baseline_count as isize
    );
    println!("\n  problems where the arms disagree:");
    for problem in &problems {
        let before = baseline
            .get(problem)
            .map(String::as_str)
            .unwrap_or("missing");
        let after = prephase
            .get(problem)
            .map(String::as_str)
            .unwrap_or("missing");
        if SOLVED.contains(&before) != SOLVED.contains(&after) {
            println!("    {problem:<20} baseline={before:<14} prephase={after}");
        }
    }
    println!("\n  raw output under {}", root.display());
    Ok(())
}

#[derive(Debug, Clone)]
struct VarianceRow {
    szs_status: String,
    verdict: String,
}

pub fn variance_report(args: VarianceArgs) -> ToolResult<()> {
    if args.reps < 2 {
        return Err("reps must be >= 2".into());
    }
    let parent = absolute(&args.parent)?;
    let mut rows = BTreeMap::<(String, String, String), BTreeMap<usize, VarianceRow>>::new();
    let mut systems = BTreeSet::new();
    for replicate in 1..=args.reps {
        let path = parent.join(format!("rep{replicate}/run.csv"));
        let mut reader = csv::Reader::from_path(&path)
            .map_err(|error| format!("variance: cannot read {}: {error}", path.display()))?;
        let headers = reader.headers()?.clone();
        let division = csv_column(&headers, "division")?;
        let system = csv_column(&headers, "system")?;
        let problem = csv_column(&headers, "problem")?;
        let szs = csv_column(&headers, "szs_status")?;
        let verdict = csv_column(&headers, "verdict")?;
        for record in reader.records() {
            let record = record?;
            let key = (
                record.get(division).unwrap_or_default().to_owned(),
                record.get(system).unwrap_or_default().to_owned(),
                record.get(problem).unwrap_or_default().to_owned(),
            );
            systems.insert(key.1.clone());
            let entry = rows.entry(key.clone()).or_default();
            if entry
                .insert(
                    replicate,
                    VarianceRow {
                        szs_status: record.get(szs).unwrap_or_default().to_owned(),
                        verdict: record.get(verdict).unwrap_or_default().to_owned(),
                    },
                )
                .is_some()
            {
                return Err(
                    format!("variance: duplicate row for {key:?} in rep{replicate}").into(),
                );
            }
        }
    }
    println!(
        "\n{:<10} {:<16} {:<16} {:>12} verdicts",
        "division", "system", "problem", "solved"
    );
    println!("{}", "-".repeat(82));
    let mut unstable = Vec::new();
    let mut totals = BTreeMap::<String, Vec<usize>>::new();
    for ((division, system, problem), by_replicate) in &rows {
        let marks = (1..=args.reps)
            .map(|replicate| match by_replicate.get(&replicate) {
                Some(row) if row.verdict == "ok" => '.',
                _ => 'x',
            })
            .collect::<String>();
        let outcomes = by_replicate
            .values()
            .map(|row| format!("{}:{}", row.szs_status, row.verdict))
            .collect::<BTreeSet<_>>();
        println!(
            "{division:<10} {system:<16} {problem:<16} {marks:>12}   {}",
            outcomes.into_iter().collect::<Vec<_>>().join("/")
        );
        if by_replicate.len() != args.reps
            || by_replicate
                .values()
                .map(|row| (&row.szs_status, &row.verdict))
                .collect::<BTreeSet<_>>()
                .len()
                > 1
        {
            unstable.push((division.clone(), system.clone(), problem.clone()));
        }
    }
    println!("{}", "-".repeat(82));
    for system in systems {
        let counts = (1..=args.reps)
            .map(|replicate| {
                rows.iter()
                    .filter(|((_, row_system, _), _)| row_system == &system)
                    .filter(|(_, per_rep)| {
                        per_rep
                            .get(&replicate)
                            .is_some_and(|row| row.verdict == "ok")
                    })
                    .count()
            })
            .collect::<Vec<_>>();
        let min = counts.iter().min().copied().unwrap_or_default();
        let max = counts.iter().max().copied().unwrap_or_default();
        let mean = counts.iter().sum::<usize>() as f64 / counts.len() as f64;
        println!("{system} solved per replicate: {counts:?}");
        println!(
            "  mean {mean:.2}  min {min}  max {max}  spread {}",
            max - min
        );
        totals.insert(system, counts);
    }
    if !unstable.is_empty() {
        println!(
            "\n{} problem(s) changed verdict between replicates:",
            unstable.len()
        );
        for (division, system, problem) in unstable {
            println!("  {division}/{system}/{problem}");
        }
        println!("\nA single-run A/B cannot separate these from a real effect. Report means.");
    } else {
        println!("\nno problem changed verdict across replicates.");
    }
    println!("\nreplicate runs under {}", parent.display());
    Ok(())
}

pub fn run_csv_summary(args: RunCsvSummaryArgs) -> ToolResult<()> {
    let mut reader = csv::Reader::from_path(args.run_csv)?;
    let headers = reader.headers()?.clone();
    let status = csv_column(&headers, "szs_status")?;
    let verdict = csv_column(&headers, "verdict")?;
    let division = args
        .division
        .as_ref()
        .map(|_| csv_column(&headers, "division"))
        .transpose()?;
    let filter = args.division.map(|value| value.to_ascii_lowercase());
    let mut statuses = BTreeMap::<String, usize>::new();
    let mut verdicts = BTreeMap::<String, usize>::new();
    for row in reader.records() {
        let row = row?;
        if let (Some(filter), Some(index)) = (&filter, division)
            && row.get(index).unwrap_or_default().to_ascii_lowercase() != *filter
        {
            continue;
        }
        *statuses
            .entry(row.get(status).unwrap_or_default().to_owned())
            .or_default() += 1;
        *verdicts
            .entry(row.get(verdict).unwrap_or_default().to_owned())
            .or_default() += 1;
    }
    println!("SZS status counts:");
    for (status, count) in statuses {
        println!("  {status}: {count}");
    }
    println!("grading verdict counts:");
    for (verdict, count) in verdicts {
        println!("  {verdict}: {count}");
    }
    Ok(())
}

fn problem_family(problem: &str) -> String {
    problem
        .trim_end_matches(|character: char| {
            character.is_ascii_digit() || matches!(character, '.' | '+')
        })
        .trim_end_matches('+')
        .to_owned()
}

pub fn failure_profile(args: FailureProfileArgs) -> ToolResult<()> {
    let mut reader = csv::Reader::from_path(args.run_csv)?;
    let headers = reader.headers()?.clone();
    let division = csv_column(&headers, "division")?;
    let problem = csv_column(&headers, "problem")?;
    let status = csv_column(&headers, "szs_status")?;
    let filter = args.division.map(|value| value.to_ascii_lowercase());
    let mut families = BTreeMap::<String, usize>::new();
    for row in reader.records() {
        let row = row?;
        if let Some(filter) = &filter
            && row.get(division).unwrap_or_default().to_ascii_lowercase() != *filter
        {
            continue;
        }
        if row.get(status) != Some("Theorem") {
            *families
                .entry(problem_family(row.get(problem).unwrap_or_default()))
                .or_default() += 1;
        }
    }
    let mut families = families.into_iter().collect::<Vec<_>>();
    families.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
    for (family, count) in families {
        println!("{family}: {count}");
    }
    Ok(())
}

pub fn list_missing(args: ListMissingArgs) -> ToolResult<()> {
    let mut reader = csv::Reader::from_path(args.csv)?;
    let headers = reader.headers()?.clone();
    let target = csv_column(&headers, &args.column)?;
    let output = csv_column(&headers, &args.print_column)?;
    for row in reader.records() {
        let row = row?;
        if row.get(target).unwrap_or_default().is_empty() {
            println!("{}", row.get(output).unwrap_or_default());
        }
    }
    Ok(())
}

pub fn codex_overlap(args: CodexOverlapArgs) -> ToolResult<()> {
    let connection = Connection::open_with_flags(
        args.database,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )?;
    let mut statement = connection.prepare(
        "select problem_name from results where corpus like ?1 and division = ?2 and status = ?3",
    )?;
    let stored = statement
        .query_map(
            [&args.corpus_like, &args.division, &args.codex_status],
            |row| row.get::<_, String>(0),
        )?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|problem| {
            Path::new(&problem)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(&problem)
                .replace(".p", "")
        })
        .collect::<BTreeSet<_>>();
    let mut reader = csv::Reader::from_path(args.run_csv)?;
    let headers = reader.headers()?.clone();
    let problem = csv_column(&headers, "problem")?;
    let verdict = csv_column(&headers, "verdict")?;
    let mut current = BTreeSet::new();
    for row in reader.records() {
        let row = row?;
        if row.get(verdict) == Some(args.run_verdict.as_str()) {
            current.insert(row.get(problem).unwrap_or_default().to_owned());
        }
    }
    let common = stored.intersection(&current).count();
    let lost = stored.difference(&current).count();
    let gained = current.difference(&stored).count();
    println!("common {common} lost {lost} gained {gained}");
    Ok(())
}

fn detail_counter(details: &str, key: &str) -> u64 {
    details
        .split_whitespace()
        .find_map(|token| {
            let (field, value) = token.split_once('=')?;
            (field == key)
                .then(|| {
                    value
                        .chars()
                        .take_while(char::is_ascii_digit)
                        .collect::<String>()
                        .parse()
                        .ok()
                })
                .flatten()
        })
        .unwrap_or_default()
}

pub fn check_canary_drift(args: CanaryDriftArgs) -> ToolResult<()> {
    let mut reader = csv::Reader::from_path(&args.run_csv)?;
    let headers = reader.headers()?.clone();
    let division = csv_column(&headers, "division")?;
    let problem = csv_column(&headers, "problem")?;
    let status = csv_column(&headers, "szs_status")?;
    let wall_time = csv_column(&headers, "wall_time_s")?;
    let details = headers.iter().position(|field| field == "failure_detail");
    let rows = reader.records().collect::<Result<Vec<_>, _>>()?;
    println!("=== CASC Division Canary Audit Report ===");
    println!("Target CSV: {}", args.run_csv.display());
    println!("---------------------------------------------------------");
    let canaries = [
        ("EPS", "HWC004-1"),
        ("FNE", "CSR026+3"),
        ("FEQ", "AGT005+1"),
        ("UEQ", "ALG212-10"),
    ];
    let mut found = 0usize;
    let mut contaminated = false;
    let mut reasons = Vec::new();
    for (expected_division, name) in canaries {
        let Some(row) = rows.iter().find(|row| {
            row.get(problem) == Some(name)
                && row
                    .get(division)
                    .is_some_and(|value| value.eq_ignore_ascii_case(expected_division))
        }) else {
            continue;
        };
        found += 1;
        let szs = row.get(status).unwrap_or_default();
        let time = row.get(wall_time).unwrap_or_default();
        let details = details.and_then(|index| row.get(index)).unwrap_or_default();
        let processed = detail_counter(details, "processed");
        let generated = detail_counter(details, "generated");
        let lrs_discarded = detail_counter(details, "lrs_discarded");
        let passive = detail_counter(details, "passive");
        let contaminated_reason = match expected_division {
            "EPS" if lrs_discarded > 1000 || matches!(szs, "GaveUp" | "Timeout") => Some(format!(
                "EPS Canary {name} bloated with {lrs_discarded} LRS discards (Axioms include-drift)"
            )),
            "FNE" if processed > 1000 || (szs == "Theorem" && time != "0.002") => Some(format!(
                "FNE Canary {name} bloated with {processed} processed clauses (CSR include-drift)"
            )),
            "FEQ" if processed > 1000 || szs == "GaveUp" => Some(format!(
                "FEQ Canary {name} bloated with {processed} processed clauses (AGT include-drift)"
            )),
            "UEQ" if processed > 1000 || szs == "GaveUp" => Some(format!(
                "UEQ Canary {name} bloated with {processed} processed clauses (ALG include-drift)"
            )),
            _ => None,
        };
        let metrics = if expected_division == "EPS" {
            format!("passive={passive} | lrs_discard=(Σ){lrs_discarded}")
        } else {
            format!("processed={processed} | generated={generated}")
        };
        if let Some(reason) = contaminated_reason {
            contaminated = true;
            reasons.push(reason);
            println!(
                "  [{expected_division}] Canary ({name}) : SZS={szs} | Time={time}s | {metrics} -> CONTAMINATED"
            );
        } else {
            println!(
                "  [{expected_division}] Canary ({name}) : SZS={szs} | Time={time}s | {metrics} -> CLEAN"
            );
        }
    }
    println!("---------------------------------------------------------");
    if found == 0 {
        println!("Result: UNVERIFIABLE (No division canaries found in this CSV)");
    } else if contaminated {
        println!("Result: KO - CONTAMINATED RUN");
        for reason in reasons {
            println!("  * Reason: {reason}");
        }
        return Err(Box::new(CanaryContaminated));
    } else {
        println!("Result: OK - CLEAN RUN (All present canaries verified clean)");
    }
    Ok(())
}

pub fn summarize_certification(args: CertificationSummaryArgs) -> ToolResult<()> {
    let mut reader = csv::Reader::from_path(&args.report)?;
    let headers = reader.headers()?.clone();
    let columns = [
        "checks",
        "division",
        "problem",
        "generation_status",
        "strict_status",
        "strict_detail",
    ]
    .map(|name| csv_column(&headers, name))
    .into_iter()
    .collect::<ToolResult<Vec<_>>>()?;
    let mut rows = reader.records().collect::<Result<Vec<_>, _>>()?;
    let checks = rows
        .first()
        .and_then(|row| row.get(columns[0]))
        .unwrap_or_default()
        .split(',')
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let metadata = fs::read_to_string(&args.run_meta).unwrap_or_default();
    let subset = metadata
        .lines()
        .find_map(|line| line.strip_prefix("subset_resolved="))
        .map(str::trim)
        .filter(|path| !path.is_empty());
    if let Some(path) = subset {
        let selected = fs::read_to_string(path)?
            .lines()
            .filter_map(|line| {
                let (division, problem) = line.trim().split_once('/')?;
                let problem = problem.strip_suffix(".p").unwrap_or(problem);
                Some((division.to_ascii_lowercase(), problem.to_owned()))
            })
            .collect::<BTreeSet<_>>();
        rows.retain(|row| {
            let division = row.get(columns[1]).unwrap_or_default().to_ascii_lowercase();
            let problem = row
                .get(columns[2])
                .unwrap_or_default()
                .strip_suffix(".p")
                .unwrap_or(row.get(columns[2]).unwrap_or_default())
                .to_owned();
            selected.contains(&(division, problem))
        });
    }
    if rows.is_empty() {
        return Err("no benchmark rows matched the selected subset".into());
    }
    let mut divisions = BTreeMap::<String, BTreeMap<String, usize>>::new();
    let mut reasons = BTreeMap::<(String, String), usize>::new();
    let mut refutations = 0usize;
    for row in &rows {
        let division = row.get(columns[1]).unwrap_or_default().to_owned();
        let generation = row.get(columns[3]).unwrap_or_default();
        let strict = row.get(columns[4]).unwrap_or_default();
        let counts = divisions.entry(division).or_default();
        if matches!(generation, "Theorem" | "Unsatisfiable") {
            refutations += 1;
            *counts.entry(strict.to_owned()).or_default() += 1;
            *counts.entry("refutations".to_owned()).or_default() += 1;
            if strict != "VerifiedGood" {
                let detail = row.get(columns[5]).unwrap_or_default();
                let normalized = normalize_reason(detail);
                *reasons.entry((strict.to_owned(), normalized)).or_default() += 1;
            }
        } else if matches!(generation, "Satisfiable" | "CounterSatisfiable") {
            *counts.entry("models".to_owned()).or_default() += 1;
            let certified_model = (strict == "VerifiedGood"
                && row
                    .get(columns[5])
                    .unwrap_or_default()
                    .contains("certified_model"))
                || (strict == "not_run" && !checks.contains("strict"));
            if certified_model {
                *counts.entry("certified_models".to_owned()).or_default() += 1;
            }
        }
    }
    println!("== outcome ==\n");
    if refutations == 0 {
        println!("no refutations in this run");
    }
    println!(
        "{:<10} {:>12} {:>10} {:>9} {:>11} {:>10} {:>10}",
        "division", "refutations", "certified", "rejected", "unverified", "unchecked", "certified%"
    );
    let mut totals = BTreeMap::<String, usize>::new();
    for (division, counts) in &divisions {
        let n = counts.get("refutations").copied().unwrap_or_default();
        let certified = counts.get("VerifiedGood").copied().unwrap_or_default();
        let rejected = counts.get("VerifiedBad").copied().unwrap_or_default();
        let unverified = counts.get("Unknown").copied().unwrap_or_default()
            + counts.get("Timeout").copied().unwrap_or_default();
        let unchecked = n.saturating_sub(certified + rejected + unverified);
        for (key, value) in [
            ("refutations", n),
            ("certified", certified),
            ("rejected", rejected),
            ("unverified", unverified),
            ("unchecked", unchecked),
        ] {
            *totals.entry(key.into()).or_default() += value;
        }
        println!(
            "{division:<10} {n:>12} {certified:>10} {rejected:>9} {unverified:>11} {unchecked:>10} {:>9.1}%",
            if n == 0 {
                0.0
            } else {
                certified as f64 * 100.0 / n as f64
            }
        );
        let models = counts.get("models").copied().unwrap_or_default();
        if models > 0 {
            println!(
                "  models: {}/{} certified",
                counts.get("certified_models").copied().unwrap_or_default(),
                models
            );
        }
    }
    let n = totals.get("refutations").copied().unwrap_or_default();
    println!(
        "{:<10} {:>12} {:>10} {:>9} {:>11} {:>10} {:>9.1}%",
        "TOTAL",
        n,
        totals.get("certified").copied().unwrap_or_default(),
        totals.get("rejected").copied().unwrap_or_default(),
        totals.get("unverified").copied().unwrap_or_default(),
        totals.get("unchecked").copied().unwrap_or_default(),
        if n == 0 {
            0.0
        } else {
            totals.get("certified").copied().unwrap_or_default() as f64 * 100.0 / n as f64
        }
    );
    println!("\nreasons (rejected first: those are the ones to investigate)");
    let mut reasons = reasons.into_iter().collect::<Vec<_>>();
    reasons.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
    for ((status, detail), count) in reasons.into_iter().take(25) {
        println!("  {count:5} {status:10} {detail}");
    }
    let _ = refutations;
    Ok(())
}

fn normalize_reason(detail: &str) -> String {
    static CLAUSE_ID: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    static NUMBER: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let clause_id = CLAUSE_ID.get_or_init(|| Regex::new(r"\bc\d+\b").unwrap());
    let number = NUMBER.get_or_init(|| Regex::new(r"\d+").unwrap());
    number
        .replace_all(&clause_id.replace_all(detail, "N"), "N")
        .chars()
        .take(100)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn details(processed: &str, digest: &str) -> BTreeMap<String, String> {
        BTreeMap::from([
            ("processed".into(), processed.into()),
            ("generated".into(), "200".into()),
            ("passive".into(), "15".into()),
            ("lrs_discarded".into(), "0".into()),
            ("weight_discarded".into(), "0".into()),
            ("trajectory_sha256".into(), digest.into()),
            ("resource_reason".into(), "max_processed".into()),
        ])
    }

    #[test]
    fn fixed_work_rejects_mismatched_selection_or_wrong_stop() {
        let good = details("100", "0:abc");
        assert!(equivalent_work(&good, &good, 100).is_ok());
        let mut different = good.clone();
        different.insert("trajectory_sha256".into(), "0:def".into());
        assert!(equivalent_work(&good, &different, 100).is_err());
        let mut wrong_stop = good.clone();
        wrong_stop.insert("resource_reason".into(), "memory".into());
        assert!(equivalent_work(&good, &wrong_stop, 100).is_err());
    }

    #[test]
    fn detail_parser_requires_one_structured_line() {
        let parsed = parse_search_detail(
            "noise\n% SZS detail processed=100 resource_detail=\"processed-clause limit reached\" resource_reason=max_processed\n",
        )
        .unwrap();
        assert_eq!(parsed.0["processed"], "100");
        assert_eq!(
            parsed.0["resource_detail"],
            "processed-clause limit reached"
        );
        assert!(parse_search_detail("no detail\n").is_err());
        assert!(
            parse_search_detail("% SZS detail processed=1\n% SZS detail processed=2\n").is_err()
        );
    }

    #[test]
    fn paired_delta_counts_paired_gains_and_rejects_different_sets() {
        let key = |problem: &str| AuditKey {
            edition: "casc-30".into(),
            division: "FEQ".into(),
            problem: problem.into(),
            system: "mrs".into(),
        };
        let baseline = BTreeMap::from([(key("A"), true), (key("B"), false), (key("C"), false)]);
        let candidate = BTreeMap::from([(key("A"), false), (key("B"), true), (key("C"), true)]);
        let delta = paired_delta(&baseline, &candidate).unwrap();
        assert_eq!(delta["gains"], 2);
        assert_eq!(delta["losses"], 1);
        assert_eq!(delta["delta"], 1);
        assert_eq!(delta["problems"], 3);
        assert!(paired_delta(&baseline, &BTreeMap::from([(key("other"), true)])).is_err());
    }

    #[test]
    fn certified_outcomes_keep_incomplete_rows_and_reject_bad_or_duplicate_rows() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("audit.csv");
        fs::write(
            &path,
            "edition,division,problem,system,strict_status\ncasc-30,FEQ,A,mrs,VerifiedGood\ncasc-30,FEQ,B,mrs,Unknown\ncasc-30,FEQ,C,mrs,proof_omitted\n",
        )
        .unwrap();
        let outcomes = certified_outcomes(&path).unwrap();
        assert_eq!(outcomes.len(), 3);
        assert_eq!(outcomes.values().filter(|&&value| value).count(), 1);

        fs::write(
            &path,
            "edition,division,problem,system,strict_status\ncasc-30,FEQ,A,mrs,VerifiedBad\n",
        )
        .unwrap();
        assert!(
            certified_outcomes(&path)
                .unwrap_err()
                .to_string()
                .contains("rejected proof")
        );

        fs::write(
            &path,
            "edition,division,problem,system,strict_status\ncasc-30,FEQ,A,mrs,Unknown\ncasc-30,FEQ,A,mrs,Unknown\n",
        )
        .unwrap();
        assert!(
            certified_outcomes(&path)
                .unwrap_err()
                .to_string()
                .contains("duplicate audit row")
        );
    }

    #[test]
    fn certification_dashboard_keeps_other_statuses_and_checks_digest() {
        let root = tempfile::tempdir().unwrap();
        let bytes = b"edition,division,problem,system,strict_status\ncasc-30,feq,A,mrs,VerifiedGood\ncasc-30,feq,B,mrs,Unknown\ncasc-30,feq,C,mrs,proof_omitted\n";
        let digest = hex_digest(bytes);
        let snapshot = root.path().join(digest);
        fs::create_dir(&snapshot).unwrap();
        fs::write(snapshot.join("audit.csv"), bytes).unwrap();
        fs::write(
            snapshot.join("audit-identity.json"),
            json!({
                "schema": 1,
                "audit_binary_sha256": "auditor",
                "proover_binary_sha256": "checker",
                "run_csv_sha256": "run",
                "corpus_sha256": "corpus",
                "strict_time": 30,
                "jobs": 1
            })
            .to_string(),
        )
        .unwrap();
        let report = render_dashboard(std::slice::from_ref(&snapshot)).unwrap();
        assert!(report.contains("Unknown: 1"));
        assert!(report.contains("proof_omitted: 1"));
        assert!(render_dashboard(&[snapshot.clone(), snapshot.clone()]).is_err());
        fs::write(snapshot.join("audit.csv"), b"forged").unwrap();
        assert!(render_dashboard(&[snapshot]).is_err());
    }

    #[test]
    fn run_metadata_hash_update_is_structured_json() {
        let directory = tempfile::tempdir().unwrap();
        let metadata = directory.path().join("run_meta.json");
        fs::write(&metadata, "{\"edition\":\"casc-30\"}\n").unwrap();
        record_binary_hash(RecordBinaryHashArgs {
            metadata: metadata.clone(),
            sha256: "abc123".into(),
        })
        .unwrap();
        let value: Value = serde_json::from_slice(&fs::read(metadata).unwrap()).unwrap();
        assert_eq!(value["edition"], "casc-30");
        assert_eq!(value["binary_sha256"], "abc123");
    }

    #[test]
    fn epr_telemetry_join_preserves_the_declared_columns() {
        let directory = tempfile::tempdir().unwrap();
        let raw = directory.path().join("run/raw/mrs/epu");
        fs::create_dir_all(&raw).unwrap();
        fs::write(
            raw.join("P001.p.stderr"),
            "epr_attempted=1 epr_route=ground epr_generated=3\n",
        )
        .unwrap();
        let run_csv = directory.path().join("run.csv");
        fs::write(&run_csv, "problem,szs_status\nP001.p,Unsatisfiable\n").unwrap();
        epr_telemetry(EprTelemetryArgs {
            run_csv,
            output_dir: directory.path().to_path_buf(),
        })
        .unwrap();
        let output = fs::read_to_string(directory.path().join("epr_telemetry.csv")).unwrap();
        assert!(output.starts_with("problem,szs_status,epr_attempted,epr_route,epr_generated"));
        assert!(output.contains("P001.p,Unsatisfiable,1,ground,3"));
    }

    #[test]
    fn canary_audit_uses_header_names_and_distinguishes_contamination() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("run.csv");
        let header = "edition,division,problem,system,timeout,szs_status,expected,verdict,wall_time_s,peak_memory_mb,failure_detail\n";
        let clean_rows = concat!(
            "casc-30,eps,HWC004-1,mrs,120,Unsatisfiable,Unsatisfiable,ok,0.1,10,lrs_discarded=1 passive=2\n",
            "casc-30,fne,CSR026+3,mrs,240,Theorem,Theorem,ok,0.002,10,processed=10 generated=20\n",
            "casc-30,feq,AGT005+1,mrs,240,Theorem,Theorem,ok,0.1,10,processed=10 generated=20\n",
            "casc-30,ueq,ALG212-10,mrs,240,Theorem,Theorem,ok,0.1,10,processed=10 generated=20\n",
        );
        fs::write(&path, format!("{header}{clean_rows}")).unwrap();
        assert!(
            check_canary_drift(CanaryDriftArgs {
                run_csv: path.clone(),
            })
            .is_ok()
        );
        fs::write(
            &path,
            format!(
                "{header}casc-30,feq,AGT005+1,mrs,240,Theorem,Theorem,ok,0.1,10,processed=1001 generated=20\n"
            ),
        )
        .unwrap();
        let error = check_canary_drift(CanaryDriftArgs { run_csv: path }).unwrap_err();
        assert!(error.downcast_ref::<CanaryContaminated>().is_some());
    }

    #[test]
    fn reason_normalization_keeps_word_boundaries() {
        assert_eq!(
            normalize_reason("clause c123 failed at 45"),
            "clause N failed at N"
        );
        assert_eq!(normalize_reason("abc123 failed"), "abcN failed");
    }
}
