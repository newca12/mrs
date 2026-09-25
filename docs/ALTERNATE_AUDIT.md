# Alternate Independent Audit — mrs CASC Competitiveness

*Independent, evidence-first audit performed 2026-09-25 on branch
`docs/alternate-audit-20260925`. This document deliberately does not reuse the
existing `docs/SOTA.md` / `docs/ARCHITECTURAL_REVIEW_VAMPIRE_GAP.md` /
`docs/TODO_CASC.md` conclusions. It re-derives the competitive gap from the raw
benchmark CSVs and the proof-audit tables, and it supersedes the headline
numbers quoted in `docs/BENCHMARKS_SUMMARY.md` where they conflict with the raw
data.*

---

## 0. Scope, evidence base, and method

| Source | What it is | Used for |
|---|---|---|
| `/home/hack/crates/mrs-bench/results/casc-{30,j13}-W8J2-*` | 8 full portfolio runs, `run.csv` + `run_meta.json` + `proof-audit` | Baseline, regressions, memory distribution |
| `docs/results/greedy_portfolios_20260925.md` | Greedy set-cover over 15-strategy solo sweeps | Achievability ceiling per division |
| `docs/BENCHMARKS.md` | `audit_casc_proofs` tables (strict / mrs / ladder) | Soundness accounting |
| `codex.db` | Historical results database | Telemetry validation coverage |
| Source inspection + local experiments on a weak NUC | Code-level root causes | Mechanism, not rate |

Two facts materially change the reading of any earlier review:

1. **The local development NUC is not benchmark hardware.** It is an
   `i3-5010U @ 2.10 GHz`, **2 physical cores** / 4 threads, 15 GB RAM, no AVX2.
   Benchmark hosts are Xeon Silver 4108 (32 cores, 64 GB); CASC StarExec
   allocates one 8-core Broadwell CPU per system with a 128 GiB `setrlimit`.
   **Any absolute rate measured on the NUC is not a competitive datum.** Only
   algorithmic findings (work per iteration, memory per clause, absence of
   deadline checks) transfer, and those are the load-bearing findings here.
2. **The previously quoted `UEQ 222/300` is not attainable.** The union of all
   15 strategies' solo coverage is **139/300** (CASC-30) and **124/400**
   (CASC-J13). The union of everything ever observed in the portfolio runs is
   **124**. The correct current figure is **122/300 measured, ~130 at HEAD
   after the greedy-order commit `b214a13`** — 94–97% of the strategy set's own
   ceiling. `222` came from a status-name match in a database in which **every
   one of the 766 claimed solves has `NULL` validation columns**.

---

## 1. The honest baseline

`casc-30-W8J2-noshare-20260922` — commit `0c41daf2b484026c266d747f3705897b53c887e5`,
binary sha256 `2abfeef5…`, host `tlpnf9701` Xeon Silver 4108 @1.80 GHz, 32 cores,
63 763 MB, `jobs=2`, `--casc-times` (fne/feq/ueq 240 s, eps/epu 120 s, icu 480 s),
`MRS_WORKERS=8`. This is the current HEAD configuration (sharing off).

| Div | mrs | Reference status mix | local Vampire | ratio | ResourceOut | GaveUp |
|---|---:|---|---:|---:|---:|---:|
| FEQ | **109**/400 | Theorem | 361 | 30% | 58 | 46 |
| FNE | **44**/100 | Theorem | 82 | 54% | 17 | 0 |
| UEQ | **122**/300 | Unsatisfiable | 243 | 50% | 14 | 7 |
| EPU | **18**/100 | Unsatisfiable | 76 | 24% | 1 | 17 |
| EPS | **18**/100 | Satisfiable | 86 | 21% | 5 | 34 |
| ICU | 5/101 | Theorem | 53 | 9% | 44 | 3 |
| **total** | **316**/1101 | | **901** | **35%** | **139** | 104 |

CASC-J13, `casc-j13-W8J2-nosharing-20260923`: FEQ 69/300, FNE 34/100, UEQ 112/400
= 215/800. ICU is out of scope for this audit by decision.

### 1.1 All eight W8J2 runs, and the two regressions they contain

| Run | FEQ | FNE | UEQ | EPU | EPS | ICU | ResourceOut (all divs) |
|---|---:|---:|---:|---:|---:|---:|---|
| casc-30-20260914 | 113 | 43 | 81 | 18 | 17 | 4 | **0** |
| casc-30-20260917 | 107 | 44 | 81 | 18 | 16 | 3 | 187 |
| casc-30-20260921 | 107 | 45 | 82 | 18 | 16 | 3 | 184 |
| casc-30-noshare-20260922 | 109 | 44 | **122** | 18 | 18 | 5 | 139 |
| casc-j13-20260914 | 72 | 35 | 78 | | | | **0** |
| casc-j13-20260917 | 68 | 34 | 80 | | | | 73 |
| casc-j13-20260921 | 68 | 34 | 80 | | | | 77 |
| casc-j13-nosharing-20260923 | 69 | 34 | **112** | | | | 45 |

EPS counts `Satisfiable`; the other divisions count `Theorem`/`Unsatisfiable`.

Two discontinuities, both now explained:

* **The "81 score regression" is clause sharing.** Sharing-on 82 vs sharing-off
  **122** on CASC-30 UEQ: **42 problems lost, 2 gained, union 124.** The 42 lost
  problems have a **median peak memory of 21.5 GB** (max 29.7 GB) and end as
  27 `Timeout`, 14 `ResourceOut`, 1 `GaveUp` with sharing enabled. The shared
  pool publishes a *full ancestor chain* per unit equality and every one of the
  8 workers splices it into its own `clause_store` / `proof_arena` / `children`
  (`crates/mrs-search/src/given_clause.rs:1136-1262`) — a memory multiplier
  aimed exactly at the problems already at the wall. Sharing is already
  default-off (`ce7bdc5`, `3e962a8`); **that default is correct and should be
  kept. The pool should be capped hard or removed, not re-tuned.** The
  interval sweeps in `docs/BENCHMARKS_SUMMARY.md` §4 are measuring noise.
* **Resource ceilings cost FEQ solves.** From 20260917 onward 184–187 rows are
  `ResourceOut` and FEQ drops 113 → 107. The containment is doing its job, but
  it is firing at a threshold set far below the available memory (§2.1).

---

## 2. Root causes, ranked by measured effect

### 2.1 A hard-coded 14 GB process ceiling is the single largest blocker

`crates/mrs-search/src/resource.rs:23-40`:

```rust
// Default to 80% of total system memory, capped at 14 GB (standard for 16GB CASC node).
let mb = (kb / 1024) * 80 / 100;
return Some(mb.clamp(1024, 14336));
```

`SearchConfig::resource_limits.max_memory_mb` defaults to this, and
`given_clause.rs:1327-1338` returns `SearchResult::ResourceOut` once
`current_memory_mb() >= limit`. The clamp is wrong on both available targets:
the bench host has 64 GB, and `docs/HARDWARE.md` §4 records that CASC StarExec
caps a run at **128 GiB**.

The effect is visible in the raw data. Across the whole `noshare-20260922` run:

| Population | n | median peak | min | p75 |
|---|---:|---:|---:|---:|
| `ResourceOut` | 139 | **25 046 MB** | 16 925 MB | 28 157 MB |
| solved | 298 | 3 084 MB | — | p95 = 15 374 MB |
| `Timeout` | 464 | 13 325 MB | — | — |

Not one `ResourceOut` row peaks below 16.9 GB: they are all the 14 GB ceiling
firing, with 3–16 GB of overshoot on top. The overshoot exists because the
watchdog is sampled only every 100 iterations
(`crates/mrs-search/src/given_clause.rs:1265`) while a single iteration can
allocate gigabytes (§2.3).

The decisive measurement — FEQ solve rate against peak memory:

| peak memory | solved | outcome |
|---|---:|---|
| 0–1 GB | **0/30** | 30 `Timeout` |
| 1–5 GB | **74/74** | 100% `Theorem` |
| 5–15 GB | 28/108 | 34 `Timeout`, 46 `GaveUp` |
| 15–30 GB | **7/182** | 52 `ResourceOut`, 49 `Timeout`, 74 `GaveUp` |
| >30 GB | 0/6 | 6 `ResourceOut` |

**Every problem mrs fits in 1–5 GB, it proves.** 182 of 400 FEQ problems (45%)
land in the 15–30 GB band where it wins 4%, and the machine those runs used
had 64 GB with `jobs=2`. This is not a search-quality gap; it is a
footprint-versus-allocation gap.

### 2.2 Per-clause memory footprint is why §2.1 binds

~3 KB per clause, from four compounding causes:

1. `mrs_core::Term` is a tree with a `Vec` at every node
   (`crates/mrs-core/src/term.rs:35-40`): `App(SymbolId, Vec<Term>)`. A
   depth-5 term is ~6 heap allocations. Terms are only interned inside
   `TermBank`, *after* the whole legacy pipeline has run on trees.
2. `SearchState` retains every clause ever created, in three structures:
   `clause_store`, the append-only `proof_arena`, and `children`
   (`crates/mrs-search/src/state.rs:35-37`, `register_clause` at `:415`).
   `register_clause` is called for **every intermediate** demodulation, DER
   step, condensation, and subsumption-resolution step, so the store grows at
   roughly 5–10 clauses per generated clause, forever.
3. Each of the 8 workers deep-clones the input **and the whole FOF provenance
   DAG**: `clauses_owned.clone()` and `provenance.to_vec()` inside the worker
   loop at `crates/mrs-search/src/strategy.rs:907-908`.
4. The shared pool adds an 8× multiplier on top when enabled (§1.1).

Measured directly: `EPU/HWV092-1` (696 691 CNF clauses) reaches **2.3 GB RSS
before the search loop starts** and 4.9 GB with a single worker — on a 2-core
NUC where that is not a throughput artefact but a data-structure property.
Division medians from the real run: FNE 11.3 GB (p90 29.7, max 54.1), UEQ
11.3 GB (p90 21.5, max 47.1), EPU 2.1 GB, EPS 2.1 GB.

Leaders carry flat 32-bit ID arrays (a few hundred MB at this size) and share
the read-only input across portfolio members.

### 2.3 Unbounded work per given-clause iteration, and no deadline checks

`crates/mrs-search/src/given_clause.rs` checks `start.elapsed()` at lines
1266, 1360, 1592, 1597, 1670, 1766, 1916, 1968 (and touches the deadline via
`state.search_deadline` at 470, 1070, 2172) — and **nowhere in the new-clause
pipeline at lines 2205–2515**, nor in the factoring / equality-resolution /
equality-factoring block at 1776–1791. A single given clause can therefore run
arbitrarily long and the run emits **no SZS status line at all**.

Found by bisection on `FNE/LCL682+1.020.p`: with `--time 2` and `--time 3` the
run returns `Timeout` after 624–1385 iterations; with `--time 4`, `5`, `15`,
`20` it never returns. The stalling iteration is given clause `921` with 21
literals producing 177 new clauses. Stage instrumentation attributed the
time to **condensation**:

`crates/mrs-calculus/src/subsumption.rs:576 condense_id` loops over all
literal pairs and, for each, calls `subsumes` (`:237`), which is a
backtracking search (`match_literals`, `:261`) that clones a `Substitution` per
attempt. Cost measured: **1.7 s per 20-literal clause** × 177 clauses ≈ 5
minutes for one given-clause iteration. The existing guard only rejects
clauses wider than 50 literals, with a comment claiming `O(N³)` in width
(`:583-585`) — the real behaviour is `O(N² × backtracking)`.

Verification of the mechanism: lowering the cap from 50 to 8 literals made
`LCL682+1.020` return a normal `Timeout` in 19.1 s having completed 1385
iterations, and cut silent hangs in a 200-problem sample from 4 to 2. The
regression cost one borderline FNE solve (`KRS260+1`, 8/40 → 7/40), which
argues the bound must be **cost-based, not a fixed literal count**.

Related, smaller: the front end has no budget. 19 of 200 sampled problems
complete ≤5 given-clause iterations in a 2 s budget — all in the HWV/LAT EPU
family, where clausification plus indexing alone consumed the budget.

### 2.4 EPS/EPU is a capability gap, not a memory gap

EPS: 18 `Satisfiable`, 34 `GaveUp`, 43 `Timeout`, 5 `ResourceOut`; median peak
2.1 GB. These are *small* problems that mrs declines. The `GaveUp` share is
fail-closed by design (`crates/mrs-search/src/given_clause.rs:2543` returns
`GaveUp` for all ordinary saturation; `src/main.rs:898-910` demotes even
certified saturation unless it is one of two narrow reasons). Sound, but it
means ~34 EPS and ~17 EPU problems are unreachable by construction. Fixing
this means InstGen/FVO coverage, not tuning.

### 2.5 Search-selection observations (lower priority, partly self-inflicted)

* `UnprocessedSet::push` (`crates/mrs-search/src/unprocessed.rs:97-179`) pushes
  every clause into up to **six** priority structures (age, weight, goal, unit,
  horn, sos) regardless of the strategy's selection function, and relies on
  lazy tombstones for deletion.
* `pop_weight_sos` (`:221-252`) inspects a bounded 32-entry window and falls
  back to `pop_age()`, so the SOS strategies degrade toward FIFO whenever the
  window misses — i.e. much of the time.
* LRS (`given_clause.rs:1265-1289`, `unprocessed.rs:335-415`) keeps the
  *weight-sorted prefix* and re-runs a full rebuild every 100 iterations. In a
  representative 10 s FEQ run it discarded 43 819 clauses against 53 888
  generated — **81% of all derived clauses thrown away**, with the passive queue
  pinned near 10 k.
* There is **no inference-restriction criterion** anywhere in
  `crates/mrs-calculus/src/` (module list: demodulation, equality, factoring,
  lib, literal_selection, ordering, rename, resolution, subsumption,
  superposition). No restricted/subsumption-based superposition, so every
  inference whose result is immediately subsumed is still paid for.

### 2.6 Telemetry cannot currently be tuned against

`stats.generated` is incremented only after a clause survives the entire
forward-simplification pipeline (`given_clause.rs:2513`), so discarded
clauses are never counted. Real outputs: `FEQ/HWV090+1` reports
`processed=15489 generated=1714 lrs_discarded=130786`; `FEQ/SWW337+1` reports
`generated=53888 lrs_discarded=43819 passive=10220`. `generated < processed`
with a six-figure `lrs_discarded` is not interpretable. Portfolio, sharing, and
LRS tuning in the current roadmap is therefore tuning on inconsistent counters.

### 2.7 The scorer reports sound solves and unverified solves identically

* `run.csv` has **no proof-validation columns** (fields are `edition`,
  `division`, `problem`, `system`, `timeout`, `szs_status`, `expected`,
  `verdict`, `wall_time_s`, `peak_memory_mb`, `failure_detail`, raw-output
  paths and hashes). `verdict` is a status-name string match.
* `codex.db`: **all 766 claimed solves across every division have
  `proover_validated = kernel_validated = mrs_validated =
  competition_validated = NULL`.** The 222 UEQ rows all read
  `verdict = 'ok'` with zero validation. The `ICU` rows are worse: their
  `expected` field mixes `GaveUp` / `Theorem` / `Satisfiable` with
  `verdict = 'unknown'`, so the reference mapping for that division is broken.
* The `audit_casc_proofs` tables in `docs/BENCHMARKS.md` show the strict
  kernel and the independent external-ATP ladder disagreeing sharply:

  | Div | solves | strict Good/**Bad**/Unknown | ladder (Vampire/E, independent) Good/**Bad**/Unknown |
  |---|---:|---|---|
  | FEQ (casc-30) | 113 | 15 / **64** / 33 | 53 / **6** / 45 |
  | FNE (casc-30) | 43 | 9 / **24** / 10 | 19 / **0** / 16 |
  | UEQ (casc-30) | 81 | 32 / **0** / 49 | **75** / 0 / 0 |
  | EPU (casc-30) | 18 | 17 / 0 / 1 | **18** / 0 / 0 |
  | FEQ (casc-j13) | 72 | 16 / **30** / 26 | 46 / **4** / 15 |
  | FNE (casc-j13) | 35 | 5 / **21** / 9 | 10 / **0** / 23 |
  | UEQ (casc-j13) | 78 | 41 / **0** / 37 | **64** / 0 / 0 |

  Read against an independent oracle, mrs has **0 refuted proofs in UEQ, FNE,
  EPU and 6/113 in FEQ**. The strict kernel's large `VerifiedBad` counts are
  overwhelmingly *checker gaps*, not refuted proofs — consistent with the recent
  commit trail, which is all "Unknown → VerifiedGood" gap-filling (skolem
  witness reuse, expanding demodulation replay, definition-heavy CNF). The
  `mrs` self-check mode cannot see these because it uses mrs as its own ATP
  oracle.

---

## 3. The portfolio axis is exhausted in FNE and UEQ

From `docs/results/greedy_portfolios_20260925.md`, the union of all 15 strategies'
solo coverage:

| Div | greedy set covers | mrs actual | fraction of own ceiling |
|---|---|---:|---:|
| FNE CASC-30 | 42/42 | 44–45 | ≥100% |
| FNE CASC-J13 | 35/35 | 34–35 | ~97% |
| UEQ CASC-30 | 136/139 | 122 (HEAD ~130) | ~90–94% |
| UEQ CASC-J13 | 122/124 | 112 (HEAD ~120) | ~90–97% |

The remaining gap to Vampire in these two divisions (FNE 82−45, UEQ 243−130) is
**not reachable by any rearrangement of the existing strategies**: those problems
are not solved by any single strategy solo. Consequently the following are
retired as productive work until §2.1–§2.3 are fixed:

* greedy set-cover / one-swap cooperative portfolio search,
* shared-pool interval sweeps,
* ML schedule selection (also already recorded as a negative result in
  `docs/BENCHMARKS.md` — a retrained model reached AUC ≈ 0.84 and proving
  performance still fell).

Every remaining problem in FNE/UEQ is a **capability** gap: a new or materially
better strategy is required, and it has to be validated cooperatively because
solo set-cover systematically overstates portfolio coverage.

---

## 3a. Low-RAM work log (branch `perf/low-ram-and-redundancy-bounds`)

Work done against the low-RAM deployment goal, after the analysis above.
Commit `ac62d0f`. Local measurements are from the 2-core NUC and are
**throughput/failure-mode measurements, not score predictions**; the CASC
score effect still has to be measured on a benchmark host.

### Silent hangs, the dominant EPS failure mode

CASC-30 EPS `noshare-20260922` records **10 of 100 problems at 120 s wall,
0 MB peak, no telemetry** — no SZS status line at all. Bisected on
`EPS/HWV042-1` (889 CNF clauses, 76 KB):

* **6 of the 15 strategies do not terminate** (s4, s7, s8, s11, s12, s15).
* The EPS portfolio order `2,3,1,8,11,12,9,14` contains **three of them**
  (8, 11, 12), and one non-terminating worker blocks the whole portfolio
  through the `thread::scope` join, discarding the results of the seven
  workers that finished in 10 ms.
* The cause was the new-clause simplification pipeline
  (`given_clause.rs`) and the unary-inference block having **no deadline
  check at all**, so a single given-clause iteration could run for minutes.

Deadline checks are now enforced per new clause, per subsumption-resolution
pass, and inside the resolution/superposition candidate-collection loops.
Result: **49 of 50 sampled problems across FNE/FEQ/UEQ/EPS/EPU always emit
a status**; the 10 archived EPS cases now report `GaveUp` or `Timeout`.

### The quadratic stage behind it

Instrumenting the per-stage cost of the new-clause pipeline on
`EPS/HWV042-1` attributed the stall to forward subsumption resolution:
**410 ms per new clause**, 235 new clauses in one iteration, i.e. ~96 s for
a single given-clause iteration. The trace also showed the actual cause: a
**202-literal clause** with **321 SR candidates**.
`subsumption_resolution_id` builds a modified copy of the target for every
literal it tries to remove, so one pass costs `O(width^2)` per candidate and
repeats per literal removed — about 13 M literal copies per pass.

Three changes, all in the redundancy path and none of which can affect
refutational completeness:

| Change | Effect on the pathological iteration |
|---|---|
| candidate retrieval returns IDs, callers borrow (no clone + sort) | 410 ms → 205 ms |
| per-predicate posting list restricts SR candidates to clauses sharing a symbol with the target (a necessary condition for SR) | included above |
| `SearchConfig::max_subsumption_resolution_literals` (default 20) skips SR on wider clauses | 205 ms → **0.7 s total** |

`FNE/LCL682+1.020` had the same shape via condensation
(`condense_id`: a backtracking subsumption test per literal pair, guarded
only by a fixed 50-literal cap whose comment claimed cubic cost). It is now
`SearchConfig::max_condensation_literals` (default 10) and returns a clean
`Timeout` instead of never returning at any `--time`.

### Backward subsumption resolution was dead code

The new differential assertion in `index_equivalence.rs` cross-checks the
symbol-filtered queries against the exact oracle and **failed against the
existing code**. The search calls
`subsumption_resolution_id(candidate, given)`, which requires the candidate
to be no wider than `given`, but the feature-vector query filtered for the
opposite direction, so backward SR only ever fired on equal-width clauses.
The criterion is restored. This is a **correctness repair, not a speedup**:
the measured clause-count effect on `UEQ/ALG213-10` is within noise
(26 370 vs 26 328 generated), and it is reported as such.

### Memory policy for constrained hosts

| Before | After |
|---|---|
| `system_memory_limit_mb` clamped the ceiling to **14 GB** ("standard 16GB CASC node"), terminating runs with tens of GB of headroom on the 64 GB host | `memory_budget_mb`: `MRS_MAX_MEMORY_MB`, else **80% of `MemAvailable`**, no upper clamp |
| default workers = physical cores, so a 4 GB host runs one full state per core into the watchdog | `default_worker_count(cores)`, bounded by `memory_budget_mb / 2 GiB` per worker; explicit `--workers` still wins, so competition runs are unchanged |
| LRS memory pressure triggered at a hard-coded **1 GB** | triggers at 80% of the configured ceiling |

### Verification

* `cargo test --workspace`: 63 test binaries, 0 failures.
* `cargo clippy --all -- -D warnings`: clean. `cargo fmt --all --check`: clean.
* Controlled before/after, 40 CASC-30 FNE problems, single strategy, 10 s,
  same machine: **8 → 9 refutations**, with all 8 previously solved problems
  still solved (`KRS258+1` gained).

### Open item: one remaining non-termination

`UEQ/GRP655-14` (6 CNF clauses) still never returns under strategy 8, and
is now the only problem in the 50-problem sample that emits no status.
Localisation is complete but the fix is not:

* 100% CPU, single thread, flat RSS (~36 MB), no AVATAR activity.
* Dies in the **superposition** phase of one given-clause iteration.
* Collection completes: 51 candidates, sorted fine; candidates 0–43 each
  take **microseconds**; candidate 44 does not return.
* Both clauses are small: the equation source has **expanded** term size 9,
  the target 85. Not term-size explosion, and not the symbol-sharing filter.
* The hang is *inside* one call to `superpose_selected_id`, and adding a
  deadline check to its innermost position loop did not fire, so a single
  position's work never returns.

Next step for whoever picks this up: reduce `EPS`/`UEQ/GRP655-14` to a unit
test by capturing the (equation source, candidate 44) pair, then instrument
`unify_ac_id`, `sigma.apply_term`, `bank.replace_at` and
`ordering.compare_id` inside `superpose_with_id` to find which one loops.
`superpose_selected_id_until` is in place so that a fix can return partial
results at the deadline, and callers already treat that as search over
budget.

---

## 4. Plan

Ordered by measured effect per unit of risk. Each phase has a gate that can
only be evaluated on benchmark hardware.

**P0 — Release the memory ceiling and prove the effect (hours).**
Replace the `14336` clamp in `crates/mrs-search/src/resource.rs:39` with a
harness-supplied per-run cap (`MRS_MAX_MEMORY_MB`, sized to the host and to
`jobs`), and sample the watchdog per iteration instead of every 100. Re-run
**FEQ only**, `jobs=1`, pinned to one 8-core slice.
*Gate:* the 15–30 GB band's solve rate must exceed 4%, and `ResourceOut` must
fall below 20 rows. This is one line plus a sampling change and needs no new
algorithm.

**P1 — Bounded per-iteration work (1–2 days).**
Add deadline checks through the new-clause pipeline and the unary-inference
block; replace the fixed 50-literal condensation cap with a cost bound.
*Gate:* no run may end without an SZS line; a 200-problem sample shows zero
silent hangs; no division loses a previously solved problem on a repeated run.

**P2 — Flat-ID representation (the project).**
Remove `Term::App(_, Vec<Term>)` from the search path and intern once; share
the read-only input across workers instead of `clauses_owned.clone()` +
`provenance.to_vec()` per worker; stop retaining simplification intermediates
in `clause_store`/`proof_arena`; delete or hard-cap the shared pool.
*Gate:* `EPU/HWV092-1` below 1 GB per worker; 74/74 in the 1–5 GB FEQ band still
holds; the 5–15 GB band moves toward 100%.

**P3 — Soundness accounting.**
Split strict-kernel `VerifiedBad` into *refuted* and *checker-gap* with a
mandatory reason code; adjudicate the 6 FEQ ladder-bad proofs first; add
validation columns to `run.csv` and make the ladder-certified count the
headline number, with raw solves reported beside it. Retire `codex.db` headline
figures that have no validation rows.
*Gate:* every reported solve carries Good/Unknown/Bad; zero *refuted*.

**P4 — Search quality.**
Restricted/subsumption-based inference restriction; then capability work for
EPS/EPU and a genuinely new strategy for the FNE/UEQ problems that no current
strategy touches.

Deliberately deferred: ICU / satisfiability (no model finder — saturation always
demotes to `GaveUp`), ML schedule selection, shared-pool tuning.

---

## 5. Open decisions

1. **May P0 proceed immediately** (one-line clamp change + FEQ re-run), given
   that it is the cheapest test of the largest hypothesis in this document?
2. **Core allocation for measurement.** CASC grants one 8-core CPU per system.
   The available hosts are 2 × 8-core and 2 × 16-core. Pinning **four
   concurrent 8-core runs** would match StarExec and remove the `jobs=2`-on-64 GB
   memory contention that can turn a solvable problem into a `ResourceOut`.
   This needs a decision on the pinning mechanism (`numactl` / `taskset`) and
   whether `casc.sh` should grow a pinning flag.
3. **P2 scope** — full term-representation rewrite, or a cheaper first step
   (drop the per-worker `provenance.to_vec()`, stop retaining intermediates)
   that may recover most of the 8× multiplier without touching `Term`?
4. **Reporting policy** — should `docs/BENCHMARKS_SUMMARY.md` be corrected in
   place to the measured 122/44/109/18/18 baseline, given that its UEQ 222 and
   419/1039 totals are contradicted by the raw CSVs?

---

## 6. Reproduction

```bash
# Honest baseline and regressions, per division, from the archived runs
for d in /home/hack/crates/mrs-bench/results/casc-*W8J2*; do
  printf '%-34s ' "$(basename "$d")"
  python3 - "$d/run.csv" <<'PY'
import csv,sys,collections
S={'Unsatisfiable','Theorem','Satisfiable','CounterSatisfiable'}
c=collections.Counter(); ro=collections.Counter()
for r in csv.DictReader(open(sys.argv[1])):
    c[(r['division'],r['szs_status'] in S)]+=1
    if r['szs_status']=='ResourceOut': ro[r['division']]+=1
divs=sorted({k[0] for k in c})
print('  '.join(f"{dv}:{c[(dv,True)]}/{c[(dv,True)]+c[(dv,False)]}" for dv in divs))
print('    ResourceOut', dict(ro))
PY
done

# FEQ solve rate against peak memory (the decisive table, §2.1)
python3 - /home/hack/crates/mrs-bench/results/casc-30-W8J2-noshare-20260922/run.csv <<'PY'
import csv,sys,collections
S={'Unsatisfiable','Theorem'}
mem=lambda r: float(r['peak_memory_mb'] or 0)
rows=[r for r in csv.DictReader(open(sys.argv[1])) if r['division']=='feq']
for lo,hi in ((0,1e3),(1e3,5e3),(5e3,15e3),(15e3,30e3),(30e3,1e9)):
    sel=[r for r in rows if lo<=mem(r)<hi]
    s=sum(1 for r in sel if r['szs_status'] in S)
    print(f'{lo/1e3:6.0f}-{hi/1e3:6.0f} GB  {s:3d}/{len(sel):3d}  {dict(collections.Counter(r["szs_status"] for r in sel))}')
PY

# The 14 GB ceiling in situ
grep -n "clamp(1024, 14336)" crates/mrs-search/src/resource.rs

# The unconditional new-clause pipeline and its missing deadline checks
sed -n '2205,2520p' crates/mrs-search/src/given_clause.rs | grep -c "start.elapsed()"   # -> 0
grep -n "start.elapsed()" crates/mrs-search/src/given_clause.rs | cut -d: -f1 | tr '\n' ' '
# -> 1266 1360 1592 1597 1670 1766 1916 1968   (all before line 2205 except 1968)

# The condensation / subsumption cost structure
sed -n '576,600p' crates/mrs-calculus/src/subsumption.rs
sed -n '237,262p' crates/mrs-calculus/src/subsumption.rs

# Per-worker deep clone of clauses and the provenance DAG
sed -n '900,912p' crates/mrs-search/src/strategy.rs

# Unvalidated solves in the historical database
sqlite3 -header -column codex.db \
  "select division, corpus, count(*) solves,
          sum(case when kernel_validated is null and proover_validated is null
              and mrs_validated is null and competition_validated is null
              then 1 else 0 end) unvalidated
     from results where status in ('Theorem','Unsatisfiable')
    group by division, corpus;"
# -> every row reports unvalidated == solves (766/766)

# Hard hang with no SZS line (needs a generous external timeout)
timeout 60 ./target/release/mrs --time 20 --workers 1 --strategy 1 \
  crates/mrs-bench/problems/casc-30/FNE/LCL682+1.020.p ; echo "exit=$?"
```
