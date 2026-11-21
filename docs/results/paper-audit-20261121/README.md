# Paper-audit replay observations, 2026-11-21

These are immutable copies of locally generated re-audit reports and identities.
They are not official or canonical CASC scores, and do not replace historical
campaign results. The checker/tool binary hashes identify the actual executables;
the producer source tree was dirty while these repair-stage audits ran.

- `canaries-*`: six reduced forged proofs, all rejected.
- `golden-*`: seven stored MRS proofs, all strictly certified.
- `prv-*`: the full mixed 100-proof PRV corpus; 30 certified, 34 rejected,
  36 inconclusive. `prv-initial-*` preserves the earlier same-day replay before
  the conjecture/pedigree repair. No result was silently overwritten.
- `epu-initial-*`: 17 archived EPU proofs at 30 seconds / 2048 MiB with the
  debug checker; 15 certified, one deadline Unknown and one outer timeout.
- `epu-*`: the separately retained 120-second / 2048-MiB replay; 16 certified,
  with `HWV078-1` still unverified at the outer guard. This does not revise the
  earlier archived 17/17 result.
- `epu-release-*`: the repaired release checker replays all 17 archived proof
  bytes as `VerifiedGood` at 120 seconds / 2048 MiB. Its distinct binary hash is
  recorded; the debug timeout observation is not overwritten.

Reports preserve per-proof hashes and loaded problem/include identities.
Compared verdicts are observations about identical artifacts, not search gains.
The [repair report](../../reports/audits/paper-implementation-repairs-2026-11.md)
states the supported conclusions and remaining archival/architecture limits.
