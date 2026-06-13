use crate::{HashMap, HashSet};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, VecDeque};
use std::sync::Arc;

use mrs_calculus::ordering::SymbolConfig;
use mrs_core::clause::ClauseId;
use mrs_core::term_bank::{IdClause, TermBank};
use mrs_index::fvi::FeatureVector;

#[derive(Clone, Debug)]
struct WeightWrapper {
    id: ClauseId,
    weight: u32,
    distance: u32,
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
    /// Priority queue ordered by ML-guided score + weight.
    #[cfg(feature = "ml-guidance")]
    ml_queue: BinaryHeap<WeightWrapper>,
    /// Configuration for symbol precedence and weights.
    /// Retained for future use (e.g. adaptive weight re-scoring).
    #[allow(dead_code)]
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
            #[cfg(feature = "ml-guidance")]
            ml_queue: BinaryHeap::new(),
            config,
        }
    }

    /// Adds an `IdClause` to the unprocessed set.
    ///
    /// `weight` is the precomputed clause weight (using the strategy's chosen
    /// weight function).  The caller is responsible for computing it.
    ///
    /// `ml_score` is the raw logit from the ML clause classifier; it only
    /// affects the ML priority queue (`ml-guidance` feature). In the default
    /// build the parameter is ignored and costs nothing.
    pub fn push(&mut self, clause: &IdClause, bank: &TermBank, weight: u32, ml_score: Option<f32>) {
        #[cfg(not(feature = "ml-guidance"))]
        let _ = ml_score;
        let id = clause.id;

        let goal_weight = if clause.distance < 100 {
            weight + (clause.distance * 2)
        } else {
            weight + 1000 // heavy penalty for pure axioms
        };

        self.active_ids.insert(id);
        self.fvs
            .insert(id, FeatureVector::from_id_clause(clause, bank));
        self.age_queue.push_back(id);
        self.weight_queue.push(WeightWrapper {
            id,
            weight,
            distance: clause.distance,
        });
        self.goal_queue.push(WeightWrapper {
            id,
            weight: goal_weight,
            distance: clause.distance,
        });
        #[cfg(feature = "ml-guidance")]
        {
            // ML priority = α * norm(weight) + (1 - α) * (1 - σ(score))
            // We use α = 0.3, K = 20 for normalization.
            let ml_priority = if let Some(score) = ml_score {
                let alpha = 0.3;
                let norm_weight = weight as f32 / (weight as f32 + 20.0);
                let sigmoid = 1.0 / (1.0 + (-score).exp());
                let priority_f32 = alpha * norm_weight + (1.0 - alpha) * (1.0 - sigmoid);
                (priority_f32 * 1_000_000.0) as u32
            } else {
                weight // Fallback
            };
            self.ml_queue.push(WeightWrapper {
                id,
                weight: ml_priority,
                distance: clause.distance,
            });
        }
    }

    /// Returns `true` if there are no clauses in the set.
    pub fn is_empty(&self) -> bool {
        self.active_ids.is_empty()
    }

    /// Returns the number of clauses currently in the unprocessed set.
    pub fn active_count(&self) -> usize {
        self.active_ids.len()
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

    /// Pops the lightest SOS-eligible clause (distance < `sos_depth`).
    ///
    /// Skips clauses whose distance exceeds `sos_depth`, falling back to
    /// `pop_age()` if no SOS clause is ready.  This implements the
    /// Set-of-Support restriction: the weight-based pick only considers
    /// goal-connected clauses; all clauses remain reachable via the age queue.
    pub fn pop_weight_sos(&mut self, sos_depth: u32) -> Option<ClauseId> {
        // Drain until we find an active SOS-eligible clause.
        // Non-SOS clauses that are active are put back into a temporary
        // buffer and re-inserted after the search.
        let mut skipped: Vec<WeightWrapper> = Vec::new();
        let result = loop {
            match self.weight_queue.pop() {
                None => break None,
                Some(wrapper) => {
                    if !self.active_ids.contains(&wrapper.id) {
                        // Tombstone — skip without re-inserting.
                        continue;
                    }
                    if wrapper.distance < sos_depth {
                        self.active_ids.remove(&wrapper.id);
                        self.fvs.remove(&wrapper.id);
                        break Some(wrapper.id);
                    } else {
                        skipped.push(wrapper);
                        // Stop after examining a bounded window to avoid O(n) scan.
                        if skipped.len() >= 32 {
                            break None;
                        }
                    }
                }
            }
        };
        // Re-insert skipped clauses.
        for w in skipped {
            self.weight_queue.push(w);
        }
        result
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

    /// Pops the clause with the lowest ML priority.
    #[cfg(feature = "ml-guidance")]
    pub fn pop_ml(&mut self) -> Option<ClauseId> {
        while let Some(wrapper) = self.ml_queue.pop() {
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
