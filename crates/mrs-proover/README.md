# mrs-proover

Proof verifier for TPTP/TSTP refutation proofs, targeting the
[ProoVer 2026](https://proover-competition.github.io/) competition.

## What it does

Reads a TSTP proof file (FOF, refutation-by-`$false`) together with its linked
problem file, and prints one of:

- `% SZS status Verified`
- `% SZS status FailedVerified : <reason>`
- `% SZS status NotVerified : <reason>`

## Build

```sh
cargo build --release -p mrs-proover
```

The binary lands at `target/release/mrs-proover`.

## Usage

```sh
mrs-proover [--problems-dir DIR] [--no-atp]
            [--eprover PATH] [--vampire PATH]
            <proof.p>
```

The proof file is expected to contain a header line of the form

```
% Proof : Problems/foo.p
```

pointing at the matching problem file. The path is resolved relative to the
proof file's directory; pass `--problems-dir` to look elsewhere.

By default the verifier auto-discovers `eprover` and `vampire` (in
`crates/mrs-bench/systems/{eprover,vampire}/bin/`) and uses them as an ordered
ladder for inference steps it cannot decide internally. Use `--no-atp` to
disable external ATP calls, or `MRS_PROOVER_EPROVER=/path` /
`MRS_PROOVER_VAMPIRE=/path` to override the discovery.

## Architecture

| Layer | What it does |
|---|---|
| `load` | Parse proof + linked problem with `mrs-tptp`. |
| `dag` | Build the proof DAG, check cycles, locate the `$false` root. |
| `lower` | Convert FOF AST → `mrs-core` `Formula`. |
| `checks::axiom_leaf` | Compare leaf nodes α-equivalently against the named axiom in the problem file. For anonymous `file(_,unknown)` leaves from pre-clausifying provers, also try matching against the CNF (`mrs-cnf::clausify`) of each problem formula modulo α/AC, upgrading `Unknown`→`Verified` without ever introducing a new `Unsound`. |
| `checks::neg_conjecture` | Verify NNF(¬conjecture) ≡α NNF(step). |
| `checks::skolemize` | Enforce: status `esa`, fresh Skolem symbol, dependency tuple matches the in-scope universals, conclusion is exactly parent[Var ↦ sK(args)]. |
| `atp::external` | Spawn `eprover` / `vampire` to discharge other steps. |
| `atp::ladder` | Try multiple backends in order; first definite verdict wins. |
| `verdict` | Aggregate: any `Unsound` → `FailedVerified`; else any `Unknown` → `NotVerified`; else `Verified`. |

The verdict policy is deliberately **conservative**: the competition scoring
penalises `bad→good` ten times more than `good→bad`, so the verifier only
emits `Verified` when every step is positively confirmed.

## Tested against the published examples

The 7 official examples (`example{1,2,3}_c_proof.p` and
`example{1,2,3,4}_e_proof.p` from <https://proover-competition.github.io/example-proofs/>)
are in `tests/fixtures/`. Running the binary on them yields:

| File | Expected | Got |
|---|---|---|
| `example1_c_proof.p` | Verified | Verified |
| `example2_c_proof.p` | Verified | Verified |
| `example3_c_proof.p` | Verified | Verified |
| `example1_e_proof.p` | FailedVerified | FailedVerified (wrong `¬∀X.p(X)`) |
| `example2_e_proof.p` | FailedVerified | FailedVerified (axiom mismatch) |
| `example3_e_proof.p` | FailedVerified | FailedVerified (Skolem symbol reuse) |
| `example4_e_proof.p` | FailedVerified | FailedVerified (substitution shape) |

## Competition packaging

A ready-to-use wrapper script lives at
`crates/mrs-bench/systems/mrs-proover/invoke.sh`. It picks up the release
binary, the bundled `eprover` and `vampire` if present, and enforces the
wall-clock budget.

```sh
crates/mrs-bench/systems/mrs-proover/invoke.sh <proof.p> 30
```
