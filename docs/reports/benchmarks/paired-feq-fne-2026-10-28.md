# Paired FEQ Portfolio Screen with FNE Control (CASC-30, 2026-10-28–11-05)

> **Status:** Four of five planned randomized pairs have complete, strictly
> audited arms. Across the four completed pairs, the candidate's paired
> certified delta is **+2, +1, 0, +1** (mean **+1.0**, observed range **0 to
> +2**). This is a descriptive screen, not an official score, confidence
> interval, or evidence that the portfolio is optimal. Pair 5 is incomplete in
> the archived snapshot and is excluded.
>
> Artifacts: `~/crates/mrs-bench/results/paired-feq-fne-20261028/`

## Question and setup

The frozen baseline and candidate artifacts used different FEQ strategy orders
while holding the FNE order constant. Each arm ran the full 400-problem FEQ and
100-problem FNE sets, sequentially (`jobs=1`), with eight workers and no shared
equality pool.

| Setting | Value |
|---|---|
| Edition and divisions | CASC-30; FEQ (400) and FNE (100) |
| Search limit | Official CASC-30 limits (`--casc-times`): 240 s for both divisions |
| Schedule selection | Automatic division schedule; no explicit schedule override |
| Hardware mode | `MRS_HARDWARE=casc`, 8 workers, `jobs=1` |
| Shared pool | Disabled (`MRS_SHARED_POOL_INTERVAL=0`) |
| Strict audit | 120 s per proof |
| Planned pairs / seed | 5 / 42 |
| Baseline FEQ order | `11,12,1,6,10,8,14,4` |
| Candidate FEQ order | `12,11,10,6,8,1,4,13` |
| FNE order in both arms | `11,13,4,8,15,7,3,1` |

All eight completed arms ran on `mtsdev03` (8 physical cores, Intel Xeon E5-2407
at 2.20 GHz, 95,969 MB host RAM). The `casc` profile reported a 131,072 MB
allowance but only 81,573 MB effective memory (`mem_unrepresentable=1`); this
was an eight-core run, but not a full 128-GB-memory match to the canonical
hardware allowance.

The manifest records baseline binary SHA-256
`e0f113ec51a5b32872006a7350658d47898c6e6a897e5272c9e2a5575ca516eb` and
candidate binary SHA-256
`09ab44efbc82f5bc97e057bd5fdedd22a67997114c902fb78c34a68da96b2326`. Both
arm metadata files name commit `df40f564` and `git_dirty=true`. Every arm log
also warns that its frozen binary is older than the checked-out sources. The
campaign records and checks the binary hashes, but the dirty source trees and
absence of source snapshots mean the exact source diff between the binaries
cannot be reconstructed from this artifact alone. The strict-auditor and
ProoVer hashes are recorded in `experiment.json`: auditor
`dacb2480935ec3d18b2c9cff3791837c772e9b1f1073bdd88bd48634bf49ba1a`, ProoVer
`7ca70120e67272edf2e327b71a51a009c83b9daff1cece23e24bdfb13cd9755a`. The
recorded Rust compiler was 1.99.1.

The manifest's randomized arm orders are baseline→candidate, candidate→baseline,
baseline→candidate, baseline→candidate, candidate→baseline. Pairs 1–4 completed
on Oct 28–Nov 4. At the review snapshot, pair 5 had only partial candidate-arm
output, with no baseline arm, strict audit, or paired summary. It is not
included below.

## Paired certified results

All counts below are strict `VerifiedGood` results, not raw `Theorem` statuses.
Each pair covers the same 500 problem/system keys in both arms.

| Pair | Order | FEQ baseline → candidate | FNE baseline → candidate | Total baseline → candidate | Candidate gains / baseline-only | Delta |
|---:|:---:|---:|---:|---:|---:|---:|
| 1 | B → C | 92 → 94 | 44 → 44 | 136 → 138 | 2 / 0 | **+2** |
| 2 | C → B | 93 → 94 | 44 → 44 | 137 → 138 | 2 / 1 | **+1** |
| 3 | B → C | 93 → 93 | 44 → 44 | 137 → 137 | 1 / 1 | **0** |
| 4 | B → C | 93 → 94 | 44 → 44 | 137 → 138 | 2 / 1 | **+1** |

The mean paired delta is **+1.0 certified problem per 500-problem pair** and the
observed range is **[0, +2]**. These four observations do not constitute a
confidence interval or a division-wide noise estimate. FNE had the same 44
certified problems in every arm; there were no paired FNE flips. Because the
FNE order is identical in both binaries, this is a control for the FEQ
comparison, not a test of the candidate FNE schedule against a different FNE
baseline.

### Where the paired FEQ flips occurred

Only three FEQ problems changed strict certified status between the two arms
in any completed pair:

| Problem | Baseline certified | Candidate certified | Observed behavior |
|---|---:|---:|---|
| `GRP776+1` | 0/4 | 4/4 | Candidate solved in 4.67–4.72 s; baseline timed out in all four pairs. |
| `SWV459+1` | 0/4 | 3/4 | Candidate solved in 23.94–24.18 s in three pairs and timed out in pair 3; baseline timed out in all four. |
| `SWV458+1` | 3/4 | 0/4 | Baseline solved in 25.96–26.54 s in three pairs; candidate timed out in all four. |

Thus `GRP776+1` is the repeatable gain, while the other two form a portfolio
tradeoff: the candidate usually finds `SWV459+1` but gives up `SWV458+1` in
these runs. The observed net is small because the recurring gains and loss
largely offset; it is not evidence of a broad FEQ coverage increase.

## Strict-audit accounting

Across the four complete pairs (eight full 500-row arms), the audit saw 1,220
generated `Theorem` statuses. Of those, **1,098 were `VerifiedGood` and none
were `VerifiedBad`**. These are repeated observations across runs, not 1,220
distinct problems.

| Division / arm | Generated `Theorem` | `VerifiedGood` | Other theorem audit outcomes |
|---|---:|---:|---|
| FEQ baseline (4 arms) | 427 | 371 | 4 `Unknown`, 4 verifier timeouts, 48 `no_proof` |
| FEQ candidate (4 arms) | 441 | 375 | 10 `Unknown`, 4 verifier timeouts, 4 proof omissions, 48 `no_proof` |
| FNE baseline (4 arms) | 176 | 176 | None |
| FNE candidate (4 arms) | 176 | 176 | None |

The 12 FEQ `no_proof` statuses occurred in every arm: `KLE071+1`,
`KLE113+1`, `KLE132+1`, `LCL894+1`, `LCL895+1`, `LCL896+1`, `LCL901+1`,
`NUM835+1`, `NUM839+2`, `NUM852+1`, `NUM853+2`, and `NUM926+2`. The candidate
also omitted `GEO559+1` in each arm because its 55.5–57.7 MB proof exceeded the
32 MB proof-output limit. The strict verifier timed out on `ALG049+1` in each
FEQ arm. These outputs are not counted as certified.

Every complete arm also recorded two FEQ errors (`CSR037+5`, `CSR047+5`) and
three FNE errors (`NLP260+1`, `NLP261+1`, `NLP262+1`), all reported as
`SIGKILL: cause unconfirmed`. The data does not establish that memory caused
these kills. They were errors, not certified solves or `VerifiedBad` proofs.

## Interpretation and limitations

- The paired screen is positive in three of four completed blocks, but the
  mean advantage is one certified problem and the per-problem churn is
  concentrated in three FEQ instances. This is evidence for a modest observed
  difference, not a claim of statistical significance or portfolio optimality.
- The candidate was selected using CASC-30 FEQ solo-sweep evidence, so this
  comparison is not a held-out evaluation of that selection. The frozen
  binaries are from dirty trees, and the host's effective memory was below the
  128-GB allowance. These limits weaken source attribution and broad
  generalization.
- Four complete pairs satisfy an initial screen; the planned fifth pair is
  incomplete. Its partial candidate output must not be treated as a result.
- The repeated FNE result confirms that both arms produced the same 44/100
  certified FNE solves under the same FNE schedule; it does not validate the
  earlier FNE candidate-versus-shipped comparison.

Use the per-arm `run.csv`, `run_meta.json`, raw output, and
`strict-audit/audit.csv` files alongside `experiment.json` and
`paired-summary.json` for the underlying records. Do not combine counts across
replicates as if they were distinct problems.
