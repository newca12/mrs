# FNE Candidate Cooperative Portfolio Validation Report (October 2026)

> Status: Historical standalone campaign observations. Candidate-versus-shipped
> deltas below are not paired estimates; see §1 and §4 for the current evidence
> qualification and the later FNE control run.
> Campaign directories:
> - CASC-30: `~/crates/mrs-bench/results/casc-30/coop-fne-candidate-20261021` (symlink: `results/coop-fne-candidate-20261021`)
> - CASC-J13 (32c host): `~/crates/mrs-bench/results/casc-j13/coop-fne-candidate-20261021` (symlink: `results/coop-fne-candidate-j13-20261021`)
> - CASC-J13 (8c host): `~/crates/mrs-bench/results/casc-j13/coop-fne-candidate-03-20261022` (symlink: `results/coop-fne-candidate-03-20261022`)
> All runs evaluated candidate portfolio `MRS_PORTFOLIO=11,13,4,8,15,7,3,1` with 8 workers and `MRS_SHARED_POOL_INTERVAL=0`.

---

## 1. Executive Summary

Following the full-time diagnostic solo sweep and greedy set-cover analysis
([`fne-fulltime-portfolio-2026-10.md`](fne-fulltime-portfolio-2026-10.md)), which
identified the omission of Strategy 13 (`FunctionWeightPenalty` + SOS) and the
redundancy of Strategies 10 and 12 in the shipped portfolio, the candidate 8-worker
portfolio:

$$\text{Candidate Portfolio: } [s11, s13, s04, s08, s15, s07, s03, s01]$$

was evaluated cooperatively across three campaigns in **CASC-30** (240 s) and
**CASC-J13** (180 s):

1. **CASC-30 FNE (`coop-fne-candidate-20261021`)**:
   - **45 Theorems solved out of 100** (45.0% raw solve rate).
   - **100% Strict Self-Certification: 45 of 45 `VerifiedGood`**, with **0 `VerifiedBad`**
     and **0 `Unknown`**.
   - **Zero Regression**: Retains all 41 problems solved by the shipped baseline (`casc_fne`
     on 2026-10-19).
   - **Net Improvement: +4 strictly certified theorems (+9.8% relative gain)**:
     Unlocks four hard logic calculi theorems derived by Strategy 13: `LCL370+2` (61.0 s),
     `LCL373+2` (66.6 s), `LCL390+2` (91.7 s), and `LCL403+2` (92.7 s).

2. **CASC-J13 FNE on Canonical 8-Core Hardware (`coop-fne-candidate-03-20261022`)**:
   - Evaluated on `mtsdev03` (8 physical cores, Intel Xeon E5-2407 @ 2.20 GHz, 96 GB RAM).
   - **40 Theorems solved out of 100** (40.0% raw solve rate), up from 36 in the shipped
     baseline.
   - **Strict Self-Certification (Audited with HEAD)**: **39 `VerifiedGood`**, **0 `VerifiedBad`**,
     **0 `Unknown`**, and **1 `Proof Omitted`** (`KRS261+1`, 55.6 MB proof exceeding the 32 MB limit).
   - **Gains vs. Baseline (+4 net refutations / +3 verified)**: Unlocks `LCL373+2` (81.1 s),
     `LCL382+1` (64.8 s), `LCL390+2` (131.1 s), `CSR116+8` (157.7 s), `CSR116+38` (159.1 s),
     and `PRD001+1` (172.4 s), with only 2 borderline timeout losses (`NUN066+1`, `SYO605+1`).

3. **CASC-J13 FNE Comparator on 32-Core Host (`coop-fne-candidate-20261021`)**:
   - Evaluated on `tlpnf9701` (Intel Xeon Silver 4108).
   - **38 Theorems solved out of 100**, with **38 of 38 `VerifiedGood` (100.0%)**,
     **0 `VerifiedBad`**, and **0 `Unknown`**.

4. **Evidence status under the current replication policy**:
   These historical campaigns observed **+4 solves on CASC-30** and **+4
   refutations / +3 verified on CASC-J13** for the candidate against the cited
   shipped runs, but they are not paired estimates. A later CASC-30 paired
   screen kept the FNE portfolio identical in both arms and found 44/100
   `VerifiedGood` in every arm; it is an FNE control, not a candidate-versus-
   shipped FNE comparison. A separate November 5 CASC-30 portfolio-swap run
   compared `[1,2,3,4,8,12,14,15]` with `[1,2,3,4,11,12,14,15]`; its 39-versus-37
   result is also a single cross-host observation, not a promotion result. See
   the [paired FEQ/FNE screen](paired-feq-fne-2026-10-28.md) and the
   [November 5 FNE comparison](fne-casc30-portfolio-s8-vs-s11-2026-11-05.md).

---

## 2. Campaign Provenance & Environment

| Parameter | CASC-30 (`20261021`) | CASC-J13 (`20261021`) | CASC-J13 8-Core (`20261022`) |
|:---|:---|:---|:---|
| **Directory** | `casc-30/coop-fne-candidate-20261021` | `casc-j13/coop-fne-candidate-20261021` | `casc-j13/coop-fne-candidate-03-20261022` |
| **Run ID** | `mrs_bench_20261021_193418_8d599c23` | `mrs_bench_20261021_192953_fe05bc16` | `mrs_bench_20261022_100700_b989a98c` |
| **Timestamp** | 2026-10-21 19:34:18 | 2026-10-21 19:29:53 | 2026-10-22 10:07:00 |
| **Commit** | `8ccc2e95` (dirty) | `8ccc2e95` (dirty) | `94ec2841` (dirty) |
| **Binary SHA-256** | `1f6eff5b0e0e...` | `387cedc30b58...` | `ae2a634d616f...` |
| **Time Limit** | 240 s (`--casc-times` official) | 180 s (`--casc-times` official) | 180 s (`--casc-times` official) |
| **Workers** | 8 | 8 | 8 |
| **Portfolio** | `11,13,4,8,15,7,3,1` | `11,13,4,8,15,7,3,1` | `11,13,4,8,15,7,3,1` |
| **Shared Pool** | Disabled (`0`) | Disabled (`0`) | Disabled (`0`) |
| **Host** | `teenf9901` (32 cores, 128 GB) | `tlpnf9701` (32 cores, 64 GB) | `mtsdev03` (**8 physical cores**, 96 GB) |

---

## 3. Results Breakdown & Audit Summary

### 3.1 Overall Outcomes

| Status | CASC-30 (`20261021`) | CASC-J13 (`20261021`) | CASC-J13 8-Core (`20261022`) |
|:---|---:|---:|---:|
| **Theorem** | **45** (45.0%) | **38** (38.0%) | **40** (40.0%) |
| **Timeout** | 44 (44.0%) | 45 (45.0%) | 45 (45.0%) |
| **GaveUp** | 11 (11.0%) | 14 (14.0%) | 12 (12.0%) |
| **ResourceOut** | 0 (0.0%) | 3 (3.0%) | 3 (3.0%) |
| **Error / Panic** | 0 (0.0%) | 0 (0.0%) | 0 (0.0%) |
| **Total** | **100** | **100** | **100** |

### 3.2 Verification (Strict Proof Audit)

Audited via `audit_casc_proofs` with strict proof-kernel replay (`--checks strict --strict-time 120`):

| Verdict | CASC-30 (`20261021`) | CASC-J13 (`20261021`) | CASC-J13 8-Core (`20261022`) | Notes |
|:---|---:|---:|---:|:---|
| **`VerifiedGood`** | **45** (100.0%) | **38** (100.0%) | **39** (97.5%) | Sound proofs certified by strict kernel |
| **`VerifiedBad`** | **0** (0.0%) | **0** (0.0%) | **0** (0.0%) | **Zero unsound inferences detected** |
| **`Unknown`** | **0** (0.0%) | **0** (0.0%) | **0** (0.0%) | Zero replay gaps across all runs |
| **`Timeout`** | **0** (0.0%) | **0** (0.0%) | **0** (0.0%) | Zero checker timeouts |
| **`Proof Omitted`** | **0** (0.0%) | **0** (0.0%) | **1** (2.5%) | `KRS261+1` (55.6 MB proof > 32 MB limit) |
| **Total Refutations** | **45** | **38** | **40** | |

---

## 4. Head-to-Head Comparison against Shipped Baseline

### 4.1 CASC-30 FNE (240 s)

Comparing Candidate `11,13,4,8,15,7,3,1` against Shipped Baseline (`campaign-casc30-fne-W8CASCHJ1-99-20261019`):

- **Baseline Solves**: 41 / 100 (41 `VerifiedGood`, 0 `VerifiedBad`)
- **Candidate Solves**: **45 / 100** (**45 `VerifiedGood`**, 0 `VerifiedBad`)
- **Retained Solves**: **41 / 41 (100.0%)** — zero regressions on previously solved problems.
- **Newly Solved Problems (+4)**:

| Problem | Solved By | Candidate Time | Peak RAM | Baseline Status | Notes |
|:---|:---|---:|---:|:---|:---|
| **`LCL370+2`** | `s13` (slot 1) | 61.03 s | 8,206 MB | Timeout (238.4 s) | Logic Calculi; 1,606 nodes |
| **`LCL373+2`** | `s13` (slot 1) | 66.58 s | 11,278 MB | Timeout (238.4 s) | Logic Calculi; 1,643 nodes |
| **`LCL390+2`** | `s13` (slot 1) | 91.71 s | 11,278 MB | Timeout (238.4 s) | Logic Calculi; 1,749 nodes |
| **`LCL403+2`** | `s13` (slot 1) | 92.71 s | 13,327 MB | Timeout (238.3 s) | Logic Calculi; 1,781 nodes |

All four newly observed solves in this campaign were produced by Strategy 13,
consistent with the set-cover analysis. This single campaign is not a replicated
validation of the portfolio gain.

### 4.2 CASC-J13 FNE (180 s)

Comparing Candidate `11,13,4,8,15,7,3,1` on `mtsdev03` against Shipped Baseline (`campaign-casc-j13-W8C8J1-20261016`):

- **Baseline Solves**: 36 / 100 (36 `VerifiedGood`, 0 `VerifiedBad`)
- **Candidate Solves**: **40 / 100** (**39 `VerifiedGood`**, 1 `Proof Omitted`, 0 `VerifiedBad`)
- **Net Solve Delta**: **+4 refutations / +3 verified**.
- **Newly Solved Problems (+6)**:
  - `LCL373+2` (81.09 s, `VerifiedGood`)
  - `LCL382+1` (64.77 s, `VerifiedGood`)
  - `LCL390+2` (131.06 s, `VerifiedGood`)
  - `CSR116+8` (157.74 s, `VerifiedGood`)
  - `CSR116+38` (159.08 s, `VerifiedGood`)
  - `PRD001+1` (172.42 s, `VerifiedGood` on 1,614-axiom problem)
- **Borderline Losses (-2)**:
  - `NUN066+1`: timed out on Oct 22 (solved on Oct 21 in 27.5 s; baseline was 31.2 s).
  - `SYO605+1`: gave up at 180 s (baseline was 175.7 s).

### 4.3 Comparison against MultiQueue Hybrid (`casc_fne_mq_hybrid`)

Parallel evaluations of the experimental `casc_fne_mq_hybrid` portfolio
([`fne-multiqueue-2026-10.md`](fne-multiqueue-2026-10.md)) provide an unpaired
comparison with the pure base candidate portfolio `[11, 13, 4, 8, 15, 7, 3, 1]`.
The observed counts are summarized below; they do not establish a general
production ranking:

| Edition / Condition | Shipped Baseline | MultiQueue Hybrid (`casc_fne_mq_hybrid`) | Candidate Portfolio (`11,13,4,8,15,7,3,1`) |
|:---|---:|---:|---:|
| **CASC-30 Solves** | 41 | 45 | **45** |
| **CASC-30 `VerifiedGood`** | 41 | 42 (2 `Unknown`, 1 omitted) | **45** (100.0% verified) |
| **CASC-J13 (8-core host)** | 36 | 37 (37 VG) | **40** (39 VG + 1 omitted) |
| **CASC-J13 (32-core host)** | — | **41** (41 VG) | 38 (38 VG) |

In these unpaired campaign observations, the pure base candidate portfolio
outperformed both the cited shipped baseline (+4 in C30, +4 in J13) and
`casc_fne_mq_hybrid` (+3 verified on C30, +2 verified on J13 8c), while
avoiding verifier factoring step-limit timeouts.

The later paired FEQ experiment used this same FNE order, `[11, 13, 4, 8, 15,
7, 3, 1]`, in both binary arms. All four completed pairs certified the same 44
FNE problems per arm, with no per-problem FNE flips. This does not re-test the
candidate against the shipped FNE portfolio; the historical single-run
comparison above remains a separate observation.

---

## 5. Domain Performance Breakdown

Performance across TPTP problem domains for the candidate portfolio:

| Domain | Description | C30 Solved / Total | J13 (8c) Solved / Total |
|:---|:---|---:|---:|
| **CSR** | Commonsense Reasoning | 20 / 24 (83.3%) | 19 / 22 (86.4%) |
| **LCL** | Logic Calculi | **11 / 37** (29.7%, up from 7) | **6 / 37** (16.2%, up from 3) |
| **KRS** | Knowledge Representation | 4 / 10 (40.0%) | 7 / 11 (63.6%) |
| **NUN** | Non-standard Number Theory | 3 / 9 (33.3%) | 4 / 13 (30.8%) |
| **SYN** | Syntactic | 2 / 4 (50.0%) | 1 / 2 (50.0%) |
| **MED** | Medicine / Physiology | 2 / 2 (100.0%) | — |
| **GEO** | Geometry | 1 / 3 (33.3%) | 1 / 1 (100.0%) |
| **SYO** | Syntactic (Other) | 1 / 2 (50.0%) | 0 / 1 (0.0%) |
| **MGT** | Management / Organization | 1 / 1 (100.0%) | 1 / 1 (100.0%) |
| **PRD** | Products / Processes | — | 1 / 1 (100.0%) |

---

## 6. Timing, Memory, and Throughput Metrics

| Metric | CASC-30 (`20261021`) | CASC-J13 8-Core (`20261022`) |
|:---|---:|---:|
| **Fastest Solve** | 0.065 s (`NUN081+1`) | 0.145 s (`NUN081+1`) |
| **Median Solve Time** | 1.976 s | 3.607 s |
| **Mean Solve Time** | 19.028 s | 35.125 s |
| **Slowest Solve** | 218.191 s (`CSR116+6`) | 172.420 s (`PRD001+1`) |
| **Mean Peak RAM on Solves** | 4,144 MB | 5,046 MB |
| **Max Peak RAM on Solves** | 13,327 MB (`LCL403+2`) | 17,586 MB (`PRD001+1`) |
| **Max Campaign Peak RAM** | 70,669 MB (`BOO109+1`) | 58,864 MB (`NLP261+1`) |
| **Processed Clauses** | 9,589,929 | 8,233,438 |
| **Generated Clauses** | 105,372,133 | 78,947,784 |

---

## 7. Conclusions & Evidence Status

The cited comparisons recorded higher counts for the candidate: +4 strictly
verified refutations on CASC-30 (45 vs. 41) and
+4 raw refutations / +3 strictly verified refutations on the 8-core CASC-J13
run (40 raw, 39 verified, vs. 36). These are single-run, unpaired observations
from different artifacts and, in some cases, different hosts; they are not
estimates of a stable portfolio effect.

The four new CASC-30 LCL solves in the October campaign were attributed to
strategy 13 in that run. This is useful cooperative evidence for candidate
generation, but it does not prove a general strategy effect. Likewise, the
strict audits detected no `VerifiedBad` proofs in these campaigns, but the
CASC-J13 8-core campaign had one proof omission (`KRS261+1`); audit results
describe these archived outputs, not global soundness.

**Do not treat this report alone as a schedule-promotion recommendation.** The
later paired FNE screen used the same FNE portfolio in both arms and therefore
was a control, not a candidate-versus-shipped comparison. The November 5
CASC-30 swap of strategy 8 for strategy 11 is another one-run, cross-host
observation and does not establish which strategy caused the solve-set flips.
See the [paired FEQ/FNE screen](paired-feq-fne-2026-10-28.md) and the
[November 5 s8/s11 comparison](fne-casc30-portfolio-s8-vs-s11-2026-11-05.md).

Before changing `CASC_FNE_ORDER`, run randomized paired cooperative campaigns
on the same host and binary, report each pair's certified delta and per-problem
flips, and validate selected portfolios on held-out problems or families, as
required by [methodology §8.5](../../policies/methodology.md#85-experimental-design-and-evidence-strength).
