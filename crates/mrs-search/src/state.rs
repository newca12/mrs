//! Search state: processed and unprocessed clause sets.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use mrs_calculus::ordering::SymbolConfig;
use mrs_core::SymbolId;
use mrs_core::clause::{Clause, ClauseId, ClauseIdGen};
use mrs_core::term_bank::{IdClause, TermBank, TermId};
use mrs_index::dtree::DTreeId;
use mrs_index::literal_index::LiteralIndex;

use crate::avatar::AvatarContext;
use crate::unprocessed::UnprocessedSet;

/// The mutable state of a proof search.
///
/// Tracks processed (active) clauses, unprocessed (passive) clauses,
/// and a clause store mapping IDs to `IdClause` for proof reconstruction.
/// A shared `TermBank` owns all interned terms; conversion to legacy `Clause`
/// is deferred to proof-extraction time only.
pub struct SearchState {
    /// Clauses that have been selected and had all inferences generated.
    /// Indexed by predicate symbol for fast resolution partner lookup.
    pub processed: LiteralIndex,
    /// DTree indexing the LHS of oriented unit equalities for fast demodulation.
    /// The value is (from_id, to_id, unit_clause_id).
    pub demod_index: DTreeId<(TermId, TermId, ClauseId)>,
    /// Clauses waiting to be selected.
    pub unprocessed: UnprocessedSet,
    /// Maps clause IDs to `IdClause` (for proof extraction).
    pub clause_store: HashMap<ClauseId, IdClause>,
    /// Generator for fresh clause IDs.
    pub id_gen: ClauseIdGen,
    /// Configuration for symbol precedence and weights.
    pub config: Arc<SymbolConfig>,
    /// AVATAR context for clause splitting.
    pub avatar: AvatarContext,
    /// Dormant processed clauses (inactive under current AVATAR model).
    pub dormant_processed: HashMap<ClauseId, IdClause>,
    /// Clauses that were in `unprocessed` but are currently inactive.
    pub dormant_unprocessed: HashMap<ClauseId, IdClause>,
    /// Binary function symbols detected as commutative (from `f(X,Y)=f(Y,X)` axioms).
    pub comm_symbols: HashSet<SymbolId>,
    /// Wall-clock deadline for the current search.
    pub search_deadline: Option<Instant>,
    /// Interned-term arena shared by all clauses in this search.
    pub term_bank: TermBank,
    /// Optional stop-flag shared across parallel strategy threads.
    ///
    /// When set to `true` by another thread (e.g. because it found a
    /// refutation), the search loop treats it as an additional timeout and
    /// returns `SearchResult::Timeout` at the next time-check iteration.
    pub stop_flag: Option<Arc<AtomicBool>>,
}

impl SearchState {
    /// Creates a new search state from legacy `Clause` inputs.
    ///
    /// All input clauses are interned into a fresh `TermBank` and converted
    /// to `IdClause`. AVATAR splitting is performed if `use_avatar` is true.
    pub fn new(
        initial_clauses: Vec<Clause>,
        mut id_gen: ClauseIdGen,
        config: Arc<SymbolConfig>,
        use_avatar: bool,
    ) -> Self {
        let mut term_bank = TermBank::new();
        let mut clause_store: HashMap<ClauseId, IdClause> = HashMap::new();
        let mut unprocessed = UnprocessedSet::new(config.clone());
        let mut avatar = AvatarContext::new();

        for clause in initial_clauses {
            let id_clause = term_bank.clause_from_legacy(&clause);
            if use_avatar {
                if let Some(splits) = avatar.split_clause_id(&id_clause, &mut id_gen, &term_bank) {
                    for split in splits {
                        clause_store.insert(split.id, split.clone());
                        unprocessed.push(&split, &term_bank);
                    }
                } else {
                    clause_store.insert(id_clause.id, id_clause.clone());
                    unprocessed.push(&id_clause, &term_bank);
                }
            } else {
                clause_store.insert(id_clause.id, id_clause.clone());
                unprocessed.push(&id_clause, &term_bank);
            }
        }

        Self {
            processed: LiteralIndex::new(),
            demod_index: DTreeId::new(),
            unprocessed,
            clause_store,
            id_gen,
            config,
            avatar,
            dormant_processed: HashMap::new(),
            dormant_unprocessed: HashMap::new(),
            comm_symbols: HashSet::new(),
            search_deadline: None,
            term_bank,
            stop_flag: None,
        }
    }

    /// Total number of clauses stored.
    pub fn total_clauses(&self) -> usize {
        self.clause_store.len()
    }

    /// Checks if a clause is active under the current AVATAR model.
    pub fn is_active(&self, clause: &IdClause) -> bool {
        clause
            .avatar
            .iter()
            .all(|&a| self.avatar.current_model.contains(&a))
    }
}
