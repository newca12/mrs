# TPTP Challenges: Proving & Verification Limits

This document catalogs notable TPTP problems and families of problems that
present unique challenges under search (proving) or step-by-step verification.
Statuses below are local observations, tied to the commands and revisions
described in the repository; they are not official CASC results.

---

## 1. Unresolved & Unverified Challenges

These problems remain difficult to solve within the tested wall-clock limits, or
produce proof steps that the current verifier cannot positively confirm. A
neutral `Unknown` is expected when a step is inconclusive; it is not evidence
that the underlying TPTP problem is false.

### Verifier Limits: Large Resolution Steps

#### SET/SET102-7.p & SET/SET841-1.p
* **SZS Status:** Unsatisfiable
* **Status in prior local reports:** `Unknown`; not independently reproduced
  here because no corresponding proof artifact is tracked in this repository
* **Technical Reason:**
  These problems have large axiomatizations and can produce large proof-step
  queries. In the competition verifier, independent ATP-bound steps are
  prepared serially and checked in parallel; each query receives a share of the
  overall budget, subject to the configured per-step cap. The cited `Unknown`
  status should be treated as configuration-specific unless reproduced with a
  recorded proof, commit, and command.
* **Soundness & Scoring Safeguard:**
  Returning `Unknown` on these steps is a safe, conservative fallback. In the
  repository scorer, an `Unknown` outcome contributes zero points; it does not
  assert that the proof step is invalid.

---

### Verifier Limits: Non-Prenex Skolemization

#### NUN/NUN055+2.p & NUN/NUN068+2.p
* **SZS Status:** Theorem
* **Observed verifier status:** `Unknown` for the cited local proofs
* **Technical Reason:**
  These proofs contain nested existential formulas inside non-prenex contexts.
  An `esa` Skolemization step is an equisatisfiability transformation, not a
  general logical entailment, so the ATP fallback cannot certify it merely by
  proving `parent |= conclusion`. The verifier has structural support for a
  number of nested and matrix-level Skolemization shapes, but when the current
  matcher cannot reproduce the exact transformation it conservatively returns
  `Unknown`.
* **Soundness & Scoring Safeguard:**
  This preserves the distinction between “not confirmed” and “shown
  unsound”; the current result is a verifier coverage limitation, not a claim
  that all non-prenex Skolemization is inherently unsupported.

---

### Solver Limits: Heavy Equational Search Loops

#### BOO/BOO007-4.p
* **SZS Status:** Unsatisfiable
* **Observed status:** `mrs` generated a refutation with strategy 1 in about
  106 seconds; `mrs-proover --only-mrs` remained `Unknown` at both 30 and 300
  seconds on that fresh proof.
* **Technical Reason:**
  `BOO007-4.p` is a small but difficult equational search case. A fresh
  strategy-1 run on TPTP v9.3.0 produced a 107-step refutation in about 106
  seconds. The generated proof contains no `spl0_*` AVATAR markers, and the
  current in-process `MrsAtp` backend did not decide one of its superposition
  steps even with a 300-second verifier budget. The remaining issue is therefore
  not evidence of the `MrsAtp` AVATAR-marker interaction described elsewhere;
  it is an equational ATP/search completeness or performance gap.

---

## 2. Solved & Verified Milestones

These are recorded local milestones, not claims of universal or
cross-machine reproducibility.

### In-process AVATAR Marker Handling

#### ALG/ALG014+1.p & ALG/ALG036+1.p
* **SZS Status:** Theorem
* **Observed verifier status:** Fresh strategy-7 proofs were `VerifiedGood`
  with the `MrsAtp` AVATAR toggle from commit `bcc9918`; the same proofs were
  `Unknown` before that toggle.
* **Technical Reason:** The generated proofs contain explicit `spl0_N` literals,
  which are ordinary 0-ary first-order predicates in the ATP query. Re-running
  AVATAR inside `MrsAtp` can split clauses containing these variable-disjoint
  literals again, allocating new internal split variables and changing the
  search path needed to close the query. Setting `use_avatar: false` for the
  two inner verification strategies preserves those proof literals for normal
  first-order inference. This is scoped to the in-process step checker; it does
  not disable AVATAR in the main prover.
* **Evidence:** On TPTP v9.3.0, `MRS_SINGLE_STRATEGY=7` generated both proofs;
  `mrs-proover --only-mrs --workers 1` verified each as `VerifiedGood` after the
  change and returned `Unknown` on a demodulation/superposition step before it.

---

### Hardware & Timing Dependency Limits

#### ALG/ALG032+1.p
* **SZS Status:** Theorem
* **Recorded status:** `VerifiedGood` in a prior local configuration using
  logical LRS budgets
* **Technical Reason:**
  Highlights the danger of LRS (Limited Resource Strategy) pruning using
  wall-clock timing:
  - On fast CPUs, iterations are quick, and LRS estimates a generous remaining budget, leaving critical proof-reaching clauses intact.
  - On slow CPUs (e.g., non-AVX Xeons), high iteration times spike the estimated `avg_nanos`. LRS can then prune the passive queue down to its minimum, **deleting clauses reported as critical to a proof at iteration 229**. Without those clauses, the search can grow expensive and time out.
* **The Solution:**
  Opt in to logical LRS limits (`export MRS_LRS_FIXED_ITERATIONS=100000`) for
  deterministic experiments. This does not guarantee that every problem will
  be solved, nor that all other sources of runtime variation disappear.

---

### Deep-Term Successor Nesting

#### NUM/NUM283-1.005.p
* **SZS Status:** Theorem
* **Observed verifier status:** `VerifiedGood` for the archived proof with the
  current 200-depth limit
* **Technical Reason:**
  In the `NUM` domain, arithmetic is represented algebraically using nested
  successor notation. The archived `NUM283-1.005` proof reaches successor term
  depths well above the old 25-depth guard. Raising the guard to 200 allows the
  archived proof to verify, although the full ATP path is still dependent on
  the selected backend and budget.

---

### Safeguard & Soundness Milestones

#### PRO/PRO013+3.p
* **SZS Status:** CounterSatisfiable
* **Status:** soundness regression milestone; not a current standalone
  `VerifiedGood` benchmark claim
* **Technical Reason:**
  `PRO013+3.p` is a counter-satisfiable TPTP problem, not a theorem to be
  reported as `VerifiedGood`. Its role in the project was as a soundness
  incident/regression target: it exposed AVATAR/CWA polarity and substitution
  risks and led to stricter branch checks and adversarial tests. The relevant
  invariant is that mutated or forged proofs must not receive a false positive,
  not that this problem itself is a verified theorem.
