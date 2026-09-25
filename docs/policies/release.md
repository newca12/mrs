# Release Checklist

This is the Phase 9 release gate for merging the current feature branch into
`main`. It keeps reviewable phase anchors and prevents a release claim from
depending on an uncommitted worktree or an unvalidated benchmark corpus.

## Focused History

The branch is organized around these reviewable phase anchors, recorded in
`docs/policies/release-phases.tsv`:

- containment and trust policy
- independent proof kernel
- core kernel rules and adversarial tests
- explicit AVATAR/CWA certificates and bounded SAT replay
- strict MRS integration
- ProoVer policy and checker integration
- reproducible benchmark corpus and harness
- Phase 9 release gate

The commits between anchors are intentionally feature- or test-focused. Do not
replace this history with a single squash commit before review; reviewers should
be able to inspect each phase independently.

## Gate

Run from a clean checkout based on `main`:

```bash
nix develop -c cargo run --release -p mrs-bench --bin release_gate -- \
  --base main
```

The gate checks:

- `main` is an ancestor of `HEAD`.
- Every phase anchor exists and is an ancestor of `HEAD`.
- No tracked worktree changes or staged changes remain.
- `git diff --check main...HEAD` is clean.
- `cargo check` passes in the Nix development shell.
- `cargo clippy --all -- -D warnings` passes.
- `cargo fmt --all --check` passes.
- `cargo test --workspace` passes.
- The 100-problem/100-proof PRV corpus, manifest, metadata, and checksums pass
  the Rust validator.

For a local development tree containing unrelated ignored or untracked output,
use `--allow-untracked`; tracked changes are always rejected:

```bash
nix develop -c cargo run --release -p mrs-bench --bin release_gate -- \
  --base main --allow-untracked
```

`--skip-checks` is only for inspecting branch ancestry and phase anchors while
editing the release metadata. It is not a release approval.

## Merge Procedure

1. Commit all intended changes and remove or ignore generated artifacts.
2. Run `release_gate --base main` from the feature branch.
3. Review `git log --oneline main..HEAD`, `git diff --stat main...HEAD`, and the
   phase manifest.
4. Merge with `git merge --ff-only feat/casc-j13-reproduction` if `main` has not
   advanced; otherwise rebase or merge explicitly and rerun the gate.
5. Run the gate again after the merge using `--base main~1` or the recorded
   pre-merge base commit.

No release process should push automatically. Publishing, tagging, and remote
merges require an explicit human decision after the gate passes.

## Certified Release Verification (Phase 9)

In addition to the baseline release gate, every certified release promotion requires verifying:

1. **Certified CASC Run**:
    - Run the benchmark sweep with the applicable certified path (`mrs --self-check`
      for candidate refutations, or `--certify-ordered` for the bounded diagnostic
      certifier). `MRS_CERTIFIED` is not a current CLI/environment switch.
   - Every emitted refutation candidate is verified by `mrs-proof-kernel`.
   - Any candidate failing kernel verification triggers fallback to subsequent portfolio candidates.

2. **Strict Proof-Audit Summary**:
   - Audit all generated proofs using `audit_casc_proofs --checks strict,mrs,ladder`.
   - Ensure zero `VerifiedBad` results in strict and ladder modes.
   - All accepted refutations match the TPTP reference polarity and have unbroken proof DAGs.

3. **Candidate Rejection Summary**:
   - Record telemetry for any uncertified candidates rejected by the async coordinator (`candidate_rejected` counts and failure reasons).
   - Confirm that an uncertified first candidate does not suppress a subsequent valid proof.

4. **FEQ Bad-Proof Regression Audit**:
   - Confirm all historic FEQ bad-proof problems are either solved with verified proofs or fail closed (`GaveUp`).
   - Regression suite passes without any reference polarity violation or unsound step.

5. **EPU and UEQ Golden-Proof Audit**:
   - EPU and UEQ golden test suites run and achieve 100% `VerifiedGood` in strict kernel mode.
   - Demodulation steps are validated step-by-step or safely elaborated.

6. **Proof-Format Reproducibility Check**:
   - TSTP outputs contain valid `% SZS output start CNFRefutation` or `% SZS output start Proof`.
   - Formulas and inferences conform strictly to TPTP v9 specification and ProoVer 2026 rules.

7. **Raw vs Certified Score Comparison**:
   - Report raw, ladder, and strict-certified scores separately in division summaries.
   - Explicitly document the delta between raw generation and kernel-certified totals.

8. **Model-Result Labeling Check for EPS**:
   - Satisfiable / CounterSatisfiable results in EPS must be accompanied by a validated `ModelCertificate` (domain size, constant/function/predicate tables, SHA-256 digest) or marked `N/A: Model`.
   - Never mislabel model results as refutation proof certificates.

## Known Boundaries

- The PRV score report requires the configured verifier mode and any external
  ATP binaries used by competition mode. Missing external binaries are an
  environment limitation, not a reason to claim the documented score.
- Bounded AVATAR SAT replay is strict-kernel certified; unsupported RAT,
  incremental, and other general SAT trace variants remain inconclusive.
- Untracked benchmark databases, binaries, logs, and `.direnv` state are never
  part of a release commit.
