use mrs_core::ml::parent_guidance::{
    PARENT_FEATURE_DIM, PARENT_GUIDANCE_SCHEMA, ParentGuidanceModel,
};
use rand::SeedableRng;
use rand::seq::SliceRandom;
use std::path::{Path, PathBuf};

fn collect_csv(path: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_csv(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "csv") {
            files.push(path);
        }
    }
}

fn parse_row(line: &str) -> Option<(u8, f32, [f32; PARENT_FEATURE_DIM])> {
    let mut columns = line.split(',');
    let kind = columns.next()?.parse::<u8>().ok()?;
    if kind > 3 {
        return None;
    }
    let label = columns.next()?.parse::<f32>().ok()?;
    if !label.is_finite() {
        return None;
    }
    let mut features = [0.0_f32; PARENT_FEATURE_DIM];
    for feature in &mut features {
        *feature = columns.next()?.parse().ok()?;
        if !(*feature).is_finite() {
            return None;
        }
    }
    columns.next().is_none().then_some((kind, label, features))
}

fn logit(
    features: &[f32; PARENT_FEATURE_DIM],
    weights: &[f32; PARENT_FEATURE_DIM],
    bias: f32,
) -> f32 {
    bias + weights
        .iter()
        .zip(features)
        .map(|(weight, feature)| weight * feature)
        .sum::<f32>()
}

pub fn run(log_dir: &str, output: &str, epochs: usize, kind: u8) -> Result<(), String> {
    if kind > 3 {
        return Err("inference kind must be in 0..=3".into());
    }
    let root = Path::new(log_dir).join("parent-guidance");
    let mut files = Vec::new();
    collect_csv(&root, &mut files);
    files.sort();
    if files.is_empty() {
        return Err(format!("no parent-guidance traces in {}", root.display()));
    }

    let single_file = files.len() == 1;
    let mut training = Vec::new();
    let mut validation = Vec::new();
    for (index, path) in files.iter().enumerate() {
        let contents = std::fs::read_to_string(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let target = if !single_file && index % 5 == 0 {
            &mut validation
        } else {
            &mut training
        };
        target.extend(
            contents
                .lines()
                .filter_map(parse_row)
                .filter(|(sample_kind, _, _)| *sample_kind == kind),
        );
    }

    let positives = training.iter().filter(|(_, label, _)| *label > 0.5).count();
    let negatives = training.len().saturating_sub(positives);
    if training.len() < 100 || positives == 0 || negatives == 0 {
        return Err(format!(
            "need >=100 rows and both labels; got {} ({} positive, {} negative)",
            training.len(),
            positives,
            negatives
        ));
    }
    let valid_positive = validation
        .iter()
        .filter(|(_, label, _)| *label > 0.5)
        .count();
    let valid_negative = validation.len().saturating_sub(valid_positive);
    if !validation.is_empty() && (valid_positive == 0 || valid_negative == 0) {
        return Err("file-held-out validation set must contain both labels".into());
    }
    if single_file {
        eprintln!("warning: single trace file; no problem-level holdout available");
    }

    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    training.shuffle(&mut rng);
    let mut weights = [0.0; PARENT_FEATURE_DIM];
    let mut bias = 0.0;
    for epoch in 0..epochs.max(1) {
        let mut gradient = [0.0; PARENT_FEATURE_DIM];
        let mut bias_gradient = 0.0;
        for (_, label, features) in &training {
            let target = f32::from(*label > 0.5);
            let probability =
                1.0 / (1.0 + (-logit(features, &weights, bias).clamp(-30.0, 30.0)).exp());
            let balance = if target > 0.5 {
                training.len() as f32 / (2.0 * positives as f32)
            } else {
                training.len() as f32 / (2.0 * negatives as f32)
            };
            let error = (probability - target) * balance;
            for (sum, feature) in gradient.iter_mut().zip(features) {
                *sum += error * feature;
            }
            bias_gradient += error;
        }
        let rate = 0.5 / training.len() as f32;
        for (weight, sum) in weights.iter_mut().zip(gradient) {
            *weight -= rate * (sum + 1e-4 * *weight * training.len() as f32);
        }
        bias -= rate * bias_gradient;
        if epoch % 10 == 0 || epoch + 1 == epochs {
            eprintln!("parent model epoch {} complete", epoch + 1);
        }
    }

    if !validation.is_empty() {
        let true_positive = validation
            .iter()
            .filter(|(_, label, features)| logit(features, &weights, bias) >= 0.0 && *label > 0.5)
            .count();
        let false_positive = validation
            .iter()
            .filter(|(_, label, features)| logit(features, &weights, bias) >= 0.0 && *label <= 0.5)
            .count();
        let false_negative = valid_positive.saturating_sub(true_positive);
        eprintln!(
            "heldout_precision={:.4} recall={:.4} tp={true_positive} fp={false_positive} fn={false_negative} rows={}",
            true_positive as f32 / true_positive.saturating_add(false_positive).max(1) as f32,
            true_positive as f32 / valid_positive as f32,
            validation.len()
        );
    }

    let model = ParentGuidanceModel {
        schema_version: PARENT_GUIDANCE_SCHEMA,
        inference_kind: kind,
        weights,
        bias,
    };
    let output = Path::new(output);
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(
        output,
        serde_json::to_vec_pretty(&model).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "trained {} pairs ({} positive, {} negative): {}",
        training.len(),
        positives,
        negatives,
        output.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_parent_pair_csv() {
        let values = std::iter::repeat_n("0.25", PARENT_FEATURE_DIM)
            .collect::<Vec<_>>()
            .join(",");
        let (kind, label, features) = parse_row(&format!("2,1,{values}")).unwrap();
        assert_eq!((kind, label), (2, 1.0));
        assert!(features.iter().all(|feature| *feature == 0.25));
        assert!(parse_row(&format!("9,1,{values}")).is_none());
    }
}
