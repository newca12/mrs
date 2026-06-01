# Evil Proofs

This repository contains a suite of "evil proofs" designed to test the robustness of TPTP/TSTP proof verifiers in the ProoVer competition. 
These proofs are logically unsound but are crafted to bypass common implementation flaws in proof verifiers.

## Categories

1.  **trivial_rule_trust**: Exploits unconditionally trusted rules (like `fof_simplification`) to derive `$false` out of thin air.
2.  **disconnected_root**: Derives `$false` without actually using the conjecture, but claims to be a refutation.
3.  **skolem_injection**: Exploits structural Skolem checks by introducing mathematically fresh but structurally malformed axioms (e.g. `? [X]: $true => $false & bad_sym(sK)`).
4.  **cyclic_dag**: Uses circular reasoning (Step A derives Step B, Step B derives Step A). Tests for cycle-detection bypass.
5.  **occurs_check**: Attempts to unify infinite terms (e.g. `p(X, f(X))` with `~p(Y, Y)`). Targets naive unification algorithms.
6.  **arity_drop**: Drops universal dependencies in Skolemization. Targets verifiers that do not properly validate variable scopes during Skolemization.
7.  **parser_poisoning**: Uses unescaped quotes or comments within valid TPTP syntax to trick naive parsers into parsing a hidden `$false` derivation.
8.  **axiom_spoofing**: Modifies the payload of an axiom imported via `file()`. Tests whether the verifier genuinely checks α-equivalence against the parsed problem file or blindly trusts the `file()` tag.