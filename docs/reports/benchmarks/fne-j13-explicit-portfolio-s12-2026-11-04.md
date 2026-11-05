# CASC-J13 FNE explicit portfolio `[1,2,3,4,11,12,13,15]` (2026-11-04)

> One complete, sequential CASC-J13 FNE campaign using eight workers and an
> explicit no-sharing portfolio. All 37 emitted refutations passed the strict
> audit. This is a single exploratory run: the run log warns that the producer
> binary was older than the sources, so it is evidence about the recorded binary
> digest, not a clean build of the recorded source tree.

## Results

The campaign completed all 100 FNE problems with `jobs=1` and the official
180-second CASC-J13 limit:

| SZS outcome | Count |
|:---|---:|
| `Theorem` | **37** |
| `GaveUp` | 15 |
| `Timeout` | 48 |
| `Error` / signal | 0 |
| **Total** | **100** |

The strict audit (`checks=[strict]`) reports 37 applicable refutations, all 37
`VerifiedGood`, with no `VerifiedBad`, `Unknown`, verifier timeout, or proof
omission. The remaining 63 rows are `N/A: Incomplete`.

Summed `actual_wall_time_s` was **12,312.693 s** (3.42 h). For the 37 theorems,
the mean was **28.350 s**, the median **2.965 s**, and the range **0.143–139.625
s**. All 100 processes exited with code 0 and no recorded termination signal.

### Memory and `NLP26*`

The host reported 95,979 MB RAM, below the `casc` profile's 131,072 MB
allowance; the run reported `effective_mem_mb=81,582` and
`mem_unrepresentable=1`. The maximum observed RSS was **75,212 MB** on
`NLP262+1`; maximum virtual address space was **87,142 MB** on `NLP260+1`.

Unlike the other Nov 4 explicit-portfolio run on `mtsdev04`, this run completed
`NLP260+1`, `NLP261+1`, and `NLP262+1` normally as `GaveUp` (peak RSS 75,002,
74,589, and 75,212 MB respectively). The host, binary digest, and other run
conditions were not all the same as the `mtsdev04` run; these differing outcomes
do not establish why that run received `SIGKILL` or prove that it was an OOM
kill.

## Comparisons

### November 3 full `casc_fne` run

The November 3 full CASC-J13 campaign strictly certified 38 FNE problems. This
run certified 37: its only missing theorem is `CSR116+19` (which was a theorem
in the November 3 run and timed out here). It added no theorem not already
present in that run. Three additional unsolved statuses changed:

| Problem | Nov 3 | Nov 4 explicit portfolio |
|:---|:---|:---|
| `LCL638+1.020` | `Timeout` | `GaveUp` |
| `LCL646+1.010` | `Timeout` | `GaveUp` |
| `LCL666+1.010` | `GaveUp` | `Timeout` |

This is a useful same-host, same-binary archive contrast: both runs record
`mtsdev02`, eight workers, `MRS_HARDWARE=casc`, sequential jobs, the official
180-second FNE limit, and producer binary SHA-256
`02316edf4a35db34ec581647d9d2f8313b6ae27a954a1d7782764e20c59f6fab`. The
November 3 run used the named `casc_fne` schedule; this run used the explicit
portfolio below. It is still one chronological run per configuration, not a
randomized paired experiment, and the explicit run's log warns that the binary
was stale relative to its source tree. Thus, it is not evidence that the
recorded source commit itself produces these results or a replicated estimate
of the portfolio effect. See the [November 3 full campaign report](cert-j13-2026-11-03.md).

### Other November 4 explicit portfolio

The companion run used `[1,2,3,4,6,11,13,15]` on `mtsdev04` and also certified
37 problems. Comparing the archived `run.csv` files, the two runs have exactly
the same 37-problem theorem set; neither adds a theorem relative to the other.
Seven unsolved rows have different SZS outcomes: `NLP260+1`, `NLP261+1`, and
`NLP262+1` are `Error` in the companion run versus `GaveUp` here; and
`LCL646+1.010`, `LCL688+1.015`, `SYO525+1.027`, and `SYO525+1.030` flip between
`Timeout` and `GaveUp`.

At the set level the portfolios differ by strategy 6 versus 12. In the ordered
lists, strategy 11 also moves from worker slot 6 to slot 5; strategies 13 and 15
remain in the same slots. The runs also differ in host and producer-binary
digest. Their identical observed solve sets therefore do **not** isolate
strategy 6 versus strategy 12 or establish that the strategies are
interchangeable. See the [companion report for
`[1,2,3,4,6,11,13,15]`](fne-j13-explicit-portfolio-2026-11-04.md).

## Configuration and provenance

| Attribute | Value |
|:---|:---|
| Artifact directory | `~/campaign-casc-j13-W8CASCHJ1-02-1_2_3,4_11_12_13_15_20261104` |
| Sibling archive | `~/campaign-casc-j13-W8CASCHJ1-02-1_2_3,4_11_12_13_15_20261104.tar.bz2` |
| Run ID / metadata timestamp | `mrs_bench_20261104_174812_28dc2f9d` / `20261104_174812` (timezone not recorded) |
| Producer commit / tree | `c021be580651c0bbf1cefdf3c56f05081eb6610e`, `main`, dirty |
| Producer binary SHA-256 | `02316edf4a35db34ec581647d9d2f8313b6ae27a954a1d7782764e20c59f6fab` |
| `run.csv` SHA-256 | `4890ada1acf01301705031dcbe75a9a884151b72888f2bd7c45e9c5bf1c15710` |
| `audit.csv` SHA-256 | `9324f071829b43912eea4f40684d91e31cb28d77ef8f8ed0956ed1a5a4552049` |
| Strict audit binary SHA-256 | `44491ecf8abc6805a17df4f188b21af1f1b4dec25fd3de90873241ed4ac3b11a` |
| Audited corpus SHA-256 | `b74bdfad012a77cb9f16227a3478c8cbf710e53191a8327dc7d35ce4597f727b` |
| Host | `mtsdev02`, Intel Xeon E5-2407 @ 2.20 GHz, 8 reported physical cores, 95,979 MB RAM |
| Search configuration | `MRS_HARDWARE=casc`, `MRS_WORKERS=8`, `MRS_PORTFOLIO=1,2,3,4,11,12,13,15`, `MRS_SHARED_POOL_INTERVAL=0`, `jobs=1` |
| Time limit | `--casc-times`; run log confirms 180 s for FNE |
| Rust toolchain | 1.99.1 |

Although `run_meta.txt` labels the schedule `auto`, `MRS_PORTFOLIO` is explicitly
recorded in `run_meta.json` and the strict-audit identity, and per-row telemetry
records strategy IDs `1,2,3,4,11,12,13,15`. The metadata's 120-second default
is overridden by `use_casc_times=1`. The campaign log begins with the warning
that the MRS binary is older than its sources; the binary digest, rather than
the dirty source commit, is therefore the precise producer identity for this
observation. The audit identity binds the audit to the producer metadata,
`run.csv`, corpus, and audit binaries.

## Interpretation

This run does not support a portfolio promotion: it certified one fewer problem
than the November 3 full `casc_fne` run, added no solves, and matched the other
Nov 4 explicit variant's solve set in one run on a different host and binary.
The same binary and host as the Nov 3 campaign make that archive comparison
more directly comparable than a cross-host comparison, but do not remove
search variability or the stale-binary caveat. Use randomized, repeated paired
cooperative campaigns on the intended 8-core configuration before drawing a
schedule conclusion, as required by
[`methodology.md` §8.5](../../policies/methodology.md#85-experimental-design-and-evidence-strength).
