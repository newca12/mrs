//! Common parser utilities shared across dialects.

use winnow::combinator::{alt, delimited, opt, preceded, separated};
use winnow::prelude::*;

use crate::ast::{
    BinaryConnective, FormulaRole, GeneralData, GeneralTerm, Quantifier, UnaryConnective,
};
use crate::lexer::{
    PResult, atomic_word, distinct_object, dollar_word, lower_word, number, upper_word, ws,
};

/// Parse a binary connective
pub fn binary_connective(input: &mut &str) -> PResult<BinaryConnective> {
    alt((
        "<=>".value(BinaryConnective::Iff),
        "=>".value(BinaryConnective::Impl),
        "<=".value(BinaryConnective::RevImpl),
        "<~>".value(BinaryConnective::Xor),
        "~|".value(BinaryConnective::Nor),
        "~&".value(BinaryConnective::Nand),
        "|".value(BinaryConnective::Or),
        "&".value(BinaryConnective::And),
    ))
    .parse_next(input)
}

/// Parse a non-associative binary connective
pub fn nonassoc_connective(input: &mut &str) -> PResult<BinaryConnective> {
    alt((
        "<=>".value(BinaryConnective::Iff),
        "=>".value(BinaryConnective::Impl),
        "<=".value(BinaryConnective::RevImpl),
        "<~>".value(BinaryConnective::Xor),
        "~|".value(BinaryConnective::Nor),
        "~&".value(BinaryConnective::Nand),
    ))
    .parse_next(input)
}

/// Parse a quantifier (! or ?)
pub fn quantifier(input: &mut &str) -> PResult<Quantifier> {
    alt(('!'.value(Quantifier::Forall), '?'.value(Quantifier::Exists))).parse_next(input)
}

/// Parse unary connective (~)
pub fn unary_connective(input: &mut &str) -> PResult<UnaryConnective> {
    '~'.value(UnaryConnective::Not).parse_next(input)
}

/// Parse a formula role
/// Handles hyphenated roles like "axiom-local" as well as simple roles
pub fn formula_role(input: &mut &str) -> PResult<FormulaRole> {
    // Parse the base word
    let start = lower_word.parse_next(input)?;

    // Check if there's a hyphen followed by more word characters
    let checkpoint = *input;
    let hyphen_result: PResult<&str> = "-".parse_next(input);
    if hyphen_result.is_ok() {
        // Try to parse the rest
        if let Ok(rest) = lower_word.parse_next(input) {
            // Reconstruct the full role string
            let end_pos = rest.as_ptr() as usize + rest.len();
            let start_pos = start.as_ptr() as usize;
            let full_len = end_pos - start_pos;
            let full_role = unsafe {
                std::str::from_utf8_unchecked(std::slice::from_raw_parts(start.as_ptr(), full_len))
            };

            if let Some(role) = FormulaRole::parse(full_role) {
                return Ok(role);
            }
        }
        // If we can't parse a hyphenated role, backtrack
        *input = checkpoint;
    }

    // Try to parse as a simple role
    FormulaRole::parse(start)
        .ok_or_else(|| winnow::error::ErrMode::Backtrack(winnow::error::ContextError::new()))
}

/// Parse a general term (used in annotations)
pub fn general_term<'a>(input: &mut &'a str) -> PResult<GeneralTerm<'a>> {
    alt((
        // Colon pair: term : term
        general_term_colon_pair,
        general_term_simple,
    ))
    .parse_next(input)
}

fn general_term_simple<'a>(input: &mut &'a str) -> PResult<GeneralTerm<'a>> {
    alt((
        // List: [...]
        general_list.map(GeneralTerm::List),
        // Formula data: $thf(...), $tff(...), etc.
        general_data,
        // Function: word(args)
        general_function,
        // Distinct object
        distinct_object.map(GeneralTerm::DistinctObject),
        // Number
        number.map(GeneralTerm::Number),
        // Variable
        upper_word.map(GeneralTerm::Variable),
        // Word (atomic)
        atomic_word.map(GeneralTerm::Word),
    ))
    .parse_next(input)
}

fn general_term_colon_pair<'a>(input: &mut &'a str) -> PResult<GeneralTerm<'a>> {
    let left = general_term_simple.parse_next(input)?;
    ws.parse_next(input)?;
    ':'.parse_next(input)?;
    ws.parse_next(input)?;
    let right = general_term.parse_next(input)?;
    Ok(GeneralTerm::ColonPair(Box::new(left), Box::new(right)))
}

fn general_function<'a>(input: &mut &'a str) -> PResult<GeneralTerm<'a>> {
    let name = atomic_word.parse_next(input)?;
    ws.parse_next(input)?;

    // Check for arguments
    if input.starts_with('(') {
        '('.parse_next(input)?;
        ws.parse_next(input)?;
        let args = separated(
            1..,
            |i: &mut &'a str| {
                let t = general_term(i)?;
                ws.parse_next(i)?;
                Ok(t)
            },
            (ws, ',', ws),
        )
        .parse_next(input)?;
        ws.parse_next(input)?;
        ')'.parse_next(input)?;
        Ok(GeneralTerm::Function(name, args))
    } else {
        Ok(GeneralTerm::Word(name))
    }
}

fn general_list<'a>(input: &mut &'a str) -> PResult<Vec<GeneralTerm<'a>>> {
    delimited(
        ('[', ws),
        separated(
            0..,
            |i: &mut &'a str| {
                let t = general_term(i)?;
                ws.parse_next(i)?;
                Ok(t)
            },
            (ws, ',', ws),
        ),
        (ws, ']'),
    )
    .parse_next(input)
}

fn general_data<'a>(input: &mut &'a str) -> PResult<GeneralTerm<'a>> {
    // $thf(...), $tff(...), $fof(...), $cnf(...), $fot(...)
    let keyword = dollar_word.parse_next(input)?;
    ws.parse_next(input)?;
    '('.parse_next(input)?;
    ws.parse_next(input)?;

    let data = match keyword {
        "thf" => {
            let formula = crate::parser::thf::thf_formula.parse_next(input)?;
            GeneralData::THF(Box::new(formula))
        }
        "tff" => {
            let formula = crate::parser::tff::tff_formula.parse_next(input)?;
            GeneralData::TFF(Box::new(formula))
        }
        "fof" => {
            let formula = crate::parser::fof::fof_formula.parse_next(input)?;
            GeneralData::FOF(Box::new(formula))
        }
        "cnf" => {
            let formula = crate::parser::cnf::cnf_formula.parse_next(input)?;
            GeneralData::CNF(Box::new(formula))
        }
        "fot" => {
            let term = crate::parser::fof::fof_term.parse_next(input)?;
            GeneralData::FOT(Box::new(term))
        }
        _ => {
            return Err(winnow::error::ErrMode::Backtrack(
                winnow::error::ContextError::new(),
            ));
        }
    };

    ws.parse_next(input)?;
    ')'.parse_next(input)?;

    Ok(GeneralTerm::Formula(Box::new(data)))
}

/// Parse optional annotations: , source [, useful_info]
pub fn annotations<'a>(input: &mut &'a str) -> PResult<Option<crate::ast::Annotations<'a>>> {
    let result = opt(preceded((',', ws), |i: &mut &'a str| {
        let source = general_term(i)?;
        ws.parse_next(i)?;

        let useful_info = opt(preceded(
            (',', ws),
            delimited(
                ('[', ws),
                separated(
                    0..,
                    |i2: &mut &'a str| {
                        let t = general_term(i2)?;
                        ws.parse_next(i2)?;
                        Ok(t)
                    },
                    (ws, ',', ws),
                ),
                (ws, ']'),
            ),
        ))
        .parse_next(i)?;

        Ok(crate::ast::Annotations {
            source,
            useful_info,
        })
    }))
    .parse_next(input)?;

    Ok(result)
}

/// Helper to parse a comma-separated list within parentheses
pub fn paren_list<'a, O, P>(
    parser: P,
) -> impl Parser<&'a str, Vec<O>, winnow::error::ErrMode<winnow::error::ContextError>>
where
    P: Parser<&'a str, O, winnow::error::ErrMode<winnow::error::ContextError>>,
{
    delimited(('(', ws), separated(1.., parser, (ws, ',', ws)), (ws, ')'))
}

/// Helper to parse a comma-separated list within brackets
pub fn bracket_list<'a, O, P>(
    parser: P,
) -> impl Parser<&'a str, Vec<O>, winnow::error::ErrMode<winnow::error::ContextError>>
where
    P: Parser<&'a str, O, winnow::error::ErrMode<winnow::error::ContextError>>,
{
    delimited(('[', ws), separated(1.., parser, (ws, ',', ws)), (ws, ']'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_general_term_function() {
        let mut input = "file('source.p', ax1)";
        let result = general_term.parse_next(&mut input);
        assert!(
            result.is_ok(),
            "general_term failed on function: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_general_function_simple() {
        let mut input = "file(a, b)";
        let result = general_function(&mut input);
        assert!(
            result.is_ok(),
            "general_function failed: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_annotations_parse() {
        let mut input = ", file('source.p', ax1)";
        let result = annotations(&mut input);
        assert!(result.is_ok(), "annotations failed: {:?}", result.err());
    }
}
