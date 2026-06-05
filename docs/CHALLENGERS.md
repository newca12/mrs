# Competitor Evaluation Strategy

From a strategic engineering perspective, the question of *when* to benchmark against competitors like GDV, Nörgler, and VTV has changed since the original analysis. The two fatal flaws that previously made comparison premature have now been resolved:

- **Definition laundering (−10 penalty):** The `introduced(definition)` structural check in `crates/mrs-proover/src/checks/introduced_definition.rs` now enforces strict `is_naming_clause` validation on the formula body, not just symbol freshness. All six `evil-proofs/exploits/evil_definition_*` cases are caught.
- **Conservative Unknown returns:** `vampire_skolemisation.rs` now returns `StepOutcome::Unsound` (not `Unknown`) on arity drops and shape violations, securing `+2` points instead of `0`.

The main remaining gap before benchmarking is **AC-equivalence matching** in `axiom_leaf.rs` — without it, valid proofs where E or Vampire reorders disjuncts (`p | q` → `q | p`) silently score `0` instead of `+1`.

## Current Competitive Position

| Weakness | Status |
|----------|--------|
| Definition laundering (`−10` risk) | ✅ **Fixed** |
| Over-conservative Unknown returns (`+2` leakage) | ✅ **Fixed** for Vampire Skolemization |
| AC-equivalence in leaf matching (`+1` leakage) | ❌ **Not fixed** — see `TODO_PROOVER.md` |
| Recursive/cyclic definition chain | ❌ **Not implemented** |
| E-prover Skolemization variable leakage | ❌ **Possibly vulnerable** |

## Competitor Landscape

### GDV (Geoff's Derivation Verifier)
*The Incumbent Champion.* Battle-tested over years of chaotic ATP output. Handles AC-rewriting, complex Skolemization chains, and edge cases natively. The benchmark to beat.

### Nörgler
*The Modern High-Performance Challenger.* Open-source, supports parallelization, poses a significant threat on the average-time tie-breaker. **Easiest modern competitor to compile** — recommended as the first external benchmark to install.

### GDV-LP (Dedukti / LambdaPi)
*The Formally Certified Approach.* Translates TSTP proofs to LambdaPi for kernel-level verification. Theoretically immune to `−10` evil-proof traps. High build friction (OCaml + Dedukti toolchain).

### VTV (Verified TESC Verifier)
*The Provably Correct Challenger.* Written and verified in Agda. Guarantees 100% soundness, will never take a `−10` hit. Forces competitors to aggressively score `+2` to keep pace. Very high build friction (Agda + Haskell).

## Recommended Path Forward

1. **Fix AC-equivalence in `axiom_leaf.rs`** — this is the last structural gap leaking easy `+1` points on every valid proof where an ATP reorders a conjunction.
2. **Install Nörgler** as an external baseline once step 1 is done. It is the easiest modern tool to compile and gives a wall-clock reference against `mrs-proover`.
3. **Address recursive/cyclic definition chains** — an adversary could launder a contradiction through `p ⟺ ¬q` then `q ⟺ p`. The verifier needs a well-foundedness check across all introduced symbols.
4. **Benchmark against GDV** once Nörgler comparison is clean. GDV is the gold standard for final pre-competition scoring.
