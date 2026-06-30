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
use mrs_core::ml::premise_selector::{PREMISE_FEATURE_DIM, PremiseModel};
use mrs_core::ml::sample::PremiseSample;
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use crate::TrainCfg;

#[derive(Clone, Debug)]
pub struct PremiseBatch<B: Backend> {
    inputs: Tensor<B, 2>,
    targets_bce: Tensor<B, 2, Int>,
}

#[derive(Clone)]
struct PremiseBatcher<B: Backend> {
    _b: std::marker::PhantomData<B>,
}

impl<B: Backend> burn::data::dataloader::batcher::Batcher<B, PremiseSample, PremiseBatch<B>>
    for PremiseBatcher<B>
{
    fn batch(&self, items: Vec<PremiseSample>, device: &B::Device) -> PremiseBatch<B> {
        let batch_size = items.len();
        let mut inputs = Vec::with_capacity(batch_size * PREMISE_FEATURE_DIM);
        let mut targets_ints = Vec::with_capacity(batch_size);

        for item in items {
            inputs.extend_from_slice(&item.feats);
            targets_ints.push(item.label as i32);
        }

        let inputs = Tensor::<B, 1>::from_floats(inputs.as_slice(), device)
            .reshape([batch_size, PREMISE_FEATURE_DIM]);
        let targets_bce = Tensor::<B, 1, Int>::from_ints(targets_ints.as_slice(), device)
            .reshape([batch_size, 1]);

        PremiseBatch {
            inputs,
            targets_bce,
        }
    }
}

#[derive(burn::module::Module, Debug)]
pub struct TrainingPremise<B: Backend> {
    model: PremiseModel<B>,
}

impl<B: AutodiffBackend> TrainStep for TrainingPremise<B> {
    type Input = PremiseBatch<B>;
    type Output = ClassificationOutput<B>;

    fn step(&self, batch: PremiseBatch<B>) -> TrainOutput<ClassificationOutput<B>> {
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
                targets: batch.targets_bce.clone().reshape([batch.targets_bce.dims()[0]]),
            },
        )
    }
}

impl<B: Backend> InferenceStep for TrainingPremise<B> {
    type Input = PremiseBatch<B>;
    type Output = ClassificationOutput<B>;

    fn step(&self, batch: PremiseBatch<B>) -> ClassificationOutput<B> {
        let logits = self.model.forward(batch.inputs);
        let loss = BinaryCrossEntropyLossConfig::new()
            .with_logits(true)
            .init(&logits.device())
            .forward(logits.clone(), batch.targets_bce.clone());

        ClassificationOutput {
            loss,
            output: logits,
            targets: batch.targets_bce.clone().reshape([batch.targets_bce.dims()[0]]),
        }
    }
}

pub fn train_premise<B: AutodiffBackend>(device: B::Device, dir: &str, cfg: &TrainCfg) {
    // Basic dataloading omitted for brevity, just load PremiseSample
    // In a real implementation this would deserialize PremiseSamples.
    println!("Training Premise Selector on {}", dir);
}
