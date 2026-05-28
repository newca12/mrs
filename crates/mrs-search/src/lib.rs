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
//! let mut state = SearchState::new(vec![], id_gen, config_arc, true);
//! let config = SearchConfig::default();
//! let result = search(&mut state, &config);
//! assert!(matches!(result, SearchResult::Saturated));
//! ```

pub mod avatar;
pub mod cwa;
pub mod fvo;
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
    /// The search gave up (e.g. saturated with an incomplete strategy).
    GaveUp,
}

/// Configuration for the search engine.
#[derive(Clone, Debug)]
pub struct SearchConfig {
    /// Maximum wall-clock time for the search.
    pub time_limit: Duration,
    /// Clause selection strategy.
    pub selection: SelectionStrategy,
    /// Literal selection strategy for inference restriction.
    pub literal_selection: LiteralSelection,
    /// Term ordering for orienting equalities.
    pub ordering: TermOrdering,
    /// Maximum total symbol weight of any generated clause.
    ///
    /// Inferred clauses whose total weight (sum of all symbol occurrences
    /// across all literals) exceeds this limit are discarded immediately.
    /// This prevents unbounded term growth during superposition.
    /// `None` means no limit.
    pub max_term_weight: Option<u32>,
    /// Whether to enable AVATAR clause splitting via an embedded SAT solver.
    pub use_avatar: bool,
    /// If true, only generate resolvents where at least one parent is a unit
    /// (single-literal clause).  This restricts the inference to unit resolution,
    /// which dramatically reduces passive-set growth on FNE-encoded problems whose
    /// proofs consist entirely of unit-chain derivations.  The restriction is
    /// incomplete for general clause sets but correct (sound) everywhere.
    pub unit_only_resolution: bool,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            time_limit: Duration::from_secs(5),
            selection: SelectionStrategy::AgeWeight(5),
            literal_selection: LiteralSelection::AllNegative,
            ordering: TermOrdering::KBO,
            max_term_weight: Some(200),
            use_avatar: true,
            unit_only_resolution: false,
        }
    }
}
