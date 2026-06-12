//! Proof extraction from the clause store.
//!
//! Traces back from the empty clause through `ClauseSource::Inference` parent
//! pointers to collect all clauses involved in the refutation.
//! The result is topologically sorted: input clauses first, empty clause last.

use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::BuildHasher;

use mrs_core::clause::{Clause, ClauseId, ClauseSource};
use mrs_core::term_bank::IdClause;

/// Extracts the proof DAG from the clause store.
///
/// Starting from `empty_clause_id`, follows parent pointers in
/// `ClauseSource::Inference` to collect all ancestor clauses.
///
/// Returns a topologically sorted vector: input clauses appear before
/// any clause that depends on them. The empty clause is last.
///
/// Handles DAGs correctly: shared parent clauses appear only once.
pub fn extract_proof<S: BuildHasher>(
    empty_clause_id: ClauseId,
    clause_store: &HashMap<ClauseId, Clause, S>,
) -> Vec<Clause> {
    // Collect all relevant clause IDs via BFS
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    let mut order = Vec::new();

    queue.push_back(empty_clause_id);
    visited.insert(empty_clause_id);

    while let Some(id) = queue.pop_front() {
        order.push(id);

        if let Some(clause) = clause_store.get(&id)
            && let ClauseSource::Inference { parents, .. } = &clause.source
        {
            for &parent_id in parents {
                if visited.insert(parent_id) {
                    queue.push_back(parent_id);
                }
            }
        }
    }

    // Reverse: inputs first, empty clause last
    order.reverse();

    // Collect the actual clauses
    order
        .into_iter()
        .filter_map(|id| clause_store.get(&id).cloned())
        .collect()
}

pub fn extract_proof_ids<S: BuildHasher>(
    empty_clause_id: ClauseId,
    clause_store: &HashMap<ClauseId, IdClause, S>,
) -> Vec<ClauseId> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    let mut order = Vec::new();

    queue.push_back(empty_clause_id);
    visited.insert(empty_clause_id);

    while let Some(id) = queue.pop_front() {
        order.push(id);

        if let Some(clause) = clause_store.get(&id)
            && let ClauseSource::Inference { parents, .. } = &clause.source
        {
            for &parent_id in parents {
                if visited.insert(parent_id) {
                    queue.push_back(parent_id);
                }
            }
        }
    }

    order.reverse();
    order
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::clause::{Clause, ClauseId, ClauseSource};

    fn input(id: u64) -> Clause {
        Clause::new(
            ClauseId(id),
            vec![],
            ClauseSource::Input {
                name: format!("c{}", id),
                role: "axiom".into(),
            },
        )
    }

    fn inferred(id: u64, parents: Vec<u64>) -> Clause {
        Clause::new(
            ClauseId(id),
            vec![],
            ClauseSource::Inference {
                rule: "resolution".into(),
                parents: parents.into_iter().map(ClauseId).collect(),
            },
        )
    }

    #[test]
    fn extract_single_input() {
        let mut store = HashMap::new();
        let c = input(0);
        store.insert(c.id, c);
        let proof = extract_proof(ClauseId(0), &store);
        assert_eq!(proof.len(), 1);
        assert_eq!(proof[0].id, ClauseId(0));
    }

    #[test]
    fn extract_one_step() {
        // c0 (input) + c1 (input) -> c2 (inferred)
        let mut store = HashMap::new();
        store.insert(ClauseId(0), input(0));
        store.insert(ClauseId(1), input(1));
        store.insert(ClauseId(2), inferred(2, vec![0, 1]));
        let proof = extract_proof(ClauseId(2), &store);
        assert_eq!(proof.len(), 3);
        // Empty clause (id=2) should be last
        assert_eq!(proof[2].id, ClauseId(2));
    }

    #[test]
    fn extract_multi_step() {
        // c0, c1 -> c2; c2, c3 -> c4
        let mut store = HashMap::new();
        store.insert(ClauseId(0), input(0));
        store.insert(ClauseId(1), input(1));
        store.insert(ClauseId(2), inferred(2, vec![0, 1]));
        store.insert(ClauseId(3), input(3));
        store.insert(ClauseId(4), inferred(4, vec![2, 3]));
        let proof = extract_proof(ClauseId(4), &store);
        assert_eq!(proof.len(), 5);
        // c4 should be last
        assert_eq!(proof[proof.len() - 1].id, ClauseId(4));
    }

    #[test]
    fn extract_dag_no_duplicates() {
        // c0 -> c1, c0 -> c2, c1 + c2 -> c3
        // c0 is a shared ancestor — should appear once
        let mut store = HashMap::new();
        store.insert(ClauseId(0), input(0));
        store.insert(ClauseId(1), inferred(1, vec![0]));
        store.insert(ClauseId(2), inferred(2, vec![0]));
        store.insert(ClauseId(3), inferred(3, vec![1, 2]));
        let proof = extract_proof(ClauseId(3), &store);
        assert_eq!(proof.len(), 4); // c0 appears once, not twice
    }
}
