//! Asynchronous candidate certification coordinator.
//!
//! Evaluates candidate refutations emitted by portfolio search workers in real time
//! against the strict proof kernel. Decouples candidate discovery from search termination:
//! an uncertified candidate (e.g. timeout, rejection, or inconclusive verification)
//! is discarded while sibling search workers continue exploring without interruption.

use std::path::PathBuf;
use std::process;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock, mpsc};
use std::time::{Duration, Instant};

use mrs_proof_kernel::KernelVerdict;
use mrs_search::SearchResult;
use mrs_search::strategy::{CandidateReceiver, CandidateRefutation};

static SELF_VERIFY_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Configuration for the asynchronous certification coordinator.
#[derive(Debug, Clone)]
pub struct AsyncCoordinatorConfig {
    pub time_limit: Duration,
    pub self_check_reserve: Duration,
    pub problem_path: String,
    pub problem_name: String,
    pub input_text: String,
    pub include_root: Option<PathBuf>,
    pub has_includes: bool,
    pub cert_oversubscribed: bool,
    pub search_workers: usize,
    pub cert_workers: usize,
    pub start_time: Instant,
}

/// Telemetry metrics collected during candidate certification.
#[derive(Debug, Clone, Default)]
pub struct CertificationTelemetry {
    pub candidate_count: usize,
    pub candidate_rejection_count: usize,
    pub candidate_reasons: Vec<String>,
    pub total_elaboration_time: Duration,
    pub total_strict_kernel_time: Duration,
    pub proof_bytes: usize,
    pub proof_nodes: usize,
    pub time_remaining_at_discovery: Duration,
    pub certified_candidate_index: Option<usize>,
    pub cert_oversubscribed: bool,
    pub search_workers: usize,
    pub cert_workers: usize,
}

enum CoordinatorMessage {
    Candidate(CandidateRefutation),
    Finish,
}

/// Thread-safe coordinator running asynchronous candidate proof verification.
pub struct AsyncCoordinator {
    config: AsyncCoordinatorConfig,
    tx: mpsc::Sender<CoordinatorMessage>,
    stop_flag: RwLock<Option<Arc<AtomicBool>>>,
    certified_winner: Mutex<Option<SearchResult>>,
    telemetry: Mutex<CertificationTelemetry>,
    join_handle: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl AsyncCoordinator {
    /// Spawns the coordinator background thread and returns an `Arc<AsyncCoordinator>`.
    pub fn new(config: AsyncCoordinatorConfig) -> Arc<Self> {
        let (tx, rx) = mpsc::channel();
        let coordinator = Arc::new(Self {
            config: config.clone(),
            tx,
            stop_flag: RwLock::new(None),
            certified_winner: Mutex::new(None),
            telemetry: Mutex::new(CertificationTelemetry {
                cert_oversubscribed: config.cert_oversubscribed,
                search_workers: config.search_workers,
                cert_workers: config.cert_workers,
                ..Default::default()
            }),
            join_handle: Mutex::new(None),
        });

        let coordinator_worker = Arc::clone(&coordinator);
        let handle = std::thread::Builder::new()
            .name("mrs-cert-coordinator".to_string())
            .spawn(move || {
                coordinator_worker.worker_loop(rx);
            })
            .expect("failed to spawn coordinator thread");

        *coordinator.join_handle.lock().unwrap() = Some(handle);
        coordinator
    }

    /// Stops the coordinator thread, waits for it to finish, and returns the telemetry.
    pub fn finish(&self) -> CertificationTelemetry {
        let _ = self.tx.send(CoordinatorMessage::Finish);
        if let Some(handle) = self.join_handle.lock().unwrap().take() {
            let _ = handle.join();
        }
        self.telemetry.lock().unwrap().clone()
    }

    /// Returns the certified winner result, if one was certified.
    pub fn certified_result(&self) -> Option<SearchResult> {
        self.certified_winner.lock().unwrap().clone()
    }

    fn worker_loop(&self, rx: mpsc::Receiver<CoordinatorMessage>) {
        let mut candidate_idx = 0;
        while let Ok(msg) = rx.recv() {
            match msg {
                CoordinatorMessage::Finish => break,
                CoordinatorMessage::Candidate(candidate) => {
                    candidate_idx += 1;
                    {
                        let mut tele = self.telemetry.lock().unwrap();
                        tele.candidate_count += 1;
                    }

                    // If an earlier candidate already certified, ignore subsequent ones.
                    if self.certified_winner.lock().unwrap().is_some() {
                        continue;
                    }

                    let remaining = self
                        .config
                        .time_limit
                        .saturating_sub(self.config.start_time.elapsed());

                    if remaining < self.config.self_check_reserve {
                        let reason = format!(
                            "candidate {candidate_idx} (strategy {}) rejected: insufficient time remains ({:.2}s < {:.2}s reserve)",
                            candidate.strategy_id,
                            remaining.as_secs_f64(),
                            self.config.self_check_reserve.as_secs_f64()
                        );
                        let mut tele = self.telemetry.lock().unwrap();
                        tele.candidate_rejection_count += 1;
                        tele.candidate_reasons.push(reason);
                        continue;
                    }

                    // Elaboration: format proof with source tag
                    let elab_start = Instant::now();
                    let proof_source = if self.config.problem_path == "-" {
                        "input"
                    } else {
                        &self.config.problem_path
                    };
                    let temp_proof_text =
                        format!("% Proof : {proof_source}\n{}", candidate.tstp_proof);
                    let elab_time = elab_start.elapsed();

                    // Verification: strict kernel execution
                    let kernel_start = Instant::now();
                    let (mut verdict, mut error_msg) =
                        verify_candidate_proof(&self.config, &temp_proof_text);
                    let kernel_time = kernel_start.elapsed();

                    // A certificate that finishes after the process budget is
                    // not a valid certified result for this run. The kernel
                    // is synchronous, so enforce the deadline immediately
                    // after it returns rather than emitting a late theorem.
                    if matches!(verdict, KernelVerdict::Certified)
                        && self.config.start_time.elapsed() >= self.config.time_limit
                    {
                        let msg = "strict self-check exceeded the time limit".to_string();
                        verdict = KernelVerdict::Inconclusive(msg.clone());
                        error_msg = Some(msg);
                    }

                    let mut tele = self.telemetry.lock().unwrap();
                    tele.total_elaboration_time += elab_time;
                    tele.total_strict_kernel_time += kernel_time;

                    if matches!(verdict, KernelVerdict::Certified) {
                        tele.certified_candidate_index = Some(candidate_idx);
                        tele.proof_bytes = candidate.tstp_proof.len();
                        tele.proof_nodes = count_proof_nodes(&candidate.tstp_proof);
                        tele.time_remaining_at_discovery = candidate.time_remaining;

                        *self.certified_winner.lock().unwrap() = Some(SearchResult::Refutation(
                            candidate.clause_id,
                            candidate.tstp_proof,
                        ));
                        drop(tele);

                        // Candidate certified! Signal search workers to stop immediately.
                        if let Some(stop) = self.stop_flag.read().unwrap().as_ref() {
                            stop.store(true, Ordering::Relaxed);
                        }
                        break;
                    } else {
                        let reason = error_msg.unwrap_or_else(|| {
                            format!(
                                "candidate {candidate_idx} (strategy {}) returned {verdict}",
                                candidate.strategy_id
                            )
                        });
                        tele.candidate_rejection_count += 1;
                        tele.candidate_reasons.push(reason);
                    }
                }
            }
        }
    }
}

impl CandidateReceiver for AsyncCoordinator {
    fn submit_candidate(&self, candidate: CandidateRefutation) -> bool {
        let _ = self.tx.send(CoordinatorMessage::Candidate(candidate));
        // Return true if certified already, signaling the caller to stop immediately.
        self.stop_flag
            .read()
            .unwrap()
            .as_ref()
            .map(|s| s.load(Ordering::Relaxed))
            .unwrap_or(false)
    }

    fn register_stop_flag(&self, stop_flag: Arc<AtomicBool>) {
        *self.stop_flag.write().unwrap() = Some(stop_flag);
    }

    fn certified_result(&self) -> Option<SearchResult> {
        self.certified_winner.lock().unwrap().clone()
    }
}

fn verify_candidate_proof(
    config: &AsyncCoordinatorConfig,
    temp_proof_text: &str,
) -> (KernelVerdict, Option<String>) {
    let tptp_root = std::env::var("TPTP").ok().map(PathBuf::from);

    if config.problem_path == "-" {
        let Some(root) = config.include_root.as_deref() else {
            let msg = "strict self-check for stdin requires --include-root DIR".to_string();
            return (KernelVerdict::Inconclusive(msg.clone()), Some(msg));
        };
        let verdict = mrs_proover::strict::verify_text_with_include_root(
            config.input_text.clone(),
            temp_proof_text.to_string(),
            root,
            mrs_proof_kernel::VerificationLimits::default(),
        );
        let reason = if matches!(verdict, KernelVerdict::Certified) {
            None
        } else {
            Some(format!("strict self-check returned {verdict}"))
        };
        (verdict, reason)
    } else if !config.has_includes {
        let verdict = mrs_proover::strict::verify_text(
            &config.input_text,
            temp_proof_text,
            Some(&config.problem_path),
            mrs_proof_kernel::VerificationLimits::default(),
        );
        let reason = if matches!(verdict, KernelVerdict::Certified) {
            None
        } else {
            Some(format!("strict self-check returned {verdict}"))
        };
        (verdict, reason)
    } else {
        let counter = SELF_VERIFY_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temp_path = std::env::temp_dir().join(format!(
            "mrs_self_verify_{}_{}_{}.p",
            process::id(),
            counter,
            config.problem_name
        ));
        match std::fs::write(&temp_path, temp_proof_text) {
            Ok(()) => match mrs_proover::load::load(&temp_path, tptp_root.as_deref()) {
                Ok(job) => {
                    let verdict = mrs_proover::strict::verify_loaded_job_default(&job);
                    let _ = std::fs::remove_file(&temp_path);
                    let reason = if matches!(verdict, KernelVerdict::Certified) {
                        None
                    } else {
                        Some(format!("strict self-check returned {verdict}"))
                    };
                    (verdict, reason)
                }
                Err(error) => {
                    let _ = std::fs::remove_file(&temp_path);
                    let msg = format!("strict self-check could not load proof: {error}");
                    (KernelVerdict::Inconclusive(msg.clone()), Some(msg))
                }
            },
            Err(error) => {
                let msg = format!("strict self-check could not write proof: {error}");
                (KernelVerdict::Inconclusive(msg.clone()), Some(msg))
            }
        }
    }
}

fn count_proof_nodes(tstp_proof: &str) -> usize {
    tstp_proof
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with("cnf(")
                || trimmed.starts_with("fof(")
                || trimmed.starts_with("tff(")
                || trimmed.starts_with("thf(")
        })
        .count()
}
