//! AST visitor / traversal support.
//!
//! This module provides a single [`Visitor`] trait that covers all TPTP dialects
//! (FOF, CNF, TFF, THF). Implement it, override only the hooks you care about,
//! and call the corresponding `walk_*` function to drive the traversal.
//!
//! # Example — collect all variable names from a FOF formula
//!
//! ```
//! use mrs_tptp::{parse_tptp, AnnotatedFormula, FOFStatement};
//! use mrs_tptp::visitor::{Visitor, walk_fof_formula};
//!
//! struct VarCollector<'a> {
//!     vars: Vec<&'a str>,
//! }
//!
//! impl<'a> Visitor<'a> for VarCollector<'a> {
//!     fn visit_variable(&mut self, name: &'a str) {
//!         self.vars.push(name);
//!     }
//! }
//!
//! let input = "fof(ax1, axiom, ![X,Y]: (p(X) => q(X, Y))).";
//! let problem = parse_tptp(input).unwrap();
//!
//! if let AnnotatedFormula::FOF(fof) = &problem.formulas[0] {
//!     if let FOFStatement::Logical(f) = &fof.formula {
//!         let mut col = VarCollector { vars: Vec::new() };
//!         walk_fof_formula(&mut col, f);
//!         println!("Variables: {:?}", col.vars);
//!     }
//! }
//! ```

use crate::ast::{
    cnf::{CNFAtomicFormula, CNFFormula, CNFLiteral},
    common::{AtomicWord, BinaryConnective, Number, Quantifier},
    fof::{FOFAtomicFormula, FOFFormula, FOFTerm},
    tcf::{TCFAtomicFormula, TCFClause, TCFFormula, TCFLiteral},
    tff::{TFFAtomicFormula, TFFFormula, TFFLetBody, TFFTerm},
    thf::{THFAtomicFormula, THFFormula},
};

/// Unified AST visitor for all TPTP dialects.
///
/// Every method has a default implementation. Structural methods recurse via
/// the corresponding [`walk_*`] free function; leaf hooks are no-ops.
/// Override only what you need.
pub trait Visitor<'a> {
    // -----------------------------------------------------------------------
    // Structural hooks — default: recurse
    // -----------------------------------------------------------------------

    /// Visit a FOF formula node.
    fn visit_fof_formula(&mut self, f: &FOFFormula<'a>) {
        walk_fof_formula(self, f);
    }

    /// Visit a FOF term node.
    fn visit_fof_term(&mut self, t: &FOFTerm<'a>) {
        walk_fof_term(self, t);
    }

    /// Visit a CNF formula (clause) node.
    fn visit_cnf_formula(&mut self, f: &CNFFormula<'a>) {
        walk_cnf_formula(self, f);
    }

    /// Visit a CNF literal node.
    fn visit_cnf_literal(&mut self, l: &CNFLiteral<'a>) {
        walk_cnf_literal(self, l);
    }

    /// Visit a TFF formula node.
    fn visit_tff_formula(&mut self, f: &TFFFormula<'a>) {
        walk_tff_formula(self, f);
    }

    /// Visit a TFF term node.
    fn visit_tff_term(&mut self, t: &TFFTerm<'a>) {
        walk_tff_term(self, t);
    }

    /// Visit a TCF formula node.
    fn visit_tcf_formula(&mut self, f: &TCFFormula<'a>) {
        walk_tcf_formula(self, f);
    }

    /// Visit a THF formula node.
    fn visit_thf_formula(&mut self, f: &THFFormula<'a>) {
        walk_thf_formula(self, f);
    }

    // -----------------------------------------------------------------------
    // Leaf hooks — default: no-op
    // -----------------------------------------------------------------------

    /// Called for every variable occurrence (`X`, `Var`, …).
    fn visit_variable(&mut self, _name: &'a str) {}

    /// Called for every predicate or functor application (`p(…)`, `f(…)`).
    ///
    /// `arg_count` is the number of arguments (0 for propositions/constants).
    fn visit_atom(&mut self, _name: &AtomicWord<'a>, _arg_count: usize) {}

    /// Called for every number literal.
    fn visit_number(&mut self, _n: &Number<'a>) {}

    /// Called for every binary connective (`&`, `|`, `=>`, …).
    fn visit_connective(&mut self, _c: BinaryConnective) {}

    /// Called for every first-order quantifier (`!` / `?`).
    fn visit_quantifier(&mut self, _q: Quantifier) {}
}

// ---------------------------------------------------------------------------
// FOF walk functions
// ---------------------------------------------------------------------------

/// Recursively walk a [`FOFFormula`], calling visitor methods on each node.
pub fn walk_fof_formula<'a, V: Visitor<'a> + ?Sized>(v: &mut V, f: &FOFFormula<'a>) {
    match f {
        FOFFormula::Atomic(a) => walk_fof_atomic(v, a),
        FOFFormula::Negation(inner) => v.visit_fof_formula(inner),
        FOFFormula::Quantified {
            quantifier,
            variables,
            formula,
        } => {
            v.visit_quantifier(*quantifier);
            for var in variables {
                v.visit_variable(var);
            }
            v.visit_fof_formula(formula);
        }
        FOFFormula::Binary {
            left,
            connective,
            right,
        } => {
            v.visit_connective(*connective);
            v.visit_fof_formula(left);
            v.visit_fof_formula(right);
        }
        FOFFormula::Equality(l, r) | FOFFormula::Inequality(l, r) => {
            v.visit_fof_term(l);
            v.visit_fof_term(r);
        }
        FOFFormula::Parens(inner) => v.visit_fof_formula(inner),
    }
}

fn walk_fof_atomic<'a, V: Visitor<'a> + ?Sized>(v: &mut V, a: &FOFAtomicFormula<'a>) {
    match a {
        FOFAtomicFormula::Plain(name, args) => {
            v.visit_atom(name, args.len());
            for arg in args {
                v.visit_fof_term(arg);
            }
        }
        FOFAtomicFormula::Defined(_, args) | FOFAtomicFormula::System(_, args) => {
            for arg in args {
                v.visit_fof_term(arg);
            }
        }
        FOFAtomicFormula::True | FOFAtomicFormula::False => {}
    }
}

/// Recursively walk a [`FOFTerm`], calling visitor methods on each node.
pub fn walk_fof_term<'a, V: Visitor<'a> + ?Sized>(v: &mut V, t: &FOFTerm<'a>) {
    match t {
        FOFTerm::Variable(name) => v.visit_variable(name),
        FOFTerm::Function(name, args) => {
            v.visit_atom(name, args.len());
            for arg in args {
                v.visit_fof_term(arg);
            }
        }
        FOFTerm::DefinedFunction(_, args) | FOFTerm::SystemFunction(_, args) => {
            for arg in args {
                v.visit_fof_term(arg);
            }
        }
        FOFTerm::Number(n) => v.visit_number(n),
        FOFTerm::DistinctObject(_) => {}
    }
}

// ---------------------------------------------------------------------------
// CNF walk functions
// ---------------------------------------------------------------------------

/// Recursively walk a [`CNFFormula`], calling visitor methods on each node.
pub fn walk_cnf_formula<'a, V: Visitor<'a> + ?Sized>(v: &mut V, f: &CNFFormula<'a>) {
    match f {
        CNFFormula::Disjunction(lits) => {
            for lit in lits {
                v.visit_cnf_literal(lit);
            }
        }
        CNFFormula::Parens(inner) => v.visit_cnf_formula(inner),
    }
}

/// Recursively walk a [`CNFLiteral`], calling visitor methods on each node.
pub fn walk_cnf_literal<'a, V: Visitor<'a> + ?Sized>(v: &mut V, l: &CNFLiteral<'a>) {
    match l {
        CNFLiteral::Positive(a) | CNFLiteral::Negative(a) => walk_cnf_atomic(v, a),
        CNFLiteral::Equality(l, r) | CNFLiteral::Inequality(l, r) => {
            v.visit_fof_term(l);
            v.visit_fof_term(r);
        }
    }
}

fn walk_cnf_atomic<'a, V: Visitor<'a> + ?Sized>(v: &mut V, a: &CNFAtomicFormula<'a>) {
    match a {
        CNFAtomicFormula::Plain(name, args) => {
            v.visit_atom(name, args.len());
            for arg in args {
                v.visit_fof_term(arg);
            }
        }
        CNFAtomicFormula::Defined(_, args) | CNFAtomicFormula::System(_, args) => {
            for arg in args {
                v.visit_fof_term(arg);
            }
        }
        CNFAtomicFormula::True | CNFAtomicFormula::False => {}
    }
}

// ---------------------------------------------------------------------------
// TFF walk functions
// ---------------------------------------------------------------------------

/// Recursively walk a [`TFFFormula`], calling visitor methods on each node.
pub fn walk_tff_formula<'a, V: Visitor<'a> + ?Sized>(v: &mut V, f: &TFFFormula<'a>) {
    match f {
        TFFFormula::Atomic(a) => walk_tff_atomic(v, a),
        TFFFormula::Negation(inner) => v.visit_tff_formula(inner),
        TFFFormula::Quantified {
            quantifier,
            variables,
            formula,
        } => {
            v.visit_quantifier(*quantifier);
            for var in variables {
                v.visit_variable(var.name);
            }
            v.visit_tff_formula(formula);
        }
        TFFFormula::TypeQuantified { formula, .. } => v.visit_tff_formula(formula),
        TFFFormula::Binary {
            left,
            connective,
            right,
        } => {
            v.visit_connective(*connective);
            v.visit_tff_formula(left);
            v.visit_tff_formula(right);
        }
        TFFFormula::Equality(l, r) | TFFFormula::Inequality(l, r) => {
            v.visit_tff_term(l);
            v.visit_tff_term(r);
        }
        TFFFormula::Parens(inner) => v.visit_tff_formula(inner),
        TFFFormula::Conditional {
            condition,
            then_branch,
            else_branch,
        } => {
            v.visit_tff_formula(condition);
            v.visit_tff_formula(then_branch);
            v.visit_tff_formula(else_branch);
        }
        TFFFormula::Let { definitions, body } => {
            for def in definitions {
                match &def.definition {
                    TFFLetBody::Formula(f) => v.visit_tff_formula(f),
                    TFFLetBody::Term(t) => v.visit_tff_term(t),
                }
            }
            match body.as_ref() {
                TFFLetBody::Formula(f) => v.visit_tff_formula(f),
                TFFLetBody::Term(t) => v.visit_tff_term(t),
            }
        }
        TFFFormula::NonClassical { formula, .. } => v.visit_tff_formula(formula),
    }
}

fn walk_tff_atomic<'a, V: Visitor<'a> + ?Sized>(v: &mut V, a: &TFFAtomicFormula<'a>) {
    match a {
        TFFAtomicFormula::Plain(name, args) => {
            v.visit_atom(name, args.len());
            for arg in args {
                v.visit_tff_term(arg);
            }
        }
        TFFAtomicFormula::Defined(_, args) | TFFAtomicFormula::System(_, args) => {
            for arg in args {
                v.visit_tff_term(arg);
            }
        }
        TFFAtomicFormula::Variable(name) => v.visit_variable(name),
        TFFAtomicFormula::True | TFFAtomicFormula::False => {}
    }
}

/// Recursively walk a [`TFFTerm`], calling visitor methods on each node.
pub fn walk_tff_term<'a, V: Visitor<'a> + ?Sized>(v: &mut V, t: &TFFTerm<'a>) {
    match t {
        TFFTerm::Variable(name) => v.visit_variable(name),
        TFFTerm::Function(name, args) => {
            v.visit_atom(name, args.len());
            for arg in args {
                v.visit_tff_term(arg);
            }
        }
        TFFTerm::DefinedFunction(_, args) | TFFTerm::SystemFunction(_, args) => {
            for arg in args {
                v.visit_tff_term(arg);
            }
        }
        TFFTerm::Number(n) => v.visit_number(n),
        TFFTerm::DistinctObject(_) => {}
        TFFTerm::Conditional {
            condition,
            then_branch,
            else_branch,
        } => {
            v.visit_tff_formula(condition);
            v.visit_tff_term(then_branch);
            v.visit_tff_term(else_branch);
        }
        TFFTerm::Let { definitions, body } => {
            for def in definitions {
                match &def.definition {
                    TFFLetBody::Formula(f) => v.visit_tff_formula(f),
                    TFFLetBody::Term(t) => v.visit_tff_term(t),
                }
            }
            v.visit_tff_term(body);
        }
        TFFTerm::Tuple(terms) => {
            for term in terms {
                v.visit_tff_term(term);
            }
        }
        TFFTerm::FormulaAsTerm(f) => v.visit_tff_formula(f),
        TFFTerm::Parens(inner) => v.visit_tff_term(inner),
    }
}

// ---------------------------------------------------------------------------
// TCF walk functions
// ---------------------------------------------------------------------------

/// Recursively walk a [`TCFFormula`], calling visitor methods on each node.
///
/// TCF terms are TFF terms, so [`visit_tff_term`] is called for term nodes.
///
/// [`visit_tff_term`]: Visitor::visit_tff_term
pub fn walk_tcf_formula<'a, V: Visitor<'a> + ?Sized>(v: &mut V, f: &TCFFormula<'a>) {
    match f {
        TCFFormula::Quantified { variables, clause } => {
            for var in variables {
                v.visit_variable(var.name);
            }
            walk_tcf_clause(v, clause);
        }
        TCFFormula::Clause(clause) => walk_tcf_clause(v, clause),
    }
}

fn walk_tcf_clause<'a, V: Visitor<'a> + ?Sized>(v: &mut V, c: &TCFClause<'a>) {
    match c {
        TCFClause::Disjunction(lits) => {
            for lit in lits {
                walk_tcf_literal(v, lit);
            }
        }
        TCFClause::Parens(inner) => walk_tcf_clause(v, inner),
    }
}

fn walk_tcf_literal<'a, V: Visitor<'a> + ?Sized>(v: &mut V, l: &TCFLiteral<'a>) {
    match l {
        TCFLiteral::Positive(a) | TCFLiteral::Negative(a) => walk_tcf_atomic(v, a),
        TCFLiteral::Equality(l, r) | TCFLiteral::Inequality(l, r) => {
            v.visit_tff_term(l);
            v.visit_tff_term(r);
        }
        TCFLiteral::Parens(inner) => walk_tcf_literal(v, inner),
    }
}

fn walk_tcf_atomic<'a, V: Visitor<'a> + ?Sized>(v: &mut V, a: &TCFAtomicFormula<'a>) {
    match a {
        TCFAtomicFormula::Plain(name, args) => {
            v.visit_atom(name, args.len());
            for arg in args {
                v.visit_tff_term(arg);
            }
        }
        TCFAtomicFormula::Defined(_, args) | TCFAtomicFormula::System(_, args) => {
            for arg in args {
                v.visit_tff_term(arg);
            }
        }
        TCFAtomicFormula::True | TCFAtomicFormula::False => {}
    }
}

// ---------------------------------------------------------------------------
// THF walk functions
// ---------------------------------------------------------------------------

/// Recursively walk a [`THFFormula`], calling visitor methods on each node.
pub fn walk_thf_formula<'a, V: Visitor<'a> + ?Sized>(v: &mut V, f: &THFFormula<'a>) {
    match f {
        THFFormula::Atomic(a) => walk_thf_atomic(v, a),
        THFFormula::Variable(name) => v.visit_variable(name),
        THFFormula::Negation(inner) => v.visit_thf_formula(inner),
        THFFormula::Quantified {
            variables, formula, ..
        } => {
            for var in variables {
                v.visit_variable(var.name);
            }
            v.visit_thf_formula(formula);
        }
        THFFormula::Lambda { variables, body } => {
            for var in variables {
                v.visit_variable(var.name);
            }
            v.visit_thf_formula(body);
        }
        THFFormula::Binary { left, right, .. } => {
            v.visit_thf_formula(left);
            v.visit_thf_formula(right);
        }
        THFFormula::Application(f, g) => {
            v.visit_thf_formula(f);
            v.visit_thf_formula(g);
        }
        THFFormula::Equality(l, r) | THFFormula::Inequality(l, r) => {
            v.visit_thf_formula(l);
            v.visit_thf_formula(r);
        }
        THFFormula::Parens(inner) => v.visit_thf_formula(inner),
        THFFormula::Typed(inner, _) => v.visit_thf_formula(inner),
        THFFormula::Conditional {
            condition,
            then_branch,
            else_branch,
        } => {
            v.visit_thf_formula(condition);
            v.visit_thf_formula(then_branch);
            v.visit_thf_formula(else_branch);
        }
        THFFormula::Let { definitions, body } => {
            for def in definitions {
                v.visit_thf_formula(&def.definition);
            }
            v.visit_thf_formula(body);
        }
        THFFormula::Tuple(formulas) => {
            for f in formulas {
                v.visit_thf_formula(f);
            }
        }
        THFFormula::Number(n) => v.visit_number(n),
        THFFormula::NonClassical { formula, .. } => {
            if let Some(f) = formula {
                v.visit_thf_formula(f);
            }
        }
        // Terminal / non-formula leaves — nothing to recurse into
        THFFormula::DistinctObject(_)
        | THFFormula::ConnectiveTerm(_)
        | THFFormula::UnaryConnectiveTerm
        | THFFormula::QuantifierTerm(_)
        | THFFormula::EqualityTerm
        | THFFormula::TypeAsFormula(_) => {}
    }
}

fn walk_thf_atomic<'a, V: Visitor<'a> + ?Sized>(v: &mut V, a: &THFAtomicFormula<'a>) {
    match a {
        THFAtomicFormula::Plain(name, args) => {
            v.visit_atom(name, args.len());
            for arg in args {
                v.visit_thf_formula(arg);
            }
        }
        THFAtomicFormula::Defined(_, args) | THFAtomicFormula::System(_, args) => {
            for arg in args {
                v.visit_thf_formula(arg);
            }
        }
        THFAtomicFormula::True | THFAtomicFormula::False => {}
    }
}
