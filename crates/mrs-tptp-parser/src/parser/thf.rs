//! THF (Typed Higher-order Form) parser.

use winnow::combinator::{alt, delimited, opt, preceded, repeat, separated};
use winnow::error::StrContext;
use winnow::prelude::*;
use winnow::token::one_of;

use crate::ast::thf::{
    LogicProperty, LogicSpecification, LogicValue, NonClassicalOperator, THFAtomicFormula,
    THFBinaryConnective, THFDefinedType, THFFormula, THFQuantifier, THFStatement, THFType,
    THFTypedSymbol, THFTyping, THFVariable,
};
use crate::lexer::{
    PResult, atomic_word, check_cancel, defined_word, distinct_object, number, single_quoted,
    system_word, upper_word, ws,
};

/// Parse a THF statement
pub fn thf_statement<'a>(input: &mut &'a str) -> PResult<THFStatement<'a>> {
    alt((
        // Logic specification: $modal == [ ... ]
        thf_logic_specification.map(THFStatement::Logic),
        // Subtype: type << type
        thf_subtype.map(|(l, r)| THFStatement::Subtype(l, r)),
        // Type declaration: symbol : type
        thf_typing.map(THFStatement::Typing),
        // Sequent
        thf_sequent.map(|(l, r)| THFStatement::Sequent(l, r)),
        // Logical formula
        thf_formula.map(THFStatement::Logical),
    ))
    .context(StrContext::Label("thf_statement"))
    .parse_next(input)
}

fn thf_typing<'a>(input: &mut &'a str) -> PResult<THFTyping<'a>> {
    let symbol = alt((
        system_word.map(THFTypedSymbol::System),
        defined_word.map(THFTypedSymbol::Defined),
        atomic_word.map(THFTypedSymbol::Atom),
    ))
    .parse_next(input)?;

    ws.parse_next(input)?;
    ':'.parse_next(input)?;
    ws.parse_next(input)?;

    let typ = thf_top_level_type.parse_next(input)?;

    Ok(THFTyping { symbol, typ })
}

fn thf_subtype<'a>(input: &mut &'a str) -> PResult<(THFType<'a>, THFType<'a>)> {
    let left = thf_atomic_type.parse_next(input)?;
    ws.parse_next(input)?;
    "<<".parse_next(input)?;
    ws.parse_next(input)?;
    let right = thf_atomic_type.parse_next(input)?;
    Ok((left, right))
}

fn thf_sequent<'a>(input: &mut &'a str) -> PResult<(Vec<THFFormula<'a>>, Vec<THFFormula<'a>>)> {
    let left = thf_formula_tuple.parse_next(input)?;
    ws.parse_next(input)?;
    "-->".parse_next(input)?;
    ws.parse_next(input)?;
    let right = thf_formula_tuple.parse_next(input)?;
    Ok((left, right))
}

/// Parse a logic specification: $modal == [ ... ] or $epistemic_modal == [ ... ] etc.
/// May be wrapped in outer parentheses: ( $modal == [...] )
fn thf_logic_specification<'a>(input: &mut &'a str) -> PResult<LogicSpecification<'a>> {
    // Check for optional outer parentheses
    let has_parens = opt('(').parse_next(input)?.is_some();
    if has_parens {
        ws.parse_next(input)?;
    }

    // Parse the logic family name (e.g., $modal, $alethic_modal, $epistemic_modal)
    let logic_family = alt((
        "$modal",
        "$alethic_modal",
        "$epistemic_modal",
        "$deontic_modal",
        // Capture any other $name
        |i: &mut &'a str| {
            let start = *i;
            '$'.parse_next(i)?;
            winnow::token::take_while(1.., |c: char| c.is_ascii_alphanumeric() || c == '_')
                .parse_next(i)?;
            let len = i.as_ptr() as usize - start.as_ptr() as usize;
            Ok(&start[..len])
        },
    ))
    .parse_next(input)?;

    ws.parse_next(input)?;
    "==".parse_next(input)?;
    ws.parse_next(input)?;

    // Parse the property list
    let properties = logic_property_list.parse_next(input)?;

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
fn logic_property_list<'a>(input: &mut &'a str) -> PResult<Vec<LogicProperty<'a>>> {
    delimited(
        ('[', ws),
        separated(0.., logic_property, (ws, ',', ws)),
        (ws, ']'),
    )
    .parse_next(input)
}

/// Parse a single logic property: key == value or key == [ ... ]
fn logic_property<'a>(input: &mut &'a str) -> PResult<LogicProperty<'a>> {
    // Parse the key (e.g., $constants, $quantification, etc.)
    let key = logic_key.parse_next(input)?;
    ws.parse_next(input)?;
    "==".parse_next(input)?;
    ws.parse_next(input)?;

    // Parse the value (could be an atom, a list, or a nested property list)
    let value = logic_value.parse_next(input)?;
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
fn logic_key<'a>(input: &mut &'a str) -> PResult<&'a str> {
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
                crate::ast::common::AtomicWord::Lower(s) => Ok(s),
                crate::ast::common::AtomicWord::SingleQuoted(s) => Ok(s),
            }
        },
    ))
    .parse_next(input)
}

/// Parse a logic value
fn logic_value<'a>(input: &mut &'a str) -> PResult<LogicValue<'a>> {
    alt((
        // List: [ ... ]
        logic_value_list.map(LogicValue::List),
        // Property assignment: name == value
        logic_value_property,
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
                crate::ast::common::AtomicWord::Lower(s) => Ok(LogicValue::Atom(s)),
                crate::ast::common::AtomicWord::SingleQuoted(s) => Ok(LogicValue::String(s)),
            }
        },
    ))
    .parse_next(input)
}

/// Parse a logic value list: [ value1, value2, ... ]
fn logic_value_list<'a>(input: &mut &'a str) -> PResult<Vec<LogicValue<'a>>> {
    delimited(
        ('[', ws),
        separated(
            0..,
            |i: &mut &'a str| {
                let v = logic_value(i)?;
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
fn logic_value_property<'a>(input: &mut &'a str) -> PResult<LogicValue<'a>> {
    let name = logic_key.parse_next(input)?;
    ws.parse_next(input)?;
    "==".parse_next(input)?;
    ws.parse_next(input)?;
    let value = logic_value.parse_next(input)?;

    Ok(LogicValue::Property {
        name,
        value: Box::new(value),
    })
}

fn thf_formula_tuple<'a>(input: &mut &'a str) -> PResult<Vec<THFFormula<'a>>> {
    delimited(
        ('[', ws),
        separated(
            0..,
            |i: &mut &'a str| {
                let f = thf_formula(i)?;
                ws.parse_next(i)?;
                Ok(f)
            },
            (ws, ',', ws),
        ),
        (ws, ']'),
    )
    .parse_next(input)
}

/// Parse a THF formula
pub fn thf_formula<'a>(input: &mut &'a str) -> PResult<THFFormula<'a>> {
    check_cancel(input)?;
    thf_binary_formula.parse_next(input)
}

/// Parse binary formula with proper precedence
fn thf_binary_formula<'a>(input: &mut &'a str) -> PResult<THFFormula<'a>> {
    let left = thf_or_formula.parse_next(input)?;
    ws.parse_next(input)?;

    // Non-associative connectives and equality
    let eq_result = opt(alt((
        // Inequality: !=
        ("!=", ws).map(|_| false),
        // Equality: = (but not => or ==)
        preceded(('=', winnow::combinator::not(one_of(['>', '=']))), ws).map(|_| true),
    )))
    .parse_next(input)?;

    if let Some(is_eq) = eq_result {
        let right = thf_or_formula.parse_next(input)?;
        return if is_eq {
            Ok(THFFormula::Equality(Box::new(left), Box::new(right)))
        } else {
            Ok(THFFormula::Inequality(Box::new(left), Box::new(right)))
        };
    }

    let result = opt(alt((
        ("<=>", ws, thf_or_formula).map(|(_, _, r)| (THFBinaryConnective::Iff, r)),
        ("=>", ws, thf_or_formula).map(|(_, _, r)| (THFBinaryConnective::Impl, r)),
        // Be careful with <= vs <=> - we already parsed <=> above
        preceded(("<=", winnow::combinator::not('>')), (ws, thf_or_formula))
            .map(|(_, r)| (THFBinaryConnective::RevImpl, r)),
        ("<~>", ws, thf_or_formula).map(|(_, _, r)| (THFBinaryConnective::Xor, r)),
        ("~|", ws, thf_or_formula).map(|(_, _, r)| (THFBinaryConnective::Nor, r)),
        ("~&", ws, thf_or_formula).map(|(_, _, r)| (THFBinaryConnective::Nand, r)),
        (":=", ws, thf_or_formula).map(|(_, _, r)| (THFBinaryConnective::Assign, r)),
        ("==", ws, thf_or_formula).map(|(_, _, r)| (THFBinaryConnective::MetaIdentity, r)),
    )))
    .parse_next(input)?;

    match result {
        Some((conn, right)) => Ok(THFFormula::Binary {
            left: Box::new(left),
            connective: conn,
            right: Box::new(right),
        }),
        None => Ok(left),
    }
}

fn thf_or_formula<'a>(input: &mut &'a str) -> PResult<THFFormula<'a>> {
    let mut result = thf_and_formula.parse_next(input)?;
    loop {
        let checkpoint = *input;
        ws.parse_next(input)?;
        if input.as_bytes().first() != Some(&b'|') {
            *input = checkpoint;
            break;
        }
        *input = &input[1..];
        ws.parse_next(input)?;
        let right = thf_and_formula.parse_next(input)?;
        result = THFFormula::Binary {
            left: Box::new(result),
            connective: THFBinaryConnective::Or,
            right: Box::new(right),
        };
    }
    Ok(result)
}

fn thf_and_formula<'a>(input: &mut &'a str) -> PResult<THFFormula<'a>> {
    let mut result = thf_apply_formula.parse_next(input)?;
    loop {
        let checkpoint = *input;
        ws.parse_next(input)?;
        if input.as_bytes().first() != Some(&b'&') {
            *input = checkpoint;
            break;
        }
        *input = &input[1..];
        ws.parse_next(input)?;
        let right = thf_apply_formula.parse_next(input)?;
        result = THFFormula::Binary {
            left: Box::new(result),
            connective: THFBinaryConnective::And,
            right: Box::new(right),
        };
    }
    Ok(result)
}

/// Parse application formula: f @ g @ h (left-associative)
fn thf_apply_formula<'a>(input: &mut &'a str) -> PResult<THFFormula<'a>> {
    let mut result = thf_unary_formula.parse_next(input)?;
    loop {
        let checkpoint = *input;
        ws.parse_next(input)?;
        // Match '@' but NOT '@@' (TH1 bare operators) or '@=' or '@+' or '@-'
        match input.as_bytes().first() {
            Some(&b'@') => match input.as_bytes().get(1) {
                Some(&b'@') | Some(&b'=') | Some(&b'+') | Some(&b'-') => {
                    *input = checkpoint;
                    break;
                }
                _ => {}
            },
            _ => {
                *input = checkpoint;
                break;
            }
        }
        *input = &input[1..];
        ws.parse_next(input)?;
        let right = thf_unary_formula.parse_next(input)?;
        result = THFFormula::Application(Box::new(result), Box::new(right));
    }
    Ok(result)
}

fn thf_unary_formula<'a>(input: &mut &'a str) -> PResult<THFFormula<'a>> {
    alt((
        // Negation
        preceded(('~', ws), thf_unary_formula).map(|f| THFFormula::Negation(Box::new(f))),
        // Unit formula
        thf_unit_formula,
    ))
    .parse_next(input)
}

fn thf_unit_formula<'a>(input: &mut &'a str) -> PResult<THFFormula<'a>> {
    alt((
        alt((
            // Lambda: ^[vars] : body
            thf_lambda,
            // Quantified: Q [vars] : F
            thf_quantified_formula,
            // Non-classical long form (NHF): {#op}(formula) or {$op}(formula)
            thf_nonclassical,
            // Non-classical short form: [.](F), <.>(F), [#name](F), <#name>(F)
            thf_short_box,
            thf_short_diamond,
            // Non-classical alternative short form: /.\, \./
            thf_alt_short_box,
            thf_alt_short_diamond,
            // Conditional: $ite(cond, then, else)
            thf_conditional,
            // Let: $let(defs, body)
            thf_let,
        )),
        alt((
            // TH1 bare quantifier/operator terms: !!, ??, @@+, @@-, @=
            thf_bare_operator_term,
            // Connective as term: (&), (~), (~&), etc.
            thf_connective_term,
            // Parenthesized formula or type (TH1: type can be an argument)
            thf_parenthesized,
            // Tuple: [f1, f2, ...]
            thf_tuple,
            // Distinct object
            distinct_object.map(THFFormula::DistinctObject),
            // Number
            number.map(THFFormula::Number),
            // Variable (must be before atomic to avoid confusion)
            thf_variable_formula,
            // Atomic formula with optional type annotation
            thf_atomic_with_type,
        )),
    ))
    .parse_next(input)
}

/// Parse TH1 bare operator terms that appear without parentheses: !!, ??, @@+, @@-, @=
/// These are used in TH1 polymorphic formulas like: !! @ type @ predicate
fn thf_bare_operator_term<'a>(input: &mut &'a str) -> PResult<THFFormula<'a>> {
    alt((
        "!!".value(THFFormula::QuantifierTerm(THFQuantifier::Forall)),
        "??".value(THFFormula::QuantifierTerm(THFQuantifier::Exists)),
        "@@+".value(THFFormula::QuantifierTerm(THFQuantifier::Choice)),
        "@@-".value(THFFormula::QuantifierTerm(THFQuantifier::Description)),
        "@=".value(THFFormula::EqualityTerm),
    ))
    .parse_next(input)
}

/// Parse a parenthesized formula or type (TH1: types can appear as formula arguments)
fn thf_parenthesized<'a>(input: &mut &'a str) -> PResult<THFFormula<'a>> {
    '('.parse_next(input)?;
    ws.parse_next(input)?;

    // Save position for backtracking
    let saved = *input;

    // Try parsing as a formula first
    if let Ok(f) = thf_formula.parse_next(input) {
        ws.parse_next(input)?;
        // Check if we're at the closing paren
        if input.starts_with(')') {
            ')'.parse_next(input)?;
            return Ok(THFFormula::Parens(Box::new(f)));
        }
    }

    // Formula didn't work (or didn't consume everything), try as a type
    *input = saved;
    let typ = thf_top_level_type.parse_next(input)?;
    ws.parse_next(input)?;
    ')'.parse_next(input)?;

    Ok(THFFormula::TypeAsFormula(typ))
}

/// Parse a connective used as a term: (&), (~), (~&), etc.
/// Also handles TH1 quantifiers/operators as terms: (!!), (??), (@@+), (@@-), (@=)
fn thf_connective_term<'a>(input: &mut &'a str) -> PResult<THFFormula<'a>> {
    delimited(
        ('(', ws),
        alt((
            alt((
                // TH1: Quantifiers as terms
                "!!".value(THFFormula::QuantifierTerm(THFQuantifier::Forall)),
                "??".value(THFFormula::QuantifierTerm(THFQuantifier::Exists)),
                "@@+".value(THFFormula::QuantifierTerm(THFQuantifier::Choice)),
                "@@-".value(THFFormula::QuantifierTerm(THFQuantifier::Description)),
                "@=".value(THFFormula::EqualityTerm),
                // Standard connectives
                "<=>".value(THFFormula::ConnectiveTerm(THFBinaryConnective::Iff)),
                "=>".value(THFFormula::ConnectiveTerm(THFBinaryConnective::Impl)),
                "<=".value(THFFormula::ConnectiveTerm(THFBinaryConnective::RevImpl)),
                "<~>".value(THFFormula::ConnectiveTerm(THFBinaryConnective::Xor)),
            )),
            alt((
                "~|".value(THFFormula::ConnectiveTerm(THFBinaryConnective::Nor)),
                "~&".value(THFFormula::ConnectiveTerm(THFBinaryConnective::Nand)),
                "~".value(THFFormula::UnaryConnectiveTerm),
                "|".value(THFFormula::ConnectiveTerm(THFBinaryConnective::Or)),
                "&".value(THFFormula::ConnectiveTerm(THFBinaryConnective::And)),
            )),
        )),
        (ws, ')'),
    )
    .parse_next(input)
}

fn thf_lambda<'a>(input: &mut &'a str) -> PResult<THFFormula<'a>> {
    '^'.parse_next(input)?;
    ws.parse_next(input)?;
    '['.parse_next(input)?;
    ws.parse_next(input)?;

    let vars: Vec<THFVariable<'a>> =
        separated(1.., thf_typed_variable, (ws, ',', ws)).parse_next(input)?;

    ws.parse_next(input)?;
    ']'.parse_next(input)?;
    ws.parse_next(input)?;
    ':'.parse_next(input)?;
    ws.parse_next(input)?;

    let body = thf_unary_formula.parse_next(input)?;

    Ok(THFFormula::Lambda {
        variables: vars,
        body: Box::new(body),
    })
}

fn thf_quantified_formula<'a>(input: &mut &'a str) -> PResult<THFFormula<'a>> {
    let q = alt((
        "!>".value(THFQuantifier::ForallType),
        "?*".value(THFQuantifier::ExistsType),
        "@+".value(THFQuantifier::Choice),
        "@-".value(THFQuantifier::Description),
        '!'.value(THFQuantifier::Forall),
        '?'.value(THFQuantifier::Exists),
    ))
    .parse_next(input)?;

    ws.parse_next(input)?;
    '['.parse_next(input)?;
    ws.parse_next(input)?;

    let vars: Vec<THFVariable<'a>> =
        separated(1.., thf_typed_variable, (ws, ',', ws)).parse_next(input)?;

    ws.parse_next(input)?;
    ']'.parse_next(input)?;
    ws.parse_next(input)?;
    ':'.parse_next(input)?;
    ws.parse_next(input)?;

    let formula = thf_unary_formula.parse_next(input)?;

    Ok(THFFormula::Quantified {
        quantifier: q,
        variables: vars,
        formula: Box::new(formula),
    })
}

fn thf_typed_variable<'a>(input: &mut &'a str) -> PResult<THFVariable<'a>> {
    let name = upper_word.parse_next(input)?;
    ws.parse_next(input)?;

    // Use thf_top_level_type to support quantified types and type applications
    let typ = opt(preceded((':', ws), thf_top_level_type)).parse_next(input)?;

    Ok(THFVariable { name, typ })
}

fn thf_nonclassical<'a>(input: &mut &'a str) -> PResult<THFFormula<'a>> {
    '{'.parse_next(input)?;

    // Can be either # or $ prefix
    let _prefix = alt(('#', '$')).parse_next(input)?;

    // Parse operator name
    let op_name: &str =
        winnow::token::take_while(1.., |c: char| c.is_ascii_alphanumeric() || c == '_')
            .parse_next(input)?;

    // Optional index: {$necessary(agent)}, {$knows(#agent)}, or {#op:index}
    let index = opt(alt((
        // Parenthesized index with # prefix: {$knows(#agent)}
        delimited(
            ('(', ws, '#'),
            winnow::token::take_while(1.., |c: char| c.is_ascii_alphanumeric() || c == '_'),
            (ws, ')'),
        ),
        // Parenthesized index without # prefix: {$necessary(agent)}
        delimited(
            ('(', ws),
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

    // Optional formula argument: @ (formula) or (formula)
    let formula = opt(alt((
        // THF application style: @ formula
        preceded((ws, '@', ws), thf_unary_formula),
        // Direct parenthesized: (formula)
        delimited(('(', ws), thf_formula, (ws, ')')),
    )))
    .parse_next(input)?;

    Ok(THFFormula::NonClassical {
        operator,
        formula: formula.map(Box::new),
    })
}

/// Parse short-form box operator: [.], [..], or [#name]
fn thf_short_box<'a>(input: &mut &'a str) -> PResult<THFFormula<'a>> {
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

    // Formula argument can be parenthesized or @ applied
    let formula = alt((
        delimited(('(', ws), thf_formula, (ws, ')')),
        preceded(('@', ws), thf_unary_formula),
        thf_unary_formula,
    ))
    .parse_next(input)?;

    Ok(THFFormula::NonClassical {
        operator,
        formula: Some(Box::new(formula)),
    })
}

/// Parse short-form diamond operator: <.>, <..>, or <#name>
fn thf_short_diamond<'a>(input: &mut &'a str) -> PResult<THFFormula<'a>> {
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

    // Formula argument can be parenthesized or @ applied
    let formula = alt((
        delimited(('(', ws), thf_formula, (ws, ')')),
        preceded(('@', ws), thf_unary_formula),
        thf_unary_formula,
    ))
    .parse_next(input)?;

    Ok(THFFormula::NonClassical {
        operator,
        formula: Some(Box::new(formula)),
    })
}

/// Parse alternative short-form box operator: /.\, /..\, or /#name\
fn thf_alt_short_box<'a>(input: &mut &'a str) -> PResult<THFFormula<'a>> {
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

    // Formula argument can be parenthesized or @ applied
    let formula = alt((
        delimited(('(', ws), thf_formula, (ws, ')')),
        preceded(('@', ws), thf_unary_formula),
        thf_unary_formula,
    ))
    .parse_next(input)?;

    Ok(THFFormula::NonClassical {
        operator,
        formula: Some(Box::new(formula)),
    })
}

/// Parse alternative short-form diamond operator: \./. \..\, or \#name/
fn thf_alt_short_diamond<'a>(input: &mut &'a str) -> PResult<THFFormula<'a>> {
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

    // Formula argument can be parenthesized or @ applied
    let formula = alt((
        delimited(('(', ws), thf_formula, (ws, ')')),
        preceded(('@', ws), thf_unary_formula),
        thf_unary_formula,
    ))
    .parse_next(input)?;

    Ok(THFFormula::NonClassical {
        operator,
        formula: Some(Box::new(formula)),
    })
}

fn thf_conditional<'a>(input: &mut &'a str) -> PResult<THFFormula<'a>> {
    "$ite".parse_next(input)?;
    ws.parse_next(input)?;
    '('.parse_next(input)?;
    ws.parse_next(input)?;

    let condition = thf_formula.parse_next(input)?;
    ws.parse_next(input)?;
    ','.parse_next(input)?;
    ws.parse_next(input)?;

    let then_branch = thf_formula.parse_next(input)?;
    ws.parse_next(input)?;
    ','.parse_next(input)?;
    ws.parse_next(input)?;

    let else_branch = thf_formula.parse_next(input)?;
    ws.parse_next(input)?;
    ')'.parse_next(input)?;

    Ok(THFFormula::Conditional {
        condition: Box::new(condition),
        then_branch: Box::new(then_branch),
        else_branch: Box::new(else_branch),
    })
}

fn thf_let<'a>(input: &mut &'a str) -> PResult<THFFormula<'a>> {
    "$let".parse_next(input)?;
    ws.parse_next(input)?;
    '('.parse_next(input)?;
    ws.parse_next(input)?;

    // Simplified: just parse the body for now
    // TODO: implement proper let definition parsing
    let defs = vec![];

    let body = thf_formula.parse_next(input)?;
    ws.parse_next(input)?;
    ')'.parse_next(input)?;

    Ok(THFFormula::Let {
        definitions: defs,
        body: Box::new(body),
    })
}

fn thf_tuple<'a>(input: &mut &'a str) -> PResult<THFFormula<'a>> {
    delimited(
        ('[', ws),
        separated(
            0..,
            |i: &mut &'a str| {
                let f = thf_formula(i)?;
                ws.parse_next(i)?;
                Ok(f)
            },
            (ws, ',', ws),
        ),
        (ws, ']'),
    )
    .map(THFFormula::Tuple)
    .parse_next(input)
}

fn thf_variable_formula<'a>(input: &mut &'a str) -> PResult<THFFormula<'a>> {
    let name = upper_word.parse_next(input)?;
    Ok(THFFormula::Variable(name))
}

fn thf_atomic_with_type<'a>(input: &mut &'a str) -> PResult<THFFormula<'a>> {
    let atomic = thf_atomic_formula.parse_next(input)?;
    ws.parse_next(input)?;

    // Check for type annotation - use thf_top_level_type to support quantified types (TH1)
    let typ = opt(preceded((':', ws), thf_top_level_type)).parse_next(input)?;

    let formula = THFFormula::Atomic(atomic);

    match typ {
        Some(t) => Ok(THFFormula::Typed(Box::new(formula), t)),
        None => Ok(formula),
    }
}

/// Parse a THF atomic formula
pub fn thf_atomic_formula<'a>(input: &mut &'a str) -> PResult<THFAtomicFormula<'a>> {
    alt((
        "$true".value(THFAtomicFormula::True),
        "$false".value(THFAtomicFormula::False),
        thf_system_atomic,
        thf_defined_atomic,
        thf_plain_atomic,
    ))
    .parse_next(input)
}

fn thf_plain_atomic<'a>(input: &mut &'a str) -> PResult<THFAtomicFormula<'a>> {
    let name = atomic_word.parse_next(input)?;
    ws.parse_next(input)?;

    let args = opt(delimited(
        ('(', ws),
        separated(
            1..,
            |i: &mut &'a str| {
                let f = thf_formula(i)?;
                ws.parse_next(i)?;
                Ok(f)
            },
            (ws, ',', ws),
        ),
        (ws, ')'),
    ))
    .parse_next(input)?;

    Ok(THFAtomicFormula::Plain(name, args.unwrap_or_default()))
}

fn thf_defined_atomic<'a>(input: &mut &'a str) -> PResult<THFAtomicFormula<'a>> {
    let name = defined_word.parse_next(input)?;
    ws.parse_next(input)?;

    let args = opt(delimited(
        ('(', ws),
        separated(
            1..,
            |i: &mut &'a str| {
                let f = thf_formula(i)?;
                ws.parse_next(i)?;
                Ok(f)
            },
            (ws, ',', ws),
        ),
        (ws, ')'),
    ))
    .parse_next(input)?;

    Ok(THFAtomicFormula::Defined(name, args.unwrap_or_default()))
}

fn thf_system_atomic<'a>(input: &mut &'a str) -> PResult<THFAtomicFormula<'a>> {
    let name = system_word.parse_next(input)?;
    ws.parse_next(input)?;

    let args = opt(delimited(
        ('(', ws),
        separated(
            1..,
            |i: &mut &'a str| {
                let f = thf_formula(i)?;
                ws.parse_next(i)?;
                Ok(f)
            },
            (ws, ',', ws),
        ),
        (ws, ')'),
    ))
    .parse_next(input)?;

    Ok(THFAtomicFormula::System(name, args.unwrap_or_default()))
}

/// Parse a THF type
pub fn thf_type<'a>(input: &mut &'a str) -> PResult<THFType<'a>> {
    thf_binary_type.parse_next(input)
}

/// Parse top-level type (with optional quantification or @ application)
pub fn thf_top_level_type<'a>(input: &mut &'a str) -> PResult<THFType<'a>> {
    alt((
        thf_quantified_type,
        thf_mapping_type,
        thf_type_with_top_app, // Support @ application at top level (TH1)
    ))
    .parse_next(input)
}

/// Parse a type that may include @ type application at top level (TH1)
fn thf_type_with_top_app<'a>(input: &mut &'a str) -> PResult<THFType<'a>> {
    let first = thf_binary_type.parse_next(input)?;
    ws.parse_next(input)?;

    // Check for @ type application
    let rest: Vec<THFType<'a>> =
        repeat(0.., preceded((ws, '@', ws), thf_type_app_unit)).parse_next(input)?;

    if rest.is_empty() {
        Ok(first)
    } else {
        // Type application: map @ A @ B
        Ok(rest
            .into_iter()
            .fold(first, |acc, t| THFType::Application(Box::new(acc), vec![t])))
    }
}

fn thf_quantified_type<'a>(input: &mut &'a str) -> PResult<THFType<'a>> {
    let q = alt((
        "!>".value(THFQuantifier::ForallType),
        "?*".value(THFQuantifier::ExistsType),
    ))
    .parse_next(input)?;

    ws.parse_next(input)?;
    '['.parse_next(input)?;
    ws.parse_next(input)?;

    // Parse variables - can be either:
    // 1. Simple type variables: X, Y (optionally with : $tType)
    // 2. Dependent type variables: X: SomeType
    let vars: Vec<THFVariable<'a>> = separated(
        1..,
        |i: &mut &'a str| {
            let v = upper_word(i)?;
            ws.parse_next(i)?;
            // Optional type annotation
            let typ = opt(preceded((':', ws), thf_top_level_type)).parse_next(i)?;
            ws.parse_next(i)?;
            Ok(THFVariable { name: v, typ })
        },
        (ws, ',', ws),
    )
    .parse_next(input)?;

    ws.parse_next(input)?;
    ']'.parse_next(input)?;
    ws.parse_next(input)?;
    ':'.parse_next(input)?;
    ws.parse_next(input)?;

    let typ = thf_type.parse_next(input)?;

    // Check if all variables have either no type or $tType - if so, use Quantified
    // Otherwise use DependentQuantified
    let all_simple = vars.iter().all(|v| {
        v.typ.is_none() || matches!(&v.typ, Some(THFType::Defined(THFDefinedType::TType)))
    });

    if all_simple {
        // Extract just the names for the simpler Quantified variant
        let var_names: Vec<&'a str> = vars.into_iter().map(|v| v.name).collect();
        Ok(THFType::Quantified {
            quantifier: q,
            variables: var_names,
            typ: Box::new(typ),
        })
    } else {
        Ok(THFType::DependentQuantified {
            quantifier: q,
            variables: vars,
            typ: Box::new(typ),
        })
    }
}

fn thf_mapping_type<'a>(input: &mut &'a str) -> PResult<THFType<'a>> {
    // Parse (t1 * t2 * ...) > result or t1 > result
    let args = alt((
        // Multiple args in parens
        delimited(
            ('(', ws),
            separated(
                1..,
                |i: &mut &'a str| {
                    let t = thf_unitary_type(i)?;
                    ws.parse_next(i)?;
                    Ok(t)
                },
                (ws, '*', ws),
            ),
            (ws, ')'),
        ),
        // Single arg
        thf_unitary_type.map(|t| vec![t]),
    ))
    .parse_next(input)?;

    ws.parse_next(input)?;
    '>'.parse_next(input)?;
    ws.parse_next(input)?;

    let result = thf_type.parse_next(input)?;

    Ok(THFType::Mapping(args, Box::new(result)))
}

fn thf_binary_type<'a>(input: &mut &'a str) -> PResult<THFType<'a>> {
    let first = thf_unitary_type.parse_next(input)?;
    ws.parse_next(input)?;

    // Check for binary type operators
    let result = opt(alt((
        preceded(('>', ws), thf_type).map(|t| (">", t)),
        preceded(('*', ws), thf_binary_type).map(|t| ("*", t)),
        preceded(('+', ws), thf_binary_type).map(|t| ("+", t)),
    )))
    .parse_next(input)?;

    match result {
        Some((">", t)) => Ok(THFType::Arrow(Box::new(first), Box::new(t))),
        Some(("*", t)) => Ok(THFType::Product(Box::new(first), Box::new(t))),
        Some(("+", t)) => Ok(THFType::Sum(Box::new(first), Box::new(t))),
        _ => Ok(first),
    }
}

fn thf_unitary_type<'a>(input: &mut &'a str) -> PResult<THFType<'a>> {
    alt((
        thf_atomic_type,
        // Parenthesized type (may contain type application with @)
        delimited(('(', ws), thf_type_with_app, (ws, ')')).map(|t| THFType::Parens(Box::new(t))),
    ))
    .parse_next(input)
}

/// Parse a type that may include @ type application (TH1)
fn thf_type_with_app<'a>(input: &mut &'a str) -> PResult<THFType<'a>> {
    let first = thf_type_app_unit.parse_next(input)?;
    ws.parse_next(input)?;

    // Check for @ type application
    let rest: Vec<THFType<'a>> =
        repeat(0.., preceded((ws, '@', ws), thf_type_app_unit)).parse_next(input)?;

    if rest.is_empty() {
        // No application, check for binary type operators
        let result = opt(alt((
            preceded(('>', ws), thf_type_with_app).map(|t| (">", t)),
            preceded(('*', ws), thf_type_with_app).map(|t| ("*", t)),
            preceded(('+', ws), thf_type_with_app).map(|t| ("+", t)),
        )))
        .parse_next(input)?;

        match result {
            Some((">", t)) => Ok(THFType::Arrow(Box::new(first), Box::new(t))),
            Some(("*", t)) => Ok(THFType::Product(Box::new(first), Box::new(t))),
            Some(("+", t)) => Ok(THFType::Sum(Box::new(first), Box::new(t))),
            _ => Ok(first),
        }
    } else {
        // Type application: map @ A @ B
        Ok(rest
            .into_iter()
            .fold(first, |acc, t| THFType::Application(Box::new(acc), vec![t])))
    }
}

/// Parse a unit for type application (atom, variable, tuple, or parenthesized)
fn thf_type_app_unit<'a>(input: &mut &'a str) -> PResult<THFType<'a>> {
    alt((
        thf_defined_type,
        thf_tuple_type, // Add tuple type support
        upper_word.map(THFType::Variable),
        atomic_word.map(THFType::Atomic),
        delimited(('(', ws), thf_type_with_app, (ws, ')')).map(|t| THFType::Parens(Box::new(t))),
    ))
    .parse_next(input)
}

fn thf_atomic_type<'a>(input: &mut &'a str) -> PResult<THFType<'a>> {
    alt((
        thf_defined_type,
        // Tuple type: [ $i, $o, ... ]
        thf_tuple_type,
        // Type variable
        upper_word.map(THFType::Variable),
        // Atomic with optional args
        thf_atomic_type_with_args,
    ))
    .parse_next(input)
}

fn thf_tuple_type<'a>(input: &mut &'a str) -> PResult<THFType<'a>> {
    delimited(
        ('[', ws),
        separated(
            0..,
            |i: &mut &'a str| {
                let t = thf_type(i)?;
                ws.parse_next(i)?;
                Ok(t)
            },
            (ws, ',', ws),
        ),
        (ws, ']'),
    )
    .map(THFType::Tuple)
    .parse_next(input)
}

fn thf_defined_type<'a>(input: &mut &'a str) -> PResult<THFType<'a>> {
    alt((
        "$tType".value(THFType::Defined(THFDefinedType::TType)),
        // Parse $int, $rat, $real before $o, $i to avoid prefix matching
        "$int".value(THFType::Defined(THFDefinedType::Int)),
        "$rat".value(THFType::Defined(THFDefinedType::Rat)),
        "$real".value(THFType::Defined(THFDefinedType::Real)),
        "$o".value(THFType::Defined(THFDefinedType::O)),
        "$i".value(THFType::Defined(THFDefinedType::I)),
    ))
    .parse_next(input)
}

fn thf_atomic_type_with_args<'a>(input: &mut &'a str) -> PResult<THFType<'a>> {
    let name = atomic_word.parse_next(input)?;
    ws.parse_next(input)?;

    let args = opt(delimited(
        ('(', ws),
        separated(
            1..,
            |i: &mut &'a str| {
                let t = thf_type(i)?;
                ws.parse_next(i)?;
                Ok(t)
            },
            (ws, ',', ws),
        ),
        (ws, ')'),
    ))
    .parse_next(input)?;

    match args {
        Some(type_args) => Ok(THFType::Application(
            Box::new(THFType::Atomic(name)),
            type_args,
        )),
        None => Ok(THFType::Atomic(name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thf_type() {
        assert!(thf_type.parse_peek("$i").is_ok());
        assert!(thf_type.parse_peek("$o").is_ok());
        assert!(thf_type.parse_peek("$i > $o").is_ok());
        assert!(thf_type.parse_peek("$i > $i > $o").is_ok());
    }

    #[test]
    fn test_thf_formula() {
        assert!(thf_formula.parse_peek("p").is_ok());
        assert!(thf_formula.parse_peek("~p").is_ok());
        assert!(thf_formula.parse_peek("p & q").is_ok());
        assert!(thf_formula.parse_peek("p | q").is_ok());
        assert!(thf_formula.parse_peek("p => q").is_ok());
    }

    #[test]
    fn test_thf_application() {
        assert!(thf_formula.parse_peek("f @ X").is_ok());
        assert!(thf_formula.parse_peek("f @ X @ Y").is_ok());
    }

    #[test]
    fn test_thf_lambda() {
        let result = thf_formula.parse_peek("^[X: $i]: p @ X");
        assert!(result.is_ok());
    }

    #[test]
    fn test_thf_quantified() {
        assert!(thf_formula.parse_peek("![X: $i]: p @ X").is_ok());
        assert!(thf_formula.parse_peek("?[X: $i]: p @ X").is_ok());
    }

    #[test]
    fn test_thf_typing() {
        let result = thf_statement.parse_peek("p: $i > $o");
        assert!(result.is_ok());
    }
}
