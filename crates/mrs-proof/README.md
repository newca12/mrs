# mrs-proof

Proof extraction and TSTP-formatted output.

Given the `ClauseId` of the empty clause and the clause store, traces parent pointers back through the derivation DAG and formats the result as a TSTP proof accepted by external verifiers.

## Key API

```rust
// Reconstruct the proof DAG (topologically sorted)
extract_proof(empty_id: ClauseId, clause_store: &ClauseStore) -> Vec<Clause>

// Render as TSTP
format_tstp(proof: &[Clause], symbols: &SymbolTable) -> String
```

### Example TSTP output

```
cnf(c0, axiom,    p(a),          file('input', ax1)).
cnf(c1, negated_conjecture, ~p(a), file('input', conj)).
cnf(c2, plain,    $false,        inference(resolution, [status(thm)], [c0,c1])).
```

## Dependencies

`mrs-core`, `mrs-szs`
