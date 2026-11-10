# TPTP 10 parser support and review

## Scope

The parser accepts the TPTP v10.0.0 corpus and checks structural
`parse -> Display -> parse` equality, not just text stability. This is parser
coverage, not a claim that the prover implements every parsed logic or theory.

The update adds back-quoted atomic words and names, signed integer names,
datatype/codatatype roles, distinct-object typing, and the current include
selection/space-name syntax. It also fixes conditional and non-classical
formula round-trips, truth-keyword boundaries, non-nesting block comments, and
UTF-8 escape handling. Unknown base roles are rejected; unclassified subroles
retain the semantics of their known base role.

Public AST changes include new enum variants, `Include::space_name`, and the
change from `AtomicWord` to `TypedSymbol` in `TCFTyping::symbol`. Downstream
exhaustive matches and struct literals need updating. The manifest is not
being released as part of this checkpoint; the next published parser version
must account for these breaking API changes.

## Corpus results

Corpus: `/home/hack/TPTP-v10.0.0`.

| Directory | Files | Formulas | Includes | Parse failures | AST round-trip failures |
| --- | ---: | ---: | ---: | ---: | ---: |
| Problems | 27,053 | 39,663,070 | 216,880 | 0 | 0 |
| Axioms | 2,414 | 6,220,418 | 0 | 0 | 0 |

Commands (run in the loaded development environment):

```bash
cargo build --release -p mrs-tptp --example parse_folder
target/release/examples/parse_folder /path/to/TPTP-v10.0.0/Problems --threads 4
target/release/examples/parse_folder /path/to/TPTP-v10.0.0/Axioms --threads 1
```

The sweep streams individual items to avoid retaining whole generated files as
ASTs. It checks formulas and includes, requires full input consumption, and
exits unsuccessfully on parse errors, round-trip mismatches, or timeouts.

## Prover integration safeguards

Unsupported goals are counted, including selected included goals, and cause
`GaveUp`. Unsupported non-goal formulas and unresolved includes prevent positive
satisfiability claims. They do not prevent searching a supported subset for a
refutation. CNF/TCF truth constants are conservatively unsupported rather than
being interned as unconstrained predicates.

These guards do not implement typed arithmetic. In particular, defined and
system words that currently lower as uninterpreted symbols need a separate
theory-aware implementation before a positive arithmetic result is justified.
Arithmetic is separate follow-up work, not covered by the corpus counts above.

## Verification

The checkpoint uses the repository gates (`cargo check`,
`cargo clippy --all -- -D warnings`, `cargo fmt --all --check`, and
`cargo test --workspace`) through `nix develop -c`, plus all-feature parser
tests and parser all-target/all-feature clippy. The larger workspace-wide
`--all-targets` clippy invocation has pre-existing test-only lints outside
the parser; it is not the repository's canonical lint gate.

The final workspace run passed 1,630 tests in 76 suites with
`RUST_TEST_THREADS=2`. An earlier run concurrent with the sweeps hit the existing
two-second wall-clock assertion in
`verus::process_tests::failed_malformed_and_hanging_peers_cannot_certify`.
That test passed in isolation and in the full rerun; its assertion was not
changed or disabled.
