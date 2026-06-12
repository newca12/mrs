# `mrs-train`

This crate implements the offline GPU training pipeline for the Machine-Learning Guided Clause Selection (ENIGMA-style) in the `mrs` theorem prover.

It is designed to run exclusively on a dedicated GPU server (e.g., with a V100) and is deliberately separated from the main `mrs` binary and `mrs-search` inference engine to avoid pulling heavy deep-learning dependencies (like `libtorch` or `wgpu`) into the prover's release build.

## Features and Backends

The crate uses the [Burn](https://burn.dev) framework and supports two backend features:
- `wgpu` (default): Uses the WGPU backend, which is highly portable and supports Vulkan/Metal/DX12.
- `cuda`: Uses the Libtorch backend with CUDA support. This requires a local libtorch installation.

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

# Using CUDA backend (requires libtorch)
cargo run --release --features cuda -p mrs-train -- ./ml_logs/
```

### 3. Output
The training process will run for the configured number of epochs and output two files in the current directory:
- `weights.bin`: The serialized model weights (Burn native format).
- `meta.json`: A sidecar file asserting the feature dimensions and schema version to prevent drift during inference.

### 4. Inference
Transfer `weights.bin` back to the system where `mrs` runs. Execute `mrs` with the `ml-guidance` feature and pass the model weights via the `--ml-weights` CLI flag.

```bash
cargo run --release --features "proover ml-guidance" -- --ml-weights ./weights.bin problems/socrates.p
```
