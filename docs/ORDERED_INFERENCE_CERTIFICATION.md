# Ordered-Inference Certification

## Scope

The prover has two distinct ordered-inference modes:

- ordinary ordered search, which is useful for finding refutations but is not
  allowed to claim positive saturation; and
- `--certify-ordered`, which runs the bounded certifier described here.

The certifier currently covers only the following fragment (EPR focus is
retained by design):

- function-free relational EPR clauses, exhaustively grounded over the finite
  constants in the input (or one fresh domain constant when the input has no
  constants);
- predicate atoms only, with equality excluded;
- no AVATAR assertions or formula-level clauses;
- KBO with positive symbol weights and a total precedence on the input
  signature, then LPO with a total precedence (weights are irrelevant to
  LPO and are not required); AC orderings are explicitly unsupported; and
- no pruning, AC normalization, simplification, indexing, or portfolio sharing.

`Saturated` is enabled for this EPR fragment only: the certifier re-checks
the pre-grounding inputs for pure relational EPR before returning
`CompletenessWitness::GroundOrderedResolution`. Non-EPR inputs fail closed
with `GaveUp` and never certify saturation.

This is deliberately narrower than the full first-order superposition engine.
Unsupported inputs return `GaveUp` rather than being treated as certified.

## Certificate Construction

For a finite grounded clause set, no inference can introduce a new term or
atom. The certifier therefore computes two finite closures:

1. ordered resolution using all literals maximal under the validated KBO
   or LPO ordering; and
2. an unrestricted ground-resolution reference closure over the same input.

The independent double-closure reference is retained by design: the
certifier does not replay the given-clause trace, it recomputes both
closures from the grounded input and requires agreement.

The result is accepted only when both closures agree:

- both derive the empty clause, producing a refutation; or
- both saturate without the empty clause, producing
  `CompletenessWitness::GroundOrderedResolution`.

Large groundings that exceed the closure tier take a second path instead:
Tier 2 encodes the grounded set propositionally and asks CaDiCaL to decide
it (`crates/mrs-search/src/certified_sat.rs`). A satisfiable verdict plus
an independently re-verified model produces
`CompletenessWitness::SatBackedGrounding`; unsatisfiable, unknown, and
failed-model outcomes all fail closed. Tier 2 is SAT-direction only by
design (there is no FRAT-to-TSTP elaborator for UNSAT proofs) and skips
ordering validation (meaningless for model checking). Tiers are selected by
grounding size with no CLI change; where both tiers run, their verdicts
must agree (tested differentially).

If the closures disagree, or a resource limit is reached, certification fails
closed with `GaveUp`.

The implementation is in `crates/mrs-search/src/certified.rs`.

## Usage

Use one worker and the explicit flag:

```bash
# KBO strategy (e.g. strategy 1)
nix develop -c cargo run -- --workers 1 --strategy 1 --certify-ordered problems/socrates.p
# LPO strategy (e.g. strategy 7)
nix develop -c cargo run -- --workers 1 --strategy 7 --certify-ordered problems/socrates.p
```

The current CLI path is diagnostic. It does not alter the default CASC
portfolio, and it does not certify equality, function terms, AC, AVATAR, or
heuristically simplified searches. Variable-bearing relational EPR inputs are
accepted only when their finite exhaustive grounding stays within the
certifier's resource bounds.

## Required Expansion Before Broader Enablement

The next certification layers require independent proofs and tests for:

- non-ground ordered resolution and factoring;
- ordered superposition side conditions and equality resolution/factoring;
- KBO/LPO substitution stability and valid custom signatures (ground
  totality/transitivity plus KBO weight and KBO/LPO precedence validation
  are certified; lifted stability is still open);
- indexed lookup equivalence to linear inference generation (per-query
  recall/validity/removal evidenced in `tests/index_equivalence.rs`;
  end-to-end trace equivalence still open);
- demodulation, subsumption, BCE/PLE, AC normalization, and AVATAR;
- shared-clause and portfolio-stop behavior; and
- EPR/FEQ/UEQ reference canaries with zero false positive statuses.

Until those layers are complete, `SearchResult::Saturated` from the ordinary
given-clause portfolio remains disabled. The only certified positive
saturation is the EPR `GroundOrderedResolution` witness produced by the
double-closure certifier described here.

## EPR Reference Canaries

The soundness gate is `crates/mrs-bench/epr_certify_canaries.sh`. Ground
truth comes from CASC division labels, never from the prover: every problem
under `EPS/` is expected satisfiable and every problem under `EPU/` is
expected unsatisfiable. A refutation on EPS, or a saturation on EPU, is a
false positive and fails the gate; `GaveUp`/`Timeout`/`ResourceOut` are
allowed (fail-closed). Every problem runs under both certified orderings
(`--strategy 1` KBO, `--strategy 7` LPO).

Last measured outcome (2026-09-20, `casc-30` corpus):

Fast gate, 10 s per problem (latest run; EPS counts wobble 5–6 across
runs — see note below):

| Division | Ordering | Total | Certified | Fail-closed | False positives |
|----------|----------|-------|-----------|-------------|-----------------|
| EPS | KBO (s1) | 100 | 6 | 94 | 0 |
| EPS | LPO (s7) | 100 | 6 | 94 | 0 |
| EPU | KBO (s1) | 100 | 0 | 100 | 0 |
| EPU | LPO (s7) | 100 | 0 | 100 | 0 |

Deep gate (`--deep`, 120 s per problem):

| Division | Ordering | Total | Certified | Fail-closed | False positives |
|----------|----------|-------|-----------|-------------|-----------------|
| EPS | KBO (s1) | 100 | 8 | 92 | 0 |
| EPS | LPO (s7) | 100 | 10 | 90 | 0 |
| EPU | KBO (s1) | 100 | 0 | 100 | 0 |
| EPU | LPO (s7) | 100 | 0 | 100 | 0 |

(The fast-gate jump from 3 EPS is the Tier-2 SAT path converting
closure-bound groundings — e.g. NLP116-1: 200 034 clauses over 5 211
atoms, verified model. Counts wobble by ±1 across runs (5–6 on either
ordering) — budget-edge flakiness: problems deciding within milliseconds
of the deadline flip with load; both outcomes are sound. The Tier-3 and
capture instrumentation did not move the fast counts outside that band.
EPU stays zero throughout: the SAT tier certifies satisfiability only,
Tier 3 found no small cores at either budget, and EPU problems otherwise
die at grounding size or real equality content before any closure runs.
No Tier-3 refutation ever fired on EPS — as required for truly
satisfiable problems.)

Dominant fail-closed reasons are resource bounds (`ground instance limit
exceeded` on 66 problems, `ground atom limit exceeded` on 23) and
out-of-fragment inputs (`equality is outside the certified fragment` where
an axiom file uses real equality, plus function terms).

Cap-sizing experiment (same corpus, `TRACE_CERTIFY=1` refusal telemetry):
raising `MAX_ATOMS` 64 → 4096 and `MAX_GROUND_INSTANCES` 100k → 500k moved
problems from instant grounding refusals into closure timeouts (28 at the
10 s budget, all EPS) with zero coverage gain — certified stayed 3/0 —
because the linear all-pairs closure could not close mid-size groundings
even at 12x budget. The caps were then kept at the raised values *together
with* indexed closures (see below), which makes them usable: retrieval is
no longer the bottleneck. The gate criterion stays zero false positives.

`TRACE_CERTIFY=1` emits per-refusal sizes (`refuse=instance_limit
estimated=… vars=… constants=…`, `refuse=atom_limit atoms=…`,
`refuse=closure_time ordered=… clauses=… inferences=…`) plus a summary line
per successful certification, following the `TRACE_LRS` precedent, so
future cap changes stay data-driven.

## Indexed Lookup Equivalence

The given-clause loop retrieves inference partners through `LiteralIndex`
(discrimination + feature-vector trees) and the demodulation `STreeId`,
never by linear scan. The trees document an imperfect-filter contract —
over-approximation allowed, misses forbidden — and
`crates/mrs-search/tests/index_equivalence.rs` pins it differentially:
every query the engine issues (resolution partners, superposition
targets/sources, all four subsumption candidate directions, demodulation
generalizations) is answered both by the index and by a naive linear scan
with an independent exact oracle (Robinson unification/matching on legacy
terms, `mrs_calculus::subsumption`), asserting recall (every exact hit is
indexed), validity (coarse-filter and FVI necessary conditions on indexed
hits), and removal consistency, with non-vacuity counters on every oracle
so the assertions cannot pass on empty exact sets. Fixtures mine
solved-run shapes from `results/` (GRP123-4.004 EPR vocabulary, UEQ-style
equational stores, a mixed-arity trap, foreign-term robustness queries).
The suite caught one oracle-side bug during development (per-position
fresh-substitution unification wrongly reported unifiable pairs the tree
correctly rejected); no index recall violation was found.

Out of scope for this layer: end-to-end search-trace equivalence (would
need a linear-scan dual-run mode) and AVATAR-gated clause filtering, which
happens after index retrieval.

## Indexed Closures (Phase 3 Outcome)

Both certifier closures now retrieve partners through a `LiteralIndex`
over hash-consed clause twins (`closure_indexed` in `certified.rs`) instead
of scanning all pairs; the linear implementation is retained under
`#[cfg(test)]` as the equivalence reference. A dedicated unit test
(`indexed_closure_matches_linear_closure`) asserts identical status always
and identical saturated clause sets on SAT and multi-step UNSAT fixtures
under both orderings — refuted runs are status-compared only, since both
sides correctly stop at the first empty clause and their truncated sets
may differ by pair-visit order. The ordered/reference agreement check is
unchanged, and the EPU canary gate stays the empirical backstop against
correlated index misses: any agreeing false saturation on EPU fails loudly.

Measured effect on the casc-30 gate (10 s budget, raised caps): zero false
positives, coverage unchanged at 3 SAT / 0 UNSAT. The indexed closure
explores ~20–50x more per second (linear stalled at ~5k clauses; indexed
reaches ~100k clauses with ~460k inferences), but the remaining EPS
problems have genuinely enormous ground closures — 300 s probes still time
out at ~90k clauses, and PUZ028-4 hits the 1M inference cap, correctly
fail-closed. EPU never reaches closure at all: roughly half its problems
refuse on grounding size and half on real equality content in (included)
axiom files, which is outside the predicate-only fragment. Peak RSS on the
largest explored grounding is ~524 MB. Conclusion: retrieval speed is no
longer the coverage bottleneck; closure *size* is — which is what the
SAT-backed Tier 2 below addresses for the satisfiable side.

## Tier 3: Constant-Subset Unsatisfiability Search

Groundings that are infeasible in full (size refusals) or proved UNSAT
without a proof (Tier-2 outcomes) fall through to a bounded search over
small constant subsets (singletons, then biased pairs/triples with try
caps). Each subset grounds with vocabulary restriction — only clauses
whose constants lie in the subset are kept, which is sound for the
refutation direction — and runs the full Tier-1 double closure with
agreement and TSTP ancestry. A subset refutation is a valid whole-problem
refutation (subset instances ⊆ full instances); subset saturation proves
nothing and is skipped. The tier can only refute, never certify
satisfiability, and shares the run deadline. Constants are tried in
goal-biased order (negated-conjecture vocabulary first, then frequency).

Measured: unit tests cover small-core refutation, saturation-never,
zero-budget instant failure, bias order, and the Tier-2-UNSAT fallthrough.
Whether corpus EPU problems yield small cores is empirical (deep-gate
numbers below); pigeonhole-style problems needing many constants stay
fail-closed by design (documented incompleteness, not a regression).

Boundary: Tier 3 fires only on size refusals and Tier-2 UNSAT, never on
fragment errors (equality, function terms, formula/AVATAR clauses).
Vocabulary restriction could in principle drop offending clauses and still
refute soundly, but that would smuggle non-EPR reasoning into an
EPR-only tier — explicitly out of scope; such inputs stay `GaveUp`.

## UNSAT Capture-First Measurement (Phase 5a Outcome)

Tier-2 UNSAT outcomes currently fail closed (`Tier2Unsat` → Tier-3 subset
search → usually exhausted) because no FRAT-to-TSTP elaborator exists yet.
Before building emission, the UNSAT path is instrumented for measurement
only — verdicts are unchanged, TRACE gains lines:

- The solver runs always-traced (`connect_trace` with antecedents and
  finalize, 1M event cap that bounds memory and fails closed on overflow),
  so SAT runs pay tracing overhead deterministically instead of depending
  on re-solve reproducibility.
- On UNSAT, `capture_and_check` disconnects, counts original/derived
  events, and runs the independent RUP re-check (`check_proof_trace`);
  `sat_trace_capture events=… originals=… derived=… check=…` records the
  outcome. RAT witnesses, malformed steps, and capture failures all fail
  closed; the verdict still maps to `Tier2Unsat` in every case.
- A contract test pins the `Solver::value` FFI convention the model
  verifier depends on (variable assignment regardless of sign — proven by
  test after the polarity-bug episode, where passing signed literals
  through inverted every negative literal).

End-to-end proof that the pieces compose: a synthetic 5 002-clause
Tier-2-window UNSAT problem yields `sat_trace_capture events=10009
originals=5002 derived=1 check=ok`, falls through to Tier 3, refutes via
the `{core}` singleton on try 1 with agreement, and reports `% SZS status
Unsatisfiable`.

Corpus measurement outcome (deep TRACE pass, 200 problems, s1): **zero**
Tier-2 UNSAT completions — every corpus UNSAT either refuses before the
solver (grounding/equality) or outlasts the budget inside it — so RAT
frequency is unmeasurable on this corpus and the synthetic probe
(`check=ok`, no RAT witnesses) remains the only UNSAT-capture datum. The
emission phase must therefore validate RAT handling on synthesized
hard-UNSAT Tier-2-window fixtures (e.g. pigeonhole EPR encodings), not on
corpus problems. Related harness finding: 22 deep runs exceeded even a
200 s wall wrapper (killed, no claim — fail-closed); the gate wrapper
margin is now budget + 120 s, and kills count as fail-closed, never error.
Always-trace overhead did not move gate outcomes (fast gate re-verified
green after the change).

## Deadline Hardening (Straggler Forensics)

Gate forensics caught workers living 80 s+ on a 10 s budget: grounding
materialization (`instantiate_clause` recursion) and SAT encoding had no
deadline checks, so multi-million-instance inputs burned unbounded wall
time before any capped loop ran. Both now carry amortized checks (every
4096th instance/clause) failing closed past the deadline. Rule of thumb
going forward: every unbounded loop in the certification path — including
test-only and telemetry-adjacent code — needs a deadline or count check;
the gate's long tail is bounded by the per-problem wrapper otherwise.

## SAT-Backed Tier 2 (Phase 4 Outcome)

Groundings too large for either closure (up to 2M instances / 16 384
atoms) are encoded propositionally and decided by CaDiCaL. The model is
then re-verified clause by clause; only a passing re-check yields
`Saturated`. During development the re-check caught a real polarity bug
(`Solver::value` reports variable assignment even for negative arguments —
passing signed literals through inverted every negative literal), which
would otherwise have kept Tier-2 coverage at zero behind a passing solver
verdict. Tier-2 UNSAT fails closed by design (no FRAT-to-TSTP elaborator);
ordering validation is skipped (vacuous for model checking, quadratic at
16k atoms). Encoder, verifier, asymmetry, and Tier-1-vs-Tier-2 differential
agreement are unit-tested; the canary gate re-validates empirically
(EPU zero-FP is the backstop: any unsound SAT claim there fails loudly).
Measured peak RSS on the largest Tier-2 grounding is ~524 MB. Remaining
EPR coverage needs lazy/incremental grounding (the 66 instance-limit + 19
overflow problems never materialize) or the UNSAT proof pipeline —
both declared future work, not regressions: Tier 1 behavior is unchanged
where closures terminate.
