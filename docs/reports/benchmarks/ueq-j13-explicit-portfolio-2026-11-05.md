# CASC-J13 UEQ explicit-portfolio campaign (2026-11-05)

> The explicit no-sharing portfolio `[1,2,5,7,8,12,14,15]` completed all 400
> CASC-J13 UEQ problems and strictly certified **119/400** refutations. Against
> the November 3 shipped-order observation (143/400), 116 solves are common,
> three are candidate-only, and 27 November 3 solves were not reproduced. This
> is a supplemental single-run observation, not a promotion result; the campaign
> log warns that its release binary was older than the sources.

## Results and strict audit

The campaign completed all 400 problems sequentially (`jobs=1`) with eight
workers, `MRS_HARDWARE=casc`, cross-strategy sharing disabled, and the official
180-second CASC-J13 UEQ limit (`use_casc_times=1`):

| SZS outcome | Count | Strict audit |
|:---|---:|:---|
| `Unsatisfiable` | **119** | **119 `VerifiedGood`** |
| `GaveUp` | 15 | no refutation to check |
| `Timeout` | 266 | no refutation to check |
| `Error` / `ResourceOut` | 0 | — |
| **Total** | **400** | |

The archived strict audit (`checks=[strict]`) reports 119 applicable refutations,
all 119 `VerifiedGood`, with no `VerifiedBad`, `Unknown`, verifier timeouts, or
proof omissions. The other 281 rows are `N/A: Incomplete`. All 400 processes
exited with code 0 and no termination signal. This certifies the emitted proofs
in this artifact; it is not a global soundness claim.

The sum of `actual_wall_time_s` was **55,118.755 s** (15.31 h). Among the 119
refutations, mean time was **38.641 s**, median **12.967 s**, and the range was
**0.100 s (`COL002-4`)–178.174 s (`LAT096-1`)**. The largest recorded process
RSS was **45,709 MB** and the largest virtual address space was **69,338 MB**,
both on `GRP164-2`.

## Comparison with the November 3 shipped-order observation

The November 3 full CASC-J13 campaign strictly certified 143/400 UEQ problems;
its telemetry identifies the shipped order `[4,8,12,11,2,14,15,1]`. The
November 5 candidate used `[1,2,5,7,8,12,14,15]`.

| Strictly certified problem sets | Count |
|:---|---:|
| Certified in both | 116 |
| November 5 only | 3 |
| November 3 only | 27 |
| November 5 total | 119 |
| November 3 total | 143 |
| Union of the two observed sets | 146 |
| Net difference (November 5 vs. November 3) | **−24** |

**November 5 only:** `LAT168-1`, `LCL400-10`, `MVA011-1`.

**November 3 only:** `COL042-10`, `DAT436-1`, `DAT436-2`, `GRP167-3`,
`GRP727-1`, `GRP766-1`, `KLE108-10`, `KLE147-10`, `KLE152-10`, `KLE169-10`,
`LAT017-1`, `LAT076-1`, `LAT078-1`, `LAT142-1`, `LAT152-1`, `LAT153-1`,
`LAT157-1`, `LCL042-10`, `LCL265-10`, `REL021-1`, `REL024-2`, `REL034-1`,
`REL034-2`, `REL035-1`, `REL036-1`, `REL041-2`, and `SYN708-10`.

By portfolio membership, this candidate keeps six IDs from the shipped order
(`1,2,8,12,14,15`), removes `4,11`, and adds `5,7`. The problem-set churn
demonstrates different observed coverage, not that any one strategy caused a
solve. This candidate did not reproduce 27 of the shipped-order solves and is
24 lower by count.

The November 3 campaign was produced from a different dirty tree and binary on
`mtsdev02`; this run is from dirty commit `c021be5` on `mtsdev01`. Both hosts
reported eight physical cores of the same Xeon E5-2407 model, but they are
different machines and neither supplied the full 128-GiB memory allowance.
Treat the set difference as an archive comparison, not a controlled schedule
effect.

## Comparison with the November 4 explicit portfolio

The November 4 no-sharing portfolio `[1,2,3,5,7,8,12,15]` strictly certified
106/400. The November 5 portfolio shares seven IDs with that run and, as a set,
replaces strategy 3 with strategy 14.

| Strictly certified problem sets | Count |
|:---|---:|
| Certified in both | 100 |
| November 5 only | 19 |
| November 4 only | 6 |
| November 5 total | 119 |
| November 4 total | 106 |
| Union of the two observed sets | 125 |
| Net difference (November 5 vs. November 4) | **+13** |

**November 5 only:** `BOO023-1`, `GRP667-2`, `GRP667-3`, `GRP752-1`,
`KLE084-10`, `LAT022-1`, `LAT085-1`, `LAT174-1`, `LAT390-1`, `LCL897-10`,
`MVA009-1`, `MVA011-1`, `REL019-1`, `REL026-3`, `REL027-1`, `REL027-3`,
`REL031-1`, `REL050-3`, and `RNG001-10`.

**November 4 only:** `GRP178-2`, `LAT076-1`, `LAT078-1`, `LAT392-1`,
`REL024-2`, and `REL025-1`.

The +13 is an archive-to-archive difference, not an estimate of the effect of
replacing strategy 3 with 14. These were chronological single runs with
different producer binaries and hosts; the November 5 binary also has the stale
build warning described below. The November 4 run used `mtsdev04` and binary
SHA-256 `5b5637d8c0d77f3feb9be5f61dab5b583f822038679425ddf8c70c70300fd3e9`,
while this run used `mtsdev01` and binary SHA-256
`06567e46e9fa02f0d42d812acc6f1454e35e6d04d26d6eefe9fdc1e1765568f4`. Both
run-metadata files record the same commit with dirty trees; the hashes differ,
and this run's log flags the binary as stale relative to sources.

## Hardware and provenance

| Attribute | Value |
|:---|:---|
| Artifact directory | `~/campaign-casc-j13-ueq-W8CASCHJ1-01-1_2_5_7_8_12_14_15_20261105` |
| Sibling archive | `~/campaign-casc-j13-ueq-W8CASCHJ1-01-1_2_5_7_8_12_14_15_20261105.tar.bz2` |
| Run ID / timestamp | `mrs_bench_20261105_094625_5cd2c502` / `20261105_094625` (timezone not recorded) |
| Producer commit / tree | `c021be580651c0bbf1cefdf3c56f05081eb6610e`, `main`, dirty |
| Producer binary SHA-256 | `06567e46e9fa02f0d42d812acc6f1454e35e6d04d26d6eefe9fdc1e1765568f4` |
| Producer metadata SHA-256 | `2c042bcfea6c694f4849163da2ea7eb0e8be8273304d5502fb4aaca1ebf2079c` |
| `run.csv` SHA-256 | `b551730be463a71edf358341823a7ed0bf2229f514ba69b9c4c0d536f074102f` |
| `audit.csv` SHA-256 | `71b6e32d2e84aecd1dbf1a01352ecc6ff31fcb3c63fa10d9500fc1dc614580ad` |
| Strict audit binary SHA-256 | `bb105af1c21b53b0d8ed682647b89bffeb47bb1f379cfed8a67171ec7ae1c73d` |
| Audited corpus SHA-256 | `b74bdfad012a77cb9f16227a3478c8cbf710e53191a8327dc7d35ce4597f727b` |
| Host | `mtsdev01`, Intel Xeon E5-2407 @ 2.20 GHz, 8 reported physical cores, 96,350 MB RAM |
| Search configuration | `MRS_HARDWARE=casc`, 8 workers, portfolio `[1,2,5,7,8,12,14,15]`, `MRS_SHARED_POOL_INTERVAL=0`, `jobs=1` |
| Effective memory | 81,897 MB against the 131,072 MB allowance; `mem_unrepresentable=1` |
| Time limit | Official 180 s (`use_casc_times=1`); `run.log` confirms `ueq=180s` |
| Metadata default | 120 s; overridden by the official CASC-J13 clock |
| Rust toolchain | 1.99.1 |

The host did not provide the full 128-GiB CASC allowance. No row ended in
`ResourceOut`, but memory was below the canonical allowance and remains a
comparability caveat.

The campaign log begins with a harness warning that the release binary was
older than the source files and that the results therefore describe a stale
prover. The source tree is also marked dirty, so the recorded binary digest—not
the commit alone—is the identity of the measured prover. Treat this as an
observation about that exact binary and audited corpus, not a clean measurement
of the named source revision.

`run_meta.txt` labels the schedule `auto`, while `run_meta.json` records
`MRS_PORTFOLIO=1,2,5,7,8,12,14,15`; the per-row telemetry also records those
strategy IDs. The audit identity binds the strict audit to the producer
metadata, `run.csv`, corpus, and audit binaries.

## Assessment

The latest full CASC-J13 campaign remains the November 3 observation at 143/400;
this UEQ-only portfolio run does not replace it. The candidate yielded 24 fewer
strictly certified solves than that shipped-order observation, although it
found three solves the November 3 run did not. The apparent +13 over the
November 4 portfolio is likewise not causal and cannot isolate strategy 14,
especially given the stale-binary warning, dirty producer tree, different host,
and single-run design.

Do not promote this portfolio from these results. A follow-up should first use
a freshly built and frozen producer, then run the shipped and candidate orders
in at least two randomized paired blocks with the same corpus, canonical
8-worker hardware/memory conditions, `jobs=1`, sharing policy, and official
180-second limit. Strictly audit both arms; if selecting among multiple
candidates, include held-out problems or families. Follow
[`methodology.md` §8.5](../../policies/methodology.md#85-experimental-design-and-evidence-strength)
and [the CASC portfolio workflow](../../../AGENTS.md).
