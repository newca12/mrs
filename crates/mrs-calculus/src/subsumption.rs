//! Subsumption: detecting and removing redundant clauses.
//!
//! A clause `C1` subsumes clause `C2` if there exists a substitution `σ`
//! such that every literal in `σ(C1)` appears in `C2`. When `C1` subsumes `C2`,
//! `C2` is logically redundant and can be deleted without losing completeness.
//!
//! Uses one-way matching (not unification) since we only bind variables in `C1`.

use mrs_core::Atom;
use mrs_core::Substitution;
use mrs_core::clause::{Clause, ClauseIdGen, ClauseSource, Literal};
use mrs_core::term::Term;

use crate::rename::{max_var, rename_clause};

/// Returns `true` if `c1` subsumes `c2`.
///
/// `c1` subsumes `c2` if there exists a substitution `σ` such that
/// `σ(c1) ⊆ c2` (as a multiset of literals). This means `c2` is
/// redundant whenever `c1` exists.
///
/// Uses backtracking search over all possible literal mappings.
/// C1's variables are renamed to be disjoint from C2's before matching,
/// preventing variable overlap issues in the matching substitution.
pub fn subsumes(c1: &Clause, c2: &Clause) -> bool {
    // Quick size check: c1 can't subsume c2 if it has more literals
    if c1.len() > c2.len() {
        return false;
    }

    // Empty clause subsumes everything
    if c1.is_empty() {
        return true;
    }

    // Rename c1's variables to be disjoint from c2's.
    // After renaming, c1's vars have VarId >= offset, c2's vars have VarId < offset.
    // Only c1's (renamed) variables may be bound during matching.
    let offset = max_var(c2);
    let c1_renamed = rename_clause(c1, offset);

    let subst = Substitution::new();
    match_literals(&c1_renamed.literals, &c2.literals, &subst, offset)
}

/// Recursively tries to match each literal in `remaining` (from c1)
/// to some literal in `targets` (from c2), building up a consistent substitution.
/// `min_bindable` is the minimum VarId that can be bound (c1's renamed vars).
fn match_literals(
    remaining: &[Literal],
    targets: &[Literal],
    current_subst: &Substitution,
    min_bindable: u32,
) -> bool {
    if remaining.is_empty() {
        return true;
    }

    let lit = &remaining[0];
    let rest = &remaining[1..];

    for target_lit in targets {
        // Polarity must match
        if lit.positive != target_lit.positive {
            continue;
        }

        // Try to match the atoms
        if let Some(extended) =
            match_atoms(&lit.atom, &target_lit.atom, current_subst, min_bindable)
        {
            // Recursively match the rest
            if match_literals(rest, targets, &extended, min_bindable) {
                return true;
            }
        }
    }

    false
}

/// Tries to extend `current_subst` so that it maps `pattern` atom to `target` atom.
/// Returns the extended substitution on success, or None on failure.
/// Only variables with VarId >= `min_bindable` may be bound.
fn match_atoms(
    pattern: &Atom,
    target: &Atom,
    current_subst: &Substitution,
    min_bindable: u32,
) -> Option<Substitution> {
    match (pattern, target) {
        (Atom::Pred(p1, args1), Atom::Pred(p2, args2)) => {
            if p1 != p2 || args1.len() != args2.len() {
                return None;
            }
            // Match each argument pair, extending the substitution
            let mut subst = current_subst.clone();
            for (a1, a2) in args1.iter().zip(args2.iter()) {
                // Apply current substitution to the pattern before matching.
                // Use flat (non-chaining) apply to avoid infinite recursion when
                // pattern and target variables share VarIds, creating transitive
                // cycles like V0→V2→mult(V0,...) in the substitution.
                let a1_applied = apply_subst_flat(&subst, a1);
                if !match_single_term(&a1_applied, a2, &mut subst, min_bindable) {
                    return None;
                }
            }
            Some(subst)
        }
        (Atom::Eq(l1, r1), Atom::Eq(l2, r2)) => {
            // Try both orientations of the equality
            // Try l1→l2, r1→r2
            let mut subst = current_subst.clone();
            let l1_applied = apply_subst_flat(&subst, l1);
            if match_single_term(&l1_applied, l2, &mut subst, min_bindable) {
                let r1_applied = apply_subst_flat(&subst, r1);
                if match_single_term(&r1_applied, r2, &mut subst, min_bindable) {
                    return Some(subst);
                }
            }
            // Try l1→r2, r1→l2
            let mut subst = current_subst.clone();
            let l1_applied = apply_subst_flat(&subst, l1);
            if match_single_term(&l1_applied, r2, &mut subst, min_bindable) {
                let r1_applied = apply_subst_flat(&subst, r1);
                if match_single_term(&r1_applied, l2, &mut subst, min_bindable) {
                    return Some(subst);
                }
            }
            None
        }
        _ => None,
    }
}

/// Tries to match a single (already-substituted) pattern term against a target,
/// extending the substitution. Returns true on success.
/// Only variables with VarId >= `min_bindable` may be bound; others are treated
/// as constants (they come from C2's variable space).
fn match_single_term(
    pattern: &Term,
    target: &Term,
    subst: &mut Substitution,
    min_bindable: u32,
) -> bool {
    match pattern {
        Term::Var(v) => {
            if let Some(bound) = subst.lookup(*v) {
                // Already bound: check consistency
                bound == target
            } else if *v < min_bindable {
                // This is a C2 variable that leaked into the pattern via substitution.
                // It must match the target exactly (treated as a constant).
                pattern == target
            } else {
                // Trivial case: Var(v) matching Var(v) is always fine (identity).
                if let Term::Var(tv) = target
                    && *v == *tv
                {
                    // Don't bind (it's a no-op), just succeed.
                    return true;
                }
                // Occurs check: don't bind a variable to a term containing itself.
                // This prevents circular bindings (e.g., V2 → mult(V2, V3)) that can
                // arise when pattern variables resolve to target variables via chains.
                if target.contains_var(*v) {
                    return false;
                }
                subst.bind(*v, target.clone());
                true
            }
        }
        Term::App(f1, args1) => match target {
            Term::App(f2, args2) => {
                if f1 != f2 || args1.len() != args2.len() {
                    return false;
                }
                for (a1, a2) in args1.iter().zip(args2.iter()) {
                    let a1_applied = apply_subst_flat(subst, a1);
                    if !match_single_term(&a1_applied, a2, subst, min_bindable) {
                        return false;
                    }
                }
                true
            }
            Term::Var(_) => false,
        },
    }
}

/// Applies a substitution without following variable chains.
///
/// In subsumption matching, pattern variables can be bound to target variables
/// that coincidentally share VarIds with other pattern variables. The standard
/// `Substitution::apply_term` follows chains transitively (V0→V2→mult(V0,...)),
/// which can infinite-loop on such substitutions.
///
/// This function does single-level lookup only: if V is bound to T, it returns T
/// without recursing into T to resolve further variables. This is correct for
/// subsumption matching because we conceptually only substitute pattern variables.
fn apply_subst_flat(subst: &Substitution, term: &Term) -> Term {
    match term {
        Term::Var(v) => match subst.lookup(*v) {
            Some(t) => t.clone(),
            None => term.clone(),
        },
        Term::App(f, args) => {
            let new_args: Vec<Term> = args.iter().map(|a| apply_subst_flat(subst, a)).collect();
            Term::App(*f, new_args)
        }
    }
}

/// Condenses a clause by removing redundant literals.
///
/// A clause C condenses to C' if C' is a factor of C and C' subsumes C.
/// This means some literals in C are "subsumed" by other literals in the
/// same clause under a consistent substitution. Condensation reduces clause
/// size without losing logical content.
///
/// Returns `Some(condensed_clause)` if condensation was possible, `None` otherwise.
pub fn condense(clause: &Clause, id_gen: &mut ClauseIdGen) -> Option<Clause> {
    // Try to find a matching σ that maps literal i to literal j (i ≠ j)
    for i in 0..clause.literals.len() {
        for j in 0..clause.literals.len() {
            if i == j {
                continue;
            }

            let lit_i = &clause.literals[i];
            let lit_j = &clause.literals[j];

            // Same polarity required
            if lit_i.positive != lit_j.positive {
                continue;
            }

            // Try to match lit_i's atom (pattern) to lit_j's atom (target)
            // min_bindable=0: within the same clause, all variables are bindable
            let subst = Substitution::new();
            if let Some(sigma) = match_atoms(&lit_i.atom, &lit_j.atom, &subst, 0) {
                // Apply σ to all literals, remove literal i (it maps to literal j)
                let mut new_lits: Vec<Literal> = Vec::new();
                for (k, lit) in clause.literals.iter().enumerate() {
                    if k == i {
                        continue;
                    }
                    new_lits.push(sigma.apply_literal(lit));
                }

                // Deduplicate (σ might collapse other pairs too)
                let mut new_clause = Clause::new(
                    id_gen.next(),
                    new_lits,
                    ClauseSource::Inference {
                        rule: "condensation".into(),
                        parents: vec![clause.id],
                    },
                );
                new_clause.deduplicate();

                // Verify that the condensed clause actually has fewer literals
                // AND that it subsumes the original. Without the subsumption check,
                // condensation can incorrectly merge independent variables (e.g.,
                // ~h(X0)|g(X0)|~i(X1)|~h(X1) condensed to g(X1)|~i(X1)|~h(X1)
                // ties X0=X1, losing a degree of freedom needed for the proof).
                if new_clause.len() < clause.len() && subsumes(&new_clause, clause) {
                    return Some(new_clause);
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::clause::{Clause, ClauseIdGen, ClauseSource};
    use mrs_core::{Atom, Literal, SymbolTable, Term};

    fn input_clause(id_gen: &mut ClauseIdGen, lits: Vec<Literal>, name: &str) -> Clause {
        Clause::new(
            id_gen.next(),
            lits,
            ClauseSource::Input {
                name: name.into(),
                role: "axiom".into(),
            },
        )
    }

    #[test]
    fn subsumes_identical() {
        // p(a) subsumes p(a)
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();

        let c1 = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
            "c1",
        );
        let c2 = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
            "c2",
        );

        assert!(subsumes(&c1, &c2));
    }

    #[test]
    fn subsumes_more_general() {
        // p(X) subsumes p(a)
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();

        let c1 = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::var(0)]))],
            "c1",
        );
        let c2 = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
            "c2",
        );

        assert!(subsumes(&c1, &c2));
        // Not the other way: p(a) does not subsume p(X)
        assert!(!subsumes(&c2, &c1));
    }

    #[test]
    fn subsumes_subset() {
        // {p(X)} subsumes {p(a), q(b)}
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let mut id_gen = ClauseIdGen::new();

        let c1 = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::var(0)]))],
            "c1",
        );
        let c2 = input_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::constant(a)])),
                Literal::pos(Atom::pred(q, vec![Term::constant(b)])),
            ],
            "c2",
        );

        assert!(subsumes(&c1, &c2));
        assert!(!subsumes(&c2, &c1));
    }

    #[test]
    fn subsumes_different_predicates() {
        // p(a) does NOT subsume q(a)
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();

        let c1 = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
            "c1",
        );
        let c2 = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(q, vec![Term::constant(a)]))],
            "c2",
        );

        assert!(!subsumes(&c1, &c2));
    }

    #[test]
    fn subsumes_polarity_matters() {
        // p(X) does NOT subsume ¬p(a)
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();

        let c1 = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::var(0)]))],
            "c1",
        );
        let c2 = input_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::pred(p, vec![Term::constant(a)]))],
            "c2",
        );

        assert!(!subsumes(&c1, &c2));
    }

    #[test]
    fn subsumes_empty_clause() {
        // Empty clause subsumes everything
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();

        let empty = input_clause(&mut id_gen, vec![], "empty");
        let c2 = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
            "c2",
        );

        assert!(subsumes(&empty, &c2));
    }

    #[test]
    fn subsumes_equality() {
        // X = Y subsumes a = b
        let mut syms = SymbolTable::new();
        let a = syms.intern("a");
        let b = syms.intern("b");
        let mut id_gen = ClauseIdGen::new();

        let c1 = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::eq(Term::var(0), Term::var(1)))],
            "c1",
        );
        let c2 = input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::eq(Term::constant(a), Term::constant(b)))],
            "c2",
        );

        assert!(subsumes(&c1, &c2));
    }

    #[test]
    fn subsumes_consistent_binding() {
        // {p(X), q(X)} subsumes {p(a), q(a)} but not {p(a), q(b)}
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let mut id_gen = ClauseIdGen::new();

        let c1 = input_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(0)])),
                Literal::pos(Atom::pred(q, vec![Term::var(0)])),
            ],
            "c1",
        );
        let c2_ok = input_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::constant(a)])),
                Literal::pos(Atom::pred(q, vec![Term::constant(a)])),
            ],
            "c2_ok",
        );
        let c2_bad = input_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::constant(a)])),
                Literal::pos(Atom::pred(q, vec![Term::constant(b)])),
            ],
            "c2_bad",
        );

        assert!(subsumes(&c1, &c2_ok));
        assert!(!subsumes(&c1, &c2_bad));
    }

    #[test]
    fn condense_redundant_literal() {
        // {p(X), q(X), p(Y)} can be condensed to {q(Y), p(Y)} by X→Y
        // because {q(Y), p(Y)} subsumes {p(X), q(X), p(Y)} (via Z→X).
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let mut id_gen = ClauseIdGen::new();

        let c = input_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(0)])),
                Literal::pos(Atom::pred(q, vec![Term::var(0)])),
                Literal::pos(Atom::pred(p, vec![Term::var(1)])),
            ],
            "c",
        );

        let result = condense(&c, &mut id_gen);
        assert!(result.is_some());
        let condensed = result.unwrap();
        assert_eq!(condensed.len(), 2); // p(Y) and q(Y)
    }

    #[test]
    fn condense_already_condensed() {
        // {p(a), q(b)} — no condensation possible
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let mut id_gen = ClauseIdGen::new();

        let c = input_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::constant(a)])),
                Literal::pos(Atom::pred(q, vec![Term::constant(b)])),
            ],
            "c",
        );

        assert!(condense(&c, &mut id_gen).is_none());
    }

    #[test]
    fn condense_duplicate_literals() {
        // {p(X), p(Y)} can be condensed to {p(X)} by X→Y (or Y→X)
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let mut id_gen = ClauseIdGen::new();

        let c = input_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(0)])),
                Literal::pos(Atom::pred(p, vec![Term::var(1)])),
            ],
            "c",
        );

        let result = condense(&c, &mut id_gen);
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[test]
    fn condense_independent_variables_rejected() {
        // ~h(X0) | g(X0) | ~i(X1) | ~h(X1) should NOT be condensed
        // because merging X0=X1 loses independence of the two variables.
        // The condensed g(X1)|~i(X1)|~h(X1) does NOT subsume the original.
        let mut syms = SymbolTable::new();
        let h = syms.intern("h");
        let g = syms.intern("g");
        let i = syms.intern("i");
        let mut id_gen = ClauseIdGen::new();

        let c = input_clause(
            &mut id_gen,
            vec![
                Literal::neg(Atom::pred(h, vec![Term::var(0)])),
                Literal::pos(Atom::pred(g, vec![Term::var(0)])),
                Literal::neg(Atom::pred(i, vec![Term::var(1)])),
                Literal::neg(Atom::pred(h, vec![Term::var(1)])),
            ],
            "c",
        );

        // This should return None because the "condensed" clause
        // doesn't subsume the original (variables get merged)
        assert!(condense(&c, &mut id_gen).is_none());
    }
}
