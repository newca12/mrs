//! Lexer/tokenizer for TPTP.
//!
//! This module provides low-level parsers for TPTP tokens: words, numbers,
//! strings, comments, and whitespace.

#[cfg(feature = "cancellation")]
use std::sync::atomic::{AtomicBool, Ordering};
use winnow::ascii::digit1;
use winnow::combinator::{alt, delimited, opt, preceded};
use winnow::error::ContextError;
use winnow::prelude::*;
use winnow::token::{one_of, take_while};

use crate::ast::{AtomicWord, Comment, DefinedWord, Name, Number, SystemWord};

/// Result type for internal parsing (with modal error handling)
pub type PResult<O> = winnow::error::ModalResult<O, ContextError>;

// Thread-local cancellation flag for cooperative cancellation
#[cfg(feature = "cancellation")]
thread_local! {
    static CANCEL_FLAG: std::cell::Cell<Option<*const AtomicBool>> = const { std::cell::Cell::new(None) };
}

/// Set the cancellation flag for the current thread.
/// The flag should remain valid for the duration of parsing.
///
/// # Safety
/// The caller must ensure the AtomicBool lives at least as long as parsing runs.
#[cfg(feature = "cancellation")]
pub fn set_cancel_flag(flag: &AtomicBool) {
    CANCEL_FLAG.with(|f| f.set(Some(flag as *const AtomicBool)));
}

/// Clear the cancellation flag for the current thread.
#[cfg(feature = "cancellation")]
pub fn clear_cancel_flag() {
    CANCEL_FLAG.with(|f| f.set(None));
}

/// Check if cancellation has been requested.
/// Returns true if cancelled, false otherwise.
#[cfg(feature = "cancellation")]
#[inline]
pub fn is_cancelled() -> bool {
    CANCEL_FLAG.with(|f| {
        if let Some(ptr) = f.get() {
            // SAFETY: The caller of set_cancel_flag guarantees the pointer is valid
            unsafe { (*ptr).load(Ordering::Relaxed) }
        } else {
            false
        }
    })
}

/// Check cancellation and return an error if cancelled.
/// This should be called periodically in parsing loops.
#[inline(always)]
pub fn check_cancel(_input: &mut &str) -> PResult<()> {
    #[cfg(feature = "cancellation")]
    if is_cancelled() {
        return Err(winnow::error::ErrMode::Cut(ContextError::new()));
    }
    Ok(())
}

/// Skip whitespace and comments -- hand-optimized fast path
#[inline(always)]
pub fn ws(input: &mut &str) -> PResult<()> {
    loop {
        // Fast skip ASCII whitespace
        let bytes = input.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b' ' | b'\t' | b'\n' | b'\r' => i += 1,
                _ => break,
            }
        }
        if i > 0 {
            *input = &input[i..];
        }

        let bytes = input.as_bytes();
        if bytes.first() == Some(&b'%') {
            // Line comment: skip to newline
            let mut j = 1;
            while j < bytes.len() && bytes[j] != b'\n' {
                j += 1;
            }
            if j < bytes.len() {
                j += 1; // skip the newline
            }
            *input = &input[j..];
        } else if bytes.len() >= 2 && bytes[0] == b'/' && bytes[1] == b'*' {
            // Block comment: use existing parser
            block_comment.void().parse_next(input)?;
        } else {
            break;
        }
    }
    Ok(())
}

/// Parse a line comment: % ... newline
pub fn line_comment<'a>(input: &mut &'a str) -> PResult<Comment<'a>> {
    let content = preceded('%', take_while(0.., |c| c != '\n' && c != '\r')).parse_next(input)?;

    // Consume the newline if present
    opt(one_of(['\n', '\r'])).parse_next(input)?;

    Ok(Comment {
        content,
        is_block: false,
    })
}

/// Parse a block comment: /* ... */
pub fn block_comment<'a>(input: &mut &'a str) -> PResult<Comment<'a>> {
    "/*".parse_next(input)?;

    let start = *input;
    let mut depth = 1;

    while depth > 0 && !input.is_empty() {
        if input.starts_with("*/") {
            depth -= 1;
            if depth == 0 {
                let content = &start[..start.len() - input.len()];
                "*/".parse_next(input)?;
                return Ok(Comment {
                    content,
                    is_block: true,
                });
            }
            *input = &input[2..];
        } else if input.starts_with("/*") {
            depth += 1;
            *input = &input[2..];
        } else {
            *input = &input[1..];
        }
    }

    Err(winnow::error::ErrMode::Cut(
        winnow::error::ContextError::new(),
    ))
}

/// Parse a lower_word: [a-z][a-zA-Z0-9_']*
#[inline]
pub fn lower_word<'a>(input: &mut &'a str) -> PResult<&'a str> {
    (
        one_of('a'..='z'),
        take_while(0.., |c: char| {
            c.is_ascii_alphanumeric() || c == '_' || c == '\''
        }),
    )
        .take()
        .parse_next(input)
}

/// Parse an upper_word (variable): [A-Z][a-zA-Z0-9_']*
#[inline]
pub fn upper_word<'a>(input: &mut &'a str) -> PResult<&'a str> {
    (
        one_of('A'..='Z'),
        take_while(0.., |c: char| {
            c.is_ascii_alphanumeric() || c == '_' || c == '\''
        }),
    )
        .take()
        .parse_next(input)
}

/// Parse a single-quoted string: 'content'
/// Handles escape sequences: \' and \\
#[inline]
pub fn single_quoted<'a>(input: &mut &'a str) -> PResult<&'a str> {
    delimited('\'', single_quoted_content, '\'').parse_next(input)
}

fn single_quoted_content<'a>(input: &mut &'a str) -> PResult<&'a str> {
    let start = *input;

    loop {
        // Take non-special characters
        take_while(0.., |c| c != '\'' && c != '\\').parse_next(input)?;

        if input.starts_with('\'') {
            // End of string
            let len = start.len() - input.len();
            return Ok(&start[..len]);
        } else if input.starts_with('\\') {
            // Escape sequence
            *input = &input[1..];
            if !input.is_empty() {
                *input = &input[1..];
            } else {
                return Err(winnow::error::ErrMode::Cut(
                    winnow::error::ContextError::new(),
                ));
            }
        } else {
            // Unexpected end
            return Err(winnow::error::ErrMode::Cut(
                winnow::error::ContextError::new(),
            ));
        }
    }
}

/// Parse a distinct object: "content"
/// Handles escape sequences: \" and \\
#[inline]
pub fn distinct_object<'a>(input: &mut &'a str) -> PResult<&'a str> {
    delimited('"', distinct_object_content, '"').parse_next(input)
}

fn distinct_object_content<'a>(input: &mut &'a str) -> PResult<&'a str> {
    let start = *input;

    loop {
        take_while(0.., |c| c != '"' && c != '\\').parse_next(input)?;

        if input.starts_with('"') {
            let len = start.len() - input.len();
            return Ok(&start[..len]);
        } else if input.starts_with('\\') {
            *input = &input[1..];
            if !input.is_empty() {
                *input = &input[1..];
            } else {
                return Err(winnow::error::ErrMode::Cut(
                    winnow::error::ContextError::new(),
                ));
            }
        } else {
            return Err(winnow::error::ErrMode::Cut(
                winnow::error::ContextError::new(),
            ));
        }
    }
}

/// Parse a dollar_word: $[a-z][a-zA-Z0-9_]*
#[inline]
pub fn dollar_word<'a>(input: &mut &'a str) -> PResult<&'a str> {
    preceded(
        '$',
        (
            one_of('a'..='z'),
            take_while(0.., |c: char| c.is_ascii_alphanumeric() || c == '_'),
        )
            .take(),
    )
    .parse_next(input)
}

/// Parse a dollar_dollar_word: $$[a-z][a-zA-Z0-9_]*
#[inline]
pub fn dollar_dollar_word<'a>(input: &mut &'a str) -> PResult<&'a str> {
    preceded(
        "$$",
        (
            one_of('a'..='z'),
            take_while(0.., |c: char| c.is_ascii_alphanumeric() || c == '_'),
        )
            .take(),
    )
    .parse_next(input)
}

/// Parse an atomic word
#[inline]
pub fn atomic_word<'a>(input: &mut &'a str) -> PResult<AtomicWord<'a>> {
    alt((
        single_quoted.map(AtomicWord::SingleQuoted),
        lower_word.map(AtomicWord::Lower),
    ))
    .parse_next(input)
}

/// Parse a defined word as DefinedWord
#[inline]
pub fn defined_word<'a>(input: &mut &'a str) -> PResult<DefinedWord<'a>> {
    dollar_word.map(DefinedWord).parse_next(input)
}

/// Parse a system word as SystemWord
#[inline]
pub fn system_word<'a>(input: &mut &'a str) -> PResult<SystemWord<'a>> {
    dollar_dollar_word.map(SystemWord).parse_next(input)
}

/// Parse an integer (with optional sign) - returns string to handle arbitrary precision
pub fn integer_str(input: &mut &str) -> PResult<String> {
    let sign = opt(one_of(['+', '-'])).parse_next(input)?;
    let digits: &str = digit1.parse_next(input)?;

    let sign_str = if sign == Some('-') { "-" } else { "" };
    Ok(format!("{}{}", sign_str, digits))
}

/// Parse an integer (with optional sign) - for backward compatibility
pub fn integer(input: &mut &str) -> PResult<i64> {
    let sign = opt(one_of(['+', '-'])).parse_next(input)?;
    let digits: &str = digit1.parse_next(input)?;

    let value: i64 = digits
        .parse()
        .map_err(|_| winnow::error::ErrMode::Cut(winnow::error::ContextError::new()))?;

    Ok(if sign == Some('-') { -value } else { value })
}

/// Parse a number (integer, rational, or real) -- zero-copy
#[inline]
pub fn number<'a>(input: &mut &'a str) -> PResult<Number<'a>> {
    let start = *input;

    // Parse the initial sign and digits
    opt(one_of(['+', '-'])).parse_next(input)?;
    digit1.parse_next(input)?;

    // Check what follows
    if input.starts_with('/') {
        // Rational: integer/positive_integer
        *input = &input[1..];
        digit1.parse_next(input)?;
        let len = input.as_ptr() as usize - start.as_ptr() as usize;
        Ok(Number::Rational(&start[..len]))
    } else if input.starts_with('.') {
        // Real: decimal
        *input = &input[1..];
        digit1.parse_next(input)?;
        // Check for exponent
        if input.starts_with(['e', 'E']) {
            *input = &input[1..];
            opt(one_of(['+', '-'])).parse_next(input)?;
            digit1.parse_next(input)?;
        }
        let len = input.as_ptr() as usize - start.as_ptr() as usize;
        Ok(Number::Real(&start[..len]))
    } else if input.starts_with(['e', 'E']) {
        // Real with exponent but no decimal
        *input = &input[1..];
        opt(one_of(['+', '-'])).parse_next(input)?;
        digit1.parse_next(input)?;
        let len = input.as_ptr() as usize - start.as_ptr() as usize;
        Ok(Number::Real(&start[..len]))
    } else {
        // Integer
        let len = input.as_ptr() as usize - start.as_ptr() as usize;
        Ok(Number::Integer(&start[..len]))
    }
}

/// Parse a name (lower_word, single_quoted, or integer for anonymous)
#[inline]
pub fn name<'a>(input: &mut &'a str) -> PResult<Name<'a>> {
    alt((
        lower_word.map(Name::Lower),
        single_quoted.map(Name::SingleQuoted),
        // Anonymous names can be integers
        digit1.map(Name::Integer),
    ))
    .parse_next(input)
}

/// Parse a variable name
#[inline]
pub fn variable<'a>(input: &mut &'a str) -> PResult<&'a str> {
    upper_word.parse_next(input)
}

/// Parse a specific keyword, ensuring it's not part of a larger word
pub fn keyword<'a>(kw: &'static str) -> impl FnMut(&mut &'a str) -> PResult<&'a str> {
    move |input: &mut &'a str| {
        let result: &str = winnow::token::literal(kw).parse_next(input)?;
        // Ensure we're not in the middle of a word
        if let Some(c) = input.chars().next()
            && (c.is_ascii_alphanumeric() || c == '_')
        {
            return Err(winnow::error::ErrMode::Backtrack(
                winnow::error::ContextError::new(),
            ));
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lower_word() {
        assert_eq!(lower_word.parse_peek("hello"), Ok(("", "hello")));
        assert_eq!(lower_word.parse_peek("camelCase"), Ok(("", "camelCase")));
        assert_eq!(
            lower_word.parse_peek("with_underscore"),
            Ok(("", "with_underscore"))
        );
        assert_eq!(lower_word.parse_peek("x123"), Ok(("", "x123")));
        assert!(lower_word.parse_peek("123").is_err());
        assert!(lower_word.parse_peek("Upper").is_err());
    }

    #[test]
    fn test_upper_word() {
        assert_eq!(upper_word.parse_peek("X"), Ok(("", "X")));
        assert_eq!(upper_word.parse_peek("Variable"), Ok(("", "Variable")));
        assert_eq!(upper_word.parse_peek("VAR_1"), Ok(("", "VAR_1")));
        assert!(upper_word.parse_peek("lower").is_err());
    }

    #[test]
    fn test_single_quoted() {
        assert_eq!(single_quoted.parse_peek("'hello'"), Ok(("", "hello")));
        assert_eq!(
            single_quoted.parse_peek("'with space'"),
            Ok(("", "with space"))
        );
        assert_eq!(
            single_quoted.parse_peek("'escaped\\'quote'"),
            Ok(("", "escaped\\'quote"))
        );
    }

    #[test]
    fn test_distinct_object() {
        assert_eq!(distinct_object.parse_peek("\"object\""), Ok(("", "object")));
        assert_eq!(
            distinct_object.parse_peek("\"with space\""),
            Ok(("", "with space"))
        );
    }

    #[test]
    fn test_dollar_word() {
        assert_eq!(dollar_word.parse_peek("$true"), Ok(("", "true")));
        assert_eq!(dollar_word.parse_peek("$false"), Ok(("", "false")));
        assert_eq!(dollar_word.parse_peek("$ite"), Ok(("", "ite")));
    }

    #[test]
    fn test_number() {
        assert_eq!(number.parse_peek("42"), Ok(("", Number::Integer("42"))));
        assert_eq!(number.parse_peek("-17"), Ok(("", Number::Integer("-17"))));
        assert_eq!(number.parse_peek("3/4"), Ok(("", Number::Rational("3/4"))));

        assert_eq!(number.parse_peek("3.14"), Ok(("", Number::Real("3.14"))));
        assert_eq!(
            number.parse_peek("1.0e10"),
            Ok(("", Number::Real("1.0e10")))
        );
    }

    #[test]
    fn test_line_comment() {
        let result = line_comment.parse_peek("% this is a comment\n");
        assert!(result.is_ok());
        let (_rest, comment) = result.unwrap();
        assert_eq!(comment.content, " this is a comment");
        assert!(!comment.is_block);
    }

    #[test]
    fn test_block_comment() {
        let result = block_comment.parse_peek("/* block comment */");
        assert!(result.is_ok());
        let (_rest, comment) = result.unwrap();
        assert_eq!(comment.content, " block comment ");
        assert!(comment.is_block);
    }
}
