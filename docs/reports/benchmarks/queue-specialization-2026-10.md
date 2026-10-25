# Passive heap specialization: local structural check

Date: 2026-10-25. Producer source: `b6f18c0`, debug build provided by the
Nix Rust 1.99.1 workspace tests. This is a local regression/structure experiment,
not a throughput measurement, division score, or default-promotion result.

Both arms ran sequentially on the same development host with one worker,
sharing disabled, LRS disabled, `MRS_MAX_PROCESSED=200`, a 10-second guard and
`MRS_TRAJECTORY_DIGEST=1`. Only `MRS_SPECIALIZE_QUEUES=0/1` differed.

| Problem | Schedule / strategy | Processed | Generated | Baseline queue entries | Specialized entries | Selection digest / work |
|---|---|---:|---:|---:|---:|---|
| `bool_hunt.p` | `casc_ueq` / 1 | 6 | 2 | 29 | 2 | identical |
| `group.p` | `casc_ueq` / 4 | 9 | 11 | 62 | 16 | identical |
| `pel43.p` | `casc_fne` / 4 | 40 | 53 | 218 | 70 | identical |

`pel27.p` also returned the same result in both arms but was solved by a
pre-pass (zero given-clause work, no selection digest); it is not evidence
about heap specialization. `bool_hunt.p` returned `GaveUp` in both arms; the
other three returned refutations. No run hit the 200-clause ceiling or deadline.

Full selection digests (same across both arms):

- `bool_hunt.p`: `f61807e5409c448becd5926438a9886f3e76f0546c216b3b99d5ed669d646c1d`
- `group.p`: `0278fc12eab65f2d9b1046cc6a68fc88aa90ad6d06ebe63b3dc31f5907f344b2`
- `pel43.p`: `8f03e51cea434f7602c4853f5674e84108f6546baa39a22d79bbf353a19fe8ca`

Separate specialized-arm runs of `group.p` and `pel43.p` with `--self-check`
passed the isolated strict checker: 24 nodes / 2,607 bytes and 78 nodes /
14,528 bytes respectively, both `self_check=Certified`, zero rejected candidates.

Reproduction for each table row:

```bash
MRS_SPECIALIZE_QUEUES=0 MRS_TRAJECTORY_DIGEST=1 MRS_NO_LRS=1 \
MRS_MAX_PROCESSED=200 MRS_SHARED_POOL_INTERVAL=0 \
  target/debug/mrs --time 10 --workers 1 --schedule casc_ueq --strategy 4 problems/group.p
# Repeat with MRS_SPECIALIZE_QUEUES=1; use the table's schedule/strategy/problem.
```

The measured reduction is in **retained entries**, not RSS or bytes. Initial
state construction still populates compatibility queues before specialization,
so this does not measure peak initialization memory. Representative release,
fixed-work, paired memory/throughput experiments and eight-worker certified
campaigns remain required before changing the default.
