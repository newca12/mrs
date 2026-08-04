# Release Checklist

This is the Phase 9 release gate for merging the current feature branch into
`main`. It keeps reviewable phase anchors and prevents a release claim from
depending on an uncommitted worktree or an unvalidated benchmark corpus.

## Focused History

The branch is organized around these reviewable phase anchors, recorded in
`docs/RELEASE_PHASES.tsv`:

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

## Known Boundaries

- The PRV score report requires the configured verifier mode and any external
  ATP binaries used by competition mode. Missing external binaries are an
  environment limitation, not a reason to claim the documented score.
- Bounded AVATAR SAT replay is strict-kernel certified; unsupported RAT,
  incremental, and other general SAT trace variants remain inconclusive.
- Untracked benchmark databases, binaries, logs, and `.direnv` state are never
  part of a release commit.
