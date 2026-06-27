use crate::HashSet;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, VecDeque};
use std::sync::Arc;

use mrs_calculus::ordering::SymbolConfig;
use mrs_core::clause::ClauseId;
use mrs_core::term_bank::IdClause;

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
    pub fn push(
        &mut self,
        clause: &IdClause,
        _bank: &mrs_core::term_bank::TermBank,
        weight: u32,
        ml_score: Option<f32>,
    ) {
        #[cfg(not(feature = "ml-guidance"))]
        let _ = ml_score;
        let id = clause.id;

        let goal_weight = if clause.distance < 100 {
            weight + (clause.distance * 2)
        } else {
            weight + 1000 // heavy penalty for pure axioms
        };

        self.active_ids.insert(id);
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
                return Some(wrapper.id);
            }
        }
        None
    }

    /// Removes a specific clause by ID from the unprocessed set.
    /// Does not physically remove it from the priority queues (lazy deletion),
    /// but removes it from `active_ids` so it will be ignored when popped.
    pub fn remove(&mut self, id: ClauseId) -> bool {
        self.active_ids.remove(&id)
    }

    /// Removes clauses that do not satisfy the predicate `f`.
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(ClauseId) -> bool,
    {
        let mut to_remove = Vec::new();
        for &id in &self.active_ids {
            if !f(id) {
                to_remove.push(id);
            }
        }
        for id in to_remove {
            self.active_ids.remove(&id);
        }
    }

    /// Returns an iterator over the IDs of the currently active clauses.
    pub fn iter(&self) -> impl Iterator<Item = ClauseId> + '_ {
        self.active_ids.iter().copied()
    }

    /// Prunes the passive set to keep only the `target_size` lightest clauses by weight.
    /// Returns the number of discarded clauses.
    pub fn prune(&mut self, target_size: usize) -> usize {
        if self.active_ids.len() <= target_size {
            return 0;
        }

        // 1. Collect all active WeightWrappers from the weight_queue
        let mut active_wrappers = Vec::with_capacity(self.active_ids.len());
        let old_queue = std::mem::take(&mut self.weight_queue);
        for w in old_queue {
            if self.active_ids.contains(&w.id) {
                active_wrappers.push(w);
            }
        }

        // 2. Sort them by weight ascending (lightest first).
        // Since WeightWrapper has Ord implemented with reversed cmp, let's sort with actual weight.
        active_wrappers
            .sort_unstable_by(|a, b| a.weight.cmp(&b.weight).then_with(|| a.id.cmp(&b.id)));

        if active_wrappers.len() <= target_size {
            // Restore weight_queue and return
            self.weight_queue = BinaryHeap::from(active_wrappers);
            return 0;
        }

        let (kept, discarded) = active_wrappers.split_at(target_size);
        let num_discarded = discarded.len();

        // 3. Remove discarded IDs from active_ids
        for w in discarded {
            self.active_ids.remove(&w.id);
        }

        // 4. Filter age_queue in-place
        self.age_queue.retain(|id| self.active_ids.contains(id));

        // 5. Rebuild weight_queue
        // Since BinaryHeap is a max-heap but WeightWrapper's Ord is reversed,
        // we can just construct BinaryHeap from the kept wrappers!
        self.weight_queue = BinaryHeap::from(kept.to_vec());

        // 6. Rebuild goal_queue
        let goal_wrappers: Vec<WeightWrapper> = kept
            .iter()
            .map(|w| {
                let goal_weight = if w.distance < 100 {
                    w.weight.saturating_add(w.distance.saturating_mul(2))
                } else {
                    w.weight.saturating_add(1000)
                };
                WeightWrapper {
                    id: w.id,
                    weight: goal_weight,
                    distance: w.distance,
                }
            })
            .collect();
        self.goal_queue = BinaryHeap::from(goal_wrappers);

        // 7. Rebuild ml_queue if ml-guidance is enabled
        #[cfg(feature = "ml-guidance")]
        {
            let old_ml = std::mem::take(&mut self.ml_queue);
            let mut kept_ml = Vec::new();
            for w in old_ml {
                if self.active_ids.contains(&w.id) {
                    kept_ml.push(w);
                }
            }
            self.ml_queue = BinaryHeap::from(kept_ml);
        }

        num_discarded
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::clause::{Clause, ClauseId, ClauseSource};
    use mrs_core::term_bank::TermBank;

    #[test]
    fn test_unprocessed_pruning() {
        let config = Arc::new(SymbolConfig::default());
        let mut set = UnprocessedSet::new(config);
        let mut bank = TermBank::new();

        // Let's push 5 clauses with different weights
        let mut clauses = Vec::new();
        for i in 0..5 {
            let legacy = Clause::new(
                ClauseId(i),
                vec![],
                ClauseSource::Input {
                    name: "test".into(),
                    role: "axiom".into(),
                },
            );
            let id_clause = bank.clause_from_legacy(&legacy);
            clauses.push(id_clause);
        }

        // push clauses with weights:
        // c0 -> wt 10
        // c1 -> wt 50
        // c2 -> wt 5
        // c3 -> wt 100
        // c4 -> wt 20
        set.push(&clauses[0], &bank, 10, None);
        set.push(&clauses[1], &bank, 50, None);
        set.push(&clauses[2], &bank, 5, None);
        set.push(&clauses[3], &bank, 100, None);
        set.push(&clauses[4], &bank, 20, None);

        assert_eq!(set.active_count(), 5);

        // Pruning to target_size = 3
        // Sorted weights: c2 (5), c0 (10), c4 (20), c1 (50), c3 (100)
        // We expect c1 and c3 (heaviest) to be pruned!
        // So c2, c0, and c4 should be kept.
        let discarded = set.prune(3);
        assert_eq!(discarded, 2);
        assert_eq!(set.active_count(), 3);

        assert!(set.active_ids.contains(&clauses[2].id)); // kept (5)
        assert!(set.active_ids.contains(&clauses[0].id)); // kept (10)
        assert!(set.active_ids.contains(&clauses[4].id)); // kept (20)

        assert!(!set.active_ids.contains(&clauses[1].id)); // pruned (50)
        assert!(!set.active_ids.contains(&clauses[3].id)); // pruned (100)

        // Verify pop_weight retrieves them in order: c2, c0, c4
        assert_eq!(set.pop_weight(), Some(clauses[2].id));
        assert_eq!(set.pop_weight(), Some(clauses[0].id));
        assert_eq!(set.pop_weight(), Some(clauses[4].id));
        assert_eq!(set.pop_weight(), None);
    }

    #[test]
    #[cfg(feature = "ml-guidance")]
    fn test_ml_guided_priority_queuing() {
        use mrs_calculus::ordering::SymbolConfig;
        use mrs_core::SymbolTable;
        use mrs_core::clause::{Clause, ClauseSource};
        use std::sync::Arc;

        let mut bank = TermBank::new();
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");

        let mut make_id_clause = |id: u64| {
            let lits = vec![mrs_core::Literal::pos(mrs_core::Atom::pred(p, vec![]))];
            let clause = Clause::new(
                ClauseId(id),
                lits,
                ClauseSource::Input {
                    name: "test".into(),
                    role: "axiom".into(),
                },
            );
            bank.clause_from_legacy(&clause)
        };

        let c1 = make_id_clause(1);
        let c2 = make_id_clause(2);
        let c3 = make_id_clause(3);

        let mut set = UnprocessedSet::new(Arc::new(SymbolConfig::default()));

        // Push three clauses with different ML scores.
        // Higher score (logit) means more relevant, selected first (lower priority value in heap).
        set.push(&c1, &bank, 10, Some(-1.5)); // Low score
        set.push(&c2, &bank, 10, Some(2.0)); // High score (best)
        set.push(&c3, &bank, 10, Some(0.0)); // Medium score

        // We expect pop_ml() to return c2 first, then c3, then c1.
        assert_eq!(set.pop_ml(), Some(c2.id));
        assert_eq!(set.pop_ml(), Some(c3.id));
        assert_eq!(set.pop_ml(), Some(c1.id));
        assert_eq!(set.pop_ml(), None);
    }
}
