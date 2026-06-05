# The ProoVer 2026 Competition Landscape

The **ProoVer Competition** (Proof Verifier Competition) is a premier event within the TPTP/TSTP ecosystem designed to benchmark the correctness, robustness, and performance of automated proof checkers. Debuting at IJCAR/FLoC 2026, it tests systems against a mix of valid proofs and intentionally falsified ("evil") proofs.

## 1. Core Competition Rules & Scoring

### Input Format
* **Problems:** First-Order Form (FOF).
* **Proofs:** TSTP (Thousands of Solutions from Theorem Provers) derivation format.
* **Scope:** Restricted to `axiom`, `conjecture`, `negated_conjecture`, and `plain` roles. All proofs are refutation proofs (ending in `$false`).

### Scoring System
The scoring is highly asymmetric, heavily punishing unsound verification:
* **+2 Points:** Correctly identifying a bad proof (`FailedVerified`).
* **+1 Point:** Correctly identifying a good proof (`Verified`).
* **0 Points:** Giving up / timing out / unknown (`NotVerified`).
* **-1 Point:** Falsely rejecting a good proof.
* **-10 Points (Fatal):** Falsely verifying a bad proof as good.

### Hardware & Limits
* Execution on a StarExec node (Octa-core Xeon, 128GB RAM).
* 30 seconds of wall-clock time per problem.

## 2. The Verification Pipeline

To succeed, verifiers (like `mrs-proover`) typically employ a **Hybrid Verification** strategy:
1. **Structural Verification:** Fast, internal checks for formatting, acyclicity, leaf provenance, and specific TSTP rules (like Skolemization and definition introduction).
2. **Semantic Verification:** Delegating non-trivial deductive steps (e.g., `resolution`) to trusted external Automated Theorem Provers (ATPs) like E or Vampire, checking if the child formula is a logical consequence of its parents.

## 3. Potential Challengers & State of the Art

If `mrs-proover` hopes to win, it must overcome these established and cutting-edge systems:

### A. GDV (Geoff's Derivation Verifier)
* **The "Incumbent Champion"**: Developed by Geoff Sutcliffe, GDV is the gold standard for TSTP verification. 
* **Strengths:** Battle-tested against years of chaotic ATP outputs. It expertly handles edge cases, Associative-Commutative (AC) rewriting, and complex Skolemization chains.

### B. Nörgler
* **The "Modern High-Performance" Challenger**: An open-source certificate checker built for speed.
* **Strengths:** Supports parallelization strategies to verify massive proofs incredibly quickly. It poses a significant threat on the "average time" tie-breaker.

### C. GDV-LP (Dedukti / LambdaPi)
* **The "Formally Certified" Approach**: Translates TSTP proofs into primitive-recursive formats verifiable by the LambdaPi proof assistant.
* **Strengths:** Mathematically proven soundness. It relies on a tiny, trusted kernel rather than delegating to massive, heuristic ATPs. It is theoretically immune to the `-10` point "evil proof" traps.

### D. VTV (Verified TESC Verifier)
* **The "Provably Correct" Challenger**: Written and formally verified in Agda.
* **Strengths:** Guarantees 100% soundness against first-order semantics. It will never make a `-10` point mistake, forcing competitors to aggressively score `+2` points to keep up.

## 4. Strategic Imperatives for `mrs-proover`

Based on the SOTA and the competition rules (see `WEAKNESS.md`), `mrs-proover` must adopt the following strategies to remain competitive:

1. **Absolute Soundness on Structural Rules:** Competitors like VTV and GDV-LP will not fall for definition laundering or scope shadowing. `mrs-proover` must rigorously validate the internal logic of `introduced(definition)` steps, not just the freshness of symbols, to avoid the fatal `-10` penalty.
2. **Aggressive Refutations:** `mrs-proover` currently outputs `NotVerified` (0 points) for structural anomalies to play it safe. To beat GDV, it must confidently assert `FailedVerified` (+2 points) when TSTP rules (like arity tracking during Skolemization) are violated.
3. **AC-Equivalence Matching:** Real-world ATPs reorder commutative operators constantly. To secure the easy `+1` points on good proofs, the verifier must implement AC-matching rather than relying on strict positional alpha-equivalence.
