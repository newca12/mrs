# CASC-J13 FNE explicit-portfolio campaign (2026-11-04)

> One complete FNE division run using the explicit eight-worker portfolio
> `[1,2,3,4,6,11,13,15]`, with sharing disabled. It produced 37 strictly
> certified theorems and three `SIGKILL` process errors whose cause is
> unconfirmed. This is an exploratory single run, not a portfolio promotion or
> a controlled comparison with the shipped schedule.

## Results

The campaign evaluated all 100 CASC-J13 FNE problems with `jobs=1` and the
official 180-second clock:

| SZS outcome | Count |
|:---|---:|
| `Theorem` | 37 |
| `GaveUp` | 14 |
| `Timeout` | 46 |
| `Error` | 3 |
| **Total** | **100** |

The strict audit (`checks=[strict]`) reports 37 applicable refutations, all 37
`VerifiedGood`, with 0 `VerifiedBad`, `Unknown`, verifier timeouts, or proof
omissions. The remaining rows are 60 `N/A: Incomplete` and 3 audit `Error`
rows corresponding to the killed search processes. All 37 emitted proofs
certified; the three errors do not invalidate those proofs.

Actual solver wall time summed to 11,886.025 s (3.30 h). For the 37 theorems,
`actual_wall_time_s` has a 27.98 s mean, 3.04 s median, and 0.149–143.410 s
range.

### Process errors and memory caveat

| Problem | Outcome | Elapsed | Peak RSS | Recorded termination |
|:---|:---|---:|---:|:---|
| `NLP260+1` | `Error` | 58.006 s | 60,758 MB | exit 137, signal 9 (`SIGKILL: cause unconfirmed`) |
| `NLP261+1` | `Error` | 40.357 s | 59,567 MB | exit 137, signal 9 (`SIGKILL: cause unconfirmed`) |
| `NLP262+1` | `Error` | 39.937 s | 59,483 MB | exit 137, signal 9 (`SIGKILL: cause unconfirmed`) |

The host reported 95,969 MB RAM. The `casc` profile warned that this was below
its 128 GiB allowance and reported `effective_mem_mb=81,573` and
`mem_unrepresentable=1`. The campaign artifacts contain no kernel or cgroup log
that attributes the signals to OOM. Do **not** label these as confirmed OOM
kills: SIGKILL alone does not establish that cause (see
[`methodology.md` §8.5](../../policies/methodology.md#85-experimental-design-and-evidence-strength)).

The same three problems were `GaveUp` in the November 3 full `casc_fne` run on
`mtsdev02` after about 184–185 s, with recorded RSS around 74–76 GB. That is a
notable cross-run difference, but the runs used different hosts and dirty
producer builds; it does not identify a memory limit or establish a portfolio
effect.

### Comparison with the November 3 full `casc_fne` run

The November 3 full campaign used the named FNE order
`[11,13,4,8,15,7,3,1]` and strictly certified 38 problems. The November 4
explicit portfolio strictly certified 37. Its only lost verified problem was
`CSR116+19` (Theorem in 152.983 s on November 3; `Timeout` at 178.390 s here),
and it added no solve relative to that run.

There were also three `GaveUp` to `Error` changes on the `NLP26*` problems and
five `Timeout`/`GaveUp` flips: `LCL638+1.020`, `LCL666+1.010`, `LCL688+1.015`,
`SYO525+1.027`, and `SYO525+1.030`. Other solve times varied substantially;
for example, `CSR052+3` took 56.363 s here versus 7.124 s on November 3. These
are descriptive run differences, not a controlled schedule-only result.

### Comparison with the October 22 FNE candidate portfolio

The October 22 eight-core candidate `[11,13,4,8,15,7,3,1]` had 39
`VerifiedGood` proofs and one additional proof omission (`KRS261+1`). The
November 4 portfolio had 37 `VerifiedGood`; 34 problems were common. The
October 22-only verified problems were `CSR116+8`, `CSR116+13`, `CSR116+19`,
`CSR116+38`, and `PRD001+1`. The November 4-only verified problems were
`KRS261+1`, `NUN066+1`, and `SYO605+1`.

Notably, `KRS261+1` emitted a 53,733-node, 10,167,724-byte proof here and
certified `VerifiedGood` in 5.407 s. The October 22 run's proof for that problem
was 55.6 MB and had been omitted by the 32 MB output limit. This resolves that
specific omission in this run; it does not offset the portfolio's lower total
strict count or establish a general gain.

## Configuration and provenance

| Attribute | Value |
|:---|:---|
| Artifact directory | `~/campaign-casc-j13-W8CASCHJ1-04-1_2_3_4_6_11_13_15_20261104` |
| Sibling archive | `~/campaign-casc-j13-W8CASCHJ1-04-1_2_3_4_6_11_13_15_20261104.tar.bz2` |
| Run ID / metadata timestamp | `mrs_bench_20261104_153412_72609cb2` / `20261104_153412` (timezone not recorded) |
| Producer commit / tree | `c021be580651c0bbf1cefdf3c56f05081eb6610e`, `main`, dirty |
| Producer binary SHA-256 | `5b5637d8c0d77f3feb9be5f61dab5b583f822038679425ddf8c70c70300fd3e9` |
| `run.csv` SHA-256 | `14b37bd51fed9fc958e120e665b4933d19ebcf4ed9ea789ff9368214e2486c55` |
| `audit.csv` SHA-256 | `9ee48fca084286359a5500b861fd2ec4ba20dbb686422c112611308e7d2623b4` |
| Strict audit binary SHA-256 | `7a76f2d99354e51765e0df877e5f51ba560361d2c069f0ec66e33c41dc856a05` |
| Audited corpus SHA-256 | `b74bdfad012a77cb9f16227a3478c8cbf710e53191a8327dc7d35ce4597f727b` |
| Host | `mtsdev04`, Intel Xeon E5-2407 @ 2.20 GHz, 8 reported physical cores, 95,969 MB RAM |
| Search configuration | `MRS_HARDWARE=casc`, `MRS_WORKERS=8`, `MRS_PORTFOLIO=1,2,3,4,6,11,13,15`, `MRS_SHARED_POOL_INTERVAL=0`, `jobs=1` |
| Time limit | `--casc-times`; `run.log` confirms official 180 s for FNE |
| Metadata default | 120 s; overridden by `use_casc_times=1` |
| Rust toolchain | 1.99.1 |

The explicit portfolio appears in both `run_meta.json` and the strict-audit
identity; per-row telemetry records the ordered strategy IDs
`1,2,3,4,6,11,13,15`. The strict audit and sibling archive were also produced,
confirming end-to-end execution of the explicit-portfolio campaign path. The
binary and tree were dirty, so the binary digest is the precise producer
identity; this run is not a clean-build benchmark.

## Interpretation

This run provides no evidence to replace the shipped `casc_fne` order: it
strictly certified one fewer problem than the November 3 full run and had no
new solve against that run. It is one observation on a different host and a
dirty build; strategy, ordering, host, timing, and the three external process
terminations are not isolated. Treat the observed solve-set differences and
`KRS261+1` proof recovery as candidates for paired, repeated testing, not as a
promotion result. See the experimental-design rules in
[`methodology.md` §8.5](../../policies/methodology.md#85-experimental-design-and-evidence-strength)
and the [October candidate portfolio report](fne-candidate-portfolio-2026-10.md).
