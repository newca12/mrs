//! Search state: processed and unprocessed clause sets.

use crate::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use mrs_calculus::ordering::SymbolConfig;
use mrs_core::SymbolId;
use mrs_core::clause::{Clause, ClauseId, ClauseIdGen};
use mrs_core::term_bank::{IdAtom, IdClause, IdLiteral, TermBank, TermId};
use mrs_index::literal_index::LiteralIndex;
use mrs_index::stree::STreeId;

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
    /// STree indexing the LHS of oriented unit equalities for fast demodulation.
    /// The value is (from_id, to_id, unit_clause_id).
    pub demod_index: STreeId<(TermId, TermId, ClauseId)>,
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
    /// Symbol table for mapping SymbolId to strings (used by ML features and TSTP output).
    pub symbols: Arc<mrs_core::SymbolTable>,
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
    ///
    /// Each entry is a full, topologically-sorted ancestor chain (input
    /// clauses first, the shared unit-equality clause last) rather than a
    /// single bare clause, so that a receiving thread can splice the entire
    /// justification into its own `clause_store` with remapped IDs. This
    /// preserves proof reconstructability: a shared clause must never end
    /// up in the final extracted proof with an empty/unjustified parent
    /// list (previously stamped `inference(shared, [status(thm)], [])`,
    /// which fails GDV-style structural leaf/parent checks).
    pub shared_pool: Option<Arc<std::sync::RwLock<Vec<Vec<Clause>>>>>,
    /// Number of clauses already consumed from the shared pool.
    pub shared_pool_read: usize,
    /// Directory to log ML feature vectors and labels to.
    pub log_ml_data: Option<String>,
    /// Whether to log in CSV format instead of wincode.
    pub ml_log_csv: bool,
    /// Loaded ML model for clause scoring.
    #[cfg(feature = "ml-guidance")]
    pub ml_model: Option<Arc<mrs_core::ml::model::ClauseClassifier<burn::backend::NdArray>>>,
    /// Cached scores for clauses
    #[cfg(feature = "ml-guidance")]
    pub scores: HashMap<ClauseId, f32>,
    /// Per-strategy performance counters (incremented by `given_clause::search`).
    pub stats: crate::SearchStats,
    /// Weight function used for passive-queue priority.  Copied from `SearchConfig`
    /// at `new_with_ml` time so every call site can access it cheaply.
    pub weight_fn: crate::ClauseWeightFn,
    /// Symbols that appear in any goal-connected clause (distance < 100).
    /// Used by the `ConjSymbolBoost` weight function.
    pub goal_symbols: rustc_hash::FxHashSet<mrs_core::SymbolId>,
}

impl SearchState {
    /// Creates a new search state from legacy `Clause` inputs.
    ///
    /// All input clauses are interned into a fresh `TermBank` and converted
    /// to `IdClause`. AVATAR splitting is performed if `use_avatar` is true.
    pub fn new(
        initial_clauses: Vec<Clause>,
        id_gen: ClauseIdGen,
        config: Arc<SymbolConfig>,
        symbols: Arc<mrs_core::SymbolTable>,
        use_avatar: bool,
    ) -> Self {
        Self::new_with_ml(
            initial_clauses,
            Vec::new(),
            id_gen,
            config,
            symbols,
            use_avatar,
            None,
            false,
            crate::ClauseWeightFn::Standard,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_ml(
        initial_clauses: Vec<Clause>,
        provenance: Vec<Clause>,
        id_gen: ClauseIdGen,
        config: Arc<SymbolConfig>,
        symbols: Arc<mrs_core::SymbolTable>,
        use_avatar: bool,
        log_ml_data: Option<String>,
        ml_log_csv: bool,
        weight_fn: crate::ClauseWeightFn,
    ) -> Self {
        let mut term_bank = TermBank::new();
        let mut clause_store: HashMap<ClauseId, IdClause> = HashMap::default();
        let mut unprocessed = UnprocessedSet::new(config.clone());
        let mut avatar = AvatarContext::new();
        let mut id_gen = id_gen;

        // Pre-compute goal symbols from conjecture-connected input clauses.
        let goal_symbols = {
            let mut syms: rustc_hash::FxHashSet<mrs_core::SymbolId> =
                rustc_hash::FxHashSet::default();
            for c in initial_clauses.iter().filter(|c| c.distance < 100) {
                for lit in &c.literals {
                    match &lit.atom {
                        mrs_core::formula::Atom::Pred(s, args) => {
                            syms.insert(*s);
                            let mut stack: Vec<&mrs_core::term::Term> = args.iter().collect();
                            while let Some(t) = stack.pop() {
                                if let mrs_core::term::Term::App(f, a) = t {
                                    syms.insert(*f);
                                    stack.extend(a.iter());
                                }
                            }
                        }
                        mrs_core::formula::Atom::Eq(l, r) => {
                            let mut stack = vec![l, r];
                            while let Some(t) = stack.pop() {
                                if let mrs_core::term::Term::App(f, a) = t {
                                    syms.insert(*f);
                                    stack.extend(a.iter());
                                }
                            }
                        }
                    }
                }
            }
            syms
        };

        for clause in initial_clauses {
            let id_clause = term_bank.clause_from_legacy(&clause);
            let w = crate::weight::clause_weight_fn(
                &id_clause,
                &term_bank,
                &config,
                &weight_fn,
                &goal_symbols,
            );
            if use_avatar {
                if let Some(splits) = avatar.split_clause_id(&id_clause, &mut id_gen, &term_bank) {
                    for split in splits {
                        let sw = crate::weight::clause_weight_fn(
                            &split,
                            &term_bank,
                            &config,
                            &weight_fn,
                            &goal_symbols,
                        );
                        clause_store.insert(split.id, split.clone());
                        unprocessed.push(&split, &term_bank, sw, None);
                    }
                } else {
                    clause_store.insert(id_clause.id, id_clause.clone());
                    unprocessed.push(&id_clause, &term_bank, w, None);
                }
            } else {
                clause_store.insert(id_clause.id, id_clause.clone());
                unprocessed.push(&id_clause, &term_bank, w, None);
            }
        }

        // Provenance-only clauses (non-clausal FOF-level proof steps, e.g.
        // NNF/Skolemization results from `mrs_cnf::clausify_with_provenance`)
        // are registered in `clause_store` for proof-extraction lookup ONLY.
        // They must never touch `unprocessed`/`processed`: their `literals`
        // are empty (real content lives in `formula`), which the given-clause
        // loop would otherwise misread as the empty clause (a refutation).
        for clause in provenance {
            debug_assert!(
                clause.formula.is_some(),
                "provenance clauses must have formula: Some(_); real clauses belong in initial_clauses"
            );
            let id_clause = term_bank.clause_from_legacy(&clause);
            clause_store.insert(id_clause.id, id_clause);
        }

        Self {
            processed: LiteralIndex::new(),
            demod_index: STreeId::new(),
            unprocessed,
            clause_store,
            id_gen,
            config,
            avatar,
            dormant_processed: HashMap::default(),
            dormant_unprocessed: HashMap::default(),
            comm_symbols: HashSet::default(),
            assoc_symbols: HashSet::default(),
            search_deadline: None,
            symbols,
            term_bank,
            children: HashMap::default(),
            stop_flag: None,
            shared_pool: None,
            shared_pool_read: 0,
            log_ml_data,
            ml_log_csv,
            #[cfg(feature = "ml-guidance")]
            ml_model: None,
            #[cfg(feature = "ml-guidance")]
            scores: HashMap::default(),
            stats: crate::SearchStats::default(),
            weight_fn,
            goal_symbols,
        }
    }

    /// Computes the clause weight using the strategy's configured weight function.
    pub fn compute_weight(&self, clause: &IdClause) -> u32 {
        crate::weight::clause_weight_fn(
            clause,
            &self.term_bank,
            &self.config,
            &self.weight_fn,
            &self.goal_symbols,
        )
    }

    /// Computes and caches the ML score for a clause.
    #[cfg(feature = "ml-guidance")]
    pub fn get_ml_score(&mut self, clause: &IdClause) -> Option<f32> {
        if let Some(model) = &self.ml_model {
            if let Some(&score) = self.scores.get(&clause.id) {
                return Some(score);
            }
            let weight =
                crate::weight::clause_weight_id(clause, &self.term_bank, &self.config) as f32;
            let feats =
                mrs_core::ml::features::extract(clause, &self.term_bank, &self.symbols, weight);

            use burn::tensor::Tensor;
            let tensor =
                Tensor::<burn::backend::NdArray, 1>::from_floats(feats, &Default::default());
            let tensor = tensor.reshape([1, 128]); // batch size 1
            let logit = model.forward(tensor).into_scalar();
            self.scores.insert(clause.id, logit);
            Some(logit)
        } else {
            None
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
        let mut visited = HashSet::default();

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
        if let mrs_core::clause::ClauseSource::Inference { rule, parents } = &clause.source {
            let is_destructive = *rule == "demodulation" || *rule == "subsumption_resolution";
            for (i, &parent) in parents.iter().enumerate() {
                // For destructive inference rules, the primary target clause is always
                // at index 0. Subsequent parents are auxiliary rewrites or subsuming clauses.
                // We only track the primary target for orphan elimination. If an auxiliary
                // parent is backward-subsumed, we should NOT delete the derived clause,
                // otherwise we lose completeness.
                if is_destructive && i > 0 {
                    continue;
                }
                self.children.entry(parent).or_default().push(clause.id);
            }
        }
    }

    /// Normalizes a clause's literals/atoms modulo AC symbols using right-association.
    /// Canonicalizes equality literals by placing the smaller TermId argument first.
    pub fn ac_normalize_clause(
        &mut self,
        clause: IdClause,
        ac_syms: &HashSet<SymbolId>,
    ) -> IdClause {
        if ac_syms.is_empty() {
            return clause;
        }
        let mut norm_lits = Vec::with_capacity(clause.literals.len());
        for lit in clause.literals {
            let norm_atom = match lit.atom {
                IdAtom::Pred(sym, args) => {
                    let norm_args = args
                        .iter()
                        .map(|&arg| self.term_bank.ac_normalize(arg, ac_syms))
                        .collect();
                    IdAtom::Pred(sym, norm_args)
                }
                IdAtom::Eq(l, r) => {
                    let norm_l = self.term_bank.ac_normalize(l, ac_syms);
                    let norm_r = self.term_bank.ac_normalize(r, ac_syms);
                    if norm_l.0 <= norm_r.0 {
                        IdAtom::Eq(norm_l, norm_r)
                    } else {
                        IdAtom::Eq(norm_r, norm_l)
                    }
                }
            };
            norm_lits.push(IdLiteral {
                positive: lit.positive,
                atom: norm_atom,
            });
        }
        let mut norm_clause =
            IdClause::new_avatar(clause.id, norm_lits, clause.source, clause.avatar);
        norm_clause.formula = clause.formula;
        norm_clause
    }

    /// Recursively AC-normalizes all active clauses and updates their weights in queues.
    pub fn ac_normalize_all(&mut self, ac_syms: &HashSet<SymbolId>) {
        if ac_syms.is_empty() {
            return;
        }

        // 1. Normalize all clauses in clause_store
        let ids: Vec<ClauseId> = self.clause_store.keys().copied().collect();
        for id in ids {
            let clause = self.clause_store.remove(&id).unwrap();
            let norm_clause = self.ac_normalize_clause(clause, ac_syms);
            self.clause_store.insert(id, norm_clause);
        }

        // 2. Re-populate unprocessed queue with normalized clauses and updated weights
        let mut unprocessed_clauses = Vec::new();
        for id in self.unprocessed.iter() {
            if let Some(clause) = self.clause_store.get(&id) {
                unprocessed_clauses.push(clause.clone());
            }
        }

        // Fix: Restore strict chronological generation order for age_queue (FIFO)
        unprocessed_clauses.sort_unstable_by_key(|c| c.id.0);

        self.unprocessed = crate::unprocessed::UnprocessedSet::new(self.config.clone());
        for clause in unprocessed_clauses {
            let w = self.compute_weight(&clause);
            #[cfg(feature = "ml-guidance")]
            let score = self.get_ml_score(&clause);
            #[cfg(not(feature = "ml-guidance"))]
            let score = None;
            self.unprocessed.push(&clause, &self.term_bank, w, score);
        }
    }
}

#[cfg(all(test, feature = "ml-guidance"))]
mod tests {
    use super::*;
    use mrs_calculus::ordering::SymbolConfig;
    use mrs_core::SymbolTable;
    use mrs_core::clause::{Clause, ClauseSource};
    use std::time::Instant;

    #[test]
    fn benchmark_scoring_overhead() {
        let mut symbols = SymbolTable::new();
        let mut bank = mrs_core::term_bank::TermBank::new();
        let p = symbols.intern("p");

        let mut clauses = Vec::new();
        for id in 1..=2000 {
            let lits = vec![mrs_core::Literal::pos(mrs_core::Atom::pred(p, vec![]))];
            let clause = Clause::new(
                ClauseId(id),
                lits,
                ClauseSource::Input {
                    name: "test".into(),
                    role: "axiom".into(),
                },
            );
            clauses.push(bank.clause_from_legacy(&clause));
        }

        let mut state = SearchState::new(
            vec![],
            ClauseIdGen::new(),
            std::sync::Arc::new(SymbolConfig::default()),
            std::sync::Arc::new(symbols),
            false,
        );

        let device = Default::default();
        let model = Arc::new(mrs_core::ml::model::ClauseClassifier::new(&device));
        state.ml_model = Some(model);

        // Run warm-up
        let _ = state.get_ml_score(&clauses[0]);

        let start = Instant::now();
        for c in &clauses {
            let _ = state.get_ml_score(c);
        }
        let elapsed = start.elapsed();
        let avg_time = elapsed.as_secs_f64() * 1_000_000.0 / clauses.len() as f64;

        println!(
            "Average ML scoring latency: {:.4} microseconds/clause",
            avg_time
        );

        let limit = if cfg!(debug_assertions) {
            10000.0
        } else {
            100.0
        };
        assert!(
            avg_time < limit,
            "ML scoring latency overhead ({:.4} µs) is too high!",
            avg_time
        );
    }
}
