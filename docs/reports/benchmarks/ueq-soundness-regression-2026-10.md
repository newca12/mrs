# UEQ after target-rigidity soundness fix: 114 → 5, with an unsound baseline

> Status: measurement recorded; coverage recovery open under
> [`UI-1`](../../policies/unresolved-issues.md#ui-1--ac_superposition-conclusions-that-are-not-a-superposition-of-their-cited-parents).
> This compares two archived full CASC-J13 UEQ campaigns on the same 400
> problems. It is **not** a comparison between two sound releases: the earlier
> run is explicitly named `clean-unsound` and has no archived proof audit.

## Result

| campaign | run date | source commit | binary SHA-256 | UEQ solves | statuses |
|---|---|---|---|---:|---|
| `campaign-cascj13-clean-unsound` | 2026-10-10 | `176a9f975a3f7256bd27d231a21e2f87ffd61071` | `1bd3efc45ea23594ca32ee17396f88c40cda2f918bdff2b6684f8a7b6e716775` | **114/400** | 114 Unsatisfiable, 269 Timeout, 17 GaveUp |
| `campaign-cascj13-clean` | 2026-10-13 | `313b9237ceff11c5db6f5e4889ced685538ce02e` | `f8825f9170e2b8de6c191d5a2b648a86beb7133f9642ef59739da1bd56703d80` | **5/400** | 5 Unsatisfiable, 394 GaveUp, 1 Timeout |

Both used the same UEQ problem set and the same machine (`teenf9901`, Intel Xeon
Silver 4108), `jobs=1`, eight workers, and the same eight-strategy order
`s4,s8,s12,s11,s2,s14,s15,s1`. Both had the official 180 s limit. The newer run
sets `MRS_HARDWARE=casc`; the earlier metadata records only `MRS_WORKERS=8`,
though its observed limit, worker count, and strategy IDs agree. The newer
campaign's five Unsatisfiable answers all passed strict proof verification.

Row-by-row over the 400 common problem names:

| comparison | problems |
|---|---:|
| Solved in both | 2 |
| Earlier run solved, clean run did not | **112** |
| Clean run solved, earlier run did not | **3** |

The three new clean solves are `RNG019-7`, `RNG020-7`, and `RNG021-7`. Thus
114 → 5 is an observed **net difference of 109 solves**, with 112 earlier
answers lost and three gained.

## Interpretation and limits

The October 10 campaign is explicitly marked `clean-unsound`. It predates the
current target-rigidity behavior and is not a sound coverage baseline. No strict
audit summary is present in its archive, and this comparison does not establish
whether any or all of its 114 answers had valid proofs. In particular, do not
state that the soundness fix removed 109 *valid* proofs, or that 112 lost rows
were individually proven unsound.

The clean run confirms a severe **coverage gap** in current UEQ behavior. It is
consistent with the mechanism already established in UI-1: the former AC
unifier could bind target-clause variables and derive specialisations not
entailed by the parents; making target variables rigid refuses those inferences.
UI-1's local test set also showed five sound AC-heavy solves disappear after
that guard, and the current bounded subset/ordered-associative rules have not
restored those five. The campaign result shows the issue is not limited to those
curated examples.

The campaigns are not a controlled code-only A/B: they use different source
commits and binary hashes, while the clean run also explicitly sets the CASC
hardware mode. Matching host, division, problem set, worker count, time limit,
and strategy order narrows the comparison, but does not isolate the rigidity
change from every other code change. Treat the timing as evidence of the scale
of the present gap, not as a precise causal estimate of one patch.

`GaveUp`/`Timeout` counts are not coverage measures: the current clean run
returns `GaveUp` on 394 rows where the earlier run mostly timed out. The
meaningful comparison here is the definitive expected status (`Unsatisfiable`,
verdict `ok`) versus no solve, with proof certification reported separately.

## What the result establishes

1. **The soundness repair exposed a major UEQ capability loss.** Current clean
   coverage is only 5/400 on this CASC-J13 campaign.
2. **The older 114/400 score is not a valid sound target.** Its proof validity
   was not established in the archived run.
3. **The lost coverage cannot be waved away as a matcher-unit-test gap.** The
   bounded associative additions and focused calculus/kernel tests did not
   recover the full-division score.
4. **Recovery must remain sound.** Preserve target-variable rigidity; diagnose
   the lost problems and add a justified inference rule or search path, with
   independently verified proofs before counting recovered coverage.

## Archives

Run directories are outside this repository:

- `/home/fr22192/pve/crates/mrs-bench/results/campaign-cascj13-clean-unsound/`
- `/home/fr22192/pve/crates/mrs-bench/results/campaign-cascj13-clean/`

Each contains `run.csv`, `run_meta.json`, and `run.log`. The clean archive also
contains `certification/audit.csv` and `certification/audit-summary.txt`; the
older unsound archive has no certification directory. Both metadata records say
`git_dirty=true`, so the hashes identify the campaign binaries, not pristine
rebuilds of the recorded commits.
