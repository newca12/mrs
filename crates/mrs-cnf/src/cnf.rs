//! CNF conversion: distribute OR over AND.
//!
//! After NNF and Skolemization, the formula contains only ∧, ∨, and literals
//! (possibly with universal quantifiers, which are stripped before this stage).
//!
//! CNF conversion distributes OR over AND using the rule:
//!   `φ ∨ (ψ ∧ χ)` → `(φ ∨ ψ) ∧ (φ ∨ χ)`
//!
//! This can cause exponential blowup in the worst case.
//! A future improvement would add definitional (Tseitin) CNF.

use mrs_core::Formula;

/// Converts a quantifier-free NNF formula to Conjunctive Normal Form.
///
/// The result is a formula where AND is at the top level and OR is nested inside.
/// That is: `∧ᵢ (∨ⱼ Lᵢⱼ)` where each `Lᵢⱼ` is a literal.
pub fn to_cnf(formula: &Formula) -> Formula {
    let flat = flatten_connectives(formula);
    distribute(&flat)
}

/// Flattens nested AND/OR:
/// `(A ∧ B) ∧ C` → `A ∧ B ∧ C`
fn flatten_connectives(formula: &Formula) -> Formula {
    match formula {
        Formula::And(cs) => {
            let mut flat = Vec::new();
            for c in cs {
                let fc = flatten_connectives(c);
                if let Formula::And(inner) = fc {
                    flat.extend(inner);
                } else {
                    flat.push(fc);
                }
            }
            Formula::And(flat)
        }
        Formula::Or(ds) => {
            let mut flat = Vec::new();
            for d in ds {
                let fd = flatten_connectives(d);
                if let Formula::Or(inner) = fd {
                    flat.extend(inner);
                } else {
                    flat.push(fd);
                }
            }
            Formula::Or(flat)
        }
        Formula::Neg(inner) => Formula::Neg(Box::new(flatten_connectives(inner))),
        other => other.clone(),
    }
}

/// Distributes OR over AND to produce CNF.
fn distribute(formula: &Formula) -> Formula {
    match formula {
        Formula::And(cs) => {
            // Recursively distribute each conjunct, then collect
            let cnf_parts: Vec<Formula> = cs.iter().map(distribute).collect();
            // Flatten nested ANDs
            let mut result = Vec::new();
            for part in cnf_parts {
                if let Formula::And(inner) = part {
                    result.extend(inner);
                } else {
                    result.push(part);
                }
            }
            Formula::And(result)
        }

        Formula::Or(ds) => {
            // First distribute each disjunct
            let distributed: Vec<Formula> = ds.iter().map(distribute).collect();

            // Find all conjuncts within this disjunction
            // e.g. (A ∧ B) ∨ C → (A ∨ C) ∧ (B ∨ C)
            distribute_or_list(&distributed)
        }

        // Atoms and negated atoms are already in CNF
        other => other.clone(),
    }
}

/// Distributes across a list of disjuncts.
/// If any disjunct is a conjunction, we need to expand.
fn distribute_or_list(disjuncts: &[Formula]) -> Formula {
    if disjuncts.is_empty() {
        return Formula::False;
    }
    if disjuncts.len() == 1 {
        return disjuncts[0].clone();
    }

    // Split into first element and rest, distribute recursively
    let first = &disjuncts[0];
    let rest = distribute_or_list(&disjuncts[1..]);

    distribute_or_pair(first, &rest)
}

/// Distributes OR over a pair: `A ∨ B`
/// If A = (A1 ∧ A2), result is (A1 ∨ B) ∧ (A2 ∨ B)
/// If B = (B1 ∧ B2), result is (A ∨ B1) ∧ (A ∨ B2)
fn distribute_or_pair(a: &Formula, b: &Formula) -> Formula {
    match (a, b) {
        (Formula::And(cs), _) => {
            let mut parts = Vec::new();
            for c in cs {
                let r = distribute_or_pair(c, b);
                // Flatten nested ANDs
                if let Formula::And(inner) = r {
                    parts.extend(inner);
                } else {
                    parts.push(r);
                }
            }
            Formula::And(parts)
        }
        (_, Formula::And(cs)) => {
            let mut parts = Vec::new();
            for c in cs {
                let r = distribute_or_pair(a, c);
                if let Formula::And(inner) = r {
                    parts.extend(inner);
                } else {
                    parts.push(r);
                }
            }
            Formula::And(parts)
        }
        _ => {
            // Neither is a conjunction: just form the disjunction
            let mut ds = Vec::new();
            if let Formula::Or(d1) = a {
                ds.extend(d1.iter().cloned());
            } else {
                ds.push(a.clone());
            }
            if let Formula::Or(d2) = b {
                ds.extend(d2.iter().cloned());
            } else {
                ds.push(b.clone());
            }
            Formula::Or(ds)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::display::DisplayWithSymbols;
    use mrs_core::{Atom, SymbolTable};

    fn fmt(f: &Formula, syms: &SymbolTable) -> String {
        format!("{}", f.display(syms))
    }

    fn atom(syms: &mut SymbolTable, name: &str) -> Formula {
        let s = syms.intern(name);
        Formula::atom(Atom::prop(s))
    }

    #[test]
    fn cnf_already_cnf() {
        let mut syms = SymbolTable::new();
        // (p ∨ q) is already CNF
        let f = Formula::or(vec![atom(&mut syms, "p"), atom(&mut syms, "q")]);
        let result = to_cnf(&f);
        assert_eq!(fmt(&result, &syms), "(p | q)");
    }

    #[test]
    fn cnf_distribute() {
        let mut syms = SymbolTable::new();
        // p ∨ (q ∧ r) → (p ∨ q) ∧ (p ∨ r)
        let f = Formula::or(vec![
            atom(&mut syms, "p"),
            Formula::and(vec![atom(&mut syms, "q"), atom(&mut syms, "r")]),
        ]);
        let result = to_cnf(&f);
        assert_eq!(fmt(&result, &syms), "((p | q) & (p | r))");
    }

    #[test]
    fn cnf_conjunction() {
        let mut syms = SymbolTable::new();
        // p ∧ q is already CNF
        let f = Formula::and(vec![atom(&mut syms, "p"), atom(&mut syms, "q")]);
        let result = to_cnf(&f);
        assert_eq!(fmt(&result, &syms), "(p & q)");
    }

    #[test]
    fn cnf_complex() {
        let mut syms = SymbolTable::new();
        // (p ∧ q) ∨ r → (p ∨ r) ∧ (q ∨ r)
        let f = Formula::or(vec![
            Formula::and(vec![atom(&mut syms, "p"), atom(&mut syms, "q")]),
            atom(&mut syms, "r"),
        ]);
        let result = to_cnf(&f);
        assert_eq!(fmt(&result, &syms), "((p | r) & (q | r))");
    }

    #[test]
    fn cnf_double_distribute() {
        let mut syms = SymbolTable::new();
        // (p ∧ q) ∨ (r ∧ s) → (p∨r) ∧ (p∨s) ∧ (q∨r) ∧ (q∨s)
        let f = Formula::or(vec![
            Formula::and(vec![atom(&mut syms, "p"), atom(&mut syms, "q")]),
            Formula::and(vec![atom(&mut syms, "r"), atom(&mut syms, "s")]),
        ]);
        let result = to_cnf(&f);
        // Should have 4 clauses
        if let Formula::And(clauses) = &result {
            assert_eq!(clauses.len(), 4);
        } else {
            panic!("expected And, got {:?}", result);
        }
    }
}
