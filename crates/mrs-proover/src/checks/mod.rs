//! Internal proof-step checks.
//!
//! Each module here implements a focused check that produces a
//! [`crate::verdict::StepOutcome`].

pub mod axiom_leaf;
pub mod definition_folding;
pub mod introduced_definition;
pub mod neg_conjecture;
pub mod propositional_sat;
pub mod skolemize;
pub mod vampire_skolemisation;
