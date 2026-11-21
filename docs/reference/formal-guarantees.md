# Actual formal guarantees

Reviewed 2026-11-21, starting from `bd04ae44`. This is the source-derived
assurance inventory, not a benchmark score. Read it together with the normative
[Verus assurance note](../../formal/superposition-checker/ASSURANCE.md),
[support manifest](../../formal/superposition-checker/supported-rules.json), and
[implementation roadmap](../research/formal-assurance-roadmap.md).

## Bottom line

**MRS is not an end-to-end formally verified theorem prover.** It has:

1. a proof-producing, heuristic Rust search engine;
2. a separate **ordinary Rust** proof/model kernel with runtime checks and
   adversarial regression evidence;
3. a Verus-verified **decoded-IR** inference/DAG checker, verified wire decoding
   and decimal tokenization, explicitly invoked through a **trusted TPTP bridge**;
4. standalone mathematical lemmas not all connected to that executable DAG path.

These are different assurance levels. A checked proof does not prove search
completeness; a Verus theorem about decoded clauses does not verify the input
parser; tests and accepted archives are not universal soundness proofs.

## Which result did the user actually request?

| Mode / result | What is checked | What it does **not** establish |
|---|---|---|
| Ordinary `mrs problem.p`, `Theorem` / `Unsatisfiable` | Search finds a candidate refutation and may emit a TSTP proof. | Strict acceptance is not required by default; no end-to-end formal guarantee. Proof output can also be omitted by output limits. |
| `mrs --self-check problem.p` | Refutation candidates pass the ordinary strict kernel before theorem output; failure, inconclusive checks and late candidates fail closed. | The kernel, parser, preparation and coordinator are not Verus-verified. This flag does not invoke the Verus artifact. |
| `mrs-proover --strict proof.p` | Full ordinary-kernel replay against a loaded problem/include closure. | Not a formal proof of that Rust implementation; not the broader competition-mode policy. |
| Ordinary `Satisfiable` / `CounterSatisfiable` | Separate bounded ground/EPR routes, with checked grounding/closure or finite-model evidence; heuristic saturation is demoted. | Not whole-engine completeness. Some ground-closure results carry no emitted model. |
| Model result under `--self-check` | A present finite certificate is independently evaluated against the original problem and conjecture polarity; unsupported/incomplete input cannot earn model credit. | A missing model is not silently replaced by a formal model theorem: supported ground-closure witnesses may still supply the claim. Neither route is Verus-verified. |
| `mrs-proover --verus-shadow PATH --verus-sha256 HASH proof.p` | Ordinary strict verdict plus separate formal IR evidence. | Formal Unknown does not suppress a successful strict verdict; `production_certification=false` remains explicit. |
| `mrs-proover --verus-gate PATH --verus-sha256 HASH proof.p` | Both strict replay and the supplied, hash-checked formal runtime must accept. | TPTP parsing/lowering, certificate discovery and normalization are still trusted. This is not direct formal certification of arbitrary original TPTP bytes. |
| Competition / ATP-assisted verifier | Broader step checks, external ATPs and the proof-wide assumption/AVATAR gate. | Not strict-kernel-only verification, not Verus certification, and not independent when a diagnostic invokes MRS search itself. |

Sources: `src/main.rs`, `src/coordinator.rs`,
`crates/mrs-search/src/strategy.rs`, `certified.rs`, `certified_sat.rs`,
`crates/mrs-proover/src/{strict,verify}.rs`, and
`crates/mrs-proof-kernel/src/{lib,model,verus}.rs`.

## 1. Machine-checked semantic results

### Main decoded-IR theorem

`formal/superposition-checker/first_order_core.rs` defines terms, atoms, literal
sequences, interpretations, valuations and simultaneous substitutions. Its
`certify_dag` postcondition is:

> `Certified` implies `dag_inputs_unsatisfiable`: no well-formed interpretation
> satisfies every supplied input clause under universal closure.

The executable API takes syntax/certificates/bounds, not a supplied model or a
Boolean asserting semantic validity. Proof induction uses the actual cited
parents. It checks exact input membership, an empty root, topological parents,
unique node identities and root-ancestor closure. Disconnected nodes cannot be
silently dropped to obtain a formal positive result. This is stricter than the
ordinary Rust kernel, which checks extra nodes but permits harmless unused ones.

A local `AcceptedStepIr` result is conditional validity preservation from the
supplied parent clauses. It does not establish that those parents follow from
a loaded original problem or that a full refutation exists. Only the whole-DAG
contract establishes unsatisfiability of the supplied decoded input set.

**Carrier qualification matters.** At the audit baseline, semantic domains were
`Set<u64>` and values were `u64`: only finite machine-bounded carriers were
represented. That theorem alone was not an unrestricted first-order no-model
theorem. The first repair changes the main semantics to **`ISet<int>`**, with
mathematical integer values and unchanged `u64` syntax identifiers. It admits
arbitrarily large finite and countably infinite domains. An infinite successor
model and values above `u64::MAX` are explicit semantic regression obligations.
The repair's verification state is recorded in the dated audit and assurance
note. **Arbitrary carrier types and a formal transport theorem to unrestricted
standard first-order semantics remain open.** No unproved model-transport or
Löwenheim–Skolem argument is being inserted into the advertised theorem.

Equality is identity on the semantic carrier. Function/predicate interpretations
are total; functions return carrier elements. Valuations are total and range
over the nonempty domain. Names/variable IDs are syntax, not domain elements.
The calculus uses a simultaneous **one-step** substitution; it does not require
an MGU, recursive substitution closure, KBO/LPO maximality, or target-variable
rigidity to justify individual steps.

Mathematical integer denotations are **model-value labels**, not an implementation
of TPTP integer arithmetic. The infinite successor example is a ghost semantic
witness, not an infinite-model certificate generator or a new search algorithm.

### Verified executable rule families

| Rule family | Formal contract / dispatch |
|---|---|
| Superposition | Independent checked parent freshening, a common substitution that equates the selected terms, whole-parent instantiation, checked target position and exact resolvent; `accepted_replay_preserves_validity`. |
| Nullary and exact-atom resolution | Complementary pivots, one occurrence deleted per parent, exact ordered residual contexts; equality pivots may be symmetric. |
| First-order binary resolution | Checked standardization apart and common pivot unifier; exact two-parent resolvent; `standardized_first_order_resolution_sound`. |
| Equality resolution | Negative equality becomes reflexive under the supplied substitution; exact deletion/instantiation. |
| Clause factoring / equality factoring | Checked polarity/equality premises, common substitution and exact retained/introduced literals. |
| Subsumption resolution | Checked standardized matching of active literals into the flipped target; exact target deletion, without arbitrary target instantiation. |
| Weakening | Explicit literal inclusion from the parent into the conclusion. |
| Demodulation | A cited positive unit equality, checked matching/substitution/path and exact replacement; chains are elaborated to binary nodes. |
| Instantiation / condensation | Simultaneous instance of the parent, with checked inclusion into the conclusion for condensation. |
| Reflexivity / excluded middle / tautology | Checked universally valid clauses; zero-parent introductions are supported, as are declared legacy one-parent annotations. |
| Transitivity / symmetry | Checked equality orientation/chaining and unchanged residual literals. |
| Congruence | A checked nonempty unary/binary single-hole function context, including unchanged siblings and symbols. |

The support manifest and [rule contract](../../formal/superposition-checker/RULE_CONTRACT.md)
are authoritative for exact premises. The currently dispatched syntax supports
variables/constants/unary/binary functions, predicates of arity 0–2 and equality.
It does not silently admit arbitrary arity, interpreted arithmetic, typed or
higher-order semantics, AC reasoning, FOF Skolemization or AVATAR.

### Verified but not connected to the DAG/wire path

- `same_clause_permutation_with_work` checks forward/inverse literal-index
  witnesses and preserves duplicate occurrences. Its semantic theorem proves
  validity preservation, but current DAG replay still requires exact order.
- Bijective variable-renaming validity and its composition with certified
  literal permutation are proved. Executable alpha-renaming discovery and the
  host normalizations are not thereby verified.
  The equality lemma assumes mutually inverse renamings on all variable IDs;
  an arbitrary host-discovered map on a clause's finite variables does not
  automatically satisfy that contract without extension/transport evidence.
- `superposition_checker.rs` and `variable_superposition.rs` are older narrow
  ground/schema pilots with machine-value semantics; they are not the main DAG
  theorem. `target_rigidity.rs` proves a binding-domain policy, not the necessity
  of target rigidity for superposition soundness.

## 2. Verified representation and resource properties

The **formal runtime**, not the host bridge, verifies wire-v1 tags, counts,
recursive decoded structures and command dispatch. Its decimal tokenizer proves
unsigned-value/byte-span correspondence, digit limits and checked accumulation.
Unknown tags, malformed/trailing fields and exhausted bounds cannot certify.

The caller-owned work counter is carried across structural preflight,
root-ancestor closure, input comparisons, node validation and nested supported
rule replay. It cannot be replenished by a later phase/node. Exact/one-short
cutoff and counter-reset/charge-removal mutations exercise these obligations.
These are **checker-defined work units**, not CPU instructions, wall-clock time,
heap allocation totals or a proof of OS resource containment. Decode bounds and
the replay meter are distinct contracts.

Source: `wire_*`, `check_wire_tokens`, `*_with_work` and `certify_dag_with_work`
in `first_order_core.rs`. Acquiring input bytes, encoding requests, process I/O,
runtime response formatting and watchdog cleanup remain ordinary host code.

## 3. Ordinary Rust checks: useful, but not formal theorems

The strict kernel is a separate inference implementation: it does not depend
on `mrs-search`, `mrs-calculus` or `mrs-unify`. It **does** share `mrs-core`,
`mrs-tptp`, `mrs-cnf`, `mrs-arithmetic` and CaDiCaL-related infrastructure with
the workspace. “Independent kernel” does not mean an independent parser,
compiler, operating system or every preprocessing library.

Runtime contracts include:

- loaded leaf/source/formula/role matching, unique proof names, resolved parents
  and cycle detection; duplicate problem names use existential exact matching;
- original positive conjectures cannot justify the final contradiction;
  negation must be checked and tied to the original conjecture, and unsupported
  roles/multiple positive conjectures fail closed;
- fresh Skolem/definition scopes and supported conservative CNF transformations;
- independent supported first-order inference recomputation, including bounded
  theory-specific AC fallback, not acceptance by rule label alone;
- AVATAR component variable independence, global name consistency, inherited
  contexts, source-bound SAT manifest and an unconditional discharged false
  root; structural-only `check_avatar_structure` is **not** a proof verdict;
- SAT-trace replay for supported payloads; a trace-free AVATAR roll-up instead
  trusts in-process CaDiCaL unsatisfiability after structure/instance checks;
- complete finite interpretation tables, identity equality, original formula /
  selected-include evaluation, correct conjecture polarity and supported closed
  arithmetic in its separate infinite integer/rational sorts.

These implementations have tests, mutation canaries and archived replays.
Their complete correctness is **not** proved in Verus. SAT/RUP/LRAT replay and
model-table evaluation do not acquire a formal theorem merely because the
general first-order replay has one. Likewise, agreement of the ordered ground
closure with its unrestricted reference is runtime differential evidence;
formal grounding/closure completeness is not implemented.

## 4. Trusted boundaries and assumptions

| Boundary | Current status / consequence |
|---|---|
| Original TPTP bytes → AST | Winnow/parser behavior is tested, not verified. Broad parser dialect support is not semantic support. |
| Include paths/selections/roles | Trusted loaders; cycle/occurrence checks exist, but no verified selected-closure semantics or immutable producer/checker closure snapshot. File-based self-check with includes can reread the main problem and includes at verification time. |
| AST → clauses / fresh symbols | Trusted lowering, type/role handling and clausification; normal strict replay checks supported transformations, but Verus does not cover the full FOF ancestry. |
| TPTP → formal IR/certificates | Trusted canonicalization, symbol assignment, alpha/equality/order normalization and certificate discovery. Formal acceptance applies to the resulting IR, not automatically to the original TPTP claim. |
| Executable/evidence identity | Explicit artifact hash, adjacent manifest/evidence checks and private executable byte snapshot on the formal bridge; metadata is trusted build evidence, not cryptographic proof that a verifier/compiler produced the binary. |
| Runtime containment | Formal bridge has private process group, OS limits and Linux `NO_NEW_PRIVS`; the self-check coordinator kills/reaps its child at its original deadline. Neither is a formal sandbox theorem. Synchronous model/parser/host operations can overrun cooperative budgets. |
| Toolchain | Pinned Verus and Rust, Verus VC generation, bundled SMT solver, vstd specifications, compiler/LLVM, build commands and host OS remain in the TCB. `--no-cheating` and source audit are safeguards, not verification of that TCB. |

The proof-DAG loader has a 32-MiB proof input limit but no per-file/aggregate
problem-source byte ceiling; its include depth/occurrence guards are not an AST
memory bound. Model-file checking has its separate bounded loader/evaluation
contract. Do not substitute one path's bounds for another's.

An explicit Verus configuration can select an older hash-pinned artifact. Its
own support/evidence sidecar, not today's source tree, defines that artifact's
scope. In particular, a historical finite-carrier artifact must not be relabeled
as an infinite-carrier or end-to-end certifier by the calling CLI.

## 5. Guarantees MRS does not currently have

- end-to-end formal soundness from original selected TPTP bytes to emitted SZS;
- a theorem over arbitrary semantic carrier types, or verified transport to
  standard all-model first-order unsatisfiability;
- complete formal verification of the ordinary Rust proof/model kernel;
- verified general FOF clausification/Skolem extension, AVATAR/SAT roll-up,
  arithmetic, typed/higher-order/nonclassical input or general AC calculus;
- verified input retrieval/index completeness, all preprocessing deletions,
  dynamic given-clause fairness or portfolio/shared-clause lifecycle;
- refutational completeness of shipped resource-pruned/heuristic schedules;
- guaranteed first-order decision termination, deterministic wall-clock search,
  or an instruction/heap/wall-time cost theorem for the complete process;
- blanket certification of archived campaigns, representative formal rule
  coverage, or compiler/Verus/SMT correctness/reproducibility across systems.

An unrestricted fair first-order procedure could at most supply a
refutation-semi-decision completeness theorem; bounded heuristics cannot be
promoted to a complete decision procedure by removing one guard or passing a
configuration audit. `SearchConfig::check_completeness` explicitly disclaims
such a proof.

## 6. Evidence and maintenance

The pinned formal gate checks all four formal `.rs` modules, strict escape-hatch
rejection, same-host reproducible runtime builds, runtime/DAG/bridge fixtures,
58 verifier-negative and 50 compiled-runtime mutation cases. The fourteen-case
dual-check corpus includes small generated and four public Prover9 proofs;
it is **not** a representative competition archive. c502 is a local-step
regression, not certification of the full SWV406+1 proof.

CI configures two independent same-image runner builds, but configuration alone
does not establish a successful comparison. Tests, rule counts, zero rejection
counts, historical ProoVer scores and historical campaign audits are evidence
for their exact artifacts/modes/budgets—not proof of universal correctness.
Fresh replays must preserve checker/input identities and explain every changed
verdict; old results remain historical.

For every assurance change, update the support manifest, assurance note, this
inventory and the roadmap. Verify before promoting a claim. The dated
[audit](../reports/audits/formal-guarantees-2026-11-21.md) records the initial
finding and the first repair's exact gate results.
