# Benchmarking Guide

The benchmark harness is in `crates/mrs-bench`. It archives raw stdout/stderr,
hashes, host metadata, toolchain metadata, and one CSV row per problem/system.

Two different questions need two different tools:

| Question | Tool |
|---|---|
| How many TPTP problems does `mrs` solve? | `casc.sh` (below) |
| How fast does `mrs` search on *this* machine? | [`perf_probe.sh`](../results/perf/README.md) |

A solved count is the objective the schedules were tuned for. Throughput is a
diagnostic: it explains *why* a run is slow on a given box, and it is the only
one of the two that can be compared across machines without a TPTP corpus.

## Quick performance probe

```bash
crates/mrs-bench/perf_probe.sh            # writes docs/results/perf/bank.tsv
crates/mrs-bench/perf_probe.sh --no-bank  # measure only, touch nothing
```

It generates a clause set, runs a fixed amount of search work on it, and banks
the result against the host's fingerprint. Work is stopped by an
iteration-counted ceiling rather than a clock, so the timings from two machines
are comparable; a "run it for 30 seconds and count clauses" measurement is not,
because the search is wall-clock sensitive. It builds both a `native` and a
`haswell` binary so ISA effects are separable, and it caps the whole probe at
12 GiB, divided across workers for the search's RSS watchdog. The driver uses
the same affinity/cgroup-aware physical-core count as the prover for its default
worker list. Two consequences on older or wider hardware: a CPU that cannot run
the `haswell` build (anything pre-Haswell) measures `native` only, and a host
with more cores than 12 GiB can hold workers for is told to pass `--workers`
rather than being measured past the ceiling. See
[`docs/results/perf/README.md`](../results/perf/README.md) for the
method, the comparability rules, and how to add another host.

## Prepare the corpus

Freeze a materialized corpus and predeclare family-disjoint development/holdout
membership before tuning:

```bash
python3 crates/mrs-bench/freeze_corpus.py --root /path/to/casc-30 \
  --output /path/outside-corpus/corpus-manifest.json --seed 42
```

The manifest hashes all files, including include axioms. The conservative default
groups all problems with the same three-letter TPTP domain prefix. For finer,
reviewed groups pass `--family-map` with a complete JSON relative-path-to-family
map. Never choose split seeds after inspecting holdout outcomes. Empty arms,
symlinks and overwriting a manifest are refused. This digest schema differs from
the auditor's streaming corpus digest; compare manifests within their schema.

```bash
nix develop -c cargo build --release
crates/mrs-bench/setup.sh
```

The extracted corpus is normally under
`crates/mrs-bench/problems/<edition>`. Set `TPTP` or `CASC_PROBLEMS_ROOT`
explicitly when using a different TPTP checkout.

## Run a benchmark

```bash
MRS_WORKERS=8 crates/mrs-bench/casc.sh \
  --edition casc-30 \
  --systems mrs \
  --divisions fne,feq,ueq \
  --casc-times \
  --jobs 1 \
  --output crates/mrs-bench/results/casc-30/current-run
```

On canonical eight-core competition hardware, use `MRS_WORKERS=8 --jobs 1` to
avoid oversubscribing the machine. `--jobs` controls independent problem
processes; `MRS_WORKERS` controls workers inside one MRS process.

Summarize a run with:

```bash
nix develop -c cargo run --release -p mrs-bench --bin bench_report -- \
  crates/mrs-bench/results/casc-30/current-run/run.csv
```

## Solo strategy diagnostics

Set `MRS_TRAJECTORY_DIGEST=1` for opt-in per-strategy SHA-256 selection digests
in `% SZS detail` (`trajectory_sha256=slot:digest;...`). `repeat_search` preserves
this field. The digest covers selected clause IDs, logical literals, distance
and AVATAR context before simplification, using symbol names rather than local
term handles. It detects differing selection histories; it is not a proof or
an exhaustive inference-event trace. Hashing/rendering has diagnostic overhead:
do not silently enable it in headline throughput campaigns. Fixed-work comparisons
must also fix pruning policy and work caps, disable sharing, leave ample time,
and check the final counters/resource reason as well as the digest.

`MRS_SPECIALIZE_QUEUES=1` experimentally drops optional passive heaps unused by
the chosen selector (age and weight remain for fallback/LRS). Compare fixed-work
digests/counters and `passive_queue_entries` with the option absent. The latter
counts retained queue entries including tombstones, not live clauses or bytes.
The optimization is opt-in pending representative memory and coverage measurements.
`MRS_COMPACT_QUEUES=1` additionally removes passive tombstones at periodic
maintenance points when retained entries exceed both 4,096 and 32 times live
membership. This does not LRS-prune live clauses. `queue_tombstones_removed`,
`retained_terms`, and `retained_proof_clauses` distinguish queue churn from
term/proof-history retention; these are final counts, not byte or peak metrics.

```bash
crates/mrs-bench/run_strategy_sweep.sh \
  --edition casc-j13 --divisions fne,feq,ueq \
  --casc-times --jobs 4 \
  --output crates/mrs-bench/results/solo-j13
```

Solo sweeps measure one strategy with one worker. Their union and greedy
set-cover output are candidate-generation diagnostics only. They do not model
cooperative strategy execution or equality sharing.

## Cooperative portfolio experiments

For a paired certified comparison of frozen baseline/candidate binaries:

```bash
python3 crates/mrs-bench/paired_campaign.py \
  --baseline /path/to/mrs-baseline --candidate /path/to/mrs-candidate \
  --problems-dir /path/to/casc-30 --edition casc-30 \
  --divisions fne,feq,ueq --reps 5 --output /path/to/new-experiment
```

The driver randomizes arm order within each pair, runs sequentially with eight
workers and `jobs=1` in CASC mode, strictly audits every arm, checks matching
evaluation sets, and reports paired certified gains/losses. It checks frozen
binary hashes before each arm. It does not build tools or automatically promote
a candidate. Remaining inherited `MRS_*` overrides are recorded: review the
manifest and hardware lines before interpreting the result. The reported range
is descriptive, not a confidence interval or a division-wide noise estimate.

```bash
MRS_WORKERS=8 MRS_SHARED_POOL_INTERVAL=500 \
  crates/mrs-bench/cooperative_portfolio_sweep.sh \
  casc-30 feq 11,12,1,6,10,8,14,4 30 1 \
  crates/mrs-bench/results/cooperative-feq

MRS_WORKERS=8 MRS_SHARED_POOL_INTERVAL=0 \
  crates/mrs-bench/cooperative_portfolio_sweep.sh \
  casc-30 feq 11,12,1,6,10,8,14,4 30 1 \
  crates/mrs-bench/results/cooperative-feq-no-sharing
```

Compare the same portfolio, corpus, binary, hardware, timeout, and process
concurrency. Repeat small differences because LRS and worker scheduling are
wall-clock-sensitive.

## Proof audits

Each new audit directory contains `audit-identity.json`, binding cached verdicts
to the auditor/checker binaries, run CSV, complete materialized corpus/include
tree, check configuration and limits. Resuming an audit with a changed identity
is refused; choose a fresh `--output` directory for re-audits, including audits
made before manifests existed. `--force` reruns checks within the same identity;
it does not authorize replacing another checker's historical results. Explicitly
pin external backends when using ladder checks; PATH alone does not hash binaries
discovered implicitly. Keep the corpus immutable throughout the audit.

Completed audits also publish immutable content-addressed directories under
`strict-audit/snapshots/<audit-csv-sha256>/` (or the chosen audit output path).
These retain earlier outcomes when the top-level resume CSV is updated. Generate
a dashboard from selected snapshots without pooling different campaigns/checkers:

```bash
python3 crates/mrs-bench/certification_dashboard.py \
  /path/to/audit/snapshots/<audit-csv-sha256>
```

The dashboard checks the CSV digest, refuses duplicate evidence, and includes
unknown/omitted/error outcomes in the denominator. It records the run CSV hash
as the link to producer metadata; retain that CSV and its `run_meta.json` too.

The harness preserves generated proofs for deferred audit:

```bash
nix develop -c cargo run --release -p mrs-bench --bin audit_casc_proofs -- \
  --run crates/mrs-bench/results/casc-30/current-run \
  --problems-dir crates/mrs-bench/problems/casc-30 \
  --checks strict,mrs,ladder \
  --strict-time 30 --mrs-time 10 --ladder-time 30 \
  --ladder-workers 8 --jobs 1 \
  --output crates/mrs-bench/results/casc-30/current-run/proof-audit
```

## Required provenance

New harness rows append `actual_wall_time_s`, `peak_rss_mb`, `peak_virtual_mb`,
`exit_code`, `termination_signal`, and `szs_emitted`. `wall_time_s` retains the
competition-credited timeout convention; use actual time for runtime diagnostics.
The legacy `peak_memory_mb` column mixes virtual memory and RSS and must not be
charted as one physical-memory series. Blank RSS means GNU time was unavailable.
A signal-derived exit status is evidence of termination, not a confirmed OOM;
`termination_signal` reflects the wrapper's exit status and cannot distinguish
an explicit exit with the same numeric code from a signal.
`last_completed_stage` records the last `% Stage complete:` marker on stderr;
parse and preparation markers precede search, and preparation reports lowering,
include and clausification costs. Missing markers are missing observations,
not proof that the process failed in a particular stage.

Every report must record:

- commit and dirty-worktree state;
- exact TPTP edition or checkout;
- problem count and divisions;
- per-division timeout;
- MRS workers and outer jobs;
- CPU, RAM, operating system, and Rust version;
- schedule, portfolio, and relevant environment variables;
- raw CSV and proof-audit paths; and
- disagreements, polarity violations, reference violations, errors, and OOMs.

Never call a local run an official competition ranking.
