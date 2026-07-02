# ML Preprocessing Tuning Guide: Zero-Code Optimizations & V100 Hardware Acceleration

This document outlines the zero-code tuning roadmap for the pre-loop ML preprocessing pipelines (Schedule Classifier and Premise Selector) across all non-equational and propositional CASC divisions (**FNE**, **EPU**, **EPS**, **ICU**). It also specifies how to maximize the acceleration footprint of our **NVIDIA V100 GPU server** to train highly robust, high-generalization networks.

---

## 1. Zero-Code Multi-Division Execution Roadmap

Our pre-loop preprocessing pipelines are fully modular. We can target and tune individual CASC divisions purely via command-line arguments and script parameters, without editing a single line of Rust code.

### A. FNE (Pure Resolution) — SInE + ML Cooperative Filter
*   **The Problem:** In pure resolution, proofs require deep, multi-step resolution trees. Our previous benchmark used an aggressive `--ml-prune 0.6` (discarding 40% of background axioms), which starved the search loop of crucial lemmas.
*   **The Zero-Code Solution:** Relax the keep ratio to a highly conservative **`0.85`** or **`0.90`** (retaining the top 85-90% of axioms) strictly for the FNE division.
*   **Harness Configuration (`crates/mrs-bench/systems/mrs-ml/invoke.sh`):**
    ```bash
    if [[ "${DIV_LOWER}" == "fne" ]]; then
        ARGS+=(--ml-prune 0.85 --ml-premise-weights "${PREMISE_WEIGHTS}")
    else
        ARGS+=(--ml-prune 0.6 --ml-premise-weights "${PREMISE_WEIGHTS}")
    fi
    ```
*   **How it works:** The global ML model filters out the grossest syntactic noise at startup. At the same time, each parallel worker thread retains its ability to apply its specialized, independent **SInE threshold filtering** on top. This creates a hybrid SInE-ML filter with maximum resolution safety and zero lemma starvation.

### B. EPU & EPS (Effectively Propositional) — Diverse SAT Splitting
*   **The Problem:** EPR problems contain only constants and variables (no functions of arity $\ge 1$). The search is driven entirely by **AVATAR** case-splitting coordinated by the CDCL SAT solver (CaDiCaL). Mismatched schedules cause the SAT solver to stall in redundant splitting loops.
*   **The Zero-Code Solution:** Retrain the **Schedule Classifier** specifically on the non-trivial EPR CASC problem list, configuring it to assign highly diverse parallel portfolios.
*   **Training Execution:**
    ```bash
    cargo run --release -p mrs-train -- --mode schedule --epochs 30 /path/to/epr_logs/ weights_schedule_epu
    ```
*   **How it works:** At runtime, the retrained model analyzes the structural density of the propositional clauses at startup and assigns a highly diverse, complementary set of SAT-splitting schedules across the 8 parallel cores (e.g. varying CaDiCaL branching and literal selection rules).

### C. ICU (Intensional Unit Equality) — Solving the 0-Solve Barrier
*   **The Zero-Code Solution:** Since ICU problems are equational, retraining the ICU Premise Selector and Schedule Classifier with our new **AC-indexing active** will allow the premise selector to cleanly prune background axioms. This provides the exact simplification needed to finally break through our historical 0-solve barrier in this division.

---

## 2. NVIDIA V100 GPU Acceleration Roadmap

Because our pre-loop preprocessing models are lightweight MLPs, standard training runs use very little VRAM and complete extremely fast on a V100. Since these models run **only once at startup**, we are not bound by the tight microsecond latency limits of the inner loop. We can leverage the massive hardware capacity of the V100 to train much more powerful networks using these four strategies:

### A. Multi-Fold Negative Sampling (`--neg-per-pos`) — Maximizing Data Signal
In our premise datasets, positive samples (clauses that made it into the final proof) are extremely sparse compared to the flood of negatives (unused active clauses).
*   **The Tuning:** Increase **`--neg-per-pos` to `5` or `8`**.
*   **Why it works:** Instead of discarding up to 98% of our hard negatives to maintain a 1:1 balance, this exposes the network to $5\times$ or $8\times$ more **hard negatives** (active "look-alike" clauses that derail the search). It expands our training dataset size by up to 500%, utilizing the V100's processing bandwidth to train a much sharper decision boundary.
*   **Command:**
    ```bash
    cargo run --release -p mrs-train -- --neg-per-pos 5 --epochs 50 ./ml_logs_ueq weights_premise_ueq
    ```

### B. Increased Max Epochs and Relaxed Early Stopping Patience
*   **The Tuning:** Increase `--epochs` to **`100`** or **`150`**, and increase the early stopping patience in `premise_train.rs` to **`10`** or **`15`** epochs.
*   **Why it works:** Because epoch iterations are incredibly fast on the V100, running more epochs with a slightly lower, more stable learning rate (e.g., `3e-4` or `1e-4` instead of `1e-3`) lets the Adam optimizer find much flatter, more generalizable local minima.

### C. Increasing Model Capacity (Wider Hidden Layers)
Our current `PremiseModel` is quite narrow: `Linear(24 → 64)` $\to$ `Linear(64 → 32)` $\to$ `Linear(32 → 16)`.
*   **The Tuning:** Double or quadruple the layer widths inside `crates/mrs-core/src/ml/model.rs`:
    *   `Linear(24 → 256)` $\to$ `Linear(256 → 128)` $\to$ `Linear(128 → 64)` $\to$ `Linear(64 → 1)`.
*   **Why it works:** CPU evaluation of a 256-wide MLP takes under **1 millisecond** at startup, which is completely undetectable at runtime. However, it gives the network the capacity to learn highly complex structural features of CASC-grade algebraic axioms.

### D. Massive Batch Sizes
*   **The Tuning:** Increase the training and validation batch sizes in `premise_train.rs` and `schedule_train.rs` to **`4096` or `8192`**.
*   **Why it works:** A batch size of 2048 on a 24-dimension vector is processed in microseconds on a V100, wasting performance on GPU launch overhead. Scaling batch sizes to `8192` fully saturates the V100's memory bus and drastically accelerates training epochs.
