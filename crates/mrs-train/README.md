# `mrs-train`

This crate implements the offline GPU training pipeline for the Machine-Learning Guided Clause Selection (ENIGMA-style) in the `mrs` theorem prover.

It is designed to run on a dedicated GPU server and is deliberately separated from the main `mrs` binary and `mrs-search` inference engine to avoid pulling heavy deep-learning dependencies into the prover's default release build.

## Features and Backends

The crate uses the [Burn](https://burn.dev) framework and supports these backend features:
- `wgpu` (default): Uses the WGPU backend, which is highly portable and supports Vulkan/Metal/DX12.
- `cuda`: Uses Burn's CUDA backend.
- `ndarray`: Uses Burn's Flex CPU backend for local validation.

## Usage

### 1. Data Collection
First, collect training data using the main `mrs` binary with the `ml` feature enabled and the `--log-ml-data` flag. This will generate `.wincode` files containing serialized `LabeledSample` feature vectors.

```bash
cargo run --release --features ml -- --log-ml-data ./ml_logs/ problems/socrates.p
```

### 2. Training
Transfer the generated `ml_logs/` directory to the GPU server. Then, run `mrs-train` pointing to that directory.

```bash
# Using default WGPU backend
cargo run --release -p mrs-train -- ./ml_logs/

# Using CUDA backend
cargo run --release --features cuda -p mrs-train -- ./ml_logs/

# Using CPU backend
cargo run --release -p mrs-train --no-default-features --features ndarray -- ./ml_logs/
```

### 3. Output
The training process will run for the configured number of epochs and output two files in the current directory:
- `weights.bpk`: The serialized model weights in Burnpack format.
- `meta.json`: A sidecar file asserting the feature dimensions and schema version to prevent drift during inference.

### 4. Inference
Transfer `weights.bpk` back to the system where `mrs` runs. Execute `mrs` with the `ml-guidance` feature and pass the model weights via the `--ml-weights` CLI flag. Burn 0.22 uses Burnpack and cannot directly load model files produced by older Burn recorder formats.

```bash
cargo run --release --features "proover ml-guidance" -- --ml-weights ./weights.bpk problems/socrates.p
```
