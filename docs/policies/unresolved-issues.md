# Unresolved issues

Known defects and open questions that are **not** being worked on right now,
recorded so they are not rediscovered from scratch later.

This page exists because that has already happened repeatedly. The FEQ certification
figures were once read as coverage when 78 of 144 refutations had never been
checked. The largest proof in the archive was recorded as "never produced a
verdict at all" when the real cause was a kernel ceiling a five-minute check
clears. And a campaign-localisation theory was written down with enough
confidence to name a fix that the next dataset disproved — see UI-1 below, which
is the worked example of why this page records *evidence*, not conclusions.

## How to use this page

Each entry states what is observed, what is **ruled out**, and what is still
unknown. Entries deliberately do not name a fix unless the evidence points at
one. When you pick an entry up:

1. Reproduce it before changing anything (each entry gives the command).
2. Prefer an invariant assertion over a trace. A trace scales with search
   volume and produced 668 293 lines / 271 MB on a 2-core box; an assertion
   scales with the number of violations and fires once.
3. Update this page in the same commit that changes behaviour, including when
   the answer is "it was something else".

Related: [work that needs the remote host](../guides/remote-only-work.md) for
what is blocked on hardware rather than on knowledge.

---

## UI-1 — `ac_superposition` conclusions that are not a superposition of their cited parents

| | |
|---|---|
| Status | Open, not scheduled |
| Severity | The examined examples lose `VerifiedGood`; no `VerifiedBad` |
| Soundness | No `VerifiedBad` was recorded. An under-cited inference node alone does not establish that the final refutation is sound. |
| Blocks | None identified; evidence does not establish a hardware blocker. |

### Observation

The strict kernel's archived campaign audit refused a small number of
`ac_superposition` nodes:

```
% SZS status Unknown : node c431338: ac_superposition replay is incomplete:
  superposition conclusion is not a valid rewrite
```

One problem in casc-30 UEQ (`KLE145-10`, node `c431338`) and two in casc-j13 UEQ
(`LAT044-1` node `c659171`, `LAT241-10` node `c53874`) appeared in the examined
proofs. The campaign counts were not independently re-audited to establish a
cross-edition total, so this register does not claim a current 4/700 rate.

Node ids are **not stable across runs of the same problem** — the same
`KLE145-10` node was observed as `c431338`, `c411616`, `c426189`, `c440158`,
`c441748` and `c449061` in successive local runs, because the portfolio is
wall-clock sensitive and clause ids follow generation order. Quote the
conclusion text when identifying a node, not the id.

The kernel's `AcReplay::NotFound` (as opposed to `BudgetExhausted`) proves the
replay exhausted every superposition position and orientation and genuinely
found no match. `max_equivalence_steps` was not hit. The diagnostic that
distinguishes these was added in commit `1edb13b` on branch
`fix/ac-superposition-cite-folded-demodulators`; before it, both situations
collapsed into one `Unknown` and this was undiagnosable from an audit report.

### Evidence: the two shapes differ

**casc-30 `KLE145-10`, node `c431338`** (archived campaign run; quoted ids are
from `certification/proofs/mrs/ueq/KLE145-10.s`):

```
cnf(c431338, plain, true = iteq(true, true,
      leq(addition(one, X14), multiplication(strong_iteration(X14), one)), true),
    inference(ac_superposition, [status(thm)], [c425221, c2428, c3, c21])).

cnf(c432531, plain, true = leq(addition(one, X14), strong_iteration(X14)),
    inference(demodulation, [...], [c431338, c7, c62])).
```

The conclusion is the superposition plus two rewrites by *uncited premises*,
both in the expanding direction:

| clause | axiom | contracting | expanding (the direction used) |
|---|---|---|---|
| `c7` | `ifeq_axiom_002` | `iteq(X1,X1,X0,X2) ↦ X0` | `X0 ↦ iteq(X1,X1,X0,X2)` |
| `c62` | `multiplicative_right_identity` | `X*one ↦ X` | `X ↦ X*one` |

`c7` is the axiom verbatim. `c62` is `ac_normalization` of `c11`
(`multiplication(X0,one) = X0`) with the sides swapped, so the contracting
orientation exists in the same problem and the expander chose the other one.
The next node demodulates both straight back off using those same two clauses.

Dumping the replay's own candidate shows the difference is **not** AC
permutation — the goal has `multiplication` where the candidate has `addition`.
(Candidate obtained by instrumenting `ac_superposition_replay` locally; the
archived proof alone does not contain it.)

**casc-j13 `LAT044-1`, node `c659171`** (archived campaign run):

```
cnf(c659171, plain,
  meet(goal_d4, complement(join(X7, goal_d5))) =
  meet(goal_d4, complement(join(X7, meet(goal_d4, meet(complement(X7), n1))))),
  inference(ac_superposition, [status(thm)], [c197935, c24789, c8, c9, c11, c14])).

cnf(c673128, plain, meet(goal_d4, complement(join(X7, goal_d5))) = n0,
    inference(demodulation, [...], [c659171, c1542, c21269, c2095])).
```

No `ifeq`, no expanding problem axiom; the extra parents are four AC axioms. The
conclusion merely duplicates a `meet` subtree the parents do not contain.

### A retracted theory, kept because it is instructive

The first theory was that the forward demodulator takes the *expanding*
orientation of a rule whose contracting orientation also exists, and that the
fix is to stop it doing so at `crates/mrs-calculus/src/demodulation.rs:615`.

**`LAT044-1` disproves it.** That node has no `iteq` and no expanding problem
axiom, so an orientation change would not have fixed it. The theory also failed
to explain why the `demodulation` node appears *immediately downstream in both
cases*, and — in `LAT044-1` — why a `demodulation` node is one of the
*parents*. That thread is the one worth pulling.

### Ruled out by reading and by instrumentation

| Candidate | Why not it |
|---|---|
| `mrs-calculus/src/superposition.rs` | no demodulation, simplification or folding anywhere in it |
| forward demodulation (`given_clause.rs:2874`, `:1796`) | `demodulate_id_until` already returns a correctly attributed `demodulation` node (`demodulation.rs:452-466`) |
| `state.rs:616` `ac_normalize_for_search` | `ac_normalize_clause` preserves `clause.id` and `clause.source` |
| `term_bank.rs:467` `ac_normalize` | pure flatten/sort/rebuild; cannot introduce `multiplication` or an `iteq` shell |
| condensation / DER / forward SR | each sets its own rule name when it fires |
| `restrict_to_maximal_id` | returns literal indices only |

### Leading suspicion, not yet confirmed

Superposition takes its partners from `LiteralIndex` (`given_clause.rs:1903`),
which returns **clones held in the index** (`literal_index.rs:224`), while
`given` comes from `clause_store` (`given_clause.rs:1581`). Two independent
copies of the same clause id. Meanwhile `SearchState::store_clause`
(`state.rs:333`) writes `clause_store` with an **unconditional** `insert` —
last write wins — while `register_clause` (`state.rs:541`) uses
`or_insert_with` and is first-write-wins, and `push_unprocessed`
(`state.rs:371`) routes through `store_clause`. A partner whose index copy and
store copy disagree would produce exactly this shape. **Unconfirmed**: no
evidence yet distinguishes the two copies as the culprit.

### Next step

The search now has a debug-only invariant assertion on both indexed-partner
superposition paths (`given_clause.rs`, the given-as-equation-source and
given-as-target-source loops). Bounded local attempts on this 2-core dev box
used `--workers 2`; KLE145-10 timed out after 60.1 s and LAT044-1 after 45.1 s,
both at 1.36 GB peak RSS, without reaching a mismatching partner. These are
inconclusive, not reproductions.

> For every clause emitted with rule `superposition`/`ac_superposition`, the
> literals used were identical in `clause_store` and in `LiteralIndex` at the
> moment of inference.

It fires only on divergence and prints the clause id and both sources. A unit
test inserts a deliberately stale processed-index copy and confirms the
assertion detects it. The assertion and focused test run locally; they do not
require the 8-worker reproduction.

```bash
# Focused reproduction, respecting this dev box's 2 physical cores and RAM.
# Previously measured: timed out after 60 s (1.36 GB peak) before the mismatch.
nix develop -c cargo build --bin mrs
./target/debug/mrs --time 200 --workers 2 --schedule casc_ueq \
    crates/mrs-bench/problems/casc-30/UEQ/KLE145-10.p

# If repeating locally, cap at --time 60 and --workers 2; this is a diagnostic,
# not a CASC coverage measurement. Do not raise the worker count on this host.
```

**Remote-only reproduction, only if the local run fails to trigger the
assertion:** use the campaign host with 8 physical cores and at least 12 GiB
available RAM; no trace is needed. Run the same debug build and problem command
with `--workers 8`, capped at 200 s. Record the `rustc` version, git revision,
available RAM and whether the assertion fired. Do not run a full CASC sweep for
this diagnosis. The earlier 8-worker solve was measured at 8.9 GB peak, so a
host with less headroom is not a suitable repro host.

### Two traps that cost real time here

- **`/tmp` is tmpfs.** An unfiltered trace is a RAM-availability problem, not a
  disk one. One unconditional `store_clause` probe emitted 668 293 lines /
  271 MB and had to be killed mid-run.
- **`IdAtom`'s `Debug` prints `SymbolId(n)`, not the function name.** Three
  grep-based probes silently matched *nothing* and their nulls were briefly read
  as findings. Any name-based probe must resolve through
  `SymbolTable::iter_names()` *before* matching.

---

## UI-2 — Problems that hang before emitting any SZS output (27 across UEQ and FEQ)

| | |
|---|---|
| Status | Open, not scheduled |
| Severity | 15 of 300 casc-30 UEQ (5%), **and 12 of 400 casc-30 FEQ (3%)** |
| Soundness | Not a soundness issue. No refutation is lost — these are timeouts either way. |

### Observation

Fifteen casc-30 UEQ problems hit the harness's outer SIGTERM with **empty
stdout and no `% SZS detail`**, recorded as bare `Timeout`:

`GRP024-5`, `GRP508-1`, `GRP654-10`, `GRP654-12`, `GRP655-13`, `GRP655-14`,
`GRP662-1`, `GRP667-10`, `GRP670-1`, `GRP681-1`, `GRP691-1`, `GRP695-1`,
`KLE110-10`, `LAT138-1`, `LAT168-1`.

All report `wall_time_s=240.000` exactly, which is the signature of
`casc.sh:690-692` capping the recorded time after `timeout 250` fired
(exit 124). The other 152 timeouts self-report at ~238.3 s, i.e. mrs's own
238 s deadline (`invoke.sh:49`, `SOFT_TIME`) fired and it printed a status line.

So on these 15 the prover overran its own deadline by more than 12 s inside a
non-preemptible section. `invoke.sh:45-48` documents this exact failure mode as
the thing the 2 s soft margin exists to avoid, so it is a live regression.

All 15 are AC/group-theory word problems, several with tiny inputs
(`GRP024-5` is 6 CNF clauses), which points at one given-clause iteration not
returning rather than at general slowness — the deadline is observed at
iteration boundaries (the LRS check at `given_clause.rs:1461` runs every 100
iterations).

### It is two distinct defects that look identical in the CSV

casc-30 **FEQ** shows the same signature 12 more times
(`ALG215+2`, `BIO005+1`, `BIO006+1`, `CSR037+5`, `CSR047+5`, `CSR052+4`,
`HWV090+1`, `HWV128+1`, `ITP015+4`, `NUM925+3`, `NUM925+7`, `SWX070+1`), but it
hangs in a **different phase**.

`% Problem:` is printed at `src/main.rs:664`, *after* lowering but *before*
clausification, which runs at `src/main.rs:705` and `:737`; the search starts at
`src/main.rs:1094`. So:

| division | last line reached | hangs in |
|---|---|---|
| UEQ | `% Problem: … 6 cnf clauses` | **search** (clausification finished; input was already CNF) |
| FEQ | `% Problem: … 0 cnf clauses` | **clausification** (input was FOF; never reaches search) |

Every one of the 12 FEQ cases reports a large FOF input — `CSR037+5` has
**540 249** axioms, `HWV128+1` 204 845, `CSR052+4` 44 216, down to `SWX070+1`
at 148 axioms — and all report `0 cnf clauses`, i.e. clausification had produced
nothing when the clock ran out.

**Clausification has no cancellation support at all.** `main.rs:705` and `:737`
pass `None` as the deadline argument to `mrs_cnf::clausify_with_provenance`, and
`crates/mrs-cnf/` contains no reference to `deadline`, `cancel` or `Instant` at
all. There is no point at which an oversized input can be interrupted, so a
148-axiom problem that takes over 240 s cannot be stopped.

This matters beyond the lost rows: a hung clausification also means the run
cannot report *which* stage it reached, so the CSV cannot distinguish "hard
problem" from "stuck before the search began".

### Secondary defect, independent of the above

The CSV cannot distinguish "searched hard and timed out" from "hung and was
killed". Both are recorded as `Timeout` with no marker. A harness change should
make exit-124-without-SZS a distinct status so the two classes are separable in
the results.

### Also affects the memory column

For those 15 rows `peak_memory_mb` comes from GNU `time` Max RSS
(`casc.sh:750-757`); the other 285 are mrs's self-reported VmPeak. The column
is **not homogeneous** and must not be charted as one series.

---

## UI-3 — `lrs_discarded` can exceed `generated`

| | |
|---|---|
| Status | Benign, documented, no fix planned |
| Severity | Diagnostic only |

Input clauses enter the passive queue without incrementing `generated`; only
derived clauses do (`given_clause.rs:3191`). On input-heavy problems
`lrs_discarded` therefore legitimately exceeds `generated`: `CSR065-10` reports
80177 against 50432, `CSR073-10` 71668 against 25390, and both have 44220 input
clauses.

Not a soundness or accounting bug. But it makes a `discarded/generated` ratio
meaningless on input-heavy problems, and `perf_probe` and
`dual_run_sanity_check` build thresholds from exactly that ratio. Anyone reading
those thresholds should exclude problems whose input clause count approaches
their generated count.
## UI-4 — `alpha_equiv` rejects re-associated conjunctions, costing `VerifiedGood`

| | |
|---|---|
| Status | Open, not scheduled. **Fixing it widens what the kernel accepts.** |
| Severity | 16 of 63 certifiable casc-j13 FEQ refutations reported `VerifiedBad` |
| Soundness | Fails **closed** — valid proofs are rejected, none are wrongly accepted. Not a soundness risk. |
| Blocks | The casc-j13 FEQ certification number is unusable until resolved. |

### Observation

`campaign-cascj13-feq-W8C8J1-20261002` is the first run anywhere to report
`VerifiedBad`: **16 rows**, every one of the form

```
leaf `c237` does not match problem formula `axiom_53`
```

All 16 problems use `%include`, and the cited axiom is not in the top-level
`.p` file. Reproduced locally against **either** edition's axiom files, so it is
not the corpus-mismatch bug described in the remote-only guide.

The cited proof leaf and the source axiom are the **same formula**, differing
only in how the conjunction is grouped. Proof leaf `c237` of `SWV453+1`:

```
![X0]: ![X1]: ( ordered(cons(X0,X1)) <=>
  ( ordered(X1) & ![X2]: ( ... => leq(pidMsg(X0), pidMsg(X2)) ) ) )
```

`axiom_53` of casc-j13 `Axioms/SWV011+0.ax`:

```
![X,Q]: ( ( ordered(Q) & ![Y]: ( ... <= ... ) ) <=> ordered(cons(X,Q)) )
```

Same conjuncts, mirrored biconditional, different binary grouping of `&`.

### Root cause, isolated

`mrs_core::alpha::alpha_equiv` compares `And`/`Or` as a **binary tree**, with
greedy backtrack-free multiset matching at each node
(`crates/mrs-core/src/alpha.rs:37-54`). It never flattens nested
conjunctions. The TPTP parser builds `a & b & c` as `And([And([a, b]), c])`, so
the tree shape is fixed by the operand order and any reordering of three or more
conjuncts is rejected.

Measured with the ignored regression tests in
`crates/mrs-proof-kernel/src/lib.rs` (`alpha_equiv_regression`):

| case | `alpha_equiv` |
|---|---|
| `p & q` vs `q & p` (2 atoms) | `true` |
| `p & q & r` vs `r & q & p` (3 atoms, reordered) | **`false`** |
| `(p & q) & r` vs `p & (q & r)` (re-associated) | **`false`** |
| `(p & q) & r` vs `(p & q) & r` (identical grouping) | `true` |
| `<=>` mirrored, atomic operands | `true` |
| `![X,Y]: F` vs `![X]: ![Y]: F` | `true` |
| `<=` vs `=>` | `true` |

So the defect is specifically **conjunction/ disjunction reordering or
re-association of three or more operands**. Biconditional mirroring, quantifier
splitting, and reverse implication are all handled correctly.

Note the module contract is *not* being violated on free variables:
`crates/mrs-core/src/alpha.rs:3-4` states "Free variables must have the same
identifiers", and `term_eq` implements exactly that.

### Blast radius

`alpha_equiv` is the leaf comparison used throughout verification:

- `crates/mrs-proof-kernel/src/lib.rs:2610`, `:2627` — input-leaf checking
- `crates/mrs-proover/src/checks/axiom_leaf.rs:85`, `:171`, `:201`
- `crates/mrs-proover/src/checks/definition_folding.rs:84`, `:231`
- `crates/mrs-proover/src/checks/trivial.rs`, `crates/mrs-proover/src/verify.rs`

Every one of those is potentially over-strict in the same way. The 16 rows are
what this corpus happened to expose; casc-30 FEQ has **61 of 90** certified
refutations citing a formula absent from their own top-level `.p` and reports
`VerifiedBad = 0`, so the corpus's own serialisation decides whether the bug
fires. That is why it went unnoticed.

### Why it is not being fixed here

Flattening `And`/`Or` to n-ary multisets before comparison would accept strictly
more proofs. `docs/policies/methodology.md` §1.4 requires that semantic claims
be independently checked rather than taken from the implementation's own
comments, and §1.6 requires failed or inconclusive validation to stay visible.
Widening a strict kernel is a soundness-sensitive change and wants a deliberate
review, not a drive-by fix alongside a docs commit.

The change itself is small — normalise to n-ary before the existing multiset
match — but the review should confirm that n-ary flattening is exactly the
intended notion of formula equality here, and that no inference rule *depends*
on the binary grouping being distinguished.

### Reproduce

```bash
cargo test -p mrs-proof-kernel alpha_equiv_regression -- --ignored --nocapture

# End to end, from the archived proof:
TPTP=crates/mrs-bench/problems/casc-j13 \
  target/release/mrs-proover --problems-dir <cert>/proofs/mrs/feq \
  --workers 1 --time 180 --strict SWV453+1.s
```

## UI-5 — EPU: the equality InstGen route refuses 43% of the division, and the pre-pass earns nothing

| | |
|---|---|
| Status | Open, not scheduled |
| Severity | EPU scores **9/100 (9.0%)**, the weakest of the five entered divisions |
| Soundness | Not a soundness issue. No refutation is lost. |

### Observation

`campaign-casc30-epu-W8C8J1-20261002` is a well-formed run: one `Total jobs:`,
100/100 completed, `expected` populated for all 100 rows, and **9 `Unsatisfiable`
all 9 `VerifiedGood`, 0 `Unknown`, 0 `VerifiedBad`**. It is also *reproducible*:
`campaign-casc30-epu-W8C8J1-20260930` ran a different binary
(`08fa4f1ffdf5` vs `fa52c23d73b3`) from a different commit (`c07cac9d` vs
`fb55719c`) on a different host, and solved **exactly the same 9 problems**.
Only 5 rows differ, all in the Timeout↔GaveUp band. So 9/100 is a real number,
not noise.

The division is dominated by very large inputs: median 828 clauses but a maximum
of **696 691** (`HWV092-1`), 2 847 336 clauses in total. None use `%include`.

### The pre-pass never once succeeds, and refuses 43 of 100 problems outright

From the `% SZS detail` telemetry:

| `instgen_fallback` | problems |
|---|---|
| `unsupported_epr_profile` | **43** |
| `timeout` | 33 |
| `max_instances_exceeded` | 15 |
| `max_rounds_reached` | 3 |
| (no telemetry) | 6 |

`instgen_result` is only ever `fallback` (51) or `none` (43). **It is never a
refutation.** All 9 solves came from the given-clause fallback:

| solve | route | `instgen_result` |
|---|---|---|
| `LAT260-2`, `LAT261-2`, `LAT264-2`, `LAT265-2`, `SET856-2`, `PUZ008-2` | *(no pre-pass)* | — |
| `PUZ036-1.005`, `SYN837-1` | `pure_relational_epr` | `fallback` (timeout) |
| `HWV107-1` | `epr_equality` | `none` (refused outright) |

`instgen_ms` totals **32.6 s across the whole division** (mean 346 ms, budget
750 ms per problem) — about 0.3% of the division's wall clock. So the pre-pass
costs little, but it also contributes nothing here, and it is not the reason
EPU scores 9%.

`crates/mrs-search/src/epr_ground.rs:36-41` already records the
`unsupported_epr_profile` refusal rate for this corpus, so the shape is known.

### A fourth exit path: the portfolio gives up with budget unspent

Four problems end `GaveUp` several seconds **before** their wall clock, which no
timeout does:

| problem | `elapsed_ms` | wall | gap |
|---|---|---|---|
| `HWV092-1` | 10 817 | 19.6 s | **8.8 s** |
| `HWV094-1` | 16 982 | 21.4 s | 4.4 s |
| `HWV126-1` | 116 643 | 125.1 s | 8.4 s |
| `HWV127-1` | 116 711 | 124.8 s | 8.0 s |

All four report `timeout=0` (no worker hit its slice) and `passive=0`. Every
other `GaveUp` in the division lands within 0.3 s of the wall. With
`lrs_discarded > 0` and an empty queue, `given_clause.rs:3212` returns
`GaveUp` — the incompleteness guard — so all 30 `GaveUp` rows are that guard
firing, and these four fire it with time still on the clock.

### Where the headroom is

The 61 timeouts and 30 `GaveUp` rows are not evenly distributed. EPU's 9 solves
are 6 sub-0.1 s trivialities plus 3 real proofs. The realistic lever is the
large-input tail (`HWV*` at 155 k–697 k clauses), where a 120 s budget is
dominated by clause loading rather than by search — `HWV092-1` retires 63 662
clauses in 10.8 s and then stops. Before spending effort on the EPR profile,
the question worth answering is whether any of the large-`HWV` problems are
decidable in the time it takes to *load* them.

## UI-6 — ICU: 44% of the division is unscoreable, and `verdict = ok` does not mean certified

| | |
|---|---|
| Status | Open, not scheduled |
| Severity | ICU scores **3/101**; the reference itself gives up on 44 of them |
| Soundness | No unsound result. But the CSV invites one: see "verdict ≠ certified". |

### Observation

`campaign-casc30-icu-W8C8J1-20261002` is a well-formed run (one `Total jobs:`,
101/101, all rows graded) and is the first run to exercise a **480 s** budget,
twice every other division. It is also the only run besides casc-j13 FEQ to
produce an `Error`:

| | count |
|---|---|
| `szs_status` | `Timeout` 83, `GaveUp` 12, `Theorem` 5, **`Error` 1** |
| strict kernel | `VerifiedGood` 3, `Unknown` 2, `Error` 1 |
| `verdict` | `ok` 3, `unknown` 98 |

### 44 of 101 problems cannot be scored at all

`expected` is populated for every row and matches the reference file exactly
(verified against `systems/reference/answers.tsv`: 101/101 present, 0
mismatches). Its distribution:

| `expected` | rows |
|---|---|
| `Theorem` | 55 |
| **`GaveUp`** | **44** |
| `Satisfiable` | 1 |
| `CounterSatisfiable` | 1 |

`casc.sh:810-811` maps a non-committal reference to `verdict = unknown`, so
**44% of ICU is unscoreable by construction** and the attainable ceiling is 57
problems. Against that, 3 is **5.3%**, not 3.0%. Quote the denominator.

The converse also happens: `CSE007+1` and `EEE009+1` are refuted by `mrs` where
the reference gives up. Both are graded `unknown` by design — and `EEE009+1` is
`VerifiedGood`, so one is a genuine certification that the CSV scores as a miss.

### `verdict = ok` does not mean the proof was checked

`casc.sh:798-816` computes `verdict` **solely** by comparing `szs_class(szs)`
with `szs_class(expected)`. It never consults the audit. So:

| problem | `verdict` | strict kernel |
|---|---|---|
| `EEE001+1` | **`ok`** | **`Unknown`** |

`EEE001+1` counts toward the solve count while its proof did not certify. For
UEQ and FEQ the two happened to coincide; here they diverge. **Read the
`certification/audit.csv` `strict_status`, not `run.csv` `verdict`, whenever the
question is "how much is certified".**

### Two of five refutations lost to kernel formula-size ceilings

| problem | `strict_detail` | problem size |
|---|---|---|
| `EEE001+1` | `node c71: negated_conjecture exceeds strict formula-size limit` | **4 axioms**, one 16.7 KB conjecture |
| `CSE007+1` | `node c18798: NNF rule exceeded strict formula-size limit` | — |

Both are `Inconclusive` from `max_formula_nodes` (default `100_000`), not prover
failures: `EEE001+1` refutes in **269 ms**. `to_nnf` distributes nested
biconditionals, so a right-nested `<=>` chain of depth *n* expands toward 2ⁿ
nodes and can cross the ceiling from a modest input. This is the same class as
the FEQ "kernel ceiling a five-minute check clears" story in the remote-only
guide, and it is the only reason ICU's certification rate is 3/5 rather than 5/5.

### One OS OOM kill, and `adaptive` mode has no hard memory ceiling

`CSI008+1` was killed by the OS OOM killer at **94 161 MB**, and its
`% Hardware:` line reports `mem_budget_mb=71642`. So it exceeded the budget it
announced by 31% and lost its status line (`stdout` is 0 bytes — the same
signature as UI-2, though the harness does distinguish this one as `Error`).

The reason is structural: `src/main.rs:603` installs an `RLIMIT_AS` **only**
under `HardwareMode::CascSim`. Under `adaptive` — the default mode — the memory
budget is advisory, consulted by the LRS pruning heuristic
(`given_clause.rs:1472-1481`) but enforced by nothing. A runaway allocation is
stopped by the kernel, not by the prover.

`CSI008+1` alone holds **2 004 866 clauses**, 73% of the division's total
2 750 576. It is the only ICU row above 60 GB; the next largest is 37 913 MB. No
solve was lost — a 2 M-clause input was not going to be refuted in 480 s — but
any future problem that trips this loses its verdict rather than reporting a
resource ceiling.
