//! First-order formulas and atomic formulas.
//!
//! [`Formula`] represents quantified first-order logic formulas with the usual
//! connectives (negation, conjunction, disjunction, implication, biconditional).
//!
//! [`Atom`] represents atomic formulas: predicate applications and equality.
//! Formulas are used as the input language (e.g., parsed from TPTP FOF format)
//! before clausification converts them to [`Clause`](crate::clause::Clause) sets.

use crate::HashSet;

use crate::symbol::SymbolId;
use crate::term::{Term, VarId};

/// An atomic formula.
///
/// Either a predicate applied to terms, or an equality between two terms.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum Atom {
    /// A predicate application `p(t1, ..., tn)`. Propositional variables have no arguments.
    Pred(SymbolId, Vec<Term>),
    /// An equality `t1 = t2`.
    Eq(Term, Term),
}

impl Atom {
    /// Creates a predicate application.
    pub fn pred(symbol: SymbolId, args: Vec<Term>) -> Self {
        Atom::Pred(symbol, args)
    }

    /// Creates a propositional atom (nullary predicate).
    pub fn prop(symbol: SymbolId) -> Self {
        Atom::Pred(symbol, Vec::new())
    }

    /// Creates an equality atom.
    pub fn eq(left: Term, right: Term) -> Self {
        Atom::Eq(left, right)
    }

    /// Collects all free variable IDs in this atom.
    pub fn collect_vars(&self, vars: &mut HashSet<VarId>) {
        match self {
            Atom::Pred(_, args) => {
                for arg in args {
                    arg.collect_vars(vars);
                }
            }
            Atom::Eq(l, r) => {
                l.collect_vars(vars);
                r.collect_vars(vars);
            }
        }
    }

    /// Returns the set of free variables in this atom.
    pub fn free_vars(&self) -> HashSet<VarId> {
        let mut vars = HashSet::default();
        self.collect_vars(&mut vars);
        vars
    }
}

/// A first-order formula.
///
/// This represents the full language of first-order logic with equality,
/// including quantifiers and all standard propositional connectives.
///
/// # Examples
///
/// ```
/// use mrs_core::formula::{Formula, Atom};
/// use mrs_core::term::Term;
/// use mrs_core::symbol::SymbolTable;
///
/// let mut syms = SymbolTable::new();
/// let p = syms.intern("p");
///
/// // ∀X. p(X)
/// let f = Formula::forall(0, Formula::atom(Atom::pred(p, vec![Term::var(0)])));
/// ```
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum Formula {
    /// An atomic formula.
    Atom(Atom),
    /// Negation: ¬φ
    Neg(Box<Formula>),
    /// Conjunction: φ₁ ∧ φ₂ ∧ ... ∧ φₙ
    And(Vec<Formula>),
    /// Disjunction: φ₁ ∨ φ₂ ∨ ... ∨ φₙ
    Or(Vec<Formula>),
    /// Implication: φ → ψ
    Implies(Box<Formula>, Box<Formula>),
    /// Biconditional: φ ↔ ψ
    Iff(Box<Formula>, Box<Formula>),
    /// Universal quantification: ∀x. φ
    Forall(VarId, Box<Formula>),
    /// Existential quantification: ∃x. φ
    Exists(VarId, Box<Formula>),
    /// Logical truth (⊤)
    True,
    /// Logical falsehood (⊥)
    False,
}

impl Formula {
    /// Creates an atomic formula.
    pub fn atom(a: Atom) -> Self {
        Formula::Atom(a)
    }

    /// Creates a negation.
    #[allow(clippy::should_implement_trait)]
    pub fn neg(f: Formula) -> Self {
        Formula::Neg(Box::new(f))
    }

    /// Creates a conjunction. Flattens nested `And`s.
    pub fn and(conjuncts: Vec<Formula>) -> Self {
        if conjuncts.len() == 1 {
            return conjuncts.into_iter().next().unwrap();
        }
        Formula::And(conjuncts)
    }

    /// Creates a disjunction. Flattens nested `Or`s.
    pub fn or(disjuncts: Vec<Formula>) -> Self {
        if disjuncts.len() == 1 {
            return disjuncts.into_iter().next().unwrap();
        }
        Formula::Or(disjuncts)
    }

    /// Creates an implication.
    pub fn implies(lhs: Formula, rhs: Formula) -> Self {
        Formula::Implies(Box::new(lhs), Box::new(rhs))
    }

    /// Creates a biconditional.
    pub fn iff(lhs: Formula, rhs: Formula) -> Self {
        Formula::Iff(Box::new(lhs), Box::new(rhs))
    }

    /// Creates a universal quantification.
    pub fn forall(var: VarId, body: Formula) -> Self {
        Formula::Forall(var, Box::new(body))
    }

    /// Creates an existential quantification.
    pub fn exists(var: VarId, body: Formula) -> Self {
        Formula::Exists(var, Box::new(body))
    }

    /// Collects all free variable IDs in this formula.
    ///
    /// Bound variables (those under a matching quantifier) are NOT included.
    pub fn free_vars(&self) -> HashSet<VarId> {
        let mut free = HashSet::default();
        let mut bound = HashSet::default();
        self.collect_free_vars(&mut free, &mut bound);
        free
    }

    fn collect_free_vars(&self, free: &mut HashSet<VarId>, bound: &mut HashSet<VarId>) {
        match self {
            Formula::Atom(a) => {
                let atom_vars = a.free_vars();
                for v in atom_vars {
                    if !bound.contains(&v) {
                        free.insert(v);
                    }
                }
            }
            Formula::Neg(f) => f.collect_free_vars(free, bound),
            Formula::And(fs) | Formula::Or(fs) => {
                for f in fs {
                    f.collect_free_vars(free, bound);
                }
            }
            Formula::Implies(a, b) | Formula::Iff(a, b) => {
                a.collect_free_vars(free, bound);
                b.collect_free_vars(free, bound);
            }
            Formula::Forall(v, body) | Formula::Exists(v, body) => {
                let was_bound = bound.insert(*v);
                body.collect_free_vars(free, bound);
                if was_bound {
                    bound.remove(v);
                }
            }
            Formula::True | Formula::False => {}
        }
    }

    /// Returns `true` if this formula contains no free variables.
    pub fn is_closed(&self) -> bool {
        self.free_vars().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::SymbolTable;

    #[test]
    fn free_vars_simple() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");

        // p(X) -> free vars = {X}
        let f = Formula::atom(Atom::pred(p, vec![Term::var(0)]));
        let fv = f.free_vars();
        assert_eq!(fv.len(), 1);
        assert!(fv.contains(&0));
    }

    #[test]
    fn free_vars_quantified() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");

        // ∀X. p(X) -> no free vars
        let f = Formula::forall(0, Formula::atom(Atom::pred(p, vec![Term::var(0)])));
        assert!(f.is_closed());
    }

    #[test]
    fn free_vars_mixed() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");

        // ∀X. p(X, Y) -> free vars = {Y}
        let f = Formula::forall(
            0,
            Formula::atom(Atom::pred(p, vec![Term::var(0), Term::var(1)])),
        );
        let fv = f.free_vars();
        assert_eq!(fv.len(), 1);
        assert!(fv.contains(&1));
    }
}
