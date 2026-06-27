# mrs-index

Term and literal indexing structures for fast clause retrieval during proof search.

Without indexing, finding inference partners requires scanning all processed clauses linearly. Substitution trees and dense feature vectors reduce this to near-logarithmic time.

## Key types

```rust
// Substitution tree: fast retrieval with path-compressed edges
STreeId<V>
  ::insert_atom(atom, bank, value)
  ::get_unifications_atom(query, bank)     // superset of unifiable terms
  ::get_generalizations_atom(query, bank)  // more general terms (for demodulation)

// Dense SIMD-optimized feature vector for linear subsumption filtering
FeatureVector
  ::can_subsume(target)
  ::can_subsumption_resolve(target)

// Clause index for resolution/superposition partner lookup
LiteralIndex
  ::insert(clause, bank)
  ::get_unifiable_resolution_partners(atom, bank)
  ::get_subsumption_candidates(feature_vector)
```

## Dependencies

`mrs-core`
