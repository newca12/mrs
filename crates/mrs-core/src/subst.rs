//! Substitutions: mappings from variables to terms.
//!
//! A [`Substitution`] maps variable IDs to terms. Applying a substitution to a
//! term replaces each mapped variable with its corresponding term.
//! Substitutions are the core mechanism for unification and inference.

use crate::HashMap;

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
            bindings: HashMap::default(),
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
    ///
    /// Variable chaining is done iteratively to avoid stack overflow on long
    /// (but acyclic) chains.  If a cycle is ever detected in the variable
    /// portion of the chain, this function panics in debug mode (it indicates
    /// a bug in the code that built the substitution) and returns the looping
    /// variable in release mode.
    pub fn apply_term(&self, term: &Term) -> Term {
        if self.bindings.is_empty() {
            return term.clone();
        }
        self.apply_term_opt(term).unwrap_or_else(|| term.clone())
    }

    fn apply_term_opt(&self, term: &Term) -> Option<Term> {
        match term {
            Term::Var(start) => {
                let mut current = *start;
                let mut steps: u32 = 0;
                let mut changed = false;
                loop {
                    match self.bindings.get(&current) {
                        None => {
                            if changed {
                                return Some(Term::Var(current));
                            } else {
                                return None;
                            }
                        }
                        Some(Term::Var(next)) => {
                            steps += 1;
                            debug_assert!(
                                steps < 100_000,
                                "apply_term: variable chain exceeded 100,000 steps — \
                                 likely a cycle. substitution = {:?}",
                                self.bindings
                            );
                            current = *next;
                            changed = true;
                        }
                        Some(t) => {
                            return Some(self.apply_term_opt(t).unwrap_or_else(|| t.clone()));
                        }
                    }
                }
            }
            Term::App(f, args) => {
                let mut new_args: Option<Vec<Term>> = None;
                for (i, arg) in args.iter().enumerate() {
                    if let Some(new_arg) = self.apply_term_opt(arg) {
                        if new_args.is_none() {
                            let mut v = Vec::with_capacity(args.len());
                            v.extend(args.iter().take(i).cloned());
                            new_args = Some(v);
                        }
                        new_args.as_mut().unwrap().push(new_arg);
                    } else {
                        if let Some(v) = new_args.as_mut() {
                            v.push(arg.clone());
                        }
                    }
                }
                new_args.map(|na| Term::App(*f, na))
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

    /// Returns true if any of the target terms in this substitution contains the variable `v`.
    pub fn captures_var(&self, v: VarId) -> bool {
        for term in self.bindings.values() {
            if term.contains_var(v) {
                return true;
            }
        }
        false
    }

    fn max_var_id(&self, formula: &Formula) -> VarId {
        let mut max_val = 0;
        for (&v, term) in &self.bindings {
            max_val = max_val.max(v);
            let mut term_vars = crate::HashSet::default();
            term.collect_vars(&mut term_vars);
            for &tv in &term_vars {
                max_val = max_val.max(tv);
            }
        }
        let mut formula_vars = crate::HashSet::default();
        collect_all_vars(formula, &mut formula_vars);
        for &fv in &formula_vars {
            max_val = max_val.max(fv);
        }
        max_val
    }

    /// Applies this substitution to a formula.
    pub fn apply_formula(&self, formula: &Formula) -> Formula {
        let mut next_var = self.max_var_id(formula) + 1;
        self.apply_formula_rec(formula, &mut next_var)
    }

    fn apply_formula_rec(&self, formula: &Formula, next_var: &mut VarId) -> Formula {
        match formula {
            Formula::Atom(a) => Formula::Atom(self.apply_atom(a)),
            Formula::Neg(f) => Formula::Neg(Box::new(self.apply_formula_rec(f, next_var))),
            Formula::And(fs) => Formula::And(
                fs.iter()
                    .map(|f| self.apply_formula_rec(f, next_var))
                    .collect(),
            ),
            Formula::Or(fs) => Formula::Or(
                fs.iter()
                    .map(|f| self.apply_formula_rec(f, next_var))
                    .collect(),
            ),
            Formula::Implies(a, b) => Formula::Implies(
                Box::new(self.apply_formula_rec(a, next_var)),
                Box::new(self.apply_formula_rec(b, next_var)),
            ),
            Formula::Iff(a, b) => Formula::Iff(
                Box::new(self.apply_formula_rec(a, next_var)),
                Box::new(self.apply_formula_rec(b, next_var)),
            ),
            Formula::Forall(v, body) => {
                if self.bindings.contains_key(v) {
                    // Shadowed: don't substitute inside
                    Formula::Forall(*v, body.clone())
                } else if self.captures_var(*v) {
                    // Avoid variable capture: alpha-rename bound variable v to a fresh one
                    let v_fresh = *next_var;
                    *next_var += 1;
                    let renamed_body = rename_var(body, *v, v_fresh);
                    Formula::Forall(
                        v_fresh,
                        Box::new(self.apply_formula_rec(&renamed_body, next_var)),
                    )
                } else {
                    Formula::Forall(*v, Box::new(self.apply_formula_rec(body, next_var)))
                }
            }
            Formula::Exists(v, body) => {
                if self.bindings.contains_key(v) {
                    Formula::Exists(*v, body.clone())
                } else if self.captures_var(*v) {
                    // Avoid variable capture: alpha-rename bound variable v to a fresh one
                    let v_fresh = *next_var;
                    *next_var += 1;
                    let renamed_body = rename_var(body, *v, v_fresh);
                    Formula::Exists(
                        v_fresh,
                        Box::new(self.apply_formula_rec(&renamed_body, next_var)),
                    )
                } else {
                    Formula::Exists(*v, Box::new(self.apply_formula_rec(body, next_var)))
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

fn collect_all_vars(formula: &Formula, vars: &mut crate::HashSet<VarId>) {
    match formula {
        Formula::Atom(a) => {
            a.collect_vars(vars);
        }
        Formula::Neg(f) => collect_all_vars(f, vars),
        Formula::And(fs) | Formula::Or(fs) => {
            for f in fs {
                collect_all_vars(f, vars);
            }
        }
        Formula::Implies(a, b) | Formula::Iff(a, b) => {
            collect_all_vars(a, vars);
            collect_all_vars(b, vars);
        }
        Formula::Forall(v, body) | Formula::Exists(v, body) => {
            vars.insert(*v);
            collect_all_vars(body, vars);
        }
        Formula::True | Formula::False => {}
    }
}

fn rename_var(formula: &Formula, old: VarId, new: VarId) -> Formula {
    match formula {
        Formula::Atom(a) => Formula::Atom(rename_atom(a, old, new)),
        Formula::Neg(f) => Formula::Neg(Box::new(rename_var(f, old, new))),
        Formula::And(fs) => Formula::And(fs.iter().map(|f| rename_var(f, old, new)).collect()),
        Formula::Or(fs) => Formula::Or(fs.iter().map(|f| rename_var(f, old, new)).collect()),
        Formula::Implies(a, b) => Formula::Implies(
            Box::new(rename_var(a, old, new)),
            Box::new(rename_var(b, old, new)),
        ),
        Formula::Iff(a, b) => Formula::Iff(
            Box::new(rename_var(a, old, new)),
            Box::new(rename_var(b, old, new)),
        ),
        Formula::Forall(v, body) => {
            if *v == old {
                Formula::Forall(*v, body.clone())
            } else {
                Formula::Forall(*v, Box::new(rename_var(body, old, new)))
            }
        }
        Formula::Exists(v, body) => {
            if *v == old {
                Formula::Exists(*v, body.clone())
            } else {
                Formula::Exists(*v, Box::new(rename_var(body, old, new)))
            }
        }
        Formula::True => Formula::True,
        Formula::False => Formula::False,
    }
}

fn rename_atom(atom: &Atom, old: VarId, new: VarId) -> Atom {
    match atom {
        Atom::Pred(p, args) => {
            let new_args = args.iter().map(|arg| rename_term(arg, old, new)).collect();
            Atom::Pred(*p, new_args)
        }
        Atom::Eq(l, r) => Atom::Eq(rename_term(l, old, new), rename_term(r, old, new)),
    }
}

fn rename_term(term: &Term, old: VarId, new: VarId) -> Term {
    match term {
        Term::Var(v) => {
            if *v == old {
                Term::Var(new)
            } else {
                Term::Var(*v)
            }
        }
        Term::App(f, args) => {
            let new_args = args.iter().map(|arg| rename_term(arg, old, new)).collect();
            Term::App(*f, new_args)
        }
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

    #[test]
    fn capture_avoidance_renaming() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let f = syms.intern("f");

        // We want to apply {Y -> f(X)} to: ∃X. (p(Y) ∧ q(X))
        // Bound variable of Exists is X (VarId 0).
        // Replacement term f(X) contains X (VarId 0) as a free variable.
        // Therefore, we must alpha-rename X inside the quantifier to a fresh one (e.g. VarId 2).
        // Result should be: ∃Z. (p(f(X)) ∧ q(Z)) where Z is VarId 2.
        let sub = Substitution::singleton(1, Term::app(f, vec![Term::var(0)])); // {1 -> f(0)}

        let body = Formula::and(vec![
            Formula::atom(Atom::pred(p, vec![Term::var(1)])), // p(1)
            Formula::atom(Atom::pred(q, vec![Term::var(0)])), // q(0)
        ]);
        let formula = Formula::exists(0, body); // ∃0. (p(1) ∧ q(0))

        let result = sub.apply_formula(&formula);

        // Verify that the bound variable was renamed to 2 (since max of 0, 1 and term 0 is 1, so fresh is 2)
        use crate::display::DisplayWithSymbols;
        let formatted = format!("{}", result.display(&syms));
        assert_eq!(formatted, "?[X2]: ((p(f(X0)) & q(X2)))");
    }
}
