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

## Remote validation needed

Use the same host with eight physical cores and 128 GB, one problem at a time.
First measure this repair with the shipped portfolio and sharing disabled;
portfolio tuning would confound the calculus comparison. Record source state,
binary hashes and environment. Build the verifier explicitly: building the audit
binary alone does not rebuild `mrs-proover`.

```bash
cargo build --release -p mrs -p mrs-proover -p mrs-bench \
  --bin mrs --bin mrs-proover --bin audit_casc_proofs
MRS_HARDWARE=casc MRS_WORKERS=8 MRS_SHARED_POOL_INTERVAL=0 \
  crates/mrs-bench/certification_campaign.sh \
  --edition casc-j13 --systems mrs --divisions ueq --casc-times --jobs 1 \
  --output crates/mrs-bench/results/ueq-syntactic-recovery-j13
```

Return `run.csv`, `run_meta.json`, `run.log`, `certification/audit.csv` and
`certification/audit-summary.txt`, preserving raw proofs for any failed audit.
Compare generated refutations and strictly certified refutations separately.
Repeat decisive comparisons at least five times after the first audited campaign
shows that recovery survives the full division. Then investigate remaining AC
Unknown nodes and only then re-open portfolio/sharing optimization.
