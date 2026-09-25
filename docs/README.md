# MRS Documentation

This directory contains the documentation for `mrs`, the MRS theorem prover,
and its proof-verification companion, `mrs-proover`.

**Current source snapshot:** `5a9c687` on `main`, 2026-09-25. The current
reference pages describe the checked-out source at that snapshot. Benchmark
reports are not automatically current: each report must name its commit,
corpus, command, hardware, and environment.

## Start here

| Need | Read |
|---|---|
| Build, test, and lint the workspace | [Development guide](guides/development.md) |
| Run the prover on a TPTP problem | [Proving guide](guides/proving.md) |
| Understand the command-line interface | [CLI reference](reference/cli.md) |
| Understand the implementation | [Architecture reference](reference/architecture.md) |
| Check current implementation status | [Current status](reference/status.md) |
| Choose or study a schedule | [Schedule reference](reference/schedules.md) |
| Run CASC-style experiments | [Benchmarking guide](guides/benchmarking.md) |
| Verify TSTP proofs | [ProoVer guide](guides/proover.md) |
| Understand the trust boundary | [Trust and verification](reference/trust-and-verification.md) |
| Work on a soundness-sensitive change | [Development methodology](policies/methodology.md) |
| Prepare a release | [Release policy](policies/release.md) |
| Learn the calculus interactively | [The mdBook](book/README.md) |

## Authority order

When documents disagree, use this order:

1. Current source code, tests, and CLI behavior.
2. Pages under `reference/`, `guides/`, and `policies/`.
3. Dated reports under `reports/`.
4. Material under `history/`, which is retained for context and is not a
   current implementation contract.
5. Generated submission descriptions and sample outputs under `submissions/`.

`AGENTS.md` remains the repository-level instruction file for agent workflow,
Nix-wrapped validation, and competition policy. It is not a substitute for the
current CLI or architecture references here.

## Directory map

| Directory | Purpose | Freshness expectation |
|---|---|---|
| `reference/` | Source-derived behavior and interfaces | Update with behavior changes |
| `guides/` | Repeatable human and agent workflows | Commands must be runnable |
| `policies/` | Normative engineering and release rules | Changes require maintainer review |
| `research/` | Design studies and active technical investigations | Mark scope and evidence |
| `reports/` | Dated benchmark, audit, and evaluation results | Never present as a current baseline without rerunning |
| `history/` | Superseded plans and archived design material | Historical only |
| `submissions/` | Competition descriptions and sample solutions | Include edition and source provenance |
| `results/` | Raw or summarized benchmark artifacts | Keep command and input provenance |
| `book/` | Educational mdBook and runnable labs | Teaching material, not normative reference |

## Freshness rules

Current-facing documentation must:

- identify the source or command it describes;
- use repository-relative paths or explicit placeholders, never a developer's
  home directory;
- distinguish `Theorem`/`Unsatisfiable` refutations from satisfiability and
  counter-satisfiability model claims;
- distinguish solo strategy diagnostics from cooperative portfolio results;
- record the commit, TPTP edition, timeout, worker count, external jobs,
  hardware, environment variables, and output path for benchmark claims; and
- label historical claims with `Status`, `As of commit`, `Date`, and
  `Environment` metadata.

Do not edit a historical report to make it look current. Add a new dated
report and link it from the relevant current guide instead.

## Current status in one paragraph

`mrs` 0.2.3 is a Rust first-order theorem prover using clausification,
superposition and resolution search, AVATAR/CaDiCaL integration, EPR-oriented
prepasses, indexed redundancy elimination, and configurable strategy
portfolios. The default `casc` schedule contains 15 active base strategies plus
a zero-time diagnostic slot. Division schedules select and scale those base
strategies by worker count. Cross-strategy unit-equality sharing is opt-in.
Strict self-checking certifies refutation proofs; incomplete search and
unsupported positive claims fail closed. `mrs-proover` provides a standalone
strict-kernel mode and a broader competition-mode verifier with optional ATP
backends.

## Updating the documentation

For a source behavior change, update the relevant `reference/` page in the same
change. For a new experiment, add a dated report under `reports/` or
`results/`, then update the relevant guide only with the durable conclusion.
For a superseded document, move it to `history/` and add a replacement link
instead of silently deleting its evidence.
