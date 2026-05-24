//! FOF (First-Order Form) AST types.
//!
//! FOF formulas are full first-order logic with quantifiers, connectives,
//! and untyped terms.

use super::common::*;

/// A FOF statement (the formula part of a fof() annotated formula)
#[derive(Debug, Clone, PartialEq)]
pub enum FOFStatement<'a> {
    /// A logical formula
    Logical(FOFFormula<'a>),
    /// A sequent: [assumptions] --> [conclusions]
    Sequent(Vec<FOFFormula<'a>>, Vec<FOFFormula<'a>>),
}

/// A FOF formula
#[derive(Debug, Clone, PartialEq)]
pub enum FOFFormula<'a> {
    /// Atomic formula (predicate application)
    Atomic(FOFAtomicFormula<'a>),
    /// Negation: ~F
    Negation(Box<FOFFormula<'a>>),
    /// Quantified formula: Q [vars] : F
    Quantified {
        quantifier: Quantifier,
        variables: Vec<&'a str>,
        formula: Box<FOFFormula<'a>>,
    },
    /// Binary formula: F op G
    Binary {
        left: Box<FOFFormula<'a>>,
        connective: BinaryConnective,
        right: Box<FOFFormula<'a>>,
    },
    /// Infix equality: term = term
    Equality(FOFTerm<'a>, FOFTerm<'a>),
    /// Infix inequality: term != term
    Inequality(FOFTerm<'a>, FOFTerm<'a>),
    /// Parenthesized formula
    Parens(Box<FOFFormula<'a>>),
}

impl<'a> FOFFormula<'a> {
    /// Create an atomic formula from a predicate and arguments
    pub fn atomic(predicate: AtomicWord<'a>, args: Vec<FOFTerm<'a>>) -> Self {
        FOFFormula::Atomic(FOFAtomicFormula::Plain(predicate, args))
    }

    /// Create a negation
    pub fn negation(formula: FOFFormula<'a>) -> Self {
        FOFFormula::Negation(Box::new(formula))
    }

    /// Create a universally quantified formula
    pub fn forall(variables: Vec<&'a str>, formula: FOFFormula<'a>) -> Self {
        FOFFormula::Quantified {
            quantifier: Quantifier::Forall,
            variables,
            formula: Box::new(formula),
        }
    }

    /// Create an existentially quantified formula
    pub fn exists(variables: Vec<&'a str>, formula: FOFFormula<'a>) -> Self {
        FOFFormula::Quantified {
            quantifier: Quantifier::Exists,
            variables,
            formula: Box::new(formula),
        }
    }

    /// Create a binary formula
    pub fn binary(left: FOFFormula<'a>, conn: BinaryConnective, right: FOFFormula<'a>) -> Self {
        FOFFormula::Binary {
            left: Box::new(left),
            connective: conn,
            right: Box::new(right),
        }
    }

    /// Create a conjunction (and)
    pub fn and(left: FOFFormula<'a>, right: FOFFormula<'a>) -> Self {
        Self::binary(left, BinaryConnective::And, right)
    }

    /// Create a disjunction (or)
    pub fn or(left: FOFFormula<'a>, right: FOFFormula<'a>) -> Self {
        Self::binary(left, BinaryConnective::Or, right)
    }

    /// Create an implication
    pub fn implies(left: FOFFormula<'a>, right: FOFFormula<'a>) -> Self {
        Self::binary(left, BinaryConnective::Impl, right)
    }

    /// Create an equivalence (iff)
    pub fn iff(left: FOFFormula<'a>, right: FOFFormula<'a>) -> Self {
        Self::binary(left, BinaryConnective::Iff, right)
    }
}

/// A FOF atomic formula
#[derive(Debug, Clone, PartialEq)]
pub enum FOFAtomicFormula<'a> {
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

impl<'a> FOFAtomicFormula<'a> {
    /// Create a plain atomic formula
    pub fn plain(predicate: AtomicWord<'a>, args: Vec<FOFTerm<'a>>) -> Self {
        FOFAtomicFormula::Plain(predicate, args)
    }

    /// Create a proposition (0-ary predicate)
    pub fn proposition(name: AtomicWord<'a>) -> Self {
        FOFAtomicFormula::Plain(name, Vec::new())
    }
}

/// A FOF term
#[derive(Debug, Clone, PartialEq)]
pub enum FOFTerm<'a> {
    /// A variable
    Variable(&'a str),
    /// Function application: f(args)
    Function(AtomicWord<'a>, Vec<FOFTerm<'a>>),
    /// Defined function: $f(args)
    DefinedFunction(DefinedWord<'a>, Vec<FOFTerm<'a>>),
    /// System function: $$f(args)
    SystemFunction(SystemWord<'a>, Vec<FOFTerm<'a>>),
    /// A number
    Number(Number<'a>),
    /// A distinct object "..."
    DistinctObject(&'a str),
}

impl<'a> FOFTerm<'a> {
    /// Create a variable term
    pub fn variable(name: &'a str) -> Self {
        FOFTerm::Variable(name)
    }

    /// Create a function term
    pub fn function(name: AtomicWord<'a>, args: Vec<FOFTerm<'a>>) -> Self {
        FOFTerm::Function(name, args)
    }

    /// Create a constant (0-ary function)
    pub fn constant(name: AtomicWord<'a>) -> Self {
        FOFTerm::Function(name, Vec::new())
    }

    /// Check if this is a variable
    pub fn is_variable(&self) -> bool {
        matches!(self, FOFTerm::Variable(_))
    }

    /// Check if this is a ground term (no variables)
    pub fn is_ground(&self) -> bool {
        match self {
            FOFTerm::Variable(_) => false,
            FOFTerm::Function(_, args)
            | FOFTerm::DefinedFunction(_, args)
            | FOFTerm::SystemFunction(_, args) => args.iter().all(|a| a.is_ground()),
            FOFTerm::Number(_) | FOFTerm::DistinctObject(_) => true,
        }
    }
}
