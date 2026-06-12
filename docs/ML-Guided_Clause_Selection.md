# Plan: Machine-Learning Guided Clause Selection (ENIGMA-style)

This document outlines the detailed strategy and implementation plan for integrating an offline-trained, CPU-evaluated Multi-Layer Perceptron (MLP) using the **Burn** deep learning framework to guide given-clause selection in the `mrs` theorem prover.

> **Status:** design, not yet implemented. This revision reconciles the plan against the
> actual codebase. File:line anchors below refer to the current tree.

---

## 1. Background & Motivation

In first-order superposition theorem proving, the search space grows exponentially. `mrs` currently uses static heuristics (age/weight ratios, distance-to-goal) to prioritize clauses for selection — see `SelectionStrategy` (`crates/mrs-search/src/select.rs:14`) and the weight heuristic (`crates/mrs-search/src/weight.rs:83`).

By training a neural network on past successful refutation proofs, the prover can learn a domain-specific representation of "what a proof-relevant clause looks like" based on structural features and symbol distributions. This method (popularized by E Prover's ENIGMA) allows the prover to guide the search loop straight towards the refutation path, dramatically reducing the number of processed clauses and boosting the CASC solve rate.

### Locked design decisions

1. **Cross-problem generalization.** The model must generalize across *all* TPTP problems, not be retrained per problem. Consequence: symbol features hash **symbol name strings**, never per-problem `SymbolId`s (which are interned densely from 0 and are meaningless across problems).
2. **Dedicated extractor module.** Feature extraction is a new shared module, intentionally decoupled from the existing `mrs-index::FeatureVector` (`crates/mrs-index/src/fvi.rs:16`) used for subsumption filtering.
3. **All three phases** (logging → GPU training → CPU inference) are in scope.
4. **`mrs-train`** supports both **CUDA (Libtorch)** and **Wgpu** backends; it is built and run only on the offline remote V100 server (toolchain already provisioned there — no local Burn spike needed).
5. **Dedicated selection strategy.** Inference is exposed as a *new* `SelectionStrategy::MlGuided` variant. The proven static strategies and the `casc`/`default` schedule remain bit-identical when the feature is off.
6. **Log serialization** uses **`wincode`** (with a small CSV debug option), via a shared record struct so the encode (in `mrs`) and decode (in `mrs-train`) sides cannot drift.

---

## 2. Workspace & Crate Architecture

```
mrs (workspace root)
├── crates/mrs-core              <-- [Existing] adds ml/ module (features, model, sample) behind `ml` feature
├── crates/mrs-search            <-- [Existing] CPU inference (NdArray backend) behind `ml-guidance` feature
└── crates/mrs-train             <-- [New] V100-only training binary (Libtorch/CUDA or Wgpu backend)
```

1. **`mrs-core`:** Owns feature extraction, the Burn model (generic over `Backend`), and the shared serialization record. All gated behind a new off-by-default `ml` feature; `burn` is an *optional* dependency enabled only by that feature.
2. **`mrs-search`:** Imports the model from `mrs-core` and runs CPU-only inline inference via the Burn `NdArray` backend with **zero PCIe latency**. Gated behind a new off-by-default `ml-guidance` feature (which enables `mrs-core/ml` + `burn` with the `ndarray` backend).
3. **`mrs-train`:** Runs exclusively on the V100 server using `Libtorch`/`Wgpu`. Depends on `mrs-core` with `features = ["ml"]`, reusing the identical model and feature code. Its heavy GPU dependency tree never touches `mrs`/`mrs-search`.

### Crate-dependency constraint (resolved)

`SymbolConfig` lives in **`mrs-calculus`** (`crates/mrs-calculus/src/ordering.rs:20`), which already depends on `mrs-core`. Importing it *into* `mrs-core` would create a circular dependency. The extractor does not need `SymbolConfig`: cross-problem features require **symbol names** from `SymbolTable` (`crates/mrs-core/src/symbol.rs:43`), plus `IdClause` + `TermBank` — all already in `mrs-core`. Therefore the `ml` module lives entirely in `mrs-core` with no new crate edges.

> Off-by-default gating also protects the release build, which uses `lto = "fat"` + `codegen-units = 1` (`Cargo.toml:26`) — Burn must never leak into the default `mrs` build.

---

## 3. Detailed Technical Design

### A. Clause Feature Extraction (Bag of Symbols with Name-Hashing Trick)

Fixed-size `[f32; 128]` vector. The **single source of truth** for both data logging (Phase 1) and inference (Phase 3); training consumes only serialized vectors, so there is no second implementation to drift from.

```rust
// crates/mrs-core/src/ml/features.rs   (feature = "ml")
use crate::symbol::SymbolTable;
use crate::term_bank::{IdClause, TermBank};

pub const FEATURE_DIM: usize = 128;
pub const HASH_BUCKETS: usize = 120;     // buckets 8..128
pub const SCHEMA_VERSION: u32 = 1;       // bump on any layout change

/// Layout (all values normalized to roughly [0, 1]):
/// - [0] num_literals
/// - [1] num_positive_literals
/// - [2] num_negative_literals
/// - [3] total symbol weight (clause_weight, normalized)
/// - [4] max term depth (traversed over the TermBank)
/// - [5] total term size
/// - [6] distance-to-conjecture bucket (IdClause.distance; <100 => near-goal)
/// - [7] is_unit / is_horn flags packed
/// - [8..128] symbol-NAME hash buckets: for each symbol occurrence,
///            bucket = fxhash(symbol_name) % HASH_BUCKETS; counts then log/L2-normalized.
pub fn extract(clause: &IdClause, bank: &TermBank, symbols: &SymbolTable) -> [f32; FEATURE_DIM] {
    let mut f = [0.0f32; FEATURE_DIM];
    // ... structural features + name-hashed symbol histogram + normalization ...
    f
}
```

Notes vs. the original sketch:
- `IdClause` carries `distance: u32` (`crates/mrs-core/src/term_bank.rs:62`), not an "age" field. The "goal ancestor" signal is `distance < 100` (already used for goal-directed weighting, `crates/mrs-search/src/unprocessed.rs:79`).
- There is no per-clause "generation/age" stored on the clause; age is implicit in the FIFO `age_queue`. It is omitted unless we add it explicitly.
- Depth/size are traversed over the `TermBank` (mirroring `count_term_id`, `crates/mrs-index/src/fvi.rs:110`).
- Symbol features hash the **name string**, so feature index `k` means the same thing across every problem.

### B. The Burn MLP Model

Defined in `mrs-core` so it is shared verbatim by training and inference. `Clone` is derived so it can be shared as `Arc<ClauseClassifier<NdArray>>` across worker threads.

```rust
// crates/mrs-core/src/ml/model.rs   (feature = "ml")
use burn::module::Module;
use burn::nn::{Linear, LinearConfig};
use burn::tensor::backend::Backend;
use burn::tensor::Tensor;

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
```

Weights are saved/loaded as `weights.bin` via Burn's `BinFileRecorder`, accompanied by a `meta.json` sidecar recording `FEATURE_DIM`, `HASH_BUCKETS`, and `SCHEMA_VERSION`. Records are backend-agnostic, so weights trained under Libtorch load under `NdArray`.

### C. Shared serialization record

```rust
// crates/mrs-core/src/ml/sample.rs   (feature = "ml")
#[derive(wincode::Encode, wincode::Decode)] // exact crate API TBD at impl time
pub struct LabeledSample {
    pub label: f32,            // 1.0 = on refutation path, 0.0 = kept-but-unused
    pub feats: [f32; super::features::FEATURE_DIM],
}
```

Both the writer (`mrs`) and the reader (`mrs-train`) depend on this one definition, guaranteeing schema match. A CSV debug writer (`--ml-log-csv`) emits `label,f0,...,f127` for human inspection.

### D. Selection-strategy integration

`UnprocessedSet` (`crates/mrs-search/src/unprocessed.rs:46`) is **three parallel queues with lazy tombstone deletion**, not a single priority queue. Integration:

- Add a fourth `ml_queue: BinaryHeap<WeightWrapper>` keyed by a quantized blended priority.
- New variant `SelectionStrategy::MlGuided { ratio: u32, alpha: f32 }` (`crates/mrs-search/src/select.rs:14`). Like `AgeWeight`, every `ratio`-th iteration picks by age (preserving completeness); otherwise it pops from `ml_queue`.
- Blended priority (smaller = selected sooner):

$$\text{Priority}(C) = \alpha \cdot \widehat{\text{Weight}}(C) + (1 - \alpha) \cdot \big(1 - \sigma(\text{MLScore}(C))\big)$$

  * $\sigma$ = sigmoid; $\alpha \in [0,1]$, default `0.3`.
  * $\widehat{\text{Weight}}(C) = \text{weight} / (\text{weight} + K)$ (K ≈ 20) maps the unbounded `u32` weight into `[0,1)` so it is commensurate with the sigmoid term. (The original formula omitted this normalization and would have let weight dominate.)

The score is computed once at clause-push time (`crates/mrs-search/src/given_clause.rs:1068`) and cached in a new `scores: FxHashMap<ClauseId, f32>` on `SearchState` (`crates/mrs-search/src/state.rs:24`).

### E. Thread-sharing model

The portfolio runs strategies in parallel via `std::thread::scope` (`crates/mrs-search/src/strategy.rs:452`). `TermId`s are thread-local to each worker's `TermBank`, but `SymbolId`s and `Arc<SymbolConfig>` are global. The loaded model is read-only at inference, so it is shared as `Arc<ClauseClassifier<NdArray>>` cloned into each worker (same pattern as `Arc<SymbolConfig>`, `strategy.rs:460`) and stored as `SearchState.ml_model: Option<Arc<…>>`. It is **not** placed on `SearchConfig` (which is `Clone + Debug` and per-strategy).

---

## 4. Phased Implementation Plan

### Phase 0: Shared `ml` module in `mrs-core`
1. Add optional `burn` dep + `ml` feature to `crates/mrs-core/Cargo.toml`.
2. Implement `ml/features.rs`, `ml/model.rs`, `ml/sample.rs` with the constants and golden-test hooks above.

### Phase 1: Data Logging (offline trace collection)
1. Add a `--log-ml-data <dir>` flag (and `--ml-log-csv` debug toggle) to `mrs` (`src/main.rs:34`), threaded through `run_schedule` into each worker's `SearchState`.
2. On `SearchResult::Refutation(empty_id, _)`, **inside the worker before the `TermBank`/solver drop** (`crates/mrs-search/src/strategy.rs:514`):
   * Add `extract_proof_ids(empty_id, &clause_store)` (an `IdClause`-based ancestry walk mirroring `extract_proof`, `crates/mrs-proof/src/extract.rs:21`) to collect the **positive** set following `ClauseSource::Inference.parents`.
   * **Negative** set = `state.clause_store` ids (`crates/mrs-search/src/state.rs:34`) not in the positive set; apply a configurable negative subsample ratio (class imbalance).
   * Run shared `extract(...)` on each `IdClause` and append `LabeledSample`s (wincode, or CSV in debug) to a per-problem file.

### Phase 2: Create the `mrs-train` GPU Crate (V100-only)
1. Add `crates/mrs-train/` as a workspace member (binary), depending on `mrs-core` with `features = ["ml"]`.
2. Own cargo features `cuda` (Burn `libtorch`) and `wgpu`, mirroring Burn examples; pick one default.
3. Build a standard pipeline: Burn `Dataset` (wincode reader) → `Batcher` → BCE-with-logits loss + Adam, with class weighting / negative downsampling → `Learner`/`Trainer`.
4. Startup assertions: dataset feature width == `FEATURE_DIM`, `SCHEMA_VERSION` matches. Output `weights.bin` + `meta.json`.

### Phase 3: Inline CPU Inference integration (`ml-guidance` feature)
1. Add `ml-guidance` feature to `crates/mrs-search/Cargo.toml` enabling `mrs-core/ml` + `burn` (`ndarray` backend). Off by default.
2. Add `--ml-weights <path>` to `mrs`; `run_schedule` loads the `.bpk` once into `Arc<ClauseClassifier<NdArray>>` and clones the `Arc` into each worker (`strategy.rs:460`); checks `meta.json` schema.
3. Wire `SearchState.ml_model` + `scores` cache; implement `SelectionStrategy::MlGuided` and the `ml_queue` in `UnprocessedSet`; score at push (`given_clause.rs:1068`). Consider batched scoring of `new_clauses` if single-clause latency dominates.
4. Add a named schedule `ml` in `strategy::named` (register in `ALL` + `by_name`; leave `casc`/`default` untouched), selected when `--ml-weights` is supplied.

---

## 5. Verification & Testing

* **Determinism:** model produces identical logits across two loads of one `weights.bin` (unit test in `mrs-core`).
* **Feature alignment / zero drift:** a single shared `extract(...)` is used by logging and inference; pin it with a golden feature-vector test over a fixed `IdClause` fixture. `meta.json` `SCHEMA_VERSION` is checked at load in both `mrs-train` and `mrs-search` (mismatch = hard error).
* **Cross-backend load:** weights trained under Libtorch load and infer under `NdArray` (test on the server).
* **Speed:** micro-benchmark `extract + forward` on CPU (report throughput, not only µs/clause). Target overhead < 10 µs/clause. First silence the uncommented `eprintln!` tracing in `given_clause.rs` (e.g. `:381`, `:407`, `:442`) or measurements are meaningless.
* **No regression:** `cargo test --workspace`, `cargo clippy --all`, `cargo fmt --all` all green with `ml-guidance` both **off** (default) and **on**; the `casc` schedule must behave identically to today when the feature is off.

---

## 6. Open risks

1. **Build cost:** `mrs-train`'s GPU tree is large; the offline server build will be slow. Strict feature gating keeps it out of `mrs`/`mrs-search`.
2. **Label noise:** "kept-but-unused" negatives include clauses that might help on other runs; standard ENIGMA accepts this — expect modest signal.
3. **`wincode` schema coupling:** the writer and reader must use the same `wincode` version and the shared `LabeledSample` struct; enforced by the single definition in `mrs-core::ml::sample`.

---

## 7. Step-by-Step User Guide

This section provides a practical guide on how to collect data, train the model, and run `mrs` with ML-guided clause selection.

### Step 1: Collect Training Data

You need to run the `mrs` prover on a corpus of TPTP problems (e.g., from the CASC competition) to generate training data. To enable data logging, you must build the prover with the `ml` feature.

```bash
# Build the prover with the ML feature enabled
cargo build --release --features ml

# Run the prover on a problem and log the ML data
# This will output .wincode files containing positive and negative clause features
# to the specified directory.
export PROBLEM_NAME="socrates" # Optional: Provides a prefix for log files
./target/release/mrs --log-ml-data ./ml_logs problems/socrates.p
```

To debug the extracted features, you can output them as CSV instead of the optimized binary format:
```bash
./target/release/mrs --log-ml-data ./ml_logs --ml-log-csv problems/socrates.p
```

To collect data massively over an entire TPTP release directory, use the provided benchmark script. This script automatically handles parallelism and time limits:

```bash
# Example: Collect data across TPTP-v9.2.1 using 16 parallel jobs with a 300s timeout per problem
./crates/mrs-bench/collect_ml_data.sh /path/to/TPTP-v9.2.1 ./ml_logs 16 300
```

#### Distributed Collection Across Multiple Servers

If you have a cluster of servers available with a shared filesystem structure (where the `mrs` repository and `TPTP` directory paths are identical across all nodes), you can distribute the massive data collection task to dramatically reduce the wall-clock time:

1. Create a `servers.txt` file containing one SSH hostname/IP per line (e.g., `user@server1`, `user@server2`).
2. Run the distributed orchestration script from a central node:

```bash
# Example: Distribute TPTP-v9.2.1 collection across servers using 8 jobs per server and a 300s timeout
./crates/mrs-bench/distribute_ml_data.sh servers.txt /path/to/TPTP-v9.2.1 ./ml_logs_cluster 8 300
```

This script will seamlessly split the problem list, securely push work to the cluster nodes via SSH, run `collect_ml_data.sh` on each node independently, and finally `rsync` all the `.wincode` files back to your central `./ml_logs_cluster/data/` directory automatically.

### Step 2: Train the ML Model (on GPU Server)

Transfer your collected `ml_logs` directory to your GPU server (e.g., a V100 machine). Run the dedicated `mrs-train` crate to train the Burn-based MLP model.

```bash
# To train using the default WGPU backend:
cargo run --release -p mrs-train -- ./ml_logs/

# To train using the CUDA/Libtorch backend (if libtorch is installed):
cargo run --release --features cuda -p mrs-train -- ./ml_logs/
```

This will run the training loop (defaulting to 10 epochs) and will output:
* `weights.bin`: The serialized model weights.
* `meta.json`: The schema metadata to ensure compatibility during inference.

### Step 3: Run Inference with ML Guidance

Copy `weights.bin` back to the system where you intend to run the `mrs` prover.

To run the prover with the ML-guided strategy, compile the prover with the `ml-guidance` feature and pass the `--ml-weights` argument.

```bash
# Build the prover with ML-guided inference enabled
cargo build --release --features ml-guidance

# Run the prover, passing the path to the trained weights
./target/release/mrs --ml-weights ./weights.bin problems/socrates.p
```

When you specify `--ml-weights`, `mrs` defaults to the `ml` schedule, which utilizes `SelectionStrategy::MlGuided`. You can observe its behavior in parallel alongside standard static heuristics.
