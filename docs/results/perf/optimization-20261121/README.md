# Local term/index optimization evidence

Baseline: `c2f2f46b`. Candidate: the term-transform/index changes in the commit
containing this directory; `candidate-sources.sha256` binds the changed runtime
sources and lockfile. Additional FVI edits only clean up test style.

This is a one-worker, local fixed-work diagnostic on an i7-10610U with four
physical cores, not a CASC score or a portfolio result. The complete methods,
limits and remaining optimization priorities are in
[`term-transforms-2026-11-21.md`](../../../reports/benchmarks/term-transforms-2026-11-21.md).

## Artifacts

The local `.gitattributes` permits only the captured tool whitespace and frozen
fixture EOFs; the input bytes are not normalized to satisfy source-style checks.

- `fixtures/` and `fixtures.sha256`: the exact measured problem/include bytes.
  The LAT include is from local TPTP v10.0.0, not an assertion about the original
  J13 library.
- `baseline.callgrind.gz`, `baseline-{inclusive,exclusive}.txt` and
  `baseline-profile.{stdout,stderr}`: source-attributed baseline profiling at
  100 processed clauses. Recursive/inlined inclusive rows overlap; do not sum
  them or use instrumented elapsed time as release timing evidence.
- `cargo-final-verification.log.gz`: the passed final required Nix-wrapped
  workspace checks, plus all-target Clippy for the three changed crates.
  `cargo-verification.log.gz` preserves the earlier required-gate pass before
  the FVI test-style cleanup.
- `baseline-ring-proof.s`, `baseline-ring-proof-check.txt` and
  `proof-checker.sha256`: preserved proof bytes, a fresh strict `VerifiedGood`
  result with exact input identity, and the rebuilt checker binary hash.
- `candidate-ring-proof.s`, `candidate-ring-proof-check.txt` and
  `ring-proof-body.s`: a newly generated strict `VerifiedGood` proof and the
  byte-identical SZS proof section shared with the baseline. Full stdout hashes
  differ because the elapsed-time footer differs; this is not a proof change.
- `candidate.callgrind.gz`, `candidate-{inclusive,exclusive}.txt` and
  `candidate-profile.{stdout,stderr}`: the matched candidate profile. The
  instruction reduction is 4.26%; this is not an instrumented-time claim.
- `lat-200-pairs/`, `ring-100-pairs/`, `lat-400-pairs/` and
  `lat-400-replication/`: all experiment manifests, binary hashes, paired
  details and raw stdout/stderr. `lat-400-work.json` checks the shared work
  across the two larger blocks. `summary.json` records every pair ratio,
  means/ranges and the mixed-timing interpretation.

The timing arms use one worker
pinned to logical CPU 2, no sharing or LRS, an exact processed-clause ceiling,
a 120-second safety guard and a 12-GiB address-space/RSS allowance. They use the
same native release/LTO toolchain settings and preserve both arms of all five
randomized pairs per block. The larger lattice follow-up has ten pairs in two
blocks: nine favor the candidate, with a mean candidate/baseline ratio of
0.961. The shorter lattice screen has a mean ratio of 1.039 and the ring screen
0.994. No general wall-clock gain is claimed from these local observations.

## Reproduce

Build both revisions with identical flags. The existing `profiling` Cargo
profile provides release optimization with symbols; these archived builds used
its equivalent release overrides:

```bash
CARGO_TARGET_DIR=/path/to/build \
  CARGO_PROFILE_RELEASE_DEBUG=1 CARGO_PROFILE_RELEASE_STRIP=false \
  cargo build --release -p mrs -p mrs-bench --bin mrs --bin perf_probe --bin mrs_tools
```

Provide the native target CPU flags in the development environment for both
builds, or explicitly use the same portable CPU flags for both. The local Nix
shell supplies `-C target-cpu=native`. Do not compare different ISA builds.
Keep separate copies of the binaries before rebuilding the candidate.

```bash
export TPTP="$PWD/docs/results/perf/optimization-20261121/fixtures"
export MRS_MAX_MEMORY_MB=12288
ulimit -v 12582912
taskset -c 2 /path/to/mrs_tools fixed-work-compare \
  --baseline /path/to/baseline/mrs --candidate /path/to/candidate/mrs \
  --problem "$TPTP/LAT141-1.p" --schedule casc_ueq --strategy 4 \
  --processed 200 --guard-seconds 120 --reps 5 --seed 42 \
  --output /path/to/new-lat-comparison
```

Use `ring_idem.p` and `--processed 100` for the ring diagnostic. Use a fresh
output directory: the tool refuses overwriting archived comparisons. Warm up
both binaries first, and do not run builds or other agent benchmarks alongside
the timed arms. CPU 2 must be replaced by an available CPU on another host.
The larger lattice blocks use `--processed 400` with seeds 123 and 456.

The comparator requires matching selection digests and work counters for every
pair and across baseline repetitions. This digest covers selected clauses, not
every inference/pruning action. No eight-worker or division-wide gain follows
from this diagnostic. Failed/early-stopping runs are not speed measurements.
