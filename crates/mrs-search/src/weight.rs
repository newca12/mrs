//! Clause weight heuristics.
//!
//! Weight functions assign a numeric cost to clauses, used by the clause
//! selection strategy to prefer simpler (lighter) clauses during proof search.
//!
//! The standard weight counts each symbol occurrence (function symbols and
//! variables) as 1, summing over all literals.

use mrs_core::clause::{Clause, Literal};
use mrs_core::formula::Atom;
use mrs_core::term::Term;

/// Returns the weight of a clause: the sum of symbol occurrences across all literals.
///
/// Lighter clauses are generally preferred because they represent simpler facts.
pub fn clause_weight(clause: &Clause) -> u32 {
    clause.literals.iter().map(literal_weight).sum()
}

/// Returns the weight of a single literal.
fn literal_weight(lit: &Literal) -> u32 {
    atom_weight(&lit.atom)
}

/// Returns the weight of an atom.
fn atom_weight(atom: &Atom) -> u32 {
    match atom {
        Atom::Pred(_, args) => 1 + args.iter().map(term_weight).sum::<u32>(),
        Atom::Eq(l, r) => term_weight(l) + term_weight(r),
    }
}

/// Returns the weight of a term: 1 per symbol/variable occurrence.
fn term_weight(term: &Term) -> u32 {
    match term {
        Term::Var(_) => 1,
        Term::App(_, args) => 1 + args.iter().map(term_weight).sum::<u32>(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::SymbolTable;
    use mrs_core::clause::{ClauseId, ClauseSource};

    fn make_clause(lits: Vec<Literal>) -> Clause {
        Clause::new(
            ClauseId(0),
            lits,
            ClauseSource::Input {
                name: "test".into(),
                role: "axiom".into(),
            },
        )
    }

    #[test]
    fn weight_of_empty_clause() {
        let c = make_clause(vec![]);
        assert_eq!(clause_weight(&c), 0);
    }

    #[test]
    fn weight_of_propositional_literal() {
        // p() -> predicate symbol counts as 1
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let c = make_clause(vec![Literal::pos(Atom::prop(p))]);
        assert_eq!(clause_weight(&c), 1);
    }

    #[test]
    fn weight_of_unary_predicate() {
        // p(a) -> p=1, a=1 -> weight 2
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let c = make_clause(vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))]);
        assert_eq!(clause_weight(&c), 2);
    }

    #[test]
    fn weight_of_nested_term() {
        // p(f(a, X)) -> p=1, f=1, a=1, X=1 -> weight 4
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let f = syms.intern("f");
        let a = syms.intern("a");
        let c = make_clause(vec![Literal::pos(Atom::pred(
            p,
            vec![Term::app(f, vec![Term::constant(a), Term::var(0)])],
        ))]);
        assert_eq!(clause_weight(&c), 4);
    }

    #[test]
    fn weight_of_equality() {
        // a = b -> weight 2 (two constants, no predicate symbol counted)
        let mut syms = SymbolTable::new();
        let a = syms.intern("a");
        let b = syms.intern("b");
        let c = make_clause(vec![Literal::pos(Atom::eq(
            Term::constant(a),
            Term::constant(b),
        ))]);
        assert_eq!(clause_weight(&c), 2);
    }

    #[test]
    fn weight_of_multi_literal_clause() {
        // p(a) | q(X, b) -> p=1 + a=1 + q=1 + X=1 + b=1 = 5
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let c = make_clause(vec![
            Literal::pos(Atom::pred(p, vec![Term::constant(a)])),
            Literal::pos(Atom::pred(q, vec![Term::var(0), Term::constant(b)])),
        ]);
        assert_eq!(clause_weight(&c), 5);
    }

    #[test]
    fn weight_negative_same_as_positive() {
        // Weight doesn't depend on polarity
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let pos = make_clause(vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))]);
        let neg = make_clause(vec![Literal::neg(Atom::pred(p, vec![Term::constant(a)]))]);
        assert_eq!(clause_weight(&pos), clause_weight(&neg));
    }
}
