use crate::{HashMap, HashSet};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, VecDeque};
use std::sync::Arc;

use mrs_calculus::ordering::SymbolConfig;
use mrs_core::clause::ClauseId;
use mrs_core::term_bank::{IdClause, TermBank};
use mrs_index::fvi::FeatureVector;

use crate::weight::clause_weight_id;

#[derive(Clone, Debug)]
struct WeightWrapper {
    id: ClauseId,
    weight: u32,
}

impl PartialEq for WeightWrapper {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for WeightWrapper {}

impl PartialOrd for WeightWrapper {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for WeightWrapper {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse order so BinaryHeap is a min-heap
        other
            .weight
            .cmp(&self.weight)
            .then_with(|| other.id.cmp(&self.id))
    }
}

/// The set of unprocessed (passive) clauses.
/// Supports fast removal by age (FIFO) and weight (SmallestFirst),
/// using lazy deletion (tombstones).
pub struct UnprocessedSet {
    /// The IDs of clauses currently in the unprocessed set.
    active_ids: HashSet<ClauseId>,
    /// Feature vectors of active clauses, used for fast subsumption filtering.
    fvs: HashMap<ClauseId, FeatureVector>,
    /// Queue ordered by arrival (age).
    age_queue: VecDeque<ClauseId>,
    /// Priority queue ordered by weight (lightest first).
    weight_queue: BinaryHeap<WeightWrapper>,
    /// Priority queue ordered by distance to conjecture + weight.
    goal_queue: BinaryHeap<WeightWrapper>,
    /// Configuration for symbol precedence and weights.
    config: Arc<SymbolConfig>,
}

impl UnprocessedSet {
    /// Creates a new, empty unprocessed set.
    pub fn new(config: Arc<SymbolConfig>) -> Self {
        Self {
            active_ids: HashSet::default(),
            fvs: HashMap::default(),
            age_queue: VecDeque::new(),
            weight_queue: BinaryHeap::new(),
            goal_queue: BinaryHeap::new(),
            config,
        }
    }

    /// Adds an `IdClause` to the unprocessed set.
    pub fn push(&mut self, clause: &IdClause, bank: &TermBank) {
        let id = clause.id;
        let weight = clause_weight_id(clause, bank, &self.config);

        let goal_weight = if clause.distance < 100 {
            weight + (clause.distance * 2)
        } else {
            weight + 1000 // heavy penalty for pure axioms
        };

        self.active_ids.insert(id);
        self.fvs
            .insert(id, FeatureVector::from_id_clause(clause, bank));
        self.age_queue.push_back(id);
        self.weight_queue.push(WeightWrapper { id, weight });
        self.goal_queue.push(WeightWrapper {
            id,
            weight: goal_weight,
        });
    }

    /// Returns `true` if there are no clauses in the set.
    pub fn is_empty(&self) -> bool {
        self.active_ids.is_empty()
    }

    /// Pops the oldest clause from the set, returning its ID.
    pub fn pop_age(&mut self) -> Option<ClauseId> {
        while let Some(id) = self.age_queue.pop_front() {
            if self.active_ids.remove(&id) {
                self.fvs.remove(&id);
                return Some(id);
            }
        }
        None
    }

    /// Pops the lightest clause from the set, returning its ID.
    pub fn pop_weight(&mut self) -> Option<ClauseId> {
        while let Some(wrapper) = self.weight_queue.pop() {
            if self.active_ids.remove(&wrapper.id) {
                self.fvs.remove(&wrapper.id);
                return Some(wrapper.id);
            }
        }
        None
    }

    /// Pops the clause with the lowest distance-penalized weight.
    pub fn pop_goal_directed(&mut self) -> Option<ClauseId> {
        while let Some(wrapper) = self.goal_queue.pop() {
            if self.active_ids.remove(&wrapper.id) {
                self.fvs.remove(&wrapper.id);
                return Some(wrapper.id);
            }
        }
        None
    }

    /// Removes a specific clause by ID from the unprocessed set.
    /// Does not physically remove it from the priority queues (lazy deletion),
    /// but removes it from `active_ids` and `fvs` so it will be ignored when popped.
    pub fn remove(&mut self, id: ClauseId) -> bool {
        if self.active_ids.remove(&id) {
            self.fvs.remove(&id);
            true
        } else {
            false
        }
    }

    /// Removes clauses that do not satisfy the predicate `f`.
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(ClauseId, &FeatureVector) -> bool,
    {
        let mut to_remove = Vec::new();
        for &id in &self.active_ids {
            if let Some(fv) = self.fvs.get(&id)
                && !f(id, fv)
            {
                to_remove.push(id);
            }
        }
        for id in to_remove {
            self.active_ids.remove(&id);
            self.fvs.remove(&id);
        }
    }

    /// Returns an iterator over the IDs of the currently active clauses.
    pub fn iter(&self) -> impl Iterator<Item = ClauseId> + '_ {
        self.active_ids.iter().copied()
    }
}
