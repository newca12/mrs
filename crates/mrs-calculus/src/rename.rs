//! Variable renaming for clause standardization apart.
//!
//! Before resolving two clauses, their variables must be disjoint.
//! This module provides utilities to rename all variables in a clause
//! by applying a fixed offset.

use mrs_core::Atom;
use mrs_core::clause::{Clause, Literal};
use mrs_core::term::Term;
use mrs_core::term::VarId;

/// Returns the maximum VarId used in a clause, plus 1.
///
/// Returns 0 if the clause has no variables.
/// This is used as the offset for renaming a second clause's variables.
pub fn max_var(clause: &Clause) -> VarId {
    clause.free_vars().into_iter().max().map_or(0, |m| m + 1)
}

/// Renames all variables in a clause by adding `offset` to each VarId.
///
/// The clause's `id` and `source` are preserved.
/// This is used to make two clauses variable-disjoint before resolution.
///
/// Uses direct traversal rather than `Substitution::apply_term` to avoid
/// transitive chain resolution. With consecutive VarIds {0,1,2} and offset 1,
/// a substitution {0→Var(1), 1→Var(2), 2→Var(3)} would cause apply_term to
/// resolve Var(0)→Var(1)→Var(2)→Var(3), collapsing all variables to Var(3).
pub fn rename_clause(clause: &Clause, offset: VarId) -> Clause {
    if offset == 0 {
        return clause.clone();
    }

    let new_lits = clause
        .literals
        .iter()
        .map(|l| rename_literal(l, offset))
        .collect();

    Clause::new(clause.id, new_lits, clause.source.clone())
}

fn rename_term(term: &Term, offset: VarId) -> Term {
    match term {
        Term::Var(v) => Term::Var(v + offset),
        Term::App(f, args) => Term::App(*f, args.iter().map(|a| rename_term(a, offset)).collect()),
    }
}

fn rename_literal(lit: &Literal, offset: VarId) -> Literal {
    let new_atom = match &lit.atom {
        Atom::Pred(p, args) => {
            Atom::Pred(*p, args.iter().map(|a| rename_term(a, offset)).collect())
        }
        Atom::Eq(l, r) => Atom::Eq(rename_term(l, offset), rename_term(r, offset)),
    };
    Literal {
        positive: lit.positive,
        atom: new_atom,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::clause::{ClauseId, ClauseSource};
    use mrs_core::{Atom, Literal, SymbolTable, Term};

    fn input_clause(id: u64, lits: Vec<Literal>) -> Clause {
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
    fn max_var_empty_clause() {
        let c = input_clause(0, vec![]);
        assert_eq!(max_var(&c), 0);
    }

    #[test]
    fn max_var_ground_clause() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let c = input_clause(
            0,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
        );
        assert_eq!(max_var(&c), 0);
    }

    #[test]
    fn max_var_with_vars() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let c = input_clause(
            0,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(0)])),
                Literal::neg(Atom::pred(p, vec![Term::var(3)])),
            ],
        );
        assert_eq!(max_var(&c), 4); // max is 3, so +1 = 4
    }

    #[test]
    fn rename_zero_offset() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let c = input_clause(0, vec![Literal::pos(Atom::pred(p, vec![Term::var(0)]))]);
        let renamed = rename_clause(&c, 0);
        assert_eq!(renamed.literals, c.literals);
    }

    #[test]
    fn rename_shifts_vars() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let c = input_clause(
            0,
            vec![Literal::pos(Atom::pred(
                p,
                vec![Term::var(0), Term::var(1)],
            ))],
        );
        let renamed = rename_clause(&c, 10);

        let expected = input_clause(
            0,
            vec![Literal::pos(Atom::pred(
                p,
                vec![Term::var(10), Term::var(11)],
            ))],
        );
        assert_eq!(renamed.literals, expected.literals);
    }

    #[test]
    fn rename_preserves_constants() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let c = input_clause(
            0,
            vec![Literal::pos(Atom::pred(
                p,
                vec![Term::var(0), Term::constant(a)],
            ))],
        );
        let renamed = rename_clause(&c, 5);

        let expected = input_clause(
            0,
            vec![Literal::pos(Atom::pred(
                p,
                vec![Term::var(5), Term::constant(a)],
            ))],
        );
        assert_eq!(renamed.literals, expected.literals);
    }

    #[test]
    fn rename_preserves_source() {
        let c = input_clause(42, vec![]);
        let renamed = rename_clause(&c, 10);
        assert_eq!(renamed.id, ClauseId(42));
    }

    #[test]
    fn rename_consecutive_vars_no_collapse() {
        // Regression test: vars {0,1,2} with offset 1 should become {1,2,3},
        // NOT all collapse to 3 (which happened when using Substitution.apply_term
        // due to transitive chain resolution).
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let c = input_clause(
            0,
            vec![Literal::pos(Atom::pred(
                f,
                vec![Term::var(0), Term::var(1), Term::var(2)],
            ))],
        );
        let renamed = rename_clause(&c, 1);
        let expected = input_clause(
            0,
            vec![Literal::pos(Atom::pred(
                f,
                vec![Term::var(1), Term::var(2), Term::var(3)],
            ))],
        );
        assert_eq!(renamed.literals, expected.literals);
    }
}
