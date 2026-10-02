//! Dump the pre-phase feature vector for every `.p` file under a corpus root.
//!
//! One CSV row per problem, columns fixed by
//! [`mrs_prephase::Analysis::columns`]. A problem that fails to parse or fails to
//! lower gets a row with `parse_status` explaining why rather than being
//! silently dropped: "we cannot even represent this input" is itself a routing
//! decision, and the size of that class is a result of the study.
//!
//! ```text
//! prephase_dump --root ~/TPTP-v9.3.0/Problems --out features.csv
//! prephase_dump --root crates/mrs-bench/problems/casc-30 --out casc30.csv --jobs 2
//! ```
//!
//! `%include` resolution follows `$TPTP` exactly as `mrs` does, so the clause
//! view matches a real run.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Parsing and clausification recurse to the depth of the input; the prover
/// sizes its own threads for exactly this reason.
const RECURSION_STACK_BYTES: usize = mrs_core::RECURSION_STACK_BYTES;

struct Row {
    path: String,
    /// One of `ok`, `empty_after_lowering`, `parse_error`, `unreadable`,
    /// `over_size_limit`, `resource_limit`, `isolated_spawn_failed`,
    /// `isolated_wait_failed`.
    status: String,
    detail: String,
    csv: Option<String>,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut root: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut jobs = num_cpus::get().max(1);
    let mut limit: Option<usize> = None;
    let mut filter: Option<String> = None;
    // Per-file address-space ceiling. `None` means analyse in-process.
    let mut rlimit_mb: Option<u64> = None;
    let mut child: Option<String> = None;
    // Inputs above this size are recorded as skipped rather than analysed. A
    // 13 MB TPTP file expands to millions of subterms, and the analysis is
    // O(subterms) in time; without a ceiling one such file decides whether the
    // whole corpus dump finishes. The refusals are reported, so the ceiling
    // cannot silently hide a class of problems.
    let mut max_bytes: u64 = 64 * 1024 * 1024;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                root = Some(PathBuf::from(
                    args.get(index + 1).expect("--root needs a path"),
                ));
                index += 2;
            }
            "--out" => {
                out = Some(PathBuf::from(
                    args.get(index + 1).expect("--out needs a path"),
                ));
                index += 2;
            }
            "--jobs" => {
                jobs = args
                    .get(index + 1)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1);
                index += 2;
            }
            "--limit" => {
                limit = args.get(index + 1).and_then(|v| v.parse().ok());
                index += 2;
            }
            "--max-bytes" => {
                max_bytes = args[index + 1].parse().expect("--max-bytes needs a number");
                index += 2;
            }
            "--rlimit-mb" => {
                rlimit_mb = Some(args[index + 1].parse().expect("--rlimit-mb needs a number"));
                index += 2;
            }
            "--child" => {
                child = Some(args[index + 1].clone());
                index += 2;
            }
            "--filter" => {
                filter = args.get(index + 1).cloned();
                index += 2;
            }
            "--help" | "-h" => {
                println!(
                    "Usage: prephase_dump --root <DIR|FILE> --out <CSV>\n\
                     \x20         [--jobs N] [--limit N] [--filter SUBSTR] [--max-bytes N]"
                );
                return;
            }
            other => {
                eprintln!("Unknown argument {other}");
                process_exit(1);
            }
        }
    }
    // Child mode: analyse exactly one file, print one CSV row, exit. The parent
    // runs this under an address-space ceiling so that a problem too large for
    // this host is *recorded* as a resource limit instead of taking the whole
    // corpus dump down with it — which is exactly what happens otherwise: the
    // 2026 CASC-J13 `FNQ/HWV062+1` needs ~6 GiB to clausify, so an in-process
    // dump of that division dies at the OOM killer with no output at all.
    if let Some(path) = child {
        let row = analyze_file(Path::new(&path));
        println!(
            "{},{},{}",
            row.path,
            row.status,
            row.csv.unwrap_or_else(empty_row)
        );
        return;
    }

    let Some(root) = root else {
        eprintln!("--root is required");
        process_exit(1);
    };
    let Some(out) = out else {
        eprintln!("--out is required");
        process_exit(1);
    };

    let mut files = collect(&root);
    files.sort();
    if let Some(filter) = &filter {
        files.retain(|path| path.to_string_lossy().contains(filter.as_str()));
    }
    if let Some(limit) = limit {
        files.truncate(limit);
    }
    eprintln!(
        "[dump] {} problem files under {}",
        files.len(),
        root.display()
    );

    // A hand-rolled pool rather than rayon: parsing and clausification recurse
    // to the depth of the input, and the stack size has to be guaranteed on
    // whichever thread a file lands on. Rayon is free to run work on the
    // *calling* thread when the pool has one worker, and that thread carries
    // the default 8 MiB stack — which aborts the whole dump on a 2 MB TFF input
    // (`mrs` needs `ulimit -s unlimited` for the same reason). Owning the spawn
    // sites is what makes the size a guarantee instead of a hint.
    let next = AtomicUsize::new(0);
    let rows: Arc<Mutex<Vec<(usize, Row)>>> = Arc::new(Mutex::new(Vec::with_capacity(files.len())));
    let files = Arc::new(files);
    let max_bytes = Arc::new(max_bytes);
    let rlimit_mb = Arc::new(rlimit_mb);
    std::thread::scope(|scope| {
        for _ in 0..jobs.max(1) {
            let next = &next;
            let rows = Arc::clone(&rows);
            let files = Arc::clone(&files);
            let max_bytes = Arc::clone(&max_bytes);
            let rlimit_mb = Arc::clone(&rlimit_mb);
            let builder = std::thread::Builder::new().stack_size(RECURSION_STACK_BYTES);
            let spawned = builder.spawn_scoped(scope, move || {
                loop {
                    let position = next.fetch_add(1, Ordering::Relaxed);
                    let Some(path) = files.get(position) else {
                        return;
                    };
                    let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                    let row = if size > *max_bytes {
                        Row {
                            path: path.to_string_lossy().into_owned(),
                            status: "over_size_limit".to_string(),
                            detail: format!("{size} bytes"),
                            csv: None,
                        }
                    } else if let Some(cap_mb) = *rlimit_mb {
                        analyze_isolated(path, cap_mb, jobs)
                    } else {
                        analyze_file(path)
                    };
                    rows.lock().unwrap().push((position, row));
                }
            });
            if spawned.is_err() {
                eprintln!("[dump] could not spawn a worker thread");
            }
        }
    });

    let mut rows = match Arc::try_unwrap(rows) {
        Ok(rows) => rows.into_inner().expect("row mutex is not poisoned"),
        Err(_) => unreachable!("every dump thread has joined, so the Arc is unique"),
    };
    rows.sort_by_key(|(position, _)| *position);

    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).ok();
    }
    let mut file = fs::File::create(&out).expect("cannot create output file");
    writeln!(
        file,
        "path,parse_status,parse_detail,{}",
        mrs_prephase::Analysis::columns().join(",")
    )
    .expect("cannot write header");

    let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
    for (_, row) in &rows {
        *counts.entry(row.status.clone()).or_default() += 1;
        let csv = row.csv.clone().unwrap_or_else(empty_row);
        let _ = writeln!(file, "{},{},{},{}", row.path, row.status, row.detail, csv);
    }
    eprintln!("[dump] wrote {} rows to {}", rows.len(), out.display());
    for (status, count) in counts {
        eprintln!("[dump]   {status}: {count}");
    }
}

fn process_exit(code: i32) -> ! {
    std::process::exit(code);
}

fn empty_row() -> String {
    mrs_prephase::Analysis::columns()
        .iter()
        .map(|column| match *column {
            "label"
            | "logic_class"
            | "shape_class"
            | "scale_class"
            | "goal_class"
            | "decomposition_class" => "UNPARSED".to_string(),
            "n_clauses" => "0".to_string(),
            other if other.starts_with("n_") || other.starts_with("has_") => "0".to_string(),
            "name" | "dialect" | "source_domain" => String::new(),
            _ => "0".to_string(),
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn collect(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if root.is_file() {
        out.push(root.to_path_buf());
        return out;
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("p") {
                out.push(path);
            }
        }
    }
    out
}

/// Analyse one file in a child process under an address-space ceiling.
fn analyze_isolated(path: &Path, cap_mb: u64, _jobs: usize) -> Row {
    use std::process::{Command, Stdio};
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("prephase_dump"));
    let limit_bytes = cap_mb.saturating_mul(1024 * 1024);
    let tptp = std::env::var("TPTP").unwrap_or_default();
    let mut command = Command::new(exe);
    command
        .arg("--child")
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env("TPTP", tptp);
    // `pre_exec` runs after fork and before exec, which is the only place an
    // address-space rlimit can be installed for this child alone.
    unsafe {
        use std::os::unix::process::CommandExt;
        command.pre_exec(move || {
            let limit = libc::rlimit {
                rlim_cur: limit_bytes,
                rlim_max: limit_bytes,
            };
            // RLIMIT_AS caps address space, which is what a runaway clausification
            // grows. RSS would be the wrong knob: the prover reserves worker
            // stacks up front and RLIMIT_AS charges the reservation.
            libc::setrlimit(libc::RLIMIT_AS, &limit);
            Ok(())
        });
    }
    let output = match command.spawn().and_then(|child| child.wait_with_output()) {
        Ok(output) => output,
        Err(error) => {
            return Row {
                path: path.to_string_lossy().into_owned(),
                status: "isolated_spawn_failed".to_string(),
                detail: error.to_string().replace(',', ";"),
                csv: None,
            };
        }
    };
    let owned: Option<(String, String)> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(|line| line.to_string())
        .and_then(|line| {
            let mut fields = line.splitn(4, ',');
            let _ = fields.next();
            match (fields.next(), fields.next(), fields.next()) {
                (Some(status), Some(_detail), Some(csv)) => {
                    Some((status.to_string(), csv.to_string()))
                }
                _ => None,
            }
        });
    match owned {
        Some((status, csv)) => Row {
            path: path.to_string_lossy().into_owned(),
            status: match status.as_str() {
                "ok" | "empty_after_lowering" => status,
                // A child that died is a resource outcome, not a parse outcome.
                _ => "resource_limit".to_string(),
            },
            detail: String::new(),
            csv: Some(csv),
        },
        None => Row {
            path: path.to_string_lossy().into_owned(),
            status: "resource_limit".to_string(),
            detail: format!("exit {:?} {}", output.status.code(), output.status),
            csv: None,
        },
    }
}

fn analyze_file(path: &Path) -> Row {
    if std::env::var_os("PREPHASE_TRACE_FILES").is_some() {
        eprintln!(
            "[file] {:?} {}",
            std::thread::current().name(),
            path.display()
        );
    }
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            return Row {
                path: path.to_string_lossy().into_owned(),
                status: "unreadable".to_string(),
                detail: error.to_string().replace(',', ";"),
                csv: None,
            };
        }
    };
    let problem = match mrs_tptp::parse_tptp(&text) {
        Ok(problem) => problem,
        Err(error) => {
            return Row {
                path: path.to_string_lossy().into_owned(),
                status: "parse_error".to_string(),
                detail: format!("{error}").replace(',', ";"),
                csv: None,
            };
        }
    };
    let prepared = mrs::pipeline::prepare(&problem, Some(&path.to_string_lossy()), None);
    let mut meta = prepared.meta;
    let (header_status, header_rating) = mrs_prephase::parse_header(&text);
    meta.header_status = header_status;
    meta.header_rating = header_rating;

    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();
    let analysis =
        mrs_prephase::analyze(&name, &meta, &prepared.clauses, &prepared.lowered.symbols);
    let status = if prepared.clauses.is_empty() {
        "empty_after_lowering"
    } else {
        "ok"
    };
    Row {
        path: path.to_string_lossy().into_owned(),
        status: status.to_string(),
        detail: String::new(),
        csv: Some(analysis.csv_row()),
    }
}
