# FEQ Candidate Cooperative Portfolio Validation Report (October 2026)

> Status: Banked single-run campaign observations; portfolio validation remains
> provisional. These are not replicated paired comparisons. Current policy uses
> two paired blocks as an initial screen and adds pairs when results are
> inconclusive. The CASC-J13 runs used 180 s;
> current canonical FEQ comparisons use 240 s, so those J13 results are
> supplemental and must not be treated as canonical-limit comparisons.
> A separate October 29 CASC-30 FEQ run is documented in §5; it is not included
> in the original four-campaign aggregate or promoted as a baseline.
> Campaign directories:
> - CASC-30 (8c host): `~/crates/mrs-bench/results/cooperative-feq-equal-candidate-casc30`
> - CASC-30 (32c host): `~/crates/mrs-bench/results/cooperative-feq-equal-candidate-casc30-x`
> - CASC-J13 (8c host): `~/crates/mrs-bench/results/cooperative-feq-equal-candidate-casc-j13`
> - CASC-J13 (32c host): `~/crates/mrs-bench/results/cooperative-feq-equal-candidate-casc-j13-x`
> Reference greedy comparator on 32c host (`teenf9901`):
> - CASC-30: `~/crates/mrs-bench/results/coop-feq-greedy-candidate-99-20261023`
> - CASC-J13: `~/crates/mrs-bench/results/cooperative-feq-greedy-candidate-99-casc-j13`

The four full-division candidate campaigns below are single observations per
host/division, not replicated paired comparisons. Their totals describe
those runs; they do not estimate a mean or establish statistical significance.
Per `AGENTS.md` §11, a canonical portfolio decision needs at least a paired
screen, with more pairs if the result is ambiguous; this report does not satisfy
that gate.

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
   The original four single-run campaigns show positive observed results and zero
   `VerifiedBad` proofs among 365 audited refutations. Their coverage deltas are
   observations, not estimates of means or evidence that the portfolio is
   optimal; the CASC-J13 runs used a supplemental historical 180 s limit, and
   the `GEO559+1` diagnostic below is not a fifth full-division run. A paired
   candidate/baseline comparison at the canonical 240 s limit, starting with
   two paired blocks and extending if the result is ambiguous, is still needed
   for a cross-edition claim.

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

| Metric | Shipped Baseline | Candidate Portfolio ($\mathbf{P}_{\text{FEQ}}$) | Pure Greedy Candidate |
|:---|:---:|:---:|:---:|
| **CASC-30 Solves** | 108 / 400 | **111 (8c) / 113 (32c)** | **115 / 400** |
| **CASC-J13 Solves** | 68–71 / 300 | **69 (8c, 180 s supplemental) / 72 raw (32c, 71 verified; 180 s supplemental)** | **71 / 300** |
| **CASC-J13 Regressions** | Baseline | **0 observed** | **4 observed** (`BIO006+1`, `GEO494+1`, `SET017+1`, `SWX203+1`) |
| **Combined Solves** | 176–179 | **180–185** | 186 |

The full-division values above exclude the `GEO559+1` one-problem diagnostic
below. The `72` CASC-J13 result includes one proof omitted by the 32 MB output
limit; only 71 of those 72 refutations were `VerifiedGood`.

### 4.3 Diagnostic Verification on `teenf9901` (`GEO559+1` and `GEO641+1`)

On CASC-30, the pure greedy candidate's full-division result was 115. A
one-problem diagnostic was run on `teenf9901` under the adopted portfolio; it
is not an independently measured 114/400 campaign:

1. **`GEO559+1` is solved by the adopted portfolio**:
   - Refutation derived at **230.1 s** (160,222 proof nodes, 68.4 MB proof text).
   - On slower 2.20 GHz hosts (`mtsdev01` at 241.8 s) and 1.80 GHz hosts with 8 pinned cores (`tlpnf9701` at 239.6 s), thread racing pushed it just over the 240 s ceiling.
   - On `teenf9901`, throughput was sufficient to close the proof within the competition limit. The resulting **114 / 400** figure is an aggregate that includes this targeted solve, not a fifth full-division campaign score.
2. **`GEO641+1` is uniquely solved by Strategy 15**:
   - Timed out at 238.6 s under the adopted portfolio.
   - Under Strategy 15 alone, `GEO641+1` solves in **0.169 s** solo (and 0.199 s in the greedy portfolio).
   - All other 14 base strategies time out on `GEO641+1`.

### 4.4 Observed Tradeoff (Not a Replicated Conclusion)
In these campaign observations, the pure greedy portfolio gains `GEO641+1` on
CASC-30 but drops `s04` and has **4 observed regressions on CASC-J13** (including
`SET017+1` and `BIO006+1`). Because the CASC-J13 runs used 180 s and the
comparison was not replicated, this does not establish a robust cross-edition
tradeoff or show that the candidate order is optimal. The selected candidate
retains `s04` and had zero observed CASC-30 regressions in the 8-core comparison.
The available report does not establish a statistically significant overall
advantage or compare strict verification rates against the pure-greedy candidate.
Replicated paired runs at the canonical 240 s limit remain necessary.

---

## 5. Supplemental CASC-30 FEQ run (2026-10-29)

`~/crates/mrs-bench/results/campaign-casc30-feq-02-20261029-123324` is a later
full-division run using the adopted candidate order. It completed all 400
problems at the official 240 s FEQ limit:

| Outcome | Count | Strict audit |
|:---|---:|:---|
| `Theorem` | 109 | 104 `VerifiedGood`, 0 `VerifiedBad` |
| `Timeout` | 219 | no proof to check |
| `GaveUp` | 72 | no proof to check |
| Errors / resource-outs | 0 | — |

The five generated proofs not certified `VerifiedGood` were: `COM149+1`
(`Unknown`, demodulation replay mismatch), `GEO340+1` (`Unknown`, factoring
step limit), `KLE071+1` (`Unknown`, incomplete AC-superposition replay),
`ALG049+1` (strict verifier exceeded 120 s), and `GEO559+1` (proof omitted at
57.46 MB / 137,948 nodes). Thus the raw result is 109/400 (27.25%), while the
strictly verified count is 104/400 (26.0%).

### Oct 28 proof-output comparison

The preceding full run on the same host,
`campaign-casc30-feq-20261028-093955`, also used `mtsdev02`, eight workers, and
the same strategy order (`12,11,10,6,8,1,4,13`). It produced 108 `Theorem`
statuses, but the audit had only 92 `VerifiedGood` and 12 `no_proof` rows. In
the Oct 29 run:

- All 92 previous `VerifiedGood` problems certified again.
- All 108 Oct 28 raw theorem statuses recurred.
- Eleven of the 12 prior `no_proof` cases now passed strict verification:
  `KLE113+1`, `KLE132+1`, `LCL894+1`, `LCL895+1`, `LCL896+1`, `LCL901+1`,
  `NUM835+1`, `NUM839+2`, `NUM852+1`, `NUM853+2`, and `NUM926+2`.
- `KLE071+1` now emitted a proof but remains `Unknown`; `SWV458+1` is one new
  raw theorem and passed strict verification.

This raises the observed strict count from 92 to 104 (+12) and the raw theorem
count from 108 to 109 (+1). It is consistent with recovery of proof emission,
but it is **not a controlled attribution**: both metadata files report dirty
trees and different binary hashes, and each is a single run. The Oct 29 result
also remains two `VerifiedGood` below the earlier documented 8-core candidate
run (106/111); do not replace that observation or infer a regression from this
one run.

### Provenance and resource observations

The run used `MRS_HARDWARE=casc`, eight workers, `jobs=1`, and the automatic FEQ
schedule (observed order `12,11,10,6,8,1,4,13`). Host: `mtsdev02`, Xeon E5-2407,
8 physical cores, 95,979 MB RAM. The configured CASC allowance was 131,072 MB;
effective memory was 81,582 MB (`mem_unrepresentable=1`). No row ended in
`ResourceOut`; the maximum RSS was 65,174 MB (`CSR037+5`). The maximum virtual
peak was 79,607 MB on the same problem and is not RSS. Five emitted proofs
exceeded 8 MB; the 57.46 MB `GEO559+1` proof was withheld.

Provenance: run ID `mrs_bench_20261029_123429_6cc4f7d9`, commit
`3bb872130a77ea2f1586ff89ab69082fd14c09ff` (`git_dirty=true`), binary SHA-256
`2681fbf48120e1d619fcf60f3f12fa85f3ae25ffa0bcd821c0f5993fa0c0c1a7`. The 400
problem rows are unique; raw stdout/stderr and the 108 present proof files match
their recorded hashes, and the `run.csv` digest matches the audit identity.

## 6. Roadmap: Retaining Strategy 15 via MultiQueue in FEQ

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
