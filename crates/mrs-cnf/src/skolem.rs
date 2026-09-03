//! Skolemization: elimination of existential quantifiers.
//!
//! After NNF conversion, a formula may contain existential quantifiers.
//! Skolemization replaces each existentially quantified variable with a
//! fresh function symbol (Skolem function) applied to the universally
//! quantified variables in scope.
//!
//! ## Examples
//!
//! - `∃X. p(X)` → `p(sk0)` (Skolem constant, no universal vars in scope)
//! - `∀X. ∃Y. p(X,Y)` → `∀X. p(X, sk1(X))` (Skolem function of X)

use mrs_core::term::VarId;
use mrs_core::{Formula, SymbolTable, Term};

/// Skolemizes a formula (assumed to be in NNF).
///
/// Replaces each existentially quantified variable with a Skolem function
/// applied to the universally quantified variables currently in scope.
/// Fresh Skolem function symbols are created in the symbol table.
///
/// The `prefix` is used to generate unique Skolem names across different
/// formulas (typically the formula's TPTP name). This prevents name
/// collisions when multiple formulas are Skolemized independently.
pub fn skolemize(formula: &Formula, symbols: &mut SymbolTable, prefix: &str) -> Formula {
    let mut ctx = SkolemCtx {
        symbols,
        prefix: prefix.to_string(),
        counter: 0,
        universal_vars: Vec::new(),
    };
    ctx.skolemize(formula)
}

struct SkolemCtx<'a> {
    symbols: &'a mut SymbolTable,
    prefix: String,
    counter: usize,
    /// Stack of universally quantified variables currently in scope.
    universal_vars: Vec<VarId>,
}

impl SkolemCtx<'_> {
    /// Generates a fresh Skolem function symbol.
    fn fresh_skolem(&mut self) -> mrs_core::SymbolId {
        let sanitized_prefix: String = self
            .prefix
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let name = format!("sk_{}_{}", sanitized_prefix, self.counter);
        self.counter += 1;
        self.symbols.intern(&name)
    }

    fn skolemize(&mut self, formula: &Formula) -> Formula {
        match formula {
            Formula::Forall(v, body) => {
                self.universal_vars.push(*v);
                let result = Formula::forall(*v, self.skolemize(body));
                self.universal_vars.pop();
                result
            }

            Formula::Exists(v, body) => {
                // Replace the existential variable with a Skolem term
                let skolem_sym = self.fresh_skolem();

                // Free-variable filtered Skolemization (optimized Skolemization):
                // The Skolem function needs to depend ONLY on the in-scope universal
                // variables that actually occur free in the existential body.
                // Any universal variable not occurring in the body cannot influence the
                // existential witness, so passing it introduces unnecessary term bloat
                // and higher arities.
                let body_fvs = body.free_vars();
                let mut seen = std::collections::HashSet::new();
                let filtered_vars: Vec<VarId> = self
                    .universal_vars
                    .iter()
                    .copied()
                    .filter(|u| body_fvs.contains(u) && seen.insert(*u))
                    .collect();

                let skolem_term = if filtered_vars.is_empty() {
                    // No relevant universal vars in scope: Skolem constant
                    Term::constant(skolem_sym)
                } else {
                    // Skolem function applied only to the universal vars that occur free in body
                    let args: Vec<Term> = filtered_vars.into_iter().map(Term::var).collect();
                    Term::app(skolem_sym, args)
                };

                // Substitute the existential variable with the Skolem term
                let sub = mrs_core::Substitution::singleton(*v, skolem_term);
                let body_subst = sub.apply_formula(body);

                // Continue Skolemizing the result (don't wrap in Exists)
                self.skolemize(&body_subst)
            }

            // Structural recursion for all other cases
            Formula::Atom(a) => Formula::Atom(a.clone()),
            Formula::Neg(inner) => Formula::neg(self.skolemize(inner)),
            Formula::And(cs) => Formula::and(cs.iter().map(|c| self.skolemize(c)).collect()),
            Formula::Or(ds) => Formula::or(ds.iter().map(|d| self.skolemize(d)).collect()),
            Formula::Implies(a, b) => Formula::implies(self.skolemize(a), self.skolemize(b)),
            Formula::Iff(a, b) => Formula::iff(self.skolemize(a), self.skolemize(b)),
            Formula::True => Formula::True,
            Formula::False => Formula::False,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::display::DisplayWithSymbols;
    use mrs_core::{Atom, Term};

    fn fmt(f: &Formula, syms: &SymbolTable) -> String {
        format!("{}", f.display(syms))
    }

    #[test]
    fn skolem_constant() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");

        // ∃X. p(X) → p(sk_t_0)
        let f = Formula::exists(0, Formula::atom(Atom::pred(p, vec![Term::var(0)])));
        let result = skolemize(&f, &mut syms, "t");
        assert_eq!(fmt(&result, &syms), "p(sk_t_0)");
    }

    #[test]
    fn skolem_function() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");

        // ∀X. ∃Y. p(X, Y) → ∀X. p(X, sk_t_0(X))
        let f = Formula::forall(
            0,
            Formula::exists(
                1,
                Formula::atom(Atom::pred(p, vec![Term::var(0), Term::var(1)])),
            ),
        );
        let result = skolemize(&f, &mut syms, "t");
        assert_eq!(fmt(&result, &syms), "![X0]: (p(X0, sk_t_0(X0)))");
    }

    #[test]
    fn nested_skolem() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");

        // ∀X. ∃Y. ∀Z. ∃W. p(X, Y, Z, W)
        // → ∀X. ∀Z. p(X, sk_t_0(X), Z, sk_t_1(X, Z))
        let f = Formula::forall(
            0,
            Formula::exists(
                1,
                Formula::forall(
                    2,
                    Formula::exists(
                        3,
                        Formula::atom(Atom::pred(
                            p,
                            vec![Term::var(0), Term::var(1), Term::var(2), Term::var(3)],
                        )),
                    ),
                ),
            ),
        );
        let result = skolemize(&f, &mut syms, "t");
        assert_eq!(
            fmt(&result, &syms),
            "![X0]: (![X2]: (p(X0, sk_t_0(X0), X2, sk_t_1(X0, X2))))"
        );
    }

    #[test]
    fn no_existentials() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");

        // ∀X. p(X) stays the same
        let f = Formula::forall(0, Formula::atom(Atom::pred(p, vec![Term::var(0)])));
        let result = skolemize(&f, &mut syms, "t");
        assert_eq!(fmt(&result, &syms), "![X0]: (p(X0))");
    }

    #[test]
    fn unique_prefixes() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");

        // Two formulas Skolemized with different prefixes get different names
        let f1 = Formula::exists(0, Formula::atom(Atom::pred(p, vec![Term::var(0)])));
        let f2 = Formula::exists(0, Formula::atom(Atom::pred(q, vec![Term::var(0)])));

        let r1 = skolemize(&f1, &mut syms, "ax1");
        let r2 = skolemize(&f2, &mut syms, "ax2");

        assert_eq!(fmt(&r1, &syms), "p(sk_ax1_0)");
        assert_eq!(fmt(&r2, &syms), "q(sk_ax2_0)");

        // Verify they're different symbols
        let sk_ax1 = syms.intern("sk_ax1_0");
        let sk_ax2 = syms.intern("sk_ax2_0");
        assert_ne!(sk_ax1, sk_ax2);
    }

    #[test]
    fn skolem_prefix_sanitization() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");

        // Prefix containing non-alphanumeric/non-underscore characters
        let complex_prefix = "def(cond(conseq(105), 0), 1)";
        let f = Formula::exists(0, Formula::atom(Atom::pred(p, vec![Term::var(0)])));
        let result = skolemize(&f, &mut syms, complex_prefix);

        // Parentheses and commas must be mapped to underscores, ensuring 100% TPTP compliance
        assert_eq!(fmt(&result, &syms), "p(sk_def_cond_conseq_105___0___1__0)");
    }

    #[test]
    fn filtered_skolem_drops_unused_universal() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");

        // ∀X. ∃Y. p(Y)
        // Since X is not free in p(Y), Y becomes a Skolem constant sk_t_0, NOT sk_t_0(X0).
        let f = Formula::forall(
            0,
            Formula::exists(1, Formula::atom(Atom::pred(p, vec![Term::var(1)]))),
        );
        let result = skolemize(&f, &mut syms, "t");
        assert_eq!(fmt(&result, &syms), "![X0]: (p(sk_t_0))");
    }

    #[test]
    fn filtered_skolem_partial_universal_dependency() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");

        // ∀X. ∀Z. ∃Y. p(Z, Y)
        // Since X is not free in p(Z, Y), Y depends only on Z (X1), yielding sk_t_0(X1), NOT sk_t_0(X0, X1).
        let f = Formula::forall(
            0,
            Formula::forall(
                1,
                Formula::exists(
                    2,
                    Formula::atom(Atom::pred(p, vec![Term::var(1), Term::var(2)])),
                ),
            ),
        );
        let result = skolemize(&f, &mut syms, "t");
        assert_eq!(fmt(&result, &syms), "![X0]: (![X1]: (p(X1, sk_t_0(X1))))");
    }

    #[test]
    fn filtered_skolem_independent_nested_existentials() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");

        // ∀X. ∀Y. (∃Z. p(X, Z) ∧ ∃W. q(Y, W))
        // Z depends only on X; W depends only on Y.
        let f = Formula::forall(
            0,
            Formula::forall(
                1,
                Formula::and(vec![
                    Formula::exists(
                        2,
                        Formula::atom(Atom::pred(p, vec![Term::var(0), Term::var(2)])),
                    ),
                    Formula::exists(
                        3,
                        Formula::atom(Atom::pred(q, vec![Term::var(1), Term::var(3)])),
                    ),
                ]),
            ),
        );
        let result = skolemize(&f, &mut syms, "t");
        assert_eq!(
            fmt(&result, &syms),
            "![X0]: (![X1]: ((p(X0, sk_t_0(X0)) & q(X1, sk_t_1(X1)))))"
        );
    }
}
