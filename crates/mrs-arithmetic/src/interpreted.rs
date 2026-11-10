//! Conservative, bounded detection of input that must not be erased into
//! ordinary first-order symbols. Returning true on exhaustion is deliberate.

use mrs_tptp::{
    AnnotatedFormula, CNFAtomicFormula, CNFFormula, CNFLiteral, CNFStatement, DefinedType,
    FOFAtomicFormula, FOFFormula, FOFStatement, FOFTerm, TCFAtomicFormula, TCFClause, TCFFormula,
    TCFLiteral, TCFStatement, TFFAtomicFormula, TFFFormula, TFFStatement, TFFTerm, TFFType,
};

pub fn requires_interpretation(input: &AnnotatedFormula<'_>) -> bool {
    let mut scan = Scan { remaining: 100_000 };
    match input {
        AnnotatedFormula::FOF(f) => match &f.formula {
            FOFStatement::Logical(f) => scan.fof(f, 0),
            _ => true,
        },
        AnnotatedFormula::CNF(f) => requires_cnf_interpretation(&f.formula),
        AnnotatedFormula::TFF(f) => match &f.formula {
            TFFStatement::Logical(f) => scan.tff(f, 0),
            TFFStatement::Typing(t) => scan.typ(&t.typ, 0),
            _ => true,
        },
        AnnotatedFormula::TCF(f) => requires_tcf_interpretation(&f.formula),
        _ => true,
    }
}

pub fn requires_cnf_interpretation(statement: &CNFStatement<'_>) -> bool {
    let CNFStatement::Logical(f) = statement;
    Scan { remaining: 100_000 }.cnf(f, 0)
}

pub fn requires_tcf_interpretation(statement: &TCFStatement<'_>) -> bool {
    let mut scan = Scan { remaining: 100_000 };
    match statement {
        TCFStatement::Logical(TCFFormula::Clause(c)) => scan.tcf_clause(c, 0),
        TCFStatement::Logical(TCFFormula::Quantified { variables, clause }) => {
            variables
                .iter()
                .any(|v| v.typ.as_ref().is_some_and(|t| scan.typ(t, 0)))
                || scan.tcf_clause(clause, 0)
        }
        TCFStatement::Typing(t) => scan.typ(&t.typ, 0),
    }
}

struct Scan {
    remaining: usize,
}

impl Scan {
    fn exhausted(&mut self, depth: usize) -> bool {
        if self.remaining == 0 || depth > 256 {
            return true;
        }
        self.remaining -= 1;
        false
    }

    fn fof(&mut self, f: &FOFFormula<'_>, depth: usize) -> bool {
        if self.exhausted(depth) {
            return true;
        }
        match f {
            FOFFormula::Atomic(a) => match a {
                FOFAtomicFormula::Plain(_, args) => {
                    args.iter().any(|t| self.fof_term(t, depth + 1))
                }
                FOFAtomicFormula::True | FOFAtomicFormula::False => false,
                _ => true,
            },
            FOFFormula::Negation(f)
            | FOFFormula::Parens(f)
            | FOFFormula::Quantified { formula: f, .. } => self.fof(f, depth + 1),
            FOFFormula::Binary { left, right, .. } => {
                self.fof(left, depth + 1) || self.fof(right, depth + 1)
            }
            FOFFormula::Equality(l, r) | FOFFormula::Inequality(l, r) => {
                self.fof_term(l, depth + 1) || self.fof_term(r, depth + 1)
            }
        }
    }

    fn fof_term(&mut self, t: &FOFTerm<'_>, depth: usize) -> bool {
        if self.exhausted(depth) {
            return true;
        }
        match t {
            FOFTerm::Variable(_) | FOFTerm::DistinctObject(_) => false,
            FOFTerm::Function(_, args) => args.iter().any(|t| self.fof_term(t, depth + 1)),
            _ => true,
        }
    }

    fn cnf(&mut self, f: &CNFFormula<'_>, depth: usize) -> bool {
        if self.exhausted(depth) {
            return true;
        }
        match f {
            CNFFormula::Parens(f) => self.cnf(f, depth + 1),
            CNFFormula::Disjunction(literals) => literals.iter().any(|l| {
                if self.exhausted(depth + 1) {
                    return true;
                }
                match l {
                    CNFLiteral::Positive(a) | CNFLiteral::Negative(a) => match a {
                        CNFAtomicFormula::Plain(_, args) => {
                            args.iter().any(|t| self.fof_term(t, depth + 2))
                        }
                        CNFAtomicFormula::True | CNFAtomicFormula::False => false,
                        _ => true,
                    },
                    CNFLiteral::Equality(l, r) | CNFLiteral::Inequality(l, r) => {
                        self.fof_term(l, depth + 2) || self.fof_term(r, depth + 2)
                    }
                }
            }),
        }
    }

    fn tff(&mut self, f: &TFFFormula<'_>, depth: usize) -> bool {
        if self.exhausted(depth) {
            return true;
        }
        match f {
            TFFFormula::Atomic(a) => match a {
                TFFAtomicFormula::Plain(_, args) => {
                    args.iter().any(|t| self.tff_term(t, depth + 1))
                }
                TFFAtomicFormula::True
                | TFFAtomicFormula::False
                | TFFAtomicFormula::Variable(_) => false,
                _ => true,
            },
            TFFFormula::Negation(f) | TFFFormula::Parens(f) => self.tff(f, depth + 1),
            TFFFormula::Quantified {
                variables, formula, ..
            } => {
                variables
                    .iter()
                    .any(|v| v.typ.as_ref().is_some_and(|t| self.typ(t, depth + 1)))
                    || self.tff(formula, depth + 1)
            }
            TFFFormula::Binary { left, right, .. } => {
                self.tff(left, depth + 1) || self.tff(right, depth + 1)
            }
            TFFFormula::Equality(l, r) | TFFFormula::Inequality(l, r) => {
                self.tff_term(l, depth + 1) || self.tff_term(r, depth + 1)
            }
            _ => true,
        }
    }

    fn tff_term(&mut self, t: &TFFTerm<'_>, depth: usize) -> bool {
        if self.exhausted(depth) {
            return true;
        }
        match t {
            TFFTerm::Variable(_) | TFFTerm::DistinctObject(_) => false,
            TFFTerm::Function(_, args) => args.iter().any(|t| self.tff_term(t, depth + 1)),
            TFFTerm::Parens(t) => self.tff_term(t, depth + 1),
            _ => true,
        }
    }

    fn typ(&mut self, t: &TFFType<'_>, depth: usize) -> bool {
        if self.exhausted(depth) {
            return true;
        }
        match t {
            TFFType::Defined(DefinedType::Int | DefinedType::Rat | DefinedType::Real) => true,
            TFFType::Function { args, result } => {
                args.iter().any(|t| self.typ(t, depth + 1)) || self.typ(result, depth + 1)
            }
            TFFType::Parens(t) => self.typ(t, depth + 1),
            TFFType::Atomic(_) | TFFType::Defined(_) | TFFType::Variable(_) => false,
            _ => true,
        }
    }

    fn tcf_clause(&mut self, c: &TCFClause<'_>, depth: usize) -> bool {
        if self.exhausted(depth) {
            return true;
        }
        match c {
            TCFClause::Parens(c) => self.tcf_clause(c, depth + 1),
            TCFClause::Disjunction(literals) => {
                literals.iter().any(|l| self.tcf_literal(l, depth + 1))
            }
        }
    }

    fn tcf_literal(&mut self, l: &TCFLiteral<'_>, depth: usize) -> bool {
        if self.exhausted(depth) {
            return true;
        }
        match l {
            TCFLiteral::Parens(l) => self.tcf_literal(l, depth + 1),
            TCFLiteral::Positive(a) | TCFLiteral::Negative(a) => match a {
                TCFAtomicFormula::Plain(_, args) => {
                    args.iter().any(|t| self.tff_term(t, depth + 1))
                }
                TCFAtomicFormula::True | TCFAtomicFormula::False => false,
                _ => true,
            },
            TCFLiteral::Equality(l, r) | TCFLiteral::Inequality(l, r) => {
                self.tff_term(l, depth + 1) || self.tff_term(r, depth + 1)
            }
        }
    }
}
