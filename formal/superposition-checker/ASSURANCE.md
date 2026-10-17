# Superposition-checker assurance note

## Claim currently supported

The pinned `verify.sh` run verifies the listed Verus specifications, proofs, and
executable refinements. `first_order_core.rs` includes a universally closed
semantic superposition theorem for standardized parents and a proposed common
unifier, plus executable term/literal/clause replay prototypes. The executable
certificate checker is **not yet proved to establish every premise of that
theorem**, so this project currently makes no general production-certification
claim. The ground-unit checker and fixed two-literal schema are narrower
verified artifacts. `check_dag_structure` returns `Inconclusive` for every
in-budget graph and is not a proof checker.

The machine-readable support declaration is
[`supported-rules.json`](supported-rules.json). Unsupported rules and
unimplemented paths must not be treated as certified.

## Trusted computing base

The proof claim depends on:

- the pinned Verus binary and its verification-condition generation;
- the SMT solver bundled/used by Verus;
- Rust/LLVM compilation of the checked Verus source;
- the shell/Nix/CI commands selecting source files, tools, and flags; and
- any future decoder, serialization adapter, or MRS integration boundary.

Verus does not prove its own verifier or the compiler. Until the input adapter is
verified or losslessly checked by the verified side, it remains trusted. A
successful verifier run is evidence about the checked source under this TCB; it
does not by itself certify arbitrary TSTP proofs or the MRS strict-kernel code.

## Verification evidence

`verify.sh` can write a CI artifact with `VERUS_LOG=/path/to/log`. The log
records the source commit, Verus binary SHA-256, pinned Rust toolchain, verifier
version/output, source-audit result, and per-module verification counts. CI
uploads that log even when a later gate fails. Local verification does not
retain a build artifact unless `VERUS_LOG` is set.

## Re-audit triggers

Changes to semantic definitions, the superposition theorem, verified replay,
toolchain pins, verifier invocation, support manifest, or any trusted input
bridge require review and re-verification. Historical certification counts
must not be attributed to this checker until a production integration gate is
passed.
