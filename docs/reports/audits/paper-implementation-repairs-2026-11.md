# Paper implementation review and repairs

## Outcome and scope

The feature branch was reviewed at `d3d71444`, repaired at `4aec8c58`, and
fast-forward merged locally into `main`. Nothing was pushed. The separate
untracked journal-report directory was not edited or committed.

The subsequent repairs address the six recommended work items: checker
soundness, first-order BCE, index recall, stored-certificate re-audits,
per-paper contracts, and architectural/performance assessment. They do not
claim complete AVATAR, general InstGen/AC calculus, or dynamic completeness of
the whole prover. The current source-derived contract is
[paper-implementations.md](../../reference/paper-implementations.md).

## 1. Proof-wide assumption soundness

The four original AVATAR failures have permanent reduced fixtures and negative
kernel/competition tests:

- an undischarged branch-local false root, including an `ex_falso` alias;
- a split across shared first-order variables;
- incompatible components reusing SAT names across splits;
- an uncited empty original in an LRAT manifest with a recomputed valid digest.

Every SAT original is now justified, not merely the cited subset. Repeated
justified clauses and tautologies remain allowed. Sparse names are densified
before trace-free CaDiCaL replay. Component equivalence is independently bounded
and does not trust the producer's key. Competition mode applies the proof-wide
structural gate before its ordinary inference checks. That gate returns
`Result<(), KernelVerdict>`, not a positive proof verdict.

Final review reproduced two additional false certifications: negating a ground
axiom through an `assume_negation` pedigree, and asserting a positive conjecture
as an axiom before refuting it. Source-bound conjecture negation is now checked;
positive-conjecture dependence cannot discharge a false root. Interpretation
roles and multiple positive conjectures fail closed as unsupported. Proper
conjecture-negation controls still certify. All six canaries are stored under
`crates/mrs-proof-kernel/tests/resources/avatar/`.

## 2. First-order BCE

The stronger assumption gate also caught two old inline definition-renaming
fixtures that used a positive conjecture as a premise. Their source and proof
roles were corrected to an asserted `negated_conjecture`, appropriate for the
local transformation replay they exercise. No checker guard was relaxed and
the newly added forged-conjecture canary remains rejected.

The old binary-only test deleted both clauses of Kiesl et al.'s Example 4,
turning a trivial diagonal contradiction into an empty residual theory.
The repaired reducer checks all L-resolvents within explicit caps. Incomplete
subset, deadline, or legacy-unifier-shape checks cannot authorize deletion.
It is a bounded enumeration, not the paper's polynomial algorithm or its
equality-blocking extension. The paper counterexample refutes with default
preprocessing and its emitted proof passes the independent kernel. A separate
two-element model oracle covers 49 clause pairs.

## 3. Index recall

The former `u8` arity truncated 256 to zero. The operational and compressed
discrimination trees now use exact `usize` arities. Independent recall/removal
checks cover both term retrieval and resolution at boundary arities; full
search refutations replay through the kernel at 255, 256, 257 and 512.
This changes the Cell layout/cost; no throughput gain is inferred from the
correctness canaries.

## 4. Fresh stored-certificate replay

The reusable `mrs-tools reaudit-proofs` command never regenerates proofs or
overwrites an earlier audit. Reports bind proof bytes, checker/tool hashes,
limits, input-tree manifests and digests of the checker’s actually loaded
problem/include strings. A positive result without loaded-input identity is
not credited. Identical-artifact comparisons also require identical loaded
input identity. See [the guide](../../guides/certificate-reaudits.md).

Banked observations are under
[`docs/results/paper-audit-20261121/`](../../results/paper-audit-20261121/):

| Replay | Result |
|---|---|
| Six reduced forged canaries | 6 `VerifiedBad`, 0 positive certifications |
| Seven stored MRS golden proofs | 7 `VerifiedGood` |
| Full 100-proof committed PRV corpus, 15 s / 2048 MiB | 30 `VerifiedGood`, 34 `VerifiedBad`, 36 `Unknown` |
| 17 locally retained EPU proofs, 120 s / 2048 MiB | 16 `VerifiedGood`, one outer timeout (`HWV078-1`) |
| Same 17 EPU proof bytes, repaired release checker, 120 s / 2048 MiB | 17 `VerifiedGood`, 0 rejected, 0 inconclusive |

The PRV corpus intentionally contains both good and bad proofs. These are strict
replay counts, not competition-mode scores. Against the earlier same-day
post-AVATAR-repair replay (31/33/36), the sole verdict change on identical
proof/input bytes is `PRV031+1`: `VerifiedGood` to `VerifiedBad`. Its `assume`
step uses the positive conjecture `q(a)` alongside its negation to manufacture
the contradiction. The corpus manifest permits either decisive verdict for
this historically ambiguous case; the stronger assumption contract rejects it.

The initial 30-second debug replay of the 17 locally retained EPU proofs had
15 `VerifiedGood`, one deadline `Unknown` (`HWV092-1`), and one outer timeout
(`HWV078-1`). It is retained separately, not used to revise the earlier
archived 17/17 result. The final wider-budget replay certifies `HWV092-1` and
16/17 overall; `HWV078-1` still reaches the outer 125-second guard before a
loaded-input identity is emitted. It remains unverified in this debug replay,
not `VerifiedBad`. Its exact proof hash is retained in the report. These runs
do not establish a corrected historical score or diagnose the timeout cause.
The separately built release checker certifies all 17 at the same declared
time/memory allowance, including `HWV078-1` in about 11 seconds. This establishes
replay of those exact archived proof bytes with the repaired checker; it is not
a matched build-profile speedup measurement or a fresh campaign result. Both
debug and release identities/reports are retained.
Neither this local replay nor the golden canaries re-audit remote full-division
archives unavailable in this checkout, or establish a canonical CASC score.

## 5–6. Claims and architectural assessment

The active ledger replaces the retired blanket checkbox contract and fixes the
McCune/FVI/InstGen/Deepire citation errors, the *Playing with AVATAR* date, and
the confusion between dependency eviction and SAT-based global subsumption.
Research adaptations are labeled as such. Unverified citations stay explicit.

Tied-shape component naming now uses bounded, genuinely invariant canonical
numbering in both representations. A refused key allocates a fresh name,
never a guessed component match. Marker allocation preserves input predicates.
Ordinary backward subsumption still does not blindly erase every descendant;
a broader orphan policy needs a separate redundancy/fairness argument.

Polynomial BCE, signed ground naming/unit abstraction, larger-body variant
indexing, full substitution trees, general InstGen/AC calculus and paper-style
ML pipelines remain separately assessed projects. Shipped schedules, routing
rules, and experimental backend defaults were not promoted. Any performance
promotion must use paired, strictly audited, held-out cooperative campaigns on
the canonical eight-physical-core shape.

## Verification gate

The final changes passed all four repository Cargo gates in the Nix
environment: `cargo check`, `cargo clippy --all -- -D warnings`,
`cargo fmt --all --check`, and `cargo test --workspace` (including doctests).
They ran sequentially with
`CARGO_TARGET_DIR=/tmp/opencode/mrs-verification-20261121`.
A separate Cargo target directory was used for final workspace
verification because concurrent formal-tool builds invalidated dependencies
while an earlier workspace rustdoc pass was running. No source tests were
disabled to bypass that build-artifact collision. The independent formal
verification run uses the
isolated `.#verus` shell and remains evidence only for its declared decoded-IR
subset, not for AVATAR or whole-prover completeness.

The first full Verus run verified the base modules with zero errors and passed
the reproducible runtime checks, but two runtime-mutation compilations reached
their 300-second guards (`resets_work_at_dag_demodulation` and
`without_freshening_lookup_charge`). This is not a passed mutation gate. The
formal runner now reports the mutation name on timeout, kills/reaps descendant
processes, and supports a bounded `VERUS_MUTATION_COMPILE_SECONDS` override
(1–3600 seconds). The 58/50 inventory, SMT rlimit, expected rejection rules and
execution limits are unchanged; timing out still fails the gate. The fresh full
run with `VERUS_MUTATION_COMPILE_SECONDS=900` completed successfully with two
Verus threads and two mutation workers: all base verification passes had zero
errors, reproducible-runtime/DAG checks passed, and the complete
`verifier_negative=58 compiled_runtime=50` gate passed. The verified runtime
hash was `8aa08f1e8006e5c1bc4da24060796538c33e15f597e5a49d044ebc149c3d3fe5`.
The exact log is banked as `verus-verification.log` beside the replay reports.
The four Cargo gates were also rerun successfully after the runner fixes.
