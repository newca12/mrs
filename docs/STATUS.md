# Project Status: MRS

`mrs` (Mechanical Reasoning System) is an automated theorem prover built in pure Rust, targeting the CASC competition.

## Current Capabilities

### Core Architecture
- **Given-Clause Loop**: Fully implemented Otter-style loop for the superposition calculus.
- **Parallel Strategy Portfolio**: 11 active strategies run **simultaneously** via `std::thread::scope`. Each thread constructs its own `SearchState` (required because `varisat::Solver` is not `Send`). A shared `Arc<AtomicBool>` stop-flag fires the moment any thread finds a refutation, causing siblings to return on their next time-check. Named schedules: `casc` (default, 11 strategies), `mini` (3 strategies), `fast` (1 strategy).
- **AVATAR**: Fully integrated using the pure-Rust `varisat` CDCL solver. Clauses are dynamically split into variable-disjoint components, delegating propositional case-splits to the SAT solver. EPR-structured problems are now handled lazily by AVATAR (naive pre-expansion was disabled after causing OOM on large problems).

### Indexing and Scaling
- **Priority Queues (Unprocessed Set)**: Dual-queue architecture (`BinaryHeap` for weight, `VecDeque` for age) with lazy tombstone deletion, enabling $O(\log N)$ extraction. Feature vectors cached inside the set for O(1) subsumption lookup.
- **Perfect Discrimination Trees (DTreeId)**: Both `unify_flat` (resolution/superposition partner lookup) and `get_generalizations_rec` (demodulation LHS lookup) track variable bindings through traversal. False-positive candidates are eliminated at the index level without post-retrieval unification checks. Literal-level insert/remove/query via `insert_atom`/`remove_atom`/`get_unifications_atom` removes all legacy `to_legacy` conversions from the hot path.
- **Feature Vector Indexing (FVI)**: Symbol frequency + polarity vectors filter subsumption candidates in constant time, skipping >99% of expensive backtracking.

### Inference and Redundancy Elimination
- **Full Superposition Calculus**: Resolution, Factoring, Equality Resolution, Equality Factoring, Superposition (into terms and literals).
- **Forward/Backward Demodulation**: Oriented unit equalities are indexed in a `DTreeId`; applied in both directions on every new clause and on all processed clauses when a new rewrite rule is derived.
- **Forward/Backward Subsumption**: Index-driven via FVI.
- **Subsumption Resolution**: Contextual literal cutting — simplifies $\neg P \lor Q \lor R$ to $Q \lor R$ when $P \lor Q$ is active.
- **Condensation** and **Tautology deletion**.
- **Global Subsumption & Orphan Elimination**: `SearchState` maintains a `children: HashMap<ClauseId, Vec<ClauseId>>` map. `register_clause` tracks every new clause's parents; `remove_clause_and_orphans` walks the dependency tree and evicts the entire subtree from processed, unprocessed, dormant-processed, and dormant-unprocessed sets. This prevents mid-run passive-queue explosion in UEQ and ICU divisions.

### Heuristics and Search Control
- **SInE (Sumo Inference Engine)**: Pre-filters large axiomatizations from the conjecture over symbol genericity. Automatic fallback: if SInE triggers and the search saturates in under 1 second, `main.rs` restarts without SInE to recover from over-pruning.
- **Term Orderings**: LPO and KBO with dynamic, rarity-based symbol precedence (rare symbols get higher precedence to eliminate them first).
- **Literal Selection**: `All`, `AllNegative`, `MaxNegative`, `MaxNegativeOrMaxPositive`.
- **Goal-Directed UEQ Heuristics**: Distance-to-conjecture is built into `Clause`; the `GoalDirected` selection strategy penalizes pure-axiom clauses in weight selection.
- **AC Axiom Elimination + Heuristic AC-Unification**: At search startup, `detect_ac_symbols` identifies commutativity (`f(X,Y)=f(Y,X)`) and associativity (`f(f(X,Y),Z)=f(X,f(Y,Z))`) axioms and removes them from the passive set. `unify_ac_id` in `mrs-unify` flattens associative chains and tries both orderings for commutativity before falling back to standard unification.

### Performance Optimisations
- **`SmallVec<[TermId; 4]>`** for `TermNode::App` and `IdAtom::Pred`: terms with arity ≤ 4 (>99% of FOL terms) are stored inline, eliminating heap allocation on every term retrieval in the hot inference paths.
- **`intern_app` accepts `impl Into<SmallVec<[TermId; 4]>>`**: call sites pass `Vec` or `SmallVec` without conversion overhead.
- Eliminated all redundant `args.clone()` calls in `avatar.rs`, `fvi.rs`, and `literal_selection.rs`.

### mrs-proover (ProoVer 2026 Entry)
- Hybrid structural + semantic verification pipeline.
- Structural checks: acyclicity, leaf provenance (`introduced(definition)` with strict formula-body validation), Vampire-style Skolemization (returns `Unsound` on arity drops), propositional SAT fast-path for AVATAR steps, definition folding via alpha-equivalence.
- Semantic fallback: delegates unrecognised deductive steps to an external ATP (E or Vampire subprocess).
- Evil-proof test suite: 9 verified exploit cases including definition laundering, false negated conjecture, quantifier shadowing, and invalid distributivity.

## Remaining Gaps

See `TODO_CASC.md` for the prover roadmap and `TODO_PROOVER.md` for the verifier roadmap.

The two highest-ROI items are:
1. **Clause sharing across parallel strategies**: if one thread derives a unit equality, broadcast it to other threads' demodulation indices.
2. **AC-equivalence matching in `axiom_leaf.rs`**: leaf-node validation currently fails on `A & B` / `B & A` rewrites produced by real ATPs, scoring 0 points instead of +1 on valid proofs.
