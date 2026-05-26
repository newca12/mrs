# Project Status: MRS

`mrs` (Mechanical Reasoning System) is an automated theorem prover built in pure Rust.

## What has been accomplished so far

### Architecture and Loops
- **Given-Clause Loop**: Fully implemented Otter-style given-clause loop for the superposition calculus.
- **Multithreading**: Portfolio solving using `crossbeam-channel` enables running multiple complete/incomplete strategies in parallel, making full use of modern multi-core processors.
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
- **SInE (Sumo Inference Engine)**: Pre-filters massive axiomatizations by traversing from the conjecture over symbol genericity.
- **Orderings (KBO / LPO)**: Fully integrated Knuth-Bendix Ordering and Lexicographic Path Ordering with dynamic, symbol-frequency-based precedence to eliminate rare symbols early.
- **Literal Selection**: Includes `All`, `AllNegative`, `MaxNegative`, and the aggressive, incomplete `MaxNegativeOrMaxPositive` (used successfully alongside complete strategies in the multithreaded portfolio).

## Discovered Performance Bottlenecks and Missing Features

To approach state-of-the-art performance (like Vampire), the following high-ROI capabilities need to be implemented:

1. **Subsumption Resolution**: Also known as contextual literal cutting. Simplifies $C_2 = \neg P \lor Q \lor R$ to $Q \lor R$ if $C_1 = P \lor Q$.
2. **Goal-Directed Equational Reasoning**: UEQ (Unit Equational) problems suffer because `mrs` explores purely by weight/age. Distance-to-conjecture heuristics are required.
3. **Perfect Discrimination Trees / Substitution Trees**: The current `DTree` handles variables sloppily (imperfect index), resulting in false-positive unification candidates that waste CPU cycles in the actual `unify` check. Tracking variable consistency within the index itself will eliminate this waste.
4. **InstGen / EPR Handlers**: The Effectively Propositional fragment needs an instantiation-based engine, as superposition is often overkill.
5. **AC-Unification**: Associativity and commutativity axioms currently cause permutation explosions in the search space.

## Next Steps

1. Implement Subsumption Resolution.
2. Implement distance-to-goal tracking for UEQ heuristics.
3. Upgrade `DTree` to prevent false-positive unifications.