# Formal assurance audit evidence — 2026-11-21

Baseline source: `bd04ae44`. This evidence belongs to the
[formal guarantee audit](../../reports/audits/formal-guarantees-2026-11-21.md),
[current guarantee inventory](../../reference/formal-guarantees.md) and
[F0–F8 implementation roadmap](../../research/formal-assurance-roadmap.md).
It is not a competition score or end-to-end proof of MRS correctness.

- `baseline-identity.json` binds the original core and support-manifest bytes
  and records their `Set<u64>` / machine-value semantic restriction.
- `source-identities.sha256` binds the repaired formal core, other formal
  modules, manifest/pin/gate, changed host tooling/bridge tests and lockfile.
- `focused-core-verification.log` records the pinned 578-check core pass with
  zero errors, including the new infinite successor model obligation.
- `cargo-verification.log.gz` records all four passed required Nix-wrapped
  workspace gates and doctests.
- `verus-verification.log` records the complete successful proof/runtime/real
  bridge/mutation gate. The runtime hash is
  `2923eea3c15679d861ef706c1cee4f57ec1e40ec0f89ed78a64f9e30b971d401`.

The complete Cargo and pinned formal gates pass. The formal run explicitly
enables actual bridge tests (`MRS_VERUS_BRIDGE_TESTS=1`), uses two verifier
threads and two mutation workers, and keeps the existing
bounded 900-second runtime-mutation compile allowance. SMT and cumulative-work
limits, required rejection semantics and the 58/50 mutation inventories are
unchanged.

The verifier log names parent commit `bd04ae44`; the checked tree contained the
uncommitted repair, so that parent alone is not the source identity. The SHA-256
ledger binds the exact checked files. Independent-runner CI was not exercised
by this local gate and is not promoted to established evidence.

The repair admits nonempty `ISet<int>` domains of arbitrary finite size and
countably infinite size in the main semantic theorem. It does not prove generic
carrier transport, TPTP parsing/lowering, FOF/AVATAR/model evaluation, dynamic
completeness or wall-clock/resource/TCB correctness. All production-certification
flags remain false; the small bridge corpus remains a regression corpus.
