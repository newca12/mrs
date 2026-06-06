//! First-order unification and matching algorithms.
//!
//! This crate provides algorithms for finding substitutions that make terms equal:
//!
//! - [`unify`] - Most general unifier (MGU) of two terms
//! - [`match_term`] - One-way matching: find a substitution making a pattern equal to a target
//!
//! Multiple algorithm implementations are available for educational comparison:
//!
//! - [`robinson`] - Classic recursive algorithm (simple, easy to understand)
//!
//! # Examples
//!
//! ```
//! use mrs_core::{Term, SymbolTable};
//! use mrs_unify::unify;
//!
//! let mut syms = SymbolTable::new();
//! let f = syms.intern("f");
//! let a = syms.intern("a");
//!
//! // Unify f(X, a) with f(b, Y)
//! let b = syms.intern("b");
//! let t1 = Term::app(f, vec![Term::var(0), Term::constant(a)]);
//! let t2 = Term::app(f, vec![Term::constant(b), Term::var(1)]);
//!
//! let mgu = unify(&t1, &t2).unwrap();
//! // X -> b, Y -> a
//! assert_eq!(mgu.apply_term(&Term::var(0)), Term::constant(b));
//! assert_eq!(mgu.apply_term(&Term::var(1)), Term::constant(a));
//! ```

pub mod matching;
pub mod robinson;

use std::fmt;

use mrs_core::Substitution;
use mrs_core::term::Term;

/// Error type for unification failures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UnifyError {
    /// The two terms have different top-level function symbols.
    SymbolClash { left: String, right: String },
    /// The two terms have different numbers of arguments.
    ArityMismatch { expected: usize, found: usize },
    /// A variable occurs in the term it would be unified with (infinite term).
    OccursCheck { var: u32 },
}

impl fmt::Display for UnifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnifyError::SymbolClash { left, right } => {
                write!(f, "symbol clash: {} vs {}", left, right)
            }
            UnifyError::ArityMismatch { expected, found } => {
                write!(f, "arity mismatch: expected {}, found {}", expected, found)
            }
            UnifyError::OccursCheck { var } => {
                write!(f, "occurs check failed for X{}", var)
            }
        }
    }
}

impl std::error::Error for UnifyError {}

/// Result type for unification.
pub type UnifyResult = Result<Substitution, UnifyError>;

/// Unifies two terms, returning their most general unifier (MGU).
///
/// Uses Robinson's algorithm. Returns `Err` if the terms cannot be unified.
///
/// # Examples
///
/// ```
/// use mrs_core::{Term, SymbolTable};
/// use mrs_unify::unify;
///
/// let mut syms = SymbolTable::new();
/// let f = syms.intern("f");
/// let a = syms.intern("a");
///
/// // Unify X with a
/// let mgu = unify(&Term::var(0), &Term::constant(a)).unwrap();
/// assert_eq!(mgu.apply_term(&Term::var(0)), Term::constant(a));
///
/// // Cannot unify f(a) with a (symbol clash)
/// let t1 = Term::app(f, vec![Term::constant(a)]);
/// assert!(unify(&t1, &Term::constant(a)).is_err());
/// ```
pub fn unify(s: &Term, t: &Term) -> UnifyResult {
    robinson::unify(s, t)
}

/// Unifies two terms with commutativity support for a set of binary symbols.
///
/// Delegates to [`robinson::unify_comm`]. See that function for details.
///
/// # Examples
///
/// ```
/// use std::collections::HashSet;
/// use mrs_core::{Term, SymbolTable};
/// use mrs_unify::unify_comm;
///
/// let mut syms = SymbolTable::new();
/// let f = syms.intern("f");
/// let a = syms.intern("a");
/// let b = syms.intern("b");
///
/// // f(a,b) ~ f(b,a) with f commutative
/// let mut comm = HashSet::new();
/// comm.insert(f);
/// let t1 = Term::app(f, vec![Term::constant(a), Term::constant(b)]);
/// let t2 = Term::app(f, vec![Term::constant(b), Term::constant(a)]);
/// assert!(unify_comm(&t1, &t2, &comm).is_ok());
/// ```
pub fn unify_comm(
    s: &Term,
    t: &Term,
    comm: &std::collections::HashSet<mrs_core::SymbolId, impl std::hash::BuildHasher>,
) -> UnifyResult {
    robinson::unify_comm(s, t, comm)
}

/// Matches a pattern against a target term (one-way unification).
///
/// Only variables in the pattern are bound; the target is treated as ground.
/// Returns `Err` if the pattern cannot be matched against the target.
///
/// # Examples
///
/// ```
/// use mrs_core::{Term, SymbolTable};
/// use mrs_unify::match_term;
///
/// let mut syms = SymbolTable::new();
/// let f = syms.intern("f");
/// let a = syms.intern("a");
///
/// // Match f(X) against f(a) -> X = a
/// let pattern = Term::app(f, vec![Term::var(0)]);
/// let target = Term::app(f, vec![Term::constant(a)]);
/// let sub = match_term(&pattern, &target).unwrap();
/// assert_eq!(sub.apply_term(&Term::var(0)), Term::constant(a));
/// ```
pub fn match_term(pattern: &Term, target: &Term) -> UnifyResult {
    matching::match_term(pattern, target)
}
