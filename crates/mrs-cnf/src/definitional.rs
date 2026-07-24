//! Definitional CNF (Tseitin transformation).
//!
//! When distributive CNF conversion would cause exponential blowup (because
//! `And` appears under `Or`), this module introduces fresh definition
//! predicates to keep the clause count linear.
//!
//! For example, `p ∨ (q ∧ r)` is transformed to:
//!   - Main clause: `p ∨ def0`
//!   - Definition clauses: `¬def0 ∨ q`, `¬def0 ∨ r`
//!
//! In first-order logic, the definition predicate takes the free variables
//! of the named subformula as arguments, ensuring correctness.

use std::collections::BTreeSet;

use mrs_core::term::VarId;
use mrs_core::{Atom, Formula, SymbolTable, Term};

/// Converts a quantifier-free NNF formula to CNF using definitional naming.
///
/// Introduces fresh definition symbols for conjunctions that appear under
/// disjunctions, avoiding exponential blowup. The result formula is in CNF
/// (a conjunction of disjunctions of literals).
///
/// The `prefix` is used to generate unique definition symbol names.
///
/// Discards the introduced definitions themselves (just their effect on the
/// resulting formula) — use [`to_cnf_definitional_with_defs`] to get those
/// too, needed to correctly document each fresh definition's introduction in
/// a TSTP proof (see that function's doc comment for why).
pub fn to_cnf_definitional(formula: &Formula, symbols: &mut SymbolTable, prefix: &str) -> Formula {
    to_cnf_definitional_with_defs(formula, symbols, prefix).0
}

/// Like [`to_cnf_definitional`], but also returns the introduced definitions
/// (each fresh predicate atom together with the conjuncts it names).
///
/// A proof that uses one of the resulting clauses needs to justify the
/// fresh `def_...` predicate's meaning somehow: citing only the pre-CNF
/// parent formula as `inference(cnf_transformation, [status(thm)], [...])`
/// does not work, because that parent formula never mentions the fresh
/// symbol at all, so no ATP can prove the child follows from it alone
/// (confirmed against a real GDV build: it reports a genuine
/// `CounterSatisfiable` countermodel, not just a timeout). The fix mirrors
/// the "introduced(definition)" convention used by E/Vampire (see
/// `mrs-proover`'s `checks::introduced_definition` module, which already
/// has to recognize both of their conventions when verifying *other*
/// systems' proofs): the caller should emit each definition's full
/// biconditional (`def_atom <=> (conjunct1 & ... & conjunctK)`) as its own
/// `introduced(definition)` step with no parents (sound by construction,
/// since the symbol is fresh — a conservative extension), then cite that
/// step as an *additional* parent for every final clause that actually
/// mentions the definition's predicate symbol.
pub fn to_cnf_definitional_with_defs(
    formula: &Formula,
    symbols: &mut SymbolTable,
    prefix: &str,
) -> (Formula, Vec<(Atom, Vec<Formula>)>) {
    let mut ctx = DefCtx {
        symbols,
        prefix: prefix.to_string(),
        counter: 0,
        definitions: Vec::new(),
    };

    // Bottom-up pass: name And-under-Or subformulas
    let renamed = ctx.name_and_under_or(formula);

    // Build final CNF: definition clauses + renamed formula
    let mut all_conjuncts = Vec::new();

    // Add definition clauses: for each def → (A1 ∧ ... ∧ Ak),
    // add clauses ¬def ∨ A1, ¬def ∨ A2, ..., ¬def ∨ Ak
    for (def_atom, conjuncts) in &ctx.definitions {
        for conj in conjuncts {
            let neg_def = Formula::neg(Formula::atom(def_atom.clone()));
            let clause = Formula::or(vec![neg_def, conj.clone()]);
            all_conjuncts.push(clause);
        }
    }

    // Add the renamed formula's conjuncts
    match renamed {
        Formula::And(cs) => all_conjuncts.extend(cs),
        other => all_conjuncts.push(other),
    }

    let result = if all_conjuncts.len() == 1 {
        all_conjuncts.into_iter().next().unwrap()
    } else {
        Formula::And(all_conjuncts)
    };

    (result, ctx.definitions)
}

struct DefCtx<'a> {
    symbols: &'a mut SymbolTable,
    prefix: String,
    counter: usize,
    /// Collected definitions: (definition_atom, conjuncts_of_named_formula)
    definitions: Vec<(Atom, Vec<Formula>)>,
}

impl DefCtx<'_> {
    /// Bottom-up pass: name conjunction nodes that appear under disjunction nodes.
    fn name_and_under_or(&mut self, formula: &Formula) -> Formula {
        match formula {
            Formula::And(cs) => {
                // Process children first (bottom-up)
                let children: Vec<Formula> = cs.iter().map(|c| self.name_and_under_or(c)).collect();
                Formula::And(children)
            }
            Formula::Or(ds) => {
                // Process children first (bottom-up)
                let children: Vec<Formula> = ds.iter().map(|d| self.name_and_under_or(d)).collect();

                // Check if any child is an And (requires naming)
                let has_and = children.iter().any(|d| matches!(d, Formula::And(_)));
                if !has_and {
                    return Formula::Or(children);
                }

                // Name each And child, leave others unchanged
                let named: Vec<Formula> = children
                    .into_iter()
                    .map(|d| {
                        if let Formula::And(conj) = d {
                            self.introduce_name(conj)
                        } else {
                            d
                        }
                    })
                    .collect();

                Formula::Or(named)
            }
            // Atoms, negated atoms, True, False: pass through unchanged
            other => other.clone(),
        }
    }

    /// Introduces a fresh definition predicate for a conjunction.
    ///
    /// Given conjuncts [A1, ..., Ak], creates a fresh predicate `def_i(X1,...,Xn)`
    /// where X1,...,Xn are the free variables of the conjuncts.
    /// Stores definition clauses: each `¬def_i(X1,...,Xn) ∨ Ai`.
    /// Returns the atom `def_i(X1,...,Xn)` to replace the conjunction.
    fn introduce_name(&mut self, conjuncts: Vec<Formula>) -> Formula {
        // Flatten nested Ands (associativity): And(a, And(b, c)) → [a, b, c]
        let mut flat = Vec::new();
        flatten_conjuncts(conjuncts, &mut flat);

        // Collect free variables from all conjuncts
        let mut vars = BTreeSet::new();
        for conj in &flat {
            collect_free_vars(conj, &mut vars);
        }
        let sorted_vars: Vec<VarId> = vars.into_iter().collect();

        // Create fresh definition predicate.
        //
        // TPTP requires predicate/function/constant symbols to start with a
        // lowercase alphabetic character (see the `lower_word`/`functor`
        // grammar productions in the TPTP language spec). A leading (or
        // trailing) underscore, as in the previous `__def_..._..__` scheme,
        // is not a valid TPTP symbol and produces a syntax error in any
        // proof that uses one of these definitions.
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
        let name = format!("def_{}_{}", sanitized_prefix, self.counter);
        self.counter += 1;
        let sym = self.symbols.intern(&name);
        let args: Vec<Term> = sorted_vars.iter().map(|&v| Term::var(v)).collect();
        let def_atom = Atom::pred(sym, args);

        // Store the definition for clause generation
        self.definitions.push((def_atom.clone(), flat));

        // Return the definition atom (replaces the conjunction)
        Formula::atom(def_atom)
    }
}

/// Flattens nested And conjuncts: And(a, And(b, c)) becomes [a, b, c].
fn flatten_conjuncts(formulas: Vec<Formula>, out: &mut Vec<Formula>) {
    for f in formulas {
        match f {
            Formula::And(cs) => flatten_conjuncts(cs, out),
            other => out.push(other),
        }
    }
}

/// Collects all variable IDs appearing in a formula.
/// After quantifier stripping, all variables are free.
fn collect_free_vars(formula: &Formula, vars: &mut BTreeSet<VarId>) {
    match formula {
        Formula::Atom(a) => collect_vars_atom(a, vars),
        Formula::Neg(inner) => collect_free_vars(inner, vars),
        Formula::And(cs) => {
            for c in cs {
                collect_free_vars(c, vars);
            }
        }
        Formula::Or(ds) => {
            for d in ds {
                collect_free_vars(d, vars);
            }
        }
        Formula::True | Formula::False => {}
        // No quantifiers expected after stripping
        _ => {}
    }
}

fn collect_vars_atom(atom: &Atom, vars: &mut BTreeSet<VarId>) {
    match atom {
        Atom::Pred(_, terms) => {
            for t in terms {
                collect_vars_term(t, vars);
            }
        }
        Atom::Eq(l, r) => {
            collect_vars_term(l, vars);
            collect_vars_term(r, vars);
        }
    }
}

fn collect_vars_term(term: &Term, vars: &mut BTreeSet<VarId>) {
    match term {
        Term::Var(v) => {
            vars.insert(*v);
        }
        Term::App(_, args) => {
            for a in args {
                collect_vars_term(a, vars);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::display::DisplayWithSymbols;

    fn fmt(f: &Formula, syms: &SymbolTable) -> String {
        format!("{}", f.display(syms))
    }

    fn atom(syms: &mut SymbolTable, name: &str) -> Formula {
        let s = syms.intern(name);
        Formula::atom(Atom::prop(s))
    }

    #[test]
    fn no_naming_needed() {
        // p ∨ q — already CNF, no And under Or
        let mut syms = SymbolTable::new();
        let f = Formula::or(vec![atom(&mut syms, "p"), atom(&mut syms, "q")]);
        let result = to_cnf_definitional(&f, &mut syms, "t");
        assert_eq!(fmt(&result, &syms), "(p | q)");
    }

    #[test]
    fn simple_naming() {
        // p ∨ (q ∧ r) → introduces def for (q ∧ r)
        let mut syms = SymbolTable::new();
        let f = Formula::or(vec![
            atom(&mut syms, "p"),
            Formula::and(vec![atom(&mut syms, "q"), atom(&mut syms, "r")]),
        ]);
        let result = to_cnf_definitional(&f, &mut syms, "t");

        // Should have 3 conjuncts: ¬def|q, ¬def|r, p|def
        if let Formula::And(cs) = &result {
            assert_eq!(cs.len(), 3);
        } else {
            panic!("Expected And, got: {}", fmt(&result, &syms));
        }
    }

    #[test]
    fn double_naming() {
        // (p ∧ q) ∨ (r ∧ s) → two definitions
        let mut syms = SymbolTable::new();
        let f = Formula::or(vec![
            Formula::and(vec![atom(&mut syms, "p"), atom(&mut syms, "q")]),
            Formula::and(vec![atom(&mut syms, "r"), atom(&mut syms, "s")]),
        ]);
        let result = to_cnf_definitional(&f, &mut syms, "t");

        // Should have 5 conjuncts:
        // ¬def0|p, ¬def0|q, ¬def1|r, ¬def1|s, def0|def1
        if let Formula::And(cs) = &result {
            assert_eq!(cs.len(), 5);
        } else {
            panic!("Expected And, got: {}", fmt(&result, &syms));
        }
    }

    #[test]
    fn pure_conjunction() {
        // p ∧ q — already CNF (no Or with And)
        let mut syms = SymbolTable::new();
        let f = Formula::and(vec![atom(&mut syms, "p"), atom(&mut syms, "q")]);
        let result = to_cnf_definitional(&f, &mut syms, "t");
        assert_eq!(fmt(&result, &syms), "(p & q)");
    }

    #[test]
    fn with_variables() {
        // p(X) ∨ (q(X) ∧ r(Y)) → def has free vars {X, Y}
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let r = syms.intern("r");

        let f = Formula::or(vec![
            Formula::atom(Atom::pred(p, vec![Term::var(0)])),
            Formula::and(vec![
                Formula::atom(Atom::pred(q, vec![Term::var(0)])),
                Formula::atom(Atom::pred(r, vec![Term::var(1)])),
            ]),
        ]);

        let result = to_cnf_definitional(&f, &mut syms, "t");

        // Should produce 3 clauses, with the def taking (X0, X1) as args
        if let Formula::And(cs) = &result {
            assert_eq!(cs.len(), 3);
            // Check that definition clauses mention the def with variables
            let display = fmt(&result, &syms);
            assert!(
                display.contains("def_t_0"),
                "Should contain definition name, got: {display}"
            );
        } else {
            panic!("Expected And, got: {}", fmt(&result, &syms));
        }
    }

    #[test]
    fn nested_and_under_or() {
        // a ∨ (b ∧ (c ∨ (d ∧ e)))
        // Inner: (d ∧ e) is under Or → name as def0
        // Then: (b ∧ (c ∨ def0)) — but (c ∨ def0) is just a disjunction, no And
        // Outer: a ∨ (b ∧ (c ∨ def0)) → And under Or → name as def1
        let mut syms = SymbolTable::new();
        let f = Formula::or(vec![
            atom(&mut syms, "a"),
            Formula::and(vec![
                atom(&mut syms, "b"),
                Formula::or(vec![
                    atom(&mut syms, "c"),
                    Formula::and(vec![atom(&mut syms, "d"), atom(&mut syms, "e")]),
                ]),
            ]),
        ]);

        let result = to_cnf_definitional(&f, &mut syms, "t");

        // Inner And (d∧e) → def0, gives: ¬def0|d, ¬def0|e
        // Outer And (b∧(c|def0)) → def1, gives: ¬def1|b, ¬def1|c|def0
        // Main: a|def1
        // Total: 5 conjuncts
        if let Formula::And(cs) = &result {
            assert_eq!(cs.len(), 5);
        } else {
            panic!("Expected And, got: {}", fmt(&result, &syms));
        }
    }

    #[test]
    fn definition_prefix_sanitization() {
        let mut syms = SymbolTable::new();

        // Complex prefix containing parentheses, commas, etc.
        let complex_prefix = "def(cond(conseq(axiom(3)), 17), 1)";

        let f = Formula::or(vec![
            atom(&mut syms, "a"),
            Formula::and(vec![atom(&mut syms, "b"), atom(&mut syms, "c")]),
        ]);

        let result = to_cnf_definitional(&f, &mut syms, complex_prefix);
        let display = fmt(&result, &syms);

        // Name of the introduced definition must be fully sanitized for TPTP compliance
        assert!(
            display.contains("def_def_cond_conseq_axiom_3____17___1__0"),
            "Should contain sanitized definition name, got: {display}"
        );
    }
}
