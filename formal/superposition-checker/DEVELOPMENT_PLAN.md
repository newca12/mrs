# Verus superposition checker: development plan

## Implementation status (2026-10-13)

**Status key:** `DONE` = deliverable and its stated acceptance gate are complete;
`PARTIAL` = an explicitly narrower slice exists and verifies, but the milestone
gate is not complete; `TODO` = work remains active and unimplemented; `DIFFICULT`
is a TODO modifier for an identified substantial obstacle or research task;
`ABANDONED` marks a specific approach deliberately stopped, with its reason
recorded. Abandoning an approach does not abandon the underlying TODO unless
the item itself is explicitly marked `ABANDONED`.

| Milestone | Status | Implemented evidence / remaining gate |
|---|---|---|
| M0 — Verus baseline and environment | **PARTIAL** | Verus/Rust versions and Linux x86_64 SHA-256 are pinned; `verify.sh` and regular CI run all formal files. Still needs a clean-checkout run and explicit audit for unchecked assumptions/ignored proof obligations. |
| M1 — Superposition semantics | **PARTIAL** | `first_order_core.rs` has recursive terms, total interpretation/valuation well-formedness, substitution preservation, bidirectional variable-renaming validity, and a verified universally quantified clause-level entailment lemma using an explicit target argument slot and term path. Still needs the standardized-parent/unifier bridge and selected calculus contract. |
| M2 — Replay primitives | **PARTIAL** | Recursive substitution application, duplicate-domain checking, executable term path lookup/replacement, whole-literal simultaneous substitution, and explicit literal-position replacement are refined against their mathematical specifications. The proposed-unifier checker has soundness postconditions; executable clause replay, standardization apart, negative-result diagnostics, and resource bounds remain open. |
| M3 — Superposition checker | **PARTIAL** | `superposition_checker.rs` verifies a ground unit fragment; `variable_superposition.rs` checks and proves one fixed variable-bearing two-literal schema with whole-target substitution. General term-tree certificates, arbitrary positions, clauses, ordering, and c502 remain unsupported/unanalysed. |
| M4 — Proof-DAG validation | **TODO** | No proof graph, parent closure, rule dispatcher, or empty-clause refutation checker. |
| M5 — MRS integration | **TODO** | The Verus executable is not called by `mrs-proof-kernel`; no interchange format or production adapter exists. |
| M6 — Broader rules | **TODO** | Resolution, AC, demodulation, transformations, and AVATAR/LRAT are not implemented in the Verus checker. |
| M7 — Assurance/release gates | **TODO** | No supported-rule manifest, full assurance case, mutation campaign, or release/default-enable gate. |

**What is DONE now:** the pinned local command `formal/superposition-checker/verify.sh`
passes all four Verus files (currently `5`, `40`, `5`, and `5` verified functions respectively),
and the existing Rust regression `rejects_swv406_c502_archived_target_specialization`
passes. These results establish only the current narrow verified artifacts; they
do not complete M0–M3 or decide c502.

The detailed milestone text below is the target specification. A milestone is
not complete merely because a related prototype exists; update this ledger and
its acceptance checkboxes as each gate is actually met.

## Executive objective

Generalize the target-rigidity pilot into a small, fail-closed **superposition
step checker** whose acceptance decisions are backed by explicit first-order
semantics, a proved superposition soundness theorem, and Verus verification of
the executable checker against that specification. The primary deliverable is
the checker for individual superposition certificates—not a general-purpose
proof kernel. Establish exactly which ordinary superposition steps are sound,
including the conditions on standardization apart, unification, substitution
application, equality orientation, and target-variable instantiation. Then
connect this verified checker to MRS's strict kernel and certification path.

The goal is **not** to verify the search engine, its heuristics, completeness,
or every feature of TSTP. The first formal claim is local and precise: every
superposition step accepted by the checker is entailed by its cited parent
clauses under the declared first-order semantics. The first integration claim
is that this checker—not a parallel reimplementation—decides superposition
steps in MRS proofs. Proof-DAG certification follows after the local rule
checker is established. Unsupported shapes, malformed certificates, exceeded
resource bounds, and inconclusive checks must fail closed.

## Read this correctly: pilot status

The original [`target_rigidity.rs`](target_rigidity.rs) verifies that a small
loop accepts exactly when no proposed substitution binding has a domain
variable in the supplied target-variable list. Its RHS is an opaque `u64`; it
does not define substitutions over terms or establish the guard as a soundness
condition. [`first_order_core.rs`](first_order_core.rs) now adds recursive
first-order term and substitution specifications plus executable lookup and
application refinement checks, a duplicate-domain validation check, a
universal-instance lemma, and a small superposition-shaped entailment example
with target-variable substitution. The executable/model bridge is not yet
completely refined; the model does not prove general superposition soundness,
full clause semantics, or a connection to `mrs-proof-kernel`.

The current results (`5 verified, 0 errors` for the original policy pilot;
`19 verified, 0 errors` for first-order core; `5 verified, 0 errors` for the
ground unit-superposition checker; `5 verified, 0 errors` for the variable
two-literal schema) are successful early proof steps. The ground checker proves
only the stated positive, ground, unit-clause rule; the variable checker covers
one fixed multi-literal schema. Neither is a general superposition checker,
proof kernel, or prover. Keep that distinction in
release notes and certification reports until the relevant gates below are
passed.

The pilot verifies an implementation of a chosen guard; it does not prove that
the guard is mathematically necessary or sufficient for superposition. The
formalization must not promote the guard into an axiom merely because both MRS
implementations and current documentation state it. In standard first-order
semantics, a universally closed clause entails each of its instances. Therefore
an MGU may bind variables from either standardized-apart parent, provided the
substitution is applied consistently to the entire inference conclusion. A
“target variables are always rigid” policy may be an implementation restriction
or a special matching discipline; it is not automatically a soundness
requirement. Derive the exact rule from clause semantics and investigate c502
as a semantic counterexample rather than choosing its verdict in advance.

## Intended trust boundary

The first deliverable checks one inference certificate:

```text
two cited parent clauses + superposition certificate
  -> validated supported-rule IR
  -> verified executable superposition replay
  -> Certified / Rejected / Unknown
```

The checker recomputes the inference from parent clauses and certificate
fields. Its specification covers terms, clauses, substitutions, parent
standardization, selected equality/literal/position, unification, replacement,
whole-clause substitution, and conclusion comparison. It does not trust
producer metadata that can be independently recomputed. Resource limits can
reject or return `Unknown`; they cannot cause acceptance. Proof-DAG closure and
empty-clause certification are later layers over the verified step checker.

Verus verification itself has a trusted computing base: the Verus tool,
verification condition generation, SMT solver, Rust/LLVM toolchain, and any
unverified input-decoding boundary. Verus explicitly does not aim to verify
its own verifier or the Rust/LLVM compilers. The project must publish this
boundary; “verified in Verus” must not be described as a proof with no trusted
tools.

## Architecture decision

Maintain a small Verus crate/project under `formal/`, centered on a callable
superposition-step checker, with:

1. mathematical specification functions/types for first-order syntax and
   semantics;
2. executable, bounded checking functions written in Verus-supported Rust;
3. refinement/postcondition proofs that executable checks implement their
   specifications; and
4. a narrow, versioned superposition-certificate IR with explicit limits and
   fail-closed verdicts; and
5. a thin adapter so the verified step checker can replace or gate the existing
   Rust replay for that rule.

Do not assume that verifying a separate model verifies the existing Rust
kernel. The integration target is one of:

- **Preferred:** compile the Verus-verified executable superposition checker and
  invoke that artifact at the superposition-check boundary (in-process or as a deterministic
  subprocess), retiring duplicate implementations for the covered rules; or
- **Transitional:** independently compare it against the Rust strict kernel,
  while continuing to call the Rust kernel authoritative. Differential
  agreement is useful evidence, but is not a proof of correspondence and must
  not be advertised as formal verification of that Rust code.

Before production integration, specify and test the input bridge. Any Rust
lowering/serialization adapter that can turn an invalid proof into a valid IR
is in the trusted boundary unless it is itself verified or the verified side
independently validates a lossless canonical encoding of the original proof.
Avoid relying on a parser that silently drops unsupported constructs.

## Workstreams and milestones

### M0 — Freeze the pilot baseline and verification environment (**PARTIAL**)

**Deliverables**

- Keep the original domain-check example, recursive substitution slice, and
  ground superposition checker as distinct smoke tests; report proof counts
  separately.
- Pin the Verus release, required Rust toolchain, Z3/tool versions, and command
  in a reproducible Nix dev shell or a dedicated lock/installer script.
- Add CI that runs the Verus verifier and fails on either verification errors
  or unreviewed assumptions/ignored proof obligations.
- Record tool versions and the verifier output as build artifacts.

**Acceptance gate**

- A clean checkout can run one documented command and reproduce each proof
  result; changing any verified executable/specification relationship makes CI
  fail.
- Documentation labels the original file as a guard pilot and the extended
  deliverable as a superposition-step-checker prototype, not a proof-DAG kernel.

### M1 — Define the mathematical contract for superposition (**PARTIAL**)

**Current status**

- **DONE:** first-order term/atom/clause data types, interpretation and valuation
  declarations, semantic evaluation/holding definitions, a non-empty carrier
  and total interpretation well-formedness predicate, and bounded instance
  examples exist in `first_order_core.rs`.
- **DONE:** a substitution/evaluation theorem is now proved for recursive term
  substitution, assuming the interpretation function and valuation have full
  domains. It establishes the semantic bridge used by clause-instantiation
  proofs.
- **DONE:** a variable-bearing two-literal superposition schema proves the
  target-side substitution must also be applied to the other target literal;
  its model-level soundness theorem verifies, and the partial-substitution
  mutation is rejected by the executable shape checker.
- **DONE:** define universal clause validity over valuations into a non-empty
  carrier and provide a total function/predicate interpretation interface.
- **DONE:** prove atom, literal, and clause truth preservation under simultaneous
  substitution, and prove that every substitution instance of a universally
  valid clause is valid. This resolves the prior undefined-predicate-lookup
  blocker by making totality explicit in the model contract.
- **DONE:** prove clause validity invariant in both directions under
  substitutions whose composition acts as the identity on every variable; this
  covers bijective variable renamings. The theorem takes the inverse pair as a
  premise, while executable freshness/injectivity checking remains M2 work.
- **DONE:** define recursive first-occurrence replacement and prove equality
  congruence through arbitrary unary/binary term contexts and predicate/equality
  atoms; a replaced literal preserves truth when the replaced and replacement
  terms have equal denotation.
- **PARTIAL:** `superposition_context_sound` combines a positive source equality,
  equal-valued selected source/target terms, and source/target clause contexts
  to prove the context-preserving conclusion is entailed. The missing bridge is
  a checked standardized-parent substitution/unifier certificate and the exact
  rule contract (including ordering/selection if required).
- **DONE, semantic theorem slice:** `universally_closed_superposition_sound`
  assumes both universally valid parent clauses and one simultaneous
  substitution applied to each whole parent. It accepts the explicit syntactic
  unification equation for the source side and target subterm, and proves the
  rewritten-target resolvent clause holds in every well-formed interpretation
  and valuation. The selected occurrence is now identified by argument slot
  plus an explicit binary child-index path. This is specification-level; bounded
  executable parsing/refinement remains M2 work.
- **TODO — DIFFICULT:** finish the exact ordinary superposition rule contract
  and bind the semantic theorem to a checked certificate (including exact
  position, variable hygiene, orientation, and any ordered-calculus conditions).

**Deliverables**

- Recursive terms: variables, constants, and unary/binary function
  applications, with symbols and arities represented explicitly; generalize to
  arbitrary arity after the structural recursion proof patterns are stable.
- Atoms: predicates and equality; literals: polarity plus atom; clauses as
  finite disjunctions whose free variables are universally quantified.
- A semantic domain consisting of a non-empty carrier, total function and
  predicate interpretations, equality as identity, and a valuation from
  variables to domain elements.
- Recursive term evaluation and clause satisfaction.
- Definitions/lemmas for free variables, substitution domains/ranges,
  renaming, positions/subterms, and equality of clauses modulo variable
  renaming and literal permutation.
- A written, independently reviewed choice of clause semantics (including
  universal closure), substitution convention, and the exact superposition
  rule being formalized; compare target-rigid matching with ordinary
  standardized-apart unification.

**Proof obligations**

- Evaluation respects structural recursion and arity.
- A clause’s universal-closure semantics agrees with the defined valuation
  semantics.
- **DONE:** a variable renaming with a two-sided substitution inverse preserves
  clause validity in both directions. Checking the finite renaming certificate
  and freshness policy remains an executable M2 obligation.
- Substitution application/evaluation lemma: evaluating `t[σ]` under a
  valuation agrees with evaluating `t` under the valuation updated by `σ`,
  under stated freshness/domain assumptions.
- **DONE:** a formal lemma that universal closure entails simultaneous
  substitution instances, with the same substitution reflected across every
  literal of the clause.
- **DONE:** prove clause validity invariant under a substitution with a
  two-sided variable-renaming inverse.
- **DONE:** prove first-order congruence for replacing an equal-valued subterm
  in a literal, both structurally and using an explicit argument slot and term
  path; the clause theorem uses the latter.
- **DONE, partial rule contract:** prove the universally closed parent-clause
  entailment theorem for a proposed simultaneous unifier and explicit target
  argument slot/path. Executable path validation, unifier checking, and
  variable standardization remain outstanding.
- **TODO:** identify precisely which freshness conditions the inference rule
  requires and specify the exact calculus, including ordered-calculus
  conditions. Decide whether rigidity is a soundness condition, a
  completeness/performance restriction, or a necessary condition of a narrower
  rule before encoding it.
- First-order core now includes universal-clause instance and
  atom/literal/clause substitution-preservation lemmas, plus a small
  ground-source, target-variable superposition-shaped entailment example. It
  proves entailment for that explicitly constrained case. The new theorem lifts
  the semantic argument to universally valid source/target clauses and arbitrary
  clause contexts, but position checking and the certificate bridge remain
  before claiming a general verified checker.
- A separate `superposition_checker.rs` now verifies executable acceptance and
  a semantic soundness theorem for positive ground unit equalities rewriting
  whole unary-predicate unit arguments. It is the first actual checker slice;
  it does not yet cover variable-bearing or multi-literal clauses.
- `variable_superposition.rs` verifies the specific `f(a)=b`,
  `q(X) | p(f(X))` to `q(a) | p(b)` schema, including a model soundness proof
  and rejection of forged partial-substitution/wrong-rewrite conclusions. It is
  still a fixed schema checker, not general superposition replay.

**Acceptance gate**

- These definitions are independent of `mrs-core` data structures and do not
  appeal to MRS tests as semantic axioms.
- Finite executable examples plus quantified lemmas cover constants,
  repeated variables, nested terms, empty clauses, and alpha-renaming.

### M2 — Verify the superposition replay primitives (**PARTIAL**)

**Current status**

- **DONE:** recursive substitution application, executable lookup/copy,
  duplicate-domain validation, general term substitution/evaluation
  correspondence, and a finite proposed-unifier checker (domain uniqueness plus
  post-substitution equality) have Verus postconditions/proofs in
  `first_order_core.rs`.
- **DONE:** define path-based term lookup/replacement, argument-slot literal
  lookup/replacement, and prove evaluation/truth preservation at a selected
  position.
- **DONE:** implement executable term path lookup/replacement and verify their
  results refine the mathematical path operations; malformed child indices and
  paths continuing below leaves fail with `None`.
- **DONE:** add executable atom/literal views, refine whole-literal simultaneous
  substitution, and refine selected argument-slot/path replacement. A regression
  exercises a literal with a non-selected argument and rejects an invalid slot.
- **PARTIAL:** the proposed unifier checker and executable term/literal path operations
  are refined; still exercise positive/negative unifier certificates with
  verified evidence, characterize the checker's negative result, specify
  occurs-check/one-step substitution policy, lift path replay to
  clauses, and implement standardization-apart.

**Deliverables**

- Bounded executable term traversal: variable collection, occurrence checks,
  subterm-at-position, replacement-at-position, and selected-position
  validation. Executable lookup/replacement refinement exists; variable
  collection, occurrence checks, and hard resource ceilings remain TODO.
- A finite substitution representation with explicit duplicate-key policy,
  lookup, application to terms/atoms/literals/clauses, and a well-formedness
  predicate (including acyclicity or a clearly specified one-step/idempotent
  convention).
- A proposed-substitution checker for the supported syntactic fragment:
  duplicate-free domain, simultaneous application, occurs-check/acyclicity as
  required by its representation, repeated-variable consistency, and a
  fail-closed resource budget. Start by validating supplied unifiers; computing
  complete MGUs is a later optimization, not a requirement for sound replay.
- Standardization-apart and certificate-side conditions: source equality
  orientation, positive polarity, non-variable rewrite side, target literal,
  term path, and any ordered-calculus restrictions selected by M1.

**Proof obligations**

- Each executable traversal refines its recursive specification.
- Applying a substitution preserves the expected syntax and has the stated
  evaluation property.
- A concrete duplicate-domain detector refines the mathematical
  `domain_unique` predicate and rejects ambiguous finite substitutions.
- A successful unifier certificate makes the two selected terms equal after
  substitution, and the same substitution is applied to all required literals
  of both parents.
- A successful one-way match binds pattern/source variables only. For general
  unification, prove the MGU properties and semantic soundness when variables
  from either parent are instantiated and the substitution is applied to both
  parent clauses as specified.
- Failure, budget exhaustion, or unsupported terms cannot return success.

**Acceptance gate**

- Universal proofs are not replaced by bounded enumeration. Bounded enumeration
  is supplementary regression testing only.
- Adversarial lemmas/tests include repeated versus distinct variables, target
  variables occurring below function symbols, variable-to-variable matches,
  cycles/occurs-check failures, and variable-ID collisions before/after
  standardization apart.

### M3 — Prove and implement the superposition checker (**PARTIAL: ground unit slice only**)

**Current status**

- **DONE:** `superposition_checker.rs` checks and proves soundness for its
  positive ground unit fragment. `variable_superposition.rs` verifies a
  variable-bearing two-literal schema and rejects leaving the non-selected
  target literal unsubstituted.
- **DONE:** both checker modules are run by the pinned Verus verification
  script/CI gate as distinct modules.
- **TODO — DIFFICULT:** general term IDs/trees, variables and unifiers, non-unit clauses,
  arbitrary selected positions, whole-clause substitution, ordered conditions,
  and the full universally quantified superposition soundness theorem. The
  archived c502 fixture is retained but not replayed by this checker.

**Deliverables**

- A declarative definition of the exact ordinary superposition rule supported
  by the checker: choose a positive source equality, orientation, non-variable
  source side, target literal and term position; standardize parents apart;
  specify whether matching or general unification is used and exactly which
  substitutions apply to each parent; replace the selected target subterm and
  form the resolvent/union of remaining literals. The base rule should use
  standardized-apart unification; a rigid-target variant, if retained, is a
  separately specified restricted rule.
- A central soundness theorem for that exact rule: for every model and
  valuation, if both standardized-apart parent clauses hold, then the checked
  conclusion holds.
- Verified executable replay for superposition certificates, including checks for
  parent roles, selected literals, positions, polarity, orientation, variable
  hygiene, conclusion construction, and exact comparison modulo the explicitly
  permitted clause equivalences.

**Central theorem**

For every interpretation and valuation, if both standardized-apart parent
clauses hold, then the clause produced by an accepted superposition step holds.
Prove this from the semantic definitions; do not
encode it as an assumption or restate an implementation guard as the theorem.
The proof must explicitly settle whether target variables may be instantiated
under the selected calculus and ensure that the computed substitution is
applied to all relevant literals of both parents. For standard universally
closed clauses, general standardized-apart unification is expected to be
sound; prove the theorem rather than assume either this expectation or the
current rigid-target policy. If a restricted rigid-target mode is retained,
document whether it is merely a completeness/performance restriction and prove
that every inference it does accept is sound. Any ordering, selection, and
freshness conditions must also be justified by the calculus contract.

**Acceptance gate**

- Positive valid steps and negative forged steps are both covered.
- **DONE for the ground unit fragment:** the checker accepts a correctly
  shaped step and rejects forged predicate/target fields; its executable
  postcondition and local model-soundness lemma verify.
- **TODO for the planned general checker:** variable-bearing and multi-literal
  positive/negative certificates, term-position replay, substitution across all
  target literals, general semantic soundness, and c502 analysis.
- The superposition checker is the named primary artifact: the specification,
  proof obligations, executable API, and test suite are all organized around
  accepting/rejecting one superposition step.
- `SWV406+1:c502` is translated losslessly into the verified IR and analyzed
  against the formal semantics. Its result—rejected as unsound, accepted as
  sound, or left inconclusive because required context is absent—is derived
  from the proved rule, not chosen in advance. Preserve the exact archived
  parents/conclusion and relevant context as a regression fixture. Also verify
  a small independently derived example where unification instantiates a
  target variable and the substitution is consistently applied to the entire
  target clause.
- Valid steps with source-variable substitutions into target terms are
  accepted; equivalent variable renamings and literal permutations preserve
  the verdict.
- Mutate the checker to apply a unifier to only the selected term/literal while
  leaving other parent literals untouched; show that the refinement or
  semantic soundness theorem fails. Separately confirm that merely removing an
  optional rigid-target restriction does not invalidate the general
  whole-clause soundness theorem. These mutation checks are diagnostic, not a
  replacement for theorem review.

### M4 — Wrap the verified step checker in proof-DAG validation (**TODO — DIFFICULT**)

**Deliverables**

- A finite proof graph model with input leaves, inference nodes, parent IDs,
  root selection, and an explicit input problem.
- A verified DAG/parent-closure traversal (or a decreasing topological-order
  certificate checker) that rejects missing parents, cycles, disconnected or
  forged leaves, duplicate/ambiguous IDs, and a non-empty claimed root.
- A verified dispatcher that invokes the superposition checker for every such
  node. Any other inference rule is unsupported and returns `Unknown` until
  separately specified and verified.
- A three-way result contract: `Certified`, `Rejected`, `Unknown`; only
  `Certified` follows from complete checks, and unsupported rules/limits yield
  `Unknown`.

**Proof obligation**

Induction over the verified DAG establishes that every certified node is
entailed by the original input; therefore a certified empty-clause node proves
the input unsatisfiable. Clarify the theorem/conjecture-negation pipeline: the
checker certifies the actual clause problem it receives, and any preprocessing
or negated-conjecture transformation must be either part of this verified
front-end or a separately justified premise transformation.

**Acceptance gate**

- Test small complete TSTP proofs and malformed/mutated variants, including
  missing parent, wrong parent, changed conclusion, wrong root, forged leaf,
  cycle, unsupported rule, and resource cutoff.
- Every unknown/rejected outcome suppresses certification in the caller.

### M5 — Integrate the verified superposition checker with MRS (**TODO — DIFFICULT**)

**Deliverables**

- A canonical, versioned interchange format for supported proof steps, with
  bounds on bytes, nodes, term depth, arguments, literals, variables, parents,
  and traversal work.
- Parsing and decoding that preserve every construct relevant to the checking
  theorem. Preferred: parse the original TSTP/proof IR inside the verified
  checker for its initial supported subset. Otherwise provide a separately
  verified refinement bridge from MRS AST to verified IR.
- A production adapter that calls the compiled verified superposition checker,
  handles
  process errors/timeouts as `Unknown`, and records tool/build/version
  provenance in telemetry.
- An explicit rollout mode: shadow/differential first, opt-in certification
  second, default only after the gates below.

**Acceptance gate**

- Actual archived proof artifacts (including c502) traverse the production
  adapter; the test does not hand-construct an already-trusted verified IR.
- Mutations at the serialized-input boundary cannot be silently dropped or
  reinterpreted into acceptance.
- The same executable artifact whose Verus proof passes is the artifact used in
  certification; record its hash and verification build identity.
- On Verus crash, version mismatch, timeout, parse error, panic, resource bound,
  or protocol violation, return `Unknown` and suppress theorem output.
- Existing strict kernel remains an independent secondary checker through the
  initial rollout. Disagreement blocks certification and is retained for
  diagnosis; agreement alone is not described as a formal proof.

### M6 — Expand beyond the superposition checker (**TODO**)

Only expand after M1–M5 are stable. Each family gets a written rule contract,
semantic theorem, verified executable replay, positive/negative mutations,
archived cases, and a versioned support declaration.

1. **Additional clause inference:** resolution, factoring, equality resolution,
   equality factoring, subsumption resolution, weakening/condensation, with
   precise set-versus-multiset and variable-renaming semantics.
2. **Equality simplification:** ground equality normalization, demodulation,
   unit rewriting and recorded rewrite traces. Prove orientation/position and
   every intermediate state; resource exhaustion is `Unknown`.
3. **Associative/commutative reasoning:** first define the equivalence relation
   and what an AC unifier/match means, then prove replay soundness and the
   applicable variable conditions. Do not inherit the prover’s AC matching
   algorithm by assumption.
4. **Formula/proof transformations:** NNF, Skolemization, CNF/Tseitin
   definitions, instantiation, existential generalization, and other currently
   supported non-clausal rules. Keep freshness, quantifier scope, and introduced
   symbol provenance explicit.
5. **AVATAR/CWA certificates:** split coverage, branch assumptions, SAT/LRAT
   traces, branch roots and roll-up. Treat the SAT proof checker as a distinct
   verified/refined component and prove certificate closure across branches.
6. **Additional TSTP dialect/rules:** add only when a rule’s semantics and
   input representation are specified. Unsupported dialect/rule forms remain
   visibly inconclusive.

### M7 — Release, audit, and maintain the assurance case (**TODO — DIFFICULT**)

**Deliverables**

- CI gates for Verus verification, mutation-negative checks, supported proof
  corpus, formatting, and deterministic build/hash recording.
- A machine-readable list of certified rules and limits emitted with each run.
- An assurance note listing definitions/theorems, verified functions, trusted
  tools, unverified adapters, assumptions, unsupported cases, and known
  limitations.
- A re-audit policy: a semantic theorem, verifier version, IR format, rule
  implementation, or trusted bridge change invalidates affected historical
  certification totals until rechecked.

**Default-enable gate**

- The initial supported corpus is accepted/rejected exactly as specified and
  all archived soundness regressions—including c502—behave correctly.
- No mutation in the threat model produces a certified result.
- Every feature outside the verified whitelist fails closed.
- Proof reviewers independently inspect the definitions and central soundness
  theorems; automated verification success is necessary, not sufficient review.
- The formal checker is integrated into the actual certification path and its
  binary/source provenance is reproducible.

## Cross-cutting test and review strategy

For every inference rule, retain both sides of the boundary:

- valid positive certificate(s), including renaming/permutation variants;
- forged conclusion, wrong parent, wrong position, invalid polarity/orientation;
- target-variable-binding cases on both sides: sound whole-clause MGU instances
  must be accepted where the rule permits them; genuinely invalid partial
  substitutions must be rejected;
- explicit semantic decision for disputed rule side conditions, independently
  reviewed rather than inherited from the producer, kernel, or existing docs;
- resource and malformed-input cases that must return `Unknown` or `Rejected`;
- archived real proof nodes plus minimized synthetic cases;
- mutations that remove one guard or alter one premise, to establish that the
  acceptance result depends on the intended checks.

Use the existing strict Rust kernel and external proof tools as differential
oracles during development, never as axioms for the Verus semantic theorems.
Keep bounded testing distinct from unbounded universal proof obligations.

## Initial supported-core target

**Status: TODO.** The list below is the intended first release target; it is
broader than the currently implemented ground-unit slice described in the
status ledger above.

The first meaningful release target is a **verified superposition step checker**.
It does not need to verify an entire prover proof DAG to demonstrate the core
claim. Its intended supported input is:

- two already-lowered first-order clauses with predicates, equality, literals,
  and universal-closure semantics;
- an explicit certificate naming the source equality/orientation, target
  literal/position, and proposed unifier;
- standardized-apart parents and whole-clause substitution semantics;
- recomputed conclusion equality modulo only proved variable-renaming and
  literal-order equivalences; and
- bounded syntax/certificate sizes with fail-closed `Unknown` behavior.

**Implemented slices, not the release target:** the ground unit checker and the
single `f(a)=b`, `q(X) | p(f(X))` two-literal schema are implemented and
verified. Arbitrary terms, substitutions, clause width, positions and general
superposition soundness are still outstanding.

The first end-to-end proof-DAG/refutation checker is a follow-on milestone that
uses this verified step checker and adds validated input leaves, parent closure,
and empty-clause root checking. Explicitly defer AC superposition,
demodulation, ordered-calculus side conditions until specified, Skolemization/
CNF proof transformations, other inference rules, AVATAR/LRAT,
satisfiability/model claims, and unrestricted full TSTP.

## Immediate next tasks

- [x] Pin Verus/toolchain and add the verification script plus CI workflow (M0
  partial; clean-checkout and assumptions audit remain TODO).
- [x] Implement and verify the restricted ground unit-superposition checker,
  soundness lemma, and forged-field examples (M3 partial).
- [x] Preserve the archived c502 clauses as a fixture, explicitly marked
  unsupported by the current checker (c502 audit remains TODO).
- [ ] Specify the exact ordinary superposition rule and required freshness
  conditions (M1; universal semantics, substitution preservation, and
  bidirectional variable-renaming validity are now verified).
- [x] Add a proposed finite-unifier checker that requires a duplicate-free
  domain and equal terms after simultaneous substitution (M2 initial slice).
- [ ] Exercise positive/negative unifier certificates; finish constructor-wide
  executable/model refinement, occurs-check, positions, replacement, and
  standardization-apart before widening the checker (M2).
- [x] Specify and prove recursive term substitution/evaluation and lift it to
  atom/literal/clause preservation under total interpretation/valuation domains
  (M1 semantic foundation).
- [ ] Use it with clause semantics to prove general ordinary superposition
  soundness and extend the checker beyond ground-unit clauses (M1–M3).
- [ ] Encode the exact `SWV406+1:c502` parents/conclusion and relevant context;
  establish whether the substitution was applied consistently and whether the
  conclusion is entailed before changing producer/kernel behavior (M3).
- [ ] Decide the interchange boundary and integrate this verified step checker
  into the Rust strict-kernel path before adding proof-DAG or broader-rule
  coverage (M5 architecture gate).
