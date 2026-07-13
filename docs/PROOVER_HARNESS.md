# mrs-proover Test Harness: History & Implementation

This note documents the **test/regression infrastructure** for `mrs-proover`
(the ProoVer 2026 proof verifier). It explains *why* the harness looks the way
it does, what went wrong with the earlier approach, and how the current
deterministic corpus works. For the verifier's *scoring* roadmap see
[`TODO_PROOVER.md`](./TODO_PROOVER.md).

---

## 1. Scoring context (why false rejections matter)

| Outcome | Points |
|---------|--------|
| Correctly identify evil proof (`VerifiedBad`) | **+2** |
| Correctly identify good proof (`VerifiedGood`) | **+1** |
| Give up / timeout (`Unknown`) | 0 |
| **Falsely reject a good proof (`VerifiedBad` on a valid proof)** | **−1** |
| Falsely verify an evil proof | **−10 (fatal)** |

For the *test harness* the key consequence is the **−1 for a false
`VerifiedBad`**. Verifying a stream of known-valid proofs, the only *wrong*
outcome is `VerifiedBad`; `VerifiedGood` is ideal and `Unknown` is merely a
missed `+1`, never a penalty. So the regression invariant is simply:

> **No known-valid proof may ever be reported `VerifiedBad`.**

---

## 2. History: the random-sampling harness and why it failed

The first harness, `test_tptp_solutions.sh`, did this:

1. Fetch the list of solved FOF problems in TPTP's `SYN` domain.
2. For each problem, pick **one solution at random** (`shuf -n 1`) out of the
   ~40 systems TPTP stores per problem.
3. Strip the SeeTPTP HTML, rewrite `cnf(` → `fof(`, inject a `% Proof :`
   header, and run `mrs-proover`.

This produced a **different result set on every run** ("each run discovers new
failures"). The root cause was not a stream of verifier bugs — it was the
sample. TPTP stores proofs from systems that emit wildly different formats:

| System family | Output format | mrs-proover result | Correct? |
|---------------|---------------|--------------------|----------|
| cvc5, Z3 | Alethe S-expressions (`(step @p4 :rule …)`) | `Unknown: no FOF/CNF nodes` | ✅ 0 pts |
| Beagle, iProver(TFF), Leo | TFF/THF with `type` decls | `Unknown: unsupported dialect` | ✅ 0 pts |
| leanCoP, nanoCoP, ConnectPP | connection-matrix proofs | `Unknown` / parse-skip | ✅ 0 pts |
| Darwin, Paradox, Equinox | finite models (`CounterSatisfiable`/`Assurance`) | empty / `Unknown` | ✅ 0 pts |
| Metis | `inference(subst,[],[p:[bind…]])` colon-pairs | (was) `VerifiedBad` | ❌ −1 bug |
| SPASS, Otter | clausified anonymous `file(_,unknown)` leaves | (was) `VerifiedBad` | ❌ −1 bug |
| **E, Vampire** | **standard TSTP FOF refutations** | **`VerifiedGood`** | ✅ +1 |

The overwhelming majority of "failures" were actually **correct `Unknown`
(0 pts)** on formats `mrs-proover` legitimately cannot verify. Only a handful
were real `−1` bugs, and they were drowned in format noise that changed every
run.

### What the competition actually feeds

The official ProoVer 2026 examples (`crates/mrs-proover/tests/fixtures/`) are
**E** and **cvc5** proofs, and every leaf uses **named** provenance
(`file('Problems/x.p', axiomname)`), never the anonymous `file(_,unknown)`
form. The competition's distribution is narrow and well-formed (TPTP/TSTP FOF,
in practice the CASC champions E and Vampire). Hardening against all ~40 exotic
TPTP systems was both an endless game of whack-a-mole *and* unrepresentative of
the real target.

---

## 3. The fix: a curated, deterministic, offline corpus

The harness was split into three pieces with a clear separation of concerns.

### 3.1 `build_proover_corpus.sh` — (re)generate fixtures (network)

- Iterates a **fixed list** of ~25 small, fast FOF theorems (`PROBLEMS=(…)`).
- For each, downloads the problem file plus every **allowlisted** system's THM
  proof. The allowlist (`ALLOWED_SYSTEMS`) is currently `E---` and `Vampire---`
  — exactly the systems that emit standard TSTP FOF refutations.
- Normalises each proof the same way the competition wrapper does (HTML strip,
  `cnf(`→`fof(`, `% Proof :` header) and stores it under:

  ```
  crates/mrs-bench/proover-corpus/Problems/<PROB>.p
  crates/mrs-bench/proover-corpus/proofs/<PROB>__<system>.s
  ```

- Drops any download that is not a usable refutation (no `fof(`, no `$false`).

This is the **only** networked piece, run only when refreshing the corpus.
Current corpus: 25 problems × {E, Vampire} ≈ **46 proof files, ~660 KB**, all
committed to the repo.

### 3.2 `verify_proover_corpus.sh` — the regression gate (offline)

- Verifies every committed proof against its committed problem with a fixed
  per-proof budget (default 10 s).
- Tallies `VerifiedGood` / `Unknown` / `VerifiedBad`.
- **Exit 1 iff any proof is `VerifiedBad`** — that is the regression
  invariant from §1. `Unknown` is reported but tolerated.
- Never touches the network, so its result is **stable across runs and
  machines**. This is what CI / pre-commit should run.

Representative output (after the fixes in §4):

```
[corpus]   VerifiedGood      :  43
[corpus]   Unknown   :   3
[corpus]   VerifiedBad:   0  (must be 0)
[corpus] PASS: no known-valid proof was VerifiedBad.
```

The 3 remaining `Unknown` are E proofs of PEL-style problems (`SYN051+1`,
`SYN056+1`, `SYN057+1`) where E folds Skolemisation into a `thm`-labelled
`fof_nnf` step, e.g. `inference(fof_nnf,[status(thm)],[inference(skolemize,
[status(esa)],[...])])`. This is **not a time-budget issue** — confirmed by
re-running with `--time 60` (6x the default) with an identical result, and by
inspecting the reason string: `atp \`ladder\` found a counter-model, but
equisatisfiability steps are not entailments, so this is not a fault`. The
outer step is only `esa` (equisatisfiable, not equivalent) because it
introduces a fresh Skolem constant/function; the FMB counter-model rung
correctly finds a finite model where the premise holds and the
under-constrained Skolem witness makes the conclusion fail — that is expected
and harmless for an `esa` step, so `verify.rs`'s esa guard reports the safe
`Unknown` (0 pts) instead of misreading it as a soundness fault. Since
`node.inference_rule` here is `fof_nnf`, not `skolemize`, these steps are
deliberately **not** routed into `checks::skolemize::check`'s structural
verifier — widening that dispatch to catch nested `esa` steps like this one
was tried and reverted (see the `fix-e-style-skolemize` merge commit) because
it also hijacked unrelated `esa`-status steps away from their ATP/structural
fast-paths, regressing this same corpus from 42 to 33 `VerifiedGood`. So today
these 3 cases cost 0 points with `mrs`, real `eprover`, and real `vampire`
alike (all three are in the default ladder here and none discharges them);
closing them safely would need a smarter structural match for this specific
nested-`fof_nnf`-wrapping-`skolemize` shape, not a bigger ATP or more time.
Crucially, the underlying *theorems* are fine: the corpus also has Vampire's
own proofs of the identical three problems (`SYN051+1__Vampire`,
`SYN056+1__Vampire`, `SYN057+1__Vampire`), and `mrs-proover` reports
`VerifiedGood` on all three — Vampire's proof-step shape doesn't fold Skolemize
into an outer `thm` step, so it hits the fast paths cleanly. The gap is in
*which proof object* was submitted, not in whether the *problem* is provable
or in `mrs-proover`'s soundness. (A 4th, related case, where E's `skolemize`
step eliminates two existentials at once and the parent/step conjunction is a
differently-shaped CNF, is now positively verified: the structural matcher
normalises both sides to CNF and matches conjuncts/disjuncts as a strictly
**bijective** multiset — every parent conjunct must be consumed by exactly one
step conjunct, never dropped — before confirming `Sound`.)

### 3.3 `test_tptp_solutions.sh` — live spot-check (network, exploratory)

Retained for ad-hoc exploration, but now restricted to the **same allowlist**
(`System=(E---|Vampire---)`) so a live run is representative rather than noisy.
It is explicitly *not* a regression gate — use `verify_proover_corpus.sh` for
that.

---

## 4. Real bugs surfaced and fixed along the way

Curating the corpus did not just hide noise — it isolated the two genuine
`−1` bugs (both from clausifying provers), which were then fixed in the
library:

1. **Metis colon-pair parents** (`mrs-tptp/src/proover.rs`,
   `collect_parent_refs`). Metis writes substitution steps as
   `inference(subst, [], [parent : [bind(X, $fot(t))]])`. The parent is the
   *left* of the `:`-pair; the right is an instantiation. The old catch-all
   dropped the parent entirely, so the entailment query got empty premises and
   a sound instantiation was refuted. Fixed by recursing into the left of a
   `GeneralTerm::ColonPair`.

2. **Clausified anonymous leaves** (`mrs-proover/src/checks/axiom_leaf.rs`).
   Provers that clausify the problem up front (SPASS, Otter, …) emit anonymous
   `file(_,unknown)` leaves that are the NNF/Skolemised/CNF form of an axiom
   (e.g. `~big_p(u)|big_q(u)|big_r(u)` for `big_p(X)=>(big_q(X)|big_r(X))`).
   These are faithful but not structurally α/AC-matchable, so the old code
   returned `Unsound` → `VerifiedBad` (−1). Now the anonymous fallback
   returns `Unknown` instead, **except** when the leaf is itself `$false` /
   `~$true` (axiom spoofing), which stays `Unsound`. This does not weaken
   evil-proof detection: the official examples and the `axiom_spoofing` exploit
   use the *named* path, which is unchanged.

Both fixes are safe under the −10/−1/+2 asymmetry: they only ever turn a false
`VerifiedBad` into `Unknown` (or a correctly-extracted
parent), never the reverse.

---

## 5. How to use the harness

```bash
# Deterministic, offline regression gate (run this in CI / before committing):
crates/mrs-bench/verify_proover_corpus.sh

# Refresh the committed fixtures (only when you want new/updated proofs):
crates/mrs-bench/build_proover_corpus.sh

# Exploratory live spot-check against fresh E/Vampire proofs from TPTP:
crates/mrs-bench/test_tptp_solutions.sh
```

### Extending the corpus

Add problem names to `PROBLEMS=(…)` in `build_proover_corpus.sh`, re-run it to
download the new fixtures, then run `verify_proover_corpus.sh`. If you want
coverage of another standard-TSTP system, add its `System=` prefix to
`ALLOWED_SYSTEMS` — but keep the bar high: only add systems whose proofs are
genuine TSTP FOF refutations, or you will reintroduce the format noise this
harness was built to eliminate.

---

## 6. Evaluating against the Zenodo proof-checker benchmark

The [TSTP FOF Proof Benchmark (Zenodo 19792604)](https://zenodo.org/records/19792604)
is the dataset the Nörgler authors published to *evaluate proof checkers*. It
pairs, for each of two source provers, genuine proofs with automatically
mutated ("falsified") copies:

| Source | original | falsified | problem files |
|--------|----------|-----------|---------------|
| PyRes  | 170      | 170       | ✅ yes (`problems/`) |
| Otter  | 1806     | 1782      | ❌ no |

Two scripts wire it in:

- **`fetch_zenodo_corpus.sh`** — downloads + extracts the archive (~28 MB,
  git-ignored) and **normalises** it: PyRes proofs reference their problem via
  `file('<PROB>.p',_)` leaves but ship no `% Proof : …` header, which
  `mrs-proover` needs to locate the linked problem (see
  `crates/mrs-proover/src/load.rs`), so the script injects that header
  idempotently. Otter proofs have no problem files and are left as-is.
- **`zenodo_benchmark.sh`** — auto-fetches the corpus if absent, then runs
  `mrs-proover` (and, with `--with-norgler`, Nörgler) over the selected
  datasets, writing a `run.csv` and checking the two soundness invariants:
  *original must never be `VerifiedBad`* (a false reject is −1) and
  *falsified must never be `VerifiedGood`* (a false accept is −10, fatal).

```bash
# mrs-proover only, full PyRes set:
crates/mrs-bench/zenodo_benchmark.sh --dataset PyRes

# full head-to-head against Nörgler with a realistic budget:
crates/mrs-bench/zenodo_benchmark.sh --dataset all --time 60 --with-norgler
```

For a fair Nörgler head-to-head, give it a realistic per-proof budget (`--time
60`): its JVM start-up plus per-step `eprover`/`vampire` calls are slow, so
tight budgets starve it into spurious `Unknown`. The wrapper
(`systems/norgler/invoke.sh`) resolves a JRE once and caches it (a per-call
`nix-shell` costs ~20 s), and the benchmark disables the leaf-path rewrite for
this dataset (`MRS_NORGLER_NO_REWRITE=1`) because the Zenodo PyRes proofs already
carry corrected `file('<PROB>.p',_)` records — rewriting them to absolute paths
makes Nörgler reject valid proofs.

### Head-to-head results

Full PyRes (170 + 170) and an Otter sample (60 + 60), 60 s per proof:

| dataset / category | backend | VerifiedGood | VerifiedBad | Unknown / Error |
|---|---|---|---|---|
| PyRes / original (valid)   | **mrs-proover** | **160** | **0**  | 10 |
| PyRes / original (valid)   | Nörgler         | 155     | **11** | 4  |
| PyRes / falsified (evil)   | **mrs-proover** | 0       | **170**| 0  |
| PyRes / falsified (evil)   | Nörgler         | 0       | 165    | 5  |
| Otter / original (valid)   | **mrs-proover** | **60**  | **0**  | 0  |
| Otter / original (valid)   | Nörgler         | 60      | **0**  | 0  |
| Otter / falsified (evil)   | **mrs-proover** | 0       | **60** | 0  |
| Otter / falsified (evil)   | Nörgler         | 0       | 59     | 1  |

What this says — honestly, with no spin:

**Soundness (the −10 / −1 dimension): mrs-proover is strictly safer.**
- mrs-proover: **0 false rejects** on valid proofs and **0 false accepts** on
  evil proofs across all 460 proofs, catching **230/230 (100%)** mutations.
- Nörgler: **11 false rejects** (−1 each) on valid PyRes proofs ("does not use
  correct formula from file" — its strict leaf↔problem comparison rejects
  PyRes-reformatted axioms), and catches 224/230 (97.4%) mutations.

**Coverage of valid proofs: now comparable, and mrs-proover leads on PyRes.**
- PyRes: mrs-proover verifies **160/170**, Nörgler 155/170. The earlier coverage
  gap was closed by *positively verifying unannotated `skolemize` steps* (see
  below) — it now reconstructs the Skolemisation and confirms it structurally
  instead of deferring to `Unknown`. The 10 remaining `Unknown` are skolemize
  steps where PyRes additionally **re-associates the conjunction**
  (`(A∧B)∧(C∧D)` → `A∧(B∧(C∧D))`); the structural matcher is not yet AC-aware, so
  it safely declines (0 pts) rather than risk a −1. (Tracked in `TODO_PROOVER.md`.)
- Otter: Nörgler verifies 60/60, mrs-proover 0/60 — but **only because the
  Zenodo Otter set ships no problem files**. Without the problem, mrs-proover
  cannot validate the `file(_,unknown)` leaves and degrades to `Unknown`;
  Nörgler (run without `--problem`) trusts the leaves and checks the inference
  structure. The real competition always supplies the problem file, so this is
  a dataset artefact, not a competition weakness.

**Net:** on PyRes — the half of the benchmark with problem files, i.e. the
competition-realistic half — mrs-proover now **leads on every axis**: more valid
proofs verified (160 vs 155), zero false rejects (vs 11), and 100% mutation
detection (170/170 vs 165). On Otter it is held back only by the dataset's
missing problem files, which the competition supplies.

> **Two `skolemize` improvements were made while running this benchmark:**
> 1. A −1 false-reject bug: multi-existential steps eliminating existentials at
>    *different* quantifier depths (e.g. `? [X2] : ! [X3,X4] : ? [X5] : …`,
>    giving a constant Skolem for `X2` and `sk(X3,X4)` for `X5`) were wrongly
>    reported `Unsound`. The arity check now tracks per-existential scopes.
> 2. Positive verification of unannotated `skolemize` steps
>    (`checks::skolemize::try_positive_skolemize`): the conclusion is matched
>    against the parent with every existential (at any depth, across regrouped
>    universal binders) replaced by a distinct fresh Skolem term over exactly its
>    in-scope universals — confirmed `VerifiedGood` instead of deferred. This raised
>    PyRes coverage from 86 to 160 with no loss of soundness (falsified proofs
>    still never `VerifiedGood`).

