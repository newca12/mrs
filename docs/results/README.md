# Benchmark Results

This directory contains result artifacts that are too detailed or too large
for the current guides. Files here are evidence, not automatically current
baselines.

Every checked-in summary must identify:

- source commit and dirty state;
- corpus and TPTP version;
- command, schedule, portfolio, workers, jobs, and environment variables;
- hardware and toolchain; and
- whether the result is solo diagnostic, cooperative, strict-audited, or
  competition-mode.

The existing `greedy_portfolios` files are solo set-cover diagnostics. They do
not establish cooperative portfolio coverage.
