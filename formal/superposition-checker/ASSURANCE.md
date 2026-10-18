# Superposition-checker assurance note

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
accounting.

This certifies the supplied IR input clauses, not a parsed TPTP problem.
Nullary-predicate resolution is DAG-dispatched in addition to superposition.
Accepted exact resolution replay proves universal validity preservation;
`check_dag_resolution_step` consumes that postcondition for the actual cited
parents and node conclusion. The generic induction consumes local entailment
for either rule. Compiled unit and multi-step refutations certify from nonempty
inputs. General first-order resolution and other inference rules remain open.

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

The verification gate also runs seventeen mutation-negative checks: omitting
substitution from executable literal replay, ignoring literal polarity,
replacing selected-position lookup with root lookup, and omitting the rewritten
target clause from resolvent construction, skipping DAG-step replay, ignoring
the cited source parent, and skipping local replay in the DAG preflight. Each mutated temporary source must
fail Verus verification. This samples key executable/model refinement
contracts; it is not comprehensive threat-model coverage.
The five certification mutations skip node validation, forge input membership,
allow forward parents, accept a nonempty root, and skip root-ancestor closure.
The gate compiles and runs
positive/negative DAG examples via `runtime-check.py`; the ordinary Rust `main`
assertion is a test oracle outside the verified certification API.
Resolution-step fixtures additionally cover complementary pivots, wrong symbols,
forged conclusions, out-of-range pivots and work-limit exhaustion.
Resolution DAG fixtures cover unit and context-bearing refutations, omitted
contexts, duplicate pivot occurrences, wrong parents/orientation and self-parent
references. New mutations reject omitted oriented polarity, skipped DAG rule
replay and checking a parent instead of the claimed conclusion.
An additional runtime regression rejects a disconnected but valid input node
and exercises zero-budget closure exhaustion.

## Re-audit triggers

Changes to semantic definitions, the superposition theorem, verified replay,
toolchain pins, verifier invocation, support manifest, or any trusted input
bridge require review and re-verification. Historical certification counts
must not be attributed to this checker until a production integration gate is
passed.
