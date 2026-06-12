use burn::data::dataloader::DataLoaderBuilder;
use burn::data::dataset::InMemDataset;
use burn::lr_scheduler::constant::ConstantLr;
use burn::nn::loss::BinaryCrossEntropyLossConfig;
use burn::optim::AdamConfig;
use burn::prelude::*;
use burn::record::BinFileRecorder;
use burn::tensor::backend::AutodiffBackend;
use burn::train::{
    ClassificationOutput, InferenceStep, Learner, SupervisedTraining, TrainOutput, TrainStep,
};
use mrs_core::ml::features::{FEATURE_DIM, SCHEMA_VERSION};
use mrs_core::ml::model::ClauseClassifier;
use mrs_core::ml::sample::LabeledSample;

#[derive(Clone, Debug)]
pub struct Batch<B: Backend> {
    inputs: Tensor<B, 2>,
    targets_bce: Tensor<B, 2, Int>,
    targets_class: Tensor<B, 1, Int>,
}

#[derive(Clone)]
struct Batcher<B: Backend> {
    _b: std::marker::PhantomData<B>,
}

impl<B: Backend> burn::data::dataloader::batcher::Batcher<B, LabeledSample, Batch<B>>
    for Batcher<B>
{
    fn batch(&self, items: Vec<LabeledSample>, device: &B::Device) -> Batch<B> {
        let batch_size = items.len();
        let mut inputs = Vec::with_capacity(batch_size * FEATURE_DIM);
        let mut targets_ints = Vec::with_capacity(batch_size);

        for item in items {
            inputs.extend_from_slice(&item.feats);
            targets_ints.push(item.label as i32);
        }

        let inputs = Tensor::<B, 1>::from_floats(inputs.as_slice(), device)
            .reshape([batch_size, FEATURE_DIM]);
        let targets_bce = Tensor::<B, 1, Int>::from_ints(targets_ints.as_slice(), device)
            .reshape([batch_size, 1]);
        let targets_class =
            Tensor::<B, 1, Int>::from_ints(targets_ints.as_slice(), device).reshape([batch_size]);

        Batch {
            inputs,
            targets_bce,
            targets_class,
        }
    }
}

#[derive(burn::module::Module, Debug)]
pub struct TrainingClassifier<B: Backend> {
    model: ClauseClassifier<B>,
}

impl<B: AutodiffBackend> TrainStep for TrainingClassifier<B> {
    type Input = Batch<B>;
    type Output = ClassificationOutput<B>;

    fn step(&self, batch: Batch<B>) -> TrainOutput<ClassificationOutput<B>> {
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

impl<B: Backend> InferenceStep for TrainingClassifier<B> {
    type Input = Batch<B>;
    type Output = ClassificationOutput<B>;

    fn step(&self, batch: Batch<B>) -> ClassificationOutput<B> {
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

fn load_dataset(dir: &str) -> Vec<LabeledSample> {
    let mut samples = Vec::new();

    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("wincode") {
            let mut file = std::fs::File::open(path).expect("Failed to open wincode file");
            let mut std_read = wincode::io::std_read::ReadAdapter::new(&mut file);
            let mut data = Vec::new();
            while let Ok(sample) = wincode::deserialize_from(&mut std_read) {
                data.push(sample);
            }
            samples.extend(data);
        }
    }

    samples
}

pub fn train<B: AutodiffBackend>(device: B::Device, data_dir: &str, out_prefix: &str) {
    let samples = load_dataset(data_dir);
    println!("Loaded dataset with {} samples", samples.len());

    let dataset_train = InMemDataset::new(samples.clone());
    let dataset_valid = InMemDataset::new(samples);

    let batcher_train = Batcher::<B> {
        _b: std::marker::PhantomData,
    };
    let batcher_valid = Batcher::<B::InnerBackend> {
        _b: std::marker::PhantomData,
    };

    let dataloader_train = DataLoaderBuilder::new(batcher_train)
        .batch_size(2048)
        .shuffle(42)
        .num_workers(4)
        .build(std::sync::Arc::new(dataset_train));

    let dataloader_valid = DataLoaderBuilder::new(batcher_valid)
        .batch_size(2048)
        .shuffle(42)
        .num_workers(4)
        .build(std::sync::Arc::new(dataset_valid)); // Re-using dataset for simplicity, usually you'd split

    let optim = AdamConfig::new();

    let lr_scheduler = ConstantLr::new(1e-3);

    let learner_config = Learner::new(
        TrainingClassifier {
            model: ClauseClassifier::<B>::new(&device),
        },
        optim.init(),
        lr_scheduler,
    );

    let supervised = SupervisedTraining::new(".", dataloader_train, dataloader_valid)
        .metric_train_numeric(burn::train::metric::LossMetric::new())
        .metric_valid_numeric(burn::train::metric::LossMetric::new())
        .with_checkpointing_strategy(burn::train::checkpoint::KeepLastNCheckpoints::new(2))
        .summary();

    let learning_result = supervised.launch(learner_config);
    let model_trained = learning_result.model;

    // Save
    let recorder = BinFileRecorder::<burn::record::HalfPrecisionSettings>::default();
    model_trained
        .save_file(out_prefix, &recorder)
        .expect("Trained model should be saved successfully");

    std::fs::write(
        format!("{}_meta.json", out_prefix),
        format!(
            r#"{{"FEATURE_DIM":{}, "HASH_BUCKETS":{}, "SCHEMA_VERSION":{}}}"#,
            FEATURE_DIM,
            mrs_core::ml::features::HASH_BUCKETS,
            SCHEMA_VERSION
        ),
    )
    .unwrap();

    println!("Saved {}.bin and {}_meta.json", out_prefix, out_prefix);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <log_dir> [out_prefix]", args[0]);
        std::process::exit(1);
    }
    let log_dir = &args[1];
    let out_prefix = if args.len() > 2 { &args[2] } else { "weights" };

    #[cfg(feature = "cuda")]
    {
        println!("Using CUDA Backend");
        type MyBackend = burn::backend::libtorch::LibTorch;
        type MyAutodiffBackend = burn::backend::autodiff::Autodiff<MyBackend>;
        train::<MyAutodiffBackend>(
            burn::backend::libtorch::LibTorchDevice::Cuda(0),
            log_dir,
            out_prefix,
        );
    }

    #[cfg(feature = "wgpu")]
    {
        println!("Using Wgpu Backend");
        type MyBackend = burn::backend::wgpu::Wgpu;
        type MyAutodiffBackend = burn::backend::autodiff::Autodiff<MyBackend>;
        train::<MyAutodiffBackend>(
            burn::backend::wgpu::WgpuDevice::default(),
            log_dir,
            out_prefix,
        );
    }

    #[cfg(not(any(feature = "cuda", feature = "wgpu")))]
    {
        eprintln!("No GPU feature selected. Use --features cuda or --features wgpu");
    }
}
