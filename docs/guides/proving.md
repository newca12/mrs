# Proving Guide

## First run

```bash
nix develop -c cargo run -- problems/socrates.p
```

The expected output starts with `% SZS status Theorem`. A successful refutation
also prints a TSTP proof block.

## Reproducible diagnosis

Use one worker and one explicit strategy when comparing a strategy or
investigating a problem:

```bash
nix develop -c cargo run -- \
  --workers 1 --schedule casc_fne --strategy 11 problem.p
```

With multiple workers, strategy timing, LRS wall-clock estimates, and optional
sharing make telemetry non-deterministic. Set
`MRS_SHARED_POOL_INTERVAL=0` for an explicit no-sharing control.

## Rule-based routing

```bash
nix develop -c cargo run -- --auto-schedule problem.p
```

The current router recognizes unit equality, EPR shape, equality-free first
order, and general first-order equality. An explicit `--schedule` takes
precedence.

## Strict self-checking

```bash
nix develop -c cargo run --features proover -- \
  --self-check --workers 8 problem.p
```

Candidate refutations are checked by the independent strict kernel. A rejected
or inconclusive candidate becomes `GaveUp`, and the unverified proof is not
emitted. For stdin with includes, also pass `--include-root`.

## Bounded ordered certification

```bash
nix develop -c cargo run -- \
  --workers 1 --strategy 1 --certify-ordered problem.p
```

This is a diagnostic bounded certifier for supported function-free or ground
fragments. It is not the default portfolio and returns `GaveUp` outside its
certified fragment.

## Inspect a problem

```bash
nix develop -c cargo run -- --stats problem.p
nix develop -c cargo run -- --profile-json problem.p
```

These modes stop after lowering/clausification and print structural information
useful for schedule and benchmark diagnosis.
