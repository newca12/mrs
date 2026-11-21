# SAT-Based Subsumption and Subsumption Resolution

## Status

Experimental and disabled by default. This is a bounded first integration of
Coutelier et al., [*SAT-Based Subsumption Resolution* (CADE 2023)](https://doi.org/10.1007/978-3-031-38499-8_11).
The legacy matchers remain the default. The SAT backend is intended to reuse one
sparse literal-match set to decide ordinary subsumption first, then SR if
subsumption fails. It explores wide clause pairs that the normal 20-literal SR
target limit skips; it is not yet a promoted portfolio strategy or a measured
coverage improvement.

## How to try it

`MRS_SAT_SR=1` enables the combined SAT simplifier. To preserve portfolio
diversity, set `MRS_SAT_SR_STRATEGY` to one base strategy ID; for example, the
first slot of `casc_epu` uses strategy 1:

```bash
nix develop -c env MRS_SAT_SR=1 MRS_SAT_SR_STRATEGY=1 \
  cargo run --release --bin mrs -- --schedule casc_epu --workers 1 --time 10 \
  problems/group.p
```

Omitting `MRS_SAT_SR_STRATEGY` enables it in every strategy. The selected
strategy keeps the existing backtracking simplifiers for targets at or below
its configured SR width limit (20 by default), then uses the bounded SAT route
for wider targets up to 512 literals. SAT first checks ordinary subsumption; a
validated match deletes the target. Otherwise the same match options and solver
encoding are queried under SR mode. An oversized encoding, exhausted query
budget, cancellation, or SAT `unknown` leaves the clause unchanged. There is no
SAT call at runtime unless the environment flag is explicitly set.

The completed run's `% SZS detail` line records the selected strategy IDs,
pair attempts, separate subsumption/SR solver entries and outcomes, accepted
simplifications, and total encoding/solve microseconds.

## Integration and safety

- The backend is called from forward and backward simplification points in the
  given-clause loop. Existing indexed/feature-vector candidate filters remain
  in front of SAT encoding. Wide forward candidate lists combine the ordinary
  subsumption and SR index results before testing each pair.
- Match choices use the calculus crate's one-way matcher, including both
  equality orientations. A mode selector makes the ordinary-subsumption query
  use only same-polarity matches; if that is UNSAT, SR mode permits one unique
  complementary target occurrence. Target-literal reuse follows MRS's existing
  set-style SR semantics. Plain subsumption is validated under the search
  matcher's existing contract before the target is deleted.
- CaDiCaL runs in a private solver instance for the current worker and observes
  that worker's deadline and portfolio cancellation flag. Each pair attempt has
  a 25 ms total budget shared by the subsumption and SR solves, a 512-literal
  pair ceiling, and explicit caps on match options, SAT variables, and clauses.
- SAT models are independently reconstructed: every active literal must have
  one selected match and source-variable bindings must agree. For SR, exactly
  one target occurrence must be the pivot, with no same-polarity match using
  that occurrence. Only then may ordinary subsumption delete a target, or SR
  remove the pivot and write the existing
  `ProofWitness::SubsumptionResolution`. The SAT solver does not introduce a
  proof rule or justify a first-order inference by itself.
- SAT `unsat` and every inconclusive/limited outcome only miss a simplification;
  they do not justify saturation or any other positive result. Telemetry
  distinguishes pre-SAT structural rejections from CaDiCaL solver entries.

## Initial local test policy

The local `crates/mrs-bench/problems/casc-*` corpora are ignored by Git, so they
may not exist in a fresh checkout. Start with the repository's curated
`problems/` fixtures and the bounded combined-SAT unit tests; those fixtures do
not provide the wide-clause workload that motivates this feature. When the
local CASC corpora are available, use them before a separate upstream TPTP tree
and record their exact release/provenance. Local runs are diagnostic only, not
CASC measurements.

For a bounded local EPU smoke test, the ignored CASC-30 corpus includes
`SWV418-1.300.p`. Keep the certified pre-passes and memory allowance explicit;
the result is an integration/telemetry check, not a coverage result:

```bash
nix develop -c env MRS_SAT_SR=1 MRS_SAT_SR_STRATEGY=1 MRS_EPR_GROUND=0 \
  MRS_NO_INSTGEN=1 MRS_MAX_MEMORY_MB=4096 \
  cargo run --release --bin mrs -- \
  --schedule casc_epu --strategy 1 --workers 1 --time 30 \
  crates/mrs-bench/problems/casc-30/EPU/SWV418-1.300.p
```

The focused unit tests exercise both simplification modes, assumption reuse,
equality symmetry, wide targets, and a generated SR proof checked by the
independent proof kernel.

For portfolio evaluation, compare otherwise-identical baseline and candidate
runs, enabling the combined SAT simplifier in only one strategy slot at first.
Use a bounded, family-aware UEQ/EPU subset, one benchmark process at a time, and
the same memory limit and problem order. Record strict proof-audit outcomes and
per-problem gains and losses; do not promote the backend based on attempts, SAT
answers, or throughput alone.
