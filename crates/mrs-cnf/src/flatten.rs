//! Clause extraction from CNF formulas.
//!
//! After CNF conversion, the formula is a conjunction of disjunctions of literals.
//! This module extracts individual [`Clause`] objects from that structure.

use mrs_core::clause::{Clause, ClauseIdGen, ClauseSource};
use mrs_core::{Formula, Literal};

/// Extracts clauses from a CNF formula.
///
/// The formula should be in CNF (conjunction of disjunctions of literals).
/// Each disjunction becomes a [`Clause`].
pub fn extract_clauses(
    formula: &Formula,
    id_gen: &mut ClauseIdGen,
    name: &str,
    role: &str,
) -> Vec<Clause> {
    let mut clauses = Vec::new();
    extract_rec(formula, id_gen, name, role, &mut clauses);
    clauses
}

fn extract_rec(
    formula: &Formula,
    id_gen: &mut ClauseIdGen,
    name: &str,
    role: &str,
    clauses: &mut Vec<Clause>,
) {
    match formula {
        Formula::And(conjuncts) => {
            for c in conjuncts {
                extract_rec(c, id_gen, name, role, clauses);
            }
        }
        Formula::True => {
            // $true as a top-level formula is trivially satisfied: no clause needed.
        }
        Formula::False => {
            // $false as a top-level formula is the empty clause (immediately unsatisfiable).
            clauses.push(Clause::new_avatar(
                id_gen.next(),
                vec![],
                ClauseSource::Input {
                    name: name.to_string(),
                    role: role.to_string(),
                },
                Vec::new(),
            ));
        }
        _ => {
            // This should be a single clause (disjunction of literals)
            if let Some(literals) = extract_literals(formula) {
                clauses.push(Clause::new_avatar(
                    id_gen.next(),
                    literals,
                    ClauseSource::Input {
                        name: name.to_string(),
                        role: role.to_string(),
                    },
                    Vec::new(),
                ));
            }
            // None means the clause contained $true and is a tautology; skip it.
        }
    }
}

/// Extracts literals from a disjunction (or a single literal).
/// Returns `None` if the clause is a tautology (contains `$true`).
fn extract_literals(formula: &Formula) -> Option<Vec<Literal>> {
    let mut lits = Vec::new();
    if collect_literals(formula, &mut lits) {
        None // tautology
    } else {
        Some(lits)
    }
}

/// Collects literals from `formula` into `lits`.
/// Returns `true` if the formula contains `$true` (making the clause a tautology).
fn collect_literals(formula: &Formula, lits: &mut Vec<Literal>) -> bool {
    match formula {
        Formula::Or(disjuncts) => {
            for d in disjuncts {
                if collect_literals(d, lits) {
                    return true; // tautology: short-circuit
                }
            }
            false
        }
        Formula::Atom(a) => {
            lits.push(Literal::pos(a.clone()));
            false
        }
        Formula::Neg(inner) => {
            if let Formula::Atom(a) = inner.as_ref() {
                lits.push(Literal::neg(a.clone()));
            } else {
                // Should not happen in proper NNF+CNF, but handle gracefully
                // by treating the whole thing as a positive atom would be wrong.
                // For now, panic to surface bugs in the pipeline.
                panic!("collect_literals: negation of non-atom in CNF: {:?}", inner);
            }
            false
        }
        Formula::True => {
            // $true in a disjunction makes the whole clause a tautology.
            true
        }
        Formula::False => {
            // $false in a disjunction contributes nothing (identity for OR).
            false
        }
        other => {
            panic!(
                "collect_literals: unexpected formula in CNF clause: {:?}",
                other
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::display::DisplayWithSymbols;
    use mrs_core::{Atom, SymbolTable, Term};

    #[test]
    fn extract_single_clause() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let mut id_gen = ClauseIdGen::new();

        // p ∨ ¬q
        let f = Formula::or(vec![
            Formula::atom(Atom::pred(p, vec![Term::var(0)])),
            Formula::neg(Formula::atom(Atom::pred(q, vec![Term::var(1)]))),
        ]);

        let clauses = extract_clauses(&f, &mut id_gen, "test", "axiom");
        assert_eq!(clauses.len(), 1);
        assert_eq!(clauses[0].len(), 2);
        let display = format!("{}", clauses[0].display(&syms));
        assert_eq!(display, "p(X0) | ~q(X1)");
    }

    #[test]
    fn extract_multiple_clauses() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let r = syms.intern("r");
        let mut id_gen = ClauseIdGen::new();

        // (p ∨ q) ∧ (¬p ∨ r)
        let f = Formula::and(vec![
            Formula::or(vec![
                Formula::atom(Atom::prop(p)),
                Formula::atom(Atom::prop(q)),
            ]),
            Formula::or(vec![
                Formula::neg(Formula::atom(Atom::prop(p))),
                Formula::atom(Atom::prop(r)),
            ]),
        ]);

        let clauses = extract_clauses(&f, &mut id_gen, "test", "axiom");
        assert_eq!(clauses.len(), 2);
    }

    #[test]
    fn extract_unit_clause() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let mut id_gen = ClauseIdGen::new();

        let f = Formula::atom(Atom::prop(p));
        let clauses = extract_clauses(&f, &mut id_gen, "test", "axiom");
        assert_eq!(clauses.len(), 1);
        assert!(clauses[0].is_unit());
    }
}
