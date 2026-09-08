# CASC Competition Problem Generation & Selection

This document details how the CASC (CADE ATP System Competition) organizers (Geoff Sutcliffe and the CASC panel) select, filter, and transform benchmark problems from the master TPTP (Thousands of Problems for Theorem Provers) library.

The analysis is based on empirical diffs between the master TPTP library (`/home/fr22192/pve/TPTP-v9.3.0`), `crates/mrs-bench/problems/casc-30`, and `crates/mrs-bench/problems/casc-j13`, as well as cross-comparison of the 489 problems shared between CASC-30 and CASC-J13.

---

## 1. Executive Summary: The 5-Stage Pipeline

```
+--------------------------------------------------------------------------+
| 1. Division & Semantic Problem Classification (SPC) Partitioning         |
|    - Maps TPTP problems to divisions based on logic, equality, & status  |
|      (e.g., UEQ: CNF_UNS_RFO_PEQ_UEQ, FEQ: FOF_THM_*_SEQ)                |
+------------------------------------+-------------------------------------+
                                     |
                                     v
+--------------------------------------------------------------------------+
| 2. Difficulty Rating & Anti-Triviality / Anti-Unsolved Filter            |
|    - Restricts problem ratings to the competition sweet spot:            |
|      0.20 < Rating < 1.00                                                |
|    - Excludes trivial problems (solved by all) and unsolved problems     |
+------------------------------------+-------------------------------------+
                                     |
                                     v
+--------------------------------------------------------------------------+
| 3. Bias Mitigation & Diversity Constraints                               |
|    - Enforces domain quotas (no single domain exceeds ~20-25%)           |
|    - Constrains clusters from Documents/VerySimilarProblemsLists         |
+------------------------------------+-------------------------------------+
                                     |
                                     v
+--------------------------------------------------------------------------+
| 4. Seeded Random Sampling                                                |
|    - Draws target problem counts (e.g., 100 FNE, 300 FEQ, 400 UEQ)       |
|      using a public random seed chosen at competition start              |
+------------------------------------+-------------------------------------+
                                     |
                                     v
+--------------------------------------------------------------------------+
| 5. Problem Sanitization & Obfuscation via tptp4X                         |
|    - Invocation: tptp4X -c -t randomize:<seed>                           |
|    - -c: Strips TPTP headers and comments (prevents SZS status cheating) |
|    - -t randomize: Permutes clauses/formulas & flips commutative syms   |
+--------------------------------------------------------------------------+
```

---

## 2. Transformation Tool: `tptp4X -c -t randomize:<seed>`

Comparing the files in `casc-30` and `casc-j13` against the TPTP master library reveals that the competition files are generated using **`tptp4X`** (located at `/home/fr22192/pve/TPTP-v9.3.0/Scripts/tptp4X`).

Specifically:
```bash
tptp4X -c -t randomize:<seed> <problem.p>
```
reproduces the competition files byte-for-byte.

### Header and Comment Stripping (`-c`)
- **Action**: Removes all non-logical lines starting with `%`, including the standard TPTP header (`% File`, `% Status`, `% Rating`, `% SPC`, `% Syntax`).
- **Purpose**: Prevents participating automated theorem provers from simply grepping `% Status: Theorem` or `% Status: Unsatisfiable` from the header.
- **Empirical Prevalence**:
  - `casc-30`: **1,785 / 1,800** standard library problems have no `%` headers (99.2%).
  - `casc-j13`: **1,349 / 1,350** problems have no `%` headers (99.9%).

### Randomization & Obfuscation (`-t randomize:<seed>`)
Inspecting the symbols and C source identifiers in `tptp4X` (`Randomize.c`) identifies the exact transformations applied:
1. **`RandomizeCommutativeFormulae`**:
   - Commutatively swaps arguments of equality: `LHS = RHS` $\leftrightarrow$ `RHS = LHS`.
   - Commutatively swaps disequality: `LHS != RHS` $\leftrightarrow$ `RHS != LHS`.
   - Commutatively reorders binary connectives: `A & B` $\leftrightarrow$ `B & A`, `A | B` $\leftrightarrow$ `B | A`.
2. **`RandomizeListOfAnnotatedFormulae`** / **`RandomizeAnnotatedFormulaeInList`**:
   - Randomly permutes the order of clauses and formulas throughout the problem file.
3. **`RandomizeFormulaeBelowIncludes`**:
   - Keeps `include('Axioms/...')` directives at the very top of the problem file, but randomizes all problem formulas following them.
4. **`srandom(seed)`**:
   - Seeds the pseudo-random generator with a publicly chosen integer (selected by the CASC panel at the start of the competition).

### Axiom Randomization
The axiom files in `crates/mrs-bench/problems/casc-30/Axioms` and `casc-j13/Axioms` are also run through `tptp4X -c -t randomize`:
- For example, in `ALG002-0.ax`, the header is stripped and the axioms (`permute1`, `permute2`, `associativity`) appear in different random orders between CASC-30 and CASC-J13.

---

## 3. Cross-Competition Comparison: `casc-30` vs `casc-j13`

Across both suites:
- **`casc-30`**: 2,901 total problems (1,800 standard TPTP problems + 1,000 SLH + 101 ICU).
- **`casc-j13`**: 1,350 total problems (all 1,350 from TPTP).
- **Common Problems**: Exactly **489 problems** appear in both competitions.

### Division Stability of Shared Problems
All 489 shared problems remained in the exact same division across competitions:
- `UEQ` $\to$ `UEQ`: 232 problems (77.3% of CASC-30 UEQ was retained in CASC-J13).
- `TEQ` $\to$ `TEQ`: 97 problems.
- `FEQ` $\to$ `FEQ`: 64 problems.
- `TNE` $\to$ `TNE`: 62 problems.
- `FNE` $\to$ `FNE`: 34 problems.

### Variation in File Content
Of the 489 shared problems:
- **54 problems are identical**: These are short, single-formula or single-conjecture problems where the randomization space has only 1 permutation (e.g. `include(...)` followed by one clause without commutative operators).
- **435 problems differ**: Every difference is an artifact of re-running `tptp4X -c -t randomize:<seed>` with a different random seed:
  - 23 problems differ solely by formula order permutation.
  - 412 problems differ by formula order plus equality orientation swaps or connective swaps.

### Concrete Proof: `ALG212-10.p`
In `ALG212-10.p`:
- **`casc-30`**: `f(f(x,u,w),f(y,u,w),f(z,u,w)) != f(f(x,y,z),u,w)`
- **`casc-j13`**: `f(f(x,y,z),u,w) != f(f(x,u,w),f(y,u,w),f(z,u,w))`
- Running `tptp4X -c -t randomize` yields the **`casc-30`** output.
- Running `tptp4X -c -t randomize:123` yields the **`casc-j13`** output.

---

## 4. Problem Selection & Difficulty Filtering

### The Rating Sweet Spot ($0.20 < \text{Rating} < 1.00$)
Every problem in TPTP has an empirical difficulty rating between $0.00$ (solved by all provers) and $1.00$ (solved by no prover).

Analyzing `casc-j13` against TPTP v9.3.0 ratings:
- **FEQ (300 problems)**: Min rating 0.22, Max 0.96, Mean 0.62. (0 trivial, 0 unsolved).
- **FNE (100 problems)**: Min rating 0.40, Max 0.80, Mean 0.58. (0 trivial, 0 unsolved).
- **FNN (50 problems)**: Min rating 0.33, Max 0.67, Mean 0.49. (0 trivial, 0 unsolved).
- **FNQ (100 problems)**: Min rating 0.33, Max 0.67, Mean 0.44. (0 trivial, 0 unsolved).
- **TEQ (300 problems)**: Min rating 0.25, Max 0.92, Mean 0.46. (0 trivial, 0 unsolved).
- **TNE (100 problems)**: Min rating 0.22, Max 0.89, Mean 0.45. (0 trivial, 0 unsolved).
- **UEQ (400 problems)**: Min rating 0.22, Max 0.94, Mean 0.58. (0 trivial, 0 unsolved).

**Rule**: Problems with $\text{Rating} \le 0.20$ or $\text{Rating} = 1.00$ are excluded from standard competition divisions.

### Rating Drift Between TPTP Versions
In `casc-30`, 12 FEQ problems currently show `0.00` in TPTP v9.3.0 (e.g., `ALG104+1.p`, `GEO559+1.p`, `GEO169+2.p`, `ALG127+1.p`).
Tracing their rating histories confirms that at the time CASC-30 was assembled (under TPTP v9.1.0 in July 2025):
- `ALG104+1.p`: was **0.82** in v9.1.0.
- `ALG127+1.p`: was **0.55** in v9.1.0.
- `GEO169+2.p`: was **0.52** in v9.1.0.
- `GEO559+1.p`: was **0.39** in v9.1.0.

Their rating dropped to `0.00` only because ATP systems evaluated during the v9.2.0/v9.3.0 cycle solved them. At competition time, **every selected problem met the sweet spot criterion**.

---

## 5. Semantic Problem Classification (SPC) Taxonomy

Problems are assigned to competition divisions strictly based on TPTP SPC metadata:

| Division | Logic & Form | Equality | Status | Canonical SPCs |
|---|---|---|---|---|
| **UEQ** | CNF, Unit clauses | Pure Equality | Unsatisfiable | `CNF_UNS_RFO_PEQ_UEQ` |
| **FEQ** | FOF, General clauses | With Equality | Theorem | `FOF_THM_RFO_SEQ`, `FOF_THM_RFO_PEQ` |
| **FNE** | FOF, General clauses | Non-Equational | Theorem | `FOF_THM_RFO_NEQ` |
| **FNN** | FOF, General clauses | Non-Equational | CounterSat / Sat | `FOF_CSA_RFO_NEQ`, `FOF_SAT_RFO_NEQ` |
| **FNQ** | FOF, General clauses | With Equality | CounterSat / Sat | `FOF_SAT_RFO_SEQ`, `FOF_CSA_RFO_SEQ` |
| **TEQ** | TH0 (Higher-Order) | With Equality | Theorem | `TH0_THM_EQU_NAR` |
| **TNE** | TH0 (Higher-Order) | Non-Equational | Theorem | `TH0_THM_NEQ_NAR` |
| **EPS** | CNF, EPR | Either | Satisfiable | `CNF_SAT_EPR_NEQ`, `CNF_SAT_EPR_EQU_NUE` |
| **EPU** | CNF, EPR | Either | Unsatisfiable | `CNF_UNS_EPR_NEQ_NHN`, `CNF_UNS_EPR_SEQ_NHN` |
| **TFE / TFI** | TF0 (Typed First-Order) | Arithmetic | Theorem | `TF0_THM_EQU_ARI`, `TF0_THM_NEQ_ARI` |
| **TFN** | TF0 (Typed First-Order) | Either | Satisfiable | `TF0_SAT_EQU_NAR`, `TF0_CSA_NEQ_NAR` |

---

## 6. Diversity Control & Bias Prevention

To ensure that competitions reflect general theorem proving ability rather than overfitting to specific benchmark generators:
1. **Very Similar Problems Lists (VSP)**:
   - TPTP maintains explicit lists of clustered, structurally isomorphic, or family-parameterized problems in `Documents/VerySimilarProblemsLists/` (e.g., `SWV_EPRMutex`, `REL_HoefnerCNF`, `GRP_StanovskyCNF`, `LAT_McCuneSet2`).
   - The competition selection caps the maximum number of problems chosen from any single VSP cluster.
2. **Domain Quotas**:
   - The selection enforces an upper bound on representation from any single domain (typically no single domain exceeds 15% to 25% of a division).
   - In `casc-j13` UEQ (400 problems across 24 domains), the highest represented domain is `GRP` with 79 problems (19.7%), followed by `LAT` with 78 (19.5%), `REL` with 58 (14.5%), and `LCL` with 44 (11.0%).

---

## 7. Special Non-TPTP Divisions (`SLH` and `ICU`)

In `casc-30`, 1,101 problems do not exist in the standard TPTP `Problems/` directory:
- **`SLH` (1,000 problems)**: Sledgehammer / Archive of Formal Proofs (AFP) problems exported directly from Isabelle/HOL. Characterized by large background theories (typically 5,000–15,000 lines per problem).
- **`ICU` (101 problems)**: Interactive Computer Theorem Proving problems (e.g., MPTP / Mizar and Coq exports such as `CPP002+1.p`). Characterized by inlined background axiom sets reaching up to 500,000 lines.

These divisions test large-theory reasoning and axiom-selection filters rather than pure given-clause saturation efficiency. In CASC-J13, both `SLH` and `ICU` were omitted, returning to 100% standard TPTP library problems.

---

## 8. Architectural Implications for `mrs`

Understanding this generation process provides concrete guidance for `mrs` development:

1. **Ordering Invariance**:
   - Because `tptp4X -c -t randomize` randomly shuffles input formulas, `mrs` clause selection heuristics (such as AgeWeight, FIFO, or SOS) and symbol precedence generation must not assume any canonical input order.
2. **Equality Symmetry Invariance**:
   - Because equality orientations (`LHS = RHS` vs `RHS = LHS`) are randomly swapped by `RandomizeCommutativeFormulae`, superposition simplification and rewriting must reliably orient equations via term orderings (KBO/LPO) rather than relying on input orientation.
3. **Canary Verification & Axiom Drift**:
   - Because benchmark problems use `include('Axioms/XYZ.ax')`, and competition suites ship randomized/pre-processed axiom files in `Axioms/`, running benchmarks without setting `$TPTP` to the local competition suite can lead to axiom drift. The canary suite and `check_canary_drift.sh` monitor processed clause counts to catch discrepancies early.
4. **No Cheating on Headers**:
   - Prover components (or ML guidance pipelines) must never inspect file comments or headers, as all headers are stripped in official competition problems.
