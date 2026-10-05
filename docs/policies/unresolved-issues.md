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
| Status | Open, not scheduled. **Fixing it widens accepted leaf matching.** |
| Severity | One archived campaign reports 16 leaf mismatches; not independently re-audited |
| Soundness | No false-accept path is identified here. Whether each refutation is valid requires checking the original proof and source axiom. |
| Blocks | Strict-certification counts for that campaign need re-audit against retained artifacts. |

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

### Why the matching contract needs review

The immediate finding is about leaf matching: a verifier may need to recognize
a proof leaf as a valid reformulation of its cited source, while the current
helper checks alpha-equivalence only. Any broader matcher must remain bounded
and establish equivalence; connective-shape normalization is justified only
when associative/commutative laws are part of the chosen comparison contract.
The archived `VerifiedBad` status alone does not establish that the rejection
was false.

An appropriate repair may be a bounded comparison modulo associativity and
commutativity of `And`/`Or` at named-leaf matching rather than changing the
global `alpha_equiv` contract. Before implementing it, obtain and independently
validate the archived proof and full included axiom (including variable
binding, implication polarity, and every connective operand); the excerpts here
abbreviate formulas and are not a complete semantic certificate.

### Reproduce

```bash
nix develop -c cargo test -p mrs-proof-kernel alpha_equiv_shape_tests -- --nocapture

# End to end, once the original campaign proof artifacts are available:
TPTP=crates/mrs-bench/problems/casc-j13 \
  target/release/mrs-proover --problems-dir <cert>/proofs/mrs/feq \
  --workers 1 --time 180 --strict SWV453+1.s
```
