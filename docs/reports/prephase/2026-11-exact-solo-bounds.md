# Exact solo-matrix bounds

This is an exact combinatorial analysis of the archived solo outcome matrices,
not a concurrent portfolio measurement and not a strict proof-certification
audit. Only raw `refutation` outcomes at or before each official time cap count.
The saved sweeps contain no saturation outcomes. Time is censored using the
solver `elapsed_s` field; the original sweep CSVs contain 240-second runs.

## Inputs and validation

| Edition | Input | Raw rows | EPU alias rows removed | Canonical rows | Instances | SHA-256 |
|---|---|---:|---:|---:|---:|---|
| casc-30 | `casc30_sweep_t240.csv` | 16320 | 1455 | 14865 | 991 | `554a5782db4f6b697e421d937aef522de72e7ccb040b0127b8427a9998b0dad3` |
| casc-j13 | `cascj13_sweep_t240.csv` | 11940 | 0 | 11940 | 796 | `d777b00bb437d82a518acb0f79c48d8ab027e7283296807c91d0e5c9b0d1235a` |

The two EPU path aliases are deduplicated only after their per-strategy
verdicts and time-censored refutation outcomes agree. Same-named problems
in different editions remain separate. Every canonical instance must have
one row for each of the 15 base strategies.

## Edition-level totals

These sum the division-local coverage counts; they do not represent one
shared portfolio across divisions.

| Edition | Instances | All-15 union | Current prefix-8 | Exact best-8 |
|---|---:|---:|---:|---:|
| casc-30 | 991 | 264 | 239 | 262 |
| casc-j13 | 796 | 238 | 230 | 235 |

## Per-division bounds

`Current prefix` is the first eight base-strategy IDs in the shipped named
schedule, projected onto these solo outcomes. It does not model the live
schedule's transformations, SInE behavior, sharing, or worker contention.

| Edition | Division | Cap (s) | Instances | Raw refutation cells | Censored | All-15 union | Current prefix-8 | Exact best-8 | Best-8 gap to union | Min width to union |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| casc-30 | EPU | 120 | 97 | 104 | 1 | 9 | 7 | 9 | 0 | 1 |
| casc-30 | FEQ | 240 | 398 | 265 | 0 | 95 | 93 | 95 | 0 | 6 |
| casc-30 | FNE | 240 | 97 | 137 | 1 | 30 | 21 | 30 | 0 | 5 |
| casc-30 | ICU | 240 | 99 | 0 | 0 | 0 | 0 | 0 | 0 | — |
| casc-30 | UEQ | 240 | 300 | 601 | 0 | 130 | 118 | 128 | 2 | — |
| casc-j13 | FEQ | 180 | 299 | 313 | 4 | 66 | 65 | 66 | 0 | 5 |
| casc-j13 | FNE | 180 | 97 | 137 | 7 | 32 | 30 | 32 | 0 | 5 |
| casc-j13 | UEQ | 180 | 400 | 609 | 36 | 140 | 135 | 137 | 3 | — |

### casc-30 / EPU

- Shipped base-strategy order: `[1, 6, 14, 11, 4, 2, 3, 7, 5, 8, 10, 9, 12, 13, 15]`.
- Exact matrix classification: all-15 union is reachable with 1 strategies.
- Capped refutation cells: 103; raw refutation cells beyond the cap: 1.
- Raw status counts after EPU alias deduplication: `{'GaveUp': 140, 'Timeout': 1178, 'Unsatisfiable': 104, 'hard_timeout': 33}`.

| Width | Exact maximum coverage | Number of optimal sets | Example optimal set(s) |
|---:|---:|---:|---|
| 1 | 9 | 1 | `[s5]` |
| 2 | 9 | 14 | `[s1, s5]; [s2, s5]; [s3, s5]` |
| 3 | 9 | 91 | `[s1, s2, s5]; [s1, s3, s5]; [s1, s4, s5]` |
| 4 | 9 | 364 | `[s1, s2, s3, s5]; [s1, s2, s4, s5]; [s1, s2, s5, s6]` |
| 5 | 9 | 1001 | `[s1, s2, s3, s4, s5]; [s1, s2, s3, s5, s6]; [s1, s2, s3, s5, s7]` |
| 6 | 9 | 2002 | `[s1, s2, s3, s4, s5, s6]; [s1, s2, s3, s4, s5, s7]; [s1, s2, s3, s4, s5, s8]` |
| 7 | 9 | 3003 | `[s1, s2, s3, s4, s5, s6, s7]; [s1, s2, s3, s4, s5, s6, s8]; [s1, s2, s3, s4, s5, s6, s9]` |
| 8 | 9 | 3432 | `[s1, s2, s3, s4, s5, s6, s7, s8]; [s1, s2, s3, s4, s5, s6, s7, s9]; [s1, s2, s3, s4, s5, s6, s7, s10]` |

### casc-30 / FEQ

- Shipped base-strategy order: `[12, 11, 10, 6, 8, 1, 4, 13, 15, 14, 5, 2, 3, 7, 9]`.
- Exact matrix classification: all-15 union is reachable with 6 strategies.
- Capped refutation cells: 265; raw refutation cells beyond the cap: 0.
- Raw status counts after EPU alias deduplication: `{'GaveUp': 206, 'Theorem': 265, 'Timeout': 5251, 'hard_timeout': 248}`.

| Width | Exact maximum coverage | Number of optimal sets | Example optimal set(s) |
|---:|---:|---:|---|
| 1 | 67 | 1 | `[s12]` |
| 2 | 81 | 1 | `[s6, s12]` |
| 3 | 86 | 2 | `[s6, s8, s12]; [s6, s11, s12]` |
| 4 | 90 | 2 | `[s6, s8, s12, s15]; [s6, s11, s12, s15]` |
| 5 | 93 | 2 | `[s6, s8, s11, s12, s15]; [s6, s10, s11, s12, s15]` |
| 6 | 95 | 2 | `[s6, s8, s10, s11, s12, s15]; [s6, s8, s11, s12, s13, s15]` |
| 7 | 95 | 17 | `[s1, s6, s8, s10, s11, s12, s15]; [s1, s6, s8, s11, s12, s13, s15]; [s2, s6, s8, s10, s11, s12, s15]` |
| 8 | 95 | 64 | `[s1, s2, s6, s8, s10, s11, s12, s15]; [s1, s2, s6, s8, s11, s12, s13, s15]; [s1, s3, s6, s8, s10, s11, s12, s15]` |

### casc-30 / FNE

- Shipped base-strategy order: `[11, 13, 4, 8, 15, 7, 3, 1, 10, 12, 6, 2, 9, 14, 5]`.
- Exact matrix classification: all-15 union is reachable with 5 strategies.
- Capped refutation cells: 136; raw refutation cells beyond the cap: 1.
- Raw status counts after EPU alias deduplication: `{'GaveUp': 56, 'Theorem': 137, 'Timeout': 1093, 'hard_timeout': 169}`.

| Width | Exact maximum coverage | Number of optimal sets | Example optimal set(s) |
|---:|---:|---:|---|
| 1 | 21 | 1 | `[s12]` |
| 2 | 25 | 1 | `[s12, s15]` |
| 3 | 28 | 1 | `[s12, s14, s15]` |
| 4 | 29 | 5 | `[s4, s12, s14, s15]; [s8, s12, s14, s15]; [s10, s12, s14, s15]` |
| 5 | 30 | 4 | `[s4, s8, s12, s14, s15]; [s4, s10, s12, s14, s15]; [s4, s11, s12, s14, s15]` |
| 6 | 30 | 34 | `[s1, s4, s8, s12, s14, s15]; [s1, s4, s10, s12, s14, s15]; [s1, s4, s11, s12, s14, s15]` |
| 7 | 30 | 130 | `[s1, s2, s4, s8, s12, s14, s15]; [s1, s2, s4, s10, s12, s14, s15]; [s1, s2, s4, s11, s12, s14, s15]` |
| 8 | 30 | 295 | `[s1, s2, s3, s4, s8, s12, s14, s15]; [s1, s2, s3, s4, s10, s12, s14, s15]; [s1, s2, s3, s4, s11, s12, s14, s15]` |

### casc-30 / ICU

- Shipped base-strategy order: `[12, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 14, 15]`.
- Exact matrix classification: no raw refutations observed in the solo matrix.
- Capped refutation cells: 0; raw refutation cells beyond the cap: 0.
- Raw status counts after EPU alias deduplication: `{'GaveUp': 81, 'Timeout': 1395, 'hard_timeout': 9}`.

| Width | Exact maximum coverage | Number of optimal sets | Example optimal set(s) |
|---:|---:|---:|---|
| 1 | 0 | 15 | `[s1]; [s2]; [s3]` |
| 2 | 0 | 105 | `[s1, s2]; [s1, s3]; [s1, s4]` |
| 3 | 0 | 455 | `[s1, s2, s3]; [s1, s2, s4]; [s1, s2, s5]` |
| 4 | 0 | 1365 | `[s1, s2, s3, s4]; [s1, s2, s3, s5]; [s1, s2, s3, s6]` |
| 5 | 0 | 3003 | `[s1, s2, s3, s4, s5]; [s1, s2, s3, s4, s6]; [s1, s2, s3, s4, s7]` |
| 6 | 0 | 5005 | `[s1, s2, s3, s4, s5, s6]; [s1, s2, s3, s4, s5, s7]; [s1, s2, s3, s4, s5, s8]` |
| 7 | 0 | 6435 | `[s1, s2, s3, s4, s5, s6, s7]; [s1, s2, s3, s4, s5, s6, s8]; [s1, s2, s3, s4, s5, s6, s9]` |
| 8 | 0 | 6435 | `[s1, s2, s3, s4, s5, s6, s7, s8]; [s1, s2, s3, s4, s5, s6, s7, s9]; [s1, s2, s3, s4, s5, s6, s7, s10]` |

### casc-30 / UEQ

- Shipped base-strategy order: `[4, 8, 12, 11, 2, 14, 15, 1, 3, 5, 6, 7, 9, 10, 13]`.
- Exact matrix classification: best fixed 8-set leaves 2 all-15-union instances uncovered.
- Capped refutation cells: 601; raw refutation cells beyond the cap: 0.
- Raw status counts after EPU alias deduplication: `{'GaveUp': 410, 'Timeout': 3463, 'Unsatisfiable': 601, 'hard_timeout': 26}`.

| Width | Exact maximum coverage | Number of optimal sets | Example optimal set(s) |
|---:|---:|---:|---|
| 1 | 80 | 1 | `[s6]` |
| 2 | 99 | 1 | `[s6, s11]` |
| 3 | 109 | 2 | `[s1, s6, s11]; [s6, s11, s12]` |
| 4 | 116 | 2 | `[s1, s6, s8, s11]; [s1, s6, s11, s12]` |
| 5 | 122 | 1 | `[s1, s6, s8, s11, s12]` |
| 6 | 125 | 1 | `[s1, s2, s6, s8, s11, s12]` |
| 7 | 127 | 1 | `[s1, s2, s6, s7, s8, s11, s12]` |
| 8 | 128 | 4 | `[s1, s2, s3, s6, s7, s8, s11, s12]; [s1, s2, s4, s6, s7, s8, s11, s12]; [s1, s2, s5, s6, s7, s8, s11, s12]` |

### casc-j13 / FEQ

- Shipped base-strategy order: `[12, 11, 10, 6, 8, 1, 4, 13, 15, 14, 5, 2, 3, 7, 9]`.
- Exact matrix classification: all-15 union is reachable with 5 strategies.
- Capped refutation cells: 309; raw refutation cells beyond the cap: 4.
- Raw status counts after EPU alias deduplication: `{'GaveUp': 226, 'ResourceOut': 8, 'Theorem': 313, 'Timeout': 3753, 'exit-1': 10, 'hard_timeout': 175}`.

| Width | Exact maximum coverage | Number of optimal sets | Example optimal set(s) |
|---:|---:|---:|---|
| 1 | 54 | 1 | `[s12]` |
| 2 | 58 | 3 | `[s6, s12]; [s8, s12]; [s12, s14]` |
| 3 | 62 | 1 | `[s6, s8, s12]` |
| 4 | 65 | 1 | `[s6, s8, s12, s13]` |
| 5 | 66 | 2 | `[s2, s6, s8, s12, s13]; [s6, s8, s12, s13, s15]` |
| 6 | 66 | 19 | `[s1, s2, s6, s8, s12, s13]; [s1, s6, s8, s12, s13, s15]; [s2, s3, s6, s8, s12, s13]` |
| 7 | 66 | 81 | `[s1, s2, s3, s6, s8, s12, s13]; [s1, s2, s4, s6, s8, s12, s13]; [s1, s2, s5, s6, s8, s12, s13]` |
| 8 | 66 | 204 | `[s1, s2, s3, s4, s6, s8, s12, s13]; [s1, s2, s3, s5, s6, s8, s12, s13]; [s1, s2, s3, s6, s7, s8, s12, s13]` |

### casc-j13 / FNE

- Shipped base-strategy order: `[11, 13, 4, 8, 15, 7, 3, 1, 10, 12, 6, 2, 9, 14, 5]`.
- Exact matrix classification: all-15 union is reachable with 5 strategies.
- Capped refutation cells: 130; raw refutation cells beyond the cap: 7.
- Raw status counts after EPU alias deduplication: `{'GaveUp': 43, 'ResourceOut': 1, 'Theorem': 137, 'Timeout': 1158, 'hard_timeout': 116}`.

| Width | Exact maximum coverage | Number of optimal sets | Example optimal set(s) |
|---:|---:|---:|---|
| 1 | 20 | 1 | `[s15]` |
| 2 | 28 | 1 | `[s6, s11]` |
| 3 | 30 | 1 | `[s6, s11, s15]` |
| 4 | 31 | 3 | `[s4, s6, s11, s15]; [s4, s11, s12, s15]; [s6, s11, s13, s15]` |
| 5 | 32 | 2 | `[s4, s6, s11, s13, s15]; [s4, s11, s12, s13, s15]` |
| 6 | 32 | 19 | `[s1, s4, s6, s11, s13, s15]; [s1, s4, s11, s12, s13, s15]; [s2, s4, s6, s11, s13, s15]` |
| 7 | 32 | 81 | `[s1, s2, s4, s6, s11, s13, s15]; [s1, s2, s4, s11, s12, s13, s15]; [s1, s3, s4, s6, s11, s13, s15]` |
| 8 | 32 | 204 | `[s1, s2, s3, s4, s6, s11, s13, s15]; [s1, s2, s3, s4, s11, s12, s13, s15]; [s1, s2, s4, s5, s6, s11, s13, s15]` |

### casc-j13 / UEQ

- Shipped base-strategy order: `[4, 8, 12, 11, 2, 14, 15, 1, 3, 5, 6, 7, 9, 10, 13]`.
- Exact matrix classification: best fixed 8-set leaves 3 all-15-union instances uncovered.
- Capped refutation cells: 573; raw refutation cells beyond the cap: 36.
- Raw status counts after EPU alias deduplication: `{'GaveUp': 655, 'ResourceOut': 3, 'Timeout': 4685, 'Unsatisfiable': 609, 'exit-1': 2, 'hard_timeout': 46}`.

| Width | Exact maximum coverage | Number of optimal sets | Example optimal set(s) |
|---:|---:|---:|---|
| 1 | 81 | 1 | `[s1]` |
| 2 | 103 | 1 | `[s1, s12]` |
| 3 | 115 | 1 | `[s1, s8, s12]` |
| 4 | 123 | 1 | `[s1, s2, s8, s12]` |
| 5 | 130 | 1 | `[s1, s2, s8, s12, s15]` |
| 6 | 133 | 1 | `[s1, s2, s5, s8, s12, s15]` |
| 7 | 135 | 3 | `[s1, s2, s3, s5, s8, s12, s15]; [s1, s2, s5, s7, s8, s12, s15]; [s1, s2, s5, s8, s12, s14, s15]` |
| 8 | 137 | 2 | `[s1, s2, s3, s5, s7, s8, s12, s15]; [s1, s2, s5, s7, s8, s12, s14, s15]` |

## Interpretation limits

- These bounds are exact only for the archived solo outcomes after the stated
  time censoring. The raw SZS outcomes are not strict proof certificates.
- The all-15 union is an observed-matrix ceiling, not a completeness result.
- An exact best 8-set is a candidate for cooperative testing, not evidence that
  it will retain the same coverage with eight concurrent workers.
- Every optimal width-8 strategy set is listed in the companion JSON artifact.

Shipped-order source: `named.rs` (SHA-256 `ccb00058c06af3f3b5b56a5e4711f2a32c3b11a50a9314de8a43a611a943dced`).
