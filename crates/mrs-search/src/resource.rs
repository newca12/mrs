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

/// Computes the default system memory limit in MB based on available RAM or environment override.
pub fn system_memory_limit_mb() -> Option<u64> {
    if let Ok(val) = std::env::var("MRS_MAX_MEMORY_MB")
        && let Ok(mb) = val.parse::<u64>()
    {
        return Some(mb);
    }

    // Read /proc/meminfo to get MemTotal
    if let Ok(content) = fs::read_to_string("/proc/meminfo") {
        for line in content.lines() {
            if line.starts_with("MemTotal:")
                && let Some(kb_str) = line.split_whitespace().nth(1)
                && let Ok(kb) = kb_str.parse::<u64>()
            {
                // Default to 80% of total system memory, capped at 14 GB (standard for 16GB CASC node).
                let mb = (kb / 1024) * 80 / 100;
                return Some(mb.clamp(1024, 14336));
            }
        }
    }
    None
}

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
        let max_memory_mb = system_memory_limit_mb();

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
        let _ = system_memory_limit_mb();
        let limits = ResourceLimits::default();
        // Just verify default construction succeeds
        assert!(limits.max_processed.is_none() || limits.max_processed.is_some());
    }
}
