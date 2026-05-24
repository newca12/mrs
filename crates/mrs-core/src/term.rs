//! First-order terms: variables and function applications.
//!
//! A [`Term`] is either a variable or a function symbol applied to zero or more
//! argument terms. Constants are represented as nullary function applications.
//!
//! Variables are identified by unique integer IDs ([`VarId`]) rather than names,
//! which simplifies variable renaming during inference.

use std::collections::HashSet;

use crate::symbol::SymbolId;

/// A variable identifier. Each variable in a clause should have a unique ID.
pub type VarId = u32;

/// A first-order term.
///
/// # Examples
///
/// ```
/// use mrs_core::term::{Term, VarId};
/// use mrs_core::symbol::{SymbolId, SymbolTable};
///
/// let mut syms = SymbolTable::new();
/// let f = syms.intern("f");
/// let a = syms.intern("a");
///
/// // The constant `a`
/// let term_a = Term::constant(a);
///
/// // The term `f(X, a)` where X is variable 0
/// let term = Term::app(f, vec![Term::var(0), term_a]);
/// ```
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum Term {
    /// A variable, identified by a unique integer.
    Var(VarId),
    /// A function application `f(t1, ..., tn)`. Constants have an empty argument list.
    App(SymbolId, Vec<Term>),
}

impl Term {
    /// Creates a variable term.
    pub fn var(id: VarId) -> Self {
        Term::Var(id)
    }

    /// Creates a function application.
    pub fn app(symbol: SymbolId, args: Vec<Term>) -> Self {
        Term::App(symbol, args)
    }

    /// Creates a constant (nullary function application).
    pub fn constant(symbol: SymbolId) -> Self {
        Term::App(symbol, Vec::new())
    }

    /// Returns `true` if this is a variable.
    pub fn is_var(&self) -> bool {
        matches!(self, Term::Var(_))
    }

    /// Returns `true` if this is a constant (function application with no arguments).
    pub fn is_constant(&self) -> bool {
        matches!(self, Term::App(_, args) if args.is_empty())
    }

    /// Collects all free variable IDs occurring in this term.
    pub fn free_vars(&self) -> HashSet<VarId> {
        let mut vars = HashSet::new();
        self.collect_vars(&mut vars);
        vars
    }

    /// Collects variable IDs into the given set.
    pub fn collect_vars(&self, vars: &mut HashSet<VarId>) {
        match self {
            Term::Var(v) => {
                vars.insert(*v);
            }
            Term::App(_, args) => {
                for arg in args {
                    arg.collect_vars(vars);
                }
            }
        }
    }

    /// Returns `true` if the given variable occurs in this term.
    pub fn contains_var(&self, var: VarId) -> bool {
        match self {
            Term::Var(v) => *v == var,
            Term::App(_, args) => args.iter().any(|a| a.contains_var(var)),
        }
    }

    /// Returns the depth of this term (variables and constants have depth 0).
    pub fn depth(&self) -> usize {
        match self {
            Term::Var(_) => 0,
            Term::App(_, args) => {
                if args.is_empty() {
                    0
                } else {
                    1 + args.iter().map(|a| a.depth()).max().unwrap_or(0)
                }
            }
        }
    }

    /// Returns the total number of symbols (variables + function symbols) in this term.
    pub fn size(&self) -> usize {
        match self {
            Term::Var(_) => 1,
            Term::App(_, args) => 1 + args.iter().map(|a| a.size()).sum::<usize>(),
        }
    }

    /// Returns the subterm at the given position, or `None` if the position is invalid.
    ///
    /// A position is a sequence of argument indices from the root.
    /// The empty slice `&[]` refers to the term itself.
    pub fn subterm_at(&self, pos: &[usize]) -> Option<&Term> {
        if pos.is_empty() {
            return Some(self);
        }
        match self {
            Term::App(_, args) => {
                if pos[0] < args.len() {
                    args[pos[0]].subterm_at(&pos[1..])
                } else {
                    None
                }
            }
            Term::Var(_) => None,
        }
    }

    /// Replaces the subterm at the given position, returning a new term.
    ///
    /// If the position is empty, returns the replacement itself.
    /// Panics if the position is invalid.
    pub fn replace_at(&self, pos: &[usize], replacement: &Term) -> Term {
        if pos.is_empty() {
            return replacement.clone();
        }
        match self {
            Term::App(f, args) => {
                let mut new_args = args.clone();
                new_args[pos[0]] = args[pos[0]].replace_at(&pos[1..], replacement);
                Term::App(*f, new_args)
            }
            Term::Var(_) => panic!("invalid position: variable has no subterms"),
        }
    }

    /// Returns positions of all non-variable subterms.
    ///
    /// Each position is a `Vec<usize>` path of argument indices from the root.
    /// The root position `vec![]` is always included (unless the term is a variable).
    pub fn non_variable_positions(&self) -> Vec<Vec<usize>> {
        let mut result = Vec::new();
        self.collect_nv_positions(&mut vec![], &mut result);
        result
    }

    fn collect_nv_positions(&self, path: &mut Vec<usize>, result: &mut Vec<Vec<usize>>) {
        match self {
            Term::Var(_) => {} // skip variables
            Term::App(_, args) => {
                result.push(path.clone());
                for (i, arg) in args.iter().enumerate() {
                    path.push(i);
                    arg.collect_nv_positions(path, result);
                    path.pop();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::SymbolTable;

    fn make_symbols() -> (SymbolTable, SymbolId, SymbolId, SymbolId) {
        let mut st = SymbolTable::new();
        let f = st.intern("f");
        let g = st.intern("g");
        let a = st.intern("a");
        (st, f, g, a)
    }

    #[test]
    fn term_construction() {
        let (_st, f, _g, a) = make_symbols();
        let x = Term::var(0);
        assert!(x.is_var());

        let c = Term::constant(a);
        assert!(c.is_constant());

        let t = Term::app(f, vec![x.clone(), c.clone()]);
        assert!(!t.is_var());
        assert!(!t.is_constant());
    }

    #[test]
    fn free_vars() {
        let (_st, f, g, a) = make_symbols();
        // f(X, g(Y, a))
        let t = Term::app(
            f,
            vec![
                Term::var(0),
                Term::app(g, vec![Term::var(1), Term::constant(a)]),
            ],
        );
        let vars = t.free_vars();
        assert_eq!(vars.len(), 2);
        assert!(vars.contains(&0));
        assert!(vars.contains(&1));
    }

    #[test]
    fn contains_var() {
        let (_st, f, _g, a) = make_symbols();
        let t = Term::app(f, vec![Term::var(0), Term::constant(a)]);
        assert!(t.contains_var(0));
        assert!(!t.contains_var(1));
    }

    #[test]
    fn depth_and_size() {
        let (_st, f, g, a) = make_symbols();
        assert_eq!(Term::var(0).depth(), 0);
        assert_eq!(Term::constant(a).depth(), 0);

        // f(a) has depth 1
        let t1 = Term::app(f, vec![Term::constant(a)]);
        assert_eq!(t1.depth(), 1);

        // f(g(a)) has depth 2
        let t2 = Term::app(f, vec![Term::app(g, vec![Term::constant(a)])]);
        assert_eq!(t2.depth(), 2);

        // f(X, a) has size 3
        let t3 = Term::app(f, vec![Term::var(0), Term::constant(a)]);
        assert_eq!(t3.size(), 3);
    }

    #[test]
    fn subterm_at_root() {
        let (_st, f, _g, a) = make_symbols();
        let t = Term::app(f, vec![Term::constant(a)]);
        assert_eq!(t.subterm_at(&[]), Some(&t));
    }

    #[test]
    fn subterm_at_arg() {
        let (_st, f, g, a) = make_symbols();
        // f(g(a), X)
        let ga = Term::app(g, vec![Term::constant(a)]);
        let t = Term::app(f, vec![ga.clone(), Term::var(0)]);
        assert_eq!(t.subterm_at(&[0]), Some(&ga));
        assert_eq!(t.subterm_at(&[1]), Some(&Term::var(0)));
        assert_eq!(t.subterm_at(&[0, 0]), Some(&Term::constant(a)));
        assert_eq!(t.subterm_at(&[2]), None);
    }

    #[test]
    fn subterm_at_var() {
        let x = Term::var(0);
        assert_eq!(x.subterm_at(&[]), Some(&x));
        assert_eq!(x.subterm_at(&[0]), None);
    }

    #[test]
    fn replace_at_root() {
        let (_st, _f, _g, a) = make_symbols();
        let t = Term::var(0);
        let result = t.replace_at(&[], &Term::constant(a));
        assert_eq!(result, Term::constant(a));
    }

    #[test]
    fn replace_at_nested() {
        let (mut st, f, g, a) = make_symbols();
        let b = st.intern("b");
        // f(g(a), X) -> replace at [0, 0] with b -> f(g(b), X)
        let t = Term::app(f, vec![Term::app(g, vec![Term::constant(a)]), Term::var(0)]);
        let result = t.replace_at(&[0, 0], &Term::constant(b));
        let expected = Term::app(f, vec![Term::app(g, vec![Term::constant(b)]), Term::var(0)]);
        assert_eq!(result, expected);
    }

    #[test]
    fn non_variable_positions_constant() {
        let (_st, _f, _g, a) = make_symbols();
        let t = Term::constant(a);
        let positions = t.non_variable_positions();
        assert_eq!(positions, vec![vec![]]);
    }

    #[test]
    fn non_variable_positions_var() {
        let t = Term::var(0);
        let positions = t.non_variable_positions();
        assert!(positions.is_empty());
    }

    #[test]
    fn non_variable_positions_nested() {
        let (_st, f, g, a) = make_symbols();
        // f(g(a), X) -> non-variable positions: [], [0], [0,0]
        let t = Term::app(f, vec![Term::app(g, vec![Term::constant(a)]), Term::var(0)]);
        let positions = t.non_variable_positions();
        assert_eq!(positions, vec![vec![], vec![0], vec![0, 0]]);
    }
}
