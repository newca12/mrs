# Banked benchmark results

Dated, per-run artefacts. These are evidence, not current baselines: a number
here describes the commit and configuration named in the file that produced it.
Re-measure before quoting one.

## Canonical CASC Baselines & Trust Status

> **Reporting Rule:** Never quote unverified historical solve counts without stating
> their audit status. Superseded runs that permitted unsound inferences must not be
> presented as valid CASC scores.
>
> **FEQ candidate evidence caveat:** the October candidate portfolio runs are
> single-run observations, not five-replicate means. The CASC-J13 runs used
> 180 s, while current canonical FEQ comparisons use 240 s; treat J13 results
> as supplemental and all portfolio deltas as preliminary pending replicated,
> paired comparisons.

| Division | Edition | Evaluated | Current Verified Baseline | Historical Unverified Snapshot (`codex.db`) | Audit & Trust Notes |
|:---|:---|---:|---:|---:|:---|
| **UEQ** | CASC-J13 | 400 | **107/400 (26.75%; current strictly verified baseline, `c07cac9`)** | ~~257 (64.3%)~~ | The canonical baseline remains the 2026-09-30 `c07cac9` campaign: 107 kernel-certified `VerifiedGood`, 0 `VerifiedBad` (see [certification status](../CERTIFICATION_STATUS.md)). Later campaigns are archive-specific supplemental evidence, not replacements: Oct 21 recorded 117 `VerifiedGood`; Oct 28 recorded 120 `VerifiedGood` plus one `no_proof` (`REL045-2`) in a dirty adaptive-mode run with `MRS_WORKERS=8`. The 16-physical-core figure is host capacity, not worker count. Keep Oct 28 supplemental because it is a dirty, single-run adaptive measurement, not a standardized CASC-profile result. Historical 257 remains superseded. See the [CASC-J13 UEQ report](../reports/benchmarks/ueq-j13-2026-10.md) and [Oct 28 follow-up](../reports/benchmarks/ueq-follow-up-2026-10-28.md). |
| **UEQ** | CASC-30 | 300 | **119–120 (39.7–40.0%; 2026-10-20/21 standalone runs) / 127 (42.3%; 2026-10-07 run)** | ~~222 (74.0%)~~ | Replicated standalone CASC-mode runs: 120 `VerifiedGood` (Oct 20) and 119 `VerifiedGood` (Oct 21), 0 `VerifiedBad`, 4 `Unknown` across consecutive days (>99% replicate consistency). Oct 28 supplemental run recorded 124 `VerifiedGood` and 0 `VerifiedBad`, but used a dirty build on a 96 GB host below the 128 GB CASC allowance, so it is not promoted as a replacement baseline. The 127 baseline (Oct 7 on 16 cores) remains the peak trusted score. Historical 222 remains superseded. See the [UEQ report](../reports/benchmarks/ueq-2026-10.md) and [Oct 28 follow-up](../reports/benchmarks/ueq-follow-up-2026-10-28.md). |
| **FEQ** | CASC-J13 | 300 | **69/69 `VerifiedGood` (23.00%; 8-core, 180 s supplemental) / 71/72 `VerifiedGood` (23.67%; 32-core, 180 s supplemental) / 68 (old baseline)** | 72 (24.0%) | Candidate portfolio `12,11,10,6,8,1,4,13` produced 69 refutations (all `VerifiedGood`) on `mtsdev01` and 72 raw refutations (71 `VerifiedGood`, 1 proof omitted) on `tlpnf9701`. These historical 180 s runs are supplemental, not canonical 240 s comparisons; results are single-run observations. See the [candidate FEQ report](../reports/benchmarks/feq-candidate-portfolio-2026-10.md). |
| **FEQ** | CASC-30 | 400 | **106/111 `VerifiedGood` (26.50%; 8-core) / 107/113 `VerifiedGood` (26.75%; 32-core) / 103 (old baseline)** | 112 (28.0%) | Candidate portfolio `12,11,10,6,8,1,4,13` solved 111/400 on `mtsdev01` (106 `VerifiedGood`), retaining the observed 108 baseline refutations and adding 3 raw solves in that run. It solved 113/400 on `tlpnf9701` (107 `VerifiedGood`). The 114 figure on `teenf9901` includes a targeted `GEO559+1` solve, not a fifth full-division result. These are single-run observations, not five-replicate means; see the [candidate FEQ report](../reports/benchmarks/feq-candidate-portfolio-2026-10.md). |
| **FNE** | CASC-J13 | 100 | **39–40 (39.0–40.0%; candidate portfolio on 8-core host) / 36 (baseline)** | 35 (35.0%) | Candidate portfolio `11,13,4,8,15,7,3,1` achieves 40 refutations (39 `VerifiedGood`, 1 proof omitted) on canonical 8-core host `mtsdev03` (+4 over shipped 36 baseline). 38/38 certified on `tlpnf9701`. 0 `VerifiedBad` across all runs. See the [candidate FNE report](../reports/benchmarks/fne-candidate-portfolio-2026-10.md). |
| **FNE** | CASC-30 | 100 | **45 (45.0%; candidate portfolio) / 41 (41.0%; baseline)** | 43 (43.0%) | Candidate portfolio `11,13,4,8,15,7,3,1` solves 45/100 (45/45 strictly `VerifiedGood`, 0 `VerifiedBad`), retaining all 41 baseline solves and adding +4 hard LCL problems via Strategy 13. See the [candidate FNE report](../reports/benchmarks/fne-candidate-portfolio-2026-10.md). |
| **EPS** | CASC-30 | 100 | **44 (40 certified models; 2026-10-19)** | 17 (17.0%) | CASC-mode dual-search run; 0 invalid models, 4 inconclusive model checks. Same 40 certified models as 2026-09-28, with two more models certified (the corresponding raw `Satisfiable` answers were already present in September). |
| **EPU** | CASC-30 | 100 | **11 (11.0%; 2026-10-19 CASC-mode run)** | 18 (18.0%) | 11/11 strict `VerifiedGood`, 0 `VerifiedBad`; +2 over both audited September 26 and October 2 runs, same 9 retained. The 18 historical solves are not the current verified score. |
| **ICU** | CASC-30 | 101 | **4 (4.0%; 2026-10-19 CASC-mode run)** | 3 (3.0%) | 4/4 refutations strictly `VerifiedGood`, 0 `VerifiedBad`. Resolves prior `Unknown` checks on `CSE007+1` and `EEE001+1` via bounded NNF fix (`13653b9`). `EEE009+1` (certified in October 2 in 45.7 s) timed out, establishing intermittency. 44 of 101 problems carry non-decisive references (`expected=GaveUp`); score is 3/55 on decisive references. 1 OOM kill (`CSI008+1`, 2M axioms). See UI-6 and the [ICU report](../reports/benchmarks/icu-2026-10.md). |

---

## Intermittent certified solves

Per-problem list of division/edition results that are strictly certified in
some full-division campaigns and not certified in others:
[`intermittency.md`](intermittency.md). The inventory groups problems by their
run-to-run outcome signature and names the campaign keys, excludes historical
`VerifiedBad` results, and is maintained from audited campaign artifacts. CASC-30
ICU is included with 3 intermittent problems between the October 2 and October 19
campaigns (`c30-icu-02` and `c30-icu-19`).

---

## CASC-J13 combined full-division campaign, 2026-10-16

`campaign-casc-j13-W8C8J1-20261016` ran all three CASC-J13 divisions in one
campaign: 800 problems total, sequential jobs, eight workers, `MRS_HARDWARE=casc`,
and official 180-second division limits. Its strict audit certified **221/800**
results: **FEQ 72/300, FNE 36/100, UEQ 113/400**. All 72 FEQ and 36 FNE
refutations passed strict verification; UEQ had 113 `VerifiedGood`, one
`Unknown`, and no `VerifiedBad`. The remaining outcomes were 505 timeouts and
73 `GaveUp`; there were no resource-outs, errors, or other results.

| Division | Strict `VerifiedGood` | Comparison campaign | Common | Combined-only | Comparison-only |
|---|---:|---|---:|---|---|
| FEQ | 72/300 | `campaign-cascj13-feq-W8C8J1-20261016` (adaptive mode) | 70 | `GEO277+1`, `SWB073+1` | `SWW298+1` |
| FNE | 36/100 | `campaign-cascj13-fne-W8C8J1-20261002` | 31 | `CSR115+8`, `CSR116+19`, `KRS234+1`, `KRS261+1`, `SYO605+1` | — |
| UEQ | 113/400 | `ueq-syntactic-recovery-j13` (CASC mode, no sharing) | 109 | `ALG240-1`, `KLE145-10`, `KLE147-10`, `LAT222-10` | `REL019-1`, `REL026-3` |

These are solve-set comparisons, not controlled A/B estimates. The FEQ standalone
run used the same eight strategies but a different binary and adaptive mode; its
host had 16 physical cores and about 44 GB per-run memory, versus the combined
run's CASC-mode host. The FNE comparator is an earlier adaptive, 8-worker run;
the UEQ comparator is the separate CASC-mode recovery run on a different binary
and host. Portfolio search is timing-sensitive, so repeat matched runs before
assigning these deltas to a source change. The combined campaign is also dirty
tree evidence: `run_meta.json` records commit `dfef326c`, `git_dirty=true`,
binary SHA-256 `b551916d3d379087fdb99ef0486d7fc98430e54146e8943960346a662d4a5069`,
and Rust 1.99.1. The host had 128,019 MB RAM; `casc` therefore reported a
128-GB allowance with an effective budget below the allowance. The metadata's
120-second default was overridden by `use_casc_times=1`; the run log and rows
show the official 180-second clocks.

Full per-problem intermittency signatures are in
[`intermittency.md`](intermittency.md); campaign-level strict totals are in
[`CERTIFICATION_STATUS.md`](../CERTIFICATION_STATUS.md).

---

## CASC-30 combined FEQ/UEQ full-division campaign, 2026-10-16

`campaign-casc30-ueq-feq-W8CASCHJ1-20261016` ran both divisions sequentially
with eight workers, `MRS_HARDWARE=casc`, and the official 240-second limits on
the 8-core Xeon E5-2407 host. It covered all 700 problems. Strict audit gives:

| Division | Generated refutations | `VerifiedGood` | `VerifiedBad` | Other proof-audit results |
|---|---:|---:|---:|---|
| FEQ (400) | 109 | **105** | 2 | 1 `Unknown`, 1 `Timeout` |
| UEQ (300) | 124 | **118** | 5 | 1 refutation had no `$false` root and no omission marker |

Therefore the audited total is **223 strictly certified refutations**. Do not
report 233 raw refutation statuses (109+124) as certified: seven generated
proofs were rejected, the FEQ audit has one `Unknown` and one checker timeout,
and the UEQ audit has one malformed/incomplete proof artifact. No rows ended in
`ResourceOut`, process `Error`, or `Other`.

The rejection reasons are specific and should remain visible pending
investigation:

- **FEQ:** `KLE071+1` has inconsistent symbol arity in the proof signature;
  `SWV486+2` has an `ac_superposition` conclusion the checker cannot validate
  as a rewrite. `LCL894+1` is `Unknown` at an incomplete AC-superposition
  replay, and `ALG049+1` exceeded the strict verifier's 120-second limit.
- **UEQ:** `MVA008-1` and `MVA009-1` have cited axiom leaves that do not match
  `residual_b` / `residual_c`; `REL034-2` and `RNG009-5` have inconsistent
  symbol arities; `RNG009-7` has a cited `distribute2` leaf mismatch.
  `REL045-2` reports refutation but its proof has no `$false` root and no
  proof-omission marker, so it is not counted as a certified solve.

Compared with the October 12 CASC-mode FEQ campaign
(`campaign-casc30-feq-clean`), the new run has **66 certified problems in
common, 39 new, and 4 no longer certified**, raising strict coverage from 70 to
105. Compared with the October 7 audited CASC-30 UEQ run, it has **113 in
common, 5 new, and 14 no longer certified**, lowering strict coverage from 127
to 118. These are separate campaigns and binaries, not a controlled A/B; in
UEQ, the five rejected derivations mean the run must not be read as proof that
all emitted refutations are valid. Per-problem outcome signatures are tracked in
[`intermittency.md`](intermittency.md).

The host had 95,969 MB RAM, below the nominal 128-GiB allowance; per-problem
hardware records show an effective memory budget of 81,897 MB. Maximum observed
row peaks were 64,650 MB in FEQ (`CSR037+5`) and 61,248 MB in UEQ (`ROB005-1`),
with no memory-limit exit. Provenance is commit `dfef326c`, dirty tree, Rust
1.99.1, binary SHA-256
`7c9230d46623933a5ab7885d0e42bc6c9b2c02ddd4021af064da1e22bb39eb4f`; the run
used TPTP v9.3.0. Keep the strict certified counts distinct from raw search
coverage when quoting this campaign.

For context, the October 12 CASC-mode FEQ campaign certified 70, so the new
run has 39 certified solves not in that set and four no longer certified (66
common). The October 7 UEQ audit certified 127; this campaign has five certified
solves not in that set and 14 prior certified solves absent (113 common), for a
net decrease of nine. The FEQ result is an encouraging measured increase, but
both runs are single measurements with different binaries/hosts. The UEQ result
is not an acceptable score improvement: its five rejected derivations must be
resolved, and the 118 current `VerifiedGood` is below the 127 trusted campaign.
The problem-level signatures in [`intermittency.md`](intermittency.md) include
only strictly certified results, not the rejected raw refutations.

The current verified UEQ CASC-30 score remains the **127/300** result from the
October 7 audited campaign. The October 16 audit's five `VerifiedBad` outcomes
are specifically `MVA008-1`, `MVA009-1`, `REL034-2`, `RNG009-5`, and `RNG009-7`;
the separate missing-proof-root row is `REL045-2`. `VerifiedBad` records that
the archived derivation failed strict replay; it is not, by itself, a diagnosis
that all five are the same producer defect. The two MVA proof leaves do not
match the named residual axioms, two proofs use conflicting symbol arities,
and the RNG009-7 leaf does not match `distribute2`. Preserve these as audit
failures until their cause is resolved and the derivations are re-audited.

**HEAD re-audit (2026-10-20):** the seven archived proofs above were rechecked
individually with a fresh release `mrs-proover --strict --no-atp` built from
HEAD `070c7800fed2e7c34500f1e492f8c7553b125649`, against the CASC-30 corpus.
All seven reproduced as `VerifiedBad` with the same rejection reasons recorded
in the campaign audit. The verifier binary SHA-256 was
`f06c373901fd8620071a164e71a74f0217f5f60a70fe95c9160ac8c07448087d`. This
confirms the failures reproduce on current HEAD; it does not classify all of
them as one defect or change the trusted 127/300 UEQ baseline.

---

## CASC-30 FEQ standalone full-division campaigns, 2026-10-20 and 2026-10-21

Two consecutive campaigns evaluated all 400 problems in FEQ sequentially (`jobs=1`) with eight workers,
`MRS_HARDWARE=casc`, and official 240-second division limits on the 8-core Xeon E5-2407 host (`mtsdev01`):
- `campaign-casc30-feq-W8CASCHJ1-01-20261020` (Oct 20)
- `campaign-casc30-feq-W8CASCHJ1-01-20261021` (Oct 21)

| Campaign | Total | Refutations (`Theorem`) | `VerifiedGood` | `VerifiedBad` | `Unknown` | Verifier Timeout | Proof Omitted |
|:---|---:|---:|---:|---:|---:|---:|---:|
| **2026-10-20** | 400 | 108 | **103** | **0** | 2 | 1 | 2 |
| **2026-10-21** | 400 | 108 | **103** | **0** | 2 | 1 | 2 |

Key takeaways:
- **Zero Unsound Proofs (`VerifiedBad = 0`)**: Replicated across both consecutive runs, resolving
  both false/invalid proof rejections from the 2026-10-16 campaign (`KLE071+1` and `SWV486+2`),
  which now certify strictly as `VerifiedGood`.
- **Replicate Stability**: 107 of 108 solved problems are identical. Exactly one borderline pair
  swapped under concurrent thread racing near 240 s: `SWV458+1` solved in 28.4 s on Oct 20 but
  timed out on Oct 21; `SWV457+1` timed out on Oct 20 but solved in 153.4 s on Oct 21 (`VerifiedGood`).
- **Production Resolution of Quadratic SOS Pre-flight**: Problems that previously hung silently
  (`BIO005+1`, `BIO006+1`, `CSR047+5`, `CSR052+4`, etc.) completed cleanly; `BIO005+1` and `BIO006+1`
  both solved as strictly certified theorems in both campaigns.
- **Proof-size limits**: `SET017+1` (49.3 MB on Oct 20; 38.2 MB on Oct 21) and `SET072+1`
  (45.2 MB on Oct 20; 38.1 MB on Oct 21) derived refutations in both runs but withheld proof
  text exceeding 32 MB.
- **Residual gaps**: `KLE132+1` and `LCL894+1` remain `Unknown` on incomplete AC-superposition replay
  in both runs; `ALG049+1` (8.2k nodes) timed out in the verifier (125 s vs. 120 s limit).

Provenance: Oct 20 commit `a38c8bc8` (SHA-256 `8029b1d6c1fee6b5b533365823755a659bca31eb33e9d4e1e92e3e23cf48f762`);
Oct 21 commit `c45227ff` (SHA-256 `fd60bd269efe77c249ba2531605b6223fed510df275e0446008c9cf0fd106d19`). Detailed
analysis and domain breakdowns are in [`reports/benchmarks/feq-2026-10.md`](../reports/benchmarks/feq-2026-10.md).

---

## CASC-30 UEQ standalone full-division campaigns, 2026-10-20 and 2026-10-21

Two consecutive campaigns evaluated all 300 problems in UEQ sequentially (`jobs=1`) with eight workers,
`MRS_HARDWARE=casc`, and official 240-second division limits on the 8-core Xeon E5-2407 host (`mtsdev02`):
- `campaign-casc30-ueq-W8CASCHJ1-02-20261020` (Oct 20)
- `campaign-casc30-ueq-W8CASCHJ1-02-20261021` (Oct 21)

| Campaign | Total problems | Refutations (`Unsatisfiable`) | `VerifiedGood` | `VerifiedBad` | `Unknown` | Verifier Timeout | Proof Omitted |
|:---|---:|---:|---:|---:|---:|---:|---:|
| **2026-10-20** | 300 | 124 | **120** | **0** | 4 | 0 | 0 |
| **2026-10-21** | 300 | 123 | **119** | **0** | 4 | 0 | 0 |

Key takeaways:
- **Zero Unsound Proofs (`VerifiedBad = 0`)**: Replicated across both runs, confirming the
  elimination of the 5 rejection bugs and 1 missing root from the 2026-10-16 campaign:
  `MVA008-1`, `MVA009-1`, `REL034-2` (on Oct 20), and `REL045-2` certify strictly as `VerifiedGood`,
  while `RNG009-5` and `RNG009-7` properly fail closed as `Unknown`.
- **Replicate Consistency**: 123 of 124 solves from Oct 20 re-solved identically on Oct 21 (>99%
  solve overlap). Only `REL034-2` timed out (206.0 s on Oct 20 vs 238.8 s timeout on Oct 21).
- **Residual AC replay barrier**: Exactly the same 4 proofs return `Unknown` across both runs
  (`GRP179-2`, `REL027-3`, `RNG009-5`, `RNG009-7`), all due to incomplete AC superposition rewrite replay.
- **Canonical hardware baseline**: On the physical 8-core host, mrs reliably achieves
  119–120 strictly verified refutations within 7–8 solves of the 16-core ceiling (127).

Provenance: Oct 20 commit `a38c8bc8` (binary SHA-256 `21e72729ecf09c86b78f35caa56d91a9b06c969771988fac806dcc68d3987ecb`);
Oct 21 commit `c45227ff` (binary SHA-256 `eb9a70780d9249618e62468501ebc56bce280f53a0051570fde2f0e0d2a99435`). Detailed
analysis and domain breakdowns are in [`reports/benchmarks/ueq-2026-10.md`](../reports/benchmarks/ueq-2026-10.md).

---

## CASC-J13 FEQ standalone full-division campaigns, 2026-10-20 and 2026-10-21

Two consecutive campaigns evaluated all 300 problems in CASC-J13 FEQ sequentially (`jobs=1`) with
eight workers, `MRS_HARDWARE=casc`, and official 180-second division limits on `mtsdev03`:
- `campaign-casc-j13-03-feq-CASCHJ1-20261020` (Oct 20)
- `campaign-casc-j13-03-feq-CASCHJ1-20261021` (Oct 21)

| Campaign | Total | Refutations (`Theorem`) | `VerifiedGood` | `VerifiedBad` | `Unknown` | Verifier Timeout | Proof Omitted |
|:---|---:|---:|---:|---:|---:|---:|---:|
| **2026-10-20** | 300 | 68 | **68** | **0** | 0 | 0 | 0 |
| **2026-10-21** | 300 | 69 | **68** | **0** | 0 | 0 | 1 |

Key takeaways:
- **100% Strict Certification on Emitted Proofs**: Every single emitted refutation passed
  strict verification without exception across both runs (0 `VerifiedBad`, 0 `Unknown`).
- **All 68 Oct 20 Solves Replicated**: All 68 `VerifiedGood` refutations from Oct 20 re-solved
  and certified on Oct 21, including `MGT091+1` (89.96 s / 89.28 s) and `SET101+1` (0.44 s).
- **New Solve `SWV406+1`**: Solved on Oct 21 in 162.75 s wall time (vs 178.84 s timeout on Oct 20),
  deriving an 83.4 MB proof (319,894 nodes) that was withheld under `--proof-bytes-limit 32000000`.
- **Speed**: Solves in this division are predominantly sub-second or single-digit seconds,
  with median solve times under 1.9 s (over 46% solved in < 1 s, over 60% in < 5 s).
- **Large proofs verified**: Multiple proofs exceeded 8 MB (`MGT005+1` at 8.3 MB, `SEV677+1` at 16.9 MB,
  `MGT079+1` at 23.1 MB with 137k nodes), all certifying strictly in under 3 seconds verifier time.

Provenance: Oct 20 commit `a38c8bc8` (SHA-256 `f1c84643ab459fca2f4b29b8def0fbbd9418559b271eadb178aedcf6e5a65bc5`);
Oct 21 commit `c45227ff` (SHA-256 `53e2c5a6071ddd912b65e497ca455effb5e39ea813085f25a3d4a0708560effc`). Detailed
analysis and domain breakdowns are in [`reports/benchmarks/feq-j13-2026-10.md`](../reports/benchmarks/feq-j13-2026-10.md).

---

## CASC-J13 UEQ standalone full-division campaigns, 2026-10-20 and 2026-10-21

Two consecutive campaigns evaluated all 400 problems in CASC-J13 UEQ sequentially (`jobs=1`) with
eight workers, `MRS_HARDWARE=casc`, and official 180-second division limits on the 8-core Xeon host (`mtsdev04`):
- `campaign-casc-j13-04-ueq-CASCHJ1-20261020` (Oct 20)
- `campaign-casc-j13-04-ueq-CASCHJ1-20261021` (Oct 21)

| Campaign | Total problems | Refutations (`Unsatisfiable`) | `VerifiedGood` | `VerifiedBad` | `Unknown` | Verifier Timeout | Proof Omitted |
|:---|---:|---:|---:|---:|---:|---:|---:|
| **2026-10-20** | 400 | 120 | **112** (re-audited 118) | 6 | 2 | 0 | 0 |
| **2026-10-21** | 400 | 119 | **117** | **0** | 2 | 0 | 0 |

Key takeaways:
- **Production Confirmation of Leaf Symmetry Fix (`b1033d3`)**: In the Oct 20 run, 6 refutations
  failed strict audit due to equality literal orientation flips in cited problem leaves (`GRP178-1`,
  `GRP183-3`, `GRP186-1`, `REL026-3`, `REL041-2`, `REL045-2`). In the Oct 21 campaign, with the fix
  integrated into the binary, **all six problems certified natively as `VerifiedGood`**, achieving
  **117 `VerifiedGood`** and **0 `VerifiedBad`**.
- **Solve set delta**: `REL027-3` solved in 84.8 s on Oct 21 (timed out on Oct 20) and certified as `VerifiedGood`.
  Two borderline problems near 180 s (`ALG240-1` at 178.3 s and `NUN133-1` at 159.5 s on Oct 20) timed out on Oct 21.
- **Residual AC replay barrier**: Only `RNG009-5` and `RNG009-7` return `Unknown` across both runs due
  to incomplete AC superposition rewrite replay.
- **Zero process errors or memory blowups**: Both runs completed 100% of jobs with clean telemetry.

Provenance: Oct 20 commit `a38c8bc8` (binary SHA-256 `c881d56f47b848d507017f4f3a4cb58fa24da0c4e3d8e267c45bc6c0d4e7ae94`);
Oct 21 commit `c45227ff` (binary SHA-256 `088f4b1e613660d094e20185b0dd23e3ce73f70c7fdaee710b4e91cc6cf5da63`). Detailed
analysis and domain breakdowns are in [`reports/benchmarks/ueq-j13-2026-10.md`](../reports/benchmarks/ueq-j13-2026-10.md).

---

## Cooperative UEQ greedy candidate portfolio (2026-10-26)

The solo-derived candidate portfolio `6,12,2,11,1,8,7,15` was evaluated once
on each edition with eight workers, `MRS_HARDWARE=casc`, and sharing disabled.
These are raw SZS results, not a strict audit or a replacement for the verified
baselines above:

| Edition | Total | SZS `Unsatisfiable` | `Timeout` | `GaveUp` | Nonempty proof blocks | Empty proof blocks | Oct 21 raw refutations | Raw delta |
|:---|---:|---:|---:|---:|---:|---:|---:|---:|
| CASC-30 | 300 | 128 | 166 | 6 | 79 | **49** | 123 | +5 |
| CASC-J13 | 400 | 117 | 269 | 14 | 81 | **36** | 119 | −2 |

No strict proof audit is recorded for either candidate artifact. A nonempty
proof block is not necessarily a valid proof, and the empty blocks are not
counted as certified results. In CASC-J13, 47 of the 117 raw `Unsatisfiable`
rows have no expected reference label and consequently have `verdict=unknown`;
the SZS count, rather than the harness verdict, is shown here. The candidate
runs are single observations on dirty builds with distinct binary hashes and
are not controlled comparisons against the October 21 runs. Do not promote the
candidate order from these results. Full solve-set deltas, provenance, and the
proof-output investigation are in the
[`UEQ candidate report`](../reports/benchmarks/ueq-candidate-portfolio-2026-10.md)
and [UI-11](../policies/unresolved-issues.md#ui-11--ueq-candidate-refutations-have-empty-tstp-proof-blocks).

---

## UEQ shipped-schedule follow-up campaigns (2026-10-28)

Two later full-division runs used the shipped `casc_ueq` order with eight
workers and `jobs=1`. They are supplemental results, not replacements for the
canonical baselines above: both metadata files record `git_dirty=true` and
distinct binary hashes. The J13 run explicitly used eight workers (`MRS_WORKERS=8`)
on a host reporting 16 physical cores; that host count does not mean the run
used 16 workers. The J13 run used the `adaptive` profile rather than `casc`.
The CASC-30 host had 96,350 MB RAM rather than the full 128 GB CASC allowance.

| Edition | Total | Raw SZS `Unsatisfiable` | Strict `VerifiedGood` | Other refutation audit | `Timeout` | `GaveUp` |
|:---|---:|---:|---:|:---|---:|---:|
| CASC-J13 | 400 | 121 | **120** | 1 `no_proof` (`REL045-2`) | 260 | 19 |
| CASC-30 | 300 | 124 | **124** | none | 166 | 10 |

There were no `VerifiedBad` proofs. CASC-30's 124 refutations all passed the
strict audit; the J13 `REL045-2` raw status is not a certified solve because
its output contained no TSTP derivation. The recorded J13 maximum of 79,521 MB
is **virtual memory**, not RSS; no RSS measurements were archived for that run.
For CASC-30, the maximum recorded RSS was 32,016 MB and the maximum virtual
peak was 51,946 MB. Full provenance, timing and memory details, and the J13
proof-output anomaly are in the
[`October 28 UEQ follow-up report`](../reports/benchmarks/ueq-follow-up-2026-10-28.md)
and [UI-12](../policies/unresolved-issues.md).

---

## Supplemental combined CASC-J13 certification campaign (2026-10-28)

`~/crates/mrs-bench/results/cert-j13-20261028-093240` covered all 800 FEQ,
FNE, and UEQ problems with eight workers and `jobs=1`. It generated 232 raw
refutations: FEQ 74/300, FNE 39/100, and UEQ 119/400. The archived audit had
189 `VerifiedGood`, 1 `VerifiedBad` (`GEO526+1`), 41 `no_proof`, and 1
`proof_omitted`. A strict re-audit at HEAD `31b81f3` certifies **190/232**:
`GEO526+1` now passes following the strict condensation fix, for 190
`VerifiedGood`, 0 `VerifiedBad`, 41 `no_proof`, and 1 `proof_omitted` overall.

This does not replace a baseline. The producer binary (SHA-256
`222464664d2bad81393aafa09f5514ac824a8ce86434c1bfe6762532a35bbc76`) was
stale relative to its dirty `df40f564` source tree, as the run log warns. Forty
refutations had empty proof envelopes (5 FEQ, 35 UEQ); the 41st `no_proof` row,
`REL045-2`, had a header-only proof and is tracked separately in UI-12. Current
source spot checks generate strictly verified proofs for `NUM853+2` and
`REL024-1`, but do not certify the old archive or establish a full-division
re-run. The host had 16 physical cores, so `MRS_HARDWARE=casc`'s eight workers
were not CPU-pinned to an eight-core simulation. See the [full report](../reports/benchmarks/cert-j13-2026-10-28.md),
[certification status](../CERTIFICATION_STATUS.md), and
[UI-11](../policies/unresolved-issues.md#ui-11--ueq-candidate-refutations-have-empty-tstp-proof-blocks).

---

## Cooperative FNE candidate portfolio campaigns (2026-10-21 and 2026-10-22)

Following solo strategy sweeps and greedy set-cover analysis, the candidate 8-strategy
portfolio `[s11, s13, s04, s08, s15, s07, s03, s01]` was evaluated cooperatively across three campaigns:
- `casc-30/coop-fne-candidate-20261021` on `teenf9901` (240 s official limit)
- `casc-j13/coop-fne-candidate-20261021` on `tlpnf9701` (180 s official limit)
- `casc-j13/coop-fne-candidate-03-20261022` on canonical 8-core host `mtsdev03` (180 s official limit)

| Campaign | Edition | Host | Total | Refutations (`Theorem`) | `VerifiedGood` | `VerifiedBad` | `Unknown` | Proof Omitted |
|:---|:---|:---|---:|---:|---:|---:|---:|---:|
| **`coop-fne-candidate-20261021`** | CASC-30 | `teenf9901` (32c) | 100 | **45** | **45** | **0** | 0 | 0 |
| **`coop-fne-candidate-20261021`** | CASC-J13 | `tlpnf9701` (32c) | 100 | **38** | **38** | **0** | 0 | 0 |
| **`coop-fne-candidate-03-20261022`** | CASC-J13 | `mtsdev03` (8c) | 100 | **40** | **39** | **0** | 0 | 1 |

Key takeaways:
- **CASC-30 Breakthrough (+4 verified)**: Solves 45/100, retaining all 41 baseline solves and
  adding four hard logic calculi theorems via Strategy 13: `LCL370+2` (61.0 s), `LCL373+2` (66.6 s),
  `LCL390+2` (91.7 s), and `LCL403+2` (92.7 s).
- **CASC-J13 Breakthrough (+4 refutations / +3 verified)**: On canonical 8-core hardware (`mtsdev03`),
  derives 40 refutations (39 `VerifiedGood`, 1 `proof_omitted` `KRS261+1`), adding `LCL373+2`, `LCL382+1`,
  `LCL390+2`, `CSR116+8`, `CSR116+38`, and `PRD001+1`.
- **Flawless Strict Soundness**: 100% of emitted proofs certified strictly with zero `VerifiedBad`.
- Detailed report: [`reports/benchmarks/fne-candidate-portfolio-2026-10.md`](../reports/benchmarks/fne-candidate-portfolio-2026-10.md).

### MultiQueue hybrid campaigns (`casc_fne_mq_hybrid`, 2026-10-22)

The hybrid portfolio combining four base strategies (`[s11, s13, s04, s15]`) and four MultiQueue
archetypes (`[mq_0, mq_1, mq_2, mq_5]`) was evaluated with 8 workers across three campaigns:
- `cooperative-fne-mq-hybrid-no-sharing` on `teenf9901` (240 s): **45 Theorems** (42 `VerifiedGood`, 0 `VerifiedBad`, 2 `Unknown`, 1 `proof_omitted`).
- `cooperative-fne-j13-04-mq-hybrid-no-sharing` on `mtsdev04` (180 s, 8 cores): **37 Theorems** (all 37 `VerifiedGood`, 100%).
- `cooperative-fne-j13-mq-hybrid-no-sharing` on `tlpnf9701` (180 s, 32 cores): **41 Theorems** (all 41 `VerifiedGood`, 100%).

While MQ-hybrid reaches 41 solves on uncongested 32-core systems, on canonical 8-core hardware (`mtsdev03`/`mtsdev04`)
the pure base candidate portfolio `[11, 13, 4, 8, 15, 7, 3, 1]` achieves higher certified yield (39 VG / 40 thm vs 37 VG)
and avoids strict factoring step limits. Full report: [`reports/benchmarks/fne-multiqueue-2026-10.md`](../reports/benchmarks/fne-multiqueue-2026-10.md).

---

## Peak memory and large proofs by division

The table reports the maximum per-problem `peak_memory_mb` in the listed
campaign and the count of emitted proofs whose `proof_bytes` is **greater than
8 MB (8,000,000 bytes)**. Memory is the campaign row's observed per-problem
peak, not the host's installed RAM or the configured memory ceiling. EPS
produces model certificates rather than refutation proofs, so a proof-size count
does not apply.

| Edition | Division | Campaign | Max per-problem peak memory | Emitted proofs >8 MB |
|:---|:---|:---|---:|---:|
| CASC-J13 | FEQ | `~/crates/mrs-bench/results/campaign-casc-j13-W8C8J1-20261016` | 70,108 MB (`CSR025+5`) | 7 |
| CASC-J13 | FEQ | `~/crates/mrs-bench/results/campaign-casc-j13-03-feq-CASCHJ1-20261021` | 64,650 MB (`CSR036+5`) | 3 (+ 1 omitted >32 MB) |
| CASC-J13 | FNE | `~/crates/mrs-bench/results/campaign-casc-j13-W8C8J1-20261016` | 66,771 MB (`NLP261+1`) | 3 |
| CASC-J13 | FNE | `~/crates/mrs-bench/results/casc-j13/coop-fne-candidate-03-20261022` | 58,864 MB (`NLP261+1`) | 3 (+ 1 omitted >32 MB) |
| CASC-J13 | FNE | `~/crates/mrs-bench/results/cooperative-fne-j13-04-mq-hybrid-no-sharing` | 61,461 MB (`NLP260+1`) | 3 |
| CASC-J13 | UEQ | `~/crates/mrs-bench/results/campaign-casc-j13-W8C8J1-20261016` | 73,685 MB (`ROB015-10`) | 0 |
| CASC-J13 | UEQ | `~/crates/mrs-bench/results/campaign-casc-j13-04-ueq-CASCHJ1-20261021` | 57,732 MB (`ROB015-10`) | 0 |
| CASC-30 | FEQ | `~/crates/mrs-bench/results/campaign-casc30-feq-W8CASCHJ1-01-20261020` | 64,651 MB (`CSR037+5`) | 5 |
| CASC-30 | FEQ | `~/crates/mrs-bench/results/campaign-casc30-feq-W8CASCHJ1-01-20261021` | 64,651 MB (`CSR037+5`) | 5 (+ 2 omitted >32 MB) |
| CASC-30 | UEQ | `~/crates/mrs-bench/results/campaign-casc30-ueq-W8CASCHJ1-02-20261020` | 63,428 MB (`ROB005-1`) | 0 |
| CASC-30 | UEQ | `~/crates/mrs-bench/results/campaign-casc30-ueq-W8CASCHJ1-02-20261021` | 67,525 MB (`ROB005-1`) | 0 |
| CASC-30 | FNE | `~/crates/mrs-bench/results/campaign-casc30-fne-W8CASCHJ1-99-20261019` | 69,716 MB (`BOO109+1`) | 4 |
| CASC-30 | FNE | `~/crates/mrs-bench/results/casc-30/coop-fne-candidate-20261021` | 70,669 MB (`BOO109+1`) | 5 |
| CASC-30 | FNE | `~/crates/mrs-bench/results/cooperative-fne-mq-hybrid-no-sharing` | 64,723 MB (`NLP260+1`) | 4 (+ 1 omitted >32 MB) |
| CASC-30 | EPS | `~/crates/mrs-bench/results/campaign-casc30-eps-W8CASCHJ1-01-20261019` | 33,731 MB (`SYN888-1`) | N/A (model certificates; no `proof_bytes`) |
| CASC-30 | EPS | `~/crates/mrs-bench/results/campaign-casc30-eps-W8CASCHJ1-99-20261026` | 66,644 MB (`SYN888-1`; 34,002 MB RSS) | N/A (model certificates; no `proof_bytes`) |
| CASC-30 | EPU | `~/crates/mrs-bench/results/campaign-casc30-epu-W8CASCHJ1-02-20261019` | 25,692 MB (`HWV081-1`) | 2 |
| CASC-30 | ICU | `~/crates/mrs-bench/results/campaign-casc30-icu-W8CASCHJ1-97-20261019` | 53,772 MB (`EEE009+1`) | 0 |

These are the largest observed values and large-proof counts in the named
campaigns, not cross-run maxima or stable properties of each division. The
CASC-J13 UEQ row uses the combined 2026-10-16 campaign (114 generated
refutations, 113 strict `VerifiedGood`, one `Unknown`); the separate
syntactic-recovery run has its own artifacts and slightly different peak.
Campaign metadata and per-row output contain the hardware, binary hash and
raw `proof_bytes`/`proof_emitted` values used to derive the table.

---

## `casc-30` EPS, before and after the 2026-09 tier-routing work

| file | what |
|---|---|
| `eps-casc-sim-20260928-run.csv` | `casc-30` EPS through `casc.sh --divisions eps --casc-times`, 120 s, CASC 8-worker dual split (`workers=7+1`), 32-core Xeon / 128 GB. **42/100 solved, 0 reference violations, 0 invalid models.** |
| `eps-casc-sim-20260928-audit-summary.txt` | The `audit_casc_proofs` report for that run: 38 of the 42 models kernel-certified, 0 invalid, 4 `Inconclusive` on the kernel's evaluation work cap. |
| `eps-casc-sim-20260928-summary.tsv` | A local re-measurement of the same division, certified track alone, on a 2-core host under `casc-sim`. Same 42 solved. Kept because it carries the refusal-reason telemetry (`cert_tier`) per problem. |
| `eps-lowram-20260928-summary.tsv` | The same 42 on the same 2-core / 15 GB host after the grounding budget was changed to track memory. Confirms the memory work is coverage-neutral on the box that motivated it, and is the reference for what the box can now afford. |

The baseline for comparison is `cert-eps-20260926` in the same results tree; the
comparison table, including the +30 / 0 gained-versus-lost split and the
10× drop in mean time on solved rows, is in
[`../reports/benchmarks/eps-2026-09.md`](../reports/benchmarks/eps-2026-09.md).

Provenance: `cert-eps-20260928/run_meta.json` records `git_commit 95f7984e`,
branch `main` — the merge of `feat/eps-casc-sim-exploration` into `main`, with
all six of that branch's commits as ancestors. So this run measures the merged
result. `95f7984` also carries intervening mainline work beyond the branch tip,
and `git_dirty: true` records an incidental local modification, so the number
belongs to the merge rather than to the branch tip in isolation. The archived
streams confirm which changes ran: every solved row carries `cert_tier=2` and
`stopped=portfolio`.

## `casc-30` EPS, 2026-10-19 CASC-mode campaign

`campaign-casc30-eps-W8CASCHJ1-01-20261019` ran the complete 100-problem EPS
division with `MRS_HARDWARE=casc`, eight workers, `jobs=1`, and the official
120-second clock. It returned **44 `Satisfiable` answers**; strict model audit
certified **40**, found **0 invalid models**, and left **4 inconclusive**. The
remaining 56 problems returned no model answer (10 `GaveUp`, 46 `Timeout`).
The four inconclusive model checks are `NLP115-1`, `NLP118-1`, `NLP120-1`, and
`NLP123-1`.

Compared with `cert-eps-20260928`, all 38 previously certified models are
retained, and `SYN441-1` and `SYN529-1` are newly certified: **+2 certified
models, no certified-model regressions**. The four inconclusive model checks are
unchanged. Thus raw and certified coverage both rise from 42/38 to 44/40. The
models for `SYN441-1` and `SYN529-1` were inconclusive in the September run, so
these are also two model-checking recoveries, not merely new solver answers.
Compared with `cert-eps-20260926`, the run retains all four previously
certified models and adds 36, with no losses. These are archived model-set
comparisons across different builds and campaigns, not a matched A/B.

The run used an 8-physical-core Xeon E5-2407 host with 96,350 MB RAM, below the
nominal 128-GiB CASC allowance; the prover reported 81,897 MB effective memory.
The maximum observed per-problem peak was 33,731 MB (`SYN888-1`), and no problem
ended at a resource limit. On `SYN441-1` and `SYN529-1`, the portfolio worker was
terminated at the 118-second division budget after the certification worker had
already returned a verified model. The certified `Satisfiable` result remained
valid in both cases.

Provenance: `run_meta.json` records commit `40278337`, `git_dirty=true`, Rust
1.99.1, and binary SHA-256
`a2f8178145f688bdf1deeb47648938d88d7d0039a19d8754237901aa458b046d`. See the
[EPS certification summary](../CERTIFICATION_STATUS.md) and the EPS
[intermittency list](intermittency.md).

## `casc-30` EPS, 2026-10-26 CASC-profile rerun

`campaign-casc30-eps-W8CASCHJ1-99-20261026` reran all 100 problems with
`MRS_HARDWARE=casc`, eight workers, `jobs=1`, and the official 120-second limit.
It returned **44 reference-matched `Satisfiable` answers**, 13 `GaveUp`, and 43
`Timeout`, with no `Error` or `ko` rows. The solved problem set is identical to
the 2026-10-19 campaign; `HWV063-1`, `HWV077-1`, and `HWV080-1` changed only
from `Timeout` to `GaveUp`. These are still unsolved rows, not coverage changes.

This was a `casc` profile run on a host with 128,019 MB RAM; the engine detected
16 physical cores and the run metadata reports 32 CPUs. The profile uses the
fixed CASC worker/memory allowances but does not pin CPUs, so this is not an
eight-physical-core simulation. All 44 answers selected the Tier-2 certifier;
the ordinary portfolio produced no unique solve. The archived `certification/`
directory is empty, however, so this run has no independent strict-kernel model
audit and does not establish 44 kernel-certified models.

The run metadata records commit `ff316de52f297c5014722ef04efc594d3b1bb1a`, a
dirty worktree, Rust 1.99.1, and binary SHA-256
`222464664d2bad81393aafa09f5514ac824a8ce86434c1bfe6762532a35bbc76`. For the
timings, track split, audit caveat, and stage-marker interpretation, see the
[EPS benchmark report](../reports/benchmarks/eps-2026-09.md#full-casc-profile-rerun-2026-10-26-ff316de).

## `casc-30` EPU, 2026-10-19 CASC-mode campaign

`campaign-casc30-epu-W8CASCHJ1-02-20261019` ran all 100 EPU problems with
`MRS_HARDWARE=casc`, eight workers, `jobs=1`, and the official 120-second
clock. It generated **11 refutations**; all **11 are strict `VerifiedGood`**,
with zero `VerifiedBad`, checker timeouts, or inconclusive proofs. The other
89 rows were 27 `GaveUp` and 62 `Timeout`.

This is **two more strictly certified solves** than both `cert-epu-20260926`
and `campaign-casc30-epu-W8C8J1-20261002`: all nine previously certified
problems remain certified, and `HWV078-1` and `MSC024-1` are new. Neither
earlier audit contained a `VerifiedBad` proof. This is a genuine full-division
gain in the archived strict solve set, not yet a replicated estimate of the
coverage rate.

Both new proofs came from the bounded EPR equality route (`epr_route=epr_equality`):
`HWV078-1` refuted in 53.4 s with a 52,857-node, 13,382,823-byte proof emitted
by `derivation_loop`; `MSC024-1` refuted in 4.5 s with a 26,189-node,
8,057,596-byte proof emitted by `lrat_lift`. The former 8-MiB proof-output cap
would have omitted `HWV078-1`; the current 32-MiB default allowed strict
replay. These successes address two earlier failure modes: the HWV078-1
ground-fallback proof extraction failure and the MSC024-1 grounding/LRAT
reconstruction gap. See UI-5 in
[`policies/unresolved-issues.md`](../policies/unresolved-issues.md).

The eight-physical-core host had 95,979 MB RAM, below the nominal 128-GiB CASC
allowance; the prover reported 81,582 MB effective memory. Peak observed
per-problem memory was 25,692 MB (`HWV081-1`); no problem hit a resource limit.
Provenance: commit `40278337`, dirty tree, Rust 1.99.1, binary SHA-256
`947aba9b604307f33e86d595b6112def78b42925f3a6ebeefac114aaf7cb9c96`. This is
one CASC-shaped measurement, so repeat the paired grounding-off/on comparison
at the same 8-core, `jobs=1` shape before attributing the gain causally to the
EPR grounding change. Per-problem solve signatures are in
[`intermittency.md`](intermittency.md).

## `casc-30` EPU, 2026-10-26 adaptive-mode follow-up

`campaign-casc30-epu-W8C8J1-20261026` completed all 100 problems with the
official 120-second limit, eight requested workers, and `jobs=1`. It returned
**11 `Unsatisfiable`**, all **11 strict `VerifiedGood`**, with zero
`VerifiedBad`, checker timeouts, or inconclusive proofs; the other 89 were 27
`GaveUp` and 62 `Timeout`. All 100 processes exited successfully and emitted an
SZS status.

The certified solve set is exactly the same 11 as in the October 19 campaign:
there are no additions or losses. Two unsolved statuses flipped:
`HWV064-1` (`Timeout` → `GaveUp`) and `SYO589-1` (`GaveUp` → `Timeout`).
Per-problem wall times for the common solve set were:

| Problem | 2026-10-19 (s) | 2026-10-26 (s) |
|---|---:|---:|
| `HWV078-1` | 53.425 | 24.884 |
| `HWV107-1` | 85.554 | 72.075 |
| `LAT260-2` | 0.142 | 0.129 |
| `LAT261-2` | 0.136 | 0.103 |
| `LAT264-2` | 0.140 | 0.120 |
| `LAT265-2` | 0.136 | 0.111 |
| `MSC024-1` | 4.463 | 3.123 |
| `PUZ008-2` | 0.159 | 0.142 |
| `PUZ036-1.005` | 41.180 | 39.066 |
| `SET856-2` | 0.136 | 0.110 |
| `SYN837-1` | 83.958 | 73.968 |

These are raw observations, not a controlled speed comparison. Despite the
campaign directory name, per-problem stderr records `hardware=adaptive` and
`host_physical_cores=16`; `run_meta.json` records 32 CPUs and 63,763 MB of RAM.
Only `MRS_WORKERS=8` was set, and `MRS_HARDWARE=casc` was not. This is therefore
not a canonical 8-physical-core CASC-mode run or a matched replication of
October 19. The different host, hardware mode, dirty build, and binary also
prevent attributing the lower observed solve times to a code change.

The two direct EPR-grounding refutations were `HWV078-1` (24.884 s, 52,857
proof nodes, 13,385,966 bytes, `derivation_loop`; strict check 6.820 s) and
`MSC024-1` (3.123 s, 26,189 nodes, 8,063,098 bytes, `lrat_lift`; strict check
0.937 s). The other nine proofs came from the ordinary portfolio/search path.
In particular, `HWV107-1`, `PUZ036-1.005`, and `SYN837-1` were solved after the
EPR pre-pass fell back. The EPR/InstGen telemetry and the notable extraction and
early-`GaveUp` cases are recorded in [UI-5](../policies/unresolved-issues.md).

The campaign's recorded per-problem peak memory was 39,150 MB (`HWV081-1`); no
row was reported as a resource-limit result. Provenance: commit `ff316de5`,
dirty tree, Rust 1.99.1, binary SHA-256
`bb13b614e303aece759880faa090f0d6ab2074d9fa8376edb6e174d41259b070`.

## `casc-30` FNE, 2026-10-19 CASC-mode campaign

`campaign-casc30-fne-W8CASCHJ1-99-20261019` completed the full 100-problem
division using eight workers, `MRS_HARDWARE=casc`, `jobs=1`, and the official
240-second clock. It generated **41 theorems**, and strict audit certified
**all 41 `VerifiedGood`**, with no `VerifiedBad`, `Unknown`, or audit timeout.
The other outcomes were 10 `GaveUp` and 49 `Timeout`; no resource limits or
process errors were recorded.

Compared with `campaign-casc30-fne-W8C8J1-20261002`, all 30 previously
certified problems are retained and **11 are newly certified**, with no losses.
Compared with the October 1 concurrent `condensed` and `ref` measurements, all
29–30 certified problems from those audits are included in the new 41. Those
October 1 measurements ran with two jobs and share the same build but not the
same resource conditions; they are context, not matched controls.

The per-problem telemetry shows the EPR/InstGen pre-pass ran on only four
`pure_relational_epr` rows; all four fell back after `grounding_exhausted`.
Across these four it generated 1,338,617 grounded clauses in total (maximum
442,641). The additional certified solves therefore cannot be attributed to
EPR grounding from this run’s telemetry. This is a single full-division result,
not a replicated estimate or a paired pre-pass A/B.

Provenance: commit `40278337`, dirty tree, Rust 1.99.1, binary SHA-256
`edd0ccdeadc5f8d1d2dc764c83057b851cbb45def4d64ad7ca5663c1af1133aa`. The
8-physical-core host had 128,019 MB RAM; maximum observed per-problem peak was
69,716 MB (`BOO109+1`). See UI-5 and the per-problem
[`intermittency inventory`](intermittency.md).

## `casc-30` ICU, 2026-10-19 CASC-mode campaign

`campaign-casc30-icu-W8CASCHJ1-97-20261019` completed the full 101-problem
ICU division using eight workers, `MRS_HARDWARE=casc`, `jobs=1`, and the official
480-second clock. It generated **4 theorems**, and strict audit certified
**all 4 `VerifiedGood`**, with zero `VerifiedBad`, `Unknown`, or audit timeouts.
The other outcomes were 18 `GaveUp`, 78 `Timeout`, and 1 `Error` (`CSI008+1`,
killed by the OS OOM killer at 68.1 s on 2,004,866 axioms).

Compared with `campaign-casc30-icu-W8C8J1-20261002`, the strict certification rate
rose from 3/5 (60.0%) to **4/4 (100.0%)**. Both `CSE007+1` and `EEE001+1`—which
returned `Unknown` in the October 2 audit due to strict formula-size limits on NNF
and negated conjecture expansion—certified cleanly (`VerifiedGood` in 1.92 s and
0.49 s, respectively) following commit `13653b9`. `EEE009+1` (certified in October 2
in 45.7 s) timed out at 480 s on the 1.80 GHz host, establishing it as an
intermittent problem under parallel 8-worker search. `CSI007+1` and `EEE007+1`
certified in both campaigns.

Because 44 of the 101 ICU problems carry non-decisive reference statuses
(`expected=GaveUp`), `casc.sh` grades `CSE007+1` as `verdict=unknown` despite its
verified proof. On decisive references alone (`expected=Theorem`), the score is
3/55 (5.5%).

Provenance: commit `40278337`, dirty tree, Rust 1.99.1, binary SHA-256
`2982908b6da78fd0c791a171f4e47af6207c93b80512befcae271caf46537865`. The
16-physical-core Xeon Silver 4108 host had 63,763 MB RAM; effective memory was
54,198 MB. Maximum observed per-problem peak memory among completing jobs was
53,772 MB (`EEE009+1`). See UI-6 in
[`policies/unresolved-issues.md`](../policies/unresolved-issues.md), the
[ICU certification report](../reports/benchmarks/icu-2026-10.md), and the
[`intermittency inventory`](intermittency.md).

## `casc-j13` UEQ, and a 150-problem regression

| file | what |
|---|---|
| `ueq-casc-j13-20260930-run.csv` | `casc-j13` UEQ, all 400 problems, official CASC-J13 180 s, 8 workers, `jobs=1`, Xeon Silver 4108 / 16 cores / `hardware=adaptive`. **107/400 solved, 107 kernel-certified, 0 reference or polarity violations.** |
| `ueq-casc-j13-20260930-audit-summary.txt` | Strict proof audit of that run: 107 `VerifiedGood`, 0 `VerifiedBad`, 0 `Unknown`, 0 `Error`. |
| `ueq-casc-j13-20260930-summary.tsv` | Derived per-family table, and the row-by-row diff against the `codex.db` CASC-J13 UEQ run. |
| `ueq-casc-j13-20260930-run_meta.json` | Provenance as recorded by the harness. Note it says `time_limit=120` while the run used 180 s — the `--casc-times` override won and the metadata field was not updated. |

Against the stored `codex.db` CASC-J13 UEQ run (reported as 257/400 in
`docs/reports/codex/status-2026-09-11.md`), this run solves 96 in common,
**loses 161**, and gains 11. On the regressed rows that carry telemetry on
both sides, median `processed_per_s` fell from 1649.5 to 116.0. The two
candidate causes are cross-strategy sharing being disabled by default
(`ce7bdc5`) and the `casc_ueq` portfolio reorder (`b214a13`); neither is
established, and the four-cell A/B that would separate them is in
[`../reports/benchmarks/ueq-2026-09.md`](../reports/benchmarks/ueq-2026-09.md) §2.

Provenance: `git_commit c07cac9` (merge of the FNE casc-sim exploration),
branch `main`, `git_dirty: true`, `binary_sha256 1a88a581…`. The dirty tree
means the SHA, not the commit, identifies the binary. The 107/400 is a
development number — `hardware=adaptive` on 16 cores is not CASC-shaped — and
must not be compared against a published CASC score.

Only the CSV, the audit summary, `run_meta.json`, and the derived `.tsv` are
banked here. The 6 MB of raw stdout/stderr under the run's `raw/mrs/ueq/` is
not, so re-running `audit_casc_proofs` against these artefacts needs the
original run directory, which lives outside any git repository. Each row's
`raw_stdout_sha256` pins the copy that was audited.

## LCL condensed-detachment pre-pass, FNE `casc-30`, 2026-10-01

Two measurements of the same pre-pass, written up in
[`../research/condensed-detachment.md`](../research/condensed-detachment.md).

| file | what |
|---|---|
| `cd-fne-casc30-condensed-20261001-run.csv` | The `condensed` arm of the FNE CASC-30 campaign: 100 problems, `MRS_CONDENSED_DETACHMENT=1`, 8 workers, 2 jobs, official 240 s CASC clock. **42 solved, 0 of them by the pre-pass.** |
| `cd-fne-casc30-ref-20261001-run.csv` | The matched `ref` arm, pre-pass off. **42 solved**, the same 42 problems. |
| `cd-fne-casc30-{condensed,ref}-20261001-run_meta.json` | Harness provenance. **Both record the same `git_commit 38e1f85` and the same `binary_sha256` — and neither records the environment, which is the whole reason the pair is hard to read.** `casc.sh` now writes a `prover_env` block for exactly this reason. |
| `cd-fne-casc30-20261001-run.csv` | `cd_bound_probe.sh` over the 37 `LCL*` problems, at production bounds (`prod`) and at 60x/20x widened bounds (`wide`). **0 pre-pass refutations in either arm.** |
| `cd-fne-casc30-20261001-summary.tsv` | The per-problem reading: stop reason, derived-fact count, inference count and elapsed time for both arms side by side. This is the table the conclusion is read off. |
| `cd-fne-casc30-20261001-probe.log` | Probe configuration header. |

Provenance: campaign arms at `git_commit 38e1f85`, `git_dirty: true`, Xeon Silver
4108 / 32 cores / 128 GB, `jobs=2`. The probe was run later from a 2-physical-core
/ 16 GB host, which does not affect its readings — the pre-pass is single-threaded
and the probe pins `--workers 1` — but it does cap how wide the wide arm could
afford to go.

Two caveats that belong with these files rather than in a report:

- **The campaign pair is not a clean A/B and was never going to be one.** Both
  editions ran two campaigns concurrently on one 128 GB box: 4 jobs x 8 workers on
  16 cores, with up to 4 x ~59 GB of memory demand. `NLP260+1`, `NLP261+1` and
  `NLP262+1` were OOM-killed or stopped at `resource_reason=memory` in the casc-j13
  arms. The pre-pass conclusion does not depend on those rows; the 42/36 headline
  does not survive them.
- **The campaign number cannot be compared to the casc-j13 number.** CASC-30 FNE
  and CASC-j13 FNE share only 34 of 100 problems, and the two editions ran
  different binary SHAs from two different dirty working trees.

The 73 MB of raw streams under the original campaign directories is not banked.
The probe's stop reasons were reconstructed from them and are now in the summary
`.tsv`, which is why that file exists rather than being derived on demand.

## Redundancy-elimination throughput, UEQ, 2026-10-02

Two fixed-wall-clock probes of the same binary pair, written up in
[`../reports/benchmarks/redundancy-throughput-2026-10.md`](../reports/benchmarks/redundancy-throughput-2026-10.md).
The unit of measurement is clauses retired out of a fixed budget, not
clauses-per-second: see `docs/results/perf/README.md` for why, and
`fne-2026-09.md` §1 for the host band that makes the wall-clock reading
untrustworthy. Both probes ran strictly sequentially, one worker, sharing off,
on one 2-physical-core / 15 GB host in one session.

| file | what |
|---|---|
| `throughput-probe-12s-{base,fixed}.csv` | 45 problems (30 UEQ, 15 FNE) at 12 s each. **UEQ `processed` 13 222 → 16 386 (1.24x), `generated` 521 233 → 704 523 (1.35x); FNE 1.02x / 1.10x. No status change on any row.** |
| `throughput-probe-12s-summary.tsv` | The per-problem reading of that pair, both arms side by side with the ratio. |
| `throughput-probe-ueq30s-{base,fixed}.csv` | 60 `casc-30` UEQ problems at 30 s each, one strategy. **10 refuted → 11; 2 gained, 1 lost; total wall on the 9 commonly-solved rows 130.9 s → 84.7 s; `processed` on the 50 timing-out rows 1.27x.** |
| `throughput-probe-ueq30s-summary.tsv` | The per-problem reading of that pair. |

Provenance: base binary built from `233e0d8` (sha256 `d12c0dd774f965a5…`),
fixed from this branch's release build (sha256 `16c38c223c6453c8…`). The
`base`/`fixed` CSVs carry the binary sha in every row, so an arm cannot be
silently swapped.

The branch-tip binary hashes to `546188b63d433fad…`, not `16c38c223c6453c8…`:
a few cosmetic refactors landed after the measurement (an early return folded
into one call in `subsumes_id`, two `let mut` bindings that no longer need to be
mutable, and comment wording). Re-running the 8-row UEQ subset on the branch-tip
binary reproduces the banked arm — 156 / 136 / 1181 / 598 / 579 / 78 / 444 / 743
`processed` against the banked 156 / 136 / 1183 / 598 / 579 / 78 / 444 / 742 — so
the two shas are the same engine. The banked numbers are still the ones the
recorded sha produced.

Two caveats that belong with these files:

- **These are single-strategy numbers, not division scores.** `--workers 1`
  against `casc_ueq` runs one strategy; the banked division runs eight
  cooperatively at 240 s. The 60-problem set was chosen from the problems
  `codex.db` records as solved, but at 30 s with one strategy only 10 of the
  60 are actually refuted in either arm, so the refuted-count column is a
  regression check and not a score.
- **`BOO017-10` is a real, reproducible flip and is not explained.** It is
  `Unsatisfiable` at 30 s on the base arm and `Timeout` at 30 s on the fixed
  arm — and `Timeout` at 60 s on base and `Unsatisfiable` at 60 s on fixed, so
  neither arm is monotone in its own budget. Three repeats of each reproduce
  it. It is reported rather than dropped; see the report §4.
