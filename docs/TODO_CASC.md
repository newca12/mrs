# TODO: CASC Competition Roadmap

This document tracks what remains to be built in `mrs` (the prover) to maximise the CASC score. Items are ordered by expected ROI against the CASC-30 division breakdown.

---

## Already Implemented (no longer blocking)

| Item | Commit |
|------|--------|
| SInE fallback on sub-second saturation | `91fa84e9` |
| EPR naive grounding disabled; AVATAR handles EPR | `fe83c4f6` |
| Global Subsumption & Orphan Elimination | `ceeb5805` |
| Heuristic AC-matching + axiom elimination | `d8d49118` |
| Perfect DTree (binding consistency in unify_flat) | `efd4c502` |
| Parallel 11-strategy portfolio with stop-flag | `34338df3` |
| SmallVec for TermNode/IdAtom | `c83e01c6` |

---

## Remaining Work (ordered by expected CASC impact)

### 1. Clause Sharing Across Parallel Strategies
**Impact:** Every division. High.

Currently each of the 11 parallel strategies starts from a clean `SearchState` and never communicates with siblings. If strategy 3 derives a short unit equality that would simplify 10,000 passive clauses, strategies 4–11 never learn of it.

**Implementation sketch:**
- After each given-clause iteration, collect newly derived unit equalities.
- Broadcast them via a `crossbeam::channel` or `std::sync::Mutex<Vec<IdClause>>` to a shared "demodulator pool".
- All threads consume from the pool at the top of each iteration.
- Difficulty: `TermBank` is not shared (varisat is not `Send`). Short-term: share a serialized form of unit equalities (the two `Term` sides + source); each thread re-interns them into its own bank. Long-term: replace `varisat` with a `Send`-compatible SAT solver to allow a shared `Arc<RwLock<TermBank>>`.

### 2. Full AC-Superposition (AC-compatible Term Orderings)
**Impact:** UEQ, FEQ. High.

The current AC-matching heuristic (`unify_ac_id`) stops the permutation explosion but does not make the search *complete* modulo AC. Standard KBO/LPO cannot soundly orient equations like `f(a, f(b, c)) ≥ f(f(a, b), c)` because associativity makes the comparison undefined. Without an AC-compatible ordering (AC-KBO or AC-RPO), the prover may silently discard necessary inferences.

**Implementation sketch:**
- Implement AC-KBO: weighting is the same but the comparison step uses the flattened form.
- Gate it on whether any AC axiom was detected: if `assoc_symbols` is non-empty, switch ordering to `TermOrdering::CustomACKBO`.
- Reference: [Bachmair & Ganzinger, 1994; Rubio & Nieuwenhuis, 1993 for AC-RPO].

| Replace `varisat` with a `Send`-Compatible SAT Solver | `bb46b4d6` |

### 4. Substitution Trees (Indexing Evolution)
**Impact:** FEQ, FNE. Medium.

The current `DTreeId` is a perfect discrimination tree — it eliminates false positives via binding consistency checks. However, D-Trees duplicate shared prefixes: if 10,000 clauses all start with `f(g(`, each path independently stores that prefix. Substitution Trees merge these contexts into a DAG, drastically reducing memory footprint for large clause sets (common in FEQ with 400 problems at up to 120,000 derived clauses).

**Reference:** [Graf, 1996; Schulz & Sutcliffe, various].

### 5. Machine-Learning Guided Clause Selection (ENIGMA/Deepire)
**Impact:** FNE, FEQ. Medium (high ceiling, high effort).

`mrs` selects clauses by static Age/Weight/GoalDistance ratios. E-prover uses ENIGMA (gradient-boosted decision trees) trained on past proof traces; Vampire uses Deepire (graph neural network). Both double the solve rate on FNE/FEQ compared to their static baselines.

**Implementation sketch (minimal viable):**
- Collect a training set: for each solved problem, label the clauses on the refutation path as "useful" and a random sample of passive clauses as "not useful".
- Train an XGBoost model on feature vectors (clause weight, literal count, goal distance, symbol frequencies).
- At selection time, use the model score to reweight the `BinaryHeap` priority.
- Training data accumulates from `casc.sh` benchmark runs.

### 6. SInE Threshold Tuning
**Impact:** FNE, FEQ. Low-Medium.

The SInE fallback (restart on <1s saturation) is a binary switch. A finer approach would try multiple SInE tolerance levels in parallel: one strict, one relaxed, one disabled — each as a separate portfolio strategy. The per-division CASC run data from `benchmarks` can guide threshold selection.

### 7. Performance: FxHashMap for Internal HashMaps
**Impact:** All divisions. Low effort, 5–15% global speedup.

Rust's default `HashMap` uses SipHash (cryptographically secure, DoS-resistant). In a theorem prover, keys are small integers (ClauseId, VarId, TermId) not adversarial strings. Swapping to `rustc_hash::FxHashMap` or `ahash::AHashMap` typically yields 10–15% speedup for free.

**Implementation:** Add `rustc-hash` to workspace dependencies, define a type alias `type HashMap<K,V> = rustc_hash::FxHashMap<K,V>`, and replace all `std::collections::HashMap` uses inside `mrs-core`, `mrs-index`, `mrs-search`, `mrs-calculus`.

---

## CASC Division Priority Map

| Division | Problems | Current (c0816a7a) | Highest-ROI fix |
|----------|----------|-------------------|-----------------|
| FEQ | 400 | 27 (7%) | Clause sharing, AC-Superposition |
| FNE | 100 | 24 (24%) | Clause sharing, SInE tuning |
| UEQ | 300 | 13 (4%) | AC-Superposition, clause sharing |
| EPS | 100 | 13 (13%) | AVATAR EPR (already improved) |
| EPU | 100 | 8 (8%) | AVATAR EPR (already improved) |
| ICU | 101 | 1 (1%) | Orphan elimination (done), clause sharing |

*Scores based on commit `c0816a7a` at 120s; newer commits expected to improve all divisions.*
