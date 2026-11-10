//! AST evaluation deliberately uses token variants, not symbol-name heuristics:
//! the quoted user symbol `'$sum'` is not the arithmetic functor `$sum`.

use crate::{Error, Evaluator, Value};
use mrs_tptp::{
    AnnotatedFormula, BinaryConnective, CNFAtomicFormula, CNFFormula, CNFLiteral, CNFStatement,
    FOFAtomicFormula, FOFFormula, FOFStatement, FOFTerm, TCFAtomicFormula, TCFClause, TCFFormula,
    TCFLiteral, TCFStatement, TFFAtomicFormula, TFFFormula, TFFStatement, TFFTerm,
};

impl Evaluator {
    /// Evaluate an entire ground formula in the fixed integer/rational
    /// interpretation. Every atom must be supported; unknown atoms are not
    /// assigned truth values, including in otherwise short-circuiting formulas.
    pub fn evaluate(&mut self, input: &AnnotatedFormula<'_>) -> Result<bool, Error> {
        match input {
            AnnotatedFormula::FOF(f) => match &f.formula {
                FOFStatement::Logical(f) => self.fof(f, 0),
                _ => Err(Error::Unsupported),
            },
            AnnotatedFormula::CNF(f) => {
                let CNFStatement::Logical(f) = &f.formula;
                self.cnf(f, 0)
            }
            AnnotatedFormula::TFF(f) => match &f.formula {
                TFFStatement::Logical(f) => self.tff(f, 0),
                _ => Err(Error::Unsupported),
            },
            AnnotatedFormula::TCF(f) => match &f.formula {
                TCFStatement::Logical(TCFFormula::Clause(c)) => self.tcf_clause(c, 0),
                _ => Err(Error::Unsupported),
            },
            _ => Err(Error::Unsupported),
        }
    }

    fn fof(&mut self, f: &FOFFormula<'_>, depth: usize) -> Result<bool, Error> {
        self.enter(depth)?;
        match f {
            FOFFormula::Atomic(a) => self.fof_atom(a, depth + 1),
            FOFFormula::Negation(f) => Ok(!self.fof(f, depth + 1)?),
            FOFFormula::Parens(f) => self.fof(f, depth + 1),
            FOFFormula::Binary {
                left,
                connective,
                right,
            } => {
                let l = self.fof(left, depth + 1)?;
                let r = self.fof(right, depth + 1)?;
                Ok(binary(*connective, l, r))
            }
            FOFFormula::Equality(l, r) | FOFFormula::Inequality(l, r) => {
                let args = [self.fof_term(l, depth + 1)?, self.fof_term(r, depth + 1)?];
                let equal = self.predicate("=", &args)?;
                Ok(equal != matches!(f, FOFFormula::Inequality(..)))
            }
            _ => Err(Error::Unsupported),
        }
    }

    fn fof_atom(&mut self, atom: &FOFAtomicFormula<'_>, depth: usize) -> Result<bool, Error> {
        self.enter(depth)?;
        match atom {
            FOFAtomicFormula::True => Ok(true),
            FOFAtomicFormula::False => Ok(false),
            FOFAtomicFormula::Defined(name, args) => {
                if let [FOFTerm::Function(a, aa), FOFTerm::Function(b, ba)] = args.as_slice()
                    && aa.is_empty()
                    && ba.is_empty()
                {
                    self.enter(depth + 1)?;
                    self.enter(depth + 1)?;
                    self.charge((a.as_str().len() as u64).saturating_add(b.as_str().len() as u64))?;
                    if a == b
                        && let Some(value) = reflexive_relation(name.0)
                    {
                        return Ok(value);
                    }
                }
                let args = args
                    .iter()
                    .map(|t| self.fof_term(t, depth + 1))
                    .collect::<Result<Vec<_>, _>>()?;
                self.predicate(name.0, &args)
            }
            _ => Err(Error::Unsupported),
        }
    }

    fn fof_term(&mut self, term: &FOFTerm<'_>, depth: usize) -> Result<Value, Error> {
        self.enter(depth)?;
        match term {
            FOFTerm::Number(n) => self.number(n),
            FOFTerm::DefinedFunction(name, args) => {
                let args = args
                    .iter()
                    .map(|t| self.fof_term(t, depth + 1))
                    .collect::<Result<Vec<_>, _>>()?;
                self.function(name.0, &args)
            }
            _ => Err(Error::Unsupported),
        }
    }

    fn cnf(&mut self, f: &CNFFormula<'_>, depth: usize) -> Result<bool, Error> {
        self.enter(depth)?;
        match f {
            CNFFormula::Parens(f) => self.cnf(f, depth + 1),
            CNFFormula::Disjunction(literals) => {
                let mut value = false;
                for literal in literals {
                    self.enter(depth + 1)?;
                    value |= match literal {
                        CNFLiteral::Positive(a) => self.cnf_atom(a, depth + 2)?,
                        CNFLiteral::Negative(a) => !self.cnf_atom(a, depth + 2)?,
                        CNFLiteral::Equality(l, r) | CNFLiteral::Inequality(l, r) => {
                            let args = [self.fof_term(l, depth + 2)?, self.fof_term(r, depth + 2)?];
                            self.predicate("=", &args)?
                                != matches!(literal, CNFLiteral::Inequality(..))
                        }
                    };
                }
                Ok(value)
            }
        }
    }

    fn cnf_atom(&mut self, atom: &CNFAtomicFormula<'_>, depth: usize) -> Result<bool, Error> {
        self.enter(depth)?;
        match atom {
            CNFAtomicFormula::True => Ok(true),
            CNFAtomicFormula::False => Ok(false),
            CNFAtomicFormula::Defined(name, args) => {
                let args = args
                    .iter()
                    .map(|t| self.fof_term(t, depth + 1))
                    .collect::<Result<Vec<_>, _>>()?;
                self.predicate(name.0, &args)
            }
            _ => Err(Error::Unsupported),
        }
    }

    fn tff(&mut self, f: &TFFFormula<'_>, depth: usize) -> Result<bool, Error> {
        self.enter(depth)?;
        match f {
            TFFFormula::Atomic(a) => self.tff_atom(a, depth + 1),
            TFFFormula::Negation(f) => Ok(!self.tff(f, depth + 1)?),
            TFFFormula::Parens(f) => self.tff(f, depth + 1),
            TFFFormula::Binary {
                left,
                connective,
                right,
            } => {
                let l = self.tff(left, depth + 1)?;
                let r = self.tff(right, depth + 1)?;
                Ok(binary(*connective, l, r))
            }
            TFFFormula::Equality(l, r) | TFFFormula::Inequality(l, r) => {
                let args = [self.tff_term(l, depth + 1)?, self.tff_term(r, depth + 1)?];
                let equal = self.predicate("=", &args)?;
                Ok(equal != matches!(f, TFFFormula::Inequality(..)))
            }
            _ => Err(Error::Unsupported),
        }
    }

    fn tff_atom(&mut self, atom: &TFFAtomicFormula<'_>, depth: usize) -> Result<bool, Error> {
        self.enter(depth)?;
        match atom {
            TFFAtomicFormula::True => Ok(true),
            TFFAtomicFormula::False => Ok(false),
            TFFAtomicFormula::Defined(name, args) => {
                if let [TFFTerm::Function(a, aa), TFFTerm::Function(b, ba)] = args.as_slice()
                    && aa.is_empty()
                    && ba.is_empty()
                {
                    self.enter(depth + 1)?;
                    self.enter(depth + 1)?;
                    self.charge((a.as_str().len() as u64).saturating_add(b.as_str().len() as u64))?;
                    if a == b
                        && let Some(value) = reflexive_relation(name.0)
                    {
                        return Ok(value);
                    }
                }
                let args = args
                    .iter()
                    .map(|t| self.tff_term(t, depth + 1))
                    .collect::<Result<Vec<_>, _>>()?;
                self.predicate(name.0, &args)
            }
            _ => Err(Error::Unsupported),
        }
    }

    fn tff_term(&mut self, term: &TFFTerm<'_>, depth: usize) -> Result<Value, Error> {
        self.enter(depth)?;
        match term {
            TFFTerm::Number(n) => self.number(n),
            TFFTerm::Parens(t) => self.tff_term(t, depth + 1),
            TFFTerm::DefinedFunction(name, args) => {
                let args = args
                    .iter()
                    .map(|t| self.tff_term(t, depth + 1))
                    .collect::<Result<Vec<_>, _>>()?;
                self.function(name.0, &args)
            }
            _ => Err(Error::Unsupported),
        }
    }

    fn tcf_clause(&mut self, clause: &TCFClause<'_>, depth: usize) -> Result<bool, Error> {
        self.enter(depth)?;
        match clause {
            TCFClause::Parens(c) => self.tcf_clause(c, depth + 1),
            TCFClause::Disjunction(literals) => {
                let mut value = false;
                for literal in literals {
                    value |= self.tcf_literal(literal, depth + 1)?;
                }
                Ok(value)
            }
        }
    }

    fn tcf_literal(&mut self, literal: &TCFLiteral<'_>, depth: usize) -> Result<bool, Error> {
        self.enter(depth)?;
        match literal {
            TCFLiteral::Parens(l) => self.tcf_literal(l, depth + 1),
            TCFLiteral::Positive(a) => self.tcf_atom(a, depth + 1),
            TCFLiteral::Negative(a) => Ok(!self.tcf_atom(a, depth + 1)?),
            TCFLiteral::Equality(l, r) | TCFLiteral::Inequality(l, r) => {
                let args = [self.tff_term(l, depth + 1)?, self.tff_term(r, depth + 1)?];
                Ok(self.predicate("=", &args)? != matches!(literal, TCFLiteral::Inequality(..)))
            }
        }
    }

    fn tcf_atom(&mut self, atom: &TCFAtomicFormula<'_>, depth: usize) -> Result<bool, Error> {
        self.enter(depth)?;
        match atom {
            TCFAtomicFormula::True => Ok(true),
            TCFAtomicFormula::False => Ok(false),
            TCFAtomicFormula::Defined(name, args) => {
                let args = args
                    .iter()
                    .map(|t| self.tff_term(t, depth + 1))
                    .collect::<Result<Vec<_>, _>>()?;
                self.predicate(name.0, &args)
            }
            _ => Err(Error::Unsupported),
        }
    }
}

fn reflexive_relation(name: &str) -> Option<bool> {
    match name {
        "less" | "greater" => Some(false),
        "lesseq" | "greatereq" => Some(true),
        _ => None,
    }
}

fn binary(connective: BinaryConnective, l: bool, r: bool) -> bool {
    match connective {
        BinaryConnective::And => l && r,
        BinaryConnective::Or => l || r,
        BinaryConnective::Impl => !l || r,
        BinaryConnective::RevImpl => !r || l,
        BinaryConnective::Iff => l == r,
        BinaryConnective::Xor => l != r,
        BinaryConnective::Nor => !(l || r),
        BinaryConnective::Nand => !(l && r),
    }
}
