//! TCF (Typed Clause Form) parser.

use winnow::combinator::{alt, delimited, opt, preceded, separated};
use winnow::error::StrContext;
use winnow::prelude::*;

use crate::ast::tcf::{
    TCFAtomicFormula, TCFClause, TCFFormula, TCFLiteral, TCFStatement, TCFTyping,
};
use crate::lexer::{PResult, atomic_word, ws};
use crate::parser::tff::{tff_term, tff_top_level_type, tff_variable};

/// Parse a TCF statement
pub fn tcf_statement<'a>(input: &mut &'a str) -> PResult<TCFStatement<'a>> {
    alt((
        // Type declaration
        tcf_typing.map(TCFStatement::Typing),
        // Logical formula (clause)
        tcf_formula.map(TCFStatement::Logical),
    ))
    .context(StrContext::Label("tcf_statement"))
    .parse_next(input)
}

fn tcf_typing<'a>(input: &mut &'a str) -> PResult<TCFTyping<'a>> {
    let symbol = atomic_word.parse_next(input)?;
    ws.parse_next(input)?;
    ':'.parse_next(input)?;
    ws.parse_next(input)?;
    let typ = tff_top_level_type.parse_next(input)?;

    Ok(TCFTyping { symbol, typ })
}

/// Parse a TCF formula
pub fn tcf_formula<'a>(input: &mut &'a str) -> PResult<TCFFormula<'a>> {
    alt((
        // Quantified: ! [vars] : clause
        tcf_quantified_formula,
        // Unquantified clause
        tcf_clause.map(TCFFormula::Clause),
    ))
    .parse_next(input)
}

fn tcf_quantified_formula<'a>(input: &mut &'a str) -> PResult<TCFFormula<'a>> {
    '!'.parse_next(input)?;
    ws.parse_next(input)?;
    '['.parse_next(input)?;
    ws.parse_next(input)?;

    let vars = separated(1.., tff_variable, (ws, ',', ws)).parse_next(input)?;

    ws.parse_next(input)?;
    ']'.parse_next(input)?;
    ws.parse_next(input)?;
    ':'.parse_next(input)?;
    ws.parse_next(input)?;

    let clause = tcf_clause.parse_next(input)?;

    Ok(TCFFormula::Quantified {
        variables: vars,
        clause: Box::new(clause),
    })
}

/// Parse a TCF clause
pub fn tcf_clause<'a>(input: &mut &'a str) -> PResult<TCFClause<'a>> {
    alt((
        // Disjunction first - handles most cases including literals starting with (
        tcf_disjunction.map(TCFClause::Disjunction),
        // Parenthesized clause - only matches if it's literally just parens around another clause
        delimited(
            ('(', ws),
            tcf_clause.map(|c| TCFClause::Parens(Box::new(c))),
            (ws, ')'),
        ),
    ))
    .parse_next(input)
}

fn tcf_disjunction<'a>(input: &mut &'a str) -> PResult<Vec<TCFLiteral<'a>>> {
    separated(1.., tcf_literal, (ws, '|', ws)).parse_next(input)
}

/// Parse a TCF literal
pub fn tcf_literal<'a>(input: &mut &'a str) -> PResult<TCFLiteral<'a>> {
    alt((
        // Parenthesized literal
        delimited(
            ('(', ws),
            tcf_literal.map(|l| TCFLiteral::Parens(Box::new(l))),
            (ws, ')'),
        ),
        // Negative
        preceded(('~', ws), tcf_atomic_formula).map(TCFLiteral::Negative),
        // Infix
        tcf_infix,
        // Positive
        tcf_atomic_formula.map(TCFLiteral::Positive),
    ))
    .parse_next(input)
}

fn tcf_infix<'a>(input: &mut &'a str) -> PResult<TCFLiteral<'a>> {
    let left = tff_term.parse_next(input)?;
    ws.parse_next(input)?;

    let is_neg = alt(("!=".value(true), "=".value(false))).parse_next(input)?;

    ws.parse_next(input)?;
    let right = tff_term.parse_next(input)?;

    Ok(if is_neg {
        TCFLiteral::Inequality(left, right)
    } else {
        TCFLiteral::Equality(left, right)
    })
}

/// Parse a TCF atomic formula
pub fn tcf_atomic_formula<'a>(input: &mut &'a str) -> PResult<TCFAtomicFormula<'a>> {
    alt((
        "$true".value(TCFAtomicFormula::True),
        "$false".value(TCFAtomicFormula::False),
        tcf_plain_atomic,
    ))
    .parse_next(input)
}

fn tcf_plain_atomic<'a>(input: &mut &'a str) -> PResult<TCFAtomicFormula<'a>> {
    let pred = atomic_word.parse_next(input)?;
    ws.parse_next(input)?;

    let args = opt(delimited(
        ('(', ws),
        separated(
            1..,
            |i: &mut &'a str| {
                let t = tff_term(i)?;
                ws.parse_next(i)?;
                Ok(t)
            },
            (ws, ',', ws),
        ),
        (ws, ')'),
    ))
    .parse_next(input)?;

    Ok(TCFAtomicFormula::Plain(pred, args.unwrap_or_default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tcf_literal() {
        assert!(tcf_literal.parse_peek("p").is_ok());
        assert!(tcf_literal.parse_peek("~p").is_ok());
        assert!(tcf_literal.parse_peek("p(X)").is_ok());
    }

    #[test]
    fn test_tcf_clause() {
        assert!(tcf_clause.parse_peek("p | q").is_ok());
        assert!(tcf_clause.parse_peek("p | ~q | r").is_ok());
    }

    #[test]
    fn test_tcf_formula() {
        assert!(tcf_formula.parse_peek("p | q").is_ok());
        // Quantified with types would need proper type parsing
    }
}
