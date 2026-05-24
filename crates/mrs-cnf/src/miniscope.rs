//! Miniscoping: push quantifiers inward.
//!
//! Miniscoping reduces the scope of quantifiers by pushing them toward the
//! atoms that actually use the bound variable. This reduces the arity of
//! Skolem functions, producing simpler clauses.
//!
//! Applied after NNF conversion and before Skolemization.
//!
//! ## Key Rules
//!
//! - `∀x. (A ∧ B)` → `(∀x. A) ∧ (∀x. B)` — distribute ∀ over ∧
//! - `∃x. (A ∨ B)` → `(∃x. A) ∨ (∃x. B)` — distribute ∃ over ∨
//! - `∀x. (A ∨ B)` where x ∉ FV(A) → `A ∨ (∀x. B)` — push past irrelevant disjuncts
//! - `∃x. (A ∧ B)` where x ∉ FV(A) → `A ∧ (∃x. B)` — push past irrelevant conjuncts
//! - `Qx. A` where x ∉ FV(A) → `A` — drop vacuous quantifiers

use mrs_core::Formula;
use mrs_core::term::VarId;

/// Pushes quantifiers inward as far as possible.
///
/// Assumes the formula is in NNF (no implications, negations only on atoms).
/// Should be called before Skolemization so that existential variables see
/// fewer universal variables in scope, producing lower-arity Skolem functions.
pub fn miniscope(formula: &Formula) -> Formula {
    match formula {
        Formula::Forall(v, body) => {
            let body = miniscope(body);
            push_forall(*v, body)
        }
        Formula::Exists(v, body) => {
            let body = miniscope(body);
            push_exists(*v, body)
        }
        Formula::And(cs) => Formula::and(cs.iter().map(miniscope).collect()),
        Formula::Or(ds) => Formula::or(ds.iter().map(miniscope).collect()),
        Formula::Neg(inner) => Formula::neg(miniscope(inner)),
        other => other.clone(),
    }
}

/// Returns true if variable `v` occurs free in the formula.
fn has_free_var(formula: &Formula, v: VarId) -> bool {
    formula.free_vars().contains(&v)
}

/// Push ∀x inward over a miniscoped body.
fn push_forall(v: VarId, body: Formula) -> Formula {
    if !has_free_var(&body, v) {
        return body; // vacuous quantifier
    }

    match body {
        // ∀x. (A₁ ∧ ... ∧ Aₙ) → (Q₁A₁) ∧ ... ∧ (QₙAₙ)
        // where Qᵢ = ∀x if x ∈ FV(Aᵢ), otherwise identity.
        Formula::And(cs) => {
            let parts: Vec<Formula> = cs
                .into_iter()
                .map(|c| {
                    if has_free_var(&c, v) {
                        Formula::forall(v, c)
                    } else {
                        c
                    }
                })
                .collect();
            Formula::and(parts)
        }

        // ∀x. (A₁ ∨ ... ∨ Aₙ): push past disjuncts not containing x.
        Formula::Or(ds) => {
            let (with_x, without_x): (Vec<_>, Vec<_>) =
                ds.into_iter().partition(|d| has_free_var(d, v));
            if without_x.is_empty() {
                // x in all disjuncts — can't push further
                Formula::forall(v, Formula::or(with_x))
            } else {
                let mut parts = without_x;
                if !with_x.is_empty() {
                    parts.push(Formula::forall(v, Formula::or(with_x)));
                }
                Formula::or(parts)
            }
        }

        other => Formula::forall(v, other),
    }
}

/// Push ∃x inward over a miniscoped body.
fn push_exists(v: VarId, body: Formula) -> Formula {
    if !has_free_var(&body, v) {
        return body; // vacuous quantifier
    }

    match body {
        // ∃x. (A₁ ∨ ... ∨ Aₙ) → (Q₁A₁) ∨ ... ∨ (QₙAₙ)
        // where Qᵢ = ∃x if x ∈ FV(Aᵢ), otherwise identity.
        Formula::Or(ds) => {
            let parts: Vec<Formula> = ds
                .into_iter()
                .map(|d| {
                    if has_free_var(&d, v) {
                        Formula::exists(v, d)
                    } else {
                        d
                    }
                })
                .collect();
            Formula::or(parts)
        }

        // ∃x. (A₁ ∧ ... ∧ Aₙ): push past conjuncts not containing x.
        Formula::And(cs) => {
            let (with_x, without_x): (Vec<_>, Vec<_>) =
                cs.into_iter().partition(|c| has_free_var(c, v));
            if without_x.is_empty() {
                // x in all conjuncts — can't push further
                Formula::exists(v, Formula::and(with_x))
            } else {
                let mut parts = without_x;
                if !with_x.is_empty() {
                    parts.push(Formula::exists(v, Formula::and(with_x)));
                }
                Formula::and(parts)
            }
        }

        other => Formula::exists(v, other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::display::DisplayWithSymbols;
    use mrs_core::{Atom, SymbolTable, Term};

    fn fmt(f: &Formula, syms: &SymbolTable) -> String {
        format!("{}", f.display(syms))
    }

    #[test]
    fn vacuous_forall() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");

        // ∀X. p(a) → p(a)
        let f = Formula::forall(0, Formula::atom(Atom::pred(p, vec![Term::constant(a)])));
        let result = miniscope(&f);
        assert_eq!(fmt(&result, &syms), "p(a)");
    }

    #[test]
    fn vacuous_exists() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");

        // ∃X. p(a) → p(a)
        let f = Formula::exists(0, Formula::atom(Atom::pred(p, vec![Term::constant(a)])));
        let result = miniscope(&f);
        assert_eq!(fmt(&result, &syms), "p(a)");
    }

    #[test]
    fn forall_over_and() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");

        // ∀X. (p(X) ∧ q(X)) → (∀X. p(X)) ∧ (∀X. q(X))
        let f = Formula::forall(
            0,
            Formula::and(vec![
                Formula::atom(Atom::pred(p, vec![Term::var(0)])),
                Formula::atom(Atom::pred(q, vec![Term::var(0)])),
            ]),
        );
        let result = miniscope(&f);
        assert_eq!(fmt(&result, &syms), "(![X0]: (p(X0)) & ![X0]: (q(X0)))");
    }

    #[test]
    fn forall_over_and_partial() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let a = syms.intern("a");

        // ∀X. (p(X) ∧ q(a)) → (∀X. p(X)) ∧ q(a)
        let f = Formula::forall(
            0,
            Formula::and(vec![
                Formula::atom(Atom::pred(p, vec![Term::var(0)])),
                Formula::atom(Atom::pred(q, vec![Term::constant(a)])),
            ]),
        );
        let result = miniscope(&f);
        assert_eq!(fmt(&result, &syms), "(![X0]: (p(X0)) & q(a))");
    }

    #[test]
    fn exists_over_or() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");

        // ∃X. (p(X) ∨ q(X)) → (∃X. p(X)) ∨ (∃X. q(X))
        let f = Formula::exists(
            0,
            Formula::or(vec![
                Formula::atom(Atom::pred(p, vec![Term::var(0)])),
                Formula::atom(Atom::pred(q, vec![Term::var(0)])),
            ]),
        );
        let result = miniscope(&f);
        assert_eq!(fmt(&result, &syms), "(?[X0]: (p(X0)) | ?[X0]: (q(X0)))");
    }

    #[test]
    fn exists_over_and_partial() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let a = syms.intern("a");

        // ∃X. (p(X) ∧ q(a)) → q(a) ∧ (∃X. p(X))
        let f = Formula::exists(
            0,
            Formula::and(vec![
                Formula::atom(Atom::pred(p, vec![Term::var(0)])),
                Formula::atom(Atom::pred(q, vec![Term::constant(a)])),
            ]),
        );
        let result = miniscope(&f);
        assert_eq!(fmt(&result, &syms), "(q(a) & ?[X0]: (p(X0)))");
    }

    #[test]
    fn forall_over_or_partial() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let a = syms.intern("a");

        // ∀X. (p(X) ∨ q(a)) → q(a) ∨ (∀X. p(X))
        let f = Formula::forall(
            0,
            Formula::or(vec![
                Formula::atom(Atom::pred(p, vec![Term::var(0)])),
                Formula::atom(Atom::pred(q, vec![Term::constant(a)])),
            ]),
        );
        let result = miniscope(&f);
        assert_eq!(fmt(&result, &syms), "(q(a) | ![X0]: (p(X0)))");
    }

    #[test]
    fn reduces_skolem_arity() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");

        // ∀X. ∀Y. (p(X) ∧ ∃Z. q(Y, Z))
        // Without miniscoping, Z would Skolemize to sk(X, Y) — arity 2.
        // With miniscoping:
        //   ∀X. ∀Y. (p(X) ∧ ∃Z. q(Y, Z))
        //   → (∀X. p(X)) ∧ (∀Y. ∃Z. q(Y, Z))  [∀X dist over ∧, vacuous in 2nd]
        // Now Z Skolemizes to sk(Y) — arity 1.
        let f = Formula::forall(
            0,
            Formula::forall(
                1,
                Formula::and(vec![
                    Formula::atom(Atom::pred(p, vec![Term::var(0)])),
                    Formula::exists(
                        2,
                        Formula::atom(Atom::pred(q, vec![Term::var(1), Term::var(2)])),
                    ),
                ]),
            ),
        );
        let result = miniscope(&f);
        // Result: (∀X. p(X)) ∧ (∀Y. ∃Z. q(Y, Z))
        assert_eq!(
            fmt(&result, &syms),
            "(![X0]: (p(X0)) & ![X1]: (?[X2]: (q(X1, X2))))"
        );
    }

    #[test]
    fn nested_miniscoping() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let r = syms.intern("r");

        // ∀X. (p(X) ∧ ∃Y. (q(Y) ∨ r(X, Y)))
        // → (∀X. p(X)) ∧ (∀X. (∃Y. q(Y)) ∨ (∃Y. r(X, Y)))
        // The ∃Y distributes over the ∨, then the q(Y) branch gets a
        // Skolem constant (no X in scope) instead of a Skolem function of X.
        let f = Formula::forall(
            0,
            Formula::and(vec![
                Formula::atom(Atom::pred(p, vec![Term::var(0)])),
                Formula::exists(
                    1,
                    Formula::or(vec![
                        Formula::atom(Atom::pred(q, vec![Term::var(1)])),
                        Formula::atom(Atom::pred(r, vec![Term::var(0), Term::var(1)])),
                    ]),
                ),
            ]),
        );
        let result = miniscope(&f);
        assert_eq!(
            fmt(&result, &syms),
            "(![X0]: (p(X0)) & ![X0]: ((?[X1]: (q(X1)) | ?[X1]: (r(X0, X1)))))"
        );
    }

    #[test]
    fn no_quantifiers_unchanged() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");

        let f = Formula::and(vec![
            Formula::atom(Atom::prop(p)),
            Formula::atom(Atom::prop(q)),
        ]);
        let result = miniscope(&f);
        assert_eq!(fmt(&result, &syms), "(p & q)");
    }
}
