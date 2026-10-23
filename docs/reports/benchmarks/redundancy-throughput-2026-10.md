# Redundancy elimination was 47% of the search, and most of it was waste

> Status: historical, **superseded pending remeasurement**. The branch measured
> here memoized negative demodulation answers with per-root invalidation. Review
> found that a rule for a nested subterm can make an enclosing term reducible,
> leaving the old entry stale. The fix uses global O(1) generation invalidation;
> the throughput and hit-rate numbers below were collected before that fix and
> must not be treated as measurements of the corrected implementation.
> Branch `perf/redundancy-throughput`, base commit `233e0d8`.
> Measured on this branch's release build (sha256 `16c38c223c6453c8…`) against a
> base build of `233e0d8` (sha256 `d12c0dd774f965a5…`). Host: 2 physical cores /
> 15 GB, single worker, sharing off, strictly sequential, both arms in one
> session. Banked as
> [`throughput-probe-12s-summary.tsv`](../../results/throughput-probe-12s-summary.tsv)
> and
> [`throughput-probe-ueq30s-summary.tsv`](../../results/throughput-probe-ueq30s-summary.tsv).
>
> No division score is claimed anywhere in this document. These are
> single-strategy measurements and are not comparable to a CASC result.

## 1. Where the UEQ search was going

The [UEQ report](ueq-2026-09.md) §2 leaves UEQ coverage 150 problems below the
stored `codex.db` run, with median `processed_per_s` down 14x, and names two
candidate causes: cross-strategy sharing turned off, and a portfolio reorder.
Neither is an engine-level explanation, and neither is a *throughput* one —
neither change can make a given-clause iteration slower.

So the question this document answers is the prior one: is the engine itself
fast? The answer on a single-strategy UEQ run, from a callgrind profile of
`--time 6 --workers 1 --strategy 1 --schedule casc_ueq` on
`casc-j13/UEQ/LAT141-1.p` and on `casc-30/UEQ/ALG212-10.p`, was no. The search
was spending its budget in three places, none of which was inference:

| phase, inclusive share of search instructions | what it is |
|---|---|
| **47 %** | forward subsumption — `subsumes_id` over the whole candidate list for every new and given clause |
| **41 %** | forward demodulation — `demodulate_id` walking every term of every literal against the unit-equality index |
| **14 %** | superposition, which is the actual work |

The two redundancy phases together were 88 %. The evidence that this is waste
rather than cost is in the same runs: `ALG212-10` retires 1 617 clauses in 6 s
and forward-subsumes 86 of them, so 22 616 subsumption tests produce 86
successes. 4 % hit rate, 47 % of the clock.

## 2. Six changes

Every change was intended as a no-op on the result. Review found that the original
per-root demodulation memo invalidation could suppress rewriting below a term
root. The corrected global invalidation is covered by a nested-rule regression
test. Throughput numbers in §3 and memo hit rates in §2.5 predate this fix and
must be rerun before they describe the corrected implementation.

### 2.1 Substituted terms were re-interned even when nothing changed

`apply_subst_flat_id`, `apply_subst_chain_id` and `IdSubstitution::apply_term`
each rebuilt an argument list and called `bank.intern_app` at every node of a
term, on the no-op path. The bank is hash-consed, so re-interning an unchanged
argument list returns the id it started from: the vector allocation and the hash
lookup were pure overhead. They now collect only the arguments that actually
change and skip the intern when there are none.

`apply_subst_flat_id` and `apply_subst_chain_id` also gained an early return for
an empty substitution. This is the common case by a wide margin: the matcher
tries one (candidate literal, target literal) pair at a time, and most pairs are
rejected by their first term comparison, before anything is bound.

That one early return is worth a paragraph on its own, because it is the
difference between the fix being 47 % and being 5 %. Before it, the matcher was
calling a full term rebuild *and* a term-bank hash lookup for every candidate
literal it tried against every target literal, and `ALG212-10` retires 1 617
clauses in 6 s.

### 2.2 `max_var` was a `HashSet` fold, and then a walk, and is now a memo

`max_var_id` built a `HashSet` of every free variable just to take the maximum,
once per side of every subsumption test. It is now an allocation-free fold, and
`TermBank` memoises it per interned node — a term is hash-consed, so its
variables never change, and the answer is one lookup per literal after the first
visit. A callgrind profile put the walk at about 8 % of search instructions on
its own.

### 2.3 A ground subsumer is not renamed

`subsumes_id` and `subsumption_resolution_id` standardise the subsumer apart by
adding `max_var_id(target)` to every variable and re-interning every subterm.
When the subsumer is ground that is the identity, and on an equational problem
the candidate set is mostly unit equalities, which are ground. Both now match
the subsumer in place when it has no variables.

### 2.4 `STreeId`'s generalization query scanned every root child

`STreeId` is documented as a drop-in replacement for `DTreeId` with identical
retrieval semantics. It is not: `DTreeId::gen_flat` dispatches on the query's
root cell and does one map lookup, while `STreeId::gen_flat` visited every child
of the node and let `gen_walk_edge` reject the mismatches. The path-compression
rewrite lost the dispatch.

`gen_flat` now dispatches the same way `DTreeId` does: one `BTreeMap` lookup for
the `Sym(f, n)` child, plus the `Var` children, which generalize any query cell.
`get_generalizations` additionally refuses to flatten the query at all when no
root child can match, since `flatten_id` allocates a variable-normalisation map
on every call. A new test compares the dispatch against a full scan of the same
tree over a corpus that mixes root symbols, arities, constants and variables —
every pre-existing generalization test in the file used a single root symbol,
which is why the difference was invisible.

This one is worth much less than it looks: the rejected roots were cheap, so it
moved the probe by about 1 %. It is here because the asymmetry is a defect
regardless, and because the differential test is the thing that would catch a
future regression in it.

### 2.5 Demodulation memoises its negative answers

`rewrite_term_id` is a pure function of a term, the rules in the demodulation
index, and the literal's AVATAR context, and the search asks it the same
question about the same interned term over and over: an argument occurring in one
clause occurs in most of the clauses that mention its symbol. The new
`DemodMemo` remembers terms found irreducible.

Three properties make it sound, and each is a place it could have gone wrong:

- **Only the negative answer is memoised.** A positive answer carries a witness —
  which unit clause was applied, at which term path — and `mrs-proof` replays that
  witness to justify the step to a checker. A synthesised substitute would be a
  step that did not happen, the same failure `mrs-search`'s `fvo` module
  documents.
- **Invalidation is global.** A negative result covers the whole term tree: a rule
  for a nested subterm can make an enclosing term reducible even when its root is
  unrelated. Every index mutation advances one generation in O(1), invalidating
  all previous entries without scanning the memo.
- **Scope is explicit.** `demodulate_id` takes the memo as an `Option`: backward
  demodulation rewrites against a temporary index and passes `None`, and a clause
  with a non-empty AVATAR context passes `None` because the memo key does not
  carry the context. Every insert into and removal from `demod_index` calls
  `invalidate`.

The hit rate is in the `% SZS detail` line as `demod_memo_lookups`,
`demod_memo_hits` and `demod_memo_evictions`, because a memo that is invalidated
as fast as it is filled looks exactly like one that is not working. Measured on
`casc-30/UEQ`, single strategy, 12 s: 81 % on `ALG212-10`, 47 % on `GRP678-1`,
35 % on `GRP420-1`.

### 2.6 The subsumption-resolution restore, and how it was caught

While making the SR candidate scan cheaper I also made its polarity flip
in-place: `subsumption_resolution_id` used to build a fresh `modified_target` for
each of the target's literals, and now flips one index, matches, and moves on.
Positions are tried in order, so the flip has to be undone — otherwise position
`i` is matched against a target whose literals `0..i` are all reversed.

The result is still a consequence of its two parents, so the refutation is sound
and the prover's own reasoning replays it. It is not the inference the step
claims, and every such step is rejected by `mrs-proover --strict` with
`conclusion is not the target with a justified literal removed`.

That is how it was found, and it is worth being precise about the sequence
because the intermediate measurement was the best result this work produced and
it was false. On the 45-problem probe with the bug in place, **10 of 15 FNE
problems went from `Timeout` to `Theorem`** and FNE `generated` fell to 0.28x —
an apparent collapse in cost, because the spurious steps were simplifying clauses
the rule had no licence to simplify, which is a very effective way to make a
search look fast. Restoring the flip returns all ten to `Timeout`. The pinned
test (`subsumption_resolution_id_reports_the_position_whose_flip_matched`)
fails without the restore and is shaped so that it cannot pass vacuously.

The lesson generalises past this bug: on a search engine, a throughput result
that comes with a step-count *drop* deserves the proof checker before the write-up,
not after.

## 3. What it measures

Both arms are the same corpus, the same fixed wall clock per problem, one
worker, one strategy, sharing off, sequential, on one host in one session. The
unit is clauses retired out of a fixed budget, not clauses per second and not
elapsed time, for the reasons in `docs/results/perf/README.md`: LRS prunes
against the clock, so a run's clause count depends on how fast the host is.

### 3.1 Throughput, 45 problems, 12 s each

| division | metric | base | fixed | ratio |
|---|---|---:|---:|---:|
| **UEQ** (30) | `processed` | 13 222 | **16 386** | **1.24x** |
| **UEQ** (30) | `generated` | 521 233 | **704 523** | **1.35x** |
| FNE (15) | `processed` | 86 394 | 88 086 | 1.02x |
| FNE (15) | `generated` | 569 239 | 624 826 | 1.10x |

Per problem, 28 of the 30 UEQ rows improve, median 1.21x, range 0.94x to 1.90x.
`REL031-1` at 0.94x is the one regression and is inside the run-to-run spread of
a single 12 s run. **No row changes SZS status in either direction.**

FNE moves much less, and the profile says why: the demodulation index is empty on
a non-equational problem, so 2.5 has nothing to memoise, and the subsumption
improvements in 2.1–2.3 are a smaller share of a search whose redundancy phase
is already cheap relative to its superposition phase.

### 3.2 A 60-problem UEQ check, 30 s each

The 60 rows are a fixed stride over the 222 `casc-30` UEQ problems
`codex.db` records as solved, taken from the stored run rather than from a list
chosen after seeing the result.

| | base | fixed |
|---|---:|---:|
| refuted | 10 | **11** |
| gained | — | `GRP422-1`, `GRP660-10` |
| lost | — | `BOO017-10` (§4) |
| total wall on the 9 commonly-solved rows | 130.9 s | **84.7 s** |
| `processed` on the 50 timing-out rows | 33 418 | **42 335 (1.27x)** |

Both gains are real at the engine level and not budget luck: `GRP422-1` goes from
96 clauses in 30 s to a refutation at 120 clauses in 25 s, and `GRP660-10` from
497 clauses and no refutation to one at 588 clauses in 21 s.

Read the refuted column as a regression check, not as a score. At 30 s with one
strategy only 10 of these 60 are refuted in either arm, so the column is mostly
measuring the 50 rows that time out in both, and the throughput column is what
carries the information.

## 4. The one loss, and why it is reported rather than dropped

`BOO017-10` is `Unsatisfiable` at 30 s on the base arm and `Timeout` at 30 s on
the fixed arm. Three repeats of each reproduce it. It is also `Timeout` at 60 s
on base and `Unsatisfiable` at 60 s on fixed:

| | 30 s | 30 s | 30 s | 60 s |
|---|---|---|---|---|
| base | `Unsatisfiable` | `Unsatisfiable` | `Unsatisfiable` | **`Timeout`** |
| fixed | `Timeout` | `Timeout` | `Timeout` | **`Unsatisfiable`** |

Neither arm is monotone in its own budget, which a search should be. A refutation
found at 30 s and lost at 60 s means something in the run is decided by the clock
rather than by the clause set — LRS pruning against elapsed time is the obvious
candidate, and the UEQ report's `ALG032+1` note and the `MRS_LRS_FIXED_ITERATIONS`
escape hatch exist because of exactly this class of behaviour. On this arm the
row also does strictly more clause churn (`generated` 26 291 → 56 379,
`fwd_subsumed` 2 833 → 2 297): it retires clauses faster and the extra churn does
not reach the proof inside 30 s.

So: one reproducible loss, out of 60, on a row that is budget-pathological in both
arms, against two gains and a 35 % cut in wall time on the rows that are solved
in both. It is the sort of row that a solved count alone would hide, which is
the reason to write it down.

## 5. Soundness

Every change in §2 is a no-op on the result, with the one exception in §2.6,
which was a bug and is reverted. The gates, all clean at the branch tip:

```
cargo check --workspace --all-targets
cargo clippy --all -- -D warnings
cargo fmt --all --check
cargo test --workspace          # 65 binaries, 1286 tests, 0 failures
```

and, on the proofs, `mrs-proover --strict --time 600` over 12 `casc-30` UEQ
refutations produced by this branch — the 11 that this branch refutes within the
probe budget plus `BOO017-10` at a longer budget:

```
BOO017-10  COL003-19  COL042-6   CSR034-10  CSR051-10  GRP416-1
GRP422-1   GRP478-1   GRP660-10  GRP750-1   LAT394-1   REL010-2
```

**12 of 12 `VerifiedGood`, 0 `VerifiedBad`, 0 `Unknown`.** The new tests are the
differential index comparison (§2.4), the three memo-semantics tests and the SR
restore test (§2.6), each written to fail without the change it pins.

## 6. What is not claimed

- **No division score.** Every number here is one strategy, one worker, on a
  2-core host. The 11-vs-10 in §3.2 is a regression check on a 60-row subset, not
  a UEQ score, and the [UEQ report](ueq-2026-09.md)'s 107/400 and the stored
  `codex.db` 257/400 are both 8-worker measurements on 16-core hosts.
- **Nothing here is CASC-shaped.** AGENTS.md §11's decision rule applies: a
  throughput change on this host says nothing about a CASC entry's ranking, and
  the FNE report's ±4-problem host band is a reminder that a solved-count
  difference of that size on one host is not evidence of anything.
- **The FNE/LCL cluster is untouched.** The condensed-detachment cluster is 31 of
  the 61 FNE failures and is a search-power problem, not a throughput one
  ([FNE report](fne-2026-09.md) §2). FNE moves 1.02x here and that is what
  should be expected.
- **The sharing question is untouched.** The UEQ report's §2 four-cell A/B is
  still the open question about the 150-problem regression, and this work does
  not speak to it: `MRS_SHARED_POOL_INTERVAL=0` throughout.

## 7. What I would do next

1. **Re-measure UEQ properly.** 2.1–2.3 and 2.5 are worth a 1.24x on single-
   strategy UEQ throughput. Whether that converts into division coverage is a
   four-cell cooperative A/B on a host that can afford `--workers 8`, and it
   should run *after* the sharing A/B so the two are not confounded.
2. **Profile the FNE division on the same footing.** FNE moved 1.02x, and the
   profile for that shape is a different profile: the 14 % superposition share
   says the redundancy phases are not what bounds FNE.
3. **`BOO017-10`.** One row, but it is a probe for something real — a search
   whose result is not monotone in its own budget. `MRS_LRS_FIXED_ITERATIONS`
   makes the run deterministic; running the fixed arm with it would say whether
   the LRS wall-clock prune is what makes the row budget-pathological.
4. **Bank a regression gate for the profile.** The `demod_memo_*` counters are
   the shape of thing that belongs in `run.csv`, and this work had to reach for
   callgrind because nothing recorded where the time went. A coarse phase
   breakdown in the detail line would make the next "47 % of the search" findable
   without a profiler.

## Reproducing

```bash
# throughput, 45 problems, 12 s each. Two arms, sequential, one session.
cargo build --release --bin mrs
cp target/release/mrs /tmp/mrs-fixed
git stash && cargo build --release --bin mrs && cp target/release/mrs /tmp/mrs-base
git stash pop
crates/mrs-bench/throughput_probe.sh /tmp/mrs-base  base  /tmp/base.csv  12
crates/mrs-bench/throughput_probe.sh /tmp/mrs-fixed fixed /tmp/fixed.csv 12

# the 60-row UEQ check, 30 s each
grep -v '^#' /tmp/opencode/ueq-solved.txt >/dev/null   # see §3.2 for how it is built
crates/mrs-bench/throughput_probe.sh /tmp/mrs-base  base  /tmp/ueq-base.csv  30 ueq-solved.txt
crates/mrs-bench/throughput_probe.sh /tmp/mrs-fixed fixed /tmp/ueq-fixed.csv 30 ueq-solved.txt

# the profile
CARGO_TARGET_DIR=/tmp/prof RUSTFLAGS="-C target-cpu=native -C debuginfo=1 -C strip=none" \
  cargo build --release --bin mrs
valgrind --tool=callgrind --callgrind-out-file=/tmp/cg.out --cache-sim=no \
  /tmp/prof/release/mrs --time 6 --workers 1 --strategy 1 --schedule casc_ueq \
  crates/mrs-bench/problems/casc-30/UEQ/ALG212-10.p
callgrind_annotate --inclusive=yes --threshold=80 /tmp/cg.out

# the strict proof gate
TPTP=crates/mrs-bench/problems/casc-30 \
  ./target/release/mrs-proover --strict --time 600 /tmp/proofs/GRP422-1.s
```
