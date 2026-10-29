# UEQ Follow-up Campaigns (2026-10-28)

> Status: supplemental, single-run evidence for the shipped `casc_ueq` schedule.
> Both campaign metadata files record a dirty build at commit `9516f0b9`; the
> runs used different binaries and hardware. These results do not replace the
> canonical UEQ baselines or establish a portfolio improvement.

## Summary

| Edition | Problems | Raw `Unsatisfiable` | Strict `VerifiedGood` | Other refutation audit | `Timeout` | `GaveUp` |
|:---|---:|---:|---:|:---|---:|---:|
| CASC-J13 | 400 | 121 | **120** | 1 `no_proof` (`REL045-2`) | 260 | 19 |
| CASC-30 | 300 | 124 | **124** | none | 166 | 10 |

Neither audit reported a `VerifiedBad` proof. CASC-30's 124 raw refutations all
passed strict verification. CASC-J13's 121 raw refutations yielded 120 strict
certificates; `REL045-2` had no derivation to audit and must not be counted as a
certified solve. The runs reported no `ResourceOut` or process `Error` rows.

## Provenance and comparability

| | CASC-J13 | CASC-30 |
|:---|:---|:---|
| Artifact | `~/crates/mrs-bench/results/campaign-cascj13-ueq-W8C8J1-97-20261028` | `~/crates/mrs-bench/results/campaign-casc30-ueq-W8CASCHJ1-01-20261028` |
| Run ID | `mrs_bench_20261028_160655_c24c5510` | `mrs_bench_20261028_160256_4398dcfa` |
| Binary SHA-256 | `9a096451e75476b0ff2a6b126564a92c0465b46fa5859f093a8b5a842dfe31b2` | `7d4658e85e44adaf02806b84cf36cd6a46afd50099d339ec0824e21dfcc9fb76` |
| Git | `9516f0b9e3bf17f8e932860b62075c9febe47332`, `main`, dirty | `9516f0b9e3bf17f8e932860b62075c9febe47332`, `main`, dirty |
| Host | `tlpnf9701`, Xeon Silver 4108 @ 1.80 GHz; 63,763 MB RAM | `mtsdev01`, Xeon E5-2407 @ 2.20 GHz; 96,350 MB RAM |
| Hardware mode | `adaptive`, explicit 8-worker override; host reported 16 physical cores and 46,609 MB effective memory | `casc`, 8 workers / 8 physical cores; 81,897 MB effective memory |
| Time limit | 180 s via `use_casc_times=1` | 240 s via `use_casc_times=1` |
| Schedule | `casc_ueq` (`4,8,12,11,2,14,15,1`) | `casc_ueq` (`4,8,12,11,2,14,15,1`) |

Both runs used `jobs=1` and Rust 1.99.1. The J13 run's explicit
`MRS_WORKERS=8` setting limited it to eight search workers; the reported 16
physical cores describe the host, not the worker count. Its profile was still
`adaptive`, not `casc`. The CASC-30 run selected `MRS_HARDWARE=casc`, but its
host had less than the 128 GB CASC allowance: the prover warned that the
allowance was not representable and reported `mem_unrepresentable=1`. The
campaigns also used distinct binary hashes and dirty trees, and each is a single
run. Thus the results are useful per-run certification evidence, not a
controlled comparison or a new standardized score. Keep the repository's
canonical baseline unchanged; see `AGENTS.md` §11 and
[`docs/results/README.md`](../../results/README.md).

## Timing observations

Times below use `wall_time_s` from `run.csv`, over raw SZS `Unsatisfiable`
rows. They describe these runs only; single portfolio runs are timing-sensitive.

| Edition | Solves | Median | Mean | Slowest solve |
|:---|---:|---:|---:|:---|
| CASC-J13 | 121 | 10.567 s | 35.066 s | 178.152 s (`LAT097-1`) |
| CASC-30 | 124 | 17.216 s | 51.628 s | 238.192 s (`LAT085-1`) |

There are 232 common problem names between these editions. The runs solved 73
common problems in both, 4 only in the J13 run (`KLE147-10`, `LAT157-1`,
`LAT241-10`, `RNG006-10`), and 7 only in the CASC-30 run (`GRP179-1`,
`GRP667-10`, `LAT168-1`, `REL026-3`, `REL027-3`, `REL034-2`, `RNG001-10`).
This overlap is descriptive, not an edition or hardware comparison: limits,
hardware mode, available memory, and run-to-run portfolio scheduling differ.

## Proof-output follow-up

The 2026-10-28 artifacts show that the widespread empty proof output seen in the
October 26 cooperative candidate campaigns did not recur at the same scale:
all 124 CASC-30 refutations passed the strict audit, and 120 of 121 J13
refutations did. The J13 exception is `REL045-2`:

- Search returned SZS `Unsatisfiable` after about 10.6 s.
- The archived stdout has no TSTP proof block. The stderr says
  `% Proof omitted: no TSTP derivation steps`; the audit classifies it as
  `no_proof` because there is no `$false` root or proof-omission marker it
  recognizes.
- The SZS detail nevertheless says `proof_nodes=0 proof_bytes=80
  proof_emitted=true`. The 80-byte value is header-only text, not a derivation.
  The current telemetry computes `proof_emitted` from the byte-limit flag, not
  whether a proof block was actually printed. See [UI-12 in the unresolved
  issues list](../../policies/unresolved-issues.md).
- The same problem in the CASC-30 artifact has a 170-node, 28,142-byte proof
  and passes strict verification. Subsequent investigation reproduced the
  anomaly deterministically under Strategy 15: `given` was rewritten by forward
  demodulation into a new clause ID that was not registered in `clause_store`
  prior to backward subsumption resolution deriving `$false`, causing proof
  extraction to fail closed on a missing dependency. Registering simplified
  clauses immediately and updating `given_fv` resolves the issue, allowing
  Strategy 15 to emit a 126-node proof that certifies strictly as `VerifiedGood`.
  See [UI-12 in the unresolved issues list](../../policies/unresolved-issues.md).

Do not count J13 `REL045-2` as certified. The separate October 26 empty-block
artifacts remain unaudited; this follow-up does not retroactively certify them.

## Memory observations

The CSV columns must be distinguished. `peak_virtual_mb` is the prover's
`VmPeak` (peak virtual address space); it is **not resident RAM**.
`peak_rss_mb` is the campaign's separately collected maximum RSS. The legacy
`peak_memory_mb` column can fall back to RSS when virtual-memory output is
missing, so use the explicit columns below.

| Edition | Virtual data | Median virtual peak | Mean virtual peak | Maximum virtual peak | RSS data | Median RSS | Maximum RSS |
|:---|---:|---:|---:|:---|---:|---:|:---|
| CASC-J13 | 400/400 rows | 15,503 MB | 16,597 MB | **79,521 MB** (`ROB015-10`, Timeout) | 0/400 rows | unavailable | unavailable |
| CASC-30 | 298/300 rows | 14,492 MB | 14,482 MB | **51,946 MB** (`ROB005-1`, Timeout) | 300/300 rows | 7,897 MB | **32,016 MB** (`LAT150-1`, Timeout) |

The J13 campaign therefore has no recorded RSS measurement; its 79,521 MB
maximum is a virtual-address-space figure and cannot be interpreted as physical
memory consumption or compared directly with the host's 63,763 MB RAM. For
CASC-30, the reported maximum RSS was below the host's 96,350 MB RAM, but the
host still did not provide the full 128 GB CASC allowance. No memory-related
resource-out status was recorded in either campaign.

## Related records

- [CASC-30 UEQ October report](ueq-2026-10.md)
- [CASC-J13 UEQ October report](ueq-j13-2026-10.md)
- [Banked benchmark results](../../results/README.md)
- [UEQ proof-output issues](../../policies/unresolved-issues.md)
