# TODO: ProoVer 2026 Competition Roadmap

This document tracks what remains to be implemented in `mrs-proover` to maximise the ProoVer 2026 score. Items are ordered by expected scoring impact.

## Scoring Recap

| Outcome | Points |
|---------|--------|
| Correctly identify evil proof (`FailedVerified`) | **+2** |
| Correctly identify good proof (`Verified`) | **+1** |
| Give up / timeout (`NotVerified`) | 0 |
| Falsely reject a good proof | −1 |
| Falsely verify an evil proof (`Unsound`) | **−10 (fatal)** |

The scoring is highly asymmetric. A single `−10` requires 10 correct `+1` verifications just to recover.

---

## Already Implemented (no longer blocking)

| Item | Commit |
|------|--------|
| `introduced(definition)` formula-body validation (definition laundering fix) | `202aae96` |
| Evil-proof test suite (9 exploit cases) | `202aae96` |
| `vampire_skolemisation.rs`: `Unknown` → `Unsound` on arity drops and shape violations | `c722f998` |
| Propositional SAT fast-path for AVATAR/rat/sat_conversion steps | (pre-existing) |
| Structural definition-folding via unfold + alpha-equivalence | (pre-existing) |

---

## Remaining Work (ordered by scoring impact)

### 1. Free Variable Skolemization Exploit — **prevent −10**
**File:** `crates/mrs-proover/src/checks/introduced_definition.rs`

**The Problem:** The `try_skolem_axiom` check ensures all explicit universal variables are present in the Skolem term, but it *fails to check for free variables*. In TPTP, free variables are implicitly universally quantified. This allows an attacker to introduce a tautological Skolem axiom with a free variable (e.g., `(? [Y]: Y = Z) => sk = Z`), which unsoundly collapses the domain and derives `$false`. This exploit currently bypasses the verifier.

**Implementation:**
- Reject any `introduced(definition)` Skolem axiom that contains free variables. Ensure all variables in the formula are explicitly bound by quantifiers.

### 2. Accept CNF Steps in Proof DAG — **prevent −1 & recover +1s**
**File:** `crates/mrs-proover/src/dag.rs`

**The Problem:** The DAG builder strictly rejects any node that is not `fof(...)` with an `UnsupportedDialect` error. Modern ATPs (like E and Vampire) output their actual refutation steps using `cnf(...)` clauses. Because `mrs-proover` drops these nodes, it fails to find the `$false` root, causing a structural failure (`proof does not derive $false`) and falsely rejecting valid proofs.

**Implementation:**
- Update `dag.rs` to accept `cnf(...)` annotated formulas.
- Convert them internally to FOF during parsing/DAG construction, or support them natively in the verification loop.

### 3. Parse-ability for CASC Hardening Dataset
**Goal:** Ingest the CASC dataset to harden `mrs-proover` against panics, OOMs, and bugs.

While the ProoVer competition uses generic TPTP rule formats, we still want to massively test `mrs-proover` on the CASC solutions dataset (which is full of E and Vampire specific outputs) to harden the system's memory, parsing, and DAG construction. 
To do this efficiently:
- We need basic parsing for E and Vampire structural steps.
- **However:** We do *not* need to harden these specific parsers to detect exploits (i.e. returning `Unsound` instead of `Unknown`). If an E or Vampire step looks slightly malformed, we can safely just return `Unknown`. The competition's "evil proofs" will use generic formats, so our exploit-detection focus should remain 100% on the generic TPTP formats.
- We should still implement AC-Equivalence or clause-reordering checks if they prevent `Unknown` fallbacks on valid CASC proofs, purely to speed up the testing pipeline and avoid launching the external ATP fallback on every step.

### 4. Fix `test_tptp_solutions.sh` HTML Stripping Bug — **recover +1s**
**File:** `crates/mrs-bench/test_tptp_solutions.sh`

**The Problem:** The benchmark script downloads TPTP problems and solutions from the web and uses `sed -e 's/<[^>]*>//g'` to strip HTML tags. Unfortunately, this regex accidentally deletes standard TPTP logical operators like `<=>` (iff) and `<=` (reverse implication), corrupting the files before they even reach the verifier and causing `mrs-tptp` to throw syntax errors.

**Implementation:**
- Replace the overly aggressive `sed` command with a targeted one that only removes anchor tags (e.g., `sed -E -e 's/<a [^>]+>//g' -e 's/<\/a>//g'`).

### 5. Recursive / Cyclic Definition Chain Detection — **prevent −10**
**File:** `crates/mrs-proover/src/checks/introduced_definition.rs`

**The Problem:** The current `is_naming_clause` check ensures each `introduced(definition)` step introduces a *fresh* predicate symbol whose body does not contain a contradiction in isolation. However, it does not check for cycles across *multiple* definitions. An adversary could:
1. Introduce `p(X) ⟺ ¬q(X)` (fresh `p`, valid).
2. Introduce `q(X) ⟺ ¬p(X)` (fresh `q`, valid in isolation).
3. Derive `p(a)` from `q(a)` and step 1, and `q(a)` from `p(a)` and step 2 — contradiction without any genuine inference.

**Implementation:**
- After collecting all `introduced(definition)` steps, build a dependency graph: `p → q` if the definition of `p` mentions `q`.
- Run a topological sort / cycle detection (`petgraph::algo::toposort` or hand-written DFS).
- If a cycle is found, return `StepOutcome::Unsound` for all definitions involved.
- This check is O(N²) in the number of definitions but definitions are rare in practice.

### 7. Stronger Structural Coverage → More `+2` Points
**Files:** Various `crates/mrs-proover/src/checks/`

Several check modules still return `StepOutcome::Unknown` for shapes they do not recognise, scoring 0 when the proof step is actually malformed. The philosophy should be:
- `Unknown` → "I cannot parse or understand this step at all."
- `Unsound` → "I understand the step and it violates the rules."

Specific targets to harden (from the evil-proofs analysis):
- `definition_folding.rs`: The recursive body check (`rejects_recursive_definition`) works but only catches one level. Multi-step recursive unfolding through chains of `definition_folding` steps should also trigger `Unsound`.
- `trivial.rs`: The `weakening_not_accepted_as_equivalence` test passes, but weakenings via conjunction-reordering (`A & B ⊢ B & A`) should be explicitly ruled as `Unknown`, not accidentally accepted via the AC-reorder path.

### 8. Performance: Avoid Re-parsing the Problem File Per Step
**File:** `crates/mrs-proover/src/verify.rs`

The current ATP-ladder fallback re-invokes an external ATP subprocess for every unverified step. On a 100-step proof where 30 steps are `resolution` (not structurally verifiable), this means 30 separate `eprover` process launches, each taking 0.5–2s. On a 30-second wall-clock limit, this easily times out.

**Implementation:**
- Batch all ATP queries for a single proof into one subprocess call using the `-m` flag or batch-query format.
- Alternatively, use an in-process ATP library (the `mrs` binary itself, compiled with `--features proover`) to avoid subprocess overhead entirely.

---

## ProoVer Competition Checklist

| Task | Priority | Status |
|------|----------|--------|
| Free Variable Skolemization block | Critical | ✅ Done |
| Accept CNF Steps in Proof DAG | High | ✅ Done |
| Definition laundering blocked | High | ✅ Done |
| Fix `test_tptp_solutions.sh` HTML Stripping Bug | High | ✅ Done |
| Cyclic/recursive definition chain detection | High | ✅ Done |
| Basic E/Vampire structural parsing for CASC dataset hardening | Medium | ❌ TODO |
| Batch ATP subprocess calls | Medium | ❌ TODO |
| Broader Unsound coverage in generic structural checks | Low–Medium | ❌ TODO |
| Benchmark against Nörgler | After fixes | ❌ TODO |
