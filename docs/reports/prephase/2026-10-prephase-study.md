# The mrs pre-phase: what a deep analysis of a problem can and cannot tell you

*Experimental study. Branch `exp/prephase-study`. Nothing described here changes
what `mrs` does by default: the routing it feeds lives behind `--pre-phase` and
is off.*

## 1. What was built

A pre-phase is a bounded analysis of a problem, run before the first inference,
whose job is to decide which algorithm and which parameters to use. `mrs` already
needs this decision and already has five algorithms to choose between; it makes
the choice with hand-written rules that nobody had measured. This study
measures.

| Component | Where | What it is |
|-----------|-------|------------|
| `mrs-prephase` | `crates/mrs-prephase/` | ~80 structural features over the clausified problem, a five-axis class taxonomy, and a declarative routing table |
| `prephase_dump` | `crates/mrs-bench/src/bin/prephase_dump.rs` | one CSV row of features per TPTP file, with per-file process isolation |
| `prephase_sweep` | `crates/mrs-bench/src/bin/prephase_sweep.rs` | the label: a configuration × problem outcome matrix with time-to-solution |
| `analyze.py` | `crates/mrs-bench/prephase/analyze.py` | joins the two; coverage, set cover, conditional coverage, portfolio simulation, cross-validation, exact sign tests |
| `ab.sh` | `crates/mrs-bench/prephase/ab.sh` | cooperative A/B of the pre-phase against the shipped per-division portfolio |
| `mrs-search::prephase` | `crates/mrs-search/src/prephase.rs` | plan → `SearchConfig`, plus the bounded search-behaviour probe |
| `--pre-phase`, `--list-rules` | `src/main.rs` | the in-binary wiring, off by default |

Data: the full 26,990-problem TPTP-v9.3.0 distribution analysed; the CASC-30
corpus (2,901 problems) analysed, 600 of them labelled across all 15 base
configurations at a 3 s budget (9,000 runs), and 127 of them — every problem the
3 s sweep solved, plus a label-stratified sample of those it did not — re-measured
across 7 configurations at 30 s (889 runs).

## 2. Headline results

1. **A fifth of the entire TPTP corpus cannot be represented at all.** 5,622 of
   26,990 problems (20.8%) lower to an empty clause set, plus 245 (0.9%) exhaust
   a 2.5 GiB address-space ceiling during clausification. In CASC-30 that is
   1,500 of 2,901 (51.7%): the whole SLH division (1,000 problems), all of TEQ
   (400) and all of TNE (100). No routing decision can help a problem the front
   end cannot express, and this is a larger effect than anything routing could
   produce.

2. **A usable taxonomy of the input space exists, and it is coarse.** The
   five-axis label `logic/shape/scale/goal/decomposition` takes 172 distinct
   values over the 26,726 analysable problems, and **54 of them cover 95.9% of
   the corpus**. The three largest classes are `EMPTY` (21.0%),
   `FEQ/NON_HORN/SMALL/TIGHT/CONNECTED` (17.5%) and
   `FEQ/NON_HORN/MEDIUM/TIGHT/CONNECTED` (8.4%).

3. **The post-clausification class does not recover CASC's division taxonomy,
   and cannot.** CASC classifies on the *input*; the analysis sees the
   *clausified* clause set. Agreement with the modal class is 99% for EPS and
   96-100% for FNE/FEQ/UEQ, but 83% for ICU, 70% for TFI and 47% for TFN. Over
   the whole TPTP distribution the agreement is much worse for content domains:
   `GRP` (group theory) is 44% UEQ and 42% FEQ; `CSR` is 50% FEQ and 35% FNE;
   `SYN` is 54% EPR and 31% FNE. A router keyed on the clausified class is not
   making a mistake — it is answering a different question than CASC's taxonomy.

4. **Routing on the syntactic class loses to the fixed per-division portfolio,
   decisively, at both budgets measured.** At 8 slots on the 600-problem 3 s
   sample, the per-division baseline covers 41 and a routing table fitted on one
   half and evaluated on the other covers 29. Every one of the 14 problems the
   baseline wins is one routing does not, and routing wins none: 0 vs 14
   discordant, two-sided p = 0.0001. Re-measured at 30 s on the hard tail (§6),
   the per-division order stays within 3 problems of the greedy optimum at every
   portfolio width, and every hand-written structural rule the measurement
   supports is *negative* on the subset it fires on.

5. **The reason routing loses is visible in the label distribution.** Of 600
   problems, 557 (92.8%) are solved by *no* configuration at 3 s and 13 (2.2%)
   are solved by *all fifteen*. Only 21 problems (3.5%) are in the regime where
   choosing between configurations matters at all. A router fitted on that data
   is mostly learning to predict "solved by nobody", which carries no information
   about which configuration to run.

6. **The one strategy signal that does survive is about scale, not about
   class.** The configurations that uniquely solve *large* problems — and only
   those — are the ones with AVATAR off and the weight cap off or low: s14
   (cap 100), s15 and s2 (uncapped), s12 (uncapped, Horn-preference). AVATAR-on
   configurations account for 1 problem no AVATAR-off configuration can solve at
   >= 2k clauses, against 5 in the other direction. And s12 — Horn-preference
   KBO, uncapped, AVATAR off — is the single broadest configuration measured
   (28 of 600), ahead of s8 (21), and no shipped `casc_*` schedule leads with it.

## 3. The taxonomy

`Analysis::label` is `logic/shape/scale/goal/decomposition`. The five axes, and
what each one separates:

| axis | values | distribution (26,726 problems) |
|------|--------|-------------------------------|
| `logic` | `UEQ` `PEQ` `EPR` `FNE` `FEQ` `EMPTY` | FEQ 51.5%, EMPTY 21.0%, FNE 11.9%, UEQ 9.0%, EPR 6.5% |
| `shape` | `HORN` `DUAL_HORN` `STRATIFIED` `NON_HORN` `GOAL_ONLY` `EMPTY` | NON_HORN 57.3%, HORN 17.8%, DUAL_HORN 2.2%, GOAL_ONLY 1.7% |
| `scale` | `TINY` `SMALL` `MEDIUM` `LARGE` `HUGE` `EMPTY` | SMALL 28.0%, TINY 25.5%, MEDIUM 15.0%, LARGE 5.4%, HUGE 5.0% |
| `goal` | `TIGHT` `LOOSE` `BACKGROUND` `NO_GOAL` `EMPTY` | TIGHT 65.1%, BACKGROUND 8.0%, NO_GOAL 5.4%, LOOSE 0.5% |
| `decomposition` | `CONNECTED` `SINGLE_GOAL` `MULTI_GOAL` `EMPTY` | CONNECTED 68.7%, SINGLE_GOAL 9.9%, MULTI_GOAL 0.4% |

Three of the five axes are worth a comment on *why the thresholds are where
they are*, because each is a point where a search heuristic changes behaviour
rather than a round number:

* `scale` puts `MEDIUM` at 257 clauses, where SInE and blocked-clause
  elimination start to remove a non-trivial fraction of the premise set — the
  same reason `ProblemProfile::is_large_theory` uses 150 axioms.
* `goal` splits at 80% and 25% goal-reachable clauses. This is not a syntactic
  measure: it is the fraction of non-goal clauses with at least one symbol inside
  the goal radius under the *same* symbol-distance BFS the engine's own
  goal-distance map uses. It is the single most direct estimate of how much of
  the input a refutation can possibly touch.
* `decomposition` counts connected components of the clause/symbol incidence
  graph, union-find over symbols shared between clauses. A component the
  conjecture does not touch is dead weight for a refutation, and one the conjecture
  does touch is a separate sub-problem — which is what componentwise refutation
  and AVATAR splitting exist to exploit.

### The classes the analysis gets most confidently wrong

`goal_reachable_ratio` is 1.0 for 65% of the corpus, so `TIGHT` is a weak
discriminator on the corpus as a whole. The `BACKGROUND` class (8.0%) is the one
worth having: those are the problems where premise filtering has something to do.

The `logic` axis has a systematic blind spot worth naming: a problem whose *input*
is propositional can clausify into a clause set with function symbols (definitional
CNF introduces Skolem functions), and a typed problem can clausify into a set whose
type axioms dominate. That is why `TFN` is only 47% FNE after clausification.
The analysis reports what the search will see, which is the right thing for
routing and the wrong thing for reproducing CASC's labels.

### Measured redundancy

`Analysis::n_redundant_removed` is not a prediction: the analysis runs the
engine's own tautology / pure-literal / blocked-clause reducer and reports how
many clauses it removed. It is the one "feature" that is a measurement of the
engine rather than of the input, which is why it is kept separate.

## 4. Labelling: how the outcomes were measured

`prephase_sweep` runs one configuration, one worker, the whole budget, sharing
off — the same thing `systems/mrs-sNN` measures, so the numbers are comparable
with anything `run_strategy_sweep.sh` already produced.

The measurement rests on one equivalence, worth stating because it is what makes
a solo sweep sufficient to evaluate a portfolio:

> Portfolio workers run concurrently, so each slot receives the whole wall clock.
> A solo run at the full budget *is* a portfolio slot. The union of the selected
> configurations' solved sets is therefore the portfolio's coverage, with no
> further measurement needed.

That turns a portfolio question into a set-cover question over already-measured
sets. The simulation in `analyze.py` is exact, not an approximation.

## 5. Why routing loses, in detail

### Coverage is too thin at 3 s

| | count |
|---|---|
| solved by all 15 configurations | 13 (2.2%) |
| solved by 1 configuration | 7 (1.2%) |
| solved by 2–3 | 14 (2.3%) |
| solved by 4–7 | 9 (1.5%) |
| solved by 8–15 | 7 (1.2%) |
| solved by none | 557 (92.8%) |

Only the 21 problems in the first four rows are in the regime where the choice
of configuration changes the answer. Fitting a routing table on 600 problems in
which 93% of the outcomes are identical ("unsolved") produces a table that
predicts the unsolved class and is free to choose any order within it. That is
what the fitted table does, and it is why it lands on configurations that are
good at the trivial 13 and bad at everything else.

The cross-validation number is the honest one: **29/600 held out, against 41/600
for the per-division baseline**. The fitted-on-everything number (34/600 at 12
slots) is not much better, which is itself diagnostic: the fit is not
over-parameterised enough for overfitting to be the whole story. The table is
simply being fitted to a target that does not vary.

### The per-division orders are already good, and greedily so

The `casc_*` orders are greedy set-cover orders derived from earlier solo sweeps.
A greedy set-cover order over *all* problems is, by construction, at least as
good as any single fixed order at every prefix length. A routing table can only
beat it by exploiting variation the global order cannot see — and at this
coverage there is not enough variation to exploit.

### What the conditional-coverage table does say

Conditional coverage per feature bucket, refutation divisions only:

| axis | bucket | n | union | leading configurations |
|------|--------|---|-------|------------------------|
| `shape` | `HORN` | 159 | 31 (19.5%) | s2=20, s12=19, s4=17, s5=17 |
| `shape` | `NON_HORN` | 275 | 12 (4.4%) | s12=9, s6=7, s8=6, s11=4 |
| `max_term_depth` | `6-10` | 83 | 19 (22.9%) | s4=9, s8=9, s12=9, s5=8 |
| `max_term_depth` | `>10` | 34 | 0 (0%) | — |
| `max_term_depth` | `1-2` | 340 | 9 (2.6%) | s12=8, s13=8, s4=7, s6=7 |
| `n_components` | `2-3` | 14 | 6 (42.9%) | s2=4, s12=4, s8=2, s10=2 |
| `n_components` | `4+` | 31 | 4 (12.9%) | s14=3 |
| `logic` | `UEQ` | 102 | 18 (17.6%) | s5=16, s4=15, s2=13, s12=13 |
| `logic` | `FNE` | 65 | 13 (20.0%) | s12=7, s2=6, s4=4, s8=4 |
| `logic` | `FEQ` | 134 | 11 (8.2%) | s12=8, s6=7, s8=5, s11=4 |
| `goal` | `BACKGROUND` | 48 | 3 | s1=s2=s3=s4 (all 3) |

Three things here are worth acting on, and none of them is a routing table:

1. **Horn-shaped problems are 4.5× more likely to be solved than non-Horn ones**
   (19.5% vs 4.4%). No shipped schedule distinguishes them. That is a scheduling
   question, not a routing one: on a Horn problem the portfolio should spend
   slots differently, and it cannot spend them differently *per problem* unless
   it knows the shape.
2. **`max_term_depth > 10` is a hard negative predictor**: 0 of 34 solved at
   3 s, by any configuration. Term depth is the cleanest "this will not finish"
   signal in the whole feature set, and it is one line of analysis.
3. **s12 is the broadest single configuration measured** (28/600) and appears at
   or near the top of five of the seven rows above. It is the seventh entry in
   `CASC_FNE_ORDER` and the twelfth in `CASC_UEQ_ORDER`. The orders were derived
   from an earlier sweep whose conclusions may not have aged well, or whose
   budget differed; either way the orders and this measurement disagree, and the
   disagreement should be resolved by re-running the sweep rather than by
   editing the orders.

## 6. The hard tail at 30 s: the decisive experiment

The 3 s measurement is dominated by "solved by nobody", which is the weakest
possible basis for a routing claim. The hard tail is where the choice between
configurations can matter, so 127 problems were re-measured at 30 s across seven
configurations: every problem the 3 s sweep solved, plus a label-stratified
sample of the ones it did not. Seven rather than fifteen, because the tail is
where each 30 s run actually costs 30 s.

Coverage rises from 7.2% to **36.2%**, and the shape of the answer changes:

| configuration | s12 | s6 | s8 | s2 | s5 | s1 | s15 |
|---|---|---|---|---|---|---|---|
| problems solved | **32** | 28 | 26 | 25 | 22 | 18 | 15 |

Problems by how many of the seven solve them:

| 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|
| 81 | 7 | 9 | 8 | 8 | 5 | 3 | 6 |

46 of 127 are solved by at least one configuration, and **81 are solved by none at
any of the seven**. The regime where the choice matters now holds 46 problems
instead of 21 — enough to be worth a decision — and the decision is still
narrow.

### The per-division order is already within three problems of optimal

Greedy set cover over the seven measured configurations:

```
s12 (+32)  s8 (+8)  s15 (+3)  s2 (+2)  s6 (+1)   →  46 with five
```

Portfolio coverage by width, comparing the greedy order against the shipped
`casc_*` order for each problem's own division (both restricted to the seven
measured configurations, since the others were not run at 30 s):

| slots | greedy | `casc` (generic) | `casc_<division>` | gap to greedy |
|---|---|---|---|---|
| 1 | 32 | 18 | 30 | 2 |
| 2 | 40 | 33 | 37 | 3 |
| 3 | 43 | 35 | 42 | 1 |
| 4 | 45 | 41 | 43 | 2 |
| 5 | 46 | 43 | 45 | 1 |
| 6 | 46 | 44 | 46 | 0 |
| 7 | 46 | 46 | 46 | 0 |

This is the study's main quantitative finding: **the shipped per-division orders
are within three problems of the greedy optimum at every portfolio width.** They
were derived from an earlier solo sweep by greedy set cover, so this is the
expected result — and it is now measured rather than assumed, which is what makes
it safe to *not* replace them.

It also says something about the generic `casc` order: at one slot it gets 18
where the per-division order gets 30. If a single configuration is going to run
— the `--workers 1` diagnosis mode, the `fast` schedule, the `mrs-proover` ATP
backend — the division order is the one to follow, not the generic one.

### Structural rules are negative on the subsets they fire on

| rule | subset | solved by any | division order, all slots | rule order, all slots | delta |
|---|---|---|---|---|---|
| `definite_ratio >= 0.99` | 34 | 20 | 20 | 17 | **−3** |
| `dual_horn_ratio >= 0.95` | 59 | 33 | 33 | 32 | **−1** |
| `n_components >= 2` | 28 | 12 | 12 | 11 | **−1** |

Every rule fires on a subset it correctly *identifies* as winnable, and then
loses coverage on it. The subsets really are easier — 20 of 34 and 33 of 59
against a 36% overall rate is well above chance — and the per-division order
already extracts all of it.

Routed portfolios at 30 s:

| slots | routed | per-division | delta |
|---|---|---|---|
| 1 | 32 | 30 | +2 |
| 2 | 38 | 37 | +1 |
| 3 | 38 | 42 | **−4** |

The +2 at one slot is not a routing win either: it is the `definite_ratio` rule
happening to lead with s12, which is also the single best configuration overall.
The rule is not what produces the gain.

### What the 30 s tail says about individual configurations

Two things that are stable across both budgets and therefore worth acting on:

* **s12 is the best single configuration** — 28/600 at 3 s and 32/127 at 30 s,
  ahead of s8 at both. It is the seventh entry in `CASC_FNE_ORDER` and the
  twelfth in `CASC_UEQ_ORDER`. The orders and this measurement disagree, and §7
  says what to do about it.
* **s15 is weak alone and valuable in company.** It is the worst of the seven
  solo (15) yet the third greedy pick (+3), and it is the most complementary
  partner in the table: `s2+s15` is 17-only-s2 and 7-only-s15, `s8+s15` is 18 and
  7, `s12+s15` is 20 and 3. A rank-by-solo-coverage portfolio design would drop
  it; the set-cover design keeps it. That is a point in favour of the existing
  greedy orders and against any "pick the best few strategies" heuristic.

### And two facts about the tail that are not about strategies at all

* **`CSR115+6` and `CSR115+98`** (10,760 and 10,663 clauses) are solved *only* by
  s15, at both 3 s and 30 s. Both are non-Horn with no conjecture-free structure,
  and both need the one configuration that penalises rare symbols with no weight
  cap and no AVATAR. This is the sharpest example in the corpus of a problem
  where the configuration is the whole answer.
* **`HWV126-1`** — 657,576 clauses after clausification, by far the largest
  problem measured — is solved only by s8, the LPO goal-directed configuration,
  at 30 s. Nothing else gets close. A problem that size has no business being
  solved at all, and the one configuration that finds it is the one with
  conjecture-symbol precedence.

## 7. What the pre-phase should therefore do

The routing table (`mrs --list-rules`) keeps its priors, because the
measurement did not support changing any of them. It gains four calibrated rows,
three of which record a *negative* portfolio effect on the subset they fire on —
which is the most useful thing a rule table can say, because it stops the next
person from re-deriving the same hope. What it now carries is:

* the `empty` rule, which is the study's first finding given teeth — an input
  that lowers to nothing is reported as such instead of being fed a portfolio;
* `max_term_depth` as a first-class rule input, on the strength of the `>10`
  result;
* three calibrated rows — `forward-chaining` (`definite_ratio >= 0.99`),
  `hyper-resolution` (`dual_horn_ratio >= 0.95`) and `unreachable-depth`
  (`max_term_depth > 10`) — each naming its own measurement in its evidence
  string, including the negative portfolio effect;
* `Analysis::feasibility`, a `LIKELY` / `UNKNOWN` / `UNLIKELY` estimate that the
  sweep does support, with a 5x spread between the extremes against a 7.2% base
  rate. This is the pre-phase output worth trusting, and the distinction from
  routing is the point: feasibility asks a question the features answer, while
  routing asks one that has the same answer for all fifteen configurations on 93%
  of problems;
* a `calibrated` / `prior` flag on every row, so a reader can tell which is
  which.

The honest summary of the study's verdict on routing: **at both budgets
measured, on this corpus, routing on static structure is a losing move.** The table stays in
because the two things it *is* good for — reporting the analysis, and reporting
a problem the front end cannot represent — are worth having, and because a
re-measurement at a competition budget (which this host cannot run) is the
experiment that would settle it.

## 8. Negative results, recorded

* **Routing on `logic` class is not better than the fixed per-division order.**
  See §5.
* **The syntactic label is almost uninformative about behaviour once "solved by
  nobody" is accounted for.** The weighted modal purity of label → behaviour is
  high (0.8–1.0 for most classes) but that number is dominated by the unsolved
  majority; conditioned on being solvable, the classes do not separate.
* **The `% Rating:` header field is deliberately excluded from routing.** It is
  available at competition time and would predict difficulty well, but it is a
  community annotation written with knowledge of other provers' results. It is
  captured in the feature table so the study can quantify how much of the
  predictive power is metadata rather than structure.
* **The probe's numbers could not be evaluated here**: the CASC-30 feature table
  was dumped before `--probe` existed, so the probe columns in that table are
  empty. The probe is implemented, tested and dumped by `prephase_dump --probe`;
  validating it needs a re-dump, which the 27k-problem corpus budget did not
  leave room for alongside everything else.

## 9. A measurement artefact that has to be stated

`goal_class = NO_GOAL` shows 0 of 70 solved. That is **not** a property of
satisfiability problems. It is a property of the instrument: a solo strategy run
has no certified saturation path, so the satisfiability divisions (EPS, ICU, TFN,
SLH) cannot produce a positive answer no matter how good the configuration is.
Any conclusion drawn from the `NO_GOAL` bucket of the 3 s sweep is a statement
about the measurement, not about the problems. The CASC-30 EPS row in
`systems/mrs/invoke.sh` runs a separate certified track for exactly this reason,
and that track is outside what `prephase_sweep` exercises.

Similarly, 223 of the 9,000 runs hit the external 23 s wall without emitting an
SZS status while the search limit was 3 s. Those are problems whose
parse-plus-clausification exceeds the whole search budget — a real and reportable
observation about front-end cost, but one that means the 3 s measurement is a
*lower* bound on those problems' difficulty.

## 10. Limitations

* **Budget.** 3 s for the broad sample and 30 s for the hard tail, not the CASC
  240 s. Every coverage number here is a 3 s or a 30 s number. Because the sweep
  records time-to-solution rather than a boolean, the 3 s data replays at any
  budget at or above 3 s — but a problem that takes 40 s is indistinguishable
  from an unsolvable one, which is exactly why the hard tail was re-measured.
* **Seven configurations at 30 s, not fifteen.** The tail is where each run costs
  its full 30 s, so the 30 s experiment covers 7 of the 15. Portfolio widths above
  7 are therefore not measured, and the greedy optimum is a lower bound on what
  the other eight would add.
* **Sample.** A stratified 600-problem sample of CASC-30, round-robin over parent
  directories. Proportional to division size, which is right for estimating a
  portfolio's behaviour and wrong for estimating a division's coverage.
* **Host.** 2 physical cores, 15 GB RAM. Wall-clock figures scale with the host;
  coverage and ordering figures do not, but a configuration's advantage can come
  from finishing inside 3 s on a slow host while finishing comfortably inside
  240 s on a fast one. This is why §6 changes nothing on the strength of a
  3-second measurement.
* **One corpus for labels.** CASC-30 only. A 9,259-problem ProoVer-2026 corpus
  (`~/proover-corpus-vampire`) with a different domain mix and no division
  structure is present on this host and is the obvious held-out set for a
  follow-up; labelling it was outside the compute this host allows.
* **One label instrument.** The 15 base configurations, not a search over the
  parameter space. A routing table can only permute what was measured.
* **One pre-existing test fails on this host and is not a regression.**
  `resource::tests::address_space_ceiling_is_applied_and_restored` asserts that a
  3.4 GiB allocation fails under a ceiling it computed from this process's own
  usage; on a 15 GiB host the ceiling lands above the request. `resource.rs` is
  untouched by this branch. The mechanism the test exercises does work here —
  `RLIMIT_AS` was verified directly, and it is what recorded the 245
  `resource_limit` rows in the corpus dump.

## 11. Reproducing

See `crates/mrs-bench/prephase/README.md` for the commands. The raw measurements
are in `crates/mrs-bench/prephase/results/`.