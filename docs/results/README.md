# Banked benchmark results

Dated, per-run artefacts. These are evidence, not current baselines: a number
here describes the commit and configuration named in the file that produced it.
Re-measure before quoting one.

| file | what |
|---|---|
| `eps-casc-sim-20260928-summary.tsv` | `casc-30` EPS, certified track, 120 s CASC limit, `MRS_HARDWARE=casc-sim`, `MRS_WORKERS=2`. 42/100 solved, 0 reference violations, 0 invalid models, 38/42 models kernel-certified. Interpretation in [`../reports/benchmarks/eps-2026-09.md`](../reports/benchmarks/eps-2026-09.md). |

Columns are `problem`, `szs_status`, `cert_tier`.
