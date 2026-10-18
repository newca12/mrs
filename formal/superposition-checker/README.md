# Verus superposition checker

This is MRS's Verus superposition-checker project. It contains the original
target-rigidity policy pilot, first-order substitution model, and an initial
checker for one ground-unit fragment. The pilot tests a chosen policy; it does
not establish that target rigidity is a soundness requirement.

## Verify

In this repository's NixOS/WSL development environment:

```sh
nix develop .#verus -c formal/superposition-checker/install-verus.sh  # once per pinned version
nix develop .#verus -c formal/superposition-checker/verify.sh
```

On ordinary Linux x86_64 hosts with `rustup`, `curl`, `unzip`, `python3`, and
`sha256sum`, run the scripts directly (CI uses this path):

```sh
formal/superposition-checker/install-verus.sh  # once per pinned version
formal/superposition-checker/verify.sh
```

The installer checksum-verifies the pinned Linux x86_64 release, installs its
required Rust toolchain selected by `verus-toolchain.json`, and stores Verus under
`$HOME/.local/opt/mrs-verus/`. Override the binary location with
`VERUS_INSTALL_DIR=/path/to/install`. The dedicated Nix shell isolates its
`RUSTUP_HOME` and `CARGO_HOME` under the user cache; override them if needed.
Set `VERUS=/path/to/verus` to verify with an already installed binary.

The verification script also runs `audit-verus-source.py`; it fails closed if
formal Rust sources contain `assume`, `admit`, `external_body`, or verifier
allow attributes. See [`ASSURANCE.md`](ASSURANCE.md) and
[`supported-rules.json`](supported-rules.json) for the current trust boundary
and machine-readable supported-slice declaration.

It also runs `mutation-check.py`: temporary copies of the first-order core omit
literal substitution, ignore literal polarity, replace selected-position
lookup with root lookup, omit appending the rewritten target clause, skip DAG-step
replay, ignore its cited source parent, or skip replay during DAG preflight. The
verification gate requires Verus to reject all seventeen broken refinements,
including skipped node validation, forged input membership, forward parents,
and nonempty-root acceptance. The
source tree is not modified by these negative checks.
`runtime-check.py` compiles the verified core and runs DAG regressions in a
temporary directory.

The checks use Verus `0.2026.10.18.7d82994` (the latest published release).
This release declares Rust `1.99.1` in
[`verus/rust-toolchain.toml`](https://github.com/verus-lang/verus/blob/release/0.2026.10.18.7d82994/rust-toolchain.toml),
matching the requested toolchain. The verification log records both requested
and effective toolchains. Verus uses its bundled Z3. Results:
`5 verified, 0 errors` for the original pilot, `204 verified, 0 errors` for
the first-order core, `5 verified, 0 errors` for the ground superposition
checker, and `5 verified, 0 errors` for the variable two-literal schema.

## Scope and limitations

The current general replay prototype's intended rule is stated explicitly in
[`RULE_CONTRACT.md`](RULE_CONTRACT.md). It uses unordered superposition with
independent standardization-apart maps, a supplied simultaneous one-step
unifier, whole-parent substitution, and exact ordered conclusion comparison.
`check_superposition_and_replay` now proves that acceptance preserves universal
clause validity, through `accepted_replay_preserves_validity` and
`standardized_superposition_sound`. The bridge removes the selected literal at
arbitrary parent indices, preserves both substituted contexts, and transfers
their truth into the exact executable conclusion order, including duplicates.
`check_dag_superposition_step` binds the certificate to its actual two cited
parents and conclusion and proves `local_node_entails_from_parents`.

The local DAG contract consumes universally valid parents: truth at a single
valuation is insufficient for an inference that instantiates variables.
`certify_dag` returns `Certified` with the proved postcondition
`dag_inputs_unsatisfiable`: no well-formed interpretation satisfies all supplied
input clauses under universal closure. `validate_dag_node` checks exact input
membership, earlier parents and local replay, exposing `validated_dag_node`.
Before node validation, certification rejects nodes outside the empty root's
ancestor closure; the bounded closure walk returns `Unknown` on work exhaustion.
The caller's work ceiling is split across structural preflight, closure checking
and per-node replay. Replay helpers still use distinct local counters, so exact
cumulative operation accounting remains an explicit limitation.
`validated_dag_has_no_model` composes these facts with topological induction and
a total valuation into the nonempty carrier. No interpretation or assumed input
validity is supplied to the executable API.

This certifies the supplied IR input clauses, not a parsed TPTP problem.
Nullary-predicate resolution is now DAG-dispatched: `Resolution(lp, rp)` selects
one positive pivot in the left parent and one negative pivot in the right parent.
The checker removes exactly those occurrences, preserves all remaining literals
in left-context/right-context order, and compares the node conclusion exactly.
`check_dag_resolution_step` proves local entailment using the accepted replay's
universal validity-preservation postcondition. The generic induction supports
both inference rules. Compiled unit and multi-step refutations certify without
empty input clauses. General first-order resolution, production adapters and
cumulative replay-work accounting remain open.

`first_order_core.rs` adds recursive first-order terms and formal,
simultaneous-substitution semantics. It includes executable term copying,
lookup, substitution application, and duplicate-domain detection with
refinement postconditions, plus the general term-level substitution/evaluation
lemma under full-domain interpretation assumptions. Its mathematical lemmas
demonstrate an individual target-variable substitution instance and a
superposition-shaped entailment where unification instantiates a target
variable while consistently substituting the other target literal.
It also proves nullary-predicate resolution soundness and verifies the bounded
`check_nullary_resolution` exact-resolvent checker with positive/negative runtime
examples. Its semantic postcondition is composed into DAG certification through
`check_dag_resolution_step`.
`superposition_checker.rs` defines an
executable certificate checker and soundness theorem for a much narrower ground
fragment: positive unit equality rewrites the whole argument of a
positive unary-predicate unit clause. It uses opaque term IDs and a supplied
interpretation. It is not yet a general superposition checker: no
variables/MGUs, recursive term checker, nested positions, multi-literal clauses,
ordering, AC, proof-DAG replay, c502 audit, or integration with
`mrs-proof-kernel`. It also does not establish that Verus, its SMT solver,
Rust/LLVM, or any future input adapter is verified.

`variable_superposition.rs` now checks and proves one variable-bearing,
two-literal superposition schema (`f(a)=b`, `q(X) | p(f(X))` deriving
`q(a) | p(b)`). Its verifier rejects a forged partial target substitution that
leaves `q(X)` unchanged, and a forged rewrite conclusion; the model-level
soundness proof for the schema also verifies. This confirms whole-clause target
instantiation is sound in that constrained case; it is not a general unifier
or clause checker. The checks use a fixed schema certificate, not a parser/replay
implementation for arbitrary clauses.

The archived c502 clauses are kept in
[`fixtures/SWV406+1-c502-clauses.p`](fixtures/SWV406+1-c502-clauses.p). The
general executable checker now reconstructs the archived `c27`, `c39`, and
`c502` clauses with independent standardization-apart maps, the supplied
whole-parent unifier, the nested target path, and exact ordered conclusion.
`executable_c502_certificate_example` accepts that step and rejects a forged
conclusion. This is a local step verdict from the verified IR, not a full TSTP
translation or proof-DAG certification of `SWV406+1`; the regression manually
encodes the IR certificate rather than parsing the archived TPTP fixture.

Verus `0.2026.10.18.7d82994` is pinned in
[`verus-toolchain.json`](verus-toolchain.json); CI downloads and checksum-checks
the corresponding Linux binary. CI runs this as a regular required project
check. See [`DEVELOPMENT_PLAN.md`](DEVELOPMENT_PLAN.md)
for the staged scope. Remaining obligations include complete hygiene/resource
refinement, general first-order refutation rules, and production integration.
The supplied substitution checker establishes equality of instances, not MGU
computation.
In standard first-order semantics, target variables may be
instantiated as part of a general unifier if the substitution is consistently
applied to all relevant parent literals. Any stricter policy must be justified
as a rule restriction, not assumed to be necessary for soundness.

## Is target-variable rigidity required for soundness?

No—not for ordinary first-order superposition on universally closed clauses.
Standardize the two parents apart, unify the source equality side with the
selected target subterm, and apply the resulting substitution to the *entire*
resolvent: all non-selected literals from both parents, as well as the rewritten
target literal. A binding for a target-clause variable then selects an instance
of that universally quantified parent clause. Since a universally closed clause
entails each of its instances, this is sound. Rigidity is a restriction on
which otherwise-valid inferences the implementation attempts, not a general
semantic requirement.

For example, take the universally closed clauses

```text
f(a) = b
q(x) | p(f(x))
```

Superpose `f(a) = b` into `p(f(x))`. The unifier binds the target variable
`x := a`, and the whole-clause conclusion is

```text
q(a) | p(b)
```

This follows directly by instantiating the target clause at `a` and using
`f(a) = b`. The binding is therefore not itself unsound.

What is unsound is applying that substitution only to the selected target
literal while leaving other target literals unchanged. The same parents would
then yield `q(x) | p(b)`, which is not entailed. A countermodel has domain
`{0,1,2}`, `a=0`, `b=1`, `f(0)=1`, `f(2)=2`, `p(1)=false`, `p(2)=true`,
`q(0)=q(1)=true`, and `q(2)=false` (choose `f(1)` and remaining predicate
values arbitrarily to satisfy the target). Both parents hold, but the proposed
conclusion is false at `x=2`. This distinguishes inconsistent substitution
application—which is unsound—from target-variable instantiation itself.

This semantic observation does not certify every AC or archived inference in
MRS: the unifier must be valid for the declared theory, the substitution must
be applied consistently, and all other rule conditions must hold. The current
general replay theorem in `first_order_core.rs` now proves the ordinary
substitution/valuation argument; the status of archived c502 still requires
formal analysis. The production implementation may
retain rigidity as a conservative rule restriction, but its justification
should be completeness/coverage or implementation scope—not necessity for
soundness of ordinary superposition.
