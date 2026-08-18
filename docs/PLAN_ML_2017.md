# Design Specification: Automated Heuristic Discovery & Pure-Rust Machine Learning for CASC

This specification outlines the integration of an Automated Heuristic Discovery (AHD) framework and a pure-Rust machine learning pipeline to optimize both **strategy portfolio selection** and **parallel worker allocation** under strict CASC competition budgets (120s–300s). This design operates entirely within the Rust ecosystem, utilizing `smartcore` for white-box rule induction and `burn` for multi-headed black-box neural networks.

---

## 1. Background & Technical Motivation

In the CASC competition environment, solvers are bound to an exactly 8-core hardware specification. Standard parallel portfolios run 8 concurrent strategies, sharing derived unit equalities. However:
1. **Memory Bus Saturation**: In highly equational domains (e.g., **UEQ**), heavy rewriting and pointer-chasing on terms can saturate the memory bus. Spawning 8 concurrent threads actually slows down individual search paths due to L3 cache thrashing.
2. **SAT-Splitting Contention**: In SAT-splitting domains (e.g., **EPR**), AVATAR spawns independent instances of the `CaDiCaL` SAT solver. Running 8 concurrent CDCL solvers leads to heavy context-switching and cache eviction.

By executing a dedicated pre-loop **Probing Engine** and utilizing a pure-Rust machine learning stack, we can dynamically configure both the **Optimal Portfolio** and the **Optimal Worker Count** (e.g., 1, 2, 4, or 8 threads) to maximize solved counts.

---

## 2. Structural Architecture & Pipeline

```
[Raw Problem File (.p)]
          │
          ▼
   A. Parser & CNF Lowering
          │
          ▼
┌──────────────────────────────────────────────┐
│  B. PROBING ENGINE (0.2s–0.5s Probe Run)      │
│     - Runs a fast KBO/LPO baseline search     │
│     - Captures dynamic inference speed        │
│     - Captures AVATAR splitting rates        │
└────────────────┬─────────────────────────────┘
                 │
                 ▼ (Combined 40+ Syntactic & Dynamic Feature Vector)
┌──────────────────────────────────────────────┐
│  C. PURE-RUST ML SELECTOR                    │
│     - WHITE-BOX: Smartcore Decision Tree      │
│                  (Auto-compiled to if-else)  │
│     - BLACK-BOX: Burn Multi-Headed MLP       │
└────────────────┬─────────────────────────────┘
                 │
                 ▼ (Assigned Portfolio & Worker Allocation)
┌──────────────────────────────────────────────┐
│  D. HIGH-SPEED SOUND AVX2 SOLVING ENGINE     │
│     - Parallel cooperative streaming if > 1   │
│     - Single-threaded sequential slicing if 1 │
└──────────────────────────────────────────────┘
```

---

## 3. Implementation Details by Phase

### Phase 1: Pre-Classification & Pruning of Trivial Problems
Standard TPTP releases contain thousands of very small, simple problems that are solved in $<0.1$ seconds. Training on these introduces severe selection bias, causing ML models to overfit to trivial structures.
*   **The Action**: Run `mrs-codex` across the entire TPTP v9.2.1 dataset with a 1-second timeout and high parallelism:
    ```bash
    cargo run --release -p mrs-codex -- \
      /mnt/wsl/CUsersfr22192WSLDatafastdatavhdx/TPTP-v9.2.1/ \
      --db tptp_preclass.db \
      --system mrs-preclass \
      --timeout 1 \
      --cmd "./target/release/mrs {file}" \
      --jobs 32
    ```
*   **The Output**: Generate a `nontrivial.list` file containing only the problems that timed out (unsolved) or took $\ge 0.1$ seconds to solve.

### Phase 2: Dual-Stage Feature Extraction & Probing Engine
We expand the feature extraction from 16 to 40+ dimensions to capture highly-granular signatures of the problem:
1.  **Syntactic Features**:
    *   *Equational Density*: Equations over total literals.
    *   *AC-Properties*: Count of Associative-Commutative operators.
    *   *Arity Signatures*: Function symbols of arity 0, 1, 2, and $\ge 3$.
2.  **Dynamic Probing Features**:
    *   Run a standard given-clause loop for **0.2s–0.5s** before resetting the state.
    *   *Inferences/Sec*: Processed/generated clauses per second (measures unification/memory latency).
    *   *AVATAR Splitting rate*: CaDiCaL splitting triggers per second.
    *   *Simplification efficiency*: Demodulations/subsumptions relative to generated clauses.

### Phase 3: Pure-Rust White-Box Learning (`smartcore` + Code Generation)
Instead of relying on Python/scikit-learn, we use the pure-Rust **`smartcore`** library to train a **Decision Tree Classifier** in `mrs-train`.
*   **The Setup**: Add `smartcore` to `crates/mrs-train/Cargo.toml`.
*   **Auto-Generated Code Generation**: After fitting the Decision Tree to optimize **solved counts**, we traverse the tree nodes in Rust and automatically generate a static Rust file (`src/adaptive_resource.rs`) containing nested `if-else` blocks. 
*   **Runtime Inference Overhead**: **0ms**. The generated code is compiled directly into the binary, executing with zero dependency overhead.

### Phase 4: Pure-Rust Black-Box Learning (`burn` MLP)
For complex interactions, we train a multi-headed deep learning model using `burn`:
*   **Architecture**:
    *   *Shared Encoder*: `Linear(40 -> 256) -> GELU -> Linear(256 -> 128)`
    *   *Portfolio Head*: `Linear(128 -> 5)` (Softmax logits over `casc_fne`, `casc_feq`, `casc_ueq`, `casc_epr`, `casc_icu`)
    *   *Worker Head*: `Linear(128 -> 4)` (Softmax logits over workers `[1, 2, 4, 8]`)
*   **Objective Function**: Multi-class Cross-Entropy Loss, optimized purely for solved counts.
*   **Compilation**: Compiled using the native CPU `NdArray` backend for high-speed inline evaluation at startup.

---

## 4. Evaluation and Verification Checklist

- [ ] **Phase 1: Pre-Classification & Filtering**
  - Run the `mrs-codex` pre-classification sweep on TPTP-v9.2.1.
  - Export `nontrivial.list` excluding $<0.1$s solves.
- [ ] **Phase 2: Probing Engine Integration**
  - Implement a configurable 0.2s–0.5s probing limit inside the main loop.
  - Integrate dynamic features (inferences/sec, AVATAR splits, demodulation ratios) into the feature vector.
- [ ] **Phase 3: White-Box Training with `smartcore`**
  - Integrate `smartcore` to train Decision Tree Classifiers.
  - Implement the code-generation module to compile the tree into `src/adaptive_resource.rs`.
- [ ] **Phase 4: Black-Box Training with `burn`**
  - Implement the multi-headed MLP in `crates/mrs-train`.
  - Train the weights with CUDA on the GPU server.
- [ ] **Phase 5: Soundness and Audit Sweeps**
  - Run the `mrs-bench` soundness audit to verify zero regressions.
  - Run comparative benchmark sweeps on the non-trivial list.
