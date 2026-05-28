//! Search state: processed and unprocessed clause sets.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use mrs_calculus::ordering::SymbolConfig;
use mrs_core::SymbolId;
use mrs_core::clause::{Clause, ClauseId, ClauseIdGen};
use mrs_core::term::Term;
use mrs_index::dtree::DTree;
use mrs_index::literal_index::LiteralIndex;

use crate::avatar::AvatarContext;
use crate::unprocessed::UnprocessedSet;

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
    /// Dormant processed clauses (inactive under current AVATAR model).
    pub dormant_processed: HashMap<ClauseId, Clause>,
    /// Clauses that were in `unprocessed` but are currently inactive.
    pub dormant_unprocessed: HashMap<ClauseId, Clause>,
    /// Binary function symbols detected as commutative (from `f(X,Y)=f(Y,X)` axioms).
    /// Used by inference rules to enable commutativity unification.
    pub comm_symbols: HashSet<SymbolId>,
    /// Wall-clock deadline for the current search.  Set by `given_clause::search`
    /// immediately after recording `start`; used by `avatar_refute_branch` to skip
    /// expensive `solver.solve()` calls once the time budget is exhausted.
    pub search_deadline: Option<Instant>,
}

impl SearchState {
    /// Creates a new search state with the given initial clauses.
    ///
    /// All initial clauses are placed in the unprocessed set.
    /// When `use_avatar` is `false`, AVATAR clause splitting is skipped and every
    /// input clause is inserted verbatim.  Pass `false` for EPR/ground instances
    /// where AVATAR would create an intractably large SAT sub-problem.
    pub fn new(
        initial_clauses: Vec<Clause>,
        mut id_gen: ClauseIdGen,
        config: Arc<SymbolConfig>,
        use_avatar: bool,
    ) -> Self {
        let mut clause_store = HashMap::new();
        let mut unprocessed = UnprocessedSet::new(config.clone());
        let mut avatar = AvatarContext::new();

        for clause in initial_clauses {
            if use_avatar {
                if let Some(splits) = avatar.split_clause(&clause, &mut id_gen) {
                    for split in splits {
                        clause_store.insert(split.id, split.clone());
                        unprocessed.push(&split);
                    }
                } else {
                    clause_store.insert(clause.id, clause.clone());
                    unprocessed.push(&clause);
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
            comm_symbols: HashSet::new(),
            search_deadline: None,
        }
    }

    /// Total number of clauses stored.
    pub fn total_clauses(&self) -> usize {
        self.clause_store.len()
    }

    /// Checks if a clause is active under the current AVATAR model.
    pub fn is_active(&self, clause: &Clause) -> bool {
        clause
            .avatar
            .iter()
            .all(|&a| self.avatar.current_model.contains(&a))
    }
}
