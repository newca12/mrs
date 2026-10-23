# casc-30 FEQ silent kills: the SOS pre-flight is quadratic

The 12 casc-30 FEQ problems recorded with empty stdout and no `% SZS detail` are
**not** the UEQ defect fixed in `a67e523`. They are a different bug, in a
different phase of the run, and the UEQ fix does not touch them.

| | |
|---|---|
| Status | Diagnosed and fixed |
| Severity | 12 of 400 casc-30 FEQ (3%) |
| Soundness | Not a false-positive issue, but the fix must not be a lossy shortcut |

## Summary

`Strategy 10` is the only strategy in the casc-30 FEQ portfolio that sets
`sos_depth`. Before the given-clause loop starts, the search runs
`sos_blocks_every_input_inference` (`given_clause.rs:1070`) as a pre-flight, and
that function enumerates **every ordered pair of input clauses**. For an FEQ
input whose FOF axioms expand to ~10^5 clauses after clausification, the scan is
~1.1x10^10 pairs and takes hours. It has no deadline check, and it runs before
the iteration loop, so nothing is printed and nothing can interrupt it.

## Locating it

`CASC_FEQ_ORDER = [11, 12, 1, 6, 10, 8, 14, 4, 5, 2, 3, 7, 9, 13, 15]`, and
`build_casc_schedule_inner` (`named.rs:545-549`) builds the schedule as
`order[0..workers]`. The portfolio is therefore a **prefix**, which makes a
worker-count sweep a strategy bisect:

| workers | strategies | result on `SWX070+1` |
|---:|---|---|
| 1 | `[11]` | clean |
| 2 | `[11, 12]` | clean |
| 3 | `[11, 12, 1]` | clean |
| 4 | `[11, 12, 1, 6]` | clean |
| 5 | `[11, 12, 1, 6, **10**]` | **hangs** |
| 8 | full prefix | hangs |

Strategy 10 is added at prefix index 4. Of `[11, 12, 1, 6, 10, 8, 14, 4]`, only
10 sets `sos_depth` (`strategy.rs:280`), and `--strategy 8 --workers 1` on the
same problem is clean — which initially suggested strategy 8 and was **wrong**,
since 8 is not in the five-strategy prefix that hangs.

Measured on `mtsdev02` (8 cores / 96 GB), `--time 238`, current `main`:

| problem | workers | outcome |
|---|---:|---|
| `ALG215+2` | 8 | `Timeout`, `elapsed_ms=238397` |
| `CSR037+5` | 8 | `GaveUp`, `elapsed_ms=242568` |
| `NUM925+3` | 8 | `Timeout`, `elapsed_ms=239220` |
| `NUM925+7` | 8 | `Timeout`, `elapsed_ms=238444` |
| `BIO005+1`, `BIO006+1`, `CSR047+5`, `CSR052+4`, `HWV090+1`, `HWV128+1`, `ITP015+4`, `SWX070+1` | 8 | **no SZS status, killed at 320 s** |

## The mechanism

Instrumenting the pre-flight on `SWX070+1`, which has 148 FOF axioms and one
conjecture:

```
[SOS] pairs=58000000 i=548 j=36943 n=105772 elapsed=50.28s
```

`n=105772` is the post-clausification clause count, not the 148 formulas in the
file. After 50 s the scan had reached `i=548` of 105772, i.e. 0.5% of the outer
loop. Full cost is ~1.12x10^10 pairs at ~1.15M pairs/s, or **roughly 2.7 hours**.

The scan is not a bound check that returns early; it has to answer two questions
and the second one is what forces the full enumeration:

```rust
for i in 0..clauses.len() {
    for j in 0..clauses.len() {
        ...
        any_inference_at_all = true;
        if clauses[i].distance < sos_depth || clauses[j].distance < sos_depth {
            return false;                 // a support-set clause can infer: keep SOS
        }
    }
}
any_inference_at_all                       // else: SOS blocks every input inference
```

So it keeps scanning to the end unless it finds an inference-capable pair with a
member inside the support set. Its doc comment claims "the normal case costs a
handful of literal comparisons and only a genuinely blocked input pays for the
full pairwise check" — but `pair_can_resolve` already filters on both clauses
having selected literals, so most pairs are rejected only *after* the loop has
been entered, and the enumeration order does not favour finding a support-set
pair early.

## Why it looks input-dependent

Same strategy, same host, three self-contained inputs:

| problem | clauses after clausification | pairs | outcome |
|---|---:|---:|---|
| `NUM925+3` | 2 053 | 4 214 809 | full scan, ~3.7 s, then searches normally |
| `NUM925+7` | 1 989 | 3 956 121 | full scan, then searches normally |
| `SWX070+1` | **105 772** | ~1.1x10^10 | never completes |

The scan is quadratic, so the cost cliff is very sharp: ~10^6 clauses is a few
seconds, ~10^5 clauses is hours. That is why three of the twelve survive at
`--workers 8` and the rest do not, and it means the number of *formulas* in the
TPTP file predicts nothing. `SWX070+1` has 149 formulas; `CSR037+5` has 3.

## Why nothing is printed

`% Problem:` is emitted from `main.rs` after lowering but before clausification,
and it reports `0 cnf clauses` for every one of these. That count is
`lowered.cnf_clauses.len()` — clauses supplied *in the input* as CNF — and it
does not include clauses generated from FOF during clausification. So `0 cnf
clauses` is **not** evidence that clausification was still running, and the
earlier suggestion that FEQ hangs in a different phase from UEQ because of it is
wrong.

Direct evidence: on a 2-core / 6.7 GB box, `HWV128+1` reaches
`passive=580061` before hitting its memory ceiling, so clausification completes
and the search loop starts. The pre-flight sits at `given_clause.rs:1298`,
between clausification and the loop, and carries no deadline argument, so it is
invisible to `TRACE_PROGRESS`, to every `search_deadline` check, and to the
harness's 250 s outer timeout.

## Fixing it (applied)

Bounding the scan and defaulting to "not blocked" was rejected: it is
coverage-only in principle, but it silently disables SOS on exactly the large
FEQ inputs where SOS is the point. The applied fix indexes selected positive and
negative predicate literals by symbol, then examines only opposite-polarity
pairs within each group. Different predicate symbols cannot resolve, so this is
the **exact** candidate set, not an over-approximation. Each candidate pair
answers both whether an inference exists and whether a support-set parent
participates; unrelated clause pairs are never visited.

`SWX070+1` now reports `Timeout` at 238 s with `processed=6080 generated=98092`;
`NUM925+3` and `NUM925+7` are unchanged. Two tests pin the behaviour:
`sos_verdict_matches_an_all_pairs_reference` compares the verdict against a
brute-force all-pairs reference across four cases (support-connected inference,
only out-of-support inferences, no inference, and isolated support beside
out-of-support inferences), and
`sos_preflight_is_not_quadratic_in_the_clause_count` fails in 68 s over 4000
clauses if the quadratic enumeration returns.

## Reproduction

```bash
# Hangs: no SZS status, killed by the outer timeout
crates/mrs-bench/problems/casc-30/FEQ/SWX070+1.p   # 105772 clauses
timeout 320 ./target/release/mrs --time 238 --workers 1 --strategy 10 \
    --schedule casc_feq crates/mrs-bench/problems/casc-30/FEQ/SWX070+1.p

# Terminates: 2053 clauses, ~3.7 s of scan
timeout 320 ./target/release/mrs --time 238 --workers 1 --strategy 10 \
    --schedule casc_feq crates/mrs-bench/problems/casc-30/FEQ/NUM925+3.p
```

`SWX070+1` is self-contained. The other 11 need `TPTP` pointed at a tree with
`Axioms/`; the bundled `casc-30` corpus has it as a sibling of `FEQ`.
