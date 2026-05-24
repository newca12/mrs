//! CNF (Clause Normal Form) parser.

use winnow::combinator::{alt, delimited, opt, separated};
use winnow::error::StrContext;
use winnow::prelude::*;

use crate::ast::cnf::{CNFAtomicFormula, CNFFormula, CNFLiteral, CNFStatement};
use crate::ast::fof::FOFTerm;
use crate::lexer::{
    PResult, atomic_word, defined_word, distinct_object, number, system_word, upper_word, ws,
};
use crate::parser::fof::fof_term;

/// Parse a CNF statement
pub fn cnf_statement<'a>(input: &mut &'a str) -> PResult<CNFStatement<'a>> {
    cnf_formula
        .map(CNFStatement::Logical)
        .context(StrContext::Label("cnf_statement"))
        .parse_next(input)
}

/// Parse a CNF formula
pub fn cnf_formula<'a>(input: &mut &'a str) -> PResult<CNFFormula<'a>> {
    alt((
        // Parenthesized formula
        delimited(
            ('(', ws),
            cnf_formula.map(|f| CNFFormula::Parens(Box::new(f))),
            (ws, ')'),
        ),
        // Disjunction of literals
        cnf_disjunction.map(CNFFormula::Disjunction),
    ))
    .parse_next(input)
}

/// Parse a CNF disjunction (list of literals separated by |)
fn cnf_disjunction<'a>(input: &mut &'a str) -> PResult<Vec<CNFLiteral<'a>>> {
    separated(1.., cnf_literal, (ws, '|', ws)).parse_next(input)
}

/// Parse a CNF literal
pub fn cnf_literal<'a>(input: &mut &'a str) -> PResult<CNFLiteral<'a>> {
    // Negation: ~atom
    if input.starts_with('~') {
        '~'.parse_next(input)?;
        ws.parse_next(input)?;
        let atom = cnf_atomic_formula.parse_next(input)?;
        return Ok(CNFLiteral::Negative(atom));
    }
    cnf_infix_or_positive(input)
}

/// Parse a positive literal or infix equality, dispatching on first byte
fn cnf_infix_or_positive<'a>(input: &mut &'a str) -> PResult<CNFLiteral<'a>> {
    let first = *input
        .as_bytes()
        .first()
        .ok_or_else(|| winnow::error::ErrMode::Backtrack(winnow::error::ContextError::new()))?;
    match first {
        // Plain predicate or function term in infix
        b'a'..=b'z' | b'\'' => {
            let pred = atomic_word.parse_next(input)?;
            ws.parse_next(input)?;
            let args: Vec<FOFTerm<'a>> = opt(delimited(
                ('(', ws),
                separated(
                    1..,
                    |i: &mut &'a str| {
                        let t = fof_term(i)?;
                        ws.parse_next(i)?;
                        Ok(t)
                    },
                    (ws, ',', ws),
                ),
                (ws, ')'),
            ))
            .parse_next(input)?
            .unwrap_or_default();
            ws.parse_next(input)?;
            // Check for infix = or !=
            if input.starts_with("!=") {
                "!=".parse_next(input)?;
                ws.parse_next(input)?;
                let right = fof_term.parse_next(input)?;
                Ok(CNFLiteral::Inequality(FOFTerm::Function(pred, args), right))
            } else if input.starts_with('=') {
                '='.parse_next(input)?;
                ws.parse_next(input)?;
                let right = fof_term.parse_next(input)?;
                Ok(CNFLiteral::Equality(FOFTerm::Function(pred, args), right))
            } else {
                Ok(CNFLiteral::Positive(CNFAtomicFormula::Plain(pred, args)))
            }
        }
        // Variable in infix context
        b'A'..=b'Z' => {
            let left = FOFTerm::Variable(upper_word.parse_next(input)?);
            ws.parse_next(input)?;
            if input.starts_with("!=") {
                "!=".parse_next(input)?;
                ws.parse_next(input)?;
                let right = fof_term.parse_next(input)?;
                Ok(CNFLiteral::Inequality(left, right))
            } else if input.starts_with('=') {
                '='.parse_next(input)?;
                ws.parse_next(input)?;
                let right = fof_term.parse_next(input)?;
                Ok(CNFLiteral::Equality(left, right))
            } else {
                Err(winnow::error::ErrMode::Backtrack(
                    winnow::error::ContextError::new(),
                ))
            }
        }
        // $ - defined/system or $true/$false
        b'$' => {
            if input.starts_with("$true")
                && !input[5..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
            {
                *input = &input[5..];
                return Ok(CNFLiteral::Positive(CNFAtomicFormula::True));
            }
            if input.starts_with("$false")
                && !input[6..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
            {
                *input = &input[6..];
                return Ok(CNFLiteral::Positive(CNFAtomicFormula::False));
            }
            if input.starts_with("$$") {
                let pred = system_word.parse_next(input)?;
                ws.parse_next(input)?;
                let args: Vec<FOFTerm<'a>> = opt(delimited(
                    ('(', ws),
                    separated(
                        1..,
                        |i: &mut &'a str| {
                            let t = fof_term(i)?;
                            ws.parse_next(i)?;
                            Ok(t)
                        },
                        (ws, ',', ws),
                    ),
                    (ws, ')'),
                ))
                .parse_next(input)?
                .unwrap_or_default();
                ws.parse_next(input)?;
                if input.starts_with("!=") {
                    "!=".parse_next(input)?;
                    ws.parse_next(input)?;
                    let right = fof_term.parse_next(input)?;
                    Ok(CNFLiteral::Inequality(
                        FOFTerm::SystemFunction(pred, args),
                        right,
                    ))
                } else if input.starts_with('=') {
                    '='.parse_next(input)?;
                    ws.parse_next(input)?;
                    let right = fof_term.parse_next(input)?;
                    Ok(CNFLiteral::Equality(
                        FOFTerm::SystemFunction(pred, args),
                        right,
                    ))
                } else {
                    Ok(CNFLiteral::Positive(CNFAtomicFormula::System(pred, args)))
                }
            } else {
                let pred = defined_word.parse_next(input)?;
                ws.parse_next(input)?;
                let args: Vec<FOFTerm<'a>> = opt(delimited(
                    ('(', ws),
                    separated(
                        1..,
                        |i: &mut &'a str| {
                            let t = fof_term(i)?;
                            ws.parse_next(i)?;
                            Ok(t)
                        },
                        (ws, ',', ws),
                    ),
                    (ws, ')'),
                ))
                .parse_next(input)?
                .unwrap_or_default();
                ws.parse_next(input)?;
                if input.starts_with("!=") {
                    "!=".parse_next(input)?;
                    ws.parse_next(input)?;
                    let right = fof_term.parse_next(input)?;
                    Ok(CNFLiteral::Inequality(
                        FOFTerm::DefinedFunction(pred, args),
                        right,
                    ))
                } else if input.starts_with('=') {
                    '='.parse_next(input)?;
                    ws.parse_next(input)?;
                    let right = fof_term.parse_next(input)?;
                    Ok(CNFLiteral::Equality(
                        FOFTerm::DefinedFunction(pred, args),
                        right,
                    ))
                } else {
                    Ok(CNFLiteral::Positive(CNFAtomicFormula::Defined(pred, args)))
                }
            }
        }
        // Number in infix context
        b'0'..=b'9' | b'+' | b'-' => {
            let left = number.map(FOFTerm::Number).parse_next(input)?;
            ws.parse_next(input)?;
            if input.starts_with("!=") {
                "!=".parse_next(input)?;
                ws.parse_next(input)?;
                let right = fof_term.parse_next(input)?;
                Ok(CNFLiteral::Inequality(left, right))
            } else {
                '='.parse_next(input)?;
                ws.parse_next(input)?;
                let right = fof_term.parse_next(input)?;
                Ok(CNFLiteral::Equality(left, right))
            }
        }
        // Distinct object in infix context
        b'"' => {
            let left = distinct_object
                .map(FOFTerm::DistinctObject)
                .parse_next(input)?;
            ws.parse_next(input)?;
            if input.starts_with("!=") {
                "!=".parse_next(input)?;
                ws.parse_next(input)?;
                let right = fof_term.parse_next(input)?;
                Ok(CNFLiteral::Inequality(left, right))
            } else {
                '='.parse_next(input)?;
                ws.parse_next(input)?;
                let right = fof_term.parse_next(input)?;
                Ok(CNFLiteral::Equality(left, right))
            }
        }
        _ => Err(winnow::error::ErrMode::Backtrack(
            winnow::error::ContextError::new(),
        )),
    }
}

/// Parse a CNF atomic formula
pub fn cnf_atomic_formula<'a>(input: &mut &'a str) -> PResult<CNFAtomicFormula<'a>> {
    alt((
        // $true
        "$true".value(CNFAtomicFormula::True),
        // $false
        "$false".value(CNFAtomicFormula::False),
        // System predicate: $$pred(args)
        cnf_system_atomic,
        // Defined predicate: $pred(args)
        cnf_defined_atomic,
        // Plain predicate: pred(args) or prop
        cnf_plain_atomic,
    ))
    .parse_next(input)
}

fn cnf_plain_atomic<'a>(input: &mut &'a str) -> PResult<CNFAtomicFormula<'a>> {
    let pred = atomic_word.parse_next(input)?;
    ws.parse_next(input)?;

    // Check for arguments
    let args = opt(delimited(
        ('(', ws),
        separated(
            1..,
            |i: &mut &'a str| {
                let t = fof_term(i)?;
                ws.parse_next(i)?;
                Ok(t)
            },
            (ws, ',', ws),
        ),
        (ws, ')'),
    ))
    .parse_next(input)?;

    Ok(CNFAtomicFormula::Plain(pred, args.unwrap_or_default()))
}

fn cnf_defined_atomic<'a>(input: &mut &'a str) -> PResult<CNFAtomicFormula<'a>> {
    let pred = defined_word.parse_next(input)?;
    ws.parse_next(input)?;

    let args = opt(delimited(
        ('(', ws),
        separated(
            1..,
            |i: &mut &'a str| {
                let t = fof_term(i)?;
                ws.parse_next(i)?;
                Ok(t)
            },
            (ws, ',', ws),
        ),
        (ws, ')'),
    ))
    .parse_next(input)?;

    Ok(CNFAtomicFormula::Defined(pred, args.unwrap_or_default()))
}

fn cnf_system_atomic<'a>(input: &mut &'a str) -> PResult<CNFAtomicFormula<'a>> {
    let pred = system_word.parse_next(input)?;
    ws.parse_next(input)?;

    let args = opt(delimited(
        ('(', ws),
        separated(
            1..,
            |i: &mut &'a str| {
                let t = fof_term(i)?;
                ws.parse_next(i)?;
                Ok(t)
            },
            (ws, ',', ws),
        ),
        (ws, ')'),
    ))
    .parse_next(input)?;

    Ok(CNFAtomicFormula::System(pred, args.unwrap_or_default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cnf_literal() {
        assert!(cnf_literal.parse_peek("p").is_ok());
        assert!(cnf_literal.parse_peek("~p").is_ok());
        assert!(cnf_literal.parse_peek("p(X)").is_ok());
        assert!(cnf_literal.parse_peek("~p(X, Y)").is_ok());
    }

    #[test]
    fn test_cnf_formula() {
        assert!(cnf_formula.parse_peek("p | q").is_ok());
        assert!(cnf_formula.parse_peek("p | ~q | r(X)").is_ok());
        assert!(cnf_formula.parse_peek("(p | q)").is_ok());
    }

    #[test]
    fn test_cnf_equality() {
        let result = cnf_formula.parse_peek("X = Y | ~p(X)");
        assert!(result.is_ok());
    }
}
