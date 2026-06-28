# Reference

Vampire 5.0.1 (Release build, commit 6b88ec04c on 2026-06-15 12:45:39 +0200)
CaDiCaL: cadical-2.1.3
Linked to Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 z3-4.8.4-9178-g3c47fd96c

[root@mtsdev02 mrs]# crates/mrs-bench/casc.sh --systems vampire --divisions fne,feq,epu,eps,ueq,icu --casc-times --jobs 8
CASC-30 Results — 2026-06-17 12:05  (1101 problems × 1 systems)
===============================================================

Division  Problems    vampire
                      Solved  Avg (s)
------------------  --------------------
FNE            100        82   22.853
FEQ            400       361   10.606
EPU            100        76   27.719
EPS            100        86    6.166
UEQ            300       243   30.747
ICU            101        53   79.142
------------------  --------------------
TOTAL         1101       901   22.204

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — 1 SOUNDNESS ERROR(S) vs reference answers:
  ICU     VVA001+1                        vampire=Theorem but expected CounterSatisfiable  ⚠ UNSOUND

E 3.3.3 Countess Grey (37fde70d516b57cb64294f8fe39bc16ece8198f8)
[root@mtsdev02 mrs]# crates/mrs-bench/casc.sh --systems eprover --divisions fne,feq,epu,eps,ueq,icu --casc-times --jobs 8
CASC-30 Results — 2026-06-17 07:22  (1101 problems × 1 systems)
===============================================================

Division  Problems    eprover
                      Solved  Avg (s)
------------------  --------------------
FNE            100        67   13.201
FEQ            400       236   25.444
EPU            100        22    3.550
EPS            100        63    3.833
UEQ            300       186   31.392
ICU            101        24  167.712
------------------  --------------------
TOTAL         1101       598   28.550

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

# Status

Latest sound full-portfolio results for mrs HEAD (commit `c6234fb`, 8 workers,
CASC times). Engine fixes: sound ordered-inference maximal-literal restriction
(`ordered_inferences`, two completeness bugs fixed — see docs/AUDIT.md) +
single-negative literal selection on `casc_fne`. All divisions sound, including
the EPU completeness gate (0 violations on the unsatisfiable division).

Figures below are from mixed accurate (jobs 1) / oversubscribed (jobs 4) runs
on commit `7316d88`; FEQ/EPU/ICU (jobs 4) are likely undercounts pending a
clean jobs-2 matrix re-run.

Division  Problems    mrs        (prev)
                      Solved
------------------  -------------------
FNE            100        45    (44)
FEQ            400        78    (76)
EPU            100        16    (13)   sound (was unsound→3 false Satisfiable)
EPS            100        43    (21)   +22, now SOUND
UEQ            300        31    (30)
ICU            101         1    ( 2)   noise
------------------  -------------------
TOTAL         1101       214   (186)   +28

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

## vs CASC-30 official results

Source: https://tptp.org/CASC/30/WWWFiles/Results.html (CASC-J30, 8-core
StarExec hardware, competition strategy schedules). Two comparisons follow:
(a) where mrs's HEAD solved-counts would place among the actual CASC-30
entrants, and (b) how our local `# Reference` Vampire/E numbers compare to
the official competition figures.

### (a) Projected mrs placement per division

| Division | mrs solved | Projected rank | Neighbours (official solved) |
|----------|-----------|----------------|------------------------------|
| FNE (100) | 46 | ~10th of 15 | cvc5 47 > **mrs 46** > ConnectPP 43 |
| FEQ (400) | 84 | ~11th of 14 | Prover9 94 > **mrs 84** > ConnectPP 59 |
| EPU (100) | 16 | 6th of 7 | Drodi-EPR 25 > **mrs 16** > SPASS-SCL 11 |
| EPS (100) | 40 | last (7th) | field ≥ 53 (SPASS-SCL); **mrs 40** trails |
| UEQ (300) | 30 | last (10th) | field ≥ 114 (Toma); **mrs 30** trails |
| ICU (101) | 2 | 8th of 9 | CSE_E 18 > **mrs 2** > ConnectPP 1 |

Reading: mid/lower-pack in the FOF divisions (ahead of several real
entrants), at or near the bottom in the equality/EPR-saturation divisions
(EPS, UEQ last; EPU, ICU second-to-last). Consistent with docs/AUDIT.md:
the gap is search/heuristic quality, most acute on UEQ and EPS saturation.

Official CASC-30 winners per division (for reference):
FNE Vampire 5.0 = 91; FEQ Vampire 4.9 = 379 (Vampire 5.0 = 364);
EPU Vampire 5.0 = 96; EPS Vampire 5.0 = 90; UEQ Vampire 5.0 = 263;
ICU Vampire 4.9 = 70 (Vampire 5.0 = 69).

### (b) Local `# Reference` vs official CASC-30

Our local Vampire/E baselines (run via crates/mrs-bench/systems/{vampire,
eprover}/invoke.sh) are systematically LOWER than the official figures —
treat them as a local lower-bound baseline, not the competition numbers.

| Div | Vampire (local) | Vampire 5.0 (CASC) | Δ | eprover (local) | E 3.3.0 (CASC) | Δ |
|-----|-----------------|--------------------|----|------------------|-----------------|----|
| FNE | 82 | 91 | −9 | 67 | 76 | −9 |
| FEQ | 361 | 364 | −3 | 236 | 288 | −52 |
| EPU | 76 | 96 | −20 | 22 | 29 | −7 |
| EPS | 86 | 90 | −4 | 63 | 59 | +4 |
| UEQ | 243 | 263 | −20 | 186 | 222 | −36 |
| ICU | 53 | 69 | −16 | 24 | 42 | −18 |
| TOTAL | 901 | 973 | −72 | 598 | 716 | −118 |

Ordering and magnitudes are directionally consistent (Vampire ≫ E in every
division), but the local harness undershoots by ~7% (Vampire) to ~16% (E).
Likely causes: (1) our invoke.sh wrappers do not reproduce the exact CASC
competition strategy/time/core configuration — the largest gaps (FEQ-E −52,
UEQ-E −36, ICU) are where CASC-mode scheduling matters most; (2) version
drift (we run Vampire 5.0.1 / E 3.3.3 vs the competition's 5.0 / 3.3.0).

## ML-guided clause selection — investigation (2026-06-22 … 06-24)

Status: **ML not shipped.** Static `casc_*` portfolios remain the competition
entry. Summary of the investigation, kept for future work.

### Eval: mrs-ml (ML schedules + weights) vs static baseline, CASC times, 8 workers

| Div | baseline `mrs` | `mrs-ml` (old model) | `mrs-ml` (retrained) | schedule |
|-----|----------------|----------------------|----------------------|----------|
| FEQ | 81 | 64 | **54** | `ml_feq` (diverse chassis) |
| FNE | 43 | 22 | 22 | `ml_fne` (homogeneous) |
| EPU | 13 | 7 | — | `ml_epr` (homogeneous) |
| EPS | 21 | 22 | — | `ml_epr` (homogeneous) |
| UEQ | — | — | — | `ml_ueq` (homogeneous) |

All runs sound (zero polarity/reference violations). ML lost in every
division that matters.

### Two distinct problems found

1. **Homogeneous `ml_fne`/`ml_ueq`/`ml_epr` schedules** replace the tuned
   15-strategy `casc_*` portfolio with ~8 near-identical `MlGuided` strategies
   → they lose portfolio diversity and roughly halve the baseline regardless
   of model quality (FNE 43→22). Only `ml_feq` is a fair test (diverse chassis
   + ML layered on).

2. **The training code was broken** (`mrs-train`, fixed in this branch):
   - Ran **1 epoch** (no `num_epochs` call → burn default of 1).
   - `valid == train` (no held-out split).
   - Plain BCE on a **63:1–223:1 imbalanced** dataset (positives = proof
     clauses, 0.4–1.6% of samples) → a near-degenerate near-constant predictor;
     the low loss was a majority-class artifact.
   Fix: `num_epochs` + early stopping, stratified split, class rebalancing
   (`--neg-per-pos`), and post-training AUC/precision/recall/score-gap metrics.
   On the same data this lifted validation **AUC from ~0.5 → 0.84–0.89**.

### Key finding: a *good* model still made proving *worse*

With the retrained FEQ model (**AUC 0.84**), `mrs-ml` FEQ = **54** — worse than
both the static baseline (81) and the degenerate-model run (64). Selection
priority is `0.3·weight + 0.7·(1−σ(score))` (`unprocessed.rs:115`), i.e. the
model drives **70%** of clause selection. A flat (degenerate) score barely
perturbs the proven ordering; a confident model strongly reorders selection
toward "resembles a final-proof clause" — a hindsight/survivorship label on a
different distribution than live search — and drags selection away from the
well-tuned heuristic. **Model quality (AUC) was never the proving bottleneck;
objective/integration alignment is.**

### Experiment A (2026-06-24): give ML far less authority

Raised the `ml_feq` `MlGuided` strategies from `alpha` 0.1–0.5 to **0.85**
(ML becomes a ~15% refinement of the weight ordering instead of dominating).
**Result: no effect — FEQ stayed at 54** (identical to `alpha=0.3`; static
`casc_feq` = 81). The `mrs-ml` FEQ gap is driven by the schedule composition,
not the ML blend weight; ML guidance simply does not help here at any alpha.

### Conclusion / future work (see docs/TODO_CASC.md)

The `mrs-train` bug is fixed and validated (AUC 0.84–0.89), but ML-guided
selection does not beat the greedy-tuned static portfolios — and tuning the
blend (`alpha`) does not rescue it. This is a research-grade gap (objective
alignment, distribution shift, iterative trace collection, richer features).

**Decision: ML is frozen and NOT shipped. The competition entry uses the
static `casc_*` portfolios via `mrs/invoke.sh` (no harness change).** The
`mrs-train` fixes, `models/` weights, and `mrs-ml` system are kept as
validated infrastructure for a future ML iteration.

# Benchmark Log

Append-only log of CASC benchmark runs (`crates/mrs-bench/casc.sh`), newest first.
Each entry records the mrs commit and the exact command used.


TODO — strategy sweeps (Step 1 of portfolio re-tuning):
    Dual 4108 — accurate coverage:
    ./crates/mrs-bench/run_strategy_sweep.sh --divisions fne --casc-times --jobs 16
    Dual E5-2407:
    ./crates/mrs-bench/run_strategy_sweep.sh --divisions fne --casc-times --jobs 8

   1     cat run_A.csv > master_run.csv
   2     tail -n +2 run_B.csv >> master_run.csv
   3     tail -n +2 run_C.csv >> master_run.csv
   4     tail -n +2 run_D.csv >> master_run.csv

  Step 3: Generate the Portfolios
  Now, on Server A, run the greedy solver on the combined master_run.csv:

   1 ./crates/mrs-bench/run_all_greedy_sweeps.sh master_run.csv > final_cacs30_portfolios.txt

rebench FNE FEQ with SiNe
commit 77591fb6ffc1fad5576d6e327301a1e38efe7d1a

[ongoing]
[root@mtsdev04 mrs]# ./crates/mrs-bench/run_strategy_sweep.sh --divisions icu --casc-times --jobs 8

[ongoing]
[root@mtsdev03 mrs]# ./crates/mrs-bench/run_strategy_sweep.sh --divisions epu --casc-times --jobs 8

[ongoing]
[root@mtsdev02 mrs]# ./crates/mrs-bench/run_strategy_sweep.sh --divisions eps --casc-times --jobs 8

[ongoing]
[root@mtsdev01 mrs]# ./crates/mrs-bench/run_strategy_sweep.sh --divisions fne --casc-times --jobs 8

[ongoing]
[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ ./crates/mrs-bench/run_strategy_sweep.sh --divisions ueq --casc-times --jobs 16

[ongoing]
[www@teenf9901 mrs]$ ./crates/mrs-bench/run_strategy_sweep.sh --divisions feq --casc-times --jobs 16

[ongoing]
hack@pve:~/mrs$ MRS_WORKERS=4 crates/mrs-bench/casc.sh --systems mrs --divisions icu  --casc-times --jobs 1

[done]
[root@mtsdev01 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions fne  --casc-times --jobs 1
CASC-30 Results — 2026-06-28 06:02  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        43   19.397
------------------  --------------------
TOTAL          100        43   19.397

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done]
[www@teenf9901 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions feq --casc-times --jobs 4
CASC-30 Results — 2026-06-28 05:52  (400 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FEQ            400        92   29.624
------------------  --------------------
TOTAL          400        92   29.624

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

rebench FNE FEQ with redundant FV fix
commit 2c5cd36bdf30f981a73756f5a64800917d8f465c

[ongoing]
X99 MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions eps  --casc-times --jobs 1

[done]
[root@mtsdev02 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions epu --casc-times --jobs 1
CASC-30 Results — 2026-06-28 06:01  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPU            100        16    8.155
------------------  --------------------
TOTAL          100        16    8.155

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done]
[root@mtsdev04 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions fne  --casc-times --jobs 1
CASC-30 Results — 2026-06-28 06:16  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        47   12.300
------------------  --------------------
TOTAL          100        47   12.300

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

commit 470191304f1db33f778372e8f1ee92ff11739115

[done]
[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions feq  --casc-times --jobs 4
CASC-30 Results — 2026-06-28 05:57  (400 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FEQ            400        79   30.411
------------------  --------------------
TOTAL          400        79   30.411

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[ongoing]
[www@teenf9901 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions ueq --casc-times --jobs 4
CASC-30 Results — 2026-06-27 15:57  (300 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
UEQ            300        30   19.305
------------------  --------------------
TOTAL          300        30   19.305

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

X99 MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions epu  --casc-times --jobs 1
CASC-30 Results — 2026-06-27 13:09  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPU            100        16    5.033
------------------  --------------------
TOTAL          100        16    5.033

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[root@mtsdev04 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions eps  --casc-times --jobs 1
CASC-30 Results — 2026-06-27 12:48  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPS            100        37    5.131
------------------  --------------------
TOTAL          100        37    5.131

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions eps  --casc-times --jobs 2
CASC-30 Results — 2026-06-27 11:44  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPS            100        37    4.507
------------------  --------------------
TOTAL          100        37    4.507

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions eps  --casc-times --jobs 4
CASC-30 Results — 2026-06-27 10:36  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPS            100        37    7.807
------------------  --------------------
TOTAL          100        37    7.807

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[www@teenf9901 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions fne --casc-times --jobs 4
CASC-30 Results — 2026-06-27 11:03  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        46   11.246
------------------  --------------------
TOTAL          100        46   11.246

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

commit ea1d55f20116a42cbd9c08fba2705bd5d43c00dd

hack@pve:~/mrs$ MRS_WORKERS=4 crates/mrs-bench/casc.sh --systems mrs --divisions ueq  --casc-times --jobs 1
CASC-30 Results — 2026-06-27 16:08  (300 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
UEQ            300        29   19.170
------------------  --------------------
TOTAL          300        29   19.170

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[root@mtsdev04 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions fne  --casc-times --jobs 1
CASC-30 Results — 2026-06-27 06:02  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        46   11.393
------------------  --------------------
TOTAL          100        46   11.393

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[root@mtsdev01 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions feq  --casc-times --jobs 1
CASC-30 Results — 2026-06-27 15:13  (400 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FEQ            400        84   34.899
------------------  --------------------
TOTAL          400        84   34.899

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ ./crates/mrs-bench/run_strategy_sweep.sh --divisions feq --casc-times --jobs 30
CASC-30 Results — 2026-06-27 05:57  (400 problems × 15 systems)
===============================================================

Division  Problems    mrs-s01               mrs-s02               mrs-s03               mrs-s04               mrs-s05               mrs-s06               mrs-s07               mrs-s08               mrs-s09               mrs-s10               mrs-s11               mrs-s12               mrs-s13               mrs-s14               mrs-s15
                      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------
FEQ            400        36   34.955          20   33.338          12    5.892          36   29.060          30   23.534          35   54.680          36   29.539          42   13.235          14   10.175          26   38.716          43   23.521          39   27.920          23   28.949          18    9.058          20   31.561
------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------
TOTAL          400        36   34.955          20   33.338          12    5.892          36   29.060          30   23.534          35   54.680          36   29.539          42   13.235          14   10.175          26   38.716          43   23.521          39   27.920          23   28.949          18    9.058          20   31.561

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[www@teenf9901 mrs]$ ./crates/mrs-bench/run_strategy_sweep.sh --divisions ueq --casc-times --jobs 30
CASC-30 Results — 2026-06-27 05:59  (300 problems × 15 systems)
===============================================================

Division  Problems    mrs-s01               mrs-s02               mrs-s03               mrs-s04               mrs-s05               mrs-s06               mrs-s07               mrs-s08               mrs-s09               mrs-s10               mrs-s11               mrs-s12               mrs-s13               mrs-s14               mrs-s15
                      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------
UEQ            300        24   96.185          22   24.903          16   28.385          29   58.328          29   73.553          27   61.796          12   58.285          18   31.525          11   39.744           2   16.878          33   59.938          29   83.641           2    2.576          27   70.105          20   79.526
------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------
TOTAL          300        24   96.185          22   24.903          16   28.385          29   58.328          29   73.553          27   61.796          12   58.285          18   31.525          11   39.744           2   16.878          33   59.938          29   83.641           2    2.576          27   70.105          20   79.526

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done]
[root@mtsdev03 mrs]# ./crates/mrs-bench/run_strategy_sweep.sh --divisions epu,icu --casc-times --jobs 7
CASC-30 Results — 2026-06-28 06:11  (201 problems × 15 systems)
===============================================================

Division  Problems    mrs-s01               mrs-s02               mrs-s03               mrs-s04               mrs-s05               mrs-s06               mrs-s07               mrs-s08               mrs-s09               mrs-s10               mrs-s11               mrs-s12               mrs-s13               mrs-s14               mrs-s15
                      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------
EPU            100        14   16.873          13    6.531          13    6.514          14    8.439           8    2.368           9   11.160           8    0.732           8    1.675           7    0.209           5    0.021          14   14.762          14   15.259           5    0.021           6    0.021          14   17.176
ICU            101         1    0.181           1   16.875           1    0.154           1    0.177           1    0.157           1   17.046           1    0.187           1    0.188           1    0.174           0    0.000           1    0.175           1   17.843           0    0.000           1   17.759           1   17.622
------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------
TOTAL          201        15   15.760          14    7.270          14    6.060          15    7.889           9    2.122          10   11.749           9    0.671           9    1.510           8    0.205           5    0.021          15   13.790          15   15.431           5    0.021           7    2.555          15   17.206                                                                                       
DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[root@mtsdev02 mrs]# ./crates/mrs-bench/run_strategy_sweep.sh --divisions fne,eps --casc-times --jobs 7
CASC-30 Results — 2026-06-27 05:59  (200 problems × 15 systems)
===============================================================

Division  Problems    mrs-s01               mrs-s02               mrs-s03               mrs-s04               mrs-s05               mrs-s06               mrs-s07               mrs-s08               mrs-s09               mrs-s10               mrs-s11               mrs-s12               mrs-s13               mrs-s14               mrs-s15
                      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------
FNE            100        23   17.333          26    6.327          23    5.575          27   10.913          14   17.153          14   16.073          22    8.875          28   14.815          23   15.043           8    0.248          29    5.322          29   20.279           8    1.838          11    2.553          30   26.385
EPS            100        37   11.966          38    4.873          37    4.977           0    0.000           9    0.183          10    0.170          32    7.963          27    4.157          32    4.721           0    0.000           0    0.000           0    0.000           0    0.000           0    0.000           0    0.000
------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------
TOTAL          200        60   14.023          64    5.463          60    5.206          27   10.913          23   10.513          24    9.447          54    8.335          55    9.583          55    9.037           8    0.248          29    5.322          29   20.279           8    1.838          11    2.553          30   26.385

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

commit 7316d88474f2e43273c346c17072ddb7d41cf6ab (HEAD -> fix-eps-ordered-inferences

hack@pve:~/mrs$ MRS_WORKERS=4 crates/mrs-bench/casc.sh --systems mrs --divisions ueq  --casc-times --jobs 1
CASC-30 Results — 2026-06-26 11:10  (300 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
UEQ            300        31   36.451
------------------  --------------------
TOTAL          300        31   36.451

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[www@teenf9901 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions icu --casc-times --jobs 4
CASC-30 Results — 2026-06-26 11:04  (101 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
ICU            101         1    1.110
------------------  --------------------
TOTAL          101         1    1.110

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions ueq  --casc-times --jobs 4
CASC-30 Results — 2026-06-26 11:20  (300 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
UEQ            300        27   23.449
------------------  --------------------
TOTAL          300        27   23.449

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions feq  --casc-times --jobs 4
CASC-30 Results — 2026-06-26 06:33  (400 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FEQ            400        78   29.435
------------------  --------------------
TOTAL          400        78   29.435

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions epu  --casc-times --jobs 4
CASC-30 Results — 2026-06-25 16:30  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPU            100        16    7.083
------------------  --------------------
TOTAL          100        16    7.083

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[root@mtsdev02 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions fne --casc-times --jobs 1
CASC-30 Results — 2026-06-26 06:35  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        45   13.702
------------------  --------------------
TOTAL          100        45   13.702

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[root@mtsdev03 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions eps --casc-times --jobs 1
CASC-30 Results — 2026-06-26 06:41  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPS            100        43    8.380
------------------  --------------------
TOTAL          100        43    8.380

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

commit a2c4e7155eaa2af211eece1a49c5f4a1df5e67dd (HEAD -> fix-eps-ordered-inferences

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions epu  --casc-times --jobs 4
CASC-30 Results — 2026-06-25 15:20  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPU            100        16    8.357
------------------  --------------------
TOTAL          100        16    8.357

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — 3 case(s) of wrong SZS polarity:
  EPU     SYN861-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SYN862-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SYN866-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND

REFERENCE VIOLATIONS — 3 SOUNDNESS ERROR(S) vs reference answers:
  EPU     SYN861-1                        mrs=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SYN862-1                        mrs=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SYN866-1                        mrs=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND


[root@mtsdev03 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions epu  --casc-times --jobs 1
CASC-30 Results — 2026-06-25 16:05  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPU            100        16    6.829
------------------  --------------------
TOTAL          100        16    6.829

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — 3 case(s) of wrong SZS polarity:
  EPU     SYN861-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SYN862-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SYN866-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND

REFERENCE VIOLATIONS — 3 SOUNDNESS ERROR(S) vs reference answers:
  EPU     SYN861-1                        mrs=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SYN862-1                        mrs=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SYN866-1                        mrs=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND

[ongoing]
[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions epu  --casc-times --jobs 4

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions eps  --casc-times --jobs 2
CASC-30 Results — 2026-06-25 14:10  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPS            100        47    7.486
------------------  --------------------
TOTAL          100        47    7.486

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

commit b16c63006ea6efd630f021f296a72017494c7f1d

[www@teenf9901 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions ueq --casc-times --jobs 4
CASC-30 Results — 2026-06-26 06:36  (300 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
UEQ            300        29   26.065
------------------  --------------------
TOTAL          300        29   26.065

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[root@mtsdev02 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions epu --casc-times --jobs 1
CASC-30 Results — 2026-06-25 14:33  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPU            100        13    6.846
------------------  --------------------
TOTAL          100        13    6.846

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

done A/B
[www@teenf9901 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions fne --casc-times --jobs 2

CASC-30 Results — 2026-06-25 14:35  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        45    7.414
------------------  --------------------
TOTAL          100        45    7.414

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

commit 8ab3bc0c81e99e5bc9e0bb71b54fc005ad093dfe

[done]
[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ MRS_NO_SINGLE_NEG=1 MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions fne --casc-times --jobs 2
CASC-30 Results — 2026-06-25 13:09  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        40   21.028
------------------  --------------------
TOTAL          100        40   21.028

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done]
[www@teenf9901 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions fne --casc-times --jobs 2
CASC-30 Results — 2026-06-25 12:33  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        42   10.683
------------------  --------------------
TOTAL          100        42   10.683

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

commit 2bffd23da6cfc505c8577b0dd819c7bab80e7b57

[www@teenf9901 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions eps --casc-times --jobs 2
CASC-30 Results — 2026-06-25 10:29  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPS            100        47    7.521
------------------  --------------------
TOTAL          100        47    7.521

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

stopped at 20 ok /75 /400
[www@teenf9901 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions feq --casc-times --jobs 4

[www@teenf9901 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions eps --casc-times --jobs 4
CASC-30 Results — 2026-06-25 08:18  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPS            100        47   14.105
------------------  --------------------
TOTAL          100        47   14.105

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[root@mtsdev02 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions epu --casc-times --jobs 1
CASC-30 Results — 2026-06-25 10:58  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPU            100        16    6.875
------------------  --------------------
TOTAL          100        16    6.875

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — 3 case(s) of wrong SZS polarity:
  EPU     SYN861-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SYN862-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SYN866-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND

REFERENCE VIOLATIONS — 3 SOUNDNESS ERROR(S) vs reference answers:
  EPU     SYN861-1                        mrs=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SYN862-1                        mrs=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SYN866-1                        mrs=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND

[done]
[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$   MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions fne --casc-times --jobs 2
CASC-30 Results — 2026-06-25 10:59  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        42   10.385
------------------  --------------------
TOTAL          100        42   10.385

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done]
[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$   MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions fne --casc-times --jobs 4
CASC-30 Results — 2026-06-25 08:50  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        41   15.064
------------------  --------------------
TOTAL          100        41   15.064

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

commit bb688bd4a272ff6e32f304b6e9c0e537c50cd724 (HEAD -> update-eps-epu-portfolios

[root@mtsdev03 mrs]# INPUT_PROBLEMS_LIST=./casc_problem_lists/ueq.list ./crates/mrs-bench/collect_ml_data.sh /mnt/sdd/TPTP-v9.2.1 ./ml_logs_ueq_bb6 8 480 1

[root@mtsdev02 mrs]# INPUT_PROBLEMS_LIST=./casc_problem_lists/fne.list ./crates/mrs-bench/collect_ml_data.sh /mnt/sdf1/TPTP-v9.2.1 ./ml_logs_fne_bb6 8 480 1

[www@teenf9901 mrs]$ INPUT_PROBLEMS_LIST=./casc_problem_lists/epr.list ./crates/mrs-bench/collect_ml_data.sh /DATA/ai/TPTP-v9.2.1 ./ml_logs_epr_bb6 29 480 1

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ INPUT_PROBLEMS_LIST=./casc_problem_lists/feq.list ./crates/mrs-bench/collect_ml_data.sh /DATA/ai/TPTP-v9.2.1 ./ml_logs_feq_bb6 30 480 1

[www@teenf9901 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions epu  --casc-times --jobs 4
CASC-30 Results — 2026-06-21 12:35  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPU            100        13    8.688
------------------  --------------------
TOTAL          100        13    8.688

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.


[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions eps  --casc-times --jobs 4
CASC-30 Results — 2026-06-21 12:33  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPS            100        21   12.115
------------------  --------------------
TOTAL          100        21   12.115

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

commit 5ee3df42684b4219430211c13f5b4ce941341cc6 (HEAD -> fix-orphan-elimination-soundness

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ MRS_WORKERS=1 ./crates/mrs-bench/run_strategy_sweep.sh --divisions eps --casc-times --jobs 30
CASC-30 Results — 2026-06-21 07:44  (100 problems × 15 systems)
===============================================================

Division  Problems    mrs-s01               mrs-s02               mrs-s03               mrs-s04               mrs-s05               mrs-s06               mrs-s07               mrs-s08               mrs-s09               mrs-s10               mrs-s11               mrs-s12               mrs-s13               mrs-s14               mrs-s15
                      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------
EPS            100        19    9.451          22   16.550          22   20.798           0    0.000          10    1.276          12    2.099          20   10.909          21   18.083          22   20.288           0    0.000           0    0.000           0    0.000           0    0.000           0    0.000           0    0.000
------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------
TOTAL          100        19    9.451          22   16.550          22   20.798           0    0.000          10    1.276          12    2.099          20   10.909          21   18.083          22   20.288           0    0.000           0    0.000           0    0.000           0    0.000           0    0.000           0    0.000

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[www@teenf9901 mrs]$ MRS_WORKERS=1 ./crates/mrs-bench/run_strategy_sweep.sh --divisions epu --casc-times --jobs 30
CASC-30 Results — 2026-06-21 07:48  (100 problems × 15 systems)
===============================================================

Division  Problems    mrs-s01               mrs-s02               mrs-s03               mrs-s04               mrs-s05               mrs-s06               mrs-s07               mrs-s08               mrs-s09               mrs-s10               mrs-s11               mrs-s12               mrs-s13               mrs-s14               mrs-s15
                      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------
EPU            100         8    1.504           7    2.843           7    2.965          11    8.181           8    3.749           9   14.916           8    1.472           8    4.126           7    2.268           5    0.040           7    1.560           8    0.932           5    0.050           6    0.044           8    0.924
------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------
TOTAL          100         8    1.504           7    2.843           7    2.965          11    8.181           8    3.749           9   14.916           8    1.472           8    4.126           7    2.268           5    0.040           7    1.560           8    0.932           5    0.050           6    0.044           8    0.924

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

commit 95afbfdc509559cf66b8f93865dc5169250741b2 

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions eps  --casc-times --jobs 4
CASC-30 Results — 2026-06-20 17:00  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPS            100        28   16.490
------------------  --------------------
TOTAL          100        28   16.490

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[www@teenf9901 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions epu  --casc-times --jobs 4
CASC-30 Results — 2026-06-20 17:13  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPU            100        13   14.535
------------------  --------------------
TOTAL          100        13   14.535

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — 4 case(s) of wrong SZS polarity:
  EPU     SYN846-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SYN861-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SYN865-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SYN885-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND

REFERENCE VIOLATIONS — 4 SOUNDNESS ERROR(S) vs reference answers:
  EPU     SYN846-1                        mrs=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SYN861-1                        mrs=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SYN885-1                        mrs=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SYN865-1                        mrs=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND

commit ec7447d8a3eb31b6c480b89e628e53cf63f22f93

[www@teenf9901 mrs]$ MRS_WORKERS=1 ./crates/mrs-bench/run_strategy_sweep.sh --divisions epu --casc-times --jobs 30
CASC-30 Results — 2026-06-20 16:20  (100 problems × 15 systems)
===============================================================

Division  Problems    mrs-s01               mrs-s02               mrs-s03               mrs-s04               mrs-s05               mrs-s06               mrs-s07               mrs-s08               mrs-s09               mrs-s10               mrs-s11               mrs-s12               mrs-s13               mrs-s14               mrs-s15
                      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------
EPU            100        11   16.184           7    2.775           7    2.579           7    2.586           8    2.652           9   15.706          10    5.233           8    4.088           7    2.795           5    0.052           7    1.159           7    0.841           5    0.045           6    0.046           7    0.907
------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------
TOTAL          100        11   16.184           7    2.775           7    2.579           7    2.586           8    2.652           9   15.706          10    5.233           8    4.088           7    2.795           5    0.052           7    1.159           7    0.841           5    0.045           6    0.046           7    0.907

DISAGREEMENTS — 1 problem(s) where systems gave contradictory answers:
  EPU     SYN885-1                        mrs-s01=Satisfiable  mrs-s02=Unsatisfiable  mrs-s03=Unsatisfiable  mrs-s07=Satisfiable  mrs-s08=Satisfiable  mrs-s09=Unsatisfiable  ⚠ SOUNDNESS

POLARITY VIOLATIONS — 8 case(s) of wrong SZS polarity:
  EPU     SYN846-1                        mrs-s01=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SYN846-1                        mrs-s07=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SYN861-1                        mrs-s01=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SYN861-1                        mrs-s07=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SYN865-1                        mrs-s01=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SYN885-1                        mrs-s01=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SYN885-1                        mrs-s07=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SYN885-1                        mrs-s08=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND

REFERENCE VIOLATIONS — 8 SOUNDNESS ERROR(S) vs reference answers:
  EPU     SYN846-1                        mrs-s01=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SYN846-1                        mrs-s07=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SYN861-1                        mrs-s01=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SYN861-1                        mrs-s07=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SYN885-1                        mrs-s01=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SYN865-1                        mrs-s01=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SYN885-1                        mrs-s07=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SYN885-1                        mrs-s08=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND


[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ MRS_WORKERS=1 ./crates/mrs-bench/run_strategy_sweep.sh --divisions eps --casc-times --jobs 30
CASC-30 Results — 2026-06-20 16:12  (100 problems × 15 systems)
===============================================================

Division  Problems    mrs-s01               mrs-s02               mrs-s03               mrs-s04               mrs-s05               mrs-s06               mrs-s07               mrs-s08               mrs-s09               mrs-s10               mrs-s11               mrs-s12               mrs-s13               mrs-s14               mrs-s15
                      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------
EPS            100        25   12.507          22   15.689          21   16.575           0    0.000          16    8.771          14    8.068          24   10.411          21   14.180          21   16.541           0    0.000           0    0.000           0    0.000           0    0.000           0    0.000           0    0.000
------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------
TOTAL          100        25   12.507          22   15.689          21   16.575           0    0.000          16    8.771          14    8.068          24   10.411          21   14.180          21   16.541           0    0.000           0    0.000           0    0.000           0    0.000           0    0.000           0    0.000

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

commit 987f208d5eb7ced5c29a4013c76d21bc2835e6db

[root@mtsdev03 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions ueq  --casc-times --jobs 1
CASC-30 Results — 2026-06-21 07:55  (300 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
UEQ            300        30   30.580
------------------  --------------------
TOTAL          300        30   30.580

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

mtsdev03
CASC-30 Results — 2026-06-20 06:03  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPS            100        23   12.607
------------------  --------------------
TOTAL          100        23   12.607

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

mtsdev02
CASC-30 Results — 2026-06-20 05:57  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPU            100        10    5.885
------------------  --------------------
TOTAL          100        10    5.885

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

99 3
CASC-30 Results — 2026-06-20 06:01  (101 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
ICU            101         2   24.564
------------------  --------------------
TOTAL          101         2   24.564

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

99 3
CASC-30 Results — 2026-06-20 06:00  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        44   22.582
------------------  --------------------
TOTAL          100        44   22.582

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

97 MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions feq  --casc-times --jobs 4
CASC-30 Results — 2026-06-20 05:56  (400 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FEQ            400        76   31.604
------------------  --------------------
TOTAL          400        76   31.604

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

commit 37b118520f1a04be9bace8a99ffecf066fb3237e

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ MRS_WORKERS=1 ./crates/mrs-bench/run_strategy_sweep.sh --divisions fne,feq,ueq,epu,eps,icu --casc-times --jobs 30
CASC-30 Results — 2026-06-19 08:05  (1101 problems × 15 systems)
================================================================

Division  Problems    mrs-s01               mrs-s02               mrs-s03               mrs-s04               mrs-s05               mrs-s06               mrs-s07               mrs-s08               mrs-s09
           mrs-s10               mrs-s11               mrs-s12               mrs-s13               mrs-s14               mrs-s15
                      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)
Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------
FNE            100        21   14.133          21    8.793          20    5.245          25   11.343          14    9.719          12   11.397          21   14.674          26    8.066          21   17.356           5    0.395          28   12.557          26   32.708           5    0.219          10    6.080          23   35.595
FEQ            400        44   41.363          17   25.471          12    6.234          36   24.598          32   28.034          26   41.493          38   23.465          45   24.635          13   16.682          25   30.314          41   14.514          35   30.511          21   22.019          19   10.462          17   29.931
UEQ            300        22   76.237          18   28.297          12   32.816          24   53.371          26   62.013          22   63.040          14   59.446          16   21.634           9   42.400           2   20.743          31   60.057          26   69.644           2    2.754          22   61.181          22   87.720
EPU            100         7    1.623           7    2.452           6    0.051           7    2.824           8    3.605           9   15.509           7    1.193           7    4.420           6    0.059           5    0.052           7    1.516           7    0.853           5    0.054           6    0.054           7    0.807
EPS            100         0    0.000          22   15.551           0    0.000           0    0.000           0    0.000          14    8.043           0    0.000           0    0.000           0    0.000           0    0.000           0    0.000           0    0.000           0    0.000           0    0.000           0    0.000
ICU            101         1    0.285           1   19.363           1    0.271           1    0.234           1    0.262           1   21.282           1    0.245           1    0.272           1    0.284           0    0.000           1    0.253           2   27.753           0    0.000           1   18.719           1   22.668
------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------
TOTAL         1101        95   40.059          86   17.508          51   11.256          93   26.559          81   33.019          84   34.237          81   25.194          95   17.849          50   19.272          37   21.664         108   26.105          96   39.485          33   14.220          58   28.010          70   46.938

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

commit d2c537256c90aa3c53c5eec437f1a7890790abb8 (HEAD -> fix-subsumption-hang, origin/fix-subsumption-hang)

[ongoing]
hack@pve:~/mrs$ MRS_WORKERS=4 crates/mrs-bench/casc.sh --systems mrs --divisions feq  --casc-times --jobs 1

hack@pve:~/mrs$ MRS_WORKERS=4 crates/mrs-bench/casc.sh --systems mrs --divisions eps  --casc-times --jobs 1

CASC-30 Results — 2026-06-16 15:52  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPS            100        31    4.910
------------------  --------------------
TOTAL          100        31    4.910

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

hack@pve:~/mrs$ MRS_WORKERS=4 crates/mrs-bench/casc.sh --systems mrs --divisions epu  --casc-times --jobs 1

CASC-30 Results — 2026-06-16 13:21  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPU            100         7    1.657
------------------  --------------------
TOTAL          100         7    1.657

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.


commit ce065cef7e083008dc26d679b4a9d8c3672f4353

[root@mtsdev03 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions epu --casc-times --jobs 1
CASC-30 Results — 2026-06-16 13:20  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPU            100         7    0.599
------------------  --------------------
TOTAL          100         7    0.599

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.



[done]
[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ MRS_WORKERS=1 ./crates/mrs-bench/run_strategy_sweep.sh --divisions fne --casc-times --jobs 30
CASC-30 Results — 2026-06-17 13:43  (100 problems × 15 systems)
===============================================================

Division  Problems    mrs-s01               mrs-s02               mrs-s03               mrs-s04               mrs-s05               mrs-s06               mrs-s07               mrs-s08               mrs-s09               mrs-s10               mrs-s11               mrs-s12               mrs-s13               mrs-s14               mrs-s15
                      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------
FNE            100        21   12.886          23   28.113          20    7.240          24    5.969          14    9.799          12   10.852          21   15.150          26    8.107          20    4.884           5    0.359          28   11.641          26   32.315           5    0.214          10    6.145          23   36.192
------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------
TOTAL          100        21   12.886          23   28.113          20    7.240          24    5.969          14    9.799          12   10.852          21   15.150          26    8.107          20    4.884           5    0.359          28   11.641          26   32.315           5    0.214          10    6.145          23   36.192

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ MRS_WORKERS=1 ./crates/mrs-bench/run_strategy_sweep.sh --divisions ueq,epu,eps,icu --casc-times --jobs 30
CASC-30 Results — 2026-06-17 13:13  (601 problems × 15 systems)
===============================================================

Division  Problems    mrs-s01               mrs-s02               mrs-s03               mrs-s04               mrs-s05               mrs-s06               mrs-s07               mrs-s08               mrs-s09               mrs-s10               mrs-s11               mrs-s12               mrs-s13               mrs-s14               mrs-s15
                      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------
UEQ            300        22   74.862          18   26.277          12   32.241          26   66.064          26   59.953          22   62.844          14   56.176          14   38.855           9   38.154           0    0.000          26   55.555          26   65.812           0    0.000          10   35.505          21   73.884
EPU            100         7    1.719           7    3.036           6    0.054           7    2.716           8    3.355           9   16.255           7    1.402           7    4.397           6    0.051           5    0.055           7    1.410           8    0.816           5    0.052           6    0.048          11    5.701
EPS            100         0    0.000          22   16.339           0    0.000           0    0.000           0    0.000          14    7.658           0    0.000           0    0.000           0    0.000           0    0.000           0    0.000          22   11.857           0    0.000           0    0.000          27   13.836
ICU            101         1    0.271           1   18.184           1    0.256           1    0.262           1    0.271           1   16.378           1    0.310           1    0.368           1    0.341           0    0.000           1    0.299           2   25.761           0    0.000           1   16.381           1   20.243
------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------
TOTAL          601        30   55.309          48   18.164          19   20.393          34   51.087          35   45.311          46   35.923          22   36.209          22   26.141          16   21.502           5    0.055          34   42.783          58   35.000           5    0.052          17   21.866          60   33.468

DISAGREEMENTS — 1 problem(s) where systems gave contradictory answers:
  EPU     SYN885-1                        mrs-s02=Unsatisfiable  mrs-s12=Satisfiable  mrs-s15=Satisfiable  ⚠ SOUNDNESS

POLARITY VIOLATIONS — 5 case(s) of wrong SZS polarity:
  EPU     SYN846-1                        mrs-s15=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SYN861-1                        mrs-s15=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SYN866-1                        mrs-s15=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SYN885-1                        mrs-s12=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SYN885-1                        mrs-s15=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND

REFERENCE VIOLATIONS — 5 SOUNDNESS ERROR(S) vs reference answers:
  EPU     SYN846-1                        mrs-s15=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SYN861-1                        mrs-s15=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SYN866-1                        mrs-s15=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SYN885-1                        mrs-s12=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SYN885-1                        mrs-s15=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND

[done]
[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ INPUT_PROBLEMS_LIST=./casc_problem_lists/feq.list ./crates/mrs-bench/collect_ml_data.sh /DATA/ai/TPTP-v9.2.1 ./ml_logs_feq_sound 30 480 1

[stopped]
[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ INPUT_PROBLEMS_LIST=./casc_problem_lists/epr.list ./crates/mrs-bench/collect_ml_data.sh /DATA/ai/TPTP-v9.2.1 ./ml_logs_epr_sound 30 480 1

[done]
[www@teenf9901 mrs]$ INPUT_PROBLEMS_LIST=./casc_problem_lists/fne.list ./crates/mrs-bench/collect_ml_data.sh /DATA/ai/TPTP-v9.2.1 ./ml_logs_fne_sound 29 480 1

[stopped]
[www@teenf9901 mrs]$ INPUT_PROBLEMS_LIST=./casc_problem_lists/ueq.list ./crates/mrs-bench/collect_ml_data.sh /DATA/ai/TPTP-v9.2.1 ./ml_logs_ueq_sound 29 480 1

[done]
[www@teenf9901 mrs]$ MRS_WORKERS=1 ./crates/mrs-bench/run_strategy_sweep.sh --divisions feq --casc-times --jobs 29

[root@mtsdev03 mrs]# cargo run --release --bin mrs -- --time 4800 /mnt/sdd/TPTP-v9.2.1/Problems/RNG/RNG008-1.p
% Resolved 1 include directive(s)
% Problem: RNG008-1 (0 axioms, 0 conjectures, 20 cnf clauses)
% SZS status GaveUp for RNG008-1
% ------------------------------
% Version: mrs 0.1.9
% Termination reason: GaveUp
% Time elapsed: 5336.549 s
% Peak memory usage: 22477 MB
% ------------------------------
% SZS detail strategies=14 timeout=8 saturated=0 processed=26720 generated=4042320 passive=189832 weight_discarded=0 lrs_discarded=57215031 fwd_subsumed=138842

[root@mtsdev02 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions ueq --casc-times --jobs 1
CASC-30 Results — 2026-06-16 13:11  (300 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
UEQ            300        14   21.791
------------------  --------------------
TOTAL          300        14   21.791

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

stopped at 20%
[root@mtsdev02 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions eps --casc-times --jobs 1

CASC-30 Results — 2026-06-15 16:19  (101 problems × 1 systems)
==============================================================

[www@teenf9901 ~]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions icu  --casc-times --jobs 3
Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
ICU            101         1   15.522
------------------  --------------------
TOTAL          101         1   15.522

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.


[www@teenf9901 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions eps  --casc-times --jobs 3
CASC-30 Results — 2026-06-15 10:17  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPS            100        33    6.209
------------------  --------------------
TOTAL          100        33    6.209

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[www@teenf9901 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions fne  --casc-times --jobs 3
CASC-30 Results — 2026-06-15 09:11  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        31   23.471
------------------  --------------------
TOTAL          100        31   23.471

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[ongoing]
[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions feq  --casc-times --jobs 4

CASC-30 Results — 2026-06-15 14:14  (400 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FEQ            400        65   49.566
------------------  --------------------
TOTAL          400        65   49.566

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

commit 6e483ae5041d24797e2d77a4bf58fe0989217b7a (HEAD -> fne-portfolio-update, origin/fne-portfolio-update)

laptop ❯ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions fne --casc-times --jobs 1 --output fne_test
CASC-30 Results — 2026-06-14 21:53  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        35   21.850
------------------  --------------------
TOTAL          100        35   21.850

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.


commit badfa811f2ba17115bc27cd72ad650b0daa6337d (HEAD -> casc-parallel-fix

ongoing stoped 503/1500 completed
laptop ./crates/mrs-bench/run_strategy_sweep.sh --divisions fne --casc-times --jobs 4 --output testC


Results for mrs commit 76519b5ca56436e37b23450f6be51274619df5cb UNSOUND

[ongoing]
hack@pve:~/mrs$ MRS_WORKERS=4 crates/mrs-bench/casc.sh --systems mrs --divisions feq  --casc-times --jobs 1


hack@pve:~/mrs$ MRS_WORKERS=4 crates/mrs-bench/casc.sh --systems mrs --divisions fne  --casc-times --jobs 1 --output results/benchmark-$(date +%Y%m%d)

CASC-30 Results — 2026-06-14 13:37  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        44   86.136
------------------  --------------------
TOTAL          100        44   86.136

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.


commit 6ad04a4743507b27d91fba37b8b4efce864d6543 (HEAD -> feature/ml-guided-clause-selection, origin/feature/ml-guided-clause-selection)

hack@pve:~/mrs$ crates/mrs-bench/casc.sh --systems mrs  --casc-times --divisions feq
CASC-30 Results — 2026-06-07 07:15  (400 problems × 1 systems)
?

stopped at LCL646+1.015.p
[root@mtsdev02 mrs]# INPUT_PROBLEMS_LIST=./casc_problem_lists/epr.list ./crates/mrs-bench/collect_ml_data.sh /mnt/sde1/TPTP-v9.2.1 ./ml_logs_epr 1 960 8

stopped at COL069-1.p
hack@pve:~/mrs$ INPUT_PROBLEMS_LIST=./casc_problem_lists/ueq.list ./crates/mrs-bench/collect_ml_data.sh /home/hack/TPTP-v9.2.1 ./ml_logs_ueq 1 960 4

stopped at NUM155-1.p maybe unsound
[www@teenf9901 mrs]$ INPUT_PROBLEMS_LIST=./casc_problem_lists/fne.list ./crates/mrs-bench/collect_ml_data.sh /DATA/ai/TPTP-v9.2.1 ./ml_logs_fne 1 480 30

stopped at SEU016+1.p maybe unsound
[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ INPUT_PROBLEMS_LIST=./casc_problem_lists/feq.list ./crates/mrs-bench/collect_ml_data.sh /DATA/ai/TPTP-v9.2.1 ./ml_logs_feq 1 480 30

commit 4252ef646b2bfeda3ef1baedb3bc47034fcc0776 (HEAD -> feature/ml-guided-clause-selection

[www@teenf9901 mrs]$ INPUT_PROBLEMS_LIST=./casc_problem_lists/fne.list ./crates/mrs-bench/collect_ml_data.sh /DATA/ai/TPTP-v9.2.1 ./ml_logs_fne 3 480

commit 369f48eccbe77c4cbd917a22d60d785dce9fd0a8 (HEAD -> feature/ml-guided-clause-selection, origin/feature/ml-guided-clause-selection)

[ongoing]
[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ crates/mrs-bench/casc.sh --systems mrs --casc-times --jobs 3 --divisions fne

commit 3380e2d65a43eab76b3f37a20c39efb123896fb8 (HEAD -> feature/ml-guided-clause-selection

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ ./crates/mrs-bench/casc.sh --systems mrs-ml --casc-times --jobs 3 --divisions feq,fne,ueq
CASC-30 Results — 2026-06-12 11:55  (800 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
FEQ            400        43   37.395
FNE            100        27   18.097
UEQ            300        18   32.996
------------------  --------------------
TOTAL          800        88   30.574

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

293abae65eda29d45d2a40111dcbec641b8dbc89

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ ./crates/mrs-bench/collect_ml_data.sh /path/to/TPTP-v9.2.1 ./ml_logs 16 30



Results for mrs commit b0ca6c15f18a5561ac795690c96f191ef61f79d7

[ongoing]
[www@teenf9901 mrs]$ crates/mrs-bench/casc.sh --systems vampire --casc-times


hack@pve:~/mrs$ crates/mrs-bench/casc.sh --systems vampire  --casc-times --divisions eps

CASC-30 Results — 2026-06-09 06:11  (100 problems × 1 systems)
==============================================================

Division  Problems    vampire
                      Solved  Avg (s)
------------------  --------------------
EPS            100        86    5.027
------------------  --------------------
TOTAL          100        86    5.027

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.


[www@teenf9901 mrs]$ crates/mrs-bench/casc.sh --systems mrs --casc-times --jobs 2 --divisions ueq

CASC-30 Results — 2026-06-09 07:35  (300 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
UEQ            300        20   23.312
------------------  --------------------
TOTAL          300        20   23.312

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

[root@mtsdev02 mrs]# TPTP=/mnt/sda1/mrs/crates/mrs-bench/problems/casc-30 crates/mrs-bench/casc.sh --systems mrs --casc-times --divisions epu
CASC-30 Results — 2026-06-09 07:25  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPU            100         8    1.808
------------------  --------------------
TOTAL          100         8    1.808

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.


hack@pve:~/mrs$ crates/mrs-bench/casc.sh --systems mrs  --casc-times --divisions eps

CASC-30 Results — 2026-06-08 21:14  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPS            100        22   16.077
------------------  --------------------
TOTAL          100        22   16.077

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

Results for mrs commit 4ea73eb2d96e6364ca73ef455cfd52d1f62bdea2

partial ongoing
[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ crates/mrs-bench/casc.sh --systems mrs --casc-times --jobs 2
------------------  --------------------
FNE            100        31   25.060
FEQ            400        48   41.741
EPU            100         9   12.152
EPS            100        26   13.328
UEQ            300        23   15.731
ICU             87         1   15.711
------------------  --------------------
TOTAL         1087       138   26.187

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — 2 case(s) of wrong SZS polarity:
  UEQ     GRP024-5                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     GRP196-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND



Results for mrs commit a3cc272eddf2fd408b4705dd650025dbde44a1e0


crash ?
hack@pve:~/mrs$ crates/mrs-bench/casc.sh --systems mrs  --casc-times --divisions feq
CASC-30 Results — 2026-06-06 21:20  (376 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FEQ            376        41   35.218
------------------  --------------------
TOTAL          376        41   35.218

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

Results for mrs commit 65a6aebe14d676ba8ab990a1397b4a340483c36f (HEAD -> casc-improvements, origin/fix-imperfect-indexing, origin/casc-improvements)

[ongoing]
[www@teenf9901 mrs]$ crates/mrs-bench/casc.sh --systems mrs --casc-times --jobs 2

Results for mrs commit 65a6aebe14d676ba8ab990a1397b4a340483c36f (HEAD -> fix-imperfect-indexing, origin/fix-imperfect-indexing)

interrupted ?
hack@pve:~/mrs$ crates/mrs-bench/casc.sh --systems mrs  --casc-times --divisions feq
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FEQ            364        41   37.644
------------------  --------------------
TOTAL          364        41   37.644

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

Results for mrs commit b8005170ff45ef6e1507863a651f28b40328e0d9 (HEAD -> fix-epr-grounding, origin/fix-epr-grounding)

[www@teenf9901 mrs]$ crates/mrs-bench/casc.sh --systems mrs --casc-times --divisions epu --jobs 2
CASC-30 Results — 2026-06-05 06:53  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPU            100         8    2.251
------------------  --------------------
TOTAL          100         8    2.251

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

Results for mrs commit ee8aaf20cfbf7a9c6fefff21303c5eb038191e09 (HEAD -> fix-sine-over-pruning, origin/fix-sine-over-pruning)

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ crates/mrs-bench/casc.sh --systems mrs --casc-times --jobs 2

CASC-30 Results — 2026-06-06 06:35  (1101 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        32   18.431
FEQ            400        46   17.958
EPU            100         8    0.603
EPS            100        23   15.448
UEQ            300        36   44.200
ICU            101         1   10.395
------------------  --------------------
TOTAL         1101       146   23.134

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.


Results for mrs commit 34338df31d907db20e708e6dc4d74e63a29d2e9a

aborted to 212/1101 (very slow)
[www@teenf9901 mrs]$ crates/mrs-bench/casc.sh --systems mrs --casc-times

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ crates/mrs-bench/casc.sh --systems mrs --casc-times --jobs 3
CASC-30 Results — 2026-06-04 14:22  (1101 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        30   21.895
FEQ            400        45   23.244
EPU            100         8    0.901
EPS            100        23   20.575
UEQ            300        32   47.636
ICU            101         1   20.648
------------------  --------------------
TOTAL         1101       139   26.822

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

[ongoing]
hack@pve:~/mrs$ crates/mrs-bench/casc.sh --systems mrs  --casc-times --divisions feq
cancelled at 212/400 33 Solved

OOM ?
hack@pve:~/mrs$ crates/mrs-bench/casc.sh --systems mrs  --casc-times

Results for mrs commit c0816a7a24dfb287d0eccc3e23b75d00c54d2fc8

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ crates/mrs-bench/casc.sh --systems mrs --jobs 30
CASC-30 Results — 2026-06-03 08:13  (1101 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        24   12.129
FEQ            400        27   19.331
EPU            100         8    2.738
EPS            100        13    0.670
UEQ            300        13   51.488
ICU            101         1    0.289
------------------  --------------------
TOTAL         1101        86   17.596

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

check commit
hack@pve:~/mrs$ crates/mrs-bench/casc.sh --systems mrs  --jobs 4
CASC-30 Results — 2026-06-03 04:51  (1101 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        24   12.374
FEQ            400        27   19.354
EPU            100         8    2.914
EPS            100        13    0.731
UEQ            300        12   49.054
ICU            101         1    0.309
------------------  --------------------
TOTAL         1101        85   16.956

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.


Results for mrs commit      f0638f5013ee34319fece821c5979f01fcaaebae

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ crates/mrs-bench/casc.sh --systems mrs --jobs 30
CASC-30 Results — 2026-06-03 06:44  (1101 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        28   17.650
FEQ            400        20   16.206
EPU            100         8    3.203
EPS            100        14    2.101
UEQ            300         0    0.000
ICU            101         2    1.154
------------------  --------------------
TOTAL         1101        72   12.162

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.



Results for mrs commit  e2dc18b19564f85f98d6e0d0c9e054a642bbc4a1

[ongoing]
hack@pve:~/mrs$ crates/mrs-bench/casc.sh --systems mrs  --jobs 4
CASC-30 Results — 2026-06-02 06:13  (1101 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        25   13.686
FEQ            400        18   19.094
EPU            100         9    2.931
EPS            100        31    2.154
UEQ            300        26   38.725
ICU            101         1    0.285
------------------  --------------------
TOTAL         1101       110   16.237

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — 1 case(s) of wrong SZS polarity:
  EPU     SYN914-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND

[ongoing]
[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ crates/mrs-bench/casc.sh --systems mrs --jobs 30
CASC-30 Results — 2026-06-02 06:16  (1101 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        26   12.955
FEQ            400        19   21.982
EPU            100         9    2.555
EPS            100        31    3.379
UEQ            300        26   38.657
ICU            101         1    0.279
------------------  --------------------
TOTAL         1101       112   16.853

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — 1 case(s) of wrong SZS polarity:
  EPU     SYN914-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND

Results for mrs commit 635834c3b5f6c2c15a7647e724a335a938a86f1b

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ crates/mrs-bench/casc.sh --systems mrs --jobs 30
CASC-30 Results — 2026-06-01 08:30  (1101 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        24   10.356
FEQ            400        18   22.705
EPU            100         8    3.132
EPS            100         0    0.000
UEQ            300        18   35.236
ICU            101         2    9.432
------------------  --------------------
TOTAL         1101        70   19.077

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

Verifying SYN355+1...
-> % SZS status NotVerified : step 4: leaf with anonymous provenance (file(_,unknown)) does not α-match any premise-role formula in the linked problem (may differ only by AC-rewriting of commutative operators)
Verifying SYN424+1...
-> % SZS status FailedVerified : structural: proof does not derive $false
Found solution for SYN325+1. Downloading...
Verifying SYN325+1...
-> % SZS status FailedVerified : structural: proof does not derive $false
Found solution for SYN516+1. Downloading...
Verifying SYN516+1...
-> % SZS status FailedVerified : structural: proof does not derive $false
Verifying SYN551+1...
-> % SZS status NotVerified : load error: cannot parse problem file /tmp/tmp.YsE8SFa4sX/Problems/SYN551+1.p: parse error at byte offset 1968: Cut(ContextError { context: [Label("fof_statement"), Label("FOF formula"), Label("annotated_formula"), Label("tptp_input")], cause: None })
Verifying SYN439+1...
-> % SZS status FailedVerified : structural: proof does not derive $false
Verifying SYN507+1...
-> % SZS status FailedVerified : structural: node c_49 is not FOF
Verifying SYN508+1...
Verifying SYN978+1...
-> % SZS status NotVerified : load error: cannot parse problem file /tmp/tmp.YsE8SFa4sX/Problems/SYN978+1.p: parse error at byte offset 1169: Cut(ContextError { context: [Label("fof_statement"), Label("FOF formula"), Label("annotated_formula"), Label("tptp_input")], cause: None })-> % SZS status FailedVerified : structural: node c_49 is not FOF

Results for mrs commit 635834c3b5f6c2c15a7647e724a335a938a86f1b



hack@pve:~/mrs$ cargo run --release --bin mrs -- --time 480 ~/TPTP-v9.2.1/Problems/GRP/GRP678-1.p
    Finished `release` profile [optimized] target(s) in 0.30s
     Running `target/release/mrs --time 480 /home/hack/TPTP-v9.2.1/Problems/GRP/GRP678-1.p`
% Problem: GRP678-1 (0 axioms, 0 conjectures, 13 cnf clauses)
% SZS status Timeout for GRP678-1
% ------------------------------
% Version: mrs 0.1.8
% Termination reason: Timeout
% Time elapsed: 480.382 s
% Peak memory usage: 258 MB
% ------------------------------

Results for mrs commit ec266ecb29b1b7db1c37341c137a41e7e9e11505

hack@pve:~/mrs$ crates/mrs-bench/casc.sh --systems mrs  --jobs 4
CASC-30 Results — 2026-06-01 06:05  (1101 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        24   13.099
FEQ            400        17   21.917
EPU            100         8    3.417
EPS            100         0    0.000
UEQ            300        17   32.421
ICU            101         1    0.323
------------------  --------------------
TOTAL         1101        67   18.892

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

Results for mrs commit 40b17e46103c71888d09be3ec59430a92c724008

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ crates/mrs-bench/casc.sh --systems mrs,vampire,eprover --jobs 30
CASC-30 Results — 2026-06-01 06:28  (1101 problems × 3 systems)
===============================================================

Division  Problems    eprover               mrs                   vampire
                      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------  --------------------
FNE            100        65   12.337          24   13.759          75   10.169
FEQ            400       223   12.913          16   18.979         348    7.484
EPU            100        22    5.084           8    3.622          67   20.728
EPS            100        63    4.745           0    0.000          86    7.600
UEQ            300       166   14.653          17   38.830         227   22.991
ICU            101        12   36.898           1    0.254          37   32.268
------------------  --------------------  --------------------  --------------------
TOTAL         1101       551   12.645          66   20.049         840   14.074

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

hack@pve:~/mrs$ crates/mrs-bench/casc.sh --systems mrs,vampire,eprover --jobs 4
     Running `target/debug/bench_report /home/hack/mrs/crates/mrs-bench/results/casc-30/20260529_195037/run.csv`
CASC-30 Results — 2026-05-30 12:40  (1101 problems × 3 systems)
===============================================================

Division  Problems    eprover               mrs                   vampire
                      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------  --------------------
FNE            100        63   11.175          23   14.392          71    7.591
FEQ            400       221   13.045          15   19.771         327    8.069
EPU            100        22    4.471           8    3.813          62   20.040
EPS            100        63    5.406           0    0.000          85    6.895
UEQ            300       166   14.856          16   38.827         195   17.546
ICU            101        12   36.522           1    0.400          27   16.116
------------------  --------------------  --------------------  --------------------
TOTAL         1101       547   12.670          63   20.313         767   11.555

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

Results for mrs commit 9f386fff39b43a4c77a631e066080524bc74231f

to be analyzed
hack@pve:~/mrs$ cargo run --release --bin mrs -- --time 480 ~/TPTP-v9.2.1/Problems/GRP/GRP751-1.p
    Finished `release` profile [optimized] target(s) in 0.27s
     Running `target/release/mrs --time 480 /home/hack/TPTP-v9.2.1/Problems/GRP/GRP751-1.p`
% Problem: GRP751-1 (0 axioms, 0 conjectures, 8 cnf clauses)
% SZS status Timeout for GRP751-1
% ------------------------------
% Version: mrs 0.1.10
% Termination reason: Timeout
% Time elapsed: 480.533 s
% Peak memory usage: 229 MB
% ------------------------------

Results for mrs commit 4345265f468dc6038471ada870239dbe9c8edec0

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ crates/mrs-bench/casc.sh --systems reference,mrs --jobs 30 --time 240
     Running `target/debug/bench_report /DATA/ai/fr22192/mrs/crates/mrs-bench/results/casc-30/20260529_153630/run.csv`
CASC-30 Results — 2026-05-29 15:49  (1101 problems × 2 systems)
===============================================================

Division  Problems    mrs                   reference
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
FNE            100        26   27.331         100    0.032
FEQ            400        16   41.223         399    0.031
EPU            100         8    5.083         100    0.032
EPS            100         0    0.000         100    0.033
UEQ            300        26   50.078         300    0.031
ICU            101         2   17.092          57    0.031
------------------  --------------------  --------------------
TOTAL         1101        78   35.219        1056    0.031

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ crates/mrs-bench/casc.sh --systems reference,mrs --jobs 30
CASC-30 Results — 2026-05-29 12:17  (1101 problems × 2 systems)
===============================================================

Division  Problems    mrs                   reference
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
FNE            100        24   13.303         100    0.032
FEQ            400        16   21.884         399    0.031
EPU            100         8    3.534         100    0.032
EPS            100         0    0.000         100    0.033
UEQ            300        17   37.019         300    0.031
ICU            101         1    0.363          57    0.032
------------------  --------------------  --------------------
TOTAL         1101        66   20.112        1056    0.032

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

hack@pve:~/mrs$ TPTP=/home/hack/mrs/crates/mrs-bench/problems/casc-30 crates/mrs-bench/casc.sh --systems reference,mrs --jobs 3 --divisions ueq
CASC-30 Results — 2026-05-29 12:21  (300 problems × 2 systems)
==============================================================

Division  Problems    mrs                   reference
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
UEQ            300        18   32.820         300    0.014
------------------  --------------------  --------------------
TOTAL          300        18   32.820         300    0.014

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

Results for mrs commit df21f4b06de8483420a0ff72aef7f2e3129dcc52

[root@mtsdev02 mrs]# TPTP=/mnt/sda1/mrs/crates/mrs-bench/problems/casc-30 crates/mrs-bench/casc.sh --systems reference,mrs
CASC-30 Results — 2026-06-01 06:13  (1101 problems × 2 systems)
===============================================================

Division  Problems    mrs                   reference
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
FNE            100        26   19.243           0    0.000
FEQ            400        16   20.713           0    0.000
EPU            100         8    3.019           0    0.000
EPS            100         0    0.000           0    0.000
UEQ            300        21   29.076           0    0.000
ICU            101         2   47.490           0    0.000
------------------  --------------------  --------------------
TOTAL         1101        73   21.390           0    0.000

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

Results for mrs commit b4b78ac62853b7903d64ab6654b2e7e86e426dd2

[www@teenf9901 mrs]$ TPTP=/DATA/ai/mrs/crates/mrs-bench/problems/casc-30 crates/mrs-bench/casc.sh --systems reference,mrs
CASC-30 Results — 2026-06-01 06:32  (1101 problems × 2 systems)
===============================================================

Division  Problems    mrs                   reference
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
FNE            100        25   15.355         100    0.019
FEQ            400        17   21.700         399    0.019
EPU            100         8    2.729         100    0.019
EPS            100         0    0.000         100    0.019
UEQ            300        21   25.095         300    0.019
ICU            101         2    9.045          57    0.019
------------------  --------------------  --------------------
TOTAL         1101        73   18.078        1056    0.019

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

Results for mrs commit a5b220380d6a24234c59275a188a4f6a948f7160

[www@teenf9901 mrs]$ TPTP=/DATA/ai/mrs/crates/mrs-bench/problems/casc-30 crates/mrs-bench/casc.sh --systems reference,mrs --divisions fne
Division  Problems    mrs                   reference
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
FNE            100        26   13.872         100    0.019
------------------  --------------------  --------------------
TOTAL          100        26   13.872         100    0.019

DISAGREEMENTS — 3 problem(s) where systems gave contradictory answers:
  FNE     LCL660+1.015                    mrs=CounterSatisfiable  reference=Theorem  ⚠ SOUNDNESS
  FNE     SYN938+1                        mrs=CounterSatisfiable  reference=Theorem  ⚠ SOUNDNESS
  FNE     SYN986+1.004                    mrs=CounterSatisfiable  reference=Theorem  ⚠ SOUNDNESS

POLARITY VIOLATIONS — none detected.

Results for mrs commit c6eb579c47b11726154c1eaa6c215c1edc21ea2c

[root@mtsdev02 mrs]# TPTP=/mnt/sda1/mrs/crates/mrs-bench/problems/casc-30 crates/mrs-bench/casc.sh --systems reference,mrs
CASC-30 Results — 2026-05-28 16:26  (1101 problems × 2 systems)
===============================================================

Division  Problems    mrs                   reference
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
FNE            100        25   11.442           0    0.000
FEQ            400       208   11.069           0    0.000
EPU            100        11   12.286           0    0.000
EPS            100        18    9.831           0    0.000
UEQ            300        26   12.263           0    0.000
ICU            101        40   15.654           0    0.000
------------------  --------------------  --------------------
TOTAL         1101       328   11.724           0    0.000

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — 13 case(s) of wrong SZS polarity:
  EPU     SYN885-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPS     NLP006-1                        mrs=Unsatisfiable  (expected one of ["CounterSatisfiable", "Satisfiable"])  ⚠ UNSOUND
  EPS     NLP008-1                        mrs=Unsatisfiable  (expected one of ["CounterSatisfiable", "Satisfiable"])  ⚠ UNSOUND
  EPS     NLP012-1                        mrs=Unsatisfiable  (expected one of ["CounterSatisfiable", "Satisfiable"])  ⚠ UNSOUND
  UEQ     LAT080-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LAT081-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LAT082-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LAT084-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LAT085-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LAT092-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LAT096-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LAT097-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LAT392-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND

Results for mrs commit 38f023ef0e0319226bf659fc5dc9f53511eb81c2

[www@teenf9901 mrs]$ TPTP=/DATA/ai/mrs/crates/mrs-bench/problems/casc-30 crates/mrs-bench/casc.sh --systems reference,mrs
CASC-30 Results — 2026-05-28 08:03  (1101 problems × 2 systems)
===============================================================

Division  Problems    mrs                   reference
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
FNE            100        16    9.517         100    0.019
FEQ            400        37    5.110         399    0.019
EPU            100        11   10.095         100    0.019
EPS            100        14    3.182         100    0.019
UEQ            300        19   34.888         300    0.019
ICU            101         7    0.058          57    0.019
------------------  --------------------  --------------------
TOTAL         1101       104   11.156        1056    0.019

DISAGREEMENTS — 3 problem(s) where systems gave contradictory answers:
  EPS     NLP006-1                        mrs=Unsatisfiable  reference=Satisfiable  ⚠ SOUNDNESS
  EPS     NLP008-1                        mrs=Unsatisfiable  reference=Satisfiable  ⚠ SOUNDNESS
  ICU     VVA001+1                        mrs=Theorem  reference=CounterSatisfiable  ⚠ SOUNDNESS

POLARITY VIOLATIONS — 2 case(s) of wrong SZS polarity:
  EPS     NLP006-1                        mrs=Unsatisfiable  (expected one of ["CounterSatisfiable", "Satisfiable"])  ⚠ UNSOUND
  EPS     NLP008-1                        mrs=Unsatisfiable  (expected one of ["CounterSatisfiable", "Satisfiable"])  ⚠ UNSOUND


hack@pve:~/mrs$ TPTP=/home/hack/mrs/crates/mrs-bench/problems/casc-30 crates/mrs-bench/casc.sh --systems reference,mrs --divisions eps,epu,feq
CASC-30 Results — 2026-05-28 10:44  (600 problems × 2 systems)
==============================================================

Division  Problems    mrs                   reference
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
EPS            100        15    3.939         100    0.011
EPU            100        11    4.952         100    0.011
FEQ            400        64    1.794         399    0.011
------------------  --------------------  --------------------
TOTAL          600        90    2.537         599    0.011

DISAGREEMENTS — 3 problem(s) where systems gave contradictory answers:
  EPS     NLP006-1                        mrs=Unsatisfiable  reference=Satisfiable  ⚠ SOUNDNESS
  EPS     NLP008-1                        mrs=Unsatisfiable  reference=Satisfiable  ⚠ SOUNDNESS
  EPS     NLP012-1                        mrs=Unsatisfiable  reference=Satisfiable  ⚠ SOUNDNESS

POLARITY VIOLATIONS — 3 case(s) of wrong SZS polarity:
  EPS     NLP006-1                        mrs=Unsatisfiable  (expected one of ["CounterSatisfiable", "Satisfiable"])  ⚠ UNSOUND
  EPS     NLP008-1                        mrs=Unsatisfiable  (expected one of ["CounterSatisfiable", "Satisfiable"])  ⚠ UNSOUND
  EPS     NLP012-1                        mrs=Unsatisfiable  (expected one of ["CounterSatisfiable", "Satisfiable"])  ⚠ UNSOUND

hack@pve:~/mrs$ TPTP=/home/hack/mrs/crates/mrs-bench/problems/casc-30 crates/mrs-bench/casc.sh --systems reference,mrs --divisions fne,ueq
[casc] Done. Results: /home/hack/mrs/crates/mrs-bench/results/casc-30/20260527_161327/run.csv
hack@pve:~/mrs$
────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
==============================================================

Division  Problems    mrs                   reference
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
FNE            100        19    4.990         100    0.011
UEQ            300        15   21.399         300    0.011
------------------  --------------------  --------------------
TOTAL          400        34   12.229         400    0.011

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

Results for mrs commit 4c7418c39df5b1ea489d560d4b5fb8a97f5836f0
[www@teenf9901 mrs]$ cargo run -p mrs-bench --bin bench_report -- /DATA/ai/mrs/crates/mrs-bench/results/casc-30/20260527_095507/run.csv
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.03s
     Running `target/debug/bench_report /DATA/ai/mrs/crates/mrs-bench/results/casc-30/20260527_095507/run.csv`
CASC-30 Results — 2026-05-27 09:43  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100         7    5.715
------------------  --------------------
TOTAL          100         7    5.715


Results for mrs commit d9dacb516c0cf5c17c5eda34d7b67436057ac138

mtsdev02 partial [casc] 586/1101 completed
CASC-30 Results — 2026-05-28 06:42  (1101 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100         7    5.954
FEQ            400        63    3.385
EPU            100         8    3.063
EPS            100        35    3.533
UEQ            300        20   18.862
ICU            101         9    1.570
------------------  --------------------
TOTAL         1101       142    5.595

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — 8 case(s) of wrong SZS polarity:
  EPU     MSC024-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LCL195-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LCL203-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LCL211-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LCL224-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LCL416-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     NUM284-10.014                   mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     RNG001-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND


[www@teenf9901 mrs]$ ./crates/mrs-bench/systems/vampire/bin/vampire --version
Vampire 5.0.1 (Release build, commit cb4838130 on 2026-05-26 10:04:53 +0200)
CaDiCaL: cadical-2.1.3
Linked to Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 z3-4.8.4-9178-g3c47fd96c
[www@teenf9901 mrs]$ TPTP=/DATA/ai/mrs/crates/mrs-bench/problems/casc-30 crates/mrs-bench/casc.sh --systems mrs,vampire --divisions fne,ueq
[www@teenf9901 mrs]$ cargo run -p mrs-bench --bin bench_report -- /DATA/ai/mrs/crates/mrs-bench/results/casc-30/20260526_191646/run.csv
   Compiling mrs-bench v0.1.1 (/DATA/ai/mrs/crates/mrs-bench)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.68s
     Running `target/debug/bench_report /DATA/ai/mrs/crates/mrs-bench/results/casc-30/20260526_191646/run.csv`
CASC-30 Results — 2026-05-27 06:16  (400 problems × 2 systems)
==============================================================

Division  Problems    mrs                   vampire
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
FNE            100         7    5.677          79   10.925
UEQ            300        22   28.067         233   16.129
------------------  --------------------  --------------------
TOTAL          400        29   22.663         312   14.812

DISAGREEMENTS — 8 problem(s) where systems gave contradictory answers:
  FNE     MGT067+1                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     SYN457+1                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     SYO606+1                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  UEQ     LCL195-10                       mrs=Satisfiable  vampire=Unsatisfiable  ⚠ SOUNDNESS
  UEQ     LCL203-10                       mrs=Satisfiable  vampire=Unsatisfiable  ⚠ SOUNDNESS
  UEQ     LCL211-10                       mrs=Satisfiable  vampire=Unsatisfiable  ⚠ SOUNDNESS
  UEQ     LCL224-10                       mrs=Satisfiable  vampire=Unsatisfiable  ⚠ SOUNDNESS
  UEQ     RNG001-10                       mrs=Satisfiable  vampire=Unsatisfiable  ⚠ SOUNDNESS

POLARITY VIOLATIONS — 6 case(s) of wrong SZS polarity:
  UEQ     LCL195-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LCL203-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LCL211-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LCL224-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     NUM284-10.014                   mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     RNG001-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND

hack@pve:~/mrs$ ./crates/mrs-bench/systems/vampire/bin/vampire --version
Vampire 5.0.1 (Release build, commit 1b13eaf on 2026-01-18 12:14:50 +0000)
CaDiCaL: cadical-2.1.3
Linked to Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 NOTFOUND
hack@pve:~/mrs$ TPTP=/home/hack/mrs/crates/mrs-bench/problems/casc-30 crates/mrs-bench/casc.sh --systems mrs,vampire --divisions fne,ueq
hack@pve:~/mrs$ cargo run -p mrs-bench --bin bench_report -- /home/hack/mrs/crates/mrs-bench/results/casc-30/20260526_192617/run.csv
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.03s
     Running `target/debug/bench_report /home/hack/mrs/crates/mrs-bench/results/casc-30/20260526_192617/run.csv`
CASC-30 Results — 2026-05-27 06:08  (400 problems × 2 systems)
==============================================================

Division  Problems    mrs                   vampire
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
FNE            100        34    0.494          75    6.086
UEQ            300        21   21.547         216   16.426
------------------  --------------------  --------------------
TOTAL          400        55    8.533         291   13.761

DISAGREEMENTS — 32 problem(s) where systems gave contradictory answers:
  FNE     CSR026+3                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR027+3                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR033+3                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR034+2                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR036+2                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR036+3                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR039+2                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR040+3                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR052+3                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR056+3                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR060+2                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR061+2                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR073+2                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR073+3                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR115+31                       mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR115+6                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR115+91                       mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR115+98                       mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR116+27                       mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR116+39                       mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR116+6                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     LCL642+1.010                    mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     LCL642+1.015                    mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     LCL660+1.015                    mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     MGT067+1                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     NLP262+1                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     SYN457+1                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     SYN938+1                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  UEQ     LCL195-10                       mrs=Satisfiable  vampire=Unsatisfiable  ⚠ SOUNDNESS
  UEQ     LCL203-10                       mrs=Satisfiable  vampire=Unsatisfiable  ⚠ SOUNDNESS
  UEQ     LCL211-10                       mrs=Satisfiable  vampire=Unsatisfiable  ⚠ SOUNDNESS
  UEQ     LCL224-10                       mrs=Satisfiable  vampire=Unsatisfiable  ⚠ SOUNDNESS

POLARITY VIOLATIONS — 7 case(s) of wrong SZS polarity:
  UEQ     LCL195-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LCL203-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LCL211-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LCL224-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LCL416-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     NUM284-10.014                   mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     RNG001-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND

[root@mtsdev02 mrs]# TPTP=/mnt/sda1/mrs/crates/mrs-bench/problems/casc-30 crates/mrs-bench/casc.sh --systems mrs
POLARITY VIOLATIONS — 1 case(s) of wrong SZS polarity:
  EPU     MSC024-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND

---
## Phase 2 Results (2026-06-13)

### Commit d96eb0fa — new heuristics branch

FNE at 30s (90/100 problems completed, 4 parallel jobs):

| Portfolio | FNE solved | Avg (s) |
|-----------|-----------|---------|
| Old (11 strategies) | 20/90 | 5.3 |
| New (16 strategies, s10-s15) | 20/90 | 4.9 |

**No regressions; +0 new solves at 30s.** The new strategies (SOS, ConjSymbolBoost, HornPenalty) solve additional *unique* problems in solo benchmarks (S11 solo: 23/100 vs S1 solo: 19/100 at 8s; S11 uniquely solves NUN060+1, NUN081+1, CSR061+2, KRS258+1 vs S1), but at 30s with 4 workers the gains are not yet visible because:
1. Hard FNE problems require E-style search space navigation that mrs still lacks
2. Problems that mrs can solve are already covered by s1-s9 at 30s

Expected improvement: visible at CASC times (240s) on problems requiring 50-200s.

### Solo strategy analysis (FNE, 8s budget):

| Strategy | Solved | Unique vs s1 |
|----------|--------|-------------|
| s1 (baseline AgeWeight+KBO) | 19/100 | — |
| s10 (SOS+KBO) | 20/100 | CSR060+2, CSR115+98 |
| s11 (ConjSymbolBoost) | **23/100** | CSR061+2, CSR115+98, KRS258+1, NUN060+1, NUN081+1 |
| s12 (HornPenalty) | 19/100 | CSR115+98, KRS258+1, SWB012+3 |
