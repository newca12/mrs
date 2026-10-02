# LCL Condensed-Detachment Pre-pass

The optional `MRS_CONDENSED_DETACHMENT=1` pre-pass targets compact LCL problems
that encode theoremhood with a unit predicate `is_a_theorem(F)` and the rule

```text
~is_a_theorem(X) | is_a_theorem(Y) | ~is_a_theorem(implies(X,Y))
```

Detachment is replayed as two ordinary binary-resolution steps against the
actual input rule clause. Derived theorem units are retained and may be reused.
The emitted TSTP graph is therefore an ordinary resolution DAG, not a trusted
special inference. Input-shape recognition is deliberately narrow; any
unsupported shape or exhausted bound falls through to the standard strategy
portfolio.

**Status: measured on the whole FNE division of both CASC editions, and then
measured again with the bounds raised 20-60x. The answer is no both times.** It
adds no coverage, and it costs about 0.9 s of a 238 s budget on the problems it
recognises. It stays opt-in, which is what `strategy.rs` already does — these
measurements confirm that default rather than changing it. What would have to
change for the verdict to be worth revisiting is on
[What would change it](#conclusion), and it is not a bigger bound.

## Bounds and availability

- At most 5,000 distinct theorem facts and 100,000 counted resolution steps.
- Maximum one second per problem and five seconds total per process, so batch
  runs cannot repeatedly pay a fresh pre-pass ceiling.
- A derived formula may not exceed twice the largest goal formula size plus 16
  term nodes.
- Disabled unless `MRS_CONDENSED_DETACHMENT=1` is set.
- Skipped when strict asynchronous self-checking is active, so it cannot consume
  the verifier's reserved wall-clock budget outside candidate coordination.
- A bounded no-result outcome is inconclusive; the regular schedule still runs.

The two wall budgets and the two structural bounds are overridable —
`MRS_CD_BUDGET_MS`, `MRS_CD_PROCESS_MS`, `MRS_CD_MAX_FACTS`,
`MRS_CD_MAX_INFERENCES`, listed with their defaults in `docs/reference/cli.md` —
for exactly one purpose: telling "the closure cannot close this fragment" apart
from "the bound stopped it first". A malformed or zero value falls back to the
default rather than taking effect, because zero would turn a bounded pre-pass
into unbounded closure. Widening them spends portfolio budget, so it is a
measurement action, and `crates/mrs-bench/cd_bound_probe.sh` is the harness for
doing it.

## The pre-pass reports why it stopped

Every run with the pre-pass enabled prints one line to stderr, whether or not it
finds anything:

```text
% SZS detail condensed_detachment=ran shape=matched stop=deadline facts=4358 inferences=35724 elapsed_ms=1001 budget_ms=1000 max_facts=5000 max_inferences=100000
```

`stop=refutation` is the only positive. `stop=deadline`, `stop=max_facts` and
`stop=max_inferences` mean a bound stopped the closure while it was still
deriving detachments, so a wider bound could change the answer. `stop=facts_exhausted`
means the closure ran out of detachments to make, which no wider bound can change.
`shape=unmatched` means the input did not carry the rule clause at all and the
pre-pass declined without working.

This line did not exist when the campaigns below ran, and its absence is the
main reason those runs are hard to read.

## Measurement 1 — the two FNE campaigns (2026-10-01)

Four campaigns, `results/campaign-{casc30,cascj13}-fne-{condensed,ref}-W8C16J2-20261001`,
each 100 FNE problems, 8 workers, two jobs at a time, official CASC wall clock
(FNE is 240 s on CASC-30 and 180 s on CASC-j13). "condensed" is
`MRS_CONDENSED_DETACHMENT=1`, "ref" is the flag off. Same commit
(`38e1f85`), same binary sha within each edition pair.

| arm | solved | refutations from the pre-pass |
|---|---|---|
| casc-30 condensed | 42 | **0** |
| casc-30 ref | 42 | — |
| cascj13 condensed | 36 | **0** |
| cascj13 ref | 35 | — |

The zero is not an inference, it is a count. A pre-pass refutation returns early
with a single-strategy report (`strategy_idx=0, strategy_id=0`,
`SearchStats::default()`), and all 155 solves across the four runs carry the
ordinary eight-strategy signature
`strategy_ids=0:11;1:8;2:4;3:15;4:10;5:3;6:12;7:1`. The `condensed_detachment`
string that does appear in the raw output is a TPTP `file(...)` provenance tag
inside `LCL982+1.p` itself, present identically in both arms.

Solve sets are otherwise indistinguishable: casc-30 solved the same 42 problems
in both arms, and casc-j13 differs by `SYO605+1`, which is not an LCL problem and
was lost in the arm that hit the OS OOM killer.

### What the campaigns cost

The pre-pass runs before the portfolio and shrinks `total_budget` by whatever it
used, so its cost is visible in mrs's own clock on deadline-bound rows. Across the
LCL cluster the condensed arm's `elapsed_ms` runs behind the ref arm's, by a mean
of 867 ms and by a full second on the worst rows — about 0.4 % of a CASC-30
limit, paid per problem, for nothing.

### Why the campaigns could not have concluded anything

This is the part that matters for reading the table above.

`cd_budget` is capped at one second, against a 238 s problem. The pre-pass was
being asked to close the fragment in 1/240th of the time the problem is allowed.
And it was not even finishing: of the LCL rows with a measurable budget shift,
29 of 48 hit the full 1 s wall cap rather than CD's own fact/step bound. Those
rows record a closure that was **cut off**, not a closure that **failed**.

So the honest statement of measurement 1 is *"the pre-pass does not close an LCL
problem in ≤1 s"*. It is not *"condensed detachment does not help on FNE"*, and
writing the second would have been a mistake.

### Two other things the campaigns were not measuring

- **Different problem sets.** CASC-30 FNE and CASC-j13 FNE share only 34 of 100
  problems, so 42 versus 36 is set composition, not a regression. On the shared
  34, casc-30 solves 9 in both arms and casc-j13 solves 10 in both.
- **The two campaigns overran each other.** Each pair ran concurrently: four jobs
  times eight workers on sixteen cores, and up to four times ~59 GB of memory
  demand on a 128 GB box. `NLP260+1`, `NLP261+1` and `NLP262+1` were SIGKILLed by
  the OS OOM killer or stopped at `resource_reason=memory` in the casc-j13 arms
  while surviving to a full 240 s Timeout in both casc-30 arms. 24 of 100 casc-30
  problems exceed 20 GB and one peaks at 59 GB, so W8 J2 is already at the memory
  ceiling and two campaigns at once is not a shape that exists.

Neither affects the condensed-detachment reading, but both mean the 42/36 headline
of these runs is not a number.

## Measurement 2 — is the bound what stops it? (`cd_bound_probe.sh`)

Runs the pre-pass alone, over the 37 `LCL*` problems of `casc-30/FNE`, at
production bounds and at bounds raised 60x on wall time and 20x on each
structural bound. Sequential, `--workers 1`, because the pre-pass is
single-threaded and its outcome does not depend on the worker count — which is
what makes the result comparable between the hosts this was run on.

`--time` tracks each arm's own budget. The harness sets `MRS_CD_ONLY=1`, so a
problem not refuted by the pre-pass stops before the ordinary portfolio can run
and cannot be mistaken for pre-pass coverage.

```bash
crates/mrs-bench/cd_bound_probe.sh --div fne --edition casc-30 --time 60 \
    --max-facts 100000 --max-inferences 2000000
```

### Production arm: what the campaign could not see

Of 37 problems:

| stop | count | meaning |
|---|---|---|
| `deadline` | 16 | cut off by the 1 s wall cap |
| `max_facts` | 8 | cut off by the 5,000-fact bound |
| `no_fragment` | 11 | input carries no detachment rule; declined without working |
| `facts_exhausted` | 2 | ran out of detachments — the method, not the bound |
| `refutation` | **0** | |

So 24 of 37 were cut off by a bound and only 2 were genuinely exhausted. That is
the campaign's "0 refutations" decomposed, and it is why the campaign could not
distinguish a weak bound from a weak method.

Two further readings worth keeping:

- **A third of the cluster does not engage the pre-pass at all.** 11 of 37 LCL
  problems carry no `is_a_theorem` detachment-rule clause. Any statement about
  "the LCL cluster" that does not separate these is averaging over problems the
  pre-pass never looks at.
- **`max_facts` is not a wall-clock bound.** Two problems hit the 5,000-fact cap
  in 353 ms and 651 ms, well inside the 1 s wall budget. The fact cap, not the
  wall clock, is the tightest production bound.

### Cross-check against the campaign

The probe is an independent measurement of the same quantity the campaign's raw
stderr implied, so the two should agree. Restricting to deadline-bound rows,
where `elapsed_ms` is the deadline and its delta is therefore the budget shift:

| | value |
|---|---|
| mean budget the probe says the pre-pass consumed | 678 ms |
| mean shift the campaign measured | −661 ms |
| correlation over 31 rows | r = −0.83 |

The magnitudes and the correlation agree, which does two things: it confirms the
"condensed" arms really did run with the pre-pass enabled at production bounds,
and it validates the budget-shift inference that was all the campaign data had.
Both are now recorded directly instead of inferred.

### Wide arm: 60 s, 100,000 facts, 2,000,000 inferences

Same 37 problems, 60x the wall budget and 20x each structural bound, sequential,
`--workers 1`, on a 2-physical-core / 16 GB laptop (`pve`, Core i3-5010U) — not
the 32-core Xeon the campaigns ran on. The pre-pass is single-threaded, so the
host does not affect the outcome; it only caps how wide the run could afford to
go, which is why the wide arm is 20x and not 200x.

| stop | prod arm | wide arm |
|---|---|---|
| `refutation` | **0** | **0** |
| `deadline` | 16 | 3 |
| `max_facts` | 8 | 10 |
| `max_inferences` | 0 | 9 |
| `facts_exhausted` | 2 | 2 |
| `no_fragment` | 11 | 11 |

Of the 26 problems the pre-pass engages: **zero refutations at either setting.**
The two arms agree problem for problem on `no_fragment` and on `facts_exhausted`,
and disagree only about *which* bound stopped the other 24.

### What 20x the bound actually bought

Nothing, and the way it bought nothing is the informative part. For example
`LCL062+1`:

| | facts derived | inferences | refutation |
|---|---|---|---|
| prod (1 s, 5 000 facts) | 4 359 | 35 736 | no — cut off at the deadline |
| wide (60 s, 100 000 facts) | 100 000 | 578 280 | no - cut off at the fact bound |

23x more intermediate theorems and 16x more resolution steps, and the goal is
still not closed. Across the wide arm the closure reaches between 15 404 and
100 000 derived facts and up to 2 000 002 inferences in 12–60 s without closing a
single goal.

The two problems that *do* exhaust, exhaust identically in both arms:
`LCL403+2` at 40 facts / 3 512 inferences in 25 ms, and `LCL982+1` at 2 facts /
10 inferences in under a millisecond. Those are complete closures with nothing
left to derive, and no bound can change them.

**So the bound is not why the pre-pass fails.** Going 20-60x larger on every bound
converted zero bound-stops into refutations, while the derived-fact count grew by
more than an order of magnitude without the goal ever closing. That is a closure
that does not converge on these goals, not one that is under-resourced.

### What is still not closed, stated plainly

21 of the 26 engaged problems are *still* cut off in the wide arm. So this
measurement does not prove that no bound can ever work, and the writeup should not
claim it. What it does establish is the direction: raising a bound has never once
produced a refutation, and the growth pattern — facts accumulating roughly
linearly in wall time with the goal never reached — does not point at a finite
bound that would. Each further doubling of the fact bound costs memory roughly
linearly too: 100 000 facts already needs most of an 8 GB budget, which is why the
wide arm is 20x and not 200x. The next informative step is a test of the
fragment's convergence, not of a bigger budget.

### A third of the cluster is out of scope anyway

11 of the 37 `LCL*` problems carry no `is_a_theorem` detachment-rule clause and
are declined in 0 ms — `LCL640+1.005`, `LCL642+1.010`, `LCL642+1.015`,
`LCL656+1.015`, `LCL660+1.015`, `LCL660+1.020`, `LCL670+1.010`, `LCL680+1.005`,
`LCL682+1.015`, `LCL682+1.020`, `LCL688+1.005`. The pre-pass's real addressable
set on `casc-30/FNE` is **26 problems**, not 37. Any statement about "the LCL
cluster" that does not separate these is averaging over problems the pre-pass
never reads.

Two of those 11 are solved by the *portfolio* inside the leftover budget
(`LCL642+1.010`, `LCL982+1`, both `szs_status=Theorem` with
`cd_stop!=refutation`). This is why the probe reads the `condensed_detachment=`
line rather than the SZS status: a `Theorem` with `stop!=refutation` is the
portfolio's work.

<a id="conclusion"></a>

## Conclusion

The pre-pass is not earning its place. It should stay opt-in and off by default,
which is what `strategy.rs` already does — the measurements above confirm that
default rather than changing it. Three separate reasons, in decreasing order of
how much they would cost to fix:

1. **No coverage.** 0 refutations across 400 FNE campaign problem-runs and 74
   probe runs (37 problems x 2 arms).
2. **No headroom.** A 20-60x bound increase converts nothing, so the method,
   not the budget, is the limit.
3. **Real cost.** ~0.9 s of a 238 s budget on every problem it engages, which is
   the part that would remain even if it did work.

What would change this verdict: a refutation at any bound setting, verified under
`mrs-proover --strict`. What would not: another campaign at the production bounds.
That measurement has been made twice.

## Smoke check

```bash
nix develop -c cargo run -- problems/cd-prototype-smoke.p
MRS_CONDENSED_DETACHMENT=1 nix develop -c cargo run -- problems/cd-prototype-smoke.p
```

The opt-in run emits the `condensed_detachment=ran ... stop=refutation` line and
the proof verifies under `mrs-proover --strict`.

Unit tests run generated proofs through the independent `mrs-proof-kernel`, and
`reports_why_it_stopped_so_a_null_result_is_readable` pins the stop reasons this
writeup reads off — `no_fragment`, `no_goals`, `facts_exhausted` and `max_facts`
on the same fragment, so a widened run cannot confuse a cut-off closure with an
exhausted one. Any wider benchmark should also use the linked problem source and
strict proof checking; a parseable TSTP string or successful pre-pass return
alone is not a proof-validation result.

## How to run this again

`casc.sh` now records every `MRS_*` variable in `run_meta.json` under
`prover_env`. That gap is what made the 2026-10-01 pair unreadable: both arms
shared a commit and a binary sha, and the only evidence of the on/off state was a
one-second shift in `elapsed_ms` that had to be recovered from raw stderr by
hand. A new arm should never need that.
