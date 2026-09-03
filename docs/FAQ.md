# FAQ: CASC Prover Architecture, Soundness, and Verification

This document compiles the foundational answers to critical questions regarding Automated Theorem Proving (ATP) architecture, competition strategies, soundness guarantees, and proof verification within the MRS environment.

---

## 1. Soundness vs. Empirical Verification

### Q: If a benchmark run reports "REFERENCE VIOLATIONS — none detected" and "POLARITY VIOLATIONS — none detected", isn't that proof that the solver's run on that division is mathematically valid?

**No, absolutely not.** It is proof of **empirical agreement** on that specific set of problems, but it is **not proof of logical soundness**.

*   **Empirical Agreement (Syntactic Match):** The benchmark script performs a simple key-value lookup: it checks if the solver's output string (`Satisfiable`) matches the reference status recorded for that problem. If they match, it reports 0 violations.
*   **Logical Soundness (Mathematical Truth):** Soundness is a mathematical guarantee that if the solver outputs `Satisfiable` or `Unsatisfiable`, that claim has been logically proven for the **original, complete set of axioms $S$**.

#### The Pruning Loophole
If the prover uses aggressive ML Premise Selection (e.g., `--ml-prune 0.6`) to discard 40% of the axioms, the remaining subset of clauses ($S_{\text{pruned}}$) becomes incredibly easy to simplify. The solver can saturate this small subset in seconds, find a valid model, and output `Satisfiable`. 

Because the original problem $S$ was indeed satisfiable, the solver's output matches the reference status, leading to **0 violations**. However:
*   The solver only proved that $S_{\text{pruned}}$ is satisfiable.
*   It did **not** prove that the original set $S$ is satisfiable, because the discarded 40% of axioms could contain a hidden contradiction. 

This is essentially a **lucky guess**. If you run the exact same solver on an unsatisfiable counterpart (like EPU), it will prune the contradiction-generating axioms, saturate the remaining satisfiable subset, guess `Satisfiable`, and generate **fatal, unsound polarity violations** (which is why the EPU benchmark suffered 57 soundness errors before our soundness guard!).

---

## 2. Division Tuning vs. Unsound Cheating

### Q: Since our test harness (`invoke.sh`) knows if a problem is in a satisfiable (EPS) or unsatisfiable (EPU) division from the folder name, why can't we just cheat by skipping the safety guard and outputting "Satisfiable" on EPS sat runs?

Attempting to "cheat" using division folders is a high-risk gamble that has historical precedents of disqualification at CASC for three reasons:

1.  **Secret Control Problems:** The CASC organizers (Geoff Sutcliffe and the panel) are fully aware of folder-based tuning. To keep solvers honest, they occasionally insert **cross-division control problems** into the suites (e.g., placing an unsatisfiable problem inside the EPS division). If your solver prunes the axioms and blindly outputs `Satisfiable` because it assumes it's safe inside the EPS directory, it will trigger an instant, public soundness violation on the control problem, leading to immediate disqualification.
2.  **Flat-Directory Sandbox Execution:** StarExec sometimes executes solvers using absolute flat paths (e.g., `/starexec/sandbox/problem_123.p`) during the official competition, entirely removing the division name from the directory path. If your solver's soundness depends on directory parsing, it will crash or default to an unsound mode under flat paths.
3.  **StarExec Proof/Model Validation:** The competition framework runs automatic post-processing verification. If a solver returns `Satisfiable`, CASC runs a certified model-validator (like `Inca` or `Z3`) on the output. If the solver pruned 40% of the axioms to saturate, the model-validator will fail to verify the resulting model against the original 100% of the axioms, resulting in **0 points** for that problem anyway.

#### The Deployed MRS Solution: Legal Portfolio Tuning
Our updated `invoke.sh` configuration does not cheat; it **legally tunes** the parameters. When we detect the `eps` division:
*   We **disable axiom pruning completely**.
*   The solver works on 100% of the axioms, saturates **soundly**, and outputs a mathematically valid `Satisfiable` proof.
*   We achieve the **exact same peak performance (72 solves)**, but it is 100% sound, 100% verifiable, and completely safe from trap problems.

---

## 3. The Role of `mrs-proover`

### Q: Can `mrs-proover` prove that MRS solutions are indeed verified?

**Yes, but only for refutations (Unsatisfiable status).** 

`mrs-proover` is a dedicated **Refutation Proof Verifier** built for the *ProoVer* competition. It reads a TSTP (TPTP Standard) proof DAG ending in `$false` (the contradiction) and certifies step-by-step that every single inference (resolution, superposition, Skolemization) is mathematically valid.

It cannot verify satisfiable runs (EPS) because satisfiability is proven by constructing a *model* (a structure where all formulas are true), which requires a dedicated model validator (like `Inca` or `Z3`), not a refutation proof DAG verifier.

---

## 4. Why Prover Competitions Exist (MRS vs. Z3/Inca)

### Q: Why do prover competitions like CASC exist if Z3 or Inca can do the job?

This question represents the boundary between **Generating a Proof (Search)** and **Verifying a Proof (Checking)**, as well as the division of labor in mathematical logic.

#### A. $NP$-Hard Search (MRS) vs. $P$-Time Verification (Inca)
Finding a proof requires searching through an infinite, chaotic tree of possible logical inferences. A first-order prover (like MRS, Vampire, or E) spends 99.9% of its resources on **search heuristics, term indexing (STree, AC-indexing), and clause selection** just to find the needle in the haystack.

Checking a proof (Inca or `mrs-proover`), however, is extremely cheap. Once the prover outputs the exact DAG of steps, a verifier can check it in linear ($P$-time) complexity. Z3 or Inca cannot find first-order proofs on their own in these infinite domains, but they can easily verify them once they are found.

#### B. The SMT Universe (Z3) vs. The FOL Universe (MRS)
There is **no free lunch** in theorem proving. Z3 and MRS are engineered for entirely different mathematical universes:

*   **Z3 (SMT - Satisfiability Modulo Theories):** Optimized for **quantifier-free formulas** linked to decidable theories (e.g., linear arithmetic, bit-vectors, arrays). SMT solvers excel at hardware and software code verification but are notoriously weak at handling quantifiers.
*   **MRS (First-Order Logic Prover):** Optimized for **pure, deeply nested quantifiers ($\forall X \exists Y \forall Z \dots$) and equality**, with no underlying theories. Pure FOL is undecidable, and solving it requires specialized calculi (Superposition, Resolution) and algebraic term normalization (like our AC-indexing) that SMT solvers do not possess. 

Without the advanced first-order calculi of provers like MRS, Z3 will instantly time out or run out of memory when faced with CASC-grade quantified algebraic problems.

---

## 5. Other CASC Divisions & MRS's Scope

### Q: What are the other CASC divisions, and why does MRS focus strictly on FOF and UEQ?

> **Correction (2026-07-18):** the table below (and the "8 official
> divisions" framing) describes **CASC-30**, the *previous* competition.
> **CASC-J13's actual official divisions are only `THF`, `FOF`, `FNT`, `UEQ`,
> and `ProoVer`** — confirmed directly on the
> [CASC-J13 Entrants page](https://tptp.org/CASC/J13/Entrants.html), which
> has no EPR, TFA, SLH, or ICU division at all this year. **`mrs 0.2.1`'s
> actual CASC-J13 registration has always been `FOF UEQ`** (entered
> 27/06/26, before the `PRO013+3.p` incident) — there is no EPR/ICU entry to
> withdraw from for this competition; it was never entered there. The `EPR`
> (`EPU`/`EPS`) and `ICU` rows below, and the corresponding `casc_epr`/
> `casc_icu` named schedules and `crates/mrs-bench/problems/casc-30/{EPU,EPS,
> ICU}` fixtures in this repo, are a **legacy CASC-30-era internal
> regression/benchmark taxonomy** kept for local testing continuity — they
> are unrelated to what mrs is actually entered for at CASC-J13. Following
> the `PRO013+3.p` soundness incident (see `docs/BENCHMARKS.md` and the
> CWA/subst fixes in `crates/mrs-search/src/cwa.rs` /
> `crates/mrs-search/src/subst.rs`), verification effort
> (`run_soundness_audit.sh` + the `mrs-proover`/`mrs-codex` independent-proof
> audit) is concentrated on **FNE + FEQ + UEQ**, matching the actual `FOF
> UEQ` entry.

At the official CASC-30 competition, **Vampire 5.0** made history by winning **all 8 official divisions** in a complete clean sweep. These divisions, their logic definitions, and how MRS's *local benchmark* taxonomy relates to them are outlined below (see the correction note above for what CASC-J13 itself actually offers and what mrs is actually entered for):

| Division | Domain | Winning System | MRS Compatibility & Scope |
| :--- | :--- | :---: | :--- |
| **FOF** | First-Order Formulas (Classical) | **Vampire 5.0** | **🟢 CASC-J13 ENTRY (FNE & FEQ)**. The core of first-order ATP. MRS competes here, split internally into **FNE** (No Equality) and **FEQ** (With Equality). |
| **EPR** | Effectively Propositional | **Vampire 5.0** | **⚪ N/A AT CASC-J13** (no such division this year). Bernays-Schönfinkel class (no functions of arity $\ge 1$). MRS's `EPU`/`EPS` CaDiCaL SAT-splitting support and benchmark fixtures remain in the tree from the CASC-30 era, but there is no EPR division to enter at CASC-J13. |
| **UEQ** | Unit Equality CNF | **Vampire 5.0** | **🟢 CASC-J13 ENTRY (UEQ)**. Pure equational logic. Our sound **AC-indexing loop** targets this division directly. |
| **THF** | Typed Higher-order Form | **Vampire 5.0** | **❌ OUT OF SCOPE**. Requires higher-order logic (lambda-calculus, type theory, currying). MRS is strictly a classical *First-Order* solver. |
| **TFA** | Typed First-order with Arithmetic | **Vampire 5.0** | **❌ OUT OF SCOPE**. Requires SMT-style arithmetic solvers to handle real, integer, and rational constraints. MRS does not support numeric theories. |
| **FNT** | First-order Non-theorems | **Vampire 5.0** | **❌ OUT OF SCOPE**. A real CASC-J13 division (Vampire 4.8 demonstration), but MRS is not entered — it strictly tests disproving formulas by finding counter-models (often requiring finite model generators like Mace4 or Paradox). |
| **SLH** | Sledgehammer (Isabelle) | **Vampire 5.0** | **⚪ N/A AT CASC-J13** (no such division this year, and MRS would be out of scope regardless — obligation proofs translated from interactive HOL proof assistants require highly specialized translation parsing). |
| **ICU** | Intuitionistic First-order logic | **Vampire 5.0** | **⚪ N/A AT CASC-J13** (no such division this year). Experimental harness and benchmark fixtures remain in the tree from the CASC-30 era. |

By specializing strictly in FOF and UEQ for CASC-J13 — matching the actual entrant registration — MRS avoids the architectural bloat of SMT/Higher-order engines, allowing us to build the highest possible throughput and search space density in pure classical first-order reasoning.

---

## 6. Determinism, CPU Contention, and Deep-Term Safeguards

### Q: Why do some proofs (like `ALG032+1.p` or `NUM283-1.005.p`) verify perfectly on fast CPUs but fail or time out on slower, non-AVX remote servers?

This behavior highlights three subtle but critical aspects of the prover and verifier architecture: **wall-clock pruning feedback loops**, **term-depth verifier safeguards**, and **SAT variable limit constraints**.

#### A. The "Slow CPU / Wall-Clock LRS" Pruning Trap
By default, the Limited Resource Strategy (LRS) prunes the passive queue every 100 given-clause iterations based on remaining wall-clock budget (`LrsPolicy::WallClock`):
* **On Fast CPUs (e.g. Core i7/i3):** Iterations are fast, so LRS estimates a generous remaining budget and does not prune the critical clauses needed to reach a short refutation proof.
* **On Slower CPUs (e.g. older 2012 Xeons like E5-2407):** Iterations run significantly slower due to much lower single-core instruction-level parallelism (IPC).
* **The Fallout:** When running a heavy first-order search like Strategy 2 (`S2` / `use_avatar: false`), which requires exactly **229 given-clause iterations** on `ALG032+1.p`:
  1. At iteration 100 on the slow Xeon, the elapsed time is very high, causing the calculated average nanoseconds per iteration (`avg_nanos`) to spike.
  2. LRS estimates a tiny remaining iteration budget and aggressively prunes the passive queue down to its minimum size of 2,000, **deleting the critical clauses needed to reach the proof at iteration 229**.
  3. Denied these critical clauses, the prover is forced into an infinite search of deeply nested, heavy terms. Because Strategy 2 has `max_term_weight: None`, term-tower growth is unrestricted, making unification exponentially slower and causing a **300-second (5-minute) timeout** where a fast CPU would solve it immediately in ~9–11 seconds.
* **The Deterministic Solution:** To make sweeps completely independent of hardware speed, scheduling, or CPU load, opt-in to a logical LRS budget:
  ```bash
  export MRS_LRS_FIXED_ITERATIONS=100000
  ```

#### B. Deep-Term Successor Nesting in Number Theory (`NUM` / `NUN`)
In the `NUM` domain, arithmetic is represented algebraically using successor notation (e.g. `s(s(s(...s(0)...)))`). Proofs for these problems generate terms with nesting depths exceeding 100+ levels (e.g. `NUM283-1.005.p` has a step with **154** nested successor applications).
* To prevent external ATPs from hanging during proof-step validation, `mrs-proover` enforces a hardcoded safeguard threshold (`formula_max_depth`), which was raised from `25` to **`200`** in `v0.2.3` to cleanly verify these deep successor arithmetic proofs as `VerifiedGood`.

#### C. SAT/AVATAR Variable Verification Limits
For complex AVATAR branch refutations, the verifier reconstructs and solves the SAT splits in CaDiCaL. `mrs-proover` originally capped the maximum allowed SAT variables per refutation step to `20` to prevent verifier hangs. This limit was raised to **`200`** in `v0.2.3` to allow complex algebra and set-theory proofs (which frequently reach 40–150 variables) to verify successfully without a soundness penalty.
