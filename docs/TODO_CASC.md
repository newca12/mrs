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
**Status (2026-06-24): infrastructure built and validated, but NOT shipped —
ML guidance does not yet beat the static portfolios.** See the "ML-guided
clause selection — investigation" section in `docs/BENCHMARKS.md`.

What works now:
- End-to-end pipeline: `collect_ml_data.sh` → `.wincode` traces → `mrs-train`
  (Burn MLP) → `models/weights_<div>.bin` → `--ml-weights` inference.
- `mrs-train` was broken (1 epoch, valid==train, no imbalance handling →
  degenerate model); now fixed: epochs + early stopping, stratified split,
  class rebalancing, AUC/PR metrics. Validation **AUC ~0.5 → 0.84–0.89**.

Open problems (the actual blockers, ordered):
1. **Objective/integration alignment.** A good offline classifier (AUC 0.84)
   made FEQ *worse* (81 static → 54 ml). Selection is `0.3·weight +
   0.7·(1−σ(score))` — ML drives 70% of selection; the proof-membership label
   is hindsight/survivorship and shifts distribution vs live search.
   - Experiment A (DONE, negative): raised `ml_feq` `alpha` 0.1–0.5 → 0.85 so
     ML only lightly refines the proven ordering. **No effect — FEQ stayed at
     54.** The gap is the schedule composition, not the blend weight.
   - Further ideas (untried): tie-breaker-only blending; calibrate via higher
     `--neg-per-pos`; iterative/DAGGER-style trace collection from the prover's
     own runs; better features (clause-graph / parent context).
2. **Homogeneous `ml_fne`/`ml_ueq`/`ml_epr` schedules** discard portfolio
   diversity (~8 clones of one ML strategy) and regress regardless of model
   quality. If/when ML is shown to help on the `ml_feq` diverse chassis,
   rebuild these to mirror it (diverse `casc`-style chassis + `MlGuided`).

Original implementation sketch (kept for reference):
- Collect a training set: for each solved problem, label the clauses on the refutation path as "useful" and a random sample of passive clauses as "not useful".
- Train a model on feature vectors (clause weight, literal count, goal distance, symbol frequencies).
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
