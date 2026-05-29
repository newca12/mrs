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
        )
    }
}

/// Backend that calls the in-tree `mrs` binary as a subprocess.
///
/// `mrs` is the cheapest rung of the ladder because it is purpose-built for
/// short FOF problems and reports back a plain SZS line. Requires `mrs` to
/// be built with the `proover` feature, which adds `--quiet` and stdin (`-`)
/// input. The `--schedule fast` flag is unconditional (works on any mrs
/// build). Falls back to a tempfile if the feature is missing, at which
/// point the only loss is some I/O overhead and noisier output — `parse_szs`
/// still extracts the SZS line correctly.
pub struct MrsAtp {
    pub binary: PathBuf,
    /// When true (default), use `--quiet --schedule fast -` (stdin). When
    /// false, fall back to the legacy tempfile-based invocation that works
    /// on any mrs build.
    pub use_proover_mode: bool,
}

impl MrsAtp {
    /// Construct an `MrsAtp` pointing at the binary we were compiled with.
    pub fn new() -> Self {
        Self {
            binary: super::discover::find_mrs().unwrap_or_else(|| PathBuf::from("mrs")),
            use_proover_mode: true,
        }
    }

    pub fn with_binary(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: binary.into(),
            use_proover_mode: true,
        }
    }

    /// Disable proover mode (stdin + `--quiet --schedule fast`). Useful for
    /// benchmarking the unmodified mrs binary.
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
    ) -> AtpVerdict {
        let problem = build_fof_problem(symbols, premises, conclusion);
        // mrs itself only accepts integer --time, so floor to 1s and
        // rely on the wall-clock kill for sub-second budgets.
        let secs = budget.as_secs().max(1).to_string();

        if self.use_proover_mode {
            // Featured mrs: read TPTP from stdin, write only the SZS line.
            return run_atp(
                &self.binary,
                &["--time", &secs, "--quiet", "--schedule", "fast", "-"],
                &problem,
                budget,
            );
        }

        // Legacy path: write a tempfile and pass it as a positional arg.
        let tmpdir = std::env::temp_dir();
        let nonce = std::process::id();
        let counter = NEXT_TMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = tmpdir.join(format!("mrs-proover-{nonce}-{counter}.p"));
        if std::fs::write(&path, problem.as_bytes()).is_err() {
            return AtpVerdict::Unknown;
        }
        let path_str = path.to_string_lossy().into_owned();
        let verdict = run_atp_file(&self.binary, &["--time", &secs, &path_str], budget);
        let _ = std::fs::remove_file(&path);
        verdict
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

fn run_atp(binary: &std::path::Path, args: &[&str], problem: &str, budget: Duration) -> AtpVerdict {
    let Ok(mut child) = Command::new(binary)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return AtpVerdict::Unknown;
    };
    // Feed stdin from a thread so a full kernel pipe buffer cannot
    // deadlock us before the child has read it.
    if let Some(mut stdin) = child.stdin.take() {
        let buf = problem.as_bytes().to_vec();
        thread::spawn(move || {
            let _ = stdin.write_all(&buf);
        });
    }
    let stdout_bytes = drain_to_bytes(child.stdout.take());
    let (verdict, stdout) = wait_with_timeout(&mut child, budget, stdout_bytes);
    maybe_debug_dump(binary, args, problem, &stdout, verdict);
    verdict
}

/// Run an ATP that reads from a file path (rather than stdin).
fn run_atp_file(binary: &std::path::Path, args: &[&str], budget: Duration) -> AtpVerdict {
    let Ok(mut child) = Command::new(binary)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return AtpVerdict::Unknown;
    };
    let stdout_bytes = drain_to_bytes(child.stdout.take());
    let (verdict, _stdout) = wait_with_timeout(&mut child, budget, stdout_bytes);
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
) -> std::sync::Arc<std::sync::Mutex<Vec<u8>>> {
    let buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::with_capacity(4096)));
    if let Some(s) = stdout.take() {
        let buf_clone = buf.clone();
        thread::spawn(move || {
            let mut local = Vec::with_capacity(4096);
            let mut reader = s;
            let _ = reader.read_to_end(&mut local);
            if let Ok(mut guard) = buf_clone.lock() {
                guard.extend_from_slice(&local);
            }
        });
    }
    buf
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
    stdout_bytes: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
) -> (AtpVerdict, String) {
    let started = Instant::now();
    let deadline = started + budget + Duration::from_millis(200);
    let mut poll = Duration::from_millis(1);
    let max_poll = Duration::from_millis(25);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    break;
                }
                thread::sleep(poll);
                // Exponential backoff once we're past the fast-poll window.
                if started.elapsed() > Duration::from_millis(50) && poll < max_poll {
                    poll = std::cmp::min(poll * 2, max_poll);
                }
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return (AtpVerdict::Unknown, String::new());
            }
        }
    }
    // Give the stdout drain thread a brief moment to flush after the
    // child exits, then read whatever we've accumulated.
    thread::sleep(Duration::from_millis(2));
    let bytes = match stdout_bytes.lock() {
        Ok(g) => g.clone(),
        Err(_) => Vec::new(),
    };
    let stdout = String::from_utf8_lossy(&bytes).into_owned();
    let verdict = parse_szs(&stdout);
    (verdict, stdout)
}

/// Debug hook: when MRS_DEBUG_ATP is set, dump every problem we send
/// along with the ATP's verdict and full stdout, so we can root-cause
/// ATP-refuted (Unsound) FailedVerified verdicts. Files land in
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
}
