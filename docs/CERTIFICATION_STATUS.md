# Certification status

> Interpretation policy (2026-10-25): the dated tables below describe their
> named producer/checker revisions and budgets; they are not a single HEAD
> baseline. Later re-audits supersede earlier interpretations only for the same
> retained artifacts. Zero `VerifiedBad` means no rejection observed in that set,
> not zero possible unsoundness. Cross-host/build comparisons are observational.
> Establish a fresh paired baseline before performance promotion; see
> [`methodology`](policies/methodology.md#85-experimental-design-and-evidence-strength)
> and the active [`roadmap`](../ROADMAP.md).

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
`mrs-proof-kernel` certify? Other outcomes include `Unknown`, timeout, omitted
proofs and `VerifiedBad`. A rejection requires investigation: the producer,
exporter or checker may be defective. Originating from `mrs` does not establish
that a proof is valid.

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

`GEO111+1` is specifically a false rejection in the historical
subsumption-resolution check: current strict replay of its archived proof
returns `VerifiedGood`. The old table above remains the audit output from
2026-09-23; do not treat its `VerifiedBad` count as a current re-audit. The
duplicate-target-literal issue and regression are recorded in UI-1 of
[`policies/unresolved-issues.md`](policies/unresolved-issues.md#ui-1--ac_superposition-conclusions-that-are-not-a-superposition-of-their-cited-parents).

An earlier revision of this table claimed 192 at HEAD. That figure predates the
bounded-NNF work, which moved 3 problems from `Certified` to `Unknown` by design
— the trade is named below, and the honest total is 189, not 192.

Every number in this section is a re-audit of an *archived search* on different
hardware than the search itself. That is legitimate — nothing in the kernel
depends on where the proof was produced — but it is not the same as a fresh
campaign, and for the 18 demodulation rows the difference is decisive.

### UEQ CASC-J13 campaign `campaign-ueq-W8C16J1-20260930` (historical, 180 s, 8 workers, `c07cac9`)

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

### Fresh UEQ syntactic-recovery campaign (2026-10-16)

The full-division follow-up at
`~/crates/mrs-bench/results/ueq-syntactic-recovery-j13` generated 112
refutations on all 400 CASC-J13 UEQ problems. Strict audit returned **111
`VerifiedGood`, 0 `VerifiedBad`, 1 `Unknown`, and 0 errors**. Thus the latest
measured strict result is **111/400 (27.75%)**, four certified problems above
the 107/400 campaign above; the raw refutation count is 112/400 (28.00%). The
remaining `Unknown` is `RNG009-7`, an incomplete `ac_superposition` replay at
node `c3774551` (“superposition conclusion is not a valid rewrite”). This is
the known AC replay gap; no proof was classified `VerifiedBad`.

The recovery run used the shipped portfolio, eight workers on eight physical
cores, and `MRS_SHARED_POOL_INTERVAL=0`. Its host had 96,350 MB RAM, below the
128 GB CASC allowance; the run reported 81,897 MB effective memory. No row
stopped for a resource limit. The 107-to-111 strict-score comparison is
descriptive, not a controlled code-only comparison: it is one run against a
different binary, toolchain and host, and the problem sets differ (101 common,
11 recovery-only, 6 baseline-only). The four-solve increase needs repeated
measurement before it is treated as stable. See the
[`UEQ recovery report`](reports/benchmarks/ueq-soundness-regression-2026-10.md)
for run provenance and the comparison details.

### The residual 22, and what closes them

One ceiling had been doing two jobs: bounding the proof's DAG and bounding
per-step CNF expansion. A long-but-honest proof was refused outright —
`SET017+1`'s 119 335 formulas drew `proof has 119335 formulas, exceeding limit
100000` — which reads in a report exactly like the checker giving up.
`VerificationLimits::max_proof_nodes` (1 000 000) now bounds the input DAG and
`max_nodes` stays at 100 000 for expansion. This fix is included on `main`.

Re-auditing all 215 archived refutations of this run on a 2-core dev box,
`--strict-time 120`, `--jobs 1`:

| | 2026-09-23 kernel | before proof-node limit fix | after proof-node limit fix, pre-target-rigidity kernel |
|---|---:|---:|---:|
| `VerifiedGood` | 152 | 189 | **193** (89.8 %) |
| `Unknown` | 57 | 25 | **22** |
| `VerifiedBad` | 5 | 0 | **0** |
| killed (wall clock) | 0 | 1 | **0** |

The four that moved in that audit were exactly the three node-limit rows plus
the killed one. **This is a historical result from the pre-target-rigidity
kernel and is superseded for `SWV406+1`:** its archived proof contains
superposition node `c502`, which the current kernel rejects because it
specializes a target-clause variable. The 98.9 s / 4.8 GB run established only
that the older kernel accepted the archived artifact; it is not current
certification evidence. `MGT079+1` (227 551 formulas) certified in 26.4 s /
970 MB, `SET017+1` (119 335) in 14.4 s / 660 MB, and `KRS234+1` (103 464) in
13.8 s / 531 MB in that historical audit; those verdicts also refer to the
kernel revision used for that audit.
Cost of the whole re-audit: **310 s wall, 4.77 GB peak RSS** — re-auditing an
archived run is not remote work.

Reasons for the residual 22 (one row per proof):

| count | reason | status |
|---:|---|---|
| 10 | `demodulation` node exceeds the strict rewrite-step limit | **fixed, awaiting re-measurement** |
| 5 | `demodulation` replay could not reach the conclusion within the implemented search | **fixed, awaiting re-measurement** |
| 3 | `demodulation` intermediate clause exceeds the strict size bound | **fixed, awaiting re-measurement** |
| 3 | a `fof_nnf` step whose parent's NNF exceeds the 100 000-node budget (`BIO006+1`, `CSR115+8`, `CSR116+19`) | open, deliberate |
| 1 | `ac_superposition` replay incomplete | **AC matcher defect identified; producer/replay correction and coverage recovery open** |

**Update, 2026-10-15:** restoring ordinary syntactic superposition replay while
retaining restricted AC replay certifies 91 of the old campaign's 114 UEQ
proofs, rejects zero, and leaves 23 inconclusive. The previous 1/82/31 audit
below describes the over-restrictive target-rigid checker, not the repaired
checker. The c26/c502 and small lattice-shaped refusal tests encoded valid
ordinary instances and have been corrected. See the
[recovery report](reports/benchmarks/ueq-soundness-regression-2026-10.md).
The historical fallback diagnosis below is not established solely by observing
target bindings or unequal pre-substitution flattened arities.

The `ac_superposition` row is the one `Unknown` in that table. The archived
replay finding was incomplete: reproducing a conclusion does not validate the
substitution, and target-variable binding alone is not a superposition
soundness violation. Superposition standardizes parents apart, applies a genuine
unifier to both parents, and emits the corresponding resolvent. The old
assoc/comm fallback could instead return a positional substitution after
flattened arities differed without establishing that it unified the terms.
`unify_ac_rigid_id` prevents those matches but also excludes valid standardized-
apart MGUs, so target rigidity is a current completeness restriction, not the
formal soundness criterion. The strict checker must validate the actual unifier
and full resolvent independently.

Two consequences for this file's numbers, stated plainly:

- **The archived old proofs fail current replay.** `VerifiedBad` means the
  archived derivation does not check; it does not classify each failure's
  specific matcher defect. A kernel must not accept a candidate merely because
  the producer emitted it.
- **The archived figures do not change; their interpretation does.** A current
  strict re-audit of the 114 archived CASC-J13 UEQ proofs gives 1
  `VerifiedGood`, 82 `VerifiedBad`, and 31 `Unknown`. This is not a sound 114
  solve baseline, nor does it show that all rejected proofs fail for the same
  reason. Only fresh inference generation measures recovered coverage. Full
  details and the corrected unifier/replay requirements are in
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

### CASC-J13 combined campaign (2026-10-16)

The combined run
`~/crates/mrs-bench/results/campaign-casc-j13-W8C8J1-20261016` covered all 800
problems in FEQ, FNE, and UEQ, sequentially with eight workers, `MRS_HARDWARE=casc`,
and official 180-second limits. Its strict audit certified **221/800**:
**FEQ 72/300, FNE 36/100, and UEQ 113/400**. The 114 UEQ refutations included
113 `VerifiedGood` and one `Unknown` (`RNG009-7`, the unresolved AC-superposition
replay issue); no proof was `VerifiedBad`. Other run outcomes: 505 `Timeout` and
73 `GaveUp`, with zero resource-outs, errors, or `Other` statuses.

This full-division measurement is CASC-mode shaped: eight workers on a host with
16 physical cores and 128,019 MB RAM. It is still a single run, and its dirty
tree/build identity is pinned by binary SHA-256
`b551916d3d379087fdb99ef0486d7fc98430e54146e8943960346a662d4a5069` (recorded
commit `dfef326c`, `git_dirty=true`). It should not be treated as a reproducible
score delta against runs with different binaries or hardware modes.

Against the closest standalone measurements, the combined campaign certified
two additional UEQ problems net (109 common, four combined-only, two
recovery-only), five additional FNE problems (31 common, five combined-only),
and one additional FEQ problem net (70 common, two combined-only, one
standalone-only). The FEQ standalone run was adaptive mode; the FNE run was an
earlier adaptive run; the UEQ recovery run used another CASC-mode binary and
host. These differences belong in the intermittency inventory, not as causal
claims about the source change.

The standalone FEQ run `campaign-cascj13-feq-W8C8J1-20261016` is a distinct
measurement: it certified 71/300 under adaptive mode and a much smaller memory
budget. See [`results/README.md`](results/README.md) for its comparison and the
per-problem [`intermittency inventory`](results/intermittency.md).

### FEQ CASC-J13 standalone campaign (2026-10-16)

The standalone run
`~/crates/mrs-bench/results/campaign-cascj13-feq-W8C8J1-20261016` completed
all 300 FEQ problems and generated **71 theorems**. Strict replay certified
**all 71 (`VerifiedGood`), with 0 `VerifiedBad`, 0 `Unknown`, and 0 errors**.
The other outcomes were 184 `Timeout`, 42 `GaveUp`, and three `ResourceOut`.
The resource-outs were `CSR036+5`, `CSR050+5`, and `CSR066+5`, each with
540,249 axioms; they reached adaptive per-run memory limits around 44 GB.

This run is not a canonical CASC-hardware score despite its directory name:
`run_meta.json` has no `MRS_HARDWARE=casc`, and per-problem hardware lines say
`adaptive` on a host with 16 physical cores and about 44 GB per-run memory. It
used eight workers and `jobs=1`, but its 71/300 result should be read as an
adaptive-host measurement. The same-day combined FEQ/FNE/UEQ campaign ran in
`casc` mode with eight workers and a 128-GB host; its FEQ slice produced 72
theorems, all `VerifiedGood`. The standalone and combined campaigns share 70
FEQ solves: the combined run additionally solved `GEO277+1` and `SWB073+1`,
while the standalone run additionally solved `SWW298+1`. These are separate
single measurements with different binaries and hardware configurations, not a
controlled A/B comparison.

The standalone metadata records commit
`abeda8c53aab2d96e7f42c10969bb569e45e1646` with a dirty tree, binary SHA-256
`76ee80a58a71dd0e2956df70dca8ed35687e21861a115f96c3ffa4474ccbc8f1`, and
Rust 1.99.1. The harness host had 63,763 MB RAM; adaptive per-problem budgets
were approximately 44 GB. The official FEQ wall clock was 180 seconds; the
metadata's 120-second default was overridden by `use_casc_times=1`.

The fresh standalone audit also certifies `GEO111+1`; its `VerifiedBad` in the
October 15 archived audit was a kernel comparison false rejection, as described
under UI-4 below. The old audit artifact remains a historical record.

### CASC-30 combined FEQ/UEQ campaign (2026-10-16)

The 700-problem run
`~/crates/mrs-bench/results/campaign-casc30-ueq-feq-W8CASCHJ1-20261016` used
the CASC hardware profile, eight workers, `jobs=1`, and official 240-second
division limits. The strict audit certifies **105 FEQ proofs** and **118 UEQ
proofs**. The other emitted results require care: FEQ has 2 `VerifiedBad`, 1
`Unknown`, and 1 verifier `Timeout`; UEQ has 5 `VerifiedBad` and 1 malformed
proof artifact. Thus this is 223 certified proofs, not the 233 raw refutations.

The FEQ rejections are `KLE071+1` (inconsistent symbol arity) and `SWV486+2`
(invalid `ac_superposition` rewrite). `LCL894+1` remains inconclusive at AC
superposition replay; `ALG049+1` exceeded the strict verifier's 120-second
budget. UEQ rejections are `MVA008-1` and `MVA009-1` (cited leaf mismatches),
`REL034-2` and `RNG009-5` (inconsistent symbol arities), and `RNG009-7` (cited
leaf mismatch). `REL045-2` has a refutation status but no `$false` proof root
and no explicit proof-omission marker; it is not certified.

Against the October 12 CASC-mode clean FEQ campaign, this run certifies 39
additional problems and loses 4, with 66 in common (70 → 105 certified).
Against the October 7 audited CASC-30 UEQ run, it gains 5 and loses 14, with
113 in common (127 → 118 certified). The latter result is a regression in
strictly certified coverage, and its five rejected proofs preclude treating
raw refutations as a sound score. These comparisons are observational across
different campaign builds, not a controlled A/B. See
[`results/README.md`](results/README.md) for the evidence, per-problem status
sets, and provenance.

Accordingly, keep **127/300** from the October 7 audited campaign as the
current trusted CASC-30 UEQ result. The October 16 result of 118
`VerifiedGood` is lower and carries five `VerifiedBad` proofs plus one
refutation without a proof root; it is not a replacement baseline. The five
rejected UEQ proofs span mismatched cited leaves (`MVA008-1`, `MVA009-1`,
`RNG009-7`) and contradictory symbol-arity signatures (`REL034-2`,
`RNG009-5`). The raw 124 UEQ refutations must not be reported as a sound solve
count until those derivations are resolved and the archived proofs pass strict
re-audit. None of these rows stopped at a resource limit.

The host had 95,969 MB RAM, below the 128-GiB CASC allowance; the effective
budget reported by the prover was 81,897 MB. No run ended for a resource limit.
The highest observed per-problem peaks were 64,650 MB (`CSR037+5`, FEQ) and
61,248 MB (`ROB005-1`, UEQ). Metadata records dirty commit `dfef326c`, Rust
1.99.1, and binary SHA-256
`7c9230d46623933a5ab7885d0e42bc6c9b2c02ddd4021af064da1e22bb39eb4f`.

The stricter FEQ and UEQ comparisons are recorded per problem in the
[`intermittency inventory`](results/intermittency.md). FEQ's 105 certified
proofs are 35 above the October 12 clean campaign; UEQ's 118 are nine below the
October 7 audited run. Because the latter has five `VerifiedBad` proofs, treat
the October 16 UEQ result as a proof-generation/audit regression to investigate,
not a new score baseline.

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

### Fresh CASC-30 EPS campaign (2026-10-19)

The full-division run
`~/crates/mrs-bench/results/campaign-casc30-eps-W8CASCHJ1-01-20261019`
returned 44 `Satisfiable` answers on 100 problems. The strict model audit
certified **40**, found **0 invalid**, and marked **4 inconclusive**; the other
56 problems were 10 `GaveUp` and 46 `Timeout`. The inconclusive models are
`NLP115-1`, `NLP118-1`, `NLP120-1`, and `NLP123-1`—the same four that remained
inconclusive in the September 28 audit.

Against `cert-eps-20260928`, all 38 previously certified models remain
`VerifiedGood`, with two added: **`SYN441-1` and `SYN529-1`**. There are no
certified-model losses. This moves both raw `Satisfiable` coverage and certified
model coverage from 42/38 to 44/40. Both new model rows were present as
inconclusive model answers in September; the change is certification recovery
as well as solve coverage. Against `cert-eps-20260926`, all four previously
certified models remain certified and 36 more are added. This is a comparison
of archived campaigns, not a matched A/B.

The run used `MRS_HARDWARE=casc`, eight workers in the certified/portfolio
7+1 split, `jobs=1`, and the official 120-second EPS limit. The host had eight
physical cores and 96,350 MB RAM, less than the nominal 128-GiB CASC allowance;
per-problem logs report an effective memory budget of 81,897 MB. The maximum
observed row peak was 33,731 MB (`SYN888-1`), with no resource-limit result.
For `SYN441-1` and `SYN529-1`, the portfolio worker was terminated at the
118-second division budget after the certified worker had already returned a
model. Both model certificates passed the strict checker; this is not a failed
or invalid answer.

Provenance: commit `40278337`, dirty tree, Rust 1.99.1, binary SHA-256
`a2f8178145f688bdf1deeb47648938d88d7d0039a19d8754237901aa458b046d`. Full
row-level outcome and comparison detail is in
[`results/README.md`](results/README.md), and the certified-model signatures
are in [`results/intermittency.md`](results/intermittency.md).

### Fresh CASC-30 EPU campaign (2026-10-19)

The full 100-problem run
`~/crates/mrs-bench/results/campaign-casc30-epu-W8CASCHJ1-02-20261019`
returned 11 refutations, all **strict `VerifiedGood`**, with no `VerifiedBad`,
`Unknown`, or proof-audit timeout. The other rows were 27 `GaveUp` and 62
`Timeout`. Compared with both the September 26 certified run and the October 2
EPU campaign, the certified set retains all nine existing solves and adds
`HWV078-1` and `MSC024-1` (+2, no regressions). This is one full-division
measurement, not a replicated coverage estimate or a matched grounding A/B.

Both new refutations used the `epr_equality` route. `HWV078-1` emitted a
52,857-node, 13,382,823-byte proof via `derivation_loop` in 53.4 s;
`MSC024-1` emitted a 26,189-node, 8,057,596-byte proof via `lrat_lift` in 4.5 s.
The larger proof exceeds the old 8-MiB output cap but fits the current 32-MiB
limit and strictly replays. These outcomes address the previously observed
`proof_extraction_failed` cases; see UI-5 for the earlier diagnosis and the
paired EPU measurements.

The CASC-mode host had eight physical cores and 95,979 MB RAM, below the
128-GiB allowance; effective memory was 81,582 MB. Peak observed memory was
25,692 MB (`HWV081-1`), with no resource-limit exit. Provenance: commit
`40278337`, dirty tree, Rust 1.99.1, binary SHA-256
`947aba9b604307f33e86d595b6112def78b42925f3a6ebeefac114aaf7cb9c96`.

This establishes an observed increase from 9 to 11 strict-certified EPU solves
against the two archived CASC-shaped runs; both new solves are `HWV078-1` and
`MSC024-1`, and all prior nine remain. It does **not** yet establish a replicated
rate gain attributable to the EPR grounding route: this is one post-change run,
not a paired off/on measurement with the same build and environment. Repeat
matched grounding-off/on CASC-mode full-division runs before making that causal
claim.

### Fresh CASC-30 ICU campaign (2026-10-19)

The full 101-problem run
`~/crates/mrs-bench/results/campaign-casc30-icu-W8CASCHJ1-97-20261019`
returned 4 refutations, all **strict `VerifiedGood`**, with zero `VerifiedBad`,
`Unknown`, or checker timeouts. The remaining rows were 18 `GaveUp`, 78 `Timeout`,
and 1 `Error` (`CSI008+1`, killed by OS OOM killer on a 2,004,866-axiom input).

Compared with `campaign-casc30-icu-W8C8J1-20261002`, the strict certification
rate on refutations rose from 3/5 (60.0%) to **4/4 (100.0%)**. Both `CSE007+1`
and `EEE001+1`—which previously returned `Unknown` under strict formula-size
limits on NNF and negated conjecture expansion—certified cleanly (`VerifiedGood`
in 1.92 s and 0.49 s, respectively) following commit `13653b9`. `EEE009+1`
(certified in October 2 in 45.7 s) timed out at 480 s on the 1.80 GHz host,
establishing it as an intermittent solve under parallel 8-worker search.
`CSI007+1` and `EEE007+1` certified in both campaigns.

Because 44 of the 101 ICU problems carry non-decisive reference statuses
(`expected=GaveUp`), `casc.sh` grades `CSE007+1` as `verdict=unknown` despite its
verified proof. On decisive references alone (`expected=Theorem`), the score is
3/55 (5.5%).

The CASC-mode host had 16 physical cores and 63,763 MB RAM, below the nominal
128-GiB CASC allowance; effective memory was 54,198 MB. Peak observed memory
among completed jobs was 53,772 MB (`EEE009+1`). Provenance: commit `40278337`,
dirty tree, Rust 1.99.1, binary SHA-256
`2982908b6da78fd0c791a171f4e47af6207c93b80512befcae271caf46537865`. Full
row-level outcome detail is in [`results/README.md`](results/README.md), the
[ICU certification report](reports/benchmarks/icu-2026-10.md), and the
[`intermittency inventory`](results/intermittency.md).

### Fresh CASC-30 FEQ campaigns (2026-10-20 and 2026-10-21)

Two consecutive full 400-problem runs were evaluated on canonical 8-core hardware (`mtsdev01`):
- `~/crates/mrs-bench/results/campaign-casc30-feq-W8CASCHJ1-01-20261020` (Oct 20)
- `~/crates/mrs-bench/results/campaign-casc30-feq-W8CASCHJ1-01-20261021` (Oct 21)

Both runs produced identical certification outcomes: exactly **108 refutations (`Theorem`)**,
**103 `VerifiedGood` (95.4%)**, **0 `VerifiedBad` (0.0% unsoundness)**, 2 `Unknown`,
1 verifier `Timeout`, and 2 `Proof Omitted`. Oct 20 returned 249 `Timeout` and 43 `GaveUp`;
Oct 21 returned 244 `Timeout` and 48 `GaveUp`. Zero process errors, crashes, or silent kills
occurred in either campaign.

Compared with `campaign-casc30-ueq-feq-W8CASCHJ1-20261016`, strict unsoundness dropped from
2 `VerifiedBad` to **0**: both `KLE071+1` (fixed symbol signature arity) and `SWV486+2` (fixed AC
superposition validation) certify strictly as `VerifiedGood` across both runs. Two residual
proofs remain `Unknown` at incomplete AC superposition replay (`KLE132+1` and `LCL894+1`),
`ALG049+1` timed out in the verifier (>120 s) on an 8.2k-node proof in both runs, and two
refutations (`SET017+1` and `SET072+1`) were withheld exceeding the 32 MB proof limit.

Across the two runs, 107 of 108 solved problems are identical. Exactly one borderline problem
pair swapped near the 240 s horizon due to thread racing under 8-worker concurrency: `SWV458+1`
solved in 28.4 s on Oct 20 (`VerifiedGood`) but timed out on Oct 21; `SWV457+1` timed out on Oct 20
but solved in 153.4 s on Oct 21 (`VerifiedGood`).

Both runs establish production confirmation of the quadratic SOS pre-flight fix (UI-10 / UI-2)
on 96 GB hardware: previously hung problems terminated cleanly, with `BIO005+1` and `BIO006+1`
both solving in ~157 s and strictly certifying in both runs.

Hardware: `mtsdev01` (8 physical cores, 96,350 MB RAM, effective memory ceiling 81,897 MB).
Peak observed memory was 64,651 MB (`CSR037+5`). Provenance: commit `a38c8bc8` (binary SHA-256
`8029b1d6c1fee6b5b533365823755a659bca31eb33e9d4e1e92e3e23cf48f762`) on Oct 20; commit
`c45227ff` (binary SHA-256 `fd60bd269efe77c249ba2531605b6223fed510df275e0446008c9cf0fd106d19`)
on Oct 21. Detailed telemetry and domain breakdowns are in the
[FEQ campaign report](reports/benchmarks/feq-2026-10.md) and
[`results/README.md`](results/README.md).

### Fresh CASC-30 UEQ campaigns (2026-10-20 and 2026-10-21)

Two consecutive full 300-problem runs were evaluated on canonical 8-core hardware (`mtsdev02`):
- `~/crates/mrs-bench/results/campaign-casc30-ueq-W8CASCHJ1-02-20261020` (Oct 20)
  returned 124 refutations (`Unsatisfiable`), 167 `Timeout`, and 9 `GaveUp`.
  Strict audit certified **120 `VerifiedGood` (96.8%)**, with **0 `VerifiedBad`**,
  4 `Unknown`, 0 checker timeouts, and 0 omitted proofs.
- `~/crates/mrs-bench/results/campaign-casc30-ueq-W8CASCHJ1-02-20261021` (Oct 21)
  returned 123 refutations (`Unsatisfiable`), 166 `Timeout`, and 11 `GaveUp`.
  Strict audit certified **119 `VerifiedGood` (96.8%)**, with **0 `VerifiedBad`**,
  4 `Unknown`, 0 checker timeouts, and 0 omitted proofs.

Both runs confirm zero unsoundness (`VerifiedBad = 0`) across 247 total evaluated refutations,
reproducing the complete resolution of the 5 false rejections and 1 missing root from the
2026-10-16 campaign:
- `MVA008-1` and `MVA009-1` (leaf matching with residual axioms) certify strictly as `VerifiedGood` in both runs.
- `REL034-2` (symbol arity conflict) certified strictly as `VerifiedGood` on Oct 20 (timed out at 238.8 s on Oct 21).
- `REL045-2` (previously omitted proof root) certifies strictly as `VerifiedGood` in both runs.
- `RNG009-5` and `RNG009-7` properly fail closed as `Unknown` due to incomplete AC rewrite replay in both runs.

Together with `GRP179-2` and `REL027-3`, exactly the same 4 residual proofs remain `Unknown`
due to incomplete AC superposition rewrite replay across both campaigns. Across both runs, 123 of 124
refutations re-solved identically (>99% replicate consistency).

Hardware: `mtsdev02` (8 physical cores, 95,979 MB RAM, effective memory ceiling 81,897 MB).
Peak observed memory among completed jobs was 63,428 MB on Oct 20 and 67,525 MB on Oct 21
(both on `ROB005-1`). Provenance: commit `a38c8bc8` (binary SHA-256
`21e72729ecf09c86b78f35caa56d91a9b06c969771988fac806dcc68d3987ecb`) on Oct 20; commit
`c45227ff` (binary SHA-256 `eb9a70780d9249618e62468501ebc56bce280f53a0051570fde2f0e0d2a99435`)
on Oct 21. Detailed telemetry and domain breakdowns are in the
[UEQ campaign report](reports/benchmarks/ueq-2026-10.md) and
[`results/README.md`](results/README.md).

### Fresh CASC-J13 FEQ standalone campaigns (2026-10-20 and 2026-10-21)

Two consecutive full 300-problem runs were evaluated on canonical 8-core hardware (`mtsdev03`):
- `~/crates/mrs-bench/results/campaign-casc-j13-03-feq-CASCHJ1-20261020` (Oct 20)
  returned 68 refutations (`Theorem`), 193 `Timeout`, and 39 `GaveUp`.
  Strict audit certified **all 68 (`VerifiedGood`, 100.0%)**, with **0 `VerifiedBad`**,
  **0 `Unknown`**, 0 checker timeouts, and 0 omitted proofs.
- `~/crates/mrs-bench/results/campaign-casc-j13-03-feq-CASCHJ1-20261021` (Oct 21)
  returned 69 refutations (`Theorem`), 194 `Timeout`, and 37 `GaveUp`.
  Strict audit certified **68 `VerifiedGood` (98.6%)**, **0 `VerifiedBad`**,
  **0 `Unknown`**, 0 checker timeouts, and 1 `Proof Omitted` (`SWV406+1`, 83.4 MB proof).

Both standalone runs used canonical CASC constraints: eight workers, `MRS_HARDWARE=casc`,
`jobs=1`, and the official 180-second clock on the physical 8-core Xeon E5-2407 host `mtsdev03`.
Across both runs, 100% of emitted proofs certified strictly as `VerifiedGood`. All 68
`VerifiedGood` refutations from Oct 20 re-solved and re-certified on Oct 21, including
`MGT091+1` (89.96 s on Oct 20; 89.28 s on Oct 21) and `SET101+1` (0.44 s both runs).
The 69th solve on Oct 21, `SWV406+1` (162.75 s wall time), derived an 83.4 MB proof (319k nodes)
withheld under the 32 MB `--proof-bytes-limit`.

Hardware: `mtsdev03` (8 physical cores, 95,969 MB RAM, effective memory ceiling 81,897 MB).
Peak observed memory was 65,040 MB on Oct 20 (`CSR025+5`) and 64,650 MB on Oct 21 (`CSR036+5`).
Provenance: commit `a38c8bc8` (binary SHA-256 `f1c84643ab459fca2f4b29b8def0fbbd9418559b271eadb178aedcf6e5a65bc5`)
on Oct 20; commit `c45227ff` (binary SHA-256 `53e2c5a6071ddd912b65e497ca455effb5e39ea813085f25a3d4a0708560effc`)
on Oct 21. Full details are in the [CASC-J13 FEQ report](reports/benchmarks/feq-j13-2026-10.md)
and [`results/README.md`](results/README.md).

### Fresh CASC-J13 UEQ standalone campaigns (2026-10-20 and 2026-10-21)

Two consecutive full 400-problem runs were evaluated on canonical 8-core hardware (`mtsdev04`):
- `~/crates/mrs-bench/results/campaign-casc-j13-04-ueq-CASCHJ1-20261020` (Oct 20)
  returned 120 refutations (`Unsatisfiable`), 266 `Timeout`, and 14 `GaveUp`.
  Initial audit certified 112 `VerifiedGood`, 6 `VerifiedBad` (leaf equality orientation flips),
  and 2 `Unknown`. Following the kernel symmetry fix (`b1033d3`), post-hoc re-audit certified
  118 of 120 (0 `VerifiedBad`).
- `~/crates/mrs-bench/results/campaign-casc-j13-04-ueq-CASCHJ1-20261021` (Oct 21)
  returned 119 refutations (`Unsatisfiable`), 267 `Timeout`, and 14 `GaveUp`.
  Strict audit certified **117 `VerifiedGood` (98.32%)**, **0 `VerifiedBad` (0.0% unsoundness)**,
  2 `Unknown`, 0 checker timeouts, and 0 omitted proofs.

Crucially, the Oct 21 campaign was executed with the `b1033d3` kernel fix integrated into the binary,
confirming natively in live production that all six previously rejected leaf symmetry problems
(`GRP178-1`, `GRP183-3`, `GRP186-1`, `REL026-3`, `REL041-2`, `REL045-2`) certify cleanly as
`VerifiedGood` with zero false rejections. Additionally, `REL027-3` solved in 84.78 s on Oct 21
(timed out on Oct 20) and certified as `VerifiedGood`, while `ALG240-1` and `NUN133-1` timed out
near the 180 s limit.

Across both runs, exactly the same two `Unknown` proofs (`RNG009-5` and `RNG009-7`) remain incomplete
at AC superposition rewrite replay.

Hardware: `mtsdev04` (8 physical cores, 95,969 MB RAM, effective memory ceiling 81,897 MB).
Peak observed memory was 57,732 MB (`ROB015-10`) in both campaigns. Provenance: commit `a38c8bc8`
(binary SHA-256 `c881d56f47b848d507017f4f3a4cb58fa24da0c4e3d8e267c45bc6c0d4e7ae94`) on Oct 20;
commit `c45227ff` (binary SHA-256 `088f4b1e613660d094e20185b0dd23e3ce73f70c7fdaee710b4e91cc6cf5da63`)
on Oct 21. Detailed telemetry and domain breakdown are in the
[CASC-J13 UEQ report](reports/benchmarks/ueq-j13-2026-10.md) and
[`results/README.md`](results/README.md).

### Cooperative FNE candidate portfolio campaigns (2026-10-21 and 2026-10-22)

The candidate 8-strategy FNE portfolio `MRS_PORTFOLIO=11,13,4,8,15,7,3,1` (derived
from full-time solo sweeps and greedy set cover to promote Strategy 13 and eliminate
redundant Strategies 10 and 12) was validated cooperatively with 8 workers and no
shared equality pool across three full-division campaigns:

1. **CASC-30 (`casc-30/coop-fne-candidate-20261021`)**:
   Evaluated on `teenf9901` at 240 s limit. Generated **45 refutations (`Theorem`)**,
   all **45 certified strictly as `VerifiedGood` (100.0%)**, with **0 `VerifiedBad`**
   and **0 `Unknown`**. Retains 100% of the 41 solves from the shipped baseline
   (`campaign-casc30-fne-W8CASCHJ1-99-20261019`) and adds **+4 newly solved theorems**
   unlocked by Strategy 13: `LCL370+2` (61.0 s), `LCL373+2` (66.6 s), `LCL390+2` (91.7 s),
   and `LCL403+2` (92.7 s).

2. **CASC-J13 on Canonical 8-Core Hardware (`casc-j13/coop-fne-candidate-03-20261022`)**:
   Evaluated on `mtsdev03` (8 physical cores, 96 GB RAM) at official 180 s limit.
   Generated **40 refutations (`Theorem`)**, up from 36 in the shipped baseline.
   Strict audit (with HEAD) certified **39 `VerifiedGood`**, **0 `VerifiedBad`**,
   **0 `Unknown`**, and **1 `Proof Omitted`** (`KRS261+1`, 55.6 MB proof exceeding 32 MB limit).
   Achieves +4 refutations / +3 verified over the shipped baseline (36).

3. **CASC-J13 Comparator (`casc-j13/coop-fne-candidate-20261021`)**:
   Evaluated on `tlpnf9701` at 180 s limit. Generated **38 refutations (`Theorem`)**,
   all **38 certified strictly as `VerifiedGood` (100.0%)**, with **0 `VerifiedBad`**
   and **0 `Unknown`**.

Across all three campaigns, 100% of emitted refutations passed strict kernel verification
without a single unsound inference. Full details are in the
[FNE candidate portfolio report](reports/benchmarks/fne-candidate-portfolio-2026-10.md) and
[`results/README.md`](results/README.md).

### Cooperative MultiQueue hybrid FNE campaigns (2026-10-22)

The hybrid portfolio `MRS_SCHEDULE=casc_fne_mq_hybrid` (`[11, 13, 4, 15]` base strategies
combined with `[mq_0, mq_1, mq_2, mq_5]` multi-queue archetypes) was evaluated with
8 workers and no shared equality pool across three full-division campaigns:

1. **CASC-30 (`cooperative-fne-mq-hybrid-no-sharing`)**:
   Evaluated on `teenf9901` at 240 s limit. Generated **45 refutations (`Theorem`)**.
   Strict audit (with HEAD) certified **42 `VerifiedGood` (93.3%)**, **0 `VerifiedBad`**,
   **2 `Unknown`** (`LCL642+1.015`, `LCL660+1.015` on factoring step limit), and
   **1 `Proof Omitted`** (`KRS261+1`, 62.8 MB proof exceeding 32 MB limit).

2. **CASC-J13 on Canonical 8-Core Hardware (`cooperative-fne-j13-04-mq-hybrid-no-sharing`)**:
   Evaluated on `mtsdev04` (8 physical cores, 96 GB RAM) at official 180 s limit.
   Generated **37 refutations (`Theorem`)**, all **37 certified strictly as `VerifiedGood` (100.0%)**,
   with **0 `VerifiedBad`** and **0 `Unknown`**.

3. **CASC-J13 on 32-Core Host (`cooperative-fne-j13-mq-hybrid-no-sharing`)**:
   Evaluated on `tlpnf9701` at 180 s limit. Generated **41 refutations (`Theorem`)**,
   all **41 certified strictly as `VerifiedGood` (100.0%)**, with **0 `VerifiedBad`**
   and **0 `Unknown`**.

Comparing against the pure base candidate portfolio on canonical 8-core hardware shows that
while MultiQueue selection scales well on uncongested 32-core systems, the pure base candidate
portfolio achieves higher certified solve counts on 8 cores (39 vs 37 on J13; 45 vs 42 on C30)
with cleaner kernel replay. Full details are in the
[FNE MultiQueue report](reports/benchmarks/fne-multiqueue-2026-10.md).

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
certificates. `--proof-bytes-limit` (default 32 MiB) omits an over-budget proof
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
