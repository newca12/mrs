# CASC-30 FNE shipped-order campaign (2026-11-06)

> One complete 100-problem campaign using the shipped `casc_fne` order observed
> 46 theorems, all 46 strictly `VerifiedGood`. This is a single run from a dirty
> producer tree on a 32-core host; it is not a paired estimate, an 8-physical-core
> CASC-equivalent measurement, or an official score. The audit used the
> checked-out CASC-30 corpus because the producer's recorded `/DATA/...` corpus
> path is unavailable here.

## Results

| Outcome | Count |
|:---|---:|
| `Theorem` | **46** |
| Strict `VerifiedGood` | **46** |
| `VerifiedBad` / `Unknown` / verifier timeout / proof omitted | 0 |
| `GaveUp` | 13 |
| `Timeout` | 41 |
| `Error` | 0 |
| **Total** | **100** |

The strict audit covered all 46 emitted refutations with a 240-second per-proof
limit and four audit jobs. The 54 non-refutation rows were marked incomplete and
were not sent to proof replay. Solved time was 0.069–225.516 s (median 2.244 s;
mean 23.987 s). `CSR116+39` was the slowest proof, at 217.285 s; the next slowest
was `CSR116+6` at 225.516 s.

### Comparison with the October 21 same-order campaign

The October 21 CASC-30 candidate campaign used the same eight strategy IDs,
`[11,13,4,8,15,7,3,1]`, and reported 45 strict refutations. Comparing the
recorded solve sets gives 45 common theorems and one additional theorem in this
November 6 run: `CSR116+39` (217.285 s). No October 21 theorem was absent from
the November 6 solve set.

This is a cross-run set comparison, not evidence that the portfolio improved:
the producer binaries and source revisions differ, the producer tree was dirty,
and the campaigns were not a randomized paired experiment. The November run's
additional solve must be attributed to that run, not to an individual strategy
or a source change.

## Configuration and provenance

| Attribute | Value |
|:---|:---|
| Artifact directory | `~/crates/mrs-bench/results/campaign-casc-30-fne-W8CASCHJ1-99-20261106` |
| Run ID / timestamp | `mrs_bench_20261106_121354_21de8259` / `20261106_121354` |
| Division / corpus edition | FNE / CASC-30, 100 problems |
| Portfolio | `[11,13,4,8,15,7,3,1]` (`casc_fne` shipped order) |
| Search settings | 8 workers, `MRS_HARDWARE=casc`, `MRS_SHARED_POOL_INTERVAL=0`, `jobs=1` |
| Time limit | 240 s per problem (`use_casc_times=1`; metadata's 120 s default is overridden) |
| Host | `teenf9901`, Intel Xeon Silver 4108 @ 1.80 GHz, 32 reported cores, 128,019 MB RAM |
| Producer commit / tree | `c021be580651c0bbf1cefdf3c56f05081eb6610e`, dirty |
| Producer binary SHA-256 | `efb06b302c61a7127d6560dc5d6522549b35753ba3ece40c160acf63872180ff` |
| `run.csv` SHA-256 | `787df485614547a2325bd65cbda5baa7b9eb496d69f4a232c6ea0d600f1effe3` |
| `audit.csv` SHA-256 | `bff7a60154b596287b2dcea355e4dbfe9ebc5b60b2c641832953c698c00955ff` |
| Strict audit binary SHA-256 | `8f8c08ecb5c5554f343c76d9225d8c9d1fb028ea9f9c72243b848c8a4893124c` |
| Audited checkout corpus SHA-256 | `9a9cabd1a059f737c56353e01f7008597b96de7a312d0fb69559d8e15d77570a` |

The audit artifacts are in the campaign's `certification/` directory. The
producer metadata names `/DATA/ai/mrs/crates/mrs-bench/problems/casc-30`, which
is not available in this environment. The audit therefore used this checkout's
`crates/mrs-bench/problems/casc-30`; the audit identity records the corpus digest
above. The run metadata has no producer-corpus digest, so identity with the
producer's exact corpus cannot be independently established from this artifact.

Although the run used `MRS_HARDWARE=casc` and exactly eight search workers, this
host reported 32 cores. `casc` does not pin CPU affinity; this was not run on the
canonical eight-physical-core setup and should not be presented as a CASC
hardware-equivalent score. The three large `NLP260+1`, `NLP261+1`, and `NLP262+1`
runs completed with `GaveUp` rather than process errors, with peak RSS around
75–77 GB; this observation does not establish their behavior under a memory
ceiling or on an eight-core host.

## Interpretation

The result confirms that all 46 proofs emitted by this campaign replay in the
strict checker against the available CASC-30 corpus. The 46/100 solve count and
the one-solve difference from the October 21 same-order run remain single-run,
cross-build observations. They do not justify promoting or changing the shipped
FNE schedule. A schedule comparison should use randomized paired cooperative
campaigns with the same binary and host, report each pair's certified delta and
per-problem flips, and use held-out problems or families for promotion evidence;
see [methodology §8.5](../../policies/methodology.md#85-experimental-design-and-evidence-strength).
