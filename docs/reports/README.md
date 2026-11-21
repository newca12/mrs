# Reports

Reports are dated evidence. They are useful for understanding decisions and
regressions, but no report is the current performance baseline unless it names
the current commit and has been rerun under current controls.

- [Paper implementation repairs](audits/paper-implementation-repairs-2026-11.md):
  AVATAR/conjecture assumption soundness, BCE/index repairs and stored-proof
  re-audits; no whole-prover completeness or score-promotion claim.
- [Term traversal and index cleanup](benchmarks/term-transforms-2026-11-21.md):
  lazy-copy transformations, exact-order retrieval tests and identity-bound
  fixed-work diagnostics; instruction reduction with mixed wall-clock results.

| Area | Contents |
|---|---|
| `benchmarks/` | Dated division and benchmark summaries. |
| `audits/` | Soundness and methodology audits. |
| `prephase/` | Pre-phase studies and solo-matrix strategy sweeps. |
| `proover/` | ProoVer scores, verifier studies, and harness history. |
| `codex/` | Database/profile analyses. |
| [`../results/`](../results/) | Raw or summarized result artifacts. |
