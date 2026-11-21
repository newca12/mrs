# Re-auditing stored certificates

Changing accepted rule semantics makes earlier `VerifiedGood` totals historical.
Re-audit the **same proof bytes**, not freshly regenerated proofs, into a new
directory. Missing archives, missing includes, resource exhaustion and changed
input identities are explicit limitations, never replacement scores.

Build the current checker and tool:

```bash
nix develop -c cargo build -p mrs-proover -p mrs-bench --bin mrs-proover --bin mrs_tools
```

Replay the committed ProoVer corpus (which contains both valid and deliberately
bad proofs, so `VerifiedBad` is expected for some entries):

```bash
target/debug/mrs_tools reaudit-proofs \
  --proof-dir crates/mrs-bench/proover-corpus/Proover2026/Proofs \
  --problem-root crates/mrs-bench/proover-corpus/Proover2026 \
  --proover target/debug/mrs-proover --time 30 --memory-mb 2048 \
  --output /tmp/opencode/prv-reaudit-NEW
```

`--avatar-only` is a diagnostic text filter. `--limit N` is explicitly a subset,
not a whole-corpus audit. Source `.p` files without proof links are skipped so
an archive's nested `Problems/` is not mistaken for more proofs. Unsupported
or malformed `.s` proofs still receive an inconclusive verdict.

The tool records verifier/tool hashes, input-tree manifests, limits, logs and
incremental `report.json`. `--audit-input-identity` asks the checker to emit
digests of its **actually loaded** problem and include source strings, as well
as the proof bytes. This matters when an archived absolute header resolves
outside the nominal problem root. A positive result without that input identity
is not credited. The root manifests are checked again after replay; a changed
tree invalidates the run. The tool does not regenerate, rewrite or overwrite
archived proofs. Keep all resolved input sources immutable for the run.

Use `--previous /path/to/old/report.json` to retain the prior verdict alongside
the new one. A changed-verdict comparison requires matching proof bytes and
matching loaded-input identity; different inputs are not a checker-effect
measurement. Inspect `INVALIDATED` before treating an interrupted/changed run
as usable. No existing result directory is overwritten, and no audit result is
automatically promoted to a CASC score.

For archived campaigns with `run.csv` and raw stdout, the existing
`audit_casc_proofs --checks strict --force --output NEW_DIR` remains available.
Its identity/snapshots must also be retained. Re-auditing a small committed
canary corpus does not revise remote campaign totals whose exact archives are
unavailable locally.
