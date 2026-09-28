# Fixed-work performance probe

> Status: current methodology for `mrs` search throughput. The numbers in
> `bank.tsv` and the dated reports beside it are measurement records, not CASC
> results and not solving results.

This directory banks **how fast `mrs` searches on a given machine**. It answers
"how many inferences per second does this box do, and how much does that scale",
not "how many TPTP problems does it solve".

For proving capability, use the CASC harness instead: `docs/guides/benchmarking.md`
and `crates/mrs-bench/casc.sh`. A solved count is the objective the schedules
were tuned for; throughput is a diagnostic.

## Why fixed work, not a wall clock

The obvious measurement — run the prover for 30 seconds, count the clauses it
processed — is **not comparable between machines**, and the reason is worth
stating because it invalidates most quick benchmarks.

The search is wall-clock sensitive. LRS (Limited Resource Strategy) prunes the
passive queue from `elapsed / iteration`, so a slower machine does not do the
same work more slowly; it prunes differently and explores a *different* search
space. Two hosts can both report "50k clauses/second" having done completely
different amounts of work, and neither number means anything about the other.

This probe removes the clock from the work and pins ambient settings that can
change the search:

- each worker is stopped by `resource_limits.max_processed`, a **clause-counted**
  ceiling, not by the clock;
- LRS runs under `LrsPolicy::FixedIterations` with a budget equal to that
  ceiling, so passive-queue pruning is iteration-counted too;
- cross-strategy sharing is disabled, and runtime overrides for LRS, ordering,
  preprocessing, InstGen and tracing are cleared inside the probe;
- the wall clock remains only as a safety net set far beyond any plausible run,
  so it never fires.

The counters — `iterations`, `processed`, `generated`, `fwd_subsumed`,
`lrs_discarded`, `weight_discarded` — are then identical on every machine and
for every build, and **elapsed time is the only variable**. `work_sha` is the
digest of exactly those counters, so a reader can verify that two rows really
did the same work instead of taking the claim on trust. The generated report
refuses to rank rows whose work fingerprints differ, and `perf_probe.sh`
re-runs the first configuration in a fresh process and aborts rather than bank
a timing whose work is not reproducible.

Tying the LRS budget to the work ceiling is what keeps the measurement steady.
With LRS effectively disabled the passive queue grows without bound, cost per
inference climbs superlinearly, and a "throughput" number becomes a function of
how long you happened to run.

## The workload

A seeded, generated clause set. No TPTP corpus, no `%include`, no `TPTP`
environment variable, and nothing to download. It is driven through
`mrs_search::strategy::run_schedule`, the same entry point `src/main.rs` uses
for a real problem, so the measured path is the production path: preprocessing,
the given-clause loop, literal selection, ordering, indexing, redundancy
elimination, and the resource ceilings.

The random stream is a hand-rolled SplitMix64 in `crates/mrs-bench/src/bin/perf_probe.rs`,
not a `rand` release. A bank row is only reproducible if the workload is pinned
by this repository's own code; a dependency bump that changed the random stream
would silently invalidate every archived row. `WORKLOAD_VERSION` in that file is
part of every workload id, so changing the generator's meaning deliberately
invalidates old rows.

Two fixed axioms — commutativity and distributivity over a binary symbol — keep
the superposition closure unbounded. Without them a random equational set tends
to saturate in a few thousand inferences, which measures nothing. They consume no
seed entropy; they are part of the workload definition.

The default single-worker run is about 11 s and under 400 MB on a 3.3 GHz
i7-5820K, with run-to-run spread near one percent.

## What a row records

`bank.tsv` is tab-separated, append-only, one row per measurement. The header is
checked on every append: a mismatch is a hard error rather than a silently
widened file. Full provenance per `docs/results/README.md`:

| Group | Columns |
|---|---|
| Host | `host_slug`, `cpu_model`, `physical_cores`, `logical_cpus`, `ram_total_mb`, `kernel`, `arch` |
| Build | `commit`, `dirty`, `target_cpu`, `rustc`, `glibc`, `binary_sha256` |
| Memory | `mem_budget_mb`, `hard_cap`, `peak_rss_mb` |
| Workload | `workload`, `workload_spec`, `selection`, `ordering`, `literals`, `avatar` |
| Budget | `workers`, `processed_cap`, `lrs_budget`, `max_passive`, `max_terms` |
| Result | `result`, `stop_reason`, `complete` |
| Work | `iterations`, `processed`, `generated`, `fwd_subsumed`, `lrs_discarded`, `weight_discarded`, `work_sha` |
| Timing | `schedule_ms`, `search_ms`, `iterations_per_s`, `processed_per_s`, `generated_per_s`, `generated_per_processed` |

`complete = 1` means the run stopped at the intended `max_processed` ceiling.
Any other stop (`saturated`, `time_limit`, `memory`, `max_passive`, `refutation`)
means the row measured something else, and the report files it under "Not
comparable" instead of ranking it. Those rows are kept because the stop reason
is diagnostic: `saturated` means the workload is too easy, `time_limit` means
the search got pathologically slower.

## The memory ceiling is 12 GiB

The probe is bounded to **12 GiB (12288 MB) of RAM in total**, enforced twice:

1. `MRS_MAX_MEMORY_MB=12288` reaches the prover's own RSS watchdog
   (`crates/mrs-search/src/resource.rs`), so the search stops itself with a
   `ResourceOut(memory)` verdict rather than being OOM-killed.
2. `perf_probe.sh` adds a kernel-level cap, recorded per row in `hard_cap`.
   The default is `rlimit-as`: an `RLIMIT_AS` bounds the whole address space,
   and resident memory can never exceed address space, so it is a real
   guarantee that needs no daemon. `--hard-cap cgroup-memory` uses a systemd
   `MemoryMax` instead, which bounds RSS more directly but needs a D-Bus session
   and fails under load, so it is opt-in.

Phases run one at a time, so the per-process ceiling is also the total — but
only after the workers are counted against it, because every worker draws on that
same total. `perf_probe.sh` budgets roughly 320 MB of search working set per
worker (the marginal RSS the bank measures is 288, 285 and 293 MB on the three
hosts measured so far) plus, under an address-space cap, the 64 MiB stack each
worker reserves, and **refuses** a worker count that will not fit rather than
starting a run that cannot finish. Under the default `rlimit-as` that is 32
workers at 12 GiB. Two things follow that are worth knowing:

- A host with more cores than that cannot run the probe at its default width.
  Pass `--workers` to say how wide to go; the 12 GiB ceiling is the probe's
  promise and will not be raised to accommodate a core count. The width the
  default lands on is the prover's own core count, so it is the same number the
  row records — see "Who counts the cores" below.
- `rlimit-as` charges a thread's stack reservation in full, even though the
  pages are not resident until used. That is why the reservation is budgeted
  here and why the CASC harness's own scripts, which do not cap address space,
  can ignore it.

`perf_probe.sh` refuses a `--memory-budget-mb` above 12288, and refuses to start
on a host with less than 2 GiB available, where the number would be shaped by
swapping rather than by the CPU. `peak_rss_mb` comes from the kernel's own
`VmHWM` high-water mark, so the ceiling can be audited from the artifacts.

If the OS refuses a worker thread anyway — a `pids.max`, a thread limit, an
address-space cap tighter than the budget — the search does not die. It runs the
workers that started, and the row records what actually ran: `workers` becomes
the count that did the work, `stop_reason` is `spawn_short`, and `complete` is 0,
so the report files it under "Not comparable" rather than ranking it. An earlier
version panicked here, and the panic's own message allocation failed on the same
exhausted address space, so the process died with SIGABRT and a core dump.

## Two builds per host: `native` and `haswell`

`.cargo/config.toml` sets `-C target-cpu=native`, so a binary built on one
machine targets that machine and is neither comparable nor necessarily runnable
on another. Every host is therefore measured twice:

- **`native`** — tuned for the host it was built on. The number you care about
  for that machine.
- **`haswell`** — a fixed floor (AVX2, BMI1/2, FMA), the CASC competition
  target, so hosts of different generations can be compared on a common ISA
  without ISA effects masquerading as hardware differences.

The two builds live in separate `CARGO_TARGET_DIR`s under `target/perf/`,
because switching `RUSTFLAGS` in a shared directory invalidates every cached
artifact. The first build of each is slow once; later runs are cache hits.

`haswell` is **skipped automatically on a CPU that does not advertise the
instructions**, rather than dying with `SIGILL`. The gate covers everything
`-C target-cpu=haswell` can emit, in the kernel's flag names (`cx16` for
`cmpxchg16b`), so it is the build, not a hand-kept subset, that decides. It
cannot see everything the build uses: `lzcnt` is not reported for Intel CPUs at
all, and `lzcnt`/`xsave`/`sse*` are omitted on the grounds that anything with
AVX2 has them. Those omissions cost no real row; a CPU that somehow passes the
gate and cannot run the build is caught by the signal named in the error.

On such a host the `haswell` row does not exist and `native` is the whole
measurement, so those rows are **not** comparable to any `haswell` row from
another host and are not a CASC hardware proxy — the CASC entry targets
`haswell` and its binary will not run on a pre-Haswell CPU. Pass
`--variants haswell` (or any set of unusable builds) and the probe says which
instructions are missing and measures `native` instead of failing.

`target_cpu` is therefore part of a row's identity, and the report never ranks
one build against the other.

## Who counts the cores

The driver sizes its default worker list from the host's physical core count,
and there are two implementations of that count: `physical_cores` in the shell
driver and `mrs_search::usable_physical_cores` in the binary that writes the row.
**The binary's answer wins**, and the driver asks for it as soon as the first
build exists — before anything is measured, and after the caller's own
`--workers` has already been checked against the memory budget.

That ordering exists because the two have been observed to disagree by 8×: on a
dual-socket Xeon E5-2407 the shell counter reported 64 physical cores on a host
with 8, so the probe asked for 64 workers, which the 12 GiB ceiling cannot hold,
and the run died rather than measuring anything. A worker list sized from the
wrong counter also contradicts the `physical_cores` column the bank records, so
one count has to be authoritative and it has to be the one that ends up in the
row.

Sizing by the prover's count costs one build before the refusal check on a
count-derived width, which is a cache hit after the first run. A width the
caller passed is checked immediately, before any build.

A disagreement is reported rather than silently corrected, because it means the
shell counter is wrong somewhere and the next host to hit it deserves to know.
The driver also prints the physical and logical CPU counts separately, since a
host whose topology sysfs is unreadable makes both counters degrade to the
logical count — indistinguishable from a host without SMT unless it is said out
loud.

## Running it

```bash
# Measure this host: both target-cpu builds, one worker and all physical cores.
# Appends to bank.tsv and writes a dated report beside it.
crates/mrs-bench/perf_probe.sh

# A fast look that touches nothing under docs/.
crates/mrs-bench/perf_probe.sh --processed 1000 --no-bank

# A single build, no AVATAR, on two workers.
crates/mrs-bench/perf_probe.sh --variants native --workers 2 --no-bank

# See every option.
crates/mrs-bench/perf_probe.sh --help
```

In a Nix-based development shell, `direnv exec .` (or an interactive shell with
the flake loaded) must already provide `cargo`. The script deliberately does not
invoke `nix develop`, so it runs unchanged on an ordinary Linux host.

Raw JSON rows are kept under `crates/mrs-bench/results/perf/<timestamp>/`
(gitignored); the bank keeps the numbers and the provenance, which is all the
comparison needs.

## Adding another machine

Run `crates/mrs-bench/perf_probe.sh` on it. That appends its rows to `bank.tsv`
and regenerates the report for that host and date. Rows from different hosts
appear in each other's "Cross-host comparison" tables automatically, **grouped
and filtered by work fingerprint** — a row is only ever ranked against rows
that did identical work.

Two rows are comparable when all of these match:

- `workload` (the generated clause set, including its seed and shape)
- `work_sha` plus the full work-counter tuple (`iterations`, `processed`,
  `generated`, `fwd_subsumed`, `lrs_discarded`, `weight_discarded`)
- `search` columns: `selection`, `ordering`, `literals`, `avatar`
- budget columns: `processed_cap`, `lrs_budget`, `max_passive`, `max_terms`
- `mem_budget_mb`, `hard_cap`, `workers`, and `target_cpu` if the ISA is meant
  to be held constant

Cross-host rankings are partitioned by every search and resource-budget field,
not just the short `work_sha`. A matching digest is a quick filter; it does not
override differences in search settings or resource ceilings.

`commit` is deliberately *not* a comparability requirement: measuring the same
commit on two machines is exactly the point of this bank. A different commit is
a legitimate row, and the `binary_sha256` column says exactly what was measured.

## Constraints this measurement does not cover

- It says nothing about solving ability. A schedule that refutes a problem in
  2 s and one that needs 200 s can have identical throughput.
- It is one workload shape. `--shape equational` and `--shape relational`
  exercise paramodulation and resolution separately; a hardware with a fast
  memory subsystem and a weak branch predictor will not necessarily rank the
  same way on all three.
- It does not model AVATAR's SAT solving as a separate cost, only as part of
  the search (`--avatar off` removes it).
- Absolute numbers are meaningless without `target_cpu`. A `haswell` number
  from a 2019 laptop and a `haswell` number from a 2024 server are comparable;
  either is incomparable to a `native` number.
