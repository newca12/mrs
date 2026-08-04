//! Offline validator for the committed CASC-J13 ProoVer corpus.

use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use mrs_tptp::parse_tptp;

#[derive(Debug)]
struct Row {
    id: String,
    problem: String,
    proof: String,
    category: String,
    accepted: String,
    max_score: u32,
}

fn main() {
    let root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("crates/mrs-bench/proover-corpus/Proover2026"));
    if let Err(error) = validate(&root) {
        eprintln!("validate_proover2026: {error}");
        std::process::exit(1);
    }
}

fn validate(root: &Path) -> Result<(), String> {
    let manifest_path = root.join("manifest.tsv");
    let checksum_path = root.join("SHA256SUMS");
    let validation_report = root.join("VALIDATION.txt");
    let manifest = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("read {}: {error}", manifest_path.display()))?;
    let mut rows = Vec::new();
    let mut ids = HashSet::new();
    let mut proof_paths = HashSet::new();
    let mut problem_paths = HashSet::new();
    for (line_number, line) in manifest.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 6 {
            return Err(format!(
                "manifest line {} has {} fields",
                line_number + 1,
                fields.len()
            ));
        }
        let row = Row {
            id: fields[0].to_owned(),
            problem: fields[1].to_owned(),
            proof: fields[2].to_owned(),
            category: fields[3].to_owned(),
            accepted: fields[4].to_owned(),
            max_score: fields[5]
                .parse()
                .map_err(|_| format!("invalid score on manifest line {}", line_number + 1))?,
        };
        if !ids.insert(row.id.clone())
            || !proof_paths.insert(row.proof.clone())
            || !problem_paths.insert(row.problem.clone())
        {
            return Err(format!("duplicate manifest row or proof: {}", row.id));
        }
        validate_row(root, &row)?;
        rows.push(row);
    }

    if rows.len() != 100 {
        return Err(format!("expected 100 manifest rows, found {}", rows.len()));
    }

    let valid = rows.iter().filter(|row| row.category == "valid").count();
    let ordinary_evil = rows.iter().filter(|row| row.category == "evil").count();
    let locally_sound = rows
        .iter()
        .filter(|row| row.category == "locally_sound_evil")
        .count();
    let max_score: u32 = rows.iter().map(|row| row.max_score).sum();
    if rows.len() != 100
        || valid != 50
        || ordinary_evil + locally_sound != 50
        || locally_sound != 10
        || max_score != 150
    {
        return Err(format!(
            "manifest counts invalid: rows={}, valid={}, ordinary_evil={}, locally_sound={}, score={}",
            rows.len(),
            valid,
            ordinary_evil,
            locally_sound,
            max_score
        ));
    }
    for index in 0..100 {
        let id = format!("PRV{index:03}+1");
        let Some(row) = rows.iter().find(|row| row.id == id) else {
            return Err(format!("manifest is missing {id}"));
        };
        if row.problem != format!("Problems/{id}.p") || row.proof != format!("Proofs/{id}.s") {
            return Err(format!("manifest paths do not match {id}"));
        }
    }

    let problems = files_with_extension(&root.join("Problems"), "p")?;
    let proofs = files_with_extension(&root.join("Proofs"), "s")?;
    if problems.len() != 100 || proofs.len() != 100 {
        return Err(format!(
            "expected 100 problem and proof files, found {} and {}",
            problems.len(),
            proofs.len()
        ));
    }

    validate_checksums(root, &checksum_path)?;
    validate_metadata(&root.join("metadata.toml"))?;
    let report = fs::read_to_string(&validation_report)
        .map_err(|error| format!("read validation report: {error}"))?;
    if report.trim()
        != "manifest=100 valid=50 evil=50 locally_sound_evil=10 max_score=150 checksums=pass"
    {
        return Err("validation report does not match corpus counts".into());
    }
    println!("manifest=100 valid=50 evil=50 locally_sound_evil=10 max_score=150 checksums=pass");
    Ok(())
}

fn validate_metadata(path: &Path) -> Result<(), String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("read metadata {}: {error}", path.display()))?;
    for required in [
        "schema_version = 1",
        "corpus_id = \"proover-2026\"",
        "edition = \"CASC-J13\"",
        "problem_count = 100",
        "proof_count = 100",
        "valid_proof_count = 50",
        "evil_proof_count = 50",
        "locally_sound_mutation_count = 10",
        "maximum_score = 150",
        "manifest = \"manifest.tsv\"",
        "checksums = \"SHA256SUMS\"",
        "normalizer = \"normalize_proover2026 v1 (mrs-bench 0.2.2)\"",
        "proover_version = \"0.2.2\"",
    ] {
        if !text.lines().any(|line| line.trim() == required) {
            return Err(format!("metadata missing `{required}`"));
        }
    }
    Ok(())
}

fn validate_row(root: &Path, row: &Row) -> Result<(), String> {
    let problem = root.join(&row.problem);
    let proof = root.join(&row.proof);
    let problem_text = fs::read_to_string(&problem)
        .map_err(|error| format!("read problem {}: {error}", problem.display()))?;
    let proof_text = fs::read_to_string(&proof)
        .map_err(|error| format!("read proof {}: {error}", proof.display()))?;
    parse_tptp(&problem_text)
        .map_err(|error| format!("problem {} does not parse: {error}", row.id))?;
    parse_tptp(&proof_text).map_err(|error| format!("proof {} does not parse: {error}", row.id))?;
    match row.category.as_str() {
        "valid" if row.accepted == "VerifiedGood" && row.max_score == 1 => Ok(()),
        "evil" if row.accepted == "VerifiedBad" && row.max_score == 2 => Ok(()),
        "locally_sound_evil"
            if row.accepted == "VerifiedGood|VerifiedBad" && row.max_score == 2 =>
        {
            Ok(())
        }
        _ => Err(format!("invalid classification row for {}", row.id)),
    }
}

fn files_with_extension(root: &Path, extension: &str) -> Result<Vec<PathBuf>, String> {
    let mut files = fs::read_dir(root)
        .map_err(|error| format!("read {}: {error}", root.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == extension))
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

fn validate_checksums(root: &Path, checksum_path: &Path) -> Result<(), String> {
    let checksums = fs::read_to_string(checksum_path)
        .map_err(|error| format!("read {}: {error}", checksum_path.display()))?;
    let mut count = 0;
    for line in checksums.lines() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let (expected, relative) = line
            .split_once("  ")
            .ok_or_else(|| format!("malformed checksum line: {line}"))?;
        let path = root.join(relative);
        let actual = Sha256::digest(
            fs::read(&path).map_err(|error| format!("read {}: {error}", path.display()))?,
        );
        if hex_encode(&actual) != expected {
            return Err(format!("checksum mismatch: {}", path.display()));
        }
        count += 1;
    }
    if count != 203 {
        return Err(format!("expected 203 checksum rows, found {count}"));
    }
    Ok(())
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}
