use burn::module::Module;
use burn::nn::{Linear, LinearConfig};
use burn::tensor::{Device, Tensor};

#[derive(Module, Debug)]
pub struct ClauseClassifier {
    layer1: Linear,
    layer2: Linear,
    layer3: Linear,
    output: Linear,
}

impl ClauseClassifier {
    pub fn new(device: &Device) -> Self {
        Self {
            layer1: LinearConfig::new(128, 64).init(device),
            layer2: LinearConfig::new(64, 32).init(device),
            layer3: LinearConfig::new(32, 16).init(device),
            output: LinearConfig::new(16, 1).init(device),
        }
    }

    pub fn forward(&self, x: Tensor<2>) -> Tensor<2> {
        let x = burn::tensor::activation::gelu(self.layer1.forward(x));
        let x = burn::tensor::activation::gelu(self.layer2.forward(x));
        let x = burn::tensor::activation::gelu(self.layer3.forward(x));
        self.output.forward(x) // raw logit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use burn::tensor::Device;

    #[test]
    fn test_model_forward_shape() {
        let device = Device::flex();
        let classifier = ClauseClassifier::new(&device);

        // Batch size of 2, FEATURE_DIM of 128
        let x = Tensor::<2>::zeros([2, 128], &device);
        let out = classifier.forward(x);

        assert_eq!(out.shape().dims(), [2, 1]);
    }
}
