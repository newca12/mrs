use std::error::Error;
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::time::Duration;

use clap::Args;
use rayon::prelude::*;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use wait_timeout::ChildExt;

type ToolResult<T> = Result<T, Box<dyn Error>>;
const CORE_SMT_RLIMIT: &str = "30";
const VERIFIER_MUTATIONS: &str = include_str!("verifier_mutations.json");
const RUNTIME_MUTATIONS: &str = include_str!("runtime_mutations.json");
const VERIFIED_RUNTIME_COMPONENTS: [&str; 3] = [
    "first-order IR checker and replay refinements",
    "wire-v1 structural decoder",
    "ASCII decimal wire tokenizer and span correspondence",
];
const UNVERIFIED_RUNTIME_BOUNDARY: [&str; 6] = [
    "TPTP parser",
    "TPTP AST lowering and symbol assignment",
    "certificate discovery and normalization",
    "wire request encoding",
    "bounded caller-byte acquisition",
    "subprocess supervision and response handling",
];

#[derive(Args, Debug)]
pub struct AuditSourceArgs {
    /// Defaults to formal/superposition-checker in the current workspace.
    #[arg(long)]
    source_dir: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct RuntimeCheckArgs {
    verus: PathBuf,
    source: PathBuf,
}

#[derive(Args, Debug)]
pub struct MutationCheckArgs {
    verus: PathBuf,
    source: PathBuf,
}

#[derive(Args, Debug)]
pub struct RuntimeEvidenceArgs {
    root: PathBuf,
    binary: PathBuf,
    verus: PathBuf,
}

#[derive(Args, Debug)]
pub struct VerusCheckVersionArgs {
    manifest: PathBuf,
    verus: PathBuf,
}

#[derive(Debug, Deserialize)]
struct VerusToolchainPin {
    verus: String,
    rust_toolchain: String,
}

#[derive(Debug, PartialEq, Eq)]
struct VerusIdentity {
    version: String,
    rust_toolchain: String,
}

#[derive(Debug)]
struct ProcessOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("mrs-bench is two levels below the workspace root")
        .to_path_buf()
}

fn formal_dir() -> PathBuf {
    workspace_root().join("formal/superposition-checker")
}

fn run_capture(
    command: &mut Command,
    input: Option<&[u8]>,
    timeout: Duration,
) -> ToolResult<ProcessOutput> {
    let stdout_file = tempfile::NamedTempFile::new()?;
    let stderr_file = tempfile::NamedTempFile::new()?;
    command
        .stdout(Stdio::from(stdout_file.reopen()?))
        .stderr(Stdio::from(stderr_file.reopen()?));
    command.stdin(if input.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    let mut child = command.spawn()?;
    if let Some(input) = input {
        child
            .stdin
            .take()
            .ok_or("child stdin was not piped")?
            .write_all(input)?;
    }
    let status = match child.wait_timeout(timeout)? {
        Some(status) => status,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("command timed out after {} seconds", timeout.as_secs()).into());
        }
    };
    Ok(ProcessOutput {
        status,
        stdout: fs::read(stdout_file.path())?,
        stderr: fs::read(stderr_file.path())?,
    })
}

fn output_text(output: &ProcessOutput) -> String {
    let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    combined
}

fn hash_file(path: &Path) -> ToolResult<String> {
    Ok(hash_bytes(&fs::read(path)?))
}

fn hash_bytes(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn load_supported_rules(path: &Path) -> ToolResult<(Value, String, String)> {
    let source = String::from_utf8(fs::read(path)?)?;
    let rules = serde_json::from_str(&source)?;
    let sha256 = hash_bytes(source.as_bytes());
    Ok((rules, source, sha256))
}

fn contains_unchecked_proof_escape(line: &str) -> bool {
    let line = line.split_once("//").map_or(line, |(code, _)| code);
    if line.contains("#[verifier::external_body")
        || line.contains("#[verifier::allow")
        || line.contains("#[verus::external_body")
    {
        return true;
    }
    ["assume", "admit"].into_iter().any(|keyword| {
        line.match_indices(keyword).any(|(index, _)| {
            let before = line[..index].chars().next_back();
            let after_keyword = index + keyword.len();
            let after = line[after_keyword..].chars().next();
            let identifier_char = |character: Option<char>| {
                character.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
            };
            if identifier_char(before) || identifier_char(after) {
                return false;
            }
            line[after_keyword..].trim_start().starts_with('(')
        })
    })
}

pub fn audit_source(args: AuditSourceArgs) -> ToolResult<()> {
    let directory = args.source_dir.unwrap_or_else(formal_dir);
    let manifest = directory.join("supported-rules.json");
    let verify_script = fs::read_to_string(directory.join("verify.sh"))?.replace("\\\n", " ");
    let mut sources = walkdir::WalkDir::new(&directory)
        .into_iter()
        .map(|entry| entry.map(|entry| entry.into_path()))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .collect::<Vec<_>>();
    sources.sort();
    let mut failures = Vec::new();
    for path in sources {
        let relative_path = path
            .strip_prefix(&directory)?
            .to_string_lossy()
            .replace('\\', "/");
        let source_argument = format!("$(dirname \"$0\")/{relative_path}");
        let verified = verify_script.lines().any(|line| {
            let line = line.trim_start();
            !line.starts_with('#')
                && line.contains("run_check \"$VERUS_BIN\"")
                && line.contains("--no-cheating")
                && line.contains(&source_argument)
        });
        if !verified {
            failures.push(format!(
                "{relative_path}: missing strict Verus verification in verify.sh"
            ));
        }
        let contents = fs::read_to_string(&path)?;
        for (line_number, line) in contents.lines().enumerate() {
            if contains_unchecked_proof_escape(line) {
                failures.push(format!(
                    "{}:{}: unchecked proof escape hatch",
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or_default(),
                    line_number + 1
                ));
            }
        }
    }
    if !failures.is_empty() {
        return Err(failures.join("\n").into());
    }
    if !manifest.is_file() {
        return Err("supported-rules.json is missing".into());
    }
    let _: Value = serde_json::from_slice(&fs::read(manifest)?)?;
    Ok(())
}

fn run_wire(binary: &Path, payload: &[u8], expected: &[u8]) -> ToolResult<()> {
    let output = run_capture(
        Command::new(binary).arg("--wire-v1"),
        Some(payload),
        Duration::from_secs(10),
    )?;
    if !output.status.success() || output.stdout != expected {
        return Err(format!(
            "wire response mismatch: status={}, stdout={:?}, expected={:?}, stderr={:?}",
            output.status, output.stdout, expected, output.stderr
        )
        .into());
    }
    Ok(())
}

pub fn runtime_check(args: RuntimeCheckArgs) -> ToolResult<()> {
    let verus = args.verus.canonicalize()?;
    let source = args.source.canonicalize()?;
    let scratch = tempfile::tempdir()?;
    let binary = scratch.path().join("first-order-core");
    let reference = scratch.path().join("first-order-core.reference");
    for pass in 0..2 {
        let output = run_capture(
            Command::new(&verus)
                .args([
                    "--no-cheating",
                    "--triggers-mode",
                    "silent",
                    "--rlimit",
                    CORE_SMT_RLIMIT,
                    "--compile",
                ])
                .arg(&source)
                .arg("-o")
                .arg(&binary),
            None,
            Duration::from_secs(300),
        )?;
        if !output.status.success() {
            return Err(format!(
                "Verus runtime compilation failed:\n{}",
                output_text(&output)
            )
            .into());
        }
        if pass == 0 {
            fs::copy(&binary, &reference)?;
        } else if fs::read(&reference)? != fs::read(&binary)? {
            return Err("verified runtime artifact changed between identical builds".into());
        }
    }
    let digest = hash_file(&binary)?;
    println!("verified_runtime_sha256={digest}");
    write_runtime_evidence(RuntimeEvidenceArgs {
        root: workspace_root(),
        binary: binary.clone(),
        verus: verus.clone(),
    })?;
    let runtime = run_capture(&mut Command::new(&binary), None, Duration::from_secs(30))?;
    if !runtime.status.success() {
        return Err(format!(
            "verified runtime regression failed:\n{}",
            output_text(&runtime)
        )
        .into());
    }

    let request = b"1 2 100000 3 2 2 1 1 0 9 1 0 0 9 0 1 1 0 9 0 0 1 1 0 0 9 0 0 2 0 2 2 0 1 0 0";
    let certified = b"MRS_VERUS 1 CertifiedIR\n";
    let unknown = b"MRS_VERUS 1 Unknown\n";
    run_wire(&binary, request, certified)?;
    let malformed = [
        format!("{} 0", String::from_utf8_lossy(request)),
        String::from_utf8_lossy(request).replacen("1 2", "2 2", 1),
        String::from_utf8_lossy(request).replacen("3 2 2", "513 2 2", 1),
        "1 1 100000 1 2".to_owned(),
        "1 99 10".to_owned(),
        "-1 0 0".to_owned(),
        "1 0 0 extra".to_owned(),
    ];
    for payload in malformed {
        run_wire(&binary, payload.as_bytes(), unknown)?;
    }
    let tokens = String::from_utf8_lossy(request)
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    for end in 0..tokens.len() {
        run_wire(&binary, tokens[..end].join(" ").as_bytes(), unknown)?;
    }
    for payload in [
        b"1 0 0 \xff".as_slice(),
        b"1 0 0 +1".as_slice(),
        b"1 0 0 18446744073709551616".as_slice(),
    ] {
        run_wire(&binary, payload, unknown)?;
    }
    run_wire(&binary, &vec![b'0'; 4 * 1024 * 1024 + 1], unknown)?;

    let structural =
        b"1 2 100000 3 2 2 1 1 0 9 1 0 0 9 1 1 1 0 9 0 0 2 1 0 0 9 0 0 3 0 2 2 0 1 0 0";
    run_wire(&binary, structural, certified)?;
    let structural_text = String::from_utf8_lossy(structural);
    for (old, new) in [
        ("1 2 100000", "2 2 100000"),
        ("1 2 100000", "1 99 100000"),
        ("1 2 100000", "1 2 10000001"),
        ("3 2 2", "513 2 2"),
        ("3 2 2 1 1 0 9", "3 2 2 1 2 0 9"),
        ("1 1 0 9", "1 9 0 9"),
    ] {
        let changed = structural_text.replacen(old, new, 1);
        if changed == structural_text {
            return Err(format!("runtime wire mutation anchor missing: {old}").into());
        }
        run_wire(&binary, changed.as_bytes(), unknown)?;
    }
    run_wire(
        &binary,
        format!("{} 0", String::from_utf8_lossy(structural)).as_bytes(),
        unknown,
    )?;

    if std::env::var("MRS_VERUS_BRIDGE_TESTS").as_deref() == Ok("1") {
        let root = workspace_root();
        let bridge = run_capture(
            Command::new("cargo")
                .args([
                    "test",
                    "-p",
                    "mrs-proof-kernel",
                    "--test",
                    "verus_bridge",
                    "--",
                    "--include-ignored",
                ])
                .current_dir(&root)
                .env("MRS_VERUS_TEST_BINARY", &binary)
                .env("MRS_VERUS_TEST_SHA256", &digest),
            None,
            Duration::from_secs(300),
        )?;
        if !bridge.status.success() {
            return Err(
                format!("Verus bridge regression failed:\n{}", output_text(&bridge)).into(),
            );
        }
        let loaded_job_bridge = run_capture(
            Command::new("cargo")
                .args([
                    "test",
                    "-p",
                    "mrs-proover",
                    "--test",
                    "verus_bridge",
                    "--",
                    "--include-ignored",
                ])
                .current_dir(&root)
                .env("MRS_VERUS_TEST_BINARY", &binary)
                .env("MRS_VERUS_TEST_SHA256", &digest),
            None,
            Duration::from_secs(300),
        )?;
        if !loaded_job_bridge.status.success() {
            return Err(format!(
                "ProoVer loaded-job Verus bridge regression failed:\n{}",
                output_text(&loaded_job_bridge)
            )
            .into());
        }
        let proof = source
            .parent()
            .ok_or("formal source has no parent directory")?
            .join("fixtures/superposition-refutation-proof.p");
        for (mode, sha, expected) in [
            ("--verus-shadow", digest.as_str(), "VerifiedGood"),
            ("--verus-gate", digest.as_str(), "VerifiedGood"),
            (
                "--verus-gate",
                "0000000000000000000000000000000000000000000000000000000000000000",
                "Unknown",
            ),
        ] {
            let cli = run_capture(
                Command::new("cargo")
                    .args([
                        "run",
                        "-p",
                        "mrs-proover",
                        "--bin",
                        "mrs-proover",
                        "--",
                        mode,
                    ])
                    .arg(&binary)
                    .args(["--verus-sha256", sha])
                    .arg(&proof)
                    .current_dir(&root),
                None,
                Duration::from_secs(300),
            )?;
            if !cli.status.success() || !String::from_utf8_lossy(&cli.stdout).contains(expected) {
                return Err(format!("ProoVer Verus bridge failed: {}", output_text(&cli)).into());
            }
            if sha == digest.as_str()
                && !String::from_utf8_lossy(&cli.stderr)
                    .contains("\"checker_status\":\"prototype_non_certifying\"")
            {
                return Err(format!(
                    "ProoVer Verus report omitted the support manifest: {}",
                    output_text(&cli)
                )
                .into());
            }
        }
    }
    println!("reproducible verified runtime and DAG regressions passed");
    Ok(())
}

#[derive(Debug, Deserialize)]
struct VerifierMutation {
    name: String,
    anchor: String,
    replacement: String,
}

#[derive(Debug, Deserialize)]
struct RuntimeEdit {
    anchor: String,
    replacement: String,
}

#[derive(Debug, Deserialize)]
struct RuntimeMutation {
    name: String,
    source_file: String,
    binary_name: String,
    edits: Vec<RuntimeEdit>,
    expected_failure: Option<String>,
    wire: bool,
    no_cheating: bool,
    compile_failure_all: Option<Vec<String>>,
    compile_failure_any: Option<Vec<String>>,
    timeout_compile_s: u64,
    timeout_run_s: u64,
}

fn replace_one(source: &str, anchor: &str, replacement: &str, label: &str) -> ToolResult<String> {
    let count = source.match_indices(anchor).count();
    if count != 1 {
        return Err(format!("{label}: expected exactly one mutation anchor, found {count}").into());
    }
    Ok(source.replacen(anchor, replacement, 1))
}

fn enclosing_function_name(source: &str, position: usize) -> ToolResult<String> {
    let prefix = source
        .get(..position)
        .ok_or("mutation anchor is not on a UTF-8 boundary")?;
    let mut enclosing = None;
    for line in prefix.lines() {
        let line = line.trim_start();
        let tokens: Vec<_> = line.split_whitespace().collect();
        let Some(fn_index) = tokens.iter().position(|token| *token == "fn") else {
            continue;
        };
        if !tokens[..fn_index].iter().all(|token| {
            matches!(
                *token,
                "pub"
                    | "proof"
                    | "spec"
                    | "open"
                    | "closed"
                    | "broadcast"
                    | "tracked"
                    | "unsafe"
                    | "async"
            ) || token.starts_with("pub(")
        }) {
            continue;
        }
        let Some(declaration) = tokens.get(fn_index + 1) else {
            continue;
        };
        let name = declaration
            .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
            .next()
            .unwrap_or_default();
        if !name.is_empty() {
            enclosing = Some(name.to_owned());
        }
    }
    enclosing.ok_or_else(|| format!("no enclosing function for mutation at byte {position}").into())
}

fn run_runtime_mutation(
    verus: &Path,
    original: &str,
    temp: &Path,
    case: &RuntimeMutation,
) -> ToolResult<()> {
    let mut mutated = original.to_owned();
    for edit in &case.edits {
        mutated = replace_one(&mutated, &edit.anchor, &edit.replacement, &case.name)?;
    }
    let source_path = temp.join(&case.source_file);
    let binary_path = temp.join(&case.binary_name);
    fs::write(&source_path, mutated)?;

    if case.compile_failure_all.is_some() || case.compile_failure_any.is_some() {
        let first_edit = case.edits.first().ok_or("runtime mutation has no edits")?;
        let anchor_position = original
            .find(&first_edit.anchor)
            .ok_or("runtime mutation anchor is missing")?;
        let function = enclosing_function_name(original, anchor_position)?;
        let mut command = Command::new(verus);
        if case.no_cheating {
            command.arg("--no-cheating");
        }
        command
            .args([
                "--triggers-mode",
                "silent",
                "--rlimit",
                CORE_SMT_RLIMIT,
                "--verify-root",
                "--verify-function",
            ])
            .arg(function)
            .arg(&source_path);
        let output = run_capture(
            &mut command,
            None,
            Duration::from_secs(case.timeout_compile_s),
        )?;
        let text = output_text(&output);
        let expected = case
            .compile_failure_all
            .as_ref()
            .is_some_and(|needles| needles.iter().all(|needle| text.contains(needle)))
            || case
                .compile_failure_any
                .as_ref()
                .is_some_and(|needles| needles.iter().any(|needle| text.contains(needle)));
        if !output.status.success() && expected {
            println!("mutation rejected at compile: {}", case.name);
            return Ok(());
        }
        return Err(format!(
            "{} compile-failure mutant did not fail for the expected reason:\n{text}",
            case.name
        )
        .into());
    }

    let mut command = Command::new(verus);
    if case.no_cheating {
        command.arg("--no-cheating");
    }
    // Runtime mutants must verify before their executable behavior is tested.
    // Give those positive compile cases enough SMT budget that a solver rlimit
    // is not mistaken for a mutation caught by the verifier.
    command
        .args([
            "--triggers-mode",
            "silent",
            "--rlimit",
            CORE_SMT_RLIMIT,
            "--num-threads",
            "1",
            "--compile",
        ])
        .arg(&source_path)
        .arg("-o")
        .arg(&binary_path);
    let compile = run_capture(
        &mut command,
        None,
        Duration::from_secs(case.timeout_compile_s),
    )?;
    let compile_text = output_text(&compile);
    if !compile.status.success() {
        return Err(format!(
            "{} mutant failed to compile unexpectedly:\n{compile_text}",
            case.name
        )
        .into());
    }

    let mut command = Command::new(&binary_path);
    if case.wire {
        command.arg("--wire-v1");
        let result = run_capture(
            &mut command,
            Some(b"1 0\xff0"),
            Duration::from_secs(case.timeout_run_s),
        )?;
        if !result.status.success() || result.stdout != b"MRS_VERUS 1 Unknown\n" {
            return Err(format!(
                "{} tokenizer mutation was not rejected by the wire decoder",
                case.name
            )
            .into());
        }
    } else {
        let result = run_capture(&mut command, None, Duration::from_secs(case.timeout_run_s))?;
        let output = output_text(&result);
        if result.status.success() {
            return Err(format!("runtime accepted mutation: {}", case.name).into());
        }
        if let Some(expected) = &case.expected_failure
            && !output.contains(expected)
        {
            return Err(format!(
                "{} mutation failed for an unexpected reason; expected {expected:?}:\n{output}",
                case.name
            )
            .into());
        }
    }
    println!("runtime mutation rejected: {}", case.name);
    Ok(())
}

pub fn mutation_check(args: MutationCheckArgs) -> ToolResult<()> {
    let verus = args.verus.canonicalize()?;
    let source_path = args.source.canonicalize()?;
    let source = fs::read_to_string(&source_path)?;
    let verifier_mutations: Vec<VerifierMutation> = serde_json::from_str(VERIFIER_MUTATIONS)?;
    let runtime_mutations: Vec<RuntimeMutation> = serde_json::from_str(RUNTIME_MUTATIONS)?;
    if verifier_mutations.len() != 58 || runtime_mutations.len() != 49 {
        return Err(format!(
            "mutation inventory changed: verifier-negative={}, compiled-runtime={}",
            verifier_mutations.len(),
            runtime_mutations.len()
        )
        .into());
    }
    let temp = tempfile::tempdir()?;
    let probe = temp.path().join("cheating_probe.rs");
    fs::write(
        &probe,
        "use vstd::prelude::*;\nverus! { proof fn cheating_probe() { assume(false); } }\nfn main() {}\n",
    )?;
    let valid_probe = run_capture(
        Command::new(&verus).arg(&probe),
        None,
        Duration::from_secs(60),
    )?;
    if !valid_probe.status.success() {
        return Err(format!(
            "valid assumption probe did not verify:\n{}",
            output_text(&valid_probe)
        )
        .into());
    }
    let strict_probe = run_capture(
        Command::new(&verus)
            .args(["--no-cheating", "--triggers-mode", "silent"])
            .arg(&probe),
        None,
        Duration::from_secs(60),
    )?;
    if strict_probe.status.success() {
        return Err("Verus --no-cheating accepted an assumption probe".into());
    }
    println!("mutation rejected: Verus --no-cheating rejects a valid assumption probe");

    let workers = std::env::var("VERUS_MUTATION_WORKERS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|workers| *workers > 0)
        .unwrap_or(2);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(workers)
        .build()?;
    let results = pool.install(|| {
        verifier_mutations
            .par_iter()
            .enumerate()
            .map(|(index, mutation)| {
                let mutated = replace_one(
                    &source,
                    &mutation.anchor,
                    &mutation.replacement,
                    &mutation.name,
                )
                .map_err(|error| error.to_string())?;
                let anchor_position = source
                    .find(&mutation.anchor)
                    .ok_or_else(|| format!("{}: mutation anchor disappeared", mutation.name))?;
                let function = enclosing_function_name(&source, anchor_position)
                    .map_err(|error| error.to_string())?;
                let path = temp
                    .path()
                    .join(format!("first_order_core_mutated_{index}.rs"));
                fs::write(&path, mutated).map_err(|error| error.to_string())?;
                let output = run_capture(
                    Command::new(&verus)
                        .args([
                            "--triggers-mode",
                            "silent",
                            "--num-threads",
                            "1",
                            "--verify-root",
                            "--verify-function",
                        ])
                        .arg(function)
                        .arg(&path),
                    None,
                    Duration::from_secs(300),
                )
                .map_err(|error| error.to_string())?;
                if output.status.success() {
                    return Err(format!("Verus accepted mutation: {}", mutation.name));
                }
                Ok::<_, String>(mutation.name.clone())
            })
            .collect::<Vec<_>>()
    });
    for result in results {
        println!(
            "mutation rejected: {}",
            result.map_err(|error| -> Box<dyn Error> { error.into() })?
        );
    }
    let runtime_results = pool.install(|| {
        runtime_mutations
            .par_iter()
            .map(|case| {
                run_runtime_mutation(&verus, &source, temp.path(), case)
                    .map_err(|error| error.to_string())
            })
            .collect::<Vec<_>>()
    });
    for result in runtime_results {
        result.map_err(|error| -> Box<dyn Error> { error.into() })?;
    }
    println!("mutation gate passed: verifier_negative=58 compiled_runtime=49");
    Ok(())
}

pub fn write_runtime_evidence(args: RuntimeEvidenceArgs) -> ToolResult<()> {
    let root = args.root.canonicalize()?;
    let binary = args.binary.canonicalize()?;
    let verus = args.verus.canonicalize()?;
    let formal = root.join("formal/superposition-checker");
    let core = formal.join("first_order_core.rs");
    let toolchain_path = formal.join("verus-toolchain.json");
    let rules_path = formal.join("supported-rules.json");
    let toolchain: Value = serde_json::from_slice(&fs::read(&toolchain_path)?)?;
    let pin: VerusToolchainPin = serde_json::from_value(toolchain.clone())?;
    let (verus_identity, verus_version_output) = inspect_verus(&verus, &pin)?;
    let (supported_rules, supported_rules_json, supported_rules_sha256) =
        load_supported_rules(&rules_path)?;
    let commit = Command::new("git")
        .args([
            "-C",
            root.to_str().ok_or("workspace path is not UTF-8")?,
            "rev-parse",
            "HEAD",
        ])
        .output()?;
    if !commit.status.success() {
        return Err("git rev-parse HEAD failed".into());
    }
    let dirty = Command::new("git")
        .args([
            "-C",
            root.to_str().ok_or("workspace path is not UTF-8")?,
            "status",
            "--porcelain",
            "--",
            "formal/superposition-checker",
        ])
        .output()?;
    let evidence = json!({
        "protocol_version": 1,
        "source_commit": String::from_utf8_lossy(&commit.stdout).trim(),
        "source_dirty": !dirty.stdout.is_empty(),
        "core_sha256": hash_file(&core)?,
        "artifact_sha256": hash_file(&binary)?,
        "verus_binary_sha256": hash_file(&verus)?,
        "verus_version": verus_identity.version,
        "effective_rust_toolchain": verus_identity.rust_toolchain,
        "verus_version_output": verus_version_output.trim(),
        "toolchain": toolchain,
        "supported_rules": supported_rules,
        "supported_rules_json": supported_rules_json,
        "supported_rules_sha256": supported_rules_sha256,
        "verified_build_flags": [
            "--no-cheating",
            "--triggers-mode",
            "silent",
            "--rlimit",
            CORE_SMT_RLIMIT,
            "--compile"
        ],
        "verified_components": VERIFIED_RUNTIME_COMPONENTS,
        "unverified_boundary": UNVERIFIED_RUNTIME_BOUNDARY,
    });
    let mut sidecar = OsString::from(binary.as_os_str());
    sidecar.push(".json");
    fs::write(
        PathBuf::from(sidecar),
        format!("{}\n", serde_json::to_string_pretty(&evidence)?),
    )?;
    Ok(())
}

pub fn check_verus_version(args: VerusCheckVersionArgs) -> ToolResult<()> {
    let pin: VerusToolchainPin = serde_json::from_slice(&fs::read(&args.manifest)?)?;
    let verus = args.verus.canonicalize()?;
    let (identity, _) = inspect_verus(&verus, &pin)?;
    println!("verus_version={}", identity.version);
    println!("effective_rust_toolchain={}", identity.rust_toolchain);
    println!("verus_binary_sha256={}", hash_file(&verus)?);
    Ok(())
}

fn inspect_verus(verus: &Path, pin: &VerusToolchainPin) -> ToolResult<(VerusIdentity, String)> {
    let output = run_capture(
        Command::new(verus).arg("--version"),
        None,
        Duration::from_secs(30),
    )?;
    let text = output_text(&output);
    if !output.status.success() {
        return Err(format!("Verus --version failed:\n{text}").into());
    }
    let identity = validate_verus_identity(&text, pin)?;
    Ok((identity, text))
}

fn validate_verus_identity(output: &str, pin: &VerusToolchainPin) -> ToolResult<VerusIdentity> {
    let identity = parse_verus_identity(output)?;
    if identity.version != pin.verus {
        return Err(format!(
            "Verus version mismatch: expected {}, found {}",
            pin.verus, identity.version
        )
        .into());
    }
    if identity.rust_toolchain != pin.rust_toolchain {
        return Err(format!(
            "Verus Rust toolchain mismatch: expected {}, found {}",
            pin.rust_toolchain, identity.rust_toolchain
        )
        .into());
    }
    Ok(identity)
}

fn parse_verus_identity(output: &str) -> ToolResult<VerusIdentity> {
    let version = output
        .lines()
        .find_map(|line| line.trim().strip_prefix("Version:"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or("could not determine Verus version")?;
    let rust_toolchain = effective_toolchain(output)?;
    Ok(VerusIdentity {
        version: version.to_owned(),
        rust_toolchain: rust_toolchain.to_owned(),
    })
}

pub fn validate_json(path: &Path) -> ToolResult<()> {
    let _: Value = serde_json::from_slice(&fs::read(path)?)?;
    Ok(())
}

pub fn pin_field(path: &Path, field: &str) -> ToolResult<String> {
    let value: Value = serde_json::from_slice(&fs::read(path)?)?;
    value
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("manifest field {field:?} is missing or not a string").into())
}

pub fn effective_toolchain(version_output: &str) -> ToolResult<&str> {
    version_output
        .split_once("Toolchain:")
        .map(|(_, rest)| rest.split_whitespace().next().unwrap_or_default())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "could not determine Verus effective toolchain".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn source_audit_matches_calls_not_identifiers_or_comments() {
        assert!(contains_unchecked_proof_escape("assume(false);"));
        assert!(contains_unchecked_proof_escape("    admit (false);"));
        assert!(contains_unchecked_proof_escape(
            "#[verifier::external_body]"
        ));
        assert!(!contains_unchecked_proof_escape("assumption(false);"));
        assert!(!contains_unchecked_proof_escape("// assume(false)"));
    }

    #[test]
    fn verus_runtime_evidence_matches_the_pinned_verifier_and_toolchain() {
        let pin = VerusToolchainPin {
            verus: "0.2026.10.18.7d82994".into(),
            rust_toolchain: "1.99.1-x86_64-unknown-linux-gnu".into(),
        };
        let output = "Verus\n  Version: 0.2026.10.18.7d82994\n  Profile: release\n  Toolchain: 1.99.1-x86_64-unknown-linux-gnu (overridden by environment variable RUSTUP_TOOLCHAIN)\n";
        let identity = validate_verus_identity(output, &pin).unwrap();
        assert_eq!(identity.version, pin.verus);
        assert_eq!(identity.rust_toolchain, pin.rust_toolchain);

        let wrong_version = output.replace("0.2026.10.18.7d82994", "0.2026.10.01.other");
        assert!(
            validate_verus_identity(&wrong_version, &pin)
                .unwrap_err()
                .to_string()
                .contains("Verus version mismatch")
        );
        let wrong_toolchain = output.replace("1.99.1-x86_64", "1.98.0-x86_64");
        assert!(
            validate_verus_identity(&wrong_toolchain, &pin)
                .unwrap_err()
                .to_string()
                .contains("Verus Rust toolchain mismatch")
        );
        assert!(parse_verus_identity("Verus\n  Profile: release\n").is_err());
    }

    #[test]
    fn runtime_evidence_distinguishes_verified_wire_decode_from_host_bridge() {
        assert!(VERIFIED_RUNTIME_COMPONENTS.contains(&"wire-v1 structural decoder"));
        assert!(
            VERIFIED_RUNTIME_COMPONENTS
                .contains(&"ASCII decimal wire tokenizer and span correspondence")
        );
        assert!(
            !UNVERIFIED_RUNTIME_BOUNDARY
                .iter()
                .any(|boundary| boundary.contains("wire decoder"))
        );
        assert!(UNVERIFIED_RUNTIME_BOUNDARY.contains(&"wire request encoding"));
        assert!(UNVERIFIED_RUNTIME_BOUNDARY.contains(&"bounded caller-byte acquisition"));
    }

    #[test]
    fn support_manifest_evidence_hashes_the_exact_json_source() {
        let directory = tempfile::tempdir().unwrap();
        let manifest = directory.path().join("supported-rules.json");
        let contents = b"{\"schema_version\":1,\"rules\":[],\"limits\":{}}\n";
        fs::write(&manifest, contents).unwrap();

        let (parsed, exact_text, sha256) = load_supported_rules(&manifest).unwrap();
        assert_eq!(parsed["schema_version"], 1);
        assert_eq!(exact_text.as_bytes(), contents);
        assert_eq!(sha256, hash_bytes(contents));
    }

    #[test]
    fn source_audit_requires_every_rust_file_in_verify_script() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("supported-rules.json"), "{}").unwrap();
        fs::write(directory.path().join("checked.rs"), "fn checked() {}\n").unwrap();
        fs::create_dir_all(directory.path().join("nested")).unwrap();
        fs::write(
            directory.path().join("nested/unverified.rs"),
            "fn unverified() {}\n",
        )
        .unwrap();
        let verify_script = directory.path().join("verify.sh");
        fs::write(
            &verify_script,
            r#"run_check "$VERUS_BIN" --no-cheating --triggers-mode silent "$(dirname "$0")/checked.rs"
"#,
        )
        .unwrap();
        let args = AuditSourceArgs {
            source_dir: Some(directory.path().to_path_buf()),
        };
        let error = audit_source(args).unwrap_err().to_string();
        assert!(error.contains("nested/unverified.rs"), "{error}");

        fs::write(
            &verify_script,
            r#"run_check "$VERUS_BIN" --no-cheating --triggers-mode silent "$(dirname "$0")/checked.rs"
run_check "$VERUS_BIN" --no-cheating --triggers-mode silent \
  "$(dirname "$0")/nested/unverified.rs"
"#,
        )
        .unwrap();
        assert!(
            audit_source(AuditSourceArgs {
                source_dir: Some(directory.path().to_path_buf()),
            })
            .is_ok()
        );
    }

    #[test]
    fn verifier_mutation_inventory_is_unique_and_complete() {
        let mutations: Vec<VerifierMutation> = serde_json::from_str(VERIFIER_MUTATIONS).unwrap();
        assert_eq!(mutations.len(), 58);
        let source = formal_source();
        for mutation in mutations {
            let position = source.find(&mutation.anchor).unwrap();
            assert!(
                !enclosing_function_name(&source, position)
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(
                source.matches(&mutation.anchor).count(),
                1,
                "{}",
                mutation.name
            );
        }
    }

    #[test]
    fn runtime_mutation_inventory_has_unique_anchors() {
        let mutations: Vec<RuntimeMutation> = serde_json::from_str(RUNTIME_MUTATIONS).unwrap();
        assert_eq!(mutations.len(), 49);
        let mut source_files = HashSet::new();
        let mut binary_names = HashSet::new();
        for mutation in &mutations {
            assert!(
                source_files.insert(mutation.source_file.as_str()),
                "duplicate runtime-mutation source file: {}",
                mutation.name
            );
            assert!(
                binary_names.insert(mutation.binary_name.as_str()),
                "duplicate runtime-mutation binary name: {}",
                mutation.name
            );
        }
        let source = formal_source();
        for mutation in mutations {
            let mut mutated = source.to_owned();
            for edit in mutation.edits {
                assert_eq!(
                    mutated.matches(&edit.anchor).count(),
                    1,
                    "{}",
                    mutation.name
                );
                mutated = mutated.replacen(&edit.anchor, &edit.replacement, 1);
            }
            assert_ne!(mutated, source, "{}", mutation.name);
        }
    }

    fn formal_source() -> String {
        fs::read_to_string(formal_dir().join("first_order_core.rs")).unwrap()
    }
}
