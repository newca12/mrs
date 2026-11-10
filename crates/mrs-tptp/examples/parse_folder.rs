//! Example: Recursively parse all TPTP files in a folder.
//!
//! This example mimics the behavior of ParserTestSuite.scala from scala-tptp-parser,
//! recursively finding and parsing all .p and .ax files in a given directory.
//!
//! Uses parallel processing with rayon for faster execution on multi-core systems.
//!
//! Run with: cargo run --example parse_folder [folder_path] [--timeout <ms>] [--threads <n>] [--verbose] [--competition-parse]
//!
//! Options:
//!   --timeout <ms>  Abort parsing of any file that takes longer than <ms> milliseconds
//!   --threads <n>   Set the number of threads (default: min(system threads, 8))
//!   --verbose       Show detailed list of timed out files (by default only count is shown)
//!   --competition-parse  Parse each file with `parse_tptp` into a complete AST;
//!                        include the source buffer in peak heap; skip round-trip checks
//!
//! `--threads N` starts N worker loops. Each worker dynamically claims one
//! whole file at a time; a file is never split across workers.
//!
//! If no folder is provided, it defaults to tests/resources

use mrs_tptp::{TPTPInput, TPTPIterator};
#[cfg(feature = "cancellation")]
use mrs_tptp::{clear_cancel_flag, set_cancel_flag};
use rayon::ThreadPoolBuilder;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};
use std::{alloc, ptr};

const TRACKED_THREADS: usize = 256;
const NO_TRACKER: usize = usize::MAX;

struct HeapStats {
    generation: std::sync::atomic::AtomicU64,
    active_generation: std::sync::atomic::AtomicU64,
    live_bytes: AtomicUsize,
    peak_bytes: AtomicUsize,
}

impl HeapStats {
    const fn new() -> Self {
        Self {
            generation: std::sync::atomic::AtomicU64::new(0),
            active_generation: std::sync::atomic::AtomicU64::new(0),
            live_bytes: AtomicUsize::new(0),
            peak_bytes: AtomicUsize::new(0),
        }
    }
}

static HEAP_STATS: [HeapStats; TRACKED_THREADS] = [const { HeapStats::new() }; TRACKED_THREADS];
static NEXT_TRACKER: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static THREAD_TRACKER: std::cell::Cell<usize> = const { std::cell::Cell::new(NO_TRACKER) };
    static FILE_GENERATION: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static FILE_HEAP_PEAK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[derive(Clone, Copy)]
#[repr(C)]
struct AllocHeader {
    raw: usize,
    raw_size: usize,
    alignment: usize,
    user_offset: usize,
    user_size: usize,
    tracker: usize,
    generation: u64,
}

struct MeteringAllocator;

#[global_allocator]
static GLOBAL_ALLOCATOR: MeteringAllocator = MeteringAllocator;

unsafe impl alloc::GlobalAlloc for MeteringAllocator {
    unsafe fn alloc(&self, layout: alloc::Layout) -> *mut u8 {
        unsafe { tracked_alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: alloc::Layout) -> *mut u8 {
        let pointer = unsafe { tracked_alloc(layout) };
        if !pointer.is_null() {
            unsafe { ptr::write_bytes(pointer, 0, layout.size()) };
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, _layout: alloc::Layout) {
        if pointer.is_null() {
            return;
        }
        unsafe {
            let header = read_alloc_header(pointer);
            release_allocation(header);
            let layout =
                alloc::Layout::from_size_align_unchecked(header.raw_size, header.alignment);
            alloc::System.dealloc(header.raw as *mut u8, layout);
        }
    }

    unsafe fn realloc(&self, pointer: *mut u8, _layout: alloc::Layout, new_size: usize) -> *mut u8 {
        if pointer.is_null() || new_size == 0 {
            return ptr::null_mut();
        }
        unsafe {
            let old = read_alloc_header(pointer);
            let Some(raw_size) = old.user_offset.checked_add(new_size) else {
                return ptr::null_mut();
            };
            let Ok(layout) = alloc::Layout::from_size_align(old.raw_size, old.alignment) else {
                return ptr::null_mut();
            };
            let raw = alloc::System.realloc(old.raw as *mut u8, layout, raw_size);
            if raw.is_null() {
                return ptr::null_mut();
            }
            let (tracker, generation) = current_measurement();
            let mut new = old;
            new.raw = raw as usize;
            new.raw_size = raw_size;
            new.user_size = new_size;
            new.tracker = tracker;
            new.generation = generation;
            let result = raw.add(old.user_offset);
            ptr::write(alloc_header_ptr(result), new);
            resize_allocation(old, new);
            result
        }
    }
}

unsafe fn tracked_alloc(layout: alloc::Layout) -> *mut u8 {
    let alignment = layout.align().max(std::mem::align_of::<AllocHeader>());
    let Some(user_offset) = align_up(std::mem::size_of::<AllocHeader>(), alignment) else {
        return ptr::null_mut();
    };
    let Some(raw_size) = user_offset.checked_add(layout.size()) else {
        return ptr::null_mut();
    };
    let Ok(raw_layout) = alloc::Layout::from_size_align(raw_size, alignment) else {
        return ptr::null_mut();
    };
    let raw = unsafe { alloc::GlobalAlloc::alloc(&alloc::System, raw_layout) };
    if raw.is_null() {
        return raw;
    }
    let (tracker, generation) = current_measurement();
    let result = unsafe { raw.add(user_offset) };
    let header = AllocHeader {
        raw: raw as usize,
        raw_size,
        alignment,
        user_offset,
        user_size: layout.size(),
        tracker,
        generation,
    };
    unsafe { ptr::write(alloc_header_ptr(result), header) };
    record_allocation(header);
    result
}

fn align_up(value: usize, alignment: usize) -> Option<usize> {
    value
        .checked_add(alignment - 1)
        .map(|value| value & !(alignment - 1))
}

unsafe fn alloc_header_ptr(pointer: *mut u8) -> *mut AllocHeader {
    unsafe { pointer.sub(std::mem::size_of::<AllocHeader>()).cast() }
}

unsafe fn read_alloc_header(pointer: *mut u8) -> AllocHeader {
    unsafe { ptr::read(alloc_header_ptr(pointer)) }
}

fn current_measurement() -> (usize, u64) {
    THREAD_TRACKER.with(|slot| {
        let mut index = slot.get();
        if index == NO_TRACKER {
            index = NEXT_TRACKER.fetch_add(1, Ordering::Relaxed);
            if index >= TRACKED_THREADS {
                return (NO_TRACKER, 0);
            }
            slot.set(index);
        }
        (index, FILE_GENERATION.with(std::cell::Cell::get))
    })
}

fn record_allocation(header: AllocHeader) {
    if header.generation == 0 || header.tracker == NO_TRACKER {
        return;
    }
    let stats = &HEAP_STATS[header.tracker];
    if stats.active_generation.load(Ordering::Relaxed) != header.generation {
        return;
    }
    let current = stats
        .live_bytes
        .fetch_add(header.user_size, Ordering::Relaxed)
        + header.user_size;
    stats.peak_bytes.fetch_max(current, Ordering::Relaxed);
}

fn release_allocation(header: AllocHeader) {
    if header.generation == 0 || header.tracker == NO_TRACKER {
        return;
    }
    let stats = &HEAP_STATS[header.tracker];
    if stats.active_generation.load(Ordering::Relaxed) == header.generation {
        stats
            .live_bytes
            .fetch_sub(header.user_size, Ordering::Relaxed);
    }
}

fn resize_allocation(old: AllocHeader, new: AllocHeader) {
    if old.tracker == new.tracker && old.generation == new.generation && new.generation != 0 {
        let stats = &HEAP_STATS[new.tracker];
        if stats.active_generation.load(Ordering::Relaxed) == new.generation {
            let live = if new.user_size >= old.user_size {
                stats
                    .live_bytes
                    .fetch_add(new.user_size - old.user_size, Ordering::Relaxed)
                    + new.user_size
                    - old.user_size
            } else {
                stats
                    .live_bytes
                    .fetch_sub(old.user_size - new.user_size, Ordering::Relaxed)
                    - old.user_size
                    + new.user_size
            };
            stats.peak_bytes.fetch_max(live, Ordering::Relaxed);
            return;
        }
    }
    release_allocation(old);
    record_allocation(new);
}

fn measure_file_heap<T>(measure: impl FnOnce() -> T) -> (T, usize) {
    THREAD_TRACKER.with(|slot| {
        let mut index = slot.get();
        if index == NO_TRACKER {
            index = NEXT_TRACKER.fetch_add(1, Ordering::Relaxed);
            if index >= TRACKED_THREADS {
                return (measure(), 0);
            }
            slot.set(index);
        }
        let stats = &HEAP_STATS[index];
        let generation = stats.generation.fetch_add(1, Ordering::Relaxed) + 1;
        stats.live_bytes.store(0, Ordering::Relaxed);
        stats.peak_bytes.store(0, Ordering::Relaxed);
        stats.active_generation.store(generation, Ordering::Release);
        FILE_GENERATION.with(|active| active.set(generation));
        let result = measure();
        FILE_GENERATION.with(|active| active.set(0));
        stats.active_generation.store(0, Ordering::Release);
        (result, stats.peak_bytes.load(Ordering::Relaxed))
    })
}

fn fmt_bytes(bytes: usize) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    if bytes as f64 >= MIB {
        format!("{:.2} MiB", bytes as f64 / MIB)
    } else if bytes as f64 >= KIB {
        format!("{:.2} KiB", bytes as f64 / KIB)
    } else {
        format!("{bytes} B")
    }
}

/// Recursively collect TPTP problem and axiom files in a directory.
fn collect_tptp_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    if dir.is_dir()
        && let Ok(entries) = fs::read_dir(dir)
    {
        {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    files.extend(collect_tptp_files(&path));
                } else if path
                    .extension()
                    .is_some_and(|ext| ext == "p" || ext == "ax")
                {
                    files.push(path);
                }
            }
        }
    }

    files.sort();
    files
}

/// Result of parsing a single file
#[derive(Debug)]
enum ParseResult {
    Success {
        formula_count: usize,
        include_count: usize,
        read_ms: f64,
        elapsed_ms: f64,
    },
    RoundtripFail {
        formula_count: usize,
        read_ms: f64,
        elapsed_ms: f64,
        error: String,
    },
    Failed {
        read_ms: f64,
        elapsed_ms: f64,
        error: String,
    },
    ReadError {
        read_ms: f64,
        error: String,
    },
    Timeout {
        timeout_ms: u64,
    },
}

enum SweepError {
    Parse(String),
    Roundtrip { formula_count: usize, error: String },
    Cancelled,
}

/// Parse one file item at a time and verify that each Display representation
/// reparses to the same AST. Streaming avoids materializing enormous generated
/// TPTP files (some exceed 250 MiB) as a single in-memory problem AST.
fn parse_and_roundtrip(
    content: &str,
    cancelled: Option<&AtomicBool>,
) -> Result<(usize, usize), SweepError> {
    let mut iterator = TPTPIterator::new(content);
    let mut formula_count = 0;
    let mut include_count = 0;

    for item in iterator.by_ref() {
        if cancelled.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            return Err(SweepError::Cancelled);
        }

        match item.map_err(|error| SweepError::Parse(error.to_string()))? {
            TPTPInput::Formula(formula) => {
                formula_count += 1;
                let name = formula.name().to_owned();
                let pretty = formula.to_string();
                let reparsed =
                    parse_single_input(&pretty).map_err(|error| SweepError::Roundtrip {
                        formula_count,
                        error: format!(
                            "Failed to reparse formula '{name}': {error}\n  Display: {}",
                            preview(&pretty)
                        ),
                    })?;

                match reparsed {
                    TPTPInput::Formula(reparsed) => {
                        if formula != reparsed {
                            return Err(SweepError::Roundtrip {
                                formula_count,
                                error: format!(
                                    "Display reparsed to a different AST for formula '{name}'\n  Display: {}\n  AST diff: {}",
                                    preview(&pretty),
                                    compact_debug_diff(&formula, &reparsed),
                                ),
                            });
                        }
                    }
                    TPTPInput::Include(_) => {
                        return Err(SweepError::Roundtrip {
                            formula_count,
                            error: format!(
                                "Display for formula '{name}' reparsed as an include\n  Display: {}",
                                preview(&pretty)
                            ),
                        });
                    }
                }
            }
            TPTPInput::Include(include) => {
                include_count += 1;
                let pretty = include.to_string();
                let reparsed =
                    parse_single_input(&pretty).map_err(|error| SweepError::Roundtrip {
                        formula_count,
                        error: format!(
                            "Failed to reparse include '{}': {error}\n  Display: {}",
                            include.file_name,
                            preview(&pretty)
                        ),
                    })?;

                match reparsed {
                    TPTPInput::Include(reparsed) => {
                        if include != reparsed {
                            return Err(SweepError::Roundtrip {
                                formula_count,
                                error: format!(
                                    "Display reparsed to a different AST for include '{}'\n  Display: {}\n  AST diff: {}",
                                    include.file_name,
                                    preview(&pretty),
                                    compact_debug_diff(&include, &reparsed),
                                ),
                            });
                        }
                    }
                    TPTPInput::Formula(_) => {
                        return Err(SweepError::Roundtrip {
                            formula_count,
                            error: format!(
                                "Display for include '{}' reparsed as a formula\n  Display: {}",
                                include.file_name,
                                preview(&pretty)
                            ),
                        });
                    }
                }
            }
        }
    }

    if cancelled.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
        return Err(SweepError::Cancelled);
    }

    if !iterator.remaining().is_empty() {
        return Err(SweepError::Parse(format!(
            "{} bytes of input were not consumed",
            iterator.remaining().len()
        )));
    }

    Ok((formula_count, include_count))
}

fn parse_single_input(input: &str) -> Result<TPTPInput<'_>, String> {
    let mut iterator = TPTPIterator::new(input);
    let item = match iterator.next() {
        Some(Ok(item)) => item,
        Some(Err(error)) => return Err(error.to_string()),
        None => return Err("expected one TPTP input, got none".to_owned()),
    };

    match iterator.next() {
        Some(Ok(_)) => return Err("expected one TPTP input, got more than one".to_owned()),
        Some(Err(error)) => return Err(error.to_string()),
        None => {}
    }

    if !iterator.remaining().is_empty() {
        return Err(format!(
            "{} bytes of input were not consumed",
            iterator.remaining().len()
        ));
    }
    Ok(item)
}

fn preview(text: &str) -> String {
    const LIMIT: usize = 600;
    if text.len() <= LIMIT {
        return text.to_owned();
    }
    let mut end = LIMIT;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

fn compact_debug_diff(left: &impl std::fmt::Debug, right: &impl std::fmt::Debug) -> String {
    let left = format!("{left:?}");
    let right = format!("{right:?}");
    let common = left
        .bytes()
        .zip(right.bytes())
        .take_while(|(left, right)| left == right)
        .count();
    let mut start = common.saturating_sub(80).min(left.len()).min(right.len());
    while !left.is_char_boundary(start) || !right.is_char_boundary(start) {
        start -= 1;
    }
    let mut left_end = (common + 160).min(left.len());
    while !left.is_char_boundary(left_end) {
        left_end -= 1;
    }
    let mut right_end = (common + 160).min(right.len());
    while !right.is_char_boundary(right_end) {
        right_end -= 1;
    }
    format!(
        "first difference at byte {common}: original {:?}, reparsed {:?}",
        &left[start..left_end],
        &right[start..right_end]
    )
}

fn result_from_sweep(
    result: Result<(usize, usize), SweepError>,
    read_ms: f64,
    elapsed_ms: f64,
    timeout_ms: Option<u64>,
) -> ParseResult {
    match result {
        Ok((formula_count, include_count)) => ParseResult::Success {
            formula_count,
            include_count,
            read_ms,
            elapsed_ms,
        },
        Err(SweepError::Parse(error)) => ParseResult::Failed {
            read_ms,
            elapsed_ms,
            error,
        },
        Err(SweepError::Roundtrip {
            formula_count,
            error,
        }) => ParseResult::RoundtripFail {
            formula_count,
            read_ms,
            elapsed_ms,
            error,
        },
        Err(SweepError::Cancelled) => ParseResult::Timeout {
            timeout_ms: timeout_ms.unwrap_or_default(),
        },
    }
}

/// A job sent to the watchdog thread: fires `cancelled` at `deadline`.
struct WatchdogJob {
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
}

/// Parse a single file and return the result
fn parse_file(
    file: &Path,
    folder: &Path,
    timeout_ms: Option<u64>,
    watchdog_sender: Option<&mpsc::SyncSender<WatchdogJob>>,
    competition_parse: bool,
) -> (PathBuf, String, ParseResult, usize) {
    FILE_HEAP_PEAK.with(|peak| peak.set(0));
    let (file, relative_path, result) =
        parse_file_inner(file, folder, timeout_ms, watchdog_sender, competition_parse);
    let peak = FILE_HEAP_PEAK.with(std::cell::Cell::get);
    (file, relative_path, result, peak)
}

fn parse_file_inner(
    file: &Path,
    folder: &Path,
    timeout_ms: Option<u64>,
    watchdog_sender: Option<&mpsc::SyncSender<WatchdogJob>>,
    competition_parse: bool,
) -> (PathBuf, String, ParseResult) {
    let relative_path = file
        .strip_prefix(folder)
        .unwrap_or(file)
        .display()
        .to_string();

    if competition_parse {
        return parse_file_materialized(file, relative_path, timeout_ms, watchdog_sender);
    }

    let read_start = Instant::now();
    let content = match fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            return (
                file.to_path_buf(),
                relative_path,
                ParseResult::ReadError {
                    read_ms: read_start.elapsed().as_micros() as f64 / 1000.0,
                    error: format!("Read error: {}", e),
                },
            );
        }
    };
    let read_ms = read_start.elapsed().as_micros() as f64 / 1000.0;

    // If timeout is specified, register with the shared watchdog thread and parse directly
    // in this Rayon worker — no per-file OS thread spawn, so N workers stay N workers.
    // The watchdog fires `cancelled` at the deadline; with the "cancellation" feature the
    // parser checks it internally and returns early.  Without that feature a runaway file
    // will block this worker until the parse finishes naturally (deadlines in the TPTP set
    // are rarely hit, so this is acceptable).
    if let Some(timeout) = timeout_ms {
        let cancelled = Arc::new(AtomicBool::new(false));
        let start = Instant::now();
        let deadline = start + Duration::from_millis(timeout);

        // Register with the shared watchdog thread
        if let Some(sender) = watchdog_sender {
            let _ = sender.send(WatchdogJob {
                deadline,
                cancelled: cancelled.clone(),
            });
        }

        // Bind the cancellation flag to this thread so the parser can check it
        #[cfg(feature = "cancellation")]
        set_cancel_flag(&cancelled);

        // Stream items in this Rayon worker thread so large files do not
        // require an AST for every formula to be resident at once.
        let (result, peak) = measure_file_heap(|| parse_and_roundtrip(&content, Some(&cancelled)));
        FILE_HEAP_PEAK.with(|file_peak| file_peak.set(peak));
        let elapsed_ms = start.elapsed().as_micros() as f64 / 1000.0;

        #[cfg(feature = "cancellation")]
        clear_cancel_flag();

        // If the watchdog fired, report Timeout
        if cancelled.load(Ordering::Relaxed) {
            return (
                file.to_path_buf(),
                relative_path,
                ParseResult::Timeout {
                    timeout_ms: timeout,
                },
            );
        }

        let result = result_from_sweep(result, read_ms, elapsed_ms, Some(timeout));
        (file.to_path_buf(), relative_path, result)
    } else {
        // No timeout; still stream and round-trip each input independently.
        let start = Instant::now();
        let (result, peak) = measure_file_heap(|| parse_and_roundtrip(&content, None));
        FILE_HEAP_PEAK.with(|file_peak| file_peak.set(peak));
        let elapsed = start.elapsed();
        let elapsed_ms = elapsed.as_micros() as f64 / 1000.0;
        let result = result_from_sweep(result, read_ms, elapsed_ms, None);
        (file.to_path_buf(), relative_path, result)
    }
}

/// Mirror MRS's parse stage: read the whole input, build a complete
/// `TPTPProblem` with `parse_tptp`, and do not run the benchmark-only
/// display/reparse validation. The heap meter starts before the read so its
/// peak includes both the source `String` and the retained AST, as they coexist
/// in MRS. It still excludes allocator/runtime state allocated before this file.
fn parse_file_materialized(
    file: &Path,
    relative_path: String,
    timeout_ms: Option<u64>,
    watchdog_sender: Option<&mpsc::SyncSender<WatchdogJob>>,
) -> (PathBuf, String, ParseResult) {
    let cancelled = timeout_ms.map(|_| Arc::new(AtomicBool::new(false)));
    let (result, peak) = measure_file_heap(|| {
        let read_start = Instant::now();
        let content = match fs::read_to_string(file) {
            Ok(content) => content,
            Err(error) => {
                return ParseResult::ReadError {
                    read_ms: read_start.elapsed().as_micros() as f64 / 1000.0,
                    error: format!("Read error: {error}"),
                };
            }
        };
        let read_ms = read_start.elapsed().as_micros() as f64 / 1000.0;
        let start = Instant::now();

        if let (Some(timeout), Some(cancelled), Some(sender)) =
            (timeout_ms, cancelled.as_ref(), watchdog_sender)
        {
            let _ = sender.send(WatchdogJob {
                deadline: start + Duration::from_millis(timeout),
                cancelled: cancelled.clone(),
            });
        }

        #[cfg(feature = "cancellation")]
        if let Some(cancelled) = cancelled.as_ref() {
            set_cancel_flag(cancelled);
        }

        let parsed = mrs_tptp::parse_tptp(&content);
        let elapsed_ms = start.elapsed().as_micros() as f64 / 1000.0;

        #[cfg(feature = "cancellation")]
        if cancelled.is_some() {
            clear_cancel_flag();
        }

        if cancelled
            .as_ref()
            .is_some_and(|cancelled| cancelled.load(Ordering::Relaxed))
        {
            return ParseResult::Timeout {
                timeout_ms: timeout_ms.unwrap_or_default(),
            };
        }

        match parsed {
            Ok(problem) => ParseResult::Success {
                formula_count: problem.formulas.len(),
                include_count: problem.includes.len(),
                read_ms,
                elapsed_ms,
            },
            Err(error) => ParseResult::Failed {
                read_ms,
                elapsed_ms,
                error: error.to_string(),
            },
        }
    });
    FILE_HEAP_PEAK.with(|file_peak| file_peak.set(peak));
    (file.to_path_buf(), relative_path, result)
}

/// Background watchdog thread: receives `WatchdogJob` entries and sets each `cancelled` flag
/// when its deadline passes.  Exits when the sender side of the channel is dropped.
fn run_watchdog(receiver: mpsc::Receiver<WatchdogJob>) {
    // Pending jobs: (deadline, cancelled_arc).  At most N entries (one per Rayon worker).
    let mut pending: Vec<(Instant, Arc<AtomicBool>)> = Vec::new();
    loop {
        // Sleep until the next deadline (or 60 s if idle)
        let wait = if pending.is_empty() {
            Duration::from_secs(60)
        } else {
            let now = Instant::now();
            let min_deadline = pending.iter().map(|(d, _)| *d).min().unwrap();
            min_deadline
                .checked_duration_since(now)
                .unwrap_or(Duration::ZERO)
        };
        match receiver.recv_timeout(wait) {
            Ok(job) => {
                pending.push((job.deadline, job.cancelled));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        // Fire any expired deadlines
        let now = Instant::now();
        pending.retain(|(deadline, cancelled)| {
            if *deadline <= now {
                cancelled.store(true, Ordering::Relaxed);
                false
            } else {
                true
            }
        });
    }
}

fn main() {
    // Parse arguments first to get thread count
    let args: Vec<String> = env::args().collect();
    let num_threads = parse_thread_count(&args);

    // Rayon workers parse TPTP with a recursive-descent parser, so their stacks
    // have to survive a deeply nested formula; a spawned thread's 2 MiB default
    // is not enough. This is the same size `mrs_core::RECURSION_STACK_BYTES`
    // gives the search's threads, kept as a local copy because `mrs-tptp` has no
    // `mrs-core` dependency and the two must be kept in step deliberately.
    const RECURSION_STACK: usize = 64 * 1024 * 1024;

    // Limited parallelism to control memory.
    let pool = ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .stack_size(RECURSION_STACK)
        .build()
        .expect("Failed to build rayon thread pool");

    // Run everything inside the custom pool
    pool.install(|| main_inner(num_threads));
}

/// Parse the --threads argument from command line args
fn parse_thread_count(args: &[String]) -> usize {
    let default_threads = std::cmp::min(rayon::current_num_threads(), 8);

    let mut i = 1;
    while i < args.len() {
        if args[i] == "--threads" && i + 1 < args.len() {
            return args[i + 1].parse().unwrap_or_else(|_| {
                eprintln!(
                    "Warning: Invalid --threads value, using default ({})",
                    default_threads
                );
                default_threads
            });
        }
        i += 1;
    }
    default_threads
}

fn main_inner(num_threads: usize) {
    let args: Vec<String> = env::args().collect();

    // Parse arguments
    let mut folder = PathBuf::from("tests/resources");
    let mut timeout_ms: Option<u64> = None;
    let mut verbose = false;
    let mut competition_parse = false;

    let mut i = 1;
    while i < args.len() {
        if args[i] == "--timeout" {
            if i + 1 < args.len() {
                timeout_ms = Some(args[i + 1].parse().expect("Invalid timeout value"));
                i += 2;
            } else {
                eprintln!("Error: --timeout requires a value in milliseconds");
                std::process::exit(1);
            }
        } else if args[i] == "--threads" {
            // Already parsed in main(), skip here
            if i + 1 < args.len() {
                i += 2;
            } else {
                eprintln!("Error: --threads requires a value");
                std::process::exit(1);
            }
        } else if args[i] == "--verbose" || args[i] == "-v" {
            verbose = true;
            i += 1;
        } else if args[i] == "--competition-parse" {
            competition_parse = true;
            i += 1;
        } else if !args[i].starts_with('-') {
            folder = PathBuf::from(&args[i]);
            i += 1;
        } else {
            eprintln!("Unknown option: {}", args[i]);
            std::process::exit(1);
        }
    }

    if !folder.exists() {
        eprintln!("Error: folder '{}' does not exist", folder.display());
        std::process::exit(1);
    }

    println!("###################################");
    println!("TPTP Parser Test Suite (Parallel)");
    println!("###################################");
    println!();
    println!("Scanning folder: {}", folder.display());
    if competition_parse {
        println!("Parse mode: MRS-compatible full AST (`parse_tptp`), no round-trip check");
        println!(
            "Peak heap includes the source buffer and complete AST; it excludes process baseline, lowering, and search."
        );
    } else {
        println!("Parse mode: streaming parse plus per-item AST round-trip validation");
    }
    if let Some(timeout) = timeout_ms {
        println!("Timeout per file: {}ms", timeout);
    }
    println!();

    let files = collect_tptp_files(&folder);

    if files.is_empty() {
        println!("No .p or .ax files found in {}", folder.display());
        return;
    }

    let total_files = files.len();
    println!("Found {} TPTP files to parse", total_files);
    println!("Using {} threads", rayon::current_num_threads());
    println!();
    println!("{}", "=".repeat(70));

    // Exactly N long-lived worker loops claim whole-file tasks dynamically.
    let progress = AtomicUsize::new(0);
    let next_file = AtomicUsize::new(0);

    // Start the single watchdog thread when a timeout is configured.
    // It replaces the old per-file polling OS threads: N Rayon workers + 1 watchdog
    // instead of N Rayon workers + N parser threads + N polling threads.
    let watchdog = timeout_ms.map(|_| {
        let (sender, receiver) = mpsc::sync_channel::<WatchdogJob>(256);
        let handle = thread::Builder::new()
            .name("tptp-watchdog".to_string())
            .spawn(move || run_watchdog(receiver))
            .expect("Failed to spawn watchdog thread");
        (sender, handle)
    });
    // Clone the sender so the parallel closure can share it without moving `watchdog`
    let watchdog_sender_opt: Option<mpsc::SyncSender<WatchdogJob>> =
        watchdog.as_ref().map(|(s, _)| s.clone());

    let start_time = Instant::now();

    let result_slots = std::sync::Mutex::new(
        std::iter::repeat_with(|| None)
            .take(total_files)
            .collect::<Vec<Option<(PathBuf, String, ParseResult, usize)>>>(),
    );
    rayon::scope(|scope| {
        for _ in 0..num_threads {
            scope.spawn(|_| loop {
                let index = next_file.fetch_add(1, Ordering::Relaxed);
                let Some(file) = files.get(index) else { break };
                let result = parse_file(
                    file,
                    &folder,
                    timeout_ms,
                    watchdog_sender_opt.as_ref(),
                    competition_parse,
                );
                let current = progress.fetch_add(1, Ordering::Relaxed) + 1;
                let status = match &result.2 {
                    ParseResult::Success { formula_count, include_count, read_ms, elapsed_ms } => format!(
                        "OK (io:{read_ms:.2}ms parse:{elapsed_ms:.2}ms, {formula_count} formulas, {include_count} includes)"
                    ),
                    ParseResult::RoundtripFail { formula_count, read_ms, elapsed_ms, .. } => format!(
                        "ROUNDTRIP FAIL (io:{read_ms:.2}ms parse:{elapsed_ms:.2}ms, {formula_count} formulas)"
                    ),
                    ParseResult::Failed { read_ms, elapsed_ms, error } => format!(
                        "FAILED (io:{read_ms:.2}ms parse:{elapsed_ms:.2}ms)\n        Error: {error}"
                    ),
                    ParseResult::ReadError { read_ms, error } => format!("FAILED (io:{read_ms:.2}ms, {error})"),
                    ParseResult::Timeout { timeout_ms } => format!("TIMEOUT (>{timeout_ms}ms)"),
                };
                println!(
                    "[{:>4}/{}] Parsing {} ... {}, peak heap:{}",
                    current, total_files, result.1, status, fmt_bytes(result.3)
                );
                result_slots.lock().unwrap()[index] = Some(result);
            });
        }
    });
    let results: Vec<_> = result_slots
        .into_inner()
        .expect("result slots mutex poisoned")
        .into_iter()
        .map(|result| result.expect("worker did not process a file"))
        .collect();

    let total_time = start_time.elapsed();

    // Shut down the watchdog: drop both sender ends so the thread exits, then join it
    drop(watchdog_sender_opt);
    if let Some((sender, handle)) = watchdog {
        drop(sender);
        let _ = handle.join();
    }

    // Collect statistics
    let mut successful = 0;
    let mut failed = 0;
    let mut roundtrip_failures = 0;
    let mut timeouts = 0;
    let mut total_formulas = 0;
    let mut total_includes = 0;
    let mut total_read_time_ms: f64 = 0.0;
    let mut total_parse_time_ms: f64 = 0.0;
    // (rel_path, read_ms, elapsed_ms, status_string) — used to print top-10 in the summary
    let mut top_entries: Vec<(String, f64, f64, String)> = Vec::new();
    let mut failed_files: Vec<(PathBuf, String)> = Vec::new();
    let mut roundtrip_failed_files: Vec<(PathBuf, String, String)> = Vec::new();
    let mut timeout_files: Vec<(PathBuf, u64)> = Vec::new();

    let mut memory_top: Vec<(String, usize)> = Vec::with_capacity(results.len());
    for (file, rel_path, result, heap_peak) in results {
        memory_top.push((rel_path.clone(), heap_peak));
        match result {
            ParseResult::Success {
                formula_count,
                include_count,
                read_ms,
                elapsed_ms,
            } => {
                successful += 1;
                total_formulas += formula_count;
                total_includes += include_count;
                total_read_time_ms += read_ms;
                total_parse_time_ms += elapsed_ms;
                top_entries.push((
                    rel_path,
                    read_ms,
                    elapsed_ms,
                    format!(
                        "OK (io:{:.2}ms parse:{:.2}ms, {} formulas, {} includes, peak heap:{})",
                        read_ms,
                        elapsed_ms,
                        formula_count,
                        include_count,
                        fmt_bytes(heap_peak)
                    ),
                ));
            }
            ParseResult::RoundtripFail {
                formula_count,
                read_ms,
                elapsed_ms,
                error,
            } => {
                successful += 1; // Parsing succeeded, roundtrip failed
                roundtrip_failures += 1;
                total_formulas += formula_count;
                total_read_time_ms += read_ms;
                total_parse_time_ms += elapsed_ms;
                top_entries.push((
                    rel_path.clone(),
                    read_ms,
                    elapsed_ms,
                    format!(
                        "ROUNDTRIP FAIL (io:{:.2}ms parse:{:.2}ms, {} formulas, peak heap:{})",
                        read_ms,
                        elapsed_ms,
                        formula_count,
                        fmt_bytes(heap_peak)
                    ),
                ));
                roundtrip_failed_files.push((file, rel_path, error));
            }
            ParseResult::Failed {
                read_ms,
                elapsed_ms,
                error,
            } => {
                failed += 1;
                total_read_time_ms += read_ms;
                total_parse_time_ms += elapsed_ms;
                top_entries.push((
                    rel_path,
                    read_ms,
                    elapsed_ms,
                    format!("FAILED (io:{:.2}ms parse:{:.2}ms)", read_ms, elapsed_ms),
                ));
                failed_files.push((file, error));
            }
            ParseResult::ReadError { read_ms, error } => {
                failed += 1;
                total_read_time_ms += read_ms;
                failed_files.push((file, error));
            }
            ParseResult::Timeout { timeout_ms } => {
                timeouts += 1;
                timeout_files.push((file, timeout_ms));
            }
        }
    }

    println!();
    println!("{}", "=".repeat(70));
    println!();
    println!("###################################");
    println!("Summary");
    println!("###################################");
    println!();
    println!("Total files:      {}", total_files);
    println!(
        "Successful:       {} ({:.1}%)",
        successful,
        (successful as f64 / total_files as f64) * 100.0
    );
    println!(
        "Failed:           {} ({:.1}%)",
        failed,
        (failed as f64 / total_files as f64) * 100.0
    );
    println!(
        "Timeouts:         {} ({:.1}%)",
        timeouts,
        (timeouts as f64 / total_files as f64) * 100.0
    );
    println!();
    println!("Total formulas:   {}", total_formulas);
    println!("Total includes:   {}", total_includes);
    if competition_parse {
        println!("Roundtrip check:  skipped (--competition-parse)");
    } else {
        println!("Roundtrip fails:  {}", roundtrip_failures);
    }
    println!();
    println!("Wall time:        {:.2}s", total_time.as_secs_f64());
    println!("I/O time (sum):   {:.2}ms", total_read_time_ms);
    println!("Parse time (sum): {:.2}ms", total_parse_time_ms);

    if total_files > 0 {
        println!(
            "Avg I/O/file:     {:.2}ms",
            total_read_time_ms / total_files as f64
        );
        println!(
            "Avg parse/file:   {:.2}ms",
            total_parse_time_ms / total_files as f64
        );
        println!(
            "Throughput:       {:.1} files/sec",
            total_files as f64 / total_time.as_secs_f64()
        );
    }

    if !top_entries.is_empty() {
        println!();
        println!("###################################");
        println!("Top 10 Slowest Files");
        println!("###################################");
        println!();
        top_entries.sort_by(|a, b| {
            (b.1 + b.2)
                .partial_cmp(&(a.1 + a.2))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for (i, (rel_path, _, _, status)) in top_entries.iter().take(10).enumerate() {
            println!("[{:>2}/10] Parsing {} ... {}", i + 1, rel_path, status);
        }
    }

    if !memory_top.is_empty() {
        println!();
        println!("###################################");
        println!("Top 10 Peak Heap Files");
        println!("###################################");
        println!();
        memory_top.sort_by_key(|entry| std::cmp::Reverse(entry.1));
        for (i, (rel_path, peak)) in memory_top.iter().take(10).enumerate() {
            println!(
                "[{:>2}/10] {} ... peak heap:{}",
                i + 1,
                rel_path,
                fmt_bytes(*peak)
            );
        }
    }

    if !failed_files.is_empty() {
        println!();
        println!("###################################");
        println!("Parse Failed Files");
        println!("###################################");
        println!();
        for (file, error) in &failed_files {
            println!("  {} ", file.display());
            println!("    Error: {}", error);
        }
    }

    if !roundtrip_failed_files.is_empty() {
        println!();
        println!("###################################");
        println!("Roundtrip Failed Files");
        println!("###################################");
        println!();
        for (file, _rel_path, error) in &roundtrip_failed_files {
            println!("  {} ", file.display());
            println!("    {}", error);
            println!();
        }
    }

    if !timeout_files.is_empty() && verbose {
        println!();
        println!("###################################");
        println!("Timed Out Files");
        println!("###################################");
        println!();
        for (file, timeout) in &timeout_files {
            println!("  {}  (>{}ms)", file.display(), timeout);
        }
    }

    if !failed_files.is_empty()
        || (!competition_parse && !roundtrip_failed_files.is_empty())
        || !timeout_files.is_empty()
    {
        std::process::exit(1);
    } else {
        println!();
        println!("✓ All files parsed successfully!");
        if competition_parse {
            println!(
                "✓ Used full-AST MRS-compatible parse mode; round-trip validation was skipped."
            );
        } else {
            println!("✓ Parsing-reparsing comparison successful for all files.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn competition_parse_measures_source_and_builds_the_complete_problem_ast() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("mrs-parse-folder-{}-{unique}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let file = directory.join("problem.p");
        let source =
            "include('axioms.ax',[chosen]).\nfof(chosen,axiom,p).\nfof(goal,conjecture,p).\n";
        fs::write(&file, source).unwrap();

        let (_, relative, result, peak) = parse_file(&file, &directory, None, None, true);
        assert_eq!(relative, "problem.p");
        assert!(matches!(
            result,
            ParseResult::Success {
                formula_count: 2,
                include_count: 1,
                ..
            }
        ));
        assert!(
            peak >= source.len(),
            "source buffer must be inside peak accounting"
        );

        fs::remove_file(file).unwrap();
        fs::remove_dir(directory).unwrap();
    }
}
