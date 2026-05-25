# mrs-szs

SZS ontology status types and TPTP output formatting for `mrs`.

## Key API

```rust
// Status enum
SzsStatus::{Theorem, Unsatisfiable, Satisfiable, CounterSatisfiable,
             Timeout, GaveUp, ResourceOut, Unknown, Error}

// Formatting helpers
szs_status_line(status, problem)  // "% SZS status Theorem for foo.p"
szs_output_start(output_type, problem)
szs_output_end(output_type, problem)
```

## Dependencies

None — this crate has no `mrs-*` dependencies.
