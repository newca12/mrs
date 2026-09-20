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

Last measured outcome (2026-09-20, `casc-30` corpus, 10 s per problem):

| Division | Ordering | Total | Certified | Fail-closed | False positives |
|----------|----------|-------|-----------|-------------|-----------------|
| EPS | KBO (s1) | 100 | 3 | 97 | 0 |
| EPS | LPO (s7) | 100 | 3 | 97 | 0 |
| EPU | KBO (s1) | 100 | 0 | 100 | 0 |
| EPU | LPO (s7) | 100 | 0 | 100 | 0 |

Dominant fail-closed reasons are resource bounds (`ground instance limit
exceeded` on 66 problems, `ground atom limit exceeded` on 23) and
out-of-fragment inputs (`equality is outside the certified fragment` where
an axiom file uses real equality, plus function terms).

Cap-sizing experiment (same corpus, `TRACE_CERTIFY=1` refusal telemetry):
raising `MAX_ATOMS` 64 → 4096 and `MAX_GROUND_INSTANCES` 100k → 500k moved
problems from instant grounding refusals into closure timeouts (28 at the
10 s budget, all EPS) with zero coverage gain — certified stayed 3/0 — and
spot checks at a 12x budget (120 s) still timed out with closures growing
past 40k clauses. The linear all-pairs closure cannot close mid-size
groundings on practical budgets, so the caps were reverted to their
conservative values: fast refusals beat slow timeouts with identical
coverage. Covering those problems needs indexed inference generation with
an indexed-vs-linear equivalence proof, which remains an open layer, not
bigger caps. The gate criterion stays zero false positives.

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
