use burn::data::dataloader::DataLoaderBuilder;
use burn::data::dataset::InMemDataset;
use burn::lr_scheduler::constant::ConstantLr;
use burn::nn::loss::BinaryCrossEntropyLossConfig;
use burn::optim::AdamConfig;
use burn::prelude::*;
use burn::record::BinFileRecorder;
use burn::tensor::backend::AutodiffBackend;
use burn::train::metric::LossMetric;
use burn::train::metric::store::{Aggregate, Direction, Split};
use burn::train::{
    ClassificationOutput, InferenceStep, Learner, MetricEarlyStoppingStrategy, StoppingCondition,
    SupervisedTraining, TrainOutput, TrainStep,
};
use mrs_core::ml::features::{FEATURE_DIM, SCHEMA_VERSION};
use mrs_core::ml::model::ClauseClassifier;
use mrs_core::ml::sample::LabeledSample;
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;

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
            while let Ok(sample) = wincode::deserialize_from(&mut std_read) {
                samples.push(sample);
            }
        }
    }

    samples
}

/// Configuration for a single training run.
struct TrainCfg {
    epochs: usize,
    val_split: f32,
    neg_per_pos: usize,
    out_prefix: String,
}

/// Split samples into (positives, negatives) by their binary label.
fn split_by_label(samples: &[LabeledSample]) -> (Vec<LabeledSample>, Vec<LabeledSample>) {
    let mut pos = Vec::new();
    let mut neg = Vec::new();
    for s in samples {
        if s.label > 0.5 {
            pos.push(s.clone());
        } else {
            neg.push(s.clone());
        }
    }
    (pos, neg)
}

fn train<B: AutodiffBackend>(device: B::Device, samples: &[LabeledSample], cfg: &TrainCfg) {
    let (mut pos, mut neg) = split_by_label(samples);
    let n_pos = pos.len();
    let n_neg_raw = neg.len();
    println!(
        "Class balance (raw): positives={} negatives={} ({:.4}% positive)",
        n_pos,
        n_neg_raw,
        100.0 * n_pos as f64 / (n_pos + n_neg_raw).max(1) as f64
    );

    if n_pos == 0 {
        eprintln!("ERROR: no positive (proof-clause) samples found; cannot train.");
        std::process::exit(1);
    }

    let mut rng = StdRng::seed_from_u64(42);

    // Rebalance: keep all positives, subsample negatives to neg_per_pos × positives.
    neg.shuffle(&mut rng);
    let keep_neg = (n_pos * cfg.neg_per_pos).min(neg.len());
    neg.truncate(keep_neg);
    println!(
        "After rebalancing to {}:1 → positives={} negatives={} (total={})",
        cfg.neg_per_pos,
        n_pos,
        neg.len(),
        n_pos + neg.len()
    );

    // Stratified train/validation split (hold out val_split of each class).
    pos.shuffle(&mut rng);
    neg.shuffle(&mut rng);
    let pos_val = ((pos.len() as f32) * cfg.val_split) as usize;
    let neg_val = ((neg.len() as f32) * cfg.val_split) as usize;
    let (pos_v, pos_t) = pos.split_at(pos_val);
    let (neg_v, neg_t) = neg.split_at(neg_val);

    let mut train_set: Vec<LabeledSample> = pos_t.iter().chain(neg_t).cloned().collect();
    let mut valid_set: Vec<LabeledSample> = pos_v.iter().chain(neg_v).cloned().collect();
    train_set.shuffle(&mut rng);
    valid_set.shuffle(&mut rng);
    println!(
        "Split: train={} valid={} (val_split={})",
        train_set.len(),
        valid_set.len(),
        cfg.val_split
    );

    let dataset_train = InMemDataset::new(train_set);
    let dataset_valid = InMemDataset::new(valid_set.clone());

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
        .build(std::sync::Arc::new(dataset_valid));

    let optim = AdamConfig::new();
    let lr_scheduler = ConstantLr::new(1e-3);

    let learner_config = Learner::new(
        TrainingClassifier {
            model: ClauseClassifier::<B>::new(&device),
        },
        optim.init(),
        lr_scheduler,
    );

    let supervised = SupervisedTraining::new(
        format!("{}.artifacts", cfg.out_prefix),
        dataloader_train,
        dataloader_valid,
    )
    .metric_train_numeric(LossMetric::new())
    .metric_valid_numeric(LossMetric::new())
    .with_checkpointing_strategy(burn::train::checkpoint::KeepLastNCheckpoints::new(2))
    .num_epochs(cfg.epochs)
    .early_stopping(MetricEarlyStoppingStrategy::new(
        &LossMetric::<B>::new(),
        Aggregate::Mean,
        Direction::Lowest,
        Split::Valid,
        StoppingCondition::NoImprovementSince { n_epochs: 5 },
    ))
    .summary();

    let learning_result = supervised.launch(learner_config);
    let model_trained = learning_result.model;

    // Manual evaluation on the held-out validation set: with class imbalance,
    // loss/accuracy alone hide a degenerate (near-constant) model, so we report
    // AUC and positive-class precision/recall plus score spread.
    evaluate::<B>(&model_trained.model, &valid_set, &device);

    // Save the INNER ClauseClassifier directly so the record matches exactly
    // what the prover loads at inference time (ClauseClassifier, not the
    // TrainingClassifier wrapper).
    let recorder = BinFileRecorder::<burn::record::HalfPrecisionSettings>::default();
    model_trained
        .model
        .save_file(cfg.out_prefix.as_str(), &recorder)
        .expect("Trained model should be saved successfully");

    std::fs::write(
        format!("{}_meta.json", cfg.out_prefix),
        format!(
            r#"{{"FEATURE_DIM":{}, "HASH_BUCKETS":{}, "SCHEMA_VERSION":{}}}"#,
            FEATURE_DIM,
            mrs_core::ml::features::HASH_BUCKETS,
            SCHEMA_VERSION
        ),
    )
    .unwrap();

    println!(
        "Saved {}.bin and {}_meta.json",
        cfg.out_prefix, cfg.out_prefix
    );
}

/// Run the trained model over the validation set and print discriminative
/// metrics (AUC, accuracy, positive-class precision/recall, score spread).
fn evaluate<B: AutodiffBackend>(
    model: &ClauseClassifier<B::InnerBackend>,
    valid: &[LabeledSample],
    device: &B::Device,
) {
    if valid.is_empty() {
        println!("Validation set empty — skipping evaluation.");
        return;
    }

    let mut scored: Vec<(f32, f32)> = Vec::with_capacity(valid.len()); // (prob, label)
    for chunk in valid.chunks(4096) {
        let bs = chunk.len();
        let mut inp = Vec::with_capacity(bs * FEATURE_DIM);
        for s in chunk {
            inp.extend_from_slice(&s.feats);
        }
        let t = Tensor::<B::InnerBackend, 1>::from_floats(inp.as_slice(), device)
            .reshape([bs, FEATURE_DIM]);
        let logits = model.forward(t);
        let vals: Vec<f32> = logits.into_data().to_vec::<f32>().expect("logits to vec");
        for (i, s) in chunk.iter().enumerate() {
            let p = 1.0 / (1.0 + (-vals[i]).exp());
            scored.push((p, s.label));
        }
    }

    let n = scored.len();
    let n_pos = scored.iter().filter(|x| x.1 > 0.5).count();
    let n_neg = n - n_pos;

    // Accuracy / precision / recall at threshold 0.5.
    let (mut tp, mut fp, mut fn_, mut tn) = (0u64, 0u64, 0u64, 0u64);
    let (mut sum_pos, mut sum_neg) = (0.0f64, 0.0f64);
    for &(p, label) in &scored {
        let pred_pos = p > 0.5;
        let is_pos = label > 0.5;
        match (pred_pos, is_pos) {
            (true, true) => tp += 1,
            (true, false) => fp += 1,
            (false, true) => fn_ += 1,
            (false, false) => tn += 1,
        }
        if is_pos {
            sum_pos += p as f64;
        } else {
            sum_neg += p as f64;
        }
    }
    let accuracy = (tp + tn) as f64 / n as f64;
    let precision = if tp + fp > 0 {
        tp as f64 / (tp + fp) as f64
    } else {
        f64::NAN
    };
    let recall = if tp + fn_ > 0 {
        tp as f64 / (tp + fn_) as f64
    } else {
        f64::NAN
    };

    // AUC via the rank-sum (Mann–Whitney U) statistic.
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&a, &b| scored[a].0.partial_cmp(&scored[b].0).unwrap());
    let mut rank_sum_pos = 0.0f64;
    for (rank, &i) in idx.iter().enumerate() {
        if scored[i].1 > 0.5 {
            rank_sum_pos += rank as f64 + 1.0;
        }
    }
    let auc = if n_pos > 0 && n_neg > 0 {
        (rank_sum_pos - (n_pos as f64) * (n_pos as f64 + 1.0) / 2.0) / (n_pos as f64 * n_neg as f64)
    } else {
        f64::NAN
    };

    let mean_pos = if n_pos > 0 {
        sum_pos / n_pos as f64
    } else {
        f64::NAN
    };
    let mean_neg = if n_neg > 0 {
        sum_neg / n_neg as f64
    } else {
        f64::NAN
    };

    println!("================ Validation metrics ================");
    println!("  samples={} (pos={} neg={})", n, n_pos, n_neg);
    println!("  AUC                = {:.4}   (0.5 = no signal)", auc);
    println!("  accuracy@0.5       = {:.4}", accuracy);
    println!("  precision (pos)    = {:.4}", precision);
    println!("  recall (pos)       = {:.4}", recall);
    println!(
        "  mean prob: pos={:.4}  neg={:.4}  (gap={:.4}; larger = better separation)",
        mean_pos,
        mean_neg,
        mean_pos - mean_neg
    );
    println!("====================================================");
}

fn parse_args() -> (String, TrainCfg, bool) {
    let args: Vec<String> = std::env::args().collect();
    let mut epochs = 30usize;
    let mut val_split = 0.15f32;
    let mut neg_per_pos = 1usize;
    let mut stats_only = false;
    let mut positionals: Vec<String> = Vec::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--epochs" => {
                i += 1;
                epochs = args[i].parse().expect("--epochs expects an integer");
            }
            "--val-split" => {
                i += 1;
                val_split = args[i].parse().expect("--val-split expects a float");
            }
            "--neg-per-pos" => {
                i += 1;
                neg_per_pos = args[i].parse().expect("--neg-per-pos expects an integer");
            }
            "--stats-only" => stats_only = true,
            other => positionals.push(other.to_string()),
        }
        i += 1;
    }

    if positionals.is_empty() {
        eprintln!(
            "Usage: mrs-train [--epochs N] [--val-split F] [--neg-per-pos R] [--stats-only] <log_dir> [out_prefix]"
        );
        std::process::exit(1);
    }
    let log_dir = positionals[0].clone();
    let out_prefix = positionals
        .get(1)
        .cloned()
        .unwrap_or_else(|| "weights".to_string());

    (
        log_dir,
        TrainCfg {
            epochs,
            val_split,
            neg_per_pos,
            out_prefix,
        },
        stats_only,
    )
}

fn main() {
    let (log_dir, cfg, stats_only) = parse_args();

    let samples = load_dataset(&log_dir);
    println!("Loaded dataset with {} samples", samples.len());

    if stats_only {
        let (pos, neg) = split_by_label(&samples);
        println!(
            "Class balance: positives={} negatives={} ({:.4}% positive, neg:pos ≈ {:.1}:1)",
            pos.len(),
            neg.len(),
            100.0 * pos.len() as f64 / samples.len().max(1) as f64,
            neg.len() as f64 / pos.len().max(1) as f64
        );
        return;
    }

    #[cfg(feature = "cuda")]
    {
        println!("Using CUDA (LibTorch) Backend");
        type MyBackend = burn::backend::libtorch::LibTorch;
        type MyAutodiffBackend = burn::backend::autodiff::Autodiff<MyBackend>;
        train::<MyAutodiffBackend>(
            burn::backend::libtorch::LibTorchDevice::Cuda(0),
            &samples,
            &cfg,
        );
    }

    #[cfg(all(feature = "wgpu", not(feature = "cuda")))]
    {
        println!("Using Wgpu Backend");
        type MyBackend = burn::backend::wgpu::Wgpu;
        type MyAutodiffBackend = burn::backend::autodiff::Autodiff<MyBackend>;
        train::<MyAutodiffBackend>(burn::backend::wgpu::WgpuDevice::default(), &samples, &cfg);
    }

    #[cfg(all(feature = "ndarray", not(any(feature = "cuda", feature = "wgpu"))))]
    {
        println!("Using NdArray (CPU) Backend");
        type MyBackend = burn::backend::ndarray::NdArray;
        type MyAutodiffBackend = burn::backend::autodiff::Autodiff<MyBackend>;
        train::<MyAutodiffBackend>(burn::backend::ndarray::NdArrayDevice::Cpu, &samples, &cfg);
    }

    #[cfg(not(any(feature = "cuda", feature = "wgpu", feature = "ndarray")))]
    {
        let _ = (&samples, &cfg);
        eprintln!("No backend feature selected. Use --features cuda | wgpu | ndarray");
    }
}
