use crate::HashSet;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, VecDeque};
use std::sync::Arc;

use mrs_calculus::ordering::SymbolConfig;
use mrs_core::clause::ClauseId;
use mrs_core::term_bank::{IdClause, TermBank};

#[derive(Clone, Debug)]
struct WeightWrapper {
    id: ClauseId,
    weight: u32,
    distance: u32,
    goal_distance: u8,
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
    /// Priority queue for unit clauses (1 literal), lightest first.
    unit_queue: BinaryHeap<WeightWrapper>,
    /// Priority queue for Horn clauses (<= 1 positive literal), lightest first.
    horn_queue: BinaryHeap<WeightWrapper>,
    /// Priority queue for Set-of-Support clauses (distance < 100), lightest first.
    sos_queue: BinaryHeap<WeightWrapper>,
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
            unit_queue: BinaryHeap::new(),
            horn_queue: BinaryHeap::new(),
            sos_queue: BinaryHeap::new(),
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
    /// `goal_distance` is the relational distance to the conjecture in the
    /// symbol-reachability bipartite graph and derivation DAG.
    ///
    /// `ml_score` is the raw logit from the ML clause classifier; it only
    /// affects the ML priority queue (`ml-guidance` feature). In the default
    /// build the parameter is ignored and costs nothing.
    pub fn push(
        &mut self,
        clause: &IdClause,
        _bank: &TermBank,
        weight: u32,
        goal_distance: Option<u8>,
        ml_score: Option<f32>,
    ) {
        #[cfg(not(feature = "ml-guidance"))]
        let _ = ml_score;
        let id = clause.id;

        let eff_goal_dist = goal_distance.unwrap_or(if clause.distance < 100 {
            clause.distance as u8
        } else {
            100
        });

        let goal_weight = if eff_goal_dist < 100 {
            weight.saturating_add((eff_goal_dist as u32).saturating_mul(2))
        } else {
            weight.saturating_add(1000) // heavy penalty for pure axioms
        };

        self.active_ids.insert(id);
        self.age_queue.push_back(id);
        self.weight_queue.push(WeightWrapper {
            id,
            weight,
            distance: clause.distance,
            goal_distance: eff_goal_dist,
        });
        self.goal_queue.push(WeightWrapper {
            id,
            weight: goal_weight,
            distance: clause.distance,
            goal_distance: eff_goal_dist,
        });
        if clause.literals.len() == 1 {
            self.unit_queue.push(WeightWrapper {
                id,
                weight,
                distance: clause.distance,
                goal_distance: eff_goal_dist,
            });
        }
        if clause.literals.iter().filter(|lit| lit.positive).count() <= 1 {
            self.horn_queue.push(WeightWrapper {
                id,
                weight,
                distance: clause.distance,
                goal_distance: eff_goal_dist,
            });
        }
        if clause.distance < 100 {
            self.sos_queue.push(WeightWrapper {
                id,
                weight,
                distance: clause.distance,
                goal_distance: eff_goal_dist,
            });
        }
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
                goal_distance: eff_goal_dist,
            });
        }
    }

    /// Returns `true` if there are no clauses in the set.
    pub fn is_empty(&self) -> bool {
        self.active_ids.is_empty()
    }

    /// Test-only: does the goal queue still hold an entry for some active
    /// clause? This is the invariant the goal-directed selector depends on —
    /// `pop_goal_directed` can only report "drained" when it returns `None`,
    /// which this rules out for as long as anything is active.
    #[cfg(test)]
    fn goal_queue_reaches_active(&self) -> bool {
        self.goal_queue
            .iter()
            .any(|w| self.active_ids.contains(&w.id))
    }

    /// Returns the number of clauses currently in the unprocessed set.
    pub fn active_count(&self) -> usize {
        self.active_ids.len()
    }

    pub fn contains(&self, id: &mrs_core::clause::ClauseId) -> bool {
        self.active_ids.contains(id)
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

    /// Pops the lightest unit clause (1 literal) from the set, returning its ID.
    pub fn pop_unit(&mut self) -> Option<ClauseId> {
        while let Some(wrapper) = self.unit_queue.pop() {
            if self.active_ids.remove(&wrapper.id) {
                return Some(wrapper.id);
            }
        }
        None
    }

    /// Pops the lightest Horn clause (<= 1 positive literal) from the set, returning its ID.
    pub fn pop_horn(&mut self) -> Option<ClauseId> {
        while let Some(wrapper) = self.horn_queue.pop() {
            if self.active_ids.remove(&wrapper.id) {
                return Some(wrapper.id);
            }
        }
        None
    }

    /// Pops the lightest Set-of-Support clause (distance < 100) from the dedicated SOS queue, returning its ID.
    pub fn pop_sos(&mut self) -> Option<ClauseId> {
        while let Some(wrapper) = self.sos_queue.pop() {
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

    /// Prunes the passive set to keep only `target_size` clauses, reserving a
    /// configurable fraction for oldest active clauses and filling the rest
    /// by weight.
    /// Returns the number of discarded clauses.
    pub fn prune(&mut self, target_size: usize, age_reserve_percent: usize) -> usize {
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

        // Reserve a bounded fraction of the oldest active clauses so a long
        // run of light, locally attractive clauses cannot erase all age
        // diversity. Disabled by default; the EPU experiment sets this through
        // MRS_LRS_AGE_RESERVE.
        let reserve_count = target_size.saturating_mul(age_reserve_percent.min(100)) / 100;
        let mut reserve = HashSet::default();
        if reserve_count > 0 {
            for id in &self.age_queue {
                if self.active_ids.contains(id) {
                    reserve.insert(*id);
                    if reserve.len() >= reserve_count {
                        break;
                    }
                }
            }
        }
        let mut protected: Vec<_> = active_wrappers
            .iter()
            .filter(|wrapper| reserve.contains(&wrapper.id))
            .cloned()
            .collect();
        let mut candidates: Vec<_> = active_wrappers
            .into_iter()
            .filter(|wrapper| !reserve.contains(&wrapper.id))
            .collect();
        let remaining = target_size.saturating_sub(protected.len());
        candidates.sort_unstable_by(|a, b| a.weight.cmp(&b.weight).then_with(|| a.id.cmp(&b.id)));

        if candidates.len() <= remaining {
            // Restore weight_queue and return
            protected.extend(candidates);
            self.weight_queue = BinaryHeap::from(protected);
            return 0;
        }

        let (kept, discarded) = candidates.split_at(remaining);
        let num_discarded = discarded.len();
        protected.extend_from_slice(kept);

        // 3. Remove discarded IDs from active_ids
        for w in discarded {
            self.active_ids.remove(&w.id);
        }

        // 4. Filter age_queue in-place
        self.age_queue.retain(|id| self.active_ids.contains(id));

        // 5. Rebuild weight_queue
        // Since BinaryHeap is a max-heap but WeightWrapper's Ord is reversed,
        // we can just construct BinaryHeap from the kept wrappers!
        self.weight_queue = BinaryHeap::from(protected.clone());

        // 6. Rebuild goal_queue
        let goal_wrappers: Vec<WeightWrapper> = protected
            .iter()
            .map(|w| {
                let goal_weight = if w.goal_distance < 100 {
                    w.weight
                        .saturating_add((w.goal_distance as u32).saturating_mul(2))
                } else {
                    w.weight.saturating_add(1000)
                };
                WeightWrapper {
                    id: w.id,
                    weight: goal_weight,
                    distance: w.distance,
                    goal_distance: w.goal_distance,
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

        // 8. Filter unit, horn, and sos queues in-place
        self.unit_queue.retain(|w| self.active_ids.contains(&w.id));
        self.horn_queue.retain(|w| self.active_ids.contains(&w.id));
        self.sos_queue.retain(|w| self.active_ids.contains(&w.id));

        num_discarded
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::clause::{Clause, ClauseId, ClauseSource};
    use mrs_core::term_bank::TermBank;

    /// Pushes a clause with an explicit weight and goal distance and returns
    /// its id. Goal distances differ so the goal queue orders differently from
    /// the weight queue, which is what makes the two queues worth comparing.
    /// Every third clause is a unit, so the unit queue is populated too.
    fn push_weighted(
        bank: &mut TermBank,
        set: &mut UnprocessedSet,
        id: u64,
        weight: u32,
        goal_distance: Option<u8>,
    ) -> ClauseId {
        let mut syms = mrs_core::SymbolTable::new();
        let p = syms.intern("p");
        let num_lits = if id.is_multiple_of(3) { 1 } else { 2 };
        let lits: Vec<_> = (0..num_lits)
            .map(|i| mrs_core::Literal::pos(mrs_core::Atom::pred(p, vec![mrs_core::Term::var(i)])))
            .collect();
        let legacy = Clause::new(
            ClauseId(id),
            lits,
            ClauseSource::Input {
                name: format!("test{id}"),
                role: "axiom".into(),
            },
        );
        let clause = bank.clause_from_legacy(&legacy);
        set.push(&clause, bank, weight, goal_distance, None);
        clause.id
    }

    fn new_set() -> UnprocessedSet {
        UnprocessedSet::new(Arc::new(SymbolConfig::default()))
    }

    /// UI-9: the goal queue holds an entry for every active clause, so it can
    /// only report "drained" once nothing is active — at which point the other
    /// queues hold only tombstones and cannot produce a clause either.
    #[test]
    fn goal_queue_drains_only_when_no_clause_is_active() {
        let mut bank = TermBank::new();
        let mut set = new_set();
        let mut ids = Vec::new();
        for (id, weight, goal_distance) in [
            (0u64, 10u32, Some(0u8)),
            (1, 20, Some(1)),
            (2, 30, None), // pure axiom: goal weight +1000
            (3, 40, Some(2)),
            (4, 50, None),
            (5, 60, Some(3)),
        ] {
            ids.push(push_weighted(
                &mut bank,
                &mut set,
                id,
                weight,
                goal_distance,
            ));
        }
        assert_eq!(set.active_count(), 6);

        let mut popped = Vec::new();
        while let Some(id) = set.pop_goal_directed() {
            popped.push(id);
            assert!(
                !set.is_empty() || popped.len() == 6,
                "goal queue reported a clause while the set was already empty"
            );
        }
        popped.sort();
        ids.sort();
        assert_eq!(popped, ids, "goal queue did not hand out every clause");
        assert!(
            set.is_empty(),
            "goal queue drained with clauses still active"
        );

        // The remaining queues are physically non-empty (tombstones) but have
        // nothing active to return.
        assert!(set.pop_age().is_none());
        assert!(set.pop_weight().is_none());
        assert!(set.pop_unit().is_none());
        assert!(set.pop_horn().is_none());
        assert!(set.pop_sos().is_none());
    }

    /// The LRS prune rebuilds the goal queue from the active set. Survivors
    /// must stay reachable through it, and pruning must never leave the goal
    /// queue empty while a survivor is active.
    #[test]
    fn pruning_keeps_survivors_reachable_from_the_goal_queue() {
        for reserve_percent in [0usize, 25, 50, 100] {
            let mut bank = TermBank::new();
            let mut set = new_set();
            for id in 0..20u64 {
                push_weighted(
                    &mut bank,
                    &mut set,
                    id,
                    10 + id as u32,
                    if id % 3 == 0 {
                        None
                    } else {
                        Some((id % 5) as u8)
                    },
                );
            }
            let discarded = set.prune(6, reserve_percent);
            assert_eq!(discarded, 14);
            let survivors: Vec<ClauseId> = set.iter().collect();
            assert_eq!(survivors.len(), 6);

            let mut reached = Vec::new();
            while let Some(id) = set.pop_goal_directed() {
                assert!(
                    survivors.contains(&id),
                    "{id:?} is not in the active set after pruning"
                );
                reached.push(id);
            }
            reached.sort();
            let mut expected = survivors;
            expected.sort();
            assert_eq!(
                reached, expected,
                "reserve_percent={reserve_percent}: goal queue lost a survivor"
            );
            assert!(set.is_empty());
        }
    }

    /// Deterministic interleaving of every mutating operation the search can
    /// perform on the set. After each step, either the set is empty or the goal
    /// queue still has a live clause to give.
    #[test]
    fn goal_queue_never_drains_early_under_interleaved_operations() {
        let mut bank = TermBank::new();
        let mut set = new_set();
        let mut next_id = 0u64;
        let mut rng = 0x2545_F491_4F6C_DD1Du64;

        for step in 0..5000u64 {
            rng = rng
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            // Push-heavy, so the set stays populated and `prune` keeps taking
            // its discarding branch rather than its no-op early return.
            match rng % 10 {
                0..=3 => {
                    push_weighted(
                        &mut bank,
                        &mut set,
                        next_id,
                        10 + (rng >> 32) as u32 % 90,
                        match (rng >> 40) % 3 {
                            0 => None,
                            1 => Some(0),
                            _ => Some((rng >> 48) as u8 % 100),
                        },
                    );
                    next_id += 1;
                }
                4 => {
                    if set.pop_goal_directed().is_none() {
                        assert!(set.is_empty(), "step {step}: goal queue drained early");
                    }
                }
                5 => {
                    if set.pop_age().is_none() {
                        assert!(set.is_empty(), "step {step}: age queue drained early");
                    }
                }
                6 => {
                    if set.pop_weight().is_none() {
                        assert!(set.is_empty(), "step {step}: weight queue drained early");
                    }
                }
                7 => {
                    // An empty unit queue can also mean "no active clause is a
                    // unit", so only the goal/age/weight queues are asserted on
                    // directly; the unit pop is here for tombstone pressure.
                    let _ = set.pop_unit();
                }
                8 => {
                    let target = 1 + (rng >> 32) as usize % 4;
                    set.prune(target, (rng >> 48) as usize % 101);
                }
                _ => {
                    // `retain`/`remove` deactivate clauses without touching the
                    // physical queues, so this is the tombstone-producing path.
                    let live: Vec<ClauseId> = set.iter().collect();
                    if let Some(&victim) = live.get((rng >> 40) as usize % live.len().max(1)) {
                        if (rng >> 56) & 1 == 0 {
                            set.remove(victim);
                        } else {
                            set.retain(|id| id != victim);
                        }
                    }
                }
            }

            // The invariant the given-clause loop relies on, checked every step
            // without consuming anything: a non-empty set always has a live
            // entry in the goal queue.
            assert_eq!(
                set.is_empty(),
                !set.goal_queue_reaches_active(),
                "step {step}: {} active clause(s) but goal queue has {} entry",
                set.active_count(),
                set.goal_queue.len(),
            );
        }
    }

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
        set.push(&clauses[0], &bank, 10, None, None);
        set.push(&clauses[1], &bank, 50, None, None);
        set.push(&clauses[2], &bank, 5, None, None);
        set.push(&clauses[3], &bank, 100, None, None);
        set.push(&clauses[4], &bank, 20, None, None);

        assert_eq!(set.active_count(), 5);

        // Pruning to target_size = 3
        // Sorted weights: c2 (5), c0 (10), c4 (20), c1 (50), c3 (100)
        // We expect c1 and c3 (heaviest) to be pruned!
        // So c2, c0, and c4 should be kept.
        let discarded = set.prune(3, 0);
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
    fn lrs_age_reserve_preserves_old_clauses_over_weight() {
        let config = Arc::new(SymbolConfig::default());
        let mut set = UnprocessedSet::new(config);
        let mut bank = TermBank::new();
        let clauses = (0..5)
            .map(|i| {
                let legacy = Clause::new(
                    ClauseId(i),
                    vec![],
                    ClauseSource::Input {
                        name: format!("test{i}"),
                        role: "axiom".into(),
                    },
                );
                bank.clause_from_legacy(&legacy)
            })
            .collect::<Vec<_>>();
        for (clause, weight) in clauses.iter().zip([100, 90, 80, 1, 2]) {
            set.push(clause, &bank, weight, None, None);
        }

        assert_eq!(set.prune(2, 50), 3);
        assert!(set.contains(&clauses[0].id));
        assert!(set.contains(&clauses[3].id));
        assert_eq!(set.active_count(), 2);
    }

    #[test]
    fn test_goal_queue_with_goal_distance() {
        let config = Arc::new(SymbolConfig::default());
        let mut set = UnprocessedSet::new(config);
        let mut bank = TermBank::new();

        let mut make_clause = |id: u64, dist: u32| {
            let legacy = Clause {
                id: ClauseId(id),
                literals: vec![].into(),
                source: ClauseSource::Input {
                    name: "test".into(),
                    role: "axiom".into(),
                },
                avatar: vec![],
                distance: dist,
                formula: None,
                certificate: None,
                proof_id: None,
                witness: None,
            };
            bank.clause_from_legacy(&legacy)
        };

        let c_pure_ax = make_clause(1, 100);
        let c_hop1_ax = make_clause(2, 100);
        let c_hop2_ax = make_clause(3, 100);

        // c_pure_ax: weight 10, goal_dist None (100) -> goal_weight = 1010
        // c_hop1_ax: weight 20, goal_dist Some(1) -> goal_weight = 20 + 2 = 22
        // c_hop2_ax: weight 15, goal_dist Some(2) -> goal_weight = 15 + 4 = 19
        set.push(&c_pure_ax, &bank, 10, None, None);
        set.push(&c_hop1_ax, &bank, 20, Some(1), None);
        set.push(&c_hop2_ax, &bank, 15, Some(2), None);

        // pop_goal_directed should return c_hop2_ax (19), then c_hop1_ax (22), then c_pure_ax (1010)
        assert_eq!(set.pop_goal_directed(), Some(c_hop2_ax.id));
        assert_eq!(set.pop_goal_directed(), Some(c_hop1_ax.id));
        assert_eq!(set.pop_goal_directed(), Some(c_pure_ax.id));
        assert_eq!(set.pop_goal_directed(), None);
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
        set.push(&c1, &bank, 10, None, Some(-1.5)); // Low score
        set.push(&c2, &bank, 10, None, Some(2.0)); // High score (best)
        set.push(&c3, &bank, 10, None, Some(0.0)); // Medium score

        // We expect pop_ml() to return c2 first, then c3, then c1.
        assert_eq!(set.pop_ml(), Some(c2.id));
        assert_eq!(set.pop_ml(), Some(c3.id));
        assert_eq!(set.pop_ml(), Some(c1.id));
        assert_eq!(set.pop_ml(), None);
    }
}
