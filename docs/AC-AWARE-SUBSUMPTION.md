# Implementation Plan - AC-Aware Subsumption and Rewrite Indexing via Normalization

## Objective
Implement AC-aware term indexing for demodulation and subsumption by recursively normalizing terms containing Associative-Commutative (AC) symbols before they are inserted or queried in our `STreeId` / `DTreeId` indexes. This avoids passive queue bloat and enables fast, scalable algebraic simplification modulo AC.

## Key Files & Context
- `crates/mrs-core/src/term_bank.rs` (term representation and interning)
- `crates/mrs-index/src/stree.rs` / `dtree.rs` (indexes)
- `crates/mrs-search/src/state.rs` / `given_clause.rs` (demodulation and subsumption search loops)

## Implementation Steps

### 1. Implement Recursive AC-Normalization (`mrs-core`)
Add an `ac_normalize` method to `TermBank` in `crates/mrs-core/src/term_bank.rs`:
```rust
pub fn ac_normalize(&mut self, term: TermId, ac_syms: &HashSet<SymbolId>) -> TermId
```
*   **Acyclicity / Base Cases**: `TermNode::Var` returns itself.
*   **AC Applications**: If `TermNode::App(sym, args)` is encountered:
    *   If `sym` is in `ac_syms`, we recursively normalize all arguments, flatten any associative nested applications of `sym` (e.g., `f(f(a, b), c)` -> `[a, b, c]`), sort the arguments lexicographically (by `TermId` or another stable structural order), and re-intern the canonical node into the `TermBank`.
    *   If `sym` is not an AC symbol, we recursively normalize arguments, maintaining their original order, and re-intern the node.

### 2. Integrate Normalization into Index Operations (`mrs-search`)
Update how indexes are queried and updated:
*   **Demodulation Index Insertion / Retrieval (`state.rs` & `given_clause.rs`)**:
    *   When inserting a newly oriented unit equality ($L = R$) into `state.demod_index`, we normalize both $L$ and $R$ modulo AC before inserting.
    *   When retrieving simplification rules for a query term $T$ from `state.demod_index`, we first normalize $T$ modulo AC, perform the retrieval, and then apply the matching rewrite.
*   **Subsumption Index Retrieval**:
    *   When retrieving candidate subsumption clauses from discrimination trees, ensure the literals are queried using their AC-normalized term representations.

## Verification & Testing
*   Create a unit test in `crates/mrs-core/src/term_bank.rs` to verify that `ac_normalize` correctly normalizes permuted AC terms (e.g. `f(b, a)` and `f(a, b)`) to identical `TermId`s.
*   Verify that `cargo test --all` passes successfully without warnings.
