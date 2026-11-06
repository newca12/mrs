# CASC-J13 FEQ explicit-portfolio campaign (2026-11-05)

> One complete 300-row FEQ campaign using the explicit eight-worker portfolio
> `[1,2,3,4,6,8,12,13]`, with cross-strategy sharing disabled. It produced 69
> theorems, all strictly `VerifiedGood`; three other processes ended in
> `SIGKILL` with unconfirmed cause. This is a single dirty-build observation,
> not a controlled comparison or a portfolio-promotion result.

## Results and strict audit

The campaign used eight workers, `MRS_HARDWARE=casc`, sequential `jobs=1`, and
the official 180-second CASC-J13 FEQ limit (`--casc-times`). It recorded all 300
problem rows:

| SZS outcome | Count | Strict audit |
|:---|---:|:---|
| `Theorem` | **69** | **69 `VerifiedGood`** |
| `Timeout` | 160 | no refutation to check |
| `GaveUp` | 68 | no refutation to check |
| `Error` | 3 | process `SIGKILL`; no proof to check |
| **Total** | **300** | |

The archived strict audit reports 69 applicable refutations, all 69
`VerifiedGood`, with 0 `VerifiedBad`, `Unknown`, verifier timeouts, or proof
omissions. It marks 228 rows `N/A: Incomplete` and the three process-error rows
as audit `Error`. The audit identity binds the audit to the producer metadata,
`run.csv`, and audited corpus. This certifies the archived proofs; it is not a
claim of global soundness.

Summed `actual_wall_time_s` over the sequential campaign is 42,305.928 seconds
(11.75 hours). For the 69 theorems, the mean was 17.976 seconds, the median
2.189 seconds, and the range 0.156–119.094 seconds (`GEO494+1`). These are
descriptive timings from one run. One emitted proof exceeded 8,000,000 bytes:
`MGT005+1` at 8,540,047 bytes; no proof was omitted.

## Process errors and memory caveat

| Problem | Elapsed | Peak RSS | Recorded termination |
|:---|---:|---:|:---|
| `CSR036+5` | 47.313 s | 58,328 MB | exit 137, signal 9 (`SIGKILL: cause unconfirmed`) |
| `SWX209+1` | 152.543 s | 58,105 MB | exit 137, signal 9 (`SIGKILL: cause unconfirmed`) |
| `SWX211+1` | 172.663 s | 57,958 MB | exit 137, signal 9 (`SIGKILL: cause unconfirmed`) |

The host reported 95,969 MB RAM. The `casc` profile requested 131,072 MB and
reported `effective_mem_mb=81,573` with `mem_unrepresentable=1`. The maximum
campaign RSS was 58,593 MB (`CSR025+5`); the maximum virtual-memory peak was
79,137 MB (`SWX210+1`), which is not an RSS measurement. The error rows have no
kernel or cgroup evidence establishing why they received `SIGKILL`; do not
describe them as confirmed OOM kills. The host also did not provide the full
128-GiB CASC memory allowance.

## Solve-set comparisons

The following comparisons are by problem name. All three compared solve sets
were strictly certified, but the campaign runs used different dirty producer
builds and/or hosts. A set union is not a tested portfolio, and aggregate
telemetry does not attribute a solve to an individual worker strategy.

| Reference observation | Common | Nov 5 only | Reference only | Observed union |
|:---|---:|---:|---:|---:|
| Nov 3 full `casc_feq` run (72) | 68 | 1 | 4 | 73 |
| October 2026 8-core candidate `[12,11,10,6,8,1,4,13]` (69) | 66 | 3 | 6 | 75 |

Against the Nov 3 full run, the Nov 5-only solve was `GEO494+1`. The Nov 3-only
solves were `GEO277+1`, `GEO526+1`, `GEO535+1`, and `SEV677+1`. The Nov 5 run
therefore has a count difference of −3; the per-problem flips do not establish
that the portfolio caused it.

Against the October 2026 8-core candidate, the Nov 5-only solves were `SWW298+1`,
`SWB073+1`, and `SET101+1`. The October candidate-only solves were `BIO006+1`,
`SET017+1`, `SWW186+1`, `SWV406+1`, `SEV677+1`, and `GEO504+1`. Equal totals
of 69 do not mean equal coverage; these sets overlap on 66 problems.

The November 3 campaign ran on `mtsdev02` with the full `casc_feq` schedule;
this explicit portfolio ran on `mtsdev04`. The October candidate campaign ran
on `mtsdev01`. Although these hosts were eight-core Xeon E5-2407 systems, the
producers were dirty builds with different binary identities. Treat the set
differences as candidate-generation evidence, not causal estimates. Under the
repository's [experimental-design policy](../../policies/methodology.md#85-experimental-design-and-evidence-strength),
changing the shipped FEQ order still requires matched cooperative evaluation.

## Provenance

| Attribute | Value |
|:---|:---|
| Artifact directory | `~/campaign-casc-j13-feq-W8CASCHJ1-04-1_2_3_4_6_8_12_13_20261105` |
| Sibling archive | `~/campaign-casc-j13-feq-W8CASCHJ1-04-1_2_3_4_6_8_12_13_20261105.tar.bz2` |
| Run ID / metadata timestamp | `mrs_bench_20261105_182709_23cb0e3f` / `20261105_182709` (timezone not recorded) |
| Edition / division | CASC-J13 / FEQ; 300 problems |
| Portfolio | `[1,2,3,4,6,8,12,13]` |
| Producer commit / tree | `c021be580651c0bbf1cefdf3c56f05081eb6610e`, `main`, dirty |
| Producer binary SHA-256 | `5b5637d8c0d77f3feb9be5f61dab5b583f822038679425ddf8c70c70300fd3e9` |
| `run.csv` SHA-256 | `780243ccdb1945fb90bfe3ebc59b82ca0774ca32bbfaa32b7142d0dc9f8a0374` |
| `audit.csv` SHA-256 | `0045c8039d802fc1c4fe824f208554eb62ab7fb9d9fd555eb29a7b4e30790f20` |
| Strict-audit binary SHA-256 | `7a76f2d99354e51765e0df877e5f51ba560361d2c069f0ec66e33c41dc856a05` |
| Audited corpus SHA-256 | `b74bdfad012a77cb9f16227a3478c8cbf710e53191a8327dc7d35ce4597f727b` |
| Host | `mtsdev04`, Intel Xeon E5-2407 @ 2.20 GHz, 8 reported physical cores, 95,969 MB RAM |
| Search settings | `MRS_HARDWARE=casc`, 8 workers, `MRS_SHARED_POOL_INTERVAL=0`, `jobs=1` |
| Time limit | `--casc-times`; `run.log` and `run.csv` confirm 180 s for FEQ |
| Metadata default | 120 s; overridden by `use_casc_times=1` |
| Memory telemetry | `effective_mem_mb=81573`, `mem_unrepresentable=1`; 3 process errors, no `ResourceOut` rows |
| Rust toolchain | 1.99.1 |

The strict-audit identity also records producer-metadata SHA-256
`49a0b5d328329e7cf4e1f66f8852f0dfcd2c1068a38c475416f743a9664911d7` and binds
it to the run CSV and corpus above.
