# Pre-phase study: instrument, method, and how to reproduce

This directory holds the experimental harness for the mrs pre-phase study. The
pre-phase is a bounded analysis of a problem, run *before* search, whose job is
to decide which algorithm and which parameters to use. This directory is the
part of that work that is measurement rather than product.

Everything here is experimental. Nothing in this directory changes what `mrs`
does at competition time; the routing code it feeds lives behind `--pre-phase`
and is off by default.

## The question

`mrs` already contains several distinct algorithms and several dozen parameter
knobs, and chooses between them with rules nobody has measured:

* `mrs_search::strategy::auto_schedule_name` maps a problem to a division
  portfolio on four syntactic predicates (all-unit-equality, no function of
  arity >= 1, no equality, otherwise).
* `mrs_core::profile::classify_problem` maps a problem to an `archetype`, a
  `casc_division`, a `recommended_schedule`, a `recommended_engine`, and
  recommended AVATAR / SInE switches, again from a handful of thresholds
  (`horn_ratio >= 1.0 && unit_ratio >= 0.3`, `max_term_depth >= 7`, `num_axioms > 150`).
* Four pre-passes (bounded grounding, componentwise refutation,
  condensed detachment, propositional-skeleton resolution) are gated on
  environment variables that are off in every shipped configuration.

Two questions follow, and they are different:

1. **Descriptive.** What kinds of problems are in the corpora we are pointed at,
   and which structural properties separate them?
2. **Predictive.** Given only properties measurable before the first inference,
   which configuration solves which problem — and does routing on that beat a
   fixed per-division portfolio?

Question 2 is the one that decides whether the pre-phase earns its place.

## Instruments

| Tool | What it does |
|------|--------------|
| `crates/mrs-prephase` | the static analysis: ~80 features over the clausified problem, plus the class labels and the routing table |
| `crates/mrs-bench/src/bin/prephase_dump.rs` | one CSV row of features per `.p` file under a corpus root |
| `crates/mrs-bench/src/bin/prephase_sweep.rs` | the label: a strategy x problem outcome matrix with time-to-solution and the engine's own telemetry |
| `analyze.py` (this directory) | joins the two and reports coverage, greedy set cover, per-bucket conditional coverage, and feasibility |

### Feature families

*Scale and shape* — clause and literal counts, width histogram, unit / horn /
dual-horn / definite / goal-only ratios, negative-literal ratio.

*Equality and variables* — equality-literal ratio, ground ratio, per-literal
nonlinearity, the fraction of predicate atoms whose arguments are all distinct
variables (the propositional-skeleton signal), variable counts.

*Symbols and terms* — predicate / function / constant counts, arities, a symbol
concentration share and a Gini coefficient, Skolem count and arity, term depth
(including its 90th percentile), term size, distinct-term count.

*Goal topology* — goal clause and literal counts, goal max depth, conjecture
symbol overlap, conjecture symbols that appear nowhere else, the fraction of
non-goal clauses reachable from the conjecture through the same symbol-distance
BFS the engine's own goal-distance map uses, mean goal distance, and the
fraction of clauses with no reachable symbol at all.

*Decomposition* — connected components of the clause/symbol incidence graph,
the largest component's share, and how many of those components the conjecture
touches.

*Measured redundancy* — how many clauses the engine's own tautology / PLE /
blocked-clause reduction removes, plus input tautologies and duplicate clauses.
This is a measurement, not a prediction: the analysis runs the real reducer.

*Signature* — AC symbols, identity / inverse / idempotence axioms, and the
rewrite-rule density (unit positive equalities between compound terms).

*Propositional abstraction* — the size of the instance that remains when every
non-variable subterm is collapsed to one fresh slot. Small abstraction plus large
input means the bottleneck is first-order search, not the propositional core.

*Header metadata* — `% Status:` and `% Rating:` are captured and reported but
**excluded from routing**. `Rating:` is a community difficulty annotation written
with knowledge of other provers' results. A router that reads it would score
better than it reasons and would not generalize to unannotated input. It is here
so the study can *quantify* how much of the predictive power is community
metadata rather than structure — which is itself worth knowing.

### Class labels

`Analysis::label` is `logic/shape/scale/goal/decomposition`:

| axis | values | what it separates |
|------|--------|-------------------|
| `logic` | `UEQ` `PEQ` `EPR` `FNE` `FEQ` `EMPTY` | which calculus applies at all |
| `shape` | `HORN` `DUAL_HORN` `STRATIFIED` `NON_HORN` `GOAL_ONLY` `EMPTY` | whether hyper-style reasoning suffices |
| `scale` | `TINY` `SMALL` `MEDIUM` `LARGE` `HUGE` | whether premise filtering pays for itself |
| `goal` | `TIGHT` `LOOSE` `BACKGROUND` `NO_GOAL` `EMPTY` | how much of the input the goal can reach |
| `decomposition` | `CONNECTED` `MULTI_GOAL` `SINGLE_GOAL` `EMPTY` | whether the problem splits |

These are *syntactic*. The behaviourally-defined taxonomy — grouping problems by
which configuration actually solves them — is derived from the label sweep and
reported separately, because the whole point of the study is to find out whether
the syntactic axes are the ones that matter.

## Reproducing the measurements

### 1. Features over a corpus

```bash
nix develop -c cargo build --release -p mrs-bench --bin prephase_dump

TPTP=~/TPTP-v9.3.0 ./target/release/prephase_dump \
    --root ~/TPTP-v9.3.0/Problems \
    --out results/prephase/tptp_features.csv \
    --jobs 2 --rlimit-mb 2500
```

`--rlimit-mb` analyses each file in a child process under an `RLIMIT_AS` ceiling.
Without it a single problem can take the whole run out with the OOM killer and no
output: the 2026 CASC-J13 `FNQ/HWV062+1` needs about 6 GiB to parse and
clausify. With it, that file is recorded as `resource_limit` and the corpus
finishes.

### 2. Labels: strategy x problem outcomes

```bash
nix develop -c cargo build --release -p mrs-bench --bin prephase_sweep

TPTP=crates/mrs-bench/problems/casc-30 ./target/release/prephase_sweep \
    --features results/prephase/casc30_features.csv \
    --sample 600 --seed 1 --time 3 --jobs 2 \
    --out results/prephase/casc30_sweep_t3.csv
```

One strategy, one worker, the whole budget, sharing off — the same thing
`systems/mrs-sNN` measures, so the numbers are comparable with anything
`run_strategy_sweep.sh` already produced. `--sample` takes a stratified sample,
round-robin over parent directories, so a division or a source domain cannot
dominate by count.

### 3. Join and report

```bash
python3 crates/mrs-bench/prephase/analyze.py \
    results/prephase/casc30_features.csv \
    results/prephase/casc30_sweep_t3.csv
```

## Honest limits of the primary measurement

* **Budget.** The label sweep runs at 3 s per run, not the CASC 240 s. Every
  quantity reported from it is a *3 s* quantity. Because the sweep records
  time-to-solution rather than a boolean, the same data can be replayed at any
  budget — but only for budgets at and above the one measured. A problem that
  takes 40 s is indistinguishable from an unsolvable one here.
* **Sample.** A stratified sample of the CASC-30 corpus, not the whole corpus.
  Divisions with few problems are represented proportionally to their size, which
  is the right choice for estimating a *portfolio's* behaviour and the wrong one
  for estimating a *division's* coverage.
* **Host.** The measurements come from a 2-physical-core development host. The
  wall-clock figures scale with the host; the coverage and ordering figures do
  not depend on it, but the time-to-solution values do, and a configuration's
  advantage can come from finishing inside 3 s on a slow host while finishing
  comfortably inside 240 s on a fast one. This is the single biggest caveat on
  the study and it is why the routing table records *coverage* evidence rather
  than timing evidence.