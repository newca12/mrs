# Parent Guidance Experiment

This is an experimental implementation inspired by *Fast and Slow Enigmas and
Parental Guidance* (<https://arxiv.org/abs/2107.06750>). It predicts whether a
candidate parent pair will generate at least one non-tautological result for a
given inference operation. Features are fixed-width, symbol-name hashed, and do
not contain clause IDs or problem-local symbol indices.

## Safety and scope

- Parent-pair observation and pruning are compiled behind the optional
  `parent-guidance` feature.
- In portfolios with at least two active slots, only the final active strategy
  slot receives parent-guidance settings. Single-strategy runs remain unaltered.
- Logging alone does not prune. Pruning requires both a valid compatible model
  and an explicit finite logit threshold.
- A strategy that prunes any pair is downgraded to `GaveUp`, regardless of its
  raw result, and completeness telemetry records the pruning count.
- This mechanism does not change clause-selection scoring or schedule choice.

## Collection and training

Build with the feature and log candidate pairs during a refutation:

```bash
nix develop -c cargo run --features parent-guidance -- \
  --log-ml-data data/parent-traces problem.p
```

The final portfolio slot samples up to 2% of candidate parent pairs. CSV files
are written beneath `data/parent-traces/parent-guidance/`, labeled `1` when the
inference routine yields at least one result and `0` otherwise. The sample cap
is 200,000 pairs per search state. Traces without an observed outcome stay
negative by default only if no result is generated; candidates interrupted by
the search deadline are left unmodified by the outcome update.

Train one inference kind at a time (`0` resolution, `1` given-as-equality
source, `2` given-as-target, `3` self-superposition):

```bash
nix develop -c cargo run -p mrs-train --no-default-features --features ndarray -- \
  --mode parent --inference-kind 0 --epochs 100 \
  data/parent-traces models/resolution.json
```

Training requires at least 100 rows of the requested kind and both labels. If
multiple files are available, a deterministic file-level holdout is reported.
The output is a versioned JSON linear model. Treat validation precision/recall
as experimental diagnostics, not as evidence that pruning preserves proof
coverage.

## Opt-in inference pruning

```bash
nix develop -c cargo run --features parent-guidance -- \
  --parent-guidance-weights models/resolution.json \
  --parent-guidance-threshold -2.0 problem.p
```

Only the model's declared inference kind is eligible. Scores below the supplied
logit threshold omit that candidate pair. Because omitted pairs make the search
incomplete, do not interpret a run that reports `parent_guidance_pruned>0` as a
complete search, even if it finds no proof.

Compare coverage against the same portfolio with the options absent. Record
the exact traces, model, threshold, problem order, worker count, schedule, and
feature set for every experiment. The feature remains off by default.
