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

### Implemented: AVATAR proof self-containedness and incomplete splitting citations

- **Resolved**: `extract_proof` and `extract_proof_ids` BFS traversal now follows `ClauseCertificate` dependencies (`split_nodes`, `branch_roots`, `split_parent`) alongside `ClauseSource::Inference.parents`.
- **Proof format & Verification**: The proof exporter outputs the full AVATAR TSTP annotation chain (`avatar_split_clause`, `avatar_component_clause`, `avatar_branch_refutation`, `avatar_sat_refutation`), ensuring step-by-step self-containedness. Both `mrs-proof-kernel` and `mrs-proover` verify AVATAR-heavy proofs.

### Follow-up: audit `fvo.rs` with the same rigor as the CWA polarity fix

The CWA polarity bug (`PRO013+3.p`) survived because it was a narrow, rare
code path with a hand-written `**Soundness**` comment and thin (synthetic-
only) test coverage — the bug was never actually triggered by anything in
our audit corpus, only found via manual code review. Confirmed 2026-07-18
that `fvo.rs` (FNE-Variable-Only propositional-skeleton refutation) is the
only other module in `mrs-search`/`mrs-calculus` with the same risk shape
(hand-written soundness justification, narrow trigger conditions). A quick
structural read of `lift_clause`'s variable-freshness handling found no
concrete issue, but it hasn't had the same adversarial review CWA got
(hunting specifically for polarity/variable-sharing/lifting-correctness
edge cases). Do this before the next soundness-sensitive release.

### Follow-up: coverage tracking for narrow soundness-critical code paths

`TRACE_CWA_POLARITY=1` (added 2026-07-18, `crates/mrs-search/src/cwa.rs`)
lets you check whether a sweep actually exercises CWA's polarity-sensitive
path at all, rather than silently passing without ever having exercised it.
Consider generalizing this into an actual coverage-tracking script (grep
`TRACE_CWA`/`TRACE_CWA_POLARITY`/`TRACE_AVATAR`-style logs across a sweep
and report which narrow, soundness-sensitive code paths fired zero times)
so a *silent* coverage gap becomes a *visible* audit signal, instead of
only being found by manual code review after the fact.

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
