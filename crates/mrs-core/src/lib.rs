//! Core logic types for the MRS automated theorem prover.
//!
//! This crate provides the foundational data types that all other MRS crates build upon:
//!
//! - [`Term`] - First-order terms (variables, function applications)
//! - [`Formula`] - First-order formulas (quantified, propositional connectives)
//! - [`Atom`] - Atomic formulas (predicates, equality)
//! - [`Literal`] - Signed atoms (positive or negative)
//! - [`Clause`] - Disjunctions of literals
//! - [`Substitution`] - Variable-to-term mappings
//! - [`SymbolTable`] - Bidirectional symbol interning

pub mod clause;
pub mod display;
pub mod formula;
pub mod subst;
pub mod symbol;
pub mod term;

pub use clause::{Clause, ClauseId, ClauseSource, Literal};
pub use formula::{Atom, Formula};
pub use subst::Substitution;
pub use symbol::{SymbolId, SymbolTable};
pub use term::{Term, VarId};
