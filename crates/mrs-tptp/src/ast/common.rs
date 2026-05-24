//! Common types shared across TPTP dialects.

/// A name in TPTP (used for formula names).
/// Names can be lower_word, single_quoted, or integer (anonymous).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Name<'a> {
    /// A lower_word: starts with lowercase, alphanumeric + underscore
    Lower(&'a str),
    /// A single-quoted string 'content'
    SingleQuoted(&'a str),
    /// An integer (for anonymous names)
    Integer(&'a str),
}

impl<'a> Name<'a> {
    pub fn as_str(&self) -> &'a str {
        match self {
            Name::Lower(s) => s,
            Name::SingleQuoted(s) => s,
            Name::Integer(s) => s,
        }
    }
}

/// A number in TPTP (integer, rational, or real).
/// Uses borrowed string slices for zero-copy parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Number<'a> {
    /// Integer number (e.g., "42", "-17")
    Integer(&'a str),
    /// Rational number (e.g., "3/4", "-1/2")
    Rational(&'a str),
    /// Real number (e.g., "3.14", "1.0e10")
    Real(&'a str),
}

impl<'a> Number<'a> {
    /// Get the raw string representation
    pub fn as_str(&self) -> &'a str {
        match self {
            Number::Integer(s) => s,
            Number::Rational(s) => s,
            Number::Real(s) => s,
        }
    }
}

/// General term used in annotations
#[derive(Debug, Clone, PartialEq)]
pub enum GeneralTerm<'a> {
    /// A word (lower_word, single_quoted, or defined)
    Word(AtomicWord<'a>),
    /// A number
    Number(Number<'a>),
    /// A distinct object "..."
    DistinctObject(&'a str),
    /// A variable
    Variable(&'a str),
    /// Function application: word(args)
    Function(AtomicWord<'a>, Vec<GeneralTerm<'a>>),
    /// A list [...]
    List(Vec<GeneralTerm<'a>>),
    /// Colon pair: term : term
    ColonPair(Box<GeneralTerm<'a>>, Box<GeneralTerm<'a>>),
    /// Formula as general term
    Formula(Box<GeneralData<'a>>),
}

/// General data wrapper for formulas in annotations
#[derive(Debug, Clone, PartialEq)]
pub enum GeneralData<'a> {
    /// $thf(formula)
    THF(Box<super::thf::THFFormula<'a>>),
    /// $tff(formula)
    TFF(Box<super::tff::TFFFormula<'a>>),
    /// $fof(formula)
    FOF(Box<super::fof::FOFFormula<'a>>),
    /// $cnf(formula)
    CNF(Box<super::cnf::CNFFormula<'a>>),
    /// $fot(term)
    FOT(Box<super::fof::FOFTerm<'a>>),
}

/// An atomic word: lower_word, single_quoted, or defined word
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AtomicWord<'a> {
    /// A lower_word: starts with lowercase, alphanumeric + underscore
    Lower(&'a str),
    /// A single-quoted string 'content'
    SingleQuoted(&'a str),
}

impl<'a> AtomicWord<'a> {
    pub fn as_str(&self) -> &'a str {
        match self {
            AtomicWord::Lower(s) => s,
            AtomicWord::SingleQuoted(s) => s,
        }
    }
}

/// A defined word ($name)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DefinedWord<'a>(pub &'a str);

/// A system word ($$name)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SystemWord<'a>(pub &'a str);

/// Binary connectives shared across dialects
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryConnective {
    /// <=> (equivalence, iff)
    Iff,
    /// => (implication)
    Impl,
    /// <= (reverse implication)
    RevImpl,
    /// <~> (exclusive or, xor)
    Xor,
    /// ~| (nor)
    Nor,
    /// ~& (nand)
    Nand,
    /// | (disjunction, or)
    Or,
    /// & (conjunction, and)
    And,
}

impl BinaryConnective {
    pub fn as_str(&self) -> &'static str {
        match self {
            BinaryConnective::Iff => "<=>",
            BinaryConnective::Impl => "=>",
            BinaryConnective::RevImpl => "<=",
            BinaryConnective::Xor => "<~>",
            BinaryConnective::Nor => "~|",
            BinaryConnective::Nand => "~&",
            BinaryConnective::Or => "|",
            BinaryConnective::And => "&",
        }
    }

    /// Precedence level (higher binds tighter)
    pub fn precedence(&self) -> u8 {
        match self {
            BinaryConnective::Iff
            | BinaryConnective::Impl
            | BinaryConnective::RevImpl
            | BinaryConnective::Xor
            | BinaryConnective::Nor
            | BinaryConnective::Nand => 1,
            BinaryConnective::Or => 2,
            BinaryConnective::And => 3,
        }
    }

    /// Whether this connective is associative
    pub fn is_associative(&self) -> bool {
        matches!(self, BinaryConnective::Or | BinaryConnective::And)
    }
}

/// Unary connective (negation)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryConnective {
    /// ~ (negation)
    Not,
}

impl UnaryConnective {
    pub fn as_str(&self) -> &'static str {
        match self {
            UnaryConnective::Not => "~",
        }
    }
}

/// Quantifier for first-order formulas
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quantifier {
    /// ! (universal, forall)
    Forall,
    /// ? (existential, exists)
    Exists,
}

impl Quantifier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Quantifier::Forall => "!",
            Quantifier::Exists => "?",
        }
    }
}

/// Infix equality/inequality
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfixEquality {
    /// = (equality)
    Equal,
    /// != (inequality)
    NotEqual,
}

impl InfixEquality {
    pub fn as_str(&self) -> &'static str {
        match self {
            InfixEquality::Equal => "=",
            InfixEquality::NotEqual => "!=",
        }
    }
}
