//! `mrs-proover` CLI entrypoint.
//!
//! Usage:
//!
//! ```text
//! mrs-proover [--problems-dir DIR] [--no-atp] [--verbose] <proof.p>
//! ```
//!
//! Emits one line on stdout: `% SZS status Verified|FailedVerified|NotVerified`.
//! With `--verbose`, also writes per-step progress to stderr.
//!
//! By default, the verifier auto-discovers `eprover` and `vampire` in the
//! repo's `crates/mrs-bench/systems/*/bin/` directory and uses them as a
//! ladder ATP backend for inference steps not covered by internal checks.
//! Use `--no-atp` to disable external ATP calls.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use mrs_proover::atp::{
    EProverAtp, LadderAtp, MrsAtp, NoopAtp, VampireAtp, VampireFmbAtp, find_eprover, find_vampire,
};
use mrs_proover::load::{LoadError, load};
use mrs_proover::verdict::Verdict;
use mrs_proover::verify::{Settings, verify_with};

fn print_and_exit(v: Verdict) -> ExitCode {
    println!("{}", v.as_szs_line());
    ExitCode::SUCCESS
}

fn usage() -> ExitCode {
    eprintln!(
        "usage: mrs-proover [--problems-dir DIR] [--no-atp] [--no-mrs] [--no-fmb]\n\
                      [--eprover PATH] [--vampire PATH]\n\
                      [--only-mrs|--only-eprover|--only-vampire]\n\
                      [--time SECS] [--verbose] <proof.p>"
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut proof_path: Option<PathBuf> = None;
    let mut problems_dir: Option<PathBuf> = None;
    let mut no_atp = false;
    let mut no_mrs = false;
    let mut no_fmb = false;
    let mut eprover_override: Option<PathBuf> = None;
    let mut vampire_override: Option<PathBuf> = None;
    let mut verbose = false;
    let mut total_budget_secs: Option<u64> = None;
    let mut only: Option<&'static str> = None;
    let mut iter = args.into_iter();
    while let Some(a) = iter.next() {
        match a.as_str() {
            "-h" | "--help" => return usage(),
            "--problems-dir" => match iter.next() {
                Some(v) => problems_dir = Some(PathBuf::from(v)),
                None => return usage(),
            },
            "--no-atp" => no_atp = true,
            "--no-mrs" => no_mrs = true,
            "--no-fmb" => no_fmb = true,
            "--only-mrs" => only = Some("mrs"),
            "--only-eprover" => only = Some("eprover"),
            "--only-vampire" => only = Some("vampire"),
            "--eprover" => match iter.next() {
                Some(v) => eprover_override = Some(PathBuf::from(v)),
                None => return usage(),
            },
            "--vampire" => match iter.next() {
                Some(v) => vampire_override = Some(PathBuf::from(v)),
                None => return usage(),
            },
            "--time" => match iter.next().and_then(|s| s.parse::<u64>().ok()) {
                Some(n) => total_budget_secs = Some(n),
                None => return usage(),
            },
            "-v" | "--verbose" => verbose = true,
            s if s.starts_with("--") => return usage(),
            _ => {
                if proof_path.is_some() {
                    return usage();
                }
                proof_path = Some(PathBuf::from(a));
            }
        }
    }
    let Some(proof_path) = proof_path else {
        return usage();
    };

    let job = match load(&proof_path, problems_dir.as_deref()) {
        Ok(j) => j,
        Err(LoadError::MissingProofHeader) => {
            return print_and_exit(Verdict::NotVerified(
                "proof file has no `% Proof :` header; cannot locate problem file".into(),
            ));
        }
        // A proof file that doesn't parse as TPTP is *by definition* a bad
        // proof: the ProoVer 2026 rules promise "All provided proofs will be
        // syntactically well-formed and parsable in TPTP", and the
        // official `example3_e_proof.p` evil example contains a malformed
        // `skolemize(Groom sK0(Marriage))` (missing comma) that should be
        // flagged as bad. Reporting `FailedVerified` scores +2 instead of
        // 0 in that case; the rules' guarantee means we should never
        // wrongly hit this branch on a legitimate good proof.
        Err(LoadError::ParseProof(detail)) => {
            return print_and_exit(Verdict::FailedVerified(format!(
                "proof file is not parseable TPTP: {detail}"
            )));
        }
        // ReadProof / ReadProblem / ParseProblem are infrastructure or
        // problem-file issues, not proof faults; stay conservative.
        Err(e) => {
            return print_and_exit(Verdict::NotVerified(format!("load error: {e}")));
        }
    };

    let mut settings = Settings {
        verbose,
        ..Settings::default()
    };
    if let Some(s) = total_budget_secs {
        settings.total_budget = Duration::from_secs(s);
    }

    if no_atp {
        let atp = NoopAtp;
        return print_and_exit(verify_with(&job, &settings, &atp));
    }

    // Build ladder: in-process mrs first (cheapest), then eprover, then vampire.
    // `--only-<backend>` overrides everything else.
    let mut ladder = LadderAtp::new();
    let pick = |name: &str| match only {
        Some(o) => o == name,
        None => match name {
            "mrs" => !no_mrs,
            _ => true,
        },
    };
    if pick("mrs") {
        ladder = ladder.push(Box::new(MrsAtp::new()));
    }
    if pick("eprover") {
        if let Some(p) = eprover_override.or_else(find_eprover) {
            ladder = ladder.push(Box::new(EProverAtp::new(p)));
        }
    }
    if pick("vampire") {
        if let Some(p) = vampire_override.clone().or_else(find_vampire) {
            ladder = ladder.push(Box::new(VampireAtp::new(p)));
        }
    }
    // Counter-model finder rung (last): only when not in single-backend mode
    // and not explicitly disabled. FMB confirms non-entailments the saturation
    // provers can only time out on, earning FailedVerified (+2) on bad proofs.
    // The esa guard in `delegate_to_atp` still suppresses any FMB refutation of
    // an equisatisfiability step, so this never costs us a good-proof point.
    if only.is_none()
        && !no_fmb
        && let Some(p) = vampire_override.or_else(find_vampire)
    {
        ladder = ladder.push(Box::new(VampireFmbAtp::new(p)));
    }
    let v = if ladder.backends.is_empty() {
        let atp = NoopAtp;
        verify_with(&job, &settings, &atp)
    } else {
        verify_with(&job, &settings, &ladder)
    };
    print_and_exit(v)
}
