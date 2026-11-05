# CASC-J13 UEQ explicit-portfolio campaign (2026-11-04)

> A complete 400-problem UEQ run with the explicit no-sharing portfolio
> `[1,2,3,5,7,8,12,15]` produced 106 refutations, all 106 `VerifiedGood` under
> the strict audit. Against the November 3 full run, 101 problems were common,
> five were solved only in this run, and 42 from November 3 were not reproduced.
> This is a single exploratory observation, not a controlled portfolio
> comparison or a promotion result.

## Results

The campaign completed all 400 CASC-J13 UEQ problems sequentially (`jobs=1`)
under the official 180-second limit:

| SZS outcome | Count |
|:---|---:|
| `Unsatisfiable` | **106** |
| `GaveUp` | 19 |
| `Timeout` | 275 |
| `Error` / signal | 0 |
| **Total** | **400** |

The strict audit (`checks=[strict]`) reports 106 applicable refutations, all
106 `VerifiedGood`, with 0 `VerifiedBad`, `Unknown`, verifier timeouts, or proof
omissions. The remaining 294 rows are `N/A: Incomplete`. All 400 processes
exited successfully; the archived rows contain no `ResourceOut` result.

The sum of `actual_wall_time_s` was **56,345.431 s** (15.65 h). For the 106
refutations, the mean was **32.997 s**, the median **12.832 s**, and the range
**0.144–173.848 s** (`COL002-4` to `REL050-1`). The largest observed process
RSS was **48,025 MB** (`RNG021-6`); the largest virtual address space was
**72,117 MB** (`LAT150-1`).

### Comparison with the November 3 full UEQ run

The November 3 full CASC-J13 campaign strictly certified 143/400 UEQ problems.
Its UEQ telemetry identifies strategy order `[4,8,12,11,2,14,15,1]`; this
campaign configured `[1,2,3,5,7,8,12,15]`.

| Strictly certified set comparison | Count |
|:---|---:|
| Certified in both runs | 101 |
| November 3 only | 42 |
| November 4 only | 5 |
| November 4 total | 106 |
| Net difference from November 3 | **−37** |

The five November 4-only problems are `GRP178-2`, `LAT168-1`, `LAT392-1`,
`LCL400-10`, and `REL025-1`. The 42 November 3-only problems are `BOO023-1`,
`COL042-10`, `DAT436-1`, `DAT436-2`, `GRP167-3`, `GRP667-2`, `GRP667-3`,
`GRP727-1`, `GRP752-1`, `GRP766-1`, `KLE084-10`, `KLE108-10`, `KLE147-10`,
`KLE152-10`, `KLE169-10`, `LAT017-1`, `LAT022-1`, `LAT085-1`, `LAT142-1`,
`LAT152-1`, `LAT153-1`, `LAT157-1`, `LAT174-1`, `LAT390-1`, `LCL042-10`,
`LCL265-10`, `LCL897-10`, `MVA009-1`, `REL019-1`, `REL021-1`, `REL026-3`,
`REL027-1`, `REL027-3`, `REL031-1`, `REL034-1`, `REL034-2`, `REL035-1`,
`REL036-1`, `REL041-2`, `REL050-3`, `RNG001-10`, and `SYN708-10`.

These are comparisons of strictly certified problem sets. Both runs used eight
workers, `MRS_HARDWARE=casc`, the official 180-second limit, and telemetry with
no shared clauses published or imported. They are nevertheless not a
schedule-only comparison: the November 3 run was produced on `mtsdev02` from a
different dirty source tree and binary, while this run was produced on
`mtsdev04`. Timing-sensitive search, host, and producer identity were not
controlled or randomized.

### Memory and hardware caveat

The host reported 95,969 MB RAM. The `casc` profile warned that this was below
its 128 GiB allowance and reported `effective_mem_mb=81,573` and
`mem_unrepresentable=1`. No process error or resource-out was recorded, but the
configured allowance exceeded the host's available memory; keep this limitation
visible when comparing runs.

## Configuration and provenance

| Attribute | Value |
|:---|:---|
| Artifact directory | `~/campaign-casc-j13-ueq-W8CASCHJ1-04-1_2_3_5_7_8_12_15_20261104` |
| Sibling archive | `~/campaign-casc-j13-ueq-W8CASCHJ1-04-1_2_3_5_7_8_12_15_20261104.tar.bz2` |
| Run ID / metadata timestamp | `mrs_bench_20261104_190458_ceb821ae` / `20261104_190458` (timezone not recorded) |
| Producer commit / tree | `c021be580651c0bbf1cefdf3c56f05081eb6610e`, `main`, dirty |
| Producer binary SHA-256 | `5b5637d8c0d77f3feb9be5f61dab5b583f822038679425ddf8c70c70300fd3e9` |
| `run.csv` SHA-256 | `997c8638a4b2b11c848f1a12bf4fba9bd43204d2ec6d1eb753d7d37e2e02c3e6` |
| `audit.csv` SHA-256 | `8d0e343a8b7120dd1124648666712b45d2ae708437c5aeb52d94608fd7055dc4` |
| Strict audit binary SHA-256 | `7a76f2d99354e51765e0df877e5f51ba560361d2c069f0ec66e33c41dc856a05` |
| Audited corpus SHA-256 | `b74bdfad012a77cb9f16227a3478c8cbf710e53191a8327dc7d35ce4597f727b` |
| Host | `mtsdev04`, Intel Xeon E5-2407 @ 2.20 GHz, 8 reported physical cores, 95,969 MB RAM |
| Search configuration | `MRS_HARDWARE=casc`, `MRS_WORKERS=8`, `MRS_PORTFOLIO=1,2,3,5,7,8,12,15`, `MRS_SHARED_POOL_INTERVAL=0`, `jobs=1` |
| Time limit | `--casc-times`; `run.log` confirms official 180 s for UEQ |
| Metadata default | 120 s; overridden by `use_casc_times=1` |
| Rust toolchain | 1.99.1 |

Although `run_meta.txt` labels the schedule `auto`, the explicit portfolio is
recorded in `run_meta.json` and the strict-audit identity. Per-row telemetry
shows that order when all strategy slots appear; the fast refutation of
`CSR072-10` completed before the final two IDs appeared in its detail. The audit
identity binds the strict check to the producer metadata, `run.csv`, corpus, and
audit binaries. The producer tree is dirty, so the binary digest is the precise
producer identity for this observation.

## Interpretation

This experiment does not support replacing the November 3 UEQ portfolio: it
strictly certified 37 fewer problems and failed to reproduce 42 solves from
that run, despite solving five different problems. The clean proof audit
establishes that the 106 emitted refutations are valid; it does not establish a
portfolio improvement. Different dirty builds and hosts, along with a single
chronological run per configuration, prevent attributing the set changes to
strategy selection alone.

Keep the November 3 full-campaign result as the latest full-campaign observation.
Treat the five November 4-only solves as candidates for a controlled follow-up,
not evidence for schedule promotion. Follow the randomized paired-campaign and
replication guidance in
[`methodology.md` §8.5](../../policies/methodology.md#85-experimental-design-and-evidence-strength).
