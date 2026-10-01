# LCL Condensed-Detachment Pre-pass

The optional `MRS_CONDENSED_DETACHMENT=1` pre-pass targets compact LCL problems
that encode theoremhood with a unit predicate `is_a_theorem(F)` and the rule

```text
~is_a_theorem(X) | is_a_theorem(Y) | ~is_a_theorem(implies(X,Y))
```

Detachment is replayed as two ordinary binary-resolution steps against the
actual input rule clause. Derived theorem units are retained and may be reused.
The emitted TSTP graph is therefore an ordinary resolution DAG, not a trusted
special inference. Input-shape recognition is deliberately narrow; any
unsupported shape or exhausted bound falls through to the standard strategy
portfolio.

## Bounds and availability

- At most 5,000 distinct theorem facts and 100,000 counted resolution steps.
- A derived formula may not exceed twice the largest goal formula size plus 16
  term nodes.
- Maximum one second per problem and five seconds total per process, so batch
  runs cannot repeatedly pay a fresh pre-pass ceiling.
- Disabled unless `MRS_CONDENSED_DETACHMENT=1` is set.
- Skipped when strict asynchronous self-checking is active, so it cannot consume
  the verifier's reserved wall-clock budget outside candidate coordination.
- A bounded no-result outcome is inconclusive; the regular schedule still runs.

## Smoke check

```bash
nix develop -c cargo run -- problems/cd-prototype-smoke.p
MRS_CONDENSED_DETACHMENT=1 nix develop -c cargo run -- problems/cd-prototype-smoke.p
```

Compare the opt-in run against the ordinary schedule. For corpus evaluation,
record per-problem statuses, elapsed time, proof sizes, and independent
`mrs-proover --strict` verdicts. Do not make it a default route based only on
the smoke fixture.

Unit tests run generated proofs through the independent `mrs-proof-kernel`.
Any wider benchmark should also use the linked problem source and strict proof
checking; a parseable TSTP string or successful pre-pass return alone is not a
proof-validation result.
