//! External-process ATP backends: eprover and vampire.
//!
//! Common pipeline:
//!
//! 1. Serialize premises and conclusion into a small FOF problem file.
//! 2. Spawn the ATP with a wall-clock time limit.
//! 3. Parse the SZS status line from stdout.
//!
//! Premises are emitted as `fof(p<i>, axiom, ...)` and the conclusion as
//! `fof(g, conjecture, ...)`. A `Theorem` / `Unsatisfiable` reply means the
//! step is sound. A `CounterSatisfiable` / `Satisfiable` reply means the
//! premises do *not* entail the conclusion — positive evidence that the
//! step is bad. Anything else (`Timeout`, `GaveUp`, `Unknown`,
//! `ResourceOut`) is reported as `Unknown`.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

use mrs_core::display::DisplayWithSymbols;
use mrs_core::{Formula, SymbolTable};

use super::{Atp, AtpVerdict};

/// Backend that calls `eprover` as a subprocess.
pub struct EProverAtp {
    pub binary: PathBuf,
}

impl EProverAtp {
    pub fn new(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: binary.into(),
        }
    }
}

impl Atp for EProverAtp {
    fn name(&self) -> &'static str {
        "eprover"
    }

    fn check_step(
        &self,
        symbols: &SymbolTable,
        premises: &[Formula],
        conclusion: &Formula,
        budget: Duration,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> AtpVerdict {
        let problem = build_fof_problem(symbols, premises, conclusion);
        // eprover only accepts integer --cpu-limit, so floor to 1s. The
        // Rust-side wall-clock kill in `run_atp` enforces the *real*
        // budget; --cpu-limit is just a fallback upper bound that
        // applies if our kill races with the binary exiting.
        let secs = budget.as_secs().max(1);
        let cpu_arg = format!("--cpu-limit={secs}");
        run_atp(
            &self.binary,
            &["--auto", "--silent", &cpu_arg, "--tptp3-format"],
            &problem,
            budget,
            cancel,
        )
    }
}

/// Backend that calls `vampire` as a subprocess.
pub struct VampireAtp {
    pub binary: PathBuf,
}

impl VampireAtp {
    pub fn new(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: binary.into(),
        }
    }
}

impl Atp for VampireAtp {
    fn name(&self) -> &'static str {
        "vampire"
    }

    fn check_step(
        &self,
        symbols: &SymbolTable,
        premises: &[Formula],
        conclusion: &Formula,
        budget: Duration,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> AtpVerdict {
        let problem = build_fof_problem(symbols, premises, conclusion);
        // Vampire accepts fractional --time_limit. Pass the budget as
        // decimal seconds so sub-second budgets are honored cleanly;
        // the Rust-side wall-clock kill is still in place as a hard
        // backstop.
        let secs = (budget.as_secs_f64()).max(0.1);
        let secs_arg = format!("{secs:.2}");
        run_atp(
            &self.binary,
            &["--time_limit", &secs_arg, "--input_syntax", "tptp"],
            &problem,
            budget,
            cancel,
        )
    }
}

/// Backend that calls `vampire` in finite-model-building mode as a counter-
/// model finder. Unlike the saturation provers, FMB actively searches for a
/// finite model of `premises ∧ ¬conclusion`; finding one is positive proof
/// that the step is *not* a valid entailment (`CounterSatisfiable` →
/// `Unsound`), which lets us report `VerifiedBad` (+2) on bad proofs that
/// the entailment provers can only time out on. FMB is sound in both
/// directions: it returns `Theorem`/`Unsatisfiable` when the step *is* valid
/// and a counter-model only when one genuinely exists, so it never refutes a
/// sound step. Placed last on the ladder, after the entailment provers.
pub struct VampireFmbAtp {
    pub binary: PathBuf,
}

impl VampireFmbAtp {
    pub fn new(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: binary.into(),
        }
    }
}

impl Atp for VampireFmbAtp {
    fn name(&self) -> &'static str {
        "vampire-fmb"
    }

    fn check_step(
        &self,
        symbols: &SymbolTable,
        premises: &[Formula],
        conclusion: &Formula,
        budget: Duration,
        cancel: &std::sync::atomic::AtomicBool,
    ) -> AtpVerdict {
        let problem = build_fof_problem(symbols, premises, conclusion);
        let secs = (budget.as_secs_f64()).max(0.1);
        let secs_arg = format!("{secs:.2}");
        run_atp(
            &self.binary,
            &[
                "--saturation_algorithm",
                "fmb",
                "--time_limit",
                &secs_arg,
                "--input_syntax",
                "tptp",
            ],
            &problem,
            budget,
            cancel,
        )
    }
}

use mrs_core::clause::ClauseIdGen;
use mrs_search::SearchResult;

/// Backend that calls the in-tree `mrs` binary logic.
/// Previously called the binary as a subprocess, but now runs completely in-process
/// using the `mrs-search` library to eliminate subprocess launch overhead (which saves
/// seconds per proof verification).
pub struct MrsAtp {
    pub binary: PathBuf,
    pub use_proover_mode: bool,
    /// In-process schedule reports, one for each completed step query.
    pub reports: std::sync::Mutex<Vec<mrs_search::ScheduleReport>>,
}

const INNER_SEARCH_WORKERS: usize = 1;

impl MrsAtp {
    /// Construct an `MrsAtp` (binary path is ignored now that it is in-process).
    pub fn new() -> Self {
        Self {
            binary: super::discover::find_mrs().unwrap_or_else(|| PathBuf::from("mrs")),
            use_proover_mode: true,
            reports: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn with_binary(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: binary.into(),
            use_proover_mode: true,
            reports: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn legacy_mode(mut self) -> Self {
        self.use_proover_mode = false;
        self
    }
}

impl Default for MrsAtp {
    fn default() -> Self {
        Self::new()
    }
}

impl Atp for MrsAtp {
    fn name(&self) -> &'static str {
        "mrs"
    }

    fn check_step(
        &self,
        symbols: &SymbolTable,
        premises: &[Formula],
        conclusion: &Formula,
        budget: Duration,
        _cancel: &std::sync::atomic::AtomicBool,
    ) -> AtpVerdict {
        let mut local_symbols = symbols.clone();
        let mut id_gen = ClauseIdGen::new();
        let mut all_clauses = Vec::new();

        // 1. Clausify premises (axioms)
        for (i, p) in premises.iter().enumerate() {
            let closed = close_universally(p);
            let name = format!("p{}", i);
            let clauses =
                mrs_cnf::clausify(&closed, &mut local_symbols, &mut id_gen, &name, "axiom");
            all_clauses.extend(clauses.into_iter().map(|c| c.with_distance(100)));
        }

        // 2. Clausify conclusion (negated conjecture)
        let closed_g = close_universally(conclusion);
        let negated_g = Formula::neg(closed_g);
        let clauses = mrs_cnf::clausify(
            &negated_g,
            &mut local_symbols,
            &mut id_gen,
            "g",
            "negated_conjecture",
        );
        all_clauses.extend(clauses.into_iter().map(|c| c.with_distance(0)));

        // 3. Setup robust schedule trying both KBO and LPO to handle any equality orientation differences
        let half_budget = budget / 2;
        let schedule = mrs_search::strategy::StrategySchedule {
            strategies: vec![
                (
                    mrs_search::SearchConfig {
                        time_limit: half_budget,
                        selection: mrs_search::SelectionStrategy::AgeWeight(5),
                        literal_selection: mrs_search::LiteralSelection::AllNegative,
                        ordering: mrs_search::TermOrdering::KBO,
                        ..mrs_search::SearchConfig::default()
                    },
                    half_budget,
                ),
                (
                    mrs_search::SearchConfig {
                        time_limit: half_budget,
                        selection: mrs_search::SelectionStrategy::AgeWeight(5),
                        literal_selection: mrs_search::LiteralSelection::AllNegative,
                        ordering: mrs_search::TermOrdering::LPO,
                        ..mrs_search::SearchConfig::default()
                    },
                    half_budget,
                ),
            ],
        };

        // 4. Run the in-process schedule with one search worker. The verifier
        // already parallelizes independent proof steps at the outer level;
        // nesting the full physical-core portfolio inside every step causes
        // severe CPU oversubscription and timing variance.
        let (result, report) = mrs_search::strategy::run_schedule(
            &all_clauses,
            &[],
            id_gen,
            &schedule,
            &local_symbols,
            mrs_search::strategy::MlOptions::default(),
            Some(INNER_SEARCH_WORKERS),
        );
        if let Ok(mut reports) = self.reports.lock() {
            reports.push(report);
        }

        match result {
            SearchResult::Refutation(..) => AtpVerdict::Sound,
            SearchResult::Saturated => AtpVerdict::Unknown,
            SearchResult::Timeout => AtpVerdict::Unknown,
            SearchResult::GaveUp => AtpVerdict::Unknown,
        }
    }

    fn search_reports(&self) -> Vec<mrs_search::ScheduleReport> {
        self.reports
            .lock()
            .map(|mut reports| std::mem::take(&mut *reports))
            .unwrap_or_default()
    }
}

static NEXT_TMP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Render premises and conclusion into one FOF problem string.
pub fn build_fof_problem(
    symbols: &SymbolTable,
    premises: &[Formula],
    conclusion: &Formula,
) -> String {
    let mut out = String::new();
    out.push_str("%----- mrs-proover step query -----\n");
    for (i, p) in premises.iter().enumerate() {
        // Free variables in proof steps are implicitly universally quantified;
        // TPTP semantics at top level treat free FOF variables the same way,
        // but to be safe we close over any explicit free vars.
        let closed = close_universally(p);
        out.push_str(&format!("fof(p{i}, axiom, {}).\n", closed.display(symbols)));
    }
    let closed_g = close_universally(conclusion);
    out.push_str(&format!(
        "fof(g, conjecture, {}).\n",
        closed_g.display(symbols)
    ));
    out
}

fn close_universally(f: &Formula) -> Formula {
    let mut fv: Vec<u32> = f.free_vars().into_iter().collect();
    fv.sort();
    let mut cur = f.clone();
    for v in fv.into_iter().rev() {
        cur = Formula::forall(v, cur);
    }
    cur
}

fn run_atp(
    binary: &std::path::Path,
    args: &[&str],
    problem: &str,
    budget: Duration,
    cancel: &std::sync::atomic::AtomicBool,
) -> AtpVerdict {
    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
        return AtpVerdict::Unknown;
    }
    let mut command = Command::new(binary);
    command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(unix)]
    // Put the ATP and any descendants it launches in their own process group
    // so timeout/cancellation cleanup cannot leave inherited pipe handles.
    unsafe {
        command.pre_exec(|| {
            if libc::setpgid(0, 0) == -1 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
    let Ok(mut child) = command.spawn() else {
        return AtpVerdict::Unknown;
    };
    // Feed stdin from a thread so a full kernel pipe buffer cannot
    // deadlock us before the child has read it.
    let stdin_handle = if let Some(mut stdin) = child.stdin.take() {
        let buf = problem.as_bytes().to_vec();
        Some(thread::spawn(move || {
            let _ = stdin.write_all(&buf);
        }))
    } else {
        None
    };
    let (drain_handle, stdout_bytes) = drain_to_bytes(child.stdout.take());
    let (verdict, stdout) = wait_with_timeout(
        &mut child,
        budget,
        stdin_handle,
        drain_handle,
        stdout_bytes,
        cancel,
    );
    maybe_debug_dump(binary, args, problem, &stdout, verdict);
    verdict
}

/// Spawn a background thread that reads the child's stdout into a
/// shared byte buffer. Returns a join handle and the shared buffer.
///
/// Draining stdout while the child runs prevents pipe-full deadlocks on
/// long-running provers that produce lots of output (e.g. vampire with
/// `--proof on`). Reading concurrently also means we already have the
/// SZS line buffered when the child exits, with no extra wait.
fn drain_to_bytes(
    mut stdout: Option<std::process::ChildStdout>,
) -> (
    Option<thread::JoinHandle<()>>,
    std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
) {
    let buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::with_capacity(4096)));
    let handle = if let Some(s) = stdout.take() {
        let buf_clone = buf.clone();
        let h = thread::spawn(move || {
            let mut local = Vec::with_capacity(4096);
            let mut reader = s;
            let _ = reader.read_to_end(&mut local);
            if let Ok(mut guard) = buf_clone.lock() {
                guard.extend_from_slice(&local);
            }
        });
        Some(h)
    } else {
        None
    };
    (handle, buf)
}

/// Wait for `child` to exit, or kill it once `budget` elapses.
///
/// Returns the parsed SZS verdict and the accumulated stdout (as a
/// `String`). If the child is killed for exceeding the budget, we still
/// parse whatever stdout was emitted before the kill — some provers
/// stream the SZS status line early and only spend the remaining budget
/// on the proof body, so we can still recover a verdict in that case.
///
/// Polling uses an **adaptive** schedule: fast 1 ms polls for the first
/// 50 ms (so easy steps that resolve in a few ms aren't penalised by
/// idle wait), then exponential backoff up to 25 ms. With 1000+ ATP
/// calls per proof, a flat 25 ms poll would otherwise add 25 seconds
/// of pure idle time, dwarfing the actual prover work and silently
/// exhausting the wall budget. A 200 ms grace is added on top of
/// `budget` so the child has a chance to exit cleanly on its own
/// (avoiding spurious SIGKILLs when the prover's internal timer fires
/// at the exact deadline).
fn wait_with_timeout(
    child: &mut Child,
    budget: Duration,
    stdin_handle: Option<thread::JoinHandle<()>>,
    drain_handle: Option<thread::JoinHandle<()>>,
    stdout_bytes: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    cancel: &std::sync::atomic::AtomicBool,
) -> (AtpVerdict, String) {
    let started = Instant::now();
    let deadline = started + budget;
    let mut poll = Duration::from_millis(1);
    let max_poll = Duration::from_millis(25);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if cancel.load(std::sync::atomic::Ordering::Relaxed) || Instant::now() >= deadline {
                    terminate_child(child);
                    break;
                }
                let remaining = deadline.saturating_duration_since(Instant::now());
                thread::sleep(poll.min(remaining));
                // Exponential backoff once we're past the fast-poll window.
                if started.elapsed() > Duration::from_millis(50) && poll < max_poll {
                    poll = std::cmp::min(poll * 2, max_poll);
                }
            }
            Err(_) => {
                terminate_child(child);
                return (AtpVerdict::Unknown, String::new());
            }
        }
    }
    // Give the stdout drain thread a brief moment to flush after the
    // child exits, then read whatever we've accumulated.
    if let Some(h) = drain_handle {
        let _ = h.join();
    }
    if let Some(h) = stdin_handle {
        let _ = h.join();
    }
    let bytes = match stdout_bytes.lock() {
        Ok(g) => g.clone(),
        Err(_) => Vec::new(),
    };
    let stdout = String::from_utf8_lossy(&bytes).into_owned();
    let verdict = parse_szs(&stdout);
    (verdict, stdout)
}

fn terminate_child(child: &mut Child) {
    #[cfg(unix)]
    {
        let process_group = -(child.id() as libc::pid_t);
        // The direct child fallback below handles races where the group was
        // already reaped or process-group setup was unavailable.
        unsafe {
            let _ = libc::kill(process_group, libc::SIGKILL);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Debug hook: when MRS_DEBUG_ATP is set, dump every problem we send
/// along with the ATP's verdict and full stdout, so we can root-cause
/// ATP-refuted (Unsound) VerifiedBad verdicts. Files land in
/// $MRS_DEBUG_ATP_DIR (default /tmp/opencode/atp-debug) with a unique
/// id per call.
fn maybe_debug_dump(
    binary: &std::path::Path,
    args: &[&str],
    problem: &str,
    stdout: &str,
    verdict: AtpVerdict,
) {
    if std::env::var("MRS_DEBUG_ATP").is_err() {
        return;
    }
    let dir = std::env::var("MRS_DEBUG_ATP_DIR")
        .unwrap_or_else(|_| "/tmp/opencode/atp-debug".to_string());
    let _ = std::fs::create_dir_all(&dir);
    let id = NEXT_TMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let bin = binary.file_name().and_then(|s| s.to_str()).unwrap_or("atp");
    let tag = match verdict {
        AtpVerdict::Sound => "sound",
        AtpVerdict::Unsound => "unsound",
        AtpVerdict::Unknown => "unknown",
    };
    let path = format!("{dir}/{bin}-{tag}-{id:06}.p");
    let mut buf = String::new();
    buf.push_str("% args:");
    for a in args {
        buf.push(' ');
        buf.push_str(a);
    }
    buf.push('\n');
    buf.push_str(&format!("% verdict: {tag}\n"));
    buf.push_str("% --- stdout ---\n");
    for line in stdout.lines() {
        buf.push_str("% ");
        buf.push_str(line);
        buf.push('\n');
    }
    buf.push_str("% --- problem ---\n");
    buf.push_str(problem);
    let _ = std::fs::write(&path, buf);
}

/// Parse an SZS status from any line of the prover's stdout.
pub fn parse_szs(stdout: &str) -> AtpVerdict {
    for line in stdout.lines() {
        // Tolerate lines like "% SZS status Theorem for problem"
        let lower = line.to_ascii_lowercase();
        if !lower.contains("szs status") {
            continue;
        }
        if lower.contains("theorem")
            || lower.contains("unsatisfiable")
            || lower.contains("contradictoryaxioms")
        {
            return AtpVerdict::Sound;
        }
        if lower.contains("countersatisfiable") || lower.contains("satisfiable") {
            // Be careful: "Unsatisfiable" already returned above, so a leftover
            // "satisfiable" here is the bare/CounterSatisfiable case.
            return AtpVerdict::Unsound;
        }
        if lower.contains("timeout")
            || lower.contains("gaveup")
            || lower.contains("unknown")
            || lower.contains("resourceout")
        {
            return AtpVerdict::Unknown;
        }
    }
    AtpVerdict::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_theorem() {
        assert_eq!(parse_szs("% SZS status Theorem for q\n"), AtpVerdict::Sound);
    }
    #[test]
    fn parse_unsat() {
        assert_eq!(
            parse_szs("% SZS status Unsatisfiable for q\n"),
            AtpVerdict::Sound
        );
    }
    #[test]
    fn parse_countersat() {
        assert_eq!(
            parse_szs("% SZS status CounterSatisfiable for q\n"),
            AtpVerdict::Unsound
        );
    }
    #[test]
    fn parse_timeout() {
        assert_eq!(parse_szs("% SZS status Timeout\n"), AtpVerdict::Unknown);
    }
    #[test]
    fn parse_unknown_when_nothing() {
        assert_eq!(parse_szs(""), AtpVerdict::Unknown);
    }

    #[test]
    fn mrs_atp_uses_single_inner_search_worker() {
        let mut symbols = SymbolTable::new();
        let predicate = symbols.intern("p");
        let constant = symbols.intern("a");
        let premise = Formula::atom(mrs_core::Atom::pred(
            predicate,
            vec![mrs_core::Term::constant(constant)],
        ));
        let atp = MrsAtp::new();
        let verdict = atp.check_step(
            &symbols,
            std::slice::from_ref(&premise),
            &premise,
            Duration::from_secs(1),
            &std::sync::atomic::AtomicBool::new(false),
        );
        assert_eq!(verdict, AtpVerdict::Sound);
        let second_verdict = atp.check_step(
            &symbols,
            std::slice::from_ref(&premise),
            &premise,
            Duration::from_secs(1),
            &std::sync::atomic::AtomicBool::new(false),
        );
        assert_eq!(second_verdict, AtpVerdict::Sound);
        let reports = atp.search_reports();
        assert_eq!(reports.len(), 2);
        assert!(
            reports
                .iter()
                .all(|report| report.workers == INNER_SEARCH_WORKERS)
        );
        assert!(atp.search_reports().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn timeout_terminates_process_group_promptly() {
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let started = Instant::now();
        let verdict = run_atp(
            std::path::Path::new("sh"),
            &["-c", "sleep 5"],
            "",
            Duration::from_millis(50),
            &cancel,
        );
        assert_eq!(verdict, AtpVerdict::Unknown);
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "timeout cleanup took too long: {:?}",
            started.elapsed()
        );
    }
}
