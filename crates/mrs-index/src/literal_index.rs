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

use std::collections::{HashMap, HashSet};

use mrs_core::clause::ClauseId;
use mrs_core::symbol::SymbolId;
use mrs_core::term_bank::{IdAtom, IdClause, TermBank};

use crate::fvi::FeatureVector;

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
    /// Feature vectors for stored clauses (used for subsumption filtering).
    fvs: HashMap<ClauseId, FeatureVector>,
    /// Maps (predicate, polarity) -> STree of clause IDs.
    pred_index: HashMap<LitKey, crate::stree::STreeId<ClauseId>>,
    /// Clause IDs that contain at least one positive equality.
    pos_eq_clauses: HashSet<ClauseId>,
    /// Clause IDs that contain at least one negative equality.
    neg_eq_clauses: HashSet<ClauseId>,
}

impl LiteralIndex {
    /// Creates an empty literal index.
    pub fn new() -> Self {
        LiteralIndex {
            clauses: HashMap::new(),
            fvs: HashMap::new(),
            pred_index: HashMap::new(),
            pos_eq_clauses: HashSet::new(),
            neg_eq_clauses: HashSet::new(),
        }
    }

    /// Inserts an `IdClause` into the index.
    pub fn insert(&mut self, clause: IdClause, bank: &TermBank) {
        let id = clause.id;
        self.fvs
            .insert(id, FeatureVector::from_id_clause(&clause, bank));

        for lit in &clause.literals {
            match &lit.atom {
                IdAtom::Pred(sym, _args) => {
                    self.pred_index
                        .entry(LitKey {
                            pred: *sym,
                            positive: lit.positive,
                        })
                        .or_default()
                        .insert_atom(&lit.atom, bank, id);
                }
                IdAtom::Eq(_, _) => {
                    if lit.positive {
                        self.pos_eq_clauses.insert(id);
                    } else {
                        self.neg_eq_clauses.insert(id);
                    }
                }
            }
        }
        self.clauses.insert(id, clause);
    }

    /// Removes a clause from the index by ID. Returns the removed clause if found.
    pub fn remove(&mut self, id: ClauseId, bank: &TermBank) -> Option<IdClause> {
        self.fvs.remove(&id);
        if let Some(clause) = self.clauses.remove(&id) {
            for lit in &clause.literals {
                match &lit.atom {
                    IdAtom::Pred(sym, _args) => {
                        if let Some(tree) = self.pred_index.get_mut(&LitKey {
                            pred: *sym,
                            positive: lit.positive,
                        }) {
                            tree.remove_atom(&lit.atom, bank, &id);
                        }
                    }
                    IdAtom::Eq(_, _) => {
                        if lit.positive {
                            self.pos_eq_clauses.remove(&id);
                        } else {
                            self.neg_eq_clauses.remove(&id);
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
        self.pos_eq_clauses
            .iter()
            .filter_map(|id| self.clauses.get(id).cloned())
            .collect()
    }

    /// Returns clauses that could potentially subsume the target (cloned).
    pub fn get_subsumption_candidates(&self, target_fv: &FeatureVector) -> Vec<IdClause> {
        self.clauses
            .iter()
            .filter(|(id, _)| self.fvs.get(*id).unwrap().can_subsume(target_fv))
            .map(|(_, c)| c.clone())
            .collect()
    }

    /// Returns clauses that could potentially BE subsumed by the given clause (cloned).
    pub fn get_subsumed_candidates(&self, subsumer_fv: &FeatureVector) -> Vec<IdClause> {
        self.clauses
            .iter()
            .filter(|(id, _)| subsumer_fv.can_subsume(self.fvs.get(*id).unwrap()))
            .map(|(_, c)| c.clone())
            .collect()
    }

    /// Returns clauses that could potentially subsumption-resolve the target (cloned).
    pub fn get_subsumption_resolution_candidates(
        &self,
        target_fv: &FeatureVector,
    ) -> Vec<IdClause> {
        self.clauses
            .iter()
            .filter(|(id, _)| {
                self.fvs
                    .get(*id)
                    .unwrap()
                    .can_subsumption_resolve(target_fv)
            })
            .map(|(_, c)| c.clone())
            .collect()
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

    /// Drains all clauses from the index, returning them as a Vec.
    pub fn drain(&mut self) -> Vec<IdClause> {
        self.pred_index.clear();
        self.pos_eq_clauses.clear();
        self.neg_eq_clauses.clear();
        self.fvs.clear();
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
