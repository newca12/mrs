# UEQ recovery: restore ordinary superposition before changing AC search

## Finding

The target-rigidity repair removed valid **ordinary first-order superposition**,
not just problematic AC matches. Ordinary superposition standardizes its parents
apart, unifies the source side with a non-variable target subterm, and applies
the substitution to **every residual literal of both parents**. Variables from
either parent may be bound. Rigidity is appropriate for one-way simplification
matching; it is not a general superposition soundness requirement.

The implementation now first tries ordinary Robinson syntactic unification in
both producer representations. The strict kernel independently reconstructs
ordinary superposition using its own occurs-checked unifier. The source ordering
check remains in the producer. Restricted AC/associative matching and its replay
remain restricted; the removed legacy unequal-flattened-arity fallback is not
restored.

This is a small calculus repair, not evidence that MRS needs a rewrite.

## Evidence and corrections

The retained `group.s` c26 is valid: instantiate associativity at
`A=inv(B)` and replace `mult(inv(B),B)` using the inverse axiom. The result is
`mult(e,C)=mult(inv(B),mult(B,C))`. The complete target equality, including its
other side, is instantiated. Its production regression now checks the exact
result rather than refusing it merely for binding a target variable.

SWV406+1 c502 is likewise an ordinary syntactic instance of both complete
parents, retaining the source side literals. Its former refusal tests have
been corrected. Several other tests labelled invalid target specializations
also describe valid instances. Their failing old assertions were not evidence
of unsoundness. In particular, the previous investigation's statement that the
small lattice-shaped replay test was a known invalid inference was incorrect.

A new kernel regression distinguishes the actual boundary:

```text
f(a)=b,  q(X) | p(f(X))
    entails q(a) | p(b)
    does not justify q(X) | p(b).
```

It certifies the whole-parent instance and rejects the partially substituted
forgery. Existing archived AC refusal tests, unmatched-operand tests and
associative-only permutation rejection tests still pass. Rejection of a
particular AC shape is not by itself a proof that the shape is semantically
invalid: incomplete AC unification/replay remains an open issue.

Earlier notes attributed the historical AC failures to malformed substitutions
from the unequal-flattened-arity fallback. That explanation was not established
by checking complete substitutions. Unequal operand counts before substitution
and target-variable bindings do not alone invalidate a unifier. The fallback
remains disabled, but deciding which archived AC steps are valid requires exact
theory-unifier validation, not another blanket restriction.

## Measurements (2026-10 investigation)

The original campaigns used 400 CASC-J13 UEQ problems, eight workers, `jobs=1`,
180 seconds, and order `4,8,12,11,2,14,15,1` on `teenf9901`:

| Campaign | Recorded source | Binary SHA-256 | Refutations |
|---|---|---|---:|
| `campaign-cascj13-clean-unsound` | `176a9f975a3f7256bd27d231a21e2f87ffd61071` | `1bd3efc45ea23594ca32ee17396f88c40cda2f918bdff2b6684f8a7b6e716775` | 114/400 |
| `campaign-cascj13-clean` | `313b9237ceff11c5db6f5e4889ced685538ce02e` | `f8825f9170e2b8de6c191d5a2b648a86beb7133f9642ef59739da1bd56703d80` | 5/400 |

Both metadata records say `git_dirty=true`; the binary hashes identify the
artifacts. The locally available metadata records `MRS_HARDWARE=casc` for both.
There are 112 old-only solves, two common solves and three clean-only solves
(`RNG019-7`, `RNG020-7`, `RNG021-7`). This is not an isolated code-only A/B.

Strict audits of the **same 114 old UEQ proofs**:

| Checker | VerifiedGood | VerifiedBad | Unknown |
|---|---:|---:|---:|
| Unmodified `dd98155` target-rigid kernel | 1 | 82 | 31 |
| Earlier diagnostic relaxation (not retained) | 15 | 82 | 17 |
| Ordinary syntactic replay restored; AC rigidity retained | **91** | **0** | **23** |

The earlier diagnostic used a one-sided helper with an empty rigid set, which
still refused compound-to-variable bindings. It did **not** implement ordinary
two-sided unification. Its initial identical counts were also cached audit
results until the verifier was rebuilt and `--force` supplied. The earlier
conclusion that removing ordinary rigidity could not recover certification was
therefore wrong.

These are archived-proof audit totals, **not a fresh 91/400 CASC score**, and
not a formal verification of all MRS inferences. The 23 Unknown proofs remain
uncertified. The historical `clean-unsound` directory name is not a per-proof
soundness verdict.

Fresh local search, one worker, five seconds per problem, recovered all five
regressions: `group.p`, `group_right_inv.p`, `group_unique_inv.p`,
`lattice_absorb.p`, and `ring_idem.p`. All five fresh proofs passed strict replay.
This host-limited smoke measurement establishes problem-level recovery, not
competition performance.

Local validation passed: `cargo check`, `cargo clippy --all -- -D warnings`,
`cargo fmt --all --check`, and `cargo test --workspace`. The mutation sweep
again requires every unmodified canary, including group/lattice/ring proofs,
to certify and rejects its generated mutants. This is executable validation;
the separate ground Verus checker does not prove this first-order repair.

Scratch evidence: `/tmp/opencode/ueq-unsound-reaudit/`,
`/tmp/opencode/ueq-unrigid-kernel-audit/`,
`/tmp/opencode/ueq-syntactic-only-audit/`, and
`/tmp/opencode/ueq-local-syntactic-recovery/`. These are temporary, unbanked
artifacts. Source campaigns live under
`/home/hack/crates/mrs-bench/results/`. Regenerate the audits with the checker
revision being evaluated; old audit summaries are historical.

## Fresh full-division validation (2026-10-16)

The requested fresh campaign was run with the shipped UEQ portfolio and
cross-strategy sharing disabled. It completed all 400 CASC-J13 UEQ problems:

| Measure | Result |
|---|---:|
| Generated refutations | **112/400 (28.00%)** |
| Strict `VerifiedGood` | **111/400 (27.75%)** |
| Strict `Unknown` | 1 |
| `VerifiedBad` / checker errors | **0 / 0** |
| Other outcomes | 273 `Timeout`, 15 `GaveUp` |

The unknown is `RNG009-7`, whose proof reaches `ac_superposition` node `c3774551`;
strict replay reports that the conclusion is not a valid rewrite and that AC
superposition replay is incomplete. This is consistent with the remaining AC
replay gap, not evidence of a bad proof. The other 111 generated proofs passed
strict replay. All 112 proof files are present, their recorded hashes match,
and the audit rows agree with `run.csv`.

Against the previously documented 107/400 strict result, the generated solve
sets have 101 problems in common, 11 current-only, and 6 baseline-only. The
strictly certified score is therefore up by four, from 107 to 111; the 112th
refutation remains uncertified. This is a measured coverage recovery, but not an
isolated code-only A/B: the baseline and recovery run used different binaries,
toolchains and hosts. It is one run, so it does not establish a repeatable
four-solve gain above the known UEQ run variance.

Run provenance: `~/crates/mrs-bench/results/ueq-syntactic-recovery-j13/`;
`git_commit abeda8c53aab2d96e7f42c10969bb569e45e1646`, dirty tree,
`binary_sha256 07626858470bfbe45c29b22b6788c2afe0367715280fc36f067d751aae1d5467`.
The run used eight workers on eight physical cores (Intel Xeon E5-2407),
`MRS_HARDWARE=casc`, `MRS_SHARED_POOL_INTERVAL=0`, strategy order
`4,8,12,11,2,14,15,1`, and the official 180-second UEQ clock. The host had
96,350 MB RAM rather than the 128 GB CASC allowance; the prover reported an
effective memory budget of 81,897 MB. This did not produce a resource-limit
outcome, but the measurement is not on the full CASC memory allowance.

## CASC-30 follow-up audit (2026-10-16)

The combined CASC-30 FEQ/UEQ campaign
`campaign-casc30-ueq-feq-W8CASCHJ1-20261016` is a separate edition and a
separate binary, so it does not replace the CASC-J13 measurement above. Its
UEQ slice generated 124 refutations, of which 118 passed strict replay, **five
were `VerifiedBad`**, and one refutation artifact lacked a `$false` root and
proof-omission marker. The five rejected problems are `MVA008-1`, `MVA009-1`,
`REL034-2`, `RNG009-5`, and `RNG009-7`.

The current evidence does not establish one common cause: two proofs cite
residual-operation leaves that do not match the named problem axioms, two have
symbol-signature/arity conflicts, and one cites a distribution axiom leaf that
does not match. Treat these as failed proof audits until each derivation is
explained and re-audited; do not count the raw 124 refutations as a sound solve
score. Relative to the October 7 CASC-30 UEQ audit (127 `VerifiedGood`), this
campaign has 113 certified problems in common, five certified only in the new
run, and 14 only in the October 7 run: net strict coverage is down nine. It is
not a controlled A/B because the binaries differ.

The companion FEQ slice is useful context: it produced 109 theorems, 105
`VerifiedGood`, two `VerifiedBad`, one `Unknown`, and one verifier timeout.
Therefore the combined campaign improves the strict FEQ count against the
October 12 clean campaign (70 to 105), while the UEQ strict count regresses and
has unresolved failed audits. Both divisions ran on eight physical cores in
CASC mode, but the 95,969 MB host had only 81,897 MB effective memory against
the 128-GiB allowance. No row was stopped by a resource limit. Full campaign
provenance and the per-problem diagnostics are recorded in
[`results/README.md`](../../results/README.md#casc-30-combined-fequeq-full-division-campaign-2026-10-16).

The run metadata's `default_time_limit=120` is the harness default, not the
actual per-problem timeout: `use_casc_times=1`, `run.log`, `run.csv`, and the
audit all show 180 seconds for UEQ.

## CASC-J13 UEQ audit follow-up (2026-10-20)

The archived campaign
`campaign-casc-j13-04-ueq-CASCHJ1-20261020` produced 120 refutations in the
400-problem CASC-J13 UEQ division. Its strict audit recorded **112
`VerifiedGood`, 6 `VerifiedBad`, and 2 `Unknown`**. The six `VerifiedBad` verdicts
are a verifier false rejection: each proof leaf cites the correct named CNF
equality axiom, but the emitted equation has its sides reversed.

| Problems | Cited leaf | Problem axiom | Proof leaf |
|---|---|---|---|
| `GRP178-1`, `GRP183-3`, `GRP186-1` | `monotony_glb2` | `greatest_lower_bound(multiply(Y,X),multiply(Z,X)) = multiply(greatest_lower_bound(Y,Z),X)` in `Axioms/GRP004-2.ax` | The same equation with sides swapped |
| `REL026-3`, `REL041-2`, `REL045-2` | `dedekind_law_14` | `composition(meet(A,composition(C,converse(B))),meet(B,composition(converse(A),C))) = join(meet(composition(A,B),C),composition(meet(A,composition(C,converse(B))),meet(B,composition(converse(A),C))))` in `Axioms/REL001-1.ax` | The same equation with sides swapped |

For example, `GRP178-1.s` line 11 attributes its input clause `c13` to
`monotony_glb2`; the audit reports `leaf c13 does not match problem formula
monotony_glb2`. The included axiom is in
`crates/mrs-bench/problems/casc-j13/Axioms/GRP004-2.ax:31-32`, and the
normalized campaign problem is
`crates/mrs-bench/results/campaign-casc-j13-04-ueq-CASCHJ1-20261020/certification/proofs/mrs/ueq/Problems/GRP178-1.p`.
The REL cases show the same pattern against
`crates/mrs-bench/problems/casc-j13/Axioms/REL001-1.ax:1-2`. This rules out a
missing include or wrong source name: the named formulas are present and match
modulo equality symmetry.

The campaign's strict audit invokes `mrs-proover --strict`, which routes to the
strict proof kernel (`crates/mrs-proover/src/bin/mrs-proover.rs`). The kernel's
bounded matcher does try both orientations of equality in
`crates/mrs-proof-kernel/src/lib.rs`, `AcState::atom_eq`; therefore the
failure is not simply a missing equation-symmetry branch.

The remaining mismatch is in matching CNF variables after the side reversal.
`lower_cnf_formula` in the proof kernel turns the implicitly universally
quantified CNF variables into nested `Forall` binders in first-occurrence order.
When an equality's sides are reversed, their first-occurrence order can change.
The formula matcher then compares variables by binder depth. Equality matching
tries the reversed sides, but does not independently recover a variable
correspondence for that orientation, so it can compare the right equation under
the wrong binder mapping and reject it. This explains why the formulas are the
same up to alpha-renaming and equation symmetry while the archived kernel audit
still returns `VerifiedBad`.

The direct replay with `target/release/mrs-proover --strict` reproduces the
archived `VerifiedBad` for `GRP178-1`. This confirms the failure in the current
strict path. The campaign's `git_dirty=true` metadata means its exact dirty
source snapshot is unavailable, so archived source-level details cannot be
reconstructed from the commit alone.

**Conclusion:** these six `VerifiedBad` rows were false rejections, not evidence
of unsound inference. The cited axioms and emitted leaves are identical modulo
variable renaming and equation symmetry; the old strict matcher rejected them
because the flipped equality orientation was checked with the wrong binder
correspondence. The targeted re-audit below now certifies all six. The other 114
audit results (112 good, 2 unknown) are unaffected by this diagnosis.

### Fix and re-audit (2026-10-21)

`mrs-proof-kernel` now compares CNF leaves as disjunctions of literals under one
explicit bijection between their implicitly universal variables. Literal
pairing backtracks consistently across the clause, and equality matching
backtracks over both orientations while retaining the successful variable map.
The comparison uses the existing `max_comparison_steps` budget; exhaustion
remains inconclusive, never a mismatch. FOF leaf comparison continues through
the existing bounded AC-alpha-equivalence path.

Regression tests cover both archived axiom shapes, reject inconsistent
cross-literal variable mappings and changed function symbols, and verify that
budget exhaustion returns `Undecided`. The full proof-kernel suite passes (302
unit tests plus integration tests). Re-running strict verification on the six
preserved campaign proofs now returns `VerifiedGood` for all six:

| Problems | Before | After |
|---|---|---|
| `GRP178-1`, `GRP183-3`, `GRP186-1` | `VerifiedBad` | `VerifiedGood` |
| `REL026-3`, `REL041-2`, `REL045-2` | `VerifiedBad` | `VerifiedGood` |

This is a targeted re-audit of the six archived proofs, not a rerun of the
400-problem search or a new division score.

Run provenance is in the campaign directory's `run_meta.json` and `run_meta.txt`:
`git_commit=a38c8bc83c5c2c16fb478e6fe2f4d6392f58c7df`,
`git_dirty=true`, 8 workers, `MRS_HARDWARE=casc`, `jobs=1`, official 180-second
UEQ limit, and binary SHA-256
`c881d56f47b848d507017f4f3a4cb58fa24da0c4e3d8e267c45bc6c0d4e7ae94`. Because
the source tree was dirty, the commit alone does not reconstruct the exact
campaign source; the archived stdout/proofs and audit details are the primary
evidence. This is a verifier diagnosis, not a fresh search or certification
measurement.

## Follow-up needed

Repeat the shipped-portfolio, no-sharing measurement at least five times on a
host with eight physical cores and 128 GB before treating the observed coverage
delta as stable. Keep the calculus and portfolio fixed during those replicates.
Record source state, binary hashes and environment. Build the verifier
explicitly: building the audit binary alone does not rebuild `mrs-proover`.

```bash
cargo build --release -p mrs -p mrs-proover -p mrs-bench \
  --bin mrs --bin mrs-proover --bin audit_casc_proofs
MRS_HARDWARE=casc MRS_WORKERS=8 MRS_SHARED_POOL_INTERVAL=0 \
  crates/mrs-bench/certification_campaign.sh \
  --edition casc-j13 --systems mrs --divisions ueq --casc-times --jobs 1 \
  --output crates/mrs-bench/results/ueq-syntactic-recovery-j13
```

Preserve `run.csv`, `run_meta.json`, `run.log`, `certification/audit.csv`,
`certification/audit-summary.txt`, and raw proofs for any failed audit. Compare
generated refutations and strictly certified refutations separately. After the
replicates, investigate the remaining AC `Unknown` proofs; only then re-open
portfolio/sharing optimization.
