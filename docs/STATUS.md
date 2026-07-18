# Project Status: MRS

`mrs` (Mechanical Reasoning System) is an automated theorem prover built in pure Rust, targeting the CASC competition.

## Current Capabilities

### Core Architecture
- **Given-Clause Loop**: Fully implemented Otter-style loop for the superposition calculus.
- **Parallel Strategy Portfolio**: 15 active strategies run **simultaneously** via `std::thread::scope`, plus a 16th diagnostic strategy (`MRS_SINGLE_STRATEGY=16`) that gets `Duration::ZERO` in normal runs. Each thread constructs its own `SearchState`. A shared `Arc<AtomicBool>` stop-flag fires the moment any thread finds a refutation, causing siblings to return on their next time-check. Named schedules: `casc` (default, 16 strategies), `casc_fne`/`casc_ueq`/`casc_epr` (division-tuned, one strategy per worker), `mini` (3 strategies), `fast` (1 strategy).
- **AVATAR**: Fully integrated using the CaDiCaL CDCL solver (`Send`-compatible). Clauses are dynamically split into variable-disjoint components, delegating propositional case-splits to the SAT solver. EPR-structured problems are now handled lazily by AVATAR (naive pre-expansion was disabled after causing OOM on large problems).
  - **Known limitation (proof self-containedness, not soundness)**: `extract_proof`'s simple parent-BFS does not carry the SAT-refutation-derived justification for why a clause's *other* split components can be safely ignored — each surviving split component's `ClauseSource` cites the same pre-split parents as the whole (unsplit) clause did, without the SAT-level reasoning. Confirmed via `mrs-proover` on `SWC351+1.p` (2026-07-18): the SZS answer was correct, but the printed proof for an AVATAR-heavy derivation is not a self-contained FOL entailment chain step-by-step, so an external checker's ATP ladder correctly reports `VerifiedBad` on some steps even though the underlying search is sound. This is the same class of well-known problem Vampire addresses with dedicated `avatar_component_clause`/`avatar_split_clause`/`avatar_sat_refutation` TSTP annotations (and typically still needs a SAT-aware checker). Properly fixing this means threading SAT-refutation justification through the proof printer — out of scope as a quick fix; see `docs/TODO_CASC.md` for tracking. Does **not** affect CASC scoring (SZS status only) or ProoVer (which never inspects this citation metadata — see the CASC-J13 soundness incident writeup in `docs/BENCHMARKS.md`).
- **CWA (Componentwise AVATAR, `crates/mrs-search/src/cwa.rs`)**: a narrow, rare pattern-matched pre-pass for definitional-CNF top disjunctions (`MIN_BRANCHES = 5`, distinct predicates, variable-disjoint literals). The `PRO013+3.p` incident's fix (polarity preservation in `make_branch_unit`) shipped with **zero real-corpus regression coverage** — confirmed by tracing `PRO013+3.p` itself with `TRACE_CWA=1`: every candidate in it gets rejected for *other* reasons (shared-variable, duplicate-predicate, equality), never for polarity, so a future regression there would only be caught by the synthetic unit test. Investigated 2026-07-18: swept `fof_non_theorems.list` plus the TPTP-v9.2.1 SEU domain with a new `TRACE_CWA_POLARITY=1` env var (prints each candidate's per-literal polarity) and found CWA fires on ~9% of SEU FOF problems checked, with **7 real files exercising mixed-polarity candidates** confirming this is not just a theoretical pattern: `SEU409+3.p`, `SEU419+3.p`, `SEU421+3.p`, `SEU425+3.p`, `SEU441+3.p`, `SEU444+3.p`, `SEU438+3.p` all produce a `[-, +, -, +, +, +]`-polarity candidate. Added `cwa_solves_definitional_cnf_with_mixed_polarity_branches` (an end-to-end unit test through the real `detect_top_disjunction -> try_componentwise_refute` path, not just `make_branch_unit` in isolation — manually confirmed it fails without the polarity fix) to close the loop at the unit-test level; the real SEU files are too large/slow (tens of thousands of clauses) to add directly as fast permanent regression fixtures.
  - **Sibling risk area, not yet audited to the same depth**: `fvo.rs` (FNE-Variable-Only propositional-skeleton refutation) is the only other module in `mrs-search`/`mrs-calculus` with a hand-written `**Soundness**` justification comment and narrow/rare trigger conditions — the same risk shape that hid the CWA polarity bug. A structural read found no concrete issue, but it hasn't received the same adversarial review CWA got; treat as a candidate for the next soundness audit pass.

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
- **Goal-Directed Heuristics**: Distance-to-conjecture is tracked in `Clause` and propagated to derived clauses (`min(parent.distance) + 1`). The `GoalDirected` selection strategy penalizes pure-axiom clauses in weight selection.
- **Clause Weight Functions** (`ClauseWeightFn`): Seven weight functions beyond the standard symbol-count baseline:
  - `FunctionDepth`: linear depth scaling (`w*(d+1)` per symbol at depth d)
  - `FunctionWeightPenalty`: quadratic depth scaling (`w*(d+1)^2`)
  - `FunctionWeightPenaltyExp`: exponential depth scaling (`w*2^d`, capped at 2^30)
  - `HornPenalty`: 3× multiplier on non-Horn clauses
  - `HornHeuristic`: `pos_count×` progressive multiplier on non-Horn clauses
  - `HornHeuristicExp`: `2^(pos_count-1)×` exponential multiplier on non-Horn clauses
  - `ConjSymbolBoost`: symbols not in the negated-conjecture closure cost 3×
  - `SymbolWeight`: each symbol costs its KBO/LPO precedence rank
- **Set of Support (SOS)**: `SearchConfig.sos_depth` enables two complementary SOS mechanisms:
  - *Selection SOS* (`pop_weight_sos`): weight picks only return goal-connected clauses (distance < sos_depth); age picks are unrestricted.
  - *Inference SOS*: resolution and superposition are skipped when *both* parents have `distance >= sos_depth`. Factoring is unconditional; equality-resolution/factoring are also restricted to goal-connected clauses under SOS.
- **AC Axiom Elimination + Heuristic AC-Unification**: At search startup, `detect_ac_symbols` identifies commutativity (`f(X,Y)=f(Y,X)`) and associativity (`f(f(X,Y),Z)=f(X,f(Y,Z))`) axioms and removes them from the passive set. `unify_ac_id` in `mrs-unify` flattens associative chains and tries both orderings for commutativity before falling back to standard unification.

### Performance Optimisations
- **LRS (Limited Resource Strategy)**: every 100 given-clause iterations, the prover estimates the remaining iteration budget from `elapsed/iteration` and prunes the passive queue to that size (min 2000). This prevents memory explosion on hard problems and dramatically reduces teardown latency (previously 300k+ clauses in the passive queue caused 10–15s of cleanup after the time limit).  Set `TRACE_LRS=1` to see per-prune log lines on stderr.
- **`SmallVec<[TermId; 4]>`** for `TermNode::App` and `IdAtom::Pred`: terms with arity ≤ 4 (>99% of FOL terms) are stored inline, eliminating heap allocation on every term retrieval in the hot inference paths.
- **`intern_app` accepts `impl Into<SmallVec<[TermId; 4]>>`**: call sites pass `Vec` or `SmallVec` without conversion overhead.
- Eliminated all redundant `args.clone()` calls in `avatar.rs`, `fvi.rs`, and `literal_selection.rs`.

### mrs-proover (ProoVer 2026 Entry)
- Hybrid structural + semantic verification pipeline.
- Structural checks: acyclicity, leaf provenance (`introduced(definition)` with strict formula-body validation), Vampire-style Skolemization (returns `Unsound` on arity drops), propositional SAT fast-path for AVATAR steps, definition folding via alpha-equivalence.
- Semantic fallback: delegates unrecognised deductive steps to an external ATP (E or Vampire subprocess).
- Evil-proof test suite: 9 verified exploit cases including definition laundering, false negated conjecture, quantifier shadowing, and invalid distributivity.

## Remaining Gaps

See `docs/AUDIT.md` for the Phase 1 failure census and root-cause analysis.
See `TODO_CASC.md` for the prover roadmap and `TODO_PROOVER.md` for the verifier roadmap.

The highest-ROI items remaining:
1. **Machine-Learning Guided Clause Selection (CASC)**: Train and integrate a model using `mrs-train` to guide clause selection on FNE/FEQ.
2. **SIMD-Optimized Feature Vector Index (CASC)**: Vectorize `can_subsume` check for performance.
3. **Multi-Step Recursive Definition Detection (ProoVer)**: Catch recursive unfolding across the proof DAG.
