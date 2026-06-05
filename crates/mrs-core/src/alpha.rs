//! α-equivalence checking for [`Formula`] and [`Term`].
//!
//! Two formulas are α-equivalent if they differ only in the names of bound
//! variables. Free variables must have the same identifiers.

use std::collections::HashMap;

use crate::formula::{Atom, Formula};
use crate::term::{Term, VarId};

/// Returns `true` if the two formulas are α-equivalent.
pub fn alpha_equiv(a: &Formula, b: &Formula) -> bool {
    let mut left = HashMap::new();
    let mut right = HashMap::new();
    let mut depth: u32 = 0;
    formula_eq(a, b, &mut left, &mut right, &mut depth)
}

/// Returns `true` if two terms are equal modulo free-variable identity.
/// (No binders inside terms, so this is structural equality.)
pub fn alpha_equiv_term(a: &Term, b: &Term) -> bool {
    let empty = HashMap::new();
    term_eq(a, b, &empty, &empty)
}

fn formula_eq(
    a: &Formula,
    b: &Formula,
    left: &mut HashMap<VarId, u32>,
    right: &mut HashMap<VarId, u32>,
    depth: &mut u32,
) -> bool {
    match (a, b) {
        (Formula::True, Formula::True) | (Formula::False, Formula::False) => true,
        (Formula::Atom(x), Formula::Atom(y)) => atom_eq(x, y, left, right),
        (Formula::Neg(x), Formula::Neg(y)) => formula_eq(x, y, left, right, depth),
        (Formula::And(xs), Formula::And(ys)) | (Formula::Or(xs), Formula::Or(ys)) => {
            if xs.len() != ys.len() {
                return false;
            }
            let mut used = vec![false; ys.len()];
            for x in xs {
                let mut matched = false;
                for (j, y) in ys.iter().enumerate() {
                    if !used[j] && formula_eq(x, y, left, right, depth) {
                        used[j] = true;
                        matched = true;
                        break;
                    }
                }
                if !matched {
                    return false;
                }
            }
            true
        }
        (Formula::Iff(a1, b1), Formula::Iff(a2, b2)) => {
            (formula_eq(a1, a2, left, right, depth) && formula_eq(b1, b2, left, right, depth))
                || (formula_eq(a1, b2, left, right, depth) && formula_eq(b1, a2, left, right, depth))
        }
        (Formula::Implies(a1, b1), Formula::Implies(a2, b2)) => {
            formula_eq(a1, a2, left, right, depth) && formula_eq(b1, b2, left, right, depth)
        }
        (Formula::Forall(v1, body1), Formula::Forall(v2, body2))
        | (Formula::Exists(v1, body1), Formula::Exists(v2, body2)) => {
            let d = *depth;
            *depth += 1;
            let old_l = left.insert(*v1, d);
            let old_r = right.insert(*v2, d);
            let ok = formula_eq(body1, body2, left, right, depth);
            match old_l {
                Some(v) => {
                    left.insert(*v1, v);
                }
                None => {
                    left.remove(v1);
                }
            }
            match old_r {
                Some(v) => {
                    right.insert(*v2, v);
                }
                None => {
                    right.remove(v2);
                }
            }
            *depth -= 1;
            ok
        }
        _ => false,
    }
}

fn atom_eq(a: &Atom, b: &Atom, left: &HashMap<VarId, u32>, right: &HashMap<VarId, u32>) -> bool {
    match (a, b) {
        (Atom::Pred(s1, args1), Atom::Pred(s2, args2)) => {
            s1 == s2
                && args1.len() == args2.len()
                && args1
                    .iter()
                    .zip(args2.iter())
                    .all(|(x, y)| term_eq(x, y, left, right))
        }
        (Atom::Eq(l1, r1), Atom::Eq(l2, r2)) => {
            (term_eq(l1, l2, left, right) && term_eq(r1, r2, left, right))
                || (term_eq(l1, r2, left, right) && term_eq(r1, l2, left, right))
        }
        _ => false,
    }
}

fn term_eq(a: &Term, b: &Term, left: &HashMap<VarId, u32>, right: &HashMap<VarId, u32>) -> bool {
    match (a, b) {
        (Term::Var(v1), Term::Var(v2)) => match (left.get(v1), right.get(v2)) {
            (Some(d1), Some(d2)) => d1 == d2,
            (None, None) => v1 == v2,
            _ => false,
        },
        (Term::App(s1, args1), Term::App(s2, args2)) => {
            s1 == s2
                && args1.len() == args2.len()
                && args1
                    .iter()
                    .zip(args2.iter())
                    .all(|(x, y)| term_eq(x, y, left, right))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::SymbolTable;

    #[test]
    fn alpha_equiv_simple_quantifier() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        // ∀X0.p(X0)
        let a = Formula::forall(0, Formula::atom(Atom::pred(p, vec![Term::var(0)])));
        // ∀X1.p(X1)
        let b = Formula::forall(1, Formula::atom(Atom::pred(p, vec![Term::var(1)])));
        assert!(alpha_equiv(&a, &b));
    }

    #[test]
    fn alpha_equiv_distinguishes_structure() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        // ∀X.p(X)  vs  ∃X.p(X)
        let a = Formula::forall(0, Formula::atom(Atom::pred(p, vec![Term::var(0)])));
        let b = Formula::exists(0, Formula::atom(Atom::pred(p, vec![Term::var(0)])));
        assert!(!alpha_equiv(&a, &b));
    }

    #[test]
    fn alpha_equiv_free_vars_must_match() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = Formula::atom(Atom::pred(p, vec![Term::var(0)]));
        let b = Formula::atom(Atom::pred(p, vec![Term::var(1)]));
        assert!(!alpha_equiv(&a, &b));
    }
}
