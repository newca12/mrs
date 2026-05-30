//! Owned parsing: parse from `String` or a file path.
//!
//! This module provides [`OwnedTPTPProblem`], a self-contained value that owns
//! both the source text and the parsed AST. Use it when you need a `'static`-
//! friendly value or when reading from a file.
//!
//! Enable with the **`owned`** Cargo feature:
//!
//! ```toml
//! [dependencies]
//! mrs-tptp = { version = "0.1", features = ["owned"] }
//! ```
//!
//! # Example
//!
//! ```no_run
//! use std::path::Path;
//! use mrs_tptp::owned::parse_tptp_file;
//!
//! let problem = parse_tptp_file(Path::new("Axioms/SET001+0.ax")).unwrap();
//! println!("{} formulas", problem.formulas.len());
//! ```

use std::fmt;
use std::path::Path;

use crate::ast::TPTPProblem;
use crate::error::ParseError;
use crate::parser::parse_tptp;

/// Error returned by [`parse_tptp_file`].
#[derive(Debug)]
pub enum ParseFileError {
    /// An I/O error occurred while reading the file.
    Io(std::io::Error),
    /// The file was read but its contents could not be parsed.
    Parse(ParseError),
}

impl fmt::Display for ParseFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseFileError::Io(e) => write!(f, "I/O error: {e}"),
            ParseFileError::Parse(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ParseFileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ParseFileError::Io(e) => Some(e),
            ParseFileError::Parse(e) => Some(e),
        }
    }
}

impl From<std::io::Error> for ParseFileError {
    fn from(e: std::io::Error) -> Self {
        ParseFileError::Io(e)
    }
}

impl From<ParseError> for ParseFileError {
    fn from(e: ParseError) -> Self {
        ParseFileError::Parse(e)
    }
}

/// A parsed TPTP problem that owns its source text.
///
/// Unlike [`TPTPProblem`], which borrows from a `&str`, `OwnedTPTPProblem`
/// holds the original `String` alongside the parsed data, making it
/// convenient to store, pass across thread boundaries, or return from
/// functions without lifetime gymnastics.
///
/// Use [`Deref`] to access the inner [`TPTPProblem`]:
///
/// ```no_run
/// # use mrs_tptp::owned::parse_tptp_owned;
/// let owned = parse_tptp_owned("fof(ax, axiom, p).".to_string()).unwrap();
/// println!("{} formula(s)", owned.formulas.len());   // via Deref
/// ```
///
/// # Safety
///
/// Internally this type stores a `TPTPProblem<'static>` whose string slices
/// actually point into the `String` kept in the same struct. This is
/// sound because:
///
/// 1. A `String`'s byte buffer lives on the heap and does not move when the
///    `String` header itself is moved into this struct.
/// 2. `_source` is dropped *after* `problem` (field declaration order).
/// 3. `_source` is private and never exposed as `&mut`.
///
/// [`Deref`]: std::ops::Deref
pub struct OwnedTPTPProblem {
    // IMPORTANT: field order matters — `_source` must be declared before
    // `problem` so it is dropped last.
    _source: String,
    // SAFETY: The 'static lifetime is an intentional lie — these slices point
    // into `_source`. See the safety comment on the struct.
    problem: TPTPProblem<'static>,
}

impl OwnedTPTPProblem {
    /// Return a reference to the underlying [`TPTPProblem`].
    ///
    /// The lifetime of the returned reference is tied to `&self`.
    pub fn problem(&self) -> &TPTPProblem<'_> {
        &self.problem
    }

    /// Return a reference to the original source text.
    pub fn source(&self) -> &str {
        &self._source
    }
}

impl std::ops::Deref for OwnedTPTPProblem {
    type Target = TPTPProblem<'static>;

    fn deref(&self) -> &Self::Target {
        &self.problem
    }
}

impl fmt::Debug for OwnedTPTPProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OwnedTPTPProblem")
            .field("formulas", &self.problem.formulas.len())
            .field("includes", &self.problem.includes.len())
            .finish()
    }
}

/// Parse a TPTP problem from an owned `String`.
///
/// The returned [`OwnedTPTPProblem`] owns both the source text and the AST,
/// so it has no lifetime parameters.
///
/// # Errors
///
/// Returns a [`ParseError`] if the content is not valid TPTP.
///
/// # Example
///
/// ```
/// use mrs_tptp::owned::parse_tptp_owned;
///
/// let src = "fof(ax, axiom, human(socrates)).".to_string();
/// let problem = parse_tptp_owned(src).unwrap();
/// assert_eq!(problem.formulas.len(), 1);
/// ```
pub fn parse_tptp_owned(source: String) -> Result<OwnedTPTPProblem, ParseError> {
    // SAFETY: `source_ref` points into the heap-allocated byte buffer of
    // `source`. We are about to move `source` into the struct without
    // touching the String's contents; moving the `String` header does not
    // move its buffer, so the pointer remains valid for the lifetime of the
    // returned `OwnedTPTPProblem`.
    let source_ref: &str = source.as_str();
    let source_static: &'static str = unsafe { std::mem::transmute(source_ref) };

    let problem = parse_tptp(source_static)?;

    Ok(OwnedTPTPProblem {
        _source: source,
        problem,
    })
}

/// Read a TPTP file from disk and parse it.
///
/// Returns an [`OwnedTPTPProblem`] that owns both the file contents and the
/// parsed AST.
///
/// # Errors
///
/// Returns [`ParseFileError::Io`] if the file cannot be read, or
/// [`ParseFileError::Parse`] if the contents are not valid TPTP.
///
/// # Example
///
/// ```no_run
/// use std::path::Path;
/// use mrs_tptp::owned::parse_tptp_file;
///
/// let problem = parse_tptp_file(Path::new("Axioms/SET001+0.ax")).unwrap();
/// for formula in problem.axioms() {
///     println!("{}", formula.name());
/// }
/// ```
pub fn parse_tptp_file(path: &Path) -> Result<OwnedTPTPProblem, ParseFileError> {
    let source = std::fs::read_to_string(path)?;
    parse_tptp_owned(source).map_err(ParseFileError::Parse)
}
