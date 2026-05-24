//! One-way matching: find a substitution making a pattern equal to a target.
//!
//! Unlike unification, matching only binds variables in the *pattern*.
//! Variables in the target are treated as constants (they are never bound).
//! This is useful for rewriting, demodulation, and subsumption checks.

use mrs_core::Substitution;
use mrs_core::term::Term;

use crate::{UnifyError, UnifyResult};

/// Matches a pattern against a target term.
///
/// Returns a substitution σ such that σ(pattern) = target,
/// or an error if no such substitution exists.
///
/// Only variables in the pattern are bound. Variables in the target
/// are treated as distinct constants.
pub fn match_term(pattern: &Term, target: &Term) -> UnifyResult {
    let mut subst = Substitution::new();
    match_rec(pattern, target, &mut subst)?;
    Ok(subst)
}

/// Recursive matching.
fn match_rec(pattern: &Term, target: &Term, subst: &mut Substitution) -> Result<(), UnifyError> {
    match pattern {
        Term::Var(v) => {
            if let Some(bound) = subst.lookup(*v) {
                // Variable already bound: check consistency
                if bound == target {
                    Ok(())
                } else {
                    Err(UnifyError::SymbolClash {
                        left: format!("X{} (bound)", v),
                        right: format!("{:?}", target),
                    })
                }
            } else {
                // Bind the pattern variable to the target
                subst.bind(*v, target.clone());
                Ok(())
            }
        }
        Term::App(f1, args1) => {
            match target {
                Term::App(f2, args2) => {
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
                        match_rec(a1, a2, subst)?;
                    }
                    Ok(())
                }
                // Pattern is a function, target is a variable -> can't match
                Term::Var(_) => Err(UnifyError::SymbolClash {
                    left: format!("{:?}", f1),
                    right: "variable".to_string(),
                }),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::SymbolTable;

    #[test]
    fn match_var_to_constant() {
        let mut syms = SymbolTable::new();
        let a = syms.intern("a");

        let sub = match_term(&Term::var(0), &Term::constant(a)).unwrap();
        assert_eq!(sub.apply_term(&Term::var(0)), Term::constant(a));
    }

    #[test]
    fn match_function_pattern() {
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let a = syms.intern("a");
        let b = syms.intern("b");

        // f(X, Y) matches f(a, b) => X=a, Y=b
        let pattern = Term::app(f, vec![Term::var(0), Term::var(1)]);
        let target = Term::app(f, vec![Term::constant(a), Term::constant(b)]);
        let sub = match_term(&pattern, &target).unwrap();
        assert_eq!(sub.apply_term(&Term::var(0)), Term::constant(a));
        assert_eq!(sub.apply_term(&Term::var(1)), Term::constant(b));
    }

    #[test]
    fn match_repeated_var() {
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let a = syms.intern("a");

        // f(X, X) matches f(a, a) => X=a
        let pattern = Term::app(f, vec![Term::var(0), Term::var(0)]);
        let target = Term::app(f, vec![Term::constant(a), Term::constant(a)]);
        let sub = match_term(&pattern, &target).unwrap();
        assert_eq!(sub.apply_term(&Term::var(0)), Term::constant(a));
    }

    #[test]
    fn match_repeated_var_conflict() {
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let a = syms.intern("a");
        let b = syms.intern("b");

        // f(X, X) does NOT match f(a, b)
        let pattern = Term::app(f, vec![Term::var(0), Term::var(0)]);
        let target = Term::app(f, vec![Term::constant(a), Term::constant(b)]);
        assert!(match_term(&pattern, &target).is_err());
    }

    #[test]
    fn match_no_target_var_binding() {
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let a = syms.intern("a");

        // f(a) does NOT match f(X) — target variable is not a valid match
        let pattern = Term::app(f, vec![Term::constant(a)]);
        let target = Term::app(f, vec![Term::var(0)]);
        // This should succeed: constant matches variable when pattern has no var
        // Actually: pattern is f(a), target is f(X). The pattern has a constant
        // where target has a variable. Matching requires pattern vars to map to
        // target terms, but here the pattern has no variable at that position.
        // a ≠ X, so this fails.
        assert!(match_term(&pattern, &target).is_err());
    }

    #[test]
    fn match_var_to_compound() {
        let mut syms = SymbolTable::new();
        let _f = syms.intern("f");
        let g = syms.intern("g");
        let a = syms.intern("a");

        // X matches g(a) => X = g(a)
        let pattern = Term::var(0);
        let target = Term::app(g, vec![Term::constant(a)]);
        let sub = match_term(&pattern, &target).unwrap();
        assert_eq!(
            sub.apply_term(&Term::var(0)),
            Term::app(g, vec![Term::constant(a)])
        );
    }
}
