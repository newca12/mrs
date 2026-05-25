# mrs-core

Core logic types for the `mrs` automated theorem prover: terms, formulas, clauses, and symbols.

This is the foundational crate depended on by every other `mrs-*` crate. It defines the shared vocabulary for all reasoning stages — parsing, clausification, unification, inference, and proof extraction.

## Key types

| Type | Description |
|------|-------------|
| `Term` | Variable (`Var(VarId)`) or function application (`App(SymbolId, Vec<Term>)`) |
| `Formula` | Quantified FOL formula with connectives (¬, ∧, ∨, →, ↔, ∀, ∃) |
| `Atom` | Atomic formula: predicate application or equality |
| `Literal` | Signed atom (`positive: bool`, `atom: Atom`) |
| `Clause` | Disjunction of literals with a unique `ClauseId` and `ClauseSource` |
| `Substitution` | Variable-to-term mapping used by unification and inference |
| `SymbolTable` | Bidirectional interning of function/predicate names to `SymbolId` |

## Dependencies

None — this crate has no `mrs-*` dependencies.
