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
- an external process or ATP for positive proof acceptance

The kernel may use the workspace's in-process CaDiCaL SAT solver only for the
bounded propositional consistency check of an explicit `avatar_sat_refutation`
certificate that has no replayable SAT trace. This is not used to validate
first-order inference steps; the split, branch, provenance, and SAT-variable
bounds are checked by the kernel first.

The kernel may accept only rules for which it recomputes the conclusion from
the cited parents or checks a precisely defined conservative transformation.

### Competition checker

`mrs-proover` competition mode supports a broader TSTP ecosystem. It may use
external ATPs, specialized Vampire/E checks, and conservative modulo-assumption
behavior to maximize ProoVer score. These facilities are not part of the strict
self-verification claim.

Competition-mode `superposition` ATP queries may append problem-level background
AC unit equalities (commutativity/associativity from axioms and the NNF of the
negated conjecture) to the step's premise list. `parents_len` stays the original
parent count so propositional checks only see real parents; the ATP sees the
full premise list. This mirrors the strict kernel's background-AC replay for
steps whose parents do not cite the AC law the rewrite used (e.g. SWX217's
negated conjecture).

Explicit AVATAR certificates are checked structurally in competition mode before
the ATP fallback: split metadata must cover the source clause, each component
must match its declared branch and SAT context, each branch refutation must
derive `$false` under a cited component context, and the final roll-up must
cover every satisfiable assignment of the cited split constraints. When a
verified LRAT/SAT-trace payload binds the certificate to the original SAT
instance, the kernel returns `Certified` after structure + binding checks and
skips re-solving. Without a replayable payload, structure validation still
binds every SAT variable to a real split component; the kernel then rebuilds
the propositional instance from those split contexts and asks the in-process
CaDiCaL solver for unsatisfiability. A SAT model at that point is positive
evidence against the certificate (`Rejected`); solver `Unknown` remains
`Inconclusive`. Legacy `avatar_sat_refutation` nodes without explicit
metadata are no longer plain ATP-fallback inputs — they follow this
structure + CaDiCaL path.

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
- at least one unparented `$false` root
- every proof node is structurally validated; nodes outside a root derivation
  cannot affect the accepted refutation
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

- named problem axiom/hypothesis/conjecture leaves, matched by exact
  formula name first, then file-source provenance
- direct `negated_conjecture` / `assume_negation`
- exact alpha-equivalent variable-renaming and identity rewrites, over one
  or more identical parents
- NNF rewrites whose conclusion equals independently computed NNF
- single-parent `skolemisation` with exact fresh witnesses, scope, arity,
  bounded associative matrix matching, and effective quantifier polarity under
  negation; steps may skolemize a subset of existentials while preserving the
  rest, and complete single-parent E-style `skolemize` metadata is also
  checked when supplied
- bounded Vampire-style multi-parent `skolemisation` with validated
  `skolem_symbol_introduction` axiom parents, dependent rewrite ordering,
  fresh witness declarations, scope/arity checks, and exact final formulas
- exact single-clause `cnf_transformation`, including bounded nested fresh
  Tseitin definitions, flat clause-shaped full definitions, and transitive
  definition dependencies
- bounded quantified `cnf_transformation` after one explicitly cited,
  independently checked Skolemization parent; NNF, universal prenexing, and
  clause expansion are recomputed, while missing/unrelated witness parents
  remain inconclusive or rejected
- first-order `resolution`
- first-order `subsumption_resolution` with standardized-apart set matching
  (the same target literal may witness several active literals once they
  coincide under the substitution) and exact target-literal deletion; sound
  because set inclusion of σ(active) in the flipped target still yields
  `active ∧ target ⊨ target \ {L}`
- `factoring` over same-polarity predicate literals, accepted modulo
  condensation of both intermediate factors and the exported conclusion
- bounded `equality_resolution` modulo condensation of both parent resolvent
  and exported conclusion
- bounded `equality_factoring`
- ground-unit `equality_normalization` over positive ground constant
  equalities (the target is the parent that is not a positive ground unit
  equality, regardless of parent order; both target and conclusion are
  normalized with the same union-find classes — recursing into nested
  function arguments — before comparison, so the verdict does not depend on
  which class representative either side picked; non-ground equality parents
  remain inconclusive, and a positive target that becomes reflexive is
  rejected)
- bounded `condensation` trying both equality orientations of the removed /
  matched pair and comparing both the raw and condensed expected clauses
  against the exported conclusion
- bounded formula equivalence after independent NNF and AC/idempotent
  canonicalization
- bounded universal `instantiate` steps with exact substitutions and rigid
  nested binders
- bounded single-parent `existential_gen` steps with consistent witness
  replacement and rigid nested binders
- bounded multi-parent `conjunction` steps with exact parent-part coverage
  modulo associative/commutative conjunction order and bounded matching work
- bounded single-parent `split_conjunct` projection with preserved universal
  prefixes, checking direct conjuncts before quantifier stripping
- structural `copy`, `duplicate`, rename/alpha, and double-negation aliases
  checked as identity (over one or more identical parents) or equivalence
  with bounded canonicalization, never by rule name alone; `alpha`
  additionally accepts a direct conjunct projection
- bounded single-parent `excluded_middle` steps concluding a tautological
  `A | ~A`
- bounded two-parent `modus_ponens` with exact implication matching and outer
  universal instantiation
- bounded `horn` forward chaining over direct single-antecedent implications
  and fact parents; unsupported conjunction antecedents remain inconclusive
- `consequence` when a cited parent pair recomputes as a bounded resolution
  step, or when a cited `$false` parent derives `$false`; the pair search is
  step-bounded and exhausts to `Inconclusive`
- exact one-parent identity aliases for `assume` and `rewrite`
- `ex_falso` only from a cited `$false` parent or a recomputed two-parent
  contradiction
- bounded `weaken` steps that add disjuncts to a parent clause after independent
  NNF/canonicalization
- exact one-parent `reflexivity` steps whose conclusion is `t = t`
- bounded ground `transitivity` steps with explicit equality orientation and
  common-middle-term checks; variable-bearing chains remain inconclusive
- `commute` and `reassociate` through bounded formula-equivalence checking
  and `instantiate_mp` through exact modus-ponens recomputation
- bounded `contrapositive` implication steps and `disjunctive_syllogism`
  disjunct deletion, both recomputed from their cited parents
- `paramodulation` through bounded superposition recomputation in either
  parent order
- bounded `demodulation` from cited positive unit equalities
- bounded `superposition` into a cited target clause, with background
  associative/commutative replay when the problem (or its negated
  conjecture, which is a premise of every refutation of that conjecture)
  supplies the AC law the rewrite used

Unannotated multi-parent E-style `skolemize` forms and general directional
multi-parent CNF transformations remain inconclusive until their kernel rules
are implemented. Bounded explicit AVATAR certificates may include a
replayable `frat-lrat` payload; unsupported RAT, incremental, and other SAT
trace variants remain inconclusive.
CWA-style splits are accepted only through the explicit case-split certificate
described below.

## 6. Case-Split Requirement

CWA and AVATAR may not be represented as ordinary parent entailments. A
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
- maximum AVATAR SAT variables
- maximum subsumption matching steps
- maximum Skolemization matching steps

The kernel also rejects inconsistent predicate/function arities across the
linked problem and proof DAG.

Limits are explicit inputs to the kernel and are recorded in strict-mode
telemetry. Telemetry also reports each reachable proof node in topological
order with its name, rule, parent count, lowered formula size, and bounded
clause-literal count.

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
