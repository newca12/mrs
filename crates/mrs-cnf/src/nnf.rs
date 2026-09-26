//! Negation Normal Form (NNF) conversion.
//!
//! A formula is in NNF when:
//! - Negations are applied only to atomic formulas
//! - The only connectives are ∧, ∨, ∀, ∃ (and negated atoms)
//! - Implications and biconditionals are eliminated
//!
//! ## Transformation Rules
//!
//! - `¬¬φ` → `φ` (double negation elimination)
//! - `¬(φ ∧ ψ)` → `¬φ ∨ ¬ψ` (De Morgan)
//! - `¬(φ ∨ ψ)` → `¬φ ∧ ¬ψ` (De Morgan)
//! - `φ → ψ` → `¬φ ∨ ψ` (implication elimination)
//! - `φ ↔ ψ` → `(¬φ ∨ ψ) ∧ (φ ∨ ¬ψ)` (biconditional elimination)
//! - `¬∀x.φ` → `∃x.¬φ` (quantifier negation)
//! - `¬∃x.φ` → `∀x.¬φ` (quantifier negation)

use mrs_core::Formula;

use std::collections::HashMap;

/// Converts a formula to Negation Normal Form.
///
/// After this transformation:
/// - No `Implies` or `Iff` nodes remain
/// - `Neg` appears only directly around `Atom` nodes
pub fn to_nnf(formula: &Formula) -> Formula {
    let mut cache = HashMap::new();
    nnf(formula, false, &mut cache)
}

/// Core NNF conversion. `negated` tracks whether we're under an odd number of negations.
fn nnf(formula: &Formula, negated: bool, cache: &mut HashMap<(Formula, bool), Formula>) -> Formula {
    let key = (formula.clone(), negated);
    if let Some(res) = cache.get(&key) {
        return res.clone();
    }

    let res = match formula {
        Formula::Atom(a) => {
            if negated {
                Formula::neg(Formula::Atom(a.clone()))
            } else {
                Formula::Atom(a.clone())
            }
        }

        Formula::True => {
            if negated {
                Formula::False
            } else {
                Formula::True
            }
        }

        Formula::False => {
            if negated {
                Formula::True
            } else {
                Formula::False
            }
        }

        Formula::Neg(inner) => {
            // Double negation: flip the polarity
            nnf(inner, !negated, cache)
        }

        Formula::And(conjuncts) => {
            if negated {
                // ¬(φ₁ ∧ ... ∧ φₙ) → ¬φ₁ ∨ ... ∨ ¬φₙ  (De Morgan)
                Formula::or(conjuncts.iter().map(|c| nnf(c, true, cache)).collect())
            } else {
                Formula::and(conjuncts.iter().map(|c| nnf(c, false, cache)).collect())
            }
        }

        Formula::Or(disjuncts) => {
            if negated {
                // ¬(φ₁ ∨ ... ∨ φₙ) → ¬φ₁ ∧ ... ∧ ¬φₙ  (De Morgan)
                Formula::and(disjuncts.iter().map(|d| nnf(d, true, cache)).collect())
            } else {
                Formula::or(disjuncts.iter().map(|d| nnf(d, false, cache)).collect())
            }
        }

        Formula::Implies(a, b) => {
            // φ → ψ ≡ ¬φ ∨ ψ
            if negated {
                // ¬(φ → ψ) ≡ φ ∧ ¬ψ
                Formula::and(vec![nnf(a, false, cache), nnf(b, true, cache)])
            } else {
                Formula::or(vec![nnf(a, true, cache), nnf(b, false, cache)])
            }
        }

        Formula::Iff(a, b) => {
            // φ ↔ ψ ≡ (φ → ψ) ∧ (ψ → φ) ≡ (¬φ ∨ ψ) ∧ (φ ∨ ¬ψ)
            if negated {
                // ¬(φ ↔ ψ) ≡ (φ ∨ ψ) ∧ (¬φ ∨ ¬ψ)
                Formula::and(vec![
                    Formula::or(vec![nnf(a, false, cache), nnf(b, false, cache)]),
                    Formula::or(vec![nnf(a, true, cache), nnf(b, true, cache)]),
                ])
            } else {
                Formula::and(vec![
                    Formula::or(vec![nnf(a, true, cache), nnf(b, false, cache)]),
                    Formula::or(vec![nnf(a, false, cache), nnf(b, true, cache)]),
                ])
            }
        }

        Formula::Forall(v, body) => {
            if negated {
                // ¬∀x.φ ≡ ∃x.¬φ
                Formula::exists(*v, nnf(body, true, cache))
            } else {
                Formula::forall(*v, nnf(body, false, cache))
            }
        }

        Formula::Exists(v, body) => {
            if negated {
                // ¬∃x.φ ≡ ∀x.¬φ
                Formula::forall(*v, nnf(body, true, cache))
            } else {
                Formula::exists(*v, nnf(body, false, cache))
            }
        }
    };

    cache.insert(key, res.clone());
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::display::DisplayWithSymbols;
    use mrs_core::{Atom, SymbolTable, Term};

    fn fmt(f: &Formula, syms: &SymbolTable) -> String {
        format!("{}", f.display(syms))
    }

    #[test]
    fn nnf_double_negation() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        // ¬¬p(a) → p(a)
        let f = Formula::neg(Formula::neg(Formula::atom(Atom::prop(p))));
        let result = to_nnf(&f);
        assert_eq!(fmt(&result, &syms), "p");
    }

    #[test]
    fn nnf_implication() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        // p => q → ¬p ∨ q
        let f = Formula::implies(Formula::atom(Atom::prop(p)), Formula::atom(Atom::prop(q)));
        let result = to_nnf(&f);
        assert_eq!(fmt(&result, &syms), "(~(p) | q)");
    }

    #[test]
    fn nnf_de_morgan_and() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        // ¬(p ∧ q) → ¬p ∨ ¬q
        let f = Formula::neg(Formula::and(vec![
            Formula::atom(Atom::prop(p)),
            Formula::atom(Atom::prop(q)),
        ]));
        let result = to_nnf(&f);
        assert_eq!(fmt(&result, &syms), "(~(p) | ~(q))");
    }

    #[test]
    fn nnf_de_morgan_or() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        // ¬(p ∨ q) → ¬p ∧ ¬q
        let f = Formula::neg(Formula::or(vec![
            Formula::atom(Atom::prop(p)),
            Formula::atom(Atom::prop(q)),
        ]));
        let result = to_nnf(&f);
        assert_eq!(fmt(&result, &syms), "(~(p) & ~(q))");
    }

    #[test]
    fn nnf_quantifier_negation() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        // ¬∀X.p(X) → ∃X.¬p(X)
        let f = Formula::neg(Formula::forall(
            0,
            Formula::atom(Atom::pred(p, vec![Term::var(0)])),
        ));
        let result = to_nnf(&f);
        assert_eq!(fmt(&result, &syms), "?[X0]: (~(p(X0)))");
    }

    #[test]
    fn nnf_iff() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        // p ↔ q → (¬p ∨ q) ∧ (p ∨ ¬q)
        let f = Formula::iff(Formula::atom(Atom::prop(p)), Formula::atom(Atom::prop(q)));
        let result = to_nnf(&f);
        assert_eq!(fmt(&result, &syms), "((~(p) | q) & (p | ~(q)))");
    }
}

/// Convert to NNF while charging each generated formula node before
/// constructing it, and refuse past `max_nodes` or `max_depth`.
///
/// [`to_nnf`] is unbounded, and NNF *distributes* nested biconditionals: a
/// right-nested `<=>` chain of depth n expands to up to 2^n nodes. Every
/// conversion that a caller cannot afford to complete must use this instead —
/// an exponential expansion is a denial-of-service vector for anything that
/// normalises proof or problem text, and the ProoVer-2026 corpus contains a
/// 100-term chain in `PRV043+1`.
pub fn to_nnf_bounded(formula: &Formula, max_nodes: usize, max_depth: usize) -> Option<Formula> {
    fn visit(
        formula: &Formula,
        negated: bool,
        nodes: &mut usize,
        max_nodes: usize,
        depth: usize,
        max_depth: usize,
    ) -> Option<Formula> {
        if depth > max_depth {
            return None;
        }
        *nodes = nodes.checked_add(1)?;
        if *nodes > max_nodes {
            return None;
        }
        let make = |node: Formula, nodes: &mut usize| {
            *nodes = nodes.checked_add(1)?;
            (*nodes <= max_nodes).then_some(node)
        };
        match formula {
            crate::Formula::Atom(atom) => {
                let atom = crate::Formula::Atom(atom.clone());
                if negated {
                    make(crate::Formula::neg(atom), nodes)
                } else {
                    Some(atom)
                }
            }
            crate::Formula::True => Some(if negated {
                crate::Formula::False
            } else {
                crate::Formula::True
            }),
            crate::Formula::False => Some(if negated {
                crate::Formula::True
            } else {
                crate::Formula::False
            }),
            crate::Formula::Neg(inner) => {
                visit(inner, !negated, nodes, max_nodes, depth + 1, max_depth)
            }
            crate::Formula::And(parts) | crate::Formula::Or(parts) => {
                let is_and = matches!(formula, crate::Formula::And(_)) != negated;
                let mut converted = Vec::with_capacity(parts.len());
                for part in parts {
                    converted.push(visit(
                        part,
                        negated,
                        nodes,
                        max_nodes,
                        depth + 1,
                        max_depth,
                    )?);
                }
                let result = if is_and {
                    crate::Formula::and(converted)
                } else {
                    crate::Formula::or(converted)
                };
                make(result, nodes)
            }
            crate::Formula::Implies(left, right) => {
                if negated {
                    let left = visit(left, false, nodes, max_nodes, depth + 1, max_depth)?;
                    let right = visit(right, true, nodes, max_nodes, depth + 1, max_depth)?;
                    make(crate::Formula::and(vec![left, right]), nodes)
                } else {
                    let left = visit(left, true, nodes, max_nodes, depth + 1, max_depth)?;
                    let right = visit(right, false, nodes, max_nodes, depth + 1, max_depth)?;
                    make(crate::Formula::or(vec![left, right]), nodes)
                }
            }
            crate::Formula::Iff(left, right) => {
                let (
                    first_left_negated,
                    first_right_negated,
                    second_left_negated,
                    second_right_negated,
                ) = if negated {
                    (false, false, true, true)
                } else {
                    (true, false, false, true)
                };
                let first_left = visit(
                    left,
                    first_left_negated,
                    nodes,
                    max_nodes,
                    depth + 1,
                    max_depth,
                )?;
                let first_right = visit(
                    right,
                    first_right_negated,
                    nodes,
                    max_nodes,
                    depth + 1,
                    max_depth,
                )?;
                let first = make(crate::Formula::or(vec![first_left, first_right]), nodes)?;
                let second_left = visit(
                    left,
                    second_left_negated,
                    nodes,
                    max_nodes,
                    depth + 1,
                    max_depth,
                )?;
                let second_right = visit(
                    right,
                    second_right_negated,
                    nodes,
                    max_nodes,
                    depth + 1,
                    max_depth,
                )?;
                let second = make(crate::Formula::or(vec![second_left, second_right]), nodes)?;
                make(crate::Formula::and(vec![first, second]), nodes)
            }
            crate::Formula::Forall(variable, body) | crate::Formula::Exists(variable, body) => {
                let is_forall = matches!(formula, crate::Formula::Forall(..)) != negated;
                let body = visit(body, negated, nodes, max_nodes, depth + 1, max_depth)?;
                let result = if is_forall {
                    crate::Formula::forall(*variable, body)
                } else {
                    crate::Formula::exists(*variable, body)
                };
                make(result, nodes)
            }
        }
    }

    visit(formula, false, &mut 0, max_nodes, 0, max_depth)
}
