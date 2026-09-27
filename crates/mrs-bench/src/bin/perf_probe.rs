//! perf_probe — fixed-work performance probe for `mrs`, and the results bank.
//!
//! # Why fixed work, not a wall clock
//!
//! The obvious way to benchmark a prover is to run it for N seconds and count
//! what it did. That number is not comparable across machines, because the
//! search itself is wall-clock sensitive: LRS prunes the passive queue from
//! `elapsed / iteration`, so a slower machine explores a *different* search
//! space, not a slower version of the same one. Two hosts can both report
//! "50k clauses/second" while having done completely different amounts of work.
//!
//! This probe removes the clock from the work. Every run is stopped by
//! `resource_limits.max_processed`, an iteration-counted ceiling, and LRS is
//! switched to `LrsPolicy::FixedIterations` so passive-queue pruning is also
//! iteration-counted. The only remaining wall-clock dependency is the deadline
//! check, and the deadline is set far beyond any plausible run so it never
//! fires. The counters (`iterations`, `processed`, `generated`,
//! `fwd_subsumed`, `lrs_discarded`) are therefore identical on every machine
//! and for every build, and elapsed time is the only variable. `work_sha` in
//! the bank is the digest of exactly those counters, so a reader can verify
//! that two rows really did the same work instead of taking the claim on
//! trust, and the report refuses to rank rows whose work fingerprints differ.
//!
//! # What is measured
//!
//! A seeded, generated clause set (no TPTP file, no corpus, no `TPTP`
//! environment variable) driven through `mrs_search::strategy::run_schedule`,
//! the same entry point `src/main.rs` uses for a real problem. The probe
//! therefore exercises the production path: preprocessing, the given-clause
//! loop, literal selection, ordering, indexing, redundancy elimination, and
//! the resource ceilings.
//!
//! The generator is a hand-rolled SplitMix64 rather than a `rand` release, on
//! purpose. A bank row is only reproducible if the workload is pinned by this
//! repository's own code; a transitive dependency bump that changed the random
//! stream would silently invalidate every archived row.
//!
//! # One measurement per process
//!
//! Two searches running *concurrently in one process* do not produce identical
//! counters, although the same search repeated in fresh processes does. The
//! probe therefore measures once and exits; `perf_probe.sh` runs the binary
//! once per row, and repeats the primary measurement in a second process to
//! confirm the two agree before it writes anything to the bank. Do not add an
//! in-process loop over `measure()`.
//!
//! # Usage
//!
//! ```text
//! perf_probe measure [OPTIONS]      # run the search, emit one JSON row per line
//! perf_probe bank --rows R.jsonl \  # append rows to the TSV bank and render
//!            --tsv  docs/results/perf/bank.tsv \   # the Markdown report
//!            --md   docs/results/perf/DATE-HOST.md
//! ```
//!
//! `crates/mrs-bench/perf_probe.sh` is the supported driver: it builds the
//! `native` and `haswell` variants, supplies the build provenance, applies the
//! memory budget, and calls both subcommands.

use std::fmt::Write as _;
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mrs_core::Literal;
use mrs_core::clause::{Clause, ClauseIdGen, ClauseSource};
use mrs_core::formula::Atom;
use mrs_core::symbol::{SymbolId, SymbolTable};
use mrs_core::term::{Term, VarId};
use mrs_search::strategy::{MlOptions, StrategySchedule, run_schedule};
use mrs_search::{
    LiteralSelection, LrsPolicy, ResourceLimits, SearchConfig, SearchResult, SelectionStrategy,
    TermOrdering,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Total RAM the whole probe may use, in MB.
///
/// One probe process runs its phases sequentially, so its own ceiling *is* the
/// total. The driver must not run two probes concurrently at this budget; it
/// divides the budget by the concurrency instead (see `perf_probe.sh`).
const DEFAULT_MEMORY_BUDGET_MB: u64 = 12 * 1024;

/// Default clause budget: enough inferences to dominate process startup and
/// clause generation, short enough that a full probe stays interactive. On the
/// reference host (6 physical cores of a 3.3 GHz i7-5820K) this is about 11
/// seconds of single-worker search at under 400 MB, with run-to-run spread
/// around one percent.
const DEFAULT_PROCESSED_CAP: u64 = 5_000;

/// Safety-net wall clock. This is *not* the measurement: it is set far beyond
/// any expected run so the fixed-work ceiling is what stops the search. A row
/// that ends in `Timeout` means the workload regressed badly, and the tool
/// marks it not comparable.
const DEFAULT_TIME_LIMIT_S: u64 = 3_600;

/// Passive-queue ceiling. Bounds the memory a runaway workload can reach
/// before the memory watchdog fires; hitting it marks the row not comparable.
const DEFAULT_MAX_PASSIVE: u64 = 2_000_000;

/// Term-bank ceiling, with the same role as [`DEFAULT_MAX_PASSIVE`].
const DEFAULT_MAX_TERMS: u64 = 8_000_000;

// ---------------------------------------------------------------------------
// Deterministic RNG
// ---------------------------------------------------------------------------

/// SplitMix64. Small, well distributed, and frozen in this file so that the
/// generated workload cannot change under an archived bank row.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform-enough value in `0..n`. The modulo bias is negligible for every
    /// `n` used here and irrelevant to a synthetic workload.
    fn below(&mut self, n: u64) -> u64 {
        debug_assert!(n > 0, "below(0) has no valid result");
        self.next_u64() % n.max(1)
    }

    fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.below(hi - lo + 1)
    }

    /// True with probability `num / den`.
    fn chance(&mut self, num: u64, den: u64) -> bool {
        self.below(den) < num
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len() as u64) as usize]
    }
}

// ---------------------------------------------------------------------------
// Workload specification and generation
// ---------------------------------------------------------------------------

/// Which part of the calculus a workload exercises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    /// Equalities and predicate atoms: superposition, demodulation, and
    /// resolution together. The default, because it is the balanced case.
    Mixed,
    /// Equalities only: paramodulation, ordering, and the equality indexes.
    Equational,
    /// Atoms only, no function terms: resolution and subsumption without
    /// paramodulation.
    Relational,
}

impl Shape {
    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "mixed" => Some(Shape::Mixed),
            "equational" => Some(Shape::Equational),
            "relational" => Some(Shape::Relational),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Shape::Mixed => "mixed",
            Shape::Equational => "equational",
            Shape::Relational => "relational",
        }
    }

    /// Whether the shape can build non-ground, non-atomic terms.
    fn has_functions(self) -> bool {
        !matches!(self, Shape::Relational)
    }
}

/// Everything that determines the generated clause set.
///
/// Two rows are comparable only when their [`Spec::id`] matches, so the
/// specification is hashed into a short identifier that the bank carries.
#[derive(Clone, Copy, Debug)]
struct Spec {
    shape: Shape,
    seed: u64,
    clauses: usize,
    funcs: usize,
    consts: usize,
    preds: usize,
    depth: u32,
    width: usize,
    max_term_weight: u32,
}

impl Default for Spec {
    fn default() -> Self {
        Spec {
            shape: Shape::Mixed,
            seed: 0x5EED_1234_ABCD_0001,
            clauses: 600,
            funcs: 8,
            consts: 12,
            preds: 16,
            depth: 4,
            width: 3,
            max_term_weight: 200,
        }
    }
}

impl Spec {
    /// Stable identifier: the shape plus a digest of every numeric parameter.
    ///
    /// Bumping [`WORKLOAD_VERSION`] deliberately invalidates every archived
    /// row, which is the correct outcome when the generator changes meaning.
    fn id(&self) -> String {
        let payload = format!(
            "v{WORKLOAD_VERSION} shape={} seed={} clauses={} funcs={} consts={} preds={} depth={} \
             width={} max_term_weight={}",
            self.shape.as_str(),
            self.seed,
            self.clauses,
            self.funcs,
            self.consts,
            self.preds,
            self.depth,
            self.width,
            self.max_term_weight,
        );
        format!(
            "{}-{}",
            self.shape.as_str(),
            short_digest(payload.as_bytes())
        )
    }

    fn describe(&self) -> String {
        format!(
            "seed={} clauses={} funcs={} consts={} preds={} depth={} width={} max_term_weight={}",
            self.seed,
            self.clauses,
            self.funcs,
            self.consts,
            self.preds,
            self.depth,
            self.width,
            self.max_term_weight
        )
    }
}

/// Version of the generator's output contract. Part of every workload id.
const WORKLOAD_VERSION: u32 = 1;

fn short_digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)[..4]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A generated clause set plus everything `run_schedule` needs to search it.
struct Generated {
    clauses: Vec<Clause>,
    id_gen: ClauseIdGen,
    symbols: SymbolTable,
}

/// Symbols interned for one workload, kept together so generation never
/// re-interns and the term shapes stay stable across clauses.
struct Signature {
    funcs: Vec<SymbolId>,
    consts: Vec<SymbolId>,
    preds: Vec<SymbolId>,
}

/// Builds the generated workload: the prelude axioms followed by the seeded
/// random clauses.
fn generate(spec: &Spec) -> Generated {
    let mut symbols = SymbolTable::new();
    let sig = Signature::intern(spec, &mut symbols);
    let mut id_gen = ClauseIdGen::new();
    let mut clauses = Vec::with_capacity(spec.clauses + 2);

    if spec.shape.has_functions() {
        clauses.extend(prelude_axioms(&mut symbols, &mut id_gen));
    }

    let mut rng = Rng::new(spec.seed);
    for i in 0..spec.clauses {
        let vars: Vec<VarId> = (0..spec.width as VarId + 2).collect();
        let width = rng.range(2, spec.width as u64) as usize;
        let literals: Vec<Literal> = (0..width)
            .map(|_| gen_literal(&mut rng, spec, &sig, &vars))
            .collect();
        clauses.push(
            Clause::new(
                id_gen.next(),
                literals,
                ClauseSource::Input {
                    name: format!("perf_{i}"),
                    role: "axiom".to_string(),
                },
            )
            .with_distance(100),
        );
    }

    Generated {
        clauses,
        id_gen,
        symbols,
    }
}

impl Signature {
    fn intern(spec: &Spec, symbols: &mut SymbolTable) -> Signature {
        // `f0` and `f1` are binary because the prelude axioms use them that
        // way, so never intern fewer than two function symbols.
        let funcs = if spec.shape.has_functions() {
            (0..spec.funcs.max(2))
                .map(|i| symbols.intern(&format!("f{i}")))
                .collect()
        } else {
            Vec::new()
        };
        let consts = (0..spec.consts)
            .map(|i| symbols.intern(&format!("c{i}")))
            .collect();
        let preds = (0..spec.preds)
            .map(|i| symbols.intern(&format!("p{i}")))
            .collect();
        Signature {
            funcs,
            consts,
            preds,
        }
    }
}

/// Fixed axioms that guarantee an unbounded superposition closure.
///
/// A purely random equational set tends to have a small finite closure and
/// saturates almost immediately, which measures nothing. Commutativity and
/// distributivity over a binary symbol keep producing new terms, so the search
/// stays in its generate/subsume/discard loop for the whole budget instead of
/// finishing in a few thousand inferences. They are part of the workload
/// definition rather than random output, so they consume no seed entropy.
fn prelude_axioms(symbols: &mut SymbolTable, id_gen: &mut ClauseIdGen) -> Vec<Clause> {
    let f0 = symbols.intern("f0");
    let f1 = symbols.intern("f1");
    let (x, y, z) = (Term::var(0), Term::var(1), Term::var(2));

    // f0(X,Y) = f0(Y,X)
    let commutativity = eq_clause(
        id_gen,
        0,
        Term::app(f0, vec![x.clone(), y.clone()]),
        Term::app(f0, vec![y.clone(), x.clone()]),
    );
    // f1(f0(X,Y),Z) = f0(f1(X,Z), f1(Y,Z))
    let distributivity = eq_clause(
        id_gen,
        1,
        Term::app(
            f1,
            vec![Term::app(f0, vec![x.clone(), y.clone()]), z.clone()],
        ),
        Term::app(
            f0,
            vec![
                Term::app(f1, vec![x.clone(), z.clone()]),
                Term::app(f1, vec![y.clone(), z.clone()]),
            ],
        ),
    );
    vec![commutativity, distributivity]
}

fn eq_clause(id_gen: &mut ClauseIdGen, index: usize, left: Term, right: Term) -> Clause {
    Clause::new(
        id_gen.next(),
        vec![Literal::pos(Atom::eq(left, right))],
        ClauseSource::Input {
            name: format!("perf_prelude_{index}"),
            role: "axiom".to_string(),
        },
    )
    .with_distance(100)
}

fn gen_literal(rng: &mut Rng, spec: &Spec, sig: &Signature, vars: &[VarId]) -> Literal {
    let want_equality =
        spec.shape == Shape::Equational || (spec.shape == Shape::Mixed && rng.chance(1, 2));
    if want_equality && !sig.funcs.is_empty() {
        // Equalities are always positive, as in real TPTP input. A clause whose
        // two sides are syntactically equal is a tautology that preprocessing
        // would delete, so keep drawing until they differ.
        loop {
            let left = gen_term(rng, sig, vars, spec.depth);
            let right = gen_term(rng, sig, vars, spec.depth);
            if left != right {
                return Literal::pos(Atom::eq(left, right));
            }
        }
    }
    let pred = *rng.pick(&sig.preds);
    let arity = u32::from(rng.chance(1, 2)) + 1;
    let args = (0..arity)
        .map(|_| gen_term(rng, sig, vars, spec.depth.saturating_sub(1)))
        .collect();
    let atom = Atom::pred(pred, args);
    if rng.chance(1, 2) {
        Literal::pos(atom)
    } else {
        Literal::neg(atom)
    }
}

fn gen_term(rng: &mut Rng, sig: &Signature, vars: &[VarId], depth: u32) -> Term {
    if depth == 0 || rng.chance(1, 4) {
        if rng.chance(1, 2) {
            return Term::var(*rng.pick(vars));
        }
        return Term::constant(*rng.pick(&sig.consts));
    }
    if sig.funcs.is_empty() {
        return Term::var(*rng.pick(vars));
    }
    let symbol = *rng.pick(&sig.funcs);
    let arity = if symbol == sig.funcs[0] || symbol == sig.funcs[1] {
        2
    } else {
        1
    };
    let args = (0..arity)
        .map(|_| gen_term(rng, sig, vars, depth - 1))
        .collect();
    Term::app(symbol, args)
}

// ---------------------------------------------------------------------------
// Host fingerprint
// ---------------------------------------------------------------------------

/// What the probe could measure about the machine it ran on.
struct HostInfo {
    cpu_model: String,
    physical_cores: usize,
    logical_cpus: usize,
    ram_total_mb: u64,
    kernel: String,
    arch: String,
}

impl HostInfo {
    fn detect() -> HostInfo {
        HostInfo {
            cpu_model: cpu_model().unwrap_or_else(|| "unknown".to_string()),
            physical_cores: mrs_search::usable_physical_cores().max(1),
            logical_cpus: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1),
            ram_total_mb: meminfo_kb("MemTotal:").map(|kb| kb / 1024).unwrap_or(0),
            kernel: fs::read_to_string("/proc/sys/kernel/osrelease")
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| "unknown".to_string()),
            arch: std::env::consts::ARCH.to_string(),
        }
    }

    /// Filesystem- and Markdown-safe short name for this host, used in report
    /// filenames. Includes a digest of the full CPU string so two machines with
    /// the same marketing name do not silently share a report.
    fn slug(&self) -> String {
        let mut label: String = self
            .cpu_model
            .to_ascii_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        while label.contains("--") {
            label = label.replace("--", "-");
        }
        let label: String = label.trim_matches('-').chars().take(28).collect();
        format!(
            "{label}-x{}-{}",
            self.physical_cores,
            short_digest(self.cpu_model.as_bytes())
        )
    }
}

fn cpu_model() -> Option<String> {
    let cpuinfo = fs::read_to_string("/proc/cpuinfo").ok()?;
    cpuinfo.lines().find_map(|line| match line.split_once(':') {
        Some((key, value)) if key.trim() == "model name" => Some(value.trim().to_string()),
        _ => None,
    })
}

fn meminfo_kb(key: &str) -> Option<u64> {
    read_keyed_kb("/proc/meminfo", key)
}

fn read_keyed_kb(path: &str, key: &str) -> Option<u64> {
    let content = fs::read_to_string(path).ok()?;
    content.lines().find_map(|line| {
        line.strip_prefix(key)?
            .split_whitespace()
            .next()?
            .parse()
            .ok()
    })
}

/// Peak resident set size in MB, from `VmHWM` in `/proc/self/status`.
///
/// `VmHWM` is the kernel's own high-water mark, so this is a true peak rather
/// than a sample taken after the run, which is all the probe needs to report.
fn peak_rss_mb() -> u64 {
    read_keyed_kb("/proc/self/status", "VmHWM:")
        .map(|kb| kb / 1024)
        .unwrap_or(0)
}

fn sha256_file(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    Some(
        Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    )
}

// ---------------------------------------------------------------------------
// Result row
// ---------------------------------------------------------------------------

/// How a bank column is encoded in the TSV, and therefore how it is parsed back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Text,
    U64,
    F64,
}

/// Bank columns, in order. This is the TSV header, and it must stay in step
/// with [`Row`]; `columns_match_row_fields` fails the build's test run when the
/// two drift apart, and a header mismatch at append time is a hard error rather
/// than a silently widened file.
const COLUMNS: &[(&str, Kind)] = &[
    ("date", Kind::Text),
    ("host_slug", Kind::Text),
    ("cpu_model", Kind::Text),
    ("physical_cores", Kind::U64),
    ("logical_cpus", Kind::U64),
    ("ram_total_mb", Kind::U64),
    ("kernel", Kind::Text),
    ("arch", Kind::Text),
    ("commit", Kind::Text),
    ("dirty", Kind::Text),
    ("target_cpu", Kind::Text),
    ("rustc", Kind::Text),
    ("glibc", Kind::Text),
    ("binary_sha256", Kind::Text),
    ("hard_cap", Kind::Text),
    ("workload", Kind::Text),
    ("workload_spec", Kind::Text),
    ("selection", Kind::Text),
    ("ordering", Kind::Text),
    ("literals", Kind::Text),
    ("avatar", Kind::Text),
    ("workers", Kind::U64),
    ("processed_cap", Kind::U64),
    ("lrs_budget", Kind::U64),
    ("max_passive", Kind::U64),
    ("max_terms", Kind::U64),
    ("mem_budget_mb", Kind::U64),
    ("result", Kind::Text),
    ("stop_reason", Kind::Text),
    ("complete", Kind::U64),
    ("iterations", Kind::U64),
    ("processed", Kind::U64),
    ("generated", Kind::U64),
    ("fwd_subsumed", Kind::U64),
    ("lrs_discarded", Kind::U64),
    ("weight_discarded", Kind::U64),
    ("schedule_ms", Kind::U64),
    ("search_ms", Kind::U64),
    ("repeat", Kind::U64),
    ("repeat_spread_pct", Kind::F64),
    ("iterations_per_s", Kind::F64),
    ("processed_per_s", Kind::F64),
    ("generated_per_s", Kind::F64),
    ("generated_per_processed", Kind::F64),
    ("peak_rss_mb", Kind::U64),
    ("work_sha", Kind::Text),
    ("note", Kind::Text),
];

fn header_line() -> String {
    COLUMNS
        .iter()
        .map(|(name, _)| *name)
        .collect::<Vec<_>>()
        .join("\t")
}

/// One measurement: the machine, the build, the workload, and what it did.
#[derive(Clone, Debug, Deserialize, Serialize)]
struct Row {
    date: String,
    host_slug: String,
    cpu_model: String,
    physical_cores: usize,
    logical_cpus: usize,
    ram_total_mb: u64,
    kernel: String,
    arch: String,
    commit: String,
    dirty: String,
    target_cpu: String,
    rustc: String,
    glibc: String,
    binary_sha256: String,
    hard_cap: String,
    workload: String,
    workload_spec: String,
    selection: String,
    ordering: String,
    literals: String,
    avatar: String,
    workers: usize,
    processed_cap: u64,
    lrs_budget: u64,
    max_passive: u64,
    max_terms: u64,
    mem_budget_mb: u64,
    result: String,
    stop_reason: String,
    complete: u8,
    iterations: u64,
    processed: u64,
    generated: u64,
    fwd_subsumed: u64,
    lrs_discarded: u64,
    weight_discarded: u64,
    schedule_ms: u64,
    search_ms: u64,
    /// How many fresh processes this row is the best of. A single run on a
    /// shared machine is easily perturbed by an unrelated process, so
    /// `perf_probe.sh` repeats each configuration and keeps the fastest.
    repeat: u64,
    /// Spread of those repeats around the best one, as a percentage of the
    /// best. This is the measurement noise floor of the row: a comparison
    /// smaller than it means nothing.
    repeat_spread_pct: f64,
    iterations_per_s: f64,
    processed_per_s: f64,
    generated_per_s: f64,
    generated_per_processed: f64,
    peak_rss_mb: u64,
    work_sha: String,
    note: String,
}

impl Row {
    fn tsv_cells(&self) -> Vec<String> {
        let value = serde_json::to_value(self).expect("Row is always serializable");
        COLUMNS
            .iter()
            .map(|(column, kind)| {
                let cell = value
                    .get(*column)
                    .unwrap_or_else(|| panic!("Row field {column} is missing"));
                let text = match (kind, cell) {
                    (Kind::Text, serde_json::Value::String(s)) => s.clone(),
                    (Kind::Text, serde_json::Value::Null) => String::new(),
                    (Kind::Text, other) => other.to_string(),
                    (_, serde_json::Value::Number(n)) => n.to_string(),
                    (_, other) => other.to_string(),
                };
                assert!(
                    !text.contains(['\t', '\n', '\r']),
                    "TSV cell {column} contains a field separator"
                );
                text
            })
            .collect()
    }

    fn from_cells(cells: &[&str]) -> Result<Row, String> {
        let mut map = serde_json::Map::new();
        for ((column, kind), cell) in COLUMNS.iter().zip(cells) {
            let value = match kind {
                Kind::Text => serde_json::Value::String((*cell).to_string()),
                Kind::U64 => serde_json::Value::from(
                    cell.parse::<u64>()
                        .map_err(|_| format!("column {column} is not an integer: {cell}"))?,
                ),
                Kind::F64 => serde_json::Value::from(
                    cell.parse::<f64>()
                        .map_err(|_| format!("column {column} is not a number: {cell}"))?,
                ),
            };
            map.insert((*column).to_string(), value);
        }
        serde_json::from_value(serde_json::Value::Object(map))
            .map_err(|e| format!("cannot rebuild a row from its TSV cells: {e}"))
    }
}

fn parse_tsv(text: &str, source: &Path) -> Result<Vec<Row>, String> {
    let mut lines = text.lines();
    match lines.next() {
        Some(header) if header == header_line() => {}
        Some(_) => {
            return Err(format!(
                "{} has an unexpected header; the bank is append-only and must not be rewritten",
                source.display()
            ));
        }
        None => return Ok(Vec::new()),
    }
    let mut rows = Vec::new();
    for (index, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let cells: Vec<&str> = line.split('\t').collect();
        if cells.len() != COLUMNS.len() {
            return Err(format!(
                "{}:{} has {} cells, expected {}",
                source.display(),
                index + 2,
                cells.len(),
                COLUMNS.len()
            ));
        }
        rows.push(Row::from_cells(&cells)?);
    }
    Ok(rows)
}

// ---------------------------------------------------------------------------
// measure
// ---------------------------------------------------------------------------

struct MeasureArgs {
    spec: Spec,
    workers: usize,
    processed_cap: u64,
    /// LRS logical-iteration budget. `None` means "the processed ceiling", which
    /// is the right answer: LRS sizes the passive queue from the iteration
    /// budget it believes it has, so tying the two keeps the queue bounded and
    /// the per-iteration cost steady. Leaving LRS effectively disabled lets the
    /// passive queue grow without bound, which makes cost per inference climb
    /// superlinearly and the measurement drift instead of holding steady.
    lrs_budget: Option<u64>,
    time_limit_s: u64,
    max_passive: u64,
    max_terms: u64,
    mem_budget_mb: u64,
    selection: SelectionStrategy,
    ordering: TermOrdering,
    literals: LiteralSelection,
    avatar: bool,
    binary: Option<PathBuf>,
    commit: String,
    dirty: String,
    target_cpu: String,
    rustc: String,
    glibc: String,
    hard_cap: String,
    note: String,
    repeat: u64,
    repeat_spread_pct: f64,
    out_path: Option<PathBuf>,
}

impl Default for MeasureArgs {
    fn default() -> MeasureArgs {
        MeasureArgs {
            spec: Spec::default(),
            workers: 1,
            processed_cap: DEFAULT_PROCESSED_CAP,
            lrs_budget: None,
            time_limit_s: DEFAULT_TIME_LIMIT_S,
            max_passive: DEFAULT_MAX_PASSIVE,
            max_terms: DEFAULT_MAX_TERMS,
            mem_budget_mb: DEFAULT_MEMORY_BUDGET_MB,
            selection: SelectionStrategy::AgeWeight(5),
            ordering: TermOrdering::KBO,
            literals: LiteralSelection::AllNegative,
            avatar: true,
            binary: None,
            commit: "unknown".to_string(),
            dirty: "unknown".to_string(),
            target_cpu: "unknown".to_string(),
            rustc: "unknown".to_string(),
            glibc: "unknown".to_string(),
            hard_cap: "none".to_string(),
            note: String::new(),
            repeat: 1,
            repeat_spread_pct: 0.0,
            out_path: None,
        }
    }
}

fn measure(args: &MeasureArgs) -> Row {
    // The memory watchdog reads the environment, so publish the budget there as
    // well as passing it in `ResourceLimits`: any nested default built deeper
    // in the search then inherits this probe's ceiling instead of the host's
    // 80%-of-RAM policy.
    //
    // Safety: single-threaded at this point in `main`, and the probe is a leaf
    // process, so nothing else can observe the environment concurrently.
    unsafe {
        std::env::set_var("MRS_MAX_MEMORY_MB", args.mem_budget_mb.to_string());
    }

    let host = HostInfo::detect();
    let generated = generate(&args.spec);
    let clauses_in = generated.clauses.len();

    let time_limit = Duration::from_secs(args.time_limit_s);
    let config = SearchConfig {
        time_limit,
        selection: args.selection.clone(),
        literal_selection: args.literals.clone(),
        ordering: args.ordering.clone(),
        max_term_weight: Some(args.spec.max_term_weight),
        use_avatar: args.avatar,
        lrs_policy: LrsPolicy::FixedIterations {
            budget: args.lrs_budget.unwrap_or(args.processed_cap),
        },
        resource_limits: ResourceLimits {
            max_processed: Some(args.processed_cap),
            max_passive: Some(args.max_passive),
            max_terms: Some(args.max_terms as usize),
            max_memory_mb: Some(args.mem_budget_mb),
        },
        ..Default::default()
    };

    // One identical strategy per worker: the classic strong-scaling shape.
    // `run_schedule` runs `min(workers, schedule.len())` searches, so a shorter
    // schedule would silently measure a single worker.
    let schedule = StrategySchedule {
        strategies: vec![(config, time_limit); args.workers],
    };

    let (result, report) = run_schedule(
        &generated.clauses,
        &[],
        generated.id_gen,
        &schedule,
        &generated.symbols,
        MlOptions::default(),
        Some(args.workers),
    );

    let (result_name, stop_reason, complete) = describe_result(&result);
    let mut totals = Totals::default();
    for strategy in &report.strategies {
        totals.add(&strategy.stats);
        totals.search_ms = totals.search_ms.max(strategy.elapsed_ms);
    }
    let schedule_ms = report.elapsed_ms.max(1);
    let seconds = schedule_ms as f64 / 1000.0;

    let work_sha = short_digest(
        format!(
            "v{WORKLOAD_VERSION} {} {} {} {} {} {}",
            totals.iterations,
            totals.processed,
            totals.generated,
            totals.fwd_subsumed,
            totals.lrs_discarded,
            totals.weight_discarded,
        )
        .as_bytes(),
    );

    Row {
        date: utc_date(),
        host_slug: host.slug(),
        cpu_model: host.cpu_model,
        physical_cores: host.physical_cores,
        logical_cpus: host.logical_cpus,
        ram_total_mb: host.ram_total_mb,
        kernel: host.kernel,
        arch: host.arch,
        commit: args.commit.clone(),
        dirty: args.dirty.clone(),
        target_cpu: args.target_cpu.clone(),
        rustc: args.rustc.clone(),
        glibc: args.glibc.clone(),
        binary_sha256: args
            .binary
            .as_deref()
            .and_then(sha256_file)
            .unwrap_or_else(|| "unknown".to_string()),
        hard_cap: args.hard_cap.clone(),
        workload: args.spec.id(),
        workload_spec: args.spec.describe(),
        selection: format!("{:?}", args.selection),
        ordering: format!("{:?}", args.ordering),
        literals: format!("{:?}", args.literals),
        avatar: u8::from(args.avatar).to_string(),
        workers: args.workers,
        processed_cap: args.processed_cap,
        lrs_budget: args.lrs_budget.unwrap_or(args.processed_cap),
        max_passive: args.max_passive,
        max_terms: args.max_terms,
        mem_budget_mb: args.mem_budget_mb,
        result: result_name,
        stop_reason,
        complete,
        iterations: totals.iterations,
        processed: totals.processed,
        generated: totals.generated,
        fwd_subsumed: totals.fwd_subsumed,
        lrs_discarded: totals.lrs_discarded,
        weight_discarded: totals.weight_discarded,
        schedule_ms,
        search_ms: totals.search_ms,
        repeat: args.repeat,
        repeat_spread_pct: args.repeat_spread_pct,
        iterations_per_s: totals.iterations as f64 / seconds,
        processed_per_s: totals.processed as f64 / seconds,
        generated_per_s: totals.generated as f64 / seconds,
        generated_per_processed: ratio(totals.generated, totals.processed),
        peak_rss_mb: peak_rss_mb(),
        work_sha,
        note: if args.note.is_empty() {
            format!("clauses_in={clauses_in}")
        } else {
            format!("clauses_in={clauses_in} {}", args.note)
        },
    }
}

/// Summed counters across every strategy that ran.
#[derive(Default)]
struct Totals {
    iterations: u64,
    processed: u64,
    generated: u64,
    fwd_subsumed: u64,
    lrs_discarded: u64,
    weight_discarded: u64,
    search_ms: u64,
}

impl Totals {
    fn add(&mut self, stats: &mrs_search::SearchStats) {
        self.iterations += stats.iterations;
        self.processed += stats.processed;
        self.generated += stats.generated;
        self.fwd_subsumed += stats.forward_subsumed;
        self.lrs_discarded += stats.lrs_discarded;
        self.weight_discarded += stats.weight_discarded;
    }
}

/// Result name, stop reason, and whether the run did the full intended work.
///
/// Only `ResourceOut(max_processed)` counts as complete: that is the
/// iteration-counted ceiling firing, which is the only stop that means "this
/// row did exactly the work the workload asked for".
fn describe_result(result: &SearchResult) -> (String, String, u8) {
    match result {
        SearchResult::ResourceOut(reason) => (
            "ResourceOut".to_string(),
            reason.as_str().to_string(),
            u8::from(reason.as_str() == "max_processed"),
        ),
        SearchResult::Refutation(..) => ("Refutation".to_string(), "refutation".to_string(), 0),
        SearchResult::Saturated(_) => ("Saturated".to_string(), "saturated".to_string(), 0),
        SearchResult::GaveUp => ("GaveUp".to_string(), "gave_up".to_string(), 0),
        SearchResult::Timeout => ("Timeout".to_string(), "time_limit".to_string(), 0),
    }
}

fn ratio(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

// ---------------------------------------------------------------------------
// bank
// ---------------------------------------------------------------------------

struct BankArgs {
    rows: PathBuf,
    tsv: PathBuf,
    md: Option<PathBuf>,
    command: String,
}

fn run_bank(args: &BankArgs) -> io::Result<()> {
    let text = fs::read_to_string(&args.rows)?;
    let mut rows: Vec<Row> = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let row = serde_json::from_str(line).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}:{}: {e}", args.rows.display(), index + 1),
            )
        })?;
        rows.push(row);
    }
    if rows.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} contains no rows", args.rows.display()),
        ));
    }

    append_tsv(&args.tsv, &rows)?;
    let bank = parse_tsv(&fs::read_to_string(&args.tsv)?, &args.tsv).map_err(io::Error::other)?;
    if let Some(md) = &args.md {
        if let Some(parent) = md.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(md, render_report(&rows, &bank, &args.command))?;
        println!("wrote {}", md.display());
    }
    println!(
        "appended {} row(s) to {} (bank now holds {})",
        rows.len(),
        args.tsv.display(),
        bank.len()
    );
    Ok(())
}

/// Appends rows, creating the bank with a header when it does not exist yet.
///
/// Refuses to touch a file whose header differs from [`header_line`]: the bank
/// is append-only evidence, and silently widening or reordering it would make
/// every archived row unreadable.
fn append_tsv(path: &Path, rows: &[Row]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let existing = fs::read_to_string(path).unwrap_or_default();
    let mut payload = String::new();
    if existing.is_empty() {
        payload.push_str(&header_line());
        payload.push('\n');
    } else {
        let header = existing.lines().next().unwrap_or_default();
        if header != header_line() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{} has a different header; the bank is append-only and must not be rewritten",
                    path.display()
                ),
            ));
        }
    }
    for row in rows {
        payload.push_str(&row.tsv_cells().join("\t"));
        payload.push('\n');
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(payload.as_bytes())
}

/// Renders the per-run Markdown report.
///
/// The report is generated, never hand-edited: a bank row that disagrees with
/// its report is a bug in this tool, not a documentation debt.
fn render_report(current: &[Row], bank: &[Row], command: &str) -> String {
    let first = &current[0];
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# MRS fixed-work perf probe — {} on {}",
        first.date, first.host_slug
    );
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "> Status: measurement record, generated by `crates/mrs-bench/perf_probe.sh`.\n\
         > Not a CASC ranking and not a solving result: this measures search\n\
         > throughput on a generated clause set. Every row below did a fixed,\n\
         > machine-independent amount of work; see [`README.md`](README.md) for the\n\
         > method and the comparability rules."
    );
    let _ = writeln!(out);

    let _ = writeln!(out, "## Provenance");
    let _ = writeln!(out);
    let _ = writeln!(out, "| Field | Value |");
    let _ = writeln!(out, "|---|---|");
    let _ = writeln!(out, "| Date | {} |", first.date);
    let _ = writeln!(out, "| Host | `{}` |", first.host_slug);
    let _ = writeln!(out, "| CPU | {} |", first.cpu_model);
    let _ = writeln!(
        out,
        "| Cores | {} physical / {} logical |",
        first.physical_cores, first.logical_cpus
    );
    let _ = writeln!(out, "| RAM | {} MB |", first.ram_total_mb);
    let _ = writeln!(out, "| Kernel | {} ({}) |", first.kernel, first.arch);
    let _ = writeln!(
        out,
        "| Commit | `{}` (worktree: {}) |",
        first.commit, first.dirty
    );
    let _ = writeln!(out, "| Toolchain | rustc {} |", first.rustc);
    let _ = writeln!(out, "| glibc | {} |", first.glibc);
    let builds = distinct(current.iter().map(|row| row.target_cpu.clone()));
    let _ = writeln!(
        out,
        "| Target CPU | {} |",
        if builds.len() == 1 {
            builds[0].clone()
        } else {
            format!("{} (see This run)", builds.join(", "))
        }
    );
    let binaries = distinct(current.iter().map(|row| row.binary_sha256.clone()));
    let _ = writeln!(
        out,
        "| Binary SHA-256 | {} |",
        if binaries.len() == 1 {
            binaries[0].clone()
        } else {
            format!(
                "{} distinct builds, one per target-cpu; per-row hashes are in `bank.tsv`",
                binaries.len()
            )
        }
    );
    let _ = writeln!(
        out,
        "| Memory budget | {} MB (hard cap: {}) |",
        first.mem_budget_mb, first.hard_cap
    );
    let _ = writeln!(
        out,
        "| Search | {} / {} / {} / avatar {} |",
        first.selection, first.ordering, first.literals, first.avatar
    );
    let _ = writeln!(out, "| Workload | `{}` |", first.workload);
    let _ = writeln!(out, "| Workload spec | `{}` |", first.workload_spec);
    let _ = writeln!(
        out,
        "| Fixed-work ceiling | {} clauses per worker |",
        first.processed_cap
    );
    let _ = writeln!(out, "| LRS iteration budget | {} |", first.lrs_budget);
    let _ = writeln!(out);

    let _ = writeln!(out, "## This run");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "| Build | Workers | Stop | Processed | Generated | Search ms | Processed/s | \
         Generated/s | Gen/proc | Work SHA | Peak RSS MB | Noise |"
    );
    let _ = writeln!(
        out,
        "|---|---:|---|---:|---:|---:|---:|---:|---:|---|---:|---:|"
    );
    for row in current {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {:.0} | {:.0} | {:.2} | `{}` | {} | {} |",
            row.target_cpu,
            row.workers,
            stop_label(row),
            row.processed,
            row.generated,
            row.search_ms,
            row.processed_per_s,
            row.generated_per_s,
            row.generated_per_processed,
            row.work_sha,
            row.peak_rss_mb,
            noise_label(row),
        );
    }
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "`Processed/s` and `Generated/s` are aggregate rates over all workers, so a\n\
         multi-worker row is total throughput, not per-core throughput. `Work SHA`\n\
         is the digest of the iteration counters: rows sharing a work SHA did\n\
         identical work, which is what makes their timings comparable. `Noise` is\n\
         the spread of the repeat runs this row is the best of: a difference between\n\
         two hosts smaller than their combined noise is not a result."
    );
    let _ = writeln!(out);

    let _ = writeln!(out, "## Scaling within this run");
    let _ = writeln!(out);
    // One baseline per build: a `haswell` worker count is scaling against the
    // `haswell` single-worker row, never against a different build's.
    let mut scaled_any = false;
    for target_cpu in distinct(current.iter().map(|row| row.target_cpu.clone())) {
        let baseline = current
            .iter()
            .find(|row| row.workers == 1 && row.complete == 1 && row.target_cpu == target_cpu);
        let Some(base) = baseline else {
            let _ = writeln!(
                out,
                "No completed single-worker row for build `{target_cpu}`, so it has no\n\
                 speed-up baseline in this run."
            );
            let _ = writeln!(out);
            continue;
        };
        let _ = writeln!(out, "**{target_cpu}**:");
        let _ = writeln!(out);
        let _ = writeln!(out, "| Workers | Processed/s | Speed-up | Efficiency |");
        let _ = writeln!(out, "|---:|---:|---:|---:|");
        for row in current
            .iter()
            .filter(|row| row.complete == 1 && row.target_cpu == target_cpu)
        {
            let speedup = row.processed_per_s / base.processed_per_s;
            let _ = writeln!(
                out,
                "| {} | {:.0} | {:.2}x | {:.0}% |",
                row.workers,
                row.processed_per_s,
                speedup,
                100.0 * speedup / row.workers as f64
            );
            scaled_any = true;
        }
        let _ = writeln!(out);
    }
    if scaled_any {
        let _ = writeln!(
            out,
            "Each worker runs the same fixed-work ceiling, so perfectly scaling workers\n\
             would show a speed-up equal to the worker count. Efficiency above 100%\n\
             means the run beat its own single-worker baseline, which is measurement\n\
             noise or turbo/boost behaviour rather than extra cores."
        );
        let _ = writeln!(out);
    }

    let _ = writeln!(out, "## Target-CPU builds on this host");
    let _ = writeln!(out);
    let builds_here = distinct(current.iter().map(|row| row.target_cpu.clone()));
    if builds_here.len() < 2 {
        let _ = writeln!(
            out,
            "Only one target-cpu build was measured (`{}`), so there is nothing to\n\
             compare on this host. Run the default `native,haswell` to see what host\n\
             tuning buys over the competition floor.",
            builds_here.first().map(String::as_str).unwrap_or("unknown")
        );
    } else {
        let _ = writeln!(
            out,
            "Identical work, identical host, two builds. The gap is what tuning for\n\
             this specific CPU buys over the fixed `haswell` floor that the CASC entry\n\
             uses. A gap inside the noise is not a result, and is reported as such."
        );
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "| Workers | Build | Processed/s | vs {} | Noise | Verdict |",
            builds_here[0]
        );
        let _ = writeln!(out, "|---:|---|---:|---:|---:|---|");
        for workers in distinct(current.iter().map(|row| row.workers)) {
            let same = |build: &str| {
                current.iter().find(|row| {
                    row.workers == workers && row.target_cpu == build && row.complete == 1
                })
            };
            let Some(reference) = same(&builds_here[0]) else {
                continue;
            };
            for build in &builds_here {
                let Some(row) = same(build) else { continue };
                let delta = (row.processed_per_s / reference.processed_per_s - 1.0) * 100.0;
                let noise = row.repeat_spread_pct + reference.repeat_spread_pct;
                let verdict = if build == &builds_here[0] {
                    "baseline".to_string()
                } else if delta.abs() <= noise {
                    format!("within noise (±{noise:.1}%)")
                } else {
                    format!("{delta:+.1}%, real")
                };
                let _ = writeln!(
                    out,
                    "| {} | {} | {:.0} | {} | ±{:.1}% | {} |",
                    row.workers,
                    row.target_cpu,
                    row.processed_per_s,
                    if build == &builds_here[0] {
                        "—".to_string()
                    } else {
                        format!("{delta:+.1}%")
                    },
                    row.repeat_spread_pct,
                    verdict,
                );
            }
        }
    }
    let _ = writeln!(out);

    let _ = writeln!(out, "## Cross-host comparison");
    let _ = writeln!(out);
    let mut ranked_any = false;
    for workload in distinct(bank.iter().map(|row| row.workload.clone())) {
        let _ = writeln!(out, "### Workload `{workload}`");
        let _ = writeln!(out);
        for target_cpu in distinct(
            bank.iter()
                .filter(|row| row.workload == workload)
                .map(|row| row.target_cpu.clone()),
        ) {
            for workers in distinct(
                bank.iter()
                    .filter(|row| row.workload == workload && row.target_cpu == target_cpu)
                    .map(|row| row.workers),
            ) {
                // Only rows that did the same work may be ranked against each
                // other, so group by work fingerprint and keep the largest group.
                let mut best: Vec<&Row> = Vec::new();
                for row in bank.iter().filter(|row| {
                    row.complete == 1
                        && row.workload == workload
                        && row.target_cpu == target_cpu
                        && row.workers == workers
                }) {
                    match best.first() {
                        Some(head) if head.work_sha == row.work_sha => best.push(row),
                        None => best.push(row),
                        _ => {}
                    }
                }
                if best.len() < 2 {
                    continue;
                }
                best.sort_by(|a, b| {
                    b.processed_per_s
                        .partial_cmp(&a.processed_per_s)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                let fastest = best[0].processed_per_s;
                let _ = writeln!(out, "**{target_cpu}**, {workers} worker(s):");
                let _ = writeln!(out);
                let _ = writeln!(out, "| Date | Host | Processed/s | Relative | Search ms |");
                let _ = writeln!(out, "|---|---|---:|---:|---:|");
                for row in &best {
                    let _ = writeln!(
                        out,
                        "| {} | `{}` | {:.0} | {:.2}x | {} |",
                        row.date,
                        row.host_slug,
                        row.processed_per_s,
                        row.processed_per_s / fastest,
                        row.search_ms,
                    );
                }
                let _ = writeln!(out);
                ranked_any = true;
            }
        }
    }
    if !ranked_any {
        let _ = writeln!(
            out,
            "No workload has two or more hosts with the same work fingerprint yet.\n\
             Run the probe on another machine and append its rows to `bank.tsv` to\n\
             get a comparison; rows are only ever ranked against identical work."
        );
        let _ = writeln!(out);
    }

    let incomplete: Vec<&Row> = bank.iter().filter(|row| row.complete != 1).collect();
    if !incomplete.is_empty() {
        let _ = writeln!(out, "## Not comparable");
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "These rows stopped before the intended fixed-work ceiling, so their\n\
             timings measure a different amount of work. They are kept because the\n\
             stop reason is diagnostic: `saturated` means the workload is too easy\n\
             and must be made harder, `time_limit` means the search got\n\
             pathologically slower, and a memory or passive-queue stop means the\n\
             budget needs revisiting."
        );
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "| Date | Host | Build | Workers | Result | Stop reason | Processed |"
        );
        let _ = writeln!(out, "|---|---|---|---:|---|---|---:|");
        for row in incomplete {
            let _ = writeln!(
                out,
                "| {} | `{}` | {} | {} | {} | {} | {} |",
                row.date,
                row.host_slug,
                row.target_cpu,
                row.workers,
                row.result,
                row.stop_reason,
                row.processed
            );
        }
        let _ = writeln!(out);
    }

    let _ = writeln!(out, "## Reproduce");
    let _ = writeln!(out);
    if command.trim().is_empty() {
        let _ = writeln!(out, "The driving command was not recorded for this run.");
    } else {
        let _ = writeln!(out, "```bash");
        let _ = writeln!(out, "{}", command.trim());
        let _ = writeln!(out, "```");
    }
    let _ = writeln!(out);
    out
}

fn stop_label(row: &Row) -> String {
    if row.complete == 1 {
        row.stop_reason.clone()
    } else {
        format!("{} ({})", row.result, row.stop_reason)
    }
}

/// How this row's timing was obtained, and the spread it carries.
fn noise_label(row: &Row) -> String {
    if row.repeat > 1 {
        format!("±{:.1}% of {}", row.repeat_spread_pct, row.repeat)
    } else {
        "single run".to_string()
    }
}

fn distinct<T: Ord>(items: impl Iterator<Item = T>) -> Vec<T> {
    let mut values: Vec<T> = items.collect();
    values.sort();
    values.dedup();
    values
}

// ---------------------------------------------------------------------------
// Date
// ---------------------------------------------------------------------------

/// Today's date in UTC as `YYYY-MM-DD`.
fn utc_date() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (year, month, day) = civil_from_days(secs.div_euclid(86_400));
    format!("{year:04}-{month:02}-{day:02}")
}

/// Days since the Unix epoch to a civil `(year, month, day)`.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { y + 1 } else { y }, month, day)
}

// ---------------------------------------------------------------------------
// Argument parsing
// ---------------------------------------------------------------------------

const USAGE: &str = "\
perf_probe — fixed-work performance probe and results bank for mrs

USAGE:
    perf_probe measure [OPTIONS]
    perf_probe bank --rows <FILE> --tsv <FILE> [--md <FILE>] [--command <TEXT>]

MEASURE OPTIONS:
    --shape <mixed|equational|relational>  calculus path to exercise (default: mixed)
    --seed <N>                              generator seed (default: 0x5EED1234ABCD0001)
    --clauses <N>                           generated clauses (default: 600)
    --funcs <N>                             function symbols (default: 8)
    --consts <N>                            constants (default: 12)
    --preds <N>                             predicate symbols (default: 16)
    --depth <N>                             max term depth (default: 4)
    --width <N>                             max literals per clause (default: 3)
    --max-term-weight <N>                   per-clause weight cap (default: 200)
    --workers <N>                           parallel searches, one per worker (default: 1)
    --processed <N>                         fixed-work ceiling per worker (default: 5000)
    --lrs-budget <N>                        LRS logical-iteration budget (default: --processed)
    --time-limit <SECS>                     safety net only, never the measurement (default: 3600)
    --max-passive <N>                       passive-queue ceiling (default: 2000000)
    --max-terms <N>                         term-bank ceiling (default: 8000000)
    --memory-budget-mb <N>                  process memory ceiling (default: 12288)
    --selection <ageweight:N|smallest|fifo> (default: ageweight:5)
    --ordering <kbo|lpo>                    (default: kbo)
    --literals <allnegative|all|maxnegative> (default: allnegative)
    --avatar <on|off>                   AVATAR clause splitting (default: on)
    --binary <FILE>                         hashed into the row as the build identity
    --commit <SHA>                          source commit
    --dirty <clean|dirty>                   worktree state
    --target-cpu <NAME>                     e.g. native, haswell
    --rustc <VERSION>                       toolchain version
    --glibc <VERSION>                       C library version
    --hard-cap <none|rlimit-as|cgroup-memory>  how the ceiling was enforced
    --repeat <N>                         runs this row is the best of (default: 1)
    --repeat-spread-pct <PCT>            spread of those repeats, noise floor of the row
    --note <TEXT>                           free text stored with the row
    --out <FILE>                            write the row here instead of stdout

BANK OPTIONS:
    --rows <FILE>                           JSONL produced by `measure`
    --tsv <FILE>                            append-only bank, header checked
    --md <FILE>                             Markdown report to (re)write
    --command <TEXT>                        driving command, recorded in the report

Run crates/mrs-bench/perf_probe.sh rather than calling this directly: the driver
builds the target-cpu variants, supplies build provenance, and enforces the
memory budget.
";

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mode = argv.first().map(String::as_str).unwrap_or("measure");
    let rest: &[String] = if argv.is_empty() { &argv } else { &argv[1..] };
    let code = match mode {
        "measure" => match parse_measure(rest) {
            Ok(args) => run_measure(&args),
            Err(message) => {
                eprintln!("{message}");
                2
            }
        },
        "bank" => match parse_bank(rest) {
            Ok(args) => match run_bank(&args) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("perf_probe bank: {e}");
                    1
                }
            },
            Err(message) => {
                eprintln!("{message}");
                2
            }
        },
        "-h" | "--help" | "help" => {
            print!("{USAGE}");
            0
        }
        other => {
            eprintln!("perf_probe: unknown mode {other}\n");
            eprint!("{USAGE}");
            2
        }
    };
    std::process::exit(code);
}

fn run_measure(args: &MeasureArgs) -> i32 {
    let row = measure(args);
    let json = serde_json::to_string(&row).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"));
    match &args.out_path {
        Some(path) => {
            if let Some(parent) = path.parent()
                && let Err(e) = fs::create_dir_all(parent)
            {
                eprintln!("perf_probe: {}: {e}", parent.display());
                return 1;
            }
            if let Err(e) = fs::write(path, format!("{json}\n")) {
                eprintln!("perf_probe: {}: {e}", path.display());
                return 1;
            }
        }
        None => println!("{json}"),
    }
    0
}

fn next_value(argv: &[String], index: &mut usize, flag: &str) -> Result<String, String> {
    *index += 1;
    argv.get(*index)
        .cloned()
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn parse_measure(argv: &[String]) -> Result<MeasureArgs, String> {
    let mut args = MeasureArgs::default();
    let mut index = 0usize;
    while index < argv.len() {
        let flag = argv[index].clone();
        match flag.as_str() {
            "--shape" => {
                let raw = next_value(argv, &mut index, &flag)?;
                args.spec.shape =
                    Shape::parse(&raw).ok_or_else(|| format!("unknown shape: {raw}"))?;
            }
            "--seed" => args.spec.seed = parse_number(&next_value(argv, &mut index, &flag)?)?,
            "--clauses" => {
                args.spec.clauses = parse_number(&next_value(argv, &mut index, &flag)?)? as usize
            }
            "--funcs" => {
                args.spec.funcs = parse_number(&next_value(argv, &mut index, &flag)?)? as usize
            }
            "--consts" => {
                args.spec.consts = parse_number(&next_value(argv, &mut index, &flag)?)? as usize
            }
            "--preds" => {
                args.spec.preds = parse_number(&next_value(argv, &mut index, &flag)?)? as usize
            }
            "--depth" => {
                args.spec.depth = parse_number(&next_value(argv, &mut index, &flag)?)? as u32
            }
            "--width" => {
                args.spec.width = parse_number(&next_value(argv, &mut index, &flag)?)? as usize
            }
            "--max-term-weight" => {
                args.spec.max_term_weight =
                    parse_number(&next_value(argv, &mut index, &flag)?)? as u32
            }
            "--workers" => {
                args.workers = parse_number(&next_value(argv, &mut index, &flag)?)?.max(1) as usize
            }
            "--processed" => {
                args.processed_cap = parse_number(&next_value(argv, &mut index, &flag)?)?.max(1)
            }
            "--lrs-budget" => {
                args.lrs_budget = Some(parse_number(&next_value(argv, &mut index, &flag)?)?.max(1))
            }
            "--time-limit" => {
                args.time_limit_s = parse_number(&next_value(argv, &mut index, &flag)?)?.max(1)
            }
            "--max-passive" => {
                args.max_passive = parse_number(&next_value(argv, &mut index, &flag)?)?.max(1)
            }
            "--max-terms" => {
                args.max_terms = parse_number(&next_value(argv, &mut index, &flag)?)?.max(1)
            }
            "--memory-budget-mb" => {
                args.mem_budget_mb = parse_number(&next_value(argv, &mut index, &flag)?)?.max(1)
            }
            "--selection" => {
                let raw = next_value(argv, &mut index, &flag)?;
                args.selection = match raw.as_str() {
                    "smallest" => SelectionStrategy::SmallestFirst,
                    "fifo" => SelectionStrategy::Fifo,
                    other => {
                        let ratio = other
                            .strip_prefix("ageweight:")
                            .ok_or_else(|| format!("unknown selection: {other}"))?;
                        SelectionStrategy::AgeWeight(
                            ratio
                                .parse()
                                .map_err(|_| format!("bad age ratio: {ratio}"))?,
                        )
                    }
                };
            }
            "--ordering" => {
                args.ordering = match next_value(argv, &mut index, &flag)?.as_str() {
                    "kbo" => TermOrdering::KBO,
                    "lpo" => TermOrdering::LPO,
                    other => return Err(format!("unknown ordering: {other}")),
                }
            }
            "--literals" => {
                args.literals = match next_value(argv, &mut index, &flag)?.as_str() {
                    "allnegative" => LiteralSelection::AllNegative,
                    "all" => LiteralSelection::All,
                    "maxnegative" => LiteralSelection::MaxNegative,
                    other => return Err(format!("unknown literal selection: {other}")),
                }
            }
            "--avatar" => {
                args.avatar = match next_value(argv, &mut index, &flag)?.as_str() {
                    "on" | "true" | "1" => true,
                    "off" | "false" | "0" => false,
                    other => return Err(format!("avatar must be on or off, not {other}")),
                }
            }
            "--binary" => args.binary = Some(PathBuf::from(next_value(argv, &mut index, &flag)?)),
            "--commit" => args.commit = next_value(argv, &mut index, &flag)?,
            "--dirty" => args.dirty = next_value(argv, &mut index, &flag)?,
            "--target-cpu" => args.target_cpu = next_value(argv, &mut index, &flag)?,
            "--rustc" => args.rustc = next_value(argv, &mut index, &flag)?,
            "--glibc" => args.glibc = next_value(argv, &mut index, &flag)?,
            "--hard-cap" => args.hard_cap = next_value(argv, &mut index, &flag)?,
            "--note" => args.note = next_value(argv, &mut index, &flag)?,
            "--repeat" => args.repeat = parse_number(&next_value(argv, &mut index, &flag)?)?.max(1),
            "--repeat-spread-pct" => {
                args.repeat_spread_pct = next_value(argv, &mut index, &flag)?
                    .parse()
                    .map_err(|_| "--repeat-spread-pct must be a number".to_string())?
            }
            "--out" => args.out_path = Some(PathBuf::from(next_value(argv, &mut index, &flag)?)),
            other => return Err(format!("unknown option: {other}\n\n{USAGE}")),
        }
        index += 1;
    }
    Ok(args)
}

fn parse_bank(argv: &[String]) -> Result<BankArgs, String> {
    let mut rows = None;
    let mut tsv = None;
    let mut md = None;
    let mut command = String::new();
    let mut index = 0usize;
    while index < argv.len() {
        let flag = argv[index].clone();
        match flag.as_str() {
            "--rows" => rows = Some(PathBuf::from(next_value(argv, &mut index, &flag)?)),
            "--tsv" => tsv = Some(PathBuf::from(next_value(argv, &mut index, &flag)?)),
            "--md" => md = Some(PathBuf::from(next_value(argv, &mut index, &flag)?)),
            "--command" => command = next_value(argv, &mut index, &flag)?,
            other => return Err(format!("unknown option: {other}\n\n{USAGE}")),
        }
        index += 1;
    }
    Ok(BankArgs {
        rows: rows.ok_or("--rows is required")?,
        tsv: tsv.ok_or("--tsv is required")?,
        md,
        command,
    })
}

/// Accepts decimal or `0x`-prefixed hexadecimal, so a seed can be written the
/// way the tool prints it.
fn parse_number(raw: &str) -> Result<u64, String> {
    let trimmed = raw.trim();
    let parsed = match trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        Some(hex) => u64::from_str_radix(hex, 16),
        None => trimmed.parse(),
    };
    parsed.map_err(|_| format!("invalid number: {raw}"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny configuration for the few tests that genuinely need a search.
    /// The debug profile that `cargo test --workspace` uses is an order of
    /// magnitude slower than release, so anything larger here would make the
    /// suite measure the machine instead of the code.
    fn tiny_args() -> MeasureArgs {
        MeasureArgs {
            spec: Spec {
                clauses: 20,
                ..Spec::default()
            },
            processed_cap: 32,
            time_limit_s: 120,
            ..MeasureArgs::default()
        }
    }

    /// A finished row built without running a search.
    ///
    /// The bank, the report renderer, and the TSV codec are pure data
    /// transformations; testing them against a real search would only make the
    /// suite slower and flakier without testing anything they own.
    fn sample_row() -> Row {
        Row {
            date: "2026-09-27".to_string(),
            host_slug: "sample-host-x4-deadbeef".to_string(),
            cpu_model: "Sample CPU @ 1.00GHz".to_string(),
            physical_cores: 4,
            logical_cpus: 8,
            ram_total_mb: 16_384,
            kernel: "6.1.0".to_string(),
            arch: "x86_64".to_string(),
            commit: "abcdef0".to_string(),
            dirty: "clean".to_string(),
            target_cpu: "native".to_string(),
            rustc: "1.98.1".to_string(),
            glibc: "2.39".to_string(),
            binary_sha256: "0".repeat(64),
            hard_cap: "rlimit-as".to_string(),
            workload: "mixed-00000000".to_string(),
            workload_spec: Spec::default().describe(),
            selection: "AgeWeight(5)".to_string(),
            ordering: "KBO".to_string(),
            literals: "AllNegative".to_string(),
            avatar: "1".to_string(),
            workers: 1,
            processed_cap: 5_000,
            lrs_budget: 5_000,
            max_passive: DEFAULT_MAX_PASSIVE,
            max_terms: DEFAULT_MAX_TERMS,
            mem_budget_mb: DEFAULT_MEMORY_BUDGET_MB,
            result: "ResourceOut".to_string(),
            stop_reason: "max_processed".to_string(),
            complete: 1,
            iterations: 5_047,
            processed: 5_047,
            generated: 123_138,
            fwd_subsumed: 2_453,
            lrs_discarded: 86_104,
            weight_discarded: 0,
            schedule_ms: 10_104,
            search_ms: 9_948,
            repeat: 3,
            repeat_spread_pct: 1.5,
            iterations_per_s: 499.0,
            processed_per_s: 499.0,
            generated_per_s: 12_175.0,
            generated_per_processed: 24.4,
            peak_rss_mb: 333,
            work_sha: "6e0ec4ba".to_string(),
            note: "clauses_in=602".to_string(),
        }
    }

    #[test]
    fn column_kinds_match_the_row_field_types() {
        let row = sample_row();
        let value = serde_json::to_value(&row).expect("Row is always serializable");
        let object = value.as_object().expect("Row serializes to an object");
        assert_eq!(
            object.len(),
            COLUMNS.len(),
            "COLUMNS and Row disagree: add the new field to COLUMNS"
        );
        for (name, kind) in COLUMNS {
            let cell = object
                .get(*name)
                .unwrap_or_else(|| panic!("Row has no field {name}"));
            let actual = match cell {
                serde_json::Value::String(_) => Kind::Text,
                serde_json::Value::Number(n) if n.is_f64() => Kind::F64,
                serde_json::Value::Number(_) => Kind::U64,
                other => panic!("column {name} has an unexpected type: {other}"),
            };
            assert_eq!(
                actual, *kind,
                "column {name} is declared {kind:?} but serializes as {actual:?}"
            );
        }
    }

    #[test]
    fn generation_is_deterministic() {
        let spec = Spec {
            clauses: 40,
            ..Spec::default()
        };
        let first = generate(&spec);
        let second = generate(&spec);
        assert_eq!(first.clauses.len(), second.clauses.len());
        for (a, b) in first.clauses.iter().zip(second.clauses.iter()) {
            assert_eq!(
                a.literals, b.literals,
                "clause generation is not reproducible"
            );
        }
        assert_eq!(first.symbols.len(), second.symbols.len());
    }

    #[test]
    fn different_seeds_give_different_clause_sets() {
        let base = Spec {
            clauses: 40,
            ..Spec::default()
        };
        let other = Spec {
            seed: base.seed.wrapping_add(1),
            ..base
        };
        let a = generate(&base);
        let b = generate(&other);
        let differs = a
            .clauses
            .iter()
            .zip(b.clauses.iter())
            .any(|(x, y)| x.literals != y.literals);
        assert!(differs, "a different seed produced an identical clause set");
    }

    #[test]
    fn workload_id_tracks_every_parameter() {
        let base = Spec::default();
        assert_eq!(base.id(), base.id(), "the workload id is not stable");
        let mutations: [fn(&mut Spec); 5] = [
            |s| s.clauses += 1,
            |s| s.depth += 1,
            |s| s.seed = s.seed.wrapping_add(1),
            |s| s.preds += 1,
            |s| s.max_term_weight += 1,
        ];
        for mutate in mutations {
            let mut changed = base;
            mutate(&mut changed);
            assert_ne!(
                base.id(),
                changed.id(),
                "the workload id ignored a parameter"
            );
        }
        let mut other_shape = base;
        other_shape.shape = Shape::Equational;
        assert_ne!(
            base.id(),
            other_shape.id(),
            "the workload id ignored the shape"
        );
    }

    #[test]
    fn prelude_axioms_are_well_formed() {
        let generated = generate(&Spec {
            clauses: 0,
            ..Spec::default()
        });
        assert_eq!(
            generated.clauses.len(),
            2,
            "expected the two prelude axioms"
        );
        for clause in &generated.clauses {
            assert_eq!(clause.len(), 1, "a prelude axiom is not a unit clause");
            let Atom::Eq(left, right) = &clause.literals[0].atom else {
                panic!("a prelude axiom is not an equality");
            };
            assert!(
                clause.literals[0].positive,
                "a prelude axiom is not positive"
            );
            assert_ne!(left, right, "a prelude axiom is a tautology");
        }
    }

    #[test]
    fn relational_shape_builds_no_function_terms() {
        let generated = generate(&Spec {
            shape: Shape::Relational,
            clauses: 30,
            ..Spec::default()
        });
        assert!(
            !generated.clauses.iter().any(|clause| {
                clause.literals.iter().any(|literal| match &literal.atom {
                    Atom::Pred(_, args) => args.iter().any(contains_function),
                    Atom::Eq(left, right) => contains_function(left) || contains_function(right),
                })
            }),
            "the relational workload built a function term"
        );
    }

    fn contains_function(term: &Term) -> bool {
        match term {
            Term::App(_, args) => args.iter().any(contains_function),
            _ => false,
        }
    }

    #[test]
    fn tsv_cells_round_trip() {
        let row = sample_row();
        let cells = row.tsv_cells();
        assert_eq!(cells.len(), COLUMNS.len());
        let borrowed: Vec<&str> = cells.iter().map(String::as_str).collect();
        let parsed = Row::from_cells(&borrowed).expect("a row parses back from its cells");
        assert_eq!(parsed.workload, row.workload);
        assert_eq!(parsed.commit, row.commit);
        assert_eq!(parsed.cpu_model, row.cpu_model);
        assert_eq!(parsed.processed, row.processed);
        assert_eq!(parsed.generated, row.generated);
        assert_eq!(parsed.processed_per_s, row.processed_per_s);
        assert_eq!(parsed.peak_rss_mb, row.peak_rss_mb);
    }

    #[test]
    fn fixed_work_run_stops_at_the_ceiling() {
        // Repeatability across processes is `perf_probe.sh`'s job to check: two
        // searches concurrent in one process are not reproducible, so asserting
        // it here would only test the test harness. What matters in-process is
        // that the run is stopped by the iteration-counted ceiling rather than
        // by the wall clock, and therefore that it is complete.
        let args = tiny_args();
        let row = measure(&args);
        assert_eq!(row.complete, 1, "the run did not stop at the ceiling");
        assert_eq!(row.stop_reason, "max_processed");
        assert_eq!(row.result, "ResourceOut");
        assert!(
            row.processed >= args.processed_cap,
            "processed {} is below the requested ceiling {}",
            row.processed,
            args.processed_cap
        );
        assert!(row.processed_per_s > 0.0, "no throughput was measured");
        assert!(row.peak_rss_mb > 0, "peak RSS was not read from VmHWM");
        assert!(
            row.peak_rss_mb <= args.mem_budget_mb,
            "peak RSS {} MB exceeded the {} MB budget",
            row.peak_rss_mb,
            args.mem_budget_mb
        );
    }

    #[test]
    fn parallel_workers_multiply_the_work() {
        let args = MeasureArgs {
            workers: 2,
            ..tiny_args()
        };
        let row = measure(&args);
        let floor = 2 * args.processed_cap;
        assert!(
            row.processed >= floor,
            "two workers processed {} clauses, expected at least {floor}",
            row.processed
        );
        assert_eq!(row.workers, 2);
        assert_eq!(row.workload, args.spec.id());
    }

    #[test]
    fn avatar_can_be_switched_off() {
        let row = measure(&MeasureArgs {
            avatar: false,
            ..tiny_args()
        });
        assert_eq!(row.avatar, "0");
        assert_eq!(row.complete, 1);
    }

    #[test]
    fn bank_refuses_a_foreign_header() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("bank.tsv");
        fs::write(&path, "wrong\theader\n1\t2\n").expect("write");
        let error =
            append_tsv(&path, &[sample_row()]).expect_err("a foreign header must be refused");
        assert!(error.to_string().contains("append-only"), "{error}");
    }

    #[test]
    fn bank_appends_a_header_then_rows() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("bank.tsv");
        let row = sample_row();
        append_tsv(&path, std::slice::from_ref(&row)).expect("first append");
        append_tsv(&path, std::slice::from_ref(&row)).expect("second append");
        let text = fs::read_to_string(&path).expect("read back");
        assert_eq!(text.lines().count(), 3, "expected a header and two rows");
        assert_eq!(text.lines().next().unwrap_or_default(), header_line());
        let parsed = parse_tsv(&text, &path).expect("parse the bank");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].work_sha, parsed[1].work_sha);
    }

    #[test]
    fn parse_tsv_rejects_a_short_row() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("bank.tsv");
        let text = format!("{}\nonly-one-cell\n", header_line());
        let error = parse_tsv(&text, &path).expect_err("a short row must be refused");
        assert!(error.contains("expected"), "{error}");
    }

    #[test]
    fn report_separates_incomplete_rows_and_records_the_command() {
        let complete = sample_row();
        let mut incomplete = complete.clone();
        incomplete.result = "Saturated".to_string();
        incomplete.stop_reason = "saturated".to_string();
        incomplete.complete = 0;
        let bank = vec![complete.clone(), incomplete.clone()];
        let report = render_report(&bank, &bank, "perf_probe.sh --all");
        assert!(report.contains("## Not comparable"), "{report}");
        assert!(report.contains("saturated"), "{report}");
        assert!(report.contains("perf_probe.sh --all"), "{report}");
        assert!(
            report.contains("No workload has two or more hosts"),
            "{report}"
        );
    }

    #[test]
    fn report_ranks_only_rows_with_identical_work() {
        let fast = sample_row();
        let mut slow = fast.clone();
        slow.host_slug = "other-host".to_string();
        slow.processed_per_s /= 2.0;
        let mut different_work = fast.clone();
        different_work.host_slug = "third-host".to_string();
        different_work.work_sha = "00000000".to_string();
        let bank = vec![fast.clone(), slow, different_work];
        let report = render_report(&bank, &bank, "cmd");
        assert!(report.contains("other-host"), "{report}");
        assert!(
            !report.contains("third-host"),
            "a row with different work was ranked: {report}"
        );
    }

    #[test]
    fn report_scales_each_build_against_its_own_baseline() {
        let mut native_single = sample_row();
        native_single.target_cpu = "native".to_string();
        let mut native_pair = native_single.clone();
        native_pair.workers = 2;
        native_pair.processed_per_s *= 1.8;
        // A haswell single-worker row that is twice as fast as the native one:
        // if it were scaled against the native baseline it would report 2.00x
        // for a single worker, which is the bug this guards against.
        let mut haswell_single = native_single.clone();
        haswell_single.target_cpu = "haswell".to_string();
        haswell_single.processed_per_s *= 2.0;
        let mut haswell_pair = haswell_single.clone();
        haswell_pair.workers = 2;
        haswell_pair.processed_per_s *= 3.6;

        let rows = vec![native_single, native_pair, haswell_single, haswell_pair];
        let report = render_report(&rows, &rows, "cmd");
        let native_section = section(&report, "**native**:");
        let haswell_section = section(&report, "**haswell**:");
        assert!(native_section.contains("1.80x"), "{native_section}");
        assert!(
            haswell_section.contains("1.00x") && haswell_section.contains("1.80x"),
            "the haswell rows were not scaled against the haswell baseline:\n{haswell_section}"
        );
    }

    #[test]
    fn cross_build_verdict_respects_the_noise_floor() {
        let base = sample_row();
        let mut native = base.clone();
        native.target_cpu = "native".to_string();
        native.repeat = 3;
        native.repeat_spread_pct = 1.0;
        // 1% faster, inside a combined 2% noise floor: not a result.
        let mut haswell = native.clone();
        haswell.target_cpu = "haswell".to_string();
        haswell.processed_per_s *= 1.01;
        let report = render_report(&[native.clone(), haswell.clone()], &[], "cmd");
        assert!(report.contains("within noise"), "{report}");

        // 30% faster, far outside the noise: a real difference.
        let mut fast = haswell;
        fast.processed_per_s *= 1.3 / 1.01;
        let report = render_report(&[native, fast], &[], "cmd");
        assert!(report.contains("real"), "{report}");
        assert!(!report.contains("within noise"), "{report}");
    }

    #[test]
    fn report_notes_a_single_build_host() {
        let row = sample_row();
        let report = render_report(std::slice::from_ref(&row), &[], "cmd");
        assert!(report.contains("Only one target-cpu build"), "{report}");
    }
    fn section<'a>(report: &'a str, start: &str) -> &'a str {
        let start = report
            .find(start)
            .unwrap_or_else(|| panic!("report has no {start} section:\n{report}"));
        let rest = &report[start..];
        let end = rest[1..]
            .find("\n#")
            .map(|index| index + 1)
            .unwrap_or(rest.len());
        &rest[..end]
    }

    #[test]
    fn memory_budget_default_is_twelve_gibibytes() {
        assert_eq!(DEFAULT_MEMORY_BUDGET_MB, 12_288);
    }

    #[test]
    fn host_slug_is_filesystem_safe() {
        let host = HostInfo {
            cpu_model: "Intel(R) Core(TM) i7-5820K CPU @ 3.30GHz".to_string(),
            physical_cores: 6,
            logical_cpus: 12,
            ram_total_mb: 15_000,
            kernel: "6.9.0".to_string(),
            arch: "x86_64".to_string(),
        };
        let slug = host.slug();
        assert!(slug.starts_with("intel-r-core-tm-i7-5820k-cpu"), "{slug}");
        assert!(slug.contains("-x6-"), "{slug}");
        assert!(
            slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'),
            "slug {slug} is not filesystem safe"
        );
    }

    #[test]
    fn civil_dates_match_known_days() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
        assert_eq!(civil_from_days(20_723), (2026, 9, 27));
    }

    #[test]
    fn splitmix_is_reproducible() {
        let stream = |seed: u64| {
            let mut rng = Rng::new(seed);
            (0..8).map(|_| rng.next_u64()).collect::<Vec<u64>>()
        };
        assert_eq!(stream(7), stream(7));
        assert_ne!(stream(7), stream(8));
        let draws = stream(7);
        assert!(draws.iter().all(|value| *value != 0));
        for pair in draws.windows(2) {
            assert_ne!(pair[0], pair[1], "the stream repeated a value");
        }
    }

    #[test]
    fn numbers_accept_hex_and_decimal() {
        assert_eq!(parse_number("42").expect("decimal"), 42);
        assert_eq!(parse_number("0x2a").expect("hex"), 42);
        assert!(parse_number("nope").is_err());
    }
}
