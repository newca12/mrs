//! Fresh, identity-bound replay of archived proofs; never regenerates proofs.

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use clap::Args;
use serde::{Deserialize, Serialize};
use serde_json::json;
use wait_timeout::ChildExt;

use super::{autosearch, bench, corpus};

#[derive(Args, Debug)]
pub struct ReauditArgs {
    #[arg(long)]
    proof_dir: PathBuf,
    /// Immutable problem/include tree used by the archived proof header links.
    #[arg(long)]
    problem_root: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value = "target/debug/mrs-proover")]
    proover: PathBuf,
    #[arg(long, default_value_t = 30)]
    time: u64,
    #[arg(long, default_value_t = 2048)]
    memory_mb: u64,
    /// Compare only identical proof bytes against a previous report.json.
    #[arg(long)]
    previous: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    avatar_only: bool,
    /// 0 replays all selected files; a bounded subset is diagnostic only.
    #[arg(long, default_value_t = 0)]
    limit: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Record {
    proof: String,
    sha256: String,
    has_avatar: bool,
    verdict: String,
    elapsed_ms: u64,
    previous_verdict: Option<String>,
    #[serde(default)]
    input_identity: Option<serde_json::Value>,
    #[serde(default)]
    previous_input_match: Option<bool>,
    detail: String,
}

#[derive(Deserialize)]
struct Previous {
    records: Vec<Record>,
}

pub fn run(args: ReauditArgs) -> Result<(), Box<dyn Error>> {
    if args.time == 0 || !(512..=8192).contains(&args.memory_mb) {
        return Err("positive --time and --memory-mb in 512..=8192 are required".into());
    }
    let proofs = args.proof_dir.canonicalize()?;
    let problems = args.problem_root.canonicalize()?;
    let verifier = args.proover.canonicalize()?;
    if !proofs.is_dir() || !problems.is_dir() || !verifier.is_file() {
        return Err("proof/problem directories and verifier must exist".into());
    }
    if args.output.exists() {
        return Err("use a fresh --output; historical verdicts are never overwritten".into());
    }
    let parent = args
        .output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let output = parent.canonicalize()?.join(
        args.output
            .file_name()
            .ok_or("output must have a filename")?,
    );
    if output.starts_with(&proofs) || output.starts_with(&problems) {
        return Err("output must be outside the proof/problem input trees".into());
    }
    let proof_manifest = corpus::freeze_files(&proofs)?;
    let problem_manifest = corpus::freeze_files(&problems)?;
    let previous = args
        .previous
        .as_ref()
        .map(|path| -> Result<_, Box<dyn Error>> {
            let report: Previous = serde_json::from_slice(&fs::read(path)?)?;
            Ok(report
                .records
                .into_iter()
                .map(|r| ((r.proof.clone(), r.sha256.clone()), r))
                .collect::<BTreeMap<_, _>>())
        })
        .transpose()?
        .unwrap_or_default();
    fs::create_dir(&output)?;
    fs::create_dir(output.join("logs"))?;
    let identity = json!({
        "schema": 1, "verifier_sha256": bench::sha256_file(&verifier)?,
        "tool_sha256": bench::sha256_file(&std::env::current_exe()?)?,
        "proof_root": proofs, "problem_root": problems,
        "proof_manifest": proof_manifest, "problem_manifest": problem_manifest,
        "time_seconds": args.time, "memory_mb": args.memory_mb,
        "avatar_only": args.avatar_only, "limit": args.limit,
        "previous_report_sha256": args.previous.as_deref().map(bench::sha256_file).transpose()?,
        "git_commit": String::from_utf8_lossy(&Command::new("git").args(["rev-parse", "HEAD"]).output()?.stdout).trim(),
        "git_dirty": !Command::new("git").args(["status", "--porcelain", "--untracked-files=no"]).output()?.stdout.is_empty(),
        "policy": "fresh strict replay; no proof regeneration, cache reuse, or score promotion",
    });
    fs::write(
        output.join("identity.json"),
        serde_json::to_vec_pretty(&identity)?,
    )?;
    let mut records = Vec::new();
    for file in &proof_manifest.files {
        let extension = Path::new(&file.path).extension().and_then(|s| s.to_str());
        if !matches!(extension, Some("p" | "s")) {
            continue;
        }
        let text = fs::read_to_string(proofs.join(&file.path))?;
        // Many archives keep their linked Problems/ tree below the proof
        // directory. Do not mistake those source .p files for proof artifacts.
        if extension == Some("p") && mrs_tptp::proover::proof_header_link(&text).is_none() {
            continue;
        }
        let has_avatar = text.contains("avatar_") || text.contains("split_component");
        if args.avatar_only && !has_avatar {
            continue;
        }
        if args.limit > 0 && records.len() >= args.limit {
            break;
        }
        let index = records.len();
        let stdout = output.join(format!("logs/{index:05}.stdout"));
        let stderr = output.join(format!("logs/{index:05}.stderr"));
        let mut command = Command::new(&verifier);
        command
            .args(["--strict", "--audit-input-identity", "--time"])
            .arg(args.time.to_string())
            .arg("--problems-dir")
            .arg(&problems)
            .arg(proofs.join(&file.path))
            .stdout(Stdio::from(fs::File::create(&stdout)?))
            .stderr(Stdio::from(fs::File::create(&stderr)?));
        for (name, _) in std::env::vars_os() {
            let value = name.to_string_lossy();
            if value.starts_with("MRS_") || value.starts_with("TRACE_") || value == "TPTP" {
                command.env_remove(name);
            }
        }
        command.env("TPTP", &problems);
        autosearch::apply_address_space_limit(&mut command, args.memory_mb);
        let started = Instant::now();
        let mut child = command.spawn()?;
        let mut verdict = match child.wait_timeout(Duration::from_secs(args.time.saturating_add(5)))
        {
            Ok(Some(status)) => {
                let output = fs::read_to_string(&stdout)?;
                let verdict = output
                    .lines()
                    .find_map(|line| line.strip_prefix("% SZS status "))
                    .and_then(|line| line.split_whitespace().next())
                    .unwrap_or("AuditNoStatus");
                autosearch::checked_audit_verdict(verdict, status.success())
            }
            Ok(None) => {
                autosearch::terminate_child(&mut child);
                "AuditTimeout".into()
            }
            Err(error) => {
                autosearch::terminate_child(&mut child);
                return Err(error.into());
            }
        };
        let stderr_text = fs::read_to_string(&stderr)?;
        let input_identity = stderr_text
            .lines()
            .find_map(|line| line.strip_prefix("% Audit input identity: "))
            .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok());
        if verdict == "VerifiedGood"
            && !input_identity.as_ref().is_some_and(|identity| {
                identity["proof_sha256"].as_str() == Some(file.sha256.as_str())
                    && identity["problem_sha256"].is_string()
            })
        {
            verdict = "AuditMissingInputIdentity".into();
        }
        let old = previous.get(&(file.path.clone(), file.sha256.clone()));
        let detail = fs::read_to_string(&stdout)?
            .lines()
            .find(|line| line.starts_with("% SZS status "))
            .unwrap_or("")
            .chars()
            .take(2048)
            .collect();
        records.push(Record {
            proof: file.path.clone(),
            sha256: file.sha256.clone(),
            has_avatar,
            verdict,
            elapsed_ms: started.elapsed().as_millis().min(u64::MAX as u128) as u64,
            previous_verdict: old.map(|record| record.verdict.clone()),
            previous_input_match: old
                .map(|record| input_identity.is_some() && record.input_identity == input_identity),
            input_identity,
            detail,
        });
        write_report(&output, &records)?;
    }
    // Input mutation cannot silently revise the identity of a long replay.
    if corpus::freeze_files(&proofs)?.corpus_sha256 != proof_manifest.corpus_sha256
        || corpus::freeze_files(&problems)?.corpus_sha256 != problem_manifest.corpus_sha256
    {
        fs::write(
            output.join("INVALIDATED"),
            "proof/problem input tree changed during replay\n",
        )?;
        return Err("re-audit invalidated: input tree changed".into());
    }
    write_report(&output, &records)?;
    println!(
        "fresh strict replay: {} proofs; {}",
        records.len(),
        output.join("report.json").display()
    );
    Ok(())
}

fn write_report(output: &Path, records: &[Record]) -> Result<(), Box<dyn Error>> {
    let mut counts = BTreeMap::<&str, usize>::new();
    for record in records {
        *counts.entry(&record.verdict).or_default() += 1;
    }
    let report = json!({ "schema": 1, "records": records, "counts": counts,
        "changed_identical_artifacts": records.iter().filter(|r| r.previous_input_match == Some(true) && r.previous_verdict.as_ref().is_some_and(|old| old != &r.verdict)).count(),
        "interpretation": "archived-proof observations, not a fresh CASC score; no prior totals are overwritten" });
    let temporary = output.join("report.json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(&report)?)?;
    fs::rename(temporary, output.join("report.json"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn report_retains_old_verdict_and_counts_changed_identical_artifacts() {
        let output = tempfile::tempdir().unwrap();
        let records = [Record {
            proof: "forged.s".into(),
            sha256: "unchanged".into(),
            has_avatar: true,
            verdict: "VerifiedBad".into(),
            elapsed_ms: 1,
            previous_verdict: Some("VerifiedGood".into()),
            input_identity: Some(json!({"fixture": "same"})),
            previous_input_match: Some(true),
            detail: String::new(),
        }];
        write_report(output.path(), &records).unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(output.path().join("report.json")).unwrap()).unwrap();
        assert_eq!(value["changed_identical_artifacts"], 1);
        assert_eq!(value["records"][0]["previous_verdict"], "VerifiedGood");
    }
}
