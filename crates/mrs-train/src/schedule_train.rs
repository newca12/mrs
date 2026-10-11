use burn::data::dataloader::DataLoaderBuilder;
use burn::data::dataset::InMemDataset;
use burn::lr_scheduler::constant::ConstantLr;
use burn::nn::loss::CrossEntropyLossConfig;
use burn::optim::AdamConfig;
use burn::prelude::*;
use burn::tensor::Device;
use burn::train::metric::store::{Aggregate, Direction, Split};
use burn::train::metric::{AccuracyMetric, LossMetric};
use burn::train::{
    ClassificationOutput, InferenceStep, Learner, MetricEarlyStoppingStrategy, StoppingCondition,
    SupervisedTraining, TrainOutput, TrainStep,
};
use mrs_core::ml::features::SCHEMA_VERSION;
use mrs_core::ml::sample::ScheduleSample;
use mrs_core::ml::schedule_classifier::{SCHEDULE_FEATURE_DIM, ScheduleModel};
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;

#[derive(Clone, Debug)]
pub struct ScheduleBatch {
    inputs: Tensor<2>,
    targets: Tensor<1, Int>,
}

#[derive(Clone)]
struct ScheduleBatcher;

impl burn::data::dataloader::batcher::Batcher<ScheduleSample, ScheduleBatch> for ScheduleBatcher {
    fn batch(&self, items: Vec<ScheduleSample>, device: &Device) -> ScheduleBatch {
        let batch_size = items.len();
        let mut inputs = Vec::with_capacity(batch_size * SCHEDULE_FEATURE_DIM);
        let mut targets_ints = Vec::with_capacity(batch_size);

        for item in items {
            inputs.extend_from_slice(&item.feats);
            targets_ints.push(item.label_idx as i32);
        }

        let inputs = Tensor::<1>::from_floats(inputs.as_slice(), device)
            .reshape([batch_size, SCHEDULE_FEATURE_DIM]);
        let targets = Tensor::<1, Int>::from_ints(targets_ints.as_slice(), device);

        ScheduleBatch { inputs, targets }
    }
}

#[derive(burn::module::Module, Debug)]
pub struct TrainingSchedule {
    model: ScheduleModel,
}

impl TrainStep for TrainingSchedule {
    type Input = ScheduleBatch;
    type Output = ClassificationOutput;

    fn step(&self, batch: ScheduleBatch) -> TrainOutput<ClassificationOutput> {
        let logits = self.model.forward(batch.inputs);
        let loss = CrossEntropyLossConfig::new()
            .init(&logits.device())
            .forward(logits.clone(), batch.targets.clone());

        TrainOutput::new(
            self,
            loss.backward(),
            ClassificationOutput {
                loss,
                output: logits,
                targets: batch.targets,
            },
        )
    }
}

impl InferenceStep for TrainingSchedule {
    type Input = ScheduleBatch;
    type Output = ClassificationOutput;

    fn step(&self, batch: ScheduleBatch) -> ClassificationOutput {
        let logits = self.model.forward(batch.inputs);
        let loss = CrossEntropyLossConfig::new()
            .init(&logits.device())
            .forward(logits.clone(), batch.targets.clone());

        ClassificationOutput {
            loss,
            output: logits,
            targets: batch.targets,
        }
    }
}

fn load_dataset(dir: &str) -> Vec<ScheduleSample> {
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
                    if parts.len() == SCHEDULE_FEATURE_DIM + 1
                        && let Ok(label_idx) = parts[0].parse::<u32>()
                    {
                        let mut feats = [0.0f32; SCHEDULE_FEATURE_DIM];
                        let mut ok = true;
                        for j in 0..SCHEDULE_FEATURE_DIM {
                            if let Ok(val) = parts[j + 1].parse::<f32>() {
                                feats[j] = val;
                            } else {
                                ok = false;
                                break;
                            }
                        }
                        if ok {
                            samples.push(ScheduleSample { label_idx, feats });
                        }
                    }
                }
            }
        }
    }
    samples
}

pub fn train_schedule(device: Device, dir: &str, out_prefix: &str, epochs: usize, val_split: f32) {
    let mut samples = load_dataset(dir);
    println!("Loaded schedule dataset with {} samples", samples.len());
    if samples.is_empty() {
        return;
    }

    let mut rng = StdRng::seed_from_u64(42);
    samples.shuffle(&mut rng);
    let val_count = ((samples.len() as f32) * val_split) as usize;
    let (valid_set, train_set) = samples.split_at(val_count);

    let batcher_train = ScheduleBatcher;
    let batcher_valid = ScheduleBatcher;

    let dataloader_train = DataLoaderBuilder::new(batcher_train)
        .batch_size(8192)
        .shuffle(42)
        .num_workers(4)
        .set_device(device.clone())
        .build(std::sync::Arc::new(InMemDataset::new(train_set.to_vec())));
    let dataloader_valid = DataLoaderBuilder::new(batcher_valid)
        .batch_size(8192)
        .shuffle(42)
        .num_workers(4)
        .set_device(device.clone())
        .build(std::sync::Arc::new(InMemDataset::new(valid_set.to_vec())));

    let learner_config = Learner::new(
        TrainingSchedule {
            model: ScheduleModel::new(&device),
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
    .metric_train_numeric(AccuracyMetric::new())
    .metric_valid_numeric(AccuracyMetric::new())
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
            r#"{{"SCHEDULE_FEATURE_DIM":{}, "SCHEMA_VERSION":{}}}"#,
            SCHEDULE_FEATURE_DIM, SCHEMA_VERSION
        ),
    )
    .unwrap();
    println!("Saved {}.bpk and {}_meta.json", out_prefix, out_prefix);
}
