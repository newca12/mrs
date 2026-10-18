# Verus superposition checker: development plan

## Implementation status (2026-10-18)

**Status key:** `DONE` = deliverable and its stated acceptance gate are complete;
`PARTIAL` = an explicitly narrower slice exists and verifies, but the milestone
gate is not complete; `TODO` = work remains active and unimplemented; `DIFFICULT`
is a TODO modifier for an identified substantial obstacle or research task;
`ABANDONED` marks a specific approach deliberately stopped, with its reason
recorded. Abandoning an approach does not abandon the underlying TODO unless
the item itself is explicitly marked `ABANDONED`.

| Milestone | Status | Implemented evidence / remaining gate |
|---|---|---|
| M0 — Verus baseline and environment | **PARTIAL** | Verus release and Linux x86_64 SHA-256 are pinned; the requested Rust toolchain follows the release's `rust-toolchain.toml` (`1.99.1`). The pinned release is `0.2026.10.18.7d82994`; `verify.sh` records both requested and effective toolchains. The installer and verifier scripts read the JSON pin with Python rather than requiring `jq`. The full `verify.sh` run passes. The escape-hatch audit is part of the script. `mutation-check.py` requires Verus to reject three executable/model mutations. Remaining: broader mutation coverage and audit ignored proof obligations. |
| M1 — Superposition semantics | **PARTIAL** | `first_order_core.rs` proves parent validity survives independent standardization substitutions and connects accepted general replay to `standardized_superposition_sound` through `accepted_replay_preserves_validity`. Arbitrary selected indices and duplicate-preserving contexts are covered. `RULE_CONTRACT.md` specifies the prototype's unordered rule. Remaining: independent review of the calculus/interchange boundary. |
| M2 — Replay primitives | **PARTIAL** | Recursive substitution, term/atom/literal/clause replay and term-position replacement are refined. Collector exactness now composes with independent freshening-scan postconditions to prove variable-valued images throughout each parent, and accepted certificates expose `renames_variables_on` for every parent literal. The replay conclusion builder has a verified pointwise sequence contract; term replacement is proved to commute with substitution at valid positions and is lifted through atom/literal slots. Collector-to-exec-set correspondence, complete hygiene/position refinement, and cumulative resource accounting remain open. |
| M3 — Superposition checker | **PARTIAL** | `check_superposition_and_replay` now postconditions both exact ordered conclusion equality and universal validity preservation for accepted general certificates. `replay_semantic_premises` connects checked source equality, selected target occurrence, unifier equation and replacement to `standardized_superposition_sound`. Context deletion/substitution and exact conclusion-order composition are proved. A bounded executable certificate accepts archived `SWV406+1:c502` and rejects a forged conclusion. Full TSTP translation/proof-DAG replay, equivalence modulo literal order/renaming, complete hygiene/resource refinement, and independent calculus review remain. |
| M4 — Proof-DAG validation | **PARTIAL** | `certify_dag` returns `Certified` with `dag_inputs_unsatisfiable`, proving no well-formed interpretation satisfies the supplied universally closed IR inputs. DAG dispatch supports input, superposition and positive-left/negative-right nullary-predicate resolution nodes. Accepted resolution replay postconditions universal validity preservation; `check_dag_resolution_step` binds it to cited parents and the node conclusion, and the generic induction consumes local entailment for both rules. Compiled unit and multi-step resolution refutations certify without empty inputs. `check_root_ancestor_closure` rejects every disconnected node and returns `Unknown` on bounded traversal exhaustion. The certifier partitions work across structural preflight, closure and per-node validation; within a node, parent-edge checks and local replay share the assigned node budget. Remaining: exact accounting inside recursive local replay helpers, production input adapter and broader rules/fixtures. |
| M5 — MRS integration | **TODO** | The Verus executable is not called by `mrs-proof-kernel`; no interchange format or production adapter exists. |
| M6 — Broader rules | **PARTIAL** | Nullary-predicate resolution is verified and DAG-dispatched; c502 is now additionally replayed as a local general superposition example. General first-order resolution, AC, demodulation, transformations, and AVATAR/LRAT remain unimplemented. |
| M7 — Assurance/release gates | **PARTIAL** | Added a machine-readable supported-rule/limits manifest, a TCB/re-audit note, and a CI audit for `assume`, `admit`, and external-body/allow escape hatches. CI now uploads a Verus evidence log with source commit, binary hash, toolchain, and proof output. Mutation corpus, supported proof corpus, reproducible production artifact, and release gates remain. |

**What is DONE now:** the pinned local command `formal/superposition-checker/verify.sh`
passes all four Verus files (currently `5`, `204`, `5`, and `5` verified functions respectively),
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
- **DONE:** prove independent standardization substitutions preserve each
  universally closed parent, and lift the position-specific superposition
  soundness theorem to parent clauses standardized separately before a shared
  unifier is applied to both.
- **DONE:** define recursive first-occurrence replacement and prove equality
  congruence through arbitrary unary/binary term contexts and predicate/equality
  atoms; a replaced literal preserves truth when the replaced and replacement
  terms have equal denotation.
- **PARTIAL:** `superposition_context_sound` combines a positive source equality,
  equal-valued selected source/target terms, and source/target clause contexts
  to prove the context-preserving conclusion is entailed. The missing bridge is
  a checked standardized-parent substitution/unifier certificate and the exact
  rule contract (including ordering/selection if required).
- **DONE:** `standardized_superposition_sound` composes parent validity,
  independent freshening instances, common-unifier substitution, and selected
  literal replacement semantically. Its premises state the exact substitution
  and position equations; the executable checker’s finite renaming maps and
  term paths still need refinement lemmas into those premises.
- **DONE:** derive validity of the ordinary binary resolvent clause by
  concatenating the source context, target context, and rewritten selected
  literal from the standardized-parent semantic theorem.
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
- **DONE, theorem interface:** `proposed_unifier_certificate_sound` packages the
  exact mathematical premise established by `check_unifier`: duplicate-free
  finite simultaneous substitution and equality of the two instantiated terms.
  The general certificate checker now also postconditions that the source and
  target selected terms are equal after the supplied unifier, that the selected
  target position exists, that the target freshening is applied to that raw
  selected term, and that the selected source literal is a positive equality
  with a non-variable left side. The accepted selected-term equation is exposed
  under explicit semantic aliases for the freshened source equality side and
  the target term at the raw selected path. The relation now includes the
  unifier on both sides: accepted certificates prove equality of the source
  left side and selected target term after freshening and unification. Accepted
  certificates also state the exact source-left term after freshening and
  unification and equate the raw/fresh target terms with their executable-view
  aliases, making these refinements directly composable. A successful target
  position is now proved to survive both the target freshening and common
  unifier, with its final selected term equal to the certificate's semantic
  alias. The remaining bridge is the replayed replacement/resolvent sequence
  into the semantic theorem call.
- **DONE:** define path-based term lookup/replacement, argument-slot literal
  lookup/replacement, and prove evaluation/truth preservation at a selected
  position.
- **DONE:** implement executable term path lookup/replacement and verify their
  results refine the mathematical path operations; malformed child indices and
  paths continuing below leaves fail with `None`.
- **PARTIAL, semantic bridge:** the general selected-position renaming lemma
  now accepts a variable-only-on-the-clause-variables premise and proves that
  the selected argument-slot/path traversal commutes with substitution. An
  explicit wrapper packages this as the executable freshening position lemma.
  Certificate replay now separately checks that the target freshening mapping
  is variable-valued on the selected target literal's variables and exposes
  that as an accepted-checker postcondition. Replay also exposes that the
  selected literal's variables are a subset of its whole target clause's
  variables, matching the collector's whole-clause freshness domain.
  The checker requires raw and freshened path sequences to be identical, but
  the executable certificate has not yet been proved to supply all lemma
  premises end to end; this is not the general checker theorem.
- **DONE:** add executable atom/literal views, refine whole-literal simultaneous
  substitution, and refine selected argument-slot/path replacement. A regression
  exercises a literal with a non-selected argument and rejects an invalid slot.
- **DONE:** add an executable clause vector/view, refine simultaneous
  substitution over every literal, and add clause-level selected literal/path
  replacement. Out-of-range literal indexes and invalid atom slots fail closed.
- **DONE, pointwise replacement contract:** `replace_clause_literal_exec` now
  postconditions that the resulting clause has the same literal count, the
  selected literal is exactly the position-rewritten literal, and every other
  literal is preserved in order. The separate relational lemma records the same
  pointwise shape at the mathematical clause layer.
- **DONE, local work bound:** successful recursive term/atom/clause variable
  collection requires the incoming shared work counter to be at most its limit
  and returns with it still at most the limit. Each visited term and the clause
  traversal charge through the shared counter; cumulative budgets across
  separate collector invocations and other certificate loops remain open.
- **DONE, variable-map acceptance contract:** `is_variable_renaming` now has an
  exact postcondition relating success to the model predicate that every
  variable in the supplied sequence receives a variable-valued substitution
  image; absent bindings and non-variable images both map to failure.
- **DONE, freshening scan postcondition:** successful `check_freshening` now
  directly states that every variable present in its finite input sequence has
  a lookup to a variable term. Its sequence-validity, variable-image, and
  membership guarantees are independent comma-separated `ensures` clauses.
  Chaining implications with `&&` previously nested these guarantees under
  earlier implications, preventing callers from obtaining the intended facts.
- **DONE, freshening representation bridge:**
  `renaming_images_cover_clause_variables` composes the collector's
  `clause_collection_exact` postcondition with the finite renaming-image
  predicate to prove that every model-level clause variable has a variable
  image. `freshen_clause_apart` now obtains the lemma's premises directly from
  the bounded collector and successful freshening scan, without repeating the
  variable-renaming scan. `collected_renaming_covers_selected_literal` lifts
  this to `renames_variables_on` for every literal. The certificate checker
  exposes that property for both parents as independent acceptance
  postconditions. The compatibility wrapper delegates to this same verified
  freshening path. This resolves the finite-map/collector caller blocker;
  the accepted position/unifier and local-soundness bridge is now proved below.
- **PARTIAL:** bounded variable collection and finite freshening checks now
  exist. They reject absent/non-variable mappings, mapped IDs in the avoid set,
  non-injective images, mismatched raw/freshened target paths, and explicit work
  exhaustion. The successful freshening check has a postcondition characterizing
  mappings over the supplied variable list. The bounded collector proves exact
  variable-set membership against `literal_variables(view_clause(...))`; its
  bridge to the parallel executable `exec_clause_variables` set, first-occurrence
  ordering, successful variable-only binding postconditions, and cumulative work
  accounting remain. Successful freshening also directly guarantees that every
  variable in the supplied list has a variable-valued lookup result.
- **PARTIAL, executable slice:** the step-certificate IR requires independent
  source/target variable maps. It checks bounded freshening and unifier
  conditions, replays the selected target slot/path, forms a binary resolvent,
  and compares ordered literals. Whole-clause substitution now also postconditions
  literal-count preservation and a pointwise correspondence for every literal;
  selected-clause replacement directly postconditions that it changes exactly
  the selected literal and preserves all other literals. The certificate checker now postconditions that
  every variable occurring in either parent is mapped to a variable term by its
  corresponding freshening. The replay-and-conclusion comparison wrapper also
  postconditions that an accepted conclusion's literal sequence agrees with
  executable replay in the mathematical literal view. The specification
  `expected_superposition_conclusion` describes the selected source equality
  removal and rewritten target literal in order, and replay-to-specification is
  proved. The certificate checker rejects before path comparison when
  the aggregate sizes of its parent literal vectors, freshening maps, unifier,
  and raw/fresh paths exceed the work limit, and successful acceptance exposes
  that bound as a postcondition. Duplicate-domain validation rejects before
  its quadratic pair scan when `n²` exceeds the supplied work limit, and
  unifier acceptance exposes that guard. Replay also caps the combined parent
  and conclusion literal counts before materializing standardized clauses and
  its output. A structural traversal additionally caps the aggregate nodes in
  parent/conclusion terms and substitution replacements before replay. Work
  across separate checks and derived replay is not cumulatively accounted.
  Local semantic soundness and whole-DAG IR certification are now proved;
  cumulative accounting remains open.
- **DONE, semantic proof connection:** the replay refuses target paths that differ
  before/after standardization. `replay_semantic_premises` connects checked
  unifier application and replacement to the theorem. Arbitrary-index parent
  contexts and exact resolvent composition complete accepted-result soundness.
  Comparison modulo additional clause equivalences remains unsupported.
- **DONE, ordered binary-clause contract:** the semantic theorem proves validity
  of the exact concatenation `source-context ++ target-context ++ rewritten
  target-literal`. The current checker compares this literal sequence exactly;
  clause permutation and alpha-equivalence normalization are unsupported rather
  than silently assumed.
- **PARTIAL:** the proposed unifier checker and executable term/literal/clause path operations
  are refined. Verified examples now cover a successful substitution, a wrong
  instance, a symbol-shape mismatch, a duplicate domain, and a cyclic RHS under
  the documented simultaneous one-step semantics. The checker validates a
  supplied substitution rather than computing an MGU; characterize the negative
  result further, lift path replay to
  the entire inference certificate, and implement standardization-apart.

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

### M3 — Prove and implement the superposition checker (**PARTIAL: general replay soundness proved**)

**Current status**

- **DONE:** `superposition_checker.rs` checks and proves soundness for its
  positive ground unit fragment. `variable_superposition.rs` verifies a
  variable-bearing two-literal schema and rejects leaving the non-selected
  target literal unsubstituted.
- **DONE:** both checker modules are run by the pinned Verus verification
  script/CI gate as distinct modules.
- **PARTIAL:** executable term trees, supplied unifiers, non-unit clauses,
  arbitrary selected target paths, whole-clause substitution and conclusion
  construction now exist, with an end-to-end validity-preservation postcondition
  proved via `standardized_superposition_sound`. Still needed: full hygiene and
  cumulative resource refinement, calculus boundary review, comparison modulo
  permitted clause equivalences, broader certificate fixtures, and full
  archived proof-context translation.

**Deliverables**

- A declarative definition of the exact ordinary superposition rule supported
  by the checker: choose a positive source equality, orientation, non-variable
  source side, target literal and term position; standardize parents apart;
  specify whether matching or general unification is used and exactly which
  substitutions apply to each parent; replace the selected target subterm and
  form the resolvent/union of remaining literals. The base rule should use
  standardized-apart unification; a rigid-target variant, if retained, is a
  separately specified restricted rule.
- A central soundness theorem for that exact rule: in every interpretation,
  universal validity of both parents implies validity of the checked conclusion.
- Verified executable replay for superposition certificates, including checks for
  parent roles, selected literals, positions, polarity, orientation, variable
  hygiene, conclusion construction, and exact comparison modulo the explicitly
  permitted clause equivalences.

**Central theorem**

For every interpretation, if both universally closed parent clauses are valid,
then the clause produced by an accepted superposition step is valid. Parent
truth at just one valuation is insufficient when the supplied unifier
instantiates variables.
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
- **DONE for general semantic soundness:** accepted exact replay preserves
  universal validity, including whole-parent substitution and arbitrary positions.
- **TODO:** broader variable-bearing/multi-literal positive/negative certificate
  fixtures, complete hygiene/resource refinement, and full c502 proof-context
  analysis.
- The superposition checker is the named primary artifact: the specification,
  proof obligations, executable API, and test suite are all organized around
  accepting/rejecting one superposition step.
- **DONE, local step replay:** `c502_exec_certificate` reconstructs the exact
  archived c27/c39/c502 clauses as executable IR, supplies disjoint parent
  freshenings and a unifier that maps target variables to source-side terms,
  rewrites the selected nested occurrence, and compares the full ordered
  conclusion. `executable_c502_certificate_example` accepts the step and rejects
  a forged conclusion; accepted replay's semantic postcondition proves validity
  preservation. The exact source clauses remain archived in the fixture; the
  verified regression uses a manually encoded executable certificate rather
  than parsing that TPTP fixture.
- **DONE, target-variable binding:** this archived replay binds standardized
  c39 variables from the target parent and applies the substitution to both
  complete parent clauses before the selected rewrite. In particular, c27's
  surviving `X2 = X1` and predicate context become `X2 = X5` and
  `~contains_slb(X0,X2)` exactly as in c502.
- **DONE, source-variable binding:** a separate positive certificate binds a
  freshened source equality variable to a ground target subterm and replays the
  resulting exact conclusion. The central accepted-replay postcondition applies
  to this case as well.
- **TODO:** translate the complete relevant TSTP proof context losslessly into
  the verified IR and certify c502 through the production-style adapter. Do not
  infer full proof certification from the local step result. Also verify
  a small independently derived example where unification instantiates a
  target variable and the substitution is consistently applied to the entire
  target clause.
- **DONE, source-variable substitution fixture:** the compiled runtime regression
  accepts a step whose unifier binds a standardized source variable to a target
  ground term. Equivalent conclusion literal permutations and alpha-equivalence
  remain unsupported.
- Mutate the checker to apply a unifier to only the selected term/literal while
  leaving other parent literals untouched; show that the refinement or
  semantic soundness theorem fails. Separately confirm that merely removing an
  optional rigid-target restriction does not invalidate the general
  whole-clause soundness theorem. These mutation checks are diagnostic, not a
  replacement for theorem review.

### M4 — Wrap the verified step checker in proof-DAG validation (**PARTIAL — DIFFICULT**)

**Current status**

- **DONE, fail-closed preflight:** `check_dag_structure` rejects graphs over
  the node/input limit, aggregate parent-edge and literal limits, and aggregate
  clause-term structure limit, including each superposition certificate's
  clauses, substitutions, and paths. It requires exactly two parent edges for
  superposition, checks parent references are in-range and topologically
  earlier, rejects unsupported node shapes, and runs the verified local checker
  for each superposition. The legacy preflight returns `Unknown`; certification
  is provided separately by `certify_dag`.
- **DONE, conditional semantic theorem:** induction over topologically ordered
  nodes proves validity from valid inputs given `local_node_entails_from_parents`
  for every inference node; an empty root then implies unsatisfiability.
- **DONE:** `check_dag_superposition_step` binds source/target/conclusion to cited
  nodes, runs verified replay, and proves `local_node_entails_from_parents`.
  The local contract uses universal validity, matching the induction hypothesis.
  Mutation gates reject skipping replay and ignoring the cited source parent.
- **DONE, whole-DAG semantic acceptance:** `certify_dag` validates every node via
  `validate_dag_node`, whose independent postcondition exposes exact input
  membership, topological references and universal local entailment. The loop
  preserves `validated_dag_node`; `validated_dag_has_no_model` invokes induction
  and constructs a total valuation in the nonempty carrier to contradict the
  empty root. `Certified` implies no model of the supplied IR input clauses,
  without semantic preconditions or interpretation arguments at the API.
  Four new verifier-negative mutations and compiled DAG regressions pass.
- **DONE, nullary resolution DAG integration:** complementary 0-ary predicate
  pivots (left positive, right negative) are proved sound;
  `check_nullary_resolution` validates bounded inputs, this oriented polarity,
  pivot symbol and exact ordered resolvent construction. Runtime
  regressions cover valid, wrong-pivot, forged-conclusion, out-of-range and
  exhausted-budget certificates. `Resolution(lp, rp)` stores the two pivot
  indices directly in the node kind. `check_dag_resolution_step` proves local
  entailment from accepted replay; `supported_inference` generalizes the DAG
  induction without reconstructing rule-specific premises inside it.
  Compiled `p, ~p -> empty` and `p | q, ~p, ~q -> q -> empty` graphs certify
  without empty input clauses. Forged context deletion, duplicate-pivot deletion,
  wrong symbols, wrong orientation, self-parent and invalid pivot regressions
  fail closed. Mutations reject skipped replay, wrong checked conclusion and
  omitted oriented polarity.
- **DONE, root ancestor closure:** certification now runs a bounded reverse
  topological traversal from the root and rejects every supplied node outside
  its dependency closure. Initialization, parent-edge traversal and final
  coverage validation share the explicit work counter; exhaustion is
  inconclusive at the closure API and cannot lead to certification. Runtime
  regressions cover a disconnected but otherwise valid input leaf and
  zero-budget exhaustion.
- **PARTIAL, phase budget partition:** `certify_dag` reserves disjoint portions
  of the caller's work ceiling for structural preflight, root-closure checking
  and per-node replay. This prevents adding unbounded graph-wide repetitions of
  a locally bounded check. Some local replay helpers still account work with
  separate counters, so this is not yet an exact cumulative operation budget.
- **PARTIAL, within-node composition:** `validate_dag_node` charges parent-edge
  validation and node structure before passing only the remaining share to the
  superposition/resolution checker. Deep helper calls still use local bounded
  counters rather than one shared mutable meter; the exact sum of all primitive
  visits remains unproved.
- **DONE, zero-share fail-closed guard:** if integer budget partitioning would
  assign no work to a node, `certify_dag` returns `Unknown` before validation.
  Runtime cases verify that low limits for one- and three-node DAGs do not
  certify.
- **DONE, bounded closure regression:** the runtime suite asserts the exact
  closure work threshold for a one-node root graph: budgets below the complete
  traversal return `None`, and the exact complete budget returns `Some(true)`.
- **DONE, strict connected-graph regression:** a complete `p, ~p -> empty`
  proof certifies; appending an otherwise-valid unrelated input node is rejected;
  removing it restores certification. This directly exercises the runtime
  regression used by the root-closure mutation gate.
- **Remaining:** general first-order resolution and other refutation rules,
  production TPTP-to-IR validation, exact cumulative replay accounting within
  local checks and broader rules/fixtures remain separate gates. M4 is still
  partial.

**Deliverables**

- A finite proof graph model with input leaves, inference nodes, parent IDs,
  root selection, and an explicit input problem.
- A verified DAG/parent-closure traversal (or a decreasing topological-order
  certificate checker) that rejects missing parents, cycles, disconnected or
  forged leaves, duplicate/ambiguous IDs, and a non-empty claimed root.
  **DONE:** root ancestor closure is explicitly checked; disconnected nodes are
  rejected and work exhaustion cannot certify.
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

**Current status**

- **DONE, documentation slice:** `ASSURANCE.md` records the current bounded
  claims, trusted computing base, unverified adapter boundary, and re-audit
  triggers. `supported-rules.json` declares current verified slices and limits.
- **DONE, CI audit slice:** `verify.sh` runs `audit-verus-source.py`, which fails
  on unreviewed `assume`, `admit`, `external_body`, and verifier-allow escapes.
- **DONE, evidence slice:** CI stores the verification log, including source
  commit, Verus binary SHA-256, Rust toolchain, verifier version, and per-module
  proof output.
- **TODO — DIFFICULT:** add mutation-negative CI, supported proof corpus,
  deterministic build/hash artifacts, and independent theorem review. The
  manifest is not yet emitted per run because no production checker adapter is
  integrated.

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

**Status: PARTIAL.** The list below is the intended first release target. The
general local superposition checker now accepts arbitrary supported executable
certificates, including c502, but equivalence normalization, complete resource
refinement and the production interchange boundary remain.

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

**Implemented slices:** the ground unit checker and fixed variable schema remain
separate smoke-test modules. The general executable replay now supports bounded
recursive terms, supplied substitutions, multi-literal clauses and arbitrary
selected positions, with exact ordered conclusion comparison and a universal
validity-preservation theorem. It does not yet accept literal permutation or
alpha-renamed conclusions, and its support still has documented resource and
hygiene gaps.

The first end-to-end proof-DAG/refutation checker is a follow-on milestone that
uses this verified step checker and adds validated input leaves, parent closure,
and empty-clause root checking. Explicitly defer AC superposition,
demodulation, ordered-calculus side conditions until specified, Skolemization/
CNF proof transformations, other inference rules, AVATAR/LRAT,
satisfiability/model claims, and unrestricted full TSTP.

## Immediate next tasks

- [x] Pin Verus/toolchain and add the verification script plus CI workflow (M0
  partial; fresh installation and all-module verification reproduce in a
  detached clean worktree; ignored-proof-obligation and mutation audits remain
  TODO).
- [x] Implement and verify the restricted ground unit-superposition checker,
  soundness lemma, and forged-field examples (M3 partial).
- [x] Preserve the archived c502 clauses as a fixture and verify a manually
  encoded local whole-parent superposition certificate; fixture parsing and full
  proof translation remain open.
- [x] Specify the prototype's unordered superposition rule and required
  freshness conditions in `RULE_CONTRACT.md` (M1; the production calculus
  boundary still needs independent review).
- [x] Add a proposed finite-unifier checker that requires a duplicate-free
  domain and equal terms after simultaneous substitution (M2 initial slice).
- [x] Exercise positive/negative unifier certificates, including duplicate
  domains, wrong instances, and cyclic RHS terms under the explicit one-step
  policy. This is evidence for the supplied-substitution checker, not MGU
  computation or an occurs-check theorem.
- [ ] Finish constructor-wide executable/model refinement, occurs-check policy,
  positions, replacement, and
  standardization-apart before widening the checker (M2). The collector-exactness
  to model-variable freshening bridge now composes with the executable scan,
  and accepted certificates expose the domain-restricted renaming premise for
  every parent literal. The semantic position/resolvent soundness bridge is proved;
  complete hygiene/resource refinement remains.
- [x] Specify and prove recursive term substitution/evaluation and lift it to
  atom/literal/clause preservation under total interpretation/valuation domains
  (M1 semantic foundation).
- [x] Connect accepted replay end-to-end to the existing general ordinary
  superposition soundness theorem and extend the checker beyond ground-unit
  clauses (M1–M3). `accepted_replay_preserves_validity` composes the checked
  premises and arbitrary-index contexts with the general semantic theorem.
- [x] Prove the general replay constructor's exact ordered resolvent shape and
  expose equality with `expected_superposition_conclusion` on successful
  replay. This connects the executable construction and conclusion comparison;
  the selected-path/unifier-to-semantic-premise connection is now proved.
- [x] Prove that a successful explicit term position remains valid after an
  arbitrary simultaneous substitution and that the selected term is transformed
  by that substitution. This removes the false assumption that position
  preservation requires variable-only renaming; the lemma is now lifted through
  atoms and polarity-preserving literal substitution. Certificate-to-theorem
  composition is now proved.
- [x] Expose certificate source/target term aliases on successful checker
  acceptance, including the freshened source equality argument, the raw target
  occurrence, and its unifier instance. These postconditions connect executable
  selected-term and freshening facts to their mathematical expressions.
- [x] Compose the successful-position substitution theorem with accepted
  certificate premises, exposing the selected target occurrence after both
  freshening and the supplied unifier.
- [x] Prove that replacing the instantiated selected occurrence with the
  identically instantiated source right side is structurally defined for every
  accepted valid position, by lifting path stability and replacement existence
  through atom slots.
- [x] Prove pointwise term replacement commutes with simultaneous substitution
  along a valid path, then lift this equality through predicate/equality atom
  slots and literal polarity. General certificate acceptance now exposes the
  semantic selected occurrence and successfully constructed rewrite as explicit
  postconditions. Parent-context/resolvent theorem composition is now proved.
- [x] Expose the accepted rewritten literal itself in the semantic views of the
  certificate source equality and target clause, so the right-side instantiation
  used by executable replay is tied to the theorem-facing selected literal.
- [x] Refine the source equality's right-hand side through executable freshening
  and unifier replay to the theorem-level `certificate_selected_source_right`
  term.
- [x] Add a semantic-context split lemma for any accepted exact resolvent
  sequence, exposing the non-equality source context and non-selected target
  context lengths. `parent_context_facts` and explicit index witnesses now
  connect these contexts to the exact conclusion.
- [x] Prove that each parent-context concatenation is a subsequence of the
  exact executable conclusion and compose that decomposition with clause
  validity to invoke the general semantic superposition theorem. Explicit index
  witnesses transfer both contexts and the rewritten literal to the conclusion.
- [x] Prove the clause validity disjunction principle for context concatenation
  under universal valuations, and add an executable-refinement lemma that a
  successfully replayed literal replacement is defined in the mathematical
  target literal after whole-literal substitution.
- [x] Derive exact rewrite-term agreement from the accepted source equality's
  executable RHS mapping. (Replacement existence is covered by the checker
  postcondition; semantic clause entailment is now proved.)
- [x] Prove the singleton rewrite consequence from positive source and target
  unit clauses for the ground identity-unifier case. This confirms the local
  semantic basis, but does not yet handle non-empty contexts or checked
  substitutions.
- [x] Prove clause validity invariant under a bijective literal-position
  permutation, and provide an exact equality between selected-literal-update
  and prefix/rewritten-literal/suffix resolvent encodings. This discharges the
  calculus's sequence-order conversion without assuming clause order is
  semantically significant.
- [x] Postcondition the exact binary resolvent literal count from successful
  replay (`source width - selected equality + target width`) to make the
  constructor's structural relation explicit.
- [x] Prove the sequence identity for deleting one indexed source literal and
  reinserting the target clause with one selected literal rewritten (the
  rewrite's contents are abstracted in this width/decomposition helper).
- [x] Connect the fully replayed resolvent to `standardized_superposition_sound`
  and prove `local_node_entails_from_parents` for a successful DAG step. This is
  discharged by `accepted_replay_preserves_validity` and the executable
  `check_dag_superposition_step`. The local contract now correctly consumes
  universally valid parent clauses, rather than parent truth at one valuation.
  Whole-DAG acceptance-loop integration is now proved by `certify_dag`.
- [x] Add a verifier-negative mutation that omits the rewritten target clause
  from the executable resolvent builder; the central constructor postcondition
  makes Verus reject the mutation.
- [x] Define universally closed nullary-predicate resolution soundness and a
  bounded executable checker that reconstructs the exact ordered resolvent.
  Standalone and DAG regression cases exercise positive and forged steps.
- [x] Compose accepted resolution replay with local DAG entailment and the
  generic induction, certifying nontrivial empty-root IR refutations with
  nullary-predicate resolution. Production TPTP integration remains open.
  - [x] Require every proof-DAG node to be in the root's ancestor closure, with
  bounded traversal work and rejection/inconclusive runtime regressions (M4).
- [x] Encode the exact `SWV406+1:c502` parents/conclusion as a verified local
  certificate; establish that the whole-parent substitution is consistent and
  the conclusion is entailed by the proved superposition theorem (M3).
- [ ] Translate the relevant archived TSTP proof context losslessly and certify
  c502 through the future interchange adapter (M3/M5).
- [ ] Decide the interchange boundary and integrate this verified step checker
  into the Rust strict-kernel path before adding proof-DAG or broader-rule
  coverage (M5 architecture gate).

- [x] Add verifier-negative mutation gates that omit substitution from
  executable literal replay, ignore literal polarity, and replace selected
  position lookup with the root term. Verus rejects all three mutations
  (M0/M7 audit evidence). Resolution, resolvent and DAG mutations now run too;
  broader corpus and threat-model coverage remain TODO.
