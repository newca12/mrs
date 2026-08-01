# MRS Proof Verification Specification

This document defines the trust boundary between MRS search, the competition
checker, and strict self-verification.

## 1. Verdicts

The strict kernel uses three internal outcomes:

| Kernel outcome | Meaning |
|---|---|
| `Certified` | Every reachable proof step was checked by the kernel. |
| `Rejected` | The input is structurally or logically invalid. |
| `Inconclusive` | The input uses an unsupported rule, dialect, or resource shape. |

Only `Certified` may be mapped to `VerifiedGood` in strict self-check mode.
Both `Rejected` and `Inconclusive` suppress theorem/proof output. The
competition checker may map `Rejected` to `VerifiedBad` and `Inconclusive` to
`Unknown` according to ProoVer scoring policy.

## 2. Verification Policies

### Strict kernel

Strict mode is used by `mrs --self-check`. It must be deterministic and must
not call:

- `mrs-search`
- E prover
- Vampire
- a finite-model finder
- another theorem prover
- an external process for positive proof acceptance

The kernel may accept only rules for which it recomputes the conclusion from
the cited parents or checks a precisely defined conservative transformation.

### Competition checker

`mrs-proover` competition mode supports a broader TSTP ecosystem. It may use
external ATPs, specialized Vampire/E checks, and conservative modulo-assumption
behavior to maximize ProoVer score. These facilities are not part of the strict
self-verification claim.

### Diagnostic mode

Diagnostic runs may use the in-process `MrsAtp` backend and external ATPs for
differential testing. `MrsAtp` is another invocation of the MRS search engine,
not an independent proof kernel.

## 3. Input Requirements

Strict verification requires:

- a parseable TSTP proof
- a `% Proof : ...` link to a parseable problem
- all includes resolved
- FOF or CNF proof formulas only
- unique formula names
- resolved parent references
- an acyclic parent graph
- exactly one reachable, unparented `$false` root
- every proof node reachable from that root
- every input leaf tied to a named formula in the linked problem

Anonymous `file(_,unknown)` provenance is not sufficient for strict
certification in the first kernel version.

## 4. Formula and Variable Semantics

- FOF free variables are treated as implicitly universally quantified at the
  clause boundary.
- Bound variables may be renamed alpha-equivalently.
- Free-variable identity is tracked explicitly during clause comparison.
- Conjunction and disjunction literal order is immaterial where the rule
  semantics defines a clause or multiset.
- Equality is not assumed to make arbitrary predicates symmetric.
- Predicate names never imply algebraic properties.

## 5. Initially Certified Rules

The first strict kernel implementation certifies only:

- named problem axiom/hypothesis/conjecture leaves
- direct `negated_conjecture` / `assume_negation`
- exact alpha-equivalent variable-renaming and identity rewrites
- NNF rewrites whose conclusion equals independently computed NNF
- existential-free `skolemisation` identity steps
- exact single-clause `cnf_transformation`
- first-order `resolution`
- first-order `subsumption_resolution` when it is an exact resolution step
- `factoring` over same-polarity predicate literals
- bounded `equality_resolution`
- bounded `demodulation` from cited positive unit equalities
- bounded `superposition` into a cited target clause

Skolemization involving existential elimination, definitions, equality
factoring, AVATAR, CWA, and multi-clause CNF transformations remain
inconclusive until their kernel rules are implemented.

## 6. Case-Split Requirement

AVATAR and CWA may not be represented as ordinary parent entailments. A future
case-split certificate must contain:

1. the original disjunctive clause;
2. every branch literal with its original polarity;
3. a branch-local derivation under that literal;
4. one `$false` root for every branch; and
5. a final root referencing the original split and every branch root.

Missing, duplicate, unrelated, or polarity-flipped branches are invalid.

## 7. Resource Limits

Resource exhaustion is never positive proof evidence. The kernel returns
`Inconclusive` when limits are exceeded. Initial limits include:

- maximum proof nodes
- maximum formula nodes per step
- maximum parent count
- maximum clause literals
- maximum term depth

Limits are explicit inputs to the kernel and are recorded in strict-mode
telemetry.

## 8. Required Invariants

The following invariants must hold before strict self-verification is enabled
by default:

- no theorem is emitted after `Inconclusive`, timeout, load failure, or missing
  provenance;
- no adversarial mutation reaches `Certified`;
- no strict kernel dependency reaches `mrs-search`;
- every accepted conclusion is derived from actual parent content;
- AVATAR/CWA proofs carry complete case-split certificates;
- clean-checkout benchmark results are reproducible from committed inputs,
  checksums, binary hashes, and commands.
