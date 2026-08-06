# MRS CASC Divisions: Schedules and Structural Mapping

This document describes how the MRS theorem prover maps CASC competition
divisions to named strategy schedules, and how per-division portfolios are
selected and tuned.

---

## 1. Division → Schedule Mapping

The benchmark harness (`crates/mrs-bench/systems/mrs/invoke.sh`) detects the
CASC division from the problem file path and selects the matching schedule:

| CASC Division | Named Schedule | Notes |
|:---|:---|:---|
| **FEQ** (FOF with Equality) | `casc_feq` | Full superposition + demodulation |
| **FNE** (FOF No Equality) | `casc_fne` | Pure resolution/factoring; no paramodulation |
| **UEQ** (Unit Equality) | `casc_ueq` | Unit clauses only; no AVATAR |
| **EPU** (EPR Unsatisfiable) | `casc_epu` | s4-first (greedy optimal: s4→s6) |
| **EPS** (EPR Satisfiable) | `casc_eps` | s1-first (greedy optimal: s1→s2→s5) |
| **ICU** (Intensional Unit Equality) | `casc_icu` | s12-first |
| other / fallback | `casc` | Generic 15-strategy CASC portfolio |

---

## 2. Data-Driven Portfolio Construction

Each division schedule is constructed by `build_casc_schedule` in
`crates/mrs-search/src/strategy/named.rs`.  The function takes:
- The total time budget and number of workers
- A **priority order array** listing strategy indices (1-indexed, 1–15) in
  the order they should be allocated to parallel worker slots

This replaces the previous loop-generated schedules (which varied parameters
via modular arithmetic) with **data-driven portfolios** derived from greedy
set-cover analysis over CASC-30 benchmark results.

### How to regenerate (after a new TPTP release or benchmark run):

```bash
# Step 1: run all 15 strategies solo across all divisions
export TPTP=/path/to/TPTP-v9.x.x
./crates/mrs-bench/run_strategy_sweep.sh \
    --divisions fne,feq,ueq,eps,epu,icu --time 30 --jobs 4 \
    --output results/sweep-$(date +%Y%m%d)

# Step 2: run the sweep optimizer
./crates/mrs-bench/run_all_greedy_sweeps.sh results/sweep-*/run.csv \
    > greedy_all.res

# Step 3: read the 8-core portfolios per division and update named.rs
```

---

## 3. Current Data-Driven Priority Orders (CASC-30)

Based on a 30 s per-strategy sweep over the CASC-30 problem set:

| Division | 8-core priority order | Unique coverage at 8 cores |
|:---------|:----------------------|:---------------------------|
| **FNE** | s11, s4, s12, s1, s6, s8, s2, s3 | 35 problems solved at 6 cores |
| **FEQ** | s11, s12, s1, s6, s10, s8, s14, s4 | 90 problems solved at 8 cores |
| **UEQ** | s11, s4, s2, s8, s14, s1, s15, s3 | 68/68 (100%) at 7 cores |
| **EPU** | s1, s6, s14, s11, s4, s2, s3, s7 | 16/16 (100%) at 2 cores |
| **EPS** | s2, s3, s1, s8, s11, s12, s9, s14 | 38/38 (100%) at 1 core |
| **ICU** | s12, s1, s2, s3, s4, s5, s6, s7 | 0 problems solved |

Beyond the minimum coverage point, extra slots cycle through remaining
strategies so no core is idle.

---

## 4. The `categorize_tptp` Utility

For custom problem sets, the `categorize_tptp` binary (in `mrs-bench`) splits a
TPTP installation into per-division problem lists:

```bash
cargo run --release -p mrs-bench --bin categorize_tptp <TPTP_DIR> ./casc_problem_lists
```

**Categorization rules** (from `crates/mrs-bench/src/bin/categorize_tptp.rs`):

| Category | Rule |
|:---------|:-----|
| EPR | No function symbols of arity ≥ 1 (only constants and variables) |
| UEQ | Strictly CNF; every clause is a unit equality/inequality literal |
| FNE | FOF/CNF with no equality literals (and has functions of arity ≥ 1) |
| FEQ | FOF/CNF with at least one equality literal (and has functions) |
| Other | TFF, THF, or other typed/higher-order; ignored |

---

## 5. Performance Safety Guards

Three hard limits protect against algorithmic blowups on large-clause problems
(e.g., the `HWV` Software Verification domain with 200+ literal clauses):

### Subsumption step limit (5 000 steps)

**Location:** `crates/mrs-calculus/src/subsumption.rs` — `subsumes_id`  
**Problem:** Subsumption checking is NP-complete in clause width.  On 200-literal
clauses naive backtracking can execute billions of operations on a single call,
bypassing the wall-clock time limit.  
**Fix:** The backtracking counter is incremented once per recursive call.  Once
it exceeds 5 000 the check immediately returns `false` (not subsumed).

### Condensation clause-width guard (> 50 literals → skip)

**Location:** `crates/mrs-calculus/src/subsumption.rs` — `condense_id`  
**Problem:** Condensation is O(N³) in clause width.  For a 200-literal clause
this requires ~40 000 expensive unification checks.  
**Fix:** Condensation is skipped entirely for clauses with more than 50 literals.
Wide clauses are extremely unlikely to be condensable in practice.

### Demodulation pass limit (100 passes)

**Location:** `crates/mrs-calculus/src/demodulation.rs` — `demodulate_id`  
**Problem:** Equational problems can generate cyclic rewrite rules (a→b and b→a),
causing the rewriter to loop indefinitely.  
**Fix:** Each call to `demodulate_id` is capped at 100 rewriting passes.  If the
limit is reached, the partially rewritten clause is returned as-is.

---

## 6. Complete CASC Division Support & Scope Map

The table below outlines all possible CASC divisions, including those that are active, legacy, or out of scope for MRS:

| CASC Division | Category / Description | MRS Support Status | Technical Reason / Details |
|:---|:---|:---|:---|
| **FOF** | First-Order Formulas (Classical) | 🟢 **Active Competitor** | Core of first-order ATP. Split internally into **FNE** (No Equality) and **FEQ** (With Equality). |
| **FNE** | FOF No Equality | 🟢 **Active Competitor** | Handled by `casc_fne` schedule with pure resolution/factoring (paramodulation/demodulation disabled). |
| **FEQ** | FOF with Equality | 🟢 **Active Competitor** | Handled by `casc_feq` schedule using full superposition + demodulation. |
| **UEQ** | Unit Equality CNF | 🟢 **Active Competitor** | Pure equational logic handled by `casc_ueq` using our sound AC-indexing given-clause loop. |
| **EPR / EPU / EPS** | Effectively Propositional | 🟡 **Legacy / Local Benchmarks** | Bernays-Schönfinkel class (no functions of arity $\ge 1$). Supported via CaDiCaL SAT-splitting, but no EPR division at CASC-J13. |
| **ICU** | Intuitionistic First-order logic | 🟡 **Legacy / Local Benchmarks** | Experimental support and fixtures remain, but the division does not exist at CASC-J13. |
| **THF** | Typed Higher-order Form | ❌ **Out of Scope** | Requires higher-order logic (lambda-calculus, type theory, currying). MRS is strictly a classical *First-Order* solver. |
| **TNE** | THF No Equality | ❌ **Out of Scope** | Higher-order logic (THF) with no equality. |
| **TEQ** | THF with Equality | ❌ **Out of Scope** | Higher-order logic (THF) containing equality. |
| **TFF** | Typed First-order Form | ❌ **Out of Scope** | Monomorphic/polymorphic typed first-order formulas; parsed but ignored by default. |
| **TFA** | Typed First-order with Arithmetic | ❌ **Out of Scope** | Requires SMT-style arithmetic solvers to handle numeric constraints. MRS does not support numeric theories. |
| **FNT** | First-order Non-theorems | ❌ **Out of Scope** | Requires finding counter-models (finite model generators). MRS is purely refutation-based. |
| **FNN** | FNT No Equality | ❌ **Out of Scope** | First-order non-theorems with no equality. |
| **FNQ** | FNT with Equality | ❌ **Out of Scope** | First-order non-theorems containing equality. |
| **LTB** | Large Theory Batch | ❌ **Out of Scope** | Large-scale problem sets requiring specialized axiom selection/filtering over thousands of formulas. |
| **SLH** | Sledgehammer (Isabelle) | ❌ **Out of Scope** | Obligations translated from interactive HOL proof assistants requiring highly specialized translation parsing. |
| **PRV** | Proof Verification | 🟢 **Active Competitor** | MRS competes in this division via the `mrs-proover` sub-crate (ProoVer 2026 entry), which functions as an independent TSTP proof verifier. |
