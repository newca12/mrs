//! TFF (Typed First-order Form) AST types.
//!
//! TFF extends FOF with a type system. TF0 is monomorphic (no type variables),
//! TF1 adds polymorphism with type quantifiers.

use super::common::*;
use super::fof::FOFTerm;
use super::thf::{LogicSpecification, NonClassicalOperator};

/// A TFF statement
#[derive(Debug, Clone, PartialEq)]
pub enum TFFStatement<'a> {
    /// A logical formula
    Logical(TFFFormula<'a>),
    /// A type declaration: name : type
    Typing(TFFTyping<'a>),
    /// A sequent
    Sequent(Vec<TFFFormula<'a>>, Vec<TFFFormula<'a>>),
    /// A logic specification (NXF): $modal == [ ... ]
    Logic(LogicSpecification<'a>),
}

/// A TFF type declaration
#[derive(Debug, Clone, PartialEq)]
pub struct TFFTyping<'a> {
    /// The symbol being typed
    pub symbol: TypedSymbol<'a>,
    /// The type
    pub typ: TFFType<'a>,
}

/// A symbol being typed
#[derive(Debug, Clone, PartialEq)]
pub enum TypedSymbol<'a> {
    /// An atomic word
    Atom(AtomicWord<'a>),
    /// A defined word
    Defined(DefinedWord<'a>),
}

/// A TFF formula
#[derive(Debug, Clone, PartialEq)]
pub enum TFFFormula<'a> {
    /// Atomic formula
    Atomic(TFFAtomicFormula<'a>),
    /// Negation: ~F
    Negation(Box<TFFFormula<'a>>),
    /// Quantified formula: Q [vars : types] : F
    Quantified {
        quantifier: Quantifier,
        variables: Vec<TFFVariable<'a>>,
        formula: Box<TFFFormula<'a>>,
    },
    /// Type-quantified formula (TF1): !> [types] : F or ?* [types] : F
    TypeQuantified {
        quantifier: TypeQuantifier,
        type_variables: Vec<&'a str>,
        formula: Box<TFFFormula<'a>>,
    },
    /// Binary formula
    Binary {
        left: Box<TFFFormula<'a>>,
        connective: BinaryConnective,
        right: Box<TFFFormula<'a>>,
    },
    /// Equality: term = term
    Equality(TFFTerm<'a>, TFFTerm<'a>),
    /// Inequality: term != term
    Inequality(TFFTerm<'a>, TFFTerm<'a>),
    /// Parenthesized formula
    Parens(Box<TFFFormula<'a>>),
    /// Conditional (TXF): $ite(cond, then, else)
    Conditional {
        condition: Box<TFFFormula<'a>>,
        then_branch: Box<TFFFormula<'a>>,
        else_branch: Box<TFFFormula<'a>>,
    },
    /// Let expression (TXF): $let(definitions, body)
    /// Body can be either a formula or a term (e.g., tuple for parallel assignment)
    Let {
        definitions: Vec<TFFLetDef<'a>>,
        body: Box<TFFLetBody<'a>>,
    },
    /// Non-classical formula (NXF): [.](F), <.>(F), [#name](F), <#name>(F)
    NonClassical {
        operator: NonClassicalOperator<'a>,
        formula: Box<TFFFormula<'a>>,
    },
}

/// A TFF variable with optional type annotation
#[derive(Debug, Clone, PartialEq)]
pub struct TFFVariable<'a> {
    pub name: &'a str,
    pub typ: Option<TFFType<'a>>,
}

impl<'a> TFFVariable<'a> {
    pub fn new(name: &'a str) -> Self {
        TFFVariable { name, typ: None }
    }

    pub fn typed(name: &'a str, typ: TFFType<'a>) -> Self {
        TFFVariable {
            name,
            typ: Some(typ),
        }
    }
}

/// Type quantifier (TF1 polymorphism)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeQuantifier {
    /// !> (type forall)
    ForallType,
    /// ?* (type exists)
    ExistsType,
}

impl TypeQuantifier {
    pub fn as_str(&self) -> &'static str {
        match self {
            TypeQuantifier::ForallType => "!>",
            TypeQuantifier::ExistsType => "?*",
        }
    }
}

/// A TFF atomic formula
#[derive(Debug, Clone, PartialEq)]
pub enum TFFAtomicFormula<'a> {
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
    /// Variable as formula (FOOL/TXF: boolean-typed variable)
    Variable(&'a str),
}

/// A TFF term
#[derive(Debug, Clone, PartialEq)]
pub enum TFFTerm<'a> {
    /// A variable
    Variable(&'a str),
    /// Function application
    Function(AtomicWord<'a>, Vec<TFFTerm<'a>>),
    /// Defined function
    DefinedFunction(DefinedWord<'a>, Vec<TFFTerm<'a>>),
    /// System function
    SystemFunction(SystemWord<'a>, Vec<TFFTerm<'a>>),
    /// A number
    Number(Number<'a>),
    /// A distinct object
    DistinctObject(&'a str),
    /// Conditional term (TXF): $ite(cond, then, else)
    Conditional {
        condition: Box<TFFFormula<'a>>,
        then_branch: Box<TFFTerm<'a>>,
        else_branch: Box<TFFTerm<'a>>,
    },
    /// Let term (TXF)
    Let {
        definitions: Vec<TFFLetDef<'a>>,
        body: Box<TFFTerm<'a>>,
    },
    /// Tuple (TXF): [t1, t2, ...]
    Tuple(Vec<TFFTerm<'a>>),
    /// Formula as term (FOOL/TXF): a parenthesized formula in a term position
    FormulaAsTerm(Box<TFFFormula<'a>>),
    /// Explicit parentheses around a term (for roundtrip preservation)
    Parens(Box<TFFTerm<'a>>),
}

/// A TFF let definition
#[derive(Debug, Clone, PartialEq)]
pub struct TFFLetDef<'a> {
    /// The symbol being defined
    pub symbol: AtomicWord<'a>,
    /// Optional type arguments
    pub type_args: Vec<&'a str>,
    /// Parameters with types
    pub params: Vec<TFFVariable<'a>>,
    /// The type of the symbol (TFX)
    pub typ: Option<TFFType<'a>>,
    /// The definition (formula or term)
    pub definition: TFFLetBody<'a>,
}

/// Body of a let definition
#[derive(Debug, Clone, PartialEq)]
pub enum TFFLetBody<'a> {
    Formula(TFFFormula<'a>),
    Term(TFFTerm<'a>),
}

/// A TFF type
#[derive(Debug, Clone, PartialEq)]
pub enum TFFType<'a> {
    /// Atomic type (type name)
    Atomic(AtomicWord<'a>),
    /// Defined type ($i, $o, $int, $rat, $real, $tType)
    Defined(DefinedType),
    /// Type variable (TF1)
    Variable(&'a str),
    /// Function type: arg1 * arg2 * ... > result
    Function {
        args: Vec<TFFType<'a>>,
        result: Box<TFFType<'a>>,
    },
    /// Type application: type(args)
    Application(Box<TFFType<'a>>, Vec<TFFType<'a>>),
    /// Tuple type: [t1, t2, ...] (TXF)
    Tuple(Vec<TFFType<'a>>),
    /// Type quantification (TF1): !> [vars] : type
    Quantified {
        variables: Vec<&'a str>,
        typ: Box<TFFType<'a>>,
    },
    /// Parenthesized type
    Parens(Box<TFFType<'a>>),
}

impl<'a> TFFType<'a> {
    /// Individual type ($i)
    pub fn individual() -> Self {
        TFFType::Defined(DefinedType::I)
    }

    /// Boolean type ($o)
    pub fn boolean() -> Self {
        TFFType::Defined(DefinedType::O)
    }

    /// Type type ($tType)
    pub fn ttype() -> Self {
        TFFType::Defined(DefinedType::TType)
    }
}

/// Defined types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefinedType {
    /// $i - individual
    I,
    /// $o - boolean/proposition
    O,
    /// $int - integer
    Int,
    /// $rat - rational
    Rat,
    /// $real - real
    Real,
    /// $tType - the type of types
    TType,
}

impl DefinedType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DefinedType::I => "$i",
            DefinedType::O => "$o",
            DefinedType::Int => "$int",
            DefinedType::Rat => "$rat",
            DefinedType::Real => "$real",
            DefinedType::TType => "$tType",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "$i" | "i" => Some(DefinedType::I),
            "$o" | "o" => Some(DefinedType::O),
            "$int" | "int" => Some(DefinedType::Int),
            "$rat" | "rat" => Some(DefinedType::Rat),
            "$real" | "real" => Some(DefinedType::Real),
            "$tType" | "tType" => Some(DefinedType::TType),
            _ => None,
        }
    }
}

/// Conversion from FOF terms
impl<'a> From<FOFTerm<'a>> for TFFTerm<'a> {
    fn from(term: FOFTerm<'a>) -> Self {
        match term {
            FOFTerm::Variable(v) => TFFTerm::Variable(v),
            FOFTerm::Function(f, args) => {
                TFFTerm::Function(f, args.into_iter().map(Into::into).collect())
            }
            FOFTerm::DefinedFunction(f, args) => {
                TFFTerm::DefinedFunction(f, args.into_iter().map(Into::into).collect())
            }
            FOFTerm::SystemFunction(f, args) => {
                TFFTerm::SystemFunction(f, args.into_iter().map(Into::into).collect())
            }
            FOFTerm::Number(n) => TFFTerm::Number(n),
            FOFTerm::DistinctObject(s) => TFFTerm::DistinctObject(s),
        }
    }
}
