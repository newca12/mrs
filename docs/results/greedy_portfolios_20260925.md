# Greedy portfolio compromise — 2026-09-25

## Scope and method

These candidate orders combine greedy set-cover results from the CASC-30 and
CASC-J13 solo-strategy sweeps. Strategy IDs are the base strategies in
`mrs-search`; the first eight entries are used for the canonical 8-worker
portfolio. The remaining entries preserve all 15 unique strategies and allow
the schedule to scale to more workers.

The source reports are:

- `/mnt/c/Users/fr22192/tmp/sweep/casc-30-sweep-fne-20260924.out`
- `/mnt/c/Users/fr22192/tmp/sweep/casc-30-sweep-ueq-20260924.out`
- `/mnt/c/Users/fr22192/tmp/sweep/casc-j13-sweep-fne-20260924.out`
- `/mnt/c/Users/fr22192/tmp/sweep/casc-j13-sweep-ueq-20260924.out`

These are solo coverage diagnostics, not measurements of an 8-worker
cooperative portfolio. No cooperative run results are included here.

## Recommended 8-worker orders

| Division | Compromise order | CASC-30 greedy result | CASC-J13 greedy result |
|---|---|---|---|
| FNE | `11,8,4,15,10,3,12,1` | `8,15,11,3,4` covers 42/42 | `11,4,10` covers 35/35 |
| UEQ | `4,8,12,11,2,14,15,1` | `4,8,12,11,1,2,5,14` covers 136/139 | Exact compromise order covers 122/124 |

For FNE, the compromise contains both editions' complete reported greedy
portfolios, so it retains their respective solo union coverage. For UEQ, the
J13 compromise order is the reported 8-strategy order. On CASC-30 it replaces
strategy 5 from the reported 8-strategy greedy set with strategy 15; strategy
15 adds two problems at the next greedy step, but the exact union for this
replacement set was not reported. The CASC-30 figure of 136/139 therefore
describes its original greedy order, not a measured result for the compromise.

## Implementation

The compromise orders are in `CASC_FNE_ORDER` and `CASC_UEQ_ORDER` in
`crates/mrs-search/src/strategy/named.rs`. The complete 15-entry orders are:

- FNE: `11,8,4,15,10,3,12,1,6,2,5,7,9,13,14`
- UEQ: `4,8,12,11,2,14,15,1,3,5,6,7,9,10,13`

Validate the 8-worker schedules with cooperative portfolio sweeps before
treating solo set-cover coverage as cooperative performance.
