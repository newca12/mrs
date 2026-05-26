//! Search state: processed and unprocessed clause sets.

use std::collections::HashMap;
use std::sync::Arc;

use mrs_calculus::ordering::SymbolConfig;
use mrs_core::clause::{Clause, ClauseId, ClauseIdGen};
use mrs_core::term::Term;
use mrs_index::dtree::DTree;
use mrs_index::literal_index::LiteralIndex;

use crate::unprocessed::UnprocessedSet;
use crate::avatar::AvatarContext;

/// The mutable state of a proof search.
///
/// Tracks processed (active) clauses, unprocessed (passive) clauses,
/// and a clause store mapping IDs to clauses for proof reconstruction.
pub struct SearchState {
    /// Clauses that have been selected and had all inferences generated.
    /// Indexed by predicate symbol for fast resolution partner lookup.
    pub processed: LiteralIndex,
    /// DTree indexing the left-hand side of oriented unit equalities for fast demodulation.
    /// The value is (from, to, unit_clause_id).
    pub demod_index: DTree<(Term, Term, ClauseId)>,
    /// Clauses waiting to be selected.
    pub unprocessed: UnprocessedSet,
    /// Maps clause IDs to clauses (for proof extraction).
    pub clause_store: HashMap<ClauseId, Clause>,
    /// Generator for fresh clause IDs.
    pub id_gen: ClauseIdGen,
    /// Configuration for symbol precedence and weights.
    pub config: Arc<SymbolConfig>,
    /// AVATAR context for clause splitting.
    pub avatar: AvatarContext,
    /// Clauses that were in `processed` but are currently inactive.
    pub dormant_processed: HashMap<ClauseId, Clause>,
    /// Clauses that were in `unprocessed` but are currently inactive.
    pub dormant_unprocessed: HashMap<ClauseId, Clause>,
}

impl SearchState {
    /// Creates a new search state with the given initial clauses.
    ///
    /// All initial clauses are placed in the unprocessed set.
    pub fn new(
        initial_clauses: Vec<Clause>,
        mut id_gen: ClauseIdGen,
        config: Arc<SymbolConfig>,
    ) -> Self {
        let mut clause_store = HashMap::new();
        let mut unprocessed = UnprocessedSet::new(config.clone());
        let mut avatar = AvatarContext::new();

        for clause in initial_clauses {
            if let Some(splits) = avatar.split_clause(&clause, &mut id_gen) {
                for split in splits {
                    clause_store.insert(split.id, split.clone());
                    unprocessed.push(&split);
                }
            } else {
                clause_store.insert(clause.id, clause.clone());
                unprocessed.push(&clause);
            }
        }

        Self {
            processed: LiteralIndex::new(),
            demod_index: DTree::new(),
            unprocessed,
            clause_store,
            id_gen,
            config,
            avatar,
            dormant_processed: HashMap::new(),
            dormant_unprocessed: HashMap::new(),
        }
    }

    /// Total number of clauses stored.
    pub fn total_clauses(&self) -> usize {
        self.clause_store.len()
    }

    /// Checks if a clause is active under the current AVATAR model.
    pub fn is_active(&self, clause: &Clause) -> bool {
        clause.avatar.iter().all(|&a| self.avatar.current_model.contains(&a))
    }
}
