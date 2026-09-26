//! Mutation sweep over MRS's own proofs.
//!
//! Two corpora guard the verifier, and this test covers the one that matters
//! most for the project's second goal — that `mrs` can certify its own proofs:
//!
//! - `evil-proofs/exploits/` (checked by `mrs-proof-kernel/tests/evil_proofs.rs`)
//!   holds hand-designed attacks;
//! - `resources/mrs_proofs/` holds proofs this prover actually produced, which
//!   the strict kernel certifies.
//!
//! For each of the latter, this test applies mechanical mutations — drop a
//! parent, duplicate one, cite a step that does not exist, demote a status,
//! rename a rule, point a leaf at another file, swap a role, turn `$false`
//! into `$true`, cite the step as its own parent, corrupt a demodulation
//! annotation — and requires that **no mutant is ever certified**. A mutant that
//! reaches `Certified` is a `-10` in the ProoVer scoring model: the single
//! failure mode a verifier cannot recover from, because it is a proof of
//! something false.
//!
//! The sweep runs the strict kernel, so it is deterministic and needs no ATP
//! binary. That is the right surface: everything the kernel decides is decided
//! before any ATP rung in competition mode, and an `Unsound` finding outranks
//! whatever a ladder could say.
//!
//! One class of mutation is deliberately *absent*: corrupting a
//! `demodulation_steps` annotation. The annotation is evidence, not the step —
//! when a recorded trace fails to replay, the kernel falls back to its bounded
//! search and still decides the step, so a corrupted trace on a derivable step
//! certifies by design. The property that does matter, that a fabricated
//! annotation cannot certify a step no rewrite sequence produces, is pinned by
//! the `recorded_demodulation_cannot_*` tests in `mrs-proof-kernel`.

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use mrs_proof_kernel::{KernelVerdict, VerificationLimits, verify_strict_with_source};
use mrs_tptp::parse_tptp;

/// The verifier's parsing is recursive descent; the competition wrappers raise
/// the worker stack for the same reason (`RUST_MIN_STACK` in
/// `systems/*/invoke.sh`). Test threads default to 2 MiB, which is not enough
/// for the larger canaries.
const WORKER_STACK: usize = 64 * 1024 * 1024;

fn canary_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/resources/mrs_proofs")
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

struct Mutation {
    name: &'static str,
    apply: fn(&str) -> Option<String>,
}

const MUTATIONS: &[Mutation] = &[
    Mutation {
        name: "drop_first_parent",
        apply: |text| {
            rewrite_parent_list(text, |list| {
                let mut parts: Vec<&str> = list.split(',').map(str::trim).collect();
                if parts.len() < 2 {
                    return None;
                }
                parts.remove(0);
                Some(parts.join(", "))
            })
        },
    },
    Mutation {
        name: "duplicate_last_parent",
        apply: |text| {
            rewrite_parent_list(text, |list| {
                let parts: Vec<&str> = list.split(',').map(str::trim).collect();
                let last = *parts.last()?;
                Some(format!("{list}, {last}"))
            })
        },
    },
    Mutation {
        name: "unknown_parent",
        apply: |text| rewrite_parent_list(text, |list| Some(format!("{list}, no_such_step"))),
    },
    Mutation {
        name: "self_parent",
        apply: |text| {
            let name = first_step_name(text)?;
            rewrite_parent_list(text, |list| Some(format!("{list}, {name}")))
        },
    },
    Mutation {
        name: "demoted_status",
        apply: |text| changed(text.replacen("status(thm)", "status(cth)", 1), text),
    },
    Mutation {
        name: "esa_status",
        apply: |text| changed(text.replacen("status(thm)", "status(esa)", 1), text),
    },
    Mutation {
        name: "renamed_rule",
        apply: |text| {
            changed(
                text.replacen("inference(", "inference(not_a_real_rule, ", 1),
                text,
            )
        },
    },
    Mutation {
        name: "demodulation_to_factoring",
        apply: |text| {
            changed(
                text.replacen("inference(demodulation,", "inference(factoring,", 1),
                text,
            )
        },
    },
    Mutation {
        name: "stripped_provenance",
        apply: |text| changed(text.replacen(", file('", ", other_file('", 1), text),
    },
    Mutation {
        name: "swapped_role",
        apply: |text| changed(text.replacen(", axiom,", ", conjecture,", 1), text),
    },
    Mutation {
        name: "false_to_true_root",
        apply: |text| changed(text.replacen("$false", "$true", 1), text),
    },
    Mutation {
        name: "flipped_literal",
        apply: |text| {
            // Negate the first literal sign in the proof body.
            let marker = text.find("% SZS output start Proof");
            let start = marker.map(|index| index + text[index..].find('\n').unwrap_or(0) + 1)?;
            let body = &text[start..];
            let head = body.find("~")?;
            let mut out = String::with_capacity(text.len() + 1);
            out.push_str(&text[..start + head]);
            out.push('~');
            out.push_str(&body[head + 1..]);
            Some(out)
        },
    },
];

fn changed(mutant: String, original: &str) -> Option<String> {
    (mutant != original).then_some(mutant)
}

fn first_step_name(text: &str) -> Option<String> {
    let start = text.find("(c")?;
    let rest = &text[start + 2..];
    let end = rest.find(',')?;
    Some(format!("c{}", &rest[..end]))
}

/// Rewrite the parent list of the first `inference(...)` record.
fn rewrite_parent_list(text: &str, rewrite: impl Fn(&str) -> Option<String>) -> Option<String> {
    let marker = text.find("inference(")?;
    let after = marker + "inference(".len();
    let mut from = after;
    // Skip the info list, which comes first in MRS's output.
    let (open, close) = bracketed(text, from)?;
    if text[open..close].contains("status(") {
        from = close;
        let (open, close) = bracketed(text, from)?;
        let replacement = rewrite(&text[open + 1..close])?;
        let mut out = String::with_capacity(text.len() + 16);
        out.push_str(&text[..open + 1]);
        out.push_str(&replacement);
        out.push_str(&text[close..]);
        return Some(out);
    }
    let replacement = rewrite(&text[open + 1..close])?;
    let mut out = String::with_capacity(text.len() + 16);
    out.push_str(&text[..open + 1]);
    out.push_str(&replacement);
    out.push_str(&text[close..]);
    Some(out)
}

/// The first balanced `[...]` group at or after `from`.
fn bracketed(text: &str, from: usize) -> Option<(usize, usize)> {
    let open = text[from..].find('[')? + from;
    let mut depth = 0usize;
    for (offset, ch) in text[open..].char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some((open, open + offset));
                }
            }
            _ => {}
        }
    }
    None
}

struct Canary {
    name: String,
    problem: String,
    proof: String,
    source: String,
}

fn load_canaries() -> Vec<Canary> {
    let dir = canary_dir();
    let root = workspace_root();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("canary directory reads")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("s"))
        .collect();
    entries.sort();
    entries
        .into_iter()
        .map(|path| {
            let proof = std::fs::read_to_string(&path).expect("canary proof reads");
            let source = mrs_tptp::proover::proof_header_link(&proof)
                .unwrap_or_else(|| panic!("{} has no % Proof : header", path.display()))
                .to_string();
            let problem_path = root.join(&source);
            let problem = std::fs::read_to_string(&problem_path).unwrap_or_else(|error| {
                panic!(
                    "{} links to {} which does not resolve from the repository root: {error}",
                    path.display(),
                    problem_path.display()
                )
            });
            Canary {
                name: path.file_stem().unwrap().to_string_lossy().to_string(),
                problem,
                proof,
                source,
            }
        })
        .collect()
}

fn check(canary: &Canary, proof: &str) -> KernelVerdict {
    let problem = parse_tptp(&canary.problem).expect("problem parses");
    let parsed = match parse_tptp(proof) {
        Ok(parsed) => parsed,
        // A mutation that breaks the syntax is not a proof at all; the kernel
        // must refuse it, which it does before considering any rule.
        Err(_) => return KernelVerdict::Rejected("unparseable mutant".into()),
    };
    verify_strict_with_source(
        &problem,
        &parsed,
        Some(&canary.source),
        VerificationLimits::default(),
    )
}

#[test]
fn no_mutation_of_an_mrs_proof_ever_certifies() {
    let canaries = load_canaries();
    assert!(
        canaries.len() >= 5,
        "expected a real canary set, found {}",
        canaries.len()
    );

    // Every canary must certify unmutated, or the sweep proves nothing.
    for canary in &canaries {
        assert_eq!(
            check(canary, &canary.proof),
            KernelVerdict::Certified,
            "canary {} no longer certifies; regenerate it or fix the kernel",
            canary.name
        );
    }

    let next = AtomicUsize::new(0);
    let failures: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let checked = AtomicUsize::new(0);

    std::thread::scope(|scope| {
        for _ in 0..4 {
            let worker = std::thread::Builder::new()
                .stack_size(WORKER_STACK)
                .spawn_scoped(scope, || {
                    let mut local = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(canary) = canaries.get(index) else {
                            break;
                        };
                        for mutation in MUTATIONS {
                            let Some(mutant) = (mutation.apply)(&canary.proof) else {
                                continue;
                            };
                            if mutant == canary.proof {
                                continue;
                            }
                            checked.fetch_add(1, Ordering::Relaxed);
                            if matches!(check(canary, &mutant), KernelVerdict::Certified) {
                                local.push(format!(
                                    "{} + {}: certified",
                                    canary.name, mutation.name
                                ));
                            }
                        }
                    }
                    if !local.is_empty() {
                        failures.lock().unwrap().extend(local);
                    }
                })
                .expect("sweep worker starts");
            worker.join().expect("sweep worker finishes");
        }
    });

    let failures = failures.into_inner().unwrap();
    assert!(
        failures.is_empty(),
        "{} mutants of MRS's own proofs certified:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(
        checked.load(Ordering::Relaxed) >= canaries.len() * 8,
        "expected a broad sweep, only {} mutants applied",
        checked.load(Ordering::Relaxed)
    );
    eprintln!(
        "mutation sweep: {} canaries, {} mutants, none certified",
        canaries.len(),
        checked.load(Ordering::Relaxed)
    );
}
