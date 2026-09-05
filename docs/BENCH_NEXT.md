# CASC Benchmarking & Build Guide for New Features

This document provides the exact Git commits, checkout/build instructions, target CASC competition divisions, and benchmark evaluation commands for each feature branch developed to close the gap with Vampire and enhance proof self-verification.

---

## Summary Matrix

| # | Feature / Branch | Git Commit | Primary Division | Secondary / Related | Key Focus |
| :--- | :--- | :--- | :--- | :--- | :--- |
| 1 | `feat/ueq-goal-transformation` | `ceea916` | **UEQ** (Unit Equality) | **FEQ** (FOF with Equality) | Goal-directed equational preprocessing |
| 2 | `feat/kernel-equational-definitions` | `658b5c7` | **PRV** (Proof Verification) | **FEQ**, **UEQ** (Self-Check) | CNF definition certification & proof DAG sorting |
| 3 | `feat/destructive-equality-resolution` | `9738467` | **FEQ** (FOF with Equality) | **EPU** (EPR Unsatisfiable), **FNE** | Eager $x \neq t$ literal elimination (DER) |
| 4 | `feat/contextual-literal-simplification` | `d97b41d` | **FNE** (FOF No Equality) | **FEQ**, **EPU / EPS** | Forward & backward subsumption resolution |
| 5 | `feat/indexed-superposition` | `f935bbc` | **FEQ** (FOF with Equality) | **UEQ** (Unit Equality) | Discrimination tree indexed superposition |
| 6 | `feat/lrat-sat-kernel-verification` | `be6e77e` | **PRV** (Proof Verification) | **FEQ**, **FNE** (AVATAR Proofs) | Zero-dependency standard LRAT SAT trace replay |
| 7 | `feat/avatar-non-ground-splitting` | `12a8d99` | **FEQ** (FOF with Equality) | **FNE**, **EPU / EPS** | Permutation- & symmetry-invariant AVATAR splitting |
| 8 | `feat/in-place-demodulation` | `2b51b83` | **FEQ** (FOF with Equality) | **UEQ** (Unit Equality) | In-place backward demodulation & DISCOUNT loop rewrite cache |
| 9 | `feat/feature-vector-tree` | `5bb8b8b` | **FNE** (FOF No Equality) | **FEQ**, **EPR** | Hierarchical Feature Vector Tree (FVT) for $O(\log N)$ forward & backward subsumption |
| 10 | `feat/multi-queue-selection` | `3ec3228` | **FNE** (FOF No Equality) | **FEQ**, **UEQ**, **EPR** | Multi-queue given-clause loop with dedicated Unit and Horn selection queues |
| 11 | `feat/dynamic-precedence` | `379baa5` | **FEQ** (FOF with Equality) | **UEQ**, **FNE**, **EPR** | Problem-specific dynamic KBO/LPO precedence and symbol weighting schemes |
| 12 | `feat/free-var-skolemization` | `3c66465` | **FNE** (FOF No Equality) | **FEQ** (FOF with Equality) | Free-variable filtered Skolemization to eliminate redundant Skolem arity |
| 13 | `feat/polarity-definitional-cnf` | `722b669` | **FNE** (FOF No Equality) | **FEQ**, **EPR** | Polarity-aware Plaisted-Greenbaum renaming & linear equivalence CNF |
| 14 | `integrate/casc-next` (Goal Distance Guidance) | `6f807a7` | **FNE** (FOF No Equality) | **FEQ**, **EPR** | Multi-hop bipartite symbol-clause reachability graph & graded conjecture weight boost |
| 15 | `integrate/casc-next` (Blocked Clause & Pure Literal Elimination) | `eca8405` | **FNE** (FOF No Equality) | **FEQ**, **EPR** | First-order Blocked Clause Elimination (BCE), Pure Literal Elimination (PLE), and Tautology Elimination in CNF preprocessing |

---

## Detailed Breakdown & Build Instructions

### 1. Twee-Style Goal-Directed Equational Preprocessing
* **Branch**: `feat/ueq-goal-transformation`
* **Commit**: `ceea916fe8a337ea0e911d5e5e0ae6df9467a025` (`ceea916`)
* **Build & Checkout**:
  ```bash
  git checkout feat/ueq-goal-transformation # or git checkout ceea916
  nix develop -c cargo build --release
  ```
* **Target Division**: **UEQ** (`--schedule casc_ueq`), secondary: **FEQ** (`--schedule casc_feq`).
* **Why**: Reorients equational conjectures ($u = v \implies u = \$goal(u) \land v = \$goal(v)$) to direct rewriting towards goal terms.
* **Problem Domains**: TPTP `GRP` (Group Theory), `BOO` (Boolean Algebra), `ROB` (Robbins Algebras), `RNG` (Ring Theory).
* **Benchmark Command**:
  ```bash
  ./crates/mrs-bench/casc.sh --systems mrs --divisions ueq --time 30 --jobs 4 --output results/ueq-goal-test
  ```

---

### 2. Strict Proof Kernel Equational Definitions & Proof DAG Sorting
* **Branch**: `feat/kernel-equational-definitions`
* **Commit**: `658b5c7f9a02f34bc709608f06d81877d033492d` (`658b5c7`)
* **Build & Checkout**:
  ```bash
  git checkout feat/kernel-equational-definitions # or git checkout 658b5c7
  nix develop -c cargo build --release -p mrs-proover
  ```
* **Target Division**: **PRV** (Proof Verification / ProoVer 2026), secondary: **FEQ** / **UEQ** self-verification.
* **Why**: Validates equational definitions $f(X_1, \dots, X_n) = t$ produced during clausification, sorts proof DAG nodes deterministically via binary heaps, and prevents cyclic demodulation loops.
* **Problem Domains**: TSTP proofs in `SYN`, `SET`, `ALG`, `SEU`.
* **Benchmark Command**:
  ```bash
  nix develop -c cargo test -p mrs-proof-kernel
  nix develop -c cargo run --release -p mrs-proover -- --verify-folder /path/to/tstp_proofs/
  ```

---

### 3. Destructive Equality Resolution (DER)
* **Branch**: `feat/destructive-equality-resolution`
* **Commit**: `9738467d6d1dc3190f663bea94f4f628d7f1d7a9` (`9738467`)
* **Build & Checkout**:
  ```bash
  git checkout feat/destructive-equality-resolution # or git checkout 9738467
  nix develop -c cargo build --release
  ```
* **Target Division**: **FEQ** (`--schedule casc_feq`) & **EPU** (`--schedule casc_epu`), secondary: **FNE**.
* **Why**: Eagerly eliminates negative equality literals $x \neq t$ ($x \notin \text{vars}(t)$) without generating superposition branches, reducing clause width.
* **Problem Domains**: TPTP `HWV` (Hardware Verification), `SWV` (Software Verification), `KRS`, `SEU`.
* **Benchmark Command**:
  ```bash
  ./crates/mrs-bench/casc.sh --systems mrs --divisions feq,epu --time 30 --jobs 4 --output results/der-eval
  ```

---

### 4. Contextual Literal Simplification (Subsumption Resolution)
* **Branch**: `feat/contextual-literal-simplification`
* **Commit**: `d97b41de558d31313e75d1272a83d74ee220f7f6` (`d97b41d`)
* **Build & Checkout**:
  ```bash
  git checkout feat/contextual-literal-simplification # or git checkout d97b41d
  nix develop -c cargo build --release
  ```
* **Target Division**: **FNE** (`--schedule casc_fne`) & **FEQ** (`--schedule casc_feq`), secondary: **EPU / EPS**.
* **Why**: Discrimination-tree powered backward and forward subsumption resolution removes redundant literals and compacts the active clause set during given-clause loops.
* **Problem Domains**: TPTP `PUZ`, `SYN`, `NLP`, `MGT`.
* **Benchmark Command**:
  ```bash
  ./crates/mrs-bench/casc.sh --systems mrs --divisions fne,feq --time 30 --jobs 4 --output results/subsumption-resolution-eval
  ```

---

### 5. Discrimination Tree Indexed Superposition
* **Branch**: `feat/indexed-superposition`
* **Commit**: `f935bbc1c3810033863649f82926d8ba628ae18e` (`f935bbc`)
* **Build & Checkout**:
  ```bash
  git checkout feat/indexed-superposition # or git checkout f935bbc
  nix develop -c cargo build --release
  ```
* **Target Division**: **FEQ** (`--schedule casc_feq`) & **UEQ** (`--schedule casc_ueq`).
* **Why**: Replaces $O(N)$ linear scans over active clauses with $O(\log N)$ subterm discrimination tree traversals, accelerating superposition clause generation throughput.
* **Problem Domains**: Large clause-set domains: `GEO`, `ALG`, `SEU`, `LCL`.
* **Benchmark Command**:
  ```bash
  ./crates/mrs-bench/casc.sh --systems mrs --divisions feq,ueq --time 30 --jobs 4 --output results/indexed-superposition-eval
  ```

---

### 6. Zero-Dependency Standard LRAT SAT Trace Replay
* **Branch**: `feat/lrat-sat-kernel-verification`
* **Commit**: `be6e77ec6fba2e01ea02c0b07175cf0dea1452c0` (`be6e77e`)
* **Build & Checkout**:
  ```bash
  git checkout feat/lrat-sat-kernel-verification # or git checkout be6e77e
  nix develop -c cargo build --release -p mrs-proof-kernel
  ```
* **Target Division**: **PRV** (Proof Verification), secondary: **FEQ** / **FNE** (Self-checking AVATAR proofs).
* **Why**: In-kernel LRAT parser and unit propagation replay engine allows verifying standard LRAT proof traces emitted by CaDiCaL for AVATAR SAT refutations.
* **Problem Domains**: AVATAR refutation proofs across `FOF` / `CNF`.
* **Benchmark Command**:
  ```bash
  nix develop -c cargo test -p mrs-proof-kernel -- certifies_explicit_avatar_certificate_lrat
  ```

---

### 7. Non-Ground AVATAR Splitting & Component Canonicalization
* **Branch**: `feat/avatar-non-ground-splitting`
* **Commit**: `12a8d9926a3dcb9160f76722aeef6239d1d220c3` (`12a8d99`)
* **Build & Checkout**:
  ```bash
  git checkout feat/avatar-non-ground-splitting # or git checkout 12a8d99
  nix develop -c cargo build --release
  ```
* **Target Division**: **FEQ** (`--schedule casc_feq`), **FNE** (`--schedule casc_fne`), and **EPU / EPS**.
* **Why**: Literal permutation sorting and equality argument orientation enable symmetric non-ground components ($p(X) \lor q(X) \equiv q(Y) \lor p(Y)$, $a = b \equiv b = a$) to reuse the same SAT variable, pruning search space.
* **Problem Domains**: Large multi-clause FOF benchmarks in `SWV`, `KRS`, `SEU`, `CSR`.
* **Benchmark Command**:
  ```bash
  ./crates/mrs-bench/casc.sh --systems mrs --divisions feq,fne,epu,eps --time 30 --jobs 4 --output results/avatar-canonical-eval
  ```

---

### 8. In-Place Backward Demodulation & DISCOUNT Loop Architecture
* **Branch**: `feat/in-place-demodulation`
* **Commit**: `2b51b839b4824b72ed78f93eb88d6d72cf9e72bd` (`2b51b83`)
* **Build & Checkout**:
  ```bash
  git checkout feat/in-place-demodulation # or git checkout 2b51b83
  nix develop -c cargo build --release
  ```
* **Target Division**: **FEQ** (`--schedule casc_feq`) & **UEQ** (`--schedule casc_ueq`).
* **Why**: When a new unit equality $l = r$ is retained, backward demodulation rewrites active clauses in-place via candidate subterm discrimination tree lookup. If a clause simplifies, its old indexes are pruned and updated, avoiding active set bloat and preventing redundant inferences against outdated equations.
* **Problem Domains**: Heavy equational domains: `BOO`, `GRP`, `COL`, `RNG`, `ROB`, `SEU`.
* **Benchmark Command**:
  ```bash
  ./crates/mrs-bench/casc.sh --systems mrs --divisions ueq,feq --time 30 --jobs 4 --output results/in-place-demod-eval
  ```

---

### 9. Hierarchical Feature Vector Tree (FVT) Subsumption Indexing
* **Branch**: `feat/feature-vector-tree`
* **Commit**: `5bb8b8b32f8b996e8e45c3d1786d0f58fdb07ad8` (`5bb8b8b`)
* **Build & Checkout**:
  ```bash
  git checkout feat/feature-vector-tree # or git checkout 5bb8b8b
  nix develop -c cargo build --release
  ```
* **Target Division**: **FNE** (`--schedule casc_fne`), **FEQ** (`--schedule casc_feq`), and **EPR** (`--schedule casc_epr`).
* **Why**: Multidimensional feature vector tree indexing prunes clause candidate space to sub-logarithmic subsets for both forward subsumption (is new clause subsumed by active?) and backward subsumption (does new clause subsume active clauses?), replacing $O(N)$ linear scans with vector coordinate bounds.
* **Problem Domains**: Large clause count problems in `PUZ`, `SYN`, `MGT`, `SET`, `KRS`.
* **Benchmark Command**:
  ```bash
  ./crates/mrs-bench/casc.sh --systems mrs --divisions fne,feq,epr --time 30 --jobs 4 --output results/fvt-subsumption-eval
  ```

---

### 10. Multi-Queue Given-Clause Selection Portfolio
* **Branch**: `feat/multi-queue-selection`
* **Commit**: `3ec32281d8c0fd00d91d322829cac967cf1c0cb5` (`3ec3228`)
* **Build & Checkout**:
  ```bash
  git checkout feat/multi-queue-selection # or git checkout 3ec3228
  nix develop -c cargo build --release
  ```
* **Target Division**: **FNE** (`--schedule casc_fne`), **FEQ** (`--schedule casc_feq`), **UEQ** (`--schedule casc_ueq`), and **EPR** (`--schedule casc_epr`).
* **Why**: Upgrades the 2-way FIFO/Weight queue to an 8-way multi-queue with priority channels for unit clauses, Horn clauses, conjecture/goal-distance clauses, and SOS clauses. Prevents unit clauses from drowning beneath large passive sets and accelerates unit-driven refutations.
* **Problem Domains**: All CASC divisions, especially `SYN`, `SET`, `PUZ`, `ALG`.
* **Benchmark Command**:
  ```bash
  ./crates/mrs-bench/casc.sh --systems mrs --divisions fne,feq,ueq,epr --time 30 --jobs 4 --output results/multi-queue-eval
  ```

---

### 11. Dynamic Problem-Specific Precedence & Symbol Weighting
* **Branch**: `feat/dynamic-precedence`
* **Commit**: `379baa599173955f298af786384fe2c15e2d7120` (`379baa5`)
* **Build & Checkout**:
  ```bash
  git checkout feat/dynamic-precedence # or git checkout 379baa5
  nix develop -c cargo build --release
  ```
* **Target Division**: **FEQ** (`--schedule casc_feq`), **UEQ** (`--schedule casc_ueq`), **FNE** (`--schedule casc_fne`), and **EPR** (`--schedule casc_epr`).
* **Why**: Problem-adaptive symbol analysis configures KBO/LPO precedence schemes (`InvFreq`, `Freq`, `ArityMax`, `ArityMin`, `GoalBoost`) and weight schemes (`Uniform`, `Arity`, `InvFreq`, `ConjectureBonus`), diversifying parallel portfolio search spaces so different workers explore radically different term-rewriting directions.
* **Problem Domains**: Deep algebraic and combinatorial problems: `BOO`, `GRP`, `RNG`, `ROB`, `KRS`, `SWV`.
* **Benchmark Command**:
  ```bash
  ./crates/mrs-bench/casc.sh --systems mrs --divisions feq,ueq,fne --time 30 --jobs 4 --output results/dynamic-precedence-eval
  ```

---

### 12. Free-Variable Filtered Skolemization
* **Branch**: `feat/free-var-skolemization`
* **Commit**: `3c664651edc9b64f3dbfbb31b7fe96cefe3a4d95` (`3c66465`)
* **Build & Checkout**:
  ```bash
  git checkout feat/free-var-skolemization # or git checkout 3c66465
  nix develop -c cargo build --release
  ```
* **Target Division**: **FNE** (`--schedule casc_fne`) & **FEQ** (`--schedule casc_feq`).
* **Why**: Filters out universally quantified variables that do not actually occur free in the existential body. Reduces Skolem functions to constants or lower arities, dramatically decreasing term weight, reducing unification overhead, and preventing term explosion in clausal deduction.
* **Problem Domains**: First-order problems with complex quantifier alternation: `SET`, `SEU`, `SWV`, `KRS`, `MGT`.
* **Benchmark Command**:
  ```bash
  ./crates/mrs-bench/casc.sh --systems mrs --divisions fne,feq --time 30 --jobs 4 --output results/filtered-skolem-eval
  ```

---

### 13. Polarity-Aware Definitional Renaming & Biconditional Expansion Fix
* **Branch**: `feat/polarity-definitional-cnf`
* **Commit**: `722b669528d22bb48c46c3fb8c8b4b74aa68d2c0` (`722b669`)
* **Build & Checkout**:
  ```bash
  git checkout feat/polarity-definitional-cnf # or git checkout 722b669
  nix develop -c cargo build --release
  ```
* **Target Division**: **FNE** (`--schedule casc_fne`), **FEQ** (`--schedule casc_feq`), and **EPR** (`--schedule casc_epr`).
* **Why**: Solves the exponential $O(2^n)$ clause and term blowup caused by naive recursive NNF expansion on nested equivalences (`<=>`). Replaces complex equivalence subformulas bottom-up with fresh definition predicates, introduces polarity-aware half-definitions (Plaisted-Greenbaum), and applies a configurable distributive threshold ($\rho=8$) so small disjunctions distribute directly without unnecessary Tseitin symbols. Full provenance and transitive parent tracking are preserved for strict verification.
* **Problem Domains**: Pelletier equivalence problems (`pel12.p`), equivalence-heavy benchmarks in `SYN`, `SET`, `KRS`, hardware and protocol verification in `HWV`, `SWV`.
* **Benchmark Command**:
  ```bash
  ./crates/mrs-bench/casc.sh --systems mrs --divisions fne,feq,epr --time 30 --jobs 4 --output results/polarity-cnf-eval
  ```

---

### 14. Non-Equational Relational Goal Distance Guidance & Multi-Hop Conjecture Weight Boost
* **Branch**: `integrate/casc-next`
* **Commit**: `6f807a7` (on `integrate/casc-next`)
* **Build & Checkout**:
  ```bash
  git checkout integrate/casc-next
  nix develop -c cargo build --release
  ```
* **Target Division**: **FNE** (`--schedule casc_fne`), **FEQ** (`--schedule casc_feq`), and **EPR** (`--schedule casc_epr`).
* **Why**: Large first-order axiomatizations often drown the prover in millions of goal-irrelevant deductions because pure axiom-axiom inferences wander away from the conjecture. Relational Goal Distance guidance builds a multi-hop reachability graph across the bipartite symbol-clause graph at problem startup (distance 0 = conjecture symbols; distance 1 = symbols in axioms sharing a conjecture symbol; distance 2 = 2-hop symbols, up to radius 5). Weights clauses and active queues (`goal_queue`) by relational distance, allowing 1-hop and 2-hop neighbor axioms to be prioritized right from iteration 0 and penalizing goal-disconnected derivations.
* **Problem Domains**: Large axiom libraries, relational puzzles, non-equational reasoning: `SYN`, `SET`, `PUZ`, `MGT`, `NLP`, `SWV`.
* **Benchmark Command**:
  ```bash
  ./crates/mrs-bench/casc.sh --systems mrs --divisions fne,feq,epr --time 30 --jobs 4 --output results/goal-distance-eval
  ```

---

### 15. First-Order Blocked Clause Elimination (BCE) & Pure Literal Elimination (PLE)
* **Branch**: `integrate/casc-next`
* **Commit**: `eca8405` (on `integrate/casc-next`)
* **Build & Checkout**:
  ```bash
  git checkout integrate/casc-next
  nix develop -c cargo build --release
  ```
* **Target Division**: **FNE** (`--schedule casc_fne`), **FEQ** (`--schedule casc_feq`), and **EPR** (`--schedule casc_epr`).
* **Why**: Preprocessing eliminates redundant clauses before search starts. Tautology elimination removes trivial validities ($s = s$ and $L \lor \neg L$). Pure Literal Elimination (PLE) detects predicates occurring with only one polarity across all clauses; axiom clauses containing such predicates can never derive the empty clause $\Box$ and are removed, cascading across rounds. First-Order Blocked Clause Elimination (BCE, Kiesl et al. 2016) eliminates clauses where a literal $L \in C$ produces only tautological resolvents with every possible partner clause in the active set. Conjectures and negated conjectures are strictly protected to preserve refutational completeness and proof certificates.
* **Problem Domains**: Knowledge bases, relational ontologies, large axiomatizations with unneeded theories or definition artifacts: `SYN`, `SET`, `KRS`, `MGT`, `SWV`, `NLP`.
* **Benchmark Command**:
  ```bash
  ./crates/mrs-bench/casc.sh --systems mrs --divisions fne,feq,epr --time 30 --jobs 4 --output results/bce-ple-eval
  ```

---

### 16. SAT-Guided InstGen Loop for EPR Problems
* **Branch**: `integrate/casc-next`
* **Commit**: `06d6831` (on `integrate/casc-next`)
* **Build & Checkout**:
  ```bash
  git checkout integrate/casc-next
  nix develop -c cargo build --release
  ```
* **Target Division**: **EPR** (`--schedule casc_epr`), **EPS** (Essentially Propositional Satisfiable), and **EPU** (Essentially Propositional Unsatisfiable).
* **Why**: The naive Herbrand expansion (`preprocess_epr`) suffered from exponential combinatorial explosion on clauses with $\ge 6$ variables ($c^v$ instances causing OOM). The lazy InstGen loop abstracts first-order clauses into propositional DIMACS representations using canonical variable mapping ($\bot$) and invokes incremental CaDiCaL (`mrs_cadical::Solver`). If CaDiCaL returns UNSAT, an exact propositional BFS refutation extracts a valid TSTP resolution DAG (`instantiation` and `resolution` steps) in milliseconds; if CaDiCaL finds a model, complementary satisfied literals are checked for Robinson MGU unification. New MGU instances are lazily added until refutation or until no candidate pairs unify (in which case the propositional model lifts to a sound first-order model $\to$ `Satisfiable`/`CounterSatisfiable`).
* **Problem Domains**: Bernays-Schönfinkel / EPR problems, finite-domain relational specs, puzzle formalizations: `PUZ`, `SYN`, `SWV`, `NLP`, `CSR`.
* **Benchmark Command**:
  ```bash
  ./crates/mrs-bench/casc.sh --systems mrs --divisions epr,eps,epu --time 30 --jobs 4 --output results/instgen-epr-eval
  ```
