# Project Status: MRS

`mrs` (Mechanical Reasoning System) is an automated theorem prover built in pure Rust.

## What has been accomplished so far

### Architecture and Loops
- **Given-Clause Loop**: Fully implemented Otter-style given-clause loop for the superposition calculus.
- **Serial Strategy Portfolio**: Nine strategies with different heuristic configurations are tried in sequence, each with a fresh `SearchState` and a proportional time slice from the total budget. The first refutation wins.
- **AVATAR**: Advanced Vampire Architecture for Theories and Resolution has been fully integrated using the pure-Rust `varisat` CDCL solver. Clauses are dynamically split into variable-disjoint components, pushing propositional reasoning down to the SAT solver and significantly accelerating evaluation of non-Horn clauses.

### Indexing and Scaling
- **Priority Queues (Unprocessed Set)**: The passive set is managed by a dual-queue architecture (`BinaryHeap` for weight, `VecDeque` for age) with lazy deletion (tombstoning), enabling $O(\log N)$ extraction.
- **Discrimination Trees (DTree)**: Replaced $O(N)$ searches for complementary clauses and rewrite rules with sub-linear trie lookups.
- **Feature Vector Indexing (FVI)**: Subsumption candidates are filtered instantly by generating and comparing feature vectors (symbol frequency maps), skipping $>99\%$ of expensive backtracking.

### Heuristics and Inference
- **Calculus**: Full superposition calculus including Equality Resolution, Equality Factoring, standard Resolution, and Factoring.
- **Redundancy Elimination**:
  - Forward and Backward Subsumption.
  - Forward and Backward Demodulation (using `DTree` to maintain a set of oriented unit equations).
  - Tautology deletion and Condensation.
  - **Subsumption Resolution**: Contextual literal cutting using the Feature Vector Index — simplifies $C_2 = \neg P \lor Q \lor R$ to $Q \lor R$ when $C_1 = P \lor Q$ is active.
- **SInE (Sumo Inference Engine)**: Pre-filters massive axiomatizations by traversing from the conjecture over symbol genericity.
- **Orderings (KBO / LPO)**: Fully integrated Knuth-Bendix Ordering and Lexicographic Path Ordering with dynamic, symbol-frequency-based precedence to eliminate rare symbols early.
- **Literal Selection**: Includes `All`, `AllNegative`, `MaxNegative`, and the aggressive, incomplete `MaxNegativeOrMaxPositive`.
- **Goal-Directed UEQ Heuristics**: Distance-to-conjecture tracking is built into the `Clause` structure and used by the `GoalDirected` selection strategy to strongly penalize pure-axiom generated clauses in weight selection.
- **InstGen / EPR**: The `preprocess_epr` stage detects Effectively Propositional problems and enumerates ground instances before the given-clause loop, bypassing the overhead of full superposition for propositional fragments.

## Discovered Performance Bottlenecks and Missing Features

To approach state-of-the-art performance (like Vampire), the following high-ROI capabilities need to be implemented:

1. **Perfect Discrimination Trees / Substitution Trees**: The current `DTree` handles variables sloppily (imperfect index), resulting in false-positive unification candidates that waste CPU cycles in the actual `unify` check. Tracking variable consistency within the index itself will eliminate this waste.
2. **AC-Unification**: Associativity and commutativity axioms currently cause permutation explosions in the search space. Vampire uses built-in AC-unification to treat permutations as identical terms algorithmically.
3. **Machine Learning Guided Selection (ENIGMA / Deepire)**: Using GNNs or XGBoost to evaluate clause usefulness based on past proofs. `mrs` currently relies on static Age/Weight/GoalDistance ratios.
4. **Clause Sharing**: In a parallel portfolio, if one strategy derives a unit equality, broadcasting it to other strategies' demodulation loops can accelerate all of them simultaneously. Currently each strategy is completely siloed.

## Next Steps

1. Upgrade `DTree` to a perfect index, eliminating false-positive unification candidates.
2. Implement AC-Unification to avoid permutation explosion on AC axioms.
