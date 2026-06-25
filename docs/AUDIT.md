# MRS Audit — June 2026

This document records the failure-mode census, throughput analysis, and soundness
check performed as Phase 1 of the CASC-J13 / ProoVer 2026 preparation roadmap.
It supersedes the speculative TODO items in `TODO_CASC.md` and gives the priority
ordering for Phases 2–4 based on measured data.

---

## Methodology

All measurements use commit `af5af8fa` (after the branch-split work) on the local
machine (8 cores, ~16 GB RAM) with TPTP v9.2.1 at
`/mnt/wsl/CUsersfr22192WSLDatafastdatavhdx/TPTP-v9.2.1`.

Tooling added for this audit:
- `SearchStats` counters in `mrs-search` (iterations, processed, generated,
  weight_discarded, forward_subsumed, backward_deleted, passive_size).
- `% SZS detail key=value…` line on stderr from `mrs` after every run.
- `casc.sh`: captures stderr, adds `failure_detail` CSV column.
- `bench_report --census <system>`: failure-mode table + aggregate stats.

Comparison provers:
- E 3.x (`--auto-schedule`): `crates/mrs-bench/systems/eprover/bin/eprover`
- Vampire 4.x (`--mode portfolio`): `crates/mrs-bench/systems/vampire/bin/vampire`

---

## 1. Soundness Check

Ran all 40 problems in CASC-30 whose reference answer is `Satisfiable` or
`CounterSatisfiable` (EPS division + a few from FEQ/FNE).  mrs correctly
returned non-`Theorem`/non-`Unsatisfiable` on all 40.  **Zero soundness
violations detected.**

---

## 2. Failure-Mode Census

### FNE (First-Order No Equality) — 30 s / problem

| Bucket        | Count | % |
|---------------|------:|--:|
| `gave_up`     |    12 | 92% |
| `saturated`   |     1 |  8% |
| `timeout`     |     0 |  — |

Aggregate over 13 unsolved problems:
- `processed / problem`: **25,131**
- `generated / problem`: **54,580**
- `passive remaining Σ`: **454,565**
- `weight_discarded Σ`:  **0**

### EPS (EPR Satisfiable) — 120 s / problem

| Bucket        | Count | % |
|---------------|------:|--:|
| `gave_up`     |     4 | 100% |

Aggregate over 4 unsolved problems:
- `processed / problem`: **71,328**
- `generated / problem`: **595,554**
- `passive remaining Σ`: **916,880**
- `weight_discarded Σ`:  **479,806** ← 20% of generated clauses killed by weight

### Key observations

1. **`weight_discarded` = 0 on FNE, 479k on EPS.** The `max_term_weight = 200`
   cap is completely inert on FOF problems but aggressively discards EPR clauses.
   For EPS, 20% of all generated clauses are thrown away — potentially including
   necessary resolution steps.

2. **`passive remaining` is huge.** ~35k passive clauses per FNE problem and
   ~229k per EPS problem when mrs stops. The search is active and generating
   plenty of clauses; it is *selecting* the wrong ones to process.

3. **`gave_up` dominates.** 92% of FNE failures are `gave_up` (all 11 strategies
   hit their time slice before finding a proof). There are almost no `saturated`
   cases (correct proof-of-unsolvability). The search simply runs out of time
   on problems that are not exponentially hard.

---

## 3. Throughput Comparison: mrs vs E

All measurements at 30 s wall-clock, 4 workers (mrs), `--auto-schedule` (E).

| Problem | mrs processed | mrs generated | mrs result | E processed | E generated | E result |
|---------|--------------|--------------|-----------|-------------|-------------|---------|
| CSR027+3 (FNE) | 1,902 | 1,449 | **Theorem** | 11,678 | 27,734 | Theorem |
| CSR034+2 (FNE) | 25,399 | 35,296 | **Theorem** | 4,621 | 7,241 | Theorem |
| NLP260+1 (FNE) | 1,097 | 271 | **Theorem** | 60 | 42 | Theorem |
| NLP261+1 (FNE) | 2,675 | 1,632 | **Theorem** | 421 | 715 | Theorem |
| GEO259+1 (FNE) | 15,499 | 118,438 | **Theorem** | 845 | 2,467 | Theorem |
| COM008+1 (FNE) | 18,077 | 147,209 | **GaveUp** | 907 | 3,832 | **Theorem** |
| AGT005+1 (FNE) | 54,721 | 647,662 | **GaveUp** | 434 | 550 | **Theorem** |
| MGT067+1 (FNE) | 5,123 | 25,214 | **GaveUp** | 460 | 294 | **Theorem** |
| CSR026+3 (FNE) | 8,139 | 3,496 | **GaveUp** | 5,621 | 14,042 | **Theorem** |
| CSR034+2 (FNE) | 7,948 | 10,330 | **GaveUp** | 4,621 | 7,241 | **Theorem** |

### Interpretation

On problems where **both** provers solve it, mrs is sometimes more focused
(NLP260+1: 1097 processed vs E's 60 processed — mrs is ~18× less efficient) and
sometimes comparable (CSR027+3).

On problems E solves that mrs does not:
- **AGT005+1**: mrs generates 647k clauses and fails; E finds a proof with 434
  processed / 550 generated. mrs generates **1,200× more clauses** for the
  same problem in the same time.
- **COM008+1**: E proves it with 907 processed; mrs generates 147k and fails.
  **162× ratio**.
- **MGT067+1**: E proves in 460 processed; mrs generates 25k. **54× ratio**.

**This is not a raw throughput (clauses/sec) problem.** mrs can iterate fast
enough. The problem is that mrs navigates the search space 50–1200× less
efficiently than E on many problems — it generates massive amounts of useless
clauses and processes them, while E's heuristics steer directly toward the proof.

---

## 4. Root Cause Analysis

### Root Cause 1: Strategy / Heuristic Quality (HIGHEST ROI)

**Measured:** 50–1200× more clauses processed vs E on failures; E solves problems
in <1000 processed clauses that mrs cannot solve in 30s.

**Cause:** E runs a portfolio of ~40 heuristic strategies, each with a tailored
combination of:
- Symbol precedence computed from the problem
- Clause selection weight functions (14+ built-in, including conjecture-distance,
  horn-weighting, symbol-weight)
- SOS (set of support) restrictions
- Literal selection policies tuned per-strategy

Mrs has 11 strategies based on AgeWeight/SmallestFirst/GoalDirected with KBO/LPO.
The *ratio* strategies (Age:Weight interleaving) are static. There is no problem-
specific symbol analysis feeding back into the weight function beyond the rarity-
based KBO precedence.

**Fix (highest ROI, 2–4 days):**
1. Add 3–5 more clause weight functions: `FunctionWeightPenalty` (penalize
   deeply nested terms), `HornHeuristic` (prefer Horn clauses), `SymbolWeight`
   (per-symbol custom weights from problem analysis).
2. Run a greedy set-cover over the 1000-problem CASC-30 set to pick the
   complementary strategy portfolio (this is how Vampire/E tune their portfolios).
3. Add SOS (Set of Support): restrict initial resolution to involve at least one
   clause from the conjecture-closure. This alone causes E to explore ~10× fewer
   clauses on many FNE problems.

### Root Cause 2: max_term_weight Cap Too Aggressive for EPS (MEDIUM ROI)

**Measured:** 479,806 clauses discarded per 4 EPS problems = 20% of generated.

EPS problems have large propositional groundings; the default `max_term_weight = 200`
is calibrated for FOF problems. EPR clauses have higher base weights.

**Fix (1 day):** Set `max_term_weight = None` or `max_term_weight = 500+` in the
`casc_epr` schedule. Add per-problem weight-cap calibration based on initial clause
weight statistics.

### Root Cause 3: No LRS (Limited Resource Strategy) (MEDIUM ROI)

**Measured:** `passive` queue grows to 35k–229k per unsolved problem. In a 30s
budget with 25k processed clauses, ~90% of passive clauses will never be processed.

**Cause:** mrs keeps all generated clauses in the passive set forever. Vampire's
LRS computes how many clauses can be processed in the remaining budget and discards
clauses whose weight rank exceeds that threshold. This eliminates passive-set
memory explosion.

**Fix (2–3 days):** Implement LRS in `given_clause.rs`: at each iteration, estimate
`remaining_iterations = remaining_time / avg_iteration_time` and discard the bottom
`passive_size - remaining_iterations` clauses by weight.

### Root Cause 4: Strategy Portfolio Not Complementary (MEDIUM ROI)

The 11 strategies in mrs's `casc` portfolio overlap heavily — they all use
AgeWeight(5)/SmallestFirst over KBO/LPO with AllNegative/All. The theoretical
exploration diversity is limited.

**Fix (1–2 days):** Run each strategy solo on the full CASC-30 problem set
(using `MRS_SINGLE_STRATEGY=N`) and pick the 8–10 strategies with the highest
*unique* solve count (greedy set-cover). At minimum, add GoalDirected and unit-
only variants to the casc_fne schedule.

### Root Cause 5: Inference Redundancy — No Rewriting Before Resolution (LOW-MEDIUM ROI)

On AGT005+1, mrs generates 647k clauses. Most are likely trivially subsumed or
reducible by unit equalities that are discovered early. Vampire applies rewriting
before every inference, so derived clauses arrive pre-simplified. Mrs applies
demodulation only to the given clause, not pre-computing rewrite-saturated
normal forms for inference partners.

**Fix:** Not low-hanging fruit; would require index changes. Defer after Root
Causes 1–4 are addressed.

---

## 5. Priority Ordering for Phase 2

1. **Strategy quality + SOS** (Root Cause 1) — expected +30–80% solved on FNE/FEQ
2. **EPS weight cap** (Root Cause 2) — expected +10–20 more EPS/EPU solves
3. **LRS** (Root Cause 3) — expected throughput improvement on large problems
4. **Set-cover portfolio retuning** (Root Cause 4) — after new strategies exist
5. **Inference redundancy** (Root Cause 5) — defer to Phase 3

---

---

## Update (2026-06-25): the real lever was inference *generation*, not selection

The Phase-2 work that followed this audit (new weight heuristics, greedy
portfolio tuning, ML-guided **selection**) gave only marginal/negative gains,
because it all optimised *which clause to pick next* — but the measured
bottleneck (§3: 50–1200× more clauses generated than E, including on problems
both solve) is clause **generation/redundancy**, an inference-restriction
problem. Code inspection confirmed two missing standard restrictions:

1. **`AllNegative` selects *all* negative literals** (`literal_selection.rs`).
   The standard, refutationally-complete choice selects a *single* negative
   literal; selecting all multiplies resolvents on FNE. Measured (CASC-30 FNE,
   `casc_fne`, 8 s/problem, 48-problem sample):
   all-negative **14** solved → single-negative **18** (**+29%**), newly
   solving MGT067+1 (E solves it; mrs did not). FEQ regresses slightly (9→8),
   so this is a *portfolio-diversity* win, not a global default → applied to
   the FNE-specific `casc_fne` schedule only (FNE has no equality → no FEQ risk).

2. **No maximal-literal restriction for all-positive clauses.** Added the
   standard ordered-inference restriction (`SearchConfig.ordered_inferences`,
   default on; `restrict_to_maximal_id`): for all-positive predicate-only
   clauses only order-maximal literals are eligible. Completeness-preserving;
   small alone (MGT067+1 −26% generated) but correct and compounding.

**Conclusion:** the original Phase-2 prioritisation (heuristic quality #1,
inference redundancy #5/deferred) was inverted. Highest-ROI remaining work is
core-engine inference restriction + simplification, not clause selection.
Next: (a) add single-negative strategies to the s1–s15 set and re-run the
greedy sweep so every division can pick them; (b) demodulate newly generated
clauses at generation time; (c) maximal-side superposition restriction for
UEQ/FEQ.

## 6. What is NOT a Problem

- **Raw throughput (iterations/sec):** mrs can process 25k+ clauses in 30s.
  Speed is not the bottleneck.
- **Correctness:** Zero soundness violations on 40 satisfiable/countersatisfiable
  problems tested.
- **Memory:** No OOMs observed; peak memory ~1.5–2 GB per strategy on hard
  problems.
- **Parse/lowering:** Zero `parse_error` buckets in the census.
- **Clause sharing:** The implemented `shared_pool` mechanism for unit equalities
  is working; no regression introduced by it.

---

## 7. Benchmark Data Location

| Run | Problems | Time | File |
|-----|---------|------|------|
| FNE 30s, 26 problems | FNE (26/100) | 30 s | `/tmp/opencode/census_fne30/run.csv` |
| FNE 10s, 29 problems | FNE (29/100) | 10 s | `/tmp/opencode/census_test3/run.csv` |
| EPS 120s, 7 problems | EPS (7/100) | 120 s | `/tmp/opencode/census_eps/run.csv` |

Full 240s CASC-time census (900 problems × mrs + eprover) was started but timed out
on this machine due to load. Rerun with `--jobs 1` on a dedicated machine:

```bash
export TPTP=/path/to/TPTP-v9.2.1
bash crates/mrs-bench/casc.sh --systems mrs,eprover --divisions fne,feq,ueq,eps \
  --casc-times --jobs 1 --output results/census-$(date +%Y%m%d)
./target/release/bench_report results/census-*/run.csv --census mrs
```
