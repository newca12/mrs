# MRS CASC Divisions & Mappings

This document outlines how the MRS theorem prover translates semantic CASC competition divisions into structural schedules, how the CASC problem list generator is implemented, and the engineering safety limits designed to keep these divisions robust.

---

## 1. Overview: Semantic vs. Structural Divisions

CASC divisions are **semantic** (determined by the competition category, such as whether a problem is satisfiable or unsatisfiable), whereas the MRS engine operates on **structural** schedules (tailored to the mathematical shape of the input clauses). 

To maximize portfolio performance, MRS uses a custom generator utility to partition any TPTP release into **four structural categories**, which are mapped to their corresponding CASC divisions.

---

## 2. The `categorize_tptp` Generator Utility

The list generator is implemented as a fast Rust binary in the `mrs-bench` workspace:
`crates/mrs-bench/src/bin/categorize_tptp.rs`

### How to Run:
```bash
cargo run --release -p mrs-bench --bin categorize_tptp <TPTP_DIR> ./casc_problem_lists
```

### Categorization Logic:
The tool recursively traverses `<TPTP_DIR>`, parses each `.p` file with the custom `mrs_tptp` parser, and analyzes its AST using structural rules:

* **EPR (Effectively Propositional):** 
  * *Rule:* Contains **no function symbols of arity $\ge 1$** (only constants and variables).
* **UEQ (Unit Equality):** 
  * *Rule:* Strictly CNF, and every clause is a **unit clause** containing a **single positive or negative equality literal**.
* **FNE (First-Order No Equality):** 
  * *Rule:* First-order logic containing **no equality literals** (and has functions of arity $\ge 1$).
* **FEQ (First-Order with Equality):** 
  * *Rule:* General first-order logic containing **at least one equality literal** (and has functions of arity $\ge 1$).
* **Other:** 
  * *Rule:* Typed First-Order (TFF, TFI, TFE) or Higher-Order (THF) formulas. These are ignored by the generator.

---

## 3. CASC Division Mapping

The benchmark runner `casc.sh` automatically maps official CASC divisions to the corresponding structural list and parallel strategy schedule:

| CASC Division | Category List | Named Schedule | Structural Features |
|:---|:---|:---|:---|
| **`EPU`** (EPR Unsatisfiable) | `epr.list` | **`casc_epr`** | No functions of arity $\ge 1$. Extreme clause sizes. |
| **`EPS`** (EPR Satisfiable) | `epr.list` | **`casc_epr`** | No functions of arity $\ge 1$. Model-finding targeted. |
| **`ICU`** (Unit Equational) | `ueq.list` | **`casc_ueq`** | Strictly unit clauses, strictly equality. No AVATAR. |
| **`FNE`** (First-Order No Eq) | `fne.list` | **`casc_fne`** | Pure resolution/factoring. No paramodulation checks. |
| **`FEQ`** (First-Order with Eq) | `feq.list` | **`casc_feq`** | General first-order with superposition and demodulation. |

---

## 4. Key Performance Safety Guards

To prevent N-hard or exponential algorithmic blowups on hard problems (particularly in the `EPR` and `FEQ` divisions), MRS implements three essential performance safety guards:

### A. Subsumption Backtracking Steps-Limit (`subsumes_id`)
* **The Issue:** Subsumption of large clauses is NP-complete. On Software Verification problems (e.g., the `HWV` domain) with clauses of up to 200+ literals, recursive backtracking matching can loop for billions of operations on a single core, bypassing time-limit checks.
* **The Fix:** Caps recursive matching at **`5000` steps**. If exceeded, the check fails fast, allowing the Given Clause loop to continue and respect the wall-clock limit.

### B. Condensation Clause-Size Guard (`condense_id`)
* **The Issue:** Condensation runs nested loops over all literals in a clause and executes subsumption checks on each pair. For a clause of size $N$, this scales as $O(N^3)$. On a clause of size 200, this requires over $40,000$ expensive checks!
* **The Fix:** Skip condensation entirely for clauses with **$> 50$ literals**, completely avoiding the $O(N^3)$ processing bottleneck.

### C. Demodulation Pass-Limit (`demodulate_id`)
* **The Issue:** Equational spaces often generate cyclic or symmetric rewrite rules (e.g., $a \to b$ and $b \to a$). This causes the term rewriter to loop infinitely back-and-forth, hanging the thread.
* **The Fix:** Caps the rewriter at **`100` passes** per literal, acting as a hard safety-valve to guarantee termination.
