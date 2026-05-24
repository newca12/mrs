//! Top-level TPTP parser: [`parse_tptp`] and [`TPTPIterator`].
//!
//! Most users only need the re-exports from the crate root — you rarely need
//! to import from this module directly.

use winnow::combinator::{alt, cut_err, delimited, opt, preceded, separated};
use winnow::error::StrContext;
use winnow::prelude::*;

use crate::ast::{
    AnnotatedFormula, CNFAnnotated, FOFAnnotated, Include, TCFAnnotated, TFFAnnotated,
    THFAnnotated, TPIAnnotated, TPTPProblem,
};
use crate::error::ParseError;
use crate::lexer::{PResult, check_cancel, name, single_quoted, ws};
use crate::parser::cnf::cnf_statement;
use crate::parser::common::{annotations, formula_role};
use crate::parser::fof::fof_statement;
use crate::parser::tcf::tcf_statement;
use crate::parser::tff::tff_statement;
use crate::parser::thf::thf_statement;

/// Parse a complete TPTP problem file.
///
/// # Example
///
/// ```
/// use mrs_tptp::parse_tptp;
///
/// let input = "fof(ax1, axiom, ![X]: (human(X) => mortal(X))).";
/// let result = parse_tptp(input);
/// assert!(result.is_ok());
/// ```
pub fn parse_tptp(input: &str) -> Result<TPTPProblem<'_>, ParseError> {
    let original = input;
    let mut remaining = input;

    match tptp_file(&mut remaining) {
        Ok(problem) => Ok(problem),
        Err(e) => {
            let offset = original.len().saturating_sub(remaining.len());
            Err(ParseError::new(format!("{e:?}"), offset))
        }
    }
}

/// Parse a TPTP file
fn tptp_file<'a>(input: &mut &'a str) -> PResult<TPTPProblem<'a>> {
    ws.parse_next(input)?;

    let mut problem = TPTPProblem::new();

    // Parse inputs one at a time with cancellation checks
    loop {
        // Check for cancellation at each iteration
        check_cancel(input)?;

        ws.parse_next(input)?;

        // Check if we're at the end
        if input.is_empty() {
            break;
        }

        // Try to parse a single input
        match tptp_input.parse_next(input) {
            Ok(item) => match item {
                TPTPInput::Include(inc) => problem.includes.push(inc),
                TPTPInput::Formula(f) => problem.formulas.push(f),
            },
            Err(winnow::error::ErrMode::Backtrack(_)) => {
                // No more valid inputs, check if we're at the end
                break;
            }
            Err(e) => return Err(e),
        }
    }

    // Check for remaining input
    if !input.is_empty() {
        return Err(winnow::error::ErrMode::Cut(
            winnow::error::ContextError::new(),
        ));
    }

    Ok(problem)
}

/// A single item parsed from a TPTP file: either an include directive or an
/// annotated formula.
///
/// Produced by [`TPTPIterator`].
pub enum TPTPInput<'a> {
    /// An `include('file', [selection]).` directive.
    Include(Include<'a>),
    /// An annotated formula (`fof(…)`, `cnf(…)`, `tff(…)`, etc.).
    Formula(AnnotatedFormula<'a>),
}

/// Streaming iterator that yields one TPTP input at a time.
///
/// This avoids building a complete `TPTPProblem` with all formulas collected
/// into vectors, yielding each parsed item as it is encountered instead.
///
/// # Example
///
/// ```
/// use mrs_tptp::TPTPIterator;
///
/// let input = "fof(ax1, axiom, p). fof(ax2, axiom, q).";
/// let mut iter = TPTPIterator::new(input);
/// let mut count = 0;
/// for item in &mut iter {
///     item.expect("parse error");
///     count += 1;
/// }
/// assert_eq!(count, 2);
/// assert!(iter.remaining().is_empty());
/// ```
pub struct TPTPIterator<'a> {
    original: &'a str,
    input: &'a str,
    done: bool,
}

impl<'a> TPTPIterator<'a> {
    /// Create a new streaming TPTP parser over the given input.
    pub fn new(input: &'a str) -> Self {
        let mut s = input;
        let _ = ws.parse_next(&mut s);
        TPTPIterator {
            original: input,
            input: s,
            done: false,
        }
    }

    /// Return the remaining unparsed input.
    pub fn remaining(&self) -> &'a str {
        self.input
    }
}

impl<'a> Iterator for TPTPIterator<'a> {
    type Item = Result<TPTPInput<'a>, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done || self.input.is_empty() {
            return None;
        }
        let _ = ws.parse_next(&mut self.input);
        if self.input.is_empty() {
            return None;
        }
        let before = self.input;
        match tptp_input.parse_next(&mut self.input) {
            Ok(item) => Some(Ok(item)),
            Err(e) => {
                self.done = true;
                // Compute byte offset from the start of the original input.
                let offset = before.as_ptr() as usize - self.original.as_ptr() as usize;
                Some(Err(ParseError::new(format!("{e:?}"), offset)))
            }
        }
    }
}

/// Parse a single TPTP input
fn tptp_input<'a>(input: &mut &'a str) -> PResult<TPTPInput<'a>> {
    alt((
        include_directive.map(TPTPInput::Include),
        annotated_formula.map(TPTPInput::Formula),
    ))
    .context(StrContext::Label("tptp_input"))
    .parse_next(input)
}

/// Parse an include directive: include('filename', [selection]).
fn include_directive<'a>(input: &mut &'a str) -> PResult<Include<'a>> {
    "include".parse_next(input)?;
    ws.parse_next(input)?;
    '('.parse_next(input)?;
    ws.parse_next(input)?;

    let file_name = single_quoted
        .context(StrContext::Label("include filename"))
        .parse_next(input)?;

    ws.parse_next(input)?;

    // Optional selection list
    let selection = opt(preceded(
        (',', ws),
        delimited(
            ('[', ws),
            separated(
                0..,
                |i: &mut &'a str| {
                    let n = name(i)?;
                    ws.parse_next(i)?;
                    Ok(n)
                },
                (ws, ',', ws),
            ),
            (ws, ']'),
        ),
    ))
    .parse_next(input)?;

    ws.parse_next(input)?;
    ')'.parse_next(input)?;
    ws.parse_next(input)?;
    '.'.parse_next(input)?;

    Ok(Include {
        file_name,
        selection,
    })
}

/// Parse an annotated formula
fn annotated_formula<'a>(input: &mut &'a str) -> PResult<AnnotatedFormula<'a>> {
    alt((
        thf_annotated.map(AnnotatedFormula::THF),
        tff_annotated.map(AnnotatedFormula::TFF),
        tcf_annotated.map(AnnotatedFormula::TCF),
        fof_annotated.map(AnnotatedFormula::FOF),
        cnf_annotated.map(AnnotatedFormula::CNF),
        tpi_annotated.map(AnnotatedFormula::TPI),
    ))
    .context(StrContext::Label("annotated_formula"))
    .parse_next(input)
}

/// Parse a THF annotated formula
fn thf_annotated<'a>(input: &mut &'a str) -> PResult<THFAnnotated<'a>> {
    "thf".parse_next(input)?;
    ws.parse_next(input)?;
    '('.parse_next(input)?;
    ws.parse_next(input)?;

    let formula_name = name
        .context(StrContext::Label("formula name"))
        .parse_next(input)?;

    ws.parse_next(input)?;
    ','.parse_next(input)?;
    ws.parse_next(input)?;

    let role = formula_role
        .context(StrContext::Label("formula role"))
        .parse_next(input)?;

    ws.parse_next(input)?;
    ','.parse_next(input)?;
    ws.parse_next(input)?;

    let formula = cut_err(thf_statement)
        .context(StrContext::Label("THF formula"))
        .parse_next(input)?;

    ws.parse_next(input)?;

    let annot = annotations.parse_next(input)?;

    ws.parse_next(input)?;
    ')'.parse_next(input)?;
    ws.parse_next(input)?;
    '.'.parse_next(input)?;

    Ok(THFAnnotated {
        name: formula_name,
        role,
        formula,
        annotations: annot,
    })
}

/// Parse a TFF annotated formula
fn tff_annotated<'a>(input: &mut &'a str) -> PResult<TFFAnnotated<'a>> {
    "tff".parse_next(input)?;
    ws.parse_next(input)?;
    '('.parse_next(input)?;
    ws.parse_next(input)?;

    let formula_name = name.parse_next(input)?;
    ws.parse_next(input)?;
    ','.parse_next(input)?;
    ws.parse_next(input)?;

    let role = formula_role.parse_next(input)?;
    ws.parse_next(input)?;
    ','.parse_next(input)?;
    ws.parse_next(input)?;

    let formula = cut_err(tff_statement)
        .context(StrContext::Label("TFF formula"))
        .parse_next(input)?;

    ws.parse_next(input)?;
    let annot = annotations.parse_next(input)?;

    ws.parse_next(input)?;
    ')'.parse_next(input)?;
    ws.parse_next(input)?;
    '.'.parse_next(input)?;

    Ok(TFFAnnotated {
        name: formula_name,
        role,
        formula,
        annotations: annot,
    })
}

/// Parse a TCF annotated formula
fn tcf_annotated<'a>(input: &mut &'a str) -> PResult<TCFAnnotated<'a>> {
    "tcf".parse_next(input)?;
    ws.parse_next(input)?;
    '('.parse_next(input)?;
    ws.parse_next(input)?;

    let formula_name = name.parse_next(input)?;
    ws.parse_next(input)?;
    ','.parse_next(input)?;
    ws.parse_next(input)?;

    let role = formula_role.parse_next(input)?;
    ws.parse_next(input)?;
    ','.parse_next(input)?;
    ws.parse_next(input)?;

    let formula = cut_err(tcf_statement)
        .context(StrContext::Label("TCF formula"))
        .parse_next(input)?;

    ws.parse_next(input)?;
    let annot = annotations.parse_next(input)?;

    ws.parse_next(input)?;
    ')'.parse_next(input)?;
    ws.parse_next(input)?;
    '.'.parse_next(input)?;

    Ok(TCFAnnotated {
        name: formula_name,
        role,
        formula,
        annotations: annot,
    })
}

/// Parse a FOF annotated formula
fn fof_annotated<'a>(input: &mut &'a str) -> PResult<FOFAnnotated<'a>> {
    "fof".parse_next(input)?;
    ws.parse_next(input)?;
    '('.parse_next(input)?;
    ws.parse_next(input)?;

    let formula_name = name.parse_next(input)?;
    ws.parse_next(input)?;
    ','.parse_next(input)?;
    ws.parse_next(input)?;

    let role = formula_role.parse_next(input)?;
    ws.parse_next(input)?;
    ','.parse_next(input)?;
    ws.parse_next(input)?;

    let formula = cut_err(fof_statement)
        .context(StrContext::Label("FOF formula"))
        .parse_next(input)?;

    ws.parse_next(input)?;
    let annot = annotations.parse_next(input)?;

    ws.parse_next(input)?;
    ')'.parse_next(input)?;
    ws.parse_next(input)?;
    '.'.parse_next(input)?;

    Ok(FOFAnnotated {
        name: formula_name,
        role,
        formula,
        annotations: annot,
    })
}

/// Parse a CNF annotated formula
fn cnf_annotated<'a>(input: &mut &'a str) -> PResult<CNFAnnotated<'a>> {
    "cnf".parse_next(input)?;
    ws.parse_next(input)?;
    '('.parse_next(input)?;
    ws.parse_next(input)?;

    let formula_name = name.parse_next(input)?;
    ws.parse_next(input)?;
    ','.parse_next(input)?;
    ws.parse_next(input)?;

    let role = formula_role.parse_next(input)?;
    ws.parse_next(input)?;
    ','.parse_next(input)?;
    ws.parse_next(input)?;

    let formula = cut_err(cnf_statement)
        .context(StrContext::Label("CNF formula"))
        .parse_next(input)?;

    ws.parse_next(input)?;
    let annot = annotations.parse_next(input)?;

    ws.parse_next(input)?;
    ')'.parse_next(input)?;
    ws.parse_next(input)?;
    '.'.parse_next(input)?;

    Ok(CNFAnnotated {
        name: formula_name,
        role,
        formula,
        annotations: annot,
    })
}

/// Parse a TPI annotated formula
fn tpi_annotated<'a>(input: &mut &'a str) -> PResult<TPIAnnotated<'a>> {
    "tpi".parse_next(input)?;
    ws.parse_next(input)?;
    '('.parse_next(input)?;
    ws.parse_next(input)?;

    let formula_name = name.parse_next(input)?;
    ws.parse_next(input)?;
    ','.parse_next(input)?;
    ws.parse_next(input)?;

    let role = formula_role.parse_next(input)?;
    ws.parse_next(input)?;
    ','.parse_next(input)?;
    ws.parse_next(input)?;

    let formula = cut_err(fof_statement)
        .context(StrContext::Label("TPI formula"))
        .parse_next(input)?;

    ws.parse_next(input)?;
    let annot = annotations.parse_next(input)?;

    ws.parse_next(input)?;
    ')'.parse_next(input)?;
    ws.parse_next(input)?;
    '.'.parse_next(input)?;

    Ok(TPIAnnotated {
        name: formula_name,
        role,
        formula,
        annotations: annot,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Name;

    #[test]
    fn test_fof_annotated() {
        let input = "fof(ax1, axiom, p).";
        let result = parse_tptp(input);
        assert!(result.is_ok());

        let problem = result.unwrap();
        assert_eq!(problem.formulas.len(), 1);
    }

    #[test]
    fn test_fof_complex() {
        let input = "fof(mortality, axiom, ![X]: (human(X) => mortal(X))).";
        let result = parse_tptp(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cnf_annotated() {
        let input = "cnf(clause1, negated_conjecture, ~mortal(socrates)).";
        let result = parse_tptp(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_thf_annotated() {
        let input = "thf(p_type, type, p: $i > $o).";
        let result = parse_tptp(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_tff_annotated() {
        let input = "tff(nat_type, type, nat: $tType).";
        let result = parse_tptp(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_include() {
        let input = "include('axioms.ax').";
        let result = parse_tptp(input);
        assert!(result.is_ok());

        let problem = result.unwrap();
        assert_eq!(problem.includes.len(), 1);
        assert_eq!(problem.includes[0].file_name, "axioms.ax");
    }

    #[test]
    fn test_include_with_selection() {
        let input = "include('axioms.ax', [ax1, ax2]).";
        let result = parse_tptp(input);
        assert!(result.is_ok());

        let problem = result.unwrap();
        assert_eq!(problem.includes.len(), 1);
        assert_eq!(
            problem.includes[0].selection,
            Some(vec![Name::Lower("ax1"), Name::Lower("ax2")])
        );
    }

    #[test]
    fn test_multiple_formulas() {
        let input = r#"
            fof(ax1, axiom, ![X]: (human(X) => mortal(X))).
            fof(ax2, axiom, human(socrates)).
            fof(goal, conjecture, mortal(socrates)).
        "#;
        let result = parse_tptp(input);
        assert!(result.is_ok());

        let problem = result.unwrap();
        assert_eq!(problem.formulas.len(), 3);
    }

    #[test]
    fn test_with_comments() {
        let input = r#"
            % This is a comment
            fof(ax1, axiom, p).
            /* Block comment */
            fof(ax2, axiom, q).
        "#;
        let result = parse_tptp(input);
        assert!(result.is_ok());

        let problem = result.unwrap();
        assert_eq!(problem.formulas.len(), 2);
    }

    #[test]
    fn test_with_annotations() {
        let input = "fof(ax1, axiom, p, file('source.p', ax1)).";
        let result = parse_tptp(input);
        assert!(result.is_ok());

        let problem = result.unwrap();
        assert_eq!(problem.formulas.len(), 1);
        if let AnnotatedFormula::FOF(f) = &problem.formulas[0] {
            assert!(f.annotations.is_some());
        } else {
            panic!("Expected FOF formula");
        }
    }
}
