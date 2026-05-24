//! TCF (Typed Clause Form) AST types.
//!
//! TCF is essentially typed CNF - clauses with typed terms.

use super::common::*;
use super::tff::{TFFTerm, TFFType, TFFVariable};

/// A TCF statement
#[derive(Debug, Clone, PartialEq)]
pub enum TCFStatement<'a> {
    /// A logical formula (clause)
    Logical(TCFFormula<'a>),
    /// A type declaration
    Typing(TCFTyping<'a>),
}

/// A TCF type declaration
#[derive(Debug, Clone, PartialEq)]
pub struct TCFTyping<'a> {
    pub symbol: AtomicWord<'a>,
    pub typ: TFFType<'a>,
}

/// A TCF formula (clause)
#[derive(Debug, Clone, PartialEq)]
pub enum TCFFormula<'a> {
    /// Quantified clause: ! [vars : types] : clause
    Quantified {
        variables: Vec<TFFVariable<'a>>,
        clause: Box<TCFClause<'a>>,
    },
    /// Unquantified clause
    Clause(TCFClause<'a>),
}

/// A TCF clause: disjunction of literals
#[derive(Debug, Clone, PartialEq)]
pub enum TCFClause<'a> {
    /// Disjunction of literals
    Disjunction(Vec<TCFLiteral<'a>>),
    /// Parenthesized clause
    Parens(Box<TCFClause<'a>>),
}

/// A TCF literal
#[derive(Debug, Clone, PartialEq)]
pub enum TCFLiteral<'a> {
    /// Positive literal
    Positive(TCFAtomicFormula<'a>),
    /// Negative literal
    Negative(TCFAtomicFormula<'a>),
    /// Equality
    Equality(TFFTerm<'a>, TFFTerm<'a>),
    /// Inequality
    Inequality(TFFTerm<'a>, TFFTerm<'a>),
    /// Parenthesized literal
    Parens(Box<TCFLiteral<'a>>),
}

/// A TCF atomic formula
#[derive(Debug, Clone, PartialEq)]
pub enum TCFAtomicFormula<'a> {
    /// Plain atomic formula
    Plain(AtomicWord<'a>, Vec<TFFTerm<'a>>),
    /// Defined atomic formula
    Defined(DefinedWord<'a>, Vec<TFFTerm<'a>>),
    /// System atomic formula
    System(SystemWord<'a>, Vec<TFFTerm<'a>>),
    /// $true
    True,
    /// $false
    False,
}
