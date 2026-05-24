//! Substitutions: mappings from variables to terms.
//!
//! A [`Substitution`] maps variable IDs to terms. Applying a substitution to a
//! term replaces each mapped variable with its corresponding term.
//! Substitutions are the core mechanism for unification and inference.

use std::collections::HashMap;

use crate::clause::Literal;
use crate::formula::{Atom, Formula};
use crate::term::{Term, VarId};

/// A substitution mapping variables to terms.
///
/// Internally stored as a `HashMap` for efficient lookup.
///
/// # Examples
///
/// ```
/// use mrs_core::subst::Substitution;
/// use mrs_core::term::Term;
/// use mrs_core::symbol::SymbolTable;
///
/// let mut syms = SymbolTable::new();
/// let a = syms.intern("a");
///
/// let mut sub = Substitution::new();
/// sub.bind(0, Term::constant(a));
///
/// // Applying {X -> a} to X yields a
/// let result = sub.apply_term(&Term::var(0));
/// assert_eq!(result, Term::constant(a));
/// ```
#[derive(Clone, Debug, Default)]
pub struct Substitution {
    bindings: HashMap<VarId, Term>,
}

impl Substitution {
    /// Creates an empty substitution.
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    /// Creates a substitution from a single binding.
    pub fn singleton(var: VarId, term: Term) -> Self {
        let mut s = Self::new();
        s.bind(var, term);
        s
    }

    /// Binds a variable to a term.
    ///
    /// If the variable was already bound, the old binding is replaced.
    pub fn bind(&mut self, var: VarId, term: Term) {
        self.bindings.insert(var, term);
    }

    /// Returns the term bound to the given variable, if any.
    pub fn lookup(&self, var: VarId) -> Option<&Term> {
        self.bindings.get(&var)
    }

    /// Returns `true` if this substitution has no bindings.
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Returns the number of bindings.
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Iterates over all (variable, term) bindings.
    pub fn iter(&self) -> impl Iterator<Item = (&VarId, &Term)> {
        self.bindings.iter()
    }

    /// Applies this substitution to a term, returning the result.
    ///
    /// Variables not in the substitution are left unchanged.
    /// Variable chains are followed transitively: if X→Y and Y→a,
    /// then applying to X yields a (not Y).
    pub fn apply_term(&self, term: &Term) -> Term {
        match term {
            Term::Var(v) => match self.bindings.get(v) {
                Some(t) => self.apply_term(t), // chase variable chains
                None => term.clone(),
            },
            Term::App(f, args) => {
                let new_args: Vec<Term> = args.iter().map(|a| self.apply_term(a)).collect();
                Term::App(*f, new_args)
            }
        }
    }

    /// Applies this substitution to an atom.
    pub fn apply_atom(&self, atom: &Atom) -> Atom {
        match atom {
            Atom::Pred(p, args) => {
                let new_args: Vec<Term> = args.iter().map(|a| self.apply_term(a)).collect();
                Atom::Pred(*p, new_args)
            }
            Atom::Eq(l, r) => Atom::Eq(self.apply_term(l), self.apply_term(r)),
        }
    }

    /// Applies this substitution to a literal.
    pub fn apply_literal(&self, lit: &Literal) -> Literal {
        Literal {
            positive: lit.positive,
            atom: self.apply_atom(&lit.atom),
        }
    }

    /// Applies this substitution to a formula.
    pub fn apply_formula(&self, formula: &Formula) -> Formula {
        match formula {
            Formula::Atom(a) => Formula::Atom(self.apply_atom(a)),
            Formula::Neg(f) => Formula::Neg(Box::new(self.apply_formula(f))),
            Formula::And(fs) => Formula::And(fs.iter().map(|f| self.apply_formula(f)).collect()),
            Formula::Or(fs) => Formula::Or(fs.iter().map(|f| self.apply_formula(f)).collect()),
            Formula::Implies(a, b) => Formula::Implies(
                Box::new(self.apply_formula(a)),
                Box::new(self.apply_formula(b)),
            ),
            Formula::Iff(a, b) => Formula::Iff(
                Box::new(self.apply_formula(a)),
                Box::new(self.apply_formula(b)),
            ),
            Formula::Forall(v, body) => {
                // If the bound variable is in the substitution, skip it.
                // (Proper capture-avoidance would require alpha-renaming,
                // but for clausification we always use globally unique vars.)
                if self.bindings.contains_key(v) {
                    // Shadowed: don't substitute inside
                    Formula::Forall(*v, body.clone())
                } else {
                    Formula::Forall(*v, Box::new(self.apply_formula(body)))
                }
            }
            Formula::Exists(v, body) => {
                if self.bindings.contains_key(v) {
                    Formula::Exists(*v, body.clone())
                } else {
                    Formula::Exists(*v, Box::new(self.apply_formula(body)))
                }
            }
            Formula::True => Formula::True,
            Formula::False => Formula::False,
        }
    }

    /// Composes this substitution with another: applies `other` after `self`.
    ///
    /// The result σ∘ρ satisfies: (σ∘ρ)(t) = ρ(σ(t)) for any term t.
    pub fn compose(&self, other: &Substitution) -> Substitution {
        let mut result = Substitution::new();

        // Apply `other` to each binding in `self`
        for (&var, term) in &self.bindings {
            result.bind(var, other.apply_term(term));
        }

        // Add bindings from `other` that aren't in `self`
        for (&var, term) in &other.bindings {
            if !self.bindings.contains_key(&var) {
                result.bind(var, term.clone());
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::SymbolTable;

    #[test]
    fn apply_to_var() {
        let mut syms = SymbolTable::new();
        let a = syms.intern("a");

        let mut sub = Substitution::new();
        sub.bind(0, Term::constant(a));

        assert_eq!(sub.apply_term(&Term::var(0)), Term::constant(a));
        // Unbound variable stays unchanged
        assert_eq!(sub.apply_term(&Term::var(1)), Term::var(1));
    }

    #[test]
    fn apply_to_compound() {
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let a = syms.intern("a");

        let mut sub = Substitution::new();
        sub.bind(0, Term::constant(a));

        // f(X) with {X -> a} yields f(a)
        let input = Term::app(f, vec![Term::var(0)]);
        let expected = Term::app(f, vec![Term::constant(a)]);
        assert_eq!(sub.apply_term(&input), expected);
    }

    #[test]
    fn composition() {
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let a = syms.intern("a");

        // σ = {X -> Y}, ρ = {Y -> a}
        let sigma = Substitution::singleton(0, Term::var(1));
        let rho = Substitution::singleton(1, Term::constant(a));

        let composed = sigma.compose(&rho);

        // (σ∘ρ)(X) = ρ(σ(X)) = ρ(Y) = a
        assert_eq!(composed.apply_term(&Term::var(0)), Term::constant(a));
        // (σ∘ρ)(Y) = ρ(Y) = a  (from rho, since Y not in sigma's domain maps don't shadow)
        assert_eq!(composed.apply_term(&Term::var(1)), Term::constant(a));

        // (σ∘ρ)(f(X)) = f(a)
        let input = Term::app(f, vec![Term::var(0)]);
        let expected = Term::app(f, vec![Term::constant(a)]);
        assert_eq!(composed.apply_term(&input), expected);
    }
}
