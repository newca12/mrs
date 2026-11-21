# Formal-guarantee audit and first repair — 2026-11-21

**Source baseline:** `bd04ae44` on local `main`. This is an assurance audit, not
a score/performance campaign. It does not replace historical certificate
replays or establish full prover correctness. The current contracts are the
[guarantee inventory](../../reference/formal-guarantees.md) and
[F0–F8 roadmap](../../research/formal-assurance-roadmap.md).

## Audit method and coverage

Reviewed the original-input/status paths in `src/main.rs`, self-check coordinator,
ordinary strict and competition verifiers, model evaluator, certified ground/EPR
routes, kernel dependencies, formal semantic/executable/wire definitions,
support manifest, build/runtime tooling, CI and current-facing documentation.
Compared declarations to actual dispatch, not to paper names or test totals.

Coverage includes the user-visible positive modes, local versus whole-DAG formal
results, model semantics, provenance/assumption handling, resource boundaries,
trusted build/input infrastructure, unsupported claims and retained evidence.
It is not a new machine proof of every ordinary Rust rule, a security penetration
test of the OS, a campaign re-audit, or independent review of Verus/SMT/LLVM.

## Findings

| ID | Baseline finding | Resolution / remaining boundary |
|---|---|---|
| FG-1 | The main no-model theorem uses `Set<u64>` carriers and `u64` denotations. These are finite/machine-bounded, not arbitrary first-order models. | First semantic repair: `ISet<int>` domains and mathematical integer values, with infinite-model obligations. Generic arbitrary-carrier / standard all-model transport remains open (F1). No invalid accepted proof was demonstrated by this scope finding. |
| FG-2 | Literal-permutation validity, bijective renaming and their composition are proved, but not all are DAG/wire-dispatched. The host still normalizes proof conclusions. | Added explicit standalone entries to the support manifest; integration is F3, not claimed done. |
| FG-3 | The ordinary strict/model kernels are runtime implementations, not the Verus-verified checker. The replay algorithm is separate, but several libraries are shared with search. | Current docs distinguish implementation independence from an independent entire TCB and from formal verification. Formalizing additional kernels is F4–F6. |
| FG-4 | Default theorem output does not require strict self-check. The Verus modes are explicit standalone verifier options, not what `mrs --self-check` invokes. | README mode matrix and inventory name the actual paths. Defaults are unchanged. |
| FG-5 | Model-bearing certificates and ground-closure saturation witnesses are different. A supported closure witness may carry no model; ordered/reference closures share implementation infrastructure. | Runtime closure agreement is labeled differential evidence, not a formal completeness theorem. Model evaluator/grounding/closure assurance is F6. |
| FG-6 | With includes, file-based self-check can reopen the original problem and included files rather than certify an immutable initial producer snapshot. Loader bounds differ between proof and model paths. | Documented the caller/input-time distinction; immutable selected closure and formal byte-to-clause refinement are F2. No new input-snapshot guarantee is claimed. |
| FG-7 | The formal wire/token decoder is verified, while a bridge module comment incorrectly called the wire decoder unverified. TPTP/request encoding still are trusted. | Corrected the comment and the boundary documentation, without changing the bridge algorithm. |
| FG-8 | The work meter proves checker-defined cumulative bounds, not instructions, heap use, wall-clock preemption or a formal process sandbox. | Kept those contracts separate. Host I/O/parser/model/deadline/memory composition is F8. |
| FG-9 | An older explicitly hash-pinned artifact can have an older scope. A sidecar is trusted build metadata, not self-authenticating verification attestation. Configured independent-runner CI is not evidence it passed. | Documented artifact-specific scope and TCB; no old artifact or pending CI comparison is relabeled. |
| FG-10 | Current-facing pages mix stale versions/status text and historical score statements with present guarantees. Local c502 and a small proof corpus do not certify full archives. | Updated README/docs links, version/status language and historical-score interpretation while preserving numeric tables and old reports. No score is revised by this audit. |

### FG-1: why the model carrier matters

The ordinary mathematical first-order notion ranges over arbitrary nonempty
domains. The baseline formal `Interpretation` instead has `domain: Set<u64>`;
function/valuation values are also `u64`. Saying “no well-formed interpretation”
therefore quantifies only over that representation. The inference checker may
be correct on its declared universe without that theorem establishing the
broader advertised notion.

A useful distinction is the finite clause theory:

```tptp
cnf(injective, axiom, f(X) != f(Y) | X = Y).
cnf(missing_range_element, axiom, f(X) != c).
```

On the nonnegative integers, `c = 0` and `f(n) = n + 1` satisfy both clauses.
An injective endofunction on a finite domain cannot omit a domain element.
This example motivates an infinite semantic carrier; it is **not** an archived
forged proof that the old runtime accepted. The verified repair constructs the
infinite model and checks both clauses under all well-formed valuations. Its
membership obligation includes `u64::MAX + 1` as a mathematical integer.

Changing only `u64` to `int` would not suffice if the domain remained `Set`:
the unbounded carrier requires **`ISet<int>`**. Runtime variable/symbol/node IDs
remain `u64`; domain denotations are no longer those IDs. The interpretation
still requires nonemptiness, total maps and domain-valued function results.

This admits arbitrary finite sizes and countably infinite carriers. It does
**not** mechanically prove arbitrary carrier types or a Löwenheim–Skolem /
model-transport bridge to all standard first-order models. The older narrow
pilot modules are not silently generalized by the main-core repair.

## Implemented first slice

- Generalized the main semantic interpretation, valuation, evaluation and
  related one-variable semantic helper lemmas to mathematical integer values.
- Re-proved the existing local-rule, substitution/renaming and whole-DAG
  obligations over that carrier; added `infinite_semantic_carrier_regression`.
- Added explicit semantic-carrier and standalone-guarantee fields to
  `supported-rules.json`. Renamed its ambiguous `prototype_non_certifying`
  status to `verified_decoded_ir_not_end_to_end_tptp`; all
  `production_certification` flags remain false.
- Updated the bridge/tooling corpus assertions to require the new manifest
  scope, with arbitrary-carrier transport and permutation dispatch still false.
- Added the full dependency-ordered roadmap and corrected current-facing docs.

No wire tag, inference acceptance algorithm, search schedule, pruning policy,
certification default, toolchain pin, archived proof fixture or SAT budget was
changed. This is an extension of the **proved semantic scope**, not a new
production certification policy or a claim that the roadmap is finished.

## Verification and evidence

The focused pinned core check passes with **578 verified, 0 errors**, under
`--no-cheating --rlimit 30` and two verifier threads. The original 577 checks
also pass after widening; the added model regression accounts for the new
obligation. One initial diagnostic run exposed missing definition-unfolding
fuel in the new regression; explicitly unfolding `eval` twice fixed the proof,
without increasing the SMT rlimit or weakening any premise.

The full required Nix-wrapped Cargo gate passed: check, Clippy with warnings
denied, format check and workspace tests including doctests. The complete pinned
formal gate also passed: all four proof modules have zero errors, the main core
has **578 verified** obligations, same-host reproducible-runtime and DAG/wire
regressions passed, and actual kernel/ProoVer loaded-job/CLI bridge tests ran
with `MRS_VERUS_BRIDGE_TESTS=1`. The fourteen-case corpus's positive/negative
checks therefore exercised the newly declared manifest/carrier scope through
the actual compiled artifact. All **58 verifier-negative + 50 compiled-runtime**
mutation cases passed their required rejection checks, including the separate
strict assumption-probe control. No inventory was removed or weakened.

The verified runtime SHA-256 is
`2923eea3c15679d861ef706c1cee4f57ec1e40ec0f89ed78a64f9e30b971d401`.
Exact logs and source/manifest identities are banked under
`docs/results/formal-audit-20261121/`. The run used Verus
`0.2026.11.18.a95764f`, Rust `1.100.0`, two verifier threads, two mutation
workers and the existing bounded 900-second runtime-mutation compile allowance.
SMT rlimit 30, runtime/work budgets and rejection requirements were unchanged.
The log names the parent commit `bd04ae44`; its producer tree was dirty because
it contained this repair. `source-identities.sha256` binds the exact checked
files rather than claiming that the parent commit alone identifies the repair.
The independent-runner CI comparison was not run by this local audit and remains
unestablished evidence here.

## What remains after this slice

F1 arbitrary-carrier transport is still open. F2 input/selected-closure fidelity,
F3 normalization/certificate transport, F4 FOF conservative extensions, F5
AVATAR/SAT semantics, F6 models/grounding, F7 dynamic completeness and F8 runtime /
build evidence are explicitly separate work packages. No observation here
justifies a whole-prover, all-rules, end-to-end TPTP, complete AVATAR, guaranteed
termination or competition-score claim.
