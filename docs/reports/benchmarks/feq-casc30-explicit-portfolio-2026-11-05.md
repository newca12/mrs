# CASC-30 FEQ explicit-portfolio campaign (2026-11-05)

> This complete 400-row run evaluated `[1,2,6,8,10,11,12,15]` with eight
> workers and no cross-strategy sharing. It returned 110 raw `Theorem` statuses,
> but the bundled strict audit certified 96. A November 6 review-time replay
> certified 98 against a different local corpus digest; it does not revise the
> archive's 96/400 result. The producer tree was dirty, the run log warns the
> prover binary was stale, and three processes ended in cause-unconfirmed
> `SIGKILL`s. This is a supplemental single-run observation, not a promotion
> result.

## Search results and bundled audit

The campaign covered the full CASC-30 FEQ division sequentially (`jobs=1`) at
the official 240-second limit:

| SZS outcome | Count | Bundled strict audit |
|:---|---:|:---|
| `Theorem` | **110** | 96 `VerifiedGood`; 0 `VerifiedBad` |
| `Timeout` | 238 | no refutation to check |
| `GaveUp` | 49 | no refutation to check |
| `Error` | 3 | process `SIGKILL`; cause unconfirmed |
| **Total** | **400** | |

The archived audit used a 120-second per-proof limit and reports one
`Unknown` (`COM149+1`), one verifier `Timeout` (`ALG049+1`), and 12 `no_proof`
rows, in addition to the 96 `VerifiedGood` proofs. It reports no
`VerifiedBad` proof. The 12 `no_proof` rows are:

`KLE071+1`, `KLE113+1`, `KLE132+1`, `LCL894+1`, `LCL895+1`, `LCL896+1`,
`LCL901+1`, `NUM835+1`, `NUM839+2`, `NUM852+1`, `NUM853+2`, and `NUM926+2`.

Their raw stdout files contain empty `% SZS output start/end Proof` blocks,
not derivations. The audit classifies these as `no_proof` (shown as `Other` in
its summary); they must not be counted as certified solves. The explicit stale
binary warning in `run.log` makes this archive unsuitable as evidence of the
current proof-emission path.

### November 6 review-time replay

The two archived audit exceptions were rechecked with the current release
`audit_casc_proofs` built from workspace commit `c1e2b26c6665d8f356584ddbdc0c7c3a548ae61a`
using `--strict-time 240 --jobs 4`: `COM149+1` and `ALG049+1` both returned
`VerifiedGood` (ALG049 took 215.64 s). Replaying the same campaign `run.csv` and
producer metadata against the local CASC-30 corpus yielded 98 `VerifiedGood`, 0
`VerifiedBad`, 0 `Unknown`, 0 verifier timeouts, and the same 12 `no_proof` rows.

This is supplemental, not an identity-matched replacement for the bundled
audit: the archived audit identity records corpus SHA-256
`bb153a22fd4e330e035386104d3cecfea03d998e224282795f83ef51233b6f16`, while
the November 6 replay used `9a9cabd1a059f737c56353e01f7008597b96de7a312d0fb69559d8e15d77570a`.
The run metadata itself does not record a corpus digest. The review replay
therefore does not change the archived 96/400 strict result or establish that
the campaign ran on the replayed corpus.

## Process terminations and resource caveat

| Problem | Axioms | Clauses | Elapsed | Peak RSS | Recorded termination |
|:---|---:|---:|---:|---:|:---|
| `CSR037+5` | 540,249 | 540,248 | 49.662 s | 44,557 MB | exit 137, signal 9 |
| `CSR047+5` | 540,249 | 540,248 | 50.106 s | 44,295 MB | exit 137, signal 9 |
| `HWV128+1` | 204,845 | 580,583 | 111.822 s | 44,153 MB | exit 137, signal 9 |

All three have empty stdout, but the archive contains no kernel or cgroup record
that establishes why they received `SIGKILL`; do not label them confirmed OOM
kills. `CSR047+5` and `HWV128+1` also appear in the earlier
[FEQ silent-kills investigation](feq-silent-kills-investigation.md), which is
useful context but does not diagnose these terminations.

The host reported 95,969 MB RAM. `MRS_HARDWARE=casc` requested 131,072 MB and
reported `effective_mem_mb=81,573` with `mem_unrepresentable=1`, below the
canonical 128-GiB allowance. Maximum observed process RSS was 44,557 MB
(`CSR037+5`).

## Timings

The sum of `actual_wall_time_s` over the sequential 400-row campaign was
70,888.234 seconds (19.69 hours). For the 110 raw theorem statuses, mean time
was 19.088 seconds, median 3.140 seconds, and range 0.185 seconds
(`COM125+1`) to 212.877 seconds (`GEO331+1`). These are descriptive statistics
from this run, not a speed comparison.

## Solve-set comparisons

These are comparisons of raw `Theorem` problem-name sets, not strict-certified
sets. The campaigns used different dirty producer trees and binaries; the
November 5 binary was explicitly reported stale. The audits also used different
corpus digests, so do not interpret the differences as a controlled portfolio
effect.

| Reference observation | Raw theorems | Common | Nov 5 only | Reference only |
|:---|---:|---:|---:|---:|
| Nov 3 full CASC-30 campaign, FEQ rows | 112 | 107 | 3 | 5 |
| October 8-core candidate `[12,11,10,6,8,1,4,13]` | 111 | 105 | 5 | 6 |

Against the Nov 3 run, the Nov 5-only problems were `GEO559+1`, `GEO641+1`, and
`SWV457+1`. The Nov 3-only problems were `GEO340+1`, `GRP776+1`, `HWV090+1`,
`KLE170+1.002`, and `NUN086+2`.

Against the October 8-core candidate, the Nov 5-only problems were `GEO559+1`,
`GEO641+1`, `NUM926+2`, `PRO012+1`, and `SWV459+1`. The October-only problems
were `BIO005+1`, `BIO006+1`, `GEO271+1`, `GRP776+1`, `SET017+1`, and
`SET072+1`. In particular, `NUM926+2` is a raw candidate-only theorem with no
proof body in the bundled artifact, so it is not a certified gain.

The candidate replaces strategies 4 and 13 from the current `casc_feq`
eight-worker prefix `[12,11,10,6,8,1,4,13]` with strategies 2 and 15, and
changes their order. The raw count is below both comparison observations, and
the proof audit has 12 missing derivations. Under the repository's
[experimental-design policy](../../policies/methodology.md#85-experimental-design-and-evidence-strength),
this evidence does not justify changing the shipped schedule; a matched,
randomized cooperative comparison and held-out evaluation remain necessary.

## Provenance

| Attribute | Value |
|:---|:---|
| Artifact directory | `/home/hack/campaign-casc-30-feq-W8CASCHJ1-03-1_2_6_8_10_11_12_15_20261105` |
| Sibling archive | `/home/hack/campaign-casc-30-feq-W8CASCHJ1-03-1_2_6_8_10_11_12_15_20261105.tar.bz2` |
| Run ID / timestamp | `mrs_bench_20261105_182301_1cbe91ce` / `20261105_182301` |
| Edition / division | CASC-30 / FEQ; 400 problems |
| Portfolio | `[1,2,6,8,10,11,12,15]` |
| Search settings | `MRS_HARDWARE=casc`, 8 workers, `MRS_SHARED_POOL_INTERVAL=0`, `jobs=1` |
| Limit | `--casc-times`; 240 s in `run.csv` and `run.log` (metadata default 120 s) |
| Producer host | `mtsdev03`, Intel Xeon E5-2407 0 @ 2.20 GHz, 8 cores, 95,969 MB RAM |
| Producer tree | `c021be580651c0bbf1cefdf3c56f05081eb6610e`, `main`, dirty |
| Producer binary SHA-256 | `e0f113ec51a5b32872006a7350658d47898c6e6a897e5272c9e2a5575ca516eb` |
| `run.csv` SHA-256 | `48ff12b4a8960261c6c147b729c6427e79a4c2ebbe51fc830a06dba745d96b65` |
| Bundled `audit.csv` SHA-256 | `30e38a0fc5bb9ddb767f69c1eb16111a4f8e6b1793061f281b3ce9021f954e32` |
| Bundled strict-audit binary SHA-256 | `6cd6998d2364bbf233fcda0e51ca9f6c4783aa9630a121e24e6118efed46dbdf` |
| Bundled audit corpus SHA-256 | `bb153a22fd4e330e035386104d3cecfea03d998e224282795f83ef51233b6f16` |
| Review-time audit settings | strict, `--strict-time 240`, `--jobs 4`, source commit `c1e2b26c6665d8f356584ddbdc0c7c3a548ae61a` |
| Review-time strict-audit binary SHA-256 | `8f8c08ecb5c5554f343c76d9225d8c9d1fb028ea9f9c72243b848c8a4893124c` |
| Review-time `audit.csv` SHA-256 | `cf8784af08b2283591e95feaf25d3ffa624e33a188de0a45813194694b53a042` |
| Review-time corpus SHA-256 | `9a9cabd1a059f737c56353e01f7008597b96de7a312d0fb69559d8e15d77570a` |
| Rust toolchain | `rustc 1.99.1 (58cf8f9c3 2026-10-13)` |

`run.log` warns that the release prover is older than its sources. The bundled
audit identity binds the audit to producer-metadata SHA-256
`890302eb99d71f69379c33c5a879ad19c6b69c547a17b732a46b145c6367700f` and the
`run.csv` hash above. The review-time replay used the same run CSV and producer
metadata but a different corpus digest; retain the archived and review-time
results as distinct observations.
