# mrs-index

Term and literal indexing structures for fast clause retrieval during proof search.

Without indexing, finding inference partners requires scanning all processed clauses linearly. Discrimination trees reduce this to near-logarithmic time.

## Key types

```rust
// Discrimination tree: indexed by pre-order DFS term flattening
DTree<V>
  ::insert(term, value)
  ::unify_retrieve(query)          // superset of unifiable terms
  ::generalization_retrieve(query) // more general terms (for demodulation)
  ::instance_retrieve(query)       // more specific terms

// Clause index for resolution/superposition partner lookup
LiteralIndex
  ::insert_clause(clause)
  ::find_partners(literal)
```

## Dependencies

`mrs-core`
