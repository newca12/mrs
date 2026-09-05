# Retrospective Methodology Status

This document applies [`METHODOLOGY.md`](METHODOLOGY.md) retrospectively to
the major feature families currently present in `mrs`.

It is a status ledger, not a claim that every historical feature has received
the same level of validation. The purpose is to make the evidence and the
remaining gaps explicit before further performance work or a soundness-sensitive
release.

## 1. Scope and Baseline

Code status is assessed at:

```text
branch: fix/integrate-casc-next-review
commit: 4653fe1 fix: fail closed on incomplete instgen models
```

The review covers major feature families rather than every individual commit.
Related commits are grouped when they share one semantic contract and one
validation boundary.

Evidence references use these abbreviations:

| Code | Meaning |
| --- | --- |
| U | Focused unit or integration tests exist |
| R | Refutation/proof replay or strict-kernel validation exists |
| O | Independent bounded oracle, differential test, or external checker exists |
| B | Reference-status or corpus benchmark evidence exists |
| T | Targeted adversarial regression exists |
| C | Current-commit evidence; historical evidence is marked `H` |
| H | Historical evidence from another commit or older benchmark run |

Status meanings:

| Status | Meaning |
| --- | --- |
| **Green** | The current claimed behavior has a clear contract and adequate evidence for its trust boundary |
| **Yellow** | No current unsoundness is established, but evidence is incomplete or the feature affects completeness/status behavior |
| **Red** | A known unsoundness or unproved definitive-result path exists; default enablement or positive claims are not acceptable |
| **Green/Yellow** | Refutations are adequately protected, but saturation/model claims or completeness evidence remain incomplete |

The benchmark state is important. A remote CASC-30 run on commit `835077e`
reported 344/1101 solved and three reference violations (`NLP260+1`,
`NLP261+1`, and `NLP262+1`). On commit `4653fe1`, those three cases were
reproduced one at a time and returned `GaveUp`, not `CounterSatisfiable`.
That is a targeted soundness fix, not a replacement for a complete current
CASC rerun.

The required Nix validation suite passed on the current code before this
documentation-only change:

```text
nix develop -c cargo check
nix develop -c cargo clippy --all -- -D warnings
nix develop -c cargo fmt --all --check
nix develop -c cargo test --workspace
```

## 2. Feature Matrix

| Feature family | Implementation and evidence | Affected definitive statuses | Status | Required next gate |
| --- | --- | --- | --- | --- |
| TPTP parser and AST | `crates/mrs-tptp`; parser, dialect, iterator, malformed-input, and round-trip tests (`mrs-tptp/tests/`) | Parsing can affect all downstream statuses | **Green/Yellow** | Add differential parser and fuzz tests; verify unsupported syntax cannot reach `Satisfiable` |
| Lowering and `%include` resolution | `src/lowering.rs`, `src/include.rs`; include and lowering tests; unsupported dialects generally become `GaveUp` | All statuses, especially unsupported-problem handling | **Yellow** | Add end-to-end tests for unsupported dialects, include selection, symbol collisions, and missing includes |
| NNF, miniscoping, and Skolemization | `mrs-cnf/src/nnf.rs`, `miniscope.rs`, `skolem.rs`; free-variable Skolemization commit `3c66465`; strict kernel checks NNF and Skolemization | `Theorem`, `Unsatisfiable` through refutation preprocessing | **Green/Yellow** | Add bounded semantic equivalence tests from original FOF to generated clauses, including nested quantifier alternation |
| Definitional CNF and polarity-aware renaming | `722b669`, current `mrs-cnf/src/lib.rs`; definition provenance, nested Tseitin, and `definition_renaming` kernel tests | `Theorem`, `Unsatisfiable` | **Green/Yellow** | Add independent bounded-model checks for generated definitions and all polarity cases |
| Goal transformation | `ceea916`, `658b5c7`, `mrs-cnf/src/goal_transform.rs`; recursive/maximal, deduplication, variable, and provenance tests; kernel rule `goal_transformation` | `Theorem`, `Unsatisfiable` | **Yellow** | Add full original-vs-transformed strict verification and bounded semantic equivalence tests |
| Resolution, factoring, and equality resolution | `mrs-calculus`; extensive calculus tests; strict-kernel reconstruction; DER commit `9738467` | Refutation statuses | **Green/Yellow** | Add independent bounded reference-calculus comparison and chained DER proof-extraction tests |
| Superposition and paramodulation | Indexed superposition `f935bbc`; calculus tests; strict kernel `superposition`/`paramodulation` checks | Refutation statuses | **Green/Yellow** | Add adversarial tests for variable capture, rewrite positions, orientation, duplicate literals, and equality symmetry |
| Demodulation and in-place backward demodulation | `2b51b83`; DTree/STree lookup; demodulation tests; search regression `avatar_forward_demod_no_false_refutation` | Refutation and saturation behavior | **Yellow** | Differentially compare indexed and linear rewriting; test cycles, orphan removal, AVATAR contexts, and all parent edges |
| DER and contextual subsumption resolution | `9738467`, `d97b41d`; `mrs-calculus/src/subsumption.rs`; strict-kernel checks; destructive-child registration fix `7ac948d` | Refutation statuses; can affect completeness | **Green/Yellow** | Add search-to-TSTP integration tests for destructive chains, auxiliary-parent deletion, and variable substitutions |
| Discrimination trees, FVI, and FVT | `f935bbc`, `5bb8b8b`; DTree/STree/FVI/FVT unit and differential tests | Indirectly affects refutation completeness and saturation | **Yellow** | Make linear-scan comparison exhaustive on bounded clauses; specifically prove no false-negative candidate lookup |
| Tautology elimination, PLE, and BCE | `eca8405`, `mrs-search/src/preprocessing.rs`; synthetic tests for cascading PLE, BCE, equality reflexivity, and conjecture protection | Refutation completeness; preprocessing can alter all results | **Yellow** | Add independent before/after satisfiability oracle tests with first-order variables and partner-cap boundary cases |
| SInE filtering | `mrs-search/src/sine.rs`; filtering test; saturation after SInE is demoted to `GaveUp` in `strategy.rs` | Positive status claims and refutation coverage | **Green/Yellow** | Add explicit tests that filtered subsets cannot stop a portfolio with `Satisfiable`/`CounterSatisfiable`; measure false-negative proof coverage |
| LRS passive pruning | `given_clause.rs`, `LrsPolicy`; fixed-iteration tests; `lrs_discarded` forces `GaveUp` | Positive status claims; refutation completeness | **Green/Yellow** | Add deterministic-vs-wall-clock canaries and test deadline/cancellation boundaries |
| Literal selection and ordered inference | `mrs-calculus/src/literal_selection.rs`; maximal-literal regression for SYN861/862/866; incomplete selection demotion | Refutation completeness and positive status | **Yellow/Red boundary** | Resolve the contradiction between the `ordered_inferences` comment and `SearchConfig::default()` setting it to `true`; then rerun EPR canaries |
| Multi-queue selection | `3ec3228`, `select.rs`, `unprocessed.rs`; queue fallback/interleaving tests | Indirectly affects completeness and saturation | **Yellow** | Prove queue choice is ordering-only; compare bounded runs with a complete FIFO baseline; ensure incomplete modes demote saturation |
| Dynamic precedence and symbol weighting | `379baa5`, `symbol_config.rs`, `weight.rs`; scheme and weight tests; non-standard weight saturation demoted | Refutation search and saturation classification | **Green/Yellow** | Add ordering stability-under-substitution tests and bounded differential search across schemes |
| Goal-distance guidance and SOS | `6f807a7`, `goal_distance.rs`, `sos_depth`; reachability tests; SOS saturation demoted | Refutation completeness and positive status | **Green/Yellow** | Add graph permutation tests and proof-coverage comparison; ensure all restricted SOS paths fail closed |
| Shared clause pool and orphan elimination | `78f00212`, `SearchState::children`; epoch/sorting/import tests; destructive parent handling | Parallel result selection and proof closure | **Yellow** | Add race-oriented one-worker/multi-worker status tests and shared-chain deletion tests |
| Parallel portfolio and stop flag | `strategy.rs`, named schedules, `casc_*`; worker-count tests and telemetry | All statuses, especially `CounterSatisfiable` | **Yellow** | Test races where one worker refutes, one saturates incompletely, and one is cancelled; only certified definitive results may stop the portfolio |
| AC detection, normalization, and AC unification | AC commits including `83a93216`; `mrs-unify`, `state.rs`; AC ordering/unification tests | Refutation and equality saturation | **Yellow** | Add algebraic property tests, axiom-removal equivalence tests, and normalized/non-normalized differential proofs |
| FVO propositional refutation | `mrs-search/src/fvo.rs`; refutation-only CaDiCaL/BFS path; tests for equality/function rejection, SAT fallback, and proof output | `Theorem`, `Unsatisfiable` only | **Yellow** | Perform a dedicated adversarial audit for variable sharing, repeated variables, predicate arity, polarity, and lifting; add real-corpus path coverage |
| InstGen refutation path | `06d6831`, `3a41263`; lazy MGU instances, BFS resolution DAG, provenance fallback; refutation tests | `Theorem`, `Unsatisfiable` | **Green/Yellow** | Compare bounded cases with exhaustive grounding and verify every generated proof through the strict kernel |
| InstGen model/satisfiability path | `06d6831`, corrected by `4653fe1`; variable abstraction previously caused NLP260-262 false `CounterSatisfiable`; variable-bearing cases now return `GaveUp` | `Satisfiable`, `CounterSatisfiable` | **Red for completeness; fail-closed safety** | Do not claim complete EPR SAT support until a certified model or complete finite-instance checker exists |
| AVATAR splitting and SAT integration | `a7d8e41`, `be6e77e`, `12a8d99`; SAT manifests, component certificates, strict bounded LRAT/FRAT replay, mutation tests | Refutations and branch roll-ups | **Green/Yellow** | Run current real-corpus certificate audits; classify unsupported RAT/incremental traces as `Unknown` |
| CWA componentwise AVATAR | `cwa.rs`; fixes `c79a87f`, `3062810`, `fcbb98c`; polarity, shared-variable, duplicate-predicate, and mixed-polarity tests; real SEU path observations | Refutations, historically `Theorem` risk | **Yellow** | Add permanent reduced real-file canaries; keep roll-ups without replayable SAT traces at `GaveUp`; continue coverage telemetry |
| Proof provenance and TSTP extraction | `mrs-proof`, `ClauseSource`, `7ac948d`; fixes for definitions, InstGen fallback, destructive parents, duplicate literals, and AVATAR ancestry | Refutation statuses | **Green/Yellow** | Run a corpus-wide invariant that every emitted refutation passes strict verification; track unsupported shapes separately |
| Strict proof kernel | `b17f149` onward; 141 kernel tests, forged-proof tests, resource limits, AVATAR checks, definition-renaming tests | Certified `Theorem`/`Unsatisfiable` proofs | **Green for certified subset** | Keep certification separate from completeness; add mutation tests for each newly accepted rule |
| `mrs-proover` verifier | Structural checks, strict mode, ATP ladder, FMB, evil-proof tests; `SOUNDNESS_STATUS.md` records historical audit results | `VerifiedGood`, `VerifiedBad`, `Unknown` | **Green/Yellow** | Repeat the corpus audit at `4653fe1`; record external ATP versions; never convert `Unknown` into a definitive verdict |
| ML clause selection and premise pruning | `98cc7df`, `ml-guidance`; trace logging, model scoring, last-slot pruning; pruned workers demoted to `GaveUp` | Refutation coverage and positive status | **Yellow** | Record model/data/version provenance; test malformed/random weights; verify every pruning path remains fail-closed |
| Benchmark harness and data-driven schedules | `mrs-bench`, `named.rs`, `DIVISIONS.md`; greedy set-cover, telemetry, reference violation reports | All benchmark conclusions | **Yellow** | Make zero reference violations a merge gate; record exact commit, TPTP version, RAM, CPU, workers, schedule, and environment |
| Release and documentation process | `METHODOLOGY.md`, `RELEASE_CHECKLIST.md`, `AUDIT.md` | Release claims | **Yellow, newly established** | Require this matrix and current benchmark evidence in release reviews; update stale historical tables explicitly |

## 3. Immediate Release Blockers

The following items must be resolved or explicitly isolated before a
soundness-sensitive default release:

1. **Ordered inference default mismatch.** The source comment describes ordered
   inference as experimental and incomplete, while `SearchConfig::default()`
   currently sets `ordered_inferences: true`. Either make the default false or
   prove completeness and add current EPR status canaries.
2. **FVO audit.** `fvo.rs` has a hand-written soundness justification and a
   narrow trigger, the same risk shape that allowed the CWA polarity defect to
   survive. Its current path is refutation-only, but it still needs an
   adversarial variable-sharing and lifting audit.
3. **BCE/PLE oracle coverage.** Existing tests are synthetic. Add bounded
   before/after satisfiability comparisons, including variable-heavy clauses and
   partner-list limits.
4. **Index false negatives.** DTree/STree/FVI/FVT failures can silently cause
   incomplete search. Preserve and expand differential tests against a linear
   reference implementation.
5. **AVATAR/CWA real-input coverage.** Synthetic tests are valuable but do not
   prove that narrow real-corpus branches execute. Keep reduced real-file
   canaries and report zero-hit soundness paths as audit gaps.
6. **InstGen model claims.** The current implementation is safely fail-closed
   for variable-bearing model checks, but is not a complete EPR satisfiability
   procedure. Do not describe it as one until a model certificate exists.
7. **Current benchmark freshness.** Historical benchmark tables are not current
   evidence for `4653fe1`. Run the representative reference-status gate again
   after the next search change.

## 4. Required Evidence Ledger

For each future feature row, record these fields before calling it validated:

```text
feature:
implementation_commit:
affected_statuses:
semantic_contract:
incomplete_paths_and_fail_closed_result:
unit_and_adversarial_tests:
independent_oracle_or_replay:
reference_benchmark:
hardware_and_resource_limits:
current_status:
remaining_follow_up:
```

The ledger should be updated when behavior changes, not only when a feature is
first implemented. A later soundness fix changes the status of the feature and
must identify which old evidence is no longer sufficient.

## 5. Evidence Interpretation Policy

The following conclusions are not interchangeable:

- A strict-kernel-certified refutation proves that one refutation is sound.
- A passing unit test proves only the tested examples and assumptions.
- A benchmark solve count measures performance under one environment.
- Agreement with Vampire or E Prover is useful differential evidence, not a
  mathematical proof.
- A `GaveUp` result after an incomplete optimization is a successful
  soundness outcome, not a failed theorem-proving result.
- Zero current reference violations is necessary for release, but does not
  replace a semantic proof for a new algorithmic claim.

This matrix should be reviewed together with
[`METHODOLOGY.md`](METHODOLOGY.md) after every new search calculus,
preprocessor, abstraction, proof rule, or status-classification change.
