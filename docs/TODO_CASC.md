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
| Clause Sharing Across Parallel Strategies | `78f00212` |
| Full AC-Superposition (AC-KBO ordering + dynamic switching) | `83a93216` |
| Replace `varisat` with CaDiCaL (`Send`-compatible SAT solver) | `6f1b1f54` |
| Optimize parallel portfolio scheduling for hardware cores | `82427ec9` |
| LTO (`fat`) + native CPU instruction set (`-C target-cpu=native`) | `349d6470` |
| ProoVer 2026: skolemize free-var safety, AnnotatedFormula API | `619e2bcc` |
| Substitution Trees: path-compressed `STreeId` replaces `DTreeId` | HEAD |
| Performance: FxHashMap for Internal HashMaps | HEAD |

---

## Remaining Work (ordered by expected CASC impact)

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

The SInE fallback (restart on <1s saturation) is a binary switch. A finer approach would try multiple SInE tolerance levels in parallel: one strict, one relaxed, one disabled — each as a separate portfolio strategy. The per-division CASC run data from `docs/BENCHMARKS.md` can guide threshold selection.

### 8. SIMD-optimized Feature Vector Index — **+2% performance**
**File:** `crates/mrs-index/src/fvi.rs`

**The Problem:** The current `can_subsume` check uses a linear scan over a sparse `Vec<(SymbolId, u32)>`. While fine for small clauses, as clause sets grow in FEQ/FNE, the overhead of this linear scan becomes a bottleneck.

**Implementation:**
- Convert `FeatureVector` to a dense, fixed-size array (e.g., `[u16; 64]`) tracking the most common symbols.
- Vectorize `can_subsume` using `std::simd` (AVX2-compatible) to compare 16-32 symbol counts in a single instruction.
- Ensure compatibility with CASC hardware (AVX2, not AVX-512) to avoid SIGILL crashes.

---

## CASC Division Priority Map

| Division | Problems | Current (c0816a7a) | Highest-ROI fix |
|----------|----------|-------------------|-----------------|
| FEQ | 400 | 27 (7%) | Clause sharing ✓, AC-Superposition ✓, STree ✓ |
| FNE | 100 | 24 (24%) | Clause sharing ✓, SInE tuning, STree ✓ |
| UEQ | 300 | 13 (4%) | AC-Superposition ✓, clause sharing ✓, STree ✓ |
| EPS | 100 | 13 (13%) | AVATAR EPR (improved), LTO ✓ |
| EPU | 100 | 8 (8%) | AVATAR EPR (improved), LTO ✓ |
| ICU | 101 | 1 (1%) | Orphan elimination ✓, clause sharing ✓ |

*Scores based on commit `c0816a7a` at 120s; newer commits (cadical, clause sharing, AC-KBO, LTO, scheduling, STree) expected to improve all divisions.*
