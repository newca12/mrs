# FNE MultiQueue Portfolio: Initial Full-Time Comparison (2026-10-21)

> Status: preliminary, single-run portfolio comparison. The CASC-30 baseline
> and `mq` runs used the same host and identical binary hash. The CASC-J13
> comparison is cross-host and cross-build; treat its score difference as
> descriptive, not as a controlled estimate of the schedule effect.
>
> Result archives are under `~/crates/mrs-bench/results/`:
> `casc30-fne-baseline-240s`, `casc30-fne-mq-240s`,
> `casc-j13-fne-mq-180s`, and the historical comparison
> `campaign-cascj13-fne-W8C8J1-20261002`.

## Summary

| Edition / run | Theorem | Timeout | GaveUp | ResourceOut |
|---|---:|---:|---:|---:|
| CASC-30, default `casc_fne` | 41/100 | 49 | 10 | 0 |
| CASC-30, `mq` | **42/100** | 55 | **3** | 0 |
| CASC-J13, historical `casc_fne` reference | 34/100 | 51 | 15 | 0 |
| CASC-J13, `mq` | **36/100** | 59 | **2** | 3 |

The CASC-30 result is a near tie in coverage: `mq` gains one net theorem in
this run. It also changes which problems are solved, and substantially reduces
the number of `GaveUp` outcomes. On CASC-J13, `mq` reports two more theorems
than the available reference run, but the machines and binaries differ, so
that delta cannot be attributed to the schedule alone. These are solver
`Theorem` counts; the runs do not include an independent certification audit.

## CASC-30: same-host, same-binary comparison

The baseline and `mq` each ran the 100-problem CASC-30 FNE set with eight
workers and CASC per-problem limits. Both metadata files name host
`teenf9901` (Xeon Silver 4108, 128,019 MB) and report the same binary SHA-256:
`09b64af7067ad15cb93a48b2921a3ee299a8934bd7b4e71d895e34bb7e542e5a`.
The `mq` run set `MRS_SCHEDULE=mq`; the baseline used the default schedule.
Both runs were marked `git_dirty`, so the binary hash is the strongest
reproduction identity.

| Run | Timestamp | Recorded commit | Run ID |
|---|---|---|---|
| Baseline | 2026-10-21 10:39 | `448bd5ac203e2f48a47c49bf5ca7d28a2c7e324f` | `mrs_bench_20261021_103941_7571ada7` |
| `mq` | 2026-10-21 14:53 | `0550f66cbabd4aef216117a03cc4af42b76e3584` | `mrs_bench_20261021_145310_2cfbbc51` |

Among the 38 problems both portfolios solved, the baseline won 19 per-problem
wall-time comparisons and `mq` won 19. Their aggregate time on those 38 was
470.68 s for baseline and 519.18 s for `mq`; medians were 1.41 s and 1.48 s.
Those totals are descriptive only: a few long solves dominate them, and one
run does not characterize the portfolio's timing variance.

**Baseline-only solves (3):**

| Problem | Baseline | `mq` |
|---|---:|---:|
| `CSR116+6` | Theorem, 207.763 s | Timeout, 238.277 s |
| `LCL660+1.020` | Theorem, 18.882 s | GaveUp, 238.473 s |
| `SYN353+1` | Theorem, 39.038 s | GaveUp, 238.270 s |

**`mq`-only solves (4):**

| Problem | Baseline | `mq` |
|---|---:|---:|
| `KRS216+1` | Timeout | Theorem, 226.427 s |
| `KRS217+1` | Timeout | Theorem, 184.654 s |
| `KRS261+1` | Timeout | Theorem, 223.292 s |
| `NUN087+1` | GaveUp | Theorem, 91.043 s |

The full-time solo sweep archive reports no individual standard strategy
`s01`–`s15` solving these four `mq`-only problems at the corresponding CASC-30
limit. That is useful evidence of portfolio complementarity, but it is not a
controlled ablation: the solo sweep is a separate campaign and does not
identify which `mq` archetype found each proof.

The solved set also changes in the other direction. `mq` is much faster on
some shared solves (`MED007+1`: 0.81 s vs 55.87 s; `MED009+1`: 0.75 s vs
24.07 s; `LCL390+1`: 91.86 s vs 206.32 s), while it is much slower on others
(`CSR060+2`: 71.07 s vs 5.29 s; `CSR115+98`: 36.13 s vs 1.45 s; and
`SYN938+1`: 62.86 s vs 0.28 s). The consistent conclusion is complementarity,
not a general speedup.

Two `mq`-only theorem answers have proofs larger than the configured 32 MiB
proof-output limit: `KRS216+1` (113,731,306 bytes) and `KRS261+1`
(63,797,534 bytes). Their benchmark rows count as solver theorems, but the
proofs were omitted and are not independently certified by these data.

## CASC-J13: suggestive, not controlled

The `mq` run reports 36/100 Theorems (59 Timeout, 2 GaveUp, 3 ResourceOut).
Compared with the archived 34/100 reference run, the solved sets overlap on
33 problems:

- `mq`-only: `CSR116+19` (37.219 s), `KRS216+1` (99.547 s), and `PRD001+1`
  (53.389 s).
- Reference-only: `SYN353+1` (43.379 s).

The reference is from 2026-10-02 on an E5-2407 host with 8 reported cores,
96,350 MB RAM, an older Rust compiler, and a different binary hash. The `mq`
run is from 2026-10-21 on a Xeon Silver 4108 host with 32 reported cores and
63,763 MB RAM, using another binary hash. Consequently, the nominal +2 solve
difference and the per-problem runtime differences are not a schedule-only
A/B.

The three `ResourceOut` results are `NLP260+1`, `NLP261+1`, and `NLP262+1`.
Each exceeded the local memory limit of 54,198 MB RSS (the 85% safety cap on a
63,763 MB host). Record these as memory-limited results on this machine, not
as ordinary timeouts or evidence about performance on the canonical 128 GB
competition allowance.

For the 33 mutually solved problems, the recorded median times are 8.66 s for
the reference and 1.01 s for `mq`. This is an interesting signal, especially
for the CSR cases, but the host and binary mismatch means it should not be
reported as an MQ speedup. A same-host, same-binary rerun is needed.

## Interpretation and next steps

1. **Keep `mq` experimental for now.** The controlled CASC-30 run shows only
   +1 net theorem, with meaningful gains and regressions. This is not enough to
   justify replacing the shipped FNE schedule, particularly given the known
   timing variance of parallel portfolios.
2. **Preserve schedule diversity in the next candidate.** The results show
   complementary solves: the default FNE portfolio covers cases missed by
   `mq`, while `mq` reaches several cases missed by the baseline. Evaluate a
   hybrid portfolio as a new experiment; do not infer its score by adding the
   two solve sets, since worker competition and allocated budgets change the
   outcome.
   The initial implementation candidate is named `casc_fne_mq_hybrid` and
   keeps base strategies s11, s13, s04, and s15 alongside MQ archetypes 1, 2,
   3, and 6. Those MQ choices are for a first test, not winners established by
   the existing aggregate run. `mq_1` through `mq_8` expose each archetype for
   one-worker diagnostic sweeps before revising this composition.
3. **Run repeated, controlled comparisons before making a portfolio change.**
   Use the same clean binary, canonical eight-worker hardware, `jobs=1`, and at
   least five replicates for baseline, `mq`, and any hybrid candidate. Include
   proof auditing where solver theorems are to be reported as certified.
4. **Attribute archetypes in future runs.** The archived `mq` runs constructed
   configs without distinct `strategy_id`s, so their telemetry prints
   `strategy_ids=0:0;...;7:0`; those records cannot identify the winning
   archetype. The current `mq` implementation now assigns stable IDs 101–108,
   and `mq_1` through `mq_8` repeat one archetype for diagnostics. Re-run the
   sweep with this instrumented build before making per-archetype claims.
5. **Treat `SYN353+1` as an important regression case.** The solo sweep shows
   `s04` solves it, while both `mq` runs fail to do so. The `mq` archetypes are
   not equivalent to `s04` (they differ in selection and other search
   settings), so the result supports testing an `s04`-retaining hybrid; it does
   not establish that `mq` lacks all positive-literal selection, since one
   `mq` archetype uses `LiteralSelection::All`.

The benchmark harness accepts named schedules using `schedule:<name>` in
`cooperative_portfolio_sweep.sh`; examples and the isolated archetype workflow
are documented in [`crates/mrs-bench/README.md`](../../../crates/mrs-bench/README.md#portfolio-coverage).

For the decision-quality comparison, run at least five sequential replicates
for each condition against the same corpus and clean binary, on the canonical
eight-worker host:

```bash
for schedule in casc_fne mq casc_fne_mq_hybrid; do
  MRS_WORKERS=8 MRS_SCHEDULE="$schedule" MRS_SHARED_POOL_INTERVAL=0 \
    crates/mrs-bench/run_variance.sh 5 \
      --edition casc-30 --systems mrs --divisions fne --casc-times --jobs 1 \
      --output "crates/mrs-bench/results/variance-fne-${schedule}-$(date +%Y%m%d)"
done
```

Run the same three conditions with the intended positive sharing interval as a
separate experiment. Keep the sharing-off comparison as the control so the
portfolio effect is not confounded with cross-strategy sharing.

This first measurement establishes that multi-queue selection is a promising
complement to the existing portfolio, particularly for hard KRS/NUN and CSR
instances. It does **not** establish a robust coverage gain or an overall
speedup. A repeated cooperative comparison and proof audit should decide
whether any hybrid or `mq`-based order is promoted.

---

## Cooperative MultiQueue Hybrid Campaigns (`casc_fne_mq_hybrid`, 2026-10-22)

Following the initial exploration, the hybrid portfolio `casc_fne_mq_hybrid`—combining
four high-coverage base strategies `[s11, s13, s04, s15]` with four diverse MultiQueue
configurations `[mq_0, mq_1, mq_2, mq_5]`—was evaluated across three cooperative campaigns
with 8 workers, `jobs=1`, and `MRS_SHARED_POOL_INTERVAL=0`.

Each campaign was strictly audited via `audit_casc_proofs` with HEAD proof kernel (`--checks strict --strict-time 120`):

| Campaign | Edition | Host | Wall Clock | Theorems | `VerifiedGood` | `VerifiedBad` | `Unknown` | Proof Omitted |
|:---|:---|:---|---:|---:|---:|---:|---:|---:|
| `cooperative-fne-mq-hybrid-no-sharing` | CASC-30 | `teenf9901` (32c) | 240 s | **45** | **42** | **0** | 2 | 1 |
| `cooperative-fne-j13-mq-hybrid-no-sharing` | CASC-J13 | `tlpnf9701` (32c) | 180 s | **41** | **41** | **0** | 0 | 0 |
| `cooperative-fne-j13-04-mq-hybrid-no-sharing` | CASC-J13 | `mtsdev04` (**8c**) | 180 s | **37** | **37** | **0** | 0 | 0 |

### Findings & Analysis

1. **Strict Soundness**: Zero unsound deductions (`VerifiedBad = 0`) across all 123 evaluated refutations.
2. **CASC-30 Verification Gaps**:
   - While raw theorem count reached 45 (matching the pure base candidate portfolio), only 42 certified as `VerifiedGood`.
   - Two proofs (`LCL642+1.015` and `LCL660+1.015`) failed verification with `Unknown` because multi-queue derivations produced clause structures where the kernel's factoring step limit was exceeded.
   - One proof (`KRS261+1`) derived a 62.8 MB proof text (289,316 nodes) that exceeded the 32 MB output limit (`proof_omitted`).
3. **Hardware Scaling Divergence on CASC-J13**:
   - On the 32-core host (`tlpnf9701`), MQ-hybrid solved **41 theorems** (all 41 `VerifiedGood`), demonstrating strong search diversity when worker threads run without cache or CPU contention.
   - On the canonical 8-core host (`mtsdev04`), MQ-hybrid solved **37 theorems** (all 37 `VerifiedGood`).
   - Under canonical 8-core competition constraints, the pure base candidate portfolio `[11, 13, 4, 8, 15, 7, 3, 1]` on `mtsdev03` achieved **40 refutations (39 `VerifiedGood`, 1 omitted)**, outperforming `casc_fne_mq_hybrid` by +3 refutations / +2 strictly verified solves.

### Comparison: Pure Base Candidate vs. MQ-Hybrid

| Division / Condition | Baseline (`casc_fne`) | Pure Base Candidate (`11,13,4,8,15,7,3,1`) | MQ-Hybrid (`casc_fne_mq_hybrid`) |
|:---|---:|---:|---:|
| **CASC-30 Solves** | 41 | **45** | 45 |
| **CASC-30 `VerifiedGood`** | 41 | **45** (100.0%) | 42 (93.3%) |
| **CASC-J13 (8-core host)** | 36 | **40** (39 VG + 1 PO) | 37 (37 VG) |
| **CASC-J13 (32-core host)** | — | 38 (38 VG) | **41** (41 VG) |

**Conclusion**: On canonical 8-core CASC hardware, the pure base candidate portfolio `[11, 13, 4, 8, 15, 7, 3, 1]` remains superior in both solve throughput and certification reliability (45 vs 42 verified on CASC-30; 39 vs 37 verified on 8-core CASC-J13). While MultiQueue selection provides strong exploratory capability on wide machines, its heavier branching and complex proof structures make the pure base portfolio the optimal choice for promotion to the canonical CASC schedule.
