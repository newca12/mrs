//! Structured parse error type with position information.

use std::fmt;

/// A structured parse error with byte-offset position information.
///
/// The [`offset`] field contains the byte offset into the original input string
/// where parsing stopped. Use [`line`], [`column`], and [`snippet`] to convert
/// this into human-readable position information.
///
/// # Example
///
/// ```
/// use mrs_tptp::parse_tptp;
///
/// let input = "fof(ok, axiom, p).\nbad input here";
/// if let Err(e) = parse_tptp(input) {
///     eprintln!("error on line {}, col {}: {}", e.line(input), e.column(input), e.message);
///     eprintln!("near: {:?}", e.snippet(input));
/// }
/// ```
///
/// [`offset`]: ParseError::offset
/// [`line`]: ParseError::line
/// [`column`]: ParseError::column
/// [`snippet`]: ParseError::snippet
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    /// Human-readable error message.
    pub message: String,
    /// Byte offset into the original input where parsing failed.
    pub offset: usize,
}

impl ParseError {
    pub(crate) fn new(message: impl Into<String>, offset: usize) -> Self {
        ParseError {
            message: message.into(),
            offset,
        }
    }

    /// Return the 1-based line number of this error in `input`.
    ///
    /// Lines are counted by `\n` characters. Pass the same string that was
    /// given to [`parse_tptp`] for accurate results.
    ///
    /// [`parse_tptp`]: crate::parse_tptp
    pub fn line(&self, input: &str) -> usize {
        let until = self.offset.min(input.len());
        1 + input[..until].bytes().filter(|&b| b == b'\n').count()
    }

    /// Return the 1-based column number (in bytes) of this error in `input`.
    ///
    /// The column resets to 1 at the start of each `\n`-terminated line.
    pub fn column(&self, input: &str) -> usize {
        let until = self.offset.min(input.len());
        let last_newline = input[..until].rfind('\n').map(|p| p + 1).unwrap_or(0);
        until - last_newline + 1
    }

    /// Return a short excerpt of `input` around the error location.
    ///
    /// The slice covers roughly 20 bytes before and 40 bytes after the error
    /// offset, clamped to valid UTF-8 char boundaries.
    pub fn snippet<'a>(&self, input: &'a str) -> &'a str {
        let len = input.len();
        let offset = self.offset.min(len);

        // Walk forward from (offset - 20) to find a valid char boundary start.
        let raw_start = offset.saturating_sub(20);
        let start = (raw_start..=offset)
            .find(|&i| input.is_char_boundary(i))
            .unwrap_or(0);

        // Walk backward from (offset + 40) to find a valid char boundary end.
        let raw_end = (offset + 40).min(len);
        let end = (offset..=raw_end)
            .rev()
            .find(|&i| input.is_char_boundary(i))
            .unwrap_or(offset);

        &input[start..end]
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "parse error at byte offset {}: {}",
            self.offset, self.message
        )
    }
}

impl std::error::Error for ParseError {}
