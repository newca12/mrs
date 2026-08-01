# mrs-proof-kernel

Independent strict proof kernel for a bounded, provenance-linked subset of
TPTP/TSTP. The kernel checks proof steps from their parsed parents and
conclusions. It does not accept a proof because an inference name looks
familiar or because another theorem prover agrees with it.

## Trust Boundary

`mrs-proof-kernel` deliberately depends only on:

- `mrs-core` for lowered formulas, terms, clauses, symbols, and substitutions;
- `mrs-tptp` for the parsed TPTP/TSTP representation.

It does not depend on `mrs-search`, `mrs-proover`, ATP adapters, external
processes, global mutable verifier state, or network services. Search may
generate a proof, but it is not trusted by the kernel.

## API

```rust
use mrs_proof_kernel::{verify_strict, KernelVerdict, VerificationLimits};

let verdict = verify_strict(
    &problem,
    &proof,
    VerificationLimits::default(),
);

assert!(matches!(verdict, KernelVerdict::Certified));
```

Use `verify_strict_with_source` when every `file(...)` leaf must cite the exact
path recorded by the proof header.

The result is intentionally three-way:

- `Certified`: every reachable node was checked by an implemented kernel rule;
- `Rejected(reason)`: the proof is structurally or logically invalid;
- `Inconclusive(reason)`: the proof uses an unsupported shape or exceeds a
  deterministic resource limit.

Resource exhaustion is never proof evidence. Callers must not map
`Inconclusive` to a theorem or unsatisfiability result.

## Certified Coverage

The current kernel checks:

- named problem leaves, roles, provenance, source identity, and reachability;
- unique names, parent references, acyclicity, disconnected nodes, and one
  reachable unparented `$false` root;
- negated conjectures, NNF transformations, and alpha-equivalent identities;
- single-parent Skolemization with fresh symbols, exact arity, active-universal
  scope, distinct witnesses, nested matrices, regrouped universals, and
  bounded associative matrix matching;
- fresh biconditional definitions and simple bounded CNF clause extraction;
- resolution, factoring, equality resolution, subsumption resolution,
  demodulation, and superposition;
- explicit CWA-style `split_component` and `avatar_sat_refutation`
  certificates with complete branch coverage.

Subsumption resolution uses standardized-apart, one-way multiset matching and
requires the conclusion to be exactly the target clause with one justified
literal removed.

## Unsupported Shapes

The kernel returns `Inconclusive` for, among other cases:

- multi-parent Skolemization;
- equality factoring and justified literal-deletion rules not listed above;
- multi-clause or nested definitional CNF transformations outside the bounded
  clause-extraction fragment;
- general AVATAR SAT proofs without explicit complete case-split certificates;
- unsupported TPTP dialects, sequents, or formula shapes.

These boundaries are deliberate. Expanding the accepted rule set requires a
parent-recomputation rule and adversarial tests; adding a fallback ATP is not a
kernel implementation.

## Limits

`VerificationLimits` bounds proof size, formula size, clause width, term depth,
rewrite steps, subsumption matching steps, and Skolemization matching steps.
Matching and transformation backtracking return `Inconclusive` when their
budgets are exhausted.

## Testing

Run the kernel tests inside the repository development environment:

```sh
nix develop -c cargo test -p mrs-proof-kernel
```

Run the required workspace checks before committing changes:

```sh
nix develop -c cargo fmt --all --check
nix develop -c cargo check
nix develop -c cargo clippy --all -- -D warnings
nix develop -c cargo test --workspace
```

Adversarial tests cover forged conclusions, invalid provenance, malformed
preprocessing, incorrect Skolem scopes and witnesses, incomplete case splits,
and resource-limit fail-closed behavior.
