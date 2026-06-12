use burn::module::Module;
use burn::nn::{Linear, LinearConfig};
use burn::tensor::Tensor;
use burn::tensor::backend::Backend;

#[derive(Module, Debug)]
pub struct ClauseClassifier<B: Backend> {
    layer1: Linear<B>,
    layer2: Linear<B>,
    layer3: Linear<B>,
    output: Linear<B>,
}

impl<B: Backend> ClauseClassifier<B> {
    pub fn new(device: &B::Device) -> Self {
        Self {
            layer1: LinearConfig::new(128, 64).init(device),
            layer2: LinearConfig::new(64, 32).init(device),
            layer3: LinearConfig::new(32, 16).init(device),
            output: LinearConfig::new(16, 1).init(device),
        }
    }

    pub fn forward(&self, x: Tensor<B, 2>) -> Tensor<B, 2> {
        let x = burn::tensor::activation::gelu(self.layer1.forward(x));
        let x = burn::tensor::activation::gelu(self.layer2.forward(x));
        let x = burn::tensor::activation::gelu(self.layer3.forward(x));
        self.output.forward(x) // raw logit
    }
}
