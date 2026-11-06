# CASC-30 UEQ explicit-portfolio campaign (2026-11-05)

> The explicit eight-strategy portfolio `[1,2,3,6,7,8,11,12]` completed all
> 300 CASC-30 UEQ problems and strictly certified **142/300** refutations.
> Against the latest shipped-`casc_ueq` observation (144/300 on November 3),
> the runs share 132 solves: ten are candidate-only and twelve November-3-only.
> This is a single comparison between dirty builds on different hosts, not a
> controlled portfolio result or a promotion case.

## Results and strict audit

The campaign used eight workers, sequential `jobs=1`, `MRS_HARDWARE=casc`,
disabled cross-strategy sharing, and the official 240-second UEQ limit
(`use_casc_times=1`). It completed all 300 problems:

| SZS outcome | Count | Strict audit |
|:---|---:|:---|
| `Unsatisfiable` | **142** | **142 `VerifiedGood`** |
| `Timeout` | 148 | no refutation to check |
| `GaveUp` | 10 | no refutation to check |
| `Error` / `ResourceOut` | 0 | — |
| **Total** | **300** | |

The archived strict audit reports 142 applicable refutations, all 142
`VerifiedGood`, with 0 `VerifiedBad`, `Unknown`, verifier timeouts, or proof
omissions. The remaining 158 rows are `N/A: Incomplete`. The audit identity
binds the audit to the producer metadata, `run.csv`, and corpus digest. This is
evidence for these archived proofs, not a claim of global soundness.

Per-problem actual wall time summed to 44,007.5 seconds (12.22 hours). Among the
142 refutations, mean time was 42.549 s, median 19.865 s, and the range was
0.138–238.555 s. These are descriptive timings from one run, not a speed
comparison.

## Comparison with the latest shipped-schedule observation

The November 3 CASC-30 all-division attempt completed all 300 UEQ jobs before
the campaign later stopped during ICU. Its UEQ schedule telemetry shows the
shipped `casc_ueq` order `[4,8,12,11,2,14,15,1]`; all 144 refutations passed
strict audit. The November 5 candidate used `[1,2,3,6,7,8,11,12]`.

| Strictly certified problem sets | Count |
|:---|---:|
| Certified in both | 132 |
| November 3 only | 12 |
| November 5 only | 10 |
| November 3 total | 144 |
| November 5 total | 142 |
| Union of the two observed sets | 154 |
| Net count difference (November 5 vs. November 3) | **−2** |

**November 5 only:** `BOO014-10`, `COL009-1`, `LAT392-1`, `LCL341-10`,
`LCL400-10`, `MVA011-1`, `REL010-1`, `REL011-1`, `REL026-1`, and `REL044-1`.

**November 3 only:** `GRP167-3`, `GRP185-2`, `GRP766-1`, `KLE152-10`,
`KLE169-10`, `LAT085-1`, `LAT174-1`, `REL020-2`, `REL036-2`, `RNG001-10`,
`SYN598-10`, and `SYN708-10`.

The candidate retains five IDs from the shipped order (`1,2,8,11,12`), removes
`4,14,15`, and adds `3,6,7`. The solve-set churn establishes that the two
observed portfolios cover different problems; it does not show that any one
strategy caused a unique solve. November 3 and November 5 used different dirty
producer trees, binaries, and hosts (`mtsdev01` and `mtsdev02`, respectively).
Both hosts had eight physical cores and the same Xeon E5-2407 model, but were
below the 128-GiB memory allowance. The November 3 audit report also records
that byte-for-byte identity between its original host-side problem tree and
the local re-audit corpus was not independently established. See the
[November 3 report](cert-30-2026-11-03.md) for that audit caveat. Treat the
problem-name overlap as an observed archive comparison, not a controlled
estimate of portfolio effect.

## How to read the earlier 124/300 and 149/300 results

The **124/300** result is the October 28 shipped-schedule follow-up, all 124
proofs strictly verified. Its metadata names host `mtsdev01`; the November 5
candidate ran on `mtsdev02`, so 124 is not a same-host comparator for this
campaign. It was a supplemental single run with a dirty producer build, not the
repository's established CASC-30 UEQ comparison reference. That historical
reference remains **127/300 from October 7**. The latest shipped-schedule
observation is **144/300 from November 3**; it too is a single dirty-build
observation, not a standardized or official score. A separate October 28
set comparison gives 116 solves in common, 26 November 5-only and 8 October
28-only (an observed count difference of +18), but the builds and hosts differ;
the two audit identities record the same corpus digest for this set comparison.
Do not attribute that difference to the portfolio.

The **149/300** October 29 artifact also has a strict audit: all 149 proofs are
`VerifiedGood`. It is not a canonical eight-core CASC comparison. The host had
16 physical cores and was not pinned to eight; its effective memory limit was
54,198 MB, and 25 problems ended in memory `ResourceOut`. Keep 149 as a valid
strictly verified count for that artifact, but not as evidence that the shipped
schedule outperforms the 144/300 eight-core observation or this 142/300
candidate.

### Related November 5 alternate portfolio

The separate artifact
`campaign-casc-30-ueq-W8CASCHJ1-97-1_2_5_6_7_8_11_12_20261105` ran the explicit
portfolio `[1,2,5,6,7,8,11,12]` and completed all 300 problems:

| SZS outcome | Count | Strict audit |
|:---|---:|:---|
| `Unsatisfiable` | **148** | **148 `VerifiedGood`** |
| `Timeout` | 120 | no refutation to check |
| `GaveUp` | 10 | no refutation to check |
| `ResourceOut` | 22 | all memory |
| `Error` | 0 | — |
| **Total** | **300** | |

The strict audit reports 148 applicable refutations, all `VerifiedGood`, with
no `VerifiedBad`, `Unknown`, verifier timeout, proof omission, or checker error.
The other 152 problems are `N/A: Incomplete`. This certifies the archived
refutations, not the prover globally. Summing `actual_wall_time_s` over the 300
sequential jobs gives 42,511.8 seconds (11.81 hours). The 148 refutations took
40.521 seconds on average, with a 16.479-second median and a 0.134–238.188-second
range (`PUZ131-10` to `LAT085-1`); these are descriptive single-run timings.

All 22 `ResourceOut` rows report `resource_reason=memory` at the host's
54,198 MB effective cap. They comprise 9 GRP, 8 LAT, 2 RNG, and one each from
KLE, MVA, and REL: `GRP164-1`, `GRP164-2`, `GRP167-3`, `GRP180-1`, `GRP180-2`,
`GRP181-1`, `GRP181-2`, `GRP725-1`, `GRP753-1`; `LAT139-1`, `LAT149-1`,
`LAT150-1`, `LAT151-1`, `LAT155-1`, `LAT162-1`, `LAT170-1`, `LAT186-10`;
`RNG021-6`, `RNG025-5`; `KLE152-10`; `MVA003-1`; and `REL029-3`. The resource
details record RSS at the limit trigger from 54,207 to 54,847 MB. The `run.csv`
`peak_rss_mb` field is blank for all 300 rows; its maximum reported virtual
memory was 93,390 MB (`GRP181-1`), which is not an RSS measurement.

The host was `tlpnf9701`, an Intel Xeon Silver 4108 @ 1.80 GHz system with 16
physical cores and 63,763 MB RAM (the run metadata records 32 CPU cores). It
used eight workers, `jobs=1`, `MRS_HARDWARE=casc`, the 240-second UEQ limit, and
disabled clause sharing. The `casc` profile requested 131,072 MB, but the
effective memory limit was 54,198 MB (`mem_unrepresentable=1`); CPU placement was
unrestricted rather than pinned to eight cores. This is therefore a valid
audited result for this artifact, but not a canonical 8-core/128-GiB CASC
comparison. The memory exits are host-censored outcomes; the artifact does not
establish that more memory would turn them into solves.

By problem name, this run and the `[1,2,3,6,7,8,11,12]` artifact above solved
140 problems in common. The strategy-5 artifact alone solved `LAT154-1`,
`LAT177-1`, `GRP185-2`, `LAT146-1`, `GRP766-1`, `LAT085-1`, `RNG006-10`, and
`SYN598-10`; the strategy-3 artifact alone solved `GRP753-1` and `GRP183-2`.
In this artifact, `GRP753-1` ended in a memory `ResourceOut`, while it was solved
in the other run. This six-solve net difference is not evidence that strategy 5
caused the gain: the producer binary hashes differ, as do the strict-audit
corpus digests (`9a9cabd1…` here versus `bb153a22…` above), and the runs used
different hosts and effective memory limits.

The alternate run's identity is: run ID
`mrs_bench_20261105_100853_1c809a46`; dirty producer tree at
`c021be580651c0bbf1cefdf3c56f05081eb6610e`; producer binary SHA-256
`6f771df032287a9ab022904356ac35b11c4eb1da4668d0008b720180478d65b7`; `run.csv`
SHA-256 `413cc6dd86f2979e40c661993ad6162732cfd849987802fa540780daf862902d`;
producer-metadata SHA-256
`42789826ccf0eec119b46ee94c7e0f8ae2c5623d6a2e7b0a4dc9da5b565998b2`;
strict-audit binary SHA-256
`7b18de1e914bbc5ff1fa4de926b6e57d89706e8b90d31e3af90c5fb393a9a083`; and
audited corpus SHA-256
`9a9cabd1a059f737c56353e01f7008597b96de7a312d0fb69559d8e15d77570a`. Its
audit identity binds the audit to the producer metadata, run CSV, and corpus.

## Provenance and resource context

| Attribute | Value |
|:---|:---|
| Artifact directory | `campaign-casc-30-ueq-W8CASCHJ1-02-1_2_3_6_7_8_11_12_20261105` |
| Run ID / timestamp | `mrs_bench_20261105_094728_13290aed` / `20261105_094728` (timezone not recorded) |
| Edition / division | CASC-30 / UEQ; 300 problems |
| Portfolio | `[1,2,3,6,7,8,11,12]` |
| Producer commit / tree | `c021be580651c0bbf1cefdf3c56f05081eb6610e`, `main`, dirty |
| Producer binary SHA-256 | `02316edf4a35db34ec581647d9d2f8313b6ae27a954a1d7782764e20c59f6fab` |
| `run.csv` SHA-256 | `60f83932ad67a3c70ea45dcbc376ae14ff76a2b829cac1e4838e355673958d47` |
| Strict audit binary SHA-256 | `44491ecf8abc6805a17df4f188b21af1f1b4dec25fd3de90873241ed4ac3b11a` |
| Audited corpus SHA-256 | `bb153a22fd4e330e035386104d3cecfea03d998e224282795f83ef51233b6f16` |
| Host | `mtsdev02`, Intel Xeon E5-2407 @ 2.20 GHz, 8 physical cores, 95,979 MB RAM |
| Search settings | `MRS_HARDWARE=casc`, 8 workers, `MRS_SHARED_POOL_INTERVAL=0`, `jobs=1` |
| Time limit | 240 s (`use_casc_times=1`; metadata's 120 s is the harness default) |
| Rust toolchain | 1.99.1 |
| Memory telemetry | `effective_mem_mb=81582`, `mem_unrepresentable=1`; no `ResourceOut` rows |
| Peak memory | 57,271 MB RSS (`RNG021-6`); 91,786 MB virtual (`GRP181-1`) |

The `casc` profile's 131,072 MB allowance exceeded host RAM. The virtual peak
is address-space use, not resident memory. Although this run had no resource
exits, it did not reproduce the full 128-GiB CASC allowance.

## Assessment

The `[1,2,3,6,7,8,11,12]` run produced 142 strictly certified solves; the
separate `[1,2,5,6,7,8,11,12]` artifact produced 148, but their six-solve net
difference is confounded by different producer binaries, audited corpus
digests, hosts, and memory limits. Neither result, nor the older 124 or 149
observations, justifies changing `casc_ueq`. Before promotion, compare the
shipped and candidate portfolios in at least two randomized paired blocks on
the same frozen build, corpus, and canonical host, strictly audit both arms,
and check selected results on held-out problems or families; see
[`AGENTS.md` §11](../../../AGENTS.md).
