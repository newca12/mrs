//! Inference calculi for automated theorem proving.
//!
//! This crate implements the core inference rules used by the prover:
//!
//! - **Binary resolution**: Given two clauses with complementary literals,
//!   produce a resolvent by unifying the complementary atoms and removing them.
//! - **Factoring**: Given a clause with two same-polarity literals whose atoms
//!   unify, merge them to produce a shorter clause.
//! - **Superposition**: Rewriting subterms in clauses using oriented equalities.
//! - **Equality resolution/factoring**: Handling equality literals directly.
//! - **Demodulation**: Simplifying clauses using unit equalities.
//! - **Subsumption**: Detecting and removing redundant clauses.
//!
//! Variable renaming ([`rename`]) ensures clauses have disjoint variables
//! before inference. Term orderings ([`ordering`]) orient equalities.

pub mod demodulation;
pub mod equality;
pub mod factoring;
pub mod literal_selection;
pub mod ordering;
pub mod rename;
pub mod resolution;
pub mod subsumption;
pub mod superposition;
