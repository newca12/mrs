//! Clause literal index by predicate symbol.
//!
//! Maps `(predicate_symbol, polarity)` to clause IDs that contain a literal
//! with that predicate and polarity. This dramatically reduces the number
//! of clause pairs considered during resolution: instead of trying every
//! processed clause, only those with a complementary predicate are examined.
//!
//! Stores `IdClause` values. The internal `DTree` is keyed on legacy `Term`
//! values converted from `IdAtom` via `TermBank::to_legacy` — this avoids
//! needing `&mut TermBank` at query time while still leveraging the existing
//! discrimination-tree infrastructure.

use crate::{HashMap, HashSet};

use mrs_core::clause::ClauseId;
use mrs_core::symbol::SymbolId;
use mrs_core::term_bank::{IdAtom, IdClause, TermBank, TermId};

use crate::fvi::{FeatureVector, FeatureVectorTree};

/// Key for the literal index: a predicate symbol with a polarity.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
struct LitKey {
    pred: SymbolId,
    positive: bool,
}

/// An index mapping predicate symbols (with polarity) to clause IDs.
///
/// Stores `IdClause` values; all indexed terms are kept in an external
/// `TermBank`. The internal discrimination tree uses legacy `Term` values
/// (converted on insert/remove/query) so that query methods only need
/// `&TermBank` (immutable).
pub struct LiteralIndex {
    /// `IdClause`s stored in the index.
    clauses: HashMap<ClauseId, IdClause>,
    /// Feature vector tree for stored clauses (hierarchical Schulz 2002 subsumption trie).
    fvt: FeatureVectorTree,
    /// Maps clause ID to its feature vector for fast O(1) removal.
    fv_map: HashMap<ClauseId, FeatureVector>,
    /// Maps (predicate, polarity) -> STree of clause IDs.
    pred_index: HashMap<LitKey, crate::stree::STreeId<ClauseId>>,
    /// Clause IDs that contain at least one positive equality.
    pos_eq_clauses: HashSet<ClauseId>,
    /// Clause IDs that contain at least one negative equality.
    neg_eq_clauses: HashSet<ClauseId>,
    /// Index of non-variable positive equality LHS/RHS terms -> clause IDs.
    pos_eq_lhs_index: crate::stree::STreeId<ClauseId>,
    /// Index of all non-variable subterms -> clause IDs.
    subterm_index: crate::stree::STreeId<ClauseId>,
}

impl LiteralIndex {
    /// Creates an empty literal index.
    pub fn new() -> Self {
        LiteralIndex {
            clauses: HashMap::default(),
            fvt: FeatureVectorTree::new(),
            fv_map: HashMap::default(),
            pred_index: HashMap::default(),
            pos_eq_clauses: HashSet::default(),
            neg_eq_clauses: HashSet::default(),
            pos_eq_lhs_index: crate::stree::STreeId::new(),
            subterm_index: crate::stree::STreeId::new(),
        }
    }

    /// Inserts an `IdClause` into the index.
    pub fn insert(&mut self, clause: IdClause, bank: &TermBank) {
        let id = clause.id;
        let fv = FeatureVector::from_id_clause(&clause, bank);
        self.fvt.insert(id, fv);
        self.fv_map.insert(id, fv);

        for lit in &clause.literals {
            match &lit.atom {
                IdAtom::Pred(sym, args) => {
                    self.pred_index
                        .entry(LitKey {
                            pred: *sym,
                            positive: lit.positive,
                        })
                        .or_default()
                        .insert_atom(&lit.atom, bank, id);
                    for &arg in args {
                        for subterm in bank.non_variable_subterms(arg) {
                            self.subterm_index.insert(subterm, bank, id);
                        }
                    }
                }
                IdAtom::Eq(l, r) => {
                    if lit.positive {
                        self.pos_eq_clauses.insert(id);
                        if !matches!(bank.get(*l), mrs_core::term_bank::TermNode::Var(_)) {
                            self.pos_eq_lhs_index.insert(*l, bank, id);
                        }
                        if !matches!(bank.get(*r), mrs_core::term_bank::TermNode::Var(_)) {
                            self.pos_eq_lhs_index.insert(*r, bank, id);
                        }
                    } else {
                        self.neg_eq_clauses.insert(id);
                    }
                    for subterm in bank.non_variable_subterms(*l) {
                        self.subterm_index.insert(subterm, bank, id);
                    }
                    for subterm in bank.non_variable_subterms(*r) {
                        self.subterm_index.insert(subterm, bank, id);
                    }
                }
            }
        }
        self.clauses.insert(id, clause);
    }

    /// Removes a clause from the index by ID. Returns the removed clause if found.
    pub fn remove(&mut self, id: ClauseId, bank: &TermBank) -> Option<IdClause> {
        if let Some(fv) = self.fv_map.remove(&id) {
            self.fvt.remove(id, &fv);
        }
        if let Some(clause) = self.clauses.remove(&id) {
            for lit in &clause.literals {
                match &lit.atom {
                    IdAtom::Pred(sym, args) => {
                        if let Some(tree) = self.pred_index.get_mut(&LitKey {
                            pred: *sym,
                            positive: lit.positive,
                        }) {
                            tree.remove_atom(&lit.atom, bank, &id);
                        }
                        for &arg in args {
                            for subterm in bank.non_variable_subterms(arg) {
                                self.subterm_index.remove(subterm, bank, &id);
                            }
                        }
                    }
                    IdAtom::Eq(l, r) => {
                        if lit.positive {
                            self.pos_eq_clauses.remove(&id);
                            if !matches!(bank.get(*l), mrs_core::term_bank::TermNode::Var(_)) {
                                self.pos_eq_lhs_index.remove(*l, bank, &id);
                            }
                            if !matches!(bank.get(*r), mrs_core::term_bank::TermNode::Var(_)) {
                                self.pos_eq_lhs_index.remove(*r, bank, &id);
                            }
                        } else {
                            self.neg_eq_clauses.remove(&id);
                        }
                        for subterm in bank.non_variable_subterms(*l) {
                            self.subterm_index.remove(subterm, bank, &id);
                        }
                        for subterm in bank.non_variable_subterms(*r) {
                            self.subterm_index.remove(subterm, bank, &id);
                        }
                    }
                }
            }
            Some(clause)
        } else {
            None
        }
    }

    /// Returns cloned `IdClause`s that have a literal with the given predicate
    /// and *complementary* polarity, whose arguments are unifiable with the query.
    ///
    /// Returns owned `IdClause` values (cheap: fields are vectors of `u32` handles)
    /// so callers can freely use `&mut TermBank` for inference without borrow conflicts.
    pub fn get_unifiable_resolution_partners(
        &self,
        atom: &IdAtom,
        positive: bool,
        bank: &TermBank,
    ) -> Vec<IdClause> {
        if let IdAtom::Pred(sym, _args) = atom {
            let key = LitKey {
                pred: *sym,
                positive: !positive,
            };
            if let Some(tree) = self.pred_index.get(&key) {
                let mut ids = tree.get_unifications_atom(atom, bank);
                ids.sort_unstable();
                ids.dedup();
                return ids
                    .into_iter()
                    .filter_map(|id| self.clauses.get(&id).cloned())
                    .collect();
            }
        }
        Vec::new()
    }

    /// Returns all clauses that contain a positive equality literal (cloned).
    pub fn get_positive_equality_clauses(&self) -> Vec<IdClause> {
        let mut res: Vec<IdClause> = self
            .pos_eq_clauses
            .iter()
            .filter_map(|id| self.clauses.get(id).cloned())
            .collect();
        res.sort_unstable_by_key(|c| c.id);
        res
    }

    /// Returns clauses that could potentially subsume the target (cloned).
    pub fn get_subsumption_candidates(&self, target_fv: &FeatureVector) -> Vec<IdClause> {
        let mut candidate_ids = Vec::new();
        self.fvt.query_subsumers(target_fv, &mut candidate_ids);
        let mut res: Vec<IdClause> = candidate_ids
            .into_iter()
            .filter_map(|id| self.clauses.get(&id).cloned())
            .collect();
        res.sort_unstable_by_key(|c| c.id);
        res
    }

    /// Returns clauses that could potentially BE subsumed by the given clause (cloned).
    pub fn get_subsumed_candidates(&self, subsumer_fv: &FeatureVector) -> Vec<IdClause> {
        let mut candidate_ids = Vec::new();
        self.fvt.query_subsumed(subsumer_fv, &mut candidate_ids);
        let mut res: Vec<IdClause> = candidate_ids
            .into_iter()
            .filter_map(|id| self.clauses.get(&id).cloned())
            .collect();
        res.sort_unstable_by_key(|c| c.id);
        res
    }

    /// Returns clauses that could potentially subsumption-resolve the target (cloned).
    pub fn get_subsumption_resolution_candidates(
        &self,
        target_fv: &FeatureVector,
    ) -> Vec<IdClause> {
        let mut candidate_ids = Vec::new();
        self.fvt
            .query_subsumption_resolution(target_fv, &mut candidate_ids);
        let mut res: Vec<IdClause> = candidate_ids
            .into_iter()
            .filter_map(|id| self.clauses.get(&id).cloned())
            .collect();
        res.sort_unstable_by_key(|c| c.id);
        res
    }

    /// Returns clauses in the index that could potentially BE subsumption-resolved by `simplifier_fv` (cloned).
    pub fn get_backward_subsumption_resolution_candidates(
        &self,
        simplifier_fv: &FeatureVector,
    ) -> Vec<IdClause> {
        let mut candidate_ids = Vec::new();
        self.fvt
            .query_backward_subsumption_resolution(simplifier_fv, &mut candidate_ids);
        let mut res: Vec<IdClause> = candidate_ids
            .into_iter()
            .filter_map(|id| self.clauses.get(&id).cloned())
            .collect();
        res.sort_unstable_by_key(|c| c.id);
        res
    }

    /// Returns a reference to a specific clause by ID.
    pub fn get(&self, id: ClauseId) -> Option<&IdClause> {
        self.clauses.get(&id)
    }

    /// Returns an iterator over all stored clauses.
    pub fn iter(&self) -> impl Iterator<Item = &IdClause> {
        self.clauses.values()
    }

    /// Returns the number of stored clauses.
    pub fn len(&self) -> usize {
        self.clauses.len()
    }

    /// Returns true if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.clauses.is_empty()
    }

    /// Returns candidate target clauses that contain at least one non-variable subterm
    /// unifiable with `term` (e.g. for superposition from an equality side `term` into active clauses).
    pub fn get_superposition_targets(&self, term: TermId, bank: &TermBank) -> Vec<IdClause> {
        let mut ids = self.subterm_index.get_unifications(term, bank);
        ids.sort_unstable();
        ids.dedup();
        ids.into_iter()
            .filter_map(|id| self.clauses.get(&id).cloned())
            .collect()
    }

    /// Returns candidate equation clauses that contain a positive equality whose LHS or RHS
    /// is unifiable with `query_subterm` (e.g. for superposition from active positive equalities into `query_subterm`).
    pub fn get_superposition_sources(
        &self,
        query_subterm: TermId,
        bank: &TermBank,
    ) -> Vec<IdClause> {
        let mut ids = self.pos_eq_lhs_index.get_unifications(query_subterm, bank);
        ids.sort_unstable();
        ids.dedup();
        ids.into_iter()
            .filter_map(|id| self.clauses.get(&id).cloned())
            .collect()
    }

    /// Drains all clauses from the index, returning them as a Vec.
    pub fn drain(&mut self) -> Vec<IdClause> {
        self.pred_index.clear();
        self.pos_eq_clauses.clear();
        self.neg_eq_clauses.clear();
        self.pos_eq_lhs_index = crate::stree::STreeId::new();
        self.subterm_index = crate::stree::STreeId::new();
        self.fvt.clear();
        self.fv_map.clear();
        self.clauses.drain().map(|(_, c)| c).collect()
    }

    /// Retains only clauses satisfying the predicate. Rebuilds the index.
    pub fn retain<F: FnMut(&IdClause) -> bool>(&mut self, mut f: F, bank: &TermBank) {
        let to_remove: Vec<ClauseId> = self
            .clauses
            .values()
            .filter(|c| !f(c))
            .map(|c| c.id)
            .collect();
        for id in to_remove {
            self.remove(id, bank);
        }
    }
}

impl Default for LiteralIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::SymbolTable;
    use mrs_core::clause::{ClauseId, ClauseSource};
    use mrs_core::term_bank::{IdAtom, IdLiteral, TermBank};
    use smallvec::smallvec;

    #[test]
    fn test_literal_index_subsumption_resolution_candidates() {
        let mut index = LiteralIndex::new();
        let mut bank = TermBank::new();
        let mut syms = SymbolTable::new();

        let p_sym = syms.intern("p");
        let q_sym = syms.intern("q");
        let a_sym = syms.intern("a");
        let a = bank.intern_app(a_sym, smallvec![]);

        let p_a = IdLiteral {
            positive: true,
            atom: IdAtom::Pred(p_sym, smallvec![a]),
        };
        let not_q_a = IdLiteral {
            positive: false,
            atom: IdAtom::Pred(q_sym, smallvec![a]),
        };

        let c1 = IdClause::new(
            ClauseId(1),
            vec![p_a.clone(), not_q_a.clone()],
            ClauseSource::Inference {
                rule: "input",
                parents: vec![].into(),
            },
        );
        index.insert(c1, &bank);

        let q_a = IdLiteral {
            positive: true,
            atom: IdAtom::Pred(q_sym, smallvec![a]),
        };
        let c2 = IdClause::new(
            ClauseId(2),
            vec![q_a],
            ClauseSource::Inference {
                rule: "input",
                parents: vec![].into(),
            },
        );

        let c2_fv = FeatureVector::from_id_clause(&c2, &bank);
        let backward_candidates = index.get_backward_subsumption_resolution_candidates(&c2_fv);
        assert_eq!(backward_candidates.len(), 1);
        assert_eq!(backward_candidates[0].id, ClauseId(1));
    }

    #[test]
    fn test_literal_index_superposition() {
        let mut index = LiteralIndex::new();
        let mut bank = TermBank::new();
        let mut syms = SymbolTable::new();

        let f_sym = syms.intern("f");
        let a_sym = syms.intern("a");
        let b_sym = syms.intern("b");

        let a = bank.intern_app(a_sym, smallvec![]);
        let b = bank.intern_app(b_sym, smallvec![]);
        let f_a = bank.intern_app(f_sym, smallvec![a]);

        // C1: f(a) = b
        let c1 = IdClause::new(
            ClauseId(1),
            vec![IdLiteral {
                positive: true,
                atom: IdAtom::Eq(f_a, b),
            }],
            ClauseSource::Inference {
                rule: "input",
                parents: vec![].into(),
            },
        );

        // C2: p(f(a))
        let p_sym = syms.intern("p");
        let c2 = IdClause::new(
            ClauseId(2),
            vec![IdLiteral {
                positive: true,
                atom: IdAtom::Pred(p_sym, smallvec![f_a]),
            }],
            ClauseSource::Inference {
                rule: "input",
                parents: vec![].into(),
            },
        );

        index.insert(c1, &bank);
        index.insert(c2, &bank);

        // Query targets for f(a) -> should find C2 (and C1)
        let targets = index.get_superposition_targets(f_a, &bank);
        assert!(targets.iter().any(|c| c.id == ClauseId(2)));

        // Query sources for f(a) -> should find C1
        let sources = index.get_superposition_sources(f_a, &bank);
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].id, ClauseId(1));
    }

    #[test]
    fn test_literal_index_subsumption_candidates() {
        let mut index = LiteralIndex::new();
        let mut bank = TermBank::new();
        let mut syms = SymbolTable::new();

        let p_sym = syms.intern("p");
        let q_sym = syms.intern("q");
        let a_sym = syms.intern("a");
        let b_sym = syms.intern("b");

        let a = bank.intern_app(a_sym, smallvec![]);
        let b = bank.intern_app(b_sym, smallvec![]);

        // C1: p(a)  (1 literal, 1 pos, 0 neg)
        let c1 = IdClause::new(
            ClauseId(1),
            vec![IdLiteral {
                positive: true,
                atom: IdAtom::Pred(p_sym, smallvec![a]),
            }],
            ClauseSource::Inference {
                rule: "input",
                parents: vec![].into(),
            },
        );

        // C2: p(a) | q(b)  (2 literals, 2 pos, 0 neg)
        let c2 = IdClause::new(
            ClauseId(2),
            vec![
                IdLiteral {
                    positive: true,
                    atom: IdAtom::Pred(p_sym, smallvec![a]),
                },
                IdLiteral {
                    positive: true,
                    atom: IdAtom::Pred(q_sym, smallvec![b]),
                },
            ],
            ClauseSource::Inference {
                rule: "input",
                parents: vec![].into(),
            },
        );

        index.insert(c1.clone(), &bank);
        index.insert(c2.clone(), &bank);

        let c1_fv = FeatureVector::from_id_clause(&c1, &bank);
        let c2_fv = FeatureVector::from_id_clause(&c2, &bank);

        // C1 can subsume C2:
        // When checking subsumers of C2, C1 should be found:
        let subsumers_of_c2 = index.get_subsumption_candidates(&c2_fv);
        assert!(subsumers_of_c2.iter().any(|c| c.id == ClauseId(1)));

        // When checking what C1 can subsume, C2 should be found:
        let subsumed_by_c1 = index.get_subsumed_candidates(&c1_fv);
        assert!(subsumed_by_c1.iter().any(|c| c.id == ClauseId(2)));

        // Removal test
        index.remove(ClauseId(1), &bank);
        let subsumers_after = index.get_subsumption_candidates(&c2_fv);
        assert!(!subsumers_after.iter().any(|c| c.id == ClauseId(1)));
    }
}
