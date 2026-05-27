//! Proof search engine with the given-clause loop.
//!
//! This crate implements the Otter-style given-clause algorithm:
//!
//! 1. Select a clause from the unprocessed set
//! 2. Generate all inferences with the processed set
//! 3. Add the clause to the processed set
//! 4. Add new clauses to the unprocessed set
//! 5. Repeat until empty clause found, saturated, or resource limit hit
//!
//! # Example
//!
//! ```
//! use std::sync::Arc;
//! use mrs_core::clause::ClauseIdGen;
//! use mrs_calculus::ordering::SymbolConfig;
//! use mrs_search::{SearchConfig, SearchResult, SelectionStrategy};
//! use mrs_search::state::SearchState;
//! use mrs_search::given_clause::search;
//!
//! let id_gen = ClauseIdGen::new();
//! let config_arc = Arc::new(SymbolConfig::default());
//! let mut state = SearchState::new(vec![], id_gen, config_arc);
//! let config = SearchConfig::default();
//! let result = search(&mut state, &config);
//! assert!(matches!(result, SearchResult::Saturated));
//! ```

pub mod avatar;
pub mod given_clause;
pub mod instgen;
pub mod select;
pub mod state;
pub mod strategy;
pub mod unprocessed;
pub mod weight;

use std::time::Duration;

use mrs_core::clause::ClauseId;

pub use mrs_calculus::literal_selection::LiteralSelection;
pub use mrs_calculus::ordering::TermOrdering;
pub use select::SelectionStrategy;

/// Result of a proof search.
#[derive(Clone, Debug)]
pub enum SearchResult {
    /// A refutation was found. Contains the ID of the empty clause and the proof TSTP string.
    Refutation(ClauseId, String),
    /// All clauses were processed without finding a contradiction.
    Saturated,
    /// The time limit was exceeded.
    Timeout,
    /// The clause limit was exceeded.
    ResourceOut,
    /// The search gave up (e.g. saturated with an incomplete strategy).
    GaveUp,
}

/// Configuration for the search engine.
#[derive(Clone, Debug)]
pub struct SearchConfig {
    /// Maximum wall-clock time for the search.
    pub time_limit: Duration,
    /// Maximum number of clauses to store.
    pub max_clauses: usize,
    /// Clause selection strategy.
    pub selection: SelectionStrategy,
    /// Literal selection strategy for inference restriction.
    pub literal_selection: LiteralSelection,
    /// Term ordering for orienting equalities.
    pub ordering: TermOrdering,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            time_limit: Duration::from_secs(5),
            max_clauses: 10_000,
            selection: SelectionStrategy::AgeWeight(5),
            literal_selection: LiteralSelection::AllNegative,
            ordering: TermOrdering::KBO,
        }
    }
}
