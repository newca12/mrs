//! THF (Typed Higher-order Form) AST types.
//!
//! THF is higher-order logic with lambda abstraction, function application,
//! and higher-order types.

use super::common::*;

/// A THF statement
#[derive(Debug, Clone, PartialEq)]
pub enum THFStatement<'a> {
    /// A logical formula
    Logical(THFFormula<'a>),
    /// A type declaration: name : type
    Typing(THFTyping<'a>),
    /// A subtype declaration: type << type
    Subtype(THFType<'a>, THFType<'a>),
    /// A sequent
    Sequent(Vec<THFFormula<'a>>, Vec<THFFormula<'a>>),
    /// A logic specification (NXF/NHF): $modal == [ ... ]
    Logic(LogicSpecification<'a>),
}

/// A THF type declaration
#[derive(Debug, Clone, PartialEq)]
pub struct THFTyping<'a> {
    pub symbol: THFTypedSymbol<'a>,
    pub typ: THFType<'a>,
}

/// A symbol being typed in THF
#[derive(Debug, Clone, PartialEq)]
pub enum THFTypedSymbol<'a> {
    /// An atomic word
    Atom(AtomicWord<'a>),
    /// A defined word
    Defined(DefinedWord<'a>),
    /// A system word
    System(SystemWord<'a>),
}

/// A THF formula
#[derive(Debug, Clone, PartialEq)]
pub enum THFFormula<'a> {
    /// Atomic formula (including constants and propositions)
    Atomic(THFAtomicFormula<'a>),
    /// Variable
    Variable(&'a str),
    /// Negation: ~F
    Negation(Box<THFFormula<'a>>),
    /// Quantified formula: Q [vars : types] : F
    Quantified {
        quantifier: THFQuantifier,
        variables: Vec<THFVariable<'a>>,
        formula: Box<THFFormula<'a>>,
    },
    /// Binary formula: F op G
    Binary {
        left: Box<THFFormula<'a>>,
        connective: THFBinaryConnective,
        right: Box<THFFormula<'a>>,
    },
    /// Application: F @ G
    Application(Box<THFFormula<'a>>, Box<THFFormula<'a>>),
    /// Lambda abstraction: ^[vars] : F
    Lambda {
        variables: Vec<THFVariable<'a>>,
        body: Box<THFFormula<'a>>,
    },
    /// Equality: F = G
    Equality(Box<THFFormula<'a>>, Box<THFFormula<'a>>),
    /// Inequality: F != G
    Inequality(Box<THFFormula<'a>>, Box<THFFormula<'a>>),
    /// Parenthesized formula
    Parens(Box<THFFormula<'a>>),
    /// Type annotation: F : type
    Typed(Box<THFFormula<'a>>, THFType<'a>),
    /// Conditional (THX): $ite(cond, then, else)
    Conditional {
        condition: Box<THFFormula<'a>>,
        then_branch: Box<THFFormula<'a>>,
        else_branch: Box<THFFormula<'a>>,
    },
    /// Let expression (THX)
    Let {
        definitions: Vec<THFLetDef<'a>>,
        body: Box<THFFormula<'a>>,
    },
    /// Tuple: [f1, f2, ...]
    Tuple(Vec<THFFormula<'a>>),
    /// A number
    Number(Number<'a>),
    /// A distinct object
    DistinctObject(&'a str),
    /// Non-classical operator (NHF): {#op}(formula) or {#op}
    NonClassical {
        operator: NonClassicalOperator<'a>,
        formula: Option<Box<THFFormula<'a>>>,
    },
    /// A connective used as a term: (&), (~), (~&), etc.
    ConnectiveTerm(THFBinaryConnective),
    /// Unary connective as term: (~)
    UnaryConnectiveTerm,
    /// TH1: Quantifier used as a term: (!!), (??), (@@+), (@@-)
    QuantifierTerm(THFQuantifier),
    /// TH1: Equality as a term: (@=)
    EqualityTerm,
    /// TH1: A type used as a formula (e.g., as argument to @=)
    TypeAsFormula(THFType<'a>),
}

impl<'a> THFFormula<'a> {
    /// Create an application F @ G
    pub fn app(f: THFFormula<'a>, g: THFFormula<'a>) -> Self {
        THFFormula::Application(Box::new(f), Box::new(g))
    }

    /// Create a lambda abstraction
    pub fn lambda(variables: Vec<THFVariable<'a>>, body: THFFormula<'a>) -> Self {
        THFFormula::Lambda {
            variables,
            body: Box::new(body),
        }
    }
}

/// A THF variable with type annotation
#[derive(Debug, Clone, PartialEq)]
pub struct THFVariable<'a> {
    pub name: &'a str,
    pub typ: Option<THFType<'a>>,
}

impl<'a> THFVariable<'a> {
    pub fn new(name: &'a str) -> Self {
        THFVariable { name, typ: None }
    }

    pub fn typed(name: &'a str, typ: THFType<'a>) -> Self {
        THFVariable {
            name,
            typ: Some(typ),
        }
    }
}

/// THF quantifiers (including higher-order quantifiers)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum THFQuantifier {
    /// ! (universal)
    Forall,
    /// ? (existential)
    Exists,
    /// ^ (lambda - treated as quantifier in some contexts)
    Lambda,
    /// @+ (choice/epsilon)
    Choice,
    /// @- (description/iota)
    Description,
    /// !> (type forall, TH1)
    ForallType,
    /// ?* (type exists, TH1)
    ExistsType,
}

impl THFQuantifier {
    pub fn as_str(&self) -> &'static str {
        match self {
            THFQuantifier::Forall => "!",
            THFQuantifier::Exists => "?",
            THFQuantifier::Lambda => "^",
            THFQuantifier::Choice => "@+",
            THFQuantifier::Description => "@-",
            THFQuantifier::ForallType => "!>",
            THFQuantifier::ExistsType => "?*",
        }
    }
}

/// THF binary connectives (extends FOF connectives)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum THFBinaryConnective {
    // Logical connectives
    /// <=> (equivalence)
    Iff,
    /// => (implication)
    Impl,
    /// <= (reverse implication)
    RevImpl,
    /// <~> (xor)
    Xor,
    /// ~| (nor)
    Nor,
    /// ~& (nand)
    Nand,
    /// | (or)
    Or,
    /// & (and)
    And,
    // Type constructors
    /// > (function type)
    Arrow,
    /// * (product type)
    Product,
    /// + (sum type)
    Sum,
    // Other
    /// := (assignment)
    Assign,
    /// == (meta identity)
    MetaIdentity,
}

impl THFBinaryConnective {
    pub fn as_str(&self) -> &'static str {
        match self {
            THFBinaryConnective::Iff => "<=>",
            THFBinaryConnective::Impl => "=>",
            THFBinaryConnective::RevImpl => "<=",
            THFBinaryConnective::Xor => "<~>",
            THFBinaryConnective::Nor => "~|",
            THFBinaryConnective::Nand => "~&",
            THFBinaryConnective::Or => "|",
            THFBinaryConnective::And => "&",
            THFBinaryConnective::Arrow => ">",
            THFBinaryConnective::Product => "*",
            THFBinaryConnective::Sum => "+",
            THFBinaryConnective::Assign => ":=",
            THFBinaryConnective::MetaIdentity => "==",
        }
    }

    /// Convert from common BinaryConnective
    pub fn from_common(c: BinaryConnective) -> Self {
        match c {
            BinaryConnective::Iff => THFBinaryConnective::Iff,
            BinaryConnective::Impl => THFBinaryConnective::Impl,
            BinaryConnective::RevImpl => THFBinaryConnective::RevImpl,
            BinaryConnective::Xor => THFBinaryConnective::Xor,
            BinaryConnective::Nor => THFBinaryConnective::Nor,
            BinaryConnective::Nand => THFBinaryConnective::Nand,
            BinaryConnective::Or => THFBinaryConnective::Or,
            BinaryConnective::And => THFBinaryConnective::And,
        }
    }
}

/// A THF atomic formula
#[derive(Debug, Clone, PartialEq)]
pub enum THFAtomicFormula<'a> {
    /// Plain atomic: f(args) or constant
    Plain(AtomicWord<'a>, Vec<THFFormula<'a>>),
    /// Defined atomic: $f(args)
    Defined(DefinedWord<'a>, Vec<THFFormula<'a>>),
    /// System atomic: $$f(args)
    System(SystemWord<'a>, Vec<THFFormula<'a>>),
    /// $true
    True,
    /// $false
    False,
}

/// A THF let definition
#[derive(Debug, Clone, PartialEq)]
pub struct THFLetDef<'a> {
    pub symbol: AtomicWord<'a>,
    pub type_args: Vec<&'a str>,
    pub params: Vec<THFVariable<'a>>,
    pub definition: THFFormula<'a>,
}

/// A THF type
#[derive(Debug, Clone, PartialEq)]
pub enum THFType<'a> {
    /// Atomic type
    Atomic(AtomicWord<'a>),
    /// Defined type ($i, $o, $int, etc.)
    Defined(THFDefinedType),
    /// Type variable
    Variable(&'a str),
    /// Function type: a > b
    Arrow(Box<THFType<'a>>, Box<THFType<'a>>),
    /// Product type: a * b
    Product(Box<THFType<'a>>, Box<THFType<'a>>),
    /// Sum type: a + b
    Sum(Box<THFType<'a>>, Box<THFType<'a>>),
    /// Type application: F(args)
    Application(Box<THFType<'a>>, Vec<THFType<'a>>),
    /// Tuple type: [t1, t2, ...]
    Tuple(Vec<THFType<'a>>),
    /// Quantified type: !> [vars] : type (for polymorphism with $tType variables)
    Quantified {
        quantifier: THFQuantifier,
        variables: Vec<&'a str>,
        typ: Box<THFType<'a>>,
    },
    /// Dependent quantified type: !> [X: T, ...] : type (for dependent types with non-$tType)
    DependentQuantified {
        quantifier: THFQuantifier,
        variables: Vec<THFVariable<'a>>,
        typ: Box<THFType<'a>>,
    },
    /// Parenthesized type
    Parens(Box<THFType<'a>>),
    /// Mapping type (for higher-kinded types)
    Mapping(Vec<THFType<'a>>, Box<THFType<'a>>),
}

impl<'a> THFType<'a> {
    /// Individual type ($i)
    pub fn individual() -> Self {
        THFType::Defined(THFDefinedType::I)
    }

    /// Boolean type ($o)
    pub fn boolean() -> Self {
        THFType::Defined(THFDefinedType::O)
    }

    /// Type type ($tType)
    pub fn ttype() -> Self {
        THFType::Defined(THFDefinedType::TType)
    }

    /// Create a function type a > b
    pub fn arrow(from: THFType<'a>, to: THFType<'a>) -> Self {
        THFType::Arrow(Box::new(from), Box::new(to))
    }
}

/// THF defined types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum THFDefinedType {
    /// $i - individual
    I,
    /// $o - boolean
    O,
    /// $int - integer
    Int,
    /// $rat - rational
    Rat,
    /// $real - real
    Real,
    /// $tType - type of types
    TType,
}

impl THFDefinedType {
    pub fn as_str(&self) -> &'static str {
        match self {
            THFDefinedType::I => "$i",
            THFDefinedType::O => "$o",
            THFDefinedType::Int => "$int",
            THFDefinedType::Rat => "$rat",
            THFDefinedType::Real => "$real",
            THFDefinedType::TType => "$tType",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "$i" | "i" => Some(THFDefinedType::I),
            "$o" | "o" => Some(THFDefinedType::O),
            "$int" | "int" => Some(THFDefinedType::Int),
            "$rat" | "rat" => Some(THFDefinedType::Rat),
            "$real" | "real" => Some(THFDefinedType::Real),
            "$tType" | "tType" => Some(THFDefinedType::TType),
            _ => None,
        }
    }
}

/// Non-classical operators (NHF - modal, temporal, epistemic)
#[derive(Debug, Clone, PartialEq)]
pub enum NonClassicalOperator<'a> {
    /// {#box} or [.] or [..] - necessity (modal)
    Box,
    /// {#dia} or <.> or <..> - possibility (modal)
    Diamond,
    /// {#always} - always (temporal)
    Always,
    /// {#eventually} - eventually (temporal)
    Eventually,
    /// {#knows} - knows (epistemic)
    Knows,
    /// {#believes} - believes (epistemic)
    Believes,
    /// Short form box with index: [#name]
    ShortBox(Option<&'a str>),
    /// Short form diamond with index: <#name>
    ShortDiamond(Option<&'a str>),
    /// Custom operator with optional index
    Custom {
        name: &'a str,
        index: Option<&'a str>,
    },
}

impl<'a> NonClassicalOperator<'a> {
    pub fn as_str(&self) -> &'static str {
        match self {
            NonClassicalOperator::Box => "{#box}",
            NonClassicalOperator::Diamond => "{#dia}",
            NonClassicalOperator::Always => "{#always}",
            NonClassicalOperator::Eventually => "{#eventually}",
            NonClassicalOperator::Knows => "{#knows}",
            NonClassicalOperator::Believes => "{#believes}",
            NonClassicalOperator::ShortBox(_) => "[.]",
            NonClassicalOperator::ShortDiamond(_) => "<.>",
            NonClassicalOperator::Custom { .. } => "{#custom}",
        }
    }
}

/// Logic specification for non-classical logics (NXF/NHF)
#[derive(Debug, Clone, PartialEq)]
pub struct LogicSpecification<'a> {
    /// The logic family (e.g., $modal, $alethic_modal, $epistemic_modal)
    pub logic_family: &'a str,
    /// Properties of the logic
    pub properties: Vec<LogicProperty<'a>>,
}

/// A property in a logic specification
#[derive(Debug, Clone, PartialEq)]
pub enum LogicProperty<'a> {
    /// Simple key-value: key == value
    KeyValue { key: &'a str, value: LogicValue<'a> },
    /// Nested list: key == [ ... ]
    KeyList {
        key: &'a str,
        values: Vec<LogicProperty<'a>>,
    },
}

/// A value in a logic specification
#[derive(Debug, Clone, PartialEq)]
pub enum LogicValue<'a> {
    /// A simple identifier (e.g., $rigid, $constant)
    Atom(&'a str),
    /// A quoted string (e.g., 'LOG002_1.l')
    String(&'a str),
    /// A list of values
    List(Vec<LogicValue<'a>>),
    /// A property assignment: name == value
    Property {
        name: &'a str,
        value: Box<LogicValue<'a>>,
    },
}
