# Conversation Export

## Q1: howto prove a TPTP proof is valid

**User:** howto prove a TPTP proof is valid

**Assistant:** To verify that a TPTP proof is valid, you need to check both its **syntactic well-formedness** and its **logical correctness**. In the TPTP/TSTP ecosystem, a proof is typically a directed acyclic graph (DAG) of annotated formulas where each step records an inference rule, parent formulas, and an SZS status. Here is how to approach validation.

### 1. Use Automated Proof Verifiers

The most reliable way is to run an independent verifier rather than checking by hand.

- **GDV (General Derivation Verifier)** — Part of the TPTP World, GDV performs both **structural verification** and **semantic verification** on TSTP derivations. It checks that parent formulas exist, the parent records form an acyclic DAG, all paths reach the root, and each inference step logically follows from its parents. For tableau proofs, it was extended to verify that every branch ends in `$false` and lemma extensions only use lemmas below the specified node.

- **SC-TPTP Library** — A newer alternative that provides a parser and an independent proof checker for first-order logic with equality. It can unfold high-level proof steps (e.g., congruence, clausification) into low-level steps and verify them. Several provers (Goeland, Prover9, egg) can export to this format.

- **SystemOnTSTP** — An online service in the TPTP World that lets you examine and process ATP solutions, including running verification tools on uploaded proofs.

### 2. Structural Verification (What to Check)

If you must inspect manually or write your own checker, verify the proof structure:

1. **Syntax** — Every formula must be valid TPTP (FOF, CNF, THF, etc.), and every inference record must include:
   - The rule name (e.g., `resolution`, `paramodulation`, `extension`, `start`)
   - An SZS status (usually `thm` for theorems or `cth` for counter-theorems)
   - Parent references (`parent(FormulaId:LiteralPosition)`)
   - Logical parents (the actual source formulas/clauses used)

2. **Parent graph is a DAG** — No cycles in the dependency graph; every derived formula ultimately traces back to axioms or the root.

3. **Leaf verification** — All leaves without parents must be declared axioms, hypotheses, or conjectures from the original problem.

4. **Reachability** — Every path through parent records must lead to the root node or an axiom.

5. **Closure (for refutation/tableau proofs)** — If the proof is a refutation, it must derive `$false`. For tableau proofs, every branch must end at a `$false` formula.

### 3. Semantic Verification (Does Each Step Follow?)

This is the hard part: ensuring each inference is logically sound.

- **Replay with a trusted prover** — One common technique is to send each inference step, together with its parents, to a simpler, trusted theorem prover (e.g., Metis) or proof assistant (e.g., Lean, Rocq) to confirm the conclusion follows. Sledgehammer and PRocH use this strategy.

- **Check unification and substitutions** — For resolution/paramodulation steps, verify that the claimed most general unifier (MGU) actually makes the complementary literals identical, and that the substitution is applied correctly to the resolvent.

- **Literal extraction** — For rules like `extension` or `connection`, confirm that the specified literals exist at the claimed positions in the parent clauses and are complementary (or unify to become complementary).

### 4. SZS Status Ontology

TPTP uses the SZS ontology to classify results. A valid proof should carry a consistent status, such as:
- `thm` — theorem (logical consequence of axioms)
- `cth` — counter-theorem (for lemma steps in tableau proofs)
- `uns` — unsatisfiable (for CNF refutations)

If a step is marked `thm` but is not actually a logical consequence of its parents, the proof fails semantic verification.

### Practical Workflow

1. **Export** the proof from your prover in TSTP/TPTP format.
2. **Run GDV** or upload to **SystemOnTSTP** for automatic structural and semantic checks.
3. If using a newer toolchain, convert to **SC-TPTP** and run its independent checker.
4. For highest assurance, **replay** the proof in a proof assistant (Lean, Rocq, Isabelle) that checks each step against its kernel.

If you have a specific proof file you want validated, you can upload it to [SystemOnTSTP](https://tptp.org/cgi-bin/SystemOnTSTP) or run the SC-TPTP checker from [github.com/SC-TPTP/sc-tptp](https://github.com/SC-TPTP/sc-tptp).

---

## Q2: Anything in Rust ?

**User:** Anything in Rust ?

**Assistant:** There is no dedicated, off-the-shelf **TPTP/TSTP proof verifier** written in Rust that matches the functionality of GDV or SC-TPTP. However, the Rust ecosystem has good building blocks and several TPTP-compliant provers you can use or study.

### What exists today

#### 1. Parsing: the `tptp` crate
The [`tptp`](https://docs.rs/tptp) crate (by Michael Rawson) is a robust, zero-copy, streaming parser for the TFX/FOF/CNF dialects of TPTP, built on `nom`. It gives you strongly-typed syntax trees and a visitor pattern, so you can walk derivations and inspect inference records.

This is the essential first step for any Rust-based checker: read the proof file, extract the formula DAG, and validate parent references.

#### 2. TPTP-compliant provers in Rust
Several Rust theorem provers read TPTP and output TSTP-style proofs, but they are **provers**, not general proof checkers:

| Prover | Notes |
|--------|-------|
| **mrs-calculus** | Targets CASC; reads TPTP and outputs SZS/TSTP. |
| **HopCoP** / **lazyCoP** / **SATCoP** | Connection-tableau family; TPTP compliant. |
| **CoP** | Reimplementation of leanCoP/nanoCoP in Rust. |
| **Serkr** | FOL with equality. |
| **res-rs** | Early-stage resolution prover. |

These show how to ingest TPTP problems and emit derivations in Rust, but they do not verify arbitrary proofs produced by other systems.

#### 3. General proof infrastructure
From the formalized-reasoning ecosystem, crates like `rate` (DRAT/DPR checker for SAT) and `carcara` (Alethe proof checker) demonstrate the pattern for building small, trusted Rust checkers, but they target other formats, not TPTP.

### What you would need to build

If you want a Rust equivalent of GDV for TPTP, you would typically layer:

1. **Parser** — use the `tptp` crate to load the proof DAG.
2. **Structural checker** — verify that every derived formula has valid parent IDs, the graph is acyclic, leaves are axioms/conjectures, and the root has the claimed SZS status.
3. **Semantic checker** — for each inference step, reconstruct the conclusion from the parents using the stated rule (resolution, paramodulation, etc.) and confirm it follows. The hard part is validating unifiers and equality reasoning.
4. **Trusted kernel** — optionally reduce each step to a tiny kernel (e.g., a sequent calculus or natural deduction checker) and replay it, similar to how Sledgehammer uses Metis or how SC-TPTP unfolds steps.

### Upcoming: ProoVer 2026
The TPTP World is launching the **Proof Verifier Competition (ProoVer)** in 2026. This may motivate more implementations—including Rust ones—for independently checking TSTP derivations.

**Bottom line:** today you can parse and manipulate TPTP proofs in Rust with the `tptp` crate, but for full validation you still need to either call an external tool (GDV, SC-TPTP, or a proof assistant) or implement the checking logic yourself on top of the parser.

---

## Q3: how vampire developer check proof

**User:** how vampire developer check proof

**Assistant:** The Vampire team uses a **pragmatic SMT-based proof-checking pipeline** as their primary internal validation method, alongside traditional techniques. Here is how they do it.

### 1. The Main Method: SMT-LIB Proof Scripts (Vampire 6)

The developers' current approach, described in their CADE 2025 paper, is to have Vampire emit each inference as a **quantifier-free SMT-LIB instance** that is checked by an external SMT solver (Z3 or cvc5).

**How to run it:**
```bash
vampire -p smtcheck problem.p
```

Instead of the usual TSTP proof, Vampire outputs an SMT-LIB script. For every inference in the proof, the script:

1. **Pushes** a fresh context.
2. **Declares Skolem constants** from the negated conclusion.
3. **Asserts ground instances** of the premises (using the unifier recorded during proof search).
4. **Asserts the negated conclusion.**
5. **Checks satisfiability** with `(check-sat)`. If the solver returns `unsat`, the inference is valid. If it returns `sat`, there is a bug.
6. **Pops** the context and moves to the next step.

The key insight is that Vampire's lifted (quantified) inferences are **re-grounded** for checking. By negating and Skolemizing the conclusion, then instantiating the premises with the same unifier used during saturation, each step becomes a small, ground SMT query that decision procedures dispatch almost instantly.

### What it covers
- Core superposition calculus
- Simplification rules (subsumption resolution, demodulation)
- Most ALASCA arithmetic rules
- AVATAR inferences (including the SAT refutation step)
- Axiomatized theories
- 58 inference rules are handled by the same generic code path

### What it does *not* cover (yet)
- **Preprocessing** (Skolemization, definition introduction, CNF conversion) — these are equisatisfiability transformations, not sound clause-to-clause inferences.
- **Quantifier elimination** steps where no unifier-based instance exists.
- **Induction** — SMT solvers do not validate induction schemes.
- **Polymorphism** — SMT-LIB 2.6 lacks support; SMT-LIB 2.7/3.0 may fix this.
- Some exotic TPTP arithmetic functions (e.g., floor) that have no SMT-LIB counterpart.

For unsupported steps, Vampire emits a `(echo "sorry ...")` warning, so you know which parts must be taken on trust.

### 2. Scale & Validation Results

The Vampire developers ran this checker at scale:

- **13,159 SMT-LIB proof scripts** generated from TPTP 9.0.0 and SMT-LIB 2024 benchmarks.
- **No unsoundness** was found in Vampire's proofs.
- A **known ALASCA bug was rediscovered**; after the fix, all proofs checked successfully.
- Ground scripts were checked by Z3 in **< 0.2 seconds** per proof. By contrast, quantified scripts (where premises are left universal) caused Z3 to time out on 85 out of 4,194 problems, confirming that the ground approach is essential for reliable checking.

### 3. Other Techniques They Use or Compare Against

The Vampire developers are aware of, and sometimes use, alternative methods:

- **Self-checking with Vampire:** They occasionally use Vampire itself to re-prove individual Vampire steps — what they call an "ouroboros" that nevertheless catches bugs in practice.
- **External ATP replay:** Tools like **Ekstrakto** re-derive each TSTP step using Zenon Modulo or iProver Modulo. The Vampire team notes this struggles with Vampire's advanced features (AVATAR, definitions, theory reasoning).
- **TESC (Theory-Extensible Sequent Calculus):** A dedicated first-order proof format with verifiers, one formalized in Agda. It covers much of Vampire but has limited theory support.
- **GDV (General Derivation Verifier):** Checks structural and semantic properties of TSTP proofs using a trusted ATP.
- **ITP reconstruction:** Exporting to Isabelle, Coq, or Dedukti/λΠ-modulo is considered the gold standard for trust, but is labor-intensive and struggles with theory reasoning that Vampire offloads to Z3 internally.

### 4. Future Direction

The team is actively working on **fully reconstructing Vampire proofs** by gluing together the SMT unsatisfiability proofs for each step into one continuous certificate. They also note that Vampire is evolving toward better support for proof assistants (e.g., tailored schedules for Isabelle's Sledgehammer) and extracting verified code from formal proofs.

**Bottom line:** If you want to check a Vampire proof the way the developers do, run `vampire -p smtcheck` and pass the resulting SMT-LIB script to Z3 or cvc5. If every step returns `unsat`, the proof is verified.

---

## Q4: what of all this can be use on Proofs that are written in TSTP format

**User:** what of all this can be use on Proofs that are written in TSTP format

**Assistant:** Here is what you can use **directly** on a proof that is already in TSTP format, versus what requires conversion or a different output mode.

### Works directly on TSTP

#### 1. GDV (General Derivation Verifier)
This is the canonical tool for TSTP. It performs both **structural** and **semantic** verification on a TSTP derivation file:
- Checks that parent references exist and form an acyclic DAG.
- Verifies that every path reaches axioms or the root.
- For tableau proofs, checks that every branch ends in `$false`.
- Replays inference steps using a trusted ATP to confirm semantic validity.

If you have a `.p` or `.s` file in TSTP, GDV is the intended checker.

#### 2. SystemOnTSTP
An online service in the TPTP World that accepts uploaded TSTP derivations and runs verification tools (including GDV) on them. You can upload a TSTP proof and have it checked automatically without installing anything locally.

#### 3. Ekstrakto
This tool reads a **TSTP proof trace** and re-derives each individual step using external ATPs (like Zenon Modulo or iProver Modulo) that produce proofs in the λΠ-modulo calculus. It works directly on TSTP, though it can struggle with advanced features like AVATAR inferences or theory reasoning.

### Requires conversion from TSTP

#### 4. SC-TPTP
SC-TPTP is a **target format**, not a general TSTP checker. Provers must export into it (Goeland, Prover9, and egg do this natively). If your proof is already in TSTP, you would need to translate it into SC-TPTP before using the SC-TPTP independent checker. There is no automatic universal converter for arbitrary TSTP proofs from any prover.

#### 5. TESC (Theory-Extensible Sequent Calculus)
TESC proofs can be generated **from TSTP traces** for a large subset of first-order problems, but not all. If the translation succeeds, you can then check it with one of the TESC verifiers (including one formalized in Agda). However, support for theories is limited, and some Vampire-specific steps do not translate.

### Does NOT work on TSTP (alternative output only)

#### 6. Vampire's `-p smtcheck`
This is **not** a TSTP checker. It is an alternative output mode where Vampire emits an SMT-LIB script *instead* of a TSTP proof. If your proof is already in TSTP, you cannot feed it to this checker. You would need to re-run Vampire with `-p smtcheck` from the original problem to get the SMT-LIB version.

### Rust ecosystem for TSTP

#### 7. The `tptp` crate
The `tptp` Rust crate can **parse** TSTP files and give you a typed syntax tree. You could use it to build a structural checker (validate parent IDs, detect cycles, check leaf axioms), but it does **not** verify that inference steps are semantically correct. For semantic checking in Rust, you would need to implement the logic yourself or shell out to an external tool.

### Practical workflow for a TSTP file

| Goal | Tool |
|------|------|
| **Quick online check** | Upload to SystemOnTSTP |
| **Thorough structural + semantic check** | Run GDV |
| **Reconstruct in a proof assistant** | Ekstrakto (λΠ-modulo), or import into Isabelle/Coq |
| **Parse/analyze in Rust** | `tptp` crate for syntax; semantic checking is DIY |
| **Check a Vampire proof specifically** | If you have TSTP, use GDV or Ekstrakto. If you want the SMT method, you must regenerate with `vampire -p smtcheck` |

**Bottom line:** For an existing TSTP proof, **GDV** and **SystemOnTSTP** are your direct, off-the-shelf options. Everything else either requires format conversion or a different prover output mode.

---

## Q5: Ok given all this I want to develop from scratch in Rust the best tool for the proover-competition. What should be the approach ?

**User:** Ok given all this I want to develop from scratch in Rust the best tool for the proover-competition. What should be the approach ?

**Assistant:** For **ProoVer 2026**, the winning strategy is not to build a pure "small kernel" verifier (too limited on theories) nor a pure SMT replayer (too heavy on trust), but a **hybrid Rust architecture** that combines the trust model of SC-TPTP with the coverage of Vampire's ground-SMT technique. Here is the approach I would take.

### 1. Core Philosophy: The "GDV Successor"

Be a **structurally rigorous + semantically pragmatic** checker:

- **Structural verification** must be bulletproof and pure Rust (no external dependencies).
- **Semantic verification** must handle everything from pure FOL resolution to ALASCA arithmetic and AVATAR splits.
- For steps you cannot directly verify, **degrade gracefully**: mark them as *assumed* rather than rejecting the whole proof.

This maximizes both **coverage** (you verify more proofs than anyone else) and **trust** (every checkable step is checked by code you control).

### 2. Architecture

```
TSTP Input
    │
    ▼
┌─────────────────┐     ┌─────────────────┐
│  Layer 1:       │────▶│  Layer 2:       │
│  Structural     │     │  Semantic       │
│  Analyzer       │     │  Engine         │
│  (Pure Rust)    │     │  (Rust + SMT)   │
└─────────────────┘     └─────────────────┘
```

#### Layer 1: Structural Analyzer (Pure Rust)

Use the **`tptp`** crate (Michael Rawson) for zero-copy parsing. Build the proof DAG with **`petgraph`**.

Verify:
- Every derived formula has valid parent IDs.
- The parent graph is a DAG (no cycles).
- All leaf nodes are declared axioms, hypotheses, or conjectures.
- The root carries the claimed SZS status (`thm`, `uns`, etc.).
- Literal positions in parent records actually exist.

**Why this wins:** This alone catches malformed proofs instantly. It is deterministic, requires no external tools, and Rust's type system makes memory safety guarantees that C checkers cannot match.

#### Layer 2: Semantic Engine (Hybrid)

Split inference steps into three classes:

| Class | Handler | Trust Level |
|-------|---------|-------------|
| **Kernel-checkable** | Direct Rust verification | Highest |
| **Theory/complex** | Ground SMT query (Z3/cvc5) | High (external oracle) |
| **Preprocessing** | Marked as assumption | Medium |

##### 2a. The Rust Kernel
Implement a tiny (~1,000 lines) kernel for **ground first-order logic with equality**:
- Resolution
- Paramodulation
- Factoring
- Equality rewriting

For quantified steps, instantiate using the unifier recorded in the TSTP inference annotation, then check the ground instance in your kernel.

##### 2b. The SMT Oracle
For steps your kernel cannot handle (arithmetic, datatypes, AVATAR SAT refutations, complex superposition), use the **Vampire SMT technique**:

1. Extract the unifier from the TSTP inference record.
2. Instantiate premises with that unifier.
3. Skolemize and negate the conclusion.
4. Emit a ground SMT-LIB query.
5. Call Z3 or cvc5 (via `std::process` or the **`rsmt2`** crate).
6. If `unsat`, the step is valid. If `sat`, you have a **countermodel** — output it as a debugging witness.

**Why this wins:** You get theory reasoning "for free" without implementing real arithmetic in Rust. The queries are small and ground, so they solve in <0.2s per step.

##### 2c. Preprocessing Auditor
TSTP proofs often include Skolemization, definition introduction, and CNF conversion. These are **equisatisfiability transformations**, not sound inferences. Do not try to verify them in-kernel. Instead:
- Maintain a **provenance map** tracking how each introduced symbol relates to the original problem.
- Flag these steps with a distinct SZS status (e.g., `assumed`) so users know what was trusted.

### 3. Competitive Advantages for ProoVer

| Feature | Implementation | Competitive Edge |
|---------|---------------|------------------|
| **Parallel checking** | Use **`rayon`** to verify independent proof branches concurrently. | Speed on large proofs (some TSTP proofs are 300MB+). |
| **Streaming** | Process the TSTP file in a single pass where possible. | Memory efficiency. |
| **Countermodel output** | When SMT returns `sat`, pretty-print the model. | Debugging value; other checkers just say "fail." |
| **Incremental SMT** | Use `(push)`/`(pop)` around each step in a single solver session. | Massive speedup on long proofs. |
| **Modular trust modes** | `--mode=kernel` (only kernel-checked steps accepted) vs `--mode=pragmatic` (SMT-assisted). | Appeals to both purists and practitioners. |

### 4. Implementation Roadmap (10 weeks)

| Phase | Task | Deliverable |
|-------|------|-------------|
| **1** | Integrate `tptp` crate; build DAG; structural checks | Validates parent graph |
| **2** | Implement ground resolution/paramodulation kernel | Checks simple FOF/CNF proofs |
| **3** | Build SMT-LIB generator + Z3/cvc5 driver | Handles theory steps |
| **4** | Add `rayon` parallelism + query caching | Fast batch checking |
| **5** | Preprocessing audit trail + SZS reporting | Competition-ready output |

### 5. What *Not* To Do

- **Do not** write a TPTP parser from scratch. The `tptp` crate is battle-tested and zero-copy.
- **Do not** try to verify Z3's internal proofs (Alethe format). Use Z3 as an oracle only; chasing SMT proof formats will consume your entire timeline.
- **Do not** reject proofs containing unsupported steps. ProoVer rewards coverage. Emit `sorry` or `assumed` and continue.
- **Do not** attempt to formalize your kernel in Coq/Agda *within the Rust codebase*. For ProoVer, a small, readable Rust kernel is sufficient; formalization in ITP can come later.

### 6. Starter Skeleton

Your main loop should look conceptually like this:

```rust
// 1. Parse TSTP into a DAG of annotated formulas
let proof = tptp::parse_file(path)?;

// 2. Topological sort for dependency order
let order = topo_sort(&proof)?;

// 3. Check each node
for node in order {
    match node.inference_rule() {
        "resolution" | "paramodulation" => {
            if let Err(e) = kernel.check(&node) {
                // Fallback to SMT oracle
                smt.verify(&node)?;
            }
        }
        "cnf" | "skolemisation" => {
            audit.mark_assumed(&node);
        }
        _ => {
            smt.verify(&node)?; // Default to SMT
        }
    }
}
```

**Bottom line:** Build a **fast structural checker in pure Rust**, a **tiny FOL kernel** for the core calculus, and an **SMT bridge** for everything else. Parallelize aggressively, report countermodels, and never reject a proof just because one step is weird. That combination — speed, coverage, and pragmatic trust — is what wins a proof-verifier competition.

---

*Exported on 2026-05-28*
