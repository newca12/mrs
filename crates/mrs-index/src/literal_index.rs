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
    /// Maps (predicate, polarity) -> set of clause IDs.
    pred_index: HashMap<LitKey, HashSet<ClauseId>>,
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
            pred_index: HashMap::new(),
            pos_eq_clauses: HashSet::new(),
            neg_eq_clauses: HashSet::new(),
        }
    }

    /// Inserts a clause into the index.
    pub fn insert(&mut self, clause: Clause) {
        let id = clause.id;
        for lit in &clause.literals {
            match &lit.atom {
                Atom::Pred(sym, _) => {
                    self.pred_index
                        .entry(LitKey {
                            pred: *sym,
                            positive: lit.positive,
                        })
                        .or_default()
                        .insert(id);
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
        if let Some(clause) = self.clauses.remove(&id) {
            for lit in &clause.literals {
                match &lit.atom {
                    Atom::Pred(sym, _) => {
                        if let Some(set) = self.pred_index.get_mut(&LitKey {
                            pred: *sym,
                            positive: lit.positive,
                        }) {
                            set.remove(&id);
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
    /// the *complementary* polarity. These are the resolution candidates.
    pub fn get_resolution_partners(&self, pred: SymbolId, positive: bool) -> Vec<&Clause> {
        let key = LitKey {
            pred,
            positive: !positive,
        };
        match self.pred_index.get(&key) {
            Some(ids) => ids.iter().filter_map(|id| self.clauses.get(id)).collect(),
            None => Vec::new(),
        }
    }

    /// Returns all clauses that contain a positive equality literal.
    /// These are candidates for superposition "from" (using equalities as rewrite rules).
    pub fn get_positive_equality_clauses(&self) -> Vec<&Clause> {
        self.pos_eq_clauses
            .iter()
            .filter_map(|id| self.clauses.get(id))
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
        self.clauses.drain().map(|(_, c)| c).collect()
    }

    /// Retains only clauses satisfying the predicate. Rebuilds the index.
    pub fn retain<F: Fn(&Clause) -> bool>(&mut self, f: F) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::clause::{ClauseIdGen, ClauseSource};
    use mrs_core::{Literal, SymbolTable, Term};

    fn make_clause(id_gen: &mut ClauseIdGen, lits: Vec<Literal>) -> Clause {
        Clause::new(
            id_gen.next(),
            lits,
            ClauseSource::Input {
                name: "test".into(),
                role: "axiom".into(),
            },
        )
    }

    #[test]
    fn resolution_partners_found() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();
        let mut idx = LiteralIndex::new();

        // Insert clause with +p(a)
        let c1 = make_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
        );
        let c1_id = c1.id;
        idx.insert(c1);

        // Look for resolution partners of -p(X) → should find c1
        let partners = idx.get_resolution_partners(p, false);
        assert_eq!(partners.len(), 1);
        assert_eq!(partners[0].id, c1_id);
    }

    #[test]
    fn no_partners_for_same_polarity() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();
        let mut idx = LiteralIndex::new();

        let c1 = make_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
        );
        idx.insert(c1);

        // Same polarity → no partners
        let partners = idx.get_resolution_partners(p, true);
        assert!(partners.is_empty());
    }

    #[test]
    fn remove_updates_index() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();
        let mut idx = LiteralIndex::new();

        let c1 = make_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
        );
        let c1_id = c1.id;
        idx.insert(c1);

        assert_eq!(idx.get_resolution_partners(p, false).len(), 1);
        idx.remove(c1_id);
        assert!(idx.get_resolution_partners(p, false).is_empty());
    }

    #[test]
    fn equality_clauses_tracked() {
        let mut syms = SymbolTable::new();
        let a = syms.intern("a");
        let b = syms.intern("b");
        let mut id_gen = ClauseIdGen::new();
        let mut idx = LiteralIndex::new();

        let c1 = make_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::Eq(Term::constant(a), Term::constant(b)))],
        );
        idx.insert(c1);

        assert_eq!(idx.get_positive_equality_clauses().len(), 1);
    }
}
