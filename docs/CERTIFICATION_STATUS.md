# Certification status

The single place that records what `mrs-proover` scores and what fraction of
`mrs`'s own proofs its strict kernel can certify, with the hardware each number
was measured on. Older status notes are kept for history but are not
authoritative; where they disagree with this file, this file is right.

Two questions, kept separate on purpose:

- **ProoVer 2026** — how well does the *verifier* score against a fixed corpus
  of foreign proofs? Scored, and the ceiling is the corpus's own wall-clock
  behaviour, not the verifier's judgement.
- **Self-certification** — can `mrs` certify the proofs `mrs` produces? This is
  an offline invariant measured by replaying a benchmark run's own output
  through the kernel.

---

## 1. ProoVer 2026

Corpus: `crates/mrs-bench/proover-corpus/Proover2026` — the 100 CASC-J13
problems `PRV000+1` … `PRV099+1`, 50 valid / 40 evil / 10 locally-sound evil,
150 points available, 138 after the panel's nine removals.

Gate: `crates/mrs-bench/certification_gate.sh` (default 20 s per proof,
`--time` overridable with `GATE_TIME`).

| measurement | result |
|---|---|
| full corpus, 20 s/proof, 4 workers, 4-core WSL box | **150 / 150** |
| official panel subset of 91 (138 available) | **138 / 138** |
| `VerifiedGood` on a valid proof (false rejection, −1) | **0** |
| `VerifiedGood` on an evil proof (−10) | **0** |
| `Unknown` | 0 |

Reproduce with the canonical scorer:

```bash
nix develop -c cargo run --release -p mrs-bench --bin score_proover2026 -- \
    crates/mrs-bench/proover-corpus/Proover2026 --competition \
    --proover target/release/mrs-proover --time 20 --workers 4 \
    --output reports/proover-2026.tsv
# score=150 good=60 bad=40 unknown=0 false_rejection=0 unsound=0
```

Getting the last point took a real fix rather than more time: `PRV043+1` used
to hang. It is a valid five-step proof whose axiom and conjecture are 100-term
right-nested `<=>` chains, and NNF *distributes* nested biconditionals, so
normalising one is exponential in the chain depth. Both the kernel and the
competition verifier normalised such formulas unconditionally — in the kernel's
case in its own problem preparation, before any step was examined — so the
verifier ran past 90 s under `--time 20` and returned nothing. Every NNF
conversion in both crates is now bounded (`mrs_cnf::nnf::to_nnf_bounded`), and
the shape decides in 0.0 s. The same class of input is a denial-of-service
vector for anything that normalises proof or problem text, so
`deep_biconditional_chain_is_decided_within_budget` pins it.

At this level the corpus is the ceiling, not the verifier. The remaining work is
therefore not score: it is robustness on proofs the corpus does not contain
(§3), and the three problems that need 8-core hardware to decide inside a
tighter budget.

### Scoring policy, and one change worth knowing about

An unresolvable `% Proof :` link used to leave every leaf unchecked — the
verifier answered `Sound` for provenance it could not confirm, i.e. it verified
the proof *modulo assumptions*. Combined with a resolution order that tried the
header path against the *process working directory* first, that turned
`PRV051+1` and `PRV074+1` from `VerifiedBad` into `VerifiedGood` whenever the
binary was run without `--problems-dir` from an unrelated directory: −20 on a
100-problem corpus, and the competition's worst outcome.

Both modes now fail closed: a leaf whose provenance cannot be checked is
`Unknown`, and the aggregation pins the invariant that a proof with no loaded
problem can never be `VerifiedGood` regardless of how individual checks treat
their steps. The competition wrappers pass `--problems-dir`, so this costs
nothing there; it costs coverage only on proof sets that ship no problem file
(the Zenodo Otter half), where the previous behaviour was a free pass for every
axiom. See `docs/VERIFIER_SPEC.md` §2 and the regression tests in
`crates/mrs-proover/tests/corpus_regressions.rs`.

---

## 2. Self-certification

Definition: of the refutations `mrs` produced in a benchmark run, how many does
`mrs-proof-kernel` certify? Anything else is `Unknown` (or, historically,
`VerifiedBad`, which is a kernel gap rather than a bad proof when the proof
came from `mrs` itself).

Campaign: `crates/mrs-bench/certification_campaign.sh` — runs the normal
competition route and then replays the archived proofs through the kernel with
a reason histogram. The `audit_casc_proofs` report is the raw data.

### Archived CASC-J13 run `casc-j13-W8J2-nosharing-20260923` (180 s, 8 workers)

215 refutations, re-verified on a dev box. **The re-audit figure is 193
`VerifiedGood` (89.8 %), 22 `Unknown`, 0 `VerifiedBad`** — see "The residual 22"
below for the node-limit fix that moved 189 to 193. This is not a fresh campaign
measurement.

| strict verdict | 2026-09-23 (build of that day) | before proof-node limit fix | after proof-node limit fix |
|---|---:|---:|---:|
| `Certified` | 152 | 189 (87.9 %) | **193** (89.8 %) |
| `Unknown` | 57 | 25 | **22** |
| `VerifiedBad` | 5 | **0** | **0** |
| killed (wall clock) | 0 | 1 | **0** |

The 2026-09-23 column is what the run's own audit recorded. The five
kernel-side rejections it reported (CSR117+1, GEO111+1, MGT005+1, SEV606+1,
SWX217+1 — equality resolution, condensation, subsumption resolution and
superposition shapes) are all fixed at HEAD; SWX217+1 had also been reported
`VerifiedBad` by the ATP ladder, and now passes both.

An earlier revision of this table claimed 192 at HEAD. That figure predates the
bounded-NNF work, which moved 3 problems from `Certified` to `Unknown` by design
— the trade is named below, and the honest total is 189, not 192.

Every number in this section is a re-audit of an *archived search* on different
hardware than the search itself. That is legitimate — nothing in the kernel
depends on where the proof was produced — but it is not the same as a fresh
campaign, and for the 18 demodulation rows the difference is decisive.

### UEQ CASC-J13 campaign `campaign-ueq-W8C16J1-20260930` (180 s, 8 workers, `c07cac9`)

A single-division UEQ campaign answers **107 refutations, 107 `VerifiedGood` of
107 applicable, 0 `VerifiedBad`, 0 `Unknown`, 0 `Error`** under `--checks
strict` — clean on every row that produced a proof, against 87.9 % on the W8J2
run above. The certification gap that dominates that table (the 18
`demodulation` rows) is therefore not what limits this division; the 293
remaining rows are `N/A: Incomplete`, i.e. they produced no proof to check.

The number to watch is not the 107 but the comparison: the stored `codex.db`
CASC-J13 UEQ run answers 257/400 on the same problems, so this campaign loses
161 and gains 11, with median `processed_per_s` on the regressed rows falling
from 1649.5 to 116.0. Coverage, not certification, is the open question. See
[`reports/benchmarks/ueq-2026-09.md`](reports/benchmarks/ueq-2026-09.md).

### The residual 22, and what closes them

One ceiling had been doing two jobs: bounding the proof's DAG and bounding
per-step CNF expansion. A long-but-honest proof was refused outright —
`SET017+1`'s 119 335 formulas drew `proof has 119335 formulas, exceeding limit
100000` — which reads in a report exactly like the checker giving up.
`VerificationLimits::max_proof_nodes` (1 000 000) now bounds the input DAG and
`max_nodes` stays at 100 000 for expansion. This fix is included on `main`.

Re-auditing all 215 archived refutations of this run on a 2-core dev box,
`--strict-time 120`, `--jobs 1`:

| | 2026-09-23 | before proof-node limit fix | after proof-node limit fix |
|---|---:|---:|---:|
| `VerifiedGood` | 152 | 189 | **193** (89.8 %) |
| `Unknown` | 57 | 25 | **22** |
| `VerifiedBad` | 5 | 0 | **0** |
| killed (wall clock) | 0 | 1 | **0** |

The four that moved are exactly the three node-limit rows plus the killed one,
and all four now certify on the dev box: `SWV406+1` (657 454 formulas) in
98.9 s / 4.8 GB, `MGT079+1` (227 551) in 26.4 s / 970 MB, `SET017+1` (119 335)
in 14.4 s / 660 MB, `KRS234+1` (103 464) in 13.8 s / 531 MB.
Cost of the whole re-audit: **310 s wall, 4.77 GB peak RSS** — re-auditing an
archived run is not remote work.

Reasons for the residual 22 (one row per proof):

| count | reason | status |
|---:|---|---|
| 10 | `demodulation` node exceeds the strict rewrite-step limit | **fixed, awaiting re-measurement** |
| 5 | `demodulation` replay could not reach the conclusion within the implemented search | **fixed, awaiting re-measurement** |
| 3 | `demodulation` intermediate clause exceeds the strict size bound | **fixed, awaiting re-measurement** |
| 3 | a `fof_nnf` step whose parent's NNF exceeds the 100 000-node budget (`BIO006+1`, `CSR115+8`, `CSR116+19`) | open, deliberate |
| 1 | `ac_superposition` replay incomplete | **root-caused and fixed — and it was a prover soundness defect, not a kernel gap** |

The `ac_superposition` row is the one `Unknown` in that table, and it was the
wrong kind of problem. Superposition was deriving *specialisations* of its
resolvents: `mrs_unify::robinson::unify_ac_id` bound the **target** clause's
variables, so a derived clause could be strictly stronger than its two cited
parents entail. Replaying the archived parents of all three affected nodes
through the prover's own superposition reproduces their conclusions character for
character — the prover computed what the proof says, it just was not a
superposition. `unify_ac_rigid_id` now refuses to bind the variables a caller
declares rigid and superposition passes the target's variables as rigid.

Two consequences for this file's numbers, stated plainly:

- **The kernel was right and the prover was wrong.** `VerifiedBad` was never
  recorded for these because `verify_superposition` happens to reject most of
  them, not because it accepted them. A kernel that accepted such a step would
  have certified an unsound refutation.
- **The archived figures above do not move, and cannot.** Those proofs are
  artifacts of the old binary; the kernel reads their text as written. Only a
  fresh campaign re-measures anything, and it will measure *less* coverage: the
  fix also removes sound inferences the prover could only find by way of the
  unsound one, which costs 5 of the 48 local `problems/*.p` regression rows
  (the AC equational ones). Full analysis, the measured loss, and the bounded
  next step — AC superposition has to match the equation's side into a *subset*
  of the target's AC arguments — are in
  [`policies/unresolved-issues.md`](policies/unresolved-issues.md) UI-1.

Re-auditing the *archived* proofs cannot close the 18 demodulation rows, and
the reason matters: those proofs predate the `demodulation_steps(...)`
annotation, so replaying them exercises the fallback search path. Only a fresh
search emits the annotation the new replay path consumes. That is why a fresh
campaign is a remote item and the re-audit above is not — see
[`guides/remote-only-work.md`](guides/remote-only-work.md) R1.

The 18 demodulation rows are one gap seen three ways: the trace-replay work
lands them in a bounded search, and the bounds added afterwards (rewrite steps,
intermediate clause size) then split the residue by which bound fired first.
They are fixed in code and unmeasured, which is exactly why the next step is a
fresh campaign rather than another edit.

The NNF-budget row is the price of bounding that conversion, and it is a price
worth paying: those three problems have IFF structures whose unbounded NNF was
large but finite, while an unbounded conversion on a slightly deeper chain is
what made `PRV043+1` undecidable in any budget. A fixed bound fails closed and
fast; an adaptive one is possible later if those three are ever worth it.

The same bounded-NNF work also removes a hang class that no corpus score would
have shown: any proof or problem containing a long biconditional chain used to
be undecidable in any budget. `PRV043+1` is the committed example.

The demodulation gap was 75 % of everything the kernel could not certify, and
it was not a budget problem: raising `max_rewrite_steps` from 64 to 10⁶ and the
rule-set cap from 16 to 4096 converted 2 of 18 and hung the rest. A demodulation
step collapses a whole fixpoint of unit-equality rewriting into one node citing
every equality it used, so the kernel had to *search* for a rewrite order, with
a greedy first-match walker and no backtracking across positions.

The prover already knows the answer, so it now records it: `demodulate_id`
writes every rewrite it applies into the `ProofWitness::Demodulation.steps`
field that was always present and always empty, and `mrs-proof` emits it as
`demodulation_steps(rule(<parent>, <literal>, [<path>]), …)`. The kernel
replays the trace — recomputing each rewrite by matching the cited equality
against the named subterm, backtracking over the two orientations, since a step
is contracting under one and expanding under the other — and accepts only when
the replay reproduces the exported conclusion. A trace that does not replay
falls back to the bounded search, so a foreign prover's annotation or a recorder
defect costs speed, never certification.

Those archived proofs predate the annotation, so they still exercise the search
path and the 193/215 re-audit figure is the *fallback* number after the
proof-node-limit fix. The post-annotation figure needs a fresh run on 8-core
hardware:

```bash
MRS_WORKERS=8 crates/mrs-bench/certification_campaign.sh \
    --edition casc-j13 --systems mrs --divisions fne,feq,ueq \
    --casc-times --jobs 2 --output crates/mrs-bench/results/cert-j13-$(date +%Y%m%d)
```

Expect the 18 demodulation rows to move to `Certified`; if they do not, the
reason histogram names the shape that still fails. `--subset <file>` restricts a
campaign to named problems when only a division or a regression is being
re-measured, and the campaign resolves the corpus from the run's own
`run_meta.txt` rather than from `--output`.

To re-audit a run that already exists without paying for the search again:

```bash
crates/mrs-bench/certification_campaign.sh --audit-only \
    crates/mrs-bench/results/cert-j13-20261002
```

Phase 1 is the expensive half — every problem at the full wall clock — and
nothing in the audit depends on the search having just finished, so a run
directory is enough to re-audit against the current kernel.

### FEQ campaigns of 2026-10-02, and the kernel-limit work

Two FEQ campaigns were audited on 2026-10-02 and, unlike the runs above, they
were audited *twice*: once by the build of that day and once after the
kernel-limit work on `cert/feq-kernel-limits`. Both audits replay the same
retained proofs, so the comparison is like for like.

| campaign | audited before | audited now |
|---|---:|---:|
| `campaign-cascj13-feq-W8C8J1-20261002` | 47 `VerifiedGood`, **16 `VerifiedBad`**, 3 `Unknown` | **64 `VerifiedGood`, 0 `VerifiedBad`**, 2 `Unknown` |
| `campaign-casc30-feq-W8C8J1-20261002` | 90 `VerifiedGood`, 8 `Unknown`, 1 `Timeout` | **92 `VerifiedGood`**, 6 `Unknown`, 1 `Timeout` |

Both columns are the official harness (`audit_casc_proofs --checks strict
--strict-time 120`) over the campaigns' own retained proofs, so they are like for
like; the proofs are not re-searched. Re-audit with
`crates/mrs-bench/certification_campaign.sh --audit-only <run-dir>`; the archived
run directories record the *producing* machine's absolute paths, so they need
`CASC_PROBLEMS_ROOT` or `--edition` pointed at the local corpus.

The casc-j13 `VerifiedBad` column is the point. Sixteen refutations were being
reported as **broken proofs** when they are sound, and the cause was one
comparison: `alpha_equiv` pairs `And`/`Or` operands without flattening the
connective, so `A & B & C` and `C & B & A` compare unequal. A proof leaf is a
re-serialisation of its cited axiom, not a copy of it, so every `%include`
problem whose axiom grouped its conjuncts differently was refused. All 16 were
confirmed to be AC-permutations of their cited axioms by a canonicaliser written
independently of the kernel before the comparison changed, and the fix adds the
AC laws of `&`/`|` — valid in every model — at named-leaf matching only. The
global `alpha_equiv` contract is unchanged and a test pins it. Full write-up,
including why this was not a limit and why widening here is sound:
[`policies/unresolved-issues.md`](policies/unresolved-issues.md) UI-4.

Eight rows remained undecided across the two campaigns after that work, in two
families, and neither was a ceiling that could be raised:

* **Five** were `cnf_transformation` steps whose cited definitions have bodies
  over blocks the greedy fold could not tell apart. The discriminator is which
  source variables each block uses, and the kernel's variable model does not
  carry that: `LowerCtx` numbers variables per formula, so two definitions over
  the same block shape get identical `VarId`s.
* **Three** were `ALG102+1` c391, `ALG104+1` c281 and `ALG127+1` c199, whose
  sources expand combinatorially into clauses, so no ceiling helps; the
  whole-source expansion is the wrong algorithm for them.

Both families are now decided by complementary bounded kernel checks. The
goal-directed residue check (`goal_directed_cnf_entailment`) distributes the
goal clause over the source instead of expanding the entire source. The
variable-name-aware definition matcher uses original per-formula names without
changing `VarId` scopes, and the conjunct-local path expands only the relevant
top-level conjunct when that suffices. The residue check remains as a fast path;
all checks fail closed when their bounds or matching preconditions are not met.
On a 16-core, 128-GB host, the strict audit of the same retained FEQ proofs
improved from 94 to 100 `VerifiedGood` out of 109 applicable refutations, with
no `VerifiedBad`. Two `Unknown`, one timeout, and six withheld proofs remain for
other reasons.

Details, the per-proof before/after verdicts, and the soundness argument:
UI-7.

`ALG049+1` deserves its own line because "certifies" and "certifies in budget"
are different claims. It is the casc-30 `Timeout` in the table above, and given
900 s it certifies — in **396 s** and 777 MB, down from over 900 s before this
work. The audit's per-proof kernel budget is `--strict-time 120`, so at the
campaign's own budget it is still a `Timeout`. Two of the eleven rows in the
"not certified" column are therefore not the same kind of thing: eight are
`Unknown` and one is over budget.

### Fast invariant

`crates/mrs-proover/tests/mutation_sweep.rs` keeps seven real `mrs` proofs
(`tests/resources/mrs_proofs/`) as canaries: each must certify, and none of 66
mechanical mutations of them may. That is the always-on regression net for
this section, in under a second, with no ATP.

### Satisfiability (EPS) is a separate axis

A `Satisfiable` claim is credited only with a model. The certifier verified a
solver model clause by clause and then discarded it, so every EPS result was
`Satisfiable` with nothing checkable — the audit's model path read 0 certified
models across the whole corpus and the kernel's 1 188-line validator was never
fed. The model now travels with the verdict: the SAT-backed tier turns its
re-verified assignment into a complete `ModelCertificate`, the ordered-closure
tier recovers one from the same grounded set, and `mrs` prints it in an SZS
`FiniteInterpretation` block that `mrs-proover` validates as `VerifiedGood`.
Under `--self-check` the kernel re-validates it before the status line stands.

That model is only sound if the certificate reads the constants the way the
equality pass wrote them, and it did not. `build_model_certificate` gave every
constant its own domain element, so for a clause set containing a unit
equation `a = b` — consumed by the union-find pass, which rewrites every
occurrence to one class representative — it emitted a certificate asserting
`a != b`. The verdict was right and the model backing it was wrong. The class
map is now threaded to the builder, and the builder re-derives the requirement
from the originals itself rather than trusting its caller, since this failure
mode is a silent wrong answer on a `Satisfiable` verdict.

The status/polarity convention matters here, and a first implementation got it
wrong in the direction that hides results. A problem supplied directly as
`negated_conjecture` clauses — the shape the CASC EPR divisions are made of —
has no conjecture to falsify, so a model of it is a `Satisfiable` answer, not a
counter-model. The kernel's per-formula checks always had that right; its
summary check for the status did not, and treated any `negated_conjecture` role
as a conjecture. `mrs --self-check` on `EPS/SYN322-1.p` therefore reported
`GaveUp` for a model that `mrs-proover` independently accepted, and a campaign
reported every EPS model as `invalid_model` — indistinguishable from "the prover
emitted no models". Now: `Satisfiable` requires no `conjecture` role,
`CounterSatisfiable` requires one, and the same problem under `--self-check`
answers `Satisfiable` with a kernel-certified model. `crates/mrs-proover/tests/model_certification.rs`
pins all four cells of that matrix.

---

## 3. Robustness beyond the corpus

The corpus is 100 proofs, and a perfect score on it says little about the 100
that were not in it. Three mechanisms cover the difference:

| mechanism | where | what it pins |
|---|---|---|
| committed exploits | `evil-proofs/exploits/` (20 cases) | `mrs-proof-kernel/tests/evil_proofs.rs` |
| hand-written mutations | `crates/mrs-proof-kernel/tests/mutations.rs` | specific attacks: formula, sign, term, parents, rule, status, role, provenance, conclusion, root |
| systematic sweep | `crates/mrs-proover/tests/mutation_sweep.rs` | breadth over real `mrs` proofs |

`crates/mrs-bench/fuzz_proover.sh` extends this to proofs from external provers
by mining the rule frequency the verifier could not handle, which is how the
`skolemize` coverage work was found.

The ProoVer scoring asymmetry is the reason for the absolute bar: a valid proof
rejected costs 1, an evil proof accepted costs 10, so the only acceptable
failure mode is `Unknown`.

---

## 4. Proof size

Median 17 KB, p90 1.0 MB, max 170 MB over 215 archived refutations; three
proofs exceed the 10 MB output floor the CASC rules state. The tail is
AVATAR-dominated — about half those nodes are `avatar_split_clause`
certificates. `--proof-bytes-limit` (default 8 MiB) omits an over-budget proof
with a diagnostic instead of letting it be killed, and proof size is now in the
`% SZS detail` telemetry. The default has never been measured against the real
CASC allowance on the real hardware, and proof size depends on portfolio width
as well as on the problem: `MGT079+1`'s 8-worker proof was 38.6 MB in this run
and 21.2 MB in the 2026-09-30 FEQ campaign, while a fresh 2-worker run on the
same problem yields 2.8 MB. Full analysis and the reduction options:
`docs/PROOF_SIZE_BUDGET.md`; the outstanding measurement:
[`guides/remote-only-work.md`](guides/remote-only-work.md) R4.

---

## 5. Superseded claims

Corrected here so they are not read as current:

- **`campaign-feq-W8C16J2-20260930` produced no measurement.** Two `casc.sh`
  processes were launched concurrently — one `--edition casc-30`, one
  `--edition casc-j13` — into the same `--output` directory. They overwrote each
  other's `run.csv` (both truncate with `>`), `run_meta.*`, raw artifacts and
  archived proofs, so 256 of 600 rows collided and 78 of 144 refutations were
  never checked at all. `run_meta.txt` was won by the `casc-j13` process, so the
  audit resolved every row against the `casc-j13` corpus: the five
  `VerifiedBad` are `casc-30` leaves checked against `casc-j13` problems, and
  every leaf reproduces its own edition's text exactly. The campaign summary's
  `88.1 % certified` also divided by only the five statuses it knew about; over
  all refutations the figure was 41.0 %. Treat the directory as void and re-run
  FEQ as one edition per directory. Diagnostic: `casc.sh` does not refuse a
  non-empty `--output`, and `audit_casc_proofs::find_problem` ignores the
  per-row `edition` column. Both are unfixed. See
  [`guides/remote-only-work.md`](guides/remote-only-work.md).
- `docs/PROOVER_2026.md` claims `138/138` and `150/150`. Both figures are now
  reproduced on a 4-core box at 20 s per proof, but the earlier ones were
  measured on 8-core competition hardware; treat the hardware line in §1 as part
  of the number.
- `docs/AUTO_PROOF_REVIEW.md` names `PRV067+1` as the last 2-point gap. It is
  `VerifiedBad` at HEAD, and that document's `max_equivalence_steps` figure
  (200 000) is wrong: the default is 5 000.
- `docs/SOUNDNESS_STATUS.md` reports 834 verified of 3 180 over the whole TPTP
  FOF/UEQ set. That is a competition-mode audit of an older build; §2 above is
  the current strict-kernel measurement.
- `crates/mrs-bench/proover-corpus/Proover2026/metadata.toml` carries
  `expected_score = 148` from one historical 8-core run. Nothing asserts it:
  the values are hard-coded in `normalize_proover2026.rs` and written out, while
  `validate_proover2026` only checks counts and checksums, and
  `certification_gate.sh` recomputes the score from the manifest every time. So
  the field is documentation, not a gate — do not read a stale 148 as a
  regression, and do not treat a fresh run's number as a mismatch against it.
