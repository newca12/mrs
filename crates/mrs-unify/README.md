# mrs-unify

First-order term unification and one-way matching.

## Key API

```rust
// Most-general unifier (Robinson's algorithm, with occurs check)
unify(s: &Term, t: &Term) -> Result<Substitution, UnifyError>

// One-way matching (pattern variables only)
match_term(pattern: &Term, target: &Term) -> Result<Substitution, UnifyError>

// Failure reasons
enum UnifyError { SymbolClash, ArityMismatch, OccursCheck }
```

## Dependencies

`mrs-core`
