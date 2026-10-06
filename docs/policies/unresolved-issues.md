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

## UI-2 — Problems that hang before emitting any SZS status (27 across UEQ and FEQ)

| | |
|---|---|
| Status | Open, not scheduled |
| Severity | 15 of 300 casc-30 UEQ (5%), **and 12 of 400 casc-30 FEQ (3%)** |
| Soundness | Not a false-positive issue. The timeouts may still cost coverage. |

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

### The same CSV symptom may occur in different stages

casc-30 **FEQ** shows the same signature 12 more times
(`ALG215+2`, `BIO005+1`, `BIO006+1`, `CSR037+5`, `CSR047+5`, `CSR052+4`,
`HWV090+1`, `HWV128+1`, `ITP015+4`, `NUM925+3`, `NUM925+7`, `SWX070+1`), but it
hangs in a **different phase**.

`% Problem:` is printed at `src/main.rs:664`, after include resolution and
lowering but before clausification, which runs at `src/main.rs:705` and `:737`;
the search starts at `src/main.rs:1094`. Crucially, the `cnf clauses` count is
only `lowered.cnf_clauses.len()` — clauses supplied directly in the problem —
and does **not** count clauses generated from FOF during the following
clausification loop. Therefore `0 cnf clauses` does not show how much
clausification had completed when the process was killed.

The available log line and source flow suggest a phase distinction, but the
absence of stage-level progress telemetry means the stage diagnosis is not
confirmed:

| division | last line reached | hangs in |
|---|---|---|
| UEQ | `% Problem: … 6 cnf clauses` | **search** (clausification finished; input was already CNF) |
| FEQ | `% Problem: … 0 input CNF clauses` | **unknown**; could be clausification or later work |

Every one of the 12 FEQ cases reports a large FOF input — `CSR037+5` has
**540 249** axioms, `HWV128+1` 204 845, `CSR052+4` 44 216, down to `SWX070+1`
at 148 axioms. Their `0 cnf clauses` values mean zero clauses were supplied in
the input as CNF; they are not a count of generated clauses.

`clausify_with_provenance` currently exposes no deadline/cancellation argument.
The final `None` at `main.rs:705` and `:737` is `leaf_id_override`, not a
deadline. Long clausification work therefore cannot be cooperatively cancelled
through this API, but the campaign rows do not prove these 12 are stuck in that
function. Stage telemetry or a targeted instrumented run is needed before
assigning a root cause.

This matters beyond these rows: the harness does not record per-stage progress,
so an outer timeout cannot currently distinguish a slow preprocessing stage
from a long search.

### Secondary defect, independent of the above

The CSV cannot distinguish "searched hard and timed out" from "stopped in a
long non-search phase and was killed". Both are recorded as `Timeout` with no
marker. A harness change should preserve the exit code and whether an SZS status
was emitted, so these cases can be separated in the results.

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

---

## UI-4 — Structural leaf matching rejects some reformulated included axioms

| | |
|---|---|
| Status | **Fixed** on `cert/feq-kernel-limits` (`2ffedfe`). Kept here for the record. |
| Severity | Was 16 rows of `campaign-cascj13-feq-W8C8J1-20261002`; measured and reproduced, now 0 |
| Soundness | No false-accept path was found. The widening is the AC laws of `&`/`\|`, which hold in every model. All 16 leaves were checked against their cited axioms by an independent canonicaliser before the comparison changed. |
| Blocks | Nothing. The archived audit needs re-running; the counts below are from that re-run. |

### Observation

The archived `campaign-cascj13-feq-W8C8J1-20261002` report records
`VerifiedBad` on **16 rows**, every one of the form

```
leaf `c237` does not match problem formula `axiom_53`
```

According to the archived investigation, all 16 problems use `%include`, and
the cited axiom is not in the top-level `.p` file. The mismatch reportedly
reproduces against either edition's axiom files, suggesting it is distinct from
the corpus-mismatch issue in the remote-only guide. Original run artifacts are
not committed, so re-audit the run before relying on these campaign counts.

The cited proof leaf and the source axiom appear to be logically equivalent,
with reordering/reassociation of conjunctions and a mirrored biconditional.
This is not alpha-equivalence in the formal sense (bound-variable renaming),
and must not be labelled as such. Proof leaf `c237` of `SWV453+1`:

```
![X0]: ![X1]: ( ordered(cons(X0,X1)) <=>
  ( ordered(X1) & ![X2]: ( ... => leq(pidMsg(X0), pidMsg(X2)) ) ) )
```

`axiom_53` of casc-j13 `Axioms/SWV011+0.ax`:

```
![X,Q]: ( ( ordered(Q) & ![Y]: ( ... <= ... ) ) <=> ordered(cons(X,Q)) )
```

Same apparent conjuncts, mirrored biconditional, different binary grouping of
`&`. The excerpts abbreviate subformulas; do not treat them alone as a complete
semantic proof of equivalence.

### Structural mismatch in the leaf checker

`mrs_core::alpha::alpha_equiv` is explicitly an alpha-equivalence checker:
variables bound under quantifiers may be renamed, while free variables must
retain their identifiers (`crates/mrs-core/src/alpha.rs:1-4`). Its `And`/`Or`
case also accepts permutations of the immediate children, but recursively
compares those children without flattening the connective
(`crates/mrs-core/src/alpha.rs:37-55`). Since the FOF parser builds conjunctions
left-associatively (`crates/mrs-tptp/src/parser/fof.rs:101-120`), differently
grouped/reordered three-or-more operand forms can fail this structural check.
This is a **structural leaf-matching limitation**, not evidence that
alpha-equivalence is defined incorrectly.

Characterized with tests in `crates/mrs-proof-kernel/src/lib.rs`
(`alpha_equiv_shape_tests`):

| case | `alpha_equiv` |
|---|---|
| `p & q` vs `q & p` (2 atoms) | `true` |
| `p & q & r` vs `r & q & p` (3 atoms, reordered) | **`false`** |
| `(p & q) & r` vs `p & (q & r)` (re-associated) | **`false`** |
| `(p & q) & r` vs `(p & q) & r` (identical grouping) | `true` |
| `<=>` mirrored, atomic operands | `true` |
| `![X,Y]: F` vs `![X]: ![Y]: F` | `true` |
| `<=` vs `=>` | `true` |
| corresponding three-operand `|` cases | reordering and reassociation are **`false`** |

The structural limitation is specifically conjunction/disjunction reordering
or reassociation of three or more operands when the binary trees do not align.
In the tested shapes, biconditional mirroring, quantifier splitting, and reverse
implication are handled by `alpha_equiv`. This test matrix characterizes the
comparator; it is not a mathematical proof that the archived leaf is entailed by
its cited axiom.

Note the module contract is *not* being violated on free variables:
`crates/mrs-core/src/alpha.rs:3-4` states "Free variables must have the same
identifiers", and `term_eq` implements exactly that.

### Blast radius

`alpha_equiv` is a structural comparison used at several verification sites:

- `crates/mrs-proof-kernel/src/lib.rs:2610`, `:2627` — input-leaf checking
- `crates/mrs-proover/src/checks/axiom_leaf.rs:85`, `:171`, `:201`
- `crates/mrs-proover/src/checks/definition_folding.rs:84`, `:231`
- `crates/mrs-proover/src/checks/trivial.rs`, `crates/mrs-proover/src/verify.rs`

These call sites share the same structural limitation. The 16 rows are findings
reported from one archived campaign, not a re-audited current rate. The cited
comparison set and exact `VerifiedBad` denominator should be checked against
that run's saved proof/audit artifacts before using the counts as a baseline.

### Resolution

Reproduced first: `mrs-proover --strict` over the campaign's retained proofs
gives exactly the archived row for row -- 47 `VerifiedGood`, 16 `VerifiedBad`,
3 `Unknown` -- so the archived numbers were not stale.

The 16 leaves were then checked against their cited axioms by a canonicaliser
written from the TPTP grammar rather than from the Rust implementation
(methodology.md 1.4): bound variables as de Bruijn indices, `&`/`|` flattened
and sorted, `<=>` and `=` as unordered pairs, `<=` rewritten to `=>` with
swapped operands, free variables keeping their names. All 16 are
AC-permutations of their cited axiom. The excerpts in the Observation section
above abbreviate subformulas and are not themselves a semantic certificate;
this is.

Fixed by a new bounded comparison used **only** at named-leaf matching.
`mrs_core::alpha::alpha_equiv` is unchanged and a test pins that it still
refuses the archived pair, so the global alpha-equivalence contract did not
move. Widening acceptance is justified only because the extra equivalences are
the associative and commutative laws of `&`/`|`, which hold in every model of
classical logic: a leaf the comparison accepts is an instance of its cited axiom
by those laws alone. It is not general logical equivalence, `And` against `Or`
is decided a difference, and nine negative cases pin that.

Two further changes in the same comparison, both only ever accepting more:
same-connective operands are flattened before matching, and operand lists are
matched by complete backtracking search rather than greedy first-fit -- greedy
pairing misses permutations that exist, which is a false negative.

The comparison is bounded by `max_comparison_steps`, and exhausting it yields
`Undecided`, reported `Inconclusive`. That distinction is the point: the 16 rows
read as `VerifiedBad`, which says "this proof is wrong", because a comparison
that ran out of room was reported as a difference.

### Measured

Replaying the archived proofs through `mrs-proover --strict`:

| campaign | before | after |
|---|---|---|
| `campaign-cascj13-feq-W8C8J1-20261002` | 47 `VerifiedGood`, 16 `VerifiedBad`, 3 `Unknown` | **64 `VerifiedGood`, 0 `VerifiedBad`**, 2 `Unknown` |
| `campaign-casc30-feq-W8C8J1-20261002` | 90 `VerifiedGood`, 8 `Unknown`, 1 `Timeout` | **92 `VerifiedGood`**, 6 `Unknown`, 1 `Timeout` |

Both columns are `audit_casc_proofs --checks strict --strict-time 120` over the
campaigns' own retained proofs, so they are like for like.

No `VerifiedGood` was lost in either campaign. The residual rows are **not** UI-4
and are recorded as UI-7.

`ALG049+1` (casc-30) needs a separate note: it was the campaign's single
`Timeout`, and it now certifies, but it takes **396 s** and 777 MB, so under the
campaign's 120 s per-proof kernel budget the audit will still record it as
`Timeout`. It was over 900 s before this work. Verifying it is not the same as
verifying it inside the budget.

### Reproduce

```bash
nix develop -c cargo test -p mrs-proof-kernel leaf_matching -- --nocapture
nix develop -c cargo test -p mrs-proof-kernel alpha_equiv_shape_characterization

# End to end, from the retained proofs:
crates/mrs-bench/certification_campaign.sh \
  --audit-only /home/hack/crates/mrs-bench/results/campaign-cascj13-feq-W8C8J1-20261002 \
  --edition casc-j13
```

## UI-5 — EPU: equality InstGen is unsupported on many inputs; its contribution needs measurement

| | |
|---|---|
| Status | Open, not scheduled |
| Severity | The archived campaign records 9/100 `verdict=ok`; this is a low result |
| Soundness | No soundness finding is made here. |

### Observation

The archived report for `campaign-casc30-epu-W8C8J1-20261002` records one
`Total jobs:`, 100/100 completed, expected statuses on all 100 rows, and **9 `Unsatisfiable`,
all 9 audited `VerifiedGood`, 0 `VerifiedBad`**. A second archived run,
`campaign-casc30-epu-W8C8J1-20260930` ran a different binary
(`08fa4f1ffdf5` vs `fa52c23d73b3`) from a different commit (`c07cac9d` vs
`fb55719c`) on a different host, and solved **exactly the same 9 problems**.
Only five statuses differ, all Timeout↔GaveUp. This supports repeatability of
these observed solves across those runs, but is not a controlled measurement of
the division ceiling.

The input set includes very large problems: `HWV092-1` has a reported 696 691
clauses, and the division totals 2 847 336 clauses. None use `%include`.

### InstGen telemetry and the 43 unsupported-profile cases

From the `% SZS detail` telemetry:

| `instgen_fallback` | rows |
|---|---:|
| `unsupported_epr_profile` | 43 |
| `timeout` | 33 |
| `max_instances_exceeded` | 15 |
| `max_rounds_reached` | 3 |
| Other/no listed reason | 6 |

These reason counts sum to 100. Separately, `instgen_result` is `fallback` on 51
rows and `none` on 43, with no `refutation` value recorded. The fields have
different coverage/semantics, so do not treat the reason buckets as a
one-to-one tally of attempted/returned InstGen outcomes. These counts describe
this run only; they do not prove that InstGen is generally incapable of
contributing. The reported detail for the nine solved rows is:

| solve | InstGen profile / telemetry | `instgen_result` |
|---|---|---|
| `LAT260-2`, `LAT261-2`, `LAT264-2`, `LAT265-2`, `SET856-2`, `PUZ008-2` | no InstGen telemetry | — |
| `PUZ036-1.005`, `SYN837-1` | `pure_relational_epr` | `fallback` |
| `HWV107-1` | `epr_equality` | `none` |

The `instgen_ms` aggregate is the sum of reported telemetry for rows that
attempted the pre-pass. The archived summary reports 32.6 s total (mean 346 ms,
selected per-problem budget 750 ms). This is elapsed duration summed across
rows, not CPU time or full-division wall time, and should not be expressed as a
share of the 100 × 120 s sum of per-problem limits. It does not establish total
resource cost or potential benefit with another profile/budget.

`crates/mrs-search/src/instgen.rs:713-716` confirms that non-pure-relational
profiles return before InstGen starts. The specific 43-row count is from this
archived run's telemetry.

### Four GaveUp rows have worker telemetry short of their wall limit

The four reported rows have aggregate `elapsed_ms` below the wall time:

| problem | `elapsed_ms` | wall | gap |
|---|---|---|---|
| `HWV092-1` | 10 817 | 19.6 s | **8.8 s** |
| `HWV094-1` | 16 982 | 21.4 s | 4.4 s |
| `HWV126-1` | 116 643 | 125.1 s | 8.4 s |
| `HWV127-1` | 116 711 | 124.8 s | 8.0 s |

They report `timeout=0` and `passive=0`. These are aggregate fields: portfolio
workers have separate time slices, and pre-pass time and worker time are not
identical measures. The values do not prove no worker used its slice, nor do
they identify which component returned `GaveUp`. EPU's InstGen pre-pass is
unsupported on equality-bearing profiles (`instgen_result=none` for HWV092-1;
absent telemetry on the other three), after which the given-clause fallback
runs. The precise `GaveUp` source remains unestablished; inspect per-strategy
reports and termination telemetry before attributing it to the LRS guard.

### Where further profiling may be useful

The 61 timeouts and 30 `GaveUp` rows are not evenly distributed. The large-input
tail (`HWV*` at 155 k–697 k clauses) is a candidate for profiling. `HWV092-1`
reports 63 662 processed clauses at 10.8 s and a final `GaveUp`; aggregate
detail does not establish that loading dominates or which worker/result caused
the final status. Use stage and per-strategy timing before selecting a fix.

## UI-6 — ICU: 44 rows lack a decisive reference, and `verdict = ok` does not mean certified

| | |
|---|---|
| Status | Open, not scheduled |
| Severity | The archived run has 3 `verdict=ok`; 44 reference answers are `GaveUp` |
| Soundness | No unsound result is reported; the CSV's `ok` label is not a certificate. |

### Observation

Among the six casc-30 division runs tabulated in the remote-only guide,
`campaign-casc30-icu-W8C8J1-20261002` is a well-formed run (one `Total jobs:`,
101/101, all rows graded) and the only one to exercise a **480 s** budget,
twice the 240 s budget of FNE/FEQ/UEQ and four times the 120 s budget of EPS/EPU.
It is also the only one of those six, alongside the archived casc-j13 FEQ run, to
produce an `Error`:

| | count |
|---|---|
| `szs_status` | `Timeout` 83, `GaveUp` 12, `Theorem` 5, **`Error` 1** |
| strict kernel | `VerifiedGood` 3, `Unknown` 2, `Error` 1 |
| `verdict` | `ok` 3, `unknown` 98 |

### 44 of 101 problems have a non-decisive reference answer

`expected` is populated for every row and matches the reference file exactly
(verified against `systems/reference/answers.tsv`: 101/101 present, 0
mismatches). Its distribution:

| `expected` | rows |
|---|---|
| `Theorem` | 55 |
| **`GaveUp`** | **44** |
| `Satisfiable` | 1 |
| `CounterSatisfiable` | 1 |

`casc.sh:810-811` maps a non-committal reference to `verdict = unknown`. Thus 44
rows cannot be graded as `ok`/`ko` against a decisive reference; this does not
mean the prover cannot solve them. Of the 57 rows with decisive references, 3
`ok` is **5.3%**. State the denominator when reporting the score.

The converse also happens: `CSE007+1` and `EEE009+1` are refuted by `mrs` where
the reference gives up. Both are graded `unknown` by design. The archived audit
reports `EEE009+1` as `VerifiedGood`; that certification is not represented by
the reference-based `ok` count.

### `verdict = ok` does not mean the proof was checked

`casc.sh:798-816` computes `verdict` **solely** by comparing `szs_class(szs)`
with `szs_class(expected)`. It never consults the audit. So:

| problem | `verdict` | strict kernel |
|---|---|---|
| `EEE001+1` | **`ok`** | **`Unknown`** |

`EEE001+1` counts toward the status-class match while its proof did not certify.
The campaign evidence described here gives one concrete divergence. **Read the
`certification/audit.csv` `strict_status`, not `run.csv` `verdict`, whenever the
question is "how much is certified".**

### Two of five generated refutations are uncertified under kernel formula-size limits

| problem | `strict_detail` | problem size |
|---|---|---|
| `EEE001+1` | `node c71: negated_conjecture exceeds strict formula-size limit` | **4 axioms**, one 16.7 KB conjecture |
| `CSE007+1` | `node c18798: NNF rule exceeded strict formula-size limit` | — |

Both are `Inconclusive` from `max_formula_nodes` (default `100_000`), so the
strict checker does not certify them; this does not show that the prover failed
to find a refutation. The archived run reports `EEE001+1` refutes in **269 ms**.
`to_nnf` distributes nested
biconditionals, so a right-nested `<=>` chain of depth *n* expands toward 2ⁿ
nodes and can cross the ceiling from a modest input. This is another strict
formula-size limit case, distinct from the FEQ proof-node ceiling described
earlier. These two rows explain the archived strict result of 3 `VerifiedGood`,
2 `Unknown` among five `Theorem` rows; they do not establish a general ICU
certification rate of 3/5.

### One OS OOM kill despite the reported adaptive memory budget

`CSI008+1` was killed by the OS OOM killer at **94 161 MB** according to the
archived benchmark row; its `% Hardware:` line reports `mem_budget_mb=71642`.
Because stdout is empty, `casc.sh` populated `peak_memory_mb` from GNU `time`
Max RSS rather than the prover's `VmPeak` line. The row has no SZS status in
stdout and is classified `Error` by the harness. Treat its memory value
separately from rows with MRS-reported VmPeak; those sources are not homogeneous.

`adaptive` does not install the `casc-sim` address-space limit; however, it is
not entirely unenforced. The given-clause loop samples RSS and returns a memory
`ResourceOut` when the configured limit is reached (`given_clause.rs:1576-1588`).
The check is periodic, not on each allocation, so a large allocation can
overshoot before sampling and the OS OOM killer can win the race. The row is
evidence of such an overshoot, not proof that adaptive memory limits have no
enforcement.

The archived run reports `CSI008+1` at **2 004 866 clauses**, 73% of the
division's total 2 750 576. Its reported peak is the only ICU memory value above
60 GB; the next largest reported value is 37 913 MB, but these memory sources
are heterogeneous as noted above. The archived run did not report a solve on
this row. Whether a similar input is solvable within 480 s is unmeasured; an OS
kill can lose the status line instead of producing a graceful resource-limit
result.

---

## UI-7 — Eight FEQ refutations the strict kernel cannot decide

| | |
|---|---|
| Status | Open, not scheduled |
| Severity | 8 `Unknown` rows across two FEQ campaigns, plus one more that is only over budget. No `VerifiedBad`. |
| Soundness | No unsound step was found. All eight fail closed, and several were *reached* only because UI-4 stopped masking them. |
| Blocks | 100% strict certification of the FEQ refutations in both campaigns, at the 120 s per-proof kernel budget. |

### Observation

After UI-4 and the demodulation replay fixes (`cert/feq-kernel-limits`,
`2ffedfe` and `32356a6`), replaying the archived FEQ proofs through
`mrs-proover --strict` leaves eight rows undecided, in two families. The
remaining 64 casc-j13 and 92 casc-30 rows certify, and `ALG049+1` certifies in
396 s given a budget it does not have (see
[CERTIFICATION_STATUS.md](../CERTIFICATION_STATUS.md) §2).

| problem | campaign | node | verdict |
|---|---|---|---|
| `GEO331+1` | casc-j13 | `c355` | definitions whose bodies could not be matched against the source |
| `GEO343+1` | casc-j13 | `c286` | same |
| `GEO299+1` | casc-30 | `c522` | same |
| `GEO300+1` | casc-30 | `c285` | same |
| `ITP019+5` | casc-30 | `c700` | same |
| `ALG102+1` | casc-30 | `c391` | CNF expansion exceeded strict limits |
| `ALG104+1` | casc-30 | `c281` | same |
| `ALG127+1` | casc-30 | `c199` | same |

Node ids are not stable across runs; quote the conclusion text.

### Family 1 — definitions over structurally identical blocks

Each `cnf_transformation` step cites several introduced definitions whose bodies
are the same literals, sometimes in opposite order, and the source contains a
block for each. `GEO331+1` c355 is the clearest:

```
source block A:  ~ron(X0,X2) & ~rinside(X0,X2)
source block B:  ~ron(X1,X2) & ~rinside(X1,X2)
c349: def_..._0(X0,X2) <=> ~ron(X0,X2) & ~rinside(X0,X2)
c350: def_..._1(X1,X2) <=> ~ron(X1,X2) & ~rinside(X1,X2)
goal clause:     ... | def_..._0(X0,X2) | ... | def_..._1(X1,X2) | ...
```

`match_core_multiset` compares `And`/`Or` operands as multisets, so `c349` and
`c350` both match both blocks and the greedy fold gives both blocks to whichever
definition runs first. The other is then left unmatched and the guard reports
`Inconclusive`.

Two things were established:

* Comparing operands **in order** first separates mirror-image definitions
  (`GEO331+1` c211, `GEO343+1` c155, `GEO300+1` c36 now certify; that pass is in
  the tree and is strictly narrower than the multiset comparison, so it only ever
  accepts less). It does **not** separate the c355 family, because there the two
  bodies have the *same* operand order and nothing but variable identity
  distinguishes the blocks.
* Reversing the order of the definitions does not help either, because the
  collision is not about order: the first definition takes both blocks whichever
  order it runs in. A block-to-definition *matching* is needed, which needs
  block identity.

### The blocker: variable identity is not available

The discriminator is which source variables each definition's block uses: block
A is over the source's `X0,X2` and belongs to `def_..._0`, block B is over `X1,X2`
and belongs to `def_..._1`. The kernel cannot see that. `LowerCtx` assigns
`VarId`s per annotated formula, in order of first appearance, so:

* `c349`'s `X0` and the source's `X0` get different `VarId`s;
* `c349` and `c350` get *identical* `VarId`s for their two bound variables, so
  they are the same formula to the matcher.

`is_identity_mapping` -- the "is this the definition's own block" test, whose own
comment says "same variable names as the rendered definition" -- compares
`VarId`s and therefore can essentially never succeed across two independently
lowered formulas. It was written for a shared name space that does not exist.

Interning one `VarId` per variable *name* for the whole verification would fix
it, but it is a change to the kernel's variable model, not to a limit, and it
introduces a capture hazard: a formula that uses one name both free and bound
would conflate them, and `alpha_equiv`'s depth-indexed relation would then treat
a free variable as bound. That is a widening of leaf matching and needs its own
independent validation before it lands. **Not attempted here.**

### Family 2 — the CNF expansion is combinatorial, not under-budgeted

`ALG102+1` c391, `ALG104+1` c281 and `ALG127+1` c199 report `CNF expansion
exceeded strict limits`, which reads like a ceiling. It is not. The ceiling is
100 000 clauses, and raising it does not converge:

| ceiling | `ALG127+1` c199 | `ALG102+1` c391 |
|---|---|---|
| 100 000 (current) | 0.47 GB, 1.4 s, declined | 0.29 GB, 1.0 s, declined |
| 200 000 | 0.77 GB, 1.9 s, declined | 0.48 GB, 1.4 s, declined |
| 500 000 | 2.6 GB, 6.7 s, declined | 1.4 GB, 3.7 s, declined |
| 2 000 000 | 12.3 GB, 33.6 s, still not finished | 6.2 GB, 15.3 s, declined |

The kernel expands the *whole* source into clauses and then asks whether the goal
is among them. For these sources that expansion is exponential in the number of
disjuncts, so no ceiling helps: it is an algorithmic choice, not a resource
number. The file already contains the right idea -- the goal-directed
direction-clause fast path, which certifies `ALG102+1` c103 without expanding
anything -- and these three nodes need that treatment (or an equivalent
residue-based check) rather than more headroom.

`cnf_expand` now reports *why* it stopped, separating "a ceiling was reached"
from "the normalizer left a shape this expander does not produce"; both used to
be reported as "exceeded strict limits", which sent the investigation after a
ceiling that was never the cause.

### Reproduce

```bash
# Each row, from the retained proof:
TPTP=crates/mrs-bench/problems/casc-30 target/release/mrs-proover \\
  --strict --no-atp --workers 1 --time 240 \\
  /home/hack/crates/mrs-bench/results/campaign-casc30-feq-W8C8J1-20261002/certification/proofs/mrs/feq/ALG127+1.s
```

The retained proofs are under
`/home/hack/crates/mrs-bench/results/campaign-<edition>-feq-*/certification/proofs/mrs/feq/`.

## UI-8 — Measured: passive-queue retention is not the constraint on the hard tail

| | |
|---|---|
| Status | Measured, **negative**. The hypothesis is retired; the underlying question stays open. |
| Severity | None directly. It redirects where to look for the tail. |
| Soundness | Unaffected. No proof or model result changed meaning. |

### The experiment

Full casc-30 UEQ division, 300 problems, 240 s, on `teenf9901`
(16 physical / 32 logical, 128 019 MB), `--jobs 1`:

```
MRS_WORKERS=8 MRS_HARDWARE=casc-sim MRS_SIM_TIME_FACTOR=1 \
MRS_NO_LRS=1 MRS_MAX_MEMORY_MB=90000 CERT_JOBS=4 \
crates/mrs-bench/certification_campaign.sh \
  --edition casc-30 --systems mrs --divisions ueq --casc-times --jobs 1 \
  --output crates/mrs-bench/results/campaign-casc30-ueq-W8P8J1-NOLRS-MB90G-20261005
```

Every knob verifiably took effect: `lrs_discarded == 0` on all 276 rows
carrying telemetry, `casc_limit_s=238 sim_limit_s=238` (so `MRS_SIM_TIME_FACTOR=1`
was needed — at the default of 2.0 the budget becomes 476 s while the harness
SIGTERMs at 250 s), `pinned_cpus=16` across 8 physical cores, and
`address_space_mb=90000`. Integrity clean: one `Total jobs:`, 300/300, nothing
reached the rlimit (peak 71 120 MB), no OOM.

### Result

| | baseline (2026-10-02) | LRS off |
|---|---:|---:|
| solved | 123 | **111** |
| certified | 121 | 110 |
| `GaveUp` | 10 | 6 |
| `Timeout` | 167 | 183 |
| silent kills (UI-2) | 15 | **24** |

17 problems lost (all to `Timeout`), 5 gained (`COL009-1`, `COL057-1`,
`LCL211-10`, `LCL400-10`, `SYN615-10`). **On the 12-problem diagnostic subset
chosen for the highest discard ratios the effect is +1 / -1 / 10 unchanged** —
no support for the hypothesis even where it should have been strongest.

### The headline -12 is confounded and should not be read as an LRS effect

The baseline ran `--hardware adaptive` (unpinned, 8 workers spread over 16
physical cores); this arm ran `casc-sim`, **pinned to 8 physical cores**. Two
variables moved. The loss distribution points at throughput rather than search
quality: the 17 lost problems were solved in a **median 144 s** in the baseline,
with **14 of 17 at 120-238 s**, against an overall baseline solve median of
**15 s**. Losses concentrate exactly where headroom is thinnest, which is the
signature of a per-worker slowdown under SMT contention, not of worse search.

Settling this needs the control arm — `casc-sim`, pinned, LRS at default — on the
12-problem subset first. Without it the -12 is uninterpretable, in either
direction.

### What it did establish

**LRS is load-bearing.** Turning it off costs solves overall and adds silent
kills. So raising the LRS floor, which was an earlier suggestion, would move in
the wrong direction.

**A new cost of large queues, distinct from memory.** Silent kills rose 15 → 24,
with 12 new ones. Every one of the 12 had a large *baseline* queue
(`passive` 40 162-95 156, `generated` 380 672-1 993 586), and the LAT family
appears repeatedly. Memory was **not** the cause — peak 71 GB against a 90 GB
rlimit, and the new silent kills sit at 10-18 GB. The consistent reading is that
queue-size-dependent per-iteration work grows without the prune, so an iteration
outruns the 238 s deadline and the harness SIGTERMs at 250 s with no SZS line:
UI-2's UEQ mechanism, made worse rather than caused.

### What is still open

Why the tail is slow is now unaddressed by this experiment. Remaining
candidates: per-worker throughput (favoured by the loss distribution above), or a
generator producing genuinely distinct heavy clauses, in which case the ~37:1
generated-per-processed ratio is intrinsic and there is no redundancy to remove.

Note that the ratio itself was never a sound measure: `forward_subsumed` counts
*selections* discarded (`given_clause.rs:1810`), so `fwd_subsumed/generated` is
bounded near 1/37 ≈ 2.70% by construction. Any future attempt to measure
redundancy elimination must compare **discarded against retained**, not generated
against subsumed.
