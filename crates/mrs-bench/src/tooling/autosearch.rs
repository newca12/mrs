//! Bounded, family-split search over given-clause selection policies.
//!
//! Candidate selectors are data, not Rust code. Every search run uses one
//! explicitly selected base strategy, one worker, and the same resource/time
//! limits. Search refutations count during training only when `mrs --self-check`
//! certifies them; the final family holdout is additionally replayed by
//! `mrs-proover --strict`.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use clap::Args;
use rand::SeedableRng;
use rand::prelude::IndexedRandom;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use wait_timeout::ChildExt;

use super::corpus::{self, FileEntry, Manifest, ProblemEntry};

type ToolResult<T> = Result<T, Box<dyn Error>>;

const MAX_CANDIDATES: usize = 500;
const MAX_HOLDOUT_REPEATS: usize = 10;
const PROOF_BYTES_LIMIT: usize = 32 * 1024 * 1024;
const ADDRESS_SPACE_HEADROOM_MB: u64 = 512;
const CHILD_TIMEOUT_MARGIN_SECS: u64 = 30;

#[derive(Args, Debug)]
pub struct AutosearchArgs {
    /// Directory of one fixed problem corpus, normally a single division.
    #[arg(long)]
    corpus_dir: PathBuf,
    /// New result directory, or an existing result directory with --resume.
    #[arg(long)]
    output: PathBuf,
    /// TPTP library root used to resolve standard includes (defaults to $TPTP).
    #[arg(long)]
    tptp_root: Option<PathBuf>,
    /// Prover executable.
    #[arg(long, default_value = "target/release/mrs")]
    mrs: PathBuf,
    /// Strict proof auditor executable.
    #[arg(long, default_value = "target/release/mrs-proover")]
    proover: PathBuf,
    /// Named division schedule supplying the non-selection settings.
    #[arg(long, default_value = "casc_fne")]
    schedule: String,
    /// One base strategy whose other settings stay fixed throughout the search.
    #[arg(long, default_value_t = 12)]
    strategy: usize,
    /// Per-problem search deadline in seconds (minimum 4; includes self-check).
    #[arg(long, default_value_t = 5)]
    time: u64,
    /// Maximum number of development problems, sampled round-robin by family;
    /// 0 uses every development problem.
    #[arg(long, default_value_t = 24)]
    train_sample: usize,
    /// Total number of candidate selectors to evaluate.
    #[arg(long, default_value_t = 40)]
    max_candidates: usize,
    /// Number of mutated candidates proposed after each initial/evolution batch.
    #[arg(long, default_value_t = 8)]
    batch_size: usize,
    /// Parallel prover processes. Each process runs one search worker.
    #[arg(long, default_value_t = 1)]
    jobs: usize,
    /// Per-process RSS watchdog budget in MiB; Linux also applies an address-space cap.
    #[arg(long, default_value_t = 2048)]
    memory_mb: u64,
    /// Percentage of problem families reserved for final evaluation.
    #[arg(long, default_value_t = 25)]
    holdout_percent: u8,
    /// Randomized baseline/champion pairs per held-out problem (minimum 2).
    #[arg(long, default_value_t = 2)]
    holdout_repeats: usize,
    /// Stable family split, sampling, and mutation seed.
    #[arg(long, default_value_t = 42)]
    seed: u64,
    /// Baseline selector compared with the search winner (default: age:5).
    #[arg(long, default_value = "age:5")]
    baseline_selection: String,
    /// Optional model-proposed selectors, one DSL expression per line. This
    /// accepts data only; the tool never compiles or executes proposal code.
    #[arg(long)]
    seed_candidates: Option<PathBuf>,
    /// Continue an interrupted run only when its frozen identity matches.
    #[arg(long, default_value_t = false)]
    resume: bool,
    /// Maximum strict-audit deadline for each retained holdout proof.
    #[arg(long, default_value_t = 30)]
    audit_time: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum QueueKind {
    Age,
    Weight,
    Goal,
    Unit,
    Horn,
    Sos,
}

impl QueueKind {
    fn name(self) -> &'static str {
        match self {
            Self::Age => "age",
            Self::Weight => "weight",
            Self::Goal => "goal",
            Self::Unit => "unit",
            Self::Horn => "horn",
            Self::Sos => "sos",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Candidate {
    Fifo,
    Smallest,
    Age(u32),
    Goal(u32),
    Multi(Vec<(QueueKind, u32)>),
}

impl Candidate {
    fn parse(value: &str) -> Result<Self, String> {
        let selection = mrs_search::select::parse_selection_strategy(value)?;
        match selection {
            mrs_search::SelectionStrategy::Fifo => Ok(Self::Fifo),
            mrs_search::SelectionStrategy::SmallestFirst => Ok(Self::Smallest),
            mrs_search::SelectionStrategy::AgeWeight(ratio) => Ok(Self::Age(ratio)),
            mrs_search::SelectionStrategy::GoalDirected(ratio) => Ok(Self::Goal(ratio)),
            mrs_search::SelectionStrategy::MultiQueue(queues) => Ok(Self::Multi(
                queues
                    .into_iter()
                    .map(|(queue, weight)| {
                        let queue = match queue {
                            mrs_search::QueueType::Age => QueueKind::Age,
                            mrs_search::QueueType::Weight => QueueKind::Weight,
                            mrs_search::QueueType::Goal => QueueKind::Goal,
                            mrs_search::QueueType::Unit => QueueKind::Unit,
                            mrs_search::QueueType::Horn => QueueKind::Horn,
                            mrs_search::QueueType::Sos => QueueKind::Sos,
                        };
                        (queue, weight)
                    })
                    .collect(),
            )),
            mrs_search::SelectionStrategy::MlGuided { .. } => {
                Err("ML-guided selection is not part of the autosearch DSL".into())
            }
        }
    }

    fn expression(&self) -> String {
        match self {
            Self::Fifo => "fifo".into(),
            Self::Smallest => "smallest".into(),
            Self::Age(ratio) => format!("age:{ratio}"),
            Self::Goal(ratio) => format!("goal:{ratio}"),
            Self::Multi(queues) => format!(
                "queue:{}",
                queues
                    .iter()
                    .map(|(queue, frequency)| format!("{}={frequency}", queue.name()))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }

    fn mutate(&self, rng: &mut StdRng) -> Self {
        const RATIOS: &[u32] = &[1, 2, 3, 4, 5, 6, 8, 10, 12, 16, 20, 24, 32, 48];
        const FREQUENCIES: &[u32] = &[1, 2, 3, 4, 6, 8];
        const SIMPLE_MUTATIONS: &[u8] = &[0, 1, 2];
        const QUEUE_MUTATIONS: &[u8] = &[0, 1, 2, 3];
        const QUEUE_KINDS: &[QueueKind] = &[
            QueueKind::Age,
            QueueKind::Weight,
            QueueKind::Goal,
            QueueKind::Unit,
            QueueKind::Horn,
            QueueKind::Sos,
        ];

        let choose_ratio = |rng: &mut StdRng, old: Option<u32>| -> u32 {
            RATIOS
                .iter()
                .copied()
                .filter(|ratio| Some(*ratio) != old)
                .collect::<Vec<_>>()
                .choose(rng)
                .copied()
                .unwrap_or(5)
        };
        let choose_frequency = |rng: &mut StdRng, old: Option<u32>| -> u32 {
            FREQUENCIES
                .iter()
                .copied()
                .filter(|frequency| Some(*frequency) != old)
                .collect::<Vec<_>>()
                .choose(rng)
                .copied()
                .unwrap_or(1)
        };

        match self {
            Self::Fifo | Self::Smallest | Self::Age(_) | Self::Goal(_) => {
                match SIMPLE_MUTATIONS.choose(rng).copied().unwrap_or(0) {
                    0 => {
                        let old = match self {
                            Self::Age(ratio) | Self::Goal(ratio) => Some(*ratio),
                            _ => None,
                        };
                        Self::Age(choose_ratio(rng, old))
                    }
                    1 => {
                        let old = match self {
                            Self::Age(ratio) | Self::Goal(ratio) => Some(*ratio),
                            _ => None,
                        };
                        Self::Goal(choose_ratio(rng, old))
                    }
                    _ => {
                        let weight = choose_frequency(rng, None);
                        Self::Multi(vec![(QueueKind::Age, 1), (QueueKind::Weight, weight)])
                    }
                }
            }
            Self::Multi(original) => {
                let mut queues = original.clone();
                match QUEUE_MUTATIONS.choose(rng).copied().unwrap_or(0) {
                    0 => {
                        if let Some(index) = (0..queues.len()).collect::<Vec<_>>().choose(rng) {
                            let (_, frequency) = &mut queues[*index];
                            *frequency = choose_frequency(rng, Some(*frequency));
                        }
                    }
                    1 if queues.len() < QUEUE_KINDS.len() => {
                        let missing = QUEUE_KINDS
                            .iter()
                            .copied()
                            .filter(|kind| !queues.iter().any(|(present, _)| present == kind))
                            .collect::<Vec<_>>();
                        if let Some(kind) = missing.choose(rng).copied() {
                            let frequency = choose_frequency(rng, None);
                            let position = (0..=queues.len())
                                .collect::<Vec<_>>()
                                .choose(rng)
                                .copied()
                                .unwrap_or(queues.len());
                            queues.insert(position, (kind, frequency));
                        }
                    }
                    2 if queues.len() > 1 => {
                        let position = (0..queues.len()).collect::<Vec<_>>().choose(rng).copied();
                        if let Some(position) = position {
                            queues.remove(position);
                        }
                    }
                    _ if queues.len() > 1 => {
                        let mut order = (0..queues.len()).collect::<Vec<_>>();
                        order.shuffle(rng);
                        queues = order.into_iter().map(|index| queues[index]).collect();
                    }
                    _ => {
                        let kind = QUEUE_KINDS
                            .iter()
                            .copied()
                            .find(|kind| !queues.iter().any(|(present, _)| present == kind));
                        if let Some(kind) = kind {
                            queues.push((kind, choose_frequency(rng, None)));
                        }
                    }
                }
                Self::Multi(queues)
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CandidatePlan {
    id: usize,
    generation: usize,
    selector: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct HoldoutPairPlan {
    id: usize,
    repeat: usize,
    problem: String,
    family: String,
    first_arm: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RunPhase {
    Training,
    Holdout,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RunRecord {
    phase: RunPhase,
    candidate_id: Option<usize>,
    pair_id: Option<usize>,
    repeat: Option<usize>,
    arm: String,
    selector: String,
    problem: String,
    family: String,
    status: String,
    self_check: String,
    certified: bool,
    strict_verdict: Option<String>,
    elapsed_ms: u64,
    processed: u64,
    generated: u64,
    exit_code: Option<i32>,
    proof_path: Option<String>,
    failure: Option<String>,
}

impl RunRecord {
    fn key(&self) -> String {
        match self.phase {
            RunPhase::Training => format!(
                "train:{}:{}",
                self.candidate_id.unwrap_or(usize::MAX),
                self.problem
            ),
            RunPhase::Holdout => format!(
                "holdout:{}:{}",
                self.pair_id.unwrap_or(usize::MAX),
                self.arm
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StatePhase {
    Training,
    Holdout,
    Complete,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RunState {
    schema: u8,
    identity_sha256: String,
    phase: StatePhase,
    candidates: Vec<CandidatePlan>,
    champion: Option<String>,
    holdout_pairs: Vec<HoldoutPairPlan>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CandidateSummary {
    id: usize,
    generation: usize,
    selector: String,
    problems: usize,
    certified: usize,
    median_certified_wall_ms: Option<u64>,
    resource_outs: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct HoldoutPairResult {
    pair_id: usize,
    repeat: usize,
    problem: String,
    family: String,
    baseline_selector: String,
    candidate_selector: String,
    baseline_status: String,
    candidate_status: String,
    baseline_verdict: String,
    candidate_verdict: String,
    baseline_verified: bool,
    candidate_verified: bool,
    delta: i8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ExperimentIdentity {
    schema: u8,
    corpus_sha256: String,
    corpus_root: String,
    output_root: String,
    tptp_root: String,
    tool_binary_sha256: String,
    mrs_binary_sha256: String,
    proover_binary_sha256: String,
    git_commit: String,
    schedule: String,
    strategy: usize,
    time_secs: u64,
    audit_time: u64,
    train_sample: usize,
    max_candidates: usize,
    batch_size: usize,
    jobs: usize,
    memory_mb: u64,
    holdout_percent: u8,
    holdout_repeats: usize,
    seed: u64,
    baseline_selector: String,
    seed_candidates: Vec<String>,
    training_problems: Vec<String>,
    holdout_problems: Vec<String>,
    cpu_model: String,
    physical_cores: usize,
    rustc_version: String,
}

struct RunStore {
    records: Mutex<HashMap<String, RunRecord>>,
    writer: Mutex<BufWriter<File>>,
}

impl RunStore {
    fn open(path: &Path) -> ToolResult<Self> {
        repair_trailing_jsonl(path)?;
        let mut records = HashMap::new();
        if path.exists() {
            for line in BufReader::new(File::open(path)?).lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                let record: RunRecord = serde_json::from_str(&line)?;
                records.insert(record.key(), record);
            }
        }
        let writer = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            records: Mutex::new(records),
            writer: Mutex::new(BufWriter::new(writer)),
        })
    }

    fn get(&self, key: &str) -> Option<RunRecord> {
        self.records.lock().ok()?.get(key).cloned()
    }

    fn contains(&self, key: &str) -> bool {
        self.records
            .lock()
            .is_ok_and(|records| records.contains_key(key))
    }

    fn put(&self, record: RunRecord) -> ToolResult<()> {
        let line = serde_json::to_string(&record)?;
        {
            let mut writer = self
                .writer
                .lock()
                .map_err(|_| "autosearch run log lock was poisoned")?;
            writeln!(writer, "{line}")?;
            writer.flush()?;
        }
        self.records
            .lock()
            .map_err(|_| "autosearch result index lock was poisoned")?
            .insert(record.key(), record);
        Ok(())
    }
}

pub fn run(args: AutosearchArgs) -> ToolResult<()> {
    validate_args(&args)?;
    let corpus_root = args.corpus_dir.canonicalize()?;
    if !corpus_root.is_dir() {
        return Err(format!("corpus is not a directory: {}", corpus_root.display()).into());
    }
    let tptp_root = resolve_tptp_root(args.tptp_root.as_deref())?;
    let mrs = args.mrs.canonicalize()?;
    let proover = args.proover.canonicalize()?;
    if !mrs.is_file() || !proover.is_file() {
        return Err("--mrs and --proover must name executable files".into());
    }
    let output = resolve_output(&args.output)?;
    if output == corpus_root || output.starts_with(&corpus_root) {
        return Err("--output must be outside --corpus-dir".into());
    }

    if args.resume {
        if !output.is_dir() {
            return Err(format!(
                "--resume requires an existing output directory: {}",
                output.display()
            )
            .into());
        }
    } else if output.exists() {
        return Err(format!(
            "output already exists (use a new path or --resume): {}",
            output.display()
        )
        .into());
    }

    let corpus_manifest = corpus::freeze(&corpus_root, args.seed, args.holdout_percent, None)?;
    let mut development = corpus_manifest
        .problems
        .iter()
        .filter(|problem| problem.split == "development")
        .cloned()
        .collect::<Vec<_>>();
    let mut holdout = corpus_manifest
        .problems
        .iter()
        .filter(|problem| problem.split == "holdout")
        .cloned()
        .collect::<Vec<_>>();
    development.sort_by(|left, right| left.path.cmp(&right.path));
    holdout.sort_by(|left, right| left.path.cmp(&right.path));
    let training = stratified_sample(&development, args.train_sample, args.seed);
    if training.is_empty() || holdout.is_empty() {
        return Err("the family split must have non-empty development and holdout sets".into());
    }

    let baseline = Candidate::parse(&args.baseline_selection)
        .map_err(|error| format!("invalid --baseline-selection: {error}"))?
        .expression();
    let seed_candidates = read_seed_candidates(args.seed_candidates.as_deref())?;
    let physical_cores = num_cpus::get_physical().max(1);
    let available_memory_mb = available_memory_mb();
    check_resource_budget(
        args.jobs,
        args.memory_mb,
        physical_cores,
        available_memory_mb,
    )?;
    let maximum_searches = args.max_candidates as u128 * training.len() as u128
        + 2 * holdout.len() as u128 * args.holdout_repeats as u128;
    let maximum_audits = 2 * holdout.len() as u128 * args.holdout_repeats as u128;
    let estimated_seconds = (maximum_searches * args.time as u128
        + maximum_audits * args.audit_time as u128)
        / args.jobs as u128;
    eprintln!(
        "[autosearch] host: {physical_cores} physical cores, available_memory_mb={available_memory_mb:?}; caps: jobs={}, memory_mb={}; worst-case budget ~{:.1} h",
        args.jobs,
        args.memory_mb,
        estimated_seconds as f64 / 3600.0,
    );

    if args.resume {
        fs::create_dir_all(output.join("work"))?;
        fs::create_dir_all(output.join("logs"))?;
        fs::create_dir_all(output.join("proofs"))?;
    } else {
        fs::create_dir(&output)?;
        fs::create_dir(output.join("work"))?;
        fs::create_dir(output.join("logs"))?;
        fs::create_dir(output.join("proofs"))?;
    }
    let _output_lock = lock_output(&output)?;
    let snapshot_root = output.join("corpus");
    prepare_corpus_snapshot(&corpus_root, &corpus_manifest, &snapshot_root, args.resume)?;

    let identity = ExperimentIdentity {
        schema: 1,
        corpus_sha256: corpus_manifest.corpus_sha256.clone(),
        corpus_root: corpus_root.display().to_string(),
        output_root: output.display().to_string(),
        tptp_root: tptp_root
            .as_ref()
            .map_or_else(String::new, |path| path.display().to_string()),
        tool_binary_sha256: sha256_file(&std::env::current_exe()?)?,
        mrs_binary_sha256: sha256_file(&mrs)?,
        proover_binary_sha256: sha256_file(&proover)?,
        git_commit: git_commit()?,
        schedule: args.schedule.clone(),
        strategy: args.strategy,
        time_secs: args.time,
        audit_time: args.audit_time,
        train_sample: args.train_sample,
        max_candidates: args.max_candidates,
        batch_size: args.batch_size,
        jobs: args.jobs,
        memory_mb: args.memory_mb,
        holdout_percent: args.holdout_percent,
        holdout_repeats: args.holdout_repeats,
        seed: args.seed,
        baseline_selector: baseline.clone(),
        seed_candidates: seed_candidates.clone(),
        training_problems: training
            .iter()
            .map(|problem| problem.path.clone())
            .collect(),
        holdout_problems: holdout.iter().map(|problem| problem.path.clone()).collect(),
        cpu_model: cpu_model().unwrap_or_else(|| "unknown".into()),
        physical_cores,
        rustc_version: rustc_version(),
    };
    let identity_value = serde_json::to_value(&identity)?;
    let identity_sha256 = sha256_bytes(&serde_json::to_vec(&identity_value)?);

    if args.resume {
        let existing: Value = serde_json::from_slice(&fs::read(output.join("experiment.json"))?)?;
        if existing.get("identity") != Some(&identity_value) {
            return Err(
                "resume identity differs (binary, corpus, split, or run options changed)".into(),
            );
        }
        if !output.join("state.json").is_file() {
            return Err("resume state.json is missing".into());
        }
    } else {
        write_json_new(
            &output.join("experiment.json"),
            &json!({
                "identity": identity,
                "corpus": corpus_manifest,
                "corpus_snapshot": "corpus/",
                "hardware": {
                    "cpu_model": cpu_model().unwrap_or_else(|| "unknown".into()),
                    "physical_cores": physical_cores,
                    "available_memory_mb_at_start": available_memory_mb,
                },
                "method": "family-disjoint stochastic search over selection DSL; training counts require mrs self-check; holdout counts require strict mrs-proover audit",
                "promotion": "local experiment only; no schedule or default promotion",
            }),
        )?;
    }

    let mut state = if args.resume {
        serde_json::from_slice::<RunState>(&fs::read(output.join("state.json"))?)?
    } else {
        let initial = initial_candidates(&baseline, &seed_candidates, args.max_candidates);
        let candidates = initial
            .into_iter()
            .enumerate()
            .map(|(id, selector)| CandidatePlan {
                id,
                generation: 0,
                selector,
            })
            .collect();
        RunState {
            schema: 1,
            identity_sha256: identity_sha256.clone(),
            phase: StatePhase::Training,
            candidates,
            champion: None,
            holdout_pairs: Vec::new(),
        }
    };
    if state.schema != 1 || state.identity_sha256 != identity_sha256 {
        return Err("resume state identity does not match experiment.json".into());
    }
    let state_path = output.join("state.json");
    write_json_atomic(&state_path, &state)?;

    let store = RunStore::open(&output.join("runs.jsonl"))?;
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(args.jobs)
        .build()?;
    let context = RunContext {
        args: &args,
        mrs: &mrs,
        proover: &proover,
        corpus_root: &snapshot_root,
        tptp_root: tptp_root.as_deref(),
        output: &output,
        store: &store,
    };

    while matches!(state.phase, StatePhase::Training) {
        run_pending_training(&pool, &context, &state.candidates, &training)?;
        let summaries = summarize_candidates(&state.candidates, &training, &store);
        write_json_atomic(&output.join("candidate-scores.json"), &summaries)?;
        let generation = state
            .candidates
            .iter()
            .map(|candidate| candidate.generation)
            .max()
            .unwrap_or(0);
        let current = state
            .candidates
            .iter()
            .filter(|candidate| candidate.generation == generation)
            .collect::<Vec<_>>();
        if current
            .iter()
            .any(|candidate| !candidate_complete(candidate, &training, &store))
        {
            return Err("candidate generation did not produce a complete training matrix".into());
        }

        if state.candidates.len() >= args.max_candidates {
            finish_training(&mut state, &summaries, &holdout, &args)?;
        } else {
            let next = next_generation(
                &state.candidates,
                &summaries,
                args.max_candidates,
                args.batch_size,
                args.seed,
            );
            if next.is_empty() {
                finish_training(&mut state, &summaries, &holdout, &args)?;
            } else {
                state.candidates.extend(next);
            }
        }
        write_json_atomic(&state_path, &state)?;
        eprintln!(
            "[autosearch] completed {} / {} candidates; phase={:?}; train={}/{} holdout={}",
            state
                .candidates
                .iter()
                .filter(|c| candidate_complete(c, &training, &store))
                .count(),
            args.max_candidates,
            state.phase,
            training.len(),
            development.len(),
            holdout.len(),
        );
    }

    if matches!(state.phase, StatePhase::Holdout) {
        run_holdout(&pool, &context, &state, &holdout, &baseline)?;
        let summaries = summarize_candidates(&state.candidates, &training, &store);
        let best = state
            .champion
            .as_deref()
            .ok_or("holdout phase has no selected champion")?;
        let pair_results = summarize_holdout(&state.holdout_pairs, &store, &baseline, best);
        let deltas = pair_results
            .iter()
            .map(|pair| i64::from(pair.delta))
            .collect::<Vec<_>>();
        let mean_delta = if deltas.is_empty() {
            0.0
        } else {
            deltas.iter().sum::<i64>() as f64 / deltas.len() as f64
        };
        let report = json!({
            "schema": 1,
            "baseline_selector": baseline,
            "best_training_selector": best,
            "training_candidates": summaries,
            "holdout_pairs": pair_results,
            "holdout": {
                "pairs": deltas.len(),
                "mean_paired_delta": mean_delta,
                "observed_delta_range": [deltas.iter().min(), deltas.iter().max()],
                "gains": deltas.iter().filter(|delta| **delta > 0).count(),
                "losses": deltas.iter().filter(|delta| **delta < 0).count(),
                "ties": deltas.iter().filter(|delta| **delta == 0).count(),
            },
            "interpretation": "descriptive local evidence on family-held-out problems; repeated pairs are a screen, not proof of low variance or CASC performance",
        });
        write_json_atomic(&output.join("summary.json"), &report)?;
        state.phase = StatePhase::Complete;
        write_json_atomic(&state_path, &state)?;
    }

    if matches!(state.phase, StatePhase::Complete) {
        println!("autosearch complete: {}", output.display());
        println!(
            "  champion: {}",
            state.champion.as_deref().unwrap_or("unknown")
        );
        println!("  summary: {}", output.join("summary.json").display());
    }
    Ok(())
}

fn validate_args(args: &AutosearchArgs) -> ToolResult<()> {
    if !(1..=15).contains(&args.strategy) {
        return Err("--strategy must be in 1..=15".into());
    }
    if args.time < 4 {
        return Err("--time must be at least 4 seconds for strict self-check runs".into());
    }
    if !(1..=MAX_CANDIDATES).contains(&args.max_candidates) {
        return Err(format!("--max-candidates must be in 1..={MAX_CANDIDATES}").into());
    }
    if args.batch_size == 0 {
        return Err("--batch-size must be positive".into());
    }
    if !(2..=MAX_HOLDOUT_REPEATS).contains(&args.holdout_repeats) {
        return Err(format!("--holdout-repeats must be in 2..={MAX_HOLDOUT_REPEATS}").into());
    }
    if !(1..100).contains(&args.holdout_percent) {
        return Err("--holdout-percent must be in 1..=99".into());
    }
    if args.jobs == 0 {
        return Err("--jobs must be positive".into());
    }
    if !(512..=8192).contains(&args.memory_mb) {
        return Err("--memory-mb must be in 512..=8192".into());
    }
    if args.audit_time == 0 {
        return Err("--audit-time must be positive".into());
    }
    Ok(())
}

fn resolve_tptp_root(explicit: Option<&Path>) -> ToolResult<Option<PathBuf>> {
    let candidate = explicit
        .map(Path::to_path_buf)
        .or_else(|| std::env::var_os("TPTP").map(PathBuf::from))
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join("TPTP-v10.0.0"))
                .filter(|path| path.is_dir())
        });
    candidate
        .map(|path| {
            let root = path.canonicalize()?;
            if !root.is_dir() {
                return Err(format!("TPTP root is not a directory: {}", root.display()).into());
            }
            Ok(root)
        })
        .transpose()
}

fn resolve_output(path: &Path) -> ToolResult<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    if absolute.exists() {
        return Ok(absolute.canonicalize()?);
    }
    let parent = absolute
        .parent()
        .ok_or_else(|| format!("output has no parent: {}", path.display()))?
        .canonicalize()?;
    let name = absolute
        .file_name()
        .ok_or_else(|| format!("output has no final component: {}", path.display()))?;
    Ok(parent.join(name))
}

fn read_seed_candidates(path: Option<&Path>) -> ToolResult<Vec<String>> {
    let Some(path) = path else {
        return Ok(Vec::new());
    };
    let text = fs::read_to_string(path)?;
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let selector = Candidate::parse(line)
            .map_err(|error| format!("{}:{}: {error}", path.display(), index + 1))?
            .expression();
        if seen.insert(selector.clone()) {
            result.push(selector);
        }
    }
    Ok(result)
}

fn initial_candidates(baseline: &str, proposed: &[String], limit: usize) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    let mut add = |value: String| {
        if candidates.len() < limit && seen.insert(value.clone()) {
            candidates.push(value);
        }
    };
    add(baseline.to_owned());
    for selector in proposed {
        add(selector.clone());
    }
    for selector in [
        "fifo",
        "smallest",
        "age:2",
        "age:3",
        "age:4",
        "age:5",
        "age:6",
        "age:8",
        "age:12",
        "age:20",
        "goal:3",
        "goal:8",
        "goal:16",
        "queue:age=1,weight=2,goal=1",
        "queue:age=1,weight=2,horn=1",
        "queue:age=1,weight=2,unit=1",
        "queue:age=1,weight=2,sos=1",
        "queue:weight=2,age=1,goal=1",
    ] {
        add(selector.to_owned());
    }
    candidates
}

fn stratified_sample(problems: &[ProblemEntry], limit: usize, seed: u64) -> Vec<ProblemEntry> {
    let mut families = BTreeMap::<String, Vec<ProblemEntry>>::new();
    for problem in problems {
        families
            .entry(problem.family.clone())
            .or_default()
            .push(problem.clone());
    }
    let mut rng = StdRng::seed_from_u64(seed ^ 0x6d72_732d_6175_746f);
    for family in families.values_mut() {
        family.shuffle(&mut rng);
    }
    let mut family_order = families.keys().cloned().collect::<Vec<_>>();
    family_order.shuffle(&mut rng);
    let count = if limit == 0 {
        problems.len()
    } else {
        limit.min(problems.len())
    };
    let mut selected = Vec::with_capacity(count);
    let mut depth = 0;
    while selected.len() < count {
        let mut found = false;
        for family_name in &family_order {
            if let Some(problem) = families
                .get(family_name)
                .and_then(|family| family.get(depth))
            {
                selected.push(problem.clone());
                found = true;
                if selected.len() == count {
                    break;
                }
            }
        }
        if !found {
            break;
        }
        depth += 1;
    }
    selected.sort_by(|left, right| left.path.cmp(&right.path));
    selected
}

fn check_resource_budget(
    jobs: usize,
    memory_mb: u64,
    physical_cores: usize,
    available_memory_mb: Option<u64>,
) -> ToolResult<()> {
    let max_jobs = (physical_cores / 2).max(1);
    if jobs > max_jobs {
        return Err(format!(
            "--jobs ({jobs}) exceeds the local CPU budget of {max_jobs} processes for {physical_cores} physical cores; each self-checked run may overlap one search and one certificate worker"
        )
        .into());
    }
    if let Some(available) = available_memory_mb {
        // A self-checked prover may overlap its same-binary checker. Their
        // address spaces/RSS are separate; budget both rather than pretending
        // that the parent's watchdog includes the child.
        let per_job = memory_mb
            .saturating_add(ADDRESS_SPACE_HEADROOM_MB)
            .saturating_mul(2);
        let aggregate = per_job.saturating_mul(jobs as u64);
        if aggregate > available.saturating_mul(3) / 4 {
            return Err(format!(
                "requested job caps need about {aggregate} MiB including address-space headroom, over 75% of available {available} MiB; reduce --jobs or --memory-mb"
            )
            .into());
        }
    }
    Ok(())
}

fn available_memory_mb() -> Option<u64> {
    let host = fs::read_to_string("/proc/meminfo").ok().and_then(|text| {
        text.lines().find_map(|line| {
            let value = line
                .strip_prefix("MemAvailable:")?
                .split_whitespace()
                .next()?;
            value.parse::<u64>().ok().map(|kb| kb / 1024)
        })
    });
    match (host, cgroup_memory_headroom_mb()) {
        (Some(host), Some(cgroup)) => Some(host.min(cgroup)),
        (Some(host), None) => Some(host),
        (None, Some(cgroup)) => Some(cgroup),
        (None, None) => None,
    }
}

fn cgroup_memory_headroom_mb() -> Option<u64> {
    if let Ok(groups) = fs::read_to_string("/proc/self/cgroup") {
        for line in groups.lines() {
            let mut fields = line.splitn(3, ':');
            let Some(hierarchy) = fields.next() else {
                continue;
            };
            let Some(controllers) = fields.next() else {
                continue;
            };
            let Some(group) = fields.next() else {
                continue;
            };
            let relative = group.trim_start_matches('/');
            if hierarchy == "0" && controllers.is_empty() {
                let base = Path::new("/sys/fs/cgroup").join(relative);
                if let Some(headroom) = cgroup_v2_headroom(&base) {
                    return Some(headroom);
                }
            } else if controllers
                .split(',')
                .any(|controller| controller == "memory")
            {
                let base = Path::new("/sys/fs/cgroup/memory").join(relative);
                if let Some(headroom) = cgroup_v1_headroom(&base) {
                    return Some(headroom);
                }
            }
        }
    }
    cgroup_v2_headroom(Path::new("/sys/fs/cgroup"))
        .or_else(|| cgroup_v1_headroom(Path::new("/sys/fs/cgroup/memory")))
}

fn cgroup_v2_headroom(base: &Path) -> Option<u64> {
    let limit = fs::read_to_string(base.join("memory.max")).ok()?;
    let current = fs::read_to_string(base.join("memory.current")).ok()?;
    parse_cgroup_headroom(&limit, &current)
}

fn cgroup_v1_headroom(base: &Path) -> Option<u64> {
    let limit = fs::read_to_string(base.join("memory.limit_in_bytes")).ok()?;
    let current = fs::read_to_string(base.join("memory.usage_in_bytes")).ok()?;
    parse_cgroup_headroom(&limit, &current)
}

fn parse_cgroup_headroom(limit: &str, current: &str) -> Option<u64> {
    let limit = limit.trim().parse::<u64>().ok()?;
    let current = current.trim().parse::<u64>().ok()?;
    if limit >= (1u64 << 60) {
        return None;
    }
    Some(limit.saturating_sub(current) / (1024 * 1024))
}

fn prepare_corpus_snapshot(
    source: &Path,
    manifest: &Manifest,
    snapshot: &Path,
    resume: bool,
) -> ToolResult<()> {
    if resume {
        if !snapshot.is_dir() {
            return Err("resume corpus snapshot is missing".into());
        }
    } else {
        fs::create_dir(snapshot)?;
        for entry in &manifest.files {
            let relative = Path::new(&entry.path);
            if relative.is_absolute()
                || relative
                    .components()
                    .any(|component| !matches!(component, std::path::Component::Normal(_)))
            {
                return Err(format!("unsafe path in corpus manifest: {}", entry.path).into());
            }
            let destination = snapshot.join(relative);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(source.join(relative), &destination)?;
        }
    }

    for FileEntry { path, sha256, .. } in &manifest.files {
        let relative = Path::new(path.as_str());
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, std::path::Component::Normal(_)))
        {
            return Err(format!("unsafe path in corpus manifest: {path}").into());
        }
        let path = snapshot.join(relative);
        if !path.is_file() || sha256_file(&path)? != *sha256 {
            return Err(
                format!("corpus snapshot failed its hash check: {}", path.display()).into(),
            );
        }
    }
    Ok(())
}

fn cpu_model() -> Option<String> {
    let text = fs::read_to_string("/proc/cpuinfo").ok()?;
    text.lines().find_map(|line| {
        let (_, value) = line.split_once(':')?;
        (line.starts_with("model name") || line.starts_with("Hardware"))
            .then(|| value.trim().to_owned())
    })
}

fn rustc_version() -> String {
    Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|version| !version.is_empty())
        .unwrap_or_else(|| "unavailable".into())
}

fn finish_training(
    state: &mut RunState,
    summaries: &[CandidateSummary],
    holdout: &[ProblemEntry],
    args: &AutosearchArgs,
) -> ToolResult<()> {
    let champion = summaries
        .iter()
        .min_by(|left, right| compare_summary(left, right))
        .ok_or("no complete training candidate was evaluated")?;
    state.champion = Some(champion.selector.clone());
    state.holdout_pairs = make_holdout_pairs(holdout, args.holdout_repeats, args.seed);
    state.phase = StatePhase::Holdout;
    Ok(())
}

fn next_generation(
    candidates: &[CandidatePlan],
    summaries: &[CandidateSummary],
    max_candidates: usize,
    batch_size: usize,
    seed: u64,
) -> Vec<CandidatePlan> {
    if candidates.len() >= max_candidates || summaries.is_empty() {
        return Vec::new();
    }
    let mut ranked = summaries.to_vec();
    ranked.sort_by(compare_summary);
    let elite_count = ranked.len().min(8);
    let elites = &ranked[..elite_count];
    let current_generation = candidates
        .iter()
        .map(|candidate| candidate.generation)
        .max()
        .unwrap_or(0);
    let generation = current_generation + 1;
    let remaining = max_candidates - candidates.len();
    let desired = batch_size.min(remaining);
    let mut seen = candidates
        .iter()
        .map(|candidate| candidate.selector.clone())
        .collect::<HashSet<_>>();
    let mut next = Vec::with_capacity(desired);

    for attempt in 0..desired.saturating_mul(100).max(desired) {
        if next.len() == desired {
            break;
        }
        let id = candidates.len() + next.len();
        let mut rng = StdRng::seed_from_u64(
            seed ^ (id as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ attempt as u64,
        );
        let Some(parent) = elites.choose(&mut rng) else {
            break;
        };
        let Ok(parsed) = Candidate::parse(&parent.selector) else {
            continue;
        };
        let child = parsed.mutate(&mut rng).expression();
        let Ok(child) = Candidate::parse(&child) else {
            continue;
        };
        let child = child.expression();
        if seen.insert(child.clone()) {
            next.push(CandidatePlan {
                id,
                generation,
                selector: child,
            });
        }
    }
    next
}

fn compare_summary(left: &CandidateSummary, right: &CandidateSummary) -> Ordering {
    right
        .certified
        .cmp(&left.certified)
        .then_with(|| {
            left.median_certified_wall_ms
                .unwrap_or(u64::MAX)
                .cmp(&right.median_certified_wall_ms.unwrap_or(u64::MAX))
        })
        .then_with(|| left.resource_outs.cmp(&right.resource_outs))
        .then_with(|| left.id.cmp(&right.id))
}

fn summarize_candidates(
    candidates: &[CandidatePlan],
    problems: &[ProblemEntry],
    store: &RunStore,
) -> Vec<CandidateSummary> {
    let mut summaries = candidates
        .iter()
        .filter_map(|candidate| {
            if !candidate_complete(candidate, problems, store) {
                return None;
            }
            let mut wall_times = Vec::new();
            let mut certified = 0;
            let mut resource_outs = 0;
            for problem in problems {
                let key = format!("train:{}:{}", candidate.id, problem.path);
                let Some(record) = store.get(&key) else {
                    continue;
                };
                if record.certified {
                    certified += 1;
                    wall_times.push(record.elapsed_ms);
                }
                if record.status == "ResourceOut" {
                    resource_outs += 1;
                }
            }
            wall_times.sort_unstable();
            let median_certified_wall_ms = if wall_times.is_empty() {
                None
            } else {
                Some(wall_times[wall_times.len() / 2])
            };
            Some(CandidateSummary {
                id: candidate.id,
                generation: candidate.generation,
                selector: candidate.selector.clone(),
                problems: problems.len(),
                certified,
                median_certified_wall_ms,
                resource_outs,
            })
        })
        .collect::<Vec<_>>();
    summaries.sort_by(compare_summary);
    summaries
}

fn candidate_complete(
    candidate: &CandidatePlan,
    problems: &[ProblemEntry],
    store: &RunStore,
) -> bool {
    problems
        .iter()
        .all(|problem| store.contains(&format!("train:{}:{}", candidate.id, problem.path)))
}

struct RunContext<'a> {
    args: &'a AutosearchArgs,
    mrs: &'a Path,
    proover: &'a Path,
    corpus_root: &'a Path,
    tptp_root: Option<&'a Path>,
    output: &'a Path,
    store: &'a RunStore,
}

struct SearchRun<'a> {
    phase: RunPhase,
    candidate_id: Option<usize>,
    pair_id: Option<usize>,
    repeat: Option<usize>,
    arm: &'a str,
    selector: &'a str,
    problem: &'a ProblemEntry,
}

fn run_pending_training(
    pool: &rayon::ThreadPool,
    context: &RunContext<'_>,
    candidates: &[CandidatePlan],
    problems: &[ProblemEntry],
) -> ToolResult<()> {
    let mut tasks = Vec::new();
    for candidate in candidates {
        for problem in problems {
            let key = format!("train:{}:{}", candidate.id, problem.path);
            if !context.store.contains(&key) {
                tasks.push((candidate.clone(), problem.clone()));
            }
        }
    }
    if tasks.is_empty() {
        return Ok(());
    }
    let mut rng = StdRng::seed_from_u64(context.args.seed ^ 0x7363_7265_656e_696e);
    tasks.shuffle(&mut rng);
    eprintln!(
        "[autosearch] evaluating {} candidate/problem runs with jobs={}",
        tasks.len(),
        context.args.jobs
    );
    pool.install(|| {
        tasks.par_iter().try_for_each(|(candidate, problem)| {
            let record = run_search(
                context,
                SearchRun {
                    phase: RunPhase::Training,
                    candidate_id: Some(candidate.id),
                    pair_id: None,
                    repeat: None,
                    arm: "candidate",
                    selector: &candidate.selector,
                    problem,
                },
            )
            .map_err(|error| error.to_string())?;
            context.store.put(record).map_err(|error| error.to_string())
        })
    })
    .map_err(|error| -> Box<dyn Error> { error.into() })
}

fn run_search(context: &RunContext<'_>, run: SearchRun<'_>) -> ToolResult<RunRecord> {
    let problem_path = context.corpus_root.join(&run.problem.path);
    let problem_tag = short_hash(run.problem.path.as_bytes());
    let run_name = match run.phase {
        RunPhase::Training => format!(
            "train-c{:04}-{}",
            run.candidate_id.unwrap_or_default(),
            problem_tag
        ),
        RunPhase::Holdout => format!(
            "holdout-p{:04}-{}-{}",
            run.pair_id.unwrap_or_default(),
            run.arm,
            problem_tag
        ),
    };
    let work_stdout = context
        .output
        .join("work")
        .join(format!("{run_name}.stdout"));
    let work_stderr = context
        .output
        .join("work")
        .join(format!("{run_name}.stderr"));
    let stdout_file = File::create(&work_stdout)?;
    let stderr_file = File::create(&work_stderr)?;
    let mut command = Command::new(context.mrs);
    command
        .arg("--time")
        .arg(context.args.time.to_string())
        .arg("--workers")
        .arg("1")
        .arg("--hardware")
        .arg("adaptive")
        .arg("--schedule")
        .arg(&context.args.schedule)
        .arg("--strategy")
        .arg(context.args.strategy.to_string())
        .arg("--selection")
        .arg(run.selector)
        .arg("--proof-bytes-limit")
        .arg(PROOF_BYTES_LIMIT.to_string())
        .arg("--self-check")
        .arg(&problem_path)
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .env_remove("TPTP");
    for (key, _) in std::env::vars_os() {
        let name = key.to_string_lossy();
        if name.starts_with("MRS_") || name.starts_with("TRACE_") {
            command.env_remove(key);
        }
    }
    command
        .env("MRS_WORKERS", "1")
        .env("MRS_HARDWARE", "adaptive")
        .env("MRS_SHARED_POOL_INTERVAL", "0")
        .env("MRS_PREPHASE", "0")
        .env("MRS_MAX_MEMORY_MB", context.args.memory_mb.to_string());
    if let Some(tptp) = context.tptp_root {
        command.env("TPTP", tptp);
    }
    apply_address_space_limit(&mut command, context.args.memory_mb);

    let started = Instant::now();
    let mut child = command.spawn()?;
    let timeout = Duration::from_secs(context.args.time.saturating_add(CHILD_TIMEOUT_MARGIN_SECS));
    let (exit_code, outer_timeout) = match child.wait_timeout(timeout) {
        Ok(Some(status)) => (status.code().or(Some(-1)), false),
        Ok(None) => {
            terminate_child(&mut child);
            (Some(124), true)
        }
        Err(error) => {
            terminate_child(&mut child);
            let _ = fs::remove_file(&work_stdout);
            let _ = fs::remove_file(&work_stderr);
            return Err(error.into());
        }
    };
    let elapsed_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    let stdout = fs::read_to_string(&work_stdout).unwrap_or_default();
    let stderr = fs::read_to_string(&work_stderr).unwrap_or_default();
    let details = parse_detail(&stderr);
    let status = if outer_timeout {
        "OuterTimeout".to_owned()
    } else {
        stdout
            .lines()
            .find_map(|line| line.strip_prefix("% SZS status "))
            .and_then(|line| line.split_whitespace().next())
            .unwrap_or("NoStatus")
            .to_owned()
    };
    if status == "NoStatus" && (stderr.contains("Usage:") || stderr.contains("Error:")) {
        let _ = fs::remove_file(&work_stdout);
        let _ = fs::remove_file(&work_stderr);
        return Err(format!(
            "mrs invocation failed before returning SZS status for {}: {}",
            run.problem.path,
            stderr.lines().take(6).collect::<Vec<_>>().join(" | ")
        )
        .into());
    }
    let self_check = details
        .get("self_check")
        .cloned()
        .unwrap_or_else(|| "Missing".into());
    let certified = certified_search(&status, &self_check, exit_code, outer_timeout);
    let proof_emitted = details
        .get("proof_emitted")
        .is_some_and(|value| value == "true")
        && stdout.contains("SZS output start Proof");
    let mut proof_path = None;
    let failure = if outer_timeout {
        Some("outer deadline expired".into())
    } else if exit_code.is_some_and(|code| code != 0) {
        Some(format!(
            "mrs exited with code {}",
            exit_code.unwrap_or_default()
        ))
    } else if status == "NoStatus" {
        Some("no SZS status line".into())
    } else if matches!(status.as_str(), "Theorem" | "Unsatisfiable") && !certified {
        Some(format!("refutation was not self-checked: {self_check}"))
    } else {
        None
    };

    match run.phase {
        RunPhase::Training => {
            let _ = fs::remove_file(&work_stdout);
            let _ = fs::remove_file(&work_stderr);
        }
        RunPhase::Holdout => {
            let logs = context.output.join("logs");
            fs::copy(&work_stdout, logs.join(format!("{run_name}.stdout")))?;
            fs::copy(&work_stderr, logs.join(format!("{run_name}.stderr")))?;
            if certified && proof_emitted {
                let relative = PathBuf::from("proofs").join(format!("{run_name}.p"));
                fs::copy(&work_stdout, context.output.join(&relative))?;
                proof_path = Some(relative.to_string_lossy().to_string());
            }
            let _ = fs::remove_file(&work_stdout);
            let _ = fs::remove_file(&work_stderr);
        }
    }

    Ok(RunRecord {
        phase: run.phase,
        candidate_id: run.candidate_id,
        pair_id: run.pair_id,
        repeat: run.repeat,
        arm: run.arm.to_owned(),
        selector: run.selector.to_owned(),
        problem: run.problem.path.clone(),
        family: run.problem.family.clone(),
        status,
        self_check,
        certified,
        strict_verdict: None,
        elapsed_ms,
        processed: details
            .get("processed")
            .and_then(|value| value.parse().ok())
            .unwrap_or_default(),
        generated: details
            .get("generated")
            .and_then(|value| value.parse().ok())
            .unwrap_or_default(),
        exit_code,
        proof_path,
        failure,
    })
}

fn run_holdout(
    pool: &rayon::ThreadPool,
    context: &RunContext<'_>,
    state: &RunState,
    holdout: &[ProblemEntry],
    baseline: &str,
) -> ToolResult<()> {
    let champion = state
        .champion
        .as_deref()
        .ok_or("holdout phase has no selected champion")?;
    let problem_by_path = holdout
        .iter()
        .map(|problem| (problem.path.clone(), problem.clone()))
        .collect::<HashMap<_, _>>();
    let mut tasks = Vec::new();
    for pair in &state.holdout_pairs {
        let Some(problem) = problem_by_path.get(&pair.problem) else {
            return Err(format!(
                "holdout problem missing from frozen corpus: {}",
                pair.problem
            )
            .into());
        };
        tasks.push((pair.clone(), problem.clone()));
    }
    eprintln!(
        "[autosearch] paired strict holdout: {} problem/repeat pairs, jobs={}",
        tasks.len(),
        context.args.jobs
    );
    pool.install(|| {
        tasks.par_iter().try_for_each(|(pair, problem)| {
            let order = if pair.first_arm == "baseline" {
                ["baseline", "candidate"]
            } else {
                ["candidate", "baseline"]
            };
            for arm in order {
                let key = format!("holdout:{}:{arm}", pair.id);
                if !context.store.contains(&key) {
                    let selector = if arm == "baseline" {
                        baseline
                    } else {
                        champion
                    };
                    let record = run_search(
                        context,
                        SearchRun {
                            phase: RunPhase::Holdout,
                            candidate_id: None,
                            pair_id: Some(pair.id),
                            repeat: Some(pair.repeat),
                            arm,
                            selector,
                            problem,
                        },
                    )
                    .map_err(|error| error.to_string())?;
                    context
                        .store
                        .put(record)
                        .map_err(|error| error.to_string())?;
                }
            }
            for arm in ["baseline", "candidate"] {
                let key = format!("holdout:{}:{arm}", pair.id);
                let Some(mut record) = context.store.get(&key) else {
                    return Err(format!("missing holdout run {key}"));
                };
                if record.strict_verdict.is_none() {
                    record.strict_verdict = Some(audit_holdout_proof(context, &record));
                    context
                        .store
                        .put(record)
                        .map_err(|error| error.to_string())?;
                }
            }
            Ok::<(), String>(())
        })
    })
    .map_err(|error| -> Box<dyn Error> { error.into() })
}

fn audit_holdout_proof(context: &RunContext<'_>, record: &RunRecord) -> String {
    let Some(relative) = record.proof_path.as_deref() else {
        return if record.certified {
            "ProofOmitted".into()
        } else {
            "NotApplicable".into()
        };
    };
    let proof_path = context.output.join(relative);
    if !proof_path.is_file() {
        return "MissingArtifact".into();
    }
    let tag = short_hash(record.key().as_bytes());
    let stdout_path = context
        .output
        .join("logs")
        .join(format!("audit-{tag}.stdout"));
    let stderr_path = context
        .output
        .join("logs")
        .join(format!("audit-{tag}.stderr"));
    let stdout = match File::create(&stdout_path) {
        Ok(file) => file,
        Err(_) => return "AuditLaunchError".into(),
    };
    let stderr = match File::create(&stderr_path) {
        Ok(file) => file,
        Err(_) => return "AuditLaunchError".into(),
    };
    let mut command = Command::new(context.proover);
    command
        .arg("--strict")
        .arg("--only-mrs")
        .arg("--time")
        .arg(context.args.audit_time.to_string())
        .arg("--workers")
        .arg("1")
        .arg("--problems-dir")
        .arg(context.tptp_root.unwrap_or(context.corpus_root))
        .arg(&proof_path)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .env_remove("TPTP");
    for (key, _) in std::env::vars_os() {
        let name = key.to_string_lossy();
        if name.starts_with("MRS_") || name.starts_with("TRACE_") {
            command.env_remove(key);
        }
    }
    command.env("MRS_MAX_MEMORY_MB", context.args.memory_mb.to_string());
    if let Some(tptp) = context.tptp_root {
        command.env("TPTP", tptp);
    }
    apply_address_space_limit(&mut command, context.args.memory_mb);
    let timeout = Duration::from_secs(context.args.audit_time.saturating_add(10));
    let Ok(mut child) = command.spawn() else {
        return "AuditLaunchError".into();
    };
    let audit_success = match child.wait_timeout(timeout) {
        Ok(Some(status)) => status.success(),
        Ok(None) => {
            terminate_child(&mut child);
            return "AuditTimeout".into();
        }
        Err(_) => {
            terminate_child(&mut child);
            return "AuditWaitError".into();
        }
    };
    let verdict = fs::read_to_string(stdout_path)
        .ok()
        .and_then(|text| {
            text.lines()
                .find_map(|line| line.strip_prefix("% SZS status "))
                .and_then(|line| line.split_whitespace().next())
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "AuditNoStatus".into());
    checked_audit_verdict(&verdict, audit_success)
}

pub(super) fn checked_audit_verdict(verdict: &str, success: bool) -> String {
    match verdict {
        "VerifiedBad" | "Unknown" => verdict.to_owned(),
        "VerifiedGood" if success => verdict.to_owned(),
        _ if !success => "AuditExitFailure".into(),
        _ => "AuditNoStatus".into(),
    }
}

fn certified_search(
    status: &str,
    self_check: &str,
    exit_code: Option<i32>,
    outer_timeout: bool,
) -> bool {
    !outer_timeout
        && exit_code == Some(0)
        && matches!(status, "Theorem" | "Unsatisfiable")
        && self_check == "Certified"
}

pub(super) fn terminate_child(child: &mut Child) {
    #[cfg(unix)]
    {
        // Children are launched into a private process group. Stop their
        // overlapping certificate subprocess as well as the coordinator.
        // SAFETY: a negative PID targets only the group created for this child.
        unsafe {
            libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn make_holdout_pairs(
    problems: &[ProblemEntry],
    repeats: usize,
    seed: u64,
) -> Vec<HoldoutPairPlan> {
    let mut rng = StdRng::seed_from_u64(seed ^ 0x686f_6c64_6f75_7421);
    let mut pairs = Vec::with_capacity(problems.len() * repeats);
    for problem in problems {
        for repeat in 0..repeats {
            let first_arm = ["baseline", "candidate"]
                .choose(&mut rng)
                .copied()
                .unwrap_or("baseline")
                .to_owned();
            pairs.push(HoldoutPairPlan {
                id: pairs.len(),
                repeat,
                problem: problem.path.clone(),
                family: problem.family.clone(),
                first_arm,
            });
        }
    }
    pairs
}

fn summarize_holdout(
    pairs: &[HoldoutPairPlan],
    store: &RunStore,
    baseline_selector: &str,
    candidate_selector: &str,
) -> Vec<HoldoutPairResult> {
    pairs
        .iter()
        .filter_map(|pair| {
            let baseline = store.get(&format!("holdout:{}:baseline", pair.id))?;
            let candidate = store.get(&format!("holdout:{}:candidate", pair.id))?;
            let baseline_verified = baseline.strict_verdict.as_deref() == Some("VerifiedGood");
            let candidate_verified = candidate.strict_verdict.as_deref() == Some("VerifiedGood");
            Some(HoldoutPairResult {
                pair_id: pair.id,
                repeat: pair.repeat,
                problem: pair.problem.clone(),
                family: pair.family.clone(),
                baseline_selector: baseline_selector.into(),
                candidate_selector: candidate_selector.into(),
                baseline_status: baseline.status,
                candidate_status: candidate.status,
                baseline_verdict: baseline.strict_verdict.unwrap_or_default(),
                candidate_verdict: candidate.strict_verdict.unwrap_or_default(),
                baseline_verified,
                candidate_verified,
                delta: i8::from(candidate_verified) - i8::from(baseline_verified),
            })
        })
        .collect()
}

fn parse_detail(stderr: &str) -> BTreeMap<String, String> {
    stderr
        .lines()
        .filter_map(|line| line.strip_prefix("% SZS detail "))
        .filter_map(shlex::split)
        .flatten()
        .filter_map(|token| {
            let (key, value) = token.split_once('=')?;
            Some((key.to_owned(), value.to_owned()))
        })
        .collect()
}

pub(super) fn apply_address_space_limit(command: &mut Command, memory_mb: u64) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt;
        let limit = memory_mb
            .saturating_add(ADDRESS_SPACE_HEADROOM_MB)
            .saturating_mul(1024 * 1024) as libc::rlim_t;
        // SAFETY: the child performs only async-signal-safe setrlimit before exec.
        unsafe {
            command.pre_exec(move || {
                let value = libc::rlimit {
                    rlim_cur: limit,
                    rlim_max: limit,
                };
                // SAFETY: `value` is a valid, initialized rlimit structure.
                if libc::setrlimit(libc::RLIMIT_AS, &value) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (command, memory_mb);
    }
}

fn repair_trailing_jsonl(path: &Path) -> std::io::Result<()> {
    if !path.is_file() {
        return Ok(());
    }
    let bytes = fs::read(path)?;
    if bytes.last() == Some(&b'\n') {
        return Ok(());
    }
    let valid_len = bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |index| index + 1);
    OpenOptions::new()
        .write(true)
        .open(path)?
        .set_len(valid_len as u64)
}

fn lock_output(output: &Path) -> std::io::Result<File> {
    use std::os::fd::AsRawFd;

    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(output.join(".autosearch.lock"))?;
    // SAFETY: the file descriptor remains owned by `file`; flock only changes
    // its advisory lock state and is released automatically when the file drops.
    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if result != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(file)
}

fn write_json_new(path: &Path, value: &Value) -> ToolResult<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> ToolResult<()> {
    let temporary = path.with_extension("json.tmp");
    {
        let mut file = File::create(&temporary)?;
        serde_json::to_writer_pretty(&mut file, value)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
    }
    fs::rename(temporary, path)?;
    Ok(())
}

fn sha256_file(path: &Path) -> ToolResult<String> {
    Ok(sha256_bytes(&fs::read(path)?))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn short_hash(bytes: &[u8]) -> String {
    sha256_bytes(bytes)[..12].to_owned()
}

fn git_commit() -> ToolResult<String> {
    let output = Command::new("git").args(["rev-parse", "HEAD"]).output()?;
    if !output.status.success() {
        return Err("could not record git revision".into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn problem(path: &str, family: &str, split: &str) -> ProblemEntry {
        ProblemEntry {
            path: path.into(),
            family: family.into(),
            split: split.into(),
        }
    }

    fn summary(id: usize, certified: usize, median: Option<u64>) -> CandidateSummary {
        CandidateSummary {
            id,
            generation: 0,
            selector: format!("age:{}", id + 1),
            problems: 10,
            certified,
            median_certified_wall_ms: median,
            resource_outs: 0,
        }
    }

    #[test]
    fn candidate_dsl_round_trips_queue_order() {
        let candidate = Candidate::parse("queue:goal=2,age=1,weight=3").unwrap();
        assert_eq!(candidate.expression(), "queue:goal=2,age=1,weight=3");
        assert_eq!(
            Candidate::parse(&candidate.expression()).unwrap(),
            candidate
        );
    }

    #[test]
    fn candidate_dsl_rejects_ml_and_invalid_frequencies() {
        assert!(Candidate::parse("ml:5:0.3").is_err());
        assert!(Candidate::parse("queue:age=0,weight=1").is_err());
        assert!(Candidate::parse("queue:age=1,age=2").is_err());
        assert_eq!(Candidate::parse("age:1").unwrap(), Candidate::Fifo);
        assert_eq!(Candidate::parse("goal:1").unwrap(), Candidate::Fifo);
    }

    #[test]
    fn stratified_sample_is_stable_and_round_robins_families() {
        let problems = vec![
            problem("A001.p", "A", "development"),
            problem("A002.p", "A", "development"),
            problem("A003.p", "A", "development"),
            problem("B001.p", "B", "development"),
            problem("C001.p", "C", "development"),
        ];
        let first = stratified_sample(&problems, 3, 7);
        let second = stratified_sample(&problems, 3, 7);
        assert_eq!(
            first
                .iter()
                .map(|problem| &problem.path)
                .collect::<Vec<_>>(),
            second
                .iter()
                .map(|problem| &problem.path)
                .collect::<Vec<_>>()
        );
        assert_eq!(first.len(), 3);
        assert_eq!(
            first
                .iter()
                .map(|problem| problem.family.as_str())
                .collect::<HashSet<_>>()
                .len(),
            3
        );
    }

    #[test]
    fn training_ranking_prioritizes_certified_coverage_over_speed() {
        assert_eq!(
            compare_summary(&summary(0, 4, Some(10)), &summary(1, 3, Some(1))),
            Ordering::Less
        );
        assert_eq!(
            compare_summary(&summary(0, 4, Some(10)), &summary(1, 4, Some(11))),
            Ordering::Less
        );
    }

    #[test]
    fn mutation_preserves_a_valid_nonempty_selector() {
        let mut rng = StdRng::seed_from_u64(3);
        let parent = Candidate::parse("queue:age=1,weight=2,horn=1").unwrap();
        for _ in 0..100 {
            let expression = parent.mutate(&mut rng).expression();
            assert!(Candidate::parse(&expression).is_ok(), "{expression}");
        }
    }

    #[test]
    fn resource_budget_rejects_oversubscription() {
        assert!(check_resource_budget(5, 512, 4, Some(8192)).is_err());
        assert!(check_resource_budget(2, 2048, 4, Some(16384)).is_ok());
        assert!(check_resource_budget(2, 2048, 4, Some(8192)).is_err());
        assert!(check_resource_budget(3, 2048, 4, Some(8192)).is_err());
        assert!(check_resource_budget(1, 2048, 1, Some(8192)).is_ok());
    }

    #[test]
    fn corpus_snapshot_is_copied_and_resume_checks_its_digest() {
        let source = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        let source_file = source.path().join("AAA001+1.p");
        fs::write(&source_file, "fof(a,axiom,p).\n").unwrap();
        let bytes = fs::read(&source_file).unwrap();
        let manifest = Manifest {
            schema: 1,
            seed: 1,
            holdout_percent: 25,
            family_policy: "tptp-domain-prefix",
            corpus_sha256: "test".into(),
            files: vec![FileEntry {
                path: "AAA001+1.p".into(),
                sha256: sha256_bytes(&bytes),
                bytes: bytes.len() as u64,
            }],
            problems: Vec::new(),
        };
        let snapshot = output.path().join("corpus");
        prepare_corpus_snapshot(source.path(), &manifest, &snapshot, false).unwrap();
        assert_eq!(fs::read(snapshot.join("AAA001+1.p")).unwrap(), bytes);

        fs::write(snapshot.join("AAA001+1.p"), "changed").unwrap();
        assert!(prepare_corpus_snapshot(source.path(), &manifest, &snapshot, true).is_err());
    }

    #[test]
    fn audit_verdicts_keep_rejections_but_never_credit_failed_children() {
        assert_eq!(checked_audit_verdict("VerifiedBad", false), "VerifiedBad");
        assert_eq!(checked_audit_verdict("Unknown", false), "Unknown");
        assert_eq!(
            checked_audit_verdict("VerifiedGood", false),
            "AuditExitFailure"
        );
        assert_eq!(checked_audit_verdict("VerifiedGood", true), "VerifiedGood");
        assert!(certified_search("Theorem", "Certified", Some(0), false));
        assert!(!certified_search("Theorem", "Certified", Some(1), false));
        assert!(!certified_search("Theorem", "Certified", Some(0), true));
        assert!(!certified_search("GaveUp", "Certified", Some(0), false));
    }
}
