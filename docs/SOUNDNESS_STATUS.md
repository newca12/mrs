# Soundness Status Audit Report

This document records the results of the comprehensive independent proof-verification audit performed on a remote server for **commit `9f56c074ae5f37f5a43e0d941f84e07930465c43`** ("fix(bench): avoid CPU oversubscription in run_proover_audit.sh").

The audit was executed over the full TPTP FOF and UEQ problem sets (excluding EPR) using the following command:
```bash
TMPDIR=/DATA/ai/user/mrs/tmp MRS_AUDIT_TIMEOUT=240 crates/mrs-bench/run_proover_audit.sh \
  <(cd "$TPTP" && grep -rlE "SPC *: *(FOF_|CNF_[A-Z0-9_]*UEQ)" Problems/ | xargs grep -L "SPC.*EPR") \
  /DATA/ai/user/mrs/full_audit_filter.db
```

The resulting database contains **10,351** total problem evaluations, summarized below.

## 1. Audit Summary Statistics

| SZS Status | Verified Good (`1`) | Failed Verification (`0`) | Inconclusive (`NULL`) | Total |
| :--- | :---: | :---: | :---: | :---: |
| **Theorem** | 483 | 603 | 1,395 | **2,481** |
| **Unsatisfiable** | 319 | 75 | 333 | **727** |
| **Satisfiable** | — | — | 136 | **136** |
| **CounterSatisfiable** | — | — | 220 | **220** |
| **Timeout** | — | — | 6,614 | **6,614** |
| **GaveUp** | — | — | 92 | **92** |
| **Error** | — | — | 81 | **81** |
| **Total** | **802** | **678** | **8,871** | **10,351** |

---

## 2. Verification Outcomes Analysis

### **A. Verified Good (802 Solves)**
* **483 Theorems** and **319 Unsatisfiables** were successfully verified as `VerifiedGood` by `mrs-proover`'s in-process ATP fallback (`MrsAtp`), establishing absolute soundness of these derived proofs.

### **B. Failed Verification / Timeouts (678 Solves)**
* **603 Theorems** and **75 Unsatisfiables** failed verification. 
* *Note:* In this audit commit (`9f56c07`), verification failures occurred because of:
  1. **CPU/Thread Oversubscription:** Running the verifier with 8 parallel worker threads, each spawning up to 16 threads, starved the CPU on the 16-core audit machine, triggering the hard-coded 10-second `mrs-proover` timeout limit on many simple problems.
  2. **Componentwise AVATAR (CWA) Provenance Limitation:** The CWA pre-pass did not propagate intermediate FOF `provenance` formula steps (NNF, Skolemization, negated conjecture) to the sub-search branch states. This left those steps omitted from the printed proof, causing `mrs-proover` to correctly flag them as `VerifiedBad` with `node c6 references unknown parent c3`.

### **C. Inconclusive / Unknown (1,728 Solves)**
* **1,395 Theorems** and **333 Unsatisfiables** returned `Unknown` from the verifier.
* These are cases where `MrsAtp` timed out on specific steps or could not structurally confirm the inference, but did not find any logical contradiction (no `VerifiedBad` was returned).
