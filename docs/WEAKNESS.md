# ProoVer 2026 Competition Weaknesses

Based on an analysis of the codebase and the official ProoVer 2026 scoring rules, the `mrs-proover` implementation has several critical weaknesses that currently jeopardize its chances of winning. The competition scoring is highly asymmetric, and the current architecture has both **fatal penalties** and **leaky point-scoring optimizations**.

Here is a breakdown of the critical weaknesses holding back the implementation:

### 1. The Catastrophic `-10` Penalty: Definition Laundering
**The Scoring Rule:** A bad proof identified as good yields a **-10 point penalty**.
**The Weakness:** The `introduced(definition)` step check in `crates/mrs-proover/src/checks/introduced_definition.rs` blindly trusts steps if they declare a fresh symbol using `new_symbols(naming, [...])`. It completely fails to validate the formula body.
**Impact on Winning:** This is a fatal flaw. Exploits like `evil_definition_false` and `evil_definition_nonfalse_laundering` leverage this loophole to bypass the verifier entirely. If the competition includes even two evil proofs that use this loophole, the system will score **-20 points**. Because valid proofs only grant **+1 point**, it would take 20 perfectly verified good proofs just to recover. This bug alone guarantees last place.

### 2. Leaving `+2` Points on the Table: Overly Conservative Structural Checks
**The Scoring Rule:** Identifying an evil proof correctly yields **+2 points**. "Giving up" (`NotVerified`) yields **0 points**.
**The Weakness:** In modules like `crates/mrs-proover/src/checks/introduced_definition.rs` and `vampire_skolemisation.rs`, the design explicitly states: 
> *"an unsupported shape means we can't certify, not that the step is wrong"*
Because of this philosophy, `mrs-proover` returns `StepOutcome::Unknown` rather than `StepOutcome::Unsound` for several known exploits (like `vampire_arity_drop`, `free_var_definition`, and `skolem_injection`). 
**Impact on Winning:** When `mrs-proover` aggregates an `Unknown` step, it outputs `%SZS status NotVerified`. While this safely avoids a penalty, it scores **0 points** on evil proofs where anomalies were successfully detected. Competitors who assertively flag these structural violations as `FailedVerified` will pull ahead by 2 points per problem.

### 3. Missing the `+1` Points: The AC-Equivalence Blindspot
**The Scoring Rule:** Identifying a good proof correctly yields **+1 point**.
**The Weakness:** The fallback logic in `crates/mrs-proover/src/checks/axiom_leaf.rs` checks if leaf axioms match the problem file using strict positional **alpha-equivalence**. If a prover like E or Vampire normalizes a conjecture by reordering disjuncts (e.g., transforming `p | q` into `q | p`), the alpha-equivalence check fails. To avoid a false positive, `mrs-proover` returns `Unknown`, citing:
> *"may differ only by AC-rewriting of commutative operators"*
**Impact on Winning:** Real-world ATPs reorder clauses constantly. By failing to implement Associative-Commutative (AC) equivalence matching for leaf nodes, `mrs-proover` will output `NotVerified` on a large percentage of perfectly valid, normal proofs. It will score **0 points** instead of **+1** on easy wins.

### 4. Overloading the ATP Ladder
**The Scoring Rule:** Time Limit is 30 seconds wall-clock per problem.
**The Weakness:** Any structural check that returns `Unknown` falls through to the ATP ladder (e.g., `eprover` or `vampire` invoked as a subprocess to verify the step). If the structural checks are weak (see point 3), the external ATP is flooded with queries. 
**Impact on Winning:** The overhead of repeatedly launching an external ATP (parsing, grounding, solving) for steps that should be resolved structurally could easily exceed the 30-second wall-clock limit, resulting in a system timeout and **0 points** for the entire problem.

---

### Actionable Roadmap for Competition Readiness
1. **Patch the `-10` Bleed:** You *must* structurally validate the formula body inside `introduced(definition)` steps. Fresh symbols alone are not proof of logical soundness.
2. **Commit to Failures:** Distinguish between "I don't understand this syntax" (which should be `Unknown`) and "This syntax violates TSTP Skolemization/arity rules" (which must return `Unsound` to secure the +2 points).
3. **Implement AC-Matching:** Add support for commutativity in `alpha_equiv` so leaf node validations don't fail on simple `A & B` -> `B & A` rewrites, securing your +1 points.
