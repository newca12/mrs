# Schedule Reference

Schedule constructors live in `crates/mrs-search/src/strategy/named.rs`; base
strategies live in `crates/mrs-search/src/strategy.rs`.

## Base strategies

IDs are stable telemetry identities, not promises that every schedule runs all
15 IDs simultaneously.

| ID | Selection | Literals | Ordering | Notable configuration |
|---:|---|---|---|---|
| 1 | `AgeWeight(3)` | `AllNegative` | KBO | balanced baseline |
| 2 | `SmallestFirst` | `AllNegative` | KBO | no weight cap, no AVATAR |
| 3 | `SmallestFirst` | `AllNegative` | KBO | best-first baseline |
| 4 | `AgeWeight(8)` | `MaxNegativeOrMaxPositive` | KBO | aggressive selection |
| 5 | `AgeWeight(5)` | `All` | KBO | unrestricted literal selection |
| 6 | `AgeWeight(10)` | `All` | KBO | no cap, no AVATAR; FNE/definition chains |
| 7 | `AgeWeight(3)` | `AllNegative` | LPO | balanced LPO |
| 8 | `GoalDirected(10)` | `AllNegative` | LPO | goal-symbol precedence |
| 9 | `SmallestFirst` | `AllNegative` | LPO | LPO best-first |
| 10 | `AgeWeight(12)` | `AllNegative` | KBO | SOS depth 100 |
| 11 | `AgeWeight(6)` | `AllNegative` | KBO | `ConjSymbolBoost` |
| 12 | `AgeWeight(5)` | `AllNegative` | KBO | `HornHeuristic`, no AVATAR |
| 13 | `AgeWeight(5)` | `AllNegative` | KBO | SOS plus quadratic function-depth penalty |
| 14 | `SmallestFirst` | `All` | KBO | `ConjSymbolBoost`, cap 100, no AVATAR |
| 15 | `AgeWeight(4)` | `AllNegative` | KBO | `SymbolWeight`, no cap, no AVATAR |

The generic schedule also contains strategy 16, a zero-time diagnostic
configuration (`SmallestFirst`, `All`, cap 15, AVATAR). It can be selected by
`MRS_SINGLE_STRATEGY=16`; the public `--strategy` option accepts IDs 1 through
15.

On equality-free inputs, the search engine may dynamically switch
`AllNegative` to a single maximal negative literal unless disabled with
`MRS_NO_SINGLE_NEG`.

## Named schedules

| Name | Purpose |
|---|---|
| `casc` / `default` | Generic CASC portfolio; default behavior. |
| `casc_feq` | First-order with equality. |
| `casc_fne` | First-order without equality. |
| `casc_ueq` | Unit equality, with complementary goal transformations. |
| `casc_epr` | Generic EPR fallback. |
| `casc_eps` | EPR satisfiable local regression schedule. |
| `casc_epu` | EPR unsatisfiable local regression schedule. |
| `casc_icu` | Legacy ICU local regression schedule. |
| `fast` | One KBO strategy for short ATP queries. |
| `mini` | Three-strategy short-budget portfolio. |
| `mq` | Multi-queue portfolio using unit, Horn, goal, SOS, age, and weight queues. |
| `ml`, `ml_feq`, `ml_fne`, `ml_ueq`, `ml_epr` | Experimental ML-guided variants; require compatible ML support for meaningful guidance. |

The registry currently exposes these 16 names through `--list-schedules`:

```text
casc casc_feq casc_fne casc_ueq casc_epr casc_eps casc_epu casc_icu
fast mini ml ml_feq ml_fne ml_ueq ml_epr mq
```

## Current division orders

The schedules use these canonical 15-entry priority orders. When a division
schedule is run with fewer workers, the prefix is used; when run with more
workers, entries cycle through the order.

| Schedule | Order |
|---|---|
| `casc_feq` | `11,12,1,6,10,8,14,4,5,2,3,7,9,13,15` |
| `casc_fne` | `11,8,4,15,10,3,12,1,6,2,5,7,9,13,14` |
| `casc_ueq` | `4,8,12,11,2,14,15,1,3,5,6,7,9,10,13` |
| `casc_epr` | `6,2,1,3,4,5,7,8,9,10,11,12,13,14,15` |
| `casc_eps` | `2,3,1,8,11,12,9,14,7,10,5,13,15,6,4` |
| `casc_epu` | `1,6,14,11,4,2,3,7,5,8,10,9,12,13,15` |
| `casc_icu` | `12,1,2,3,4,5,6,7,8,9,10,11,13,14,15` |

These orders are candidate portfolios derived from solo sweeps. They are not a
claim that their solo union equals cooperative coverage. Use the cooperative
benchmark workflow before changing them.

## Explicit portfolios

An explicit portfolio must use a CASC schedule name, one valid base ID per
worker, and the same worker count as the list length:

```bash
MRS_SHARED_POOL_INTERVAL=500 \
  nix develop -c cargo run -- --workers 8 --schedule casc_feq \
  --portfolio 11,12,1,6,10,8,14,4 problem.p
```
