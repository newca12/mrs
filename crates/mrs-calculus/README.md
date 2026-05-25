# mrs-calculus

Inference rules for superposition calculus with equality.

## Inference rules

| Module | Rule |
|--------|------|
| `resolution` | Binary resolution on complementary literals |
| `factoring` | Merge same-polarity unifiable literals |
| `superposition` | Rewrite subterms using positive equality literals |
| `equality` | Equality resolution and equality factoring |
| `demodulation` | Simplify clauses using unit equalities |
| `subsumption` | Detect and discard subsumed (redundant) clauses |

## Supporting types

```rust
enum TermOrdering    { KBO, LPO }          // controls equality orientation
enum LiteralSelection { AllNegative, MaxNegative, All } // restricts inferences
```

## Dependencies

`mrs-core`, `mrs-unify`
