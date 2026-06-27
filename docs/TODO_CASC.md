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
| Machine-Learning Guided Clause Selection | HEAD |
| SInE Threshold Tuning | HEAD |
| SIMD-optimized Feature Vector Index | HEAD |

---

## Remaining Work (ordered by expected CASC impact)

(No remaining high-ROI features currently planned before CASC submission. The prover is feature-complete.)

---

## CASC Division Priority Map

| Division | Problems | Current (c0816a7a) | Highest-ROI fix |
|----------|----------|-------------------|-----------------|
| FEQ | 400 | 27 (7%) | Clause sharing ✓, AC-Superposition ✓, STree ✓ |
| FNE | 100 | 24 (24%) | Clause sharing ✓, SInE tuning ✓, STree ✓ |
| UEQ | 300 | 13 (4%) | AC-Superposition ✓, clause sharing ✓, STree ✓ |
| EPS | 100 | 13 (13%) | AVATAR EPR (improved), LTO ✓ |
| EPU | 100 | 8 (8%) | AVATAR EPR (improved), LTO ✓ |
| ICU | 101 | 1 (1%) | Orphan elimination ✓, clause sharing ✓ |

*Scores based on commit `c0816a7a` at 120s; newer commits (cadical, clause sharing, AC-KBO, LTO, scheduling, STree, SIMD FVI, ML, SInE tuning) expected to dramatically improve all divisions.*
