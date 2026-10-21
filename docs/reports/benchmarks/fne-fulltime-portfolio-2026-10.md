# FNE Full-Time Solo Strategy Sweeps & Greedy Set Cover Analysis (2026-10-20)

> Status: Banked strategy sweep and portfolio set-cover analysis.
> Input directories:
> - `~/crates/mrs-bench/results/solo-fne-casc-j13-fulltime` (100 problems × 15 strategies @ 180 s)
> - `~/crates/mrs-bench/results/solo-fne-casc30-fulltime` (100 problems × 15 strategies @ 240 s)

---

## 1. Executive Summary

A comprehensive full-time diagnostic evaluation of all 15 base strategies was
conducted across both **CASC-J13** and **CASC-30** FNE divisions at official
competition wall-clock limits (180 s and 240 s per strategy solo run). Using
`greedy_set_cover`, the solo coverage and portfolio complementarity were analyzed:

- **Total Solvable Corpus**:
  - **CASC-J13 FNE**: **38 of 100** problems are solved by at least one strategy.
  - **CASC-30 FNE**: **44 of 100** problems are solved by at least one strategy.
  - **Combined Unique Problem-Instances**: **82 of 200** instances solved.
- **Flaw in Current Shipped Portfolio (`CASC_FNE_ORDER`)**:
  - The current 8-worker portfolio prefix `[11, 8, 4, 15, 10, 3, 12, 1]` was
    calibrated on short 30 s sweeps.
  - In full-time runs, **Strategy 10 and Strategy 12 add 0 unique solves** in
    both CASC-J13 and CASC-30.
  - Meanwhile, **Strategy 13**—currently banished to slot 14 (`CASC_FNE_ORDER[13]`)
    outside the 8-worker portfolio—emerges as a top-3 contributor in both
    editions, adding **+4 unique solves in CASC-J13**, **+4 unique solves in
    CASC-30**, and **+12 unique problem-instances combined**.
  - Current top-8 coverage is **35/38 (92.1%)** on J13 and **40/44 (90.9%)** on
    CASC-30, missing exactly the hard LCL problems that `s13` cracks.
- **Optimal Greedy Cover**:
  - **CASC-J13**: 100% coverage (38/38) achieved with only **4 strategies**:
    `[s11, s13, s04, s15]`.
  - **CASC-30**: 100% coverage (44/44) achieved with **7 strategies**:
    `[s08, s15, s13, s01, s03, s04, s11]`.
  - **Joint Coverage**: 100% coverage across both editions (82/82 instances)
    achieved with **6 strategies**: `[s11, s13, s04, s07, s15, s03]`.
- **Candidate 8-Strategy Portfolio**:
  Replacing the redundant `s10` and `s12` with `s13` and `s07` yields the
  recommended candidate order:
  `[s11, s13, s04, s08, s15, s07, s03, s01]`.
  This reaches **100% solo coverage** on both editions (38/38 on J13, 44/44 on C30),
  improving diagnostic coverage by +3 on J13 and +4 on CASC-30 (+7 combined).

---

## 2. Sweep Provenance & Environment

| Parameter | CASC-J13 Sweep | CASC-30 Sweep |
|:---|:---|:---|
| **Directory** | `~/crates/mrs-bench/results/solo-fne-casc-j13-fulltime` | `~/crates/mrs-bench/results/solo-fne-casc30-fulltime` |
| **Run ID** | `mrs_bench_20261020_185123_bbc388b9` | `mrs_bench_20261020_184332_b565cf3d` |
| **Commit** | `8aaee2bb264cbf515e3e23458fb1c40410d55fb8` | `8aaee2bb264cbf515e3e23458fb1c40410d55fb8` |
| **Time Limit** | 180 s per strategy (`--casc-times`) | 240 s per strategy (`--casc-times`) |
| **Problems** | 100 | 100 |
| **Evaluated Systems** | `mrs-s01` through `mrs-s15` (15 systems) | `mrs-s01` through `mrs-s15` (15 systems) |
| **Total Jobs** | 1,500 | 1,500 |
| **Host** | `teenf9901` (Intel Xeon Silver 4108, 32 cores, 128 GB) | `tlpnf9701` (Intel Xeon Silver 4108, 32 cores, 64 GB) |

---

## 3. Solo Solve Rankings per Strategy

Number of problems solved solo by each strategy out of 100 in each edition:

| Strategy | Config Summary | J13 Solves (180 s) | C30 Solves (240 s) | Combined Solves (82 max) |
|:---|:---|---:|---:|---:|
| **`mrs-s11`** | `ConjSymbolBoost` + AgeWeight(6) + AllNegative + KBO | **31** (81.6%) | **27** (61.4%) | **58** (70.7%) |
| **`mrs-s08`** | LPO baseline + AgeWeight(5) | 20 (52.6%) | **27** (61.4%) | **47** (57.3%) |
| **`mrs-s04`** | KBO baseline + AgeWeight(8) | 19 (50.0%) | 23 (52.3%) | **42** (51.2%) |
| **`mrs-s13`** | `FunctionWeightPenalty` + SOS + AgeWeight(5) + KBO | 19 (50.0%) | 23 (52.3%) | **42** (51.2%) |
| **`mrs-s07`** | KBO baseline + AgeWeight(4) | 14 (36.8%) | 23 (52.3%) | 37 (45.1%) |
| **`mrs-s10`** | SOS (depth=100) + AgeWeight(12) + KBO | 16 (42.1%) | 21 (47.7%) | 37 (45.1%) |
| **`mrs-s01`** | Standard KBO baseline | 14 (36.8%) | 21 (47.7%) | 35 (42.7%) |
| **`mrs-s12`** | `HornHeuristic` + AgeWeight(5) | 14 (36.8%) | 19 (43.2%) | 33 (40.2%) |
| **`mrs-s15`** | `SymbolWeight` + AgeWeight(4) | 19 (50.0%) | 11 (25.0%) | 30 (36.6%) |
| **`mrs-s03`** | KBO baseline + AgeWeight(10) | 9 (23.7%) | 20 (45.5%) | 29 (35.4%) |
| **`mrs-s06`** | KBO baseline + AgeWeight(5) | 17 (44.7%) | 11 (25.0%) | 28 (34.1%) |
| **`mrs-s09`** | LPO baseline + AgeWeight(8) | 7 (18.4%) | 20 (45.5%) | 27 (32.9%) |
| **`mrs-s02`** | KBO baseline + AgeWeight(3) | 10 (26.3%) | 16 (36.4%) | 26 (31.7%) |
| **`mrs-s05`** | KBO baseline + AgeWeight(6) | 10 (26.3%) | 11 (25.0%) | 21 (25.6%) |
| **`mrs-s14`** | `ConjSymbolBoost` + SmallestFirst + All + KBO | 6 (15.8%) | 5 (11.4%) | 11 (13.4%) |

---

## 4. Greedy Set Cover Results

### 4.1 CASC-J13 (180 s, 38 unique solved)

```
greedy_set_cover solo-fne-casc-j13-fulltime/run.csv 8 --division fne
```

| Step | Strategy | New Solves | Cumulative | Coverage % | Newly Covered Problems |
|:---|:---|---:|---:|---:|:---|
| 01 | `mrs-s11` | +31 | 31 / 38 | 81.6% | CSR (16), KRS (7), NUN (4), LCL (3), MGT (1) |
| 02 | **`mrs-s13`** | **+4** | 35 / 38 | 92.1% | `LCL373+2`, `LCL382+1`, `LCL390+2`, `NUN060+1` |
| 03 | `mrs-s04` | +2 | 37 / 38 | 97.4% | `GEO260+1`, `SYN353+1` |
| 04 | `mrs-s15` | +1 | 38 / 38 | **100.0%** | `CSR116+19` |

All 38 solvable problems in CASC-J13 are covered by just **4 strategies**:
`[s11, s13, s04, s15]`.

### 4.2 CASC-30 (240 s, 44 unique solved)

```
greedy_set_cover solo-fne-casc30-fulltime/run.csv 8 --division fne
```

| Step | Strategy | New Solves | Cumulative | Coverage % | Newly Covered Problems |
|:---|:---|---:|---:|---:|:---|
| 01 | `mrs-s08` | +27 | 27 / 44 | 61.4% | CSR (13), KRS (3), LCL (4), MED (2), MGT (1), SYN (2), SYO (1) |
| 02 | `mrs-s15` | +9 | 36 / 44 | 81.8% | `CSR115+31`, `CSR115+6`, `CSR115+98`, `GEO259+1`, `LCL390+1`, `LCL640+1.005`, `NUN060+1`, `NUN066+1`, `NUN081+1` |
| 03 | **`mrs-s13`** | **+4** | 40 / 44 | 90.9% | `LCL370+2`, `LCL373+2`, `LCL390+2`, `LCL403+2` |
| 04 | `mrs-s01` | +1 | 41 / 44 | 93.2% | `CSR060+2` |
| 05 | `mrs-s03` | +1 | 42 / 44 | 95.5% | `LCL660+1.020` |
| 06 | `mrs-s04` | +1 | 43 / 44 | 97.7% | `SYN353+1` |
| 07 | `mrs-s11` | +1 | 44 / 44 | **100.0%** | `KRS234+1` |

*(Note: Step 1 tie-breaks `s08` over `s11` alphabetically; seeding with `s11`
yields 100% coverage in only 6 steps: `s11` (+27), `s13` (+8), `s04` (+4),
`s07` (+3), `s02` (+1), `s03` (+1).)*

### 4.3 Joint Set Cover (Both Editions, 82 unique instances)

| Step | Strategy | New Solves | Cumulative | Coverage % |
|:---|:---|---:|---:|---:|
| 01 | **`mrs-s11`** | +58 | 58 / 82 | 70.7% |
| 02 | **`mrs-s13`** | **+12** | 70 / 82 | 85.4% |
| 03 | **`mrs-s04`** | +6 | 76 / 82 | 92.7% |
| 04 | **`mrs-s07`** | +3 | 79 / 82 | 96.3% |
| 05 | **`mrs-s15`** | +2 | 81 / 82 | 98.8% |
| 06 | **`mrs-s03`** | +1 | 82 / 82 | **100.0%** |

Just **6 strategies** cover 100% of all solvable problems across both CASC-J13
and CASC-30.

---

## 5. The Strategy 13 Discovery

### 5.1 Why `s13` Was Previously Overlooked

In the historical 30 s sweeps (`docs/reports/benchmarks/divisions-2026-09.md`),
`s13` was positioned at index 13 (`CASC_FNE_ORDER[13]`) because it had not
accumulated enough given-clause iterations within 30 seconds to solve deep
problems.

At full competition limits (180 s and 240 s), the solve times for `s13` on the
hard Logic Calculi (LCL) problems reveal why:

| Problem | Edition | Solve Time | Clauses Processed | Solved by Any Other Top-8 Strategy? |
|:---|:---|---:|---:|:---|
| **`LCL382+1`** | J13 | 56.80 s | 893 | **No** (Unique to `s13`) |
| **`LCL370+2`** | C30 | 63.81 s | 1,185 | **No** (Unique to `s13`) |
| **`LCL373+2`** | J13 & C30 | 67.60 s / 68.72 s | 1,197 / 1,192 | **No** (Unique to `s13`) |
| **`LCL390+2`** | J13 & C30 | 105.33 s / 93.38 s | 1,302 / 1,288 | **No** (Unique to `s13`) |
| **`LCL403+2`** | C30 | 98.27 s | 1,300 | **No** (Unique to `s13`) |

Every one of these problems solves between **56 s and 106 s**, requiring only
~900 to 1,300 processed clauses. In a 30 s sweep, they were all cut off as timeouts.

### 5.2 Algorithmic Synergy of `s13`

Strategy 13 configures:
- `selection: AgeWeight(5)`
- `literal_selection: AllNegative`
- `ordering: KBO`
- `weight_fn: FunctionWeightPenalty` (quadratic scaling: $w \cdot (d+1)^2$)
- `sos_depth: 100` (inference-level set of support)

This combination is uniquely tailored for LCL schemata problems:
1. **Inference-level SOS** keeps derivations anchored to parents connected to the
   negated conjecture, avoiding aimless forward expansions of the detachment rule.
2. **`FunctionWeightPenalty`** heavily penalizes term depth, preventing term-tower
   explosions (nested implication chains) from clogging the passive queue.
3. The modest iteration count (~1,200 clauses) demonstrates that `s13` does not
   brute-force the search; it navigates directly to the condensed detachment refutation.

---

## 6. Portfolio Audit: Current vs. Proposed

### 6.1 Current 8-Worker Portfolio (`CASC_FNE_ORDER[0..8]`)

Current order: `[s11, s8, s4, s15, s10, s3, s12, s1]`

- **Redundancy Analysis**:
  Given the presence of `{s11, s08, s04, s15}`:
  - `s10` adds **+0** on J13 and **+0** on C30.
  - `s12` adds **+0** on J13 and **+0** on C30.
  Both workers are completely redundant in diagnostic solo coverage.
- **Coverage Deficit**:
  - Missing on J13: `LCL373+2`, `LCL382+1`, `LCL390+2` (all solved by `s13`).
  - Missing on C30: `LCL370+2`, `LCL373+2`, `LCL390+2`, `LCL403+2` (all solved by `s13`).
  - Score: **35 / 38** on J13, **40 / 44** on C30 (75 / 82 combined).

### 6.2 Proposed Candidate Portfolio

Replace redundant `s10` and `s12` with `s13` and `s07`:
`[s11, s13, s04, s08, s15, s07, s03, s01]`

| Metric | Current Top-8 | Proposed Candidate Top-8 | Delta |
|:---|---:|---:|:---|
| **CASC-J13 Coverage** | 35 / 38 (92.1%) | **38 / 38 (100.0%)** | **+3 solves (+7.9%)** |
| **CASC-30 Coverage** | 40 / 44 (90.9%) | **44 / 44 (100.0%)** | **+4 solves (+9.1%)** |
| **Combined Coverage** | 75 / 82 (91.5%) | **82 / 82 (100.0%)** | **+7 solves (+8.5%)** |

Both editions reach **100% theoretical diagnostic coverage** under this 8-worker set.

---

## 7. Conclusions & Next Steps

1. **High-Impact Improvement**: Promoting `s13` into the active 8-worker portfolio
   resolves the longstanding LCL coverage gap in FNE without sacrificing any
   currently solved problem.
2. **Decision Rule Compliance (AGENTS.md §11)**:
   - Solo coverage gap exceeds 5% (+7.9% on J13, +9.1% on C30).
   - Before editing `CASC_FNE_ORDER` in `crates/mrs-search/src/strategy/named.rs`,
     run the cooperative portfolio validation script:
     ```bash
     MRS_WORKERS=8 MRS_SHARED_POOL_INTERVAL=0 \
       crates/mrs-bench/cooperative_portfolio_sweep.sh \
       casc-30 fne 11,13,4,8,15,7,3,1 240 1 \
       results/cooperative-fne-candidate-$(date +%Y%m%d)
     ```
   - Validate cooperative performance on canonical 8-core hardware to confirm
     that concurrency and resource allocation reproduce the expected gain.
