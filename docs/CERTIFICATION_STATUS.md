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

215 refutations, re-verified at HEAD on a 4-core box:

| strict verdict | 2026-09-23 (build of that day) | HEAD |
|---|---:|---:|
| `Certified` | 152 | **192** (89.3 %) |
| `Unknown` | 57 | 22 |
| `VerifiedBad` | 5 | **0** |
| killed (wall clock) | 0 | 1 |

The 2026-09-23 column is what the run's own audit recorded. The five
kernel-side rejections it reported (CSR117+1, GEO111+1, MGT005+1, SEV606+1,
SWX217+1 — equality resolution, condensation, subsumption resolution and
superposition shapes) are all fixed at HEAD; SWX217+1 had also been reported
`VerifiedBad` by the ATP ladder, and now passes both.

### The residual 23, and what closes them

| count | reason | status |
|---:|---|---|
| 18 | `demodulation` steps the kernel could not replay | **fixed, awaiting re-measurement** |
| 3 | proof exceeds the kernel's 100 000-formula limit | open |
| 1 | `ac_superposition` replay incomplete | open |
| 1 | killed on wall clock (`feq/SWV406+1`, 170 MB proof) | open |

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
path and the 192/215 figure is the *fallback* number. The post-change figure
needs a fresh run on 8-core hardware:

```bash
MRS_WORKERS=8 crates/mrs-bench/certification_campaign.sh \
    --edition casc-j13 --systems mrs --divisions fne,feq,ueq \
    --casc-times --jobs 2 --output crates/mrs-bench/results/cert-j13-$(date +%Y%m%d)
```

Expect the 18 demodulation rows to move to `Certified`; if they do not, the
reason histogram names the shape that still fails.

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
`% SZS detail` telemetry. Full analysis and the reduction options:
`docs/PROOF_SIZE_BUDGET.md`.

---

## 5. Superseded claims

Corrected here so they are not read as current:

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
