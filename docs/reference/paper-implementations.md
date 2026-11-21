# Paper-derived implementation contracts

This is the current-facing replacement for the retired SOTA checkbox list.
Implementing a technique, reproducing a paper's entire system, proving dynamic
refutational completeness, and independently certifying a proof are different
claims. None of the whole-prover completeness claims follows from test counts.

## Contracts and evidence

| Reference / technique | Implemented contract and source | Deliberate limits / missing paper scope | Executable evidence |
|---|---|---|---|
| Bachmair & Ganzinger, *Rewrite-based Equational Theorem Proving with Selection and Simplification*, JLC 1994, [DOI](https://doi.org/10.1093/logcom/4.3.217) | Resolution, factoring, equality resolution/factoring and superposition in `mrs-calculus`; parent instances are replayed by `mrs-proof-kernel`. | The ordinary engine has no proved dynamic completeness argument for the combined selection, simplification, indexes, splitting and resource policies. Ordered-inference certification is a separate bounded fragment. | Calculus tests, whole-parent substitution mutations, `high_arity_resolution_refutes_and_replays`, supported Verus decoded-IR subset. |
| Schulz, *E: A Brainiac Theorem Prover*, 2002 | Given-clause processed/passive loop in `mrs-search/src/given_clause.rs`; worker-bounded schedules in `strategy.rs`. | A given-clause implementation is not a reproduction of all E/DISCOUNT machinery. Weight-only policies need not be fair; pruning and budgets are explicit. | Selector queue-recall tests; all ordinary end-of-search cases fail closed. |
| Voronkov, *AVATAR: The Architecture for First-Order Theorem Provers*, CAV 2014, [DOI](https://doi.org/10.1007/978-3-319-08867-9_46) | Variable-disjoint components, conditional SAT constraints, unioned inference assumptions, branch blockers and active/dormant synchronization; `avatar.rs` and `given_clause.rs`. | Positive-only component naming, no complete first-order model claim, and no proof of whole-engine fairness. These are not statements that every omission is unsound. SAT is CaDiCaL, not the retired varisat backend. | Producer splitting tests; strict and competition proof-wide independence/naming/discharge gates; stored adversarial fixtures in `mrs-proof-kernel/tests/resources/avatar/`. |
| Reger, Suda & Voronkov, *Playing with AVATAR*, CADE-25 **2015**, [DOI](https://doi.org/10.1007/978-3-319-21401-6_28) | Selected architectural ideas, not a whole-paper reproduction. | Unit clauses are not generally abstracted by the splitter; complementary ground components have independent positive names. More aggressive conditional simplification/restoration is not established. | Tied-shape canonicalization tests, dormant/demodulation regression. |
| McCune, *Experiments with Discrimination-Tree Indexing and Path Indexing for Term Retrieval*, JAR 1992, [DOI](https://doi.org/10.1007/BF00245458) | `DTreeId` and `STreeId` provide candidate retrieval; callers check exact matches. Arity uses `usize`, without the former 255/256 truncation. | Unification retrieval is **imperfect**, not a perfect index. `STreeId` is a path-compressed discrimination tree, not Graf-style substitution-tree indexing. | Independent linear-oracle recall/removal tests at arities 0, 1, 254, 255, 256, 257, 511, 512. |
| Schulz, *Simple and Efficient Clause Subsumption with Feature Vector Indexing*, workshop 2004 / revised 2013 [DOI](https://doi.org/10.1007/978-3-642-36675-8_3) | Fixed symbol buckets and polarity/width features, hierarchical candidate tree in `mrs-index/src/fvi.rs`. | A fixed-dimensional comparison is not constant-time whole-index retrieval. Set-style matching and width/backtracking caps may miss reductions. | FVI/FVT tests and `index_equivalence`; SAT-vs-legacy bounded differential tests. |
| Orphan elimination / global subsumption | Dependency tracking and startup descendant eviction in `SearchState`. | The only located eviction call is startup AC removal. Backward subsumption does not blindly erase all descendants. **This is not SAT-based global subsumption.** The former standalone “Orphan Elimination in Vampire (2010)” citation is unverified. | Dependency-registration and destructive-parent replay regressions. |
| Hoder & Voronkov, *Sine Qua Non for Large Theory Reasoning*, CADE 2011, [DOI](https://doi.org/10.1007/978-3-642-22438-6_23) | Tolerance-based symbol triggers and bounded-depth traversal in `sine.rs`. | Heuristic premise selection, not a complete original-input decision procedure. Whole-paper parameter/experiment reproduction is not established. | SInE policy/fallback tests and positive-result demotion. |
| Ganzinger & Korovin, *New Directions in Instantiation-Based Theorem Proving*, LICS 2003, [DOI](https://doi.org/10.1109/LICS.2003.1210045) | Bounded pure-relational EPR SAT-guided instantiation with MGU-derived instances and refutation ancestry in `instgen.rs`. | General function-bearing InstGen and equality InstGen are outside this path. Variable-bearing abstract models return `GaveUp`; certified grounding/model tiers are separate. | InstGen variable/polarity regressions and kernel proof replay. |
| Kiesl, Suda, Seidl, Tompits & Biere, *Blocked Clauses in First-Order Logic*, LPAR 2017, [paper](https://arxiv.org/html/1702.00847) | `mrs-prephase/src/preprocessing/bce.rs` checks all nonempty jointly unifiable opposite-literal subsets (L-resolvents) within explicit bounds. | Exhaustive bounded enumeration, **not** the paper's polynomial implementation. At most 12 opposite literals per partner and 4096 subset checks per candidate clause; deadline/cap exhaustion cannot authorize deletion. Unification pairs are bounded to 128 variables / 10,000 term nodes (32 nodes when function-bearing) to stay below legacy substitution fallbacks. Equality-blocking is absent; BCE is disabled in the presence of equality. | Paper Example 4 retained and refuted with kernel certification; exhaustive two-element model comparison on 49 clause pairs; bound/refusal tests. |
| AC-superposition / AC unification | AC-aware ordering helpers, normalization, syntactic unification first, and bounded restricted theory matching/replay. | Not complete AC-superposition or a complete AC-unifier enumeration. The old claim that no AC-aware orderings exist is stale; existence of those helpers does not prove the calculus complete. The old unnamed paper citation requires bibliographic verification. | Theory-unifier mutation/refusal tests; ordinary-superposition recovery regressions; `unresolved-issues.md` UI-1. |
| ENIGMA-style / Deepire comparison | Optional fixed-feature Burn MLP in `mrs-core/src/ml/model.rs` and experimental search guidance. | **Inspired infrastructure**, not a reproduction of Deepire, ENIGMA Anonymous, GNN or GBDT pipelines. The old arXiv Deepire citation was unrelated and has been removed. | Feature/model shape tests; guidance off by default; no claimed current portfolio gain. |
| Goertzel et al., *Fast and Slow Enigmas and Parental Guidance*, 2021, [paper](https://arxiv.org/html/2107.06750) | Optional parent-pair prediction of whether an inference yields a result. | Different labels from proof/selected-clause usefulness; omissions rather than the paper's freezer/revival; no GPU-server/GNN/GBDT combination. Pruned strategies cannot claim saturation. | Feature/label tests and explicit incompleteness telemetry. |
| Smallbone, *Twee: An Equational Theorem Prover*, CADE 2021, [DOI](https://doi.org/10.1007/978-3-030-79876-5_35) | Ground goal-subterm naming and definition/proof transport in `mrs-cnf/src/goal_transform.rs`. | A scoped preprocessing adaptation, not Twee's complete proving architecture. | Recursive/maximal transformation and strict replay tests. |
| Tseitin / Plaisted–Greenbaum directional CNF | Fresh definition predicates carry all free variables; polarity chooses half-definitions or biconditionals in `mrs-cnf/src/definitional.rs`. | Supported first-order lowering/clausification, not formal verification of TPTP bytes or every source dialect. | Polarity, nested-equivalence, freshness and strict definition-mutation tests. |
| Coutelier et al., *SAT-Based Subsumption Resolution*, CADE 2023, [paper](https://arxiv.org/html/2401.17832) | Opt-in combined ordinary-subsumption/SR mode using sparse matches, compatible bindings and a unique target pivot in `sat_subsumption.rs`. | CaDiCaL Boolean encoding instead of the paper's substitution-aware solver. Existing MRS set-style plain matching differs from paper multiset subsumption. 25 ms / 512-literal / encoding caps; indexed guards can miss reductions. | 54 legacy-oracle combinations, equality/pivot/assumption tests, deadline/cancellation refusal and kernel-certified wide SR. |

References used only for comparisons (GDV, Ivy, LambdaPi, external systems) are
not implementation claims. The formal checker covers its declared decoded-IR
subset and **does not cover AVATAR or prove search completeness**.

## Case-split certification contract

Conjecture polarity is checked proof-wide too: a displayed positive conjecture
is not an axiom. Its positive dependence cannot justify a final refutation.
Direct or nested `assume_negation` must refer to a source-bound conjecture,
never an axiom or arbitrarily relabeled derived formula. Proper conjecture
negation is recomputed even by the structural-only gate.

Strict certification requires variable independence across components, a
proof-wide consistent definition for each SAT name, correct inherited contexts,
and an unconditional discharged false root. A branch-local false node is not
a final refutation, including when transported through an ordinary alias rule.
Trace replay binds **every** original SAT clause to validated evidence;
duplicates of justified clauses and tautologies are allowed, invented premises
are not. Sparse names are densified for trace-free SAT replay. Reused component
equivalence is independently bounded (512 literals and the comparison budget),
not trusted from the producer's key. Competition mode applies the same
structural gate in addition to its ordinary inference checks.

Producer canonicalization retains the cheap path for unambiguous shapes.
Tied shapes use bounded variable-numbering enumeration (4096 permutations,
512 literals); a refused key receives a fresh name and cannot merge unequal
components. Marker allocation does not overwrite existing input symbols.
Neither strict replay nor this naming optimization proves portfolio completeness.

## Further architectural work: assessed, not silently promoted

After the soundness repairs, the next candidates are polynomial L-resolvent
checking, signed complementary ground naming/unit abstraction, and better
component canonicalization for large ambiguous bodies. Each must preserve
proof transport and pass the negative canaries before evaluation. A broader
orphan policy needs a redundancy/fairness argument before changing deletion.
Graf-style indexes, general InstGen, full AC calculus, and paper-style ML
pipelines are separate projects, not prerequisites hidden behind a checkmark.

The widened Cell layout and bounded canonicalization change cost/trajectories;
no speed or coverage gain is claimed from this audit. Shipped schedules and
routing rules are not promoted. Performance decisions require paired,
strictly audited, held-out cooperative campaigns under the canonical
eight-physical-core policy, not extrapolation from local canaries.
