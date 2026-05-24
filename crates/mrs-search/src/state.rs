//! Search state: processed and unprocessed clause sets.

use std::collections::{HashMap, VecDeque};

use mrs_core::clause::{Clause, ClauseId, ClauseIdGen};
use mrs_index::literal_index::LiteralIndex;

/// The mutable state of a proof search.
///
/// Tracks processed (active) clauses, unprocessed (passive) clauses,
/// and a clause store mapping IDs to clauses for proof reconstruction.
pub struct SearchState {
    /// Clauses that have been selected and had all inferences generated.
    /// Indexed by predicate symbol for fast resolution partner lookup.
    pub processed: LiteralIndex,
    /// Clauses waiting to be selected.
    pub unprocessed: VecDeque<Clause>,
    /// Maps clause IDs to clauses (for proof extraction).
    pub clause_store: HashMap<ClauseId, Clause>,
    /// Generator for fresh clause IDs.
    pub id_gen: ClauseIdGen,
}

impl SearchState {
    /// Creates a new search state with the given initial clauses.
    ///
    /// All initial clauses are placed in the unprocessed set.
    pub fn new(initial_clauses: Vec<Clause>, id_gen: ClauseIdGen) -> Self {
        let mut clause_store = HashMap::new();
        let mut unprocessed = VecDeque::new();

        for clause in initial_clauses {
            clause_store.insert(clause.id, clause.clone());
            unprocessed.push_back(clause);
        }

        Self {
            processed: LiteralIndex::new(),
            unprocessed,
            clause_store,
            id_gen,
        }
    }

    /// Total number of clauses stored.
    pub fn total_clauses(&self) -> usize {
        self.clause_store.len()
    }
}
