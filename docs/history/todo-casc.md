# TODO: CASC Competition Roadmap

> Status: Superseded task ledger. Current work should use source-derived
> references and new dated benchmark reports.

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
| Twee-Style Goal-Directed Preprocessing for UEQ | HEAD |

---

## Remaining Work (ordered by expected CASC impact)

### High Priority: Mitigate Portfolio Run Jitter & Score Variance

#### 1. Deterministic LRS Pruning (Wall-Clock Sensitivity)
- **Problem**: The Limited Resource Strategy (LRS) pruning algorithm calculates its passive queue target size using real wall-clock elapsed time (`start.elapsed()`). Under SMT thread contention or heavy CPU sharing, iteration times inflate, causing the prover to estimate a much smaller number of remaining iterations. This leads to overly aggressive passive clause pruning, permanently discarding proof-relevant clauses and causing search paths to non-deterministically transition from `Refutation` to `GaveUp`.
- **Proposed Mitigations**:
  - **Thread-specific CPU Time (Linux)**: Query thread-local CPU time (`libc::CLOCK_THREAD_CPUTIME_ID`) instead of wall-clock time to ignore context switches and pipeline contention.
  - **Deterministic Virtual Time**: `SearchConfig::lrs_policy` now supports an opt-in `FixedIterations` budget for deterministic experiments. Set `MRS_LRS_FIXED_ITERATIONS=<N>` to apply it to a portfolio run. The default remains wall-clock based until coverage benchmarks justify changing competition behavior.

#### 2. Deterministic Clause Sharing (RwLock Crosstalk)
- **Problem**: Parallel strategies share derived unit equalities via a shared `RwLock<Vec<Clause>>`. Because threads poll and import these clauses asynchronously on every iteration, CPU scheduling fluctuations change the exact iteration at which a thread learns a new unit, leading to divergent, non-reproducible search paths.
- **Proposed Mitigations**:
  - **Interval-based Importing**: Implemented. Shared units are published with logical epochs and imported only at fixed `SearchConfig::shared_pool_poll_interval` boundaries. Sharing is disabled by default; set `MRS_SHARED_POOL_INTERVAL` to a positive interval to enable it, or set it to `0` explicitly for no-sharing controls.
  - **Logical Epochs**: Implemented. Imports are stable-key sorted and deduplicated per search state.

### Implemented: AVATAR proof self-containedness and incomplete splitting citations

- **Resolved**: `extract_proof` and `extract_proof_ids` BFS traversal now follows `ClauseCertificate` dependencies (`split_nodes`, `branch_roots`, `split_parent`) alongside `ClauseSource::Inference.parents`.
- **Proof format & Verification**: The proof exporter outputs the full AVATAR TSTP annotation chain (`avatar_split_clause`, `avatar_component_clause`, `avatar_branch_refutation`, `avatar_sat_refutation`). The independent kernel and competition verifier validate the explicit split/component/branch structure; strict SAT-trace replay remains limited to the bounded certificate shape documented in `reference/trust-and-verification.md`. Legacy CWA roll-ups without explicit metadata are handled conservatively and are not described as fully self-contained.

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

## Codex-Driven Current Priorities

The old division table below was based on commit `c0816a7a` and is no longer a
useful baseline. The current measured baselines are maintained in
[`reports/codex/status-2026-09-11.md`](../reports/codex/status-2026-09-11.md):

| Corpus | Division | Definitive | Evaluated | Rate |
|---|---|---:|---:|---:|
| CASC-30 | FEQ | 112 | 400 | 28.0% |
| CASC-30 | FNE | 43 | 100 | 43.0% |
| CASC-30 | UEQ | 222 | 300 | 74.0% |
| CASC-30 | EPS | 17 | 100 | 17.0% |
| CASC-30 | EPU | 18 | 100 | 18.0% |
| CASC-30 | ICU | 7 | 39 | 17.9% |
| CASC-J13 | FEQ | 72 | 300 | 24.0% |
| CASC-J13 | FNE | 35 | 100 | 35.0% |
| CASC-J13 | UEQ | 257 | 400 | 64.3% |

Failure labels are overlapping diagnostics, not disjoint buckets. A single
problem can be large, EPR, deep, non-Horn, and LRS-heavy at the same time. Do
not add proposed gains from separate labels.

### P0: Measurement And Attribution

- Add a benchmark `run_id` and record commit/hash, command, corpus, division,
  timeout, outer jobs, internal workers, host, TPTP root, and raw output path.
- Parse `% SZS detail` into typed telemetry while retaining the raw detail.
- Distinguish external timeout, internal timeout, LRS-pruned `GaveUp`,
  SInE-subset `GaveUp`, InstGen fallback, parse failure, and resource failure.
- Exclude `profile_complete = 0` from all structural aggregates.
- Report `results.division` and `problem_profiles.casc_division` separately.

**Exit gate:** every benchmark row is reproducible and attributable to one
configuration; every score report states its completeness and telemetry
coverage.

### P0: EPR / InstGen Coverage

The core instrumentation and adaptive pre-pass budget are implemented. The
remaining work is validation and coverage improvement, not a second grounder.

InstGen exists in `mrs-search/src/instgen.rs`; telemetry now records route,
invocation, rounds, instances, SAT size, elapsed time, return reason, fallback
reason, estimated grounding size, and selected budget tier.

- Separate pure relational EPR, EPR with equality, ground EPR, and non-EPR
  rows inside EPS/EPU result divisions.
- Validate those telemetry fields against benchmark CSVs and compare adaptive
  tiers against bounded exhaustive-grounding canaries.
- Tune the adaptive budget based on constants, variables, clauses, and
  estimated grounding size without weakening the fail-fast cap.
- Keep variable-bearing satisfiability conclusions fail-closed and preserve
  UNSAT proof extraction.

**Exit gate:** improve EPS/EPU definitive coverage without wrong definitive
statuses or uncertified satisfiable results. Variable-bearing SAT remains
explicitly fail-closed until a certified model path exists.

### P0: Large-Theory SInE Experiments

SInE and threshold tuning already exist. The next task is controlled tuning on
complete profiles:

- compare unpruned, conservative, standard, and aggressive SInE;
- measure retained axioms, goal connectivity, first useful inference, passive
  size, and LRS interaction;
- preserve at least one unpruned worker;
- keep subset saturation as `GaveUp`.

At the analytical `>500`-axiom threshold, current complete CASC-30 results are
FEQ 9/122 (7.4%) and EPU 2/46 (4.3%).

### P1: Generalize Shared-Clause Exchange

The shared pool currently transports unit equalities and correctly remaps
publisher symbols. Telemetry shows no equality imports in FNE and EPU, and no
imports in EPS. This is a target for an experiment, not proof that sharing is
the sole bottleneck.

- Extend sharing to selected positive/negative unit predicates and ground unit
  lemmas.
- Standardize variables apart and preserve complete ancestor chains.
- Record eligible, published, imported, duplicate, rejected, and used counts.
- Compare equality-only, predicate-unit, and disabled-sharing controls.

### P1: LRS And Queue Resilience

- Record LRS target, queue size before/after pruning, discarded goal-distance
  bands, and the final completeness reason.
- Compare wall-clock LRS, fixed-iteration LRS, disabled LRS, and a measured
  protected goal-connected tier.
- Keep incomplete outcomes fail-closed; no LRS experiment may turn saturation
  of a pruned subset into `Satisfiable` or `CounterSatisfiable`.

### P1: ICU Resource Containment

The current database has one CASC-30 OS-OOM event at 88.6 GB. Add per-strategy
resource telemetry, clause/term ceilings, memory watchdogs, graceful
`ResourceOut`/`GaveUp` handling, and complete worker cleanup.

### P2: FNE Morphology And Non-AC UEQ

- FNE is domain-sensitive: CSR is 23/24 definitive while LCL is 6/37;
  size alone is not a reliable predictor.
- Build held-out morphology portfolios using definite ratio, goal-clause ratio,
  conjecture overlap, non-linearity, clause length, and predicate connectivity.
- For UEQ, retain the 74.0% CASC-30 and 64.3% CASC-J13 portfolios as gates and
  test conservative non-AC/combinator guidance on LCL, COL, RNG, and SWX.

### Soundness And Release Gate

Every performance change must retain:

- fail-closed handling for SInE, ML-pruned, SOS, unit-only, and non-standard
  weight-function saturation;
- strict proof-kernel and adversarial mutation coverage;
- no new wrong-polarity or wrong-definitive results;
- the required Nix-wrapped format, check, clippy, and workspace test passes.

The complete data-driven roadmap, phase gates, and controlled experiment rules
are in [`plan-2027.md`](plan-2027.md#phase-8a-codex-driven-casc-optimization).
