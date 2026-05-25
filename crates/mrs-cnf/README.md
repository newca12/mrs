# mrs-cnf

Clausification and preprocessing for first-order logic formulas.

Converts raw `Formula` values from `mrs-core` into clause normal form (CNF) through a standard pipeline of preprocessing steps.

## Pipeline

```
Formula
  → NNF          (push negations inward)
  → Miniscoping  (push quantifiers inward to reduce Skolem arity)
  → Skolemization (eliminate existential quantifiers)
  → CNF          (distributive or Tseitin definitional, chosen by heuristic)
  → Flatten      (extract Vec<Clause>)
  → Simplify     (remove tautologies and duplicate literals)
```

## Key API

```rust
// Full pipeline in one call
clausify(formula, symbols, id_gen, name, role) -> Vec<Clause>
```

Individual steps are exposed as submodule functions (`nnf`, `miniscope`, `skolem`, `cnf`, `definitional`, `flatten`, `simplify`) for testing or custom pipelines.

## Dependencies

`mrs-core`
