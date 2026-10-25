//! Proof extraction from the clause store.
//!
//! Traces back from the empty clause through `ClauseSource::Inference` and
//! `ClauseSource::Introduced` parent pointers to collect all clauses involved
//! in the refutation.
//! The result is topologically sorted: input clauses first, empty clause last.

use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;

use mrs_core::clause::{Clause, ClauseCertificate, ClauseId, ClauseSource};
use mrs_core::term_bank::{IdClause, TermBank};

/// Iterative postorder traversal. Missing dependencies and cycles fail closed.
/// Reversing breadth-first discovery is not topological for DAGs with shortcuts.
fn ancestor_order<I: Copy + Eq + std::hash::Hash>(
    root: I,
    mut parents: impl FnMut(I) -> Option<Vec<I>>,
) -> Vec<I> {
    let mut active = HashSet::new();
    let mut finished = HashSet::new();
    let mut stack = vec![(root, false)];
    let mut order = Vec::new();
    while let Some((id, exiting)) = stack.pop() {
        if exiting {
            active.remove(&id);
            finished.insert(id);
            order.push(id);
        } else if !finished.contains(&id) {
            if !active.insert(id) {
                return Vec::new();
            }
            let Some(dependencies) = parents(id) else {
                return Vec::new();
            };
            stack.push((id, true));
            stack.extend(dependencies.into_iter().rev().map(|parent| (parent, false)));
        }
    }
    order
}

fn dependencies(source: &ClauseSource, certificate: Option<&ClauseCertificate>) -> Vec<ClauseId> {
    let mut parents = match source {
        ClauseSource::Inference { parents, .. } | ClauseSource::Introduced { parents, .. } => {
            parents.to_vec()
        }
        ClauseSource::Input { .. } => Vec::new(),
    };
    match certificate {
        Some(ClauseCertificate::AvatarComponent { split_parent, .. }) => {
            parents.push(*split_parent)
        }
        Some(ClauseCertificate::AvatarSatRefutation {
            split_nodes,
            branch_roots,
            ..
        }) => {
            parents.extend(split_nodes);
            parents.extend(branch_roots);
        }
        Some(ClauseCertificate::SatBackedRefutation { inputs, .. }) => parents.extend(inputs),
        _ => {}
    }
    parents
}

/// Extracts the proof DAG from the clause store.
///
/// Starting from `empty_clause_id`, follows parent pointers in
/// `ClauseSource::Inference`, `ClauseSource::Introduced`, and
/// `ClauseCertificate` dependencies to collect all ancestor clauses.
///
/// Returns a topologically sorted vector: input clauses appear before
/// any clause that depends on them. The empty clause is last.
///
/// Handles DAGs correctly: shared parent clauses appear only once.
pub fn extract_proof<S: BuildHasher>(
    empty_clause_id: ClauseId,
    clause_store: &HashMap<ClauseId, Clause, S>,
) -> Vec<Clause> {
    ancestor_order(empty_clause_id, |id| {
        clause_store
            .get(&id)
            .map(|clause| dependencies(&clause.source, clause.certificate.as_ref()))
    })
    .into_iter()
    .filter_map(|id| clause_store.get(&id).cloned())
    .collect()
}

pub fn extract_proof_ids<S: BuildHasher>(
    empty_clause_id: ClauseId,
    clause_store: &HashMap<ClauseId, IdClause, S>,
) -> Vec<ClauseId> {
    ancestor_order(empty_clause_id, |id| {
        clause_store
            .get(&id)
            .map(|clause| dependencies(&clause.source, clause.certificate.as_ref()))
    })
}

/// Convert only the reachable derivation, rather than the entire search history.
/// Invalid dependency graphs return an empty proof.
pub fn extract_interned_proof<S: BuildHasher>(
    root: ClauseId,
    store: &HashMap<ClauseId, IdClause, S>,
    bank: &TermBank,
) -> Vec<Clause> {
    extract_proof_ids(root, store)
        .into_iter()
        .map(|id| bank.clause_to_legacy(&store[&id]))
        .collect()
}

/// Extracts the topologically sorted sequence of proof nodes from `ProofArena`
/// starting from the empty clause node `root`.
pub fn extract_proof_nodes(
    root: mrs_core::witness::ProofNodeId,
    arena: &mrs_core::witness::ProofArena,
) -> Vec<mrs_core::witness::ProofNode> {
    ancestor_order(root, |id| arena.get(id).map(|node| node.parents.to_vec()))
        .into_iter()
        .filter_map(|id| arena.get(id).cloned())
        .collect()
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
                rule: "resolution",
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

    #[test]
    fn shortcut_dependencies_are_topological_in_both_representations() {
        // The root cites both a node and its descendant. Reverse BFS puts the
        // descendant before its parent for this shape.
        let clauses = [input(90), inferred(7, vec![90]), inferred(2, vec![90, 7])];
        let store: HashMap<_, _> = clauses.into_iter().map(|c| (c.id, c)).collect();
        let expected = vec![ClauseId(90), ClauseId(7), ClauseId(2)];
        assert_eq!(
            extract_proof(ClauseId(2), &store)
                .iter()
                .map(|c| c.id)
                .collect::<Vec<_>>(),
            expected
        );
        let mut bank = TermBank::new();
        let mut interned: HashMap<_, _> = store
            .values()
            .map(|c| (c.id, bank.clause_from_legacy(c)))
            .collect();
        // Unreachable malformed data must never be converted or traversed.
        interned.insert(
            ClauseId(100),
            IdClause::new(
                ClauseId(100),
                vec![],
                ClauseSource::Inference {
                    rule: "resolution",
                    parents: vec![ClauseId(999)].into(),
                },
            ),
        );
        assert_eq!(extract_proof_ids(ClauseId(2), &interned), expected);
        assert_eq!(
            extract_interned_proof(ClauseId(2), &interned, &bank)
                .iter()
                .map(|c| c.id)
                .collect::<Vec<_>>(),
            expected
        );
    }

    #[test]
    fn missing_and_cyclic_dependencies_fail_closed() {
        let mut store = HashMap::new();
        store.insert(ClauseId(1), inferred(1, vec![2]));
        assert!(extract_proof(ClauseId(1), &store).is_empty());
        store.insert(ClauseId(2), inferred(2, vec![1]));
        assert!(extract_proof(ClauseId(1), &store).is_empty());
    }

    #[test]
    fn extract_introduced_definition_parents() {
        let mut store = HashMap::new();
        let mut symbols = mrs_core::SymbolTable::new();
        let definition_symbol = symbols.intern("d");
        let source = input(0);
        let definition = Clause::new_formula_step(
            ClauseId(1),
            mrs_core::Formula::True,
            ClauseSource::Introduced {
                symbol: definition_symbol,
                parents: vec![ClauseId(0)].into(),
            },
        );
        let conclusion = inferred(2, vec![1]);
        store.insert(source.id, source);
        store.insert(definition.id, definition);
        store.insert(conclusion.id, conclusion);

        let proof = extract_proof(ClauseId(2), &store);
        assert_eq!(
            proof.iter().map(|clause| clause.id).collect::<Vec<_>>(),
            vec![ClauseId(0), ClauseId(1), ClauseId(2)]
        );
    }

    #[test]
    fn extract_avatar_certificate_dependencies() {
        // Test that extract_proof traverses certificate dependencies:
        // c0 (input unsplit clause)
        // c1 (avatar_split_clause, parent: c0)
        // c2 (avatar_component_clause, cert split_parent: c1)
        // c3 (avatar_branch_refutation, parent: c2)
        // c4 (avatar_sat_refutation, cert split_nodes: [c1], branch_roots: [c3])
        let mut store = HashMap::new();
        store.insert(ClauseId(0), input(0));

        let mut c1 = inferred(1, vec![0]);
        c1.certificate = Some(ClauseCertificate::AvatarSplit {
            inherited: vec![],
            components: vec![],
        });
        store.insert(ClauseId(1), c1);

        let mut c2 = Clause::new(
            ClauseId(2),
            vec![],
            ClauseSource::Inference {
                rule: "avatar_component_clause",
                parents: vec![ClauseId(1)].into(),
            },
        );
        c2.certificate = Some(ClauseCertificate::AvatarComponent {
            split_parent: ClauseId(1),
            branch_index: 0,
            sat_var: 1,
        });
        store.insert(ClauseId(2), c2);

        let mut c3 = Clause::new(
            ClauseId(3),
            vec![],
            ClauseSource::Inference {
                rule: "avatar_branch_refutation",
                parents: vec![ClauseId(2)].into(),
            },
        );
        c3.certificate = Some(ClauseCertificate::AvatarBranchRefutation { context: vec![1] });
        store.insert(ClauseId(3), c3);

        let mut c4 = Clause::new(
            ClauseId(4),
            vec![],
            ClauseSource::Inference {
                rule: "avatar_sat_refutation",
                parents: vec![ClauseId(1), ClauseId(3)].into(),
            },
        );
        c4.certificate = Some(ClauseCertificate::AvatarSatRefutation {
            split_nodes: vec![ClauseId(1)],
            branch_roots: vec![ClauseId(3)],
            sat_trace: None,
        });
        store.insert(ClauseId(4), c4);

        let proof = extract_proof(ClauseId(4), &store);
        assert_eq!(proof.len(), 5);
        assert_eq!(proof[proof.len() - 1].id, ClauseId(4));
    }

    #[test]
    fn test_extract_proof_nodes_dag() {
        use mrs_core::witness::{ProofArena, ProofWitness};

        let mut arena = ProofArena::new();
        let n0 = arena.alloc(
            ClauseId(1),
            ProofWitness::Input {
                name: "ax1".to_string(),
                role: "axiom".to_string(),
            },
            vec![].into(),
        );
        let n1 = arena.alloc(
            ClauseId(2),
            ProofWitness::Input {
                name: "ax2".to_string(),
                role: "axiom".to_string(),
            },
            vec![].into(),
        );
        let n2 = arena.alloc(
            ClauseId(3),
            ProofWitness::Resolution {
                parent_left: n0,
                parent_right: n1,
                lit_idx_left: 0,
                lit_idx_right: 0,
                renaming_offset: 0,
                unifier: None,
            },
            vec![n0, n1].into(),
        );
        let n3 = arena.alloc(
            ClauseId(4),
            ProofWitness::EqualityResolution {
                parent: n2,
                lit_idx: 0,
                unifier: None,
            },
            vec![n2].into(),
        );

        let nodes = extract_proof_nodes(n3, &arena);
        assert_eq!(nodes.len(), 4);
        let pos_n0 = nodes.iter().position(|n| n.id == n0).unwrap();
        let pos_n1 = nodes.iter().position(|n| n.id == n1).unwrap();
        let pos_n2 = nodes.iter().position(|n| n.id == n2).unwrap();
        let pos_n3 = nodes.iter().position(|n| n.id == n3).unwrap();
        assert!(pos_n0 < pos_n2);
        assert!(pos_n1 < pos_n2);
        assert!(pos_n2 < pos_n3);
    }
}
