# ProoVer Guide

`mrs-proover` verifies TPTP/TSTP refutation proofs and finite model
certificates. It reports `VerifiedGood`, `VerifiedBad`, or `Unknown`.

## Build and basic usage

```bash
nix develop -c cargo build --release -p mrs-proover
nix develop -c cargo run --release -p mrs-proover -- proof.p
```

The proof should contain a `% Proof : ...` header pointing to its problem. Use
`--problems-dir DIR` or `TPTP` when the linked path needs a search root.

## Verification policies

```bash
# Independent kernel only. No MRS search or external ATPs.
nix develop -c cargo run --release -p mrs-proover -- \
  --strict --problems-dir Problems proof.p

# Competition-mode ladder with a bounded budget.
nix develop -c cargo run --release -p mrs-proover -- \
  --time 30 --workers 8 --problems-dir Problems proof.p

# Force one backend.
nix develop -c cargo run --release -p mrs-proover -- \
  --only-mrs --problems-dir Problems proof.p
```

Competition mode uses internal structural checks and may use the in-process
MRS backend, E, Vampire, and Vampire-FMB if available. It is broader than the
strict kernel and should not be described as the strict self-check trust
boundary.

## Deterministic corpora

The committed ProoVer corpus is under
`crates/mrs-bench/proover-corpus/Proover2026`:

```bash
nix develop -c cargo run -p mrs-bench --bin validate_proover2026 -- \
  crates/mrs-bench/proover-corpus/Proover2026

nix develop -c cargo run --release -p mrs-bench --bin score_proover2026 -- \
  crates/mrs-bench/proover-corpus/Proover2026 \
  --competition --proover target/release/mrs-proover \
  --time 10 --workers 8 \
  --output reports/proover.tsv
```

Use `--official` to exclude the panel-removed problems defined by the scorer.
The scorer exits non-zero for unknown, false-rejection, or unsound outcomes;
inspect the printed summary before treating a run as a score claim.

## Regression harnesses

```bash
crates/mrs-bench/verify_proover_corpus.sh
crates/mrs-bench/zenodo_benchmark.sh --dataset PyRes
crates/mrs-bench/proover_compare.sh --help
```

See [Trust and verification](../reference/trust-and-verification.md) for the
strict contract and `reports/` for dated benchmark evidence.
