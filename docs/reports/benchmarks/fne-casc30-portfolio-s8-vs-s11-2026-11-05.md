# CASC-30 FNE portfolio swap: strategy 8 vs strategy 11 (2026-11-05)

> Two complete 100-problem FNE campaigns swapped strategy 8 for strategy 11 in
> the same slot of otherwise identical explicit portfolios. They observed 39
> versus 37 strictly verified theorems. This is a single, cross-host
> comparison, not a randomized paired estimate: both producer trees were dirty
> and the producer binary hashes differ. It does not support a schedule
> promotion.

## Results

Both runs used eight workers, sequential problem jobs, the official CASC-30
FNE 240-second limit, and disabled cross-strategy sharing.

| Outcome | Campaign 03: `[1,2,3,4,8,12,14,15]` | Campaign 04: `[1,2,3,4,11,12,14,15]` |
|:---|---:|---:|
| `Theorem` | **39** | **37** |
| Strict `VerifiedGood` | **39** | **37** |
| `VerifiedBad` / `Unknown` / verifier timeout / proof omitted | 0 | 0 |
| `GaveUp` | 4 | 7 |
| `Timeout` | 54 | 53 |
| `Error` | 3 | 3 |
| `ResourceOut` | 0 | 0 |
| **Total** | **100** | **100** |

The strict audit reports every emitted refutation as `VerifiedGood`. The three
`Error` rows in each run are the interrupted `NLP26*` processes, not failed
proof checks.

### Solve-set comparison

The two campaigns have **36 theorems in common**. Campaign 03 has three
theorems not present in campaign 04; campaign 04 has one theorem not present
in campaign 03:

| Problem | Campaign 03 | Campaign 04 |
|:---|---:|---:|
| `LCL390+1` | Theorem, 236.333 s | Timeout, 238.920 s |
| `MED007+1` | Theorem, 70.232 s | Timeout, 239.683 s |
| `MED009+1` | Theorem, 39.309 s | Timeout, 239.788 s |
| `KRS234+1` | Timeout, 238.841 s | Theorem, 3.926 s |

Thus the **observed union contains 40 distinct solved problems**. It is only a
union of two separate runs; no eight-worker portfolio containing both
strategies was tested, and the union does not show that either strategy caused
its campaign's unique solves. Per-problem telemetry is aggregate and does not
identify the worker that found a refutation.

Three additional unsolved rows changed from `Timeout` in campaign 03 to
`GaveUp` in campaign 04: `LCL682+1.015`, `NUN057+1`, and `SYN986+1.004`.
The recorded details do not establish a common cause for these status changes.
In particular, campaign 04 records `SYN986+1.004` as `GaveUp` after 245.794 s,
slightly beyond the nominal 240-second limit; this is not evidence that the
search stopped early.

### Descriptive timing on common solves

For the 36 common theorems, the sum of reported per-problem `wall_time_s` was
603.140 s in campaign 03 and 524.011 s in campaign 04. Individual times moved
substantially in both directions: `CSR115+31` was 238.212 s versus 96.025 s,
while `CSR052+3` was 5.423 s versus 140.729 s. These are descriptive
cross-campaign measurements, not a timing comparison: the runs used different
hosts and different producer binaries.

## Configuration and provenance

| Attribute | Campaign 03 | Campaign 04 |
|:---|:---|:---|
| Artifact directory | `~/campaign-casc-30-fne-W8CASCHJ1-03-1_2_3_4_8_12_14__15_20261105` | `~/campaign-casc-30-fne-W8CASCHJ1-04-1_2_3_4_11_12_14_15_20261105` |
| Run ID / timestamp | `mrs_bench_20261105_102219_732c3776` / `20261105_102219` | `mrs_bench_20261105_104651_54121131` / `20261105_104651` |
| Portfolio | `[1,2,3,4,8,12,14,15]` | `[1,2,3,4,11,12,14,15]` |
| Host | `mtsdev03`, Xeon E5-2407 @ 2.20 GHz, 8 cores | `mtsdev04`, Xeon E5-2407 @ 2.20 GHz, 8 cores |
| Producer commit / tree | `c021be580651c0bbf1cefdf3c56f05081eb6610e`, dirty | `c021be580651c0bbf1cefdf3c56f05081eb6610e`, dirty |
| Producer binary SHA-256 | `e0f113ec51a5b32872006a7350658d47898c6e6a897e5272c9e2a5575ca516eb` | `5b5637d8c0d77f3feb9be5f61dab5b583f822038679425ddf8c70c70300fd3e9` |
| `run.csv` SHA-256 | `38a4e395c26ee3e5f2e85121e2a4b461127643d06b23c214d90c45cb0bab412e` | `1d97451b1dee91b35357ae677023e4f439ac22b470e36a40c1c4e6a54ec66467` |
| `audit.csv` SHA-256 | `80776ed407c3d7af0acd3437b0685fcc8df6281810064b2853a6299a5bbb3df3` | `f6c0116821f2c262fc7ec30a32222b0aff96a7604a11b50e5aefaa0c103adfb3` |
| Strict audit binary SHA-256 | `6cd6998d2364bbf233fcda0e51ca9f6c4783aa9630a121e24e6118efed46dbdf` | `7a76f2d99354e51765e0df877e5f51ba560361d2c069f0ec66e33c41dc856a05` |
| Corpus SHA-256 | `bb153a22fd4e330e035386104d3cecfea03d998e224282795f83ef51233b6f16` | same |
| Search settings | `MRS_HARDWARE=casc`, 8 workers, `MRS_SHARED_POOL_INTERVAL=0`, `jobs=1` | same |
| Time / Rust | `--casc-times` (240 s FNE); Rust 1.99.1 | `--casc-times` (240 s FNE); Rust 1.99.1 |

The metadata's 120-second default is overridden by `use_casc_times=1`; each
campaign log confirms the official 240-second FNE limit. Although the portfolios
differ only by strategy 8 versus 11 in the fifth position, the binaries are not
byte-identical and the campaigns ran on separate physical hosts. Their common
commit does not identify the dirty source changes that produced each binary.

Strategy 8 is the LPO `GoalDirected(10)` configuration; strategy 11 is the KBO
`ConjSymbolBoost` plus `AgeWeight(6)` configuration. Because the producer
telemetry does not identify the winning worker, the unique results above should
be attributed to the **portfolio runs**, not to an individual strategy.

## Process errors and memory caveat

Both campaigns recorded exit-137 / signal-9 `SIGKILL` errors on the same three
problems. The stderr logs show that preparation completed and search started;
`last_completed_stage` is `prepare` for each row because no search-complete
record was written.

| Problem | Campaign 03 peak RSS | Campaign 04 peak RSS |
|:---|---:|---:|
| `NLP260+1` | 45,131 MB | 59,119 MB |
| `NLP261+1` | 44,674 MB | 58,729 MB |
| `NLP262+1` | 44,645 MB | 58,681 MB |

Both hosts reported 95,969 MB RAM. The `casc` profile logs a nominal 131,072 MB
allowance and `effective_mem_mb=81,573` with `mem_unrepresentable=1`, below that
allowance. The archived campaigns do not include kernel or cgroup evidence
attributing the signals to OOM; **SIGKILL alone does not establish OOM**. Treat
these as process errors of unconfirmed cause, not confirmed memory-limit
outcomes.

## Interpretation

This is a candidate-generation observation: the run with strategy 8 recorded
two more strict solves, but the opposite portfolio uniquely solved
`KRS234+1`, and three other solve outcomes flipped the other way. The distinct
hosts, dirty producer trees, different binary hashes, one run per portfolio,
and timing-sensitive parallel search prevent attributing the net difference to
the strategy swap. The 40-problem union is not a measured result for a portfolio
that includes both strategies; such a portfolio would have to displace another
worker and be tested directly.

Before changing the shipped FNE schedule, run randomized paired cooperative
campaigns with the same binary and host, reporting each pair's certified delta
and per-problem flips. Since these portfolios were selected using the same
division data, promotion should also be checked on held-out problems or
families. See [methodology §8.5](../../policies/methodology.md#85-experimental-design-and-evidence-strength)
and the [October FNE candidate report](fne-candidate-portfolio-2026-10.md).
