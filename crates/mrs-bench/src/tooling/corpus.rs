use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Args;
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Args, Debug)]
pub struct FreezeArgs {
    /// Root of a materialized corpus.
    #[arg(long)]
    pub root: PathBuf,
    /// New manifest path, outside the corpus.
    #[arg(long)]
    pub output: PathBuf,
    #[arg(long, default_value_t = 42)]
    pub seed: u64,
    #[arg(long, default_value_t = 20)]
    pub holdout_percent: u8,
    /// Optional JSON object mapping relative problem paths to family names.
    #[arg(long)]
    pub family_map: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
pub struct FileEntry {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct ProblemEntry {
    pub path: String,
    pub family: String,
    pub split: String,
}

#[derive(Debug, Serialize)]
pub struct Manifest {
    pub schema: u8,
    pub seed: u64,
    pub holdout_percent: u8,
    pub family_policy: &'static str,
    pub corpus_sha256: String,
    pub files: Vec<FileEntry>,
    pub problems: Vec<ProblemEntry>,
}

pub fn freeze_command(args: FreezeArgs) -> Result<(), Box<dyn std::error::Error>> {
    if !(1..100).contains(&args.holdout_percent) {
        return Err("holdout percentage must be in 1..99".into());
    }
    let root = args.root.canonicalize()?;
    if !root.is_dir() {
        return Err(format!("corpus root is not a directory: {}", root.display()).into());
    }
    let output = resolve_path(&args.output)?;
    if output == root || output.starts_with(&root) {
        return Err("output must be outside the corpus directory".into());
    }
    let family_map = args
        .family_map
        .as_ref()
        .map(
            |path| -> Result<BTreeMap<String, String>, Box<dyn std::error::Error>> {
                let map: BTreeMap<String, String> = serde_json::from_slice(&fs::read(path)?)?;
                if map.values().any(|family| family.trim().is_empty()) {
                    return Err("family-map values must be non-empty strings".into());
                }
                Ok(map)
            },
        )
        .transpose()?;
    let manifest = freeze(&root, args.seed, args.holdout_percent, family_map.as_ref())?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)?;
    serde_json::to_writer_pretty(&mut file, &manifest)?;
    writeln!(file)?;
    Ok(())
}

fn absolute_path(path: &Path) -> Result<PathBuf, std::io::Error> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn resolve_path(path: &Path) -> Result<PathBuf, std::io::Error> {
    let absolute = absolute_path(path)?;
    let mut unresolved = Vec::new();
    let mut ancestor = absolute.as_path();
    while !ancestor.exists() {
        let name = ancestor.file_name().ok_or_else(|| {
            std::io::Error::other(format!("cannot resolve output path {}", path.display()))
        })?;
        unresolved.push(name.to_os_string());
        ancestor = ancestor.parent().ok_or_else(|| {
            std::io::Error::other(format!("cannot resolve output path {}", path.display()))
        })?;
    }
    let mut resolved = ancestor.canonicalize()?;
    for component in unresolved.into_iter().rev() {
        resolved.push(component);
    }
    Ok(resolved)
}

fn collect_files(root: &Path, directory: &Path, output: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(std::io::Error::other(format!(
                "materialize corpus symlink before freezing: {}",
                path.display()
            )));
        }
        if metadata.is_dir() {
            collect_files(root, &path, output)?;
        } else if metadata.is_file() {
            output.push(path.strip_prefix(root).unwrap().to_path_buf());
        }
    }
    Ok(())
}

pub fn split_family(family: &str, seed: u64, holdout_percent: u8) -> &'static str {
    let key = format!("mrs-corpus-split-v1\0{seed}\0{family}");
    let digest = Sha256::digest(key.as_bytes());
    let bucket = u64::from_be_bytes(digest[..8].try_into().unwrap()) % 100;
    if bucket < u64::from(holdout_percent) {
        "holdout"
    } else {
        "development"
    }
}

pub fn freeze(
    root: &Path,
    seed: u64,
    holdout_percent: u8,
    families: Option<&BTreeMap<String, String>>,
) -> Result<Manifest, Box<dyn std::error::Error>> {
    if !(1..100).contains(&holdout_percent) {
        return Err("holdout percentage must be in 1..99".into());
    }
    let mut paths = Vec::new();
    collect_files(root, root, &mut paths)?;
    paths.sort();

    let mut files = Vec::with_capacity(paths.len());
    let mut problems = Vec::new();
    for relative in paths {
        let relative = relative.to_string_lossy().replace('\\', "/");
        let path = root.join(&relative);
        let bytes = fs::read(&path)?;
        let sha256 = hex_digest(&bytes);
        files.push(FileEntry {
            path: relative.clone(),
            sha256,
            bytes: bytes.len() as u64,
        });
        if Path::new(&relative)
            .extension()
            .is_some_and(|ext| ext == "p")
        {
            let family = match families {
                Some(map) => map.get(&relative).cloned().ok_or_else(|| {
                    format!("missing/non-string family for {relative} in family map")
                })?,
                None => Path::new(&relative)
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or_default()
                    .chars()
                    .take(3)
                    .collect(),
            };
            if family.is_empty() {
                return Err(format!("missing/non-string family for {relative}").into());
            }
            problems.push(ProblemEntry {
                path: relative,
                split: split_family(&family, seed, holdout_percent).to_string(),
                family,
            });
        }
    }
    if problems.is_empty() {
        return Err("corpus contains no .p problems".into());
    }
    let splits = problems
        .iter()
        .map(|problem| problem.split.as_str())
        .collect::<BTreeSet<_>>();
    if splits.len() != 2 {
        return Err(
            "split has an empty arm; choose a larger corpus or another declared seed".into(),
        );
    }

    // The historical digest is over a compact JSON array with object keys sorted
    // alphabetically. serde_json's default map representation is ordered.
    let digest_rows = files
        .iter()
        .map(|file| {
            serde_json::json!({
                "path": file.path,
                "sha256": file.sha256,
                "bytes": file.bytes,
            })
        })
        .collect::<Vec<_>>();
    let digest_input = serde_json::to_vec(&digest_rows)?;
    Ok(Manifest {
        schema: 1,
        seed,
        holdout_percent,
        family_policy: if families.is_some() {
            "explicit-reviewed-map"
        } else {
            "tptp-domain-prefix"
        },
        corpus_sha256: hex_digest(&digest_input),
        files,
        problems,
    })
}

fn hex_digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_is_stable_and_content_identity_includes_axioms() {
        let root = tempfile::tempdir().unwrap();
        let files = ["a.p", "a-variant.p", "b.p", "include.ax"];
        for name in files {
            fs::write(root.path().join(name), "premise").unwrap();
        }
        let development_family = (0..1000)
            .map(|index| format!("family-{index}"))
            .find(|family| split_family(family, 42, 20) == "development")
            .unwrap();
        let holdout_family = (0..1000)
            .map(|index| format!("holdout-{index}"))
            .find(|family| split_family(family, 42, 20) == "holdout")
            .unwrap();
        let mut families = BTreeMap::new();
        families.insert("a.p".into(), development_family.clone());
        families.insert("a-variant.p".into(), development_family);
        families.insert("b.p".into(), holdout_family);
        let first = freeze(root.path(), 42, 20, Some(&families)).unwrap();
        let second = freeze(root.path(), 42, 20, Some(&families)).unwrap();
        assert_eq!(
            first.corpus_sha256,
            "84d7a53d9d189b608e0aacb1deddb2387efc6cb84a2d3f1a8612e7a031587539"
        );
        assert_eq!(first.corpus_sha256, second.corpus_sha256);
        let split_for = |path: &str| {
            first
                .problems
                .iter()
                .find(|problem| problem.path == path)
                .unwrap()
                .split
                .as_str()
        };
        assert_eq!(split_for("a.p"), split_for("a-variant.p"));
        assert_ne!(split_for("a.p"), split_for("b.p"));
        fs::write(root.path().join("include.ax"), "changed").unwrap();
        assert_ne!(
            first.corpus_sha256,
            freeze(root.path(), 42, 20, Some(&families))
                .unwrap()
                .corpus_sha256
        );
        assert!(freeze(root.path(), 42, 20, Some(&BTreeMap::new())).is_err());
    }

    #[test]
    fn a_single_problem_fails_closed_when_one_split_arm_is_empty() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("abc001.p"), "fof(a,axiom,p).").unwrap();
        fs::write(root.path().join("axiom.ax"), "axiom").unwrap();
        let result = freeze(root.path(), 1, 50, None).unwrap_err();
        assert!(result.to_string().contains("empty arm"));
    }
}
