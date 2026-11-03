use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use clap::Args;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

type ToolResult<T> = Result<T, Box<dyn Error>>;
const STRATEGIES: [u8; 15] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
const MAX_WIDTH: usize = 8;
const MAX_EXAMPLES: usize = 3;

const CASC30_HASH: &str = "554a5782db4f6b697e421d937aef522de72e7ccb040b0127b8427a9998b0dad3";
const CASCJ13_HASH: &str = "d777b00bb437d82a518acb0f79c48d8ab027e7283296807c91d0e5c9b0d1235a";

#[derive(Args, Debug)]
pub struct ExactSoloBoundsArgs {
    #[arg(long)]
    casc30: PathBuf,
    #[arg(long)]
    cascj13: PathBuf,
    #[arg(long)]
    named_rs: Option<PathBuf>,
    #[arg(long)]
    out_prefix: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    allow_different_inputs: bool,
}

#[derive(Debug, Clone)]
struct Outcome {
    path: String,
    status: String,
    verdict: String,
    elapsed_s: f64,
    cap_s: u64,
    alias: bool,
}

impl Outcome {
    fn counts_as_refutation(&self) -> bool {
        self.verdict == "refutation" && self.elapsed_s <= self.cap_s as f64
    }
}

#[derive(Debug)]
struct Sweep {
    edition: String,
    path: PathBuf,
    sha256: String,
    raw_rows: usize,
    alias_rows_removed: usize,
    alias_problem_count: usize,
    divisions: BTreeMap<String, BTreeMap<String, BTreeMap<u8, Outcome>>>,
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("mrs-bench is two levels below the workspace root")
        .to_path_buf()
}

fn sha256_file(path: &Path) -> ToolResult<String> {
    let digest = Sha256::digest(fs::read(path)?);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn division_caps(edition: &str) -> Option<BTreeMap<&'static str, u64>> {
    match edition {
        "casc-30" => Some(BTreeMap::from([
            ("EPU", 120),
            ("FEQ", 240),
            ("FNE", 240),
            ("ICU", 240),
            ("UEQ", 240),
        ])),
        "casc-j13" => Some(BTreeMap::from([("FEQ", 180), ("FNE", 180), ("UEQ", 180)])),
        _ => None,
    }
}

fn expected_counts(edition: &str) -> BTreeMap<&'static str, usize> {
    match edition {
        "casc-30" => BTreeMap::from([
            ("EPU", 97),
            ("FEQ", 398),
            ("FNE", 97),
            ("ICU", 99),
            ("UEQ", 300),
        ]),
        "casc-j13" => BTreeMap::from([("FEQ", 299), ("FNE", 97), ("UEQ", 400)]),
        _ => BTreeMap::new(),
    }
}

fn status_verdict(status: &str) -> &'static str {
    match status {
        "Theorem" | "Unsatisfiable" => "refutation",
        "Satisfiable" | "CounterSatisfiable" => "saturation",
        _ => "unsolved",
    }
}

fn path_parts(path: &str) -> Vec<String> {
    path.replace('\\', "/")
        .split('/')
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect()
}

fn derive_division_and_alias(path: &str, edition: &str) -> ToolResult<(String, bool)> {
    let parts = path_parts(path);
    if parts.len() < 2 {
        return Err(format!("cannot derive division from path {path:?}").into());
    }
    let division = parts[parts.len() - 2].to_ascii_uppercase();
    let caps = division_caps(edition).ok_or_else(|| format!("unsupported edition {edition:?}"))?;
    if !caps.contains_key(division.as_str()) {
        if division == "EPS" {
            return Err("EPS is excluded from this refutation-portfolio analysis".into());
        }
        return Err(format!("unexpected {edition} division {division:?} in {path:?}").into());
    }
    let alias = edition == "casc-30"
        && division == "EPU"
        && parts.len() >= 3
        && parts[parts.len() - 3].eq_ignore_ascii_case("Problems");
    Ok((division, alias))
}

fn field(headers: &csv::StringRecord, name: &str, path: &Path) -> ToolResult<usize> {
    headers
        .iter()
        .position(|header| header == name)
        .ok_or_else(|| format!("{} is missing required column {name}", path.display()).into())
}

fn load_sweep(path: &Path, edition: &str) -> ToolResult<Sweep> {
    let caps = division_caps(edition).ok_or_else(|| format!("unsupported edition {edition:?}"))?;
    let mut reader = csv::Reader::from_path(path)?;
    let headers = reader.headers()?.clone();
    let columns = [
        "problem",
        "path",
        "strategy",
        "status",
        "verdict",
        "elapsed_s",
    ]
    .map(|column| field(&headers, column, path))
    .into_iter()
    .collect::<ToolResult<Vec<_>>>()?;
    let mut grouped = BTreeMap::<(String, String, u8), Vec<Outcome>>::new();
    let mut raw_rows = 0;
    for (row_index, record) in reader.records().enumerate() {
        let record = record?;
        raw_rows += 1;
        let problem = record.get(columns[0]).unwrap_or_default().trim();
        let row_path = record.get(columns[1]).unwrap_or_default().trim();
        if problem.is_empty() || row_path.is_empty() {
            return Err(format!(
                "{}:{}: empty problem or path",
                path.display(),
                row_index + 2
            )
            .into());
        }
        let normalized_path = row_path.replace('\\', "/");
        let path_stem = Path::new(&normalized_path)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default();
        if path_stem != problem {
            return Err(format!(
                "{}:{}: problem {problem:?} does not match path stem {path_stem:?}",
                path.display(),
                row_index + 2
            )
            .into());
        }
        let strategy: u8 = record
            .get(columns[2])
            .unwrap_or_default()
            .parse()
            .map_err(|_| format!("{}:{}: invalid strategy ID", path.display(), row_index + 2))?;
        if !(1..=15).contains(&strategy) {
            return Err(format!(
                "{}:{}: strategy ID must be in 1..15",
                path.display(),
                row_index + 2
            )
            .into());
        }
        let (division, alias) = derive_division_and_alias(row_path, edition)?;
        let elapsed_s: f64 = record
            .get(columns[5])
            .unwrap_or_default()
            .parse()
            .map_err(|_| format!("{}:{}: invalid elapsed_s", path.display(), row_index + 2))?;
        if !elapsed_s.is_finite() || elapsed_s < 0.0 {
            return Err(format!(
                "{}:{}: elapsed_s must be finite and non-negative",
                path.display(),
                row_index + 2
            )
            .into());
        }
        let status = record.get(columns[3]).unwrap_or_default().trim().to_owned();
        let verdict = record.get(columns[4]).unwrap_or_default().trim().to_owned();
        let expected = status_verdict(&status);
        if verdict != expected {
            return Err(format!(
                "{}:{}: status {status:?} requires verdict {expected:?}, found {verdict:?}",
                path.display(),
                row_index + 2
            )
            .into());
        }
        grouped
            .entry((division.clone(), problem.to_owned(), strategy))
            .or_default()
            .push(Outcome {
                path: row_path.to_owned(),
                status,
                verdict,
                elapsed_s,
                cap_s: caps[division.as_str()],
                alias,
            });
    }

    let mut divisions = BTreeMap::<String, BTreeMap<String, BTreeMap<u8, Outcome>>>::new();
    let mut alias_rows_removed = 0;
    let mut alias_problems = BTreeSet::new();
    for ((division, problem, strategy), outcomes) in grouped {
        let outcome = match outcomes.as_slice() {
            [only] if !only.alias => only.clone(),
            [_only] => {
                return Err(format!(
                    "{edition} EPU alias {problem} has no matching canonical EPU row"
                )
                .into());
            }
            [first, second]
                if edition == "casc-30" && division == "EPU" && first.alias != second.alias =>
            {
                let canonical = if first.alias { second } else { first };
                let alias = if first.alias { first } else { second };
                if canonical.verdict != alias.verdict
                    || canonical.counts_as_refutation() != alias.counts_as_refutation()
                {
                    return Err(format!("CASC-30 EPU aliases disagree for {problem}, strategy {strategy}: audit the duplicate inputs/outcomes before deduplicating").into());
                }
                alias_rows_removed += 1;
                alias_problems.insert(problem.clone());
                canonical.clone()
            }
            _ => {
                let paths = outcomes
                    .iter()
                    .map(|outcome| outcome.path.as_str())
                    .collect::<Vec<_>>();
                return Err(format!(
                    "duplicate matrix cell {edition}/{division}/{problem}/s{strategy}: {paths:?}"
                )
                .into());
            }
        };
        divisions
            .entry(division)
            .or_default()
            .entry(problem)
            .or_default()
            .insert(strategy, outcome);
    }
    for (division, problems) in &divisions {
        for (problem, outcomes) in problems {
            let found = outcomes.keys().copied().collect::<BTreeSet<_>>();
            let expected = STRATEGIES.into_iter().collect::<BTreeSet<_>>();
            if found != expected {
                let missing = expected.difference(&found).copied().collect::<Vec<_>>();
                let extra = found.difference(&expected).copied().collect::<Vec<_>>();
                return Err(format!("incomplete matrix for {edition}/{division}/{problem}: missing={missing:?}, extra={extra:?}").into());
            }
        }
    }
    Ok(Sweep {
        edition: edition.to_owned(),
        path: path.to_path_buf(),
        sha256: sha256_file(path)?,
        raw_rows,
        alias_rows_removed,
        alias_problem_count: alias_problems.len(),
        divisions,
    })
}

fn load_current_orders(path: &Path) -> ToolResult<BTreeMap<String, Vec<u8>>> {
    let source = fs::read_to_string(path)?;
    let constants = [
        ("EPU", "CASC_EPU_ORDER"),
        ("FEQ", "CASC_FEQ_ORDER"),
        ("FNE", "CASC_FNE_ORDER"),
        ("ICU", "CASC_ICU_ORDER"),
        ("UEQ", "CASC_UEQ_ORDER"),
    ];
    let mut result = BTreeMap::new();
    for (division, constant) in constants {
        let after = source
            .find(constant)
            .map(|index| &source[index + constant.len()..])
            .ok_or_else(|| format!("could not find {constant} in {}", path.display()))?;
        let start = after
            .find("= [")
            .map(|index| index + 3)
            .ok_or_else(|| format!("could not parse {constant} in {}", path.display()))?;
        let end = after[start..]
            .find(']')
            .map(|index| start + index)
            .ok_or_else(|| format!("could not parse {constant} in {}", path.display()))?;
        let order = after[start..end]
            .split(',')
            .filter(|item| !item.trim().is_empty())
            .map(|item| item.trim().parse::<u8>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| format!("invalid strategy ID in {constant}"))?;
        if order.len() != STRATEGIES.len()
            || order.iter().copied().collect::<BTreeSet<_>>() != STRATEGIES.into_iter().collect()
        {
            return Err(format!("{constant} must be a permutation of strategies 1..15").into());
        }
        result.insert(division.to_owned(), order);
    }
    Ok(result)
}

struct CombinationSearch<'a> {
    width: usize,
    masks: &'a [Vec<u64>],
    problem_count: usize,
    max_width: usize,
    best: usize,
    optimum_count: usize,
    examples: Vec<Vec<u8>>,
    all_optima: Vec<Vec<u8>>,
}

fn combinations(search: &mut CombinationSearch<'_>, start: u8, current: &mut Vec<u8>) {
    if current.len() == search.width {
        let mut words = vec![0u64; search.problem_count.div_ceil(64)];
        for strategy in current.iter() {
            for (word, mask) in words
                .iter_mut()
                .zip(&search.masks[(*strategy - 1) as usize])
            {
                *word |= mask;
            }
        }
        let coverage = words.iter().map(|word| word.count_ones() as usize).sum();
        if coverage > search.best {
            search.best = coverage;
            search.optimum_count = 0;
            search.examples.clear();
            search.all_optima.clear();
        }
        if coverage == search.best {
            search.optimum_count += 1;
            if search.examples.len() < MAX_EXAMPLES {
                search.examples.push(current.clone());
            }
            if search.width == search.max_width {
                search.all_optima.push(current.clone());
            }
        }
        return;
    }
    let remaining = search.width - current.len();
    if usize::from(start) + remaining - 1 > STRATEGIES.len() {
        return;
    }
    let last = (STRATEGIES.len() - remaining + 1) as u8;
    for strategy in start..=last {
        current.push(strategy);
        combinations(search, strategy + 1, current);
        current.pop();
    }
}

fn enumerate_widths(
    problems: &[String],
    solved: &BTreeMap<u8, BTreeSet<String>>,
) -> (usize, Vec<Value>) {
    let word_count = problems.len().div_ceil(64);
    let mut masks = vec![vec![0u64; word_count]; STRATEGIES.len()];
    let indices = problems
        .iter()
        .enumerate()
        .map(|(index, problem)| (problem.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let mut oracle = vec![0u64; word_count];
    for strategy in STRATEGIES {
        for problem in &solved[&strategy] {
            if let Some(index) = indices.get(problem.as_str()) {
                masks[(strategy - 1) as usize][index / 64] |= 1u64 << (index % 64);
                oracle[index / 64] |= 1u64 << (index % 64);
            }
        }
    }
    let oracle_coverage = oracle.iter().map(|word| word.count_ones() as usize).sum();
    let mut by_width = Vec::new();
    for width in 1..=MAX_WIDTH {
        let mut search = CombinationSearch {
            width,
            masks: &masks,
            problem_count: problems.len(),
            max_width: MAX_WIDTH,
            best: 0,
            optimum_count: 0,
            examples: Vec::new(),
            all_optima: Vec::new(),
        };
        combinations(&mut search, 1, &mut Vec::new());
        let mut entry = json!({
            "width": width,
            "coverage": search.best,
            "optimal_portfolio_count": search.optimum_count,
            "examples": search.examples,
        });
        if width == MAX_WIDTH {
            entry["all_optimal_portfolios"] = json!(search.all_optima);
        }
        by_width.push(entry);
    }
    (oracle_coverage, by_width)
}

fn analyze_division(
    edition: &str,
    division: &str,
    problems_by_strategy: &BTreeMap<String, BTreeMap<u8, Outcome>>,
    current_orders: &BTreeMap<String, Vec<u8>>,
) -> ToolResult<Value> {
    let problems = problems_by_strategy.keys().cloned().collect::<Vec<_>>();
    let mut solved = STRATEGIES
        .into_iter()
        .map(|strategy| (strategy, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut status_counts = BTreeMap::<String, usize>::new();
    let mut censored_refutations = 0usize;
    let mut raw_refutations = 0usize;
    for (problem, outcomes) in problems_by_strategy {
        for (strategy, outcome) in outcomes {
            *status_counts.entry(outcome.status.clone()).or_default() += 1;
            if outcome.verdict == "refutation" {
                raw_refutations += 1;
                if outcome.counts_as_refutation() {
                    solved.get_mut(strategy).unwrap().insert(problem.clone());
                } else {
                    censored_refutations += 1;
                }
            }
        }
    }
    let (union_coverage, by_width) = enumerate_widths(&problems, &solved);
    let current_order = current_orders
        .get(division)
        .ok_or_else(|| format!("no shipped strategy order for division {division}"))?;
    let current_prefix = current_order
        .iter()
        .take(MAX_WIDTH)
        .copied()
        .collect::<Vec<_>>();
    let current_coverage = current_prefix
        .iter()
        .flat_map(|strategy| solved[strategy].iter())
        .collect::<BTreeSet<_>>()
        .len();
    let best_width_coverage = by_width[MAX_WIDTH - 1]["coverage"]
        .as_u64()
        .unwrap_or_default() as usize;
    let minimum_width = if union_coverage > 0 {
        by_width
            .iter()
            .find(|entry| entry["coverage"].as_u64() == Some(union_coverage as u64))
            .and_then(|entry| entry["width"].as_u64())
    } else {
        None
    };
    let classification = if union_coverage == 0 {
        "no raw refutations observed in the solo matrix".to_owned()
    } else if minimum_width.is_some_and(|width| width < MAX_WIDTH as u64) {
        format!(
            "all-15 union is reachable with {} strategies",
            minimum_width.unwrap()
        )
    } else if best_width_coverage < union_coverage {
        format!(
            "best fixed 8-set leaves {} all-15-union instances uncovered",
            union_coverage - best_width_coverage
        )
    } else {
        "all eight slots are needed to reach the all-15 union".to_owned()
    };
    let cap = division_caps(edition).unwrap()[division];
    Ok(json!({
        "edition": edition,
        "division": division,
        "time_cap_s": cap,
        "instance_count": problems.len(),
        "raw_refutation_cells": raw_refutations,
        "censored_refutation_cells": censored_refutations,
        "within_cap_refutation_cells": raw_refutations - censored_refutations,
        "status_counts": status_counts,
        "all_15_union_coverage": union_coverage,
        "current_schedule_order": current_order,
        "current_schedule_prefix_8": current_prefix,
        "current_schedule_prefix_8_coverage": current_coverage,
        "exact_best_8_coverage": best_width_coverage,
        "exact_best_8_gap_to_union": union_coverage - best_width_coverage,
        "minimum_width_to_union": minimum_width,
        "classification": classification,
        "best_by_width": by_width,
    }))
}

fn validate_archive(sweep: &Sweep) -> ToolResult<()> {
    let expected = expected_counts(&sweep.edition);
    let actual = sweep
        .divisions
        .iter()
        .map(|(division, problems)| (division.as_str(), problems.len()))
        .collect::<BTreeMap<_, _>>();
    if actual != expected {
        return Err(format!(
            "unexpected {} instance counts: expected {expected:?}, found {actual:?}",
            sweep.edition
        )
        .into());
    }
    Ok(())
}

fn make_report(sweeps: &[Sweep], divisions: Vec<Value>, named_rs_path: &Path) -> ToolResult<Value> {
    let mut edition_summaries = Vec::new();
    for edition in ["casc-30", "casc-j13"] {
        let matching = divisions
            .iter()
            .filter(|item| item["edition"] == edition)
            .collect::<Vec<_>>();
        if matching.is_empty() {
            continue;
        }
        edition_summaries.push(json!({
            "edition": edition,
            "instance_count": matching.iter().map(|item| item["instance_count"].as_u64().unwrap_or_default()).sum::<u64>(),
            "all_15_union_coverage": matching.iter().map(|item| item["all_15_union_coverage"].as_u64().unwrap_or_default()).sum::<u64>(),
            "current_schedule_prefix_8_coverage": matching.iter().map(|item| item["current_schedule_prefix_8_coverage"].as_u64().unwrap_or_default()).sum::<u64>(),
            "exact_best_8_coverage": matching.iter().map(|item| item["exact_best_8_coverage"].as_u64().unwrap_or_default()).sum::<u64>(),
        }));
    }
    let inputs = sweeps
        .iter()
        .map(|sweep| {
            json!({
                "edition": sweep.edition,
                "file": sweep.path.file_name().and_then(|name| name.to_str()).unwrap_or_default(),
                "sha256": sweep.sha256,
                "raw_rows": sweep.raw_rows,
                "alias_rows_removed": sweep.alias_rows_removed,
                "alias_problem_count": sweep.alias_problem_count,
                "canonical_strategy_rows": sweep.raw_rows - sweep.alias_rows_removed,
                "instance_count": sweep.divisions.values().map(BTreeMap::len).sum::<usize>(),
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "method": {
            "strategy_ids": STRATEGIES,
            "max_fixed_width": MAX_WIDTH,
            "time_field": "elapsed_s",
            "counted_outcome": "verdict=refutation with elapsed_s <= official division cap",
            "scope": "exact unions of cached solo outcomes; not concurrent or proof-certified coverage",
            "eps": "excluded from portfolio optimization under AGENTS.md CASC policy",
        },
        "schedule_source": {
            "file": named_rs_path.file_name().and_then(|name| name.to_str()).unwrap_or_default(),
            "sha256": sha256_file(named_rs_path)?,
        },
        "inputs": inputs,
        "edition_summaries": edition_summaries,
        "divisions": divisions,
    }))
}

fn display_portfolios(portfolios: &[Value]) -> String {
    if portfolios.is_empty() {
        return "—".into();
    }
    portfolios
        .iter()
        .map(|portfolio| {
            let strategies = portfolio
                .as_array()
                .into_iter()
                .flatten()
                .map(|strategy| format!("s{}", strategy.as_u64().unwrap_or_default()))
                .collect::<Vec<_>>();
            format!("[{}]", strategies.join(", "))
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn render_status_counts(value: &Value) -> String {
    let Some(map) = value.as_object() else {
        return "{}".into();
    };
    let entries = map
        .iter()
        .map(|(key, value)| format!("'{}': {}", key, value.as_u64().unwrap_or_default()))
        .collect::<Vec<_>>();
    format!("{{{}}}", entries.join(", "))
}

pub fn render_markdown(report: &Value) -> String {
    let mut lines = vec![
        "# Exact solo-matrix bounds".to_owned(),
        String::new(),
        "This is an exact combinatorial analysis of the archived solo outcome matrices,".into(),
        "not a concurrent portfolio measurement and not a strict proof-certification".into(),
        "audit. Only raw `refutation` outcomes at or before each official time cap count.".into(),
        "The saved sweeps contain no saturation outcomes. Time is censored using the".into(),
        "solver `elapsed_s` field; the original sweep CSVs contain 240-second runs.".into(),
        String::new(),
        "## Inputs and validation".into(),
        String::new(),
        "| Edition | Input | Raw rows | EPU alias rows removed | Canonical rows | Instances | SHA-256 |".into(),
        "|---|---|---:|---:|---:|---:|---|".into(),
    ];
    for input in report["inputs"].as_array().into_iter().flatten() {
        lines.push(format!(
            "| {} | `{}` | {} | {} | {} | {} | `{}` |",
            input["edition"].as_str().unwrap_or_default(),
            input["file"].as_str().unwrap_or_default(),
            input["raw_rows"].as_u64().unwrap_or_default(),
            input["alias_rows_removed"].as_u64().unwrap_or_default(),
            input["canonical_strategy_rows"]
                .as_u64()
                .unwrap_or_default(),
            input["instance_count"].as_u64().unwrap_or_default(),
            input["sha256"].as_str().unwrap_or_default(),
        ));
    }
    lines.extend([
        String::new(),
        "The two EPU path aliases are deduplicated only after their per-strategy".into(),
        "verdicts and time-censored refutation outcomes agree. Same-named problems".into(),
        "in different editions remain separate. Every canonical instance must have".into(),
        "one row for each of the 15 base strategies.".into(),
        String::new(),
        "## Edition-level totals".into(),
        String::new(),
        "These sum the division-local coverage counts; they do not represent one".into(),
        "shared portfolio across divisions.".into(),
        String::new(),
        "| Edition | Instances | All-15 union | Current prefix-8 | Exact best-8 |".into(),
        "|---|---:|---:|---:|---:|".into(),
    ]);
    for summary in report["edition_summaries"].as_array().into_iter().flatten() {
        lines.push(format!(
            "| {} | {} | {} | {} | {} |",
            summary["edition"].as_str().unwrap_or_default(),
            summary["instance_count"].as_u64().unwrap_or_default(),
            summary["all_15_union_coverage"]
                .as_u64()
                .unwrap_or_default(),
            summary["current_schedule_prefix_8_coverage"]
                .as_u64()
                .unwrap_or_default(),
            summary["exact_best_8_coverage"]
                .as_u64()
                .unwrap_or_default(),
        ));
    }
    lines.extend([
        String::new(),
        "## Per-division bounds".into(),
        String::new(),
        "`Current prefix` is the first eight base-strategy IDs in the shipped named".into(),
        "schedule, projected onto these solo outcomes. It does not model the live".into(),
        "schedule's transformations, SInE behavior, sharing, or worker contention.".into(),
        String::new(),
        "| Edition | Division | Cap (s) | Instances | Raw refutation cells | Censored | All-15 union | Current prefix-8 | Exact best-8 | Best-8 gap to union | Min width to union |".into(),
        "|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|".into(),
    ]);
    let divisions = report["divisions"].as_array().cloned().unwrap_or_default();
    for result in &divisions {
        lines.push(format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            result["edition"].as_str().unwrap_or_default(),
            result["division"].as_str().unwrap_or_default(),
            result["time_cap_s"].as_u64().unwrap_or_default(),
            result["instance_count"].as_u64().unwrap_or_default(),
            result["raw_refutation_cells"].as_u64().unwrap_or_default(),
            result["censored_refutation_cells"]
                .as_u64()
                .unwrap_or_default(),
            result["all_15_union_coverage"].as_u64().unwrap_or_default(),
            result["current_schedule_prefix_8_coverage"]
                .as_u64()
                .unwrap_or_default(),
            result["exact_best_8_coverage"].as_u64().unwrap_or_default(),
            result["exact_best_8_gap_to_union"]
                .as_u64()
                .unwrap_or_default(),
            result["minimum_width_to_union"]
                .as_u64()
                .map(|value| value.to_string())
                .unwrap_or_else(|| "—".into()),
        ));
    }
    for result in &divisions {
        let order = result["current_schedule_order"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|value| value.as_u64().unwrap_or_default().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        lines.extend([
            String::new(),
            format!(
                "### {} / {}",
                result["edition"].as_str().unwrap_or_default(),
                result["division"].as_str().unwrap_or_default()
            ),
            String::new(),
            format!("- Shipped base-strategy order: `[{order}]`."),
            format!(
                "- Exact matrix classification: {}.",
                result["classification"].as_str().unwrap_or_default()
            ),
            format!(
                "- Capped refutation cells: {}; raw refutation cells beyond the cap: {}.",
                result["within_cap_refutation_cells"]
                    .as_u64()
                    .unwrap_or_default(),
                result["censored_refutation_cells"]
                    .as_u64()
                    .unwrap_or_default()
            ),
            format!(
                "- Raw status counts after EPU alias deduplication: `{}`.",
                render_status_counts(&result["status_counts"])
            ),
            String::new(),
            "| Width | Exact maximum coverage | Number of optimal sets | Example optimal set(s) |"
                .into(),
            "|---:|---:|---:|---|".into(),
        ]);
        for width in result["best_by_width"].as_array().into_iter().flatten() {
            lines.push(format!(
                "| {} | {} | {} | `{}` |",
                width["width"].as_u64().unwrap_or_default(),
                width["coverage"].as_u64().unwrap_or_default(),
                width["optimal_portfolio_count"]
                    .as_u64()
                    .unwrap_or_default(),
                display_portfolios(
                    width["examples"]
                        .as_array()
                        .map(Vec::as_slice)
                        .unwrap_or_default()
                ),
            ));
        }
    }
    lines.extend([
        String::new(),
        "## Interpretation limits".into(),
        String::new(),
        "- These bounds are exact only for the archived solo outcomes after the stated".into(),
        "  time censoring. The raw SZS outcomes are not strict proof certificates.".into(),
        "- The all-15 union is an observed-matrix ceiling, not a completeness result.".into(),
        "- An exact best 8-set is a candidate for cooperative testing, not evidence that".into(),
        "  it will retain the same coverage with eight concurrent workers.".into(),
        "- Every optimal width-8 strategy set is listed in the companion JSON artifact.".into(),
        String::new(),
        format!(
            "Shipped-order source: `{}` (SHA-256 `{}`).",
            report["schedule_source"]["file"]
                .as_str()
                .unwrap_or_default(),
            report["schedule_source"]["sha256"]
                .as_str()
                .unwrap_or_default()
        ),
        String::new(),
    ]);
    lines.join("\n")
}

pub fn exact_solo_bounds(args: ExactSoloBoundsArgs) -> ToolResult<()> {
    let root = workspace_root();
    let named_rs = args
        .named_rs
        .unwrap_or_else(|| root.join("crates/mrs-search/src/strategy/named.rs"));
    let sweeps = [
        load_sweep(&args.casc30, "casc-30")?,
        load_sweep(&args.cascj13, "casc-j13")?,
    ];
    if !args.allow_different_inputs {
        for sweep in &sweeps {
            let expected_hash = if sweep.edition == "casc-30" {
                CASC30_HASH
            } else {
                CASCJ13_HASH
            };
            if sweep.sha256 != expected_hash {
                return Err(format!("{} input hash mismatch: expected {expected_hash}, found {}; use --allow-different-inputs only for a new matrix", sweep.edition, sweep.sha256).into());
            }
            validate_archive(sweep)?;
        }
    }
    let orders = load_current_orders(&named_rs)?;
    let mut division_reports = Vec::new();
    for sweep in &sweeps {
        for (division, problems) in &sweep.divisions {
            division_reports.push(analyze_division(
                &sweep.edition,
                division,
                problems,
                &orders,
            )?);
        }
    }
    let report = make_report(&sweeps, division_reports, &named_rs)?;
    let markdown = render_markdown(&report);
    if let Some(prefix) = args.out_prefix {
        if let Some(parent) = prefix.parent() {
            fs::create_dir_all(parent)?;
        }
        let markdown_path = prefix.with_extension("md");
        let json_path = prefix.with_extension("json");
        fs::write(&markdown_path, markdown)?;
        fs::write(
            &json_path,
            format!("{}\n", serde_json::to_string_pretty(&report)?),
        )?;
        println!("wrote {}", markdown_path.display());
        println!("wrote {}", json_path.display());
    } else {
        print!("{markdown}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solved_sets() -> BTreeMap<u8, BTreeSet<String>> {
        let mut solved = STRATEGIES
            .into_iter()
            .map(|strategy| (strategy, BTreeSet::new()))
            .collect::<BTreeMap<_, _>>();
        solved.get_mut(&1).unwrap().extend(["a".into(), "b".into()]);
        solved.get_mut(&2).unwrap().extend(["c".into(), "d".into()]);
        solved.get_mut(&3).unwrap().extend(["a".into(), "c".into()]);
        solved
    }

    #[test]
    fn exact_width_enumeration_finds_bounds_and_ties() {
        let solved = solved_sets();
        let problems = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        let (oracle, widths) = enumerate_widths(&problems, &solved);
        assert_eq!(oracle, 4);
        assert_eq!(widths[0]["coverage"], 2);
        assert_eq!(widths[1]["coverage"], 4);
        assert_eq!(widths[1]["examples"][0], json!([1, 2]));
        assert_eq!(widths[7]["coverage"], 4);
        assert_eq!(widths[7]["optimal_portfolio_count"], 1716);
        assert_eq!(
            widths[7]["all_optimal_portfolios"][0],
            json!([1, 2, 3, 4, 5, 6, 7, 8])
        );
    }

    #[test]
    fn zero_union_still_enumerates_every_portfolio() {
        let solved = STRATEGIES
            .into_iter()
            .map(|strategy| (strategy, BTreeSet::new()))
            .collect();
        let (oracle, widths) = enumerate_widths(&["a".into(), "b".into()], &solved);
        assert_eq!(oracle, 0);
        assert_eq!(widths[7]["coverage"], 0);
        assert_eq!(widths[7]["optimal_portfolio_count"], 6435);
    }

    #[test]
    fn no_raw_refutations_has_no_minimum_union_width() {
        let outcomes = STRATEGIES
            .into_iter()
            .map(|strategy| {
                (
                    strategy,
                    Outcome {
                        path: "ICU/ICU001+1.p".into(),
                        status: "Timeout".into(),
                        verdict: "unsolved".into(),
                        elapsed_s: 240.0,
                        cap_s: 240,
                        alias: false,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let problems = BTreeMap::from([("ICU001+1".into(), outcomes)]);
        let orders =
            load_current_orders(&workspace_root().join("crates/mrs-search/src/strategy/named.rs"))
                .unwrap();
        let result = analyze_division("casc-30", "ICU", &problems, &orders).unwrap();
        assert!(result["minimum_width_to_union"].is_null());
        assert!(
            result["classification"]
                .as_str()
                .unwrap()
                .contains("no raw refutations")
        );
    }

    #[test]
    fn official_caps_are_inclusive() {
        assert!(
            Outcome {
                path: "FEQ/CAP001.p".into(),
                status: "Theorem".into(),
                verdict: "refutation".into(),
                elapsed_s: 180.0,
                cap_s: 180,
                alias: false
            }
            .counts_as_refutation()
        );
        assert!(
            !Outcome {
                path: "FEQ/CAP001.p".into(),
                status: "Theorem".into(),
                verdict: "refutation".into(),
                elapsed_s: 180.001,
                cap_s: 180,
                alias: false
            }
            .counts_as_refutation()
        );
    }

    #[test]
    fn division_is_derived_and_only_casc30_epu_alias_is_special() {
        assert_eq!(
            derive_division_and_alias("casc-30/EPU/ALIAS.p", "casc-30").unwrap(),
            ("EPU".into(), false)
        );
        assert_eq!(
            derive_division_and_alias("casc-30/Problems/EPU/ALIAS.p", "casc-30").unwrap(),
            ("EPU".into(), true)
        );
        assert!(derive_division_and_alias("casc-30/EPS/FOO.p", "casc-30").is_err());
    }

    #[test]
    fn shipped_orders_are_loaded_from_the_rust_source_of_truth() {
        let path = workspace_root().join("crates/mrs-search/src/strategy/named.rs");
        let orders = load_current_orders(&path).unwrap();
        assert_eq!(&orders["FEQ"][..8], &[12, 11, 10, 6, 8, 1, 4, 13]);
        assert_eq!(&orders["ICU"][..3], &[12, 1, 2]);
        for order in orders.values() {
            assert_eq!(
                order.iter().copied().collect::<BTreeSet<_>>(),
                STRATEGIES.into_iter().collect()
            );
        }
    }

    fn sweep_row(
        problem: &str,
        path: &str,
        strategy: u8,
        status: &str,
        elapsed: f64,
    ) -> [String; 9] {
        [
            problem.into(),
            path.into(),
            strategy.to_string(),
            "casc_feq".into(),
            status.into(),
            status_verdict(status).into(),
            elapsed.to_string(),
            elapsed.to_string(),
            String::new(),
        ]
    }

    fn write_sweep(rows: &[[String; 9]]) -> tempfile::NamedTempFile {
        let file = tempfile::NamedTempFile::new().unwrap();
        let mut writer = csv::Writer::from_path(file.path()).unwrap();
        writer
            .write_record([
                "problem",
                "path",
                "strategy",
                "schedule",
                "status",
                "verdict",
                "elapsed_s",
                "wall_s",
                "detail",
            ])
            .unwrap();
        for row in rows {
            writer.write_record(row).unwrap();
        }
        writer.flush().unwrap();
        file
    }

    #[test]
    fn sweep_loader_applies_j13_cap_and_requires_full_strategy_matrix() {
        let rows = (1..=15)
            .map(|strategy| {
                sweep_row(
                    "CAP001+1",
                    "FEQ/CAP001+1.p",
                    strategy,
                    "Theorem",
                    if strategy == 1 { 180.0 } else { 180.001 },
                )
            })
            .collect::<Vec<_>>();
        let file = write_sweep(&rows);
        let sweep = load_sweep(file.path(), "casc-j13").unwrap();
        let outcomes = &sweep.divisions["FEQ"]["CAP001+1"];
        assert!(outcomes[&1].counts_as_refutation());
        assert!(!outcomes[&2].counts_as_refutation());

        let incomplete = write_sweep(&rows[..14]);
        assert!(
            load_sweep(incomplete.path(), "casc-j13")
                .unwrap_err()
                .to_string()
                .contains("incomplete matrix")
        );
    }

    #[test]
    fn epu_aliases_are_removed_only_when_capped_outcomes_agree() {
        let mut rows = Vec::new();
        for strategy in 1..=15 {
            let status = if strategy == 1 {
                "Unsatisfiable"
            } else {
                "Timeout"
            };
            let elapsed = if strategy == 1 { 120.0 } else { 240.0 };
            rows.push(sweep_row(
                "ALIAS001-1",
                "EPU/ALIAS001-1.p",
                strategy,
                status,
                elapsed,
            ));
            rows.push(sweep_row(
                "ALIAS001-1",
                "Problems/EPU/ALIAS001-1.p",
                strategy,
                status,
                elapsed,
            ));
        }
        let file = write_sweep(&rows);
        let sweep = load_sweep(file.path(), "casc-30").unwrap();
        assert_eq!(sweep.raw_rows, 30);
        assert_eq!(sweep.alias_rows_removed, 15);
        assert_eq!(sweep.alias_problem_count, 1);

        let mut disagree = rows;
        disagree[1] = sweep_row(
            "ALIAS001-1",
            "Problems/EPU/ALIAS001-1.p",
            1,
            "Timeout",
            240.0,
        );
        let file = write_sweep(&disagree);
        assert!(
            load_sweep(file.path(), "casc-30")
                .unwrap_err()
                .to_string()
                .contains("aliases disagree")
        );
    }
}
