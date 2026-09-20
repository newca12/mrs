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
- indexed lookup equivalence to linear inference generation;
- demodulation, subsumption, BCE/PLE, AC normalization, and AVATAR;
- shared-clause and portfolio-stop behavior; and
- EPR/FEQ/UEQ reference canaries with zero false positive statuses.

Until those layers are complete, `SearchResult::Saturated` from the ordinary
given-clause portfolio remains disabled. The only certified positive
saturation is the EPR `GroundOrderedResolution` witness produced by the
double-closure certifier described here.
