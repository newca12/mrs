//! Auto-discover bundled ATP binaries at runtime.

use std::path::PathBuf;

/// Search likely locations for `eprover`.
pub fn find_eprover() -> Option<PathBuf> {
    find_binary(
        "eprover",
        &[
            // crates/mrs-bench/systems/eprover/bin/eprover, relative to the binary CWD
            "crates/mrs-bench/systems/eprover/bin/eprover",
            "mrs-bench/systems/eprover/bin/eprover",
            "systems/eprover/bin/eprover",
            "../systems/eprover/bin/eprover",
            "/usr/local/bin/eprover",
            "/usr/bin/eprover",
        ],
    )
}

/// Search likely locations for `vampire`.
pub fn find_vampire() -> Option<PathBuf> {
    find_binary(
        "vampire",
        &[
            "crates/mrs-bench/systems/vampire/bin/vampire",
            "mrs-bench/systems/vampire/bin/vampire",
            "systems/vampire/bin/vampire",
            "../systems/vampire/bin/vampire",
            "/usr/local/bin/vampire",
            "/usr/bin/vampire",
        ],
    )
}

/// Search likely locations for the in-tree `mrs` binary.
pub fn find_mrs() -> Option<PathBuf> {
    find_binary(
        "mrs",
        &[
            "target/release/mrs",
            "target/debug/mrs",
            "../../target/release/mrs",
            "../../target/debug/mrs",
        ],
    )
}

fn find_binary(_name: &str, candidates: &[&str]) -> Option<PathBuf> {
    // 1) Honor explicit env var (e.g. MRS_PROOVER_EPROVER=/path/to/eprover).
    let env_var = format!("MRS_PROOVER_{}", _name.to_ascii_uppercase());
    if let Ok(p) = std::env::var(&env_var) {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
    }
    // 2) Try each candidate path.
    for c in candidates {
        let pb = PathBuf::from(c);
        if pb.is_file() {
            return Some(pb);
        }
    }
    // 3) Search relative to CARGO_MANIFEST_DIR walking upward.
    // (At build time we know the workspace root; at install time we don't.)
    None
}
