//! CNF (Clause Normal Form) AST types.
//!
//! CNF formulas are disjunctions of literals, where literals are atomic formulas
//! or their negations. Variables are implicitly universally quantified.

use super::common::*;
use super::fof::FOFTerm;

/// A CNF statement (the formula part of a cnf() annotated formula)
#[derive(Debug, Clone, PartialEq)]
pub enum CNFStatement<'a> {
    /// A logical formula (disjunction of literals)
    Logical(CNFFormula<'a>),
}

/// A CNF formula: a disjunction of literals or parenthesized formula
#[derive(Debug, Clone, PartialEq)]
pub enum CNFFormula<'a> {
    /// Disjunction of literals
    Disjunction(Vec<CNFLiteral<'a>>),
    /// Parenthesized formula
    Parens(Box<CNFFormula<'a>>),
}

impl<'a> CNFFormula<'a> {
    /// Create a disjunction from literals
    pub fn disjunction(literals: Vec<CNFLiteral<'a>>) -> Self {
        CNFFormula::Disjunction(literals)
    }

    /// Get all literals in this formula
    pub fn literals(&self) -> Vec<&CNFLiteral<'a>> {
        match self {
            CNFFormula::Disjunction(lits) => lits.iter().collect(),
            CNFFormula::Parens(inner) => inner.literals(),
        }
    }
}

/// A CNF literal: an atomic formula or its negation
#[derive(Debug, Clone, PartialEq)]
pub enum CNFLiteral<'a> {
    /// Positive literal
    Positive(CNFAtomicFormula<'a>),
    /// Negative literal (negated atomic formula)
    Negative(CNFAtomicFormula<'a>),
    /// Infix equality: term = term
    Equality(FOFTerm<'a>, FOFTerm<'a>),
    /// Infix inequality: term != term
    Inequality(FOFTerm<'a>, FOFTerm<'a>),
}

impl<'a> CNFLiteral<'a> {
    /// Check if this literal is positive
    pub fn is_positive(&self) -> bool {
        matches!(self, CNFLiteral::Positive(_) | CNFLiteral::Equality(_, _))
    }

    /// Check if this literal is negative
    pub fn is_negative(&self) -> bool {
        !self.is_positive()
    }
}

/// A CNF atomic formula: predicate application or defined/system predicate
#[derive(Debug, Clone, PartialEq)]
pub enum CNFAtomicFormula<'a> {
    /// Plain atomic formula: predicate(args) or proposition
    Plain(AtomicWord<'a>, Vec<FOFTerm<'a>>),
    /// Defined atomic formula: $predicate(args)
    Defined(DefinedWord<'a>, Vec<FOFTerm<'a>>),
    /// System atomic formula: $$predicate(args)
    System(SystemWord<'a>, Vec<FOFTerm<'a>>),
    /// $true
    True,
    /// $false
    False,
}

impl<'a> CNFAtomicFormula<'a> {
    /// Create a plain atomic formula
    pub fn plain(predicate: AtomicWord<'a>, args: Vec<FOFTerm<'a>>) -> Self {
        CNFAtomicFormula::Plain(predicate, args)
    }

    /// Create a proposition (0-ary predicate)
    pub fn proposition(name: AtomicWord<'a>) -> Self {
        CNFAtomicFormula::Plain(name, Vec::new())
    }
}
