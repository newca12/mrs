# Banked benchmark results

Dated, per-run artefacts. These are evidence, not current baselines: a number
here describes the commit and configuration named in the file that produced it.
Re-measure before quoting one.

## `casc-30` EPS, before and after the 2026-09 tier-routing work

| file | what |
|---|---|
| `eps-casc-sim-20260928-run.csv` | `casc-30` EPS through `casc.sh --divisions eps --casc-times`, 120 s, CASC 8-worker dual split (`workers=7+1`), 32-core Xeon / 128 GB. **42/100 solved, 0 reference violations, 0 invalid models.** |
| `eps-casc-sim-20260928-audit-summary.txt` | The `audit_casc_proofs` report for that run: 38 of the 42 models kernel-certified, 0 invalid, 4 `Inconclusive` on the kernel's evaluation work cap. |
| `eps-casc-sim-20260928-summary.tsv` | A local re-measurement of the same division, certified track alone, on a 2-core host under `casc-sim`. Same 42 solved. Kept because it carries the refusal-reason telemetry (`cert_tier`) per problem. |

The baseline for comparison is `cert-eps-20260926` in the same results tree; the
comparison table, including the +30 / 0 gained-versus-lost split and the
10× drop in mean time on solved rows, is in
[`../reports/benchmarks/eps-2026-09.md`](../reports/benchmarks/eps-2026-09.md).

Provenance caveat: `cert-eps-20260928/run_meta.json` records `git_commit
95f7984e`, branch `main`, `git_dirty true` — not a clean tree of this branch.
The archived streams do prove which changes it exercised (every solved row
carries `cert_tier=2` and `stopped=portfolio`), so the run covers this work, but
it should be quoted as a dirty-tree run and not as a build of `37d411c`.
