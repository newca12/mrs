# TODO: ProoVer 2026 Competition Roadmap

This document tracks what remains to be implemented in `mrs-proover` to maximise the ProoVer 2026 score. Items are ordered by expected scoring impact.

## Scoring Recap

| Outcome | Points |
|---------|--------|
| Correctly identify evil proof (`FailedVerified`) | **+2** |
| Correctly identify good proof (`Verified`) | **+1** |
| Give up / timeout (`NotVerified`) | 0 |
| Falsely reject a good proof | −1 |
| Falsely verify an evil proof (`Unsound`) | **−10 (fatal)** |

The scoring is highly asymmetric. A single `−10` requires 10 correct `+1` verifications just to recover.

---

## Already Implemented (no longer blocking)

| Item | Commit |
|------|--------|
| E-prover `introduced(definition)` peeling bug fixed | `4239d51d` |
| `test_tptp_solutions.sh` HTML Stripping Bug | `4239d51d` |
| Accept CNF Steps in Proof DAG | `4239d51d` |
| `introduced(definition)` formula-body validation (definition laundering fix) | `202aae96` |
| Evil-proof test suite (9 exploit cases) | `202aae96` |
| `vampire_skolemisation.rs`: `Unknown` → `Unsound` on arity drops and shape violations | `c722f998` |
| Propositional SAT fast-path for AVATAR/rat/sat_conversion steps | (pre-existing) |
| Structural definition-folding via unfold + alpha-equivalence | (pre-existing) |
| Free Variable Skolemization block (`try_skolem_axiom` free-var check) | `c53c0ad6` |
| `split_conjunct` AC-reorder exploit closed (`strict_alpha_equiv`) | `c53c0ad6` |
| `MrsAtp` in-process backend (zero subprocess overhead per step) | `c53c0ad6` |
| `~$true` recognised as valid `$false` root in DAG | `22ab3d02` |
| Refutation root: pick topologically last `$false` node (AVATAR multi-`$false`) | `1cbc351d` |
| TFF type-decls skipped; `EmptyProof`/`UnsupportedDialect` → `NotVerified` | `4ed23bdf` |
| `has_esa_in_term`: propagate `[status(esa)]` from nested E-prover inference chains | `4ed23bdf` |
| `match_formula` permuted quantifier variable lists (Vampire multi-var Skolem) | `4ed23bdf` |
| Unrecognised `introduced(definition)` intro tags (e.g. `general_splitting_component_introduction`) → `Unknown` | `c621b94b` |
| `introduced(choice_axiom)` routed through Skolem-axiom verifier (SnakeForV/Vampire) | `38187884` |
| Metis `ColonPair` parent extraction (`inference(subst,[],[p:[bind…]])`) | `8f8b3d50` |
| Clausified anonymous `file(_,unknown)` leaves → `Unknown` (except `$false` spoofing stays `Unsound`) | `8f8b3d50` |
| Cyclic/recursive definition chain detection (`check_cycles` DFS over DAG) | `4ed23bdf` |
| Broader structural coverage: `strict_alpha_equiv` for `split_conjunct` projection | `c53c0ad6` |
| Batch ATP queries: in-process `mrs_search` replaces per-step subprocess | `c53c0ad6` |
| Deterministic offline E+Vampire regression corpus (46 proofs, 0 `FailedVerified`) | `8f275b16` |
| `test_tptp_solutions.sh` restricted to E/Vampire allowlist (no format noise) | `8f275b16` |
| AC-equivalence matching in `axiom_leaf.rs` | `202aae96` |
| In-process MrsAtp saturation fallback | `bbd640cf` |

---

## Remaining Work

### Basic E/Vampire Structural Parsing for CASC Dataset Hardening — **more `+2` points**
**Files:** Various `crates/mrs-proover/src/checks/`

Several check modules still return `StepOutcome::Unknown` for shapes they do not
recognise, scoring 0 when the proof step is actually malformed.

Specific targets (from the evil-proofs analysis and CASC dataset experience):

- `definition_folding.rs`: The recursive body check (`rejects_recursive_definition`)
  works but only catches a *single* level of self-reference inside one definition's
  body.  Multi-step recursive unfolding through chains of `definition_folding` steps
  should also trigger `Unsound` (the `has_dependency_cycle` guard already exists at
  the per-step level; extend it across the whole proof DAG).

- Anonymous `file(_,unknown)` leaves from pre-clausifying provers (SPASS, Otter)
  currently return `Unknown`.  An AC-normalising comparison against the Skolemised /
  CNF form of each axiom would turn these into `Verified` (+1) rather than
  `NotVerified` (0).  Low priority vs. soundness work but meaningful at scale.

### Benchmark Against Nörgler — ✅ **done**
A full head-to-head against Nörgler on the
[TSTP FOF Proof Benchmark (Zenodo 19792604)](https://zenodo.org/records/19792604)
— the dataset the Nörgler authors published to evaluate proof checkers
(original + automatically falsified PyRes/Otter proofs). Two scripts wire it in:
`crates/mrs-bench/fetch_zenodo_corpus.sh` (download + normalise) and
`crates/mrs-bench/zenodo_benchmark.sh` (`--with-norgler` for the comparison).
See `docs/PROOVER_HARNESS.md` §6 for the full table.

Result on full PyRes (170+170) + an Otter sample (60+60), 60 s/proof:

- **Soundness — mrs-proover is strictly safer.** 0 false rejects and 0 false
  accepts across all 460 proofs, catching **230/230 (100%)** mutations. Nörgler
  has **11 false rejects** (−1 each) on valid PyRes proofs and misses 6 mutations.
- **Coverage — Nörgler is more complete.** It verifies 155/170 PyRes (vs 86) and
  60/60 Otter (vs 0). On raw points its coverage edges ahead (≈ +474 vs +426 on
  PyRes), but with a worse safety record.

Concrete gaps the benchmark exposed (now the real follow-up work, see below):
1. mrs-proover defers all unannotated `skolemize` steps to `Unknown` instead of
   positively verifying them — the single biggest coverage gap on PyRes.
2. Without a problem file mrs-proover cannot validate `file(_,unknown)` leaves
   (all Otter originals → `NotVerified`); harmless at the competition, which
   always supplies the problem.

A real soundness bug was found and fixed during this run: multi-existential
`skolemize` steps were false-rejected (−1) on 8/170 valid PyRes proofs; the
arity check now tracks per-existential scopes.

### Positively verify unannotated `skolemize` steps — **more coverage**
**File:** `crates/mrs-proover/src/checks/skolemize.rs`

The E-style fallback (`check_e_style_skolemize`) only ever returns `Unknown`: it
infers the fresh Skolem symbols and sanity-checks their arity, but never
*positively* confirms the step. Reconstructing the Skolemisation (replace each
existential by `sk(in-scope universals)`) and α/AC-comparing against the
conclusion would turn ~84/170 PyRes `NotVerified` into `Verified` (+1), closing
most of the coverage gap vs Nörgler without weakening soundness.

---

## ProoVer Competition Checklist

| Task | Priority | Status |
|------|----------|--------|
| Free Variable Skolemization block | Critical | ✅ Done (`c53c0ad6`) |
| Accept CNF Steps in Proof DAG | High | ✅ Done (`4239d51d`) |
| Definition laundering blocked | High | ✅ Done (`202aae96`) |
| Fix `test_tptp_solutions.sh` HTML Stripping Bug | High | ✅ Done (`4239d51d`) |
| Cyclic/recursive definition chain detection | High | ✅ Done (`4ed23bdf`) |
| `split_conjunct` AC-reorder exploit closed | High | ✅ Done (`c53c0ad6`) |
| In-process ATP backend (zero subprocess overhead) | Medium | ✅ Done (`c53c0ad6`) |
| TFF/non-TSTP proofs → `NotVerified` not `FailedVerified` | Medium | ✅ Done (`4ed23bdf`) |
| Nested `[status(esa)]` propagation (E combined steps) | Medium | ✅ Done (`4ed23bdf`) |
| Permuted quantifier variable lists in Skolem axioms | Medium | ✅ Done (`4ed23bdf`) |
| Unrecognised `introduced(definition)` intro tags | Medium | ✅ Done (`c621b94b`) |
| `introduced(choice_axiom)` variant | Medium | ✅ Done (`38187884`) |
| Metis `ColonPair` parent extraction | Medium | ✅ Done (`8f8b3d50`) |
| Anonymous leaves from clausifying provers → `Unknown` | Medium | ✅ Done (`8f8b3d50`) |
| Deterministic offline regression corpus | Medium | ✅ Done (`8f275b16`) |
| AC-equivalence matching in `axiom_leaf.rs` | High | ✅ Done (`202aae96`) |
| In-process MrsAtp saturation fallback (`Unknown` vs `Unsound`) | High | ✅ Done (`bbd640cf`) |
| Basic E/Vampire structural parsing for CASC dataset hardening | Low–Medium | ✅ Done |
| Benchmark against Nörgler | After fixes | ✅ Done (full Zenodo 19792604 head-to-head; see §6) |
| Multi-existential `skolemize` false-reject fixed | High | ✅ Done |
| Positively verify unannotated `skolemize` steps (coverage) | Medium | ❌ TODO |
