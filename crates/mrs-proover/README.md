# mrs-proover

Proof verifier for TPTP/TSTP refutation proofs, targeting the
[ProoVer 2026](https://proover-competition.github.io/) competition.

## What it does

Reads a TSTP proof file (FOF, refutation-by-`$false`) together with its linked
problem file, and prints one of:

- `% SZS status VerifiedGood`
- `% SZS status VerifiedBad : <reason>`
- `% SZS status Unknown : <reason>`

## Build

```sh
cargo build --release -p mrs-proover
```

The binary lands at `target/release/mrs-proover`.

## Usage

```sh
mrs-proover [--problems-dir DIR] [--no-atp] [--no-mrs] [--no-fmb]
            [--eprover PATH] [--vampire PATH]
            [--only-mrs|--only-eprover|--only-vampire]
            [--time SECS] [--workers N] [--verbose]
            <proof.p>
```

The proof file is expected to contain a header line of the form

```
% Proof : Problems/foo.p
```

pointing at the matching problem file. The path is resolved relative to the
proof file's directory; pass `--problems-dir` to look elsewhere.

By default the verifier auto-discovers `eprover` and `vampire` (in
`crates/mrs-bench/systems/{eprover,vampire}/bin/`) and uses them, together
with the in-process `mrs` fallback, as an ATP ladder for inference steps it
cannot decide internally. Use `--no-atp` to disable external ATP calls
entirely, `--only-mrs`/`--only-eprover`/`--only-vampire` to restrict the
ladder to a single backend, or `MRS_PROOVER_EPROVER=/path` /
`MRS_PROOVER_VAMPIRE=/path` to override binary discovery.

`--workers N` (default `8`, matching the 8-core CASC/ProoVer competition
hardware) controls how many proof steps are verified concurrently — see
`Settings::workers` in `src/verify.rs`. Independently, within each step the
ladder now runs the in-process `mrs` backend first (cheap, no subprocess
spawn), then races any remaining external backends (`eprover`, `vampire`) in
parallel rather than trying them one after another: the first `Sound`/
`Unsound` verdict wins and the losing backend(s) are cancelled (killed) via a
shared `AtomicBool` flag, so a step that only `eprover` can close no longer
pays the full `vampire` budget (or vice versa) on top of it.

## Architecture

| Layer | What it does |
|---|---|
| `load` | Parse proof + linked problem with `mrs-tptp`. |
| `dag` | Build the proof DAG, check cycles, locate the `$false` root. |
| `lower` | Convert FOF AST → `mrs-core` `Formula`. |
| `checks::axiom_leaf` | Compare leaf nodes α-equivalently against the named axiom in the problem file. For anonymous `file(_,unknown)` leaves from pre-clausifying provers, also try matching against the CNF (`mrs-cnf::clausify`) of each problem formula modulo α/AC, upgrading `Unknown`→`VerifiedGood` without ever introducing a new `Unsound`. |
| `checks::neg_conjecture` | Verify NNF(¬conjecture) ≡α NNF(step). |
| `checks::skolemize` | Enforce: status `esa`, fresh Skolem symbol, dependency tuple matches the in-scope universals, conclusion is exactly parent[Var ↦ sK(args)]. |
| `atp::external` | Spawn `eprover` / `vampire` to discharge other steps. |
| `atp::ladder` | Run `mrs` in-process first, then race remaining external backends (`eprover`, `vampire`) in parallel per step; the first definite verdict wins and cancels the rest. |
| `verdict` | Aggregate: any `Unsound` → `VerifiedBad`; else any per-step `Unknown` → `Verdict::Unknown`; else `VerifiedGood`. |

The verdict policy is deliberately **conservative**: the competition scoring
penalises `bad→good` ten times more than `good→bad`, so the verifier only
emits `VerifiedGood` when every step is positively confirmed.

## Tested against the published examples

The 7 official examples (`example{1,2,3}_c_proof.p` and
`example{1,2,3,4}_e_proof.p` from <https://proover-competition.github.io/example-proofs/>)
are in `tests/fixtures/`. Running the binary on them yields:

| File | Expected | Got |
|---|---|---|
| `example1_c_proof.p` | VerifiedGood | VerifiedGood |
| `example2_c_proof.p` | VerifiedGood | VerifiedGood |
| `example3_c_proof.p` | VerifiedGood | VerifiedGood |
| `example1_e_proof.p` | VerifiedBad | VerifiedBad (wrong `¬∀X.p(X)`) |
| `example2_e_proof.p` | VerifiedBad | VerifiedBad (axiom mismatch) |
| `example3_e_proof.p` | VerifiedBad | VerifiedBad (Skolem symbol reuse) |
| `example4_e_proof.p` | VerifiedBad | VerifiedBad (substitution shape) |

## Confirmed sound with `--only-mrs` (no eprover/vampire needed)

`mrs-proover` was cross-checked against two external reference corpora —
[leoprover/noergler](https://github.com/leoprover/noergler) (PyRes original +
falsified proof pairs) and
[ValueAchooMatthew/ATP-Research-Project](https://github.com/ValueAchooMatthew/ATP-Research-Project)
(`tests/examples/{correct,incorrect,samples}`, also exercised by
`crates/mrs-proover/tests/atp_research_project.rs`) — running with `--only-mrs`
to force the internal `MrsAtp` fallback and exclude `eprover`/`vampire`
entirely, then compared against the default full ladder (`eprover` + `vampire`
+ `mrs`):

| Corpus | `--only-mrs` | Full ladder |
|---|---|---|
| Built-in `proover-corpus` (25 problems, 46 E/Vampire proofs) | 43 VerifiedGood / 3 Unknown / **0 VerifiedBad** | identical: 43/3/0 |
| ATP-Research-Project `correct` (3) | 3/3 VerifiedGood | 3/3 VerifiedGood |
| ATP-Research-Project `incorrect` (4 "evil") | 3/4 correctly `VerifiedBad`, 1 (`EVL002+1`) degrades to `Unknown` | 4/4 `VerifiedBad` |
| ATP-Research-Project `samples` (3) | `COR000+1` VerifiedGood, `EVL000+1` VerifiedBad, `TMO000+1` degrades to `Unknown` | all 3 match expectations |
| noergler PyRes **original** (170 valid proofs) | 162 VerifiedGood / 8 Unknown / **0 VerifiedBad** | 170/170 VerifiedGood |
| noergler PyRes **falsified** (170 mutated/evil proofs) | 39 `VerifiedBad` / 131 `Unknown` / **0 VerifiedGood** | 165 `VerifiedBad` / 5 `Unknown` / **0 VerifiedGood** |

**Takeaway:** dropping `eprover`/`vampire` never turns a valid proof into
`VerifiedBad` and never turns an evil proof into `VerifiedGood` — the critical
soundness invariant (`bad→good` never happens) holds with `mrs` alone. The
only cost is *detection strength*: without the external ATPs to discharge a
few inference steps within the time budget, some proofs that would otherwise
resolve to `VerifiedGood`/`VerifiedBad` instead give up safely as `Unknown`
(0 points, never wrong). Concretely, `--only-mrs` loses positive detection on
2 of the 10 ATP-Research-Project cases and roughly a third of the noergler
PyRes corpus, with zero soundness regressions anywhere. This confirms
`mrs-proover --only-mrs` is safe to run standalone (e.g. from `mrs-codex`'s
automatic post-proof verification), with no hard dependency on bundled
`eprover`/`vampire` binaries.

## Competition packaging

A ready-to-use wrapper script lives at
`crates/mrs-bench/systems/mrs-proover/invoke.sh`. It picks up the release
binary, the bundled `eprover` and `vampire` if present, and enforces the
wall-clock budget.

```sh
crates/mrs-bench/systems/mrs-proover/invoke.sh <proof.p> 30
```
