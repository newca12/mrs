# The mrs pre-phase at competition budget: 240-second strategy sweeps on CASC-30 and CASC-J13

*Date: 2026-11-02. Follow-up to `2026-10-prephase-study.md`.*
*Artifacts: `/home/hack/results/prephase/` (`casc30_sweep_t240.csv`, `cascj13_sweep_t240.csv`, `casc30_features_probed.csv`, `cascj13_features_probed.csv`).*

## 1. Executive Summary

The initial pre-phase study (`2026-10-prephase-study.md`) investigated whether pre-search syntactic analysis could dynamically route problems to specialized strategies better than static per-division portfolios. Evaluated at 3 s (600 problems) and 30 s (127 hard-tail problems), the initial study concluded that syntactic routing underperformed fixed portfolios, but noted that a re-measurement at the full competition budget (240 s) remained necessary to settle whether the gap was an artifact of short time horizons.

This report records the decisive experiment: **full 240-second solo sweeps across all 15 base strategies for every refutation problem in both CASC-30 and CASC-J13** (26,805 solo runs).

### Headline Results

1. **Pre-phase syntactic routing definitively loses at 240 s.**
   At the canonical 8-worker CASC hardware width:
   - On **CASC-30**: dynamic syntactic routing solves **140 problems**, compared to **240** for the per-division portfolio (`casc_div`) and **204** for generic `casc` (**-100 deficit vs `casc_div`**, $p = 0.0000$, discordant: 110, routed-only: 5, division-only: 105).
   - On **CASC-J13**: dynamic syntactic routing solves **139 problems**, compared to **236** for `casc_div` and **204** for generic `casc` (**-97 deficit vs `casc_div`**, $p = 0.0000$, discordant: 99, routed-only: 1, division-only: 98).
   - Two-fold cross-validation confirms out-of-fold routing generalisation loses 39 to 44 problems relative to static schedules. The open question from §8 of the initial study is settled: dynamic syntactic routing fails to beat fixed portfolios at any budget.

2. **Strategy 12 is the dominant solo engine of MRS at 240 s.**
   Strategy 12 (`HornHeuristic` + AgeWeight(5) + AllNegative + KBO, no AVATAR, no weight cap) is the single strongest strategy by a wide margin across both benchmarks:
   - Solves **256 problems solo** across CASC-30 and CASC-J13 (62.0% of all 413 solvable problems).
   - Carries **38 unique solves** that no other base strategy can solve within 240 s.
   - Dominates both FEQ (112 solo solves across both corpora) and FNE (34 solo solves).

3. **Immediate portfolio tuning opportunities identified in `named.rs`:**
   - **Default 8-worker CASC schedule (`DEFAULT_CASC_ORDER`)**: Historically ran `[1, 2, 3, 4, 5, 6, 7, 8]` on 8 cores, leaving out `s12` (slot 12), `s11` (slot 11), and `s15` (slot 15). At 240 s, slots 1–8 of the default schedule solve **329 problems**; a greedy set-cover 8-strategy portfolio (`[12, 6, 11, 1, 8, 15, 2, 13]`) covers **403 problems** (**+74 problems / +22.5%** solo gain).
   - **FNE schedule (`CASC_FNE_ORDER`)**: Currently places Strategy 12 at slot 10, outside the 8-worker window. In reality, `s12` is the #1 strategy in FNE (34 solo solves). Promoting `s12` into the top 8 raises FNE 8-worker coverage from 45 to **55 solves (+10)**.
   - **UEQ schedule (`CASC_UEQ_ORDER`)**: Currently places Strategy 6 at slot 11, despite it having 113 solo solves in UEQ. Reordering to prioritize `[1, 11, 8, 12, 2, 6, 15, 3]` increases solo set-cover from 191 to **196 solves (+5)**.

---

## 2. Dataset and Corpus Coverage

The experiment covers all refutation problem files from the CASC-30 and CASC-J13 competition corpora:

| Dataset | Refutation Divisions | Total Problems | Solved by $\ge 1$ Strategy | Solo Runs ($15 \times N$) |
|---|---|---|---|---|
| **CASC-30** | EPU (97), FEQ (398), FNE (97), ICU (99), UEQ (300) | 991 | 264 (26.6%) | 16,320* |
| **CASC-J13** | FEQ (299), FNE (97), UEQ (400) | 796 | 246 (30.9%) | 11,940 |
| **Combined** | All refutation divisions (1,461 unique problems) | 1,461 | 413 (28.3%) | 26,805 |

*\* Note: In `casc30_refutation_paths.txt`, 97 EPU problems were listed with two path prefixes (`crates/mrs-bench/problems/casc-30/EPU/` and `.../Problems/EPU/`). `prephase/analyze.py` and the analyses here deduplicate by problem name.*

Both corpora were processed through `prephase_dump` with the bounded search-behaviour probe enabled (`probe_iterations = 2000`), yielding full feature vectors joined to the 240 s sweep outcomes.

---

## 3. Pre-Phase Routing Performance at 240 s

Running `crates/mrs-bench/prephase/analyze.py` over the 240 s sweep yields the following simulated portfolio coverage across worker widths:

### CASC-30 Portfolio Simulation ($N = 991$)

| Workers | Oracle (All 15) | Best Fixed | Pre-Phase Routed | Default CASC | Per-Division (`casc_div`) | Delta (Routed vs Division) |
|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| 1 | 264 | 264 | 41 | 104 | 127 | **-86** |
| 2 | 264 | 264 | 63 | 133 | 147 | **-84** |
| 4 | 264 | 264 | 95 | 148 | 197 | **-102** |
| **8** | **264** | **264** | **140** | **204** | **240** | **-100** |
| 12 | 264 | 264 | 172 | 256 | 263 | **-91** |
| 15 | 264 | 264 | 264 | 264 | 264 | 0 |

* **Significance at 8 slots**: Routed+only = 5, Division+only = 105, discordant = 110, two-sided $p = 0.0000$.
* **2-Fold Cross-Validation (Held-out)**: Routed covers 201/991 (20.3%) vs Per-Division baseline 240/991 (24.2%), delta = **-39**.

### CASC-J13 Portfolio Simulation ($N = 796$)

| Workers | Oracle (All 15) | Best Fixed | Pre-Phase Routed | Default CASC | Per-Division (`casc_div`) | Delta (Routed vs Division) |
|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| 1 | 246 | 246 | 43 | 113 | 109 | **-66** |
| 2 | 246 | 246 | 61 | 147 | 133 | **-72** |
| 4 | 246 | 246 | 94 | 163 | 184 | **-90** |
| **8** | **246** | **246** | **139** | **204** | **236** | **-97** |
| 12 | 246 | 246 | 177 | 239 | 245 | **-68** |
| 15 | 246 | 246 | 246 | 246 | 246 | 0 |

* **Significance at 8 slots**: Routed+only = 1, Division+only = 98, discordant = 99, two-sided $p = 0.0000$.
* **2-Fold Cross-Validation (Held-out)**: Routed covers 192/796 (24.1%) vs Per-Division baseline 236/796 (29.6%), delta = **-44**.

### Why Routing Loses at 240 s

The failure mechanism observed at 3 s persists and deepens at 240 s:
1. **High syntactic purity is driven by the unsolved majority.** Across the 991 CASC-30 problems, weighted modal purity is 0.845, but 73.4% (727 problems) are solved by 0 configurations. Conditioned on a problem being solvable, syntactic features do not separate which strategy will succeed.
2. **Specialization starves diversity.** The per-division portfolios (`casc_feq`, `casc_fne`, `casc_ueq`) allocate slots across diverse heuristic archetypes (Horn heuristic, conjecture symbol boosting, age-weight ratios, KBO/LPO). Pre-phase routing attempts to pick a specialized configuration based on clausified shape (e.g. `HORN`, `TIGHT`, `CONNECTED`). When the routing guess is wrong, all assigned workers run variations of the suboptimal algorithm. In contrast, an 8-worker static portfolio guarantees that the problem is simultaneously attacked by complementary calculi and heuristics.

---

## 4. Individual Strategy Characterization at 240 s

Across the combined 1,461 problems (413 solved by any strategy):

| Strategy | Config Key | Solo Solves (Total) | Share of Solved | Unique Solves | Median Time | Mean Time | P90 Time |
|:---:|---|:---:|:---:|:---:|:---:|:---:|:---:|
| **s12** | `HornHeuristic` + AgeWeight(5) + AllNeg + KBO | **256** | **62.0%** | **38** | 20.7 s | 44.9 s | 122.5 s |
| **s6** | AgeWeight(5) + KBO baseline | **221** | **53.5%** | **11** | 13.6 s | 40.1 s | 120.4 s |
| **s1** | AgeWeight(5) + AllNeg + KBO | **181** | **43.8%** | **11** | 37.2 s | 62.9 s | 164.4 s |
| **s11** | `ConjSymbolBoost` + AgeWeight(6) + AllNeg + KBO | **153** | **37.0%** | **11** | 13.3 s | 38.8 s | 122.3 s |
| **s15** | `SymbolWeight` + AgeWeight(4) + AllNeg + KBO | **129** | **31.2%** | **6** | 7.0 s | 40.0 s | 145.1 s |
| **s2** | SmallestFirst + AllNeg + KBO | **120** | **29.1%** | **6** | 3.8 s | 20.5 s | 67.0 s |
| **s8** | AgeWeight(10) + AllNeg + KBO | **113** | **27.4%** | **9** | 4.4 s | 24.7 s | 76.7 s |
| **s4** | AgeWeight(5) + MaxNegOrMaxPos + KBO | **111** | **26.9%** | **1** | 12.8 s | 31.2 s | 94.6 s |
| **s5** | AgeWeight(5) + All + KBO | **104** | **25.2%** | **2** | 17.1 s | 37.5 s | 117.5 s |
| **s14** | `ConjSymbolBoost` + SmallestFirst + All + KBO | **104** | **25.2%** | **1** | 2.1 s | 27.3 s | 128.3 s |
| **s3** | FIFO (pure Age) + AllNeg + KBO | **85** | **20.6%** | **1** | 4.5 s | 31.1 s | 117.8 s |
| **s7** | AgeWeight(2) + AllNeg + KBO | **85** | **20.6%** | **2** | 11.9 s | 46.4 s | 166.2 s |
| **s10** | SOS (`sos_depth=100`) + AgeWeight(12) + KBO | **65** | **15.7%** | **1** | 1.1 s | 35.3 s | 130.1 s |
| **s13** | `FunctionWeightPenalty` + SOS + AgeWeight(5) + KBO | **47** | **11.4%** | **1** | 1.9 s | 29.5 s | 103.8 s |
| **s9** | AgeWeight(5) + AllNeg + LPO | **46** | **11.1%** | **0** | 0.4 s | 10.1 s | 43.9 s |

### Observations on Individual Strategies

1. **The dominance of Strategy 12:** Strategy 12 solves more than 60% of all refutable problems in the corpus. By combining `HornHeuristic` (which penalizes non-Horn clauses to prioritize horn/goal derivations) without the clause-splitting overhead of AVATAR or weight caps, it prevents passive queue bloat on deep equality proofs.
2. **Bimodal timing distributions:**
   - *Fast/shallow filters*: Strategies 9, 10, 13, and 14 have medians below 2.5 s. When they succeed, they find short proofs almost immediately, but fail to scale to deep saturations.
   - *Saturation drivers*: Strategies 1, 6, 11, and 12 have median times between 13 s and 37 s, and maintain productive saturation out to 240 s.
3. **LPO underperformance (s9):** Strategy 9 (LPO ordering) has 0 unique solves across all 1,461 problems. Every problem s9 solved was also solved faster by a KBO strategy.

---

## 5. Per-Division Breakdown & Greedy Set Cover

### Division FEQ (First-Order with Equality)
* 634 unique problems across CASC-30 and CASC-J13; 148 solved by any strategy.
* Solo ranking: s12 (112), s6 (75), s11 (39), s8 (38), s2 (36), s14 (36), s15 (33), s1 (31).
* **Greedy set cover**:
  $$\text{s12 } (+112) \to \text{s6 } (+17) \to \text{s8 } (+9) \to \text{s11 } (+4) \to \text{s13 } (+4) \to \text{s15 } (+2) \implies 148/148 \text{ (100\%) in 6 strategies.}$$
* **Comparison with `CASC_FEQ_ORDER`**:
  * Current order: `[12, 11, 10, 6, 8, 1, 4, 13]` $\implies$ 146 solves at 8 workers.
  * Greedy 8: `[12, 6, 8, 11, 13, 15, ...]` $\implies$ 148 solves (+2).

### Division FNE (First-Order Non-Equality)
* 163 unique problems across CASC-30 and CASC-J13; 55 solved by any strategy.
* Solo ranking: s12 (34), s15 (28), s6 (26), s2 (23), s11 (23), s4 (18), s1 (14), s8 (14).
* **Greedy set cover**:
  $$\text{s12 } (+34) \to \text{s11 } (+10) \to \text{s15 } (+5) \to \text{s14 } (+3) \to \text{s4 } (+2) \to \text{s13 } (+1) \implies 55/55 \text{ (100\%) in 6 strategies.}$$
* **Comparison with `CASC_FNE_ORDER`**:
  * Current order: `[11, 13, 4, 8, 15, 7, 3, 1, 10, 12, ...]` (s12 is at slot 10!).
  * Top 8 in current order solves **45 problems**.
  * Greedy 8 with s12 in slot 1 solves **55 problems** (**+10 problems, +22.2%**).

### Division UEQ (Unit Equality)
* 468 unique problems across CASC-30 and CASC-J13; 201 solved by any strategy.
* Solo ranking: s1 (129), s6 (113), s12 (103), s11 (84), s5 (65), s4 (62), s15 (60).
* **Greedy set cover**:
  $$\text{s1 } (+129) \to \text{s11 } (+25) \to \text{s8 } (+14) \to \text{s12 } (+10) \to \text{s2 } (+7) \to \text{s6 } (+6) \to \text{s15 } (+3) \to \text{s3 } (+2) \implies 196/201 \text{ in 8 strategies.}$$
* **Comparison with `CASC_UEQ_ORDER`**:
  * Current order: `[4, 8, 12, 11, 2, 14, 15, 1, 3, 5, 6, ...]` (s1 is at slot 8, s6 is at slot 11).
  * Top 8 in current order solves **191 problems**.
  * Greedy 8 solves **196 problems** (**+5 problems**).

### Division EPU (EPR Unsatisfiable)
* 97 unique problems in CASC-30; 9 solved by any strategy.
* Greedy set cover: s5 (+9) covers all 9 solvable problems alone. (s4 and s15 each solve 8).

### Division ICU (Inconsistency with Unit Equality)
* 99 problems in CASC-30; **0 solved** across all 15 base strategies at 240 s. Indicates that the general superposition path without specialized lemma synthesis or theory reasoning cannot refute these problems.

---

## 6. Overall Greedy Portfolio vs `DEFAULT_CASC_ORDER`

Across the entire 1,461-problem corpus (all divisions combined):

$$\begin{aligned}
\text{Oracle (All 15 strategies):} \quad & 413 \text{ problems} \\
\text{Current Default CASC 1–8: } \quad & 329 \text{ problems (79.7\% of oracle)} \\
\text{Greedy 8-strategy Portfolio: } \quad & \mathbf{403} \text{ problems (97.6\% of oracle)} \\
\text{Gain: } \quad & \mathbf{+74} \text{ problems (+22.5\% increase on 8 workers)}
\end{aligned}$$

The overall greedy 8-strategy priority sequence is:
1. **s12** (+256 solves, 62.0%)
2. **s6** (+61 solves, 76.8%)
3. **s11** (+33 solves, 84.7%)
4. **s1** (+18 solves, 89.1%)
5. **s8** (+14 solves, 92.5%)
6. **s15** (+10 solves, 94.9%)
7. **s2** (+6 solves, 96.4%)
8. **s13** (+5 solves, 97.6%)

---

## 7. Conclusions and Next Steps

1. **Conclusion on Pre-Phase Routing**:
   The hypothesis that pre-phase syntactic routing could outperform static portfolios at full scale is rejected. At 240 s on canonical 8-worker hardware, dynamic routing falls ~100 problems behind static division portfolios. Pre-phase infrastructure remains valuable for detecting trivial inputs (`empty_after_lowering`), calculating difficulty/feasibility, and emitting diagnostics, but search configuration should remain governed by static per-division portfolios.

2. **Next Steps for Portfolio Deployment (per AGENTS.md §11)**:
   - Solo greedy set-cover generates candidate orders. In accordance with §11 ("Per-Division Portfolio Optimisation"), candidate portfolios must be validated cooperatively:
     1. Test candidate `casc_fne` (`[12, 11, 15, 14, 4, 13, ...]`) with 8 workers and shared equality pool enabled (`MRS_SHARED_POOL_INTERVAL=500`).
     2. Test candidate `casc_ueq` (`[1, 11, 8, 12, 2, 6, 15, 3]`).
     3. Test candidate `default_casc` (`[12, 6, 11, 1, 8, 15, 2, 13]`).
   - Use `paired_campaign.py --reps 2` to verify that cooperation and resource sharing preserve the solo coverage gains before committing updates to `crates/mrs-search/src/strategy/named.rs`.
