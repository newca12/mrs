//! Phase 9 release/merge gate.
//!
//! The gate is intentionally repository-aware: it verifies that the release
//! branch remains based on `main`, that the focused phase anchors are present,
//! that no tracked changes are pending, and that the required Nix-wrapped
//! checks plus the committed PRV corpus validation pass.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug)]
struct Args {
    base: String,
    allow_untracked: bool,
    skip_checks: bool,
}

fn main() {
    let args = parse_args(std::env::args().skip(1).collect()).unwrap_or_else(|error| fail(&error));
    let root = repository_root().unwrap_or_else(|error| fail(&error));
    if let Err(error) = run_gate(&root, &args) {
        fail(&error);
    }
    println!("release gate: PASS");
}

fn parse_args(args: Vec<String>) -> Result<Args, String> {
    let mut base = "main".to_owned();
    let mut allow_untracked = false;
    let mut skip_checks = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--base" => base = iter.next().ok_or("--base requires a ref")?,
            "--allow-untracked" => allow_untracked = true,
            "--skip-checks" => skip_checks = true,
            "--help" | "-h" => {
                println!("release_gate [--base REF] [--allow-untracked] [--skip-checks]");
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(Args {
        base,
        allow_untracked,
        skip_checks,
    })
}

fn repository_root() -> Result<PathBuf, String> {
    let output = command("git", &["rev-parse", "--show-toplevel"], None)?;
    if !output.status.success() {
        return Err("not inside a git repository".into());
    }
    let root = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if root.is_empty() {
        Err("git returned an empty repository root".into())
    } else {
        Ok(PathBuf::from(root))
    }
}

fn run_gate(root: &Path, args: &Args) -> Result<(), String> {
    check_clean_state(args.allow_untracked)?;
    check_base(root, &args.base)?;
    check_phase_manifest(root)?;
    check_diff(root, &args.base)?;
    if !args.skip_checks {
        run_required_checks(root)?;
    }
    Ok(())
}

fn check_clean_state(allow_untracked: bool) -> Result<(), String> {
    let output = command(
        "git",
        &["status", "--porcelain", "--untracked-files=all"],
        None,
    )?;
    if !output.status.success() {
        return Err("git status failed".into());
    }
    let mut tracked = Vec::new();
    let mut untracked = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if line.starts_with("?? ") {
            untracked.push(line.to_owned());
        } else if !line.trim().is_empty() {
            tracked.push(line.to_owned());
        }
    }
    if !tracked.is_empty() {
        return Err(format!(
            "tracked worktree changes must be committed before release:\n{}",
            tracked.join("\n")
        ));
    }
    if !allow_untracked && !untracked.is_empty() {
        return Err(format!(
            "untracked files require --allow-untracked for this gate:\n{}",
            untracked.join("\n")
        ));
    }
    Ok(())
}

fn check_base(_root: &Path, base: &str) -> Result<(), String> {
    let exists = command(
        "git",
        &["rev-parse", "--verify", &format!("{base}^{{commit}}")],
        None,
    )?;
    if !exists.status.success() {
        return Err(format!("base ref does not exist: {base}"));
    }
    let ancestor = command("git", &["merge-base", "--is-ancestor", base, "HEAD"], None)?;
    if !ancestor.status.success() {
        return Err(format!("HEAD is not based on {base}"));
    }
    Ok(())
}

fn check_phase_manifest(root: &Path) -> Result<(), String> {
    let path = root.join("docs/RELEASE_PHASES.tsv");
    let text =
        fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let mut rows = 0;
    for line in text.lines() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 5 {
            return Err(format!("malformed release phase row: {line}"));
        }
        let commit = fields[2];
        let commit_exists = command(
            "git",
            &["cat-file", "-e", &format!("{commit}^{{commit}}")],
            None,
        )?;
        if !commit_exists.status.success() {
            return Err(format!("phase anchor does not exist: {commit}"));
        }
        let ancestor = command(
            "git",
            &["merge-base", "--is-ancestor", commit, "HEAD"],
            None,
        )?;
        if !ancestor.status.success() {
            return Err(format!("phase anchor is not an ancestor of HEAD: {commit}"));
        }
        rows += 1;
    }
    if rows != 9 {
        return Err(format!("expected 9 release phase rows, found {rows}"));
    }
    Ok(())
}

fn check_diff(root: &Path, base: &str) -> Result<(), String> {
    let output = command(
        "git",
        &["diff", "--check", &format!("{base}...HEAD")],
        Some(root),
    )?;
    if !output.status.success() {
        return Err("release diff contains whitespace errors".into());
    }
    Ok(())
}

fn run_required_checks(root: &Path) -> Result<(), String> {
    let checks = [
        ("cargo check", vec!["cargo", "check"]),
        (
            "cargo clippy",
            vec!["cargo", "clippy", "--all", "--", "-D", "warnings"],
        ),
        ("cargo fmt", vec!["cargo", "fmt", "--all", "--check"]),
        ("cargo test", vec!["cargo", "test", "--workspace"]),
        (
            "PRV corpus validation",
            vec![
                "cargo",
                "run",
                "-p",
                "mrs-bench",
                "--bin",
                "validate_proover2026",
                "--",
                "crates/mrs-bench/proover-corpus/Proover2026",
            ],
        ),
    ];
    for (name, command_args) in checks {
        println!("release gate: running {name}");
        let output = command("nix", &command_args_with_develop(&command_args), Some(root))?;
        if !output.status.success() {
            return Err(format!("{name} failed"));
        }
    }
    Ok(())
}

fn command_args_with_develop<'a>(args: &'a [&'a str]) -> Vec<&'a str> {
    let mut result = vec!["develop", "-c"];
    result.extend_from_slice(args);
    result
}

fn command(
    program: &str,
    args: &[&str],
    cwd: Option<&Path>,
) -> Result<std::process::Output, String> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    command
        .output()
        .map_err(|error| format!("run {program}: {error}"))
}

fn fail(message: &str) -> ! {
    eprintln!("release_gate: {message}");
    std::process::exit(1)
}
