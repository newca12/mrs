# TPTP and TSTP Reference

## Input

`mrs-tptp` is a zero-copy parser for the TPTP family. The parser recognizes CNF,
FOF, TFF, TCF, THF, TXF, and NXF/NHF-related syntax. The root prover lowers a
supported subset into first-order core clauses; parsed constructs outside the
search engine's supported logical fragment fail closed rather than becoming a
positive result.

The binary accepts a problem path and derives the problem name from its file
stem. A path of `-` means stdin only in a build with the `proover` feature.

## Includes

Problems using the standard TPTP library need a local TPTP root:

```bash
TPTP=/path/to/TPTP-v9.x.x \
  nix develop -c cargo run -- problem.p
```

The benchmark wrapper sets `TPTP` to the selected extracted corpus. Do not
benchmark a sliced problem with a global unsliced TPTP root: include drift can
load a much larger axiom library and invalidate performance measurements.

For strict checking of stdin, provide an explicit include root:

```bash
cat problem.p | TPTP=/path/to/TPTP-v9.x.x \
  nix develop -c cargo run --features proover -- \
  --self-check --include-root /path/to/TPTP-v9.x.x -
```

## Output

Successful refutations begin with one of:

```text
% SZS status Theorem for problem
% SZS status Unsatisfiable for problem
```

An unverified refutation is not printed in `--self-check` mode. Ordinary
successful refutations include a TSTP proof block. Telemetry is emitted as
`% SZS detail` output when non-quiet mode is active.

The proof contains explicit provenance for input leaves, conjecture negation,
NNF, Skolemization, definitional CNF, inference parents, and AVATAR or InstGen
certificates where applicable.

## Proof verification

`mrs-proover` expects a TSTP proof with a `% Proof : ...` link to the associated
problem. Strict mode requires a parseable problem, resolved includes, named
provenance, an acyclic proof DAG, and a reachable `$false` root. Unsupported
rules or resource exhaustion produce `Unknown`/`Inconclusive`, not acceptance.

See [Trust and verification](trust-and-verification.md) for the complete
contract and [ProoVer](../guides/proover.md) for commands.
