//! TFF (Typed First-order Form) parser.

use winnow::combinator::{alt, delimited, opt, preceded, separated};
use winnow::error::StrContext;
use winnow::prelude::*;

use crate::ast::BinaryConnective;
use crate::ast::common::AtomicWord;
use crate::ast::tff::{
    DefinedType, TFFAtomicFormula, TFFFormula, TFFLetBody, TFFLetDef, TFFStatement, TFFTerm,
    TFFType, TFFTyping, TFFVariable, TypeQuantifier, TypedSymbol,
};
use crate::ast::thf::{LogicProperty, LogicSpecification, LogicValue, NonClassicalOperator};
use crate::lexer::{
    PResult, atomic_word, check_cancel, defined_word, distinct_object, number, single_quoted,
    system_word, upper_word, ws,
};
use crate::parser::common::{nonassoc_connective, quantifier};

/// Parse equality operator (= or !=) for infix formulas.
/// IMPORTANT: Must not match => or <=> which are connectives, not equality.
/// Returns Some(true) for !=, Some(false) for =, None if no equality operator.
fn tff_equality_op(input: &mut &str) -> PResult<Option<bool>> {
    // First check for !=
    if input.starts_with("!=") {
        "!=".parse_next(input)?;
        return Ok(Some(true));
    }
    // Check for = but NOT => or <=>
    if input.starts_with("=") && !input.starts_with("=>") {
        '='.parse_next(input)?;
        return Ok(Some(false));
    }
    Ok(None)
}

/// Convert a TFFTerm back to a TFFFormula (for FOOL contexts)
/// Used when we parse something as a term but it turns out to be used as a formula
fn term_to_formula<'a>(term: TFFTerm<'a>) -> TFFFormula<'a> {
    match term {
        // FormulaAsTerm was parsed from (formula), so wrap back in Parens to preserve
        TFFTerm::FormulaAsTerm(f) => TFFFormula::Parens(f),
        // Parens around a term - convert inner and wrap in Parens
        TFFTerm::Parens(t) => TFFFormula::Parens(Box::new(term_to_formula(*t))),
        TFFTerm::Variable(v) => TFFFormula::Atomic(TFFAtomicFormula::Variable(v)),
        TFFTerm::Function(name, args) => TFFFormula::Atomic(TFFAtomicFormula::Plain(name, args)),
        TFFTerm::DefinedFunction(name, args) => {
            TFFFormula::Atomic(TFFAtomicFormula::Defined(name, args))
        }
        TFFTerm::SystemFunction(name, args) => {
            TFFFormula::Atomic(TFFAtomicFormula::System(name, args))
        }
        TFFTerm::Conditional {
            condition,
            then_branch,
            else_branch,
        } => TFFFormula::Conditional {
            condition,
            then_branch: Box::new(term_to_formula(*then_branch)),
            else_branch: Box::new(term_to_formula(*else_branch)),
        },
        TFFTerm::Let { definitions, body } => TFFFormula::Let {
            definitions,
            body: Box::new(TFFLetBody::Formula(term_to_formula(*body))),
        },
        // For other cases, wrap in atomic (may not be semantically correct but handles edge cases)
        other => TFFFormula::Atomic(TFFAtomicFormula::Plain(
            AtomicWord::Lower("_term"),
            vec![other],
        )),
    }
}

/// Parse a TFF statement
///
/// Uses alt to try each statement type in order.
/// Note: Typing MUST be tried before logical formulas because a type declaration
/// like "foo : type" would otherwise be parsed as an atomic formula "foo".
pub fn tff_statement<'a>(input: &mut &'a str) -> PResult<TFFStatement<'a>> {
    alt((
        // Logic specification: $name == [...] (starts with $ so won't conflict)
        tff_logic_specification.map(TFFStatement::Logic),
        // Parenthesized typing: ( symbol : type )
        delimited(('(', ws), tff_typing, (ws, ')')).map(TFFStatement::Typing),
        // Typing: symbol : type (must come before formula - both start with symbol)
        tff_typing.map(TFFStatement::Typing),
        // Sequent: formulas --> formulas (starts with [ so won't conflict)
        tff_sequent.map(|(l, r)| TFFStatement::Sequent(l, r)),
        // Logical formula (most common but must come last to avoid partial match on typings)
        tff_formula.map(TFFStatement::Logical),
    ))
    .context(StrContext::Label("tff_statement"))
    .parse_next(input)
}

/// Parse a TFF type declaration
fn tff_typing<'a>(input: &mut &'a str) -> PResult<TFFTyping<'a>> {
    let symbol = alt((
        defined_word.map(TypedSymbol::Defined),
        atomic_word.map(TypedSymbol::Atom),
    ))
    .parse_next(input)?;

    ws.parse_next(input)?;
    ':'.parse_next(input)?;
    ws.parse_next(input)?;

    let typ = tff_top_level_type.parse_next(input)?;

    Ok(TFFTyping { symbol, typ })
}

/// Parse a TFF sequent
fn tff_sequent<'a>(input: &mut &'a str) -> PResult<(Vec<TFFFormula<'a>>, Vec<TFFFormula<'a>>)> {
    let left = tff_formula_tuple.parse_next(input)?;
    ws.parse_next(input)?;
    "-->".parse_next(input)?;
    ws.parse_next(input)?;
    let right = tff_formula_tuple.parse_next(input)?;
    Ok((left, right))
}

/// Parse a logic specification: $modal == [ ... ] or $epistemic_modal == [ ... ] etc.
/// May be wrapped in outer parentheses: ( $modal == [...] )
fn tff_logic_specification<'a>(input: &mut &'a str) -> PResult<LogicSpecification<'a>> {
    // Check for optional outer parentheses
    let has_parens = opt('(').parse_next(input)?.is_some();
    if has_parens {
        ws.parse_next(input)?;
    }

    // Parse the logic family name (e.g., $modal, $alethic_modal, $epistemic_modal)
    let logic_family = tff_logic_key.parse_next(input)?;

    ws.parse_next(input)?;
    "==".parse_next(input)?;
    ws.parse_next(input)?;

    // Parse the property list
    let properties = tff_logic_property_list.parse_next(input)?;

    // Handle closing paren if we had an opening one
    if has_parens {
        ws.parse_next(input)?;
        ')'.parse_next(input)?;
    }

    Ok(LogicSpecification {
        logic_family,
        properties,
    })
}

/// Parse a list of logic properties: [ prop1, prop2, ... ]
fn tff_logic_property_list<'a>(input: &mut &'a str) -> PResult<Vec<LogicProperty<'a>>> {
    delimited(
        ('[', ws),
        separated(0.., tff_logic_property, (ws, ',', ws)),
        (ws, ']'),
    )
    .parse_next(input)
}

/// Parse a single logic property: key == value or key == [ ... ]
fn tff_logic_property<'a>(input: &mut &'a str) -> PResult<LogicProperty<'a>> {
    // Parse the key (e.g., $constants, $quantification, etc.)
    let key = tff_logic_key.parse_next(input)?;
    ws.parse_next(input)?;
    "==".parse_next(input)?;
    ws.parse_next(input)?;

    // Parse the value (could be an atom, a list, or a nested property list)
    let value = tff_logic_value.parse_next(input)?;
    ws.parse_next(input)?;

    // Convert value to property
    match value {
        LogicValue::List(values) => {
            // Try to convert list items to properties if they are property assignments
            let props: Vec<LogicProperty<'a>> = values
                .into_iter()
                .filter_map(|v| match v {
                    LogicValue::Property { name, value } => Some(LogicProperty::KeyValue {
                        key: name,
                        value: *value,
                    }),
                    LogicValue::Atom(a) => Some(LogicProperty::KeyValue {
                        key: a,
                        value: LogicValue::Atom(a),
                    }),
                    _ => None,
                })
                .collect();
            if props.is_empty() {
                Ok(LogicProperty::KeyValue {
                    key,
                    value: LogicValue::List(vec![]),
                })
            } else {
                Ok(LogicProperty::KeyList { key, values: props })
            }
        }
        other => Ok(LogicProperty::KeyValue { key, value: other }),
    }
}

/// Parse a logic key: $name or atomic word
fn tff_logic_key<'a>(input: &mut &'a str) -> PResult<&'a str> {
    alt((
        // Dollar word like $constants, $quantification
        |i: &mut &'a str| {
            let start = *i;
            '$'.parse_next(i)?;
            winnow::token::take_while(1.., |c: char| c.is_ascii_alphanumeric() || c == '_')
                .parse_next(i)?;
            let len = i.as_ptr() as usize - start.as_ptr() as usize;
            Ok(&start[..len])
        },
        // Regular atomic word
        |i: &mut &'a str| {
            let word = atomic_word(i)?;
            match word {
                AtomicWord::Lower(s) => Ok(s),
                AtomicWord::SingleQuoted(s) => Ok(s),
            }
        },
    ))
    .parse_next(input)
}

/// Parse a logic value
fn tff_logic_value<'a>(input: &mut &'a str) -> PResult<LogicValue<'a>> {
    alt((
        // List: [ ... ]
        tff_logic_value_list.map(LogicValue::List),
        // Property assignment: name == value
        tff_logic_value_property,
        // Single-quoted string
        single_quoted.map(LogicValue::String),
        // Dollar word like $rigid, $constant, $modal_system_S5
        |i: &mut &'a str| {
            let start = *i;
            '$'.parse_next(i)?;
            winnow::token::take_while(1.., |c: char| c.is_ascii_alphanumeric() || c == '_')
                .parse_next(i)?;
            let len = i.as_ptr() as usize - start.as_ptr() as usize;
            Ok(LogicValue::Atom(&start[..len]))
        },
        // Regular atomic word
        |i: &mut &'a str| {
            let word = atomic_word(i)?;
            match word {
                AtomicWord::Lower(s) => Ok(LogicValue::Atom(s)),
                AtomicWord::SingleQuoted(s) => Ok(LogicValue::String(s)),
            }
        },
    ))
    .parse_next(input)
}

/// Parse a logic value list: [ value1, value2, ... ]
fn tff_logic_value_list<'a>(input: &mut &'a str) -> PResult<Vec<LogicValue<'a>>> {
    delimited(
        ('[', ws),
        separated(
            0..,
            |i: &mut &'a str| {
                let v = tff_logic_value(i)?;
                ws.parse_next(i)?;
                Ok(v)
            },
            (ws, ',', ws),
        ),
        (ws, ']'),
    )
    .parse_next(input)
}

/// Parse a logic property assignment: name == value
fn tff_logic_value_property<'a>(input: &mut &'a str) -> PResult<LogicValue<'a>> {
    let name = tff_logic_key.parse_next(input)?;
    ws.parse_next(input)?;
    "==".parse_next(input)?;
    ws.parse_next(input)?;
    let value = tff_logic_value.parse_next(input)?;

    Ok(LogicValue::Property {
        name,
        value: Box::new(value),
    })
}

fn tff_formula_tuple<'a>(input: &mut &'a str) -> PResult<Vec<TFFFormula<'a>>> {
    delimited(
        ('[', ws),
        separated(
            0..,
            |i: &mut &'a str| {
                let f = tff_formula(i)?;
                ws.parse_next(i)?;
                Ok(f)
            },
            (ws, ',', ws),
        ),
        (ws, ']'),
    )
    .parse_next(input)
}

/// Parse a TFF formula
pub fn tff_formula<'a>(input: &mut &'a str) -> PResult<TFFFormula<'a>> {
    check_cancel(input)?;
    tff_binary_formula.parse_next(input)
}

/// Parse a binary formula with proper precedence
fn tff_binary_formula<'a>(input: &mut &'a str) -> PResult<TFFFormula<'a>> {
    let left = tff_or_formula.parse_next(input)?;
    ws.parse_next(input)?;

    let result = opt((nonassoc_connective, ws, tff_or_formula)).parse_next(input)?;

    match result {
        Some((conn, _, right)) => Ok(TFFFormula::Binary {
            left: Box::new(left),
            connective: conn,
            right: Box::new(right),
        }),
        None => Ok(left),
    }
}

fn tff_or_formula<'a>(input: &mut &'a str) -> PResult<TFFFormula<'a>> {
    let mut result = tff_and_formula.parse_next(input)?;
    loop {
        let checkpoint = *input;
        ws.parse_next(input)?;
        if input.as_bytes().first() != Some(&b'|') {
            *input = checkpoint;
            break;
        }
        *input = &input[1..];
        ws.parse_next(input)?;
        let right = tff_and_formula.parse_next(input)?;
        result = TFFFormula::Binary {
            left: Box::new(result),
            connective: BinaryConnective::Or,
            right: Box::new(right),
        };
    }
    Ok(result)
}

fn tff_and_formula<'a>(input: &mut &'a str) -> PResult<TFFFormula<'a>> {
    let mut result = tff_unary_formula.parse_next(input)?;
    loop {
        let checkpoint = *input;
        ws.parse_next(input)?;
        if input.as_bytes().first() != Some(&b'&') {
            *input = checkpoint;
            break;
        }
        *input = &input[1..];
        ws.parse_next(input)?;
        let right = tff_unary_formula.parse_next(input)?;
        result = TFFFormula::Binary {
            left: Box::new(result),
            connective: BinaryConnective::And,
            right: Box::new(right),
        };
    }
    Ok(result)
}

fn tff_unary_formula<'a>(input: &mut &'a str) -> PResult<TFFFormula<'a>> {
    alt((
        preceded(('~', ws), tff_unary_formula).map(|f| TFFFormula::Negation(Box::new(f))),
        tff_unit_formula,
    ))
    .parse_next(input)
}

// fn tff_unit_formula<'a>(input: &mut &'a str) -> PResult<TFFFormula<'a>> {
//     // Optimized ordering: most common cases first, rare cases last
//     // Based on typical TPTP files: quantified formulas > atomic > parenthesized > others
//     alt((
//         // Quantified: Q [vars] : F (most common - starts with !, ?)
//         tff_quantified_formula,
//         // Type-quantified (TF1): !> [types] : F or ?* [types] : F
//         tff_type_quantified_formula,
//         // Combined infix/atomic: handles both t1 = t2 and pred(args) without backtracking
//         // This avoids expensive backtracking between tff_infix_formula and tff_atomic_formula
//         tff_infix_or_atomic,
//         // Parenthesized formula (no infix lookahead since infix_or_atomic handles parens)
//         delimited(('(', ws), tff_formula, (ws, ')')).map(|f| TFFFormula::Parens(Box::new(f))),
//         // Conditional (TXF): $ite(cond, then, else) - as a standalone formula
//         // MUST come before tff_atomic_formula to avoid $ite being parsed as defined predicate
//         tff_conditional,
//         // Let (TXF): $let(defs, body) - as a standalone formula
//         // MUST come before tff_atomic_formula to avoid $let being parsed as defined predicate
//         tff_let,
//         // Non-classical operators (rare in most TPTP problems)
//         // Non-classical long form: {$op} @ (formula) or {#op}(formula)
//         tff_nonclassical,
//         // Non-classical short form: [.](F), <.>(F), [#name](F), <#name>(F)
//         tff_short_box,
//         tff_short_diamond,
//         // Non-classical alternative short form: /.\, \./
//         tff_alt_short_box,
//         tff_alt_short_diamond,
//     ))
//     .context(StrContext::Label("tff_unit_formula"))
//     .parse_next(input)
// }

fn tff_unit_formula<'a>(input: &mut &'a str) -> PResult<TFFFormula<'a>> {
    // Optimized ordering: most common cases first, rare cases last
    // Based on typical TPTP files: quantified formulas > atomic > parenthesized > others
    alt((
        alt((
            // Quantified: Q [vars] : F (most common - starts with !, ?)
            tff_quantified_formula,
            // Type-quantified (TF1): !> [types] : F or ?* [types] : F
            tff_type_quantified_formula,
            // Combined infix/atomic: handles both t1 = t2 and pred(args) without backtracking
            // This avoids expensive backtracking between tff_infix_formula and tff_atomic_formula
            // Note: Does NOT handle '(' - that's handled by tff_paren_or_infix below
            tff_infix_or_atomic,
            // Parenthesized content: (formula) or (term/formula) = term (FOOL equality)
            // Handles both regular parenthesized formulas and FOOL formula equality
            tff_paren_or_infix,
            // Conditional (TXF): $ite(cond, then, else) - as a standalone formula
            // MUST come before tff_atomic_formula to avoid $ite being parsed as defined predicate
            tff_conditional,
            // Let (TXF): $let(defs, body) - as a standalone formula
            // MUST come before tff_atomic_formula to avoid $let being parsed as defined predicate
            tff_let,
            // Non-classical operators (rare in most TPTP problems)
            // Non-classical long form: {$op} @ (formula) or {#op}(formula)
            tff_nonclassical,
            // Non-classical short form: [.](F), <.>(F), [#name](F), <#name>(F)
            tff_short_box,
            tff_short_diamond,
        )),
        // Non-classical alternative short form: /.\, \./
        alt((tff_alt_short_box, tff_alt_short_diamond)),
    ))
    .parse_next(input)
}

/// Parse parenthesized content that may be followed by infix equality
/// Handles:
/// 1. (formula) - regular parenthesized formula
/// 2. (term) = term - FOOL infix equality with parenthesized left side
/// 3. (formula) = term - FOOL formula equality (e.g., ((X) & (Y)) = (~(~(X) | ~(Y))))
///
/// This avoids expensive backtracking by parsing the parenthesized content once
/// and then deciding based on what follows.
fn tff_paren_or_infix<'a>(input: &mut &'a str) -> PResult<TFFFormula<'a>> {
    '('.parse_next(input)?;
    ws.parse_next(input)?;

    // Parse the inner content as a full formula
    // This handles both simple terms and complex formulas
    let inner_formula = tff_formula.parse_next(input)?;

    ws.parse_next(input)?;
    ')'.parse_next(input)?;
    ws.parse_next(input)?;

    // Check for infix operator - if present, this is a FOOL equality
    let infix_op = tff_equality_op(input)?;

    match infix_op {
        Some(is_neg) => {
            ws.parse_next(input)?;
            let right = tff_simple_term.parse_next(input)?;
            // Wrap the formula as a term for the equality
            let left = TFFTerm::FormulaAsTerm(Box::new(inner_formula));
            Ok(if is_neg {
                TFFFormula::Inequality(left, right)
            } else {
                TFFFormula::Equality(left, right)
            })
        }
        None => {
            // No infix operator - just a parenthesized formula
            Ok(TFFFormula::Parens(Box::new(inner_formula)))
        }
    }
}

/// Combined parser for infix formulas (t1 = t2) and atomic formulas (pred(args))
/// This avoids expensive backtracking by parsing once and deciding based on what follows
fn tff_infix_or_atomic<'a>(input: &mut &'a str) -> PResult<TFFFormula<'a>> {
    // First, peek at the start to decide the approach
    let start_char = input
        .chars()
        .next()
        .ok_or_else(|| winnow::error::ErrMode::Backtrack(winnow::error::ContextError::new()))?;

    match start_char {
        // Uppercase: could be variable in infix or propositional variable in atomic
        'A'..='Z' => {
            let var = upper_word.parse_next(input)?;
            ws.parse_next(input)?;

            // Check for infix operator
            let infix_op = tff_equality_op(input)?;

            match infix_op {
                Some(is_neg) => {
                    ws.parse_next(input)?;
                    let right = tff_simple_term.parse_next(input)?;
                    let left = TFFTerm::Variable(var);
                    Ok(if is_neg {
                        TFFFormula::Inequality(left, right)
                    } else {
                        TFFFormula::Equality(left, right)
                    })
                }
                None => {
                    // Just a variable - atomic formula (FOOL propositional variable)
                    Ok(TFFFormula::Atomic(TFFAtomicFormula::Variable(var)))
                }
            }
        }
        // Lowercase or single-quoted: predicate/function
        'a'..='z' | '\'' => {
            let name = atomic_word.parse_next(input)?;
            ws.parse_next(input)?;

            // Optional arguments
            let args: Option<Vec<TFFTerm<'a>>> = opt(delimited(
                ('(', ws),
                separated(
                    1..,
                    |i: &mut &'a str| {
                        let t = tff_arg_term(i)?;
                        ws.parse_next(i)?;
                        Ok(t)
                    },
                    (ws, ',', ws),
                ),
                (ws, ')'),
            ))
            .parse_next(input)?;

            ws.parse_next(input)?;

            // Check for infix operator - if present, this was a function term in infix formula
            let infix_op = tff_equality_op(input)?;

            match infix_op {
                Some(is_neg) => {
                    ws.parse_next(input)?;
                    let right = tff_simple_term.parse_next(input)?;
                    let left = TFFTerm::Function(name, args.unwrap_or_default());
                    Ok(if is_neg {
                        TFFFormula::Inequality(left, right)
                    } else {
                        TFFFormula::Equality(left, right)
                    })
                }
                None => {
                    // Just a predicate - atomic formula
                    Ok(TFFFormula::Atomic(TFFAtomicFormula::Plain(
                        name,
                        args.unwrap_or_default(),
                    )))
                }
            }
        }
        // Dollar: defined predicate/function - including $ite and $let as terms in infix context
        '$' => {
            // Check for $true/$false first (atomic formulas)
            if let Some(rest) = input.strip_prefix("$true")
                && rest
                    .chars()
                    .next()
                    .is_none_or(|c| !c.is_ascii_alphanumeric() && c != '_')
            {
                *input = rest;
                ws.parse_next(input)?;

                // Check for infix operator (unlikely but possible)
                let infix_op = tff_equality_op(input)?;
                match infix_op {
                    Some(is_neg) => {
                        ws.parse_next(input)?;
                        let right = tff_simple_term.parse_next(input)?;
                        // $true as term in infix - use FormulaAsTerm
                        let left = TFFTerm::FormulaAsTerm(Box::new(TFFFormula::Atomic(
                            TFFAtomicFormula::True,
                        )));
                        return Ok(if is_neg {
                            TFFFormula::Inequality(left, right)
                        } else {
                            TFFFormula::Equality(left, right)
                        });
                    }
                    None => return Ok(TFFFormula::Atomic(TFFAtomicFormula::True)),
                }
            }
            if let Some(rest) = input.strip_prefix("$false")
                && rest
                    .chars()
                    .next()
                    .is_none_or(|c| !c.is_ascii_alphanumeric() && c != '_')
            {
                *input = rest;
                ws.parse_next(input)?;

                // Check for infix operator
                let infix_op = tff_equality_op(input)?;
                match infix_op {
                    Some(is_neg) => {
                        ws.parse_next(input)?;
                        let right = tff_simple_term.parse_next(input)?;
                        let left = TFFTerm::FormulaAsTerm(Box::new(TFFFormula::Atomic(
                            TFFAtomicFormula::False,
                        )));
                        return Ok(if is_neg {
                            TFFFormula::Inequality(left, right)
                        } else {
                            TFFFormula::Equality(left, right)
                        });
                    }
                    None => return Ok(TFFFormula::Atomic(TFFAtomicFormula::False)),
                }
            }

            // Check for $ite - parse as conditional term, then check for infix
            if input.starts_with("$ite") {
                let cond_term = tff_conditional_term.parse_next(input)?;
                ws.parse_next(input)?;

                // Check for infix operator
                let infix_op = tff_equality_op(input)?;

                match infix_op {
                    Some(is_neg) => {
                        ws.parse_next(input)?;
                        let right = tff_simple_term.parse_next(input)?;
                        Ok(if is_neg {
                            TFFFormula::Inequality(cond_term, right)
                        } else {
                            TFFFormula::Equality(cond_term, right)
                        })
                    }
                    None => {
                        // No infix - convert term back to formula
                        // $ite as formula: TFFFormula::Conditional
                        if let TFFTerm::Conditional {
                            condition,
                            then_branch,
                            else_branch,
                        } = cond_term
                        {
                            Ok(TFFFormula::Conditional {
                                condition,
                                then_branch: Box::new(term_to_formula(*then_branch)),
                                else_branch: Box::new(term_to_formula(*else_branch)),
                            })
                        } else {
                            unreachable!(
                                "tff_conditional_term should always return TFFTerm::Conditional"
                            )
                        }
                    }
                }
            }
            // Check for $let - parse as let term, then check for infix
            else if input.starts_with("$let") {
                let let_term = tff_let_term.parse_next(input)?;
                ws.parse_next(input)?;

                // Check for infix operator
                let infix_op = tff_equality_op(input)?;

                match infix_op {
                    Some(is_neg) => {
                        ws.parse_next(input)?;
                        let right = tff_simple_term.parse_next(input)?;
                        Ok(if is_neg {
                            TFFFormula::Inequality(let_term, right)
                        } else {
                            TFFFormula::Equality(let_term, right)
                        })
                    }
                    None => {
                        // No infix - convert let term back to formula
                        // For now, just wrap it as formula-as-term (not ideal but works)
                        // Actually we should handle $let formula separately
                        // Let tff_let handle standalone $let
                        Err(winnow::error::ErrMode::Backtrack(
                            winnow::error::ContextError::new(),
                        ))
                    }
                }
            } else {
                // Regular defined function/predicate
                let name = defined_word.parse_next(input)?;
                ws.parse_next(input)?;

                // Optional arguments
                let args: Option<Vec<TFFTerm<'a>>> = opt(delimited(
                    ('(', ws),
                    separated(
                        1..,
                        |i: &mut &'a str| {
                            let t = tff_arg_term(i)?;
                            ws.parse_next(i)?;
                            Ok(t)
                        },
                        (ws, ',', ws),
                    ),
                    (ws, ')'),
                ))
                .parse_next(input)?;

                ws.parse_next(input)?;

                // Check for infix operator
                let infix_op = tff_equality_op(input)?;

                match infix_op {
                    Some(is_neg) => {
                        ws.parse_next(input)?;
                        let right = tff_simple_term.parse_next(input)?;
                        let left = TFFTerm::DefinedFunction(name, args.unwrap_or_default());
                        Ok(if is_neg {
                            TFFFormula::Inequality(left, right)
                        } else {
                            TFFFormula::Equality(left, right)
                        })
                    }
                    None => {
                        // Just a defined predicate - atomic formula
                        Ok(TFFFormula::Atomic(TFFAtomicFormula::Defined(
                            name,
                            args.unwrap_or_default(),
                        )))
                    }
                }
            }
        }
        // Double dollar: system predicate/function
        _ if input.starts_with("$$") => {
            let name = system_word.parse_next(input)?;
            ws.parse_next(input)?;

            // Optional arguments
            let args: Option<Vec<TFFTerm<'a>>> = opt(delimited(
                ('(', ws),
                separated(
                    1..,
                    |i: &mut &'a str| {
                        let t = tff_arg_term(i)?;
                        ws.parse_next(i)?;
                        Ok(t)
                    },
                    (ws, ',', ws),
                ),
                (ws, ')'),
            ))
            .parse_next(input)?;

            ws.parse_next(input)?;

            // Check for infix operator
            let infix_op = tff_equality_op(input)?;

            match infix_op {
                Some(is_neg) => {
                    ws.parse_next(input)?;
                    let right = tff_simple_term.parse_next(input)?;
                    let left = TFFTerm::SystemFunction(name, args.unwrap_or_default());
                    Ok(if is_neg {
                        TFFFormula::Inequality(left, right)
                    } else {
                        TFFFormula::Equality(left, right)
                    })
                }
                None => {
                    // Just a system predicate - atomic formula
                    Ok(TFFFormula::Atomic(TFFAtomicFormula::System(
                        name,
                        args.unwrap_or_default(),
                    )))
                }
            }
        }
        // Number: can only be in infix context (term = term)
        '0'..='9' | '+' | '-' => {
            let left = number.map(TFFTerm::Number).parse_next(input)?;
            ws.parse_next(input)?;

            let is_neg = alt(("!=".value(true), "=".value(false))).parse_next(input)?;
            ws.parse_next(input)?;
            let right = tff_simple_term.parse_next(input)?;

            Ok(if is_neg {
                TFFFormula::Inequality(left, right)
            } else {
                TFFFormula::Equality(left, right)
            })
        }
        // Distinct object: can only be in infix context
        '"' => {
            let left = distinct_object
                .map(TFFTerm::DistinctObject)
                .parse_next(input)?;
            ws.parse_next(input)?;

            let is_neg = alt(("!=".value(true), "=".value(false))).parse_next(input)?;
            ws.parse_next(input)?;
            let right = tff_simple_term.parse_next(input)?;

            Ok(if is_neg {
                TFFFormula::Inequality(left, right)
            } else {
                TFFFormula::Equality(left, right)
            })
        }
        // '(' is NOT handled here - it's handled by tff_paren_or_infix in tff_unit_formula
        // This avoids expensive backtracking for parenthesized content
        // Not something this function handles
        _ => Err(winnow::error::ErrMode::Backtrack(
            winnow::error::ContextError::new(),
        )),
    }
}

/// Parse non-classical long form: {$op}@(formula) or {#op}(formula)
fn tff_nonclassical<'a>(input: &mut &'a str) -> PResult<TFFFormula<'a>> {
    '{'.parse_next(input)?;

    // Can be either # or $ prefix
    let _prefix = alt(('#', '$')).parse_next(input)?;

    // Parse operator name
    let op_name: &str =
        winnow::token::take_while(1.., |c: char| c.is_ascii_alphanumeric() || c == '_')
            .parse_next(input)?;

    // Optional index (with # prefix): {$knows(#agent)} or {#op:index}
    let index = opt(alt((
        // Parenthesized index: {$knows(#agent)}
        delimited(
            ('(', ws, '#'),
            winnow::token::take_while(1.., |c: char| c.is_ascii_alphanumeric() || c == '_'),
            (ws, ')'),
        ),
        // Colon index: {#op:index}
        preceded(
            ':',
            winnow::token::take_while(1.., |c: char| c.is_ascii_alphanumeric() || c == '_'),
        ),
    )))
    .parse_next(input)?;

    '}'.parse_next(input)?;
    ws.parse_next(input)?;

    let operator = match op_name {
        "box" => NonClassicalOperator::Box,
        "dia" => NonClassicalOperator::Diamond,
        "always" => NonClassicalOperator::Always,
        "eventually" => NonClassicalOperator::Eventually,
        "knows" => match index {
            Some(idx) => NonClassicalOperator::Custom {
                name: "knows",
                index: Some(idx),
            },
            None => NonClassicalOperator::Knows,
        },
        "believes" => match index {
            Some(idx) => NonClassicalOperator::Custom {
                name: "believes",
                index: Some(idx),
            },
            None => NonClassicalOperator::Believes,
        },
        _ => NonClassicalOperator::Custom {
            name: op_name,
            index,
        },
    };

    // Formula argument: @ (formula) or (formula)
    let formula = alt((
        // THF application style: @ formula
        preceded((ws, '@', ws), tff_unary_formula),
        // Direct parenthesized: (formula)
        delimited(('(', ws), tff_formula, (ws, ')')),
    ))
    .parse_next(input)?;

    Ok(TFFFormula::NonClassical {
        operator,
        formula: Box::new(formula),
    })
}

/// Parse short-form box operator: [.], [..], or [#name]
fn tff_short_box<'a>(input: &mut &'a str) -> PResult<TFFFormula<'a>> {
    '['.parse_next(input)?;

    let operator = alt((
        // [..] - double dot box
        "..".value(NonClassicalOperator::Box),
        // [.] - single dot box
        '.'.value(NonClassicalOperator::Box),
        // [#name] - named box
        |i: &mut &'a str| {
            '#'.parse_next(i)?;
            let name: &str =
                winnow::token::take_while(1.., |c: char| c.is_ascii_alphanumeric() || c == '_')
                    .parse_next(i)?;
            Ok(NonClassicalOperator::ShortBox(Some(name)))
        },
    ))
    .parse_next(input)?;

    ']'.parse_next(input)?;
    ws.parse_next(input)?;

    // Formula argument can be parenthesized or directly following
    let formula = alt((
        delimited(('(', ws), tff_formula, (ws, ')')),
        tff_unary_formula,
    ))
    .parse_next(input)?;

    Ok(TFFFormula::NonClassical {
        operator,
        formula: Box::new(formula),
    })
}

/// Parse short-form diamond operator: <.>, <..>, or <#name>
fn tff_short_diamond<'a>(input: &mut &'a str) -> PResult<TFFFormula<'a>> {
    '<'.parse_next(input)?;

    let operator = alt((
        // <..> - double dot diamond
        "..".value(NonClassicalOperator::Diamond),
        // <.> - single dot diamond
        '.'.value(NonClassicalOperator::Diamond),
        // <#name> - named diamond
        |i: &mut &'a str| {
            '#'.parse_next(i)?;
            let name: &str =
                winnow::token::take_while(1.., |c: char| c.is_ascii_alphanumeric() || c == '_')
                    .parse_next(i)?;
            Ok(NonClassicalOperator::ShortDiamond(Some(name)))
        },
    ))
    .parse_next(input)?;

    '>'.parse_next(input)?;
    ws.parse_next(input)?;

    // Formula argument can be parenthesized or directly following
    let formula = alt((
        delimited(('(', ws), tff_formula, (ws, ')')),
        tff_unary_formula,
    ))
    .parse_next(input)?;

    Ok(TFFFormula::NonClassical {
        operator,
        formula: Box::new(formula),
    })
}

/// Parse alternative short-form box operator: /.\, /..\, or /#name\
fn tff_alt_short_box<'a>(input: &mut &'a str) -> PResult<TFFFormula<'a>> {
    '/'.parse_next(input)?;

    let operator = alt((
        // /..\ - double dot box
        "..".value(NonClassicalOperator::Box),
        // /.\ - single dot box
        '.'.value(NonClassicalOperator::Box),
        // /#name\ - named box
        |i: &mut &'a str| {
            '#'.parse_next(i)?;
            let name: &str =
                winnow::token::take_while(1.., |c: char| c.is_ascii_alphanumeric() || c == '_')
                    .parse_next(i)?;
            Ok(NonClassicalOperator::ShortBox(Some(name)))
        },
    ))
    .parse_next(input)?;

    '\\'.parse_next(input)?;
    ws.parse_next(input)?;

    // Formula argument can be parenthesized or directly following
    let formula = alt((
        delimited(('(', ws), tff_formula, (ws, ')')),
        tff_unary_formula,
    ))
    .parse_next(input)?;

    Ok(TFFFormula::NonClassical {
        operator,
        formula: Box::new(formula),
    })
}

/// Parse alternative short-form diamond operator: \./, \..\, or \#name/
fn tff_alt_short_diamond<'a>(input: &mut &'a str) -> PResult<TFFFormula<'a>> {
    '\\'.parse_next(input)?;

    let operator = alt((
        // \../ - double dot diamond
        "..".value(NonClassicalOperator::Diamond),
        // \./ - single dot diamond
        '.'.value(NonClassicalOperator::Diamond),
        // \#name/ - named diamond
        |i: &mut &'a str| {
            '#'.parse_next(i)?;
            let name: &str =
                winnow::token::take_while(1.., |c: char| c.is_ascii_alphanumeric() || c == '_')
                    .parse_next(i)?;
            Ok(NonClassicalOperator::ShortDiamond(Some(name)))
        },
    ))
    .parse_next(input)?;

    '/'.parse_next(input)?;
    ws.parse_next(input)?;

    // Formula argument can be parenthesized or directly following
    let formula = alt((
        delimited(('(', ws), tff_formula, (ws, ')')),
        tff_unary_formula,
    ))
    .parse_next(input)?;

    Ok(TFFFormula::NonClassical {
        operator,
        formula: Box::new(formula),
    })
}

fn tff_quantified_formula<'a>(input: &mut &'a str) -> PResult<TFFFormula<'a>> {
    let q = quantifier.parse_next(input)?;
    ws.parse_next(input)?;
    '['.parse_next(input)?;
    ws.parse_next(input)?;

    let vars: Vec<TFFVariable<'a>> =
        separated(1.., tff_variable, (ws, ',', ws)).parse_next(input)?;

    ws.parse_next(input)?;
    ']'.parse_next(input)?;
    ws.parse_next(input)?;
    ':'.parse_next(input)?;
    ws.parse_next(input)?;

    let formula = tff_unary_formula.parse_next(input)?;

    Ok(TFFFormula::Quantified {
        quantifier: q,
        variables: vars,
        formula: Box::new(formula),
    })
}

fn tff_type_quantified_formula<'a>(input: &mut &'a str) -> PResult<TFFFormula<'a>> {
    let q = alt((
        "!>".value(TypeQuantifier::ForallType),
        "?*".value(TypeQuantifier::ExistsType),
    ))
    .parse_next(input)?;

    ws.parse_next(input)?;
    '['.parse_next(input)?;
    ws.parse_next(input)?;

    let type_vars: Vec<&'a str> = separated(
        1..,
        |i: &mut &'a str| {
            let v = upper_word(i)?;
            // Optional : $tType
            ws.parse_next(i)?;
            opt((':', ws, "$tType")).parse_next(i)?;
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

    let formula = tff_unary_formula.parse_next(input)?;

    Ok(TFFFormula::TypeQuantified {
        quantifier: q,
        type_variables: type_vars,
        formula: Box::new(formula),
    })
}

pub fn tff_variable<'a>(input: &mut &'a str) -> PResult<TFFVariable<'a>> {
    let name = upper_word.parse_next(input)?;
    ws.parse_next(input)?;

    // Optional type annotation
    let typ = opt(preceded((':', ws), tff_type)).parse_next(input)?;

    Ok(TFFVariable { name, typ })
}

fn tff_conditional<'a>(input: &mut &'a str) -> PResult<TFFFormula<'a>> {
    "$ite".parse_next(input)?;
    ws.parse_next(input)?;
    '('.parse_next(input)?;
    ws.parse_next(input)?;

    let condition = tff_formula.parse_next(input)?;
    ws.parse_next(input)?;
    ','.parse_next(input)?;
    ws.parse_next(input)?;

    let then_branch = tff_formula.parse_next(input)?;
    ws.parse_next(input)?;
    ','.parse_next(input)?;
    ws.parse_next(input)?;

    let else_branch = tff_formula.parse_next(input)?;
    ws.parse_next(input)?;
    ')'.parse_next(input)?;

    Ok(TFFFormula::Conditional {
        condition: Box::new(condition),
        then_branch: Box::new(then_branch),
        else_branch: Box::new(else_branch),
    })
}

fn tff_let<'a>(input: &mut &'a str) -> PResult<TFFFormula<'a>> {
    "$let".parse_next(input)?;
    ws.parse_next(input)?;
    '('.parse_next(input)?;
    ws.parse_next(input)?;

    // Parse type specification: either "var: type" or "[var1: type1, var2: type2]"
    let type_specs = alt((
        // Single: var: type
        tff_let_type_spec.map(|s| vec![s]),
        // Multiple: [var1: type1, var2: type2]
        delimited(
            ('[', ws),
            separated(1.., tff_let_type_spec, (ws, ',', ws)),
            (ws, ']'),
        ),
    ))
    .parse_next(input)?;

    ws.parse_next(input)?;
    ','.parse_next(input)?;
    ws.parse_next(input)?;

    // Parse bindings: either "var := expr", "[var1 := expr1, var2 := expr2]", or "[var1, var2] := expr"
    let bindings = alt((
        // Tuple unpacking: [var1, var2, ...] := expr
        tff_let_tuple_binding,
        // Single: var := expr
        tff_let_binding.map(|b| vec![b]),
        // Multiple individual bindings: [var1 := expr1, var2 := expr2]
        delimited(
            ('[', ws),
            separated(1.., tff_let_binding, (ws, ',', ws)),
            (ws, ']'),
        ),
    ))
    .parse_next(input)?;

    ws.parse_next(input)?;
    ','.parse_next(input)?;
    ws.parse_next(input)?;

    // Parse body - can be either a formula or a term (e.g., tuple for parallel assignment)
    // Try formula first, then fall back to term if that fails
    let body = alt((
        tff_formula.map(TFFLetBody::Formula),
        tff_term.map(TFFLetBody::Term),
    ))
    .parse_next(input)?;
    ws.parse_next(input)?;
    ')'.parse_next(input)?;

    // Convert to TFFLetDef
    let definitions = type_specs
        .into_iter()
        .zip(bindings)
        .map(|((name, typ), (_bind_name, value))| TFFLetDef {
            symbol: name,
            type_args: vec![],
            params: vec![],
            typ: Some(typ),
            definition: value,
        })
        .collect();

    Ok(TFFFormula::Let {
        definitions,
        body: Box::new(body),
    })
}

/// Parse a let type specification: var: type or fn(args): type
fn tff_let_type_spec<'a>(input: &mut &'a str) -> PResult<(AtomicWord<'a>, TFFType<'a>)> {
    let name = atomic_word.parse_next(input)?;
    ws.parse_next(input)?;

    // Optional function parameters
    let _params: Option<Vec<&str>> = opt(delimited(
        ('(', ws),
        separated(
            0..,
            |i: &mut &'a str| {
                let v = upper_word(i)?;
                ws.parse_next(i)?;
                Ok(v)
            },
            (ws, ',', ws),
        ),
        (ws, ')'),
    ))
    .parse_next(input)?;

    ws.parse_next(input)?;
    ':'.parse_next(input)?;
    ws.parse_next(input)?;

    let typ = tff_type.parse_next(input)?;
    ws.parse_next(input)?;

    Ok((name, typ))
}

/// Parse tuple unpacking let binding: [var1, var2, ...] := expr
/// Returns a vector of (name, body) pairs where the body is the same tuple term
fn tff_let_tuple_binding<'a>(
    input: &mut &'a str,
) -> PResult<Vec<(AtomicWord<'a>, TFFLetBody<'a>)>> {
    // Parse the list of variable names: [var1, var2, ...]
    let names: Vec<AtomicWord<'a>> = delimited(
        ('[', ws),
        separated(
            1..,
            |i: &mut &'a str| {
                let n = atomic_word.parse_next(i)?;
                ws.parse_next(i)?;
                Ok(n)
            },
            (ws, ',', ws),
        ),
        (ws, ']'),
    )
    .parse_next(input)?;

    ws.parse_next(input)?;
    ":=".parse_next(input)?;
    ws.parse_next(input)?;

    // The value should be a term (typically a tuple or $ite that returns a tuple)
    let value = alt((
        tff_term.map(TFFLetBody::Term),
        tff_formula.map(TFFLetBody::Formula),
    ))
    .parse_next(input)?;

    ws.parse_next(input)?;

    // Create bindings: all names are bound to the same tuple expression
    // The semantic interpretation is that each name gets the corresponding tuple element
    // For parsing purposes, we bind each name to the whole tuple term
    Ok(names
        .into_iter()
        .map(|name| (name, value.clone()))
        .collect())
}

/// Parse a let binding: var := expr or fn(args) := expr
fn tff_let_binding<'a>(input: &mut &'a str) -> PResult<(AtomicWord<'a>, TFFLetBody<'a>)> {
    let name = atomic_word.parse_next(input)?;
    ws.parse_next(input)?;

    // Optional function parameters
    let _params: Option<Vec<&str>> = opt(delimited(
        ('(', ws),
        separated(
            0..,
            |i: &mut &'a str| {
                let v = upper_word(i)?;
                ws.parse_next(i)?;
                Ok(v)
            },
            (ws, ',', ws),
        ),
        (ws, ')'),
    ))
    .parse_next(input)?;

    ws.parse_next(input)?;
    ":=".parse_next(input)?;
    ws.parse_next(input)?;

    // The value can be a term or a formula - try term first
    let value = alt((
        tff_term.map(TFFLetBody::Term),
        tff_formula.map(TFFLetBody::Formula),
    ))
    .parse_next(input)?;

    ws.parse_next(input)?;

    Ok((name, value))
}

/// Parse a TFF atomic formula
/// Optimized ordering: most common (plain atomic) first
pub fn tff_atomic_formula<'a>(input: &mut &'a str) -> PResult<TFFAtomicFormula<'a>> {
    alt((
        // Plain atomic: pred(args) or prop (most common)
        tff_plain_atomic,
        // FOOL/TXF: variable of type $o used as atomic formula (common in FOOL)
        upper_word.map(TFFAtomicFormula::Variable),
        // Defined predicate: $pred(args) - includes $true, $false
        tff_defined_atomic,
        // System predicate: $$pred(args) (rare)
        tff_system_atomic,
        // Explicit $true/$false after defined_atomic catches most cases
        "$true".value(TFFAtomicFormula::True),
        "$false".value(TFFAtomicFormula::False),
    ))
    .parse_next(input)
}

fn tff_plain_atomic<'a>(input: &mut &'a str) -> PResult<TFFAtomicFormula<'a>> {
    let pred = atomic_word.parse_next(input)?;
    ws.parse_next(input)?;

    let args = opt(delimited(
        ('(', ws),
        separated(
            1..,
            |i: &mut &'a str| {
                let t = tff_arg_term(i)?;
                ws.parse_next(i)?;
                Ok(t)
            },
            (ws, ',', ws),
        ),
        (ws, ')'),
    ))
    .parse_next(input)?;

    Ok(TFFAtomicFormula::Plain(pred, args.unwrap_or_default()))
}

fn tff_defined_atomic<'a>(input: &mut &'a str) -> PResult<TFFAtomicFormula<'a>> {
    let pred = defined_word.parse_next(input)?;
    ws.parse_next(input)?;

    let args = opt(delimited(
        ('(', ws),
        separated(
            1..,
            |i: &mut &'a str| {
                let t = tff_arg_term(i)?;
                ws.parse_next(i)?;
                Ok(t)
            },
            (ws, ',', ws),
        ),
        (ws, ')'),
    ))
    .parse_next(input)?;

    Ok(TFFAtomicFormula::Defined(pred, args.unwrap_or_default()))
}

fn tff_system_atomic<'a>(input: &mut &'a str) -> PResult<TFFAtomicFormula<'a>> {
    let pred = system_word.parse_next(input)?;
    ws.parse_next(input)?;

    let args = opt(delimited(
        ('(', ws),
        separated(
            1..,
            |i: &mut &'a str| {
                let t = tff_arg_term(i)?;
                ws.parse_next(i)?;
                Ok(t)
            },
            (ws, ',', ws),
        ),
        (ws, ')'),
    ))
    .parse_next(input)?;

    Ok(TFFAtomicFormula::System(pred, args.unwrap_or_default()))
}

/// Parse a TFF term
/// Optimized ordering: most common cases first (variables, functions, numbers)
/// Note: $ite and $let must come BEFORE tff_defined_term to match first
pub fn tff_term<'a>(input: &mut &'a str) -> PResult<TFFTerm<'a>> {
    alt((
        alt((
            // Variable (very common - starts with uppercase)
            upper_word.map(TFFTerm::Variable),
            // Function/constant (very common - starts with lowercase or quoted)
            tff_function_term,
            // Number (common)
            number.map(TFFTerm::Number),
            // Distinct object (less common - starts with ")
            distinct_object.map(TFFTerm::DistinctObject),
            // Conditional term (TXF): $ite(cond, then, else) - MUST be before tff_defined_term
            tff_conditional_term,
            // Let term (TXF): $let(...) - MUST be before tff_defined_term
            tff_let_term,
            // Defined function: $f(args) - includes $sum, $difference, etc.
            tff_defined_term,
            // System function: $$f(args) (rare)
            tff_system_term,
            // FOOL: Quantified formula as term - starts with ! or ?
            tff_quantified_as_term,
        )),
        alt((
            // FOOL: Negation formula as term - starts with ~
            tff_negation_as_term,
            // FOOL: Formula as term - parenthesized formula in term position
            tff_formula_as_term,
            // Tuple (TXF): [t1, t2, ...] (rare)
            tff_tuple,
        )),
    ))
    .parse_next(input)
}

/// Parse a $let term (TXF feature)
fn tff_let_term<'a>(input: &mut &'a str) -> PResult<TFFTerm<'a>> {
    "$let".parse_next(input)?;
    ws.parse_next(input)?;
    '('.parse_next(input)?;
    ws.parse_next(input)?;

    // Parse type specification: either "var: type" or "[var1: type1, var2: type2]"
    let type_specs = alt((
        tff_let_type_spec.map(|s| vec![s]),
        delimited(
            ('[', ws),
            separated(1.., tff_let_type_spec, (ws, ',', ws)),
            (ws, ']'),
        ),
    ))
    .parse_next(input)?;

    ws.parse_next(input)?;
    ','.parse_next(input)?;
    ws.parse_next(input)?;

    // Parse bindings: either "var := expr", "[var1 := expr1, var2 := expr2]", or "[var1, var2] := expr"
    let bindings = alt((
        // Tuple unpacking: [var1, var2, ...] := expr
        tff_let_tuple_binding_term,
        // Single binding: var := expr
        tff_let_binding_term.map(|b| vec![b]),
        // Multiple individual bindings: [var1 := expr1, var2 := expr2]
        delimited(
            ('[', ws),
            separated(1.., tff_let_binding_term, (ws, ',', ws)),
            (ws, ']'),
        ),
    ))
    .parse_next(input)?;

    ws.parse_next(input)?;
    ','.parse_next(input)?;
    ws.parse_next(input)?;

    // Parse body (as term)
    let body = tff_term.parse_next(input)?;
    ws.parse_next(input)?;
    ')'.parse_next(input)?;

    // Convert to TFFLetDef
    let definitions = type_specs
        .into_iter()
        .zip(bindings)
        .map(|((name, typ), (_bind_name, value))| TFFLetDef {
            symbol: name,
            type_args: vec![],
            params: vec![],
            typ: Some(typ),
            definition: value,
        })
        .collect();

    Ok(TFFTerm::Let {
        definitions,
        body: Box::new(body),
    })
}

/// Parse tuple unpacking let binding for term context: [var1, var2, ...] := expr
/// Returns a vector of (name, body) pairs where the body is the same term
fn tff_let_tuple_binding_term<'a>(
    input: &mut &'a str,
) -> PResult<Vec<(AtomicWord<'a>, TFFLetBody<'a>)>> {
    // Parse the list of variable names: [var1, var2, ...]
    let names: Vec<AtomicWord<'a>> = delimited(
        ('[', ws),
        separated(
            1..,
            |i: &mut &'a str| {
                let n = atomic_word.parse_next(i)?;
                ws.parse_next(i)?;
                Ok(n)
            },
            (ws, ',', ws),
        ),
        (ws, ']'),
    )
    .parse_next(input)?;

    ws.parse_next(input)?;
    ":=".parse_next(input)?;
    ws.parse_next(input)?;

    // The value should be a term (typically a tuple or $ite that returns a tuple)
    let value = alt((
        tff_term.map(TFFLetBody::Term),
        tff_formula.map(TFFLetBody::Formula),
    ))
    .parse_next(input)?;

    ws.parse_next(input)?;

    // Create bindings: all names are bound to the same tuple expression
    Ok(names
        .into_iter()
        .map(|name| (name, value.clone()))
        .collect())
}

/// Parse a let binding for term context: var := expr
fn tff_let_binding_term<'a>(input: &mut &'a str) -> PResult<(AtomicWord<'a>, TFFLetBody<'a>)> {
    let name = atomic_word.parse_next(input)?;
    ws.parse_next(input)?;

    // Optional function parameters
    let _params: Option<Vec<&str>> = opt(delimited(
        ('(', ws),
        separated(
            0..,
            |i: &mut &'a str| {
                let v = upper_word(i)?;
                ws.parse_next(i)?;
                Ok(v)
            },
            (ws, ',', ws),
        ),
        (ws, ')'),
    ))
    .parse_next(input)?;

    ws.parse_next(input)?;
    ":=".parse_next(input)?;
    ws.parse_next(input)?;

    // The body can be a term or a formula
    let value = alt((
        tff_term.map(TFFLetBody::Term),
        tff_formula.map(TFFLetBody::Formula),
    ))
    .parse_next(input)?;

    ws.parse_next(input)?;

    Ok((name, value))
}

/// Parse a $ite term (TXF feature)
fn tff_conditional_term<'a>(input: &mut &'a str) -> PResult<TFFTerm<'a>> {
    "$ite".parse_next(input)?;
    ws.parse_next(input)?;
    '('.parse_next(input)?;
    ws.parse_next(input)?;

    let condition = tff_formula.parse_next(input)?;
    ws.parse_next(input)?;
    ','.parse_next(input)?;
    ws.parse_next(input)?;

    let then_branch = tff_term.parse_next(input)?;
    ws.parse_next(input)?;
    ','.parse_next(input)?;
    ws.parse_next(input)?;

    let else_branch = tff_term.parse_next(input)?;
    ws.parse_next(input)?;
    ')'.parse_next(input)?;

    Ok(TFFTerm::Conditional {
        condition: Box::new(condition),
        then_branch: Box::new(then_branch),
        else_branch: Box::new(else_branch),
    })
}

/// Parse a term in a function argument position (allows FOOL infix equality as term)
/// Optimized to avoid backtracking - parse term once, then check for infix operator
fn tff_arg_term<'a>(input: &mut &'a str) -> PResult<TFFTerm<'a>> {
    // Parse a simple term first (without full recursion into FOOL features)
    let left = tff_simple_term.parse_next(input)?;
    ws.parse_next(input)?;

    // Check if followed by = or != (infix equality as term - FOOL feature)
    // Use opt() to avoid backtracking - if no infix operator, just return the term
    let infix_op = tff_equality_op(input)?;

    match infix_op {
        Some(is_neg) => {
            // Parse right side of equality
            ws.parse_next(input)?;
            let right = tff_simple_term.parse_next(input)?;

            let formula = if is_neg {
                TFFFormula::Inequality(left, right)
            } else {
                TFFFormula::Equality(left, right)
            };
            Ok(TFFTerm::FormulaAsTerm(Box::new(formula)))
        }
        None => {
            // Not an infix equality - check if this simple term needs to be expanded to full term
            // If the simple term is complete (variable, number, function with args, etc.), use it
            // Otherwise we need to handle cases where tff_term might parse more (rare)
            Ok(left)
        }
    }
}

/// Parse a simple term (with limited FOOL features) - used for infix equality
/// This uses simple_function_term which doesn't recurse back to full FOOL features
/// but still supports negation and parenthesized formulas within arguments
/// Optimized ordering: most common cases first
fn tff_simple_term<'a>(input: &mut &'a str) -> PResult<TFFTerm<'a>> {
    alt((
        alt((
            // Variable (very common - starts with uppercase)
            upper_word.map(TFFTerm::Variable),
            // Function/constant (very common - starts with lowercase or quoted)
            tff_function_term_simple,
            // Number (common)
            number.map(TFFTerm::Number),
            // Distinct object (less common - starts with ")
            distinct_object.map(TFFTerm::DistinctObject),
            // FOOL: Conditional term $ite(...) and let term $let(...)
            // MUST be before tff_defined_term_simple to avoid partial parsing
            tff_conditional_term,
            tff_let_term,
            // Defined term: $f(args)
            tff_defined_term_simple,
            // System term: $$f(args) (rare)
            tff_system_term_simple,
            // FOOL: Quantified formula as term - starts with ! or ?
            tff_quantified_as_term_simple,
        )),
        alt((
            // FOOL: Negation formula as term - starts with ~
            tff_negation_as_term_simple,
            // FOOL: Formula as term - parenthesized formula in term position
            tff_formula_as_term_simple,
            // Tuple (rare - starts with [)
            tff_tuple_simple,
        )),
    ))
    .parse_next(input)
}

/// Simple function term - allows FOOL infix equality in arguments via tff_arg_term
fn tff_function_term_simple<'a>(input: &mut &'a str) -> PResult<TFFTerm<'a>> {
    let name = atomic_word.parse_next(input)?;
    ws.parse_next(input)?;

    let args = opt(delimited(
        ('(', ws),
        separated(
            1..,
            |i: &mut &'a str| {
                let t = tff_arg_term(i)?;
                ws.parse_next(i)?;
                Ok(t)
            },
            (ws, ',', ws),
        ),
        (ws, ')'),
    ))
    .parse_next(input)?;

    Ok(TFFTerm::Function(name, args.unwrap_or_default()))
}

/// Simple defined term - allows FOOL infix equality in arguments via tff_arg_term
fn tff_defined_term_simple<'a>(input: &mut &'a str) -> PResult<TFFTerm<'a>> {
    let name = defined_word.parse_next(input)?;
    ws.parse_next(input)?;

    let args = opt(delimited(
        ('(', ws),
        separated(
            1..,
            |i: &mut &'a str| {
                let t = tff_arg_term(i)?;
                ws.parse_next(i)?;
                Ok(t)
            },
            (ws, ',', ws),
        ),
        (ws, ')'),
    ))
    .parse_next(input)?;

    Ok(TFFTerm::DefinedFunction(name, args.unwrap_or_default()))
}

/// Simple system term - allows FOOL infix equality in arguments via tff_arg_term
fn tff_system_term_simple<'a>(input: &mut &'a str) -> PResult<TFFTerm<'a>> {
    let name = system_word.parse_next(input)?;
    ws.parse_next(input)?;

    let args = opt(delimited(
        ('(', ws),
        separated(
            1..,
            |i: &mut &'a str| {
                let t = tff_arg_term(i)?;
                ws.parse_next(i)?;
                Ok(t)
            },
            (ws, ',', ws),
        ),
        (ws, ')'),
    ))
    .parse_next(input)?;

    Ok(TFFTerm::SystemFunction(name, args.unwrap_or_default()))
}

/// Simple tuple - allows FOOL infix equality in elements via tff_arg_term
fn tff_tuple_simple<'a>(input: &mut &'a str) -> PResult<TFFTerm<'a>> {
    delimited(
        ('[', ws),
        separated(
            0..,
            |i: &mut &'a str| {
                let t = tff_arg_term(i)?;
                ws.parse_next(i)?;
                Ok(t)
            },
            (ws, ',', ws),
        ),
        (ws, ']'),
    )
    .map(TFFTerm::Tuple)
    .parse_next(input)
}

/// Parse a quantified formula as a simple term (for infix equality arguments)
fn tff_quantified_as_term_simple<'a>(input: &mut &'a str) -> PResult<TFFTerm<'a>> {
    let f = tff_quantified_formula.parse_next(input)?;
    Ok(TFFTerm::FormulaAsTerm(Box::new(f)))
}

/// Parse a negation formula as a simple term (for infix equality arguments)
fn tff_negation_as_term_simple<'a>(input: &mut &'a str) -> PResult<TFFTerm<'a>> {
    '~'.parse_next(input)?;
    ws.parse_next(input)?;
    let f = tff_unary_formula_simple.parse_next(input)?;
    Ok(TFFTerm::FormulaAsTerm(Box::new(TFFFormula::Negation(
        Box::new(f),
    ))))
}

/// Parse a unary formula for simple terms (avoids infix equality recursion)
fn tff_unary_formula_simple<'a>(input: &mut &'a str) -> PResult<TFFFormula<'a>> {
    alt((
        tff_atomic_formula.map(TFFFormula::Atomic),
        tff_quantified_formula,
        tff_negation_simple,
        tff_conditional,
        tff_let,
        tff_parens_formula_simple,
    ))
    .parse_next(input)
}

/// Parse a negation formula for simple terms
fn tff_negation_simple<'a>(input: &mut &'a str) -> PResult<TFFFormula<'a>> {
    '~'.parse_next(input)?;
    ws.parse_next(input)?;
    let inner = tff_unary_formula_simple.parse_next(input)?;
    Ok(TFFFormula::Negation(Box::new(inner)))
}

/// Parse a parenthesized formula for simple terms
fn tff_parens_formula_simple<'a>(input: &mut &'a str) -> PResult<TFFFormula<'a>> {
    '('.parse_next(input)?;
    ws.parse_next(input)?;
    let f = tff_formula.parse_next(input)?;
    ws.parse_next(input)?;
    ')'.parse_next(input)?;
    Ok(TFFFormula::Parens(Box::new(f)))
}

/// Parse a parenthesized formula as a simple term (for infix equality arguments)
fn tff_formula_as_term_simple<'a>(input: &mut &'a str) -> PResult<TFFTerm<'a>> {
    '('.parse_next(input)?;
    ws.parse_next(input)?;
    let f = tff_formula.parse_next(input)?;
    ws.parse_next(input)?;
    ')'.parse_next(input)?;
    // FormulaAsTerm display already adds parentheses, so don't wrap in Parens
    Ok(TFFTerm::FormulaAsTerm(Box::new(f)))
}

/// Parse a quantified formula as a term (FOOL/TXF feature)
fn tff_quantified_as_term<'a>(input: &mut &'a str) -> PResult<TFFTerm<'a>> {
    let f = tff_quantified_formula.parse_next(input)?;
    Ok(TFFTerm::FormulaAsTerm(Box::new(f)))
}

/// Parse a negation formula as a term (FOOL/TXF feature)
fn tff_negation_as_term<'a>(input: &mut &'a str) -> PResult<TFFTerm<'a>> {
    '~'.parse_next(input)?;
    ws.parse_next(input)?;
    let f = tff_unary_formula.parse_next(input)?;
    Ok(TFFTerm::FormulaAsTerm(Box::new(TFFFormula::Negation(
        Box::new(f),
    ))))
}

/// Parse a parenthesized formula as a term (FOOL/TXF feature)
fn tff_formula_as_term<'a>(input: &mut &'a str) -> PResult<TFFTerm<'a>> {
    '('.parse_next(input)?;
    ws.parse_next(input)?;
    let f = tff_formula.parse_next(input)?;
    ws.parse_next(input)?;
    ')'.parse_next(input)?;
    // FormulaAsTerm display already adds parentheses, so don't wrap in Parens
    Ok(TFFTerm::FormulaAsTerm(Box::new(f)))
}

fn tff_tuple<'a>(input: &mut &'a str) -> PResult<TFFTerm<'a>> {
    delimited(
        ('[', ws),
        separated(
            0..,
            |i: &mut &'a str| {
                let t = tff_arg_term(i)?;
                ws.parse_next(i)?;
                Ok(t)
            },
            (ws, ',', ws),
        ),
        (ws, ']'),
    )
    .map(TFFTerm::Tuple)
    .parse_next(input)
}

fn tff_function_term<'a>(input: &mut &'a str) -> PResult<TFFTerm<'a>> {
    let name = atomic_word.parse_next(input)?;
    ws.parse_next(input)?;

    let args = opt(delimited(
        ('(', ws),
        separated(
            1..,
            |i: &mut &'a str| {
                let t = tff_arg_term(i)?;
                ws.parse_next(i)?;
                Ok(t)
            },
            (ws, ',', ws),
        ),
        (ws, ')'),
    ))
    .parse_next(input)?;

    Ok(TFFTerm::Function(name, args.unwrap_or_default()))
}

fn tff_defined_term<'a>(input: &mut &'a str) -> PResult<TFFTerm<'a>> {
    let name = defined_word.parse_next(input)?;
    ws.parse_next(input)?;

    let args = opt(delimited(
        ('(', ws),
        separated(
            1..,
            |i: &mut &'a str| {
                let t = tff_arg_term(i)?;
                ws.parse_next(i)?;
                Ok(t)
            },
            (ws, ',', ws),
        ),
        (ws, ')'),
    ))
    .parse_next(input)?;

    Ok(TFFTerm::DefinedFunction(name, args.unwrap_or_default()))
}

fn tff_system_term<'a>(input: &mut &'a str) -> PResult<TFFTerm<'a>> {
    let name = system_word.parse_next(input)?;
    ws.parse_next(input)?;

    let args = opt(delimited(
        ('(', ws),
        separated(
            1..,
            |i: &mut &'a str| {
                let t = tff_arg_term(i)?;
                ws.parse_next(i)?;
                Ok(t)
            },
            (ws, ',', ws),
        ),
        (ws, ')'),
    ))
    .parse_next(input)?;

    Ok(TFFTerm::SystemFunction(name, args.unwrap_or_default()))
}

/// Parse a TFF type
pub fn tff_type<'a>(input: &mut &'a str) -> PResult<TFFType<'a>> {
    alt((tff_mapping_type, tff_arrow_type)).parse_next(input)
}

/// Parse top-level type (with optional type quantification)
pub fn tff_top_level_type<'a>(input: &mut &'a str) -> PResult<TFFType<'a>> {
    alt((tff_quantified_type, tff_type)).parse_next(input)
}

fn tff_quantified_type<'a>(input: &mut &'a str) -> PResult<TFFType<'a>> {
    "!>".parse_next(input)?;
    ws.parse_next(input)?;
    '['.parse_next(input)?;
    ws.parse_next(input)?;

    let vars: Vec<&'a str> = separated(
        1..,
        |i: &mut &'a str| {
            let v = upper_word(i)?;
            ws.parse_next(i)?;
            opt((':', ws, "$tType", ws)).parse_next(i)?;
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

    let typ = tff_type.parse_next(input)?;

    Ok(TFFType::Quantified {
        variables: vars,
        typ: Box::new(typ),
    })
}

fn tff_mapping_type<'a>(input: &mut &'a str) -> PResult<TFFType<'a>> {
    // (arg1 * arg2 * ...) > result  or  arg > result
    let args = alt((
        // Multiple args: (t1 * t2 * ...)
        delimited(
            ('(', ws),
            separated(
                1..,
                |i: &mut &'a str| {
                    let t = tff_atomic_type(i)?;
                    ws.parse_next(i)?;
                    Ok(t)
                },
                (ws, '*', ws),
            ),
            (ws, ')'),
        ),
        // Single arg
        tff_atomic_type.map(|t| vec![t]),
    ))
    .parse_next(input)?;

    ws.parse_next(input)?;
    '>'.parse_next(input)?;
    ws.parse_next(input)?;

    let result = tff_type.parse_next(input)?;

    Ok(TFFType::Function {
        args,
        result: Box::new(result),
    })
}

fn tff_arrow_type<'a>(input: &mut &'a str) -> PResult<TFFType<'a>> {
    let first = tff_atomic_type.parse_next(input)?;
    ws.parse_next(input)?;

    // Check for > (function type arrow)
    let result = opt(preceded(('>', ws), tff_type)).parse_next(input)?;

    match result {
        Some(ret) => Ok(TFFType::Function {
            args: vec![first],
            result: Box::new(ret),
        }),
        None => Ok(first),
    }
}

fn tff_atomic_type<'a>(input: &mut &'a str) -> PResult<TFFType<'a>> {
    alt((
        // Defined types
        tff_defined_type,
        // Tuple type: [t1, t2, ...]
        tff_tuple_type,
        // Parenthesized
        delimited(('(', ws), tff_type, (ws, ')')).map(|t| TFFType::Parens(Box::new(t))),
        // Type variable
        upper_word.map(TFFType::Variable),
        // Atomic type with optional args
        tff_atomic_type_with_args,
    ))
    .parse_next(input)
}

fn tff_tuple_type<'a>(input: &mut &'a str) -> PResult<TFFType<'a>> {
    delimited(
        ('[', ws),
        separated(
            0..,
            |i: &mut &'a str| {
                let t = tff_type(i)?;
                ws.parse_next(i)?;
                Ok(t)
            },
            (ws, ',', ws),
        ),
        (ws, ']'),
    )
    .map(TFFType::Tuple)
    .parse_next(input)
}

fn tff_defined_type<'a>(input: &mut &'a str) -> PResult<TFFType<'a>> {
    alt((
        "$tType".value(TFFType::Defined(DefinedType::TType)),
        "$int".value(TFFType::Defined(DefinedType::Int)),
        "$rat".value(TFFType::Defined(DefinedType::Rat)),
        "$real".value(TFFType::Defined(DefinedType::Real)),
        "$o".value(TFFType::Defined(DefinedType::O)),
        "$i".value(TFFType::Defined(DefinedType::I)),
    ))
    .parse_next(input)
}

fn tff_atomic_type_with_args<'a>(input: &mut &'a str) -> PResult<TFFType<'a>> {
    let name = atomic_word.parse_next(input)?;
    ws.parse_next(input)?;

    // Check for type arguments
    let args = opt(delimited(
        ('(', ws),
        separated(
            1..,
            |i: &mut &'a str| {
                let t = tff_type(i)?;
                ws.parse_next(i)?;
                Ok(t)
            },
            (ws, ',', ws),
        ),
        (ws, ')'),
    ))
    .parse_next(input)?;

    match args {
        Some(type_args) => Ok(TFFType::Application(
            Box::new(TFFType::Atomic(name)),
            type_args,
        )),
        None => Ok(TFFType::Atomic(name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tff_type() {
        assert!(tff_type.parse_peek("$i").is_ok());
        assert!(tff_type.parse_peek("$o").is_ok());
        assert!(tff_type.parse_peek("$tType").is_ok());
        assert!(tff_type.parse_peek("nat").is_ok());
    }

    #[test]
    fn test_tff_function_type() {
        let result = tff_top_level_type.parse_peek("$i > $o");
        assert!(result.is_ok());
    }

    #[test]
    fn test_tff_tuple_type() {
        let result = tff_type.parse_peek("[ $i, $i ]");
        assert!(result.is_ok(), "Failed to parse [ $i, $i ]: {:?}", result);

        let result2 = tff_type.parse_peek("[ tt, $i ]");
        assert!(result2.is_ok(), "Failed to parse [ tt, $i ]: {:?}", result2);
    }

    #[test]
    fn test_tff_complex_function_type() {
        // ( $i * [ $i, tt, $i ] ) > [ tt, $i ]
        let result = tff_top_level_type.parse_peek("( $i * [ $i, tt, $i ] ) > [ tt, $i ]");
        assert!(
            result.is_ok(),
            "Failed to parse complex function type: {:?}",
            result
        );
    }

    #[test]
    fn test_tff_formula() {
        assert!(tff_formula.parse_peek("p").is_ok());
        assert!(tff_formula.parse_peek("![X: $i]: p(X)").is_ok());
        assert!(
            tff_formula
                .parse_peek("![X: nat, Y: nat]: eq(X, Y)")
                .is_ok()
        );
    }

    #[test]
    fn test_tff_typing() {
        let result = tff_statement.parse_peek("nat: $tType");
        assert!(result.is_ok());

        let result2 = tff_statement.parse_peek("zero: nat");
        assert!(result2.is_ok());
    }

    #[test]
    fn test_tff_nested_function_type() {
        // ( ( $i * [ $i, tt, $i ] ) > [ tt, $i ] )
        let result = tff_type.parse_peek("( ( $i * [ $i, tt, $i ] ) > [ tt, $i ] )");
        assert!(
            result.is_ok(),
            "Failed to parse nested function type: {:?}",
            result
        );
    }
}
