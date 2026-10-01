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
| `eps-lowram-20260928-summary.tsv` | The same 42 on the same 2-core / 15 GB host after the grounding budget was changed to track memory. Confirms the memory work is coverage-neutral on the box that motivated it, and is the reference for what the box can now afford. |

The baseline for comparison is `cert-eps-20260926` in the same results tree; the
comparison table, including the +30 / 0 gained-versus-lost split and the
10× drop in mean time on solved rows, is in
[`../reports/benchmarks/eps-2026-09.md`](../reports/benchmarks/eps-2026-09.md).

Provenance: `cert-eps-20260928/run_meta.json` records `git_commit 95f7984e`,
branch `main` — the merge of `feat/eps-casc-sim-exploration` into `main`, with
all six of that branch's commits as ancestors. So this run measures the merged
result. `95f7984` also carries intervening mainline work beyond the branch tip,
and `git_dirty: true` records an incidental local modification, so the number
belongs to the merge rather than to the branch tip in isolation. The archived
streams confirm which changes ran: every solved row carries `cert_tier=2` and
`stopped=portfolio`.

## `casc-j13` UEQ, and a 150-problem regression

| file | what |
|---|---|
| `ueq-casc-j13-20260930-run.csv` | `casc-j13` UEQ, all 400 problems, official CASC-J13 180 s, 8 workers, `jobs=1`, Xeon Silver 4108 / 16 cores / `hardware=adaptive`. **107/400 solved, 107 kernel-certified, 0 reference or polarity violations.** |
| `ueq-casc-j13-20260930-audit-summary.txt` | Strict proof audit of that run: 107 `VerifiedGood`, 0 `VerifiedBad`, 0 `Unknown`, 0 `Error`. |
| `ueq-casc-j13-20260930-summary.tsv` | Derived per-family table, and the row-by-row diff against the `codex.db` CASC-J13 UEQ run. |
| `ueq-casc-j13-20260930-run_meta.json` | Provenance as recorded by the harness. Note it says `time_limit=120` while the run used 180 s — the `--casc-times` override won and the metadata field was not updated. |

Against the stored `codex.db` CASC-J13 UEQ run (reported as 257/400 in
`docs/reports/codex/status-2026-09-11.md`), this run solves 96 in common,
**loses 161**, and gains 11. On the regressed rows that carry telemetry on
both sides, median `processed_per_s` fell from 1649.5 to 116.0. The two
candidate causes are cross-strategy sharing being disabled by default
(`ce7bdc5`) and the `casc_ueq` portfolio reorder (`b214a13`); neither is
established, and the four-cell A/B that would separate them is in
[`../reports/benchmarks/ueq-2026-09.md`](../reports/benchmarks/ueq-2026-09.md) §2.

Provenance: `git_commit c07cac9` (merge of the FNE casc-sim exploration),
branch `main`, `git_dirty: true`, `binary_sha256 1a88a581…`. The dirty tree
means the SHA, not the commit, identifies the binary. The 107/400 is a
development number — `hardware=adaptive` on 16 cores is not CASC-shaped — and
must not be compared against a published CASC score.

Only the CSV, the audit summary, `run_meta.json`, and the derived `.tsv` are
banked here. The 6 MB of raw stdout/stderr under the run's `raw/mrs/ueq/` is
not, so re-running `audit_casc_proofs` against these artefacts needs the
original run directory, which lives outside any git repository. Each row's
`raw_stdout_sha256` pins the copy that was audited.
