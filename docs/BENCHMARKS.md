# Reference

## CASC Division Canary Suite Methodology (Include-Drift Detection)

Competitive benchmarks require clean, pre-sliced problem inputs. If the parent environment exports a global `TPTP` variable pointing to the un-sliced `TPTP-v9.2.1` library, or if the problem path allows the prover's path-resolution logic to walk up and find a global `Axioms/` folder (such as inside `/DATA/ai/`), any `%include` pulls in the *entire* generic axiom library. This include-drift inflates small problems into million-clause monsters and starves the LRS passive queue, invalidating the solved counts/timings for that run.

To rigorously identify and ensure the absence of include-drift across all divisions, we establish a **"Canary Suite"**—identifying the simplest, include-dependent problem in each division. If include-drift occurs, these canary problems experience massive processing and LRS discard footprints, but solve instantly with minimal clause footprints under clean, sliced environments.

### 1. EPS Division Canary: `HWC004-1`
*   *Include Dependency:* Includes `Axioms/HWC001-0.ax`.
*   *Clean Signature:* Solved in **< 0.1s** with **`passive=726`** (with `ac-indexing` active) and **`lrs_discarded=0`**.
*   *Contaminated Signature:* `GaveUp` / `Timeout` with **`lrs_discarded=851,760`** (or millions of passive clauses loaded at startup).

### 2. FNE Division Canary: `CSR026+3`
*   *Include Dependency:* Includes `Axioms/CSR001+0.ax` and others.
*   *Clean Signature:* `CounterSatisfiable` (Trivial saturation on 0 axioms) in **0.002s** with **`processed=6`** and **`passive=4`** (warnings thrown).
*   *Contaminated Signature:* `Theorem` (Refuted 8005 axioms) in **32.13s** with **`processed=190,828`** and **`generated=230,438`**.

### 3. FEQ Division Canary: `AGT005+1`
*   *Include Dependency:* Includes `Axioms/AGT001+0.ax`.
*   *Clean Signature:* `CounterSatisfiable` (Trivial saturation) in **0.002s** with **`processed=1`** and **`passive=0`** (warnings thrown).
*   *Contaminated Signature:* `GaveUp` in **42.56s** with **`processed=121,518`**, **`generated=1,046,647`**, and **`lrs_discarded=829,242`**.

### 4. UEQ Division Canary: `ALG212-10`
*   *Include Dependency:* Includes `Axioms/ALG001-0.ax` and others.
*   *Clean Signature:* `Satisfiable` (Trivial saturation) in **0.002s** with **`processed=4`** and **`passive=0`** (warnings thrown).
*   *Contaminated Signature:* `GaveUp` in **28.45s** with **`processed=1,905`**, **`generated=96,103`**, and **`lrs_discarded=555,465`**.

### 5. EPU & ICU Divisions (100% Environmentally Immune):
*   *Verification:* Complete scans of `epu.list` and `icu.list` confirm that **zero problems in the EPU and ICU divisions contain `%include` directives.** They are physically unaffected by the `$TPTP` environment path and are always clean and valid.

### 6. OK vs. KO Tagging Rules:
*   **`[done] OK`**: Complete runs where every active Division Canary shows the clean, minimal clause-count signature (e.g. `server02`, `server03`, `server11`, and July 11 `server97` EPS).
*   **`[done] KO`**: Runs where any Division Canary shows the contaminated clause-explosion footprint (such as July 10 `server97` and July 7 `server01`).
*   **`[done]` (Unclassified)**: Runs where the corresponding folder or completed `run.csv` inside `remote_results/` is missing or truncated locally (such as July 12 UEQ and EPU). Because we cannot verify the Canaries, they remain untagged.

Vampire 5.0.1 (Release build, commit 6b88ec04c on 2026-06-15 12:45:39 +0200)
CaDiCaL: cadical-2.1.3
Linked to Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 z3-4.8.4-9178-g3c47fd96c

[root@server02 mrs]# crates/mrs-bench/casc.sh --systems vampire --divisions fne,feq,epu,eps,ueq,icu --casc-times --jobs 8
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
[root@server02 mrs]# crates/mrs-bench/casc.sh --systems eprover --divisions fne,feq,epu,eps,ueq,icu --casc-times --jobs 8
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

Representative historical static-portfolio (`casc_*`, no ML) measurements for
`mrs`, using 8 workers and CASC times (`--casc-times`). **These numbers are
aggregated from separate per-division `crates/mrs-bench/casc.sh` runs across
several commits and machines — not a single clean full-matrix run and not a
measurement of the current `HEAD`.** Exact source per division:

| Division | Problems | mrs solved | Avg (s) | Source commit | Date |
|----------|---------:|-----------:|--------:|----------------|------|
| FNE      |      100 |         45 |    22.0 | `927353a`      | 2026-07-06 |
| FEQ      |      400 |         98 |    25.3 | `55986ce`      | 2026-07-05 |
| EPU      |      100 |         16 |     7.7 | `edb5d2d`      | 2026-07-07 |
| EPS†     |      100 |         43 |     9.6 | `7827a33`      | 2026-07-11 |
| UEQ      |      300 |         40 |       — | `d7e7501`      | 2026-07-02 |
| ICU      |      101 |          3 |   186.8 | `927353a`      | 2026-07-06 |
| **TOTAL**|   **1101** |    **245** |       — | | |


FNE and EPS solved-counts fluctuate ±1-3 across repeated runs on different
machines (observed ranges: FNE 43-45, EPS 39-43) — treat any single number
above as representative, not exact. All divisions were reported sound (0
polarity/reference violations) in every run cited above. The source commits
predate subsequent portfolio and verifier changes, so re-run the relevant
division before treating these figures as a current baseline.

† **EPS is not yet Canary-Suite-verified for the static `mrs` system.**
Every other row above traces to a run explicitly tagged `[done] OK` under
the Canary Suite methodology (see the top of this document). The `7827a33`
run cited for EPS is tagged bare `[done]` (unclassified) because its
`run.csv` is not available locally to re-check against the `HWC004-1`
canary — it has *not* been confirmed either clean or contaminated. No
plain-`mrs` EPS run anywhere in this document currently carries a
canary-confirmed `OK` tag (only one `mrs-ml` run does, coincidentally also
solving 43). Treat the EPS=43 baseline as provisional until a clean,
canary-verified `--systems mrs --divisions eps` re-run is captured.

## vs CASC-30 official results — context only, not a ranking claim

Source: https://tptp.org/CASC/30/WWWFiles/Results.html (CASC-J30, 8-core
StarExec hardware, official competition strategy schedules and time
limits).

**This comparison is not statistically valid for ranking mrs against
other systems, and should not be read as one.** Section (b) below shows
our local harness undershoots *official* CASC-30 numbers by 7-16% even
for Vampire/E — and unlike Vampire/E, we have no independent "official
mrs" run to measure mrs's own undershoot against. Placing mrs's local
numbers next to other systems' official competition numbers could make
mrs look better *or* worse than it would running at actual competition
fidelity; treat the numbers below as a rough sanity check ("are we in
the right ballpark"), not a leaderboard position.

### (a) mrs (local) vs official CASC-30 field, per division

| Division | mrs (local) | Official CASC-30 winner | Nearby official entrants (for context) |
|----------|-------------|--------------------------|------------------------------------------|
| FNE (100) | 45 | Vampire 5.0 — 91 | cvc5 — 47; ConnectPP — 43 |
| FEQ (400) | 98 | Vampire 4.9 — 379 (Vampire 5.0 — 364) | Prover9 — 94; ConnectPP — 59 |
| EPU (100) | 16 | Vampire 5.0 — 96 | Drodi-EPR — 25; SPASS-SCL — 11 |
| EPS (100) | 40 | Vampire 5.0 — 90 | SPASS-SCL — 53 |
| UEQ (300) | 39 | Vampire 5.0 — 263 | Toma — 114 |
| ICU (101) | 3  | Vampire 4.9 — 70 (Vampire 5.0 — 69) | CSE_E — 18; ConnectPP — 1 |

Reading, with the caveat above in mind: mrs's raw solved-counts sit
ahead of a few real entrants in FNE/FEQ (cvc5, ConnectPP) and near the
back of the field elsewhere, most acutely in UEQ/ICU. Consistent with
docs/AUDIT.md's assessment that the remaining gap is search/heuristic
quality, not a soundness or infrastructure problem.

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
This is exactly why section (a) above cannot be read as a ranking: if a
known-good system like Vampire undershoots by 7% on our harness, mrs's
undershoot (unmeasured) could easily be larger or smaller, and there is no
way to correct for it from local data alone.

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

Append-only log of CASC and ProoVer benchmark runs, newest first. Each entry
records the mrs commit and the exact command used.


## ProoVer 2026 PRV Corpus — 2026-08-22

Recorded local evaluation at commit `bcc9918` (`fix(proover): disable avatar in MrsAtp step checks`) using a 30-second per-proof budget and 8 workers:

### 1. Competition Mode (Full ATP Verification Ladder)

```text
./target/release/score_proover2026 \
  ./crates/mrs-bench/proover-corpus/Proover2026 \
  --competition \
  --proover ./target/release/mrs-proover \
  --time 30 \
  --workers 8

score=148 good=60 bad=39 unknown=1 false_rejection=0 unsound=0
```

The corpus contains 50 valid proofs, 10 locally sound evil mutations, and 40
ordinary evil proofs. This run verified all 50 valid proofs, gave all 10
locally sound mutations a permitted scoring verdict, rejected 39 ordinary evil
proofs, and left `PRV067+1` as the one neutral `Unknown`. This is a recorded
local reproduction of a 148/150 result, not an official CASC-J13 score or a
claim of cross-machine stability.

### 2. Strict Mode (Independent `mrs-proof-kernel` Only)

```text
./target/release/score_proover2026 \
  ./crates/mrs-bench/proover-corpus/Proover2026 \
  --kernel \
  --proover ./target/release/mrs-proover \
  --time 30 \
  --workers 8

score=61 good=16 bad=59 unknown=25 false_rejection=26 unsound=0
```

The kernel-only run bypasses all external and in-process ATPs. It structurally
verified 16 proofs, left 25 inconclusive, and falsely rejected 26 valid proofs
in this configuration (`false_rejection=26`). It produced zero unsound passes,
but the result is structural coverage data, not perfect verification.

---

## ProoVer 2026 PRV Corpus — 2026-08-04

Commit `0e10c0d` (`fix: harden ProoVer provenance checks`), 100-problem
competition-mode reproduction with 10-second per-proof budget and 8 workers:

```text
./target/release/score_proover2026 \
  ./crates/mrs-bench/proover-corpus/Proover2026 \
  --competition \
  --proover ./target/release/mrs-proover \
  --time 10 \
  --workers 8

score=148 good=60 bad=39 unknown=1 false_rejection=0 unsound=0
```

The result is 50 valid proofs verified, 39 ordinary evil proofs rejected, 10
locally sound evil mutations accepted under the corpus scoring rule, and one
ordinary evil proof left `Unknown`.


[done]
[www@server99 mrs]$ crates/mrs-bench/casc.sh --edition casc-j13 --divisions fne,feq,ueq --systems vampire --jobs 2 --casc-times
CASC-J13 Results — 2026-08-14 06:18  (800 problems × 1 systems)
===============================================================

Division  Problems    vampire
                      Solved  Avg (s)
------------------  --------------------
FNE            100        90   22.078
FEQ            300       252    4.755
UEQ            400       332   13.225
------------------  --------------------
TOTAL          800       674   11.240

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
50038 Aug 13 23:42 /DATA/ai/mrs/crates/mrs-bench/results/casc-j13/20260813_193044/run.csv

[done]
[PPROD:user@server97:/DATA/ai/user/mrs]$ numactl --physcpubind=0,2,4,6,8,10,12,14 --membind=0 crates/mrs-bench/casc.sh --edition casc-j13 --divisions fne,feq,ueq --systems vampire --jobs 1 --casc-times
CASC-J13 Results — 2026-08-13 16:57  (800 problems × 1 systems)
===============================================================

Division  Problems    vampire
                      Solved  Avg (s)
------------------  --------------------
FNE            100        90   21.803
FEQ            300       251    3.980
UEQ            400       333   12.723
------------------  --------------------
TOTAL          800       674   10.680

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
50033 Aug 13 17:17 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-j13/20260813_090132/run.csv

[done]
[PPROD:user@server97:/DATA/ai/user/mrs]$ crates/mrs-bench/casc.sh --edition casc-j13 --divisions fne,feq,ueq --systems vampire --jobs 2 --casc-times
CASC-J13 Results — 2026-08-13 06:54  (800 problems × 1 systems)
===============================================================

Division  Problems    vampire
                      Solved  Avg (s)
------------------  --------------------
FNE            100        90   21.754
FEQ            300       253    4.940
UEQ            400       331   13.082
------------------  --------------------
TOTAL          800       674   11.184

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
50027 Aug 12 22:45 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-j13/20260812_183400/run.csv

===================================================================
===================================================================

commit 9738467d6d1dc3190f663bea94f4f628d7f1d7a9 (HEAD -> feat/destructive-equality-resolution

[ongoing]
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh   --edition casc-30  --systems mrs --divisions feq,fne,epu,eps  --casc-times --jobs 1 --output crates/mrs-bench/results/casc-30-W8J1-$(date +%Y%m%d)
/DATA/ai/mrs/crates/mrs-bench/results/casc-30

[done]
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh   --edition casc-j13  --systems mrs   --divisions feq,fne  --casc-times   --jobs 1   --output crates/mrs-bench/results/casc-j13-W8J1-$(date +%Y%m%d)
Running `target/debug/bench_report /DATA/ai/mrs/crates/mrs-bench/results/casc-j13-W8J1-20260901/run.csv`
CASC-J13 Results — 2026-09-02 07:41  (400 problems × 1 systems)
===============================================================

Division  Problems    mrs
                 Solved  Avg (s)
------------------  --------------------
FEQ            300        68   17.304
FNE            100        32   34.309
------------------  --------------------
TOTAL          400       100   22.745

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.


commit 658b5c7f9a02f34bc709608f06d81877d033492d (HEAD -> feat/kernel-equational-definitions

[done]
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh   --edition casc-j13   --systems mrs   --divisions ueq  --casc-times   --jobs 1   --output crates/mrs-bench/results/casc-j13-W8J1-$(date +%Y%m%d)
     Running `target/debug/bench_report /DATA/ai/mrs/crates/mrs-bench/results/casc-j13-W8J1-20260831/run.csv`
CASC-J13 Results — 2026-09-02 07:44  (400 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
UEQ            400        65   15.657
------------------  --------------------
TOTAL          400        65   15.657

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done]
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh   --edition casc-30   --systems mrs   --divisions ueq  --casc-times   --jobs 1   --output crates/mrs-bench/results/casc-30-W8J1-$(date +%Y%m%d)
Running `target/debug/bench_report /DATA/ai/mrs/crates/mrs-bench/results/casc-30-W8J1-20260830/run.csv`
CASC-30 Results — 2026-09-02 08:11  (300 problems × 1 systems)
==============================================================

Division  Problems    mrs
                 Solved  Avg (s)
------------------  --------------------
UEQ            300        52   24.345
------------------  --------------------
TOTAL          300        52   24.345

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

commit f13912c763c8c22309fbc7bc2fc6126cad1eb55f

[ongoing] 994/13011
export RUST_MIN_STACK=67108864
[root@server01 mrs]# ./crates/mrs-bench/run_codex_sweep.sh "$TPTP" codex_cat_filtered_sweep_f13912c763_01-03.db 300 1

[ongoing] 1039/13011
export RUST_MIN_STACK=67108864
[root@server02 mrs]# ./crates/mrs-bench/run_codex_sweep.sh "$TPTP" codex_cat_filtered_sweep_f13912c763_04-06.db 300 1

[ongoing] 1035/13011
export RUST_MIN_STACK=67108864
[root@server03 mrs]# ./crates/mrs-bench/run_codex_sweep.sh "$TPTP" codex_cat_filtered_sweep_f13912c763_07-09.db 300 1

[ongoing] 1295/13011
export RUST_MIN_STACK=67108864
[root@server04 mrs]# ./crates/mrs-bench/run_codex_sweep.sh "$TPTP" codex_cat_filtered_sweep_f13912c763_10-12.db 300 1

[ongoing] 2078/13011
export RUST_MIN_STACK=67108864
[PPROD:user@server97:/DATA/ai/user/mrs]$ ./crates/mrs-bench/run_codex_sweep.sh "$TPTP" codex_cat_filtered_sweep_f13912c763_13-15.db 300 1


[done]
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh   --edition casc-30   --systems mrs   --divisions fne,eps,ueq,epu,icu,feq  --casc-times   --jobs 1   --output crates/mrs-bench/results/casc-30-W8J1-$(date +%Y%m%d)
Running `target/debug/bench_report /DATA/ai/mrs/crates/mrs-bench/results/casc-30-W8J1-20260828/run.csv`
CASC-30 Results — 2026-09-02 08:14  (1101 problems × 1 systems)
===============================================================

Division  Problems    mrs
                 Solved  Avg (s)
------------------  --------------------
FNE            100        44   36.363
EPS            100        43   10.595
UEQ            300        74   58.646
EPU            100        16    7.656
ICU            101         2  150.291
FEQ            400        90   21.873
------------------  --------------------
TOTAL         1101       269   32.665

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done]
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh   --edition casc-j13   --systems mrs   --divisions fne,feq,ueq   --casc-times   --jobs 1   --output crates/mrs-bench/results/casc-j13-W8J1-$(date +%Y%m%d)
Running `target/debug/bench_report /DATA/ai/mrs/crates/mrs-bench/results/casc-j13-W8J1-20260826/run.csv`
CASC-J13 Results — 2026-09-02 08:15  (800 problems × 1 systems)
===============================================================

Division  Problems    mrs
                 Solved  Avg (s)
------------------  --------------------
FNE            100        33   35.573
FEQ            300        64   17.603
UEQ            400        70   37.753
------------------  --------------------
TOTAL          800       167   29.600

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

commit 8988837437da7dbf05e867ae9a26bb5d1eb2e1e3

[done] check also why not always 4 mrs process running
[www@server99 mrs]$ cargo run --release -p mrs-codex -- $TPTP/Problems  --db codex_casc_remaining.db   --system mrs-0.2.3   --timeout 300   --jobs 4 --cmd "env MRS_WORKERS=8 ./crates/mrs-bench/systems/mrs/invoke.sh {file} {timeout}" > codex_casc_remaining_89888374.out 2> codex_casc_remaining_89888374.err

commit 810f1ff7a8da03dc243667a4d6fc68e4bf6999ae
[ongoing] ISSUE idle process
[www@server99 mrs]$ export TPTP=/DATA/ai/TPTP-v9.3.0/
[www@server99 mrs]$ cargo run --release -p mrs-codex -- $TPTP/Problems  --db codex_casc_remaining.db   --system mrs-0.2.3   --timeout 300   --jobs 2   --cmd "env MRS_WORKERS=8 ./crates/mrs-bench/systems/mrs/invoke.sh {file} {timeout}" > codex_casc_remaining.out 2> codex_casc_remaining.err


commit 613e2ffa3c01e99db6f3ca2fe278e8fd10711b3b

[abort] ISSUE idle process
[www@server99 mrs]$ export TPTP=/DATA/ai/TPTP-v9.3.0/
[www@server99 mrs]$ cargo run --release -p mrs-codex -- $TPTP/Problems  --db codex_casc_remaining.db   --system mrs-0.2.3   --timeout 300   --jobs 2   --cmd "env MRS_WORKERS=8 ./crates/mrs-bench/systems/mrs/invoke.sh {file} {timeout}" > codex_casc_remaining.out 2> codex_casc_remaining.err

[ended] ISSUE idle process
[www@server99 mrs]$ cargo run --release -p mrs-codex -- $TPTP/Problems  --db codex_casc_remaining.db   --system mrs-0.2.3   --timeout 300   --jobs 4   --cmd "./target/release/mrs --workers 8 --time {timeout} {file}" > codex_casc_remaining.out 2> codex_casc_remaining.err

commit 1f352f04c91925b76c36ba668a9ad24bed2d0885


[done] without TPTP
[root@server02 mrs]# MRS_WORKERS=8 numactl --interleave=all crates/mrs-bench/casc.sh --edition casc-j13 --divisions feq --systems mrs-starexec --jobs 1 --casc-times
CASC-J13 Results — 2026-08-13 17:04  (300 problems × 1 systems)
===============================================================

Division  Problems    mrs-starexec
                      Solved  Avg (s)
------------------  --------------------
FEQ            300        60   14.783
------------------  --------------------
TOTAL          300        60   14.783

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
61104 Aug 12 21:28 /mnt/sda1/mrs/crates/mrs-bench/results/casc-j13/20260812_091556/run.csv

[done] unset TPTP
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 numactl --physcpubind=0,2,4,6,8,10,12,14 --membind=0 crates/mrs-bench/casc.sh --edition casc-j13 --divisions fne --systems mrs-starexec --jobs 1 --casc-times
CASC-J13 Results — 2026-08-12 15:56  (100 problems × 1 systems)
===============================================================

Division  Problems    mrs-starexec
                      Solved  Avg (s)
------------------  --------------------
FNE            100        26   14.255
--------------------------------------                                                                                                                                                                         TOTAL          100        26   14.255

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
25465 Aug 12 12:52 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-j13/20260812_091429/run.csv

[done]
[root@server03 mrs]# MRS_WORKERS=8 numactl --interleave=all crates/mrs-bench/casc.sh --edition casc-30 --divisions eps --systems mrs-starexec --jobs 1 --casc-times
CASC-30 Results — 2026-08-12 16:10  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs-starexec
                      Solved  Avg (s)
------------------  --------------------
EPS            100        42    8.978
------------------  --------------------
TOTAL          100        42    8.978

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done]
[root@server03 mrs]# MRS_WORKERS=8 numactl --interleave=all crates/mrs-bench/casc.sh --edition casc-j13 --divisions fne --systems mrs-starexec --jobs 1 --casc-times
CASC-J13 Results — 2026-08-12 06:47  (100 problems × 1 systems)
===============================================================

Division  Problems    mrs-starexec
                      Solved  Avg (s)
------------------  --------------------
FNE            100        26   15.614
------------------  --------------------
TOTAL          100        26   15.614

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
25085 Aug 11 23:16 /mnt/sdd/mrs/crates/mrs-bench/results/casc-j13/20260811_193339/run.csv

[done] with TPTP
[root@server02 mrs]# MRS_WORKERS=8 numactl --interleave=all crates/mrs-bench/casc.sh --edition casc-j13 --divisions feq --systems mrs-starexec --jobs 1 --casc-times
CASC-J13 Results — 2026-08-12 07:15  (300 problems × 1 systems)
===============================================================

Division  Problems    mrs-starexec
                      Solved  Avg (s)
------------------  --------------------
FEQ            300        62   16.352
------------------  --------------------
TOTAL          300        62   16.352

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done] unset TPTP
[root@server04 mrs]# MRS_WORKERS=8 numactl --interleave=all crates/mrs-bench/casc.sh --edition casc-j13 --divisions ueq --systems mrs-starexec --jobs 1 --casc-times
CASC-J13 Results — 2026-08-12 16:07  (400 problems × 1 systems)
===============================================================                                                                                                                                                  
Division  Problems    mrs-starexec
                      Solved  Avg (s)
------------------  --------------------
UEQ            400        60   28.990
------------------  --------------------
TOTAL          400        60   28.990

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[ongoing]
[www@server99 mrs]$ ./cpuset_sweep > cpuset_sweep_nocanary.log 2> cpuset_sweep_nocanary.err

[done] canary
[www@server99 mrs]$ ./cpuset_sweep > cpuset_sweep.log 2> cpuset_sweep.err

[done]
[PPROD:user@server97:/DATA/ai/user/mrs]$ numactl --physcpubind=0,2,4,6,8,10,12,14 --membind=0 ./target/release/mrs-codex $TPTP/Problems --db codex_cat1.db --system mrs-0.2.3 --timeout 300 --cmd "./target/release/mrs {file}" --verify-mode competition

commit 14b56cff8f2541f15ebecf0965766065bfa390ee v0.2.3

[done]
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --edition casc-30 --divisions ueq --systems mrs-starexec --jobs 2 --casc-times
CASC-30 Results — 2026-08-11 12:23  (300 problems × 1 systems)
==============================================================

Division  Problems    mrs-starexec
                      Solved  Avg (s)
------------------  --------------------
UEQ            300        76   51.556
------------------  --------------------
TOTAL          300        76   51.556

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
83629 Aug 10 20:49 /DATA/ai/mrs/crates/mrs-bench/results/casc-30/20260810_125113/run.csv

[done]
target-cpu=x86-64
[root@server99 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --edition casc-30 --divisions eps --systems mrs-starexec --jobs 2 --casc-times
CASC-30 Results — 2026-08-10 10:48  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs-starexec
                      Solved  Avg (s)
------------------  --------------------
EPS            100        43    9.392
------------------  --------------------
TOTAL          100        43    9.392

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
25265 Aug 10 12:40 /DATA/ai/mrs/crates/mrs-bench/results/casc-30/20260810_113951/run.csv


commit 7e69de89a2dd29adad9972f86e0b5c93b6745636 

[done]
[PPROD:user@server97:/DATA/ai/user/mrs]$ crates/mrs-bench/casc.sh --edition casc-j13 --divisions ueq --systems mrs-starexec --jobs 4 --casc-times
target-cpu=x86-64
CASC-J13 Results — 2026-08-10 14:53  (400 problems × 1 systems)
===============================================================

Division  Problems    mrs-starexec
                      Solved  Avg (s)
------------------  --------------------
UEQ            400        65   41.361
------------------  --------------------
TOTAL          400        65   41.361

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done]
target-cpu=x86-64
[PPROD:user@server97:/DATA/ai/user/mrs]$ crates/mrs-bench/casc.sh --edition casc-j13 --divisions fne --systems mrs-starexec --jobs 4 --casc-times
CASC-J13 Results — 2026-08-10 10:19  (100 problems × 1 systems)
===============================================================

Division  Problems    mrs-starexec
                      Solved  Avg (s)
------------------  --------------------
FNE            100        25   16.880
------------------  --------------------
TOTAL          100        25   16.880

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done]
target-cpu=native
[PPROD:user@server97:/DATA/ai/user/mrs]$ crates/mrs-bench/casc.sh --edition casc-j13 --divisions fne --systems mrs-starexec --jobs 4 --casc-times
CASC-J13 Results — 2026-08-10 09:11  (100 problems × 1 systems)
===============================================================

Division  Problems    mrs-starexec
                      Solved  Avg (s)
------------------  --------------------
FNE            100        27   22.074
------------------  --------------------
TOTAL          100        27   22.074

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
25669 Aug 10 10:48 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-j13/20260810_095259/run.csv

[done] no telemetry wrong binary in crates/mrs-bench/systems/mrs, probably the previous one
target-cpu=native
[PPROD:user@server97:/DATA/ai/user/mrs]$ crates/mrs-bench/casc.sh --edition casc-j13 --divisions fne --systems mrs-starexec --jobs 4 --casc-times
CASC-J13 Results — 2026-08-10 07:50  (100 problems × 1 systems)
===============================================================

Division  Problems    mrs-starexec
                      Solved  Avg (s)
------------------  --------------------
FNE            100        22    9.993
------------------  --------------------
TOTAL          100        22    9.993

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
17936 Aug 10 09:47 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-j13/20260810_084757/run.csv

[done]
target-cpu=x86-64
[root@server99 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --edition casc-30 --divisions eps --systems mrs-starexec --jobs 2 --casc-times
CASC-30 Results — 2026-08-10 09:32  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs-starexec
                      Solved  Avg (s)
------------------  --------------------
EPS            100        43    9.570
------------------  --------------------
TOTAL          100        43    9.570

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
25270 Aug 10 11:03 /DATA/ai/mrs/crates/mrs-bench/results/casc-30/20260810_100230/run.csv

[done]
target-cpu=native
[root@server99 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --edition casc-30 --divisions eps --systems mrs-starexec --jobs 2 --casc-times
CASC-30 Results — 2026-08-10 07:56  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs-starexec
                      Solved  Avg (s)
------------------  --------------------
EPS            100        43   10.520
------------------  --------------------
TOTAL          100        43   10.520

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
25052 Aug 10 09:40 /DATA/ai/mrs/crates/mrs-bench/results/casc-30/20260810_083954/run.csv

[[done]
target-cpu=x86-64
[root@server03 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --edition casc-30 --divisions eps --systems mrs-starexec --jobs 1 --casc-times
CASC-30 Results — 2026-08-11 17:09  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs-starexec
                      Solved  Avg (s)
------------------  --------------------
EPS            100        43   11.044
------------------  --------------------
TOTAL          100        43   11.044

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[[done] no telemetry wrong binary in crates/mrs-bench/systems/mrs, probably the previous one]
target-cpu=x86-64
[root@server03 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --edition casc-30 --divisions eps --systems mrs-starexec --jobs 1 --casc-times
CASC-30 Results — 2026-08-11 17:10  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs-starexec
                      Solved  Avg (s)
------------------  --------------------
EPS            100        42    8.663
------------------  --------------------
TOTAL          100        42    8.663

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done]
target-cpu=native
[root@server04 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --edition casc-30 --divisions eps --systems mrs-starexec --jobs 1 --casc-times
CASC-30 Results — 2026-08-10 08:04  (100 problems × 1 systems)
==============================================================
Division  Problems    mrs-starexec
                      Solved  Avg (s)
------------------  --------------------
EPS            100        42    8.855
------------------  --------------------
TOTAL          100        42    8.855

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
24854 Aug 10 10:00 /mnt/sdd1/mrs/crates/mrs-bench/results/casc-30/20260810_075819/run.csv

commit 15957cf0b25eb4c4c0a6811399552aeeec689175

[done] server99
  MRS_WORKERS=1 \
  crates/mrs-bench/casc.sh \
    --edition casc-j13 \
    --systems mrs \
    --divisions fne,feq,ueq \
    --casc-times \
    --jobs 16 \
    --output "crates/mrs-bench/results/casc-j13-workers-${workers}-$(date +%Y%m%d-%H%M%S)"
    Output:      crates/mrs-bench/results/casc-j13-workers--20260806-145513

[done] server99
  MRS_WORKERS=2 \
  crates/mrs-bench/casc.sh \
    --edition casc-j13 \
    --systems mrs \
    --divisions fne,feq,ueq \
    --casc-times \
    --jobs 8 \
    --output "crates/mrs-bench/results/casc-j13-workers-${workers}-$(date +%Y%m%d-%H%M%S)"
    crates/mrs-bench/results/casc-j13-workers--20260806-182727/run.csv

[done] server99
  MRS_WORKERS=4 \
  crates/mrs-bench/casc.sh \
    --edition casc-j13 \
    --systems mrs \
    --divisions fne,feq,ueq \
    --casc-times \
    --jobs 4 \
    --output "crates/mrs-bench/results/casc-j13-workers-${workers}-$(date +%Y%m%d-%H%M%S)"
   132857 Aug  7 18:54 crates/mrs-bench/results/casc-j13-workers--20260807-103146/run.csv

  [done] server99
  MRS_WORKERS=8 \
  crates/mrs-bench/casc.sh \
    --edition casc-j13 \
    --systems mrs \
    --divisions fne,feq,ueq \
    --casc-times \
    --jobs 2 \
    --output "crates/mrs-bench/results/casc-j13-workers-${workers}-$(date +%Y%m%d-%H%M%S)"
  crates/mrs-bench/results/casc-j13-workers--20260807-195229
  124726 Aug  8 12:28 crates/mrs-bench/results/casc-j13-workers--20260807-195229/run.csv

commit 18e2cbec7a6899edd811839f84e5f4b20e569759

[ongoing]
[root@server01 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --edition casc-30 --divisions epu,icu,feq --systems mrs-starexec --jobs 1 --casc-times

[ongoing]
[root@server02 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --edition casc-30 --divisions ueq --systems mrs-starexec --jobs 1 --casc-times

[done]
[root@server03 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --edition casc-30 --divisions fne --systems mrs-starexec --jobs 1 --casc-times
CASC-30 Results — 2026-08-06 05:46  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs-starexec
                      Solved  Avg (s)
------------------  --------------------
FNE            100        36   10.101
------------------  --------------------
TOTAL          100        36   10.101

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
16400 Aug  6 00:06 /mnt/sdd/mrs/crates/mrs-bench/results/casc-30/20260805_194831/run.csv

[done]
[root@server04 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --edition casc-30 --divisions eps --systems mrs-starexec --jobs 1 --casc-times
CASC-30 Results — 2026-08-06 05:40  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs-starexec
                      Solved  Avg (s)
------------------  --------------------
EPS            100        42    8.941
------------------  --------------------
TOTAL          100        42    8.941

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
18397 Aug  5 21:47 /mnt/sdd1/mrs/crates/mrs-bench/results/casc-30/20260805_194456/run.csv

commit 1454a85ee6db2736dc15359a70b7d0aba772b606 

[done]
[www@server99 mrs]$ crates/mrs-bench/casc.sh   --edition casc-j13   --systems mrs   --divisions fne,feq,ueq   --casc-times   --jobs 2   --output crates/mrs-bench/results/casc-j13-baseline-$(date +%Y%m%d)
crates/mrs-bench/results/casc-j13-baseline-20260805
CASC-J13 Results — 2026-08-06 06:51  (800 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        29   19.161
FEQ            300        61   16.006
UEQ            400        78   31.863
------------------  --------------------
TOTAL          800       168   23.913

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
123972 Aug  6 08:49 /DATA/ai/mrs/crates/mrs-bench/results/casc-j13-baseline-20260805/run.csv

[ongoing]
[PPROD:user@server97:/DATA/ai/user/mrs]$  export RUST_MIN_STACK=67108864 
for workers in 1 2 4 8; do
  MRS_WORKERS="$workers" \
  crates/mrs-bench/casc.sh \
    --edition casc-j13 \
    --systems mrs \
    --divisions fne,feq,ueq \
    --casc-times \
    --jobs 1 \
    --output "crates/mrs-bench/results/casc-j13-workers-${workers}-$(date +%Y%m%d-%H%M%S)"

[done]
[www@server99 mrs]$ export RUST_MIN_STACK=67108864
[www@server99 ~]$ crates/mrs-bench/run_strategy_sweep.sh   --edition casc-j13   --divisions fne,feq,ueq   --casc-times   --jobs 32   --output crates/mrs-bench/results/casc-j13-sweep-$(date +%Y%m%d)


commit 0b849cb172171e94bf7051eabea1241d095ae4ae (HEAD -> feat/casc-j13-reproduction

[done]
[www@server99 mrs]$  crates/mrs-bench/casc.sh --edition casc-j13 --divisions fne,feq,ueq --systems mrs-starexec --jobs 2 --casc-times
CASC-J13 Results — 2026-08-04 06:52  (800 problems × 1 systems)
===============================================================

Division  Problems    mrs-starexec
                      Solved  Avg (s)
------------------  --------------------
FNE            100        26   17.782
FEQ            300        59   12.573
UEQ            400        78   32.218
------------------  --------------------
TOTAL          800       163   22.804

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected
130601 Aug  4 08:28 /DATA/ai/mrs/crates/mrs-bench/results/casc-j13/20260803_160925/run.csv

[done]
[www@server99 mrs]$  crates/mrs-bench/casc.sh --edition casc-j13 --divisions fne,feq,ueq --systems mrs-starexec --jobs 8 --casc-times
CASC-J13 Results — 2026-08-03 14:05  (800 problems × 1 systems)
===============================================================
Division  Problems    mrs-starexec
                      Solved  Avg (s)
------------------  --------------------
FNE            100        24   13.472
FEQ            300        55   19.788
UEQ            400        67   47.951
------------------  --------------------
TOTAL          800       146   31.674

DISAGREEMENTS — none detected.
POLARITY VIOLATIONS — none detected.
REFERENCE VIOLATIONS — none detected.
137306 Aug  3 16:03 /DATA/ai/mrs/crates/mrs-bench/results/casc-j13/20260803_114647/run.csv

commit 67f31a71d316e0d898fa60f4d5969bbfe6a8cc7f (HEAD -> feat/casc-j13-reproduction-200

[done]
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --edition casc-30 --divisions fne,eps,ueq,epu,icu,feq --systems mrs-starexec --jobs 2 --casc-times
CASC-30 Results — 2026-08-04 17:44  (1101 problems × 1 systems)
===============================================================

Division  Problems    mrs-starexec
                      Solved  Avg (s)
------------------  --------------------
FNE            100        35   13.805
EPS            100        43   10.421
UEQ            300        87   40.304
EPU            100        16    5.450
ICU            101         2  208.479
FEQ            400        91   25.743
------------------  --------------------
TOTAL         1101       274   26.586

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
191887 Aug  4 19:42 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30/20260803_141834/run.csv

[done]
[PPROD:user@server97:/DATA/ai/user/mrs]$ crates/mrs-bench/casc.sh --edition casc-30 --divisions eps --systems mrs-starexec --jobs 8 --casc-times
CASC-30 Results — 2026-08-03 12:10  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs-starexec
                      Solved  Avg (s)
------------------  --------------------
EPS            100        38    9.440
------------------  --------------------
TOTAL          100        38    9.440

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
18090 Aug  3 14:05 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30/20260803_134843/run.csv

[done]
[PPROD:user@server97:/DATA/ai/user/mrs]$ crates/mrs-bench/casc.sh --edition casc-30 --divisions fne --systems mrs-starexec --jobs 8 --casc-times
CASC-30 Results — 2026-08-03 11:45  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs-starexec
                      Solved  Avg (s)
------------------  --------------------
FNE            100        33   27.290
------------------  --------------------
TOTAL          100        33   27.290

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
16921 Aug  3 13:29 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30/20260803_125324/run.csv

commit 3e95f1a11a446d3d53265831339e4fefbf49d39a (HEAD -> fix/fof-formula-memory-bloat

[ongoing]
[PPROD:user@server11:/DATA/DISK1/BENCH/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions eps,fne,epu,ueq,icu,feq --casc-times --jobs 1
eps 43 fne 43 epu 16 ueq partial 47/111
20260717_183541/run.csv

[done]
[root@server02 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions icu --casc-times --jobs 1
CASC-30 Results — 2026-07-18 06:32  (101 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
ICU            101         2  145.091
------------------  --------------------
TOTAL          101         2  145.091

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
16827 Jul 18 03:32 /mnt/sda1/mrs/crates/mrs-bench/results/casc-30/20260717_141614/run.csv

[ongoing]
[root@server03 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions feq --casc-times --jobs 1

[done]
[root@server04 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions ueq --casc-times --jobs 1
CASC-30 Results — 2026-07-18 06:34  (300 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
UEQ            300        88   39.935
------------------  --------------------
TOTAL          300        88   39.935

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
49186 Jul 18 05:29 /mnt/sdd1/mrs/crates/mrs-bench/results/casc-30/20260717_141308/run.csv

[done]
[root@server04 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions epu --casc-times --jobs 1
CASC-30 Results — 2026-07-17 12:10  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPU            100        16    8.626
------------------  --------------------
TOTAL          100        16    8.626

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
14771 Jul 17 12:36 /mnt/sdd1/mrs/crates/mrs-bench/results/casc-30/20260717_094036/run.csv

[done]
[root@serve03 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions eps --casc-times --jobs 1
CASC-30 Results — 2026-07-17 12:15  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPS            100        42    8.861
------------------  --------------------
TOTAL          100        42    8.861

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
17304 Jul 17 11:33 /mnt/sdd/mrs/crates/mrs-bench/results/casc-30/20260717_092842/run.csv

[done]
[root@server02 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions fne --casc-times --jobs 1
CASC-30 Results — 2026-07-17 12:14  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        37   18.150
------------------  --------------------
TOTAL          100        37   18.150

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
15887 Jul 17 13:36 /mnt/sda1/mrs/crates/mrs-bench/results/casc-30/20260717_091502/run.csv

commit 21774f2302c5a2afd9fac9384bc7c7c9b2f2971e version used for the competition

[partial] 920/1101  feq not completed 
[PPROD:user@server11:/DATA/DISK1/BENCH/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions eps,fne,epu,ueq,icu,feq --casc-times --jobs 2
eps 43 fne 40 epu 16 ueq 86 icu 1 feq partial 66
147466 Jul 17 18:24 /DATA/DISK1/BENCH/mrs/crates/mrs-bench/results/casc-30/20260716_175345/run.csv

[done]
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions eps,fne,epu,ueq,icu,feq --casc-times --jobs 2
CASC-30 Results — 2026-07-16 17:02  (1101 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPS            100        43   10.872
FNE            100        36   18.662
EPU            100        16    7.726
UEQ            300        85   40.703
ICU            101         2  152.168
FEQ            400        91   24.033
------------------  --------------------
TOTAL         1101       273   26.425

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

commit cba4bd0c8da7893669ace31328d01cf7ba1fafdc (HEAD -> main

[done]
[root@server02 mrs]# crates/mrs-bench/casc.sh --systems mrs --divisions fne --casc-times --jobs 1
CASC-30 Results — 2026-07-13 14:30  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        43   11.592
------------------  --------------------
TOTAL          100        43   11.592

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
15428 Jul 13 12:45 /mnt/sda1/mrs/crates/mrs-bench/results/casc-30/20260713_085051/run.csv

[done]
[root@server03 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions eps --casc-times --jobs 1
CASC-30 Results — 2026-07-13 14:33  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPS            100        42    8.937
------------------  --------------------
TOTAL          100        42    8.937

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
17306 Jul 13 10:50 /mnt/sdd/mrs/crates/mrs-bench/results/casc-30/20260713_084549/run.csv

[done]
[root@server04 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions epu --casc-times --jobs 1
CASC-30 Results — 2026-07-13 14:31  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPU            100        16    8.717
------------------  --------------------
TOTAL          100        16    8.717

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
14769 Jul 13 11:34 /mnt/sdd1/mrs/crates/mrs-bench/results/casc-30/20260713_083741/run.csv

[done]
[PPROD:user@server11:/DATA/DISK1/BENCH/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions fne --casc-times --jobs 2
[PPROD:user@server11:/DATA/DISK1/BENCH/mrs]$ cat /DATA/DISK1/BENCH/mrs/crates/mrs-bench/results/casc-30/20260713_083420/run.csv | grep ko | wc -l
0
[PPROD:user@server11:/DATA/DISK1/BENCH/mrs]$ cat /DATA/DISK1/BENCH/mrs/crates/mrs-bench/results/casc-30/20260713_083420/run.csv | grep ok | wc -l
44
14789 Jul 13 10:33 /DATA/DISK1/BENCH/mrs/crates/mrs-bench/results/casc-30/20260713_083420/run.csv

[done]
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions eps --casc-times --jobs 2
CASC-30 Results — 2026-07-13 14:03  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPS            100        43   10.826
------------------  --------------------
TOTAL          100        43   10.826

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
17188 Jul 13 09:34 /DATA/ai/fr22192/mrs/crates/mrs-bench/results/casc-30/20260713_083231/run.csv

commit 05fe51faadfd5d5f2f540f3c7c9ac5c58194abea (HEAD -> fix/cross-strategy-pool-flooding

[done]
[PPROD:user@server11:/DATA/DISK1/BENCH/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions fne --casc-times --jobs 2
[PPROD:user@server11:/DATA/DISK1/BENCH/mrs]$ cat /DATA/DISK1/BENCH/mrs/crates/mrs-bench/results/casc-30/20260712_180122/run.csv | grep ok | wc -l
45
[PPROD:user@server11:/DATA/DISK1/BENCH/mrs]$ cat /DATA/DISK1/BENCH/mrs/crates/mrs-bench/results/casc-30/20260712_180122/run.csv | grep ko | wc -l
0

[done]
[root@server02 mrs]# crates/mrs-bench/casc.sh --systems mrs,mrs-ml --divisions fne --casc-times --jobs 1
CASC-30 Results — 2026-07-13 06:16  (100 problems × 2 systems)
==============================================================

Division  Problems    mrs                   mrs-ml
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
FNE            100        43   11.579          42   13.900
------------------  --------------------  --------------------
TOTAL          100        43   11.579          42   13.900

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
31023 Jul 12 23:50 /mnt/sda1/mrs/crates/mrs-bench/results/casc-30/20260712_155242/run.csv

[done]
[root@server04 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs,mrs-ml --divisions epu --casc-times --jobs 1
CASC-30 Results — 2026-07-13 06:16  (100 problems × 2 systems)
==============================================================

Division  Problems    mrs                   mrs-ml
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
EPU            100        16    8.169          16    8.207
------------------  --------------------  --------------------
TOTAL          100        16    8.169          16    8.207

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
29460 Jul 12 21:51 /mnt/sdd1/mrs/crates/mrs-bench/results/casc-30/20260712_155613/run.csv

[done]
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions ueq --casc-times --jobs 2
CASC-30 Results — 2026-07-13 06:02  (300 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
UEQ            300        88   43.590
------------------  --------------------
TOTAL          300        88   43.590

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
48787 Jul 12 23:29 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30/20260712_154530/run.csv

[done]
[root@server03 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs,mrs-ml --divisions eps --casc-times --jobs 1
CASC-30 Results — 2026-07-13 06:18  (100 problems × 2 systems)
==============================================================

Division  Problems    mrs                   mrs-ml
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
EPS            100        43   11.106          43   11.135
------------------  --------------------  --------------------
TOTAL          100        43   11.106          43   11.135

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
34819 Jul 12 20:02 /mnt/sdd/mrs/crates/mrs-bench/results/casc-30/20260712_155018/run.csv

commit 7827a3368586ac06277eac7f5fb295030e8d9e2e (HEAD -> perf/jemalloc-remote-bench

[done]
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions epu --casc-times --jobs 2
CASC-30 Results — 2026-07-12 13:41  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPU            100        16    6.576
------------------  --------------------
TOTAL          100        16    6.576

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
14931 Jul 12 10:18 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30/20260712_084914/run.csv

[done]
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions ueq --casc-times --jobs 2
CASC-30 Results — 2026-07-12 06:45  (300 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
UEQ            300        36   33.765
------------------  --------------------
TOTAL          300        36   33.765

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
40314 Jul 12 00:16 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30/20260711_150100/run.csv

[ongoing]
[PPROD:user@server11:/DATA/DISK1/BENCH/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions ueq --casc-times --jobs 2

[done]
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions eps  --casc-times --jobs 2
CASC-30 Results — 2026-07-11 13:00  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPS            100        43    9.621
------------------  --------------------
TOTAL          100        43    9.621

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
17160 Jul 11 14:59 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30/20260711_135717/run.csv

[done]
[root@server04 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs,mrs-ml --divisions epu --casc-times --jobs 1
CASC-30 Results — 2026-07-12 06:51  (100 problems × 2 systems)
==============================================================

Division  Problems    mrs                   mrs-ml
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
EPU            100        16    7.804          16    7.759
------------------  --------------------  --------------------
TOTAL          100        16    7.804          16    7.759

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
/mnt/sdd1/mrs/crates/mrs-bench/results/casc-30/20260711_135450/run.csv

[done]
[root@server03 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs,mrs-ml --divisions eps  --casc-times --jobs 1
CASC-30 Results — 2026-07-12 06:53  (100 problems × 2 systems)
==============================================================

Division  Problems    mrs                   mrs-ml
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
EPS            100        43   10.444          42   10.710
------------------  --------------------  --------------------
TOTAL          100        43   10.444          42   10.710

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
34666 Jul 11 18:43 /mnt/sdd/mrs/crates/mrs-bench/results/casc-30/20260711_143012/run.csv

[done]
[root@server02 mrs]# crates/mrs-bench/casc.sh --systems mrs,mrs-ml --divisions fne --casc-times --jobs 1
CASC-30 Results — 2026-07-12 13:48  (100 problems × 2 systems)
==============================================================

Division  Problems    mrs                   mrs-ml
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
FNE            100        45   22.674          43   18.739
------------------  --------------------  --------------------
TOTAL          100        45   22.674          43   18.739

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
31207 Jul 11 22:23 /mnt/sda1/mrs/crates/mrs-bench/results/casc-30/20260711_142550/run.csv

commit b81aed47c982581f41b205012ceacab90681aa27

[done] OK
[PPROD:user@server97:~]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions ueq  --casc-times --jobs 2
CASC-30 Results — 2026-07-11 09:36  (300 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
UEQ            300        37   34.208
------------------  --------------------
TOTAL          300        37   34.208

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
38711 Jul 10 23:02 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30/20260710_134850/run.csv

[done] OK
[PPROD:user@server97:~]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions epu  --casc-times --jobs 2
CASC-30 Results — 2026-07-11 09:36  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPU            100        16    7.341
------------------  --------------------
TOTAL          100        16    7.341

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
15213 Jul 10 12:57 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30/20260710_112814/run.csv

[done] KO
[PPROD:user@server97:~]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions eps  --casc-times --jobs 2
CASC-30 Results — 2026-07-11 11:25  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPS            100        42   10.853
------------------  --------------------
TOTAL          100        42   10.853

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
17339 Jul 10 10:30 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30/20260710_092650/run.csv

[done] OK
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions icu --casc-times --jobs 2
CASC-30 Results — 2026-07-10 07:23  (101 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
ICU            101         3  141.386
------------------  --------------------
TOTAL          101         3  141.386

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
14988 Jul 10 02:16 /DATA/ai/mrs/crates/mrs-bench/results/casc-30/20260709_193541/run.csv

[done] OK
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions fne,feq  --casc-times --jobs 2
CASC-30 Results — 2026-07-10 07:16  (500 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        44   15.588
FEQ            400       104   26.235
------------------  --------------------
TOTAL          500       148   23.070

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
77936 Jul 10 08:01 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30/20260709_193910/run.csv

[ongoing]
[PPROD:user@server11:/DATA/DISK1/BENCH/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions ueq --casc-times --jobs 2

[done] OK
[root@server04 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs,mrs-ml --divisions epu --casc-times --jobs 1
CASC-30 Results — 2026-07-10 07:25  (100 problems × 2 systems)
==============================================================

Division  Problems    mrs                   mrs-ml
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
EPU            100        16    8.552          16    8.548
------------------  --------------------  --------------------
TOTAL          100        16    8.552          16    8.548

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done] KO
[root@server03 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs,mrs-ml --divisions eps  --casc-times --jobs 1
CASC-30 Results — 2026-07-10 07:29  (100 problems × 2 systems)
==============================================================

Division  Problems    mrs                   mrs-ml
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
EPS            100        41    9.543          41    9.518
------------------  --------------------  --------------------
TOTAL          100        41    9.543          41    9.518

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
34851 Jul 10 00:04 /mnt/sdd/mrs/crates/mrs-bench/results/casc-30/20260709_194807/run.csv

[root@server02 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs,mrs-ml --divisions fne --casc-times --jobs 1
CASC-30 Results — 2026-07-10 09:20  (100 problems × 2 systems)
==============================================================

Division  Problems    mrs                   mrs-ml
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
FNE            100        44   17.568          43   19.062
------------------  --------------------  --------------------
TOTAL          100        44   17.568          43   19.062

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
30857 Jul 10 03:43 /mnt/sda1/mrs/crates/mrs-bench/results/casc-30/20260709_194456/run.csv

minialloc main
commit 4abc9e8d77facac65d5fa710ace3e02c35cc47be

[done] OK
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions feq --casc-times --jobs 2
CASC-30 Results — 2026-07-09 07:09  (400 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
FEQ            400        99   29.143
------------------  --------------------
TOTAL          400        99   29.143

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
64436 Jul  9 00:39 /DATA/ai/mrs/crates/mrs-bench/results/casc-30/20260708_140214/run.csv

[done] OK
[root@server02 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs,mrs-ml --divisions epu --casc-times --jobs 1
CASC-30 Results — 2026-07-09 07:14  (100 problems × 2 systems)
==============================================================

Division  Problems    mrs                   mrs-ml
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
EPU            100        16    8.032          16    8.056
------------------  --------------------  --------------------
TOTAL          100        16    8.032          16    8.056

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
29457 Jul  8 16:35 /mnt/sda1/mrs/crates/mrs-bench/results/casc-30/20260708_104124/run.csv

[done] OK
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions fne,feq  --casc-times --jobs 2
CASC-30 Results — 2026-07-09 07:16  (500 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        44   14.780
FEQ            400       104   23.832
------------------  --------------------
TOTAL          500       148   21.141

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
79555 Jul  8 22:14 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30/20260708_095324/run.csv


commit 0541f0ca18af2278a3c52ec8597855518c473258 (HEAD -> perf/mimalloc-allocator,

[done] KO
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions eps --casc-times --jobs 1
CASC-30 Results — 2026-07-08 07:39  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
EPS            100        42    9.081
------------------  --------------------
TOTAL          100        42    9.081

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
17621 Jul  7 21:16 /DATA/ai/mrs/crates/mrs-bench/results/casc-30/20260707_191041/run.csv

[done] OK
[PPROD:user@server11:/DATA/DISK1/BENCH/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions fne,feq --casc-times --jobs 2
[PPROD:user@server11:/DATA/DISK1/BENCH/mrs]$ cat /DATA/DISK1/BENCH/mrs/crates/mrs-bench/results/casc-30/20260708_095127/run.csv | grep feq | grep ok  | wc -l
95
[PPROD:user@server11:/DATA/DISK1/BENCH/mrs]$ cat /DATA/DISK1/BENCH/mrs/crates/mrs-bench/results/casc-30/20260708_095127/run.csv | grep fne | grep ok  | wc -l
44

[done] OK
[PPROD:user@server11:/DATA/DISK1/BENCH/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions fne,feq --casc-times --jobs 2
[PPROD:user@server11:/DATA/DISK1/BENCH/mrs]$ cat /DATA/DISK1/BENCH/mrs/crates/mrs-bench/results/casc-30/20260707_185343/run.csv | grep feq | grep ok | wc -l
104
[PPROD:user@server11:/DATA/DISK1/BENCH/mrs]$ cat /DATA/DISK1/BENCH/mrs/crates/mrs-bench/results/casc-30/20260707_185343/run.csv | grep fne | grep ok | wc -l
45
75031 Jul  8 07:16 /DATA/DISK1/BENCH/mrs/crates/mrs-bench/results/casc-30/20260707_185343/run.csv

[done] OK
[root@server04 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs,mrs-ml --divisions epu --casc-times --jobs 1
CASC-30 Results — 2026-07-08 07:37  (100 problems × 2 systems)
==============================================================

Division  Problems    mrs                   mrs-ml
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
EPU            100        16    8.287          16    8.058
------------------  --------------------  --------------------
TOTAL          100        16    8.287          16    8.058

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
29883 Jul  7 23:16 /mnt/sdd1/mrs/crates/mrs-bench/results/casc-30/20260707_172258/run.csv

[done] KO
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions eps --casc-times --jobs 2
CASC-30 Results — 2026-07-07 16:41  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
EPS            100        42    9.239
------------------  --------------------
TOTAL          100        42    9.239

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
17465 Jul  7 18:14 /DATA/ai/mrs/crates/mrs-bench/results/casc-30/20260707_171053/run.csv

commit 38769a7cabb4a04b8c3ba372e0be56012c95d658 (HEAD -> perf/jemalloc-allocator

[done] OK
[root@server01 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs,mrs-ml --divisions epu --casc-times --jobs 1
CASC-30 Results — 2026-07-08 07:29  (100 problems × 2 systems)
==============================================================

Division  Problems    mrs                   mrs-ml
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
EPU            100        16    8.248          16    8.289
------------------  --------------------  --------------------
TOTAL          100        16    8.248          16    8.289

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
29175 Jul  8 01:01 /mnt/sdf1/mrs/crates/mrs-bench/results/casc-30/20260707_190724/run.csv

[done] OK
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions fne,feq  --casc-times --jobs 2
CASC-30 Results — 2026-07-08 07:42  (500 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        44   14.965
FEQ            400       100   20.899
------------------  --------------------
TOTAL          500       144   19.086

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
78134 Jul  8 07:10 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30/20260707_184612/run.csv

[done] OK
[root@server02 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs,mrs-ml --divisions epu --casc-times --jobs 1
CASC-30 Results — 2026-07-08 07:28  (100 problems × 2 systems)
==============================================================

Division  Problems    mrs                   mrs-ml
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
EPU            100        16    7.661          16    7.670
------------------  --------------------  --------------------
TOTAL          100        16    7.661          16    7.670

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
29744 Jul  7 21:54 /mnt/sda1/mrs/crates/mrs-bench/results/casc-30/20260707_160021/run.csv

[done] OK
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions eps --casc-times --jobs 2
CASC-30 Results — 2026-07-07 15:05  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
EPS            100        43    9.000
------------------  --------------------
TOTAL          100        43    9.000

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
17566 Jul  7 16:27 /DATA/ai/mrs/crates/mrs-bench/results/casc-30/20260707_152537/run.csv

commit edb5d2da98e77fe86994698aaaf5058886f6a157

[done] KO
[PPROD:user@server11:/DATA/DISK1/BENCH/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions epu --casc-times --jobs 2
16

[done] KO
[root@server01 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions eps --casc-times --jobs 1
CASC-30 Results — 2026-07-07 15:44  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
EPS            100        40    8.809
------------------  --------------------
TOTAL          100        40    8.809

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected
17624 Jul  7 16:11 /mnt/sdf1/mrs/crates/mrs-bench/results/casc-30/20260707_140101/run.csv

[done] KO
[root@server01 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions eps --casc-times --jobs 1
CASC-30 Results — 2026-07-07 11:44  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPS            100        40    8.763
------------------  --------------------
TOTAL          100        40    8.763

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
17193 Jul  7 12:25 /mnt/sdf1/mrs/crates/mrs-bench/results/casc-30/20260707_101429/run.csv

[done] OK
[root@server02 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs,mrs-ml --divisions epu --casc-times --jobs 1
CASC-30 Results — 2026-07-07 13:50  (100 problems × 2 systems)
==============================================================

Division  Problems    mrs                   mrs-ml
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
EPU            100        16   11.558          16   11.341
------------------  --------------------  --------------------
TOTAL          100        16   11.558          16   11.341

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
29166 Jul  7 15:47 /mnt/sda1/mrs/crates/mrs-bench/results/casc-30/20260707_095047/run.csv

[done] OK
[root@server03 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions icu  --casc-times --jobs 1
CASC-30 Results — 2026-07-08 07:35  (101 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
ICU            101         2   14.357
------------------  --------------------
TOTAL          101         2   14.357

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
13737 Jul  7 23:19 /mnt/sdd/mrs/crates/mrs-bench/results/casc-30/20260707_095835/run.csv

[done] KO
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions eps --casc-times --jobs 2
CASC-30 Results — 2026-07-07 09:11  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
EPS            100        41    9.494
------------------  --------------------
TOTAL          100        41    9.494

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
17758 Jul  7 10:49 /DATA/ai/mrs/crates/mrs-bench/results/casc-30/20260707_094441/run.csv

[done] OK
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions ueq  --casc-times --jobs 2
CASC-30 Results — 2026-07-07 16:35  (300 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
UEQ            300        42   36.811
------------------  --------------------
TOTAL          300        42   36.811

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
41564 Jul  7 18:33 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30/20260707_092728/run.csv

[done] OK
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions feq  --casc-times --jobs 2
CASC-30 Results — 2026-07-07 06:59  (400 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
FEQ            400        96   28.312
------------------  --------------------
TOTAL          400        96   28.312

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
64612 Jul  7 05:12 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30/20260706_182806/run.csv

[done] OK
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions fne --casc-times --jobs 1
CASC-30 Results — 2026-07-07 06:54  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
FNE            100        44   22.066
------------------  --------------------
TOTAL          100        44   22.066

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
15654 Jul  6 22:37 /DATA/ai/mrs/crates/mrs-bench/results/casc-30/20260706_183715/run.csv

[done] OK
[root@server04 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions feq  --casc-times --jobs 1
CASC-30 Results — 2026-07-07 15:17  (400 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
FEQ            400        91   25.640
------------------  --------------------
TOTAL          400        91   25.640

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
62891 Jul  7 16:18 /mnt/sdd1/mrs/crates/mrs-bench/results/casc-30/20260706_183439/run.csv

[done] OK
[root@server03 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions fne  --casc-times --jobs 1
CASC-30 Results — 2026-07-07 07:01  (100 problems × 1 systems)
==============================================================                                                                                                                                          
Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
FNE            100        43   20.299
------------------  --------------------
TOTAL          100        43   20.299

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
15852 Jul  6 22:39 /mnt/sdd/mrs/crates/mrs-bench/results/casc-30/20260706_183552/run.csv

commit 927353aeacbb365e64b8f837035570e97391de67 (HEAD -> perf/ml-prune-budget-tax

[done] OK
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions fne --casc-times --jobs 2
CASC-30 Results — 2026-07-06 11:42  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        45   22.042
------------------  --------------------
TOTAL          100        45   22.042

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
15262 Jul  6 13:08 /DATA/ai/mrs/crates/mrs-bench/results/casc-30/20260706_110843/run.csv

[done] OK
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions fne  --casc-times --jobs 2
CASC-30 Results — 2026-07-06 11:44  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
FNE            100        42   17.837
------------------  --------------------
TOTAL          100        42   17.837

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
15956 Jul  6 13:08 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30/20260706_110418/run.csv

[done] KO
[root@server02 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions eps --casc-times --jobs 1
CASC-30 Results — 2026-07-06 11:47  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPS            100        40    7.789
------------------  --------------------
TOTAL          100        40    7.789

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
17324 Jul  6 11:45 /mnt/sda1/mrs/crates/mrs-bench/results/casc-30/20260706_093535/run.csv

[done] KO
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions eps  --casc-times --jobs 2
CASC-30 Results — 2026-07-06 08:20  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
EPS            100        41    9.526
------------------  --------------------
TOTAL          100        41    9.526

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
17903 Jul  6 10:20 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30/20260706_091539/run.csv

[done] KO
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions eps --casc-times --jobs 2
CASC-30 Results — 2026-07-06 08:22  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPS            100        41    9.517
------------------  --------------------
TOTAL          100        41    9.517

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
17334 Jul  6 10:21 /DATA/ai/mrs/crates/mrs-bench/results/casc-30/20260706_091652/run.csv

[done] OK
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions icu  --casc-times --jobs 2
CASC-30 Results — 2026-07-06 07:13  (101 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
ICU            101         3  186.798
------------------  --------------------
TOTAL          101         3  186.798

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
14369 Jul  6 01:35 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30/20260705_185418/run.csv

[done] OK
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions icu --casc-times --jobs 2
CASC-30 Results — 2026-07-06 07:00  (101 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
ICU            101         3  143.242
------------------  --------------------
TOTAL          101         3  143.242

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
14687 Jul  6 01:30 /DATA/ai/mrs/crates/mrs-bench/results/casc-30/20260705_184918/run.csv

[done] OK
root@server04 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions feq  --casc-times --jobs 1
CASC-30 Results — 2026-07-06 14:49  (400 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
FEQ            400        88   26.180
------------------  --------------------
TOTAL          400        88   26.180

DISAGREEMENTS — none detected.                                                                                                                                                                                   
POLARITY VIOLATIONS — none detected.                                                                                                                                                                             
REFERENCE VIOLATIONS — none detected.
63995 Jul  6 16:23 /mnt/sdd1/mrs/crates/mrs-bench/results/casc-30/20260705_182901/run.csv

[done] OK
[root@server03 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions ueq  --casc-times --jobs 1
CASC-30 Results — 2026-07-06 11:50  (300 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
UEQ            300        38   36.738
------------------  --------------------
TOTAL          300        38   36.738

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
37794 Jul  6 13:01 /mnt/sdd/mrs/crates/mrs-bench/results/casc-30/20260705_183422/run.csv

commit 55986ce809ead712c328c27d62804d610bef99f2

[done] OK
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions ueq  --casc-times --jobs 2
CASC-30 Results — 2026-07-05 16:51  (300 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
UEQ            300        34   19.014
------------------  --------------------
TOTAL          300        34   19.014

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
45283 Jul  5 18:45 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30/20260705_093224/run.csv

[done] OK
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions ueq --casc-times --jobs 2
CASC-30 Results — 2026-07-05 16:45  (300 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
UEQ            300        39   28.508
------------------  --------------------
TOTAL          300        39   28.508

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
41302 Jul  5 18:44 /DATA/ai/mrs/crates/mrs-bench/results/casc-30/20260705_093534/run.csv

[done]
hack@pve:~/mrs$ MRS_WORKERS=4 crates/mrs-bench/casc.sh --systems mrs --divisions feq  --casc-times --jobs 1
CASC-30 Results — 2026-07-05 21:36  (400 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FEQ            400        85   28.452
------------------  --------------------
TOTAL          400        85   28.452

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
67576 Jul  5 17:55 /home/hack/mrs/crates/mrs-bench/results/casc-30/20260704_195736/run.csv

[done] OK
[root@server03 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions feq --casc-times --jobs 1
CASC-30 Results — 2026-07-05 16:30  (400 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FEQ            400        98   25.305
------------------  --------------------
TOTAL          400        98   25.305

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
59404 Jul  5 16:22 /mnt/sdd/mrs/crates/mrs-bench/results/casc-30/20260704_190435/run.csv

[done] OK
[root@server04 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions feq  --casc-times --jobs 1
CASC-30 Results — 2026-07-05 16:23  (400 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
FEQ            400        82   23.548
------------------  --------------------
TOTAL          400        82   23.548

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
63513 Jul  5 17:09 /mnt/sdd1/mrs/crates/mrs-bench/results/casc-30/20260704_185616/run.csv

commit 79f6c0640467983b4146060f8ebab1490a70c85c 

[done] OK
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions fne --casc-times --jobs 2
CASC-30 Results — 2026-07-05 07:17  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
FNE            100        42   23.953
------------------  --------------------
TOTAL          100        42   23.953

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
15814 Jul  4 20:14 /DATA/ai/mrs/crates/mrs-bench/results/casc-30/20260704_180751/run.csv

[done] KO
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions eps  --casc-times --jobs 2
CASC-30 Results — 2026-07-05 07:14  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
EPS            100        40    6.826
------------------  --------------------
TOTAL          100        40    6.826

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
18041 Jul  4 19:41 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30/20260704_183650/run.csv

[done] OK
[PPROD:user@server97:/DATA/ai/user/mrs]$ cargo run --release -p mrs-train --features wgpu -- --mode premise --epochs 150 --val-split 0.15 --neg-per-pos 5 ./ml_logs_collection_epr models/weights_premise_epr
TrainingProgress { progress: Some(Progress { items_processed: 142218, items_total: 142218 }), global_progress: Progress { items_processed: 105, items_total: 150 }, iteration: Some(18) }
======================== Learner Summary ========================
Model:
"TrainingPremise" {
  model: "PremiseModel" {
    layer1: Linear {d_input: 24, d_output: 256, bias: true, params: 6400}
    layer2: Linear {d_input: 256, d_output: 128, bias: true, params: 32896}
    layer3: Linear {d_input: 128, d_output: 64, bias: true, params: 8256}
    output: Linear {d_input: 64, d_output: 1, bias: true, params: 65}
    params: 47617
  }
  params: 47617
}
Total Epochs: 105


| Split | Metric | Min.     | Epoch    | Max.     | Epoch    |
|-------|--------|----------|----------|----------|----------|
| Train | Loss   | 0.081    | 105      | 0.429    | 1        |
| Valid | Loss   | 0.083    | 99       | 0.202    | 1        |

Saved models/weights_premise_epr.bin and models/weights_premise_epr_meta.json

[done] OK
[PPROD:user@server97:/DATA/ai/user/mrs]$ cargo run --release -p mrs-train --features wgpu -- --mode premise --epochs 150 --val-split 0.15 --neg-per-pos 5 ./ml_logs_collection_ac_feq/premise models/weights_premise_feq
======================== Learner Summary ========================
Model:
"TrainingPremise" {
  model: "PremiseModel" {
    layer1: Linear {d_input: 24, d_output: 256, bias: true, params: 6400}
    layer2: Linear {d_input: 256, d_output: 128, bias: true, params: 32896}
    layer3: Linear {d_input: 128, d_output: 64, bias: true, params: 8256}
    output: Linear {d_input: 64, d_output: 1, bias: true, params: 65}
    params: 47617
  }
  params: 47617
}
Total Epochs: 40


| Split | Metric | Min.     | Epoch    | Max.     | Epoch    |
|-------|--------|----------|----------|----------|----------|
| Train | Loss   | 0.179    | 40       | 0.273    | 1        |
| Valid | Loss   | 0.181    | 25       | 0.203    | 1        |

Saved models/weights_premise_feq.bin and models/weights_premise_feq_meta.json


[done]
hack@pve:~/mrs$ MRS_WORKERS=4 crates/mrs-bench/casc.sh --systems mrs,mrs-ml --divisions eps  --casc-times --jobs 1
CASC-30 Results — 2026-07-04 17:55  (100 problems × 2 systems)
==============================================================

Division  Problems    mrs                   mrs-ml
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
EPS            100        39   10.875          39   10.851
------------------  --------------------  --------------------
TOTAL          100        39   10.875          39   10.851

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
35619 Jul  4 19:55 /home/hack/mrs/crates/mrs-bench/results/casc-30/20260704_153408/run.csv

commit 5838b353b050eda616bb78722b4c25f201e2d8f5

[done] KO
[root@server04 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions eps  --casc-times --jobs 1
CASC-30 Results — 2026-07-04 16:01  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
EPS            100        40    7.602
------------------  --------------------
TOTAL          100        40    7.602

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
17761 Jul  4 15:58 /mnt/sdd1/mrs/crates/mrs-bench/results/casc-30/20260704_134855/run.csv

[done] OK
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions fne --casc-times --jobs 1
CASC-30 Results — 2026-07-04 15:57  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        45   18.708
------------------  --------------------
TOTAL          100        45   18.708

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
15417 Jul  4 17:39 /DATA/ai/mrs/crates/mrs-bench/results/casc-30/20260704_134535/run.csv

commit 03615fd02ab63cec85643ecd59abf8bb3f792807

[done] OK
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs,mrs-ml --divisions fne  --casc-times --jobs 2
CASC-30 Results — 2026-07-04 15:48  (100 problems × 2 systems)
==============================================================

Division  Problems    mrs                   mrs-ml
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
FNE            100        44   15.438          31   31.626
------------------  --------------------  --------------------
TOTAL          100        44   15.438          31   31.626

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done] OK
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs,mrs-ml --divisions epu  --casc-times --jobs 2
CASC-30 Results — 2026-07-04 09:37  (100 problems × 2 systems)
==============================================================

Division  Problems    mrs                   mrs-ml
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
EPU            100        16    9.532          13    5.982
------------------  --------------------  --------------------
TOTAL          100        16    9.532          13    5.982

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
32959 Jul  4 11:15 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30/20260704_091222/run.csv

[done] OK
[root@server02 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs,mrs-ml --divisions ueq --casc-times --jobs 1
CASC-30 Results — 2026-07-06 07:16  (300 problems × 2 systems)
==============================================================

Division  Problems    mrs                   mrs-ml
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
UEQ            300        36   27.221          28   25.839
------------------  --------------------  --------------------
TOTAL          300        36   27.221          28   25.839

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
79273 Jul  5 21:31 /mnt/sda1/mrs/crates/mrs-bench/results/casc-30/20260704_090244/run.csv

[done] OK
[root@server03 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs,mrs-ml --divisions fne  --casc-times --jobs 1
CASC-30 Results — 2026-07-04 16:03  (100 problems × 2 systems)
==============================================================

Division  Problems    mrs                   mrs-ml
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
FNE            100        43   13.173          30   32.511
------------------  --------------------  --------------------
TOTAL          100        43   13.173          30   32.511

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
32558 Jul  4 17:44 /mnt/sdd/mrs/crates/mrs-bench/results/casc-30/20260704_085301/run.csv

[done]
hack@pve:~/mrs$ MRS_WORKERS=4 crates/mrs-bench/casc.sh --systems mrs,mrs-ml --divisions epu  --casc-times --jobs 1
CASC-30 Results — 2026-07-04 13:26  (100 problems × 2 systems)
==============================================================

Division  Problems    mrs                   mrs-ml
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
EPU            100        16   21.573          13   14.646
------------------  --------------------  --------------------
TOTAL          100        16   21.573          13   14.646

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
34334 Jul  4 13:24 /home/hack/mrs/crates/mrs-bench/results/casc-30/20260704_091409/run.csv

[done]
hack@pve:~/mrs$ MRS_WORKERS=4 crates/mrs-bench/casc.sh --systems mrs,mrs-ml --divisions eps  --casc-times --jobs 1
CASC-30 Results — 2026-07-04 06:33  (100 problems × 2 systems)
==============================================================

Division  Problems    mrs                   mrs-ml
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
EPS            100        39   10.876          37    6.879
------------------  --------------------  --------------------
TOTAL          100        39   10.876          37    6.879

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
34608 Jul  4 04:32 /home/hack/mrs/crates/mrs-bench/results/casc-30/20260704_000814/run.csv

commit commit aa07504a14725d9d4ca64bfca1c649e413dbc268 (HEAD -> ac-indexing, origin/ac-indexing)

[done] KO
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions eps  --casc-times --jobs 2
CASC-30 Results — 2026-07-04 06:39  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
EPS            100        40    8.606
------------------  --------------------
TOTAL          100        40    8.606

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
17645 Jul  3 20:30 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30/20260703_192514/run.csv


epu unsoundness fixed
commit not yet
diff --git a/src/main.rs b/src/main.rs
index 1ebd1cd..3a47c46 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -473,7 +473,9 @@ fn main() {
                 }
             }
             SearchResult::Saturated => {
-                if has_conjecture {
+                   if ml_prune_ratio.is_some() {
+                        SzsStatus::GaveUp //   Soundness Guard!
+                   } else if has_conjecture {
                     SzsStatus::CounterSatisfiable
                 } else {
                     SzsStatus::Satisfiable

[done] OK
[PPROD:user@server97:/DATA/ai/user/mrs]$ cargo run --release -p mrs-train --features wgpu -- --mode premise --epochs 150 --val-split 0.15 --neg-per-pos 5 ./ml_logs_collection_fne models/weights_premise_fne
======================== Learner Summary ========================
Model:
"TrainingPremise" {
  model: "PremiseModel" {
    layer1: Linear {d_input: 24, d_output: 256, bias: true, params: 6400}
    layer2: Linear {d_input: 256, d_output: 128, bias: true, params: 32896}
    layer3: Linear {d_input: 128, d_output: 64, bias: true, params: 8256}
    output: Linear {d_input: 64, d_output: 1, bias: true, params: 65}
    params: 47617
  }
  params: 47617
}
Total Epochs: 150


| Split | Metric | Min.     | Epoch    | Max.     | Epoch    |
|-------|--------|----------|----------|----------|----------|
| Train | Loss   | 0.148    | 149      | 0.507    | 1        |
| Valid | Loss   | 0.148    | 147      | 0.271    | 1        |

Saved models/weights_premise_fne.bin and models/weights_premise_fne_meta.json

[done] OK
[PPROD:user@server97:/DATA/ai/user/mrs]$ cargo run --release -p mrs-train --features wgpu -- --mode schedule --epochs 150 --val-split 0.15 ./ml_logs_collection_fne models/weights_schedule_fne
======================== Learner Summary ========================
Model:
"TrainingSchedule" {
  model: "ScheduleModel" {
    layer1: Linear {d_input: 16, d_output: 256, bias: true, params: 4352}
    layer2: Linear {d_input: 256, d_output: 128, bias: true, params: 32896}
    layer3: Linear {d_input: 128, d_output: 64, bias: true, params: 8256}
    output: Linear {d_input: 64, d_output: 5, bias: true, params: 325}
    params: 45829
  }
  params: 45829
}
Total Epochs: 27


| Split | Metric   | Min.     | Epoch    | Max.     | Epoch    |
|-------|----------|----------|----------|----------|----------|
| Train | Accuracy | 90.071   | 1        | 90.071   | 27       |
| Train | Loss     | 0.000e0  | 13       | 0.061    | 1        |
| Valid | Accuracy | 90.032   | 1        | 90.032   | 27       |
| Valid | Loss     | 0.000e0  | 12       | 9.710e-5 | 1        |

Saved models/weights_schedule_fne.bin and models/weights_schedule_fne_meta.json

[done] OK
[PPROD:user@server97:/DATA/ai/user/mrs]$ cargo run --release -p mrs-train --features wgpu -- --mode schedule --epochs 150 --val-split 0.15 ./ml_logs_collection_ac_ueq models/weights_schedule_ueq
======================== Learner Summary ========================
Model:
"TrainingSchedule" {
  model: "ScheduleModel" {
    layer1: Linear {d_input: 16, d_output: 256, bias: true, params: 4352}
    layer2: Linear {d_input: 256, d_output: 128, bias: true, params: 32896}
    layer3: Linear {d_input: 128, d_output: 64, bias: true, params: 8256}
    output: Linear {d_input: 64, d_output: 5, bias: true, params: 325}
    params: 45829
  }
  params: 45829
}
Total Epochs: 32


| Split | Metric   | Min.     | Epoch    | Max.     | Epoch    |
|-------|----------|----------|----------|----------|----------|
| Train | Accuracy | 90.737   | 1        | 92.994   | 32       |
| Train | Loss     | 0.000e0  | 18       | 0.135    | 1        |
| Valid | Accuracy | 93.026   | 1        | 93.026   | 32       |
| Valid | Loss     | 0.000e0  | 17       | 2.332e-4 | 1        |

Saved models/weights_schedule_ueq.bin and models/weights_schedule_ueq_meta.json

[done] OK
PPROD:user@server97:/DATA/ai/user/mrs]$ cargo run --release -p mrs-train --features wgpu -- --mode premise --epochs 150 --val-split 0.15 --neg-per-pos 5 ./ml_logs_collection_ueq models/weights_premise_ueq
======================== Learner Summary ========================
Model:
"TrainingPremise" {
  model: "PremiseModel" {
    layer1: Linear {d_input: 24, d_output: 256, bias: true, params: 6400}
    layer2: Linear {d_input: 256, d_output: 128, bias: true, params: 32896}
    layer3: Linear {d_input: 128, d_output: 64, bias: true, params: 8256}
    output: Linear {d_input: 64, d_output: 1, bias: true, params: 65}
    params: 47617
  }
  params: 47617
}
Total Epochs: 96


| Split | Metric | Min.     | Epoch    | Max.     | Epoch    |
|-------|--------|----------|----------|----------|----------|
| Train | Loss   | 0.205    | 95       | 0.614    | 1        |
| Valid | Loss   | 0.207    | 93       | 0.439    | 1        |

Saved models/weights_premise_ueq.bin and models/weights_premise_ueq_meta.json

[done] KO
[PPROD:user@server97:/DATA/ai/user/mrs]$ ./crates/mrs-bench/collect_ml_data.sh /DATA/ai/user/TPTP-v9.2.1 ./ml_logs_collection_ac_ueq 16 auto 1

[done] KO
[www@server99 mrs]$ ./crates/mrs-bench/collect_ml_data.sh /DATA/ai/TPTP-v9.2.1 ./ml_logs_collection_ac_feq 14 auto 1

[done] KO
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions eps  --casc-times --jobs 2
CASC-30 Results — 2026-07-03 13:58  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
EPS            100         0    0.000
------------------  --------------------
TOTAL          100         0    0.000

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
19217 Jul  3 15:52 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30/20260703_150454/run.csv

[done] OK
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions epu  --casc-times --jobs 2
CASC-30 Results — 2026-07-03 13:02  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
EPU            100        13    5.922
------------------  --------------------
TOTAL          100        13    5.922

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
nobody 18419 Jul  3 14:27 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30/20260703_135230/run.csv


commit d8f129f4dcfc277ebc750c091e47465b13846345 

[done] OK
still issue even with 1 and MiB Mem :  95969.6 total
[root@server03 mrs]# ./crates/mrs-bench/collect_ml_data.sh /mnt/sdd/TPTP-v9.2.1 ./ml_logs_collection_epr 1 auto 1
Building prover with 'ml' feature...
   Compiling mrs v0.1.9 (/mnt/sdd/mrs)
    Finished `release` profile [optimized] target(s) in 2m 47s
Using provided problem list: ./casc_problem_lists/epr.list
Found 4928 problems.
Running data collection with 1 parallel jobs, 1 threads per problem (Time limit: Division-Specific Auto-Scaling)...
bash: line 33: 1870083 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 33: 1872542 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 33: 1872732 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 33: 1872795 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 33: 1872951 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 33: 1873622 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 33: 1873768 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 33: 1873838 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 33: 1873933 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 33: 1874002 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 33: 1874149 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 33: 1884168 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 33: 1887131 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 33: 1887561 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 33: 1887634 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 33: 1887773 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 33: 1887786 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1

[done] OK
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions fne --casc-times --jobs 2
[www@server99 mrs]$ cargo run -p mrs-bench --bin bench_report -- /DATA/ai/mrs/crates/mrs-bench/results/casc-30/20260703_124524/run.csv
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.00s
     Running `target/debug/bench_report /DATA/ai/mrs/crates/mrs-bench/results/casc-30/20260703_124524/run.csv`
CASC-30 Results — 2026-07-03 13:16  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
FNE            100        35   52.943
------------------  --------------------
TOTAL          100        35   52.943

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
8212 Jul  3 15:15 /DATA/ai/mrs/crates/mrs-bench/results/casc-30/20260703_124524/run.csv

[done] OK
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions epu  --casc-times --jobs 2
CASC-30 Results — 2026-07-03 11:11  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
EPU            100        71    6.187
------------------  --------------------
TOTAL          100        71    6.187

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — 57 case(s) of wrong SZS polarity:
  EPU     HWV051-1                        mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     HWV058-1                        mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     HWV065-1                        mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     HWV078-1                        mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     HWV081-1                        mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     HWV083-1                        mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     MSC015-1.022                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     MSC015-1.025                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     MSC015-1.030                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     MSC024-1                        mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     PLA031-1.007                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     PLA031-1.008                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     PLA037-1                        mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     PLA042-1                        mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     PUZ008-2                        mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     PUZ037-2                        mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     PUZ037-3                        mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV418-1.300                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV418-1.500                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV418-1.580                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV418-1.820                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV418-1.900                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV419-1.010                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV419-1.020                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV419-1.030                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV419-1.035                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV419-1.040                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV420-1.020                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV420-1.030                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV420-1.035                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV420-1.040                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV420-1.045                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV421-1.200                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV421-1.205                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV421-1.300                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV421-1.360                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV421-1.365                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV421-1.400                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV421-1.405                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV421-1.505                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV422-1.300                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV422-1.305                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV422-1.365                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV422-1.400                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV422-1.405                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV422-1.460                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV422-1.465                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV422-1.500                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV422-1.505                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV423-1.010                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SWV423-1.020                    mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SYO588-1                        mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SYO589-1                        mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SYO591-1                        mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SYO592-1                        mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SYO594-1                        mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPU     SYO597-1                        mrs-ml=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND

REFERENCE VIOLATIONS — 57 SOUNDNESS ERROR(S) vs reference answers:
  EPU     HWV051-1                        mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     HWV058-1                        mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     HWV078-1                        mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     HWV081-1                        mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     HWV083-1                        mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     HWV065-1                        mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     MSC015-1.022                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     MSC015-1.025                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     MSC024-1                        mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     PLA031-1.007                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     PLA031-1.008                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     PLA037-1                        mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     MSC015-1.030                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     PLA042-1                        mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     PUZ008-2                        mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     PUZ037-2                        mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     PUZ037-3                        mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV418-1.300                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV418-1.500                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV418-1.580                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV418-1.820                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV418-1.900                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV419-1.010                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV419-1.020                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV419-1.030                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV419-1.035                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV419-1.040                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV420-1.020                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV420-1.030                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV420-1.035                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV420-1.040                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV420-1.045                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV421-1.200                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV421-1.205                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV421-1.300                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV421-1.365                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV421-1.400                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV421-1.360                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV421-1.405                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV421-1.505                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV422-1.300                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV422-1.305                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV422-1.365                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV422-1.400                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV422-1.405                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV422-1.460                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV422-1.465                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV422-1.500                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV422-1.505                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV423-1.010                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SWV423-1.020                    mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SYO588-1                        mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SYO589-1                        mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SYO591-1                        mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SYO592-1                        mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SYO594-1                        mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND
  EPU     SYO597-1                        mrs-ml=Satisfiable but expected Unsatisfiable  ⚠ UNSOUND

[done] KO
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions eps  --casc-times --jobs 2
CASC-30 Results — 2026-07-03 10:35  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
EPS            100        72    4.474
------------------  --------------------
TOTAL          100        72    4.474

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
16152 Jul  3 12:24 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30/20260703_115201/run.csv

[PPROD:user@server97:/DATA/ai/user/mrs]$ cargo run --release -p mrs-train --features wgpu -- --mode schedule --epochs 150 --val-split 0.15 ./ml_logs_collection_epr/ models/weights_schedule_epr
======================== Learner Summary ========================
Model:
"TrainingPremise" {
  model: "PremiseModel" {
    layer1: Linear {d_input: 24, d_output: 256, bias: true, params: 6400}
    layer2: Linear {d_input: 256, d_output: 128, bias: true, params: 32896}
    layer3: Linear {d_input: 128, d_output: 64, bias: true, params: 8256}
    output: Linear {d_input: 64, d_output: 1, bias: true, params: 65}
    params: 47617
  }
  params: 47617
}
Total Epochs: 139


| Split | Metric | Min.     | Epoch    | Max.     | Epoch    |
|-------|--------|----------|----------|----------|----------|
| Train | Loss   | 0.080    | 136      | 0.430    | 1        |
| Valid | Loss   | 0.081    | 130      | 0.209    | 1        |

Saved models/weights_premise_epr.bin and models/weights_premise_epr_meta.json

[PPROD:user@server97:/DATA/ai/user/mrs]$ cargo run --release -p mrs-train --features wgpu -- --mode premise --epochs 150 --val-split 0.15 --neg-per-pos 5 ./ml_logs_collection_epr/premise models/weights_premise_epr

[PPROD:user@server97:/DATA/ai/user/mrs]$ cargo run --release -p mrs-train --features wgpu -- --mode schedule --epochs 150 --val-split 0.15 ./ml_logs_collection_epr/ models/weights_schedule_epr
======================== Learner Summary ========================
Model:
"TrainingSchedule" {
  model: "ScheduleModel" {
    layer1: Linear {d_input: 16, d_output: 256, bias: true, params: 4352}
    layer2: Linear {d_input: 256, d_output: 128, bias: true, params: 32896}
    layer3: Linear {d_input: 128, d_output: 64, bias: true, params: 8256}
    output: Linear {d_input: 64, d_output: 5, bias: true, params: 325}
    params: 45829
  }
  params: 45829
}
Total Epochs: 22


| Split | Metric   | Min.     | Epoch    | Max.     | Epoch    |
|-------|----------|----------|----------|----------|----------|
| Train | Accuracy | 89.763   | 1        | 90.667   | 22       |
| Train | Loss     | 0.000e0  | 8        | 0.050    | 1        |
| Valid | Accuracy | 90.648   | 1        | 90.648   | 22       |
| Valid | Loss     | 0.000e0  | 7        | 1.774e-5 | 1        |

Saved models/weights_schedule_epr.bin and models/weights_schedule_epr_meta.json

commit d8f129f4dcfc277ebc750c091e47465b13846345 

[root@server03 mrs]# export INPUT_PROBLEMS_LIST=./casc_problem_lists/epr.list
[root@server03 mrs]# ./crates/mrs-bench/collect_ml_data.sh /mnt/sdd/TPTP-v9.2.1 ./ml_logs_collection_epr 1 auto 1

[root@server02 mrs]# export INPUT_PROBLEMS_LIST=./casc_problem_lists/epr.list
[root@server02 mrs]# ./crates/mrs-bench/collect_ml_data.sh /mnt/sdf1/TPTP-v9.2.1 ./ml_logs_collection_epr 8 auto 1

commit d7e750106462c663e3f95bcb5c28ac251eecdf27 (HEAD -> ac-indexing

[done] OK
[PPROD:user@server11:/DATA/DISK1/BENCH/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions ueq --casc-times --jobs 2

TODO report
40164 Jul  2 23:36 /DATA/DISK1/BENCH/mrs/crates/mrs-bench/results/casc-30/20260702_142456/run.csv

[done] OK
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions feq --casc-times --jobs 2

CASC-30 Results — 2026-07-03 06:30  (400 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FEQ            400        98   24.849
------------------  --------------------
TOTAL          400        98   24.849

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
60883 Jul  3 00:41 /DATA/ai/mrs/crates/mrs-bench/results/casc-30/20260702_140158/run.csv

commit 8c6d6460032a9c7d779049b758d07e6584926208

[done] OK

[root@server02 mrs]# crates/mrs-bench/casc.sh --systems mrs-ml --divisions fne --casc-times --jobs 8
CASC-30 Results — 2026-07-02 07:49 (100 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
FNE            100        39   38.875
------------------  --------------------
TOTAL          100        39   38.875

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

15678 Jul  2 09:38 /mnt/sda1/mrs/crates/mrs-bench/results/casc-30/20260702_090213/run.csv

[done] OK
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions ueq  --casc-times --jobs 2
CASC-30 Results — 2026-07-02 16:26  (300 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
UEQ            300        27   53.585
------------------  --------------------
TOTAL          300        27   53.585

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected
22909 Jul  2 17:54 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30/20260702_081423/run.csv

[done] KO till END but still OOM
[PPROD:user@server97:/DATA/ai/user/mrs]$ ./crates/mrs-bench/collect_ml_data.sh /DATA/ai/user/TPTP-v9.2.1 ./ml_logs_collection_epr 2 auto 1


[stopped] OOM 22G Gb RAM per worker
[www@server99 mrs]$ ./crates/mrs-bench/collect_ml_data.sh /DATA/ai/TPTP-v9.2.1 ./ml_logs_collection_epr 14 auto 1
Using provided problem list: ./casc_problem_lists/epr.list
Found 4928 problems.
Running data collection with 14 parallel jobs, 1 threads per problem (Time limit: Division-Specific Auto-Scaling)...
xargs: warning: options --max-args and --replace/-I/-i are mutually exclusive, ignoring previous --max-args value
bash: line 34: 3847643 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 34: 3848945 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 34: 3849040 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 34: 3849154 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 34: 3849271 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 34: 3849764 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 34: 3849866 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 34: 3849919 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 34: 3849989 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 34: 3850038 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 34: 3850138 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 34: 3850402 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 34: 3850543 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 34: 3850866 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 34: 3850913 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 34: 3851033 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1
bash: line 34: 3851043 Aborted                 timeout "${LIMIT}s" "$MRS_BIN" --time "$LIMIT" --workers "$WORKERS" --schedule "$SCHEDULE" --log-ml-data "$SPECIFIC_LOG_DIR" "$FILE" > /dev/null 2>&1

[PPROD:user@server97:/DATA/ai/user/mrs]$ cargo run --release -p mrs-train --features wgpu -- --mode premise --epochs 20 --val-split 0.15 --neg-per-pos 2 ./ml_logs_collection_ueq
======================== Learner Summary ========================
Model:
"TrainingPremise" {
  model: "PremiseModel" {
    layer1: Linear {d_input: 24, d_output: 64, bias: true, params: 1600}
    layer2: Linear {d_input: 64, d_output: 32, bias: true, params: 2080}
    output: Linear {d_input: 32, d_output: 1, bias: true, params: 33}
    params: 3713
  }
  params: 3713
}
Total Epochs: 20


| Split | Metric | Min.     | Epoch    | Max.     | Epoch    |
|-------|--------|----------|----------|----------|----------|
| Train | Loss   | 0.326    | 20       | 0.544    | 1        |
| Valid | Loss   | 0.329    | 19       | 0.402    | 1        |

Saved weights.bin and weights_meta.json

[PPROD:user@server97:/DATA/ai/user/mrs]$ cargo run --release -p mrs-train --features wgpu -- --mode schedule --epochs 25 --val-split 0.15 ./ml_logs_collection_ueq
======================== Learner Summary ========================
Model:
"TrainingSchedule" {
  model: "ScheduleModel" {
    layer1: Linear {d_input: 16, d_output: 32, bias: true, params: 544}
    layer2: Linear {d_input: 32, d_output: 16, bias: true, params: 528}
    output: Linear {d_input: 16, d_output: 5, bias: true, params: 85}
    params: 1157
  }
  params: 1157
}
Total Epochs: 6


| Split | Metric   | Min.     | Epoch    | Max.     | Epoch    |
|-------|----------|----------|----------|----------|----------|
| Train | Accuracy | 92.980   | 1        | 93.058   | 6        |
| Train | Loss     | 0.000e0  | 2        | 3.926e-3 | 1        |
| Valid | Accuracy | 93.091   | 1        | 93.091   | 6        |
| Valid | Loss     | 0.000e0  | 1        | 0.000e0  | 6        |

Saved weights.bin and weights_meta.json

[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs-ml --divisions fne  --casc-times --jobs 2
CASC-30 Results — 2026-07-02 05:33  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
FNE            100        42   16.948
------------------  --------------------
TOTAL          100        42   16.948

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
15830 Jul  1 22:23 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30/20260701_202000/run.csv

commit 06a429aedfe5061d5a43b6dee12276e5d695536a (HEAD -> ml-preprocessing, origin/ml-preprocessing)

[root@server02 mrs]# export INPUT_PROBLEMS_LIST=./casc_problem_lists/epr.list
[root@server02 mrs]# ./crates/mrs-bench/collect_ml_data.sh /mnt/sdf1/TPTP-v9.2.1 ./ml_logs_collection_epr 8 auto 1

commit without EPR fix
commit 03bf807040125bf62d6eb2d3ae8b50611fb1a605 (HEAD -> ml-preprocessing, origin/ml-preprocessing)

[ongoing]
[PPROD:user@server97:/DATA/ai/user/mrs]$ cargo run --release -p mrs-train --features wgpu -- --mode premise --epochs 20 --val-split 0.15 --neg-per-pos 2 ./ml_logs_collection_fne weights_premise_fne
======================== Learner Summary ========================
Model:
"TrainingPremise" {
  model: "PremiseModel" {
    layer1: Linear {d_input: 24, d_output: 64, bias: true, params: 1600}
    layer2: Linear {d_input: 64, d_output: 32, bias: true, params: 2080}
    output: Linear {d_input: 32, d_output: 1, bias: true, params: 33}
    params: 3713
  }
  params: 3713
}
Total Epochs: 20


| Split | Metric | Min.     | Epoch    | Max.     | Epoch    |
|-------|--------|----------|----------|----------|----------|
| Train | Loss   | 0.235    | 20       | 0.445    | 1        |
| Valid | Loss   | 0.237    | 19       | 0.334    | 1        |

Saved weights_premise_fne.bin and weights_premise_fne_meta.json

[done] OK
[PPROD:user@server97:/DATA/ai/user/mrs]$ cargo run --release -p mrs-train --features wgpu -- --mode schedule --epochs 25 --val-split 0.15 ./ml_logs_collection_fne weights_schedule_fne
======================== Learner Summary ========================
Model:
"TrainingSchedule" {
  model: "ScheduleModel" {
    layer1: Linear {d_input: 16, d_output: 32, bias: true, params: 544}
    layer2: Linear {d_input: 32, d_output: 16, bias: true, params: 528}
    output: Linear {d_input: 16, d_output: 5, bias: true, params: 85}
    params: 1157
  }
  params: 1157
}
Total Epochs: 6


| Split | Metric   | Min.     | Epoch    | Max.     | Epoch    |
|-------|----------|----------|----------|----------|----------|
| Train | Accuracy | 90.033   | 1        | 90.071   | 6        |
| Train | Loss     | 0.000e0  | 2        | 2.082e-3 | 1        |
| Valid | Accuracy | 90.032   | 1        | 90.032   | 6        |
| Valid | Loss     | 0.000e0  | 1        | 0.000e0  | 6        |

Saved weights_schedule_fne.bin and weights_schedule_fne_meta.json

[done] KO
[PPROD:user@server97:/DATA/ai/user/mrs]$ ./crates/mrs-bench/collect_ml_data.sh /DATA/ai/user/TPTP-v9.2.1 ./ml_logs_collection_ueq 16 auto 1

[done] OK
[PPROD:user@server97:/DATA/ai/user/mrs]$ ./crates/mrs-bench/collect_ml_data.sh /DATA/ai/user/TPTP-v9.2.1 ./ml_logs_collection_fne 16 auto 1

[done] OK
epr with errors

[ongoing]
99 feq

final AVX2 portfolios
commit 298c71c43d58c532eefdaf75da40c730dcf26383

11
20260630_134541

[done] OK
[PPROD:user@server11:/DATA/DISK1/BENCH/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions epu --casc-times --jobs 2

CASC-30 Results — 2026-06-30 13:26  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPU            100        16    0.000
------------------  --------------------
TOTAL          100        16    0.000

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.
14474 Jun 30 15:15 /DATA/DISK1/BENCH/mrs/crates/mrs-bench/results/casc-30/20260630_134541/run.csv


---

## Older Results (May - June 2026)

*Note: The raw CASC benchmark output logs for runs from late May 2026 through the end of June 2026 have been removed from this file to reduce its size. The essential findings, including the ML investigation, are summarized in the "Status" and "ML-guided clause selection — investigation" sections above.*
