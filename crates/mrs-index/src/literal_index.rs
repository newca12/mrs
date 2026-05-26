//! Clause literal index by predicate symbol.
//!
//! Maps `(predicate_symbol, polarity)` to clause IDs that contain a literal
//! with that predicate and polarity. This dramatically reduces the number
//! of clause pairs considered during resolution: instead of trying every
//! processed clause, only those with a complementary predicate are examined.

use std::collections::{HashMap, HashSet};

use mrs_core::Atom;
use mrs_core::clause::{Clause, ClauseId};
use mrs_core::symbol::SymbolId;
use mrs_core::term::Term;

use crate::dtree::DTree;
use crate::fvi::FeatureVector;

/// Key for the literal index: a predicate symbol with a polarity.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
struct LitKey {
    pred: SymbolId,
    positive: bool,
}

/// An index mapping predicate symbols (with polarity) to clause IDs.
///
/// Maintains a secondary index alongside the processed clause set for
/// fast lookup of resolution/superposition candidates.
pub struct LiteralIndex {
    /// Clauses stored in the index.
    clauses: HashMap<ClauseId, Clause>,
    /// Feature vectors for stored clauses (used for subsumption filtering).
    fvs: HashMap<ClauseId, FeatureVector>,
    /// Maps (predicate, polarity) -> DTree of clause IDs.
    pred_index: HashMap<LitKey, DTree<ClauseId>>,
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

    /// Inserts a clause into the index.
    pub fn insert(&mut self, clause: Clause) {
        let id = clause.id;
        self.fvs.insert(id, FeatureVector::from_clause(&clause));
        
        for lit in &clause.literals {
            match &lit.atom {
                Atom::Pred(sym, args) => {
                    let term = Term::App(*sym, args.clone());
                    self.pred_index
                        .entry(LitKey {
                            pred: *sym,
                            positive: lit.positive,
                        })
                        .or_default()
                        .insert(&term, id);
                }
                Atom::Eq(_, _) => {
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
    pub fn remove(&mut self, id: ClauseId) -> Option<Clause> {
        self.fvs.remove(&id);
        if let Some(clause) = self.clauses.remove(&id) {
            for lit in &clause.literals {
                match &lit.atom {
                    Atom::Pred(sym, args) => {
                        if let Some(tree) = self.pred_index.get_mut(&LitKey {
                            pred: *sym,
                            positive: lit.positive,
                        }) {
                            let term = Term::App(*sym, args.clone());
                            tree.remove(&term, &id);
                        }
                    }
                    Atom::Eq(_, _) => {
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

    /// Returns clauses that have a literal with the given predicate symbol and
    /// the *complementary* polarity, and whose arguments are unifiable with the query.
    pub fn get_unifiable_resolution_partners(&self, atom: &Atom, positive: bool) -> Vec<&Clause> {
        if let Atom::Pred(sym, args) = atom {
            let key = LitKey {
                pred: *sym,
                positive: !positive,
            };
            if let Some(tree) = self.pred_index.get(&key) {
                let term = Term::App(*sym, args.clone());
                let mut ids = tree.get_unifiable(&term);
                ids.sort_unstable();
                ids.dedup();
                return ids.into_iter().filter_map(|id| self.clauses.get(&id)).collect();
            }
        }
        Vec::new()
    }

    /// Returns all clauses that contain a positive equality literal.
    /// These are candidates for superposition "from" (using equalities as rewrite rules).
    pub fn get_positive_equality_clauses(&self) -> Vec<&Clause> {
        self.pos_eq_clauses
            .iter()
            .filter_map(|id| self.clauses.get(id))
            .collect()
    }
    
    /// Returns clauses from the index that could potentially subsume the target clause,
    /// based on feature vector filtering.
    pub fn get_subsumption_candidates(&self, target_fv: &FeatureVector) -> Vec<&Clause> {
        self.clauses
            .iter()
            .filter(|(id, _)| self.fvs.get(*id).unwrap().can_subsume(target_fv))
            .map(|(_, c)| c)
            .collect()
    }

    /// Returns clauses from the index that could potentially BE subsumed by the given clause,
    /// based on feature vector filtering.
    pub fn get_subsumed_candidates(&self, subsumer_fv: &FeatureVector) -> Vec<&Clause> {
        self.clauses
            .iter()
            .filter(|(id, _)| subsumer_fv.can_subsume(self.fvs.get(*id).unwrap()))
            .map(|(_, c)| c)
            .collect()
    }

    /// Returns a specific clause by ID.
    pub fn get(&self, id: ClauseId) -> Option<&Clause> {
        self.clauses.get(&id)
    }

    /// Returns an iterator over all stored clauses.
    pub fn iter(&self) -> impl Iterator<Item = &Clause> {
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
    pub fn drain(&mut self) -> Vec<Clause> {
        self.pred_index.clear();
        self.pos_eq_clauses.clear();
        self.neg_eq_clauses.clear();
        self.fvs.clear();
        self.clauses.drain().map(|(_, c)| c).collect()
    }

    /// Retains only clauses satisfying the predicate. Rebuilds the index.
    pub fn retain<F: FnMut(&Clause) -> bool>(&mut self, mut f: F) {
        let to_remove: Vec<ClauseId> = self
            .clauses
            .values()
            .filter(|c| !f(c))
            .map(|c| c.id)
            .collect();
        for id in to_remove {
            self.remove(id);
        }
    }
}

impl Default for LiteralIndex {
    fn default() -> Self {
        Self::new()
    }
}

