# Pre-phase at a 240-second per-strategy budget: solo sweeps on CASC-30 and CASC-J13

*Analysis date: 2026-11-02. Follow-up to `2026-10-prephase-study.md`.*
*Data bundles: `/home/hack/prephase_casc30.tar.bz2` and `/home/hack/prephase_cascj13.tar.bz2`; extracted CSVs are under `/home/hack/results/prephase/`.*

> **Scope:** `prephase_sweep` ran one base strategy at a time with one worker. The “8-slot” portfolio numbers below are unions of solo results, not measured 8-worker concurrent runs. They are candidate-generation evidence only. The tarballs do not include the producing binary hash, host/run metadata, or the original `.p` files, so this is not a standardized performance baseline.

## 1. Executive Summary

The repaired 240-second solo matrices give a mixed result for a label-fitted route on one deterministic two-fold holdout: it is slightly ahead on CASC-30 and slightly behind on CASC-J13. This is a limited comparison of the saved matrices—not a decisive measurement of concurrent portfolio behavior or of the production `--pre-phase` rules.

At an 8-slot **simulated** width:
- **CASC-30:** the current per-division order covers **239/991** instances after applying the official 120 s EPU cutoff; the generic `casc` prefix covers 204. A greedy fixed-order candidate covers 257; the repaired label fit covers 263 in-sample.
- **CASC-J13:** the current per-division order covers **236/796**, generic `casc` covers 204, a greedy fixed-order candidate covers 239, and the in-sample label fit covers 242.
- On the corrected two-fold held-out simulation, the fitted route covers **246/991 vs 239/991** for per-division on CASC-30 (+7), and **230/796 vs 236/796** on CASC-J13 (−6). The split is deterministic and alternating by problem name; this is a single exploratory CV, not a significance result.

The corrected, edition-specific union contains **1,787 benchmark instances**, of which 510 are solved by at least one base strategy. The best 8-strategy greedy candidate covers 496 in the solo matrix. It must not be promoted from these numbers alone: concurrent interference, search races, and sharing were not measured.

Review found two defects in `prephase/analyze.py` that produced the initial draft of this follow-up: the fitted strategy order put greedy-selected strategies last, and the “best fixed” column reported the full 15-strategy union at every width. The archived feature CSVs had a third defect: a missing delimiter concatenated `probe_iterations` onto `label` and shifted all probe columns. The serializer is fixed in `prephase_dump.rs`; the legacy CSVs were repaired with `repair_probe_columns.py` before analysis (3,088 C30 and 1,331 J13 feature rows repaired; remaining rows have no probe data). The 3 s cross-validated route count and p-value in `2026-10-prephase-study.md` also used the inverted fitter and are withdrawn; the 3 s/30 s label matrices are not present in the retained artifact bundles for correction.

---

## 2. Dataset and Corpus Coverage

The saved data covers these refutation instances:

| Dataset | Refutation divisions | Name-keyed edition/problem entries | Solved by at least one strategy | Raw CSV rows | Distinct name-key/strategy cells |
|---|---|---:|---:|---:|---:|
| **CASC-30** | EPU (97), FEQ (398), FNE (97), ICU (99), UEQ (300) | 991 | 264 (26.6%) | 16,320 | 14,865 |
| **CASC-J13** | FEQ (299), FNE (97), UEQ (400) | 796 | 246 (30.9%) | 11,940 | 11,940 |
| **Combined** | Edition-specific instances; shared names stay distinct | **1,787** | **510 (28.5%)** | **28,260** | **26,805** |

In the CASC-30 path list, 97 EPU names occur under both `EPU/` and
`Problems/EPU/`, adding 1,455 repeated name/strategy rows. The saved verdicts
agree for those repeated rows, and this analysis uses the direct `EPU/` path as
the canonical row for coverage and timing. The tarballs contain no `.p` sources;
`Problems/EPU/` is also absent from the current checkout, so byte identity of
that path pair cannot be verified. Accordingly, the table is deduplicated by
problem name, not proven byte-unique input files. Same-named files across the two
editions are kept separate: among 326 refutation names shared by the corpora,
289 have different byte content in the checked-out trees (37 are byte-identical).

`prephase_dump --probe` caps the reference search at **2,000 processed clauses**
(`PROBE_CLAUSES`); `probe_iterations` in the CSV is an observed counter, not
that cap. Within each release, feature and sweep rows join by problem name;
when pooling releases, edition is part of the instance key.
The repair verifies every recovered label against the five class columns; all
valid repaired rows have the declared width and `probe_processed` stays at or
below 2,000.

The sweep itself gave every run 240 s. CASC-30 EPU's official cap is 120 s, so
the competition-time-adjusted coverage tables below censor EPU refutations after
120 s. This removes one late EPU strategy/problem result (`PUZ037-2`, s4 at
233.391 s) but leaves the 9-problem EPU union unchanged. The producer host,
binary hash, source commit, and exact jobs count are not present in the archives;
the data cannot establish canonical hardware performance.

After extracting the tarballs, repair the legacy feature CSVs with:

```bash
python3 crates/mrs-bench/prephase/repair_probe_columns.py \
  /home/hack/results/prephase/casc30_features_probed.csv \
  > /tmp/casc30_features_probed_repaired.csv
python3 crates/mrs-bench/prephase/repair_probe_columns.py \
  /home/hack/results/prephase/cascj13_features_probed.csv \
  > /tmp/cascj13_features_probed_repaired.csv
```

### Input and repaired-feature hashes

Rows labeled “Repaired” are derived by `repair_probe_columns.py`; they are not
members of the original tarballs.

| Artifact | SHA-256 |
|---|---|
| `casc30_sweep_t240.csv` | `554a5782db4f6b697e421d937aef522de72e7ccb040b0127b8427a9998b0dad3` |
| `casc30_features_probed.csv` | `8035a583e09b60e1a745ca01c49a40f0ca561d0db3967d7bf17276fbad041b2b` |
| Repaired CASC-30 feature view | `cfab1b1a26c469ef68d16d259a8dbf8bf932a22fe66136e17a0ee854ecfec046` |
| `casc30_refutation_paths.txt` | `4713bbecd98578ae495ffa1f17fd4f98a2648c03572743f5a2178e279e68508e` |
| `cascj13_sweep_t240.csv` | `d777b00bb437d82a518acb0f79c48d8ab027e7283296807c91d0e5c9b0d1235a` |
| `cascj13_features_probed.csv` | `a0f71ad1562734c5959b261760948863906ff591cae5a625fd720667bf608e05` |
| Repaired CASC-J13 feature view | `7a0edda249234481620ebe4db6638614fe9100c2995ce20c373f3320494848fc` |
| `cascj13_refutation_paths.txt` | `5664e0fcc7c70d3b23d88cfe2260026bbe166f8a977e19969a5b55a0ce463023` |
| `prephase_casc30.tar.bz2` | `6edd6c17f046d82ebdde44e5a7b494554079240fb1cf0e2a292079489d6c8865` |
| `prephase_cascj13.tar.bz2` | `782c7bd8359af0f7b5a7af535b5112efd44e5107c6e5fb38094d4c547cd65efb` |

---

## 3. Solo-Matrix Portfolio Simulation at 240 s

The following tables are unions of solo solved sets. “Slots” means the number
of strategy results unioned; it does **not** mean those strategies were run
concurrently. “Greedy fixed” is the prefix of a greedy set-cover order, not an
exact optimization over every fixed portfolio. “Fit route” is trained and
scored on the same labels, so it is an in-sample upper bound, not a held-out
result.

### CASC-30 (991 edition/problem instances; EPU capped at 120 s)

| Slots | Oracle (all 15) | Greedy fixed | Fit route (in-sample) | Default `casc` | Current `casc_div` | Fit − division |
|:---:|---:|---:|---:|---:|---:|---:|
| 1 | 264 | 159 | 186 | 104 | 127 | +59 |
| 2 | 264 | 208 | 228 | 133 | 147 | +81 |
| 4 | 264 | 236 | 252 | 147 | 197 | +55 |
| **8** | **264** | **257** | **263** | **204** | **239** | **+24** |
| 12 | 264 | 263 | 264 | 256 | 263 | +1 |
| 15 | 264 | 264 | 264 | 264 | 264 | 0 |

### CASC-J13 (796 edition/problem instances)

| Slots | Oracle (all 15) | Greedy fixed | Fit route (in-sample) | Default `casc` | Current `casc_div` | Fit − division |
|:---:|---:|---:|---:|---:|---:|---:|
| 1 | 246 | 141 | 178 | 113 | 109 | +69 |
| 2 | 246 | 182 | 209 | 147 | 133 | +76 |
| 4 | 246 | 215 | 228 | 163 | 184 | +44 |
| **8** | **246** | **239** | **242** | **204** | **236** | **+6** |
| 12 | 246 | 245 | 246 | 239 | 245 | +1 |
| 15 | 246 | 246 | 246 | 246 | 246 | 0 |

The in-sample fit nearly reaches the oracle at eight slots (263/264 and 242/246)
because it uses the same outcomes to choose and score each label's order. It
must not be read as generalization. The corrected two-fold held-out results are:

| Dataset | Fold 1: fit / test | Fold 1 routed / division | Fold 2: fit / test | Fold 2 routed / division | Total routed / division | Delta |
|---|---:|---:|---:|---:|---:|---:|
| CASC-30 | 496 / 495 | 125 / 122 | 495 / 496 | 121 / 117 | 246 / 239 | **+7** |
| CASC-J13 | 398 / 398 | 109 / 112 | 398 / 398 | 121 / 124 | 230 / 236 | **−6** |

This is one deterministic split formed by alternating sorted problem names. It
is a useful diagnostic, not replicated cross-validation or a statistical
significance claim. It also measures an outcome-fitted label router, not the
hand-written `mrs_prephase::plan::rules` and not a concurrent `--pre-phase` run.

### Interpretation

- The CASC-30 union shows 727/991 problems unsolved by every base strategy. After
  repairing the probe-column offset, weighted modal feature-label purity is
  0.730 on CASC-30 and 0.691 on CASC-J13. These descriptive values do not show
  that a class label identifies the best strategy.
- The corrected held-out fit is slightly ahead of the per-division order on
  CASC-30 and slightly behind on CASC-J13. The small, mixed differences from one
  deterministic split do not establish a routing advantage. The solo-matrix
  results do not measure concurrent coverage or the production pre-phase route.

---

## 4. Individual Strategy Characterization

Counts and timings use edition/problem instances, not bare problem names. A
“unique” solve is an instance solved by that strategy and by no other. Times are
the solver's `elapsed_s` for successful runs; P90 uses linear interpolation.
The CASC-30 EPU observations are censored at its official 120 s limit.

| Strategy | Configuration summary | Solves / 510 | Share | Unique | Median | Mean | P90 |
|:---:|---|---:|---:|---:|---:|---:|---:|
| **s12** | `HornHeuristic` + AgeWeight(5) + AllNegative + KBO | **300** | **58.8%** | **46** | 22.0 s | 45.9 s | 123.0 s |
| **s6** | AgeWeight(5) + KBO baseline | **269** | **52.7%** | **18** | 13.0 s | 40.7 s | 131.5 s |
| **s1** | AgeWeight(5) + AllNegative + KBO | **216** | **42.4%** | **13** | 42.8 s | 63.9 s | 166.3 s |
| **s11** | `ConjSymbolBoost` + AgeWeight(6) + AllNegative + KBO | **182** | **35.7%** | **15** | 15.2 s | 40.0 s | 119.7 s |
| **s15** | `SymbolWeight` + AgeWeight(4) + AllNegative + KBO | **153** | **30.0%** | **9** | 9.6 s | 41.4 s | 143.8 s |
| **s2** | SmallestFirst + AllNegative + KBO | **144** | **28.2%** | **8** | 5.1 s | 21.3 s | 67.5 s |
| **s4** | AgeWeight(5) + MaxNegOrMaxPos + KBO | **133** | **26.1%** | **2** | 13.9 s | 29.6 s | 84.8 s |
| **s8** | AgeWeight(10) + AllNegative + KBO | **133** | **26.1%** | **11** | 5.7 s | 25.7 s | 80.8 s |
| **s5** | AgeWeight(5) + All + KBO | **124** | **24.3%** | **2** | 17.0 s | 39.2 s | 112.1 s |
| **s14** | `ConjSymbolBoost` + SmallestFirst + All + KBO | **122** | **23.9%** | **1** | 2.4 s | 28.6 s | 128.3 s |
| **s3** | FIFO + AllNegative + KBO | **104** | **20.4%** | **1** | 5.7 s | 31.4 s | 134.2 s |
| **s7** | AgeWeight(2) + AllNegative + KBO | **99** | **19.4%** | **4** | 15.3 s | 49.3 s | 170.0 s |
| **s10** | SOS (`sos_depth=100`) + AgeWeight(12) + KBO | **75** | **14.7%** | **1** | 1.3 s | 37.3 s | 120.4 s |
| **s9** | AgeWeight(5) + AllNegative + LPO | **55** | **10.8%** | **0** | 0.6 s | 10.5 s | 23.1 s |
| **s13** | `FunctionWeightPenalty` + SOS + AgeWeight(5) + KBO | **55** | **10.8%** | **2** | 1.2 s | 31.3 s | 103.9 s |

Strategy 12 has the highest solo union and the most unique instance solves in
this matrix. This is a coverage observation; the sweep does not isolate why its
heuristic succeeds. Strategy 9 has no unique solves, but some LPO runs are as
fast as or faster than the KBO runs on shared solved instances, so a blanket
“KBO is always faster” conclusion is not supported.

---

## 5. Per-Division Greedy Candidates

The counts below keep CASC-30 and CASC-J13 instances separate, use the
competition-time EPU cutoff, and simulate strategy unions from the solo matrix.
Greedy orders are candidates only.

| Division | Instances | Oracle (all 15) | Current `casc_div` prefix 8 | Greedy prefix 8 | Greedy order (useful prefix) |
|---|---:|---:|---:|---:|---|
| FEQ | 697 | 162 | 159 | 162 | `[12, 6, 8, 13, 11, 15]` |
| FNE | 194 | 62 | 51 | 62 | `[12, 11, 15, 4, 14, 13]` |
| UEQ | 700 | 277 | 258 | 271 | `[1, 12, 8, 6, 11, 2, 7, 15]` |
| EPU | 97 | 9 | 7 | 9 | `[5]` |
| ICU | 99 | 0 | 0 | 0 | — |
| **Total** | **1,787** | **510** | **475** | **504** | — |

The full FEQ/FNE greedy prefixes cover the observed solo union. UEQ's best
8-strategy prefix leaves six of its 277 solvable instances uncovered. EPU's
single-strategy s5 coverage is measured after censoring at 120 s; the shipped
`casc_epu` prefix covers seven of those nine in this solo matrix.

---

## 6. Global Greedy Portfolio Candidate

Across all 1,787 edition/problem instances, treating each release as a distinct
input:

| Simulated 8-slot set | Coverage | Share of 510-instance oracle |
|---|---:|---:|
| Generic default `casc` (s1–s8) | 408 | 80.0% |
| Current per-division prefixes | 475 | 93.1% |
| Greedy fixed-order candidate | **496** | **97.3%** |
| Oracle (all 15) | 510 | 100% |

The greedy candidate is `[12, 6, 11, 8, 1, 15, 2, 13]`, with incremental solo
coverage gains of `+300, +88, +41, +21, +17, +14, +8, +7`. That is +88 over the
generic prefix and +21 over current per-division prefixes in this matrix. These
are not cooperative portfolio gains and are not grounds for editing
`strategy/named.rs` without the paired validation required by `AGENTS.md` §11.

---

## 7. Investigation Plan: Allocate the Eight-Core Budget by Marginal Value

The target is not “one universal portfolio of eight.” It is: for each problem,
use at most eight concurrent workers in the way that maximizes verified solves
under the division's wall-clock and memory limits. A division may use fewer than
eight *distinct* strategies in its best coverage set, while another may need
problem-dependent choices among more than eight complementary strategies.

### Phase 1 — Establish exact solo-matrix bounds

1. Key every row by `(edition, division, problem)`; do not merge same-named
   problems across releases. Deduplicate only the known CASC-30 EPU path aliases.
2. Apply the official per-division time caps, including 120 s for CASC-30 EPU.
   Treat the saved solo sweep as a candidate-generation matrix. Its raw
   `refutation` status is not a substitute for strict proof certification.
3. For each division, build each base strategy's solved set and enumerate all
   `C(15, 8) = 6,435` eight-strategy subsets from those cached sets. Also compute
   the best coverage for widths 1–7 and the all-15 union. This enumeration is
   inexpensive; it proves an optimum **for the solo matrix only**. The existing
   `greedy_set_cover` command remains a useful heuristic, not an exact optimizer.
4. Classify each division from those bounds:
   - **Compressible in the matrix:** some width `k < 8` reaches the all-15 union.
     FEQ and FNE have greedy candidates that do so with six strategies; EPU's
     s5 alone covers its nine matrix solves at the 120 s cap.
   - **Potentially width-limited:** the exact best width-8 union is below the
     all-15 union. UEQ's current greedy-8 candidate misses six of 277 union
     solves, but this does not prove that every eight-set misses them.
   - **Strategy/algorithm limited:** even the all-15 union is small or empty.
     ICU has no solve in this matrix; adding workers from the same portfolio is
     not evidence-based without a different algorithm or inference capability.

The numbers above are screening bounds. A smaller-width portfolio may still be
valuable for time-to-solution, and a solo result may not survive concurrency.
Conversely, an eight-set that misses an oracle solve may still be the best real
portfolio on the competition machine.

### Phase 2 — Test a small candidate set cooperatively

For each division, compare the shipped order with the exact best solo eight-set
and a small number of diverse near-best candidates. Do not run all 6,435 sets as
full campaigns. Use the canonical 8-physical-core host, `MRS_HARDWARE=casc`,
`jobs=1`, and the official division limit. Measure both shared equality exchange
(`MRS_SHARED_POOL_INTERVAL=500`) and the no-sharing control (`=0`).

Start with at least two randomized paired baseline/candidate blocks as required
by `AGENTS.md` §11. Report each pair delta, mean/range, per-problem flips,
resource-outs, and strict-audit results; add pairs when effects are small or
replicates disagree. Capture per-worker work/progress so “unused slot” means
measured low marginal contribution, not merely a missing unique solve in a solo
set.

### Phase 3 — Route resources, not just labels

If the cooperative results show a repeatable opportunity, test a router that
selects among candidate eight-slot plans by division and held-out problem
families. Compare in stages:

1. shipped static per-division schedules;
2. one global fixed portfolio and the per-division exact-solo candidates;
3. static-feature routing;
4. routing with a bounded, fixed-work trajectory probe;
5. eligible certified prepasses or alternative engines, with their setup time
   charged to the same official budget.

Train on some source families and hold out others before tuning. Do not use a
problem-name split across editions: the same name can denote different source
files. Include probe/prepass overhead, and compare the actual `--pre-phase` run
against the static baseline; this solo sweep did not run the shipped plan under
concurrency.

A fixed eight-core cap is a concurrency limit, not a limit on the number of
strategies considered over a whole solve. A router may time-slice more than
eight candidates or reuse a worker for a different algorithm, but every phase
reduces the remaining time available to its strategies. For a division whose
useful coverage saturates below eight distinct strategies, test assigning spare
workers to a complementary certified engine/prepass or to reduce solve time.
For a division that needs more than eight complementary candidates, test
problem-dependent eight-sets or staged schedules. In either case, measure the
cooperative run; do not assume spare cores can be transferred to another
division during a single-problem invocation. Today `schedule_from_plan` cycles a
short strategy order across all workers, so fewer distinct strategies do not
automatically leave cores idle; using those slots for another engine or prepass
requires an explicit plan/schedule design and its own cooperative test.

### Promotion gate

Do not change `strategy/named.rs` or enable pre-phase routing by default based on
the solo matrix. Promote only after paired cooperative tests improve
strictly-certified coverage on held-out families, with no `VerifiedBad` proofs
and acceptable resource behavior. EPS remains excluded from this portfolio
optimization under the §11 policy; its satisfiable/model-certified track is a
separate problem.
