# Superposition-checker assurance note

## Integration boundary (2026-10-20)

The bounded wire-v1 checker and CNF TPTP adapter are implemented; see
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
The verified DAG contract requires an empty root and exactly two parent edges
for each supported inference.

The actual archived c502 fixture is now parsed, translated and accepted as a
local step through the subprocess boundary. Complete supported CNF DAGs,
including an essential superposition plus exact-atom-resolution refutation,
are checked against the actual artifact built and verified in formal CI.
Exact-atom resolution has a separate semantic theorem and executable checker;
it removes one occurrence per parent with identical complementary atoms and
does not implement pivot unification. The complete archived SWV406+1 proof is
not present in the three-clause fixture and is not certified by these tests.
Loaded-job bridge tests also resolve selected and nested includes through the
ProoVer loader, then require the same formal DAG gate to accept; introducing an
unselected premise into the resolved problem is rejected by the adapter. Include
path resolution and selection filtering remain trusted loader behavior.

Build evidence records source commit/dirty state/hash, artifact SHA-256, toolchain,
support manifest and strict verification flags. Artifact execution uses a private
read-only byte snapshot of the hash-checked caller binary, reducing check/launch
replacement risk. On Unix, the child has a cleared environment, `/` cwd, a private
process group, 60 s CPU, 768 MiB address-space, 1 MiB file-size and zero core-dump
limits; Linux also sets `NO_NEW_PRIVS`. Startup or rlimit failures cannot certify.
Per-call reports record artifact
and request hashes, protocol version and node count. Same-host double compilation
is checked; cross-host reproducibility and a verified decoder remain open.

The mutation inventory is now thirty-six verifier-negative checks and four
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
The caller's certification budget is partitioned between structural preflight,
ancestor-closure validation and per-node replay, so the number of nodes cannot
multiply one local replay allowance without bound. Some replay helpers still
use distinct local counters; this is not yet exact cumulative instruction/work
accounting. Within each node, parent-edge validation and clause structure are
charged before the remaining node share is passed into the local rule checker.
Superposition and resolution DAG wrappers also charge each cited parent and the
conclusion before passing the remainder to local replay. Nested helper
traversals still have separate local budgets, and exact literal-comparison
operation accounting is not established.
Input-leaf validation charges both clause structures for every candidate in its
repeated exact-membership scan, with a runtime regression for the exact cutoff;
this closes that specific repeated-comparison undercount, not all comparison
accounting. The runtime suite also exercises standardized-parent collision
rejection, malformed/deep target paths, mismatched pre/post-freshening paths,
out-of-range atom slots and forged conclusions. A compiled mutation removing the
target-parent freshening guard must fail the collision regression.
Budget division that would leave a node with a zero share returns inconclusive
before any node can be certified.

This certifies the supplied IR input clauses, not a parsed TPTP problem.
The current IR is constructed directly as Verus data types; it has no byte-level
serialization or verified TPTP/MRS lowering. Consequently, the local c502 and
DAG runtime fixtures do not exercise an interchange boundary.
Nullary-predicate resolution is DAG-dispatched in addition to superposition.
Accepted exact resolution replay proves universal validity preservation;
`check_dag_resolution_step` consumes that postcondition for the actual cited
parents and node conclusion. The generic induction consumes local entailment
for either rule. Compiled unit and multi-step refutations certify from nonempty
inputs. General first-order resolution and other inference rules remain open.

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
source audit.

The Verus executable is pinned to release `0.2026.10.18.7d82994` with a
verified Linux x86_64 SHA-256. The release's `rust-toolchain.toml` declares
Rust `1.99.1`, matching the requested toolchain. `verify.sh` records both
requested and effective values. Updating either pin requires a fresh full
verification run and recorded evidence.

## Verification evidence

`verify.sh` can write a CI artifact with `VERUS_LOG=/path/to/log`. The log
records the source commit, Verus binary SHA-256, requested and effective Rust
toolchains, verifier version/output, source-audit result, and per-module verification counts. CI
uploads that log even when a later gate fails. Local verification does not
retain a build artifact unless `VERUS_LOG` is set.
`runtime-check.py` also compiles the core twice and compares executable SHA-256
values before running it; this currently checks byte reproducibility on one
host and toolchain only.

The verification gate also runs sixteen verifier-negative checks: omitting
substitution from executable literal replay, ignoring literal polarity,
replacing selected-position lookup with root lookup, and omitting the rewritten
target clause from resolvent construction, skipping DAG-step replay, ignoring
the cited source parent, and skipping local replay in DAG-node validation. Each mutated temporary source must
fail Verus verification. This samples key executable/model refinement
contracts; it is not comprehensive threat-model coverage.
The four Verus-negative certification mutations skip node validation, forge
input membership, allow forward parents and accept a nonempty root. Compiled-
runtime mutations skip root-ancestor closure, omit the repeated node-clause
charge during input membership checks, and remove target-parent freshening.
The gate also compiles
and runs positive/negative DAG examples via `runtime-check.py`; the ordinary Rust `main`
assertion is a test oracle outside the verified certification API.
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
removes the repeated node-clause charge must fail this regression. A separate
compiled mutation removes the target-parent freshening guard and must fail the
standardized-variable-collision regression.

## Re-audit triggers

Changes to semantic definitions, the superposition theorem, verified replay,
toolchain pins, verifier invocation, support manifest, or any trusted input
bridge require review and re-verification. Historical certification counts
must not be attributed to this checker until a production integration gate is
passed.
