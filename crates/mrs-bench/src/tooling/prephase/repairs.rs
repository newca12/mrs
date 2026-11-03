use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

use clap::Args;

type ToolResult<T> = Result<T, Box<dyn Error>>;

#[derive(Args, Debug)]
pub struct RepairProbeArgs {
    /// Input CSV, or `-` for stdin.
    input: PathBuf,
}

#[derive(Args, Debug)]
pub struct RepairIsolatedArgs {
    /// Input CSV, or `-` for stdin.
    input: PathBuf,
}

const CLASS_FIELDS: [&str; 5] = [
    "logic_class",
    "shape_class",
    "scale_class",
    "goal_class",
    "decomposition_class",
];
const PROBE_FIELDS: [&str; 10] = [
    "probe_iterations",
    "probe_generated",
    "probe_processed",
    "probe_forward_subsumed",
    "probe_weight_discarded",
    "probe_passive",
    "probe_elapsed_ms",
    "probe_generation_rate",
    "probe_redundancy_rate",
    "probe_throughput_per_ms",
];

fn read_input(path: &PathBuf) -> ToolResult<Vec<u8>> {
    if path.as_os_str() == "-" {
        let mut bytes = Vec::new();
        io::stdin().read_to_end(&mut bytes)?;
        Ok(bytes)
    } else {
        Ok(fs::read(path)?)
    }
}

pub fn repair_probe_columns(args: RepairProbeArgs) -> ToolResult<()> {
    let data = read_input(&args.input)?;
    let (output, repaired, blank, already_fixed) = repair_probe_csv(&data)?;
    io::stdout().lock().write_all(&output)?;
    writeln!(
        io::stderr().lock(),
        "repaired={repaired} blank_probe_rows={blank} already_fixed={already_fixed}"
    )?;
    Ok(())
}

fn repair_probe_csv(data: &[u8]) -> ToolResult<(Vec<u8>, usize, usize, usize)> {
    let mut reader = csv::ReaderBuilder::new().flexible(true).from_reader(data);
    let headers = reader.headers()?.clone();
    let mut indexes = BTreeMap::new();
    for field in CLASS_FIELDS.into_iter().chain(PROBE_FIELDS) {
        let index = headers
            .iter()
            .position(|header| header == field)
            .ok_or_else(|| format!("input CSV does not contain required column {field}"))?;
        indexes.insert(field, index);
    }
    let status_index = headers
        .iter()
        .position(|header| header == "parse_status")
        .ok_or("input CSV does not contain parse_status")?;
    let label_index = headers
        .iter()
        .position(|header| header == "label")
        .ok_or("input CSV does not contain label")?;
    let name_index = headers.iter().position(|header| header == "name");
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer.write_record(&headers)?;
    let mut repaired = 0usize;
    let mut blank = 0usize;
    let mut already_fixed = 0usize;
    for (row_index, record) in reader.records().enumerate() {
        let record = record?;
        if record.len() > headers.len() {
            return Err(format!("row {} has unexpected extra CSV fields", row_index + 2).into());
        }
        let mut row = record.iter().map(str::to_owned).collect::<Vec<_>>();
        row.resize(headers.len(), String::new());
        let status = &row[status_index];
        let recoverable = matches!(
            status.as_str(),
            "ok" | "empty_after_lowering" | "include_error"
        );
        if !recoverable {
            for field in PROBE_FIELDS {
                row[indexes[field]].clear();
            }
            blank += 1;
            writer.write_record(row)?;
            continue;
        }

        let static_label = CLASS_FIELDS
            .iter()
            .map(|field| row[indexes[field]].as_str())
            .collect::<Vec<_>>()
            .join("/");
        let label = row[label_index].clone();
        if label == static_label && !row[indexes["probe_throughput_per_ms"]].is_empty() {
            already_fixed += 1;
            writer.write_record(row)?;
            continue;
        }
        if !label.starts_with(&static_label) {
            let name = name_index.map(|index| row[index].as_str()).unwrap_or("?");
            return Err(format!("label does not match class columns for {name}").into());
        }
        let iterations = label[static_label.len()..].to_owned();
        if iterations.is_empty() || !iterations.bytes().all(|byte| byte.is_ascii_digit()) {
            let name = name_index.map(|index| row[index].as_str()).unwrap_or("?");
            return Err(format!("cannot recover probe iteration suffix for {name}").into());
        }
        let old = PROBE_FIELDS
            .iter()
            .map(|field| row[indexes[field]].clone())
            .collect::<Vec<_>>();
        if old[8].is_empty() {
            let name = name_index.map(|index| row[index].as_str()).unwrap_or("?");
            return Err(format!("missing shifted throughput value for {name}").into());
        }
        row[label_index] = static_label;
        row[indexes["probe_iterations"]] = iterations;
        for offset in 1..PROBE_FIELDS.len() {
            row[indexes[PROBE_FIELDS[offset]]] = old[offset - 1].clone();
        }
        row[indexes["probe_throughput_per_ms"]] = old[8].clone();
        repaired += 1;
        writer.write_record(row)?;
    }
    let output = writer.into_inner()?;
    Ok((output, repaired, blank, already_fixed))
}

pub fn repair_isolated_dump(args: RepairIsolatedArgs) -> ToolResult<()> {
    let data = read_input(&args.input)?;
    let (output, recovered, degraded) = repair_isolated_csv(&data)?;
    io::stdout().lock().write_all(&output)?;
    writeln!(
        io::stderr().lock(),
        "recovered name: {recovered}, detail text lost: {degraded}"
    )?;
    Ok(())
}

fn repair_isolated_csv(data: &[u8]) -> ToolResult<(Vec<u8>, usize, usize)> {
    let mut reader = csv::ReaderBuilder::new().flexible(true).from_reader(data);
    let headers = reader.headers()?.clone();
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer.write_record(&headers)?;
    let mut recovered = 0usize;
    let mut degraded = 0usize;
    for (row_index, record) in reader.records().enumerate() {
        let record = record?;
        if record.len() == headers.len() {
            writer.write_record(&record)?;
            continue;
        }
        if record.len() + 1 != headers.len() || record.len() < 4 {
            return Err(format!(
                "unexpected row width {} at row {}",
                record.len(),
                row_index + 2
            )
            .into());
        }
        let path = record.get(0).unwrap_or_default();
        let status = record.get(1).unwrap_or_default();
        let name = if matches!(status, "ok" | "empty_after_lowering") {
            recovered += 1;
            PathBuf::from(path)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or_default()
                .to_owned()
        } else {
            degraded += 1;
            String::new()
        };
        let mut repaired = vec![path.to_owned(), status.to_owned(), String::new(), name];
        repaired.extend(record.iter().skip(3).map(str::to_owned));
        writer.write_record(repaired)?;
    }
    let output = writer.into_inner()?;
    Ok((output, recovered, degraded))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_repair_recovers_suffix_and_shifts_fields() {
        let headers = [
            "parse_status",
            "name",
            "logic_class",
            "shape_class",
            "scale_class",
            "goal_class",
            "decomposition_class",
            "label",
            "probe_iterations",
            "probe_generated",
            "probe_processed",
            "probe_forward_subsumed",
            "probe_weight_discarded",
            "probe_passive",
            "probe_elapsed_ms",
            "probe_generation_rate",
            "probe_redundancy_rate",
            "probe_throughput_per_ms",
        ];
        let fields = [
            "ok",
            "probe-case",
            "FEQ",
            "HORN",
            "TINY",
            "TIGHT",
            "CONNECTED",
            "FEQ/HORN/TINY/TIGHT/CONNECTED4",
            "10",
            "3",
            "2",
            "1",
            "5",
            "50",
            "2.5",
            "0.2",
            "0.06",
            "",
        ];
        let mut writer = csv::Writer::from_writer(Vec::new());
        writer.write_record(headers).unwrap();
        writer.write_record(fields).unwrap();
        let input = writer.into_inner().unwrap();
        let (output, repaired, blank, fixed) = repair_probe_csv(&input).unwrap();
        let mut reader = csv::Reader::from_reader(output.as_slice());
        let header = reader.headers().unwrap().clone();
        let row = reader.records().next().unwrap().unwrap();
        let map = header.iter().zip(row.iter()).collect::<BTreeMap<_, _>>();
        assert_eq!(repaired, 1);
        assert_eq!(blank, 0);
        assert_eq!(fixed, 0);
        assert_eq!(map["label"], "FEQ/HORN/TINY/TIGHT/CONNECTED");
        assert_eq!(map["probe_iterations"], "4");
        assert_eq!(map["probe_generated"], "10");
        assert_eq!(map["probe_processed"], "3");
        assert_eq!(map["probe_forward_subsumed"], "2");
        assert_eq!(map["probe_weight_discarded"], "1");
        assert_eq!(map["probe_passive"], "5");
        assert_eq!(map["probe_elapsed_ms"], "50");
        assert_eq!(map["probe_generation_rate"], "2.5");
        assert_eq!(map["probe_redundancy_rate"], "0.2");
        assert_eq!(map["probe_throughput_per_ms"], "0.06");
    }

    #[test]
    fn isolated_dump_repair_recovers_name_and_marks_lost_diagnostics() {
        let input = b"path,parse_status,parse_detail,name,label\nProblems/FEQ/FOO001.p,ok,,label\nProblems/FEQ/BAD001.p,parse_error,detail,label\n";
        let (output, recovered, degraded) = repair_isolated_csv(input).unwrap();
        let mut reader = csv::Reader::from_reader(output.as_slice());
        let rows = reader.records().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(recovered, 1);
        assert_eq!(degraded, 1);
        assert_eq!(rows[0].get(3), Some("FOO001"));
        assert_eq!(rows[1].get(2), Some(""));
        assert_eq!(rows[1].get(3), Some(""));
    }
}
