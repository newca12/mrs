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
                // ¬(φ ↔ ψ) ≡ (φ ∧ ¬ψ) ∨ (¬φ ∧ ψ)
                Formula::or(vec![
                    Formula::and(vec![nnf(a, false, cache), nnf(b, true, cache)]),
                    Formula::and(vec![nnf(a, true, cache), nnf(b, false, cache)]),
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
