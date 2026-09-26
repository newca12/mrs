# Proof Size: rules, measurements, and the output budget

`mrs` records every inference in the derivation DAG, so proof size is a
function of how long the derivation is, not of how large the problem is. This
note records the competition constraint, what `mrs` currently emits, and the
budget that keeps a runaway proof from costing more than the proof itself.

## What the rules say

CASC does not leave proof size unbounded. From the CASC design documents
(<https://tptp.org/CASC/23/Design.html>, and the equivalent wording in the
other editions):

> For practical reasons excessive output from the ATP systems is not allowed.
> A limit, dependent on the disk space available, is imposed on the amount of
> `stdout` and `stderr` output that can be produced. The limit is at least
> 10MB per system.

Two consequences matter here:

1. A system has *solved* a problem iff it outputs its termination string
   within the time limit, and has *produced a proof* iff it outputs its
   end-of-proof string within the time limit. A proof that is too large can
   therefore cost the proof credit, and
2. the panel checks the proofs of the winners after the competition and
   downgrades systems whose proofs are unacceptable.

The exact limit is set by the organizers per run, and `at least 10MB per
system` is the floor stated in the rules. The editions before CASC-23 phrased
the floor per problem instead ("at least 10KB per problem (averaged over all
problems)"), so the practical target is a per-problem proof well under 10MB.

## What `mrs` currently emits

Measured over the 215 CASC-J13 refutations archived by the 2026-09-23
`casc-j13-W8J2-nosharing-20260923` run:

| statistic | value |
|---|---|
| median | 17 KB |
| p90 | 1.0 MB |
| max | 170 MB |
| > 1 MB | 22 / 215 |
| > 5 MB | 10 / 215 |

The tail is not diffuse. The three largest proofs are 170 MB (`feq/SWV406+1`,
657 454 nodes), 38 MB (`feq/MGT079+1`) and 31 MB (`feq/SET017+1`), and all
three are **AVATAR-dominated**: roughly half of their nodes are
`avatar_split_clause` steps, one per split occurrence, each carrying the full
`avatar_split([branch(…), …], […])` certificate. `feq/SWV406+1` is 325 179
split nodes and 320 061 resolution steps — the splits, not the calculus, are
the cost.

Three of 215 proofs (1.4 %) exceed the 10MB floor. That is a bounded, known
risk rather than a systemic one, and it is confined to one division.

## The output budget

`--proof-bytes-limit N` (default 8 MiB) caps the proof body. A proof larger
than the cap is *omitted* rather than truncated:

```
% SZS status Theorem for SWV406+1
% Proof omitted: 169921314 bytes / 657454 nodes exceeds the --proof-bytes-limit of 8388608 bytes
```

and the machine-readable detail line always carries the true size:

```
% SZS detail … result=Refutation … proof_nodes=657454 proof_bytes=169921314 proof_emitted=false
```

The default sits below the smallest allowance the CASC rules state on purpose.
An over-budget proof that the harness kills takes the *status line* with it,
so the solve is lost as well as the proof; a proof that is deliberately
omitted keeps the solve and reports exactly what was dropped. Every number is
recorded in `run.csv` via `failure_detail`, so the benchmark harness can
track the distribution without re-running anything.

## Reducing the tail

The remaining lever is the AVATAR split representation, not the calculus.
Options, in increasing order of risk:

1. Deduplicate identical `avatar_split_clause` certificates: several split
   nodes in these proofs differ only in name, so one node could be cited by
   every component that shares the certificate. Sound (a split is a
   proposition, and identical propositions are interchangeable), but it
   changes the derivation graph the proof extractor walks, so it needs the
   AVATAR certificate tests re-run.
2. Emit the branch list once per split and have components reference it by
   index, trading node count for a slightly more complex annotation that
   third-party checkers must also understand — the reason this is second.
3. Bound the number of splits whose certificates are exported at all. This
   would lose the case-split certificate, which the strict kernel requires
   before it will certify an AVATAR refutation, so it trades the proof credit
   for the certification. Not worth it while (1) is available.

Until then the budget makes the failure mode explicit and cheap: three
problems lose their proof, none lose their solve, and the size is visible in
telemetry.
