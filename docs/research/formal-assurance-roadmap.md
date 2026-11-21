# Formal assurance implementation roadmap

Active plan, 2026-11-21; starting source `bd04ae44`. The
[guarantee inventory](../reference/formal-guarantees.md) defines what is true
today. This plan defines claim expansions, dependencies and acceptance gates;
it is not a promise that the whole prover has been verified.

## Target and non-goals

The primary target is **end-to-end refutation soundness for an explicit TPTP
fragment**, from an immutable selected problem closure and certificate bytes
to a justified emitted status. A separate target covers original-problem finite
model certification. Search may remain untrusted if every accepted result has
an independently verified certificate.

Completeness, producer performance, arbitrary input dialects and a verified
compiler/OS are separate projects. No existing theorem about IR is renamed into
a theorem about TPTP; no test suite is renamed into a semantic proof. No default
or competition schedule changes as a consequence of this roadmap.

Status vocabulary: **proved + integrated**, **proved standalone**,
**runtime checked**, **implemented/pending verification**, **open**. A task is
not proved because its file exists or CI is configured.

## Dependency order

```text
F0 claim inventory / evidence ledger
 └─ F1 semantic carrier and model transport
     ├─ F2 byte/AST/input-closure fidelity
     │   └─ F3 verified normalization and certificate transport
     │       ├─ F4 FOF conservative transformations
     │       ├─ F5 AVATAR + propositional certificates
     │       └─ F6 finite models / EPR grounding
     └─ F7 ordered calculus + dynamic search completeness (separate target)
F8 build, runtime, integration and reproduction gates spans every phase
```

Within each phase, land small vertical slices with positive and forged-certificate
controls. Prove acceptance implications first; widen coverage only after the
contract is integrated. Prioritize representative produced proof shapes, not
the number of named rules.

## F0 — One truthful claim inventory

**Owner area:** assurance/docs/tooling. **Paths:** `docs/reference/`,
`formal/superposition-checker/{ASSURANCE.md,supported-rules.json}`,
`crates/mrs-bench/src/tooling/formal.rs`.

- [x] Inventory actual CLI policies, semantic theorems, standalone lemmas,
  ordinary checks, trusted boundaries and unsupported claims.
- [x] Audit the model carrier rather than assuming “first-order” implies
  arbitrary models; retain the `Set<u64>` baseline finding.
- [x] Link README/current docs to the inventory; distinguish historical scores
  from the assurance contract.
- [ ] Add a machine-readable claim ledger and drift checks linking each claim
  to source symbols, formal dispatch, manifest limits, gate logs and exact
  corpus identities. Missing/unknown evidence must not become `proved`.
- [ ] Generate current evidence summaries from that ledger rather than hand-edit
  scores; keep historical records immutable.

**Gate:** each headline has one exact scope/status/TCB, and an independent reader
can find its implementation and validation. This phase does not expand logic.

## F1 — Semantic universes, then standard-model transport

**Owner area:** formal semantics. **Path:** `first_order_core.rs`.

- [x] Implement mathematical integer denotations and `ISet<int>` domains in the
  main core, retaining machine-width symbol/variable IDs. This is the first
  repair; completed verification evidence is recorded by the dated audit.
- [x] Add an infinite successor interpretation, injectivity and out-of-range
  clauses, and a value above `u64::MAX` as semantic regression obligations.
- [x] Complete the full pinned proof/runtime/bridge/mutation gate for that
  repair: 578 main-core checks, zero module errors, reproducible runtime,
  real bridge tests and the complete 58/50 mutation inventories pass.
- [ ] Parameterize semantic interpretations by an arbitrary carrier type, or
  prove an explicit sound model transport. A countable carrier must not silently
  acquire an unproved all-cardinality/Löwenheim–Skolem bridge.
- [ ] Re-prove substitution, equality/context replacement, all local replay and
  whole-DAG induction over that carrier. Document totality/nonemptiness and
  syntax-ID assumptions separately.
- [ ] Align or explicitly retire the narrow machine-denotation pilot modules;
  do not mistake the target-rigidity pilot for the general soundness theorem.

**Gate:** accepted DAG implies no model in the **stated** semantic universe;
the larger universe has an explicit proof, not only changed documentation.
No executable inference acceptance or wire tag changes are needed for the
initial carrier repair.

## F2 — Exact input meaning and immutable closure

**Owner area:** parser/lowering/loader/formal bridge.
**Paths:** `mrs-tptp`, `src/{include,lowering,pipeline}.rs`, ProoVer loaders,
`mrs-proof-kernel/src/verus.rs` and a bounded verified input module.

- [ ] Freeze original problem/include bytes and selections before search;
  require certification to consume that same closure, not reread changed files.
  Preserve file/selection identities and source/name/role binding.
- [ ] Define a small untyped CNF byte grammar and its denotation. Prove bounded
  token spans, identifier classification, variables, polarity, equality, arity
  and complete clause conversion; reject every unsupported construct explicitly.
- [ ] Prove AST/decoded-clause fidelity and symbol/variable identity mapping.
  Do not silently trust a serialized/reparsed round trip as a formal refinement.
- [ ] Extend a verified selected-include contract incrementally. Filesystem
  access/authenticity may remain an explicit TCB assumption, but omitted or
  unselected formulas must not enter the checked theory.
- [ ] Widen syntax only with an explicit semantics/refinement; broad parsing
  support must not imply typed, arithmetic or higher-order proof support.

**Gate:** the formally certified supplied clauses have the same meaning as the
exact supported original problem closure. Forged source names, selection
changes, include races, inconsistent arities and unsupported syntax fail closed.
Fuzz/differential tests support this gate but do not replace its proof.

## F3 — Normalization/certificate transport

**Owner area:** formal replay + bridge. **Paths:** `first_order_core.rs`,
`mrs-proof-kernel/src/verus.rs`, `INTERCHANGE.md`.

- [ ] Integrate the **already proved standalone** literal-permutation comparator
  into explicit DAG/wire normalization nodes. Preserve multiplicity and require
  checked forward/inverse witnesses under the cumulative work meter.
- [ ] Implement checked executable bijective alpha-renaming and compose it with
  permutation; eliminate the corresponding trusted conclusion rewriting.
- [ ] Elaborate equality orientation, identity/copy and multi-step rule aliases
  to verified nodes instead of replacing an original conclusion in host code.
- [ ] Preserve exact original/canonical parent identities and certificate
  closure, including every used fresh symbol and intermediate rule step.

**Gate:** the theorem justifies the original conclusion modulo only explicitly
proved transformations. Mutate duplicate/lost literals, non-inverse witnesses,
renaming collisions, wrong parents, intermediate conclusions and meter resets.
An unsupported transformation is Unknown, not a trusted “equivalent” shortcut.

## F4 — FOF-to-CNF and conservative signature extension

**Owner area:** formal transformations + clausification.
**Paths:** `mrs-cnf`, ordinary strict transformation checks, new formal FOF IR.

- [ ] Specify FOF quantifiers/connectives, universal closure and conjecture
  negation with original role/source identity.
- [ ] Prove bounded NNF and universal/prenex transformations, including variable
  capture avoidance and mixed quantifier polarity.
- [ ] Prove fresh definitional predicates and Skolemization as **conservative /
  satisfiability-preserving signature extensions**, with witness dependencies,
  arity, freshness and scope—not ordinary local entailment under every old model.
- [ ] Connect transformed-clause unsatisfiability back to the original problem
  by an explicit extension/transport theorem; integrate the exact ancestry DAG.

**Gate:** original satisfiability implies satisfiability of the supported
transformed theory in an expanded signature. Reused witnesses, omitted scopes,
positive-goal premises and uncertified ancestry cannot yield a theorem.
Local c502 acceptance does not close this phase or certify SWV406+1.

## F5 — AVATAR and SAT evidence

**Owner area:** formal case-split/SAT kernel + producer annotations.
**Paths:** `mrs-search/src/avatar*`, `mrs-proof-kernel/src/avatar*`,
`mrs-cadical`, new formal conditional proof rules.

- [ ] Specify variable-disjoint components, fresh globally consistent Boolean
  names, inherited branch contexts and a discharged unconditional false root.
- [ ] Prove the abstraction/branch-blocker/roll-up connection to first-order
  models; SAT unsatisfiability alone is not a first-order certificate.
- [ ] Verify bounded propositional RUP/LRAT replay with original-instance
  binding, deletion semantics and explicit unsupported RAT/incremental modes.
- [ ] Integrate the conditional proof DAG with full ordinary inference replay
  and conjecture dependence. Structural validation alone cannot certify.
- [ ] Provide a trace-required assurance policy to remove the trace-free
  CaDiCaL-unsat assumption; keep solver search untrusted when proof replay suffices.

**Gate:** the six stored assumption/AVATAR attacks and independent valid controls
traverse the formal integrated path. Add mutations for invented originals,
shared variables, reused unequal components, residual branch assumptions and
missing trace dependencies. Prove soundness; signed/unit abstraction and full
dynamic AVATAR completeness are separate coverage projects.

## F6 — Finite-model and ground/EPR assurance

**Owner area:** formal model evaluator / bounded grounding.
**Paths:** `mrs-proof-kernel/src/model.rs`, `mrs-core/src/model.rs`,
`mrs-search/src/{certified*,epr_ground*}`.

- [ ] Specify dense interpretation tables, nonempty domain, row-major indexing,
  full signature coverage, total functions/predicates and equality identity.
- [ ] Prove bounded term/formula evaluation, universal enumeration, original
  include/role coverage and falsification of the positive conjecture.
- [ ] Prove finite-domain grounding and supported unit-equality normalization
  soundness/completeness. Subset grounding can refute, never certify a full model.
- [ ] Either formally prove the supported ground reference-closure completeness
  path or require a complete independently checked model for model-status credit.
  Closure agreement and a `CompletenessWitness` enum are not that theorem.
- [ ] Formalize supported arithmetic with its separate infinite sorts before
  mixing it with finite tables. Do not reinterpret integers/rationals as finite
  uninterpreted elements.

**Gate:** accepted model implies the exact original supported problem has a
model/countermodel. Missing tables/formulas, aliased constants, invalid equality,
wrong conjecture polarity and exhausted evaluation cannot certify.

## F7 — Calculus and search completeness (independent target)

**Owner area:** calculus/index/search semantics.
**Paths:** `mrs-calculus`, `mrs-index`, `mrs-prephase`, `mrs-search`.

- [ ] Specify the static ordered calculus separately from inference soundness:
  KBO/LPO admissibility, maximality/selection and complete eligible inferences.
- [ ] Prove indexing recall and exact update/removal invariants; false positives
  are permitted only when subsequent checks validate the inference.
- [ ] Prove redundancy/deletion contracts for PLE/BCE, subsumption, rewriting,
  condensation and the proposed broader orphan/sharing policies.
- [ ] Define a fair, unpruned diagnostic search and prove dynamic saturation /
  refutational completeness with immutable proof/history invariants.
- [ ] State precisely how AVATAR reactivation and sharing preserve those
  invariants. Resource-pruned, weight-only, SInE/ML-filtered competition schedules
  remain heuristic and cannot inherit an unrestricted completeness theorem.

**Gate:** a theorem for a clearly declared abstract/unpruned procedure, followed
by an executable refinement if claimed. First-order undecidability precludes
promising a terminating complete decision procedure for all inputs. Valid
certificates remain usable from incomplete searches.

## F8 — Integration, budgets, TCB and independent evidence

**Owner area:** formal tooling/runtime/CI. Applies throughout.

- [ ] Maintain a TCB ledger: Verus/vstd/SMT, Rust/LLVM, build metadata,
  filesystem/process/OS assumptions and the remaining trusted transformations.
- [ ] Record complete checker/source/manifest/input/proof identities with every
  positive artifact. A sidecar is evidence, not self-authenticating attestation.
- [ ] Keep the cumulative formal work meter, decode bounds and exact cutoff /
  reset mutations. Add metered obligations for each new rule; never replenish
  fuel at a new stage.
- [ ] Extend hard deadlines to blocking host I/O, model checking, parsing,
  elaboration and shutdown; budget transient AST/proof/bridge memory. Do not call
  work units a verified instruction, heap or wall-clock bound.
- [ ] Obtain and retain independent-runner comparison results before marking
  reproducibility established; distinguish same-image, cross-distribution and
  verifier/compiler bootstrap claims.
- [ ] Grow family-diverse archived positive proofs and independent semantic
  review, not just synthetic examples. Re-audit exactly retained bytes after
  rule/assumption/bridge changes; preserve old verdicts and explain every delta.
- [ ] Add an explicit opt-in end-to-end assurance policy only after F1–F3 and
  the required F4/F5/F6 slices pass. Do not replace the ordinary strict policy
  or relax Unknown into a positive result for coverage.

**Gate for every commit:** all four Nix-wrapped Cargo checks, relevant optional
features, full pinned Verus verification/runtime/bridge/mutation gate when formal
sources/manifests/fixtures change, source/identity audit, independent negative
controls, and documentation matching the actual completed scope. Keep builds
isolated; do not push. Performance/competition promotion needs its separate
paired held-out eight-core campaign gate.

## First delivered slice and hand-off

This change begins F1 after the F0 audit: the `u64`/finite-set carrier restriction
is replaced in the main semantic core, and an infinite model regression is
added. It does **not** implement arbitrary-carrier transport, a verified TPTP
parser, FOF/AVATAR/model semantics, permutation dispatch or search completeness.
Next logical work is the remaining F1 transport proof, then a bounded F2/F3
vertical slice. Full evidence and unresolved boundaries belong in the dated
[audit](../reports/audits/formal-guarantees-2026-11-21.md), not in a checked box
claiming the entire roadmap is finished.
