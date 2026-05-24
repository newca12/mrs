//! Robinson's unification algorithm.
//!
//! This is the classic recursive unification algorithm. It's simple and
//! easy to understand, making it ideal for educational purposes.
//!
//! ## Algorithm
//!
//! Given two terms `s` and `t`, the algorithm works as follows:
//! 1. If `s` is a variable, bind it to `t` (with occurs check).
//! 2. If `t` is a variable, bind it to `s` (with occurs check).
//! 3. If both are function applications with the same symbol and arity,
//!    recursively unify corresponding arguments, composing substitutions.
//! 4. Otherwise, fail.

use mrs_core::Substitution;
use mrs_core::term::{Term, VarId};

use crate::{UnifyError, UnifyResult};

/// Unifies two terms using Robinson's algorithm.
///
/// Returns the most general unifier (MGU) or an error if unification fails.
pub fn unify(s: &Term, t: &Term) -> UnifyResult {
    let mut subst = Substitution::new();
    unify_rec(s, t, &mut subst)?;
    Ok(subst)
}

/// Recursive unification, accumulating bindings in `subst`.
fn unify_rec(s: &Term, t: &Term, subst: &mut Substitution) -> Result<(), UnifyError> {
    // Apply current substitution to both sides
    let s = subst.apply_term(s);
    let t = subst.apply_term(t);

    match (&s, &t) {
        // Identical terms: nothing to do
        _ if s == t => Ok(()),

        // Variable on the left
        (Term::Var(v), _) => bind_var(*v, &t, subst),

        // Variable on the right
        (_, Term::Var(v)) => bind_var(*v, &s, subst),

        // Two function applications
        (Term::App(f1, args1), Term::App(f2, args2)) => {
            if f1 != f2 {
                return Err(UnifyError::SymbolClash {
                    left: format!("{:?}", f1),
                    right: format!("{:?}", f2),
                });
            }
            if args1.len() != args2.len() {
                return Err(UnifyError::ArityMismatch {
                    expected: args1.len(),
                    found: args2.len(),
                });
            }
            for (a1, a2) in args1.iter().zip(args2.iter()) {
                unify_rec(a1, a2, subst)?;
            }
            Ok(())
        }
    }
}

/// Binds a variable to a term, performing the occurs check.
fn bind_var(var: VarId, term: &Term, subst: &mut Substitution) -> Result<(), UnifyError> {
    // If the term is the same variable, nothing to do
    if let Term::Var(v) = term
        && *v == var
    {
        return Ok(());
    }

    // Occurs check: variable must not appear in the term
    if term.contains_var(var) {
        return Err(UnifyError::OccursCheck { var });
    }

    subst.bind(var, term.clone());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::{SymbolId, SymbolTable};

    fn syms() -> (SymbolTable, SymbolId, SymbolId, SymbolId, SymbolId) {
        let mut st = SymbolTable::new();
        let f = st.intern("f");
        let g = st.intern("g");
        let a = st.intern("a");
        let b = st.intern("b");
        (st, f, g, a, b)
    }

    #[test]
    fn unify_identical_vars() {
        let mgu = unify(&Term::var(0), &Term::var(0)).unwrap();
        assert!(mgu.is_empty());
    }

    #[test]
    fn unify_var_with_constant() {
        let (_, _, _, a, _) = syms();
        let mgu = unify(&Term::var(0), &Term::constant(a)).unwrap();
        assert_eq!(mgu.apply_term(&Term::var(0)), Term::constant(a));
    }

    #[test]
    fn unify_two_vars() {
        let mgu = unify(&Term::var(0), &Term::var(1)).unwrap();
        // One var should be bound to the other
        let result = mgu.apply_term(&Term::var(0));
        assert_eq!(result, mgu.apply_term(&Term::var(1)));
    }

    #[test]
    fn unify_function_terms() {
        let (_, f, _, a, b) = syms();
        // f(X, a) ~ f(b, Y) -> {X/b, Y/a}
        let t1 = Term::app(f, vec![Term::var(0), Term::constant(a)]);
        let t2 = Term::app(f, vec![Term::constant(b), Term::var(1)]);
        let mgu = unify(&t1, &t2).unwrap();
        assert_eq!(mgu.apply_term(&Term::var(0)), Term::constant(b));
        assert_eq!(mgu.apply_term(&Term::var(1)), Term::constant(a));
    }

    #[test]
    fn unify_nested() {
        let (_, f, g, a, _) = syms();
        // f(X, g(X)) ~ f(a, g(a))
        let t1 = Term::app(f, vec![Term::var(0), Term::app(g, vec![Term::var(0)])]);
        let t2 = Term::app(
            f,
            vec![Term::constant(a), Term::app(g, vec![Term::constant(a)])],
        );
        let mgu = unify(&t1, &t2).unwrap();
        assert_eq!(mgu.apply_term(&Term::var(0)), Term::constant(a));
    }

    #[test]
    fn unify_transitive() {
        let (_, f, _, a, _) = syms();
        // f(X, Y) ~ f(Y, a) -> {X/a, Y/a}
        let t1 = Term::app(f, vec![Term::var(0), Term::var(1)]);
        let t2 = Term::app(f, vec![Term::var(1), Term::constant(a)]);
        let mgu = unify(&t1, &t2).unwrap();
        assert_eq!(mgu.apply_term(&Term::var(0)), Term::constant(a));
        assert_eq!(mgu.apply_term(&Term::var(1)), Term::constant(a));
    }

    #[test]
    fn unify_symbol_clash() {
        let (_, f, g, a, _) = syms();
        // f(a) ~ g(a) -> fail
        let t1 = Term::app(f, vec![Term::constant(a)]);
        let t2 = Term::app(g, vec![Term::constant(a)]);
        assert!(matches!(
            unify(&t1, &t2),
            Err(UnifyError::SymbolClash { .. })
        ));
    }

    #[test]
    fn unify_occurs_check() {
        let (_, f, _, _, _) = syms();
        // X ~ f(X) -> occurs check
        let t1 = Term::var(0);
        let t2 = Term::app(f, vec![Term::var(0)]);
        assert!(matches!(
            unify(&t1, &t2),
            Err(UnifyError::OccursCheck { var: 0 })
        ));
    }

    #[test]
    fn unify_different_constants() {
        let (_, _, _, a, b) = syms();
        // a ~ b -> fail
        assert!(unify(&Term::constant(a), &Term::constant(b)).is_err());
    }

    #[test]
    fn unify_idempotent() {
        let (_, f, _, a, _) = syms();
        // Applying the MGU twice should give the same result
        let t1 = Term::app(f, vec![Term::var(0), Term::var(1)]);
        let t2 = Term::app(f, vec![Term::constant(a), Term::var(0)]);
        let mgu = unify(&t1, &t2).unwrap();
        let applied1 = mgu.apply_term(&t1);
        let applied2 = mgu.apply_term(&applied1);
        assert_eq!(applied1, applied2);
    }
}
