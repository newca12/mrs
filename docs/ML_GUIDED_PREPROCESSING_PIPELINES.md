# Design Specification: ML Preprocessing Pipelines (Pre-Loop Preprocessing)

This specification outlines the integration of two complementary, high-performance pre-loop machine learning components designed to maximize the proving throughput of the `mrs` theorem prover. Both models are trained offline using the **Burn** deep learning framework and run inline on CPU using the fast `NdArray` backend.

---

## 1. Background & Technical Motivation

In first-order theorem proving (CASC competition environment), solvers are bound by rigid CPU-core limits and strict per-problem wall-clock budgets (e.g., 240 or 300 seconds).

Previous attempts to integrate in-loop Machine Learning (ENIGMA-style given-clause scoring) degraded solver performance by up to 45% due to:
1. **In-Loop Latency**: Normalizing features and evaluating a Multi-Layer Perceptron (MLP) within the tight given-clause loop adds significant overhead (10–15 microseconds per clause), reducing total inferences/second.
2. **Loss of Portfolio Diversity**: Homogeneous schedules executing near-identical neural models across all CPU cores destroy the orthogonal diversity of static, highly optimized KBO/LPO heuristics.
3. **Exploration Starvation**: The network, trained on hindsight proof histories, over-prioritizes proof-relevant clauses and starves the prover of the intermediate exploration steps required to discover the proof.

### The Solution: Pre-Loop Preprocessing
By moving ML execution entirely **before** the main given-clause loop, we completely solve these bottlenecks:
* **Zero Loop Overhead**: Neural inference runs exactly once (at startup or parse-time).
* **Undiminished Portfolio Power**: The core superposition engine runs at 100% native speeds, leveraging sound AVX2 SIMD vectorization and highly-tuned static portfolios.
* **Dual-Tier Synergy**: Premise selection simplifies the problem space, and the schedule classifier selects the optimal execution strategy for that simplified space.

---

## 2. Structural Pipelines & Architectures

```
[Raw Problem File (.p)]
          │
          ▼
   A. Parser & CNF Lowering
          │
          ▼
┌──────────────────────────────────────────────┐
│  B. PARADIGM B: ML PREMISE SELECTION         │
│     - Scores axioms relative to conjecture   │
│     - Prunes Passive Set to Top N premises   │
└────────────────┬─────────────────────────────┘
                 │
                 ▼ (Compact, highly-relevant problem)
┌──────────────────────────────────────────────┐
│  C. PARADIGM A: STARTUP SCHEDULE CLASSIFIER   │
│     - Extracts structural/signature features │
│     - Selects optimal parallel static schedule│
└────────────────┬─────────────────────────────┘
                 │
                 ▼ (Assigned FNE/FEQ/UEQ schedule)
┌──────────────────────────────────────────────┐
│  D. HIGH-SPEED SOUND AVX2 SOLVING ENGINE     │
│     - Native in-loop execution (Age/Weight)  │
│     - Zero neural inner-loop evaluation      │
└──────────────────────────────────────────────┘
```

---

## 3. Paradigm A: Startup Schedule Classifier

### A. Architectural Role
The Schedule Classifier predicts which of our 5 highly-tuned parallel portfolios (`casc_fne`, `casc_feq`, `casc_ueq`, `casc_epr`, or `casc_icu`) is mathematically most likely to solve the problem, given the global structural signature.

### B. Feature Extraction Vector (`[f32; 16]`)
At startup, after CNF lowering but before solver initialization, the prover extracts a compact, size-16 global structural feature vector:
1. `f[0]`  = Total number of initial clauses (normalized: $\tanh(\text{count} / 1000)$).
2. `f[1]`  = Total number of unit clauses ratio ($\text{unit\_count} / \text{total\_count}$).
3. `f[2]`  = Horn clause ratio ($\text{horn\_count} / \text{total\_count}$).
4. `f[3]`  = Equation-free clause ratio ($\text{equality\_free\_count} / \text{total\_count}$).
5. `f[4]`  = Pure equality clause ratio ($\text{pure\_equality\_count} / \text{total\_count}$).
6. `f[5]`  = Average number of literals per clause.
7. `f[6]`  = Maximum literal count in a single clause.
8. `f[7]`  = Average term depth.
9. `f[8]`  = Maximum term depth.
10. `f[9]`  = Functor count ratio (function symbols / total symbols).
11. `f[10]` = Predicate count ratio (predicate symbols / total symbols).
12. `f[11]` = Skolem symbol ratio (Skolem constants / total symbols).
13. `f[12]` = Conjecture clause ratio ($\text{conjecture\_clauses} / \text{total\_clauses}$).
14. `f[13]` = Average variables per clause.
15. `f[14]` = Has equality flag (`1.0` if any equality predicate exists, `0.0` otherwise).
16. `f[15]` = Is unit-only equality flag (`1.0` if UEQ, `0.0` otherwise).

### C. Neural Network Model (Classifier MLP)
* **Input Dimension**: 16
* **Hidden Layers**: Linear(16 → 32, ReLU) → Linear(32 → 16, ReLU)
* **Output Dimension**: 5 (Softmax over `[FNE, FEQ, UEQ, EPR, ICU]`)
* **Objective Function**: Multi-class Cross-Entropy Loss.
* **Label Representation**: One-hot vector of the static schedule that solved the problem in the shortest wall-clock time during sweeps.

### D. Native Rust Inference API
```rust
// crates/mrs-core/src/ml/schedule_classifier.rs
use std::time::Duration;
use burn::tensor::backend::Backend;

pub struct ScheduleClassifier<B: Backend> {
    model: ClauseClassifier<B>, // Burn model
}

impl<B: Backend> ScheduleClassifier<B> {
    /// Evaluates the 16 structural features and returns the selected schedule name.
    pub fn classify(&self, features: [f32; 16]) -> &'static str {
        let logits = self.model.forward(tensor_from_array(features));
        let selected_idx = logits.argmax(1);
        match selected_idx {
            0 => "casc_fne",
            1 => "casc_feq",
            2 => "casc_ueq",
            3 => "casc_epr",
            4 => "casc_icu",
            _ => "casc", // Default fallback
        }
    }
}
```

---

## 4. Paradigm B: ML Premise Selection (Axiom Pruning)

### A. Architectural Role
In large-theory problems (e.g., mathematics, software verification), solvers are overwhelmed by hundreds or thousands of background axioms. ML Premise Selection evaluates the relevance of each axiom relative to the conjecture and drops the bottom $K\%$ before the search begins.

### B. Feature Extraction Representation (Pairwise TF-IDF Embedding)
Premise selection relies on symbol overlap and structural connection between a candidate axiom $A$ and the conjecture/goal set $C$.
1. **Structural Simplicity (`[f32; 8]`)**: Structural properties of the axiom $A$ itself (literal count, depth, Horn status, unit status, variable count).
2. **Pairwise Symbol Overlap (`[f32; 16]`)**: Joint symbol overlap features:
   * Predicate overlap: $\frac{|S_{\text{preds}}(A) \cap S_{\text{preds}}(C)|}{|S_{\text{preds}}(A) \cup S_{\text{preds}}(C)|}$
   * Functor overlap: $\frac{|S_{\text{funcs}}(A) \cap S_{\text{funcs}}(C)|}{|S_{\text{funcs}}(A) \cup S_{\text{funcs}}(C)|}$
   * TF-IDF similarity over hashed symbol name frequencies.
   * Sine-distance heuristic (structural distance to goal in the symbol-sharing graph).

### C. Neural Network Model (Pairwise Scorer MLP)
* **Input Dimension**: 24 (8 structural + 16 pairwise)
* **Hidden Layers**: Linear(24 → 64, GELU) → Linear(64 → 32, GELU)
* **Output Dimension**: 1 (Logit representing relevance score in `[0, 1]`)
* **Objective Function**: Binary Cross-Entropy (BCE) Loss with Logits.
* **Label Representation**: `1.0` if the axiom $A$ was part of the final refutation proof of $C$ in successful sweeps; `0.0` otherwise.

### D. Native Rust Inference API
```rust
// crates/mrs-core/src/ml/premise_selector.rs
use crate::term_bank::IdClause;

pub struct PremiseSelector<B: Backend> {
    model: ScorerModel<B>,
}

impl<B: Backend> PremiseSelector<B> {
    /// Scores and prunes the set of input axioms, retaining the top-N relevant premises.
    pub fn select_premises(
        &self,
        axioms: Vec<IdClause>,
        conjectures: &[IdClause],
        keep_ratio: f32, // e.g. 0.60 to keep top 60%
    ) -> Vec<IdClause> {
        let mut scored_axioms = Vec::with_capacity(axioms.len());
        for axiom in axioms {
            let feats = self.extract_pairwise_features(&axiom, conjectures);
            let score = self.evaluate_score(feats);
            scored_axioms.push((axiom, score));
        }
        
        // Sort descending by relevance score
        scored_axioms.sort_by(|(_, s1), (_, s2)| s2.partial_cmp(s1).unwrap());
        
        // Retain top proportion
        let keep_count = ((scored_axioms.len() as f32) * keep_ratio).round() as usize;
        scored_axioms.into_iter().take(keep_count.max(10)).map(|(ax, _)| ax).collect()
    }
}
```

---

## 5. Offline Training & Data Collection Protocol

### A. Data Logging
Data collection occurs during a full-portfolio sweep (e.g., CASC-30 problem list):
1. **Schedule Classifier**: For each problem, log the size-16 structural vector at startup, and label it with the ID of the portfolio that solved it fastest.
2. **Premise Selector**: For each successful refutation, walk the proof ancestry DAG back to the initial input clauses. Initial axioms on this path are labeled `1.0` (relevant). All other input axioms are labeled `0.0` (irrelevant). Log the pairwise feature vectors (`[f32; 24]`).

### B. GPU Training Configuration (`mrs-train`)
* **Framework**: **Burn** (using Libtorch/CUDA on V100 GPU server).
* **Early Stopping**: Triggered when validation loss (on a 20% stratified held-out split) fails to decrease for 3 consecutive epochs.
* **Imbalance Mitigation (Premise Selector)**: Positives are highly sparse. We use Weighted Binary Cross-Entropy Loss to penalize false negatives more heavily:
  $$\text{Loss} = - \left( w_{\text{pos}} \cdot y \log(\sigma(x)) + (1 - y) \log(1 - \sigma(x)) \right)$$

---

## 6. Implementation & Verification Checklist

- [ ] **Phase 1: Feature Extraction & Sample Definitions**
  - Implement `mrs-core::ml::schedule_classifier` structural extraction.
  - Implement `mrs-core::ml::premise_selector` pairwise extraction.
- [ ] **Phase 2: Offline Training Integration (`mrs-train`)**
  - Create dataloaders for pairwise and multiclass datasets.
  - Write PyTorch-equivalent MLP architectures in Burn.
  - Output binary serialization models + `meta.json` validation schema.
- [ ] **Phase 3: Integration into Native Solving Loop**
  - Add `--ml-schedule` and `--ml-prune <ratio>` CLI flags to `mrs::main`.
  - Wire Premise Selector to execute immediately after CNF lowering.
  - Wire Schedule Classifier to assign the optimal portfolio before solver initialization.
- [ ] **Phase 4: Rigorous Quality Assurance**
  - Verify zero impact on standard static portfolios when features are disabled.
  - Unit test that model loads correctly across Libtorch/NdArray backends.
  - Assert that feature dimensions and version checksums in `meta.json` match at runtime.
