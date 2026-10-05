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
resolved against the working directory, not the repo root
(`crates/mrs-bench/casc.sh:191`).

---

## The two machines

| | dev box | remote (campaign) host |
|---|---|---|
| physical cores | **2** (4 logical) | **16** (32 logical) |
| CPU | Xeon Silver 4108 @ 1.80 GHz | Xeon Silver 4108 @ 1.80 GHz |
| RAM | **15.9 GiB total, ~11.7 GiB usable** | 128 GiB |
| `nix` | **absent** — call `cargo` directly | absent (same, by design) |
| rustc | 1.99.0 | 1.98.1 (as of the 2026-09-30 campaigns) |

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

> **`AGENTS.md` discrepancy.** It mandates `nix develop -c …` for validation, but
> `nix` is not installed in the environment these figures were measured in
> (`nix: command not found`; `/nix/store` absent). Everything here was run with
> `cargo` directly at `rustc 1.99.0`, which is the version `AGENTS.md` pins, so
> the toolchain matches the policy even though the wrapper does not exist. Either
> the doc or the environment should be corrected — do not silently keep
> substituting one for the other on a machine where `nix` *is* present.

---

## Before anything runs remotely

These are **not** remote items — they are code fixes that fit here, and they
gate the value of every remote run below. Running a campaign before they land
reproduces the failure they were found through.

1. **`casc.sh` must refuse a non-empty `--output`.** It unconditionally
   truncates `run.csv` with `>` (`crates/mrs-bench/casc.sh:356`) and never
   checks the directory. Two `casc.sh` processes were once launched
   concurrently with different `--edition` values and the same `--output`; they
   overwrote each other's `run.csv`, `run_meta.*`, raw artifacts and proof
   files. The signature is in the log: two `Total jobs:` lines with one
   `Output:`, and progress counters that exceed their own total
   (`600/300 completed`). The result is a run directory whose `Applicable`
   count is meaningless.
2. **`audit_casc_proofs::find_problem` must be edition-aware.** It takes one
   `--problems-dir` for the whole run and ignores the per-row `edition`
   column, so in a merged directory every `casc-30` proof gets leaf-checked
   against the `casc-j13` problem files. The two editions are the same problems
   re-serialised in a different formula order, so the mismatch is invisible
   except as `VerifiedBad` on the five problems whose cited leaves happen to
   differ. Those five are *not* prover bugs — every leaf reproduces its own
   edition's text exactly.
3. **Proof and raw-artifact paths must include the edition**, or a mixed-edition
   `run.csv` must be rejected outright. 256 of 600 rows in that directory
   collided and reported `artifact_mismatch`, i.e. were never checked, and the
   audit counted them as applicable anyway.
4. **The audit denominator is fixed** (commit `bdb5674`, branch
   `fix/no-proof-mgt079-set017`, not yet on `main`): `Other` was a dead column,
   `proof_omitted` did not exist, and `certification_campaign.sh` divided by
   only the five statuses it knew about. Merge it before a campaign, or the new
   campaign's summary will be as misleading as the old one.

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
MRS_WORKERS=8 crates/mrs-bench/certification_campaign.sh \
    --edition casc-j13 --systems mrs --divisions fne,feq,ueq \
    --casc-times --jobs 2 --output crates/mrs-bench/results/cert-j13-$(date +%Y%m%d)
```

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
MRS_WORKERS=8 crates/mrs-bench/certification_campaign.sh \
    --edition casc-30 --systems mrs --divisions feq \
    --casc-times --jobs 1 \
    --output crates/mrs-bench/results/campaign-casc30-feq-$(date +%Y%m%d)
```

One edition, one directory, one `--jobs` value. `--jobs 1` unless the host is
otherwise idle, because `--jobs N` and `MRS_WORKERS=8` together put N
searches' worth of workers on the same 8 pinned cores — which is exactly how the
void campaign's timings became meaningless. This is also the validation for
prerequisite items 2 and 3 above: after the corpus-resolution fixes land,
**no** campaign should produce a `VerifiedBad` leaf mismatch. A single one means
the edition is still being resolved wrongly.

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

> **Blocked by:** the probe refuses a worker count its ceiling cannot hold; at
> 32 workers under `rlimit-as` that refusal fires, and on a 2-core box the
> worker ceiling is reached almost immediately, so the measurement would be of
> a configuration nobody ships.

```bash
crates/mrs-bench/perf_probe.sh              # measure, bank, and report (~3 min)
crates/mrs-bench/perf_probe.sh --no-bank    # measure only
```

Method and comparability rules: `docs/results/perf/README.md`.

### R6 — Locate the prover site that emits an under-cited `ac_superposition` conclusion

The symptom is fully characterised and reproduces in 90 s on this box; only the
final step needs the campaign host.

`KLE145-10` (CASC-30 UEQ) refutes here in 90.7 s at `--workers 8`, producing a
199-node proof whose node `c411616` the strict kernel refuses:

```
cnf(c411616, plain, true = iteq(true, true,
      leq(addition(one, X14), multiplication(strong_iteration(X14), one)), true),
    inference(ac_superposition, [status(thm)], [c409250, c2118, c3, c21])).

cnf(c412250, plain, true = leq(addition(one, X14), strong_iteration(X14)),
    inference(demodulation, [status(thm),
      demodulation_steps(rule(1, 0, [1]), rule(2, 0, [1, 1]))], [c411616, c7, c92])).
```

`c7` is `ifeq_axiom_002` (`X0 = ifeq(X1,X1,X0,X2)`) and `c92` is
`multiplicative_right_identity` (`multiplication(X0,one) = X0`). Both are problem
axioms, and both are applied to `c411616` **in the expanding direction** — `X`
becomes `iteq(true,true,X,true)` and `X` becomes `X*one`. The very next proof node
demodulates both away again. So the emitted conclusion is the true superposition
conclusion plus two expanding rewrites by uncited problem axioms, and the search
immediately undid them, which is why nothing downstream looks wrong.

Two facts make this a prover bug rather than a checker limit:

- The kernel's `AcReplay::NotFound` (not `BudgetExhausted`) proves the replay
  exhausted every superposition position and orientation and genuinely found no
  match. `max_equivalence_steps` was not hit. See the `AcReplay` enum added in
  `crates/mrs-proof-kernel/src/lib.rs` on branch
  `fix/ac-superposition-cite-folded-demodulators`.
- Dumping the replay's own candidate shows the difference is **not** AC
  permutation. Candidate:
  `iteq(addition(X2,addition(X2,X3)), addition(X2,X3), leq(X2,addition(X2,X3)), true)`.
  Goal: `iteq(true,true, leq(addition(one,X14), multiplication(strong_iteration(X14), one)), true)`.
  The goal has `multiplication` where the candidate has `addition`.

**What is still unknown: which code path applies the expanding rewrite and keeps
the superposition's identity.** These were all read and all behave correctly, so
the site is not among them:

| candidate site | why it is not it |
|---|---|
| `mrs-calculus/src/superposition.rs` | no demodulation, simplification or folding anywhere in it |
| `mrs-search/src/given_clause.rs:2874` forward demodulation | `demodulate_id_until` already returns a correctly attributed `demodulation` node (`demodulation.rs:452-466`) |
| `state.rs:616` `ac_normalize_for_search` | `ac_normalize_clause` preserves `clause.id` and `clause.source` |
| `mrs-core/src/term_bank.rs:467` `ac_normalize` | pure flatten/sort/rebuild; cannot introduce `multiplication` or an `iteq` shell |
| condensation / DER / forward SR | each sets its own rule name when it fires |

The one structural asymmetry that remains is `SearchState::store_clause`
(`state.rs:333`), which does an **unconditional** `clause_store.insert` — last
write wins — while `register_clause` (`state.rs:541`) uses `or_insert_with` and
so is first-write-wins. `push_unprocessed` (`state.rs:371`) goes through
`store_clause`. A clause rewritten in place and re-pushed would keep its id and
its arena witness while its stored literals changed, which is exactly the observed
shape.

> **Blocked by:** the search. Every instrumented run costs 90 s on this box, and
> the probes that would localise it are high-volume — a single unconditional
> `store_clause` trace emitted 668 293 lines / 271 MB, and `/tmp` here is tmpfs,
> so an unfiltered trace is a RAM-availability problem, not just a disk one. Note
> also that `IdAtom`'s `Debug` prints `SymbolId(n)`, not the function name, so any
> grep-based probe must resolve names through `SymbolTable::iter_names()` before
> matching, or it will silently match nothing.

Two ways to close it, cheapest first:

```bash
# 1. Cheapest: one targeted trace at the suspected overwrite, keyed on the clause
#    id taken from the proof of the *same* run (ids are not stable across runs --
#    observed 431338 / 411616 / 426189 / 440158 / 441748 / 449061 for the same
#    node). Compare the literals passed to store_clause against the literals
#    already in clause_store for that id, and write the diff somewhere off tmpfs.
#
# 2. Better: an invariant assertion in the prover rather than a trace. For every
#    clause registered with rule superposition/ac_superposition, assert that the
#    clause the DAG keeps for that id has literals equal to the ones the
#    superposition produced. A debug_assert fires once, at the exact site, with
#    no volume problem at all. This is the one to prefer.
```

Once the site is known the prover-side fix is small: either keep the expanded
clause's own `demodulation` provenance, or stop the forward demodulator from
taking the expanding orientation of a rule whose contracting orientation exists
(`demodulation.rs:615` iterates whatever orientation the index hands it). The
second is better — it removes the need to cite anything, because the conclusion
then *is* the superposition.

Note for whoever picks this up: `casc-30` UEQ scores 123/300 with 121 certified,
and exactly **one** node in one proof is affected. This is not a soundness
problem — the refutations are genuine — but it does cost 2 of 300 rows their
`VerifiedGood`, and a prover that emits uncited steps will keep producing them.

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
outright. Splitting them (`max_proof_nodes`, branch
`fix/no-proof-mgt079-set017`) admits all four. All from
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

---

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

Put the resulting numbers in
[`../CERTIFICATION_STATUS.md`](../CERTIFICATION_STATUS.md) §2, which is the
authoritative record for what `mrs-proover` certifies, and correct the hardware
line along with the figure — `AGENTS.md` §11 makes 8 physical cores part of the
number, not a footnote on it.