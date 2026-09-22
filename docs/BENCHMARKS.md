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

commit 12bc17efd34edfc2cd4b00d9e7ec9fc033a261fc (HEAD -> feat/completeness-witness-current, origin/feat/completeness-witness-current)

[ongoing]
[www@server99 mrs]$ ./crates/mrs-bench/remote-cert-campaign.sh all > remote-cert-campaign.out 2> remote-cert-campaign.err

commit 0c41daf2b484026c266d747f3705897b53c887e5 (HEAD -> main, origin/main, origin/HEAD)

[ongoing]
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 MRS_SHARED_POOL_INTERVAL=0 crates/mrs-bench/casc.sh --edition casc-30  --systems mrs --divisions fne,feq,epu,eps,ueq,icu  --casc-times --jobs 2 --output crates/mrs-bench/results/casc-30-W8J2-noshare-$(date +%Y%m%d)

[done]
[www@server99 mrs]$ MRS_WORKERS=8 MRS_SHARED_POOL_INTERVAL=2000 crates/mrs-bench/casc.sh --edition casc-j13 --systems mrs --divisions fne  --casc-times --jobs 2 --output crates/mrs-bench/results/casc-j13-fne-share2000-W8J2-$(date +%Y%m%d)
Running `target/debug/bench_report crates/mrs-bench/results/casc-j13-fne-share2000-W8J2-20260922/run.csv`
CASC-J13 Results — 2026-09-22 11:37  (100 problems × 1 systems)
===============================================================

Division  Problems    mrs
                 Solved  Avg (s)
------------------  --------------------
FNE            100        34   11.318
------------------  --------------------
TOTAL          100        34   11.318

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[ongoing]
[root@server04 mrs]# for interval in 2000; do MRS_WORKERS=8 MRS_SHARED_POOL_INTERVAL="${interval}" crates/mrs-bench/casc.sh --edition casc-j13 --systems mrs --divisions feq --casc-times --jobs 1 --output crates/mrs-bench/results/casc-j13-feq-sharing-${interval}-$(date +%Y%m%d_%H%M%S); done

[ongoing]
[root@server01 mrs]# MRS_WORKERS=8 MRS_SHARED_POOL_INTERVAL=2000 crates/mrs-bench/casc.sh --edition casc-30  --systems mrs --divisions feq  --casc-times --jobs 1 --output crates/mrs-bench/results/casc30-feq-sharing2000-W8J1-$(date +%Y%m%d)

[done]
[root@server01 mrs]# MRS_WORKERS=8 MRS_SHARED_POOL_INTERVAL=0 crates/mrs-bench/casc.sh --edition casc-30  --systems mrs --divisions ueq  --casc-times --jobs 1 --output crates/mrs-bench/results/casc30-ueq-noshare-W8J1-$(date +%Y%m%d)
Running `target/debug/bench_report crates/mrs-bench/results/casc30-ueq-noshare-W8J1-20260921/run.csv`
CASC-30 Results — 2026-09-22 07:15  (300 problems × 1 systems)
==============================================================

Division  Problems    mrs
                 Solved  Avg (s)
------------------  --------------------
UEQ            300       118   46.590
------------------  --------------------
TOTAL          300       118   46.590

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done]
[root@server01 mrs]# MRS_WORKERS=8 MRS_SHARED_POOL_INTERVAL=0 crates/mrs-bench/casc.sh --edition casc-30  --systems mrs --divisions ueq  --casc-times --jobs 2 --output crates/mrs-bench/results/casc30-ueq-noshare-W8J2-$(date +%Y%m%d)
Running `target/debug/bench_report crates/mrs-bench/results/casc30-ueq-noshare-W8J2-20260921/run.csv`
CASC-30 Results — 2026-09-21 16:11  (300 problems × 1 systems)
==============================================================

Division  Problems    mrs
                 Solved  Avg (s)
------------------  --------------------
UEQ            300       103   50.435
------------------  --------------------
TOTAL          300       103   50.435

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done]
[root@server04 mrs]# for interval in 2000; do MRS_WORKERS=8 MRS_SHARED_POOL_INTERVAL="${interval}" crates/mrs-bench/casc.sh --edition casc-j13 --systems mrs --divisions ueq --casc-times --jobs 1 --output crates/mrs-bench/results/casc-j13-ueq-sharing-${interval}-$(date +%Y%m%d_%H%M%S); done
Running `target/debug/bench_report crates/mrs-bench/results/casc-j13-ueq-sharing-2000-20260921_101723/run.csv`
CASC-J13 Results — 2026-09-22 07:13  (400 problems × 1 systems)
===============================================================

Division  Problems    mrs
                 Solved  Avg (s)
------------------  --------------------
UEQ            400       104   32.464
------------------  --------------------
TOTAL          400       104   32.464

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done]
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --edition casc-30  --systems mrs --divisions fne,feq,epu,eps,ueq,icu  --casc-times --jobs 2 --output crates/mrs-bench/results/casc-30-W8J2-$(date +%Y%m%d)
Running `target/debug/bench_report /DATA/ai/fr22192/mrs/crates/mrs-bench/results/casc-30-W8J2-20260921/run.csv`
CASC-30 Results — 2026-09-22 14:10  (1101 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        45   35.206
FEQ            400       107   23.869
EPU            100        18   13.725
EPS            100        16   17.726
UEQ            300        82   34.363
ICU            101         3   96.043
------------------  --------------------
TOTAL         1101       271   28.689

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[ongoing]
[PPROD:server@server9701:/DATA/ai/user/mrs]$ cargo run --release -p mrs-bench --bin audit_casc_proofs -- --run crates/mrs-bench/results/casc-30-W8J2-20260921 --problems-dir crates/mrs-bench/problems/casc-30 --checks strict,mrs,ladder --strict-time 60 --mrs-time 60 --ladder-time 60 --ladder-workers 8 --jobs 4 --output crates/mrs-bench/results/casc-30-W8J2-20260921/proof-audit

audit_report=crates/mrs-bench/results/casc-30-W8J2-20260921/proof-audit/audit.csv
summary_report=crates/mrs-bench/results/casc-30-W8J2-20260921/proof-audit/audit-summary.txt
checks=[strict,mrs,ladder]

================================================================================
Division: eps                                                          Rows: 100
================================================================================

Generation
+--------------------+-------+
|             Status | Count |
+--------------------+-------+
|            Theorem |     0 |
|      Unsatisfiable |     0 |
|        Satisfiable |    16 |
| CounterSatisfiable |     0 |
|             GaveUp |    35 |
|            Timeout |    44 |
|              Error |     0 |
|              Other |     0 |
+--------------------+-------+

Verification (Refutations)
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
|   Mode | Applicable | VerifiedGood | VerifiedBad | Unknown | Timeout | N/A: Model | N/A: Incomplete | Error | Other |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
| strict |          0 |            0 |           0 |       0 |       0 |         16 |              84 |     0 |     0 |
|    mrs |          0 |            0 |           0 |       0 |       0 |         16 |              84 |     0 |     0 |
| ladder |          0 |            0 |           0 |       0 |       0 |         16 |              84 |     0 |     0 |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+

Verification (Model Certificates)
+--------------+------------------+----------------+--------------------------+
| Total Models | Certified Models | Invalid Models | Uncertified (N/A: Model) |
+--------------+------------------+----------------+--------------------------+
|           16 |                0 |              0 |                       16 |
+--------------+------------------+----------------+--------------------------+

================================================================================
Division: epu                                                          Rows: 100
================================================================================

Generation
+--------------------+-------+
|             Status | Count |
+--------------------+-------+
|            Theorem |     0 |
|      Unsatisfiable |    18 |
|        Satisfiable |     0 |
| CounterSatisfiable |     0 |
|             GaveUp |    16 |
|            Timeout |    66 |
|              Error |     0 |
|              Other |     0 |
+--------------------+-------+

Verification (Refutations)
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
|   Mode | Applicable | VerifiedGood | VerifiedBad | Unknown | Timeout | N/A: Model | N/A: Incomplete | Error | Other |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
| strict |         18 |           17 |           0 |       1 |       0 |          0 |              82 |     0 |     0 |
|    mrs |         18 |           18 |           0 |       0 |       0 |          0 |              82 |     0 |     0 |
| ladder |         18 |           18 |           0 |       0 |       0 |          0 |              82 |     0 |     0 |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+

================================================================================
Division: feq                                                          Rows: 400
================================================================================

Generation
+--------------------+-------+
|             Status | Count |
+--------------------+-------+
|            Theorem |   107 |
|      Unsatisfiable |     0 |
|        Satisfiable |     0 |
| CounterSatisfiable |     0 |
|             GaveUp |   109 |
|            Timeout |   122 |
|              Error |     0 |
|              Other |     0 |
+--------------------+-------+

Verification (Refutations)
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
|   Mode | Applicable | VerifiedGood | VerifiedBad | Unknown | Timeout | N/A: Model | N/A: Incomplete | Error | Other |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
| strict |        107 |           60 |           1 |      45 |       1 |          0 |             293 |     0 |     0 |
|    mrs |        107 |           26 |           0 |      69 |      12 |          0 |             293 |     0 |     0 |
| ladder |        107 |           52 |           0 |      43 |      12 |          0 |             293 |     0 |     0 |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+

================================================================================
Division: fne                                                          Rows: 100
================================================================================

Generation
+--------------------+-------+
|             Status | Count |
+--------------------+-------+
|            Theorem |    45 |
|      Unsatisfiable |     0 |
|        Satisfiable |     0 |
| CounterSatisfiable |     0 |
|             GaveUp |     2 |
|            Timeout |    37 |
|              Error |     1 |
|              Other |     0 |
+--------------------+-------+

Verification (Refutations)
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
|   Mode | Applicable | VerifiedGood | VerifiedBad | Unknown | Timeout | N/A: Model | N/A: Incomplete | Error | Other |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
| strict |         45 |           18 |           1 |      26 |       0 |          0 |              54 |     1 |     0 |
|    mrs |         45 |           20 |           0 |      18 |       7 |          0 |              54 |     1 |     0 |
| ladder |         45 |           21 |           0 |      18 |       6 |          0 |              54 |     1 |     0 |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+

================================================================================
Division: icu                                                          Rows: 101
================================================================================

Generation
+--------------------+-------+
|             Status | Count |
+--------------------+-------+
|            Theorem |     3 |
|      Unsatisfiable |     0 |
|        Satisfiable |     0 |
| CounterSatisfiable |     0 |
|             GaveUp |     5 |
|            Timeout |    54 |
|              Error |     1 |
|              Other |     0 |
+--------------------+-------+

Verification (Refutations)
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
|   Mode | Applicable | VerifiedGood | VerifiedBad | Unknown | Timeout | N/A: Model | N/A: Incomplete | Error | Other |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
| strict |          3 |            1 |           0 |       2 |       0 |          0 |              97 |     1 |     0 |
|    mrs |          3 |            1 |           0 |       2 |       0 |          0 |              97 |     1 |     0 |
| ladder |          3 |            1 |           0 |       2 |       0 |          0 |              97 |     1 |     0 |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+

================================================================================
Division: ueq                                                          Rows: 300
================================================================================

Generation
+--------------------+-------+
|             Status | Count |
+--------------------+-------+
|            Theorem |     0 |
|      Unsatisfiable |    82 |
|        Satisfiable |     0 |
| CounterSatisfiable |     0 |
|             GaveUp |    14 |
|            Timeout |   140 |
|              Error |     0 |
|              Other |     0 |
+--------------------+-------+

Verification (Refutations)
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
|   Mode | Applicable | VerifiedGood | VerifiedBad | Unknown | Timeout | N/A: Model | N/A: Incomplete | Error | Other |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
| strict |         82 |           70 |           0 |      12 |       0 |          0 |             218 |     0 |     0 |
|    mrs |         82 |           33 |           0 |       7 |      42 |          0 |             218 |     0 |     0 |
| ladder |         82 |           37 |           0 |       1 |      44 |          0 |             218 |     0 |     0 |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+



Running `target/release/audit_casc_proofs --run crates/mrs-bench/results/casc-j13-W8J2-20260921 --problems-dir crates/mrs-bench/problems/casc-j13 --checks strict,mrs,ladder --strict-time 60 --mrs-time 60 --ladder-time 60 --ladder-workers 8 --jobs 4 --output crates/mrs-bench/results/casc-j13-W8J2-20260921/proof-audit`
audit_report=crates/mrs-bench/results/casc-j13-W8J2-20260921/proof-audit/audit.csv
summary_report=crates/mrs-bench/results/casc-j13-W8J2-20260921/proof-audit/audit-summary.txt
checks=[strict,mrs,ladder]

================================================================================
Division: feq                                                          Rows: 300
================================================================================

Generation
+--------------------+-------+
|             Status | Count |
+--------------------+-------+
|            Theorem |    68 |
|      Unsatisfiable |     0 |
|        Satisfiable |     0 |
| CounterSatisfiable |     0 |
|             GaveUp |    98 |
|            Timeout |   114 |
|              Error |     0 |
|              Other |     0 |
+--------------------+-------+

Verification (Refutations)
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
|   Mode | Applicable | VerifiedGood | VerifiedBad | Unknown | Timeout | N/A: Model | N/A: Incomplete | Error | Other |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
| strict |         68 |           46 |           6 |      16 |       0 |          0 |             232 |     0 |     0 |
|    mrs |         68 |           24 |           0 |      35 |       9 |          0 |             232 |     0 |     0 |
| ladder |         68 |           42 |           1 |      17 |       8 |          0 |             232 |     0 |     0 |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+

================================================================================
Division: fne                                                          Rows: 100
================================================================================

Generation
+--------------------+-------+
|             Status | Count |
+--------------------+-------+
|            Theorem |    34 |
|      Unsatisfiable |     0 |
|        Satisfiable |     0 |
| CounterSatisfiable |     0 |
|             GaveUp |     5 |
|            Timeout |    53 |
|              Error |     0 |
|              Other |     0 |
+--------------------+-------+

Verification (Refutations)
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
|   Mode | Applicable | VerifiedGood | VerifiedBad | Unknown | Timeout | N/A: Model | N/A: Incomplete | Error | Other |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
| strict |         34 |           12 |           0 |      22 |       0 |          0 |              66 |     0 |     0 |
|    mrs |         34 |           10 |           0 |      22 |       2 |          0 |              66 |     0 |     0 |
| ladder |         34 |           10 |           0 |      22 |       2 |          0 |              66 |     0 |     0 |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+

================================================================================
Division: ueq                                                          Rows: 400
================================================================================

Generation
+--------------------+-------+
|             Status | Count |
+--------------------+-------+
|            Theorem |     0 |
|      Unsatisfiable |    80 |
|        Satisfiable |     0 |
| CounterSatisfiable |     0 |
|             GaveUp |    24 |
|            Timeout |   247 |
|              Error |     0 |
|              Other |     0 |
+--------------------+-------+

Verification (Refutations)
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
|   Mode | Applicable | VerifiedGood | VerifiedBad | Unknown | Timeout | N/A: Model | N/A: Incomplete | Error | Other |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
| strict |         80 |           71 |           0 |       9 |       0 |          0 |             320 |     0 |     0 |
|    mrs |         80 |           37 |           0 |       0 |      43 |          0 |             320 |     0 |     0 |
| ladder |         80 |           37 |           0 |       0 |      43 |          0 |             320 |     0 |     0 |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+

[done]
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --edition casc-j13 --systems mrs --divisions fne,feq,ueq  --casc-times --jobs 2 --output crates/mrs-bench/results/casc-j13-W8J2-$(date +%Y%m%d)
Running `target/debug/bench_report crates/mrs-bench/results/casc-j13-W8J2-20260921/run.csv`
CASC-J13 Results — 2026-09-22 07:04  (800 problems × 1 systems)
===============================================================

Division  Problems    mrs
                 Solved  Avg (s)
------------------  --------------------
FNE            100        34   11.397
FEQ            300        68   18.866
UEQ            400        80   18.960
------------------  --------------------
TOTAL          800       182   17.512

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[ongoing]
[www@server99 mrs]$ cargo run --release -p mrs-bench --bin audit_casc_proofs -- --run crates/mrs-bench/results/casc-j13-W8J2-20260921 --problems-dir crates/mrs-bench/problems/casc-j13 --checks strict,mrs,ladd
er --strict-time 60 --mrs-time 60 --ladder-time 60 --ladder-workers 8 --jobs 4 --output crates/mrs-bench/results/casc-j13-W8J2-20260921/proof-audit

commit e3b1cd978a8e7306ccbeffd462d4c6659fa26e2f (HEAD -> main, origin/main, origin/HEAD)

[done]
[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ MRS_WORKERS=8 crates/mrs-bench/cooperative_portfolio_sweep.sh casc-j13 feq 11,12,8,6,5,7,1,4 240 1 crates/mrs-bench/results/coop-cascj13-feq-candidate
Running `target/debug/bench_report crates/mrs-bench/results/coop-cascj13-feq-candidate/run.csv`
CASC-J13 Results — 2026-09-21 08:02  (300 problems × 1 systems)
===============================================================

Division  Problems    mrs
                 Solved  Avg (s)
------------------  --------------------
FEQ            300        70   19.774
------------------  --------------------
TOTAL          300        70   19.774

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done]
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/cooperative_portfolio_sweep.sh casc-j13 ueq 11,4,12,5,1,8,2,15 240 2 crates/mrs-bench/results/coop-cascj13-ueq-candidat
Running `target/debug/bench_report crates/mrs-bench/results/coop-cascj13-ueq-candidat/run.csv`
CASC-J13 Results — 2026-09-21 07:54  (400 problems × 1 systems)
===============================================================

Division  Problems    mrs
                 Solved  Avg (s)
------------------  --------------------
UEQ            400        72   24.068
------------------  --------------------
TOTAL          400        72   24.068

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done]
[root@server01 mrs]# MRS_WORKERS=8 crates/mrs-bench/cooperative_portfolio_sweep.sh casc-30 feq 11,12,8,6,5,7,1,4 240 1 crates/mrs-bench/results/coop-casc30-feq-candidate
Running `target/debug/bench_report crates/mrs-bench/results/coop-casc30-feq-candidate/run.csv`
CASC-30 Results — 2026-09-21 08:21  (400 problems × 1 systems)
==============================================================

Division  Problems    mrs
                 Solved  Avg (s)
------------------  --------------------
FEQ            400       102   25.316
------------------  --------------------
TOTAL          400       102   25.316

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done]
[root@mtsdev04 mrs]# MRS_WORKERS=8 crates/mrs-bench/cooperative_portfolio_sweep.sh casc-30 ueq 11,4,12,5,1,8,2,15 240 1 crates/mrs-bench/results/coop-casc30-ueq-candidat
Running `target/debug/bench_report crates/mrs-bench/results/coop-casc30-ueq-candidate/run.csv`
CASC-30 Results — 2026-09-21 08:11  (300 problems × 1 systems)
==============================================================

Division  Problems    mrs
                 Solved  Avg (s)
------------------  --------------------
UEQ            300        69   31.751
------------------  --------------------
TOTAL          300        69   31.751

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done]
[root@server01 mrs]# MRS_WORKERS=8 crates/mrs-bench/cooperative_portfolio_sweep.sh casc-j13 fne 11,4,8,1,7,3,12,2 180 1  crates/mrs-bench/results/coop-cascj13-fne-candidate
Running `target/debug/bench_report crates/mrs-bench/results/coop-cascj13-fne-candidate/run.csv`
CASC-J13 Results — 2026-09-18 11:49  (100 problems × 1 systems)
===============================================================

Division  Problems    mrs
                 Solved  Avg (s)
------------------  --------------------
FNE            100        34   17.642
------------------  --------------------
TOTAL          100        34   17.642

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done]
[root@server04 mrs]# MRS_WORKERS=8 crates/mrs-bench/cooperative_portfolio_sweep.sh casc-30 epu 1,2,3,4,7,8,12,15 120 1  crates/mrs-bench/results/coop-casc30-epu-candidate
Running `target/debug/bench_report crates/mrs-bench/results/coop-casc30-epu-candidate/run.csv`
CASC-30 Results — 2026-09-18 11:44  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                 Solved  Avg (s)
------------------  --------------------
EPU            100        19   12.943
------------------  --------------------
TOTAL          100        19   12.943

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.


[done]
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --edition casc-30  --systems mrs --divisions fne,feq,epu,eps,ueq,icu  --casc-times --jobs 2 --output crates/mrs-bench/results/casc-30-W8J2-$(date +%Y%m%d
Running crates/mrs-bench/results/casc-30-W8J2-20260917/run.csv`
CASC-30 Results — 2026-09-18 16:10  (1101 problems × 1 systems)
===============================================================

Division  Problems    mrs
                 Solved  Avg (s)
------------------  --------------------
FNE            100        44   31.625
FEQ            400       107   24.791
EPU            100        18   15.706
EPS            100        16   18.198
UEQ            300        81   28.114
ICU            101         3   89.908
------------------  --------------------
TOTAL         1101       269   26.636

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.



[done]
[PPROD:user@server97:/DATA/ai/user/mrs]$ cargo run --release -p mrs-bench --bin audit_casc_proofs -- --run crates/mrs-bench/results/casc-30-W8J2-20260917 --problems-dir crates/mrs-bench/problems/casc-30 --checks strict,mrs,ladder --strict-time 60 --mrs-time 60 --ladder-time 60 --ladder-workers 8 --jobs 4 --output crates/mrs-bench/results/casc-30-W8J2-20260917/proof-audit

[PPROD:user@server97:/DATA/ai/user/mrs]$ cargo run --release -p mrs-bench --bin audit_casc_proofs -- --run crates/mrs-bench/results/casc-30-W8J2-20260917 --problems-dir crates/mrs-bench/problems/casc-30 --checks strict,mrs,ladder --strict-time 60 --mrs-time 60 --ladder-time 60 --ladder-workers 8 --jobs 4 --output crates/mrs-bench/results/casc-30-W8J2-20260917/proof-audit
     Running `target/release/audit_casc_proofs --run crates/mrs-bench/results/casc-30-W8J2-20260917 --problems-dir crates/mrs-bench/problems/casc-30 --checks strict,mrs,ladder --strict-time 60 --mrs-time 60 --ladder-time 60 --ladder-workers 8 --jobs 4 --output crates/mrs-bench/results/casc-30-W8J2-20260917/proof-audit`
audit_report=crates/mrs-bench/results/casc-30-W8J2-20260917/proof-audit/audit.csv
summary_report=crates/mrs-bench/results/casc-30-W8J2-20260917/proof-audit/audit-summary.txt
checks=[strict,mrs,ladder]

================================================================================
Division: eps                                                          Rows: 100
================================================================================

Generation
+--------------------+-------+
|             Status | Count |
+--------------------+-------+
|            Theorem |     0 |
|      Unsatisfiable |     0 |
|        Satisfiable |    16 |
| CounterSatisfiable |     0 |
|             GaveUp |    35 |
|            Timeout |    44 |
|              Error |     0 |
|              Other |     0 |
+--------------------+-------+

Verification (Refutations)
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
|   Mode | Applicable | VerifiedGood | VerifiedBad | Unknown | Timeout | N/A: Model | N/A: Incomplete | Error | Other |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
| strict |          0 |            0 |           0 |       0 |       0 |         16 |              84 |     0 |     0 |
|    mrs |          0 |            0 |           0 |       0 |       0 |         16 |              84 |     0 |     0 |
| ladder |          0 |            0 |           0 |       0 |       0 |         16 |              84 |     0 |     0 |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+

Verification (Model Certificates)
+--------------+------------------+----------------+--------------------------+
| Total Models | Certified Models | Invalid Models | Uncertified (N/A: Model) |
+--------------+------------------+----------------+--------------------------+
|           16 |                0 |              0 |                       16 |
+--------------+------------------+----------------+--------------------------+

================================================================================
Division: epu                                                          Rows: 100
================================================================================

Generation
+--------------------+-------+
|             Status | Count |
+--------------------+-------+
|            Theorem |     0 |
|      Unsatisfiable |    18 |
|        Satisfiable |     0 |
| CounterSatisfiable |     0 |
|             GaveUp |    16 |
|            Timeout |    66 |
|              Error |     0 |
|              Other |     0 |
+--------------------+-------+

Verification (Refutations)
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
|   Mode | Applicable | VerifiedGood | VerifiedBad | Unknown | Timeout | N/A: Model | N/A: Incomplete | Error | Other |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
| strict |         18 |           17 |           0 |       1 |       0 |          0 |              82 |     0 |     0 |
|    mrs |         18 |           18 |           0 |       0 |       0 |          0 |              82 |     0 |     0 |
| ladder |         18 |           18 |           0 |       0 |       0 |          0 |              82 |     0 |     0 |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+

================================================================================
Division: feq                                                          Rows: 400
================================================================================

Generation
+--------------------+-------+
|             Status | Count |
+--------------------+-------+
|            Theorem |   107 |
|      Unsatisfiable |     0 |
|        Satisfiable |     0 |
| CounterSatisfiable |     0 |
|             GaveUp |   106 |
|            Timeout |   124 |
|              Error |     0 |
|              Other |     0 |
+--------------------+-------+

Verification (Refutations)
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
|   Mode | Applicable | VerifiedGood | VerifiedBad | Unknown | Timeout | N/A: Model | N/A: Incomplete | Error | Other |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
| strict |        107 |           28 |          41 |      37 |       1 |          0 |             293 |     0 |     0 |
|    mrs |        107 |           22 |           0 |      83 |       2 |          0 |             293 |     0 |     0 |
| ladder |        107 |           57 |           0 |      48 |       2 |          0 |             293 |     0 |     0 |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+

================================================================================
Division: fne                                                          Rows: 100
================================================================================

Generation
+--------------------+-------+
|             Status | Count |
+--------------------+-------+
|            Theorem |    44 |
|      Unsatisfiable |     0 |
|        Satisfiable |     0 |
| CounterSatisfiable |     0 |
|             GaveUp |     2 |
|            Timeout |    36 |
|              Error |     0 |
|              Other |     0 |
+--------------------+-------+

Verification (Refutations)
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
|   Mode | Applicable | VerifiedGood | VerifiedBad | Unknown | Timeout | N/A: Model | N/A: Incomplete | Error | Other |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
| strict |         44 |           10 |          12 |      22 |       0 |          0 |              56 |     0 |     0 |
|    mrs |         44 |           19 |           0 |      15 |      10 |          0 |              56 |     0 |     0 |
| ladder |         44 |           20 |           0 |      15 |       9 |          0 |              56 |     0 |     0 |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+

================================================================================
Division: icu                                                          Rows: 101
================================================================================

Generation
+--------------------+-------+
|             Status | Count |
+--------------------+-------+
|            Theorem |     3 |
|      Unsatisfiable |     0 |
|        Satisfiable |     0 |
| CounterSatisfiable |     0 |
|             GaveUp |     5 |
|            Timeout |    53 |
|              Error |     1 |
|              Other |     0 |
+--------------------+-------+

Verification (Refutations)
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
|   Mode | Applicable | VerifiedGood | VerifiedBad | Unknown | Timeout | N/A: Model | N/A: Incomplete | Error | Other |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
| strict |          3 |            0 |           1 |       2 |       0 |          0 |              97 |     1 |     0 |
|    mrs |          3 |            1 |           0 |       2 |       0 |          0 |              97 |     1 |     0 |
| ladder |          3 |            2 |           0 |       1 |       0 |          0 |              97 |     1 |     0 |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+

================================================================================
Division: ueq                                                          Rows: 300
================================================================================

Generation
+--------------------+-------+
|             Status | Count |
+--------------------+-------+
|            Theorem |     0 |
|      Unsatisfiable |    81 |
|        Satisfiable |     0 |
| CounterSatisfiable |     0 |
|             GaveUp |    11 |
|            Timeout |   146 |
|              Error |     0 |
|              Other |     0 |
+--------------------+-------+

Verification (Refutations)
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
|   Mode | Applicable | VerifiedGood | VerifiedBad | Unknown | Timeout | N/A: Model | N/A: Incomplete | Error | Other |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
| strict |         81 |           70 |           0 |      11 |       0 |          0 |             219 |     0 |     0 |
|    mrs |         81 |           45 |           0 |      29 |       7 |          0 |             219 |     0 |     0 |
| ladder |         81 |           75 |           0 |       0 |       6 |          0 |             219 |     0 |     0 |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+

[done]
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --edition casc-j13 --systems mrs --divisions fne,feq,ueq  --casc-times --jobs 2 --output crates/mrs-bench/results/casc-j13-W8J2-$(date +%Y%m%d)
Running `target/debug/bench_report crates/mrs-bench/results/casc-j13-W8J2-20260917/run.csv`
CASC-J13 Results — 2026-09-18 06:08  (800 problems × 1 systems)
===============================================================

Division  Problems    mrs
                 Solved  Avg (s)
------------------  --------------------
FNE            100        34   11.401
FEQ            300        68   19.716
UEQ            400        80   20.220
------------------  --------------------
TOTAL          800       182   18.384

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[ongoing]
[www@server99 mrs]$ cargo run --release -p mrs-bench --bin audit_casc_proofs -- --run crates/mrs-bench/results/casc-j13-W8J2-20260917 --problems-dir crates/mrs-bench/problems/casc-j13 --checks strict,mrs,ladder --strict-time 60 --mrs-time 60 --ladder-time 60 --ladder-workers 8 --jobs 4 --output crates/mrs-bench/results/casc-j13-W8J2-20260917/proof-audit

commit 4f7bd15d31ec2814bac2372f4475f243fee3552f (HEAD -> main, origin/main, origin/HEAD)

[done]
[www@server99 mrs]$ crates/mrs-bench/run_strategy_sweep.sh --edition casc-30 --divisions fne,feq,epu,eps,ueq,icu --casc-times --jobs 30 --output crates/mrs-bench/results/casc-30-sweep-$(date +%Y%m%d
Running `target/debug/bench_report crates/mrs-bench/results/casc-30-sweep-20260915/run.csv`
CASC-30 Results — 2026-09-17 17:03  (1101 problems × 15 systems)
================================================================

Division  Problems    mrs-s01               mrs-s02               mrs-s03               mrs-s04               mrs-s05               mrs-s06               mrs-s07               mrs-s08               mrs-s09               mrs-s10               mrs-s11               mrs-s12               mrs-s13               mrs-s14               mrs-s15
                 Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------
FNE            100        32   58.120          21   17.266          25   25.108          30   43.738          11   35.769          15   59.723          32   62.714          33   28.903          27   37.606           0    0.000          32   34.500          26   51.078           0    0.000           5   55.231          19   32.481
FEQ            400        45   44.944          16    4.428          16   23.581          39   34.764          40   28.717          44   35.883          42   37.998          52   23.798          17   33.146          21   32.382          56   26.819          63   34.378          21   47.280          19   20.029          18   13.208
EPU            100        15   18.639          16   16.191          14   13.176          15   16.470           9    9.956           9   30.255          14   17.287          15   18.116           7    5.232           5    0.073          16   19.422          16   17.696           5    0.079           6    0.084          15    8.800
EPS            100        15   21.489          15   18.510          15   24.057           2    0.092           2    0.096           3    0.084          11   16.156          11    8.932           8   18.649           2    0.095           2    0.100           2    0.097           2    0.110           2    0.102           2    0.098
UEQ            300        41   75.579          35   65.231          33   57.523          75   77.462          57   79.078          41   55.662          13   53.585          32   64.313           4  103.588           3   41.967          58   70.723          74   71.348           4    1.538          34   41.247          32   43.113
ICU            101         2  161.604           2  161.440           2  139.793           1  212.070           2   64.528           2  206.837           1  213.347           1  435.130           1  260.171           0    0.000           1    9.531           2  105.832           0    0.000           1   44.657           2  247.619
------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------
TOTAL         1101       150   52.708         105   34.058         105   35.506         162   55.166         121   51.805         114   47.746         113   43.650         144   35.100          64   38.112          31   26.016         165   42.596         183   50.648          32   31.239          67   31.410          88   32.522

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

 [done]
 [root@mtsdev04 mrs]# for interval in 250 500 1000; do MRS_WORKERS=8 MRS_SHARED_POOL_INTERVAL="${interval}" crates/mrs-bench/casc.sh --edition casc-j13 --systems mrs --divisions ueq --casc-times --jobs 1 --output crates/mrs-bench/results/casc-j13-ueq-sharing-${interval}-$(date +%Y%m%d_%H%M%S); done

      Running `target/debug/bench_report crates/mrs-bench/results/casc-j13-ueq-sharing-250-20260915_194745/run.csv`
 CASC-J13 Results — 2026-09-18 06:23  (400 problems × 1 systems)
 ===============================================================
 
 Division  Problems    mrs
                       Solved  Avg (s)
 ------------------  --------------------
 UEQ            400        71   30.633
 ------------------  --------------------
 TOTAL          400        71   30.633
 
 DISAGREEMENTS — none detected.
 
 POLARITY VIOLATIONS — none detected.
 
 REFERENCE VIOLATIONS — none detected.

      Running `target/debug/bench_report crates/mrs-bench/results/casc-j13-ueq-sharing-500-20260916_130019/run.csv`
 CASC-J13 Results — 2026-09-18 06:23  (400 problems × 1 systems)
 ===============================================================
 
 Division  Problems    mrs
                       Solved  Avg (s)
 ------------------  --------------------
 UEQ            400        81   21.866
 ------------------  --------------------
 TOTAL          400        81   21.866
 
 DISAGREEMENTS — none detected.
 
 POLARITY VIOLATIONS — none detected.
 
 REFERENCE VIOLATIONS — none detected.
 
      Running `target/debug/bench_report crates/mrs-bench/results/casc-j13-ueq-sharing-1000-20260917_053552/run.csv`
 CASC-J13 Results — 2026-09-18 06:24  (400 problems × 1 systems)
 ===============================================================
 
 Division  Problems    mrs
                       Solved  Avg (s)
 ------------------  --------------------
 UEQ            400        95   26.195
 ------------------  --------------------
 TOTAL          400        95   26.195
 
 DISAGREEMENTS — none detected.
 
 POLARITY VIOLATIONS — none detected.
 
 REFERENCE VIOLATIONS — none detected.

[done]
[root@mtsdev01 mrs]# for interval in 25 50 100; do MRS_WORKERS=8 MRS_SHARED_POOL_INTERVAL="${interval}" crates/mrs-bench/casc.sh --edition casc-j13 --systems mrs --divisions ueq --casc-times --jobs 1 --output crates/mrs-bench/results/casc-j13-ueq-sharing-${interval}-$(date +%Y%m%d_%H%M%S); done

     Running `target/debug/bench_report crates/mrs-bench/results/casc-j13-ueq-sharing-25-20260915_194417/run.csv`
CASC-J13 Results — 2026-09-18 06:58  (400 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
UEQ            400        49   67.023
------------------  --------------------
TOTAL          400        49   67.023

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

     Running `target/debug/bench_report crates/mrs-bench/results/casc-j13-ueq-sharing-50-20260916_144004/run.csv`
CASC-J13 Results — 2026-09-18 07:00  (400 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
UEQ            400        61   52.607
------------------  --------------------
TOTAL          400        61   52.607

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

     Running `target/debug/bench_report crates/mrs-bench/results/casc-j13-ueq-sharing-100-20260917_085047/run.csv`
CASC-J13 Results — 2026-09-18 07:00  (400 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
UEQ            400        62   40.798
------------------  --------------------
TOTAL          400        62   40.798

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done]
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --edition casc-30  --systems mrs --divisions fne,feq,epu,eps,ueq,icu  --casc-times --jobs 2 --output crates/mrs-bench/results/casc-30-W8J2-$(date +%Y%m%d)
Running `target/debug/bench_report /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30-W8J2-20260914/run.csv`
CASC-30 Results — 2026-09-16 07:13  (1101 problems × 1 systems)
===============================================================

Division  Problems    mrs
                 Solved  Avg (s)
------------------  --------------------
FNE            100        43   27.309
FEQ            400       113   24.132
EPU            100        18   12.052
EPS            100        17   23.478
UEQ            300        81   30.882
ICU            101         4  147.508
------------------  --------------------
TOTAL         1101       276   27.568

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done]
[PPROD:user@server97:/DATA/ai/user/mrs]$ cargo run --release -p mrs-bench --bin audit_casc_proofs -- --run crates/mrs-bench/results/casc-30-W8J2-20260914 --problems-dir crates/mrs-bench/problems/casc-30 --checks strict,mrs,ladder --strict-time 60 --mrs-time 60 --ladder-time 60 --ladder-workers 8 --jobs 4 --output crates/mrs-bench/results/casc-30-W8J2-20260914/proof-audit
     Running `target/release/audit_casc_proofs --run crates/mrs-bench/results/casc-30-W8J2-20260914 --problems-dir crates/mrs-bench/problems/casc-30 --checks strict,mrs,ladder --strict-time 60 --mrs-time 60 --ladder-time 60 --ladder-workers 8 --jobs 4 --output crates/mrs-bench/results/casc-30-W8J2-20260914/proof-audit`
audit_report=crates/mrs-bench/results/casc-30-W8J2-20260914/proof-audit/audit.csv
summary_report=crates/mrs-bench/results/casc-30-W8J2-20260914/proof-audit/audit-summary.txt
checks=[strict,mrs,ladder]

================================================================================
Division: eps                                                          Rows: 100
================================================================================

Generation
+--------------------+-------+
|             Status | Count |
+--------------------+-------+
|            Theorem |     0 |
|      Unsatisfiable |     0 |
|        Satisfiable |    17 |
| CounterSatisfiable |     0 |
|             GaveUp |    63 |
|            Timeout |    20 |
|              Error |     0 |
|              Other |     0 |
+--------------------+-------+

Verification
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
|   Mode | Applicable | VerifiedGood | VerifiedBad | Unknown | Timeout | N/A: Model | N/A: Incomplete | Error | Other |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
| strict |          0 |            0 |           0 |       0 |       0 |         17 |              83 |     0 |     0 |
|    mrs |          0 |            0 |           0 |       0 |       0 |         17 |              83 |     0 |     0 |
| ladder |          0 |            0 |           0 |       0 |       0 |         17 |              83 |     0 |     0 |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+

================================================================================
Division: epu                                                          Rows: 100
================================================================================

Generation
+--------------------+-------+
|             Status | Count |
+--------------------+-------+
|            Theorem |     0 |
|      Unsatisfiable |    18 |
|        Satisfiable |     0 |
| CounterSatisfiable |     0 |
|             GaveUp |    26 |
|            Timeout |    56 |
|              Error |     0 |
|              Other |     0 |
+--------------------+-------+

Verification
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
|   Mode | Applicable | VerifiedGood | VerifiedBad | Unknown | Timeout | N/A: Model | N/A: Incomplete | Error | Other |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
| strict |         18 |           17 |           0 |       1 |       0 |          0 |              82 |     0 |     0 |
|    mrs |         18 |           18 |           0 |       0 |       0 |          0 |              82 |     0 |     0 |
| ladder |         18 |           18 |           0 |       0 |       0 |          0 |              82 |     0 |     0 |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+

================================================================================
Division: feq                                                          Rows: 400
================================================================================

Generation
+--------------------+-------+
|             Status | Count |
+--------------------+-------+
|            Theorem |   113 |
|      Unsatisfiable |     0 |
|        Satisfiable |     0 |
| CounterSatisfiable |     0 |
|             GaveUp |   252 |
|            Timeout |    35 |
|              Error |     0 |
|              Other |     0 |
+--------------------+-------+

Verification
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
|   Mode | Applicable | VerifiedGood | VerifiedBad | Unknown | Timeout | N/A: Model | N/A: Incomplete | Error | Other |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
| strict |        113 |           15 |          64 |      33 |       1 |          0 |             287 |     0 |     0 |
|    mrs |        113 |           23 |           4 |      77 |       9 |          0 |             287 |     0 |     0 |
| ladder |        113 |           53 |           6 |      45 |       9 |          0 |             287 |     0 |     0 |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+

================================================================================
Division: fne                                                          Rows: 100
================================================================================

Generation
+--------------------+-------+
|             Status | Count |
+--------------------+-------+
|            Theorem |    43 |
|      Unsatisfiable |     0 |
|        Satisfiable |     0 |
| CounterSatisfiable |     0 |
|             GaveUp |    52 |
|            Timeout |     5 |
|              Error |     0 |
|              Other |     0 |
+--------------------+-------+

Verification
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
|   Mode | Applicable | VerifiedGood | VerifiedBad | Unknown | Timeout | N/A: Model | N/A: Incomplete | Error | Other |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
| strict |         43 |            9 |          24 |      10 |       0 |          0 |              57 |     0 |     0 |
|    mrs |         43 |           18 |           0 |      15 |      10 |          0 |              57 |     0 |     0 |
| ladder |         43 |           19 |           0 |      16 |       8 |          0 |              57 |     0 |     0 |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+

================================================================================
Division: icu                                                          Rows: 101
================================================================================

Generation
+--------------------+-------+
|             Status | Count |
+--------------------+-------+
|            Theorem |     4 |
|      Unsatisfiable |     0 |
|        Satisfiable |     0 |
| CounterSatisfiable |     0 |
|             GaveUp |    73 |
|            Timeout |    19 |
|              Error |     5 |
|              Other |     0 |
+--------------------+-------+

Verification
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
|   Mode | Applicable | VerifiedGood | VerifiedBad | Unknown | Timeout | N/A: Model | N/A: Incomplete | Error | Other |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
| strict |          4 |            0 |           3 |       1 |       0 |          0 |              92 |     5 |     0 |
|    mrs |          4 |            1 |           0 |       2 |       1 |          0 |              92 |     5 |     0 |
| ladder |          4 |            2 |           0 |       1 |       1 |          0 |              92 |     5 |     0 |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+

================================================================================
Division: ueq                                                          Rows: 300
================================================================================

Generation
+--------------------+-------+
|             Status | Count |
+--------------------+-------+
|            Theorem |     0 |
|      Unsatisfiable |    81 |
|        Satisfiable |     0 |
| CounterSatisfiable |     0 |
|             GaveUp |   191 |
|            Timeout |    28 |
|              Error |     0 |
|              Other |     0 |
+--------------------+-------+

Verification
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
|   Mode | Applicable | VerifiedGood | VerifiedBad | Unknown | Timeout | N/A: Model | N/A: Incomplete | Error | Other |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
| strict |         81 |           32 |           0 |      49 |       0 |          0 |             219 |     0 |     0 |
|    mrs |         81 |           48 |           0 |      27 |       6 |          0 |             219 |     0 |     0 |
| ladder |         81 |           75 |           0 |       0 |       6 |          0 |             219 |     0 |     0 |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+


[done]
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --edition casc-j13 --systems mrs --divisions fne,feq,ueq  --casc-times --jobs 2 --output crates/mrs-bench/results/casc-j13-W8J2-$(date +%Y%m%d)
     Running `target/debug/bench_report crates/mrs-bench/results/casc-j13-W8J2-20260914/run.csv`
CASC-J13 Results — 2026-09-15 11:39  (800 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        35   12.877
FEQ            300        72   19.004
UEQ            400        78   17.773
------------------  --------------------
TOTAL          800       185   17.326

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done]
[www@server99 mrs]$ cargo run --release -p mrs-bench --bin audit_casc_proofs -- --run crates/mrs-bench/results/casc-j13-W8J2-20260914 --problems-dir crates/mrs-bench/problems/casc-j13 --checks strict,mrs,ladder --strict-time 60 --mrs-time 60 --ladder-time 60 --ladder-workers 8 --jobs 4 --output crates/mrs-bench/results/casc-j13-W8J2-20260914/proof-audit
[www@server99 mrs]$ cat crates/mrs-bench/results/casc-j13-W8J2-20260914/proof-audit/audit-summary.txt
audit_report=crates/mrs-bench/results/casc-j13-W8J2-20260914/proof-audit/audit.csv
summary_report=crates/mrs-bench/results/casc-j13-W8J2-20260914/proof-audit/audit-summary.txt
checks=[strict,mrs,ladder]

================================================================================
Division: feq                                                          Rows: 300
================================================================================

Generation
+--------------------+-------+
|             Status | Count |
+--------------------+-------+
|            Theorem |    72 |
|      Unsatisfiable |     0 |
|        Satisfiable |     0 |
| CounterSatisfiable |     0 |
|             GaveUp |   179 |
|            Timeout |    49 |
|              Error |     0 |
|              Other |     0 |
+--------------------+-------+

Verification
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
|   Mode | Applicable | VerifiedGood | VerifiedBad | Unknown | Timeout | N/A: Model | N/A: Incomplete | Error | Other |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
| strict |         72 |           16 |          30 |      26 |       0 |          0 |             228 |     0 |     0 |
|    mrs |         72 |           23 |           2 |      41 |       6 |          0 |             228 |     0 |     0 |
| ladder |         72 |           46 |           4 |      15 |       7 |          0 |             228 |     0 |     0 |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+

================================================================================
Division: fne                                                          Rows: 100
================================================================================

Generation
+--------------------+-------+
|             Status | Count |
+--------------------+-------+
|            Theorem |    35 |
|      Unsatisfiable |     0 |
|        Satisfiable |     0 |
| CounterSatisfiable |     0 |
|             GaveUp |    58 |
|            Timeout |     7 |
|              Error |     0 |
|              Other |     0 |
+--------------------+-------+

Verification
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
|   Mode | Applicable | VerifiedGood | VerifiedBad | Unknown | Timeout | N/A: Model | N/A: Incomplete | Error | Other |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
| strict |         35 |            5 |          21 |       9 |       0 |          0 |              65 |     0 |     0 |
|    mrs |         35 |           10 |           0 |      21 |       4 |          0 |              65 |     0 |     0 |
| ladder |         35 |           10 |           0 |      23 |       2 |          0 |              65 |     0 |     0 |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+

================================================================================
Division: ueq                                                          Rows: 400
================================================================================

Generation
+--------------------+-------+
|             Status | Count |
+--------------------+-------+
|            Theorem |     0 |
|      Unsatisfiable |    78 |
|        Satisfiable |     0 |
| CounterSatisfiable |     0 |
|             GaveUp |   289 |
|            Timeout |    33 |
|              Error |     0 |
|              Other |     0 |
+--------------------+-------+

Verification
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
|   Mode | Applicable | VerifiedGood | VerifiedBad | Unknown | Timeout | N/A: Model | N/A: Incomplete | Error | Other |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+
| strict |         78 |           41 |           0 |      37 |       0 |          0 |             322 |     0 |     0 |
|    mrs |         78 |           40 |           0 |      24 |      14 |          0 |             322 |     0 |     0 |
| ladder |         78 |           64 |           0 |       0 |      14 |          0 |             322 |     0 |     0 |
+--------+------------+--------------+-------------+---------+---------+------------+-----------------+-------+-------+

commit 11e36a05958984e0fe9b5118223fd57ba7ffec2e (HEAD -> main, origin/main, origin/HEAD)

[done]
[root@server04 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --edition casc-30 --divisions fne --systems mrs --jobs 1 --casc-times
Running `target/debug/bench_report crates/mrs-bench/results/casc-30/20260914_112303/run.csv`
CASC-30 Results — 2026-09-14 16:22  (100 problems × 1 systems)
==============================================================
Division  Problems    mrs
                 Solved  Avg (s)
------------------  --------------------
FNE            100        43   31.267
------------------  --------------------
TOTAL          100        43   31.267

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[root@server04 mrs]# cargo run --release -p mrs-bench --bin audit_casc_proofs -- --run crates/mrs-bench/results/casc-30/20260914_112303 --problems-dir crates/mrs-bench/problems/casc-30 --checks strict,mrs,ladder --strict-time 60 --mrs-time 60 --ladder-time 60 --ladder-workers 8 --jobs 1 --output crates/mrs-bench/results/casc-30/20260914_112303/proof-audit
strict
  Unknown=10
  VerifiedBad=24
  VerifiedGood=9
  non_refutation=57
mrs
  Timeout=9
  Unknown=15
  VerifiedGood=19
  non_refutation=57
ladder
  Timeout=7
  Unknown=16
  VerifiedGood=20
  non_refutation=57

[root@server01 mrs]# MRS_WORKERS=8 crates/mrs-bench/casc.sh --edition casc-30 --divisions eps --systems mrs --jobs 1 --casc-times

MRS_WORKERS=8 crates/mrs-bench/casc.sh --systems mrs --divisions eps  --casc-times --jobs 1
     Running `target/debug/bench_report /mnt/sdf1/mrs/crates/mrs-bench/results/casc-30/20260914_122604/run.csv`
CASC-30 Results — 2026-09-14 16:52  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPS            100        16   19.308
------------------  --------------------
TOTAL          100        16   19.308

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[root@mtsdev01 mrs]# cargo run --release -p mrs-bench --bin audit_casc_proofs -- --run crates/mrs-bench/results/casc-30/20260914_122604 --problems-dir crates/mrs-bench/problems/casc-30 --checks strict,mrs,ladder --strict-time 60 --mrs-time 60 --ladder-time 60 --ladder-workers 8 --jobs 1 --output crates/mrs-bench/results/casc-30/20260914_122604/proof-audit
checks=strict,mrs,ladder
strict
  non_refutation=100
mrs
  non_refutation=100
ladder
  non_refutation=100

commit f0558694c5e1df15df6013d619dfc5ed339a8862 (HEAD -> main, origin/main, origin/HEAD)

[done]
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh   --edition casc-j13 --systems mrs --divisions ueq  --casc-times --jobs 2 --output crates/mrs-bench/results/casc-j13-W8J1-$(date +%Y%m%d)
Running `target/debug/bench_report crates/mrs-bench/results/casc-j13-W8J2-20260914/run.csv`
CASC-J13 Results — 2026-09-14 16:11  (400 problems × 1 systems)
===============================================================

Division  Problems    mrs
                 Solved  Avg (s)
------------------  --------------------
UEQ            400        80   22.653
------------------  --------------------
TOTAL          400        80   22.653

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.


[done]
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh --edition casc-30  --systems mrs --divisions ueq  --casc-times --jobs 2 --output crates/mrs-bench/results/casc-30-W8J2-$(date +%Y%m%d)
Running `target/debug/bench_report /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30-W8J2-20260914/run.csv`
CASC-30 Results — 2026-09-14 15:40  (300 problems × 1 systems)
==============================================================

Division  Problems    mrs
                 Solved  Avg (s)
------------------  --------------------
UEQ            300        82   27.348
------------------  --------------------
TOTAL          300        82   27.348

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

commit 50f37f86ab3353d893b3e6388b090930523dbde2 (HEAD -> main, origin/main, origin/HEAD)

[done]
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh   --edition casc-j13 --systems mrs --divisions ueq  --casc-times --jobs 2 --output crates/mrs-bench/results/casc-j13-W8J1-$(date +%Y%m%d)
Running `target/debug/bench_report /DATA/ai/mrs/crates/mrs-bench/results/casc-j13-W8J1-20260913/run.csv`
CASC-J13 Results — 2026-09-14 06:22  (400 problems × 1 systems)
===============================================================

Division  Problems    mrs
                 Solved  Avg (s)
------------------  --------------------
UEQ            400        69   17.774
------------------  --------------------
TOTAL          400        69   17.774

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

commit 440404cf925292e314e2b2f9705383873c5c26a3 (HEAD -> main, origin/main, origin/HEAD)

[done]
[PPROD:user@server97:/DATA/ai/user/mrs]$ crates/mrs-bench/run_strategy_sweep.sh --edition casc-30 --divisions fne,feq,epu,eps,ueq,icu --casc-times --jobs 30 --output crates/mrs-bench/results/casc-30-sweep-$(date +%Y%m%d)
Running `target/debug/bench_report /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30-sweep-20260912/run.csv`
CASC-30 Results — 2026-09-14 07:15  (1101 problems × 15 systems)
================================================================

Division  Problems    mrs-s01               mrs-s02               mrs-s03               mrs-s04               mrs-s05               mrs-s06               mrs-s07               mrs-s08               mrs-s09               mrs-s10               mrs-s11               mrs-s12               mrs-s13               mrs-s14               mrs-s15
                 Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------
FNE            100        31   63.764          21   22.233          25   35.434          30   56.531          10   30.567          15   68.894          31   72.409          31   24.430          25   36.737           3    0.433          32   37.872          23   48.077           3    0.411           4    9.008          18   37.214
FEQ            400        40   31.157          16    4.631          16   25.004          37   30.470          39   26.186          44   39.971          43   40.573          51   21.219          17   33.840          22   46.478          55   23.418          62   28.968          17   50.045          19   19.494          19   14.125
EPU            100        15   17.775          15   15.241          14   13.528          15   16.655           9    9.781           8   19.293          14   16.467          15   19.098           7    5.222           5    0.186          16   19.710          16   18.805           5    0.105           6    0.118          15    8.928
EPS            100        15   21.756          16   19.687          15   24.255           2    0.112           2    0.111           3    0.100          11   16.685          12   11.138           8   19.334           2    0.108           2    0.096           2    0.096           2    0.098           2    0.111           2    0.106
UEQ            300        48   89.647          36   60.610          31   37.765          78   61.491          58   79.793          44   60.242          13   56.777          36   58.231           5   84.060           3   41.726          54   65.268          71   58.344           4    1.696          38   51.313          31   45.521
ICU            101         2  168.262           2  198.448           2  166.014           1  263.891           1    3.888           1   20.724           1  262.863           1  447.609           1  314.749           0    0.000           1    6.368           2  144.667           0    0.000           1   37.469           1   42.129
------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------
TOTAL         1101       151   55.997         106   34.561         103   32.445         163   49.899         119   50.816         115   48.853         113   47.826         146   32.901          63   38.412          35   32.861         160   39.665         176   43.379          31   27.726          70   34.210          86   29.368

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[ongoing]
[root@mtsdev01 mrs]# for interval in 25 50 100 250; do MRS_WORKERS=8 MRS_SHARED_POOL_INTERVAL="${interval}" crates/mrs-bench/casc.sh --edition casc-j13 --systems mrs --divisions ueq --casc-times --jobs 1 --output crates/mrs-bench/results/casc-j13-ueq-sharing-${interval}-$(date +%Y%m%d_%H%M%S); done
/mnt/sdf1/mrs/crates/mrs-bench/results/casc-j13-ueq-sharing-25-20260912_182655/run.csv
Running `target/debug/bench_report /mnt/sdf1/mrs/crates/mrs-bench/results/casc-j13-ueq-sharing-25-20260912_182655/run.csv`
CASC-J13 Results — 2026-09-14 08:05  (400 problems × 1 systems)
===============================================================

Division  Problems    mrs
                 Solved  Avg (s)
------------------  --------------------
UEQ            400       255   12.751
------------------  --------------------
TOTAL          400       255   12.751

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected

     Running `target/debug/bench_report /mnt/sdf1/mrs/crates/mrs-bench/results/casc-j13-ueq-sharing-50-20260913_024037/run.csv`
CASC-J13 Results — 2026-09-14 08:06  (400 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
UEQ            400       258   11.356
------------------  --------------------
TOTAL          400       258   11.356

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

     Running `target/debug/bench_report /mnt/sdf1/mrs/crates/mrs-bench/results/casc-j13-ueq-sharing-100-20260913_103938/run.csv`
CASC-J13 Results — 2026-09-14 08:07  (400 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
UEQ            400       260   12.147
------------------  --------------------
TOTAL          400       260   12.147

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

     Running `target/debug/bench_report /mnt/sdf1/mrs/crates/mrs-bench/results/casc-j13-ueq-sharing-250-20260913_183558/run.csv`
CASC-J13 Results — 2026-09-14 08:07  (400 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
UEQ            400       254   10.579
------------------  --------------------
TOTAL          400       254   10.579

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.


[done]
[www@server99 mrs]$ crates/mrs-bench/run_strategy_sweep.sh   --edition casc-j13   --divisions fne,feq,ueq   --casc-times   --jobs 30  --output crates/mrs-bench/results/casc-j13-sweep-$(date +%Y%m%d)
crates/mrs-bench/results/casc-j13-sweep-20260912
Running `target/debug/bench_report /DATA/ai/mrs/crates/mrs-bench/results/casc-j13-sweep-20260912/run.csv`
CASC-J13 Results — 2026-09-13 06:08  (800 problems × 15 systems)
================================================================

Division  Problems    mrs-s01               mrs-s02               mrs-s03               mrs-s04               mrs-s05               mrs-s06               mrs-s07               mrs-s08               mrs-s09               mrs-s10               mrs-s11               mrs-s12               mrs-s13               mrs-s14               mrs-s15
                 Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------
FNE            100        21   35.238          11   21.203          12   27.391          21   31.947          10   17.844          12   26.716          20   45.451          24   37.869           8   20.284           7    1.426          32   19.950          21   44.201           7    1.936           6   12.430          20   16.655
FEQ            300        39   19.523          25    8.687          18   10.519          42   20.970          38   23.279          40   21.565          39   14.607          41    9.050          21    7.204          21   48.873          47   14.633          46   17.657          13   38.596          24   23.651          18   20.175
UEQ            400        36   45.713          38   36.840          30   42.525          59   52.138          43   48.325          33   35.654          23   36.591          47   21.494           7   43.083           2    0.181          41   38.158          58   41.709           3   12.670          35   45.535          28   43.181
------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------  --------------------
TOTAL          800        96   32.782          74   25.004          60   29.897         122   37.932          91   34.517          85   27.762          82   28.296         112   20.447          36   17.087          30   34.556         120   24.088         125   33.276          23   24.057          65   34.399          66   28.869

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[ongoing]
[root@server01 mrs]# for interval in 25 50 100 250; do MRS_WORKERS=8 MRS_SHARED_POOL_INTERVAL="${interval}" crates/mrs-bench/casc.sh --edition casc-30 --systems mrs --divisions ueq --casc-times --jobs 1 --output crates/mrs-bench/results/casc-30-ueq-sharing-${interval}-$(date +%Y%m%d_%H%M%S); done
Running `target/debug/bench_report /mnt/sdf1/mrs/crates/mrs-bench/results/casc-30-ueq-sharing-25-20260911_183710/run.csv`
CASC-30 Results — 2026-09-12 06:37  (300 problems × 1 systems)
==============================================================

Division  Problems    mrs
                 Solved  Avg (s)
------------------  --------------------
UEQ            300       224   12.747
------------------  --------------------
TOTAL          300       224   12.747

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

Running `target/debug/bench_report /mnt/sdf1/mrs/crates/mrs-bench/results/casc-30-ueq-sharing-50-20260912_003201/run.csv`
CASC-30 Results — 2026-09-12 06:38  (300 problems × 1 systems)
==============================================================

Division  Problems    mrs
                 Solved  Avg (s)
------------------  --------------------
UEQ            300       226   12.491
------------------  --------------------
TOTAL          300       226   12.491

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

     Running `target/debug/bench_report /mnt/sdf1/mrs/crates/mrs-bench/results/casc-30-ueq-sharing-100-20260912_061754/run.csv`
CASC-30 Results — 2026-09-12 16:22  (300 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
UEQ            300       228   12.160
------------------  --------------------
TOTAL          300       228   12.160

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

     Running `target/debug/bench_report /mnt/sdf1/mrs/crates/mrs-bench/results/casc-30-ueq-sharing-250-20260912_115449/run.csv`
CASC-30 Results — 2026-09-12 16:23  (300 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
UEQ            300       222   11.549
------------------  --------------------
TOTAL          300       222   11.549

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done]
[root@server04 mrs]# for interval in 250 500 1000; do   MRS_WORKERS=8   MRS_SHARED_POOL_INTERVAL="${interval}"   crates/mrs-bench/cooperative_portfolio_sweep.sh     casc-30 feq     11,12,1,6,10,8,14,4     240 1     "results/feq-interval-${interval}-$(date +%Y%m%d_%H%M%S)"; done
[root@server04 mrs]# cargo run -p mrs-bench --bin bench_report -- /mnt/sdd1/mrs/results/feq-interval-250-20260911_175359/run.csv
    Finished `dev` profile [unoptimized] target(s) in 0.35s
     Running `target/debug/bench_report /mnt/sdd1/mrs/results/feq-interval-250-20260911_175359/run.csv`
CASC-30 Results — 2026-09-14 07:43  (400 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FEQ            400       108   21.477
------------------  --------------------
TOTAL          400       108   21.477

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[root@server04 mrs]# cargo run -p mrs-bench --bin bench_report -- /mnt/sdd1/mrs/results/feq-interval-500-20260912_140750/run.csv
    Finished `dev` profile [unoptimized] target(s) in 0.36s
     Running `target/debug/bench_report /mnt/sdd1/mrs/results/feq-interval-500-20260912_140750/run.csv`
CASC-30 Results — 2026-09-14 07:44  (400 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FEQ            400       110   24.137
------------------  --------------------
TOTAL          400       110   24.137

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[root@server04 mrs]# cargo run -p mrs-bench --bin bench_report -- /mnt/sdd1/mrs/results/feq-interval-1000-20260913_101832/run.csv
    Finished `dev` profile [unoptimized] target(s) in 0.36s
     Running `target/debug/bench_report /mnt/sdd1/mrs/results/feq-interval-1000-20260913_101832/run.csv`
CASC-30 Results — 2026-09-14 07:44  (400 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FEQ            400       110   24.484
------------------  --------------------
TOTAL          400       110   24.484

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done]
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 MRS_SHARED_POOL_INTERVAL=0 crates/mrs-bench/casc.sh --edition casc-30  --systems mrs --divisions fne,feq,epu,eps,ueq,icu  --casc-times --jobs 2 --output crates/mrs-bench/results/casc-30-W8J2-$(date +%Y%m%d)
Running `target/debug/bench_report /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30-W8J2-noshare-20260911/run.csv`
CASC-30 Results — 2026-09-12 16:18  (1095 problems × 1 systems)
===============================================================

Division  Problems    mrs
                 Solved  Avg (s)
------------------  --------------------
FNE            100        42   25.846
FEQ            400       109   22.962
EPU            100        18   11.868
EPS            100        17   23.284
UEQ            300       124   35.133
ICU            101         4   75.029
------------------  --------------------
TOTAL         1101       314   28.199

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done]
[www@server99 mrs]$ MRS_WORKERS=8 MRS_SHARED_POOL_INTERVAL=0 crates/mrs-bench/casc.sh --edition casc-j13 --systems mrs --divisions fne,feq,ueq  --casc-times --jobs 2 --output crates/mrs-bench/results/casc-j13-W8J2-nosharing-$(date +%Y%m%d)
Running `target/debug/bench_report /DATA/ai/mrs/crates/mrs-bench/results/casc-j13-W8J2-nosharing-20260911/run.csv`
CASC-J13 Results — 2026-09-12 06:28  (800 problems × 1 systems)
===============================================================

Division  Problems    mrs
                 Solved  Avg (s)
------------------  --------------------
FNE            100        35   12.884
FEQ            300        71   20.391
UEQ            400       119   32.096
------------------  --------------------
TOTAL          800       225   25.414

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done]
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh   --edition casc-j13 --systems mrs --divisions fne,feq,ueq  --casc-times --jobs 2 --output crates/mrs-bench/results/casc-j13-W8J1-$(date +%Y%m%d)
Running `target/debug/bench_report /DATA/ai/mrs/crates/mrs-bench/results/casc-j13-W8J1-20260910/run.csv`
CASC-J13 Results — 2026-09-11 06:13  (800 problems × 1 systems)
===============================================================

Division  Problems    mrs
                 Solved  Avg (s)
------------------  --------------------
FNE            100        35   12.848
FEQ            300        72   22.843
UEQ            400       257   10.997
------------------  --------------------
TOTAL          800       364   13.518

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

commit 4b72fdc54af60e074768506c7cdcc9b81d333aaa (HEAD -> main, origin/main, origin/HEAD)

[done]
[root@server04 mrs]# MRS_WORKERS=8 crates/mrs-bench/cooperative_portfolio_sweep.sh casc-j13 feq 11,12,1,6,10,8,14,4 180 1 "crates/mrs-bench/results/cooperative-j13-feq-$(date +%Y%m%d_%H%M%S)"
Running `target/debug/bench_report /mnt/sdd1/mrs/crates/mrs-bench/results/cooperative-j13-feq-20260910_111944/run.csv`
CASC-J13 Results — 2026-09-11 07:05  (300 problems × 1 systems)
===============================================================

Division  Problems    mrs
                 Solved  Avg (s)
------------------  --------------------
FEQ            300        74   21.582
------------------  --------------------
TOTAL          300        74   21.582

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

[done]
[PPROD:user@server97:/DATA/ai/user/mrs]$ MRS_WORKERS=8 MRS_SHARED_POOL_INTERVAL=0 JOBS=2 crates/mrs-bench/cooperative_portfolio_sweep.sh casc-30 feq 11,12,1,6,10,8,14,4 240 1 "crates/mrs-bench/results/cooperative-feq-no-sharing-$(date +%Y%m%d_%H%M%S)"
Running `target/debug/bench_report /DATA/ai/user/mrs/crates/mrs-bench/results/cooperative-feq-no-sharing-20260910_104450/run.csv`
CASC-30 Results — 2026-09-11 07:03  (400 problems × 1 systems)
==============================================================

Division  Problems    mrs
Solved  Avg (s)
------------------  --------------------
FEQ            400       122   27.828
------------------  --------------------
TOTAL          400       122   27.828

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

commit f0010b740ac3e4f6e1c0e43507f780bd5a38b1e8 (HEAD -> main, origin/main, origin/HEAD)

[done]
[www@server99 mrs]$   crates/mrs-bench/cooperative_portfolio_sweep.sh \
  casc-30 feq 11,12,1,6,10,8,14,4 240 1 \
  "crates/mrs-bench/results/cooperative-feq-$(date +%Y%m%d_%H%M%S)"
[www@server99 mrs]$ ./target/release/bench_report /DATA/ai/mrs/crates/mrs-bench/results/cooperative-feq-20260909_205003/run.csv
  CASC-30 Results — 2026-09-10 16:13  (400 problems × 1 systems)
  ==============================================================

  Division  Problems    mrs
                        Solved  Avg (s)
  ------------------  --------------------
  FEQ            400       119   24.963
  ------------------  --------------------
  TOTAL          400       119   24.963

  DISAGREEMENTS — none detected.

  POLARITY VIOLATIONS — none detected.
  
  REFERENCE VIOLATIONS — none detected.

commit 181929ae1ea647c30cf91ea92219d69f23d7e9f3 (HEAD -> fix/integrate-casc-next-review, origin/fix/integrate-casc-next-review)

[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh   --edition casc-30  --systems mrs --divisions fne,feq,epu,eps,ueq,icu  --casc-times --jobs 2 --output crates/mrs-bench/results/casc-30-W8J2-$(date +%Y%m%d)
     Running `target/debug/bench_report /DATA/ai/mrs/crates/mrs-bench/results/casc-30-W8J2-20260908/run.csv`
CASC-30 Results — 2026-09-09 16:26  (1039 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        43   26.548
FEQ            400       112   23.087
EPU            100        18   12.797
EPS            100        17   22.541
UEQ            300       222   11.933
ICU             39         7   79.992
------------------  --------------------
TOTAL         1039       419   18.019

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

commit dbf462d82d3ff24d13ce38090dab10f278765836 (HEAD -> fix/integrate-casc-next-review, origin/fix/integrate-casc-next-review)

[done]
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh   --edition casc-30  --systems mrs --divisions fne,feq,epu,eps,ueq,icu  --casc-times --jobs 1 --output crates/mrs-bench/results/casc-30-W8J1-$(date +%Y%m%d)
Running `target/debug/bench_report /DATA/ai/mrs/crates/mrs-bench/results/casc-30-W8J1-20260907/run.csv`
CASC-30 Results — 2026-09-08 17:10  (993 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        43   23.628
FEQ            400       102   21.605
EPU            100        18   12.707
EPS            100        17   22.893
UEQ            300       129    7.559
------------------  --------------------
TOTAL         1000       309   15.623

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.


commit 4d2f610ad61279e848bd6ffcb59ff62094acb0a9 (HEAD -> fix/integrate-casc-next-review, origin/fix/integrate-casc-next-review)

[done]
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh   --edition casc-j13 --systems mrs --divisions fne,feq,ueq  --casc-times --jobs 1 --output crates/mrs-bench/results/casc-j13-W8J1-$(date +%Y%m%d)

     Running `target/debug/bench_report /DATA/ai/mrs/crates/mrs-bench/results/casc-j13-W8J1-20260906/run.csv`
CASC-J13 Results — 2026-09-07 10:41  (800 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        35   12.184
FEQ            300        69   22.174
UEQ            400       167    7.176
------------------  --------------------
TOTAL          800       271   11.642

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

commit 835077e8c628078f9385b2740aea744c4c50031d (HEAD -> integrate/casc-next

[done]
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh   --edition casc-j13 --systems mrs --divisions fne,feq,ueq  --casc-times --jobs 1 --output crates/mrs-bench/results/casc-j13-W8J1-$(date +%Y%m%d)
/DATA/ai/mrs/crates/mrs-bench/results/casc-j13-W8J1-20260905/run.csv
     Running `target/debug/bench_report /DATA/ai/mrs/crates/mrs-bench/results/casc-j13-W8J1-20260905/run.csv`
CASC-J13 Results — 2026-09-06 12:38  (800 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        40   14.938
FEQ            300        69   21.857
UEQ            400       168    7.122
------------------  --------------------
TOTAL          800       277   11.921

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — 4 SOUNDNESS ERROR(S) vs reference answers:
  FNE     NLP260+1                        mrs=CounterSatisfiable but expected Theorem  ⚠ UNSOUND
  FNE     NLP261+1                        mrs=CounterSatisfiable but expected Theorem  ⚠ UNSOUND
  FNE     NLP262+1                        mrs=CounterSatisfiable but expected Theorem  ⚠ UNSOUND
  FNE     PRD001+1                        mrs=CounterSatisfiable but expected Theorem  ⚠ UNSOUND

[done]
     Running `target/debug/bench_report /DATA/ai/mrs/crates/mrs-bench/results/casc-30-W8J1-20260903/run.csv`
CASC-30 Results — 2026-09-05 13:28  (1101 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        46   22.429
FEQ            400       102   23.024
EPU            100        18   12.627
EPS            100        48    8.268
UEQ            300       128    7.189
ICU            101         2   67.921
------------------  --------------------
TOTAL         1101       344   14.711

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — 3 SOUNDNESS ERROR(S) vs reference answers:
  FNE     NLP260+1                        mrs=CounterSatisfiable but expected Theorem  ⚠ UNSOUND
  FNE     NLP261+1                        mrs=CounterSatisfiable but expected Theorem  ⚠ UNSOUND
  FNE     NLP262+1                        mrs=CounterSatisfiable but expected Theorem  ⚠ UNSOUND


commit 9738467d6d1dc3190f663bea94f4f628d7f1d7a9 (HEAD -> feat/destructive-equality-resolution

[done]
[www@server99 mrs]$ MRS_WORKERS=8 crates/mrs-bench/casc.sh   --edition casc-30  --systems mrs --divisions feq,fne,epu,eps  --casc-times --jobs 1 --output crates/mrs-bench/results/casc-30-W8J1-$(date +%Y%m%d)
     Running `target/debug/bench_report /DATA/ai/mrs/crates/mrs-bench/results/casc-30-W8J1-20260902/run.csv`
CASC-30 Results — 2026-09-03 15:40  (700 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FEQ            400       104   29.486
FNE            100        44   35.721
EPU            100        18   17.805
EPS            100        43   10.584
------------------  --------------------
TOTAL          700       209   25.904

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

REFERENCE VIOLATIONS — none detected.

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

[interrupted] 994/13011
export RUST_MIN_STACK=67108864
[root@server01 mrs]# ./crates/mrs-bench/run_codex_sweep.sh "$TPTP" codex_cat_filtered_sweep_f13912c763_01-03.db 300 1
[root@server01 mrs]# cat codex_sweep_mrs-s01.out | grep -v Timeout | grep -v GaveUp | grep -v Error | wc -l
347

[ongoing] 1039/13011
export RUST_MIN_STACK=67108864
[root@server02 mrs]# ./crates/mrs-bench/run_codex_sweep.sh "$TPTP" codex_cat_filtered_sweep_f13912c763_04-06.db 300 1

[ongoing] 1035/13011
export RUST_MIN_STACK=67108864
[root@server03 mrs]# ./crates/mrs-bench/run_codex_sweep.sh "$TPTP" codex_cat_filtered_sweep_f13912c763_07-09.db 300 1

[partial] category 10 only
export RUST_MIN_STACK=67108864
[root@server04 mrs]# ./crates/mrs-bench/run_codex_sweep.sh "$TPTP" codex_cat_filtered_sweep_f13912c763_10-12.db 300 1
cat codex_sweep_mrs-s10.out | grep -v GaveUp | grep -v Timeout  | wc -l
131 / 13011

[partial] category 13 only
export RUST_MIN_STACK=67108864
[PPROD:user@server97:/DATA/ai/user/mrs]$ ./crates/mrs-bench/run_codex_sweep.sh "$TPTP" codex_cat_filtered_sweep_f13912c763_13-15.db 300 1
[PPROD:user@server97:/DATA/ai/user/mrs]$ cat codex_sweep_mrs-s13.out | grep -v GaveUp | grep -v Timeout  | wc -l
176 / 13011

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
17188 Jul 13 09:34 /DATA/ai/user/mrs/crates/mrs-bench/results/casc-30/20260713_083231/run.csv

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
