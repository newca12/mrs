# Intermittent certified solves by competition division

This is the current inventory of problems that have a **strictly certified
solve in at least one complete division campaign and no certified solve in at
least one other included campaign** for the same competition edition/division.
For EPS, a certified solve means a `VerifiedGood` model certificate. A dash in
the `Certified in`/`Not certified in` sense does not mean a failed proof: it
means that campaign did not produce a strict `VerifiedGood` answer (or checked
EPS model) for that problem.

Only complete division runs with archived `certification/audit.csv` data are
considered. A solve counts only when that row is `VerifiedGood`; raw `Theorem`,
`Unsatisfiable`, or `Satisfiable` without a strict good audit is not enough.
Thus historical unsound refutations and old audit rows marked `VerifiedBad` are
not counted as solves. The 2026-10-02 CASC-30 FEQ campaign is omitted because
its archived audit is stale and contains `VerifiedBad` rows; the later
per-proof re-audit has not been materialized as a row-level campaign audit in
this inventory. The incomplete FEQ campaign at
`campaign-feq-W8C16J2-20260930` is also omitted because it evaluated 600 rows
with artifact-mismatch entries, rather than one auditable 300-problem division
run.

This inventory is an **observed run-to-run difference list**, not a claim that
the prover is nondeterministic under a fixed binary and configuration. Included
runs differ in binary, host, hardware mode, strategy schedule, or other settings;
the list is useful for targeting matched repeated measurements. A new complete,
strictly audited run should be added to the campaign key below and the signatures
recomputed.

## Included campaign keys

`+` means the problem is certified in that run; `-` means it is not.
In the tables, each row groups problem names with the same certified/not-
certified signature.

| Key | Edition / division | Campaign |
|---|---|---|
| `j13-16c` | CASC-J13 FEQ | `campaign-casc-j13-W8C8J1-20261016` (combined CASC-mode run) |
| `j13-16a` | CASC-J13 FEQ | `campaign-cascj13-feq-W8C8J1-20261016` (standalone adaptive-mode run) |
| `j13-16` | CASC-J13 FNE, UEQ | `campaign-casc-j13-W8C8J1-20261016` (combined CASC-mode run) |
| `j13-02` | CASC-J13 FNE, UEQ | `campaign-cascj13-fne-W8C8J1-20261002`; `campaign-cascj13-ueq-W8C8J1-20261002` |
| `j13-01c`, `j13-01r` | CASC-J13 FNE | `campaign-cascj13-fne-condensed-W8C16J2-20261001`; `campaign-cascj13-fne-ref-W8C16J2-20261001` |
| `j13-clean` | CASC-J13 FEQ, FNE, UEQ | `campaign-cascj13-clean` (strict-audited clean run) |
| `j13-recov` | CASC-J13 UEQ | `ueq-syntactic-recovery-j13` |
| `j13-09` | CASC-J13 UEQ | `campaign-ueq-W8C16J1-20260930` |
| `c30-16` | CASC-30 FEQ, UEQ | `campaign-casc30-ueq-feq-W8CASCHJ1-20261016` (combined CASC-mode run) |
| `c30-clean` | CASC-30 FEQ | `campaign-casc30-feq-clean` |
| `c30-19` | CASC-30 FNE, EPS, EPU | `campaign-casc30-fne-W8CASCHJ1-99-20261019`; `campaign-casc30-eps-W8CASCHJ1-01-20261019`; `campaign-casc30-epu-W8CASCHJ1-02-20261019` |
| `c30-02` | CASC-30 FNE, EPU | `campaign-casc30-fne-W8C8J1-20261002`; `campaign-casc30-epu-W8C8J1-20261002` |
| `c30-01c`, `c30-01r` | CASC-30 FNE | `campaign-casc30-fne-condensed-W8C16J2-20261001`; `campaign-casc30-fne-ref-W8C16J2-20261001` |
| `c30-07`, `c30-02`, `c30-05`, `c30-16` | CASC-30 UEQ | `campaign-casc30-ueq-W8C8J1-20261007`; `campaign-casc30-ueq-W8C8J1-20261002`; `campaign-casc30-ueq-W8P8J1-NOLRS-MB90G-20261005`; `campaign-casc30-ueq-feq-W8CASCHJ1-20261016` (combined FEQ/UEQ campaign) |
| `c30-28`, `c30-26` | CASC-30 EPS | `cert-eps-20260928`; `cert-eps-20260926` |
| `c30-26` | CASC-30 EPU | `cert-epu-20260926` |
| `c30-icu-02` | CASC-30 ICU | `campaign-casc30-icu-W8C8J1-20261002` |

## CASC-J13

### FEQ — 3 intermittent problems

| Certified in | Not certified in | Problems |
|---|---|---|
| `j13-16c` | `j13-16a` | `GEO277+1`, `SWB073+1` |
| `j13-16a` | `j13-16c` | `SWW298+1` |

### FNE — 5 intermittent problems

| Certified in | Not certified in | Problems |
|---|---|---|
| `j13-16` | `j13-02`, `j13-01c`, `j13-01r`, `j13-clean` | `KRS234+1`, `KRS261+1`, `SYO605+1` |
| `j13-16`, `j13-clean` | `j13-02`, `j13-01c`, `j13-01r` | `CSR115+8`, `CSR116+19` |

### UEQ — 122 intermittent problems

| Certified in | Not certified in | Problems |
|---|---|---|
| `j13-16`, `j13-recov`, `j13-02`, `j13-09` | `j13-clean` | `ALG245-1`, `ANA034-10`, `BOO017-10`, `COL002-4`, `COL003-10`, `COL006-6`, `COL042-10`, `CSR034-10`, `CSR044-10`, `CSR051-10`, `CSR057-10`, `CSR059-10`, `CSR068-10`, `CSR073-10`, `DAT436-1`, `DAT436-2`, `GRP002-10`, `GRP048-10`, `GRP183-3`, `GRP186-1`, `GRP404-1`, `GRP416-1`, `GRP419-1`, `GRP422-1`, `GRP439-1`, `GRP442-1`, `GRP469-1`, `GRP472-1`, `GRP557-1`, `GRP597-1`, `GRP655-11`, `GRP655-12`, `GRP655-14`, `GRP656-10`, `GRP696-1`, `GRP697-1`, `GRP751-1`, `GRP752-1`, `GRP753-1`, `GRP766-1`, `GRP772-1`, `HEN010-10`, `KLE044-10`, `KLE084-10`, `KLE152-10`, `KLE169-10`, `LAT017-1`, `LAT018-1`, `LAT070-1`, `LAT072-1`, `LAT076-1`, `LAT078-1`, `LAT080-1`, `LAT081-1`, `LAT083-1`, `LAT084-1`, `LAT085-1`, `LAT092-1`, `LAT093-1`, `LAT094-1`, `LAT095-1`, `LAT171-1`, `LAT389-1`, `LAT390-1`, `LAT393-2`, `LCL026-10`, `LCL042-10`, `LCL050-10`, `LCL109-2`, `LCL160-1`, `LCL203-10`, `LCL218-10`, `LCL265-10`, `LCL890-10`, `LCL897-10`, `MVA009-1`, `NUN135-1.006`, `PLA017-10`, `PUZ164-1`, `REL010-2`, `REL012-2`, `REL024-1`, `REL031-1`, `REL042-1`, `REL042-2`, `REL043-2`, `REL044-2`, `REL045-2`, `REL050-1`, `REL050-3`, `RNG006-10`, `SWX203-1`, `SWX239-1`, `SYN563-10`, `SYN598-10`, `SYN708-10`, `TOP052-1` |
| `j13-16`, `j13-recov` | `j13-02`, `j13-09`, `j13-clean` | `GRP654-11`, `GRP654-12`, `GRP655-13`, `GRP705-1`, `LAT157-1`, `REL041-2` |
| `j13-02`, `j13-09` | `j13-16`, `j13-recov`, `j13-clean` | `LAT174-1`, `REL012-1`, `RNG009-5`, `RNG009-7` |
| `j13-16`, `j13-recov`, `j13-02` | `j13-09`, `j13-clean` | `GRP660-10`, `LAT096-1`, `LAT097-1` |
| `j13-clean` | `j13-16`, `j13-recov`, `j13-02`, `j13-09` | `RNG019-7`, `RNG020-7`, `RNG021-7` |
| `j13-16` | `j13-recov`, `j13-02`, `j13-09`, `j13-clean` | `KLE147-10`, `LAT222-10` |
| `j13-16`, `j13-02`, `j13-09` | `j13-recov`, `j13-clean` | `ALG240-1`, `KLE145-10` |
| `j13-02` | `j13-16`, `j13-recov`, `j13-09`, `j13-clean` | `REL021-1` |
| `j13-09` | `j13-16`, `j13-recov`, `j13-02`, `j13-clean` | `LCL220-10` |
| `j13-16`, `j13-recov`, `j13-09` | `j13-02`, `j13-clean` | `LAT044-1` |
| `j13-recov` | `j13-16`, `j13-02`, `j13-09`, `j13-clean` | `REL026-3` |
| `j13-recov`, `j13-02` | `j13-16`, `j13-09`, `j13-clean` | `REL019-1` |

## CASC-30

### FEQ — 43 intermittent problems

| Certified in | Not certified in | Problems |
|---|---|---|
| `c30-16` | `c30-clean` | `ALG210+2`, `GEO126+1`, `GEO331+1`, `GEO499+1`, `GEO524+1`, `GEO559+1`, `GEO641+1`, `ITP013+1`, `KLE113+1`, `LCL518+1`, `LCL895+1`, `LCL896+1`, `LCL901+1`, `MGT054+1`, `NUM835+1`, `NUM852+1`, `NUM926+2`, `NUN086+2`, `SET017+1`, `SET072+1`, `SET090+1`, `SET637+3`, `SWV449+1`, `SWV451+1`, `SWV452+1`, `SWV453+1`, `SWV455+1`, `SWV457+1`, `SWV459+1`, `SWV460+1`, `SWV462+1`, `SWV466+1`, `SWV468+1`, `SWV471+1`, `SWV473+1`, `SWV476+1`, `SWV480+1`, `SWV487+1`, `SWW383+1` |
| `c30-clean` | `c30-16` | `GEO625+1`, `GRA009+1`, `SWB073+1`, `SWW313+1` |

### FNE — 12 intermittent problems

| Certified in | Not certified in | Problems |
|---|---|---|
| `c30-19` | `c30-02`, `c30-01c`, `c30-01r` | `CSR115+31`, `CSR115+6`, `CSR115+91`, `CSR116+6`, `KRS203+1`, `LCL390+1`, `LCL642+1.015`, `LCL660+1.020`, `MED007+1`, `MED009+1`, `SYN938+1` |
| `c30-19`, `c30-02`, `c30-01c` | `c30-01r` | `LCL660+1.015` |

### UEQ — 39 intermittent problems

| Certified in | Not certified in | Problems |
|---|---|---|
| `c30-07`, `c30-02`, `c30-16` | `c30-05` | `GRP167-4`, `GRP185-3`, `GRP423-1`, `GRP667-3`, `GRP766-1`, `KLE152-10`, `REL019-2`, `REL031-2`, `TOP052-1` |
| `c30-07`, `c30-02`, `c30-05` | `c30-16` | `LAT174-1`, `MVA009-1`, `REL045-2`, `RNG006-10`, `RNG009-5`, `RNG009-7` |
| `c30-07`, `c30-02` | `c30-05`, `c30-16` | `LAT241-10`, `MVA008-1`, `REL005-3`, `REL027-3`, `REL035-1` |
| `c30-07`, `c30-16` | `c30-02`, `c30-05` | `GRP654-12`, `GRP655-13`, `GRP655-14`, `GRP667-10`, `LAT022-1` |
| `c30-16` | `c30-07`, `c30-02`, `c30-05` | `GRP167-3`, `GRP184-1`, `KLE145-10`, `LAT168-1`, `REL021-1` |
| `c30-05` | `c30-07`, `c30-02`, `c30-16` | `COL009-1`, `COL057-1`, `LCL400-10`, `SYN615-10` |
| `c30-02` | `c30-07`, `c30-05`, `c30-16` | `REL028-2`, `REL034-1` |
| `c30-07` | `c30-02`, `c30-05`, `c30-16` | `MVA011-1`, `REL032-1` |
| `c30-07`, `c30-05` | `c30-02`, `c30-16` | `LCL211-10` |

### EPS — 36 intermittent problems

| Certified in | Not certified in | Problems |
|---|---|---|
| `c30-19`, `c30-28` | `c30-26` | `GRP123-4.004`, `GRP123-6.005`, `GRP123-8.004`, `GRP124-3.005`, `GRP124-4.005`, `GRP124-9.005`, `GRP125-2.004`, `GRP125-3.004`, `GRP125-4.004`, `GRP126-2.005`, `GRP126-4.005`, `GRP128-3.004`, `GRP128-4.004`, `GRP129-3.005`, `GRP129-4.005`, `GRP130-3.004`, `GRP132-1.005`, `NLP116-1`, `NLP121-1`, `PUZ001-3`, `PUZ018-2`, `PUZ028-1`, `PUZ028-2`, `PUZ028-3`, `PUZ028-4`, `SYN056-1`, `SYN059-1`, `SYN438-1`, `SYN446-1`, `SYN490-1`, `SYN494-1`, `SYN515-1`, `SYN527-1`, `SYN720-1` |
| `c30-19` | `c30-28`, `c30-26` | `SYN441-1`, `SYN529-1` |

The October 19 campaign is the current addition to this signature: its models
for both `SYN441-1` and `SYN529-1` are now strict `VerifiedGood`; their earlier
September 28 results were `inconclusive_model`. The two problems were already
raw `Satisfiable` in September, so this is a model-certification recovery, not a
new raw solver answer.

### EPU — 2 intermittent problems

| Certified in | Not certified in | Problems |
|---|---|---|
| `c30-19` | `c30-02`, `c30-26` | `HWV078-1`, `MSC024-1` |

The October 19 EPU campaign certified both intermittent problems. Their earlier
audited runs returned `Timeout`; the new `HWV078-1` proof uses the
`derivation_loop` extractor and `MSC024-1` uses `lrat_lift`.

### ICU — no intermittent list yet

Only one complete, audited CASC-30 ICU campaign is currently in the results
tree: `c30-icu-02` (`campaign-casc30-icu-W8C8J1-20261002`). It certified 3 of
5 generated proofs; one run cannot establish that any problem is intermittent.
Add this division once another complete, row-level audited ICU campaign is
available.
