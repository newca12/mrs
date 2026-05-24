//! FOF (First-Order Form) parser.

use winnow::combinator::{alt, delimited, opt, preceded, separated};
use winnow::error::StrContext;
use winnow::prelude::*;

use crate::ast::BinaryConnective;
use crate::ast::fof::{FOFAtomicFormula, FOFFormula, FOFStatement, FOFTerm};
use crate::lexer::{
    PResult, atomic_word, check_cancel, defined_word, distinct_object, number, system_word,
    upper_word, ws,
};
use crate::parser::common::{nonassoc_connective, quantifier};

/// Parse a FOF statement
pub fn fof_statement<'a>(input: &mut &'a str) -> PResult<FOFStatement<'a>> {
    alt((
        // Sequent: [formulas] --> [formulas]
        fof_sequent.map(|(l, r)| FOFStatement::Sequent(l, r)),
        // Logical formula
        fof_formula.map(FOFStatement::Logical),
    ))
    .context(StrContext::Label("fof_statement"))
    .parse_next(input)
}

/// Parse a FOF sequent
fn fof_sequent<'a>(input: &mut &'a str) -> PResult<(Vec<FOFFormula<'a>>, Vec<FOFFormula<'a>>)> {
    let left = fof_formula_tuple.parse_next(input)?;
    ws.parse_next(input)?;
    "-->".parse_next(input)?;
    ws.parse_next(input)?;
    let right = fof_formula_tuple.parse_next(input)?;
    Ok((left, right))
}

fn fof_formula_tuple<'a>(input: &mut &'a str) -> PResult<Vec<FOFFormula<'a>>> {
    delimited(
        ('[', ws),
        separated(
            0..,
            |i: &mut &'a str| {
                let f = fof_formula(i)?;
                ws.parse_next(i)?;
                Ok(f)
            },
            (ws, ',', ws),
        ),
        (ws, ']'),
    )
    .parse_next(input)
}

/// Parse a FOF formula
pub fn fof_formula<'a>(input: &mut &'a str) -> PResult<FOFFormula<'a>> {
    check_cancel(input)?;
    fof_binary_formula.parse_next(input)
}

/// Parse a binary formula with proper precedence
fn fof_binary_formula<'a>(input: &mut &'a str) -> PResult<FOFFormula<'a>> {
    // Handle non-associative connectives first
    let left = fof_or_formula.parse_next(input)?;
    ws.parse_next(input)?;

    // Try to parse a non-associative connective
    let result = opt((nonassoc_connective, ws, fof_or_formula)).parse_next(input)?;

    match result {
        Some((conn, _, right)) => Ok(FOFFormula::Binary {
            left: Box::new(left),
            connective: conn,
            right: Box::new(right),
        }),
        None => Ok(left),
    }
}

/// Parse an or-formula (disjunction, left-associative)
fn fof_or_formula<'a>(input: &mut &'a str) -> PResult<FOFFormula<'a>> {
    let mut result = fof_and_formula.parse_next(input)?;
    loop {
        let checkpoint = *input;
        ws.parse_next(input)?;
        if input.as_bytes().first() != Some(&b'|') {
            *input = checkpoint;
            break;
        }
        *input = &input[1..];
        ws.parse_next(input)?;
        let right = fof_and_formula.parse_next(input)?;
        result = FOFFormula::Binary {
            left: Box::new(result),
            connective: BinaryConnective::Or,
            right: Box::new(right),
        };
    }
    Ok(result)
}

/// Parse an and-formula (conjunction, left-associative)
fn fof_and_formula<'a>(input: &mut &'a str) -> PResult<FOFFormula<'a>> {
    let mut result = fof_unary_formula.parse_next(input)?;
    loop {
        let checkpoint = *input;
        ws.parse_next(input)?;
        if input.as_bytes().first() != Some(&b'&') {
            *input = checkpoint;
            break;
        }
        *input = &input[1..];
        ws.parse_next(input)?;
        let right = fof_unary_formula.parse_next(input)?;
        result = FOFFormula::Binary {
            left: Box::new(result),
            connective: BinaryConnective::And,
            right: Box::new(right),
        };
    }
    Ok(result)
}

/// Parse a unary formula (negation or unit formula)
fn fof_unary_formula<'a>(input: &mut &'a str) -> PResult<FOFFormula<'a>> {
    alt((
        // Negation: ~F
        preceded(('~', ws), fof_unary_formula).map(|f| FOFFormula::Negation(Box::new(f))),
        // Unit formula
        fof_unit_formula,
    ))
    .parse_next(input)
}

/// Parse a unit formula (quantified, atomic, or parenthesized)
fn fof_unit_formula<'a>(input: &mut &'a str) -> PResult<FOFFormula<'a>> {
    let first = *input
        .as_bytes()
        .first()
        .ok_or_else(|| winnow::error::ErrMode::Backtrack(winnow::error::ContextError::new()))?;
    match first {
        b'!' | b'?' => fof_quantified_formula.parse_next(input),
        b'(' => delimited(('(', ws), fof_formula, (ws, ')'))
            .map(|f| FOFFormula::Parens(Box::new(f)))
            .parse_next(input),
        _ => fof_infix_or_atomic(input),
    }
}

/// Parse an atomic formula, checking for trailing infix =, !=
fn fof_infix_or_atomic<'a>(input: &mut &'a str) -> PResult<FOFFormula<'a>> {
    let first = *input
        .as_bytes()
        .first()
        .ok_or_else(|| winnow::error::ErrMode::Backtrack(winnow::error::ContextError::new()))?;
    match first {
        // Variable - only valid in infix context
        b'A'..=b'Z' => {
            let left = FOFTerm::Variable(upper_word.parse_next(input)?);
            ws.parse_next(input)?;
            let is_neg = fof_equality_op(input)?;
            match is_neg {
                Some(true) => {
                    ws.parse_next(input)?;
                    let right = fof_term.parse_next(input)?;
                    Ok(FOFFormula::Inequality(left, right))
                }
                Some(false) => {
                    ws.parse_next(input)?;
                    let right = fof_term.parse_next(input)?;
                    Ok(FOFFormula::Equality(left, right))
                }
                None => Err(winnow::error::ErrMode::Backtrack(
                    winnow::error::ContextError::new(),
                )),
            }
        }
        // Plain predicate or infix
        b'a'..=b'z' | b'\'' => {
            let pred = atomic_word.parse_next(input)?;
            ws.parse_next(input)?;
            // Check for function args
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
            // Check for infix equality
            match fof_equality_op(input)? {
                Some(true) => {
                    ws.parse_next(input)?;
                    let right = fof_term.parse_next(input)?;
                    Ok(FOFFormula::Inequality(FOFTerm::Function(pred, args), right))
                }
                Some(false) => {
                    ws.parse_next(input)?;
                    let right = fof_term.parse_next(input)?;
                    Ok(FOFFormula::Equality(FOFTerm::Function(pred, args), right))
                }
                None => Ok(FOFFormula::Atomic(FOFAtomicFormula::Plain(pred, args))),
            }
        }
        // $ - defined/system predicate or $true/$false
        b'$' => {
            if input.starts_with("$true")
                && !input[5..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
            {
                *input = &input[5..];
                return Ok(FOFFormula::Atomic(FOFAtomicFormula::True));
            }
            if input.starts_with("$false")
                && !input[6..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
            {
                *input = &input[6..];
                return Ok(FOFFormula::Atomic(FOFAtomicFormula::False));
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
                match fof_equality_op(input)? {
                    Some(true) => {
                        ws.parse_next(input)?;
                        let right = fof_term.parse_next(input)?;
                        Ok(FOFFormula::Inequality(
                            FOFTerm::SystemFunction(pred, args),
                            right,
                        ))
                    }
                    Some(false) => {
                        ws.parse_next(input)?;
                        let right = fof_term.parse_next(input)?;
                        Ok(FOFFormula::Equality(
                            FOFTerm::SystemFunction(pred, args),
                            right,
                        ))
                    }
                    None => Ok(FOFFormula::Atomic(FOFAtomicFormula::System(pred, args))),
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
                match fof_equality_op(input)? {
                    Some(true) => {
                        ws.parse_next(input)?;
                        let right = fof_term.parse_next(input)?;
                        Ok(FOFFormula::Inequality(
                            FOFTerm::DefinedFunction(pred, args),
                            right,
                        ))
                    }
                    Some(false) => {
                        ws.parse_next(input)?;
                        let right = fof_term.parse_next(input)?;
                        Ok(FOFFormula::Equality(
                            FOFTerm::DefinedFunction(pred, args),
                            right,
                        ))
                    }
                    None => Ok(FOFFormula::Atomic(FOFAtomicFormula::Defined(pred, args))),
                }
            }
        }
        // Number - only in infix context
        b'0'..=b'9' | b'+' | b'-' => {
            let left = number.map(FOFTerm::Number).parse_next(input)?;
            ws.parse_next(input)?;
            let is_neg = fof_equality_op(input)?;
            match is_neg {
                Some(true) => {
                    ws.parse_next(input)?;
                    let right = fof_term.parse_next(input)?;
                    Ok(FOFFormula::Inequality(left, right))
                }
                Some(false) => {
                    ws.parse_next(input)?;
                    let right = fof_term.parse_next(input)?;
                    Ok(FOFFormula::Equality(left, right))
                }
                None => Err(winnow::error::ErrMode::Backtrack(
                    winnow::error::ContextError::new(),
                )),
            }
        }
        // Distinct object - only in infix context
        b'"' => {
            let left = distinct_object
                .map(FOFTerm::DistinctObject)
                .parse_next(input)?;
            ws.parse_next(input)?;
            let is_neg = fof_equality_op(input)?;
            match is_neg {
                Some(true) => {
                    ws.parse_next(input)?;
                    let right = fof_term.parse_next(input)?;
                    Ok(FOFFormula::Inequality(left, right))
                }
                Some(false) => {
                    ws.parse_next(input)?;
                    let right = fof_term.parse_next(input)?;
                    Ok(FOFFormula::Equality(left, right))
                }
                None => Err(winnow::error::ErrMode::Backtrack(
                    winnow::error::ContextError::new(),
                )),
            }
        }
        _ => Err(winnow::error::ErrMode::Backtrack(
            winnow::error::ContextError::new(),
        )),
    }
}

/// Check for = or != (guarding against =>)
fn fof_equality_op(input: &mut &str) -> PResult<Option<bool>> {
    if input.starts_with("!=") {
        "!=".parse_next(input)?;
        return Ok(Some(true));
    }
    if input.starts_with('=') && !input.starts_with("=>") {
        '='.parse_next(input)?;
        return Ok(Some(false));
    }
    Ok(None)
}

/// Parse a quantified formula
fn fof_quantified_formula<'a>(input: &mut &'a str) -> PResult<FOFFormula<'a>> {
    let q = quantifier.parse_next(input)?;
    ws.parse_next(input)?;
    '['.parse_next(input)?;
    ws.parse_next(input)?;

    let vars: Vec<&'a str> = separated(
        1..,
        |i: &mut &'a str| {
            let v = upper_word(i)?;
            ws.parse_next(i)?;
            Ok(v)
        },
        (ws, ',', ws),
    )
    .parse_next(input)?;

    ws.parse_next(input)?;
    ']'.parse_next(input)?;
    ws.parse_next(input)?;
    ':'.parse_next(input)?;
    ws.parse_next(input)?;

    let formula = fof_unary_formula.parse_next(input)?;

    Ok(FOFFormula::Quantified {
        quantifier: q,
        variables: vars,
        formula: Box::new(formula),
    })
}

/// Parse a FOF atomic formula
pub fn fof_atomic_formula<'a>(input: &mut &'a str) -> PResult<FOFAtomicFormula<'a>> {
    let first = *input
        .as_bytes()
        .first()
        .ok_or_else(|| winnow::error::ErrMode::Backtrack(winnow::error::ContextError::new()))?;
    match first {
        b'$' => {
            if input.starts_with("$true")
                && !input[5..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
            {
                *input = &input[5..];
                Ok(FOFAtomicFormula::True)
            } else if input.starts_with("$false")
                && !input[6..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
            {
                *input = &input[6..];
                Ok(FOFAtomicFormula::False)
            } else if input.starts_with("$$") {
                fof_system_atomic.parse_next(input)
            } else {
                fof_defined_atomic.parse_next(input)
            }
        }
        _ => fof_plain_atomic.parse_next(input),
    }
}

fn fof_plain_atomic<'a>(input: &mut &'a str) -> PResult<FOFAtomicFormula<'a>> {
    let pred = atomic_word.parse_next(input)?;
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

    Ok(FOFAtomicFormula::Plain(pred, args.unwrap_or_default()))
}

fn fof_defined_atomic<'a>(input: &mut &'a str) -> PResult<FOFAtomicFormula<'a>> {
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

    Ok(FOFAtomicFormula::Defined(pred, args.unwrap_or_default()))
}

fn fof_system_atomic<'a>(input: &mut &'a str) -> PResult<FOFAtomicFormula<'a>> {
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

    Ok(FOFAtomicFormula::System(pred, args.unwrap_or_default()))
}

/// Parse a FOF term
pub fn fof_term<'a>(input: &mut &'a str) -> PResult<FOFTerm<'a>> {
    let first = *input
        .as_bytes()
        .first()
        .ok_or_else(|| winnow::error::ErrMode::Backtrack(winnow::error::ContextError::new()))?;
    match first {
        b'A'..=b'Z' => upper_word.map(FOFTerm::Variable).parse_next(input),
        b'a'..=b'z' | b'\'' => fof_function_term.parse_next(input),
        b'"' => distinct_object
            .map(FOFTerm::DistinctObject)
            .parse_next(input),
        b'0'..=b'9' | b'+' | b'-' => number.map(FOFTerm::Number).parse_next(input),
        b'$' => {
            if input.starts_with("$$") {
                fof_system_term.parse_next(input)
            } else {
                fof_defined_term.parse_next(input)
            }
        }
        _ => Err(winnow::error::ErrMode::Backtrack(
            winnow::error::ContextError::new(),
        )),
    }
}

fn fof_function_term<'a>(input: &mut &'a str) -> PResult<FOFTerm<'a>> {
    let name = atomic_word.parse_next(input)?;
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

    Ok(FOFTerm::Function(name, args.unwrap_or_default()))
}

fn fof_defined_term<'a>(input: &mut &'a str) -> PResult<FOFTerm<'a>> {
    let name = defined_word.parse_next(input)?;
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

    Ok(FOFTerm::DefinedFunction(name, args.unwrap_or_default()))
}

fn fof_system_term<'a>(input: &mut &'a str) -> PResult<FOFTerm<'a>> {
    let name = system_word.parse_next(input)?;
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

    Ok(FOFTerm::SystemFunction(name, args.unwrap_or_default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fof_term() {
        assert!(fof_term.parse_peek("X").is_ok());
        assert!(fof_term.parse_peek("c").is_ok());
        assert!(fof_term.parse_peek("f(X)").is_ok());
        assert!(fof_term.parse_peek("f(X, Y, Z)").is_ok());
        assert!(fof_term.parse_peek("f(g(X), h(Y, Z))").is_ok());
    }

    #[test]
    fn test_fof_atomic() {
        assert!(fof_atomic_formula.parse_peek("p").is_ok());
        assert!(fof_atomic_formula.parse_peek("p(X)").is_ok());
        assert!(fof_atomic_formula.parse_peek("$true").is_ok());
        assert!(fof_atomic_formula.parse_peek("$false").is_ok());
    }

    #[test]
    fn test_fof_formula() {
        assert!(fof_formula.parse_peek("p").is_ok());
        assert!(fof_formula.parse_peek("~p").is_ok());
        assert!(fof_formula.parse_peek("p & q").is_ok());
        assert!(fof_formula.parse_peek("p | q").is_ok());
        assert!(fof_formula.parse_peek("p => q").is_ok());
        assert!(fof_formula.parse_peek("p <=> q").is_ok());
        assert!(fof_formula.parse_peek("![X]: p(X)").is_ok());
        assert!(fof_formula.parse_peek("?[X]: p(X)").is_ok());
        assert!(fof_formula.parse_peek("![X]: (p(X) => q(X))").is_ok());
    }

    #[test]
    fn test_fof_equality() {
        assert!(fof_formula.parse_peek("X = Y").is_ok());
        assert!(fof_formula.parse_peek("X != Y").is_ok());
        assert!(fof_formula.parse_peek("f(X) = g(Y)").is_ok());
    }

    #[test]
    fn test_fof_complex() {
        let input = "![X]: (human(X) => mortal(X))";
        let result = fof_formula.parse_peek(input);
        assert!(result.is_ok());

        let _input2 = "![X, Y]: (parent(X, Y) & parent(Y, Z) => grandparent(X, Z))";
        // Note: This tests associativity; the & should bind tighter than =>
    }

    #[test]
    fn test_fof_precedence() {
        // & binds tighter than |
        let input = "p & q | r";
        let result = fof_formula.parse_peek(input);
        assert!(result.is_ok());

        // | binds tighter than =>
        let input2 = "p | q => r";
        let result2 = fof_formula.parse_peek(input2);
        assert!(result2.is_ok());
    }
}
