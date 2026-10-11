use burn::data::dataloader::DataLoaderBuilder;
use burn::data::dataset::InMemDataset;
use burn::lr_scheduler::constant::ConstantLr;
use burn::nn::loss::BinaryCrossEntropyLossConfig;
use burn::optim::AdamConfig;
use burn::prelude::*;
use burn::tensor::Device;
use burn::train::metric::LossMetric;
use burn::train::metric::store::{Aggregate, Direction, Split};
use burn::train::{
    ClassificationOutput, InferenceStep, Learner, MetricEarlyStoppingStrategy, StoppingCondition,
    SupervisedTraining, TrainOutput, TrainStep,
};
use mrs_core::ml::features::SCHEMA_VERSION;
use mrs_core::ml::premise_selector::{PREMISE_FEATURE_DIM, PremiseModel};
use mrs_core::ml::sample::PremiseSample;
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;

#[derive(Clone, Debug)]
pub struct PremiseBatch {
    inputs: Tensor<2>,
    targets_bce: Tensor<2, Int>,
    targets_class: Tensor<1, Int>,
}

#[derive(Clone)]
struct PremiseBatcher;

impl burn::data::dataloader::batcher::Batcher<PremiseSample, PremiseBatch> for PremiseBatcher {
    fn batch(&self, items: Vec<PremiseSample>, device: &Device) -> PremiseBatch {
        let batch_size = items.len();
        let mut inputs = Vec::with_capacity(batch_size * PREMISE_FEATURE_DIM);
        let mut targets_ints = Vec::with_capacity(batch_size);

        for item in items {
            inputs.extend_from_slice(&item.feats);
            targets_ints.push(item.label as i32);
        }

        let inputs = Tensor::<1>::from_floats(inputs.as_slice(), device)
            .reshape([batch_size, PREMISE_FEATURE_DIM]);
        let targets_bce =
            Tensor::<1, Int>::from_ints(targets_ints.as_slice(), device).reshape([batch_size, 1]);
        let targets_class =
            Tensor::<1, Int>::from_ints(targets_ints.as_slice(), device).reshape([batch_size]);

        PremiseBatch {
            inputs,
            targets_bce,
            targets_class,
        }
    }
}

#[derive(burn::module::Module, Debug)]
pub struct TrainingPremise {
    model: PremiseModel,
}

impl TrainStep for TrainingPremise {
    type Input = PremiseBatch;
    type Output = ClassificationOutput;

    fn step(&self, batch: PremiseBatch) -> TrainOutput<ClassificationOutput> {
        let logits = self.model.forward(batch.inputs);
        let loss = BinaryCrossEntropyLossConfig::new()
            .with_logits(true)
            .init(&logits.device())
            .forward(logits.clone(), batch.targets_bce.clone());

        TrainOutput::new(
            self,
            loss.backward(),
            ClassificationOutput {
                loss,
                output: logits,
                targets: batch.targets_class,
            },
        )
    }
}

impl InferenceStep for TrainingPremise {
    type Input = PremiseBatch;
    type Output = ClassificationOutput;

    fn step(&self, batch: PremiseBatch) -> ClassificationOutput {
        let logits = self.model.forward(batch.inputs);
        let loss = BinaryCrossEntropyLossConfig::new()
            .with_logits(true)
            .init(&logits.device())
            .forward(logits.clone(), batch.targets_bce.clone());

        ClassificationOutput {
            loss,
            output: logits,
            targets: batch.targets_class,
        }
    }
}

fn load_dataset(dir: &str) -> Vec<PremiseSample> {
    let mut samples = Vec::new();
    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() {
            let ext = path.extension().and_then(|e| e.to_str());
            if ext == Some("wincode") {
                let mut file = std::fs::File::open(path).expect("Failed to open wincode file");
                let mut std_read = wincode::io::std_read::ReadAdapter::new(&mut file);
                while let Ok(sample) = wincode::deserialize_from(&mut std_read) {
                    samples.push(sample);
                }
            } else if ext == Some("csv")
                && let Ok(content) = std::fs::read_to_string(path)
            {
                for line in content.lines() {
                    let parts: Vec<&str> = line.split(',').collect();
                    if parts.len() == PREMISE_FEATURE_DIM + 1
                        && let Ok(label) = parts[0].parse::<f32>()
                    {
                        let mut feats = [0.0f32; PREMISE_FEATURE_DIM];
                        let mut ok = true;
                        for j in 0..PREMISE_FEATURE_DIM {
                            if let Ok(val) = parts[j + 1].parse::<f32>() {
                                feats[j] = val;
                            } else {
                                ok = false;
                                break;
                            }
                        }
                        if ok {
                            samples.push(PremiseSample { label, feats });
                        }
                    }
                }
            }
        }
    }
    samples
}

pub fn train_premise(
    device: Device,
    dir: &str,
    out_prefix: &str,
    epochs: usize,
    val_split: f32,
    neg_per_pos: usize,
) {
    let samples = load_dataset(dir);
    println!("Loaded premise dataset with {} samples", samples.len());

    let mut pos = Vec::new();
    let mut neg = Vec::new();
    for s in &samples {
        if s.label > 0.5 {
            pos.push(s.clone());
        } else {
            neg.push(s.clone());
        }
    }

    if pos.is_empty() {
        eprintln!("ERROR: no positive samples found.");
        return;
    }

    let mut rng = StdRng::seed_from_u64(42);
    neg.shuffle(&mut rng);
    let keep_neg = (pos.len() * neg_per_pos).min(neg.len());
    neg.truncate(keep_neg);

    pos.shuffle(&mut rng);
    neg.shuffle(&mut rng);
    let pos_val = ((pos.len() as f32) * val_split) as usize;
    let neg_val = ((neg.len() as f32) * val_split) as usize;

    let mut train_set: Vec<PremiseSample> = pos[pos_val..]
        .iter()
        .chain(&neg[neg_val..])
        .cloned()
        .collect();
    let mut valid_set: Vec<PremiseSample> = pos[..pos_val]
        .iter()
        .chain(&neg[..neg_val])
        .cloned()
        .collect();
    train_set.shuffle(&mut rng);
    valid_set.shuffle(&mut rng);

    let batcher_train = PremiseBatcher;
    let batcher_valid = PremiseBatcher;

    let dataloader_train = DataLoaderBuilder::new(batcher_train)
        .batch_size(8192)
        .shuffle(42)
        .num_workers(4)
        .set_device(device.clone())
        .build(std::sync::Arc::new(InMemDataset::new(train_set)));
    let dataloader_valid = DataLoaderBuilder::new(batcher_valid)
        .batch_size(8192)
        .shuffle(42)
        .num_workers(4)
        .set_device(device.clone())
        .build(std::sync::Arc::new(InMemDataset::new(valid_set)));

    let learner_config = Learner::new(
        TrainingPremise {
            model: PremiseModel::new(&device),
        },
        AdamConfig::new().init(),
        ConstantLr::new(3e-4),
    );

    let supervised = SupervisedTraining::new(
        format!("{}.artifacts", out_prefix),
        dataloader_train,
        dataloader_valid,
    )
    .metric_train_numeric(LossMetric::new())
    .metric_valid_numeric(LossMetric::new())
    .with_checkpointing_strategy(burn::train::checkpoint::KeepLastNCheckpoints::new(2))
    .num_epochs(epochs)
    .early_stopping(MetricEarlyStoppingStrategy::new(
        &LossMetric::new(),
        Aggregate::Mean,
        Direction::Lowest,
        Split::Valid,
        StoppingCondition::NoImprovementSince { n_epochs: 15 },
    ))
    .summary();

    let learning_result = supervised.launch(learner_config);

    learning_result
        .model
        .model
        .save_file(format!("{out_prefix}.bpk"))
        .unwrap();

    std::fs::write(
        format!("{}_meta.json", out_prefix),
        format!(
            r#"{{"PREMISE_FEATURE_DIM":{}, "SCHEMA_VERSION":{}}}"#,
            PREMISE_FEATURE_DIM, SCHEMA_VERSION
        ),
    )
    .unwrap();
    println!("Saved {}.bpk and {}_meta.json", out_prefix, out_prefix);
}
