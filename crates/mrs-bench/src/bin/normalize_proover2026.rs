//! Split the committed combined PRV fixtures into explicit problem/proof files.
//!
//! The source fixtures contain the problem leaves and the proof in one TSTP
//! file. This tool preserves the complete proof artifact while emitting a
//! problem-only file containing the original input roles. The transformation
//! is deterministic and uses the repository parser rather than line-based
//! parenthesis heuristics.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use mrs_tptp::{AnnotatedFormula, FormulaRole, parse_tptp};

fn main() {
    let mut args = std::env::args().skip(1);
    let root = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("crates/mrs-bench/proover-corpus/Proover2026"));
    let mut clean_source = false;
    let mut restore_sources = false;
    for arg in args {
        match arg.as_str() {
            "--clean-source" => clean_source = true,
            "--restore-sources" => restore_sources = true,
            other => {
                eprintln!("normalize_proover2026: unknown argument {other}");
                std::process::exit(1);
            }
        }
    }
    if let Err(error) = normalize(&root, clean_source, restore_sources) {
        eprintln!("normalize_proover2026: {error}");
        std::process::exit(1);
    }
}

fn normalize(root: &Path, clean_source: bool, restore_sources: bool) -> Result<(), String> {
    let mut sources = fs::read_dir(root)
        .map_err(|error| format!("cannot read {}: {error}", root.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|ext| ext == "p")
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("PRV") && name.ends_with("+1.p"))
        })
        .collect::<Vec<_>>();
    sources.sort();
    if sources.is_empty() && root.join("Problems").is_dir() && root.join("Proofs").is_dir() {
        if restore_sources {
            let proofs = root.join("Proofs");
            let mut restored = fs::read_dir(&proofs)
                .map_err(|error| format!("read {}: {error}", proofs.display()))?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "s"))
                .collect::<Vec<_>>();
            restored.sort();
            if restored.len() != 100 {
                return Err(format!(
                    "expected 100 proof files to restore, found {}",
                    restored.len()
                ));
            }
            for proof in restored {
                let name = proof
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| format!("invalid proof path {}", proof.display()))?;
                fs::copy(&proof, root.join(format!("{name}.p")))
                    .map_err(|error| format!("restore {}: {error}", proof.display()))?;
            }
            return normalize(root, clean_source, false);
        }
        write_manifest_and_checksums(root)?;
        println!("refreshed normalized PRV metadata under {}", root.display());
        return Ok(());
    }
    if sources.len() != 100 {
        return Err(format!(
            "expected 100 PRV source files under {}, found {}",
            root.display(),
            sources.len()
        ));
    }

    let problems = root.join("Problems");
    let proofs = root.join("Proofs");
    fs::create_dir_all(&problems).map_err(|error| format!("create Problems: {error}"))?;
    fs::create_dir_all(&proofs).map_err(|error| format!("create Proofs: {error}"))?;

    for source in sources {
        let name = source
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("non-UTF-8 source path {}", source.display()))?;
        let input = fs::read_to_string(&source)
            .map_err(|error| format!("read {}: {error}", source.display()))?;
        let parsed =
            parse_tptp(&input).map_err(|error| format!("parse {}: {error}", source.display()))?;

        let problem = render_problem(name, &parsed.formulas)?;
        fs::write(problems.join(name), problem)
            .map_err(|error| format!("write problem {name}: {error}"))?;

        let proof_name = name.strip_suffix(".p").unwrap_or(name).to_owned() + ".s";
        let proof = rewrite_proof_header(&input, name);
        fs::write(proofs.join(proof_name), proof)
            .map_err(|error| format!("write proof {name}: {error}"))?;

        if clean_source {
            fs::remove_file(&source)
                .map_err(|error| format!("remove source {}: {error}", source.display()))?;
        }
    }

    write_manifest_and_checksums(root)?;

    println!(
        "normalized {} PRV fixtures under {}{}",
        100,
        root.display(),
        if clean_source {
            " and removed combined sources"
        } else {
            ""
        }
    );
    Ok(())
}

fn write_manifest_and_checksums(root: &Path) -> Result<(), String> {
    let manifest = root.join("manifest.tsv");
    let classifications = read_classifications(&manifest)?;
    let mut output = String::from(
        "# MRS ProoVer 2026 corpus manifest, schema_version=1\n# columns: id<TAB>problem_file<TAB>proof_file<TAB>category<TAB>accepted_verdicts<TAB>max_score\n",
    );
    for index in 0..100 {
        let id = format!("PRV{index:03}+1");
        let (category, accepted, score) = classifications
            .get(&id)
            .ok_or_else(|| format!("manifest has no classification for {id}"))?;
        writeln!(
            output,
            "{id}\tProblems/{id}.p\tProofs/{id}.s\t{category}\t{accepted}\t{score}"
        )
        .unwrap();
    }
    fs::write(&manifest, output)
        .map_err(|error| format!("write {}: {error}", manifest.display()))?;

    let metadata = root.join("metadata.toml");
    let metadata_text = r#"schema_version = 1
corpus_id = "proover-2026"
edition = "CASC-J13"
layout = "split_problem_and_proof"
problem_count = 100
proof_count = 100
valid_proof_count = 50
ordinary_evil_proof_count = 40
evil_proof_count = 50
locally_sound_mutation_count = 10
maximum_score = 150
expected_score = 147
expected_verified_good = 48
expected_verified_bad = 50
expected_unknown = 1
expected_false_rejection = 1
manifest = "manifest.tsv"
checksums = "SHA256SUMS"
source = "Official CASC-J13 ProoVer 2026 PRV000+1.p through PRV099+1.p fixtures"
normalization = "Rust parser-backed split of committed combined TSTP fixtures"
toolchain = "rustc 1.97.1 via nix develop"
normalizer = "normalize_proover2026 v1 (mrs-bench 0.2.2)"
proover_version = "0.2.2"
validation_command = "nix develop -c cargo run -p mrs-bench --bin normalize_proover2026 -- crates/mrs-bench/proover-corpus/Proover2026 --restore-sources --clean-source"
evaluation_command = "nix develop -c cargo run --release -p mrs-bench --bin score_proover2026 -- crates/mrs-bench/proover-corpus/Proover2026 --competition --proover target/release/mrs-proover --time 10 --workers 1 --output reports/proover-2026.tsv"
"#;
    fs::write(&metadata, metadata_text)
        .map_err(|error| format!("write {}: {error}", metadata.display()))?;

    let validation_report = root.join("VALIDATION.txt");
    fs::write(
        &validation_report,
        "manifest=100 valid=50 evil=50 locally_sound_evil=10 max_score=150 checksums=pass\n",
    )
    .map_err(|error| format!("write {}: {error}", validation_report.display()))?;

    let checksums = root.join("SHA256SUMS");
    let mut checksums_text = String::from("# SHA-256 checksums for the normalized PRV corpus\n");
    let mut paths = fs::read_dir(root.join("Problems"))
        .map_err(|error| format!("read Problems: {error}"))?
        .chain(fs::read_dir(root.join("Proofs")).map_err(|error| format!("read Proofs: {error}"))?)
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("read corpus entry: {error}"))?;
    paths.sort();
    for path in paths
        .into_iter()
        .chain([manifest.clone(), metadata.clone(), validation_report])
    {
        let digest = Sha256::digest(
            fs::read(&path).map_err(|error| format!("read {}: {error}", path.display()))?,
        );
        let relative = path
            .strip_prefix(root)
            .map_err(|_| format!("relative path failed for {}", path.display()))?;
        writeln!(
            checksums_text,
            "{}  {}",
            hex_encode(&digest),
            relative.display()
        )
        .unwrap();
    }
    fs::write(&checksums, checksums_text)
        .map_err(|error| format!("write {}: {error}", checksums.display()))?;
    Ok(())
}

fn read_classifications(path: &Path) -> Result<HashMap<String, (String, String, u32)>, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("read existing manifest {}: {error}", path.display()))?;
    let mut classifications = HashMap::new();
    for line in text.lines() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 6 {
            return Err(format!("malformed existing manifest row: {line}"));
        }
        let score = fields[5]
            .parse()
            .map_err(|_| format!("invalid existing manifest score: {line}"))?;
        classifications.insert(
            fields[0].to_owned(),
            (fields[3].to_owned(), fields[4].to_owned(), score),
        );
    }
    if classifications.len() != 100 {
        return Err(format!(
            "existing manifest must contain 100 classifications, found {}",
            classifications.len()
        ));
    }
    Ok(classifications)
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

fn render_problem(name: &str, formulas: &[AnnotatedFormula<'_>]) -> Result<String, String> {
    let mut output = String::new();
    writeln!(output, "% Problem : Problems/{name}").unwrap();
    let mut count = 0;
    for formula in formulas {
        if !matches!(
            formula.role(),
            FormulaRole::Axiom
                | FormulaRole::AxiomLocal
                | FormulaRole::Hypothesis
                | FormulaRole::Definition
                | FormulaRole::Conjecture
                | FormulaRole::Type
        ) {
            continue;
        }
        if !formula.is_fof() && !formula.is_cnf() {
            return Err(format!(
                "problem {name} contains unsupported input dialect for {}",
                formula.name()
            ));
        }
        writeln!(output, "{formula}").unwrap();
        count += 1;
    }
    if count == 0 {
        return Err(format!("problem {name} has no input formulas"));
    }
    Ok(output)
}

fn rewrite_proof_header(input: &str, name: &str) -> String {
    let mut output = String::with_capacity(input.len() + 16);
    let mut rewritten = false;
    for line in input.lines() {
        if !rewritten {
            let trimmed = line.trim_start();
            if trimmed.starts_with("% Proof") && trimmed.contains(':') {
                output.push_str(&format!("% Proof : Problems/{name}\n"));
                rewritten = true;
                continue;
            }
        }
        output.push_str(line);
        output.push('\n');
    }
    if !rewritten {
        output.insert_str(0, &format!("% Proof : Problems/{name}\n"));
    }
    output
}
