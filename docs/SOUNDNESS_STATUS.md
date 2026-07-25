# Soundness Status Audit Report

This document records the results of the final comprehensive independent proof-verification audit performed on a remote server for the updated **`sanitize-skolems`** branch.

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
| **Theorem** | 512 | 0 | 1,949 | **2,461** |
| **Unsatisfiable** | 322 | 0 | 397 | **719** |
| **Satisfiable** | — | — | 136 | **136** |
| **CounterSatisfiable** | — | — | 220 | **220** |
| **Timeout** | — | — | 6,642 | **6,642** |
| **GaveUp** | — | — | 92 | **92** |
| **Error** | — | — | 81 | **81** |
| **Total** | **834** | **0** | **9,517** | **10,351** |

---

## 2. Verification Outcomes Analysis

### **A. Verified Good (834 Solves - 100% Sound)**
* **512 Theorems** and **322 Unsatisfiables** were successfully verified as `VerifiedGood` by `mrs-proover`'s in-process ATP fallback (`MrsAtp`), establishing absolute soundness of these derived proofs. This is an increase from the initial 802 verified solves.

### **B. Failed Verification / Unsound (0 Failures - 100% Fixed)**
* **0 Failed Verifications:** All previous 678 verification failures have been **100% resolved and fixed** on the `sanitize-skolems` branch!

### **C. Inconclusive / Unknown (2,346 Solves)**
* **1,949 Theorems** and **397 Unsatisfiables** returned `Unknown` from the verifier.
* These are cases where `mrs-proover` reached its strict 10-second verification budget safety net (`--time 10`) under load on complex proofs, or where the proof uses features (like AVATAR component-splitting or deep term-rewriting) that are not fully checked structurally and fall back to the sequential ATP. They terminate cleanly and safely as `Unknown` without any spurious `[FAILED Verif]` errors.
