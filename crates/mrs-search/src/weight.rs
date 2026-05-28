//! Clause weight heuristics.
//!
//! Weight functions assign a numeric cost to clauses, used by the clause
//! selection strategy to prefer simpler (lighter) clauses during proof search.
//!
//! The standard weight counts each symbol occurrence (function symbols and
//! variables) as 1, summing over all literals.

use mrs_calculus::ordering::SymbolConfig;
use mrs_core::clause::{Clause, Literal};
use mrs_core::formula::Atom;
use mrs_core::term::Term;

/// Returns the weight of a clause: the sum of symbol occurrences across all literals.
///
/// Lighter clauses are generally preferred because they represent simpler facts.
///
/// Uses an iterative traversal to avoid stack overflow on deeply nested terms.
pub fn clause_weight(clause: &Clause, config: &SymbolConfig) -> u32 {
    clause
        .literals
        .iter()
        .map(|lit| literal_weight(lit, config))
        .sum()
}

/// Returns true if the clause weight exceeds `max`.
///
/// Bails out early as soon as the running total exceeds the limit, so it is
/// cheaper than computing the full weight when the clause is very heavy.
pub fn clause_weight_exceeds(clause: &Clause, max: u32, config: &SymbolConfig) -> bool {
    let mut total: u32 = 0;
    for lit in &clause.literals {
        total = total.saturating_add(literal_weight(lit, config));
        if total > max {
            return true;
        }
    }
    false
}

/// Returns the weight of a single literal.
fn literal_weight(lit: &Literal, config: &SymbolConfig) -> u32 {
    atom_weight(&lit.atom, config)
}

/// Returns the weight of an atom.
fn atom_weight(atom: &Atom, config: &SymbolConfig) -> u32 {
    match atom {
        Atom::Pred(sym, args) => {
            config.symbol_weight(*sym)
                + args.iter().map(|arg| term_weight(arg, config)).sum::<u32>()
        }
        Atom::Eq(l, r) => term_weight(l, config) + term_weight(r, config),
    }
}

/// Returns the weight of a term using an iterative (explicit-stack) traversal.
///
/// This avoids stack overflow on deeply nested terms that arise during
/// superposition without demodulation simplification.
pub fn term_weight(term: &Term, config: &SymbolConfig) -> u32 {
    let mut stack: Vec<&Term> = vec![term];
    let mut weight: u32 = 0;
    while let Some(t) = stack.pop() {
        match t {
            Term::Var(_) => {
                weight = weight.saturating_add(config.w0);
            }
            Term::App(sym, args) => {
                weight = weight.saturating_add(config.symbol_weight(*sym));
                stack.extend(args.iter());
            }
        }
    }
    weight
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
        .with_distance(0)
    }

    #[test]
    fn weight_of_empty_clause() {
        let c = make_clause(vec![]);
        let config = SymbolConfig::default();
        assert_eq!(clause_weight(&c, &config), 0);
    }

    #[test]
    fn weight_of_propositional_literal() {
        // p() -> predicate symbol counts as 1
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let c = make_clause(vec![Literal::pos(Atom::prop(p))]);
        let config = SymbolConfig::default();
        assert_eq!(clause_weight(&c, &config), 1);
    }

    #[test]
    fn weight_of_unary_predicate() {
        // p(a) -> p=1, a=1 -> weight 2
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let c = make_clause(vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))]);
        let config = SymbolConfig::default();
        assert_eq!(clause_weight(&c, &config), 2);
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
        let config = SymbolConfig::default();
        assert_eq!(clause_weight(&c, &config), 4);
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
        let config = SymbolConfig::default();
        assert_eq!(clause_weight(&c, &config), 2);
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
        let config = SymbolConfig::default();
        assert_eq!(clause_weight(&c, &config), 5);
    }

    #[test]
    fn weight_negative_same_as_positive() {
        // Weight doesn't depend on polarity
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let pos = make_clause(vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))]);
        let neg = make_clause(vec![Literal::neg(Atom::pred(p, vec![Term::constant(a)]))]);
        let config = SymbolConfig::default();
        assert_eq!(clause_weight(&pos, &config), clause_weight(&neg, &config));
    }

    #[test]
    fn weight_exceeds_detects_heavy_clauses() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let c = make_clause(vec![
            Literal::pos(Atom::pred(p, vec![Term::constant(a)])),
            Literal::pos(Atom::pred(p, vec![Term::constant(b)])),
        ]);
        let config = SymbolConfig::default();
        // weight = 4; threshold 3 should trigger, threshold 4 should not
        assert!(clause_weight_exceeds(&c, 3, &config));
        assert!(!clause_weight_exceeds(&c, 4, &config));
    }
}
