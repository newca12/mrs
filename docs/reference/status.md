# Current Status

**Reviewed:** 2026-11-21, source baseline `bd04ae44` on `main`.
Current assurance details are in the [formal guarantee inventory](formal-guarantees.md)
and [formal implementation roadmap](../research/formal-assurance-roadmap.md).

## Implemented

- Rust 2024 workspace with the zero-copy `mrs-tptp` parser.
- FOF/CNF-oriented lowering and clausification with provenance.
- Given-clause resolution and superposition search with KBO/LPO orderings.
- AVATAR/CaDiCaL integration, FVO refutation, and lazy EPR InstGen paths.
- Discrimination/path-compressed, literal, and feature-vector indexing
  (not Graf-style substitution-tree indexing).
- Demodulation, subsumption, subsumption resolution, condensation, BCE, and
  PLE.
- Parallel named schedules with 15 active base strategies plus a diagnostic
  slot in the generic schedule.
- Opt-in proof-carrying unit-equality sharing between sibling workers.
- Strict proof self-checking and the independent `mrs-proof-kernel`.
- Standalone `mrs-proover` strict and competition verification modes.
- Finite model certificate data structures and validation in the proof kernel.
- Verus-verified supported decoded-IR replay/DAG and wire/token checks, with
  explicit shadow/dual checking through a trusted TPTP bridge; standalone
  normalization lemmas are not all dispatched.
- CASC and ProoVer harnesses with raw-output preservation and telemetry.

## Important boundaries

- Ordinary heuristic search is refutation-oriented. It must not treat timeout,
  pruning, restricted inference, or subset saturation as a satisfiability proof.
- ML schedule classification is retired from normal routing. Use
  `--auto-schedule` for deterministic rule-based routing.
- `casc_*` priority orders are benchmark-derived candidates. Solo set-cover
  coverage is not cooperative portfolio coverage.
- `--certify-ordered` covers a bounded fragment and returns `GaveUp` outside it.
- Model certificates are validated separately; the normal `mrs` output path is
  not a general finite-model generator.
- Strict self-check is off by default; the ordinary proof/model kernels are
  runtime checks, not formally verified implementations. The formal carrier
  repair does not prove arbitrary-carrier transport or original-TPTP semantics.

## Evidence status

There is no clean full benchmark and proof-audit baseline recorded for this
commit in the repository. Existing benchmark and audit numbers are historical
reports under [`reports/`](../reports/) or the append-only log under
[`history/benchmark-log.md`](../history/benchmark-log.md). Do not quote them as
current performance without rerunning the documented command and recording the
new artifact.
