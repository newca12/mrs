# Soundness-First Development Methodology

This document defines the required workflow for implementing, reviewing, and
benchmarking changes to `mrs`. It applies to human-written changes,
AI-assisted changes, and changes produced by autonomous agents.

The primary objective is not merely to improve the solve count. A theorem
prover must never convert an incomplete search, heuristic, abstraction, or
resource-limited computation into an unjustified logical answer.

This methodology complements:

- [`ARCHITECTURAL_REVIEW_VAMPIRE_GAP.md`](ARCHITECTURAL_REVIEW_VAMPIRE_GAP.md),
  which records architectural opportunities and their implementation history.
- [`AUTO_PROOF_REVIEW.md`](AUTO_PROOF_REVIEW.md), which documents proof
  recording and independent verification.
- [`SOUNDNESS_STATUS.md`](SOUNDNESS_STATUS.md), which records audit results.
- [`AUDIT.md`](AUDIT.md), which records benchmark methodology and failure
  analysis.
- [`VERIFIER_SPEC.md`](VERIFIER_SPEC.md), which defines verifier behavior.

## 1. Core Rules

These rules are mandatory:

1. Every new positive SZS status claim must have an explicit mathematical
   justification and a testable certificate or completeness argument.
2. An incomplete method may return `GaveUp`, `Timeout`, or `Unknown`; it must
   not return `Satisfiable`, `CounterSatisfiable`, `Unsatisfiable`, or
   `Theorem` solely because it stopped searching.
3. A passing unit-test suite is not evidence of soundness when the tests use
   the implementation's own unproved assumptions as their oracle.
4. Existing design documents, comments, benchmarks, and tests are evidence,
   not authority. Reviewers must independently check the semantic claims.
5. Reference-answer violations are release-blocking defects, even when the
   affected run is fast, reproducible, or improves the aggregate solve count.
6. Proof verification and search-result classification are separate trust
   problems. A strict proof kernel can validate refutations; it does not
   automatically validate a satisfiability claim produced by search.
7. Any failed or inconclusive validation must remain visible in the final
   report. Do not turn an unknown result into a success by changing the
   reporting layer.

## 2. Classify the Change

Before implementation, classify the change. A change can belong to more than
one category.

| Category | Examples | Main risk |
| --- | --- | --- |
| Search heuristic | clause weighting, queue order, precedence, SOS | Incompleteness accidentally reported as satisfiability |
| Search calculus | resolution, superposition, InstGen, AVATAR | Unsound inference or invalid model claim |
| Preprocessing | BCE, PLE, demodulation, clausification | Removing a clause needed for a refutation |
| Representation | indexing, canonicalization, variable renaming | Changing logical identity or substitutions |
| Proof/provenance | parent registration, TSTP formatting, certificates | Uncheckable or misleading proofs |
| Status/runtime | SZS mapping, timeout handling, worker coordination | False definitive status after partial work |
| Performance/resource | allocation, parallelism, memory caps | Hidden truncation, races, or result-dependent timing |

The category determines the minimum review and test requirements. Any change
that can influence `Saturated`, `Satisfiable`, or `CounterSatisfiable` is a
soundness-sensitive change, even if its code is described as a performance
optimization.

## 3. Write the Semantic Contract First

Before writing code, record the intended behavior in the change description,
issue, or design note. The contract must answer:

- What mathematical object does the implementation maintain?
- What does each internal result mean?
- Which results are definitive, and which are merely search outcomes?
- What assumptions are made about clauses, terms, variables, domains, or
  preprocessing?
- What happens when an assumption is false or a resource limit is reached?
- What exact certificate, proof, model, or completeness theorem justifies each
  definitive result?

Use a result contract like this:

| Internal/output result | Minimum justification |
| --- | --- |
| `Refutation` / `Theorem` / `Unsatisfiable` | Replayable derivation DAG, independently checked where applicable |
| `Saturated` / `Satisfiable` / `CounterSatisfiable` | Complete saturation under the declared calculus, or an independently checkable model certificate |
| `GaveUp` | Search was incomplete, pruned, bounded, unsupported, or otherwise unable to justify a definitive answer |
| `Timeout` | The deadline expired before a definitive answer |
| `Unknown` / `Inconclusive` | Verification or decision could not establish the claim within its contract |

If the proof obligation cannot be stated clearly, the feature is not ready
for implementation. Implement the path as refutation-only or fail-closed
instead.

### Example: model claims

The following implication requires proof; it is not a safe heuristic:

```text
SAT of an abstraction
+ no currently discovered conflicting pair
= first-order satisfiability
```

The reviewer must show that the abstraction preserves all distinctions needed
by the first-order semantics and that the conflict search is complete. If
variables, substitutions, domains, or ground instances were collapsed, the
implication is not established by intuition or by a few examples.

## 4. Separate Refutation and Model Paths

Refutation and satisfiability have asymmetric safety properties.

### Refutation path

A derived empty clause can be accepted when every inference is recorded and
replayable. The required work is:

- Record exact parent IDs and rule names.
- Preserve all preprocessing and introduced-definition provenance.
- Extract the reachable derivation DAG.
- Check the emitted proof with the strict kernel or an independent verifier.
- Demote the result to `GaveUp` if proof extraction or verification fails.

### Model/saturation path

Saturation is not automatically a model certificate. Before returning a
positive satisfiability status, establish all of the following:

- The active clause set is the complete logical problem, not a selected subset.
- Every simplification or elimination preserves satisfiability.
- The calculus is complete for the active fragment and configuration.
- Every resource limit preserves the claimed conclusion.
- The reported interpretation can be checked against the original problem, or
  a formal completeness argument covers the exact algorithm.

If any item is missing, return `GaveUp` rather than a positive SZS status.

This distinction applies to:

- SInE and ML premise pruning.
- SOS-restricted search.
- Unit-only or ordered-inference restrictions.
- LRS queue pruning.
- BCE and PLE implementations whose preservation argument is incomplete.
- AVATAR branch roll-ups without replayable SAT evidence.
- SAT abstractions and lazy instantiation procedures.

## 5. Prove the Invariants Before Optimizing

For each algorithmic change, write the invariants that must hold. Typical
invariants include:

- **Parent completeness:** every generated clause cites all logical parents
  needed to derive it.
- **Variable hygiene:** independently quantified clauses are standardized
  apart before unification or resolution.
- **Substitution correctness:** repeated variables, independent variables, and
  variable capture are distinguished correctly.
- **Clause preservation:** preprocessing removes only clauses whose removal is
  proven satisfiability-preserving under the actual first-order semantics.
- **Abstraction soundness:** every abstract model or conflict corresponds to a
  valid first-order object or a conservatively handled unknown.
- **Status monotonicity:** losing completeness can change a result to `GaveUp`,
  but cannot create a new definitive positive or negative status.
- **Deadline safety:** a timeout or worker cancellation cannot be mistaken for
  saturation.
- **Certificate closure:** every parent referenced by a proof or certificate is
  present and itself justified.

For a new inference rule, specify:

1. The formal rule or semantic equivalence.
2. The conditions under which it is valid.
3. The exact data recorded in `ClauseSource` or `ClauseCertificate`.
4. The independent replay/checking procedure.
5. The behavior when the rule is outside the supported fragment.

## 6. Test Design

Tests must be designed to falsify the implementation, not merely demonstrate
the happy path.

### 6.1 Unit tests

Add small tests for:

- The normal case.
- Empty, singleton, and degenerate inputs.
- Repeated variables versus independent variables.
- Variable renaming and standardization apart.
- Literal and clause permutation.
- Multiple constants and no constants.
- Nested terms and quantifier scope.
- Duplicate literals and tautologies.
- Resource-limit boundaries.
- Malformed or incomplete metadata.

### 6.2 Negative tests

Every new soundness-sensitive path needs tests that must be rejected or
downgraded. Include cases where:

- An abstraction collapses distinct first-order structures.
- A purported model satisfies only a subset of clauses.
- A clause is removed by an invalid simplification.
- A proof omits a parent or uses a forged parent.
- A timeout occurs immediately before a result is reported.
- A worker is cancelled after another worker reports a non-definitive result.

Do not use only hand-constructed examples that mirror the implementation.
Generate adversarial variants by changing variable sharing, constants,
literal order, polarity, and clause order.

### 6.3 Metamorphic tests

Where possible, assert that semantics are unchanged under transformations such
as:

- Renaming bound and free variables correctly.
- Permuting literals and clauses.
- Duplicating harmless clauses.
- Adding a tautological clause.
- Reordering input files and includes.
- Enabling a sound preprocessing optimization.
- Running with one worker versus multiple workers, modulo timing.

Any changed definitive SZS answer under a semantics-preserving transformation
is a blocker.

### 6.4 Independent oracle tests

For a bounded fragment, compare against one of:

- Exhaustive ground enumeration.
- A small trusted reference implementation.
- An independent SAT/SMT/model checker.
- E Prover or Vampire, with the result interpreted as a diagnostic rather than
  an unquestionable oracle.
- The strict proof kernel for refutations.

An external prover disagreement is not by itself proof that `mrs` is wrong,
but it must be investigated before accepting a definitive result.

### 6.5 The InstGen incident as a required pattern

The SAT-guided InstGen implementation abstracted every variable to one
placeholder. A clause containing `p(X, X)` and one containing `p(X, Y)` could
therefore lose distinctions between variable identity and independent
variables. The test suite initially expected positive SAT results from this
incomplete abstraction, so the tests reinforced the bug.

The required lesson is:

- A SAT abstraction is not a first-order model certificate by default.
- "No currently found unifiable pair" is not a completeness proof.
- Variable-bearing positive results must be `GaveUp` until the model-lifting
  theorem and checker are implemented.
- The large NLP theorem cases must remain permanent regression canaries.

## 7. Review Procedure

Every non-trivial change requires two distinct reviews.

### Review A: implementation review

Check:

- API and ownership correctness.
- Parent registration and DAG reachability.
- Concurrency, cancellation, and deadline behavior.
- Memory growth and asymptotic cost.
- Formatting, clippy, and test coverage.

### Review B: semantic review

Ignore the implementation's comments and existing tests initially. Check:

- Which SZS statuses can this change influence?
- What exact theorem supports each definitive result?
- Can a counterexample be constructed with two constants, repeated variables,
  or a reordered clause?
- Does preprocessing preserve the original problem's semantics?
- Does the proof/model certificate cover the original complete problem?
- Is any incomplete path accidentally classified as `Saturated`?

The semantic reviewer must be independent of the implementation author. For
AI-assisted work, a second model may perform this review, but the prompt must
explicitly require challenging the algorithm's mathematical claims rather
than summarizing the design document.

Useful review prompt:

```text
Audit every path that can return Theorem, Unsatisfiable, Satisfiable, or
CounterSatisfiable. Do not trust comments, existing tests, or the design
document. State the mathematical certificate for each result. Construct a
minimal counterexample for every abstraction, pruning rule, timeout path, and
model-lifting claim. If the proof is unavailable, require GaveUp instead.
```

## 8. Benchmark Acceptance Gates

Benchmarks are correctness gates as well as performance measurements.

### 8.1 Local focused gate

Before a broad run:

1. Build in the declared Nix environment.
2. Run focused unit and integration tests.
3. Run all relevant positive, negative, and metamorphic tests.
4. Run one-problem reproductions with an external timeout.
5. Inspect stdout and stderr, including `% SZS detail` telemetry.

For memory-constrained machines, never start a broad benchmark first. Use one
problem at a time, explicit worker counts, and an external timeout:

```bash
timeout --signal=TERM --kill-after=10s 45s \
  env TPTP=/path/to/TPTP RUST_MIN_STACK=67108864 \
  target/release/mrs --time 30 --workers 1 --schedule casc_fne problem.p
```

### 8.2 Reference-status gate

For every changed search or status path, run a representative reference set
containing:

- Theorem problems.
- Unsatisfiable problems.
- Satisfiable problems.
- CounterSatisfiable problems.
- Problems near resource limits.
- Known historical regressions.
- Cross-division control problems.

The run must have:

- Zero soundness/reference violations.
- Zero polarity violations.
- No unexpected disagreements.
- No unexplained status changes from the baseline.
- No OOM, abort, stack overflow, or silent process termination.

The `bench_report` output must be saved with the exact commit, TPTP release,
worker count, schedule, time limit, machine memory, and command line.

### 8.3 Full validation gate

Before merging or committing a stable stage, run:

```bash
nix develop -c cargo check
nix develop -c cargo clippy --all -- -D warnings
nix develop -c cargo fmt --all --check
nix develop -c cargo test --workspace
```

These commands establish build and regression quality. They do not replace the
semantic and benchmark gates above.

### 8.4 Performance claims

Do not claim a solve-rate gain from one run. Record:

- Baseline commit and candidate commit.
- Exact TPTP release and division list.
- Physical and logical worker counts.
- Wall-clock limit.
- RAM and CPU hardware.
- Schedule and environment variables.
- Solved count by division.
- Reference violations and disagreements.
- Peak memory and abnormal exits.

A faster run with one false definitive answer is a regression, not an
improvement.

## 9. Safe Rollout Policy

New mechanisms should be introduced in stages:

1. **Refutation-only stage:** allow only derivations with replayable proofs;
   downgrade all uncertain positive results to `GaveUp`.
2. **Opt-in stage:** expose the feature behind an explicit flag or schedule;
   run targeted semantic and reference benchmarks.
3. **Canary stage:** enable it for a small, permanent regression set and
   compare against the previous implementation.
4. **Default stage:** enable it only after the full validation gate passes and
   reference-status violations remain zero.
5. **Post-merge monitoring:** retain telemetry and rerun canaries after changes
   to neighboring code, even when the feature itself is untouched.

Incomplete search restrictions must be converted to `GaveUp` before they can
participate in portfolio result selection. A worker that pruned clauses,
restricted inference, used an incomplete ordering, or exhausted a bounded
abstraction must not stop the portfolio with a definitive satisfiability
answer.

## 10. Incident Response

When a benchmark finds an unsound result:

1. Stop treating the benchmark as a performance result.
2. Preserve the exact input, output, stderr, commit, and environment.
3. Reproduce one problem at a time under a bounded external timeout.
4. Identify the first code path that produced the definitive status.
5. Disable that path or make it fail closed immediately.
6. Add the problem as a permanent regression test or canary.
7. Minimize the issue to a small semantic counterexample when possible.
8. Update the relevant design document so the invalid assumption is explicit.
9. Re-run focused tests, reference-status benchmarks, and full validation.
10. Only then resume performance work.

Do not hide a soundness failure by changing the benchmark classifier,
discarding the problem, or calling the result "borderline." A theorem-prover
soundness failure is a product defect until proven otherwise.

## 11. Definition of Done

A feature or change is ready to merge only when all applicable boxes are true:

- [ ] The change category and affected SZS statuses are documented.
- [ ] The semantic contract and proof obligations are written down.
- [ ] Incomplete paths fail closed as `GaveUp`, `Timeout`, or `Unknown`.
- [ ] Refutation provenance and certificate closure are complete.
- [ ] Positive status claims have a model certificate or completeness proof.
- [ ] Normal, adversarial, negative, metamorphic, and limit tests exist.
- [ ] At least one independent oracle or replay check was used.
- [ ] Historical soundness canaries pass.
- [ ] Reference-status benchmark reports zero violations.
- [ ] Memory and timeout behavior were checked on the target resource class.
- [ ] The independent semantic review is recorded.
- [ ] `cargo check`, clippy, format, and workspace tests pass in Nix.
- [ ] The commit identifies the exact validation commands and results.

If any soundness-sensitive box is unchecked, the implementation may remain in
an experimental branch, but it must not be presented as a validated default
strategy.
