//! Clause simplification.
//!
//! Post-clausification simplification to remove trivially redundant clauses:
//!
//! - **Tautology deletion**: Remove clauses containing `L` and `¬L`
//! - **Duplicate literal removal**: Remove repeated literals within a clause
//! - **True/False handling**: Remove trivially true clauses

use std::collections::HashSet;

use mrs_core::clause::Clause;

/// Simplifies a set of clauses.
///
/// - Removes tautological clauses
/// - Removes duplicate literals within each clause
pub fn simplify_clauses(clauses: Vec<Clause>) -> Vec<Clause> {
    clauses
        .into_iter()
        .filter(|c| !c.is_tautology())
        .map(remove_duplicate_literals)
        .collect()
}

/// Removes duplicate literals from a clause.
fn remove_duplicate_literals(clause: Clause) -> Clause {
    let mut seen = HashSet::new();
    let mut unique_lits = Vec::new();

    for lit in &clause.literals {
        if seen.insert(lit.clone()) {
            unique_lits.push(lit.clone());
        }
    }

    if unique_lits.len() == clause.literals.len() {
        // No duplicates found
        clause
    } else {
        Clause::new(clause.id, unique_lits, clause.source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::clause::{ClauseId, ClauseSource};
    use mrs_core::{Atom, Literal, SymbolTable, Term};

    fn make_clause(id: u64, lits: Vec<Literal>) -> Clause {
        Clause::new(
            ClauseId(id),
            lits,
            ClauseSource::Input {
                name: "test".into(),
                role: "axiom".into(),
            },
        )
    }

    #[test]
    fn remove_tautology() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let atom = Atom::pred(p, vec![Term::var(0)]);

        let clauses = vec![make_clause(
            0,
            vec![Literal::pos(atom.clone()), Literal::neg(atom)],
        )];
        let result = simplify_clauses(clauses);
        assert!(result.is_empty());
    }

    #[test]
    fn keep_non_tautology() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");

        let clauses = vec![make_clause(
            0,
            vec![Literal::pos(Atom::prop(p)), Literal::neg(Atom::prop(q))],
        )];
        let result = simplify_clauses(clauses);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn remove_duplicate_lits() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let atom = Atom::prop(p);

        let clauses = vec![make_clause(
            0,
            vec![Literal::pos(atom.clone()), Literal::pos(atom)],
        )];
        let result = simplify_clauses(clauses);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].len(), 1); // duplicate removed
    }
}
