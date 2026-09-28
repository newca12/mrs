//! Resource monitoring and containment limits.
//!
//! Provides memory watchdog telemetry, term bank ceilings, and clause ceilings
//! to ensure that huge or pathologically explosive problems (e.g. ICU problems)
//! terminate gracefully with `SearchResult::ResourceOut` or `SearchResult::GaveUp`,
//! instead of triggering OS OOM kills (SIGKILL) or infinite resource consumption.

use std::collections::HashSet;
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

// ---------------------------------------------------------------------------
// Hardware modes
// ---------------------------------------------------------------------------
//
// Three ways to answer "how many workers, how much memory, which CPUs", because
// the right answer differs by purpose and a single policy conflates them:
//
//   adaptive  fit the host. One worker per usable physical core, bounded by
//             what memory supports, 80% of currently available RAM. The right
//             default for development and for benchmarking on a big box, where
//             the only question is "can this be solved at all".
//
//   casc      exactly what a CASC entry gets: 8 workers and the CASC memory
//             allowance, never auto-adapted. Portfolio design treats 8 as
//             canonical (AGENTS.md §11), so a run handed 16 or 32 workers is
//             not measuring the thing the schedules were tuned for.
//
//   casc-sim  simulate that on any host, for developing on hardware that is not
//             the competition machine. Same workers and memory allowance, plus
//             the CPU set pinned to 8 physical cores and a longer wall clock,
//             so a memory-bound failure surfaces instead of being masked by the
//             clock.
//
// Explicit `--workers` and `MRS_MAX_MEMORY_MB` always win, so the benchmark
// harness and competition invocations stay authoritative and nothing that runs
// today changes behaviour.

/// CASC competition hardware: **exactly 8 physical cores** (AGENTS.md §11).
///
/// Physical cores, not logical CPUs. SMT siblings add no capacity, so a
/// simulation that pinned 8 *logical* CPUs would be simulating 4 cores.
pub const CASC_PHYSICAL_CORES: usize = 8;

/// CASC per-run memory allowance, in MB (128 GiB).
pub const CASC_MEMORY_MB: u64 = 128 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HardwareMode {
    Adaptive,
    Casc,
    CascSim,
}

impl HardwareMode {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "adaptive" | "auto" | "host" => Some(Self::Adaptive),
            "casc" => Some(Self::Casc),
            "casc-sim" | "sim" => Some(Self::CascSim),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Adaptive => "adaptive",
            Self::Casc => "casc",
            Self::CascSim => "casc-sim",
        }
    }

    /// Whether this mode fixes the worker count and memory at the CASC values
    /// instead of fitting the host.
    pub fn is_casc_shaped(self) -> bool {
        !matches!(self, Self::Adaptive)
    }
}

/// A fully resolved answer to "how do we run on this hardware".
#[derive(Clone, Debug)]
pub struct HardwareProfile {
    pub mode: HardwareMode,
    /// Workers the search will actually get.
    pub workers: usize,
    /// Physical cores this process may use here, after affinity, cgroup quota
    /// and SMT topology are taken into account.
    pub host_physical_cores: usize,
    /// The memory ceiling the watchdog will enforce, in MB.
    pub memory_budget_mb: Option<u64>,
    /// The most memory this host can actually supply. Below
    /// `memory_budget_mb`, a run needing the difference is killed by the OS
    /// before the simulated allowance means anything.
    pub effective_memory_mb: Option<u64>,
    /// True when the host cannot represent the requested allowance.
    pub memory_unrepresentable: bool,
    /// Caveats worth reporting, in priority order.
    pub warnings: Vec<String>,
}

impl HardwareProfile {
    /// One `% Hardware:` line, so an archived run records what it was measured
    /// under without needing the invocation that produced it.
    pub fn describe(&self) -> String {
        let mut fields = vec![
            format!("hardware={}", self.mode.as_str()),
            format!("workers={}", self.workers),
            format!("host_physical_cores={}", self.host_physical_cores),
        ];
        if let Some(mb) = self.memory_budget_mb {
            fields.push(format!("mem_budget_mb={mb}"));
        }
        if let Some(mb) = self.effective_memory_mb {
            // The most this run can actually get: the budget, clipped to what
            // the host has. In adaptive mode that is the 80% allowance; in
            // casc mode on a small box it is the host's RAM, which is exactly
            // the fact `mem_unrepresentable` warns about.
            fields.push(format!("effective_mem_mb={mb}"));
        }
        if self.memory_unrepresentable {
            fields.push("mem_unrepresentable=1".to_string());
        }
        fields.join(" ")
    }
}

/// Resolve a mode into a concrete profile.
///
/// `explicit_workers` and `explicit_memory_mb` come from `--workers` and
/// `MRS_MAX_MEMORY_MB`. Either being set overrides the mode for that dimension.
pub fn resolve_profile(
    mode: HardwareMode,
    explicit_workers: Option<usize>,
    explicit_memory_mb: Option<u64>,
) -> HardwareProfile {
    let host_physical_cores = usable_physical_cores();
    let host_memory_mb = host_total_memory_mb();

    let workers = match explicit_workers {
        Some(w) => w.max(1),
        None if mode.is_casc_shaped() => CASC_PHYSICAL_CORES,
        None => default_worker_count(host_physical_cores),
    };

    let memory_budget_mb = match explicit_memory_mb {
        Some(mb) => Some(mb),
        None if mode.is_casc_shaped() => Some(CASC_MEMORY_MB),
        None => memory_budget_mb(),
    };

    // A limit above what the box can supply is not a limit. Say so instead of
    // letting "simulated CASC" quietly not be simulating memory.
    let memory_unrepresentable = match (memory_budget_mb, host_memory_mb) {
        (Some(limit), Some(host)) => host < limit,
        _ => false,
    };
    // The enforced ceiling is clipped to what this process can actually get,
    // not to what the machine has. See `host_available_memory_mb` for why the
    // distinction is load-bearing.
    let effective_memory_mb = match (memory_budget_mb, host_memory_mb, host_available_memory_mb()) {
        (Some(limit), Some(host), available) => Some(limit.min(available.unwrap_or(host))),
        (Some(limit), None, _) => Some(limit),
        (None, Some(host), _) => Some(host),
        (None, None, _) => None,
    };

    HardwareProfile {
        mode,
        workers,
        host_physical_cores,
        memory_budget_mb,
        effective_memory_mb,
        memory_unrepresentable,
        warnings: profile_warnings(mode, host_physical_cores, memory_budget_mb, host_memory_mb),
    }
}

/// Caveats for a profile, as a function of the host facts rather than of the
/// host itself, so the wording and the thresholds can be tested on any machine.
pub fn profile_warnings(
    mode: HardwareMode,
    host_physical_cores: usize,
    memory_budget_mb: Option<u64>,
    host_memory_mb: Option<u64>,
) -> Vec<String> {
    let mut warnings = Vec::new();
    if mode.is_casc_shaped() && host_physical_cores < CASC_PHYSICAL_CORES {
        warnings.push(format!(
            "hardware={} wants {CASC_PHYSICAL_CORES} workers but this host offers only \
             {host_physical_cores} usable physical core(s), so the worker count is not \
             representative of a CASC entry",
            mode.as_str()
        ));
    }
    if let (Some(limit), Some(host)) = (memory_budget_mb, host_memory_mb)
        && host < limit
    {
        warnings.push(format!(
            "hardware={} sets a {limit} MB allowance but this host has only {host} MB, so a \
             run needing more is killed by the host before that allowance applies",
            mode.as_str()
        ));
    }
    warnings
}

/// Total RAM on this host in MB, ignoring cgroup limits.
///
/// Used to decide whether a requested allowance is representable here. That is
/// a property of the machine, so `MemTotal` is the right quantity: it does not
/// change between one call and the next.
pub fn host_total_memory_mb() -> Option<u64> {
    meminfo_field_mb("MemTotal:")
}

/// RAM this process can still get in MB, ignoring cgroup limits.
///
/// This is the quantity the *enforced* ceiling has to be derived from, and it
/// is not the same as [`host_total_memory_mb`]. A ceiling set from `MemTotal`
/// on a host that is not otherwise idle is above what the kernel will actually
/// give this process, so the OOM killer gets there first and the run dies with
/// no status — the watchdog never gets the chance to fail it closed. Measured
/// on the 2-core/15 GB development box, a ceiling derived from `MemTotal`
/// (15 876 MB) sat ~2 GB above `MemAvailable` (13 897 MB).
///
/// `MemAvailable` is a kernel estimate that moves, so it is only used for the
/// enforced ceiling and never for the representability check or a warning;
/// nothing user-visible should flap with transient memory pressure.
pub fn host_available_memory_mb() -> Option<u64> {
    meminfo_field_mb("MemAvailable:")
}

fn meminfo_field_mb(field: &str) -> Option<u64> {
    let meminfo = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in meminfo.lines() {
        if let Some(rest) = line.strip_prefix(field) {
            return rest
                .split_whitespace()
                .next()?
                .parse::<u64>()
                .ok()
                .map(|kb| kb / 1024);
        }
    }
    None
}

/// Physical cores this process may actually use.
///
/// `num_cpus::get_physical()` describes the whole machine, so it is wrong
/// wherever the process is confined: a container limited to 4 CPUs of a 64-core
/// host is told it has 64, and the default worker count then follows that lie.
/// Three views are intersected instead:
///
/// 1. the affinity mask the kernel will really schedule us on,
/// 2. the cgroup CPU quota, when one is set, and
/// 3. SMT topology, so two hyperthreads on one core count once.
pub fn usable_physical_cores() -> usize {
    let by_topology = match allowed_logical_cpus() {
        Some(cpus) if !cpus.is_empty() => sibling_groups(&cpus).len(),
        _ => num_cpus::get_physical(),
    };
    match cgroup_cpu_quota() {
        Some(quota_cores) => by_topology.min(quota_cores.max(1.0).ceil() as usize).max(1),
        None => by_topology.max(1),
    }
}

/// Group logical CPUs into physical cores using sysfs thread siblings.
///
/// CPUs sharing a `thread_siblings_list` are hyperthreads of one core and must
/// be counted once. A CPU whose topology cannot be read forms its own group,
/// which is the conservative direction: it never claims fewer cores than the
/// topology proves.
pub fn sibling_groups(cpus: &[usize]) -> Vec<Vec<usize>> {
    group_by_siblings(cpus, thread_siblings)
}

fn group_by_siblings(
    cpus: &[usize],
    mut siblings_of: impl FnMut(usize) -> Option<Vec<usize>>,
) -> Vec<Vec<usize>> {
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut seen: HashSet<usize> = HashSet::new();
    for &cpu in cpus {
        if seen.contains(&cpu) {
            continue;
        }
        let mut group = vec![cpu];
        seen.insert(cpu);
        if let Some(siblings) = siblings_of(cpu) {
            for sibling in siblings {
                if cpus.contains(&sibling) && seen.insert(sibling) {
                    group.push(sibling);
                }
            }
        }
        groups.push(group);
    }
    groups
}

/// Parse one `thread_siblings_list` entry: `0-1`, `0,4`, `0-1,8`.
fn parse_cpu_list(raw: &str) -> Vec<usize> {
    let mut out = Vec::new();
    for part in raw.trim().split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        match part.split_once('-') {
            Some((lo, hi)) => {
                if let (Ok(lo), Ok(hi)) = (lo.trim().parse::<usize>(), hi.trim().parse::<usize>())
                    && lo <= hi
                {
                    out.extend(lo..=hi);
                }
            }
            None => {
                if let Ok(cpu) = part.parse::<usize>() {
                    out.push(cpu);
                }
            }
        }
    }
    out
}

fn thread_siblings(cpu: usize) -> Option<Vec<usize>> {
    let path = format!("/sys/devices/system/cpu/cpu{cpu}/topology/thread_siblings_list");
    let raw = std::fs::read_to_string(path).ok()?;
    Some(parse_cpu_list(&raw))
}

/// Logical CPUs this process is allowed to run on, per the kernel.
#[cfg(target_os = "linux")]
pub fn allowed_logical_cpus() -> Option<Vec<usize>> {
    const BITS_PER_BYTE: usize = 8;
    // SAFETY: `cpu_set_t` is a plain CPU bitmask, so viewing it as bytes is how
    // its set bits are counted (`bits` is private in the libc crate). The
    // buffer is zeroed before the kernel call, its size is passed explicitly,
    // and the byte slice is derived from that same initialized value.
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        libc::CPU_ZERO(&mut set);
        let size = std::mem::size_of::<libc::cpu_set_t>();
        if libc::sched_getaffinity(0, size, &raw mut set) != 0 {
            return None;
        }
        let mask = std::slice::from_raw_parts((&raw const set).cast::<u8>(), size);
        let mut cpus = Vec::new();
        for (byte_index, byte) in mask.iter().enumerate() {
            for bit in 0..BITS_PER_BYTE {
                if byte & (1u8 << bit) != 0 {
                    cpus.push(byte_index * BITS_PER_BYTE + bit);
                }
            }
        }
        Some(cpus)
    }
}

#[cfg(not(target_os = "linux"))]
pub fn allowed_logical_cpus() -> Option<Vec<usize>> {
    None
}

/// CPU cores granted by the cgroup CPU quota, as a fraction.
///
/// `None` means unlimited or unreadable. cgroup v2 (`cpu.max`) and v1
/// (`cpu.cfs_quota_us` / `cpu.cfs_period_us`) are both handled, including v1's
/// negative "unlimited" sentinel.
pub fn cgroup_cpu_quota() -> Option<f64> {
    if let Ok(raw) = std::fs::read_to_string("/sys/fs/cgroup/cpu.max") {
        return parse_cgroup_v2_cpu_max(&raw);
    }
    let quota = std::fs::read_to_string("/sys/fs/cgroup/cpu/cpu.cfs_quota_us").ok()?;
    let period = std::fs::read_to_string("/sys/fs/cgroup/cpu/cpu.cfs_period_us").ok()?;
    parse_cgroup_v1_quota(&quota, &period)
}

/// cgroup v2 `cpu.max`, whose form is `quota period` or the literal `max`.
fn parse_cgroup_v2_cpu_max(raw: &str) -> Option<f64> {
    let mut fields = raw.split_whitespace();
    let quota = fields.next()?;
    let period = fields.next().unwrap_or("100000");
    if quota == "max" {
        return None;
    }
    let (quota, period) = (quota.parse::<f64>().ok()?, period.parse::<f64>().ok()?);
    (period > 0.0).then(|| quota / period)
}

/// cgroup v1 `cpu.cfs_quota_us` over `cpu.cfs_period_us`, where a negative quota
/// is the "unlimited" sentinel.
fn parse_cgroup_v1_quota(quota_raw: &str, period_raw: &str) -> Option<f64> {
    let quota = quota_raw.trim().parse::<f64>().ok()?;
    let period = period_raw.trim().parse::<f64>().ok()?;
    if quota <= 0.0 || period <= 0.0 {
        return None;
    }
    Some(quota / period)
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

    // ---- hardware modes ----

    #[test]
    fn hardware_mode_parsing_covers_aliases() {
        assert_eq!(
            HardwareMode::parse("adaptive"),
            Some(HardwareMode::Adaptive)
        );
        assert_eq!(HardwareMode::parse("AUTO"), Some(HardwareMode::Adaptive));
        assert_eq!(HardwareMode::parse("casc"), Some(HardwareMode::Casc));
        assert_eq!(HardwareMode::parse(" casc "), Some(HardwareMode::Casc));
        assert_eq!(HardwareMode::parse("casc-sim"), Some(HardwareMode::CascSim));
        assert_eq!(HardwareMode::parse("casc_sim"), Some(HardwareMode::CascSim));
        assert_eq!(HardwareMode::parse("sim"), Some(HardwareMode::CascSim));
        assert_eq!(HardwareMode::parse("casc-slim"), None);
        assert_eq!(HardwareMode::parse(""), None);
    }

    #[test]
    fn casc_modes_fix_workers_and_memory_regardless_of_host() {
        // Resolution is host-dependent by design, so assert the part that must
        // not be: the CASC worker count and allowance are the mode's contract.
        for mode in [HardwareMode::Casc, HardwareMode::CascSim] {
            assert!(mode.is_casc_shaped());
            let warnings = profile_warnings(mode, 64, Some(CASC_MEMORY_MB), Some(1_048_576));
            assert!(
                warnings.is_empty(),
                "a 64-core host with RAM to spare must raise no warning: {warnings:?}"
            );
        }
        assert!(!HardwareMode::Adaptive.is_casc_shaped());
    }

    #[test]
    fn explicit_overrides_beat_the_mode() {
        // The harness and the competition invocations set these, so a mode must
        // never be able to overrule them.
        let adaptive = resolve_profile(HardwareMode::Adaptive, Some(3), Some(1234));
        assert_eq!(adaptive.workers, 3);
        assert_eq!(adaptive.memory_budget_mb, Some(1234));

        let casc = resolve_profile(HardwareMode::Casc, Some(2), Some(4096));
        assert_eq!(
            casc.workers, 2,
            "explicit --workers must win over the CASC count"
        );
        assert_eq!(casc.memory_budget_mb, Some(4096));
    }

    #[test]
    fn a_memory_allowance_above_the_host_is_reported_not_silently_clipped() {
        let warnings =
            profile_warnings(HardwareMode::Casc, 8, Some(CASC_MEMORY_MB), Some(16 * 1024));
        assert!(
            warnings.iter().any(|w| w.contains("killed by the host")),
            "an unrepresentable allowance must be called out: {warnings:?}"
        );
        // Equal is representable: the host can supply exactly the allowance.
        let equal = profile_warnings(HardwareMode::Casc, 8, Some(16 * 1024), Some(16 * 1024));
        assert!(!equal.iter().any(|w| w.contains("killed by the host")));
    }

    #[test]
    fn a_host_with_too_few_cores_is_reported() {
        let warnings = profile_warnings(HardwareMode::CascSim, 2, None, None);
        assert!(
            warnings.iter().any(|w| w.contains("usable physical core")),
            "an under-provisioned host must be called out: {warnings:?}"
        );
        // Adaptive is allowed to fit the host, so it says nothing.
        assert!(
            profile_warnings(HardwareMode::Adaptive, 2, None, None).is_empty(),
            "adaptive mode is meant for small hosts"
        );
    }

    #[test]
    fn profile_description_records_what_a_run_was_measured_under() {
        let profile = resolve_profile(HardwareMode::CascSim, Some(8), Some(CASC_MEMORY_MB));
        let described = profile.describe();
        assert!(described.contains("hardware=casc-sim"), "{described}");
        assert!(described.contains("workers=8"), "{described}");
        assert!(described.contains("mem_budget_mb=131072"), "{described}");
    }

    // ---- topology detection ----

    #[test]
    fn cpu_lists_parse_the_forms_sysfs_uses() {
        assert_eq!(parse_cpu_list("0-1"), vec![0, 1]);
        assert_eq!(parse_cpu_list("3"), vec![3]);
        assert_eq!(parse_cpu_list("0,4"), vec![0, 4]);
        assert_eq!(parse_cpu_list("0-1,8"), vec![0, 1, 8]);
        assert_eq!(parse_cpu_list(" 2 , 5 - 6 "), vec![2, 5, 6]);
        assert!(parse_cpu_list("").is_empty());
        assert!(parse_cpu_list("garbage").is_empty());
        // A reversed range must not become a huge span.
        assert!(parse_cpu_list("9-2").is_empty());
    }

    #[test]
    fn hyperthreads_are_counted_once() {
        // Two SMT pairs and one lonely core: three physical cores, five CPUs.
        let cpus = vec![0, 1, 2, 3, 8];
        let groups = group_by_siblings(&cpus, |cpu| match cpu {
            0 | 2 => Some(vec![0, 2]),
            1 | 3 => Some(vec![1, 3]),
            8 => Some(vec![8]),
            _ => None,
        });
        assert_eq!(
            groups.len(),
            3,
            "SMT siblings must not inflate the core count"
        );
        let mut sizes: Vec<usize> = groups.iter().map(Vec::len).collect();
        sizes.sort_unstable();
        assert_eq!(sizes, vec![1, 2, 2]);
    }

    #[test]
    fn an_unreadable_topology_never_undercounts_cores() {
        // No topology available: every CPU is its own core, which is the safe
        // direction. Assuming cores were shared would cap the worker count
        // below what the host can actually run.
        let groups = group_by_siblings(&[0, 1, 2, 3], |_| None);
        assert_eq!(groups.len(), 4);
    }

    #[test]
    fn cgroup_v2_cpu_max_is_understood() {
        assert_eq!(parse_cgroup_v2_cpu_max("max 100000"), None);
        assert_eq!(parse_cgroup_v2_cpu_max("400000 100000"), Some(4.0));
        assert_eq!(parse_cgroup_v2_cpu_max("250000 100000"), Some(2.5));
        // A period-less line is the default 100 ms period.
        assert_eq!(parse_cgroup_v2_cpu_max("800000"), Some(8.0));
    }

    #[test]
    fn cgroup_v1_quota_treats_negative_as_unlimited() {
        assert_eq!(parse_cgroup_v1_quota("-1", "100000"), None);
        assert_eq!(parse_cgroup_v1_quota("0", "100000"), None);
        assert_eq!(parse_cgroup_v1_quota("200000", "100000"), Some(2.0));
        assert_eq!(parse_cgroup_v1_quota(" 300000 ", " 100000 "), Some(3.0));
    }

    #[test]
    fn live_detection_does_not_panic_and_agrees_with_itself() {
        let cpus = allowed_logical_cpus();
        let cores = usable_physical_cores();
        assert!(cores >= 1, "a process always has at least one core");
        if let Some(cpus) = cpus {
            assert!(!cpus.is_empty(), "affinity must report at least one CPU");
            // Never more physical cores than allowed logical CPUs, and never
            // fewer logical CPUs than the fallback would report on an
            // unconfined host.
            assert!(sibling_groups(&cpus).len() <= cpus.len());
        }
        assert!(host_total_memory_mb().is_some_and(|mb| mb > 0));
    }

    // ---- casc simulation: process constraints ----

    #[cfg(target_os = "linux")]
    #[test]
    fn address_space_ceiling_is_applied_and_restored() {
        /// Resident+reserved address space this process already occupies, in MB.
        fn current_address_space_mb() -> Option<u64> {
            let status = std::fs::read_to_string("/proc/self/status").ok()?;
            for line in status.lines() {
                if let Some(rest) = line.strip_prefix("VmSize:") {
                    return rest
                        .split_whitespace()
                        .next()?
                        .parse::<u64>()
                        .ok()
                        .map(|kb| kb / 1024);
                }
            }
            None
        }

        let Some(current_mb) = current_address_space_mb() else {
            return; // not Linux, or /proc unavailable: nothing to assert
        };

        // Room to spare for the harness's own allocations, and a request well
        // past it. The ceiling is relative to what this process already holds,
        // so the test needs no large allocation to be meaningful.
        let headroom_mb = 256;
        let request_mb = current_mb + headroom_mb;
        let over_request_mb = request_mb + headroom_mb;

        let inherited = {
            let mut limit: libc::rlimit = unsafe { std::mem::zeroed() };
            // SAFETY: live rlimit the kernel fills in.
            assert_eq!(unsafe { libc::getrlimit(libc::RLIMIT_AS, &mut limit) }, 0);
            limit.rlim_cur
        };

        {
            let guard =
                limit_address_space_mb(request_mb).expect("setrlimit(RLIMIT_AS) must be permitted");
            let applied = guard.applied_mb();
            assert_ne!(applied, u64::MAX, "the ceiling must actually be in force");
            assert!(
                applied <= request_mb,
                "applied ceiling {applied} MB must not exceed the {request_mb} MB request"
            );
            assert!(
                applied >= current_mb,
                "a ceiling below current usage would abort the test process: \
                 applied {applied} MB, current {current_mb} MB"
            );

            // The mechanism itself: an allocation past the ceiling must fail
            // rather than succeed and quietly exceed it.
            let attempt = over_request_mb.saturating_mul(BYTES_PER_MB as u64) as usize;
            let allocation = std::alloc::Layout::from_size_align(attempt, 1).expect("valid layout");
            // SAFETY: `allocation` is a valid non-zero-sized layout; the pointer
            // is only inspected, never read, and the deallocation below is
            // reached only if the allocation succeeded.
            let ptr = unsafe { std::alloc::alloc_zeroed(allocation) };
            assert!(
                ptr.is_null(),
                "an allocation of {attempt} bytes must fail under a {applied} MB ceiling"
            );
        }

        // Dropped: the inherited limit is back, so the rest of the suite is
        // unaffected.
        let mut restored: libc::rlimit = unsafe { std::mem::zeroed() };
        // SAFETY: live rlimit the kernel fills in.
        assert_eq!(
            unsafe { libc::getrlimit(libc::RLIMIT_AS, &mut restored) },
            0
        );
        assert_eq!(
            restored.rlim_cur, inherited,
            "dropping the guard must restore the inherited address-space limit"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn pinning_refuses_rather_than_silently_downgrading() {
        let available = sibling_groups(&allowed_logical_cpus().unwrap_or_default()).len();
        // Asking for more cores than exist must fail: a silent smaller set would
        // make the run look simulated when it is not.
        let too_many = available + 1;
        match pin_to_physical_cores(too_many) {
            Err(reason) => assert!(
                reason.contains("only"),
                "the refusal must say what was available: {reason}"
            ),
            Ok(_) => panic!("pinning to {too_many} cores should not succeed"),
        }
        if available > 0 {
            let pin = pin_to_physical_cores(1).expect("pinning to one core must work");
            // One physical core, but both of its SMT siblings.
            assert!(
                !pin.cpus().is_empty(),
                "a pin must name the CPUs it selected"
            );
            drop(pin);
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn pinning_restores_the_previous_mask() {
        let Some(cpus) = allowed_logical_cpus() else {
            return;
        };
        let before: Vec<usize> = cpus.clone();
        {
            let _pin = pin_to_physical_cores(1).expect("pinning to one core must work");
        }
        assert_eq!(
            allowed_logical_cpus(),
            Some(before),
            "dropping the pin must restore the affinity mask"
        );
    }
}

// ---------------------------------------------------------------------------
// CASC simulation: actually constraining the process
// ---------------------------------------------------------------------------
//
// A simulated mode that only renames the worker count would measure a different
// machine, not a smaller one. Two things make the constraint real: the CPU set
// the kernel will schedule us on, and the address space we are allowed to
// reserve. Both are process-wide and both are restored on drop, so the library
// behaves for any caller that does not ask for them.

/// An applied CPU affinity restriction. The previous mask is restored on drop.
#[cfg(target_os = "linux")]
pub struct CpuPinning {
    previous: libc::cpu_set_t,
    size: usize,
    cpus: Vec<usize>,
}

#[cfg(target_os = "linux")]
impl Drop for CpuPinning {
    fn drop(&mut self) {
        // SAFETY: `previous` was filled by sched_getaffinity in the constructor
        // and is only read here; the size matches the type it came from.
        unsafe {
            libc::sched_setaffinity(0, self.size, &raw const self.previous);
        }
    }
}

#[cfg(target_os = "linux")]
impl CpuPinning {
    /// The logical CPUs this process was pinned to.
    pub fn cpus(&self) -> &[usize] {
        &self.cpus
    }
}

/// Restrict this process to `cores` **physical** cores.
///
/// Each chosen core contributes all of its SMT siblings, because CASC hardware is
/// specified in physical cores: pinning 8 logical CPUs of an SMT machine would
/// be simulating 4 cores, which is the opposite of the intent. If the host has
/// fewer than `cores` physical cores available this fails rather than silently
/// pinning a smaller set, because a silent downgrade would make the run look
/// like a simulation when it is not.
#[cfg(target_os = "linux")]
pub fn pin_to_physical_cores(cores: usize) -> Result<CpuPinning, String> {
    let allowed = allowed_logical_cpus().ok_or("cannot read the CPU affinity mask")?;
    let groups = sibling_groups(&allowed);
    if groups.len() < cores {
        return Err(format!(
            "cannot simulate {cores} physical cores: only {} available",
            groups.len()
        ));
    }
    // SAFETY: `cpu_set_t` is a plain integer bitmask, for which all-zero is the
    // documented empty set.
    let mut mask: libc::cpu_set_t = unsafe { std::mem::zeroed() };
    let mut chosen = Vec::new();
    for group in groups.iter().take(cores) {
        for &cpu in group {
            // SAFETY: `cpu` comes from the affinity mask, so it indexes within
            // the bitmask's width, and `mask` is a live, zeroed cpu_set_t.
            unsafe { libc::CPU_SET(cpu, &mut mask) };
            chosen.push(cpu);
        }
    }
    chosen.sort_unstable();
    let size = std::mem::size_of::<libc::cpu_set_t>();
    // SAFETY: both masks are live cpu_set_t values and `size` matches them.
    unsafe {
        let mut previous: libc::cpu_set_t = std::mem::zeroed();
        if libc::sched_getaffinity(0, size, &raw mut previous) != 0 {
            return Err("cannot read the current CPU affinity mask".to_string());
        }
        if libc::sched_setaffinity(0, size, &raw const mask) != 0 {
            return Err("sched_setaffinity was refused".to_string());
        }
        Ok(CpuPinning {
            previous,
            size,
            cpus: chosen,
        })
    }
}

#[cfg(not(target_os = "linux"))]
pub fn pin_to_physical_cores(_cores: usize) -> Result<(), String> {
    Err("CPU pinning is only implemented on Linux".to_string())
}

/// An applied address-space ceiling. The previous limit is restored on drop.
#[cfg(target_os = "linux")]
pub struct AddressSpaceLimit {
    previous: libc::rlimit,
    applied_mb: u64,
}

/// The limit actually in force, in MB, which may be below the request if the
/// inherited hard limit forbids it.
#[cfg(target_os = "linux")]
impl AddressSpaceLimit {
    /// The ceiling actually in force, in MB.
    pub fn applied_mb(&self) -> u64 {
        self.applied_mb
    }
}

#[cfg(not(target_os = "linux"))]
pub struct AddressSpaceLimit;

#[cfg(target_os = "linux")]
impl Drop for AddressSpaceLimit {
    fn drop(&mut self) {
        // SAFETY: `previous` was filled by getrlimit in the constructor.
        unsafe {
            libc::setrlimit(libc::RLIMIT_AS, &raw const self.previous);
        }
    }
}

#[cfg(not(target_os = "linux"))]
impl Drop for AddressSpaceLimit {
    fn drop(&mut self) {}
}

const BYTES_PER_MB: u64 = 1024 * 1024;

#[cfg(target_os = "linux")]
fn bytes_to_mb(bytes: libc::rlim_t) -> u64 {
    // RLIM_INFINITY is the maximum rlim_t rather than a byte count, and must not
    // be reported as an absurd number of megabytes.
    if bytes == libc::rlim_t::MAX {
        return u64::MAX;
    }
    bytes / BYTES_PER_MB
}

/// Cap the address space at `mb`, the way an external harness would.
///
/// This is the backstop under the RSS watchdog. The watchdog samples resident
/// size at given-clause boundaries and can report `ResourceOut` with a reason;
/// `RLIMIT_AS` is what actually stops a run whose memory grows faster than the
/// sampling interval -- the failure mode that currently shows up as an OOM kill
/// with no SZS line at all. Bounding address space rather than RSS is the
/// conservative direction, and it is the only per-process limit available
/// without privileges.
///
/// Only the soft limit can be lowered without `CAP_SYS_RESOURCE`, so a request
/// above the inherited hard limit is clamped and the applied value is reported
/// rather than assumed.
#[cfg(target_os = "linux")]
pub fn limit_address_space_mb(mb: u64) -> Result<AddressSpaceLimit, String> {
    let want = mb.saturating_mul(BYTES_PER_MB);
    // SAFETY: `previous` is a live rlimit the kernel fills in.
    unsafe {
        let mut previous: libc::rlimit = std::mem::zeroed();
        if libc::getrlimit(libc::RLIMIT_AS, &raw mut previous) != 0 {
            return Err("getrlimit(RLIMIT_AS) failed".to_string());
        }
        let ceiling = if previous.rlim_max == libc::rlim_t::MAX {
            want
        } else {
            want.min(previous.rlim_max)
        };
        // Never raise the soft limit: some of it may already be mapped.
        let applied = if previous.rlim_cur == libc::rlim_t::MAX {
            ceiling
        } else {
            ceiling.min(previous.rlim_cur)
        };
        let next = libc::rlimit {
            rlim_cur: applied,
            rlim_max: previous.rlim_max,
        };
        if libc::setrlimit(libc::RLIMIT_AS, &raw const next) != 0 {
            return Err("setrlimit(RLIMIT_AS) was refused".to_string());
        }
        // Read the limit back rather than trusting the request: the kernel
        // clamps silently, and reporting what was asked for would misstate what
        // is in force.
        let mut confirmed: libc::rlimit = std::mem::zeroed();
        let applied_mb = if libc::getrlimit(libc::RLIMIT_AS, &raw mut confirmed) == 0 {
            bytes_to_mb(confirmed.rlim_cur)
        } else {
            bytes_to_mb(applied)
        };
        Ok(AddressSpaceLimit {
            previous,
            applied_mb,
        })
    }
}

#[cfg(not(target_os = "linux"))]
pub fn limit_address_space_mb(_mb: u64) -> Result<AddressSpaceLimit, String> {
    Ok(AddressSpaceLimit)
}
