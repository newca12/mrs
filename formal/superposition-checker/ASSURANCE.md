# Superposition-checker assurance note

## Integration boundary (2026-10-20)

The bounded wire-v1 checker and CNF-problem/restricted-FOF-clause proof adapter are implemented; see
[`INTERCHANGE.md`](INTERCHANGE.md). `mrs-proof-kernel::verus` and
`mrs-proover::strict` call the compiled artifact in explicit shadow or opt-in
dual-check mode. The latter certifies only when both independent paths accept.
Crashes, timeouts, unsupported syntax, malformed/version-mismatched responses,
hash mismatches and disagreements suppress certification in that gate.

The mathematical theorem still covers **decoded IR**, not the TPTP bytes.
Parsing, syntax lowering, symbol assignment, equality orientation, ordered
alpha conversion, certificate discovery and subprocess orchestration are
unverified. Wire-v1 structural decoding and the tokenizer's limits, checked
arithmetic, and exact decimal-value/byte-span correspondence are verified by
`wire_*` with quantified postcondition `wire_tokens_span_correspondence`;
bounded byte acquisition and response I/O are still host shims. Reports explicitly retain
`production_certification=false` for
formal evidence; the dual gate relies on both this trusted bridge and the
independent strict kernel. No default certification path has been replaced.
The verified DAG contract requires an empty root and checks the declared parent
arity for each supported rule. Reflexivity, excluded-middle, and tautology
introductions may have zero parents; the adapter preserves that shape rather
than inventing dependencies. Legacy one-parent annotations remain accepted for
these rules, but their optional parent is not used by the inference.

The actual archived c502 fixture is now parsed, translated and accepted as a
local step through the subprocess boundary. Complete supported CNF DAGs,
including an essential superposition plus exact-atom-resolution refutation,
are checked against the actual artifact built and verified in formal CI.
The indexed ten-entry corpus also exercises full parsed TPTP refutations
through the strict/Verus dual-check path with exact node-count checks. It has
four hand-authored smoke proofs, a seven-node synthetic MRS 0.3.0
function-bearing Horn-resolution chain, and three MRS 0.3.0 proofs generated
from real TPTP v10.0.0 CNF problems: a 28-node `PUZ001-1` Dreadbury Mansion
proof using instantiation/resolution, a five-node `SYN343-1` proof using
condensation/subsumption resolution, and a 29-node `PUZ001-2` equality proof
using superposition, equality resolution, demodulation and subsumption
resolution. The `PUZ001-2` proof expands a symmetric source equality into an
explicit symmetry node before superposition. One `PUZ001-1` conclusion has its
literal order normalized to the exact ordered-resolvent contract. The corpus
also includes archived Prover9 2026-6A proofs for `SYN343-1` and `PUZ001-1`
from the public TPTP solution archive. Their v9.3.1 proofs are checked against
the vendored v10.0.0 problems after strict source/formula validation and bounded
adapter normalization of FOF universal clauses and resolution literal order.
All vendored problems and proof fixtures match their
SHA-256 and node-count records in `supported-rules.json`. For
each case, formal CI mutates the first cited parent to a different existing
input, replaces its first inference with an unsupported rule, adds an
otherwise-valid but disconnected input node, and forges a nonempty root; none
may produce a certified result. The corpus is not a representative competition
archive; these two external Prover9 solutions do not establish campaign
provenance or representative archived coverage.
Exact-atom resolution has a separate semantic theorem and executable checker;
it removes one occurrence per parent with identical complementary atoms and
does not implement pivot unification. The three-clause c502 fixture is not the
complete MRS refutation. A supplied MRS 0.2.3 stdout transcript is recorded by
SHA-256, with its nine-node c502 ancestor closure retained as
`fixtures/SWV406+1-c502-proof-context.p`. The bridge runs the exact archived
c27/c39/c502 step through `check_tptp_step` and rejects a forged conclusion.
The transcript's reported local benchmark path does not independently establish
official competition provenance.
This does not formally check the FOF NNF/skolemisation/CNF ancestry of c27 and
c39 or certify the complete refutation; the full transcript includes AVATAR/SAT
rules outside the verified whitelist.
Separately, `fixtures/` preserves a complete public Leo-III THF TSTP solution for
SWV406+1; the adapter parses it and returns `Unknown` at the unsupported-type
guard before lowering. That is a negative fail-closed regression, not
supported-proof coverage or certification.
Loaded-job bridge tests also resolve selected and nested includes through the
ProoVer loader, then require the same formal DAG gate to accept; introducing an
unselected premise into the resolved problem is rejected by the adapter. Include
path resolution and selection filtering remain trusted loader behavior.

Build evidence records source commit/dirty state/hash, artifact SHA-256, Verus
binary SHA-256/version, effective Rust toolchain, support manifest and exact
strict verification flags. The sidecar also records the exact support-manifest
source hash. The build rejects a Verus release or effective toolchain that
differs from the checked-in pin. Artifact execution uses a private read-only byte
snapshot of the hash-checked caller binary, reducing check/launch replacement
risk. The bridge requires the adjacent bounded sidecar to name that artifact
hash and checks the manifest copy/hash before execution. Reports from an invoked
runtime include the full supported-rules manifest and both manifest and
build-evidence hashes; an adapter preflight `Unknown` may precede any runtime
report. The sidecar is trusted build metadata, not part of the Verus theorem or
evidence that TPTP lowering is verified. On Unix, the child has a cleared
environment, `/` cwd, a private process group, 60 s CPU, 768 MiB address-space,
1 MiB file-size and zero core-dump limits; Linux also sets `NO_NEW_PRIVS`.
Startup or rlimit failures cannot certify. Same-host double compilation is
checked. A two-independent-runner same-image comparison is configured, but its
result is pending; cross-distribution reproducibility and verified TPTP lowering
remain open.

The mutation inventory is now fifty-eight verifier-negative checks and forty-nine
compiled-runtime checks plus one valid assumption probe (accepted without strict mode,
rejected with `--no-cheating`). Prior claims that the redundant target-freshening
guard mutation failed were incorrect; the actual forbidden-image check is now
mutated and rejected by Verus. The detailed historical notes below are superseded
where they describe no serialized interchange or MRS caller.
The additional runtime mutation weakens decimal-digit validation and maps a
non-ASCII byte to a digit; its malformed-request response regression must fail.

## Claim currently supported

The pinned `verify.sh` run verifies the listed Verus specifications, proofs, and
executable refinements. `first_order_core.rs` includes a universally closed
semantic superposition theorem for standardized parents and a proposed common
unifier. Successful `check_superposition_and_replay` now establishes the semantic
premises and proves universal validity preservation for its exact conclusion.
`check_dag_superposition_step` checks the certificate against its cited parent
nodes and conclusion and proves local entailment from universally valid parents.
The ground-unit checker and fixed two-literal schema are narrower verified
artifacts. `certify_dag` connects executable per-node validation to topological
induction. Its `Certified` result proves `dag_inputs_unsatisfiable`: no
well-formed interpretation satisfies every supplied input clause under universal
closure. Exact leaf membership is checked, not assumed; the API takes no model
or semantic-validity arguments. The legacy structural entry point still returns
`Unknown` for valid graphs. Before node validation, `certify_dag` also checks
that every supplied node is in the root's ancestor closure. The reverse
topological traversal shares a bounded work counter across initialization,
parent processing and coverage validation; exhaustion cannot certify.
`certify_dag` now threads one mutable counter through structural preflight,
root-ancestor closure and every node validation; no phase or node replenishes the
caller’s ceiling. Supported inference branches pass that same counter through
parent/structure charges and local replay. A multi-node exact-cutoff regression
and compiled resets at the structural, closure and node-validation handoffs test
the composition. The source audit confirms that no `_with_work` helper resets the
caller-owned meter and that the certified path uses those shared-counter variants.
`dag_exact_cumulative_replay_work` is true in checker-defined work units across
structural preflight, closure, node replay and nested helpers. These units are not
an instruction-level or cycle-accurate cost model.
Input-leaf validation charges both clause structures for every candidate in its
repeated exact-membership scan, with a runtime regression for the exact cutoff;
structural preflight charges both operands on every exact
certificate-clause comparison against a cited parent or the node conclusion for
superposition, first-order resolution, equality resolution, factoring, subsumption
resolution, weakening, demodulation, instantiation and condensation. A fixed
superposition DAG accepts at the exact 128-unit structural cutoff and rejects at
127; a compiled mutation that bypasses the source-parent comparison charge must
fail that regression. The runtime suite also exercises standardized-parent collision
rejection, malformed/deep target paths, mismatched pre/post-freshening paths,
out-of-range atom slots and forged conclusions. A superposition certificate
with a target freshening that collides with a source-fresh variable is rejected,
alongside a disjoint positive control. A compiled-runtime mutation removes
source-fresh images from both target avoid sets; the mutant still passes
`--no-cheating` verification but must fail at the collision-specific runtime
assertion. This checks the standardized-apart representation policy, not an
extra premise of the semantic soundness theorem.
The standalone superposition check carries one counter from outer structure
charges through certificate preflight and replay: clause-variable collection,
coverage/membership scans, domain uniqueness, binding lookups/copies, avoid scans,
injectivity, freshened-clause materialization, unifier application/comparison,
path equality, selected-subterm lookup, both-parent substitution, target
replacement, resolvent construction and exact output comparison.
Exact 4/3 freshening/path, sequential 7-unit, 5/4 selected-term, 21/20-unit,
11/10-unit, 12/11-unit and 23/22-unit regressions exercise these boundaries;
compiled mutations remove lookup, clause-literal, materialization,
unifier-comparison, path-comparison, selected-term, replay substitution,
replacement, build, comparison, preflight/replay handoff, DAG-superposition handoff,
first-order-resolution DAG handoff, equality-resolution deletion/DAG,
clause-factoring DAG, equality-factoring pivot/DAG, subsumption literal/DAG,
weakening DAG, instantiation/condensation DAG, demodulation DAG, nullary/exact-
resolution DAG handoffs, reflexivity comparison/DAG charges, transitivity
comparison/DAG charges, excluded-middle atom/DAG charges, congruence comparison/DAG
charges, tautology atom/DAG charges, and resets at the three outer DAG handoffs.
A computed boundary
funds exactly the standalone structure/preflight work and requires replay to fail
closed at its first charge. The DAG per-node validator carries parent-index,
node-clause and cited-parent comparison work into local replay meters for
superposition, nullary/exact and first-order resolution, equality resolution,
clause factoring, equality factoring, subsumption resolution, weakening,
instantiation, condensation, demodulation, reflexivity, transitivity, symmetry,
excluded middle, congruence and tautology.
The global meter now covers the outer DAG phases, supported rule paths, and
repeated exact certificate-to-parent/conclusion comparisons in structural
preflight. The 8/7 transitivity fallback scan regression and compiled mutant
complete the last identified conditional-scan gap; supported local cutoffs and
the whole-DAG composed cutoff exercise the accounting boundaries.
First-order binary-resolution replay also has a shared local work counter for
collection, freshening, unifier application, pivot comparison, resolvent
construction and conclusion comparison; its pivot atom meter has a 4/3 cutoff
mutation test. The first-order resolution DAG wrapper carries its per-node work
into this meter; an exact composed node cutoff and reset mutant guard the handoff.
Equality-resolution replay also shares its local meter across unification,
substitution, pivot deletion and conclusion comparison; a 4/3 deletion cutoff
and compiled mutation test the work charge. Its DAG node path has the exact
composed-cutoff and counter-reset mutation checks.
Clause factoring replay shares work across domain validation, substitution,
pivot comparison, deletion and exact conclusion comparison; the DAG node path
has a composed-cutoff/reset-mutant test as well.
Equality factoring replay shares a meter across unifier application and left-pivot
comparison, substitution of the clause, resolvent construction and exact comparison.
The 5/4 pivot cutoff and its compiled charge-removal mutant exercise that meter; the
DAG node path also has an exact composed cutoff and reset mutation.
Subsumption-resolution replay shares work across collection, freshening, unifier
application, pivot and mapped-literal comparisons, deletion and exact comparison.
Its literal comparator has a 3/2 cutoff mutation test, and the DAG node path carries
parent comparisons into replay under an exact composed cutoff/reset mutant.
Weakening replay meters its inclusion-literal matching; the DAG wrapper's shared
counter is checked by an exact composed cutoff and reset mutation.
Instantiation and condensation also share work across local substitution/inclusion
checks, cited-parent comparisons and replay. Their per-node exact cutoffs and
counter-reset mutants check each DAG handoff.
Demodulation shares work across path comparison, target/equality collection and
freshening, unifier application, selected-term matching, replacement and conclusion
comparison; its DAG wrapper has an exact composed cutoff and reset mutation.
Nullary and exact-atom resolution DAG checks share node/parent structure work with
resolvent construction and conclusion comparison; both handoffs have exact composed
cutoffs and counter-reset mutations.
Reflexivity meters its term-equality comparison and carries the node counter through
its DAG wrapper; a 5/4 local cutoff and compiled reset mutant test the boundary.
Transitivity shares its meter across clause matching, candidate orientation checks
and equality-term comparisons. Its 12/11 local cutoff and exact composed DAG cutoff
are protected by comparison-charge and counter-reset mutations.
The unchanged-clause transitivity fallback now charges each candidate-scan iteration;
its 8/7 exact cutoff and compiled charge-removal mutation guard the otherwise skipped
non-equality-literal scan.
Symmetry shares work across candidate equality orientation and unchanged-literal
matching. Its 3/2 local comparison cutoff and independently computed DAG cutoff are
guarded by charge-removal and counter-reset mutations.
Excluded middle shares work across polarity and atom checks, including the DAG node
wrapper. A 3/2 local cutoff and exact composed cutoff/reset mutation cover both charges.
Congruence shares work across recursive orientation checks, unchanged binary siblings,
and the clause/DAG wrappers. The 5/4 local cutoff and exact composed node cutoff have
compiled charge-removal and counter-reset mutations. Verifier-negative mutations that
remove the unchanged-sibling or nested unary function-symbol check are rejected too.
Tautology meters reflexive-equality and complementary-literal scans, also in structural
preflight. Its 4/3 local comparison cutoff and exact DAG cutoff have charge-removal and
counter-reset mutations.
Exhaustion of the shared global counter in any phase or node fails closed; no
phase or node receives a replenished budget share.

This certifies the supplied wire-decoded IR input clauses, not a parsed TPTP
problem. Wire-v1 structural decoding and decimal tokenization are verified; the
TPTP/MRS lowering remains trusted adapter code. The bridge and runtime fixtures
exercise the interchange boundary. The DAG dispatch includes first-order
resolution, equality resolution, clause and equality factoring, subsumption
resolution, weakening, demodulation, clause instantiation, clause condensation,
equality reflexivity, equality transitivity, equality symmetry, clause excluded
middle, equality congruence, and clause tautology in addition to superposition
and exact-atom resolution. Each accepted replay is bound to its declared parent
edges (or has no edges for a zero-parent tautology introduction) and its
conclusion, and proves local universal-validity preservation.
Exact and standardized first-order resolution accept equality pivots in either
ordered or swapped orientation; executable regressions cover both forms.
Compiled refutations certify from nonempty inputs with the audited cumulative
work-unit counter. Broader inference rules remain open.

The compiled runtime regression also exercises an exact `SWV406+1:c502` local
superposition certificate. It standardizes the c27 and c39 parents apart, binds
variables from the target parent, substitutes across the complete parents,
rewrites the nested selected occurrence and checks c502's exact ordered
conclusion; a forged conclusion is rejected. This local IR result does not
parse the archived TPTP fixture or certify the complete archived TSTP proof.
The runtime suite also accepts an independently constructed step in which the
unifier binds a freshened source variable to a ground target subterm.

The machine-readable support declaration is
[`supported-rules.json`](supported-rules.json). Unsupported rules and
unimplemented paths must not be treated as certified.
The intended rule premises and substitution convention are documented in
[`RULE_CONTRACT.md`](RULE_CONTRACT.md). Complete
hygiene/resource refinement, and production integration remain open; no
production-certification claim is made.

## Trusted computing base

The proof claim depends on:

- the pinned Verus binary and its verification-condition generation;
- the SMT solver bundled/used by Verus;
- the Rust toolchain pinned by the Verus repository's `rust-toolchain.toml`;
- Rust/LLVM compilation of the checked Verus source;
- the shell/Nix/CI commands selecting source files, tools, and flags; and
- any future decoder, serialization adapter, or MRS integration boundary.

Verus does not prove its own verifier or the compiler. Until the input adapter is
verified or losslessly checked by the verified side, it remains trusted. A
successful verifier run is evidence about the checked source under this TCB; it
does not by itself certify arbitrary TSTP proofs or the MRS strict-kernel code.
The formal verification command enables Verus `--no-cheating` for every proof
module, so verifier-recognized escape hatches (including
`assume_specification`) fail verification in addition to the separate lexical
source audit. That audit recursively covers every `.rs` file under the checker
directory and requires a matching strict invocation in `verify.sh`; a unit
regression rejects a nested source omitted from the verification list. It does not
audit Rust modules outside this formal directory.

The Verus executable is pinned to release `0.2026.10.18.7d82994` with a
verified Linux x86_64 SHA-256. The release's `rust-toolchain.toml` declares
Rust `1.99.1`, matching the requested toolchain. `verify.sh` records both
requested and effective values. Updating either pin requires a fresh full
verification run and recorded evidence.
The first-order core, reproducible runtime builds and compiled-runtime mutants use
the explicit Verus SMT `--rlimit 30`; the default limit of 10 reproducibly exhausted
resources in the structural-preflight loop. `verify.sh` detects the available
CPU count and uses it for Verus verification threads by default
(`VERUS_THREADS` overrides this); runtime compilation uses the same setting.
Verifier-negative mutations keep the default SMT limit and run as one-thread
Verus processes in a pool sized to the detected CPU count by default
(`VERUS_MUTATION_WORKERS` overrides the pool size). These settings change solver
resource use and scheduling, not the verified specification.

## Verification evidence

`verify.sh` can write a CI artifact with `VERUS_LOG=/path/to/log`. The log
records the source commit, Verus binary SHA-256, requested and effective Rust
toolchains, verifier version/output, source-audit result, and per-module verification counts. CI
uploads that log even when a later gate fails. Local verification does not
retain a build artifact unless `VERUS_LOG` is set.
`mrs-tools verus-runtime-check` also compiles the core twice and compares executable SHA-256
values before running it; this currently checks byte reproducibility on one
host and toolchain only. CI additionally builds the runtime and evidence sidecar
on two independent GitHub-hosted Ubuntu 24.04 runners and requires byte equality
for both artifacts. This checks independent-host reproducibility for the same
pinned OS/toolchain, not cross-distribution reproducibility; it remains
unestablished until that CI comparison passes.

The verification gate enforces 58 verifier-negative mutations across inference
replay, DAG dispatch and wire tokenization. Each mutated temporary source must
fail Verus verification. This samples key executable/model refinement
contracts; it is not comprehensive threat-model coverage. Forty-nine
compiled-runtime mutations cover root-ancestor closure, input and node identity,
target-parent freshening, freshening and replay work charges, clause/term
comparison, and shared work across local inference replay and DAG nodes. Each
must fail its corresponding runtime regression. The gate also compiles and runs positive and negative DAG
examples via `mrs-tools verus-runtime-check`; the ordinary Rust `main` assertion
is a test oracle outside the verified certification API.
The mutation suite injects `#[verifier::assume_specification]` into a temporary
source copy and requires `--no-cheating` to reject it.
Resolution-step fixtures additionally cover complementary pivots, wrong symbols,
forged conclusions, out-of-range pivots and work-limit exhaustion.
Resolution DAG fixtures cover unit and context-bearing refutations, omitted
contexts, duplicate pivot occurrences, wrong parents/orientation and self-parent
references. New mutations reject omitted oriented polarity, skipped DAG rule
replay and checking a parent instead of the claimed conclusion.
An additional runtime regression rejects a disconnected but valid input node
and exercises both zero-budget and exact-threshold closure exhaustion/success.
Input-leaf comparison regression exercises three candidates, accepts at the
exact 15-unit node budget and rejects at 14. A compiled-runtime mutation that
removes the repeated node-clause charge must fail this regression. The target
freshening mutation omits source-parent fresh images from both target avoid sets;
its compiled program must fail specifically at the parent-variable collision
assertion. Freshening-budget mutants remove the binding-lookup, collection-literal
or materialization charge and must fail their exact local meter regressions. The mutation runner enforces the
expected inventory counts so an accidental removal of either verifier-negative
or compiled-runtime coverage fails the gate.

## Re-audit triggers

Changes to semantic definitions, the superposition theorem, verified replay,
toolchain pins, verifier invocation, support manifest, or any trusted input
bridge require review and re-verification. Historical certification counts
must not be attributed to this checker until a production integration gate is
passed.
