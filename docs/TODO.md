# Codebase Review & Outstanding TODOs

This document tracks identified architectural issues, outstanding performance bottlenecks, and unmerged feature branches that represent the highest-yield future improvements for the `mrs` theorem prover.

## 1. Recently Resolved Issues
*   **Severe Memory Exhaustion & Clone Pressure:** `TermNode::App` and `IdAtom::Pred` previously used `Vec<TermId>`, causing heap allocations every time a term was retrieved or cloned via `bank.get(term).clone()`. This was fixed by migrating to `SmallVec<[TermId; 4]>`, moving 99% of term allocations inline and eliminating heap pressure.
*   **Imperfect Indexing Bottleneck:** `DTreeId::get_generalizations_rec` was returning false positives for terms with multiple identical variables (e.g., `p(X, X)` matching `p(a, b)`). A `bindings` state array was added to restore perfect structural indexing during traversal.

## 2. Unmerged Feature Branches (High Priority)
The following remote branches contain implementations for known theoretical limitations but have not yet been integrated into `main`:

*   **`origin/fix-ac-unification`:** Equality rules for Associativity and Commutativity (AC) currently cause combinatorial explosions. Merging the AC-matching heuristic and axiom elimination will dramatically improve the solver's success rate on equational (FEQ/UEQ) categories.
*   **`origin/fix-memory-exhaustion` (Global Subsumption):** While `SmallVec` stops clone pressure, the prover still retains all clauses indefinitely. Implementing global subsumption (cleaning up clauses subsumed by newer, shorter clauses) and orphan elimination is critical for surviving long CASC limits (480s).
*   **`origin/fix-sine-over-pruning`:** For very large axiom sets, SInE (Sumo Inference Engine) premise selection is required to prevent the search space from being flooded with irrelevant axioms immediately.
*   **`origin/fix-epr-grounding`:** Naive EPR (Effectively Propositional) grounding blows up memory on problems with many constants. The strategy should be updated to rely on AVATAR for SAT-based clause splitting instead of naive grounding.

## 3. Concurrency & Architectural Improvements
*   **Unified TermBank (Shared Memory):** Currently, parallel strategy execution via `std::thread::scope` forces every thread to clone the initial clause set and instantiate its own `TermBank` and `SearchState`. This is because `varisat::Solver` is not `Send`. 
    *   **TODO:** Replace `varisat` with a `Send`-compliant SAT solver. This would allow all threads to share a unified `TermBank` behind an `Arc<RwLock>`, drastically reducing RAM usage per run.

## 4. Minor `mrs-tptp` TODOs
*   `crates/mrs-tptp/src/parser/thf.rs`: Implement proper `let` definition parsing.
*   `crates/mrs-tptp/tests/resources/non-classical/CorrectSpecifications.p`: Multiple non-classical test cases are marked with `TODO` comments requiring implementation for specific semantics, domains, and modalities.
