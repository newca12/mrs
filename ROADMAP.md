# MRS performance and assurance roadmap

Active plan, 2026-10-25. Starting review: `030e423`. Preserve the crate-level
architecture and worker-local search; refactor boundaries incrementally.
Objective: **independently certified problems solved inside eight physical cores
and the applicable official division budget**. Targets below are research goals,
not measured gains. EPS is a certified-model objective, not a portfolio set-cover
objective. Historical plans remain evidence, not current implementation status.

## Landed foundations

- [x] Reachability-first interned proof extraction, including AVATAR roots.
- [x] Dependency-topological extraction with missing-parent/cycle refusal.
- [x] Preserve refutations from incomplete parent-guidance workers.
- [x] Stable proof identities independent of arena allocation/hash iteration.
- [x] First-registration semantics shared by clause/proof registration.
- [x] Correct solo-union, determinism and variance interpretation policies.

These fixes pass the Nix-wrapped workspace gates. They establish regression
quality, not a measured throughput or certified-coverage gain.

Additional landed runtime/evidence work: explicit pipeline-thread stack;
cooperative strict-kernel deadlines propagated from the certification coordinator;
bounded candidate backpressure; identity-bound audit caches with fresh-directory
re-audits; separate actual time/RSS/address-space fields; profiling build;
optional-feature and paired-analysis CI gates. Deadline checks are between kernel
stages/nodes, not hard preemption of a long rule or parser operation.

## P0 — Reproducible baseline and experiment ledger (weeks 1–3)

Owners: benchmark/runtime maintainers. Paths: `crates/mrs-bench/`, `docs/results/`.

- [ ] Freeze clean producer binaries, checker binaries, toolchains and corpus/include hashes.
- [ ] Append audit records keyed by producer, checker, corpus, configuration and budgets.
  Completed audits now publish immutable content-addressed CSV/identity/summary
  snapshots. Producer source metadata remains linked through the hashed run CSV;
  consolidating a full producer/checker ledger is still open.
- [ ] Generate the current certification dashboard from records; retain old audits unchanged.
  `certification_dashboard.py` renders selected immutable snapshots and preserves
  all outcomes. Selecting/re-auditing the authoritative current baseline is open.
- [x] Separate actual/credited time and RSS/address-space in new harness rows;
  retain signal/status evidence without inferring OOM from SIGKILL alone.
- [x] Add paired, order-randomized strictly audited baseline/candidate campaigns
  and paired descriptive outcome analysis (`paired_campaign.py`).
- [ ] Freeze stratified profiling fixtures and family-disjoint development/holdout sets.

Gate: a third party can reconstruct every headline number from immutable
artifacts. Use >=5 paired replicates as a starting screen; increase replication
when uncertainty exceeds the proposed effect. Never extrapolate a subset range.

## P1 — Representation and proof contracts (weeks 3–8)

Owners: core/search/proof maintainers. Paths: `mrs-core`, `mrs-search`, `mrs-proof`.

- [ ] Separate formula provenance from live search clauses at the type boundary.
  Landed first slice: immutable validated `SearchClauseRef`, typed passive
  insertion, unconditional active/passive compatibility-boundary validation,
  and formula steps no longer report `is_empty()`. Owning clause/provenance
  storage types and migration of remaining legacy APIs are still open.
- [ ] Replace mutable duplicate clause content with immutable per-worker storage and ID postings.
- [ ] Validate witness parent identities against source/certificate closure at registration/export.
- [ ] Migrate rule families to authoritative typed witnesses with exact replay.
- [ ] Add bounded streaming output and measure proof-finalization latency/peak memory.
- [ ] Make incomplete-search reasons explicit without discarding valid refutations.

Gate: mutation/certificate-closure canaries and archived replay comparisons pass;
every changed verdict is explained. Preserve independent kernel semantics.

## P2 — Bounded, measurable runtime (weeks 6–12)

Owners: runtime/search maintainers. Paths: `src/pipeline.rs`, `src/coordinator.rs`,
`mrs-cnf`, `mrs-search`, `mrs-proof-kernel`.

- [ ] Propagate run-level cancellation/deadlines through preparation, setup,
  SAT, inference, simplification, elaboration, verification and shutdown.
- [ ] Isolate non-interruptible verification when a hard process deadline is required.
- [x] Bound candidate queues by bytes as well as count: eight queued candidates,
  64 MiB queued proof text, nonblocking rejection with telemetry. One in-flight
  proof and producer-owned payloads are additional memory; verification itself
  still needs an interruptible budget boundary.
- [ ] Add stage completion telemetry, phase costs and memory-by-subsystem accounting.
- [ ] Add fixed-work diagnostic configuration, stable tie-breaking/seeds and trajectory digests.
- [ ] Extract selection, simplification, inference, AVATAR and sharing modules
  behind concrete contracts; avoid dynamic dispatch/allocation in hot loops.

Gate: adversarial/deep inputs stop within a declared grace period; fixed-work
trajectories reproduce. Profile representative FEQ/FNE/UEQ/EPR classes before
choosing optimization work. Shared CI timings are not performance evidence.

## P3 — Engine efficiency (months 3–5)

Prerequisites: P0/P2 measurements and P1 identity contract.

- [ ] Measure ID-only retrieval and reduced clause cloning.
  Landed ID-only resolution/superposition query APIs and migrated the main loop
  and backward-demodulation discovery. Compatibility APIs remain; differential
  recall/removal tests cover both. Fixed-work timing and memory measurements open.
- [ ] Improve subsumption filter selectivity and repeated-variable matching costs.
- [ ] Measure corrected demodulation memo hit rates and invalidation cost.
- [ ] Allocate only selected passive queues; compact tombstones by measured thresholds.
- [ ] Compare indexed backward rewriting policies, including requeue-to-passive.
- [ ] Prototype shared immutable prepared input with worker-local derived arenas.
- [ ] Bound inference batches and avoid avoidable intermediate collections.

Targets: 1.5–2x fixed-work throughput on identified bottleneck classes and
25–40% lower large-input/history peak memory. Gate: repeated actual eight-worker
certified coverage plus proof audits; throughput alone cannot promote a default.

## P4 — Algorithmic power (months 4–8)

- [ ] FEQ/UEQ: exact AC/associative witnesses, ordering/rewrite ablations and
  proof-aware algebraic search. Re-audit affected archived rule shapes.
- [ ] FNE: develop existing condensed-detachment search from current failure
  clusters; evaluate multi-queue complementarity against the shipped schedule.
- [ ] EPS/EPU: counterexample-guided instances, equality-aware grounding,
  original-problem model validation and compact independently replayable SAT certificates.
- [ ] Evaluate learned guidance only with family-disjoint data, inference overhead,
  certificate costs and paired portfolio coverage. Keep optional until promoted.

Target: 15–25% relative certified-coverage growth over P0 on entered divisions.
Gate: held-out improvements, mechanism ablations, no unexplained losses or false
definitive answers. A SAT abstraction is not a first-order model certificate.

## P5 — Actual eight-core portfolio (months 6–9)

- [ ] Optimize certified outcomes including proof size and checking latency.
- [ ] Compare sharing off/on with identical portfolios and budgets.
- [ ] Measure seven-search-plus-one-checker versus other within-eight-core allocations.
- [ ] Try bounded behavioral probes only where their full overhead is recovered.
- [ ] Replace the global lemma vector with bounded ancestry-aware exchange only
  if controlled sharing experiments demonstrate value.

Gate: paired held-out concurrent campaigns, not solo-union simulation. Account
for multiple noisy portfolio selections using a fresh final holdout.

## P6 — Formal assurance integration (months 3–12)

Follow `formal/superposition-checker/DEVELOPMENT_PLAN.md` and `ASSURANCE.md`.
Prioritize real production rule frequency and replay cost, adapter fidelity,
resource accounting and dual-check disagreement analysis. Publish a rule/coverage
matrix with explicit trusted boundaries. Formal rule count is not production
certification; decoded-IR validity does not verify TPTP lowering.

## P7 — Competition freeze (months 9–12)

Freeze source/toolchains/binaries/models/schedules; run official-budget campaigns
on eight physical cores, resource/shutdown stress tests, proof-output compliance
checks and independent reproduction. Report raw, strict-certified and model
scores separately. Stretch target: 30–50% relative certified-coverage growth over
P0 with >=99% supported-refutation certification yield under the declared budget.

## Work-package and commit contract

Each package records affected statuses, invariants, reproduction, independent
semantic review, negative canaries, telemetry and a rollback criterion. Commit
stable stages after `cargo check`, `cargo clippy --all -- -D warnings`,
`cargo fmt --all --check`, and `cargo test --workspace` (Nix-wrapped locally).
Run affected optional-feature gates too. Formal changes require the isolated
Verus gate. Do not promote hypotheses as fixes or claim campaign gains from tests.
