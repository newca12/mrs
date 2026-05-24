# AGENTS.md

Quick-start context for AI agents working in this repo.

## What this is

`mrs` is an automated theorem prover in Rust targeting the CASC competition. It reads **TPTP** problem files and outputs **SZS/TSTP**-formatted results using a superposition calculus with a given-clause loop and strategy portfolio scheduler.

## mrs-tptp-parser

The TPTP parser lives at `crates/mrs-tptp-parser/` inside this repo. The **directory** is `mrs-tptp-parser`; the **crate name** (package name in its `Cargo.toml`) is `mrs-tptp`. It is a full workspace member — no sibling repo or external clone required.

- Zero-copy TPTP parser built with [winnow](https://crates.io/crates/winnow). AST borrows `&str` slices from the input — no per-token allocation.
- Single library crate (no binary). Edition 2024.
- Supports all major dialects: CNF, FOF, TFF, TCF, THF, TXF, NXF/NHF.
- Two opt-in feature flags (both off by default):
  - `cancellation` — cooperative parse cancellation via `set_cancel_flag` / `clear_cancel_flag`
  - `owned` — `OwnedTPTPProblem` / `parse_tptp_file` that own their data (no lifetime parameter)
- `mrs` uses neither feature flag (default features only).

**Key public API used by `mrs`:**

| Symbol | Description |
|--------|-------------|
| `parse_tptp(input: &str)` | Parse a full problem into `TPTPProblem<'_>` |
| `TPTPIterator::new(input)` | Streaming iterator, one `TPTPInput` per item |
| `TPTPInput::{Formula, Include}` | Variants yielded by the iterator |
| `AnnotatedFormula::{FOF, CNF, TFF, …}` | Enum over dialect-specific annotated formulas |
| `FormulaRole` | `Axiom`, `Conjecture`, `NegatedConjecture`, `Type`, `Definition`, … |
| `ParseError` | Carries byte offset; `.line()`, `.column()`, `.snippet()` helpers |

**Testing mrs-tptp:**

```bash
# from mrs/ workspace root
cargo test -p mrs-tptp                          # unit + integration tests
cargo test -p mrs-tptp parser_tests             # integration tests only
cargo test -p mrs-tptp -- --nocapture           # see stdout

# runnable examples (from mrs/ workspace root)
cargo run -p mrs-tptp --example parse_file
cargo run --release -p mrs-tptp --example parse_folder -- /path/to/TPTP --timeout 5000 --threads 4
```

Integration tests live in `crates/mrs-tptp-parser/tests/` (not inline): `parser_tests.rs`, `non_classical_tests.rs`, `syn000_tests.rs`, plus `tests/resources/` fixtures.

## Toolchain

- Rust edition **2024**, resolver **3** — requires stable ≥ 1.85.
- Check version: `rustup show`. Update if needed: `rustup update stable`.

## Developer commands

```bash
cargo build                          # debug build
cargo build --release                # release (use for benchmarking)
cargo check                          # fast type-check, no output
cargo clippy --workspace             # lint
cargo fmt                            # format
cargo fmt --check                    # CI-style format check

cargo test --workspace               # all tests (use --workspace; bare cargo test only runs root crate)
cargo test -p mrs-search             # single crate
cargo test -p mrs-calculus resolution  # single test (substring match)
cargo test -p mrs-search -- --nocapture  # show stdout

# Run the binary on a TPTP problem file
cargo run -- problems/socrates.p
cargo run --release -- problems/pel1.p
# Expected output: lines starting with "% SZS status ..."
```

## Workspace layout

```
mrs/                  ← workspace root AND the binary crate (src/main.rs)
├── src/
│   ├── main.rs       ← CLI entrypoint; orchestrates the full pipeline
│   ├── lowering.rs   ← TPTP AST → mrs-core types
│   └── include.rs    ← resolves TPTP %include directives
├── crates/
│   ├── mrs-core/     ← Term, Formula, Clause, Literal, Substitution, SymbolTable
│   ├── mrs-szs/      ← SZS status enum + formatting
│   ├── mrs-cnf/      ← clausification: NNF, Skolemization, definitional CNF
│   ├── mrs-unify/    ← Robinson unification + matching
│   ├── mrs-calculus/ ← inference rules, KBO/LPO ordering, literal selection
│   ├── mrs-index/    ← discrimination tree indexing (indirect dep via mrs-search)
│   ├── mrs-proof/    ← proof extraction + TSTP output
│   ├── mrs-search/   ← given-clause loop, clause weighting, strategy scheduler
│   └── mrs-tptp-parser/ ← TPTP parser (crate name: mrs-tptp)
└── problems/         ← curated TPTP .p files for manual testing (not wired into cargo test)
```

The root `Cargo.toml` is both `[workspace]` and `[package]` — valid but unusual.

## Architecture notes

- **Strategy portfolio:** 9 strategies run **serially**, each with a fresh `SearchState`. No shared state between strategies.
- **Default time budget:** 30 seconds, hardcoded in `main.rs`. No CLI flag yet.
- **`max_clauses`:** 50,000 per strategy. Hitting this gives `ResourceOut`, not `Timeout`.
- **Refutation-based:** conjectures are negated before search. A problem with no `conjecture` role checks satisfiability (outputs `Unsatisfiable`/`Satisfiable`).
- **TSTP proof output** only on `Refutation`; other statuses produce only the SZS status line.

## Testing

- Most tests are `#[cfg(test)]` inline modules — no separate test directories, no fixtures.
- Exception: `mrs-tptp` has integration tests under `crates/mrs-tptp-parser/tests/` with fixture files in `tests/resources/`.
- Some `mrs-search` and `mrs-calculus` tests run the full given-clause loop with real `Duration::from_secs(5)` timeouts.
- `problems/` is for manual binary runs only, not `cargo test`.

## Runtime env var

`TPTP=/path/to/TPTP` — only needed at runtime when problems use `%include` pointing to the standard TPTP library. Not required for local `problems/` files.
