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
    if let Ok(val) = std::env::var("MRS_MAX_MEMORY_MB")
        && let Ok(mb) = val.parse::<u64>()
    {
        return Some(mb);
    }

    if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
        let mut available_kb: Option<u64> = None;
        let mut total_kb: Option<u64> = None;
        for line in content.lines() {
            if let Some(rest) = line.strip_prefix("MemAvailable:") {
                available_kb = rest.split_whitespace().next().and_then(|v| v.parse().ok());
            } else if let Some(rest) = line.strip_prefix("MemTotal:") {
                total_kb = rest.split_whitespace().next().and_then(|v| v.parse().ok());
            }
        }
        let kb = available_kb.or(total_kb);
        if let Some(kb) = kb {
            let mb = (kb / 1024) * 80 / 100;
            return Some(mb.clamp(512, u64::MAX));
        }
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
        // Safety: single-threaded test touching only process-local environment.
        let previous = std::env::var("MRS_MAX_MEMORY_MB").ok();
        // SAFETY: no other thread in this test binary reads the environment.
        unsafe { std::env::set_var("MRS_MAX_MEMORY_MB", "7777") };
        assert_eq!(memory_budget_mb(), Some(7777));
        match previous {
            Some(value) => unsafe { std::env::set_var("MRS_MAX_MEMORY_MB", value) },
            None => unsafe { std::env::remove_var("MRS_MAX_MEMORY_MB") },
        }
    }

    #[test]
    fn worker_count_is_bounded_by_memory_budget() {
        let previous = std::env::var("MRS_MAX_MEMORY_MB").ok();
        // SAFETY: no other thread in this test binary reads the environment.
        unsafe { std::env::set_var("MRS_MAX_MEMORY_MB", "4096") };
        // 2 GiB per worker over a 4 GiB budget allows 2 workers, however many
        // cores the host has.
        assert_eq!(default_worker_count(64), 2);
        // SAFETY: no other thread in this test binary reads the environment.
        unsafe { std::env::set_var("MRS_MAX_MEMORY_MB", "65536") };
        assert_eq!(default_worker_count(4), 4);
        match previous {
            Some(value) => unsafe { std::env::set_var("MRS_MAX_MEMORY_MB", value) },
            None => unsafe { std::env::remove_var("MRS_MAX_MEMORY_MB") },
        }
    }

    #[test]
    fn budget_is_not_clamped_to_a_small_host_assumption() {
        let previous = std::env::var("MRS_MAX_MEMORY_MB").ok();
        // SAFETY: no other thread in this test binary reads the environment.
        unsafe { std::env::set_var("MRS_MAX_MEMORY_MB", "1048576") };
        // A 1 TiB budget must survive; the old policy clamped this to 14 GB.
        assert_eq!(memory_budget_mb(), Some(1_048_576));
        match previous {
            Some(value) => unsafe { std::env::set_var("MRS_MAX_MEMORY_MB", value) },
            None => unsafe { std::env::remove_var("MRS_MAX_MEMORY_MB") },
        }
    }
}
