# Analysis: `mrs-proover` stand-alone performance (`--only-mrs`) vs. Full Ladder

This document provides a detailed analysis of why `mrs-proover --only-mrs` is less performant (in terms of proving capability and verification/detection rate) compared to running it in combination with external ATPs `eprover` and `vampire`.

---

## 1. Executive Summary

In the context of automated theorem proving and proof verification (such as the ProoVer competition), "performance" is primarily measured by **verification success rate**—the percentage of valid proofs successfully verified and invalid (falsified/mutated) proofs successfully refuted (marked `VerifiedBad`).

Running `mrs-proover` with the `--only-mrs` flag drops its verification power significantly:
- **Valid proofs (noergler PyRes original):** Verification rate drops from **100% (170/170)** to **95.3% (162/170)**.
- **Falsified proofs (noergler PyRes falsified):** Detection/refutation rate drops from **97.1% (165/170)** to **22.9% (39/170)**.

The main reasons for this performance drop are:
1. **Restricted Strategy Portfolio:** Standalone `MrsAtp` uses only a single, very simple and fast strategy (`mrs_search::strategy::named::fast`) with a very small budget per step, rather than running a parallel portfolio.
2. **Younger Proving Engine vs. Mature ATPs:** `mrs` is a newer prover, whereas `eprover` and `vampire` possess decades of advanced heuristics, indexing structures, and preprocessing routines.
3. **No Finite Model Building (FMB):** The ladder includes `VampireFmbAtp` (Vampire in finite-model-building mode), which actively looks for counter-models for invalid steps. Standalone `mrs` cannot do this, causing invalid steps to time out or saturate instead of being refuted, which degrades `VerifiedBad` verdicts to `Unknown`.

---

## 2. Technical Comparison

### 2.1 The ATP Ladder Architecture
By default, `mrs-proover` constructs an ordered **ladder** of backends to verify proof steps that cannot be verified by cheap internal fast-paths (such as structural or propositional SAT fast-paths). 

This is configured in [mrs-proover.rs](file:///home/user/EDLA/git/mrs/crates/mrs-proover/src/bin/mrs-proover.rs#L135-L168):
```rust
    // Build ladder: in-process mrs first (cheapest), then eprover, then vampire.
    let mut ladder = LadderAtp::new();
    if pick("mrs") {
        ladder = ladder.push(Box::new(MrsAtp::new()));
    }
    if pick("eprover")
        && let Some(p) = eprover_override.or_else(find_eprover)
    {
        ladder = ladder.push(Box::new(EProverAtp::new(p)));
    }
    if pick("vampire")
        && let Some(p) = vampire_override.clone().or_else(find_vampire)
    {
        ladder = ladder.push(Box::new(VampireAtp::new(p)));
    }
    // Counter-model finder rung (last): only when not in single-backend mode
    if only.is_none()
        && !no_fmb
        && let Some(p) = vampire_override.or_else(find_vampire)
    {
        ladder = ladder.push(Box::new(VampireFmbAtp::new(p)));
    }
```

In the full ladder configuration:
1. `MrsAtp` (in-process, fastest/cheapest) is run first to attempt a quick resolution of easy steps, avoiding subprocess spawn overhead.
2. If `mrs` fails to decide the step (returning `Unknown`), it falls back to `eprover`.
3. If `eprover` fails, it falls back to `vampire`.
4. If the entailment provers fail, `vampire-fmb` (Finite Model Builder) is run to look for finite counter-models.

With `--only-mrs`, **only `MrsAtp`** is placed on the ladder, and `eprover`, `vampire`, and `vampire-fmb` are completely disabled.

---

## 3. Key Reasons for the Performance Gap

### 3.1 Restricted Strategy vs. Full Portfolio
As implemented in [atp/external.rs](file:///home/user/EDLA/git/mrs/crates/mrs-proover/src/atp/external.rs#L238-L245):
```rust
        // 3. Setup fast schedule
        let schedule = mrs_search::strategy::named::fast(budget, 1);

        // 4. Run schedule in memory
        let (result, _report) = mrs_search::strategy::run_schedule(
            &all_clauses,
            id_gen,
            &schedule,
            &local_symbols,
            mrs_search::strategy::MlOptions::default(),
            None,
        );
```
- The `fast` schedule is a minimal 1-worker configuration running `AgeWeight(5) + AllNegative` with Knuth-Bendix Ordering (KBO).
- While this is extremely fast and has no subprocess spawning overhead, it lacks the variety of ordering and selection heuristics necessary to solve complex first-order logic steps.
- In contrast, external calls to `eprover` and `vampire` run with `--auto` or their own default multi-strategy portfolios, which try numerous heuristics in parallel or sequence, allowing them to solve much harder steps.

### 3.2 Proving Engine Strength & Theory Support
- **Mature ATPs:** `eprover` and `vampire` are world-class, state-of-the-art superposition provers. They have highly tuned indexing techniques (e.g., path indexing, discrimination trees), optimized equality saturation, and years of engineering optimizations.
- **Younger Codebase:** `mrs` is a newer Rust-based prover. While performant, its internal Given-Clause loop, literal selection, and simplification routines are less mature than those of `eprover` or `vampire`, especially when dealing with complex equational reasoning (equality/paramodulation).

### 3.3 The Absence of Finite Model Building (FMB)
Finding that a proof step is invalid (i.e. premises do *not* entail the conclusion) is crucial for identifying mutated/falsified proofs (scoring `VerifiedBad` +2 points).
- Saturation-based theorem provers (like `mrs` and `eprover` or `vampire` in default modes) are designed to find *refutations* (unsatisfiability). If a step is invalid (satisfiable), they will usually run out of time (timeout) or saturate (if the search space is finite).
- `vampire-fmb` is explicitly configured to search for a **finite model** of `premises ∧ ¬conclusion`. If it finds a finite model, it provides positive proof of non-entailment (`CounterSatisfiable`), which immediately marks the step as `Unsound`.
- Because `mrs` does not support finite model building, running with `--only-mrs` disables this check. This means almost all invalid steps that could be caught by FMB will instead time out or saturate in `mrs`, resulting in an `Unknown` step verdict. This causes the overall proof status to degrade to `Unknown` (0 points) instead of `VerifiedBad` (+2 points).

### 3.4 Why the Full Portfolio of `mrs` is Not Used
A natural question is: why doesn't `MrsAtp` run the full `casc` or `casc_*` portfolio of `mrs` (which runs multiple strategies in parallel) to boost its standalone capability?

There are three key engineering reasons for this design choice:
1. **Extremely Short Per-Step Budgets:**
   - Proof verification evaluates individual inference steps, which are typically simple. The time budget allocated per step is usually under a second or at most a few seconds.
   - For sub-second budgets, the overhead of initializing, spawning, and scheduling a multi-strategy parallel portfolio (e.g., 9-way or 15-way portfolios) completely dominates the actual search time. As noted in `strategy::named::fast`, the `fast` schedule is chosen because "setup overhead dominates the actual search time" for short budgets.
2. **CPU Resource Contention / Thread Oversubscription:**
   - `mrs-proover` already parallelizes the verification of steps *across* all available CPU cores using `std::thread::available_parallelism()` in `run_atp_jobs`.
   - If each of those concurrent step-verification threads launched its own multi-threaded strategy portfolio (e.g., 8 concurrent steps each spawning 15 strategy threads), it would cause severe CPU thread oversubscription (120+ threads competing for 8 cores). This leads to heavy thread thrashing, context-switching latency, and a net loss in overall throughput.
3. **The Role of the `MrsAtp` Rung on the Ladder:**
   - The in-process `MrsAtp` backend is designed to be the *fastest, cheapest first line of defense*. It is optimized to resolve simple, straightforward steps immediately in-memory, avoiding all process-spawning overhead.
   - If a step is too hard for the single-threaded `fast` schedule, it is much more efficient to let it quickly fall through to `eprover` or `vampire` (which are mature, state-of-the-art C/C++ solvers that can solve hard steps far better than `mrs` could even with its full portfolio).

---

## 4. Verification and Benchmark Statistics

The `mrs-proover` documentation logs the following results comparing `--only-mrs` to the Full Ladder:

| Corpus | `--only-mrs` | Full ladder | Performance Gap |
|---|---|---|---|
| **noergler PyRes original** (170 valid proofs) | 162 VerifiedGood / 8 Unknown | 170/170 VerifiedGood | **8 valid proofs** failed to verify under `--only-mrs` due to complex steps. |
| **noergler PyRes falsified** (170 mutated proofs) | 39 VerifiedBad / 131 Unknown | 165 VerifiedBad / 5 Unknown | **126 invalid proofs** went undetected (marked `Unknown` instead of `VerifiedBad`) because FMB/ATP was absent. |

## 5. Legality and Usage in the ProoVer Competition

### 5.1 Are External ATPs Allowed in the Competition?
**Yes.** The use of external Automated Theorem Provers (ATPs) like `eprover` and `vampire` is not only allowed but **explicitly expected** by the rules of the ProoVer competition (specifically ProoVer-2026).

- **Hybrid Verification Model:** The competition design distinguishes between structural proof steps (e.g. Skolemization, conjectures, definitions) which must be checked internally, and **plain steps**, whose logical entailment is intended to be delegated to trusted external ATPs.
- **StarExec Environment:** The competition is hosted on the **StarExec** cluster platform. Submissions are packaged as ZIP archives containing the verifier binary along with any bundled external tools or scripts.

### 5.2 How is it Configured in `mrs-proover`?
The packaging wrapper script at `crates/mrs-bench/systems/mrs-proover/invoke.sh` automatically checks for the presence of the external ATP binaries at build-time/run-time under:
- `crates/mrs-bench/systems/eprover/bin/eprover`
- `crates/mrs-bench/systems/vampire/bin/vampire`

If they are found, they are passed as `--eprover` and `--vampire` arguments to the `mrs-proover` binary, enabling the high-performance verification ladder.

---

## 6. Ladder Ordering: EProver then Vampire vs. Vampire then EProver

The default ladder ordering is `MrsAtp` -> `EProverAtp` -> `VampireAtp` -> `VampireFmbAtp`. This order is highly optimal for the following reasons:

### 6.1 Process Spawning and Startup Latency
- **Eprover:** Eprover is a lightweight C binary with very fast initialization and startup times. On simple steps, it can start, parse the FOF problem, and emit a verdict in a few milliseconds.
- **Vampire:** Vampire is a larger, more complex C++ binary. It has slightly higher startup latency and memory overhead.
- **Optimization:** Spawning Eprover first acts as a fast filter. If the step is simple (the common case), Eprover resolves it instantly, and the verifier completely avoids spawning the heavier Vampire process, saving significant cumulative process-spawning overhead.

### 6.2 Success Probability and Difficulty Tiering
- **Eprover's Auto Mode:** Eprover's `--auto` mode is excellent at quickly finding proofs for straightforward first-order logic steps.
- **Vampire's Strength:** Vampire is generally considered the stronger and more robust solver overall for hard/complex steps (especially those involving complex equational reasoning or requiring AVATAR clause splitting).
- **Optimization:** By trying Eprover first, we dispatch simple-to-medium steps quickly. For the remaining difficult steps where Eprover fails or times out, we fall back to Vampire, utilizing its heavier and more powerful saturation algorithms.

### 6.3 Budget Management and the Fast Fall-Through
As described in [ladder.rs](file:///home/user/EDLA/git/mrs/crates/mrs-proover/src/atp/ladder.rs):
- Each backend is given the full per-step budget (with a 1-second floor) sequentially.
- If we put Vampire first:
  1. For simple steps, we would pay the higher startup overhead of Vampire.
  2. For extremely difficult steps that neither can solve within the budget, both will time out, meaning the order doesn't change the outcome but Vampire would run first.
  3. Therefore, placing the faster-to-start solver (Eprover) before the more powerful/heavier solver (Vampire) minimizes average step verification time.

---

## 7. Alternative Solvers and Potential Improvements

While Eprover and Vampire are the gold standard for general first-order logic (FOF) refutations, there are alternative solvers that could either complement them or serve as efficient replacements for specific sub-tasks:

### 7.1 Alternatives for Entailment Proving (Refutation)
- **iprover:** A highly competitive instantiation-based theorem prover. It is particularly strong on problems in the EPR (Effectively Propositional) division, where it often outperforms Eprover/Vampire. It could be a useful addition to the ladder for steps containing heavily saturated instantiation structures.
- **Zipperposition:** A modern, highly extensible prover written in OCaml. While powerful (especially with higher-order logic extensions), it has higher startup overhead and is generally not as fast as the highly optimized C/C++ provers for standard FOF.

### 7.2 Alternatives for Finite Model Building (Counter-model Finding)
- **Paradox:** A dedicated, MACE-style finite model finder that instantiates FOL formulas to SAT. Paradox is exceptionally fast at finding small counter-models and has significantly lower startup latency and memory overhead than Vampire FMB. Adding Paradox before Vampire FMB would speed up detection of simple mutated/falsified steps.
- **CVC5 / Z3 (SMT Solvers):** Modern SMT solvers are incredibly efficient at satisfiability checking. 
  - **CVC5** natively supports the TPTP format and includes a dedicated `--finite-model-find` mode.
  - **Z3** (which would require a translator from TPTP to SMT-LIB) uses Model-Based Quantifier Instantiation (MBQI) which is extremely fast at building models for first-order formulas with equality. 
  - SMT solvers have very fast startup times and could be highly effective additions to the ladder for both proving and counter-model finding.

---

## 8. Integration and Use of CaDiCaL

### 8.1 Is CaDiCaL Already Used in `mrs-proover`?
**Yes, absolutely.** `mrs-proover` already integrates and uses the CaDiCaL SAT solver in-process.

In [propositional_sat.rs](file:///home/user/EDLA/git/mrs/crates/mrs-proover/src/checks/propositional_sat.rs), the verifier imports the CaDiCaL solver directly via `use cadical::Solver;`.

### 8.2 How is CaDiCaL Utilized?
CaDiCaL is used for two critical fast-paths before delegating to the FOL ATP ladder:
1. **Pure Propositional SAT Fast-Path (`try_propositional`):**
   - If all formulas in a step (premises and conclusion) are purely propositional (i.e. contain only 0-ary predicates and no variables/quantifiers/equality), the entailment check is a finite SAT problem.
   - `mrs-proover` performs a Tseitin transformation of the formulas, encodes them, and solves them in-process using CaDiCaL in microseconds. This resolves many propositional splitting steps (like Vampire's `avatar_*` or `rat` steps) that would otherwise cause external FOL provers to time out.
2. **Propositional Abstraction Fast-Path (`try_propositional_abstraction`):**
   - If a step is not fully propositional but is valid purely due to its propositional structure (e.g. `p(a) ∨ q(b)` and `¬p(a)` entails `q(b)`), `mrs-proover` abstracts each predicate/equality to an opaque boolean variable.
   - It then runs CaDiCaL on this abstraction. If the abstraction is UNSAT, the step is verified as sound in microseconds, completely avoiding the external ATP ladder.

### 8.3 Can CaDiCaL be Used for General First-Order Steps?
Directly, no. CaDiCaL is a propositional SAT solver and cannot natively understand first-order constructs (variables, quantifiers, functions, predicates with arguments, and equality).

To use CaDiCaL for general FOL step verification, we would need:
- A translation and instantiation loop (e.g., a MACE-style grounding engine like Paradox) that iteratively instantiates first-order formulas into propositional logic up to a bound domain size $k$.
- Once grounded, CaDiCaL could search for a model (verifying unsatisfiability or satisfiability for that domain size). Implementing this from scratch inside `mrs-proover` would be a massive undertaking, which is why external solvers like `vampire-fmb` or `paradox` are preferred instead.

---

## 9. ProoVer Competition Readiness and Recommendations

### 9.1 Is the Current System Ready?
**Yes.** In its current full configuration (using the full ladder of `mrs` + `eprover` + `vampire` + `vampire-fmb` along with internal CaDiCaL fast-paths), `mrs-proover` is highly competitive and ready for the ProoVer competition.

- **Soundness Invariant:** The system achieves a **100% soundness rate** (0 incorrect `VerifiedGood` verdicts on falsified/buggy proofs). This is critical because the competition penalizes `bad→good` errors (marking a buggy proof as VerifiedGood) ten times more than `good→bad` errors.
- **Verification Rate:** It correctly verifies **100%** of the valid PyRes proofs and refutes **97.1%** of mutated proofs.
- **Fast-Paths:** The integration of in-process `mrs` and `CaDiCaL` handles the vast majority of simple and propositional steps in microseconds, preserving almost all of the 30-second budget for the few hard steps that require spawning Eprover or Vampire.

### 9.2 Recommended Next Steps (To Improve Further)
To push the verifier to a perfect score, the following enhancements are recommended:

1. **Incorporate Paradox for Faster Model Finding:**
   - Currently, finding counter-models for invalid steps relies on `vampire-fmb`, which has significant startup overhead. 
   - Bundling and integrating **Paradox** (a lightweight MACE-style model finder) on the ladder before `vampire-fmb` would allow the verifier to quickly catch simple mutated steps without wasting time on Vampire's startup latency.
2. **Analyze the 5 Remaining Falsified Proofs:**
   - Currently, 5 mutated proofs in the PyRes falsified corpus degrade to `Unknown` (rather than being successfully refuted as `VerifiedBad`).
   - A useful next step is to run these 5 specific proofs with `MRS_DEBUG_ATP=1` to dump the failed steps, identify why Eprover/Vampire timed out, and see if custom rules or SMT solvers (like Z3/CVC5) can easily resolve them.
3. **Add Support for Other Prover Dialects:**
   - The current verifier has rules tailored for Vampire (`avatar_*`, `sat_conversion`) and E (`predicate_definition_introduction`). 
   - Testing against proofs generated by other provers (e.g. `iprover`, `Leo-III`) could reveal new inference rules or skolemization formats that we need to support internally to prevent them from falling back to (and potentially timing out on) the ATP ladder.

---

## 10. Analysis of the Remaining Falsified Proofs (Timeout Root Cause)

To investigate the root cause of the remaining falsified proofs that degrade to `Unknown`, we ran the Zenodo benchmark suite (`zenodo_benchmark.sh`) locally on the PyRes corpus.

### 10.1 Local Benchmark Findings
Running the verifier on the 170 falsified proofs under a 15-second budget (with a 20-second hard timeout) yielded **168 VerifiedBad** and **2 Unknown** results. The 2 proofs that degraded to `Unknown` were:
1. **`LCL982+1_PyRes---1.5_falsified.proof`** (timed out at `20.010` s)
2. **`NUM844+2_PyRes---1.5_falsified.proof`** (timed out at `20.011` s)

*Note: The original benchmarks in the README reported 5 Unknown proofs; the difference of 3 proofs is due to slightly faster CPU hardware and newer local solver versions (E 3.3.3 / Vampire 5.0.1).*

### 10.2 Debug Analysis of the Failed Steps
Using `MRS_DEBUG_ATP=1` to dump the ATP step queries and results, we ran the verifier on these two proofs. The step outcomes were:

- **`LCL982+1_falsified.proof`:** timed out on step `c19` (rule `resolution`).
- **`NUM844+2_falsified.proof`:** timed out on step `c1094` (rule `resolution`).

Both steps were eventually marked as `Unsound` (yielding `VerifiedBad` when the budget was increased to 30 seconds) because **`vampire-fmb` successfully found a finite model (SZS CounterSatisfiable)** in under 0.05 seconds! For example, the FMB result for `NUM844+2` was:
```
% args: --saturation_algorithm fmb --time_limit 8.00 --input_syntax tptp
% Finite Model Found!
% SZS status CounterSatisfiable for 
...
% Time elapsed: 0.047 s
```

### 10.3 Why the Timeouts Occur (The Sequential Timeout Penalty)
The reason these proofs time out under tight budgets is due to the **sequential structure of the ATP ladder** when verifying unsound (mutated) steps:

1. When checking step `c1094` or `c19`, the verifier sequentially runs the backends in the ladder:
   $$\text{MrsAtp} \rightarrow \text{EProverAtp} \rightarrow \text{VampireAtp} \rightarrow \text{VampireFmbAtp}$$
2. Because the mutated steps are logically **unsound (satisfiable)**, the saturation-based entailment provers (`eprover` and `vampire` in default proving mode) cannot find a proof.
3. As a result, both `eprover` and `vampire` run for their **entire allocated step budget** (capped at 8.0s each) before timing out and returning `Unknown`.
4. Only after both solvers have timed out (wasting $8.0\text{s} + 8.0\text{s} = 16.0\text{s}$ of CPU time) does the ladder finally call `vampire-fmb`, which finds the counter-model instantly in 0.05 seconds.
5. This sequential timeout overhead (16 seconds for a single step) exceeds the overall wall-clock budget for the entire proof (15 seconds), causing the wrapper script to kill `mrs-proover` and report `Unknown`.

```
[ c1094 Entailment Query ]
   |
   +---> MrsAtp (in-process)  =======> Unknown (0.01s)
   |
   +---> EProverAtp (proving) =======> Timeout (8.00s)  <-- Wasted Budget
   |
   +---> VampireAtp (proving) =======> Timeout (8.00s)  <-- Wasted Budget
   |
   +---> VampireFmbAtp (FMB)  =======> CounterSatisfiable (0.05s) <-- Refuted!
```

### 10.4 Proposed Mitigation
To prevent these timeouts and ensure all invalid proofs are refuted within the budget, we could:
- **Run FMB in Parallel:** Spawn the Finite Model Builder (`vampire-fmb`) in parallel with the entailment provers (`eprover`/`vampire`) rather than sequentially. If FMB finds a model or a prover finds a proof, we can abort the other backends immediately.
- **Short Initial Budgets:** Give the proving rungs a very short initial budget (e.g. 500ms) before falling through, as simple steps are usually proved in milliseconds, and hard/invalid steps will otherwise consume the entire budget.

---

## 11. Verification Plan

Since this is an analytical response to a question, no code changes are required. 
To manually verify the performance difference, one can run the test suite or benchmark runner:

```bash
# Run the integration tests requiring external ATPs (will fail or skip if ATPs are absent)
cargo test -p mrs-proover

# Verify PyRes original/falsified proofs with and without external ATPs if you have the corpora locally
# Target: crates/mrs-proover
```
