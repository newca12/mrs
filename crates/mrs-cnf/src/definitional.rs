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
//!
//! This module also provides:
//! - Polarity tracking ([`Polarity`]) for generating directional half-definitions
//!   (Plaisted-Greenbaum) or full biconditionals.
//! - Clause count estimation ([`estimate_clause_count`]).
//! - Configurable thresholding ([`DEFAULT_RENAMING_THRESHOLD`]).
//! - Bottom-up renaming of complex equivalences ([`rename_complex_equivalences`])
//!   to eliminate $2^k$ exponential blowup on formulas with nested biconditionals.

use std::collections::BTreeSet;

use mrs_core::term::VarId;
use mrs_core::{Atom, Formula, SymbolTable, Term};

/// Polarity of a subformula occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Polarity {
    /// Positive polarity (+): under an even number of negations.
    Positive,
    /// Negative polarity (-): under an odd number of negations.
    Negative,
    /// Both polarities (0): under equivalence or XOR.
    Both,
}

impl Polarity {
    /// Invert polarity (under negation or LHS of implication).
    #[inline]
    #[must_use]
    pub fn invert(self) -> Self {
        match self {
            Polarity::Positive => Polarity::Negative,
            Polarity::Negative => Polarity::Positive,
            Polarity::Both => Polarity::Both,
        }
    }
}

/// An introduced definition representing `∀X_1...X_n. (head <=> rhs)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntroducedDefinition {
    /// The fresh definition predicate atom: `def_i(X_1, ..., X_n)`
    pub head: Atom,
    /// The right-hand side formula defined by the head
    pub rhs: Formula,
    /// Polarity of the occurrence(s) being defined
    pub polarity: Polarity,
}

/// Default renaming threshold: disjunctions with estimated distributive clause count
/// greater than this threshold will introduce Tseitin definitions.
pub const DEFAULT_RENAMING_THRESHOLD: usize = 8;

/// Estimates the number of clauses produced by distributive CNF conversion of `formula`.
#[must_use]
pub fn estimate_clause_count(formula: &Formula) -> usize {
    match formula {
        Formula::And(cs) => cs
            .iter()
            .map(estimate_clause_count)
            .fold(0usize, |acc, c| acc.saturating_add(c)),
        Formula::Or(ds) => ds
            .iter()
            .map(estimate_clause_count)
            .fold(1usize, |acc, d| acc.saturating_mul(d)),
        Formula::Atom(_) | Formula::True | Formula::False => 1,
        Formula::Neg(inner) => estimate_neg_clause_count(inner),
        Formula::Implies(a, b) => {
            let na = estimate_neg_clause_count(a);
            let nb = estimate_clause_count(b);
            na.saturating_mul(nb)
        }
        Formula::Iff(a, b) => {
            let c1 = estimate_neg_clause_count(a).saturating_mul(estimate_clause_count(b));
            let c2 = estimate_clause_count(a).saturating_mul(estimate_neg_clause_count(b));
            c1.saturating_add(c2)
        }
        Formula::Forall(_, body) | Formula::Exists(_, body) => estimate_clause_count(body),
    }
}

/// Estimates the number of clauses produced by distributive CNF conversion of `¬formula`.
#[must_use]
pub fn estimate_neg_clause_count(formula: &Formula) -> usize {
    match formula {
        Formula::And(cs) => cs
            .iter()
            .map(estimate_neg_clause_count)
            .fold(1usize, |acc, c| acc.saturating_mul(c)),
        Formula::Or(ds) => ds
            .iter()
            .map(estimate_neg_clause_count)
            .fold(0usize, |acc, d| acc.saturating_add(d)),
        Formula::Atom(_) | Formula::True | Formula::False => 1,
        Formula::Neg(inner) => estimate_clause_count(inner),
        Formula::Implies(a, b) => {
            let ca = estimate_clause_count(a);
            let nb = estimate_neg_clause_count(b);
            ca.saturating_add(nb)
        }
        Formula::Iff(a, b) => {
            let c1 = estimate_clause_count(a).saturating_mul(estimate_clause_count(b));
            let c2 = estimate_neg_clause_count(a).saturating_mul(estimate_neg_clause_count(b));
            c1.saturating_add(c2)
        }
        Formula::Forall(_, body) | Formula::Exists(_, body) => estimate_neg_clause_count(body),
    }
}

fn estimate_clause_count_or(disjuncts: &[Formula]) -> usize {
    disjuncts
        .iter()
        .map(estimate_clause_count)
        .fold(1usize, |acc, d| acc.saturating_mul(d))
}

/// Builds the formula whose clauses are required by an introduced definition
/// according to its polarity (Plaisted-Greenbaum directional definitions).
///
/// - `Polarity::Positive`: emits `~def | rhs` (satisfies `def => rhs`).
/// - `Polarity::Negative`: emits `~rhs | def` (satisfies `rhs => def`).
/// - `Polarity::Both`: emits `def <=> rhs`.
#[must_use]
pub fn definition_clauses_formula(head: &Atom, rhs: &Formula, polarity: Polarity) -> Formula {
    let head_formula = Formula::atom(head.clone());
    match polarity {
        Polarity::Positive => Formula::or(vec![Formula::neg(head_formula), rhs.clone()]),
        Polarity::Negative => Formula::or(vec![Formula::neg(rhs.clone()), head_formula]),
        Polarity::Both => Formula::iff(head_formula, rhs.clone()),
    }
}

/// Helper to check if a formula is a literal (atom, negated atom, true, or false).
#[must_use]
pub fn is_literal_formula(formula: &Formula) -> bool {
    match formula {
        Formula::Atom(_) | Formula::True | Formula::False => true,
        Formula::Neg(inner) => matches!(inner.as_ref(), Formula::Atom(_)),
        _ => false,
    }
}

/// Checks if a formula contains an equivalence anywhere in its subformulas.
#[must_use]
pub fn has_iff(formula: &Formula) -> bool {
    match formula {
        Formula::Iff(_, _) => true,
        Formula::Neg(inner) | Formula::Forall(_, inner) | Formula::Exists(_, inner) => {
            has_iff(inner)
        }
        Formula::And(parts) | Formula::Or(parts) => parts.iter().any(has_iff),
        Formula::Implies(a, b) => has_iff(a) || has_iff(b),
        Formula::Atom(_) | Formula::True | Formula::False => false,
    }
}

/// Checks if a formula contains quantifiers anywhere in its subformulas.
#[must_use]
pub fn has_quantifiers(formula: &Formula) -> bool {
    match formula {
        Formula::Forall(_, _) | Formula::Exists(_, _) => true,
        Formula::Neg(inner) => has_quantifiers(inner),
        Formula::And(parts) | Formula::Or(parts) => parts.iter().any(has_quantifiers),
        Formula::Implies(a, b) | Formula::Iff(a, b) => has_quantifiers(a) || has_quantifiers(b),
        Formula::Atom(_) | Formula::True | Formula::False => false,
    }
}

fn needs_biconditional_naming(f: &Formula, other: &Formula, threshold: usize) -> bool {
    if is_literal_formula(f) {
        return false;
    }
    if has_iff(f) || has_quantifiers(f) || !is_literal_formula(other) {
        return true;
    }
    estimate_clause_count(&Formula::iff(f.clone(), other.clone())) > threshold
}

/// Traverses `formula` and renames complex subformulas under `Formula::Iff` (biconditionals)
/// using fresh definition predicates. This eliminates exponential blowup on formulas with
/// nested or complex equivalences (such as `(((A <=> B) <=> C) <=> D)`).
///
/// Returns the preprocessed formula (where complex equivalence arguments have been replaced
/// by atomic definition predicates) and a list of introduced definitions.
pub fn rename_complex_equivalences(
    formula: &Formula,
    symbols: &mut SymbolTable,
    prefix: &str,
    threshold: usize,
) -> (Formula, Vec<IntroducedDefinition>) {
    let mut ctx = BicondCtx {
        symbols,
        prefix: prefix.to_string(),
        counter: 0,
        definitions: Vec::new(),
    };
    let renamed = ctx.traverse(formula, threshold);
    (renamed, ctx.definitions)
}

struct BicondCtx<'a> {
    symbols: &'a mut SymbolTable,
    prefix: String,
    counter: usize,
    definitions: Vec<IntroducedDefinition>,
}

impl BicondCtx<'_> {
    fn traverse(&mut self, formula: &Formula, threshold: usize) -> Formula {
        match formula {
            Formula::Atom(_) | Formula::True | Formula::False => formula.clone(),
            Formula::Neg(inner) => {
                let inner_renamed = self.traverse(inner, threshold);
                Formula::neg(inner_renamed)
            }
            Formula::And(cs) => {
                let cs_renamed = cs.iter().map(|c| self.traverse(c, threshold)).collect();
                Formula::And(cs_renamed)
            }
            Formula::Or(ds) => {
                let ds_renamed = ds.iter().map(|d| self.traverse(d, threshold)).collect();
                Formula::Or(ds_renamed)
            }
            Formula::Implies(a, b) => {
                let a_renamed = self.traverse(a, threshold);
                let b_renamed = self.traverse(b, threshold);
                Formula::implies(a_renamed, b_renamed)
            }
            Formula::Forall(v, body) => {
                let body_renamed = self.traverse(body, threshold);
                Formula::forall(*v, body_renamed)
            }
            Formula::Exists(v, body) => {
                let body_renamed = self.traverse(body, threshold);
                Formula::exists(*v, body_renamed)
            }
            Formula::Iff(a, b) => {
                let a_renamed = self.traverse(a, threshold);
                let b_renamed = self.traverse(b, threshold);

                let a_needs = needs_biconditional_naming(&a_renamed, &b_renamed, threshold);
                let b_needs = needs_biconditional_naming(&b_renamed, &a_renamed, threshold);

                let a_final = if a_needs {
                    self.introduce_definition(a_renamed, Polarity::Both)
                } else {
                    a_renamed
                };

                let b_final = if b_needs {
                    self.introduce_definition(b_renamed, Polarity::Both)
                } else {
                    b_renamed
                };

                Formula::iff(a_final, b_final)
            }
        }
    }

    fn introduce_definition(&mut self, formula: Formula, polarity: Polarity) -> Formula {
        let mut free_vars: Vec<VarId> = formula.free_vars().into_iter().collect();
        free_vars.sort_unstable();

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
        let args: Vec<Term> = free_vars.iter().map(|&v| Term::var(v)).collect();
        let def_atom = Atom::pred(sym, args);

        self.definitions.push(IntroducedDefinition {
            head: def_atom.clone(),
            rhs: formula,
            polarity,
        });

        Formula::atom(def_atom)
    }
}

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
pub fn to_cnf_definitional_with_defs(
    formula: &Formula,
    symbols: &mut SymbolTable,
    prefix: &str,
) -> (Formula, Vec<(Atom, Vec<Formula>)>) {
    to_cnf_definitional_with_defs_thresh(formula, symbols, prefix, 1)
}

/// Like [`to_cnf_definitional_with_defs`], but with a custom threshold $\rho$.
/// Disjunctions with estimated distributive clause count $\le \rho$ are distributed
/// directly rather than introducing Tseitin definitions.
pub fn to_cnf_definitional_with_defs_thresh(
    formula: &Formula,
    symbols: &mut SymbolTable,
    prefix: &str,
    threshold: usize,
) -> (Formula, Vec<(Atom, Vec<Formula>)>) {
    let mut ctx = DefCtx {
        symbols,
        prefix: prefix.to_string(),
        counter: 0,
        threshold,
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

    // If threshold > 1, renamed may still contain some small And-under-Or
    // disjunctions that were intentionally kept. Convert to pure CNF via distribution.
    let cnf_renamed = crate::cnf::to_cnf(&renamed);
    match cnf_renamed {
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
    threshold: usize,
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

                if self.threshold > 1 {
                    let est = estimate_clause_count_or(&children);
                    if est <= self.threshold {
                        return Formula::Or(children);
                    }
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

    #[test]
    fn threshold_avoids_unnecessary_definitions() {
        let mut syms = SymbolTable::new();
        // p ∨ (q ∧ r): distributive CNF gives 2 clauses: (p ∨ q) ∧ (p ∨ r).
        let f = Formula::or(vec![
            atom(&mut syms, "p"),
            Formula::and(vec![atom(&mut syms, "q"), atom(&mut syms, "r")]),
        ]);

        // With threshold = 1: names (q ∧ r) -> 1 definition, 3 clauses
        let (cnf_th1, defs_th1) = to_cnf_definitional_with_defs_thresh(&f, &mut syms, "t", 1);
        assert_eq!(defs_th1.len(), 1);
        if let Formula::And(cs) = &cnf_th1 {
            assert_eq!(cs.len(), 3);
        } else {
            panic!("Expected And, got: {}", fmt(&cnf_th1, &syms));
        }

        // With threshold = 8: est_clause_count is 2 <= 8 -> 0 definitions, 2 clauses directly distributed
        let (cnf_th8, defs_th8) = to_cnf_definitional_with_defs_thresh(&f, &mut syms, "t", 8);
        assert_eq!(defs_th8.len(), 0);
        if let Formula::And(cs) = &cnf_th8 {
            assert_eq!(cs.len(), 2);
        } else {
            panic!("Expected And, got: {}", fmt(&cnf_th8, &syms));
        }
    }

    #[test]
    fn threshold_still_names_when_exceeded() {
        let mut syms = SymbolTable::new();
        // (a ∧ b ∧ c) ∨ (d ∧ e ∧ f): distributive CNF produces 3 * 3 = 9 clauses.
        let f = Formula::or(vec![
            Formula::and(vec![
                atom(&mut syms, "a"),
                atom(&mut syms, "b"),
                atom(&mut syms, "c"),
            ]),
            Formula::and(vec![
                atom(&mut syms, "d"),
                atom(&mut syms, "e"),
                atom(&mut syms, "f"),
            ]),
        ]);

        // With threshold = 8: 9 > 8 -> names both conjunctions
        let (cnf, defs) = to_cnf_definitional_with_defs_thresh(&f, &mut syms, "t", 8);
        assert_eq!(defs.len(), 2);
        // Each def gives 3 clauses, plus main def0 ∨ def1 = 7 clauses
        if let Formula::And(cs) = &cnf {
            assert_eq!(cs.len(), 7);
        } else {
            panic!("Expected And, got: {}", fmt(&cnf, &syms));
        }
    }

    #[test]
    fn rename_complex_equivalences_prevents_exponential_blowup() {
        let mut syms = SymbolTable::new();
        // Construct a chain of 10 nested biconditionals:
        // p0 <=> (p1 <=> (p2 <=> ... <=> p10))
        let mut f = atom(&mut syms, "p10");
        for i in (0..10).rev() {
            let p_i = atom(&mut syms, &format!("p{i}"));
            f = Formula::iff(p_i, f);
        }

        // Without renaming, to_nnf on 10 nested biconditionals would blow up to 2^10 = 1024 leaves.
        // With renaming:
        let (renamed, defs) =
            rename_complex_equivalences(&f, &mut syms, "t", DEFAULT_RENAMING_THRESHOLD);
        // Exactly 9 definitions introduced (one for each nested Iff rhs)
        assert_eq!(defs.len(), 9);
        // Each definition has Polarity::Both
        for def in &defs {
            assert_eq!(def.polarity, Polarity::Both);
        }

        // In the renamed formula, no nested Iff exists
        if let Formula::Iff(_, right) = &renamed {
            assert!(
                matches!(right.as_ref(), Formula::Atom(_)),
                "Nested Iff was successfully replaced by atom"
            );
        } else {
            panic!("Expected Iff, got: {}", fmt(&renamed, &syms));
        }
    }

    #[test]
    fn polarity_directional_formulas() {
        let mut syms = SymbolTable::new();
        let head = Atom::prop(syms.intern("def0"));
        let rhs = Formula::and(vec![atom(&mut syms, "p"), atom(&mut syms, "q")]);

        // Positive polarity: ~def0 | (p & q)
        let pos = definition_clauses_formula(&head, &rhs, Polarity::Positive);
        assert_eq!(fmt(&pos, &syms), "(~(def0) | (p & q))");

        // Negative polarity: ~(p & q) | def0
        let neg = definition_clauses_formula(&head, &rhs, Polarity::Negative);
        assert_eq!(fmt(&neg, &syms), "(~((p & q)) | def0)");

        // Both polarities: def0 <=> (p & q)
        let both = definition_clauses_formula(&head, &rhs, Polarity::Both);
        assert_eq!(fmt(&both, &syms), "(def0 <=> (p & q))");
    }

    #[test]
    fn estimate_clause_count_accuracy() {
        let mut syms = SymbolTable::new();
        let p = atom(&mut syms, "p");
        let q = atom(&mut syms, "q");
        let r = atom(&mut syms, "r");

        // Single literal -> 1
        assert_eq!(estimate_clause_count(&p), 1);
        // p & q -> 1 + 1 = 2
        assert_eq!(
            estimate_clause_count(&Formula::and(vec![p.clone(), q.clone()])),
            2
        );
        // p | q -> 1 * 1 = 1
        assert_eq!(
            estimate_clause_count(&Formula::or(vec![p.clone(), q.clone()])),
            1
        );
        // (p & q) | r -> 2 * 1 = 2
        let p_and_q = Formula::and(vec![p.clone(), q.clone()]);
        assert_eq!(
            estimate_clause_count(&Formula::or(vec![p_and_q.clone(), r.clone()])),
            2
        );
        // (p & q) | (r & s) -> 2 * 2 = 4
        assert_eq!(
            estimate_clause_count(&Formula::or(vec![p_and_q.clone(), p_and_q.clone()])),
            4
        );
        // p <=> q -> (~p | q) & (p | ~q) -> 1 + 1 = 2
        assert_eq!(
            estimate_clause_count(&Formula::iff(p.clone(), q.clone())),
            2
        );
    }
}
