//! Term indexing structures for efficient retrieval.
//!
//! Provides data structures for finding terms that unify with, match,
//! or generalize a query term. These are essential for efficient
//! resolution, superposition, and demodulation in theorem provers.
//!
//! # Structures
//!
//! - [`DTree`] - Discrimination tree: a trie indexed by the pre-order DFS
//!   traversal of terms. Supports unification retrieval (imperfect),
//!   generalization retrieval, and instance retrieval.

pub mod dtree;
pub mod fvi;
pub mod literal_index;
pub mod stree;
