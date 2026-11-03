use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use clap::Args;
use serde_json::{Value, json};

type ToolResult<T> = Result<T, Box<dyn Error>>;
const STRATEGIES: [u8; 15] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];

#[derive(Args, Debug)]
pub struct AnalyzeArgs {
    /// CSV emitted by prephase_dump.
    features: PathBuf,
    /// CSV emitted by prephase_sweep. Omit for a feature-only taxonomy.
    labels: Option<PathBuf>,
    #[arg(long, value_delimiter = ',')]
    divisions: Vec<String>,
    #[arg(long, default_value_t = false)]
    emit_rules: bool,
    #[arg(long, default_value_t = false)]
    features_only: bool,
    /// Optional machine-readable report output.
    #[arg(long)]
    json_out: Option<PathBuf>,
    /// Rust source containing the shipped named strategy orders.
    #[arg(long)]
    named_rs: Option<PathBuf>,
}

type FeatureRow = BTreeMap<String, String>;
type Features = BTreeMap<String, FeatureRow>;
type Solved = BTreeMap<u8, BTreeSet<String>>;
type Labels = BTreeMap<String, BTreeMap<u8, LabelRow>>;
type GreedyTrace = Vec<(u8, usize, usize)>;

#[derive(Debug, Clone)]
struct LabelRow {
    verdict: String,
    elapsed_s: f64,
}

fn csv_index(headers: &csv::StringRecord, name: &str, path: &Path) -> ToolResult<usize> {
    headers
        .iter()
        .position(|header| header == name)
        .ok_or_else(|| format!("{} is missing required column {name}", path.display()).into())
}

fn load_features(path: &Path) -> ToolResult<(Features, BTreeMap<String, usize>, usize)> {
    let mut reader = csv::Reader::from_path(path)?;
    let headers = reader.headers()?.clone();
    let name = csv_index(&headers, "name", path)?;
    let status = csv_index(&headers, "parse_status", path)?;
    let mut features = BTreeMap::new();
    let mut statuses = BTreeMap::<String, usize>::new();
    let mut duplicates_overwritten = 0usize;
    for (row_index, record) in reader.records().enumerate() {
        let record = record?;
        let status_value = record.get(status).unwrap_or_default().to_owned();
        *statuses.entry(status_value.clone()).or_default() += 1;
        if !matches!(status_value.as_str(), "ok" | "empty_after_lowering") {
            continue;
        }
        let problem = record.get(name).unwrap_or_default().to_owned();
        if problem.is_empty() {
            return Err(format!(
                "{}:{}: empty feature problem name",
                path.display(),
                row_index + 2
            )
            .into());
        }
        let row = headers
            .iter()
            .zip(record.iter())
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect::<BTreeMap<_, _>>();
        if features.insert(problem, row).is_some() {
            // TPTP releases sometimes expose the same problem through both a
            // canonical division directory and a `Problems/<division>` alias.
            // Keep the last sorted dump row, matching the prior study contract,
            // and make the collapse visible in the report.
            duplicates_overwritten += 1;
        }
    }
    Ok((features, statuses, duplicates_overwritten))
}

fn load_labels(path: &Path) -> ToolResult<(Labels, usize)> {
    let mut reader = csv::Reader::from_path(path)?;
    let headers = reader.headers()?.clone();
    let problem = csv_index(&headers, "problem", path)?;
    let strategy = csv_index(&headers, "strategy", path)?;
    let verdict = csv_index(&headers, "verdict", path)?;
    let elapsed = csv_index(&headers, "elapsed_s", path)?;
    let mut labels = BTreeMap::<String, BTreeMap<u8, LabelRow>>::new();
    let mut duplicates_overwritten = 0usize;
    for (row_index, record) in reader.records().enumerate() {
        let record = record?;
        let name = record.get(problem).unwrap_or_default().to_owned();
        if name.is_empty() {
            return Err(format!("{}:{}: empty problem name", path.display(), row_index + 2).into());
        }
        let strategy: u8 = record
            .get(strategy)
            .unwrap_or_default()
            .parse()
            .map_err(|_| format!("{}:{}: invalid strategy ID", path.display(), row_index + 2))?;
        let elapsed_s = record
            .get(elapsed)
            .unwrap_or_default()
            .parse()
            .unwrap_or(f64::INFINITY);
        let row = LabelRow {
            verdict: record.get(verdict).unwrap_or_default().to_owned(),
            elapsed_s,
        };
        if labels
            .entry(name)
            .or_default()
            .insert(strategy, row)
            .is_some()
        {
            duplicates_overwritten += 1;
        }
    }
    Ok((labels, duplicates_overwritten))
}

fn division_of(path: &str) -> &str {
    let parts = path
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    parts
        .get(parts.len().saturating_sub(2))
        .copied()
        .unwrap_or("?")
}

fn greedy_set_cover(
    universe: &BTreeSet<String>,
    solved: &Solved,
) -> (Vec<u8>, GreedyTrace, BTreeSet<String>) {
    let mut remaining = universe.clone();
    let mut chosen = Vec::new();
    let mut trace = Vec::new();
    loop {
        let mut best = None;
        let mut gain = 0;
        for (strategy, members) in solved {
            if chosen.contains(strategy) {
                continue;
            }
            let covered = members.intersection(&remaining).count();
            if covered > gain {
                best = Some(*strategy);
                gain = covered;
            }
        }
        let Some(strategy) = best else { break };
        if gain == 0 {
            break;
        }
        chosen.push(strategy);
        remaining.retain(|problem| !solved[&strategy].contains(problem));
        trace.push((strategy, gain, remaining.len()));
    }
    (chosen, trace, remaining)
}

pub fn fit_orders(
    features: &Features,
    fit_problems: &[String],
    strategies: &[u8],
    solved: &Solved,
) -> BTreeMap<String, Vec<u8>> {
    let mut by_label = BTreeMap::<String, BTreeSet<String>>::new();
    for problem in fit_problems {
        if let Some(feature) = features.get(problem) {
            by_label
                .entry(feature.get("label").cloned().unwrap_or_default())
                .or_default()
                .insert(problem.clone());
        }
    }
    by_label
        .into_iter()
        .map(|(label, members)| {
            let local = strategies
                .iter()
                .map(|strategy| {
                    (
                        *strategy,
                        solved[strategy]
                            .intersection(&members)
                            .cloned()
                            .collect::<BTreeSet<_>>(),
                    )
                })
                .collect::<Solved>();
            let (chosen, _, _) = greedy_set_cover(&members, &local);
            let order = chosen
                .iter()
                .copied()
                .chain(
                    strategies
                        .iter()
                        .copied()
                        .filter(|strategy| !chosen.contains(strategy)),
                )
                .collect();
            (label, order)
        })
        .collect()
}

pub fn best_fixed_portfolio(
    problems: &[String],
    solved: &Solved,
    strategies: &[u8],
    workers: usize,
) -> (Vec<u8>, usize, GreedyTrace, BTreeSet<String>) {
    let universe = problems.iter().cloned().collect::<BTreeSet<_>>();
    let local = strategies
        .iter()
        .map(|strategy| {
            (
                *strategy,
                solved[strategy]
                    .intersection(&universe)
                    .cloned()
                    .collect::<BTreeSet<_>>(),
            )
        })
        .collect::<Solved>();
    let (chosen, trace, _) = greedy_set_cover(&universe, &local);
    let selected = chosen.into_iter().take(workers).collect::<Vec<_>>();
    let covered = selected
        .iter()
        .flat_map(|strategy| local[strategy].iter().cloned())
        .collect::<BTreeSet<_>>();
    let remaining = universe.difference(&covered).cloned().collect();
    let trace = trace.into_iter().take(workers).collect();
    (selected, covered.len(), trace, remaining)
}

fn best_strategy_for(members: &BTreeSet<String>, solved: &Solved, strategies: &[u8]) -> Option<u8> {
    strategies
        .iter()
        .copied()
        .fold(None, |best, strategy| match best {
            None => Some(strategy),
            Some(current)
                if solved[&strategy].intersection(members).count()
                    > solved[&current].intersection(members).count() =>
            {
                Some(strategy)
            }
            Some(current) => Some(current),
        })
}

const NUMERIC_BUCKETS: &[(&str, &[(f64, &str)])] = &[
    (
        "log_n_clauses",
        &[
            (2.0, "<100"),
            (3.0, "100-999"),
            (4.0, "1k-10k"),
            (6.0, ">10k"),
        ],
    ),
    (
        "unit_ratio",
        &[
            (0.25, "<.25"),
            (0.75, ".25-.75"),
            (0.999, ".75-1"),
            (2.0, "=1"),
        ],
    ),
    (
        "horn_ratio",
        &[(0.5, "<.5"), (0.9, ".5-.9"), (0.999, ".9-1"), (2.0, "=1")],
    ),
    (
        "avg_clause_len",
        &[
            (1.5, "1-2"),
            (2.5, "2-3"),
            (4.0, "3-4"),
            (9.0, "4-9"),
            (99.0, ">=9"),
        ],
    ),
    (
        "goal_reachable_ratio",
        &[(0.25, "<.25"), (0.8, ".25-.8"), (1.01, ">=.8")],
    ),
    ("n_components", &[(2.0, "1"), (4.0, "2-3"), (1.0e9, "4+")]),
    (
        "max_term_depth",
        &[(3.0, "1-2"), (6.0, "3-5"), (11.0, "6-10"), (1.0e9, ">10")],
    ),
    (
        "n_skolems",
        &[
            (1.0, "0"),
            (11.0, "1-10"),
            (101.0, "11-100"),
            (1.0e9, ">100"),
        ],
    ),
    (
        "max_fun_arity",
        &[(1.0, "0"), (2.0, "1"), (3.0, "2"), (1.0e9, "3+")],
    ),
    (
        "redundant_ratio",
        &[(0.001, "0"), (0.1, "<=.1"), (0.4, "<=.4"), (1.01, ">.4")],
    ),
    (
        "negative_literal_ratio",
        &[
            (0.34, "<.34"),
            (0.5, ".34-.5"),
            (0.67, ".5-.67"),
            (1.01, ">.67"),
        ],
    ),
    (
        "symbol_concentration",
        &[
            (0.3, "<.3"),
            (0.6, ".3-.6"),
            (0.85, ".6-.85"),
            (1.01, ">.85"),
        ],
    ),
    (
        "ground_ratio",
        &[(0.01, "0"), (0.5, "<.5"), (0.99, "<1"), (1.01, "=1")],
    ),
    (
        "fvo_ratio",
        &[(0.01, "0"), (0.5, "<.5"), (0.99, "<1"), (1.01, "=1")],
    ),
    (
        "avg_vars_per_clause",
        &[
            (0.5, "0"),
            (1.5, "0.5-1.5"),
            (3.5, "1.5-3.5"),
            (1.0e9, ">3.5"),
        ],
    ),
    (
        "n_predicates",
        &[(5.0, "<5"), (20.0, "5-20"), (80.0, "20-80"), (1.0e9, ">80")],
    ),
    (
        "abstraction_trivial_clause_ratio",
        &[(0.01, "0"), (0.5, "<.5"), (0.99, "<1"), (1.01, "=1")],
    ),
    (
        "n_goal_clauses",
        &[(1.0, "0"), (3.0, "1-2"), (10.0, "3-9"), (1.0e9, ">=10")],
    ),
    ("n_input_cnf_clauses", &[(1.0, "0"), (1.0e9, ">=1")]),
    (
        "largest_component_ratio",
        &[(0.5, "<.5"), (0.9, ".5-.9"), (1.01, ">=.9")],
    ),
    (
        "avg_term_depth",
        &[(1.5, "<1.5"), (3.0, "1.5-3"), (1.0e9, ">=3")],
    ),
    (
        "nonlinear_ratio",
        &[(0.01, "0"), (0.3, "<.3"), (0.8, "<.8"), (1.01, ">=.8")],
    ),
    (
        "rewrite_rule_ratio",
        &[(0.01, "0"), (0.3, "<.3"), (0.8, "<.8"), (1.01, ">=.8")],
    ),
    (
        "n_literals",
        &[
            (50.0, "<50"),
            (500.0, "50-500"),
            (5000.0, "500-5k"),
            (1.0e9, ">=5k"),
        ],
    ),
    (
        "probe_generation_rate",
        &[
            (0.001, "0"),
            (0.5, "<=.5"),
            (2.0, ".5-2"),
            (6.0, "2-6"),
            (1.0e9, ">6"),
        ],
    ),
    (
        "probe_redundancy_rate",
        &[(0.001, "0"), (0.25, "<=.25"), (0.6, "<=.6"), (1.01, ">.6")],
    ),
    (
        "probe_forward_subsumed",
        &[
            (1.0, "0"),
            (50.0, "1-50"),
            (500.0, "51-500"),
            (1.0e9, ">500"),
        ],
    ),
    (
        "probe_processed",
        &[
            (1.0, "0"),
            (100.0, "1-100"),
            (1000.0, "101-1000"),
            (2001.0, "1001-2000"),
        ],
    ),
    (
        "probe_passive",
        &[
            (1.0, "0"),
            (100.0, "1-100"),
            (1000.0, "101-1000"),
            (1.0e9, ">1000"),
        ],
    ),
    (
        "probe_throughput_per_ms",
        &[
            (0.001, "0"),
            (10.0, "<10"),
            (100.0, "10-100"),
            (1.0e9, ">=100"),
        ],
    ),
];

fn numeric_feature(row: &FeatureRow, field: &str) -> Option<f64> {
    row.get(field)
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite())
}

fn probe_saturated(row: &FeatureRow) -> Option<bool> {
    Some(
        numeric_feature(row, "probe_processed")? < 2000.0
            && numeric_feature(row, "probe_elapsed_ms")? < 1000.0,
    )
}

fn conditional_coverage(
    features: &Features,
    problems: &[String],
    strategies: &[u8],
    solved: &Solved,
) -> Vec<Value> {
    let mut groups = BTreeMap::<(String, String), BTreeSet<String>>::new();
    for (field, edges) in NUMERIC_BUCKETS {
        for problem in problems {
            let value = numeric_feature(&features[problem], field).unwrap_or(0.0);
            let bucket = edges
                .iter()
                .find(|(edge, _)| value < *edge)
                .map(|(_, label)| *label)
                .unwrap_or(edges.last().map(|(_, label)| *label).unwrap_or("?"));
            groups
                .entry(((*field).to_owned(), bucket.to_owned()))
                .or_default()
                .insert(problem.clone());
        }
    }
    for field in [
        "logic_class",
        "shape_class",
        "scale_class",
        "goal_class",
        "decomposition_class",
        "dialect",
    ] {
        for problem in problems {
            if let Some(value) = features[problem].get(field) {
                groups
                    .entry((field.to_owned(), value.to_owned()))
                    .or_default()
                    .insert(problem.clone());
            }
        }
    }
    for (bucket, saturated) in [("saturated", true), ("budget-bound", false)] {
        let members = problems
            .iter()
            .filter(|problem| probe_saturated(&features[*problem]) == Some(saturated))
            .cloned()
            .collect::<BTreeSet<_>>();
        if !members.is_empty() {
            groups.insert(("probe_saturation".into(), bucket.into()), members);
        }
    }
    let all_solved = solved
        .values()
        .flat_map(|members| members.iter().cloned())
        .collect::<BTreeSet<_>>();
    groups
        .into_iter()
        .filter(|(_, members)| members.len() >= 8)
        .map(|((field, bucket), members)| {
            let mut ranking = strategies
                .iter()
                .map(|strategy| (*strategy, solved[strategy].intersection(&members).count()))
                .collect::<Vec<_>>();
            ranking.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
            json!({
                "feature": field,
                "bucket": bucket,
                "problems": members.len(),
                "union_coverage": all_solved.intersection(&members).count(),
                "best_strategies": ranking.into_iter().filter(|(_, count)| *count > 0).take(4).map(|(strategy, coverage)| json!({"strategy": strategy, "coverage": coverage})).collect::<Vec<_>>(),
            })
        })
        .collect()
}

fn best_feasibility_split(
    features: &Features,
    problems: &[String],
    solved: &Solved,
) -> Option<Value> {
    let union = solved
        .values()
        .flat_map(|members| members.iter().cloned())
        .collect::<BTreeSet<_>>();
    let mut best: Option<(f64, String, f64, usize, f64, usize, f64)> = None;
    for (field, edges) in NUMERIC_BUCKETS {
        for (threshold, _) in edges.iter().take(edges.len().saturating_sub(1)) {
            let low = problems
                .iter()
                .filter(|problem| {
                    numeric_feature(&features[*problem], field).unwrap_or(0.0) < *threshold
                })
                .cloned()
                .collect::<BTreeSet<_>>();
            let high = problems
                .iter()
                .filter(|problem| {
                    numeric_feature(&features[*problem], field).unwrap_or(0.0) >= *threshold
                })
                .cloned()
                .collect::<BTreeSet<_>>();
            if low.len() < 4 || high.len() < 4 {
                continue;
            }
            let low_rate = union.intersection(&low).count() as f64 / low.len() as f64;
            let high_rate = union.intersection(&high).count() as f64 / high.len() as f64;
            let gain = (low_rate - high_rate).abs();
            if best.as_ref().is_none_or(|current| gain > current.0) {
                best = Some((
                    gain,
                    (*field).to_owned(),
                    *threshold,
                    low.len(),
                    low_rate,
                    high.len(),
                    high_rate,
                ));
            }
        }
    }
    let (lift, feature, threshold, below_count, below_rate, at_or_above_count, at_or_above_rate) =
        best?;
    let low_side = problems
        .iter()
        .filter(|problem| numeric_feature(&features[*problem], &feature).unwrap_or(0.0) < threshold)
        .cloned()
        .collect::<Vec<_>>();
    let mut second_splits = Vec::new();
    for (candidate, edges) in NUMERIC_BUCKETS {
        if *candidate == feature {
            continue;
        }
        let mut candidate_best: Option<(f64, f64, usize, f64, usize, f64)> = None;
        for (candidate_threshold, _) in edges.iter().take(edges.len().saturating_sub(1)) {
            let low = low_side
                .iter()
                .filter(|problem| {
                    numeric_feature(&features[*problem], candidate).unwrap_or(0.0)
                        < *candidate_threshold
                })
                .collect::<Vec<_>>();
            let high = low_side
                .iter()
                .filter(|problem| {
                    numeric_feature(&features[*problem], candidate).unwrap_or(0.0)
                        >= *candidate_threshold
                })
                .collect::<Vec<_>>();
            if low.len() < 4 || high.len() < 4 {
                continue;
            }
            let low_rate = low
                .iter()
                .filter(|problem| union.contains(**problem))
                .count() as f64
                / low.len() as f64;
            let high_rate = high
                .iter()
                .filter(|problem| union.contains(**problem))
                .count() as f64
                / high.len() as f64;
            let gain = (low_rate - high_rate).abs();
            if candidate_best
                .as_ref()
                .is_none_or(|current| gain > current.0)
            {
                candidate_best = Some((
                    gain,
                    *candidate_threshold,
                    low.len(),
                    low_rate,
                    high.len(),
                    high_rate,
                ));
            }
        }
        if let Some((gain, split, low_count, low_rate, high_count, high_rate)) = candidate_best
            && gain >= 0.2
        {
            second_splits.push(json!({
                "feature": candidate,
                "threshold": split,
                "lift": gain,
                "below_count": low_count,
                "below_solved_rate": low_rate,
                "at_or_above_count": high_count,
                "at_or_above_solved_rate": high_rate,
            }));
        }
    }
    Some(json!({
        "feature": feature,
        "threshold": threshold,
        "lift": lift,
        "below_count": below_count,
        "below_solved_rate": below_rate,
        "at_or_above_count": at_or_above_count,
        "at_or_above_solved_rate": at_or_above_rate,
        "second_splits_with_lift_at_least_0_2": second_splits,
    }))
}

fn behavioral_taxonomy(
    features: &Features,
    problems: &[String],
    strategies: &[u8],
    solved: &Solved,
    elapsed: &BTreeMap<String, BTreeMap<u8, f64>>,
) -> Value {
    let mut histogram = BTreeMap::<String, usize>::new();
    let mut signatures = BTreeMap::<Vec<u8>, Vec<String>>::new();
    let mut fastest = BTreeMap::<String, usize>::new();
    for problem in problems {
        let signature = strategies
            .iter()
            .filter(|strategy| solved[strategy].contains(problem))
            .copied()
            .collect::<Vec<_>>();
        let bucket = match signature.len() {
            0 => "0",
            1 => "1",
            2..=3 => "2-3",
            4..=7 => "4-7",
            _ => "8-15",
        };
        *histogram.entry(bucket.to_owned()).or_default() += 1;
        signatures
            .entry(signature.clone())
            .or_default()
            .push(problem.clone());
        if let Some(winner) = signature.iter().min_by(|left, right| {
            elapsed
                .get(problem)
                .and_then(|row| row.get(left))
                .copied()
                .unwrap_or(f64::INFINITY)
                .total_cmp(
                    &elapsed
                        .get(problem)
                        .and_then(|row| row.get(right))
                        .copied()
                        .unwrap_or(f64::INFINITY),
                )
                .then(left.cmp(right))
        }) {
            *fastest.entry(format!("s{winner}")).or_default() += 1;
        } else {
            *fastest.entry("none".into()).or_default() += 1;
        }
    }
    let distinct_signatures = signatures.len();
    let mut pairs = Vec::new();
    for (index, left) in strategies.iter().enumerate() {
        for right in strategies.iter().skip(index + 1) {
            let only_left = solved[left].difference(&solved[right]).count();
            let only_right = solved[right].difference(&solved[left]).count();
            let both = solved[left].intersection(&solved[right]).count();
            pairs.push((
                only_left + only_right,
                *left,
                *right,
                only_left,
                only_right,
                both,
            ));
        }
    }
    pairs.sort_by(|left, right| right.cmp(left));
    let top_pairs = pairs.into_iter().take(8).map(|(exclusive, left, right, only_left, only_right, both)| json!({"left": left, "right": right, "exclusive_total": exclusive, "only_left": only_left, "only_right": only_right, "both": both})).collect::<Vec<_>>();
    let mut signature_by_label = BTreeMap::<String, BTreeMap<Vec<u8>, usize>>::new();
    for (signature, members) in &signatures {
        for problem in members {
            let label = features[problem].get("label").cloned().unwrap_or_default();
            *signature_by_label
                .entry(label)
                .or_default()
                .entry(signature.clone())
                .or_default() += 1;
        }
    }
    let mut label_purity = Vec::new();
    let mut modal_total = 0usize;
    let mut considered_total = 0usize;
    for (label, counts) in signature_by_label {
        let total = counts.values().sum::<usize>();
        if total < 4 {
            continue;
        }
        let modal = counts.values().copied().max().unwrap_or_default();
        let modal_signature = counts
            .iter()
            .filter(|(_, count)| **count == modal)
            .map(|(signature, _)| signature)
            .min()
            .cloned()
            .unwrap_or_default();
        modal_total += modal;
        considered_total += total;
        label_purity.push(json!({"label": label, "problems": total, "behavior_sets": counts.len(), "modal_count": modal, "modal_signature": modal_signature, "modal_purity": modal as f64 / total as f64}));
    }
    label_purity.sort_by(|left, right| right["problems"].as_u64().cmp(&left["problems"].as_u64()));
    let mut largest_signatures = signatures.into_iter().filter(|(_, members)| members.len() >= 4).map(|(signature, members)| {
        let mut labels = BTreeMap::<String, usize>::new();
        for problem in &members { *labels.entry(features[problem].get("label").cloned().unwrap_or_default()).or_default() += 1; }
        let mut labels = labels.into_iter().collect::<Vec<_>>();
        labels.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
        json!({"signature": signature, "problems": members.len(), "top_labels": labels.into_iter().take(2).map(|(label,count)| json!({"label":label,"count":count})).collect::<Vec<_>>()})
    }).collect::<Vec<_>>();
    largest_signatures
        .sort_by(|left, right| right["problems"].as_u64().cmp(&left["problems"].as_u64()));
    largest_signatures.truncate(10);
    json!({"solved_configuration_histogram": histogram, "exclusive_strategy_pairs": top_pairs, "fastest_strategy_per_problem": fastest, "distinct_solver_sets": distinct_signatures, "largest_solver_sets": largest_signatures, "label_behavior_purity": label_purity, "weighted_modal_purity": if considered_total == 0 { Value::Null } else { json!(modal_total as f64 / considered_total as f64) }})
}

fn load_named_orders(path: &Path) -> ToolResult<BTreeMap<String, Vec<u8>>> {
    let source = fs::read_to_string(path)?;
    let mut orders = BTreeMap::from([("casc".to_owned(), STRATEGIES.to_vec())]);
    // Load the shipped named baseline from its Rust source of truth. Divisions
    // without an explicit order here use the generic `casc` schedule.
    for (division, constant) in [
        ("feq", "CASC_FEQ_ORDER"),
        ("fne", "CASC_FNE_ORDER"),
        ("ueq", "CASC_UEQ_ORDER"),
        ("epr", "CASC_EPR_ORDER"),
        ("eps", "CASC_EPS_ORDER"),
        ("epu", "CASC_EPU_ORDER"),
        ("icu", "CASC_ICU_ORDER"),
    ] {
        let tail = source
            .find(constant)
            .map(|offset| &source[offset + constant.len()..])
            .ok_or_else(|| format!("missing {constant} in {}", path.display()))?;
        let start = tail
            .find("= [")
            .map(|index| index + 3)
            .ok_or_else(|| format!("invalid {constant}"))?;
        let end = tail[start..]
            .find(']')
            .map(|offset| start + offset)
            .ok_or_else(|| format!("invalid {constant}"))?;
        let order = tail[start..end]
            .split(',')
            .filter(|value| !value.trim().is_empty())
            .map(|value| value.trim().parse::<u8>())
            .collect::<Result<Vec<_>, _>>()?;
        if order.len() == 15 {
            orders.insert(format!("casc_{division}"), order);
        }
    }
    Ok(orders)
}

fn portfolio_coverage(problems: &[String], order: &[u8], workers: usize, solved: &Solved) -> usize {
    let chosen = order.iter().take(workers).collect::<BTreeSet<_>>();
    problems
        .iter()
        .filter(|problem| {
            chosen
                .iter()
                .any(|strategy| solved[strategy].contains(*problem))
        })
        .count()
}

struct AnalysisInput<'a> {
    features: &'a Features,
    statuses: &'a BTreeMap<String, usize>,
    labels: &'a Labels,
    feature_duplicates: usize,
    label_duplicates: usize,
    selected_problems: &'a [String],
    divisions: &'a [String],
    named_orders: &'a BTreeMap<String, Vec<u8>>,
    emit_rules: bool,
}

fn build_report(input: AnalysisInput<'_>) -> Value {
    let AnalysisInput {
        features,
        statuses,
        labels,
        feature_duplicates,
        label_duplicates,
        selected_problems,
        divisions,
        named_orders,
        emit_rules,
    } = input;
    let strategies = labels
        .values()
        .flat_map(|outcomes| outcomes.keys().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut solved = strategies
        .iter()
        .map(|strategy| (*strategy, BTreeSet::new()))
        .collect::<Solved>();
    let mut refuted = strategies
        .iter()
        .map(|strategy| (*strategy, BTreeSet::new()))
        .collect::<Solved>();
    let mut saturated = strategies
        .iter()
        .map(|strategy| (*strategy, BTreeSet::new()))
        .collect::<Solved>();
    let mut elapsed = BTreeMap::<String, BTreeMap<u8, f64>>::new();
    for problem in selected_problems {
        if let Some(outcomes) = labels.get(problem) {
            for (strategy, outcome) in outcomes {
                elapsed
                    .entry(problem.clone())
                    .or_default()
                    .insert(*strategy, outcome.elapsed_s);
                match outcome.verdict.as_str() {
                    "refutation" => {
                        solved.get_mut(strategy).unwrap().insert(problem.clone());
                        refuted.get_mut(strategy).unwrap().insert(problem.clone());
                    }
                    "saturation" => {
                        solved.get_mut(strategy).unwrap().insert(problem.clone());
                        saturated.get_mut(strategy).unwrap().insert(problem.clone());
                    }
                    _ => {}
                }
            }
        }
    }
    let problem_set = selected_problems.iter().cloned().collect::<BTreeSet<_>>();
    let (greedy_order, greedy_trace, uncovered) = greedy_set_cover(&problem_set, &solved);
    let mut per_strategy = strategies
        .iter()
        .map(|strategy| {
            json!({
                "strategy": strategy,
                "solved": solved[strategy].len(),
                "refutation": refuted[strategy].len(),
                "saturation": saturated[strategy].len(),
            })
        })
        .collect::<Vec<_>>();
    per_strategy.sort_by(|left, right| {
        right["solved"]
            .as_u64()
            .cmp(&left["solved"].as_u64())
            .then(left["strategy"].as_u64().cmp(&right["strategy"].as_u64()))
    });

    let mut by_division = BTreeMap::<String, BTreeSet<String>>::new();
    for problem in selected_problems {
        let division = division_of(&features[problem]["path"]).to_ascii_lowercase();
        by_division
            .entry(division)
            .or_default()
            .insert(problem.clone());
    }
    let division_rows = by_division
        .iter()
        .map(|(division, members)| {
            let union = solved
                .values()
                .flat_map(|covered| covered.intersection(members).cloned())
                .collect::<BTreeSet<_>>();
            // The historical `max(..., key=coverage)` kept the first strategy
            // on a tie; strategies are passed in ascending-ID order.
            let best = best_strategy_for(members, &solved, &strategies);
            json!({
                "division": division,
                "problems": members.len(),
                "union_coverage": union.len(),
                "best_strategy": best,
                "best_coverage": best.map(|s| solved[&s].intersection(members).count()).unwrap_or_default(),
            })
        })
        .collect::<Vec<_>>();
    let union_all = solved
        .values()
        .flat_map(|members| members.iter().cloned())
        .collect::<BTreeSet<_>>();

    let mut behavior_histogram = BTreeMap::<String, usize>::new();
    for problem in selected_problems {
        let count = strategies
            .iter()
            .filter(|strategy| solved[strategy].contains(problem))
            .count();
        let bucket = match count {
            0 => "0",
            1 => "1",
            2..=3 => "2-3",
            4..=7 => "4-7",
            _ => "8-15",
        };
        *behavior_histogram.entry(bucket.to_owned()).or_default() += 1;
    }
    let routed_orders = fit_orders(features, selected_problems, &strategies, &solved);
    let routed_cover = selected_problems
        .iter()
        .filter(|problem| {
            routed_orders
                .get(&features[*problem]["label"])
                .unwrap_or(&strategies)
                .iter()
                .take(8)
                .any(|strategy| solved[strategy].contains(*problem))
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    let division_cover = selected_problems
        .iter()
        .filter(|problem| {
            let division = division_of(&features[*problem]["path"]).to_ascii_lowercase();
            named_orders
                .get(&format!("casc_{division}"))
                .unwrap_or(&named_orders["casc"])
                .iter()
                .take(8)
                .any(|strategy| solved[strategy].contains(*problem))
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    let in_sample_8 = json!({
        "routed_only": routed_cover.difference(&division_cover).count(),
        "division_only": division_cover.difference(&routed_cover).count(),
        "discordant": routed_cover.symmetric_difference(&division_cover).count(),
    });
    let mut portfolio_rows = Vec::new();
    for workers in [1, 2, 4, 8, 12, 15] {
        let (_, fixed_coverage, _, _) =
            best_fixed_portfolio(selected_problems, &solved, &strategies, workers);
        let routed = selected_problems
            .iter()
            .filter(|problem| {
                let order = routed_orders
                    .get(&features[*problem]["label"])
                    .cloned()
                    .unwrap_or_else(|| strategies.clone());
                order
                    .iter()
                    .take(workers)
                    .any(|strategy| solved[strategy].contains(*problem))
            })
            .count();
        let casc = portfolio_coverage(selected_problems, &named_orders["casc"], workers, &solved);
        let per_division = selected_problems
            .iter()
            .filter(|problem| {
                let division = division_of(&features[*problem]["path"]).to_ascii_lowercase();
                let order = named_orders
                    .get(&format!("casc_{division}"))
                    .unwrap_or(&named_orders["casc"]);
                order
                    .iter()
                    .take(workers)
                    .any(|strategy| solved[strategy].contains(*problem))
            })
            .count();
        portfolio_rows.push(json!({
            "slots": workers,
            "all_strategy_union": union_all.len(),
            "greedy_fixed": fixed_coverage,
            "fit_routed": routed,
            "casc": casc,
            "per_division": per_division,
            "routed_delta": routed as isize - per_division as isize,
        }));
    }
    let mut folds = Vec::new();
    let ordered = selected_problems.to_vec();
    for fold in 0..2 {
        let fit = ordered
            .iter()
            .enumerate()
            .filter(|(index, _)| index % 2 == fold)
            .map(|(_, problem)| problem.clone())
            .collect::<Vec<_>>();
        let test = ordered
            .iter()
            .enumerate()
            .filter(|(index, _)| index % 2 != fold)
            .map(|(_, problem)| problem.clone())
            .collect::<Vec<_>>();
        let orders = fit_orders(features, &fit, &strategies, &solved);
        let routed = test
            .iter()
            .filter(|problem| {
                orders
                    .get(&features[*problem]["label"])
                    .unwrap_or(&strategies)
                    .iter()
                    .take(8)
                    .any(|strategy| solved[strategy].contains(*problem))
            })
            .count();
        let baseline = test
            .iter()
            .filter(|problem| {
                let division = division_of(&features[*problem]["path"]).to_ascii_lowercase();
                named_orders
                    .get(&format!("casc_{division}"))
                    .unwrap_or(&named_orders["casc"])
                    .iter()
                    .take(8)
                    .any(|strategy| solved[strategy].contains(*problem))
            })
            .count();
        folds.push(json!({
            "fold": fold + 1,
            "test_size": test.len(),
            "routed": routed,
            "per_division": baseline,
            "delta": routed as isize - baseline as isize,
        }));
    }
    let held_out_routed = folds
        .iter()
        .map(|fold| fold["routed"].as_u64().unwrap_or_default())
        .sum::<u64>();
    let held_out_baseline = folds
        .iter()
        .map(|fold| fold["per_division"].as_u64().unwrap_or_default())
        .sum::<u64>();
    let behavior = behavioral_taxonomy(features, selected_problems, &strategies, &solved, &elapsed);
    let conditional = conditional_coverage(features, selected_problems, &strategies, &solved);
    let feasibility = best_feasibility_split(features, selected_problems, &solved);

    let mut report = json!({
        "feature_status_counts": statuses,
        "features_analysable": features.len(),
        "feature_duplicates_overwritten": feature_duplicates,
        "labels_problems": labels.len(),
        "label_duplicates_overwritten": label_duplicates,
        "joined_problems": selected_problems.len(),
        "selected_divisions": divisions,
        "per_strategy": per_strategy,
        "all_strategy_union": union_all.len(),
        "greedy_set_cover": {
            "order": greedy_order,
            "trace": greedy_trace.iter().map(|(strategy, gain, remaining)| json!({"strategy": strategy, "gain": gain, "remaining": remaining})).collect::<Vec<_>>(),
            "coverage": problem_set.len() - uncovered.len(),
            "uncovered": uncovered,
        },
        "per_division": division_rows,
        "behavior_histogram": behavior_histogram,
        "behavioral_taxonomy": behavior,
        "conditional_coverage": conditional,
        "best_feasibility_split": feasibility,
        "portfolio_simulation": portfolio_rows,
        "in_sample_8": in_sample_8,
        "cross_validation": {
            "folds": folds,
            "routed_coverage": held_out_routed,
            "per_division_coverage": held_out_baseline,
            "delta": held_out_routed as isize - held_out_baseline as isize,
        },
        "routed_orders": routed_orders,
        "emit_rules": emit_rules,
    });
    if emit_rules {
        let mut by_label = BTreeMap::<String, BTreeSet<String>>::new();
        for problem in selected_problems {
            by_label
                .entry(features[problem]["label"].clone())
                .or_default()
                .insert(problem.clone());
        }
        report["routing_evidence"] = json!(by_label
            .iter()
            .filter(|(_, members)| members.len() >= 3)
            .map(|(label, members)| {
                let mut ranking = strategies.iter().map(|strategy| {
                    (*strategy, solved[strategy].intersection(members).count())
                }).collect::<Vec<_>>();
                ranking.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
                json!({"label": label, "problems": members.len(), "ranking": ranking.into_iter().map(|(strategy, coverage)| json!({"strategy": strategy, "coverage": coverage})).collect::<Vec<_>>()})
            })
            .collect::<Vec<_>>());
    }
    report
}

fn features_only_report(
    features: &Features,
    statuses: &BTreeMap<String, usize>,
    duplicate_rows: usize,
) -> Value {
    let axes = [
        "logic_class",
        "shape_class",
        "scale_class",
        "goal_class",
        "decomposition_class",
        "dialect",
    ];
    let distributions = axes
        .iter()
        .map(|axis| {
            let mut values = BTreeMap::<String, usize>::new();
            for row in features.values() {
                *values
                    .entry(row.get(*axis).cloned().unwrap_or_default())
                    .or_default() += 1;
            }
            ((*axis).to_owned(), values)
        })
        .collect::<BTreeMap<_, _>>();
    let mut division_logic = BTreeMap::<String, BTreeMap<String, usize>>::new();
    let mut labels = BTreeMap::<String, usize>::new();
    for row in features.values() {
        *division_logic
            .entry(division_of(row.get("path").map(String::as_str).unwrap_or("?")).to_owned())
            .or_default()
            .entry(row.get("logic_class").cloned().unwrap_or_default())
            .or_default() += 1;
        *labels
            .entry(row.get("label").cloned().unwrap_or_default())
            .or_default() += 1;
    }
    let distinct_labels = labels.len();
    let mut top_labels = labels.into_iter().collect::<Vec<_>>();
    top_labels.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
    top_labels.truncate(20);
    json!({
        "total_files": statuses.values().sum::<usize>(),
        "status_counts": statuses,
        "analysable": features.len(),
        "duplicate_rows_overwritten": duplicate_rows,
        "class_distributions": distributions,
        "division_logic_agreement": division_logic,
        "distinct_labels": distinct_labels,
        "top_labels": top_labels.into_iter().map(|(label, count)| json!({"label": label, "count": count})).collect::<Vec<_>>(),
    })
}

fn print_report(report: &Value, features_only: bool) {
    if features_only {
        println!(
            "corpus: {} files",
            report["total_files"].as_u64().unwrap_or_default()
        );
        println!("front-end outcome:");
        let total_files = report["total_files"].as_u64().unwrap_or_default();
        if let Some(statuses) = report["status_counts"].as_object() {
            let mut statuses = statuses
                .iter()
                .map(|(status, count)| (status, count.as_u64().unwrap_or_default()))
                .collect::<Vec<_>>();
            statuses.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(right.0)));
            for (status, count) in statuses {
                println!(
                    "  {status:<24} {count:>6}  {:>5.1}%",
                    100.0 * count as f64 / total_files.max(1) as f64
                );
            }
        }
        println!(
            "\nanalysable problems: {}",
            report["analysable"].as_u64().unwrap_or_default()
        );
        println!(
            "duplicate names overwritten: {}",
            report["duplicate_rows_overwritten"]
                .as_u64()
                .unwrap_or_default()
        );
        if let Some(distributions) = report["class_distributions"].as_object() {
            for (axis, values) in distributions {
                println!("\n{axis}:");
                if let Some(values) = values.as_object() {
                    let mut values = values
                        .iter()
                        .map(|(value, count)| (value, count.as_u64().unwrap_or_default()))
                        .collect::<Vec<_>>();
                    values.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(right.0)));
                    for (value, count) in values {
                        println!("  {value:<18} {count:>6}");
                    }
                }
            }
        }
        println!("\nCASC division / inferred logic-class agreement:");
        if let Some(divisions) = report["division_logic_agreement"].as_object() {
            for (division, values) in divisions {
                let Some(values) = values.as_object() else {
                    continue;
                };
                let mut values = values
                    .iter()
                    .map(|(logic, count)| (logic, count.as_u64().unwrap_or_default()))
                    .collect::<Vec<_>>();
                values.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(right.0)));
                let total = values.iter().map(|(_, count)| *count).sum::<u64>();
                let classes = values
                    .iter()
                    .take(4)
                    .map(|(logic, count)| format!("{logic}={count}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                let modal = values
                    .first()
                    .map(|(logic, count)| {
                        format!(
                            "modal={logic} ({:.1}% agree)",
                            100.0 * *count as f64 / total.max(1) as f64
                        )
                    })
                    .unwrap_or_else(|| "modal=?".into());
                println!("  {division:<6} n={total:<5} {modal}  {classes}");
            }
        }
        println!(
            "\n{} distinct full labels",
            report["distinct_labels"].as_u64().unwrap_or_default()
        );
        println!("full label distribution (top 20):");
        for item in report["top_labels"].as_array().into_iter().flatten() {
            println!(
                "  {:<48} {:>6}",
                item["label"].as_str().unwrap_or_default(),
                item["count"].as_u64().unwrap_or_default()
            );
        }
        return;
    }
    println!(
        "features: {} analysable of {} files ({} duplicate names overwritten)",
        report["features_analysable"].as_u64().unwrap_or_default(),
        report["feature_status_counts"]
            .as_object()
            .map(|map| map
                .values()
                .map(|value| value.as_u64().unwrap_or_default())
                .sum::<u64>())
            .unwrap_or_default(),
        report["feature_duplicates_overwritten"]
            .as_u64()
            .unwrap_or_default()
    );
    println!(
        "labels:   {} problems ({} duplicate outcomes overwritten)",
        report["labels_problems"].as_u64().unwrap_or_default(),
        report["label_duplicates_overwritten"]
            .as_u64()
            .unwrap_or_default()
    );
    println!(
        "join:     {} problems with both features and labels",
        report["joined_problems"].as_u64().unwrap_or_default()
    );
    println!("\nper-strategy coverage:");
    for item in report["per_strategy"].as_array().into_iter().flatten() {
        println!(
            "  s{:<3} solved={:<5} refutation={:<5} saturation={}",
            item["strategy"].as_u64().unwrap_or_default(),
            item["solved"].as_u64().unwrap_or_default(),
            item["refutation"].as_u64().unwrap_or_default(),
            item["saturation"].as_u64().unwrap_or_default()
        );
    }
    println!(
        "  union of all strategies: {}",
        report["all_strategy_union"].as_u64().unwrap_or_default()
    );
    println!("\ngreedy set cover:");
    for item in report["greedy_set_cover"]["trace"]
        .as_array()
        .into_iter()
        .flatten()
    {
        println!(
            "  + s{:<3} +{:<4} uncovered={}",
            item["strategy"].as_u64().unwrap_or_default(),
            item["gain"].as_u64().unwrap_or_default(),
            item["remaining"].as_u64().unwrap_or_default()
        );
    }
    println!(
        "  coverage {}/{}",
        report["greedy_set_cover"]["coverage"]
            .as_u64()
            .unwrap_or_default(),
        report["joined_problems"].as_u64().unwrap_or_default()
    );
    println!("\nconditional coverage (feature buckets with at least eight problems):");
    for item in report["conditional_coverage"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let best = item["best_strategies"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|entry| {
                format!(
                    "s{}={}",
                    entry["strategy"].as_u64().unwrap_or_default(),
                    entry["coverage"].as_u64().unwrap_or_default()
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        println!(
            "  {:<34} {:<12} n={:<4} union={:<4} {}",
            item["feature"].as_str().unwrap_or_default(),
            item["bucket"].as_str().unwrap_or_default(),
            item["problems"].as_u64().unwrap_or_default(),
            item["union_coverage"].as_u64().unwrap_or_default(),
            best
        );
    }
    if let Some(split) = report
        .get("best_feasibility_split")
        .filter(|value| !value.is_null())
    {
        println!(
            "\nstrongest feasibility split: {} < {} (lift {:.3})",
            split["feature"].as_str().unwrap_or_default(),
            split["threshold"].as_f64().unwrap_or_default(),
            split["lift"].as_f64().unwrap_or_default()
        );
    }
    println!("\nportfolio simulation is a solo-matrix union, not concurrent coverage:");
    println!(
        "{:>7} {:>7} {:>12} {:>13} {:>6} {:>9} {:>8}",
        "slots", "union", "greedy", "fit-route", "casc", "division", "delta"
    );
    for item in report["portfolio_simulation"]
        .as_array()
        .into_iter()
        .flatten()
    {
        println!(
            "{:>7} {:>7} {:>12} {:>13} {:>6} {:>9} {:>+8}",
            item["slots"].as_u64().unwrap_or_default(),
            item["all_strategy_union"].as_u64().unwrap_or_default(),
            item["greedy_fixed"].as_u64().unwrap_or_default(),
            item["fit_routed"].as_u64().unwrap_or_default(),
            item["casc"].as_u64().unwrap_or_default(),
            item["per_division"].as_u64().unwrap_or_default(),
            item["routed_delta"].as_i64().unwrap_or_default()
        );
    }
    println!(
        "\nin-sample fit diagnostics at 8 slots: routed-only={} division-only={} discordant={}",
        report["in_sample_8"]["routed_only"]
            .as_u64()
            .unwrap_or_default(),
        report["in_sample_8"]["division_only"]
            .as_u64()
            .unwrap_or_default(),
        report["in_sample_8"]["discordant"]
            .as_u64()
            .unwrap_or_default()
    );
    println!(
        "\nheld-out deterministic two-fold comparison at 8 slots: routed {} / per-division {} (delta {:+})",
        report["cross_validation"]["routed_coverage"]
            .as_u64()
            .unwrap_or_default(),
        report["cross_validation"]["per_division_coverage"]
            .as_u64()
            .unwrap_or_default(),
        report["cross_validation"]["delta"]
            .as_i64()
            .unwrap_or_default()
    );
    println!(
        "\nbehavioral summary (problems solved by this many configurations): {:?}",
        report["behavioral_taxonomy"]["solved_configuration_histogram"]
    );
    for pair in report["behavioral_taxonomy"]["exclusive_strategy_pairs"]
        .as_array()
        .into_iter()
        .flatten()
    {
        println!(
            "  s{}+s{} exclusive={}",
            pair["left"].as_u64().unwrap_or_default(),
            pair["right"].as_u64().unwrap_or_default(),
            pair["exclusive_total"].as_u64().unwrap_or_default()
        );
    }
    let mut fastest = report["behavioral_taxonomy"]["fastest_strategy_per_problem"]
        .as_object()
        .map(|counts| {
            counts
                .iter()
                .map(|(strategy, count)| (strategy.clone(), count.as_u64().unwrap_or_default()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    fastest.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
    println!("fastest solving configuration per problem:");
    for (strategy, count) in fastest.into_iter().take(10) {
        println!("  {strategy}={count}");
    }
    println!("largest solver sets:");
    for signature in report["behavioral_taxonomy"]["largest_solver_sets"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let solved_by = signature["signature"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|strategy| format!("s{}", strategy.as_u64().unwrap_or_default()))
            .collect::<Vec<_>>()
            .join(",");
        let labels = signature["top_labels"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|label| {
                format!(
                    "{}({})",
                    label["label"].as_str().unwrap_or_default(),
                    label["count"].as_u64().unwrap_or_default()
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        println!(
            "  n={:<4} solved_by={:<30} {}",
            signature["problems"].as_u64().unwrap_or_default(),
            if solved_by.is_empty() {
                "-"
            } else {
                &solved_by
            },
            labels
        );
    }
    println!("label / solver-set purity:");
    for label in report["behavioral_taxonomy"]["label_behavior_purity"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let modal = label["modal_signature"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|strategy| format!("s{}", strategy.as_u64().unwrap_or_default()))
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "  {:<46} n={:<4} behavior_sets={:<3} modal={:<26} purity={:.2}",
            label["label"].as_str().unwrap_or_default(),
            label["problems"].as_u64().unwrap_or_default(),
            label["behavior_sets"].as_u64().unwrap_or_default(),
            if modal.is_empty() { "-" } else { &modal },
            label["modal_purity"].as_f64().unwrap_or_default()
        );
    }
    if let Some(purity) = report["behavioral_taxonomy"]["weighted_modal_purity"].as_f64() {
        println!("  weighted modal label purity: {purity:.3}");
    }
    if report["emit_rules"] == true {
        println!("\nrouting evidence (per label, strategy ranking):");
        for entry in report["routing_evidence"].as_array().into_iter().flatten() {
            let ranking = entry["ranking"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|row| row["coverage"].as_u64().unwrap_or_default() > 0)
                .take(6)
                .map(|row| {
                    format!(
                        "s{}={}",
                        row["strategy"].as_u64().unwrap_or_default(),
                        row["coverage"].as_u64().unwrap_or_default()
                    )
                })
                .collect::<Vec<_>>()
                .join(" ");
            println!(
                "  {:<46} n={:<4} {}",
                entry["label"].as_str().unwrap_or_default(),
                entry["problems"].as_u64().unwrap_or_default(),
                ranking
            );
        }
    }
}

pub fn analyze(args: AnalyzeArgs) -> ToolResult<()> {
    let (features, statuses, feature_duplicates) = load_features(&args.features)?;
    let features_only = args.features_only || args.labels.is_none();
    let report = if features_only {
        features_only_report(&features, &statuses, feature_duplicates)
    } else {
        let (labels, label_duplicates) = load_labels(args.labels.as_deref().unwrap())?;
        let division_filter = args
            .divisions
            .iter()
            .map(|value| value.to_ascii_lowercase())
            .collect::<BTreeSet<_>>();
        let mut problems = features
            .keys()
            .filter(|problem| labels.contains_key(*problem))
            .cloned()
            .collect::<Vec<_>>();
        if !division_filter.is_empty() {
            problems.retain(|problem| {
                division_filter
                    .contains(&division_of(&features[problem]["path"]).to_ascii_lowercase())
            });
        }
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap();
        let named_rs = args
            .named_rs
            .unwrap_or_else(|| root.join("crates/mrs-search/src/strategy/named.rs"));
        let named_orders = load_named_orders(&named_rs)?;
        build_report(AnalysisInput {
            features: &features,
            statuses: &statuses,
            labels: &labels,
            feature_duplicates,
            label_duplicates,
            selected_problems: &problems,
            divisions: &args.divisions,
            named_orders: &named_orders,
            emit_rules: args.emit_rules,
        })
    };
    print_report(&report, features_only);
    if let Some(path) = args.json_out {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(
            path,
            format!("{}\n", serde_json::to_string_pretty(&report)?),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fitted_order_places_greedy_cover_first_and_prefix_is_respected() {
        let problems = vec!["p1".to_owned(), "p2".to_owned(), "p3".to_owned()];
        let features = problems
            .iter()
            .map(|problem| {
                (
                    problem.clone(),
                    BTreeMap::from([("label".into(), "same-class".into())]),
                )
            })
            .collect::<Features>();
        let solved = BTreeMap::from([
            (1, BTreeSet::from(["p1".into(), "p2".into()])),
            (2, BTreeSet::from(["p2".into(), "p3".into()])),
            (3, BTreeSet::from(["p3".into()])),
        ]);
        let order = fit_orders(&features, &problems, &[1, 2, 3], &solved);
        assert_eq!(&order["same-class"][..2], &[1, 2]);
        let (chosen, coverage, _, remaining) =
            best_fixed_portfolio(&problems, &solved, &[1, 2, 3], 1);
        assert_eq!(chosen, [1]);
        assert_eq!(coverage, 2);
        assert_eq!(remaining, BTreeSet::from(["p3".into()]));
    }

    #[test]
    fn greedy_ties_keep_lowest_strategy_id() {
        let universe = BTreeSet::from(["a".into(), "b".into()]);
        let solved = BTreeMap::from([(1, universe.clone()), (2, universe.clone())]);
        assert_eq!(greedy_set_cover(&universe, &solved).0, [1]);
    }

    #[test]
    fn named_orders_match_the_shipped_strategy_registry() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .join("crates/mrs-search/src/strategy/named.rs");
        let orders = load_named_orders(&path).unwrap();
        assert_eq!(&orders["casc_feq"][..8], &[12, 11, 10, 6, 8, 1, 4, 13]);
        assert_eq!(&orders["casc_fne"][..8], &[11, 13, 4, 8, 15, 7, 3, 1]);
        assert_eq!(&orders["casc_epr"][..8], &[6, 2, 1, 3, 4, 5, 7, 8]);
        assert_eq!(&orders["casc_eps"][..8], &[2, 3, 1, 8, 11, 12, 9, 14]);
    }

    #[test]
    fn division_best_strategy_ties_keep_the_first_strategy() {
        let members = BTreeSet::from(["p1".to_owned()]);
        let solved = BTreeMap::from([
            (1, BTreeSet::new()),
            (2, BTreeSet::new()),
            (3, BTreeSet::from(["p1".to_owned()])),
            (4, BTreeSet::from(["p1".to_owned()])),
        ]);
        assert_eq!(best_strategy_for(&members, &solved, &[1, 2, 3, 4]), Some(3));
    }

    #[test]
    fn csv_alias_rows_keep_the_last_sorted_dump_outcome_and_report_collapses() {
        let directory = tempfile::tempdir().unwrap();
        let feature_path = directory.path().join("features.csv");
        fs::write(
            &feature_path,
            "path,parse_status,name,label,logic_class\ncanonical/FEQ/ALIAS.p,ok,ALIAS,FEQ/HORN,FEQ\nProblems/FEQ/ALIAS.p,ok,ALIAS,FEQ/NON_HORN,FEQ\n",
        )
        .unwrap();
        let (features, statuses, duplicates) = load_features(&feature_path).unwrap();
        assert_eq!(duplicates, 1);
        assert_eq!(features["ALIAS"]["label"], "FEQ/NON_HORN");
        assert_eq!(statuses["ok"], 2);

        let labels_path = directory.path().join("labels.csv");
        fs::write(
            &labels_path,
            "problem,strategy,verdict,elapsed_s\nALIAS,1,unsolved,1\nALIAS,1,refutation,2\n",
        )
        .unwrap();
        let (labels, label_duplicates) = load_labels(&labels_path).unwrap();
        assert_eq!(label_duplicates, 1);
        assert_eq!(labels["ALIAS"][&1].verdict, "refutation");
    }
}
