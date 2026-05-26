use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet, VecDeque};

use mrs_core::clause::{Clause, ClauseId};

use crate::weight::clause_weight;

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
    /// Queue ordered by arrival (age).
    age_queue: VecDeque<ClauseId>,
    /// Priority queue ordered by weight (lightest first).
    weight_queue: BinaryHeap<WeightWrapper>,
}

impl UnprocessedSet {
    /// Creates a new, empty unprocessed set.
    pub fn new() -> Self {
        Self {
            active_ids: HashSet::new(),
            age_queue: VecDeque::new(),
            weight_queue: BinaryHeap::new(),
        }
    }

    /// Adds a clause to the unprocessed set.
    pub fn push(&mut self, clause: &Clause) {
        let id = clause.id;
        let weight = clause_weight(clause);
        self.active_ids.insert(id);
        self.age_queue.push_back(id);
        self.weight_queue.push(WeightWrapper { id, weight });
    }

    /// Returns `true` if there are no clauses in the set.
    pub fn is_empty(&self) -> bool {
        self.active_ids.is_empty()
    }

    /// Pops the oldest clause from the set, returning its ID.
    pub fn pop_age(&mut self) -> Option<ClauseId> {
        while let Some(id) = self.age_queue.pop_front() {
            if self.active_ids.remove(&id) {
                return Some(id);
            }
        }
        None
    }

    /// Pops the lightest clause from the set, returning its ID.
    pub fn pop_weight(&mut self) -> Option<ClauseId> {
        while let Some(wrapper) = self.weight_queue.pop() {
            if self.active_ids.remove(&wrapper.id) {
                return Some(wrapper.id);
            }
        }
        None
    }

    /// Retains only the clauses specified by the predicate.
    /// Clauses that fail the predicate are marked for lazy deletion.
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(ClauseId) -> bool,
    {
        self.active_ids.retain(|&id| f(id));
    }

    /// Returns an iterator over the active clause IDs.
    pub fn iter(&self) -> impl Iterator<Item = ClauseId> + '_ {
        self.active_ids.iter().copied()
    }
}

impl Default for UnprocessedSet {
    fn default() -> Self {
        Self::new()
    }
}
