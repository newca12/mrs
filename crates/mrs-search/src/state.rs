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
    /// Binary function symbols detected as associative.
    pub assoc_symbols: HashSet<SymbolId>,
    /// Wall-clock deadline for the current search.
    pub search_deadline: Option<Instant>,
    /// Interned-term arena shared by all clauses in this search.
    pub term_bank: TermBank,
    /// Maps a clause ID to the IDs of all clauses generated from it (its children).
    pub children: HashMap<ClauseId, Vec<ClauseId>>,
    /// Optional stop-flag shared across parallel strategy threads.
    ///
    /// When set to `true` by another thread (e.g. because it found a
    /// refutation), the search loop treats it as an additional timeout and
    /// returns `SearchResult::Timeout` at the next time-check iteration.
    pub stop_flag: Option<Arc<AtomicBool>>,
    /// Shared pool of globally discovered unit equalities.
    pub shared_pool: Option<Arc<std::sync::RwLock<Vec<Clause>>>>,
    /// Number of clauses already consumed from the shared pool.
    pub shared_pool_read: usize,
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
            assoc_symbols: HashSet::new(),
            search_deadline: None,
            term_bank,
            children: HashMap::new(),
            stop_flag: None,
            shared_pool: None,
            shared_pool_read: 0,
        }
    }

    /// Total number of clauses stored.
    pub fn total_clauses(&self) -> usize {
        self.clause_store.len()
    }

    /// Removes a clause and all its descendants from all active and passive sets.
    /// This is Global Subsumption (Orphan Elimination).
    pub fn remove_clause_and_orphans(&mut self, id: ClauseId, ordering: &crate::TermOrdering) {
        let mut stack = vec![id];
        let mut visited = HashSet::new();

        while let Some(current) = stack.pop() {
            if !visited.insert(current) {
                continue;
            }

            // Remove from processed and demod_index
            if let Some(p) = self.processed.remove(current, &self.term_bank)
                && p.literals.len() == 1
                && p.literals[0].positive
            {
                use mrs_calculus::ordering::TermComparison;
                use mrs_core::term_bank::IdAtom;
                if let IdAtom::Eq(l, r) = &p.literals[0].atom {
                    if ordering.compare_id(*l, *r, &self.term_bank) == TermComparison::Greater {
                        self.demod_index
                            .remove(*l, &self.term_bank, &(*l, *r, p.id));
                    } else if ordering.compare_id(*r, *l, &self.term_bank)
                        == TermComparison::Greater
                    {
                        self.demod_index
                            .remove(*r, &self.term_bank, &(*r, *l, p.id));
                    }
                }
            }

            // Remove from unprocessed
            self.unprocessed.remove(current);

            // Remove from dormant sets
            self.dormant_processed.remove(&current);
            self.dormant_unprocessed.remove(&current);

            // Add children to stack
            if let Some(children) = self.children.get(&current) {
                stack.extend(children.iter().copied());
            }
        }
    }

    /// Checks if a clause is active under the current AVATAR model.
    pub fn is_active(&self, clause: &IdClause) -> bool {
        clause
            .avatar
            .iter()
            .all(|&a| self.avatar.current_model.contains(&a))
    }

    /// Registers a new clause in the store and tracks its dependencies.
    pub fn register_clause(&mut self, clause: &IdClause) {
        self.clause_store.insert(clause.id, clause.clone());
        if let mrs_core::clause::ClauseSource::Inference { parents, .. } = &clause.source {
            for &parent in parents {
                self.children.entry(parent).or_default().push(clause.id);
            }
        }
    }
}
