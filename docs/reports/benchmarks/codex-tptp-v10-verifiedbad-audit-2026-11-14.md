# Codex TPTP v10.0.0 Sweep: VerifiedBad Audit and Re-certification (2026-11-14)

## Context

A full sweep of TPTP v10.0.0 (27,053 problems) was run on a remote server (16 physical / 32 logical CPUs, 128G RAM; 8 workers/job, 2 jobs) using:
```bash
DB="codex-tptp-v10.0.0.db"
TIMEOUT=30
REV="$(git rev-parse --short HEAD)" # ca16b8f
SYSTEM="mrs-0.2.3-g${REV}"
MRS_INVOKE="$PWD/crates/mrs-bench/systems/mrs/invoke.sh"

TPTP="$TPTP_ROOT" target/release/mrs-codex "$TPTP_ROOT/Problems" \
  --db "$DB" \
  --corpus tptp-v10.0.0 \
  --system "$SYSTEM" \
  --hardware "remote: 16 physical / 32 logical CPUs, 128G RAM; 8 workers/job, 2 jobs" \
  --timeout "$TIMEOUT" \
  --jobs 2 \
  --cmd "env MRS_WORKERS=8 MRS_HARDWARE=adaptive MRS_MAX_MEMORY_MB=48000 MRS_SHARED_POOL_INTERVAL=0 $MRS_INVOKE {file} {timeout}" \
  --params "TPTP v10; generic casc schedule; adaptive; 8 workers; 48000 MB per MRS process; jobs=2" \
  --verify-mode kernel > codex-tptp-v10.0.0.out 2> codex-tptp-v10.0.0.err
```

During the sweep, `/home/hack/CODEX/codex_VerifiedBad` initially recorded 31 problems with status `VerifiedBad` (up to problem index `[19004/27053]`). Across the complete 27,053-problem database, an additional 11 problems exhibited the same outcome, giving **42 `VerifiedBad` problems total**.

## Root Causes on Commit `ca16b8f`

All 42 problems produced refutations via EPR ground resolution (`epr_attempted=true`, `epr_extraction=lrat_lift`). Two distinct defects caused `mrs-proover --strict` rejection:

1. **EPR Ground Refutation Lifting (`lift_bfs_refutation`):**
   The propositional SAT abstraction canonicalized or dropped literals (removing false reflexive disequalities `c != c`, sorting equality orientation, and deduplicating literals). In the lifted first-order TSTP proof, unsimplified ground clauses were cited directly as parents of resolution inferences. The strict proof kernel rejected these resolution steps because the resolvent did not match the unnormalized ground premises.
2. **Skolem Symbol Freshness Tracking (`mrs-proof-kernel`):**
   In formulas with multiple existential quantifiers across separate branches (e.g. `KRS168+1`), constants used as arbitrary instantiation terms earlier in the proof DAG were tracked in `known_function_symbols`. A subsequent independent Skolemization introducing that same symbol was falsely rejected as non-fresh.

Both issues were resolved in commit **`ae5af99`** (*"fix(proof): repair EPR lifting and Skolem freshness"*):
- Introduced `normalize_ground_input_for_lift` in `crates/mrs-search/src/epr_ground.rs` to insert explicit `equality_resolution`, `symmetry`, and `condensation` steps.
- Scoped Skolem symbol freshness in `crates/mrs-proof-kernel/src/lib.rs` to problem-declared symbols (`declared_function_symbols`).

## Re-Certification on HEAD

All 42 problems were re-executed against HEAD using the exact sweep configuration and verified with `mrs-proover --strict` with `TPTP` set to `/home/hack/TPTP-v10.0.0`.

**Result:** 42 / 42 (100%) refutations strictly verify as `VerifiedGood`. Zero `VerifiedBad` refutations remain.

Database `/home/hack/CODEX/codex-tptp-v10.0.0.db` has been patched accordingly (backup retained at `codex-tptp-v10.0.0.db.bak`), updating `kernel_validated = 'VerifiedGood'` and `proover_validated = 'VerifiedGood'`, increasing total certified proofs from 4,892 to 4,934.

| Problem | Division | Status | Solve Time (s) | Kernel Time (s) | Kernel Verdict |
|:---|:---|:---|---:|---:|:---|
| `PHI/PHI011+1.p` | EPU | Theorem | 0.036 | 0.010 | **VerifiedGood** |
| `PUZ/PUZ056-2.005.p` | FNE | Unsatisfiable | 0.070 | 0.013 | **VerifiedGood** |
| `SET/SET043+1.p` | FNE | Theorem | 0.051 | 0.011 | **VerifiedGood** |
| `SET/SET043-5.p` | FNE | Unsatisfiable | 0.044 | 0.012 | **VerifiedGood** |
| `GRP/GRP124-1.004.p` | FNE | Unsatisfiable | 0.107 | 0.011 | **VerifiedGood** |
| `GRP/GRP124-3.004.p` | FNE | Unsatisfiable | 0.278 | 0.012 | **VerifiedGood** |
| `GRP/GRP124-4.004.p` | FNE | Unsatisfiable | 0.099 | 0.012 | **VerifiedGood** |
| `GRP/GRP128-1.003.p` | FNE | Unsatisfiable | 0.047 | 0.015 | **VerifiedGood** |
| `GRP/GRP128-4.003.p` | FNE | Unsatisfiable | 0.053 | 0.012 | **VerifiedGood** |
| `GRP/GRP129-1.003.p` | FNE | Unsatisfiable | 0.044 | 0.014 | **VerifiedGood** |
| `GRP/GRP129-4.004.p` | FNE | Unsatisfiable | 0.048 | 0.013 | **VerifiedGood** |
| `GRP/GRP130-1.003.p` | FNE | Unsatisfiable | 0.051 | 0.011 | **VerifiedGood** |
| `GRP/GRP130-2.003.p` | FNE | Unsatisfiable | 0.055 | 0.012 | **VerifiedGood** |
| `GRP/GRP130-3.003.p` | FNE | Unsatisfiable | 0.060 | 0.012 | **VerifiedGood** |
| `GRP/GRP130-4.003.p` | FNE | Unsatisfiable | 0.054 | 0.011 | **VerifiedGood** |
| `GRP/GRP131-1.002.p` | FNE | Unsatisfiable | 0.040 | 0.012 | **VerifiedGood** |
| `GRP/GRP131-2.002.p` | FNE | Unsatisfiable | 0.048 | 0.010 | **VerifiedGood** |
| `GRP/GRP132-1.002.p` | FNE | Unsatisfiable | 0.051 | 0.013 | **VerifiedGood** |
| `GRP/GRP132-2.002.p` | FNE | Unsatisfiable | 0.048 | 0.010 | **VerifiedGood** |
| `GRP/GRP133-1.003.p` | FNE | Unsatisfiable | 0.043 | 0.014 | **VerifiedGood** |
| `GRP/GRP133-2.003.p` | FNE | Unsatisfiable | 0.040 | 0.012 | **VerifiedGood** |
| `GRP/GRP134-1.003.p` | FNE | Unsatisfiable | 0.040 | 0.012 | **VerifiedGood** |
| `GRP/GRP134-2.003.p` | FNE | Unsatisfiable | 0.041 | 0.012 | **VerifiedGood** |
| `GRP/GRP135-1.002.p` | FNE | Unsatisfiable | 0.038 | 0.011 | **VerifiedGood** |
| `GRP/GRP135-2.002.p` | FNE | Unsatisfiable | 0.038 | 0.010 | **VerifiedGood** |
| `SWB/SWB015+2.p` | EPU | Theorem | 0.051 | 0.010 | **VerifiedGood** |
| `HWV/HWV069-1.p` | EPU | Unsatisfiable | 4.350 | 0.405 | **VerifiedGood** |
| `SWV/SWV001-1.p` | FNE | Unsatisfiable | 0.132 | 0.010 | **VerifiedGood** |
| `KRS/KRS168+1.p` | FNE | Theorem | 0.048 | 0.011 | **VerifiedGood** |
| `KRS/KRS174+1.p` | EPU | Theorem | 0.052 | 0.013 | **VerifiedGood** |
| `KRS/KRS175+1.p` | EPU | Theorem | 0.037 | 0.012 | **VerifiedGood** |
| `SYN/SYN009-1.p` | FNE | Unsatisfiable | 0.054 | 0.010 | **VerifiedGood** |
| `SYN/SYN009-2.p` | FNE | Unsatisfiable | 0.053 | 0.011 | **VerifiedGood** |
| `SYN/SYN009-3.p` | FNE | Unsatisfiable | 0.049 | 0.009 | **VerifiedGood** |
| `SYN/SYN009-4.p` | FNE | Unsatisfiable | 0.049 | 0.012 | **VerifiedGood** |
| `SYN/SYN412+1.p` | FNE | Theorem | 0.036 | 0.010 | **VerifiedGood** |
| `SYN/SYN445-1.p` | FNE | Unsatisfiable | 3.306 | 0.025 | **VerifiedGood** |
| `SYN/SYN957+1.p` | FNE | Theorem | 0.037 | 0.010 | **VerifiedGood** |
| `LCL/LCL684+1.001.p` | FNE | Theorem | 0.051 | 0.010 | **VerifiedGood** |
| `MGT/MGT052+1.p` | EPU | Theorem | 0.063 | 0.012 | **VerifiedGood** |
| `MGT/MGT052-1.p` | EPU | Unsatisfiable | 0.046 | 0.010 | **VerifiedGood** |
| `MSC/MSC008-2.002.p` | FNE | Unsatisfiable | 0.047 | 0.013 | **VerifiedGood** |
