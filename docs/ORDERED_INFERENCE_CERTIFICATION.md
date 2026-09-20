# Ordered-Inference Certification

## Scope

The prover has two distinct ordered-inference modes:

- ordinary ordered search, which is useful for finding refutations but is not
  allowed to claim positive saturation; and
- `--certify-ordered`, which runs the bounded certifier described here.

The certifier currently covers only the following fragment:

- function-free relational EPR clauses, exhaustively grounded over the finite
  constants in the input (or one fresh domain constant when the input has no
  constants);
- predicate atoms only, with equality excluded;
- no AVATAR assertions or formula-level clauses;
- KBO with positive symbol weights and a total precedence on the input
  signature; and
- no pruning, AC normalization, simplification, indexing, or portfolio sharing.

This is deliberately narrower than the full first-order superposition engine.
Unsupported inputs return `GaveUp` rather than being treated as certified.

## Certificate Construction

For a finite grounded clause set, no inference can introduce a new term or
atom. The certifier therefore computes two finite closures:

1. ordered resolution using all literals maximal under the validated KBO; and
2. an unrestricted ground-resolution reference closure over the same input.

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
nix develop -c cargo run -- --workers 1 --strategy 1 --certify-ordered problems/socrates.p
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
- KBO/LPO substitution stability and valid custom signatures;
- indexed lookup equivalence to linear inference generation;
- demodulation, subsumption, BCE/PLE, AC normalization, and AVATAR;
- shared-clause and portfolio-stop behavior; and
- EPR/FEQ/UEQ reference canaries with zero false positive statuses.

Until those layers are complete, `SearchResult::Saturated` from the ordinary
given-clause portfolio remains disabled.
