# mrs — Mechanical Reasoning System

Current workspace version: **0.3.0**. For stable deployments, use the current
release source and follow the validation rules in
[`docs/policies/release.md`](docs/policies/release.md).

[![Crates.io](https://img.shields.io/crates/v/mrs.svg)](https://crates.io/crates/mrs)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

MRS is a proof-producing automated theorem prover written in Rust, targeting the [CASC](http://www.tptp.org/CASC/) competition. Its distinguishing feature is that candidate refutations can be checked by a separate proof kernel, rather than trusting the search that found them.

MRS reads [TPTP](https://www.tptp.org/) problem files and outputs results in [SZS/TSTP](https://tptp.org/Seminars/TPTPContentAndStandards/SZSPresentationSlides.pdf) format. Its zero-copy `mrs-tptp` parser supports TPTP v10 syntax; downstream lowering and search support depends on the input fragment. Search uses a **parallel strategy portfolio** running a **superposition calculus** within a **given-clause loop**, augmented by **AVATAR** (using CaDiCaL) for advanced clause splitting and **cross-strategy clause sharing**.

## Proof production and verification

MRS separates search from proof checking. Search produces a candidate TSTP
refutation; the distinct `mrs-proof-kernel` replays its inference steps against
the cited parents. Use `--self-check` to require strict kernel certification
before a theorem/proof result is emitted. This option is **off by default**:
without it, a `Theorem` result and emitted proof have not necessarily been
accepted by the strict kernel. Rejected or inconclusive candidates fail closed
when self-checking is enabled. See [Trust and verification](docs/reference/trust-and-verification.md)
and the [certification status](docs/CERTIFICATION_STATUS.md).

```bash
mrs --self-check problem.p
```

### What is formally guaranteed?

**MRS is not an end-to-end formally verified prover.** Its assurance levels are
deliberately separate:

| Layer | Actual assurance |
|---|---|
| Ordinary search | Produces candidate proofs; no proved whole-engine soundness/completeness or strict certification requirement by default. |
| `--self-check` / standalone strict verifier | A separate **ordinary Rust** kernel checks supported refutations against the supplied problem. Strong runtime/regression evidence, not formal verification of that kernel. |
| Finite models / bounded ground-EPR closure | Separate runtime-checked routes; not a formally proved model evaluator or grounding/closure completeness theorem. |
| Verus checker | Machine-checked supported inference replay, proof-DAG refutation and wire/token decoding **over decoded IR**. Main semantic domains use nonempty `ISet<int>` carriers, including infinite domains; arbitrary-carrier transport remains open. |
| TPTP-to-Verus bridge | Trusted parsing/lowering, source/role handling, normalization and certificate discovery. Opt-in dual checking requires both kernels, but does not verify original TPTP bytes end to end. |

The main semantic carrier was audited and widened from finite `u64` domains;
this does not add FOF/AVATAR/AC/arithmetic rules to the verified checker. A
successful local c502 check is not certification of the complete SWV406+1 proof.
Formal evidence does not replace the normal strict policy or prove search
fairness, completeness, wall-clock determinism, or compiler/OS correctness.

See the [complete guarantee audit](docs/reference/formal-guarantees.md),
[formal assurance roadmap](docs/research/formal-assurance-roadmap.md), and
[normative Verus assurance note](formal/superposition-checker/ASSURANCE.md).
The roadmap names the remaining semantic, input-fidelity, normalization,
FOF, AVATAR/SAT, model and runtime/build work with explicit acceptance gates.

To invoke the supplied formal artifact explicitly:

```bash
mrs-proover --verus-gate /path/to/verified-checker --verus-sha256 HASH proof.p
```

This implies strict verification and fails closed unless both paths accept.
`--verus-shadow` instead reports formal evidence without changing the strict
verdict. The selected artifact's own hash-bound support manifest defines its
scope; an older artifact does not inherit today's stronger theorem.

## Install

The easiest way to get `mrs` is via [crates.io](https://crates.io/crates/mrs):

```bash
cargo install mrs
```

This requires a Rust toolchain compatible with the workspace's current
`rust-version` (`1.100.0`). The repository development shell provides it via
the Nix flake.

Pre-built binaries are not yet provided; see [Building from source](#building-from-source) if you prefer not to use `cargo install`.

## Basic usage

```bash
mrs problem.p
```

Output lines begin with `% SZS status ...`.  On a successful refutation a TSTP proof block is also printed.

**Example** — given `socrates.p`:

```tptp
fof(ax1, axiom, ![X]: (human(X) => mortal(X))).
fof(ax2, axiom, human(socrates)).
fof(goal, conjecture, mortal(socrates)).
```

```
% SZS status Theorem for socrates
% SZS output start Proof for socrates
...
% SZS output end Proof for socrates
```

## Options

```
mrs [options] <file.p>
```

| Option | Default | Description |
|--------|---------|-------------|
| `--time <n>` | `30` | Wall-clock time limit in seconds |
| `--workers <N>` | physical cores | Maximum number of parallel search threads |
| `--schedule <name>` | `casc` | Select a named schedule. See [`docs/reference/schedules.md`](docs/reference/schedules.md). |
| `--auto-schedule` | off | Select a division schedule from clause shape. An explicit schedule wins. |
| `--strategy <N>` | — | Run one base strategy for the full budget. Valid IDs are 1 through 15. |
| `--selection <selector>` | — | Experimental queue-policy override for all scheduled workers (`fifo`, `smallest`, `age:N`, `goal:N`, `ml:N:ALPHA`, or `queue:...`). Used by the local autosearch tool. |
| `--portfolio <IDs>` | — | Run one explicit base strategy per worker, for example `12,11,10,6,8,1,4,13`. |
| `--self-check` | off | Strict-check candidate refutations before emitting theorem/proof output. Rejection or inconclusive verification becomes `GaveUp`. |
| `--certify-ordered` | off | Run bounded ordered-resolution certification; requires `--workers 1 --strategy N`. |
| `--list-schedules` | — | Print known schedule names and exit |

### Reproducible single-strategy runs

With `--workers N>1` (the default), every strategy in the schedule runs
concurrently in its own thread. Cross-strategy sharing of derived unit
equalities is disabled by default; set `MRS_SHARED_POOL_INTERVAL` to a
positive iteration count to enable it (see [Architecture](#architecture)).
When sharing is enabled, it lets the portfolio solve more problems in
aggregate than any strategy could alone, but **per-strategy telemetry
(`processed`/`generated`/`lrs_discarded` in the `% SZS detail` line) is not
reproducible run-to-run** — how much material a strategy receives from its
siblings, and how CPU contention affects timing-sensitive heuristics like LRS
pruning, both depend on real-time thread scheduling.

To isolate a *single* strategy, run with `--workers 1`. This removes sibling
search contention and clause-pool cross-talk, but does not make a wall-clock
run deterministic: host load can still change LRS decisions and timeout points.
For fixed-work diagnostics use explicit work ceilings and fixed-iteration LRS;
compare trajectories before treating two runs as equivalent.

```bash
nix develop -c cargo run -- --workers 1 --schedule casc_eps problem.p
```

## TPTP `%include` directives

Problems that reference the standard TPTP library via `%include` need the `TPTP` environment variable set to the root of a local TPTP installation:

```bash
TPTP=/path/to/TPTP-v9.x.x mrs problem.p
```

This is not needed for problems that are self-contained.

When reading a problem from stdin, pass an explicit include root so strict
self-verification can resolve external files:

```bash
cat problem.p | TPTP=/path/to/TPTP-v9.x.x \
  nix develop -c cargo run --features proover -- \
  --self-check --include-root /path/to/TPTP-v9.x.x -
```

## Building from source

```bash
git clone https://github.com/newca12/mrs
cd mrs
nix develop -c cargo build --release      # binary at target/release/mrs
nix develop -c cargo test --workspace     # run all tests
```

The complete current CLI, environment-variable, and feature reference is in
[`docs/reference/cli.md`](docs/reference/cli.md).

## Architecture

The pipeline for each problem:

1. **Parse** — `mrs-tptp` converts TPTP text to a zero-copy AST.
2. **Lower** — `src/lowering.rs` maps the AST to `mrs-core` types.
3. **Clausify** — `mrs-cnf` transforms formulas to CNF (NNF → Skolemization → definitional CNF). Conjectures are negated for refutation-based proving.
4. **Search** — `mrs-search` runs a given-clause loop with 15 active base configurations plus a diagnostic slot in the generic schedule; division schedules scale selected configurations to the requested workers.
5. **Output** — `mrs-szs` formats the SZS status line; `mrs-proof` extracts and formats the TSTP proof on refutation.

### Strategy portfolio

The generic schedule contains 15 active strategies plus a zero-time diagnostic
slot. Division schedules select from the 15 base strategies. Workers use fresh
search states. They can
share a pool of globally discovered unit equalities when
`MRS_SHARED_POOL_INTERVAL` is set to a positive value; sharing is disabled by
default. Time is distributed from the total budget to bound execution:

| # | Selection | Weight fn | Literal selection | Ordering | Time share | Notes |
|---|-----------|-----------|-------------------|----------|------------|-------|
| 1 | AgeWeight(3) | Standard | AllNegative | KBO | 14% | balanced exploration |
| 2 | SmallestFirst | Standard | AllNegative | KBO | 10% | no weight limit + no AVATAR (deep chain proofs) |
| 3 | SmallestFirst | Standard | AllNegative | KBO | 10% | pure best-first |
| 4 | AgeWeight(8) | Standard | MaxNegativeOrMaxPositive | KBO | 9% | aggressive selection |
| 5 | AgeWeight(5) | Standard | All | KBO | 9% | unrestricted literal selection |
| 6 | AgeWeight(10) | Standard | All | KBO | 10% | no AVATAR (FNE/definitional CNF) |
| 7 | AgeWeight(3) | Standard | AllNegative | LPO | 14% | LPO balanced exploration |
| 8 | GoalDirected(10) | Standard | AllNegative | LPO | 9% | LPO goal-directed |
| 9 | SmallestFirst | Standard | AllNegative | LPO | 9% | LPO best-first |
| 10 | AgeWeight(12) | Standard | AllNegative | KBO | 5% | SOS (sos_depth=100) + KBO |
| 11 | AgeWeight(6) | ConjSymbolBoost | AllNegative | KBO | 5% | goal-symbol boosted weight |
| 12 | AgeWeight(5) | HornHeuristic | AllNegative | KBO | 3% | Horn-preferred weight, no AVATAR |
| 13 | AgeWeight(5) | FunctionWeightPenalty | AllNegative | KBO | 2% | SOS + quadratic depth weight |
| 14 | SmallestFirst | ConjSymbolBoost | All | KBO | 2% | FEQ: goal-symbol + All selection |
| 15 | AgeWeight(4) | SymbolWeight | AllNegative | KBO | ~1% | precedence-based symbol weight |

Each launched strategy runs within the shared wall-clock budget. The schedule
scales nominal slices for concurrent workers; it is not a sequential sum of all
15 slices.
LRS (Limited Resource Strategy) periodically prunes the passive queue to stay within the time budget.

### Workspace layout

```
mrs/
├── src/
│   ├── main.rs        CLI entrypoint; orchestrates the full pipeline
│   ├── lowering.rs    TPTP AST → mrs-core types
│   └── include.rs     resolves TPTP %include directives
└── crates/
    ├── mrs-core/      Term, Formula, Clause, Literal, Substitution, SymbolTable
    ├── mrs-szs/       SZS status enum + formatting
    ├── mrs-cnf/       clausification: NNF, Skolemization, definitional CNF
    ├── mrs-unify/     Robinson unification + matching
    ├── mrs-calculus/  inference rules (resolution, superposition, …), KBO/LPO, literal selection
    ├── mrs-index/     discrimination tree indexing
    ├── mrs-proof/     proof extraction + TSTP output
    ├── mrs-search/    given-clause loop, clause weighting, strategy scheduler
    ├── mrs-tptp/      zero-copy TPTP parser
    ├── mrs-proof-kernel/ independent proof/model kernel
    ├── mrs-proover/   standalone TSTP verifier
    └── mrs-bench/     CASC and ProoVer harnesses
```

For the organized documentation map, see [`docs/README.md`](docs/README.md).
Paper-derived techniques and their supported scope are listed in [`docs/reference/paper-implementations.md`](docs/reference/paper-implementations.md); technique names are not whole-paper or completeness claims.
The active performance and architecture plan is [`ROADMAP.md`](ROADMAP.md);
the detailed formal work packages are in the
[formal assurance roadmap](docs/research/formal-assurance-roadmap.md).

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.
