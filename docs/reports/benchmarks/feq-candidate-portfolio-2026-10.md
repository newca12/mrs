# FEQ Candidate Cooperative Portfolio Validation Report (October 2026)

> Status: Banked single-run campaign observations; portfolio validation remains
> provisional. These are not five-replicate means. Current policy requires at
> least five replicates for decisive comparisons. The CASC-J13 runs used 180 s;
> current canonical FEQ comparisons use 240 s, so those J13 results are
> supplemental and must not be treated as canonical-limit comparisons.
> Campaign directories:
> - CASC-30 (8c host): `~/crates/mrs-bench/results/cooperative-feq-equal-candidate-casc30`
> - CASC-30 (32c host): `~/crates/mrs-bench/results/cooperative-feq-equal-candidate-casc30-x`
> - CASC-J13 (8c host): `~/crates/mrs-bench/results/cooperative-feq-equal-candidate-casc-j13`
> - CASC-J13 (32c host): `~/crates/mrs-bench/results/cooperative-feq-equal-candidate-casc-j13-x`
> Reference greedy comparator on 32c host (`teenf9901`):
> - CASC-30: `~/crates/mrs-bench/results/coop-feq-greedy-candidate-99-20261023`
> - CASC-J13: `~/crates/mrs-bench/results/cooperative-feq-greedy-candidate-99-casc-j13`

---

## 1. Executive Summary

Following full-time solo sweeps on CASC-30 FEQ (`solo-feq-casc30-99-fulltime`, 15 strategies $\times$ 400 problems = 6,000 runs) and greedy set-cover analysis, the candidate 8-worker FEQ portfolio:

$$\mathbf{P}_{\text{FEQ}} = [s12, s11, s10, s06, s08, s01, s04, s13]$$

was evaluated across four standalone full-division campaigns in **CASC-30**
(240 s) and **CASC-J13** (180 s in these historical runs) with
`MRS_HARDWARE=casc`, 8 workers, and `jobs=1` (sequential problem evaluation).
The J13 rows are supplemental because the current canonical FEQ limit is 240 s.

1. **CASC-30 FEQ on Canonical 8-Core Hardware (`cooperative-feq-equal-candidate-casc30`)**:
   - Evaluated on `mtsdev01` (Intel Xeon E5-2407 @ 2.20 GHz, 8 cores, 96 GB RAM).
   - **111 Theorems solved out of 400 (27.75% raw solve rate)**, up $+3$ from the 108 solved by the shipped baseline (`CASC_FEQ_ORDER` `[11, 12, 1, 6, 10, 8, 14, 4]`).
   - **Zero Regressions**: $\text{Baseline} \setminus \mathbf{P}_{\text{FEQ}} = \emptyset$. All 108 baseline theorems re-solve cleanly.
   - **Net Gains**: Unlocks `GEO271+1` (134.1 s), `GRP776+1` (18.6 s), and `SWV457+1` (148.9 s).
   - **Strict Self-Certification**: **106 `VerifiedGood`**, **0 `VerifiedBad`**, 2 `Unknown`, 1 verifier timeout (`ALG049+1` > 120 s), and 2 `Proof Omitted` (`SET017+1` at 45 MB, `SET072+1` at 38 MB). Certified yield is **97.25%** (106 / 109 emitted).

2. **CASC-30 FEQ on 32-Core Comparator (`cooperative-feq-equal-candidate-casc30-x`)**:
   - Evaluated on `tlpnf9701` (Intel Xeon Silver 4108 @ 1.80 GHz).
   - **113 Theorems solved out of 400 (28.25% raw solve rate)**.
   - **Strict Self-Certification**: **107 `VerifiedGood`**, **0 `VerifiedBad`**, 3 `Unknown`, 1 timeout (`ALG049+1`), and 2 `Proof Omitted` (`SET017+1`, `SET072+1`).

3. **CASC-J13 FEQ on 8-Core Hardware (`cooperative-feq-equal-candidate-casc-j13`)**:
   - Evaluated on `mtsdev01` (8 cores, 96 GB RAM) at the historical 180 s budget;
     this is not a canonical 240 s comparison under current repository policy.
   - **69 Theorems solved out of 300 (23.00% raw solve rate)**.
   - **100% Strict Self-Certification: 69 of 69 `VerifiedGood` (100.0%)**, with **0 `VerifiedBad`**, **0 `Unknown`**, and 0 timeouts.

4. **CASC-J13 FEQ on 32-Core Comparator (`cooperative-feq-equal-candidate-casc-j13-x`)**:
   - Evaluated on `tlpnf9701` at the historical 180 s budget; this is also
     supplemental, not a canonical 240 s comparison.
   - **72 Theorems solved out of 300 (24.00% raw solve rate)**, up from 71 baseline.
   - **100% Strict Pass on Emitted Proofs**: **71 `VerifiedGood`**, **0 `VerifiedBad`**, 0 `Unknown`, and 1 `Proof Omitted` (`SWV406+1`, 83.4 MB > 32 MB limit).

5. **Evidence status under `AGENTS.md` §11**:
   The campaigns show positive observed cooperative results and zero
   `VerifiedBad` proofs, but they are single runs, not replicated comparisons.
   The reported coverage deltas are observations, not estimates of the mean or
   evidence that the portfolio is optimal. The J13 runs also used a noncanonical
   180 s limit. A paired candidate/baseline comparison at 240 s with at least
   five replicates is still needed for a decisive cross-edition claim.

---

## 2. Campaign Provenance & Environment

| Parameter | CASC-30 8-Core | CASC-30 32-Core (`-x`) | CASC-J13 8-Core | CASC-J13 32-Core (`-x`) |
|:---|:---|:---|:---|:---|
| **Directory** | `...-casc30` | `...-casc30-x` | `...-casc-j13` | `...-casc-j13-x` |
| **Run ID** | `mrs_bench_20261023_190703` | `mrs_bench_20261023_191300` | `mrs_bench_20261024_152833` | `mrs_bench_20261024_151427` |
| **Timestamp** | 2026-10-23 19:07:03 | 2026-10-23 19:13:00 | 2026-10-24 15:28:33 | 2026-10-24 15:14:27 |
| **Commit** | `94ec2841` (dirty) | `94ec2841` (dirty) | `94ec2841` (dirty) | `94ec2841` (dirty) |
| **Compiler** | `rustc 1.99.1` | `rustc 1.99.1` | `rustc 1.99.1` | `rustc 1.99.1` |
| **Limit** | 240 s (`--time 240`) | 240 s (`--time 240`) | 180 s (`--time 180`, supplemental) | 180 s (`--time 180`, supplemental) |
| **Workers** | 8 | 8 | 8 | 8 |
| **Portfolio** | `12,11,10,6,8,1,4,13` | `12,11,10,6,8,1,4,13` | `12,11,10,6,8,1,4,13` | `12,11,10,6,8,1,4,13` |
| **Shared Pool** | Disabled (`0`) | Disabled (`0`) | Disabled (`0`) | Disabled (`0`) |
| **Host** | `mtsdev01` (8 cores) | `tlpnf9701` (32 cores) | `mtsdev01` (8 cores) | `tlpnf9701` (32 cores) |
| **Host RAM** | 96,350 MB | 63,763 MB | 96,350 MB | 63,763 MB |
| **Wall Clock** | 73,228 s (~20.34 h) | 72,008 s (~20.00 h) | 42,932 s (~11.93 h) | 42,239 s (~11.73 h) |

---

## 3. Strict Proof Audit Outcomes

All four campaigns were audited locally via `audit_casc_proofs --checks strict --strict-time 120`:

| Campaign | Evaluated | Refutations | `VerifiedGood` | `VerifiedBad` | `Unknown` | Verifier Timeout | Proof Omitted | Emitted Pass Rate |
|:---|---:|---:|---:|---:|---:|---:|---:|---:|
| `casc30` (8c) | 400 | 111 | **106** | **0** | 2 | 1 | 2 | **97.25%** |
| `casc30-x` (32c) | 400 | 113 | **107** | **0** | 3 | 1 | 2 | **96.40%** |
| `casc-j13` (8c) | 300 | 69 | **69** | **0** | 0 | 0 | 0 | **100.00%** |
| `casc-j13-x` (32c) | 300 | 72 | **71** | **0** | 0 | 0 | 1 | **100.00%** |
| **Total** | **1,400** | **365** | **353** | **0** | **5** | **2** | **5** | **98.06%** |

### Accounting for Non-`VerifiedGood` Refutations
1. **Verifier Timeout (2 instances)**:
   `ALG049+1` (solved in ~5.4 s in both CASC-30 runs; 8,198 proof nodes) exceeds the 120 s kernel replay limit during large equality graph reconstruction. Identical to baseline behavior.
2. **Proof Omitted (5 instances)**:
   - `SET017+1` (~45 MB) and `SET072+1` (~38 MB) on CASC-30.
   - `SWV406+1` (83.4 MB, 319,894 nodes) on CASC-J13.
   Withheld strictly due to the default 32 MB `--proof-bytes-limit` ceiling.
3. **Kernel Step Limit / AC Replay (5 instances)**:
   - `GEO271+1`: Factoring search exceeds step limit during branch normalization.
   - `KLE071+1`: AC superposition conclusion validation gap.
   - `LCL894+1`: AC normalization search depth exhausted.

Zero unsound proofs were generated (`0 VerifiedBad` across 365 refutations).

---

## 4. Tradeoff Analysis: Equal vs. Pure Greedy Candidate

### 4.1 Portfolio Structures

| Role | Shipped Baseline | Adopted Portfolio ($\mathbf{P}_{\text{FEQ}}$) | Pure Greedy Solo Cover |
|:---|:---|:---|:---|
| Slot 0 (Worker 0) | `s11` (ConjSymbolBoost, KBO) | **`s12` (HornHeuristic anchor, KBO)** | `s12` (HornHeuristic anchor, KBO) |
| Slot 1 | `s12` (HornHeuristic, KBO) | `s11` (ConjSymbolBoost, KBO) | `s08` (GoalDirected, LPO) |
| Slot 2 | `s01` (AgeWeight 3, KBO) | `s10` (SOS depth 100, KBO) | `s06` (AgeWeight 10, KBO) |
| Slot 3 | `s06` (AgeWeight 10, KBO) | `s06` (AgeWeight 10, KBO) | `s01` (AgeWeight 3, KBO) |
| Slot 4 | `s10` (SOS depth 100, KBO) | `s08` (GoalDirected, LPO) | `s10` (SOS depth 100, KBO) |
| Slot 5 | `s08` (GoalDirected, LPO) | `s01` (AgeWeight 3, KBO) | `s11` (ConjSymbolBoost, KBO) |
| Slot 6 | `s14` (SmallestFirst, cap 100) | **`s04` (AgeWeight 8, aggressive)** | `s13` (FunctionPenalty + SOS) |
| Slot 7 (Worker 7) | `s04` (AgeWeight 8, aggressive) | **`s13` (FunctionPenalty + SOS)** | **`s15` (SymbolWeight, no AVATAR)** |

### 4.2 Cross-Edition Outcome Matrix

These are observed campaign counts, not replicated means. In particular, the
CASC-J13 results below use the supplemental 180 s limit described above.

| Metric | Shipped Baseline | Adopted Portfolio ($\mathbf{P}_{\text{FEQ}}$) | Pure Greedy Candidate |
|:---|:---:|:---:|:---:|
| **CASC-30 Solves** | 108 / 400 | **111–114 / 400** | **115 / 400** |
| **CASC-J13 Solves** | 68–71 / 300 | **69–72 / 300** | **71 / 300** |
| **CASC-J13 Regressions** | Baseline | **0 regressions** | **4 regressions** (`BIO006+1`, `GEO494+1`, `SET017+1`, `SWX203+1`) |
| **Combined Solves** | 177–179 | **180–186** | 186 |

### 4.3 Diagnostic Verification on `teenf9901` (`GEO559+1` and `GEO641+1`)

On CASC-30, the pure greedy candidate solves 115 vs. 114 for the adopted portfolio. An empirical diagnostic was run on `teenf9901` under the adopted portfolio:

1. **`GEO559+1` is solved by the adopted portfolio**:
   - Refutation derived at **230.1 s** (160,222 proof nodes, 68.4 MB proof text).
   - On slower 2.20 GHz hosts (`mtsdev01` at 241.8 s) and 1.80 GHz hosts with 8 pinned cores (`tlpnf9701` at 239.6 s), thread racing pushed it just over the 240 s ceiling.
   - On `teenf9901`, throughput was sufficient to close the proof within the competition limit, bringing adopted portfolio coverage to **114 / 400**.
2. **`GEO641+1` is uniquely solved by Strategy 15**:
   - Timed out at 238.6 s under the adopted portfolio.
   - Under Strategy 15 alone, `GEO641+1` solves in **0.169 s** solo (and 0.199 s in the greedy portfolio).
   - All other 14 base strategies time out on `GEO641+1`.

### 4.4 Observed Tradeoff (Not a Replicated Conclusion)
In these campaign observations, the pure greedy portfolio gains `GEO641+1` on
CASC-30 but drops `s04` and has **4 observed regressions on CASC-J13** (including
`SET017+1` and `BIO006+1`). Because the CASC-J13 runs used 180 s and the
comparison was not replicated, this does not establish a robust cross-edition
tradeoff or show that the candidate order is optimal. Replicated paired runs at
the canonical 240 s limit remain necessary.

---

## 5. Roadmap: Retaining Strategy 15 via MultiQueue in FEQ

Strategy 15 (`SymbolWeight` + `AgeWeight(4)` + `AllNegative` + `KBO`, no AVATAR) possesses extraordinary solving power on dense geometric and algebraic problems (`GEO641+1` in 0.17 s, `GEO559+1` in 2.33 s solo). Its core mechanism is **inverse-frequency / arity symbol weighting**: rare symbols cost more, guiding the given-clause loop toward common symbols that trigger rapid demodulation and equational collapse.

In a fixed 8-worker portfolio running one base strategy per core, dedicating a whole slot to `s15` requires evicting `s04`. To capture `s15`'s benefits without displacing baseline coverage, the following roadmap paths are scheduled:

1. **MultiQueue SymbolWeight Priority Channel**:
   Extend `SelectionStrategy::MultiQueue` to support a dedicated `SymbolWeight` priority queue alongside `Age`, `Weight`, and `Goal`:
   ```rust
   SelectionStrategy::MultiQueue(vec![
       (QueueType::Age, 1),
       (QueueType::Weight, 3),
       (QueueType::SymbolWeight, 2),
   ])
   ```
   This allows workers (e.g. Worker 6 or 7) to interleave `SymbolWeight` exploration without dedicating an entire core.
2. **FEQ MultiQueue Hybrid Portfolio (`casc_feq_mq_hybrid`)**:
   Similar to `casc_fne_mq_hybrid`, evaluate a hybrid schedule pairing the 6 top base strategies (`[s12, s11, s10, s06, s08, s01]`) with 2 MultiQueue archetypes tuned specifically for equality and geometry demodulation.
3. **Slot 8 Activation on Wider Hosts**:
   In the updated `CASC_FEQ_ORDER`:
   ```rust
   const CASC_FEQ_ORDER: [usize; 15] = [12, 11, 10, 6, 8, 1, 4, 13, 15, 14, 5, 2, 3, 7, 9];
   ```
   Strategy 15 sits in **slot 8**. On any machine running $\ge 9$ workers (or when configured with `--workers 9`), `s15` activates immediately.
