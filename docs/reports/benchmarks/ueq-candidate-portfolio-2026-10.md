# UEQ Greedy Candidate Cooperative Portfolio Report (October 2026)

> Status: exploratory cooperative runs with raw SZS outcomes only. The candidate
> artifacts have **not** received a strict proof audit and are not a verified
> score or a replacement for any certified baseline.
>
> Candidate portfolio: `MRS_PORTFOLIO=6,12,2,11,1,8,7,15` (8 workers).
> Campaign artifacts:
> - CASC-30: `~/crates/mrs-bench/results/cooperative-ueq-greedy-candidate-01-casc-30-20261026`
> - CASC-J13: `~/crates/mrs-bench/results/cooperative-ueq-greedy-candidate-02-casc-j13-20261026`

---

## 1. Summary

The portfolio was selected from solo-strategy greedy coverage and then run once
on each edition with cooperative search, eight workers, and cross-strategy
sharing disabled. It returned 128 SZS `Unsatisfiable` statuses on CASC-30 and
117 on CASC-J13. Those are raw search outcomes, **not certified refutations**.

The proof artifacts need attention before the run can support a score claim:

- CASC-30: 79 of 128 refutations have nonempty TSTP proof blocks; **49 have an
  empty proof block**.
- CASC-J13: 81 of 117 have nonempty TSTP proof blocks; **36 have an empty proof
  block**.
- No strict audit is recorded for either candidate run. The nonempty proofs are
  therefore not yet `VerifiedGood`, and the empty proof blocks provide no
  derivation to audit.

Against the October 21 standalone run artifacts, the raw solve-set comparison
is **+5** on CASC-30 and **−2** on CASC-J13. This is not a controlled or
replicated portfolio A/B: the candidate and baseline runs use different source
revisions, hosts, and binaries. The evidence does not justify promoting the
candidate or changing the shipped `CASC_UEQ_ORDER`.

## 2. Campaign provenance

Both candidate runs recorded commit `ff316de52f297c5014722ef04efec594d3b1bb1a`
on branch `main`, with `git_dirty=true`. They used `MRS_HARDWARE=casc`,
`MRS_WORKERS=8`, `MRS_SHARED_POOL_INTERVAL=0`, and `jobs=1`. The per-problem
limits were 240 seconds for CASC-30 and 180 seconds for CASC-J13. The records
have different binary hashes, so the two editions should not be treated as
byte-identical builds.

| Parameter | CASC-30 | CASC-J13 |
|:---|:---|:---|
| Result directory | `cooperative-ueq-greedy-candidate-01-casc-30-20261026` | `cooperative-ueq-greedy-candidate-02-casc-j13-20261026` |
| Run ID | `mrs_bench_20261026_183945_3f375994` | `mrs_bench_20261026_184338_85400095` |
| Edition / problems | CASC-30 UEQ / 300 | CASC-J13 UEQ / 400 |
| Time limit | 240 s | 180 s |
| Portfolio | `6,12,2,11,1,8,7,15` | `6,12,2,11,1,8,7,15` |
| Binary SHA-256 | `c098777a39070d678b32bf9902e02d6eda00d9e9604373228bb939cc52f0a6c1` | `b6b9dc306eb6c699e20c72c4990a1f4b38cc141cf7a5f57620fb3a92ef71f326` |
| Host | `mtsdev01` (8-core Xeon E5-2407) | `mtsdev02` (8-core Xeon E5-2407) |
| Rust | `1.99.1` | `1.99.1` |

## 3. Outcomes and proof-output status

Counts below are from each run's `run.csv` and SZS output. `Nonempty proof
blocks` means proof text was present; it does **not** mean that the proof passed
the strict kernel.

| Edition | SZS `Unsatisfiable` | `Timeout` | `GaveUp` | Other | Nonempty proof blocks | Empty proof blocks |
|:---|---:|---:|---:|---:|---:|---:|
| CASC-30 | 128 | 166 | 6 | 0 | 79 | **49** |
| CASC-J13 | 117 | 269 | 14 | 0 | 81 | **36** |

The empty cases have an SZS proof-start/proof-end envelope but no `cnf(...)` or
`fof(...)` derivation lines. Examples include `GRP167-1` and `GRP185-2` in
CASC-30, and `GRP178-1` and `LAT017-1` in CASC-J13. The available campaign
artifacts do not retain the proof DAGs, and both recorded binaries were dirty
tree builds. Subsequent source review found a topological-sort failure mode for
AC inferences with cited AC axioms; the code fix and evidence limits are recorded
in [UI-11](../../policies/unresolved-issues.md#ui-11--ueq-candidate-refutations-have-empty-tstp-proof-blocks).
This explains the matching source path but does not prove that every archived
empty block followed it or certify any candidate status.

### CASC-J13 reference-label caveat

The CASC-J13 `run.csv` has 168 rows with a blank `expected` value. Of the 117
SZS `Unsatisfiable` statuses, 70 have `verdict=ok` and 47 have
`verdict=unknown` because their expected label is blank. Use the SZS status
count when describing this raw run; neither field is a strict proof-audit
verdict.

## 4. Comparison with the October 21 runs

This compares raw `Unsatisfiable` problem sets against the October 21 standalone
campaign artifacts, not against a current certified baseline:

- CASC-30: `~/crates/mrs-bench/results/campaign-casc30-ueq-W8CASCHJ1-02-20261021`
- CASC-J13: `~/crates/mrs-bench/results/campaign-casc-j13-04-ueq-CASCHJ1-20261021`

The October 21 baselines were run on `mtsdev02` (CASC-30) and `mtsdev04`
(CASC-J13), rather than the candidate hosts `mtsdev01` and `mtsdev02`.

| Edition | Oct 21 raw refutations | Candidate raw refutations | In both | Candidate-only | Oct 21-only | Raw count delta |
|:---|---:|---:|---:|---:|---:|---:|
| CASC-30 | 123 | 128 | 114 | 14 | 9 | **+5** |
| CASC-J13 | 119 | 117 | 106 | 11 | 13 | **−2** |

**CASC-30 candidate-only:** `BOO014-10`, `COL009-1`, `GRP184-1`, `LAT082-1`,
`LAT392-1`, `LCL341-10`, `LCL400-10`, `REL005-1`, `REL021-1`, `REL034-1`,
`REL034-2`, `REL035-1`, `REL036-1`, `RNG006-10`.

**CASC-30 Oct 21-only:** `ALG240-1`, `GRP183-2`, `GRP654-12`, `GRP655-11`,
`GRP753-1`, `REL008-3`, `REL026-3`, `RNG001-10`, `SYN708-10`.

**CASC-J13 candidate-only:** `GRP667-10`, `GRP727-1`, `KLE147-10`, `LAT082-1`,
`LAT086-1`, `LAT087-1`, `LAT157-1`, `LAT241-10`, `LAT392-1`, `LCL400-10`,
`NUN133-1`.

**CASC-J13 Oct 21-only:** `DAT436-1`, `DAT436-2`, `GRP654-12`, `GRP655-11`,
`GRP655-12`, `GRP655-13`, `GRP753-1`, `LAT022-1`, `LAT171-1`, `REL012-1`,
`REL024-2`, `REL026-3`, `SYN708-10`.

The solve-set changes are observations, not causal effects of the portfolio:
the candidate artifacts are dirty-tree builds at `ff316de`, whereas the
October 21 baselines were produced by an earlier revision; the hosts also
differ. The candidate result consists of one run per edition, and the proof
output gaps prevent treating its raw statuses as verified results.

## 5. Decision and next steps

Do not replace the shipped UEQ order on the basis of these runs. First:

1. Strictly audit all nonempty candidate proofs and preserve the audit outputs.
2. Keep the archived empty-proof outcomes labeled as unaudited: the source-level
   serialization failure is fixed, but the archived artifacts contain no proof
   DAG to audit and do not establish which individual rows took that path.
3. If the candidate remains viable, run matched baseline/candidate comparisons
   using the same build and host, with at least five replicates as required by
   the portfolio-measurement guidance in `AGENTS.md` §11.

Until then, keep these counts labeled as **raw, unaudited October 26
observations**. The previously documented verified baselines and the shipped
`CASC_UEQ_ORDER` remain unchanged.
