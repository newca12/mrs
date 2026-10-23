# Machine Learning Status

## Current position

The workspace contains optional Burn-based ML infrastructure in `mrs-core`,
`mrs-search`, and `mrs-train`. It is off by default and is not the default CASC
portfolio.

The old learned schedule classifier is retired from runtime routing:

- `--auto-schedule` uses deterministic clause-shape rules;
- `--ml-schedule` is a deprecated alias for `--auto-schedule`;
- `--ml-schedule-weights` is parsed for compatibility and ignored; and
- static `casc_*` schedules remain the competition baseline.

ML premise pruning and ML clause selection remain experimental. They require
feature builds and model artifacts with compatible schema metadata. Pruned
workers cannot claim positive saturation, so an ML experiment cannot turn an
incomplete premise subset into a definitive model result.

## Data and training

Collect traces with:

```bash
nix develop -c cargo build --release --features ml --bin mrs
nix develop -c cargo run --release --features ml --bin mrs -- \
  --log-ml-data ml-logs problem.p
```

Train premise or schedule models with:

```bash
nix develop -c cargo run --release -p mrs-train -- \
  --mode premise --epochs 30 ml-logs weights_premise
nix develop -c cargo run --release -p mrs-train -- \
  --mode schedule --epochs 30 ml-logs weights_schedule
```

The trainer writes a Burn weight file and a `<prefix>_meta.json` schema file.
Keep those files together and record the feature/schema version in any report.

## Evaluation rule

Do not compare ML against static search using solo coverage alone. Use the same
cooperative portfolio objective, same workers, same corpus, no-sharing control,
and zero reference/polarity violations. Keep ML in shadow or opt-in mode until
it beats the static baseline under repeated cooperative experiments.

The earlier design proposals are retained in `history/retired-designs/`.
