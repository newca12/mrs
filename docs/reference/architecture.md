# Architecture Reference

## Pipeline

For a TPTP problem, the root binary performs this pipeline:

1. `mrs-tptp` parses the input into a borrowing AST.
2. `src/lowering.rs` maps supported formulas and clauses into MRS core types.
3. `src/include.rs` resolves `%include` directives using the problem location
   and `TPTP` hints.
4. `mrs-cnf` performs NNF conversion, miniscoping, Skolemization, definitional
   CNF, and explicit conjecture negation.
5. `mrs-search` runs preprocessing, EPR/FVO/CWA prepasses where applicable,
   and the given-clause portfolio.
6. `mrs-proof` extracts a reachable derivation DAG and formats TSTP.
7. `mrs-szs` emits the SZS result and `mrs` prints telemetry.

The ordinary portfolio is refutation-oriented. Incomplete restrictions such as
SInE, SOS, LRS pruning, ML premise pruning, non-standard weights, and bounded
search do not justify a positive saturation result.

## Search engine

The given-clause loop maintains passive and processed clause sets. It supports:

- resolution, factoring, equality resolution, equality factoring, and
  superposition;
- KBO, LPO, dynamic symbol configurations, and AC-aware helpers;
- forward/backward demodulation and subsumption;
- subsumption resolution, condensation, tautology deletion, BCE, and PLE;
- discrimination-tree, substitution-tree, feature-vector, and literal indexes;
- SInE, goal distance, SOS, LRS, multi-queue selection, and clause-weight
  functions;
- AVATAR splitting with CaDiCaL;
- FVO propositional refutation and lazy EPR InstGen;
- optional cross-worker sharing of proof-carrying positive unit equalities.

Every worker owns its `SearchState`, term bank, indexes, and local symbol-ID
space. Shared equality chains carry symbol names and complete ancestry so they
can be remapped and checked in the receiving worker.

## Trust boundary

Search produces candidates and provenance. It does not by itself establish that
an incomplete search has proved satisfiability. `--self-check` sends candidate
refutations through the strict kernel and suppresses the theorem/proof result if
the kernel rejects or cannot certify it. The separate `mrs-proover` binary has:

- `--strict`, which uses `mrs-proof-kernel` and no ATP search;
- competition mode, which combines structural checks, CaDiCaL fast paths,
  in-process MRS, E, Vampire, and optional Vampire-FMB backends; and
- model-certificate validation for finite interpretations.

See [Trust and verification](trust-and-verification.md) and the
[ProoVer guide](../guides/proover.md).

## Workspace crates

| Crate or path | Responsibility |
|---|---|
| `src/` | CLI orchestration, lowering, includes, profiling, certification coordination |
| `mrs-core` | Terms, formulas, clauses, symbol tables, optional ML features/models |
| `mrs-tptp` | Zero-copy TPTP/TSTP parser and AST |
| `mrs-cnf` | NNF, Skolemization, definitional CNF, goal transformation |
| `mrs-unify` | Robinson unification, matching, AC helpers |
| `mrs-calculus` | Inference rules, orderings, literal selection, subsumption |
| `mrs-index` | Discrimination, substitution, feature-vector, and literal indexes |
| `mrs-search` | Given-clause loop, prepasses, schedules, portfolio coordination |
| `mrs-proof` | Provenance traversal and TSTP formatting |
| `mrs-proof-kernel` | Independent strict proof and model certificate checks |
| `mrs-proover` | Standalone ProoVer checker and ATP ladder |
| `mrs-cadical` / `mrs-cadical-sys` | CaDiCaL Rust binding and vendored solver |
| `mrs-bench` | CASC harness, audits, scoring, reports, and benchmark utilities |
| `mrs-train` | Offline Burn training for optional ML models |
| `mrs-book-labs` | Runnable educational examples for the mdBook |

## Resource and concurrency notes

The default worker count is physical-core and memory aware. A named division
schedules create one configuration per requested worker. The generic `casc`
schedule retains the full base strategy registry and may run configurations in
waves as workers become available. The wall-clock budget is shared by the
schedule; a strategy's nominal slice is scaled for concurrent workers.

The benchmark wrapper adds a slightly shorter internal deadline, raises worker
stack size, and resolves includes against the selected benchmark corpus. See
[Benchmarking](../guides/benchmarking.md) for the distinction between wrapper
behavior and direct binary behavior.
