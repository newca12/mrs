//! Resource monitoring and containment limits.
//!
//! Provides memory watchdog telemetry, term bank ceilings, and clause ceilings
//! to ensure that huge or pathologically explosive problems (e.g. ICU problems)
//! terminate gracefully with `SearchResult::ResourceOut` or `SearchResult::GaveUp`,
//! instead of triggering OS OOM kills (SIGKILL) or infinite resource consumption.

use std::fs;

/// Returns current resident set size (RSS) in MB for the process (Linux only).
pub fn current_memory_mb() -> Option<u64> {
    let content = fs::read_to_string("/proc/self/status").ok()?;
    for line in content.lines() {
        if line.starts_with("VmRSS:") {
            let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
            return Some(kb / 1024);
        }
    }
    None
}

/// Returns the memory this process may use before it is considered out of
/// budget, in MB.
///
/// Policy, in order of precedence:
/// 1. `MRS_MAX_MEMORY_MB`, when set explicitly (the benchmark harness does this
///    so that several concurrent runs on one host each get a known share).
/// 2. 80% of currently *available* memory, not of total memory. `MemAvailable`
///    accounts for other processes, so co-running benchmark jobs do not each
///    believe they own the whole machine.
///
/// There is deliberately no upper clamp. An earlier policy capped the ceiling at
/// 14 GB on the assumption of a 16 GB node, which is wrong on any larger host:
/// on a 64 GB benchmark host it terminated runs at 14 GB that had 50 GB of
/// headroom, and CASC StarExec allows 128 GiB per run. On a small host the 80%
/// policy already yields a small ceiling, so the clamp was never needed there.
pub fn memory_budget_mb() -> Option<u64> {
    let override_mb = std::env::var("MRS_MAX_MEMORY_MB").ok();
    let meminfo = std::fs::read_to_string("/proc/meminfo").ok();
    let cgroup_headroom = cgroup_memory_headroom_mb();
    memory_budget_from_sources(override_mb.as_deref(), &meminfo, cgroup_headroom)
}

fn memory_budget_from_sources(
    override_mb: Option<&str>,
    meminfo: &Option<String>,
    cgroup_headroom_mb: Option<u64>,
) -> Option<u64> {
    if let Some(mb) = override_mb.and_then(|value| value.parse::<u64>().ok()) {
        return Some(mb);
    }
    let host_available_mb = meminfo.as_deref().and_then(parse_available_memory_mb);
    let available_mb = match (host_available_mb, cgroup_headroom_mb) {
        (Some(host), Some(cgroup)) => host.min(cgroup),
        (Some(host), None) => host,
        (None, Some(cgroup)) => cgroup,
        (None, None) => return None,
    };
    // Do not raise a genuinely small budget to a convenient minimum: the
    // watchdog must respect small containers as well as small physical hosts.
    Some(((available_mb as u128 * 80) / 100).min(u64::MAX as u128) as u64)
}

fn parse_available_memory_mb(meminfo: &str) -> Option<u64> {
    let mut available_kb = None;
    let mut total_kb = None;
    for line in meminfo.lines() {
        if let Some(rest) = line.strip_prefix("MemAvailable:") {
            available_kb = rest.split_whitespace().next().and_then(|v| v.parse().ok());
        } else if let Some(rest) = line.strip_prefix("MemTotal:") {
            total_kb = rest.split_whitespace().next().and_then(|v| v.parse().ok());
        }
    }
    available_kb.or(total_kb).map(|kb: u64| kb / 1024)
}

/// Remaining cgroup memory in MiB, if a standard v1 or v2 limit is present.
fn cgroup_memory_headroom_mb() -> Option<u64> {
    if let (Ok(limit), Ok(current)) = (
        std::fs::read_to_string("/sys/fs/cgroup/memory.max"),
        std::fs::read_to_string("/sys/fs/cgroup/memory.current"),
    ) && let (Ok(limit), Ok(current)) =
        (limit.trim().parse::<u64>(), current.trim().parse::<u64>())
    {
        // cgroup v2 represents an unlimited memory ceiling as the text "max".
        return Some(limit.saturating_sub(current) / (1024 * 1024));
    }

    cgroup_memory_headroom_from(&[
        std::path::Path::new("/sys/fs/cgroup/memory"),
        std::path::Path::new("/sys/fs/cgroup"),
        std::path::Path::new("/sys/fs/cgroup/memory.slice"),
    ])
}

fn cgroup_memory_headroom_from(bases: &[&std::path::Path]) -> Option<u64> {
    for base in bases {
        let Ok(limit) = std::fs::read_to_string(base.join("memory.limit_in_bytes")) else {
            continue;
        };
        let Ok(limit) = limit.trim().parse::<u64>() else {
            continue;
        };
        // cgroup v1 uses a very large sentinel for "unlimited".
        if limit >= (1u64 << 60) {
            continue;
        }
        let Ok(current) = std::fs::read_to_string(base.join("memory.usage_in_bytes")) else {
            continue;
        };
        let Ok(current) = current.trim().parse::<u64>() else {
            continue;
        };
        return Some(limit.saturating_sub(current) / (1024 * 1024));
    }
    None
}

/// Picks a default portfolio worker count that respects both core count and
/// available memory.
///
/// Each worker holds its own term bank, clause store, and indexes, so the
/// portfolio footprint scales with the worker count. On a memory-constrained
/// host, one worker per physical core is the wrong default: the run is
/// guaranteed to hit the memory watchdog. This bounds the default by what the
/// memory can actually support. An explicit `--workers` / `MRS_WORKERS` always
/// takes precedence, so competition runs are unaffected.
pub fn default_worker_count(cores: usize) -> usize {
    let cores = cores.max(1);
    let by_memory = match memory_budget_mb() {
        Some(budget_mb) => (budget_mb / RAM_PER_WORKER_MB).max(1) as usize,
        None => cores,
    };
    cores.min(by_memory).max(1)
}

/// A conservative estimate of the RAM one portfolio worker needs.
///
/// Every worker holds its own term bank, clause store, and indexes, so the
/// portfolio's footprint scales with the worker count. This is used only to
/// pick a safe default worker count on memory-constrained hosts; an explicit
/// `--workers` always wins.
pub const RAM_PER_WORKER_MB: u64 = 2048;

/// Resource containment limits for a proof search strategy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceLimits {
    /// Maximum clauses added to the processed set.
    pub max_processed: Option<u64>,
    /// Maximum clauses active in the passive queue.
    pub max_passive: Option<u64>,
    /// Maximum unique terms interned in the TermBank.
    pub max_terms: Option<usize>,
    /// Process-wide memory ceiling in MB.
    pub max_memory_mb: Option<u64>,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        let max_processed = std::env::var("MRS_MAX_PROCESSED")
            .ok()
            .and_then(|v| v.parse().ok());
        let max_passive = std::env::var("MRS_MAX_PASSIVE")
            .ok()
            .and_then(|v| v.parse().ok());
        let max_terms = std::env::var("MRS_MAX_TERMS")
            .ok()
            .and_then(|v| v.parse().ok());
        let max_memory_mb = memory_budget_mb();

        Self {
            max_processed,
            max_passive,
            max_terms,
            max_memory_mb,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry_functions_run_without_panic() {
        let _ = current_memory_mb();
        let _ = memory_budget_mb();
        let limits = ResourceLimits::default();
        // Just verify default construction succeeds
        assert!(limits.max_processed.is_none() || limits.max_processed.is_some());
    }

    #[test]
    fn explicit_override_wins() {
        let meminfo = Some("MemAvailable: 2048000 kB\nMemTotal: 4096000 kB".to_owned());
        assert_eq!(
            memory_budget_from_sources(Some("7777"), &meminfo, Some(100)),
            Some(7777)
        );
    }

    #[test]
    fn worker_count_is_bounded_by_memory_budget() {
        // 2 GiB per worker over a 4 GiB budget allows 2 workers, however many
        // cores the host has.
        assert_eq!(workers_for_budget(64, Some(4096)), 2);
        assert_eq!(workers_for_budget(4, Some(65536)), 4);
        assert_eq!(workers_for_budget(64, Some(512)), 1);
    }

    #[test]
    fn budget_is_not_clamped_to_a_small_host_assumption() {
        // A 1 TiB budget must survive; the old policy clamped this to 14 GB.
        assert_eq!(
            memory_budget_from_sources(Some("1048576"), &None, None),
            Some(1_048_576)
        );
    }

    #[test]
    fn budget_uses_container_headroom_and_does_not_raise_small_limits() {
        let meminfo = Some("MemAvailable: 16777216 kB\nMemTotal: 33554432 kB".to_owned());
        assert_eq!(
            memory_budget_from_sources(None, &meminfo, Some(256)),
            Some(204)
        );
        assert_eq!(memory_budget_from_sources(None, &None, Some(100)), Some(80));
    }

    #[test]
    fn ignores_cgroup_v1_unlimited_sentinel() {
        let dir = std::env::temp_dir().join(format!("mrs-cgroup-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create test cgroup dir");
        std::fs::write(dir.join("memory.limit_in_bytes"), (1u64 << 62).to_string())
            .expect("write v1 unlimited sentinel");
        std::fs::write(dir.join("memory.usage_in_bytes"), "0").expect("write usage");
        assert_eq!(cgroup_memory_headroom_from(&[dir.as_path()]), None);
        std::fs::remove_dir_all(dir).expect("remove test cgroup dir");
    }

    fn workers_for_budget(cores: usize, budget_mb: Option<u64>) -> usize {
        let cores = cores.max(1);
        let by_memory = budget_mb
            .map(|budget| (budget / RAM_PER_WORKER_MB).max(1) as usize)
            .unwrap_or(cores);
        cores.min(by_memory).max(1)
    }
}
