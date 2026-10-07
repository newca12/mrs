# Work that needs the remote host

Everything `mrs` cannot measure on a development box, in one place, with the
constraint that blocks it and the command that unblocks it.

The point of this file is that the split is easy to get wrong in both
directions. Work gets deferred to the remote host that would have fitted here,
and work gets reported from here as though it were a competition number. Both
have already happened: the FEQ certification numbers in
[`../CERTIFICATION_STATUS.md`](../CERTIFICATION_STATUS.md) were once read as
coverage when 78 of 144 refutations had never been checked, and the largest
proof in the archive was recorded as "never produced a verdict at all" when the
real cause was a kernel ceiling that a five-minute check here clears.

So each item below states what blocks it. If an item's blocker turns out to be
absent on your machine, it is not a remote item and belongs on the dev box.

Commands are written to be run **from the repository root**; `--output` is
resolved against the working directory, not the repo root.

---

## The two machines

| | dev box | remote (campaign) host |
|---|---|---|
| physical cores | **2** (4 logical) | **16** (32 logical) |
| CPU | Xeon Silver 4108 @ 1.80 GHz | Xeon Silver 4108 @ 1.80 GHz |
| RAM | **15.9 GiB total, ~11.7 GiB usable** | 128 GiB |
| `nix` | **absent in the measured campaign environment** — call `cargo` directly | absent (same, by design) |
| rustc | 1.99.0 | 1.98.1 (as of the 2026-09-30 campaigns; current runner expects 1.99.0) |

`--hardware casc-sim` pins the process to 8 physical cores and sets an
`RLIMIT_AS` of 128 GB, because that is what CASC is. Neither is satisfiable
here, so **no number produced on this box is a CASC number** — see the decision
rule in `AGENTS.md` §11, which is permanent policy and not up for local
relaxation. `--workers N` does override the mode and really does run N workers,
so a reduced-width run is legitimate for smoke-testing the harness; it just is
not a measurement of anything competitive.

`nix develop` is unavailable on both hosts. `remote-cert-campaign.sh` states
the remote convention explicitly — "plain cargo/rustup (no nix on remote)" —
and requires pinning rustc to the same version on both sides (1.99.0 today),
recording `rustc --version` plus the git revision in every phase output. A
version mismatch warns but does not stop a campaign; note it in the report
instead of ignoring it.

> **Validation environment note.** The recorded campaign figures came from
> environments without Nix and were built with plain Cargo. In this repository's
> NixOS development environment, follow `AGENTS.md` and wrap validation commands
> with `nix develop`; remote campaign scripts intentionally invoke Cargo
> directly. The old machine snapshot does not describe the current checkout
> environment.

---

## Before anything runs remotely

These protections are implemented on `main`; keep them in place because they
guard the value of every campaign.

1. **`casc.sh` refuses non-empty `--output` directories and atomically claims
   new/empty ones.** This prevents reruns or concurrent invocations from
   truncating `run.csv`, metadata, or raw artifacts.
2. **The proof audit rejects mixed-edition CSVs.** It previously took one
   `--problems-dir` for the whole run and ignored per-row `edition`, which could
   check `casc-30` leaves against `casc-j13` problem text. Keep editions in
   separate run directories; the audit now fails closed if they are mixed.
3. **The audit denominator fix is on `main`** (commit `bdb5674`): `Other` captures
   unclassified outcomes, `proof_omitted` is explicit, and campaign percentages
   include every refutation. Preserve this invariant when changing audit
   statuses.

---

## Remote-only items

### R1 — Fresh `casc-j13` campaign for fne, feq, ueq

**Blocks the single largest open measurement in the repo.** The 18
`demodulation` rows are recorded as fixed but unmeasured, and they cannot be
measured here or by re-audit: the archived proofs predate the
`demodulation_steps(...)` annotation, so replaying them exercises the *fallback
search* path, which still returns `Unknown` for all 18. Only a fresh search
emits the annotation the new replay path needs.

> **Blocked by:** 8 pinned physical cores and a 128 GB `RLIMIT_AS`. Also wall
> clock — `--casc-times` gives CASC-J13's 180 s for FEQ/FNE/UEQ, across ~800
> problems.

```bash
MRS_WORKERS=8 MRS_HARDWARE=casc crates/mrs-bench/certification_campaign.sh \
    --edition casc-j13 --systems mrs --divisions fne,feq,ueq \
    --casc-times --jobs 1 --output crates/mrs-bench/results/cert-j13-$(date +%Y%m%d-%H%M%S)
```

The explicit `casc` profile keeps the run at the canonical 8-worker CASC
configuration on the 16-core host. One external job avoids competition between
multiple prover processes for those workers.

Expect the 18 demodulation rows to become `Certified`. If they do not, the
reason histogram in the campaign output names the shape that still fails — that
histogram is the deliverable, not the percentage.

### R2 — Fresh `casc-30` FEQ campaign, as its own directory

The existing `campaign-feq-W8C16J2-20260930` is void: two `casc.sh` processes
raced into one output directory (see "Before anything runs remotely" item 1),
and its 600-row `run.csv` is 300 `casc-30` plus 300 `casc-j13` rows sharing one
set of artifact paths. Its headline numbers cannot be repaired after the fact.

Note the campaign is also not a valid `W8C16J2` measurement independent of the
corruption: 4 concurrent `mrs` processes × 8 workers each, all pinned to the
same 8 cores. And the `casc-30` half is internally inconsistent too — 48 of its
300 rows carry `timeout=240` and 252 carry `timeout=180`, while `--casc-times`
gives FEQ a single 240 s limit. Two invocations that disagreed about the limit
wrote into one file.

```bash
MRS_WORKERS=8 MRS_HARDWARE=casc crates/mrs-bench/certification_campaign.sh \
    --edition casc-30 --systems mrs --divisions feq \
    --casc-times --jobs 1 \
    --output crates/mrs-bench/results/campaign-casc30-feq-$(date +%Y%m%d-%H%M%S)
```

Set `MRS_HARDWARE=casc` for this campaign too.

One edition, one directory, one `--jobs` value. `--jobs 1` avoids competing
benchmark processes for the same 8-worker CASC allocation — which is exactly
how the void campaign's timings became meaningless. Check that the audit has no
edition-related artifact or leaf-provenance errors.

### R3 — `SET017+1` re-solve

The refutation is real — its archived 119,335-formula proof certifies — but the
*search* for it is not reproducible on this box.

> **Blocked by:** the 8-worker `casc_feq` cascade. Measured here: strategies 11
> and 12 solo at `--time 240` both `Timeout`; 2 workers with `--time 300`
> `Timeout`. The campaign run solved it in 177 s using 17.4 GB.

`MGT079+1` in the same family needs nothing special (36 s, 7.2 GB on the
campaign host; 2 min and 2.5 GB here at 2 workers). `SET017+1` is the one that
needs the machine.

### R4 — `--proof-bytes-limit` policy, measured against the CASC allowance

The default is 8 MiB and the harness never overrides it, so proofs above it are
*omitted with a diagnostic* rather than truncated — a deliberate trade, because
an over-budget proof that the harness kills takes the status line with it and
loses the solve too. The cost is that a solved problem arrives with no
evidence. In the void FEQ campaign that was 2 of 144 refutations (21.2 MB and
38.0 MB).

> **Blocked by:** a measurement, not a code change. Deciding the right limit
> needs the real CASC output allowance on the real hardware, and the
> proof-size distribution at CASC wall clocks.

Two things worth measuring while there:

- **Proof size varies with portfolio width, not just with the problem.**
  `MGT079+1`'s archived 8-worker proof is 38.6 MB / 227 551 formulas and the
  kernel refuses to print it under the default limit; a fresh **2-worker** run on
  the same problem produces a 2.8 MB / 17 792-node proof. A 7× swing from
  scheduling alone. Whether a portfolio's proofs fit the allowance is partly a
  schedule decision, which makes this an argument for measuring per-division
  rather than picking one global number.
- **`SWV406+1` is 169,921,314 bytes / 657,454 nodes.** Its archived proof now
  certifies (98.9 s, 4.8 GB, within the harness's 120 s `--strict-time`), but a
  fresh solve at CASC limits is what produces a proof that size today.

Until this is measured, `docs/PROOF_SIZE_BUDGET.md` §"Reducing the tail" is the
analysis and the 8 MiB default is the policy.

### R5 — Performance probe on the campaign host

`crates/mrs-bench/perf_probe.sh` banks fixed-work measurements in
`docs/results/perf/bank.tsv`. It is capped at 12 GiB by design and *will* run
here with a warning, but a 2-core row cannot be read against a 16-core run's
per-worker numbers.

> **Blocked by:** comparability, not whether it can run. The probe runs on the
> dev box too, but its default `1,<physical cores>` worker counts measure that
> 2-core machine. A row for the 16-core campaign host must be collected there
> with the same probe parameters and build variants.

```bash
crates/mrs-bench/perf_probe.sh              # measure, bank, and report (~3 min)
crates/mrs-bench/perf_probe.sh --no-bank    # measure only
```

Method and comparability rules: `docs/results/perf/README.md`.

### R6 — ~~Locate the prover site that emits an under-cited `ac_superposition` conclusion~~

**Not remote. Retracted — see [UI-1](../policies/unresolved-issues.md#ui-1--ac_superposition-conclusions-that-are-not-a-superposition-of-their-cited-parents).**

This entry originally claimed the investigation was blocked by the 8-core/128 GB
pin and named `demodulation.rs:615` as the fix. Both were wrong:

- casc-j13 UEQ (`LAT044-1`, node `c659171`) shows the same defect with **no
  `ifeq` and no expanding problem axiom**, so an orientation change would not
  have fixed it;
- the work needs one ~90 s solve on this box, because an invariant assertion
  costs nothing while the traces I used cost 668 293 lines / 271 MB.

The under-cited conclusion is an observed kernel-replay mismatch, not evidence
that the prover's final refutation is unsound. The claimed 4/700 certification
rate was not independently re-audited and should not be used as a current
measurement.

Kept as a stub so the original claim is not rediscovered and re-believed. The
real entry is UI-1.

### R7 — Campaign hosts are not uniform in RAM, and the tail is memory-bound

> **Blocked by:** nothing. This is a recording, not a task. It is here because
> the two 2026-10-02 UEQ campaigns disagree about their own hardware and the
> disagreement is not visible in either `run.csv`.

The casc-j13 UEQ campaign ran on `tlpnf9701` with **63 763 MB**, the casc-30 UEQ
campaign on `teenf9901` with **128 019 MB**. Both used `MRS_WORKERS=8` and
`hardware=adaptive`. Consequences already visible in the data:

| | casc-j13 (63 GB) | casc-30 (128 GB) |
|---|---|---|
| problems | 400 | 300 |
| budget (`--casc-times`) | 180 s | 240 s |
| solved | 112 | 123 |
| certified | 110 | 123 − 2 Unknown |
| `ResourceOut` | 1 (`RNG130-1`) | 0 |
| runs over 40 GB peak | **14** | 8 |

`RNG130-1` stops on
`resource_reason=memory resource_detail="memory limit 46735 MB reached (rss 46927 MB)"`.
It is not in the casc-30 problem set, so the 63 GB ceiling decided that problem's
outcome and the 128 GB ceiling would not have.

The load-bearing consequence is that **the tail of these runs is set by the
memory ceiling, not by the prover**, so two runs on unequal hosts are not
comparable in the tail even when the problem set matches. When hosts differ,
report the ceiling alongside the score, or restrict comparison to problems whose
peak RSS sits well below the smaller ceiling.

A **third** host appears in the FEQ pair, which makes the point sharper: the two
2026-10-02 FEQ campaigns ran on `mtsdev03` and `mtsdev04` — matched at 8 cores /
95 969 MB / Xeon E5-2407 @ 2.20 GHz — which is the only properly matched pair in
the set. The UEQ pair ran on `teenf9901` (128 GB, Silver 4108 @ 1.80 GHz) and
`tlpnf9701` (63 GB, same CPU). So the corpus editions were measured on
different CPU generations, and every run has a distinct `binary_sha256` from the
same commit `af983acae`, i.e. dirty trees that are not the same dirty tree.

**Fixing this properly** means either provisioning campaign hosts with matched
CPU and RAM, or setting an explicit `MRS_HARDWARE` memory allowance so the
ceiling is a policy decision recorded in the run rather than a property of
whichever machine answered the job.

### R8 — Optional 8-worker reproduction of the AC-superposition mismatch

The local debug attempts (`--workers 2`) timed out on KLE145-10 after 60.1 s
and LAT044-1 after 45.1 s, both at 1.36 GB peak, before reaching the mismatch.
This item is the fallback if further debugging should reproduce it; do not run a
full campaign or enable inference tracing.

> **Blocked by:** the reproduction previously used 8 workers and peaked at
> **8.9 GB RSS**. The campaign host must have 8 available physical cores and at
> least 12 GiB available memory before starting; skip the run if either limit
> is not met. Do not run it alongside another benchmark.

```bash
cargo build --bin mrs
MRS_HARDWARE=casc-sim MRS_MAX_MEMORY_MB=10000 \
    ./target/debug/mrs --time 200 --workers 8 --schedule casc_ueq \
    crates/mrs-bench/problems/casc-30/UEQ/KLE145-10.p
```

Record the git revision, rustc version, `free -h`, core allocation, and whether
the debug assertion fired. `casc-sim` pins the process to 8 physical cores; the
explicit 10 GB memory cap leaves headroom below the 12 GiB minimum. This single
bounded problem run is diagnostic only; it does not produce a CASC coverage
number.




### R9 — casc-j13 FEQ: the `expected` column is empty for 236 of 300 rows

> **Blocked by:** nothing. This invalidates the run's headline number and needs
> re-running once the grading input is fixed.

In the archived `campaign-cascj13-feq-W8C8J1-20261002`, `run.csv` reports **16**
rows with `verdict=ok` out of 300. The prover emitted `Theorem` on **69** rows:

| `szs_status` | `expected` | `verdict` | rows |
|---|---|---|---|
| `Theorem` | *(blank)* | `unknown` | **53** |
| `Theorem` | `Theorem` | `ok` | 16 |
| `Timeout` | *(blank)* | `unknown` | 154 |
| `Timeout` | `Theorem` | `unknown` | 44 |
| `GaveUp` | *(blank)* | `unknown` | 29 |
| `GaveUp` | `Theorem` | `unknown` | 4 |

`expected` is blank for 236 of 300 rows, so 53 `Theorem` results have no
reference grading and become `unknown`. The archived audit reportedly found 66
proof artifacts for those 69 results and certified 47; the run directory/raw
proofs are not committed here, so treat those audit counts as reported campaign
data until re-audited. **The `ok` count is a grading count, not prover
coverage**, and must not be quoted as the number proved.

The blank cells appear to come from an absent/incomplete edition-specific
answer table: `casc.sh` reads `answers_casc-j13.tsv` (falling back to
`answers.tsv`), and both `fetch_answers.sh --edition casc-j13` and the fallback
must be checked before attributing the issue to corpus format. The archived
casc-30 FEQ run populated `expected` for 399/400 rows, but was graded with a
different edition answer table; this is not a controlled comparison of grading
inputs. Before re-running, produce and validate the casc-j13 answer file and
verify expected-status counts against the edition's official status sources.

### R10 — Strict structural leaf matching rejects some equivalent included axioms — RESOLVED

**Not remote, and not a remote item any more.** The artifacts were reachable
after all, and the finding was reproduced and fixed on
`cert/feq-kernel-limits`.

`mrs-proover --strict` over the retained proofs of
`campaign-cascj13-feq-W8C8J1-20261002` reproduces the archived row for row --
47 `VerifiedGood`, 16 `VerifiedBad`, 3 `Unknown` -- so the count was not stale
and the rows were distinct from the answer-table grading issue above. All 16
leaves were then checked against their cited axioms by a canonicaliser written
from the TPTP grammar rather than from the kernel (de Bruijn indices for bound
variables, flattened and sorted `&`/`|`, mirrored `<=>` and `=`, `<=` as `=>`).
All 16 are AC-permutations of their cited axiom.

Fixed by adding the AC laws of `&`/`|` to the *leaf* comparison only;
`mrs_core::alpha::alpha_equiv` is unchanged and a test pins that it still
refuses the archived pair. The campaign now audits 64/66 `VerifiedGood` with 0
`VerifiedBad`. Full write-up and repro in
[`unresolved-issues.md`](../policies/unresolved-issues.md) **UI-4**.

The eight FEQ rows that remain are a different question, and two of them are
*not* remote either: they need a change to the kernel's variable model and a
different CNF expansion algorithm respectively. See **UI-7**.

### R11 — Hard-tail throughput: the LRS-off A/B on casc-30 UEQ — RUN, NEGATIVE

**Done on `teenf9901`.** The result is negative and the hypothesis it tested is
retired: passive-queue retention is **not** what limits the hard tail.

```
MRS_WORKERS=8 MRS_HARDWARE=casc-sim MRS_SIM_TIME_FACTOR=1 \
MRS_NO_LRS=1 MRS_MAX_MEMORY_MB=90000 CERT_JOBS=4 \
crates/mrs-bench/certification_campaign.sh \
  --edition casc-30 --systems mrs --divisions ueq --casc-times --jobs 1 \
  --output crates/mrs-bench/results/campaign-casc30-ueq-W8P8J1-NOLRS-MB90G-20261005
```

111 solved against the baseline's 123, but the comparison is **uninterpretable in
either direction**, for two independent reasons. The baseline ran `--hardware
adaptive` unpinned while this arm is `casc-sim` pinned to 8 physical cores, so
throughput moved too; and the -12 is **exactly the measured run-to-run noise
floor**. Five replicates of a fixed 25-problem subset, identical configuration,
solved 21 / 20 / 19 / 21 / 20 — ±1 per 25, which scales to ±12 per 300.

The loss distribution (14 of 17 lost problems solved at 120-238 s, against an
overall solve median of 15 s) looks like per-worker slowdown rather than worse
search. What survives is the directional sign on this hardware configuration and
nothing stronger; the earlier "LRS is load-bearing" reading overstated it.
Settling this needs the control arm — `casc-sim`, pinned, LRS at default — on the
12-problem subset, **replicated ≥5 times and compared on means**. Full analysis is
in [`unresolved-issues.md`](../policies/unresolved-issues.md) **UI-8**.

**Every portfolio A/B in this file is affected by this.** `cooperative_portfolio_sweep.sh`
is a single `casc.sh` exec with no replicates, and `cooperative_portfolio_search.sh`
maximises over ~120 noisy evaluations per round (`rounds × 8 slots × 15
candidates`), which selects on noise. `greedy_set_cover` builds each strategy's
solved-set from one `run.csv`. Against a ±12/300 noise floor, a single-run
comparison cannot resolve a portfolio difference of the size those tools are asked
to detect. Use `run_variance.sh` and compare means over ≥5 reps. It runs full
campaign replicates sequentially, so it measures within-configuration variance;
comparisons between configurations should still be paired/interleaved to limit
host-drift bias.

Two things worth keeping from the run mechanics:

- **`MRS_SIM_TIME_FACTOR=1` is mandatory under `casc-sim`.** The default of 2.0
  makes `total_budget` 476 s while `casc.sh` SIGTERMs at 250 s, which turns every
  over-running problem into a silent kill with no SZS line — UI-2's signature,
  introduced by the harness rather than the prover.
- **`MRS_MAX_MEMORY_MB` is not a cap.** It feeds a polled watchdog
  (`given_clause.rs:1576`); the only hard limit is casc-sim's `RLIMIT_AS`. Use
  both, or a runaway queue ends at the OOM killer.

---

## What is *not* remote — measured here, do it here

Recorded so these are not deferred a second time. All figures from this box.

**Re-auditing archived runs is cheap.** The whole point of
`certification_campaign.sh --audit-only` is that nothing in the audit depends on
the search having just finished.

| measurement | result |
|---|---|
| 215 archived refutations, `--strict-time 120`, `--jobs 1` | **310 s wall, 4.77 GB peak RSS** |
| 20-problem sample, same settings | 35 s, 547 MB |

```bash
crates/mrs-bench/certification_campaign.sh --audit-only \
    crates/mrs-bench/results/casc-j13-W8J2-nosharing-20260923
```

**The four proofs the kernel used to refuse all certify here.** One
100 000-formula ceiling used to do two jobs — bounding the proof's DAG and
bounding per-step CNF expansion — so a long-but-honest proof was refused
outright. Splitting them (`max_proof_nodes`) admits all four. All from
`casc-j13-W8J2-nosharing-20260923`, `--strict-time 120`:

| proof | formulas | proof bytes | verdict | cost here |
|---|---:|---:|---|---|
| `feq/SWV406+1` | 657 454 | 169.9 MB | `VerifiedGood` | 98.9 s, 4.77 GB |
| `feq/MGT079+1` | 227 551 | 38.6 MB | `VerifiedGood` | 26.4 s, 970 MB |
| `feq/SET017+1` | 119 335 | 31.1 MB | `VerifiedGood` | 14.4 s, 660 MB |
| `fne/KRS234+1` | 103 464 | 21.1 MB | `VerifiedGood` | 13.8 s, 531 MB |

`SWV406+1` at 4.8 GB is the largest single allocation on this box. It fits, but
do not run it concurrently with anything else memory-hungry. Note the four were
`Unknown` before the split and are `VerifiedBad`-free after it, so this is
strictly additive: 189 → 193 certified on that run.

**Re-auditing cannot close the demodulation gap** (see R1) — that is the one
place where re-audit and fresh search genuinely differ, and it is worth being
precise about which is which.

## Recording a result

A remote run is only useful if it is traceable afterwards. Every phase should
record, per `remote-cert-campaign.sh`:

- the exact command line, including `MRS_WORKERS`, `MRS_HARDWARE` and
  `MRS_SHARED_POOL_INTERVAL`
- `rustc --version` and the git revision, and whether the tree was dirty
- host `cpu_cores`, physical cores and `memory_mb`
- the corpus root, and whether it came from the run's own `run_meta.txt`

Then check the output before reading the headline: one `Total jobs:` line per
`casc.sh` invocation, progress counters that do not exceed their own total, and
a verification table whose columns sum to `Applicable`. A campaign that fails
those three checks produced no measurement, whatever its percentages say.

### Comparing two runs is a separate operation from running one

The 2026-10-02 UEQ campaigns are the worked example of getting this wrong.
casc-30 UEQ reports **41.0%** (123/300) and casc-j13 UEQ reports **28.0%**
(112/400), which reads as a 13-point gap and is **not one**:

| | casc-j13 | casc-30 |
|---|---|---|
| problems in run | 400 | 300 |
| problems in common | **232** | **232** |
| budget | 180 s | 240 s |
| solved on the 232 common problems | 71 | 78 |
| solved at a *common* 180 s budget | **71** | **71** |

At an equal budget the two editions solve **exactly the same 71 problems**. The
headline difference is 60 s of extra wall clock, and the per-run rates are
inflated by 168 and 68 problems that appear in only one of the two sets.

The two runs also differ in `binary_sha256` and in host RAM (see R7), so they are
not a controlled comparison in the first place.

### The six casc-30 divisions, as measured on 2026-10-02

Worth keeping together, because the per-division numbers are otherwise quoted
from memory and four of these six are not comparable to each other.

| division | n | budget | `verdict=ok` | rate | certified | non-definitive expected | host | `binary_sha256` | commit |
|---|---:|---:|---:|---:|---:|---:|---|---|---|
| ueq | 300 | 240 s | 123 | 41.0% | 121 | 0 | teenf9901 | `a63d62fd` | `af983aca` |
| fne | 100 | 240 s | 39 | 39.0% | not audited | 0 | mtsdev02 | `d0df62c2` | `af983aca` |
| eps | 100 | 120 s | 44 | 44.0% | not audited | 0 | mtsdev02 | `d0df62c2` | `af983aca` |
| feq | 400 | 240 s | 106 | 26.5% | **90** | 0 | mtsdev04 | `a644d9de` | `af983aca` |
| epu | 100 | 120 s | 9 | 9.0% | 9 | **5** | mtsdev01 | `fa52c23d` | `fb55719c` |
| icu | 101 | **480 s** | 3 | 3.0% | **3** | **44** | mtsdev02 | `d0df62c2` | `fb55719c` |

Caveats that apply to this table — each of these has cost a wrong number:

- **`verdict=ok` is not "certified".** `casc.sh:798-816` grades by status class
  only and never consults the audit. In ICU, `EEE001+1` is `ok` with
  `strict_status = Unknown`. Use the `certified` column above, from
  `certification/audit.csv`.
- **Rows with inconclusive references are ungraded.** ICU has 44 `GaveUp`
  references (the other expected statuses are decisive); its 3 `ok` rows are
  5.3% of the 57 rows with decisive references. EPU has 2 `expected=Timeout`
  rows as well as 3 `expected=GaveUp` rows, so its scoreable denominator is 95,
  not 100: 9 `ok` is 9.5% of that subset. `casc.sh` maps all non-definitive
  reference statuses to `verdict=unknown`. See **UI-6**.
- **feq certifies 90, not 106.** 7 solves have no proof, over the 8 MiB
  `--proof-bytes-limit` (R4).
- **Budgets differ by division**, following `--casc-times`: 480 s for icu,
  240 s for ueq/fne/feq, 120 s for eps/epu. Rates are not comparable across
  that split, and icu is not comparable to anything.
- **Four hosts, four binary shas, two commits.** fne, eps and icu share binary
  `d0df62c2`; only ueq/feq/fne/eps share commit `af983acae`. See R7.
- **epu's 9/100 `ok` results reproduce**, but the raw percentage includes five
  rows with non-definitive expected statuses. On the 95 rows with a decisive
  reference, 9 `ok` is 9.5%. A second run on a different binary and host
  solved the same nine problems. See **UI-5**.

**Before comparing any two runs, check all four of:** problem-set intersection
and size; per-problem `timeout`; `binary_sha256`; and host RAM. If the
intersection is smaller than either run, the rates are not comparable and the
comparison must be done on the intersection at a common budget. Comparing
per-run rates across editions is the single easiest way to publish a number that
does not mean anything.

Put the resulting numbers in
[`../CERTIFICATION_STATUS.md`](../CERTIFICATION_STATUS.md) §2, which is the
authoritative record for what `mrs-proover` certifies, and correct the hardware
line along with the figure — `AGENTS.md` §11 makes 8 physical cores part of the
number, not a footnote on it.
