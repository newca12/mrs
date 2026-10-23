//! Typed proof witnesses and stable proof arena.
//!
//! Provides first-class, explicit inference justification witnesses replacing
//! lossy string-and-parent-pointer annotations.

use smallvec::SmallVec;

use crate::HashMap;
use crate::clause::ClauseId;
use crate::formula::Formula;
use crate::subst::Substitution;
use crate::symbol::SymbolId;
use crate::term::VarId;

/// A globally unique, stable identifier for a proof node in the proof arena.
///
/// Unlike operational `ClauseId`s which may be recycled or discarded by search
/// indexing (e.g. LRS pruning or backwards subsumption), a `ProofNodeId` is
/// permanently retained in the append-only `ProofArena`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofNodeId(pub u64);

impl std::fmt::Display for ProofNodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "p{}", self.0)
    }
}

/// Typed witness data explaining the exact justification of an inference step.
#[derive(Clone, Debug, PartialEq)]
pub enum ProofWitness {
    /// Input premise from the problem file.
    Input { name: String, role: String },
    /// Classical binary resolution.
    Resolution {
        parent_left: ProofNodeId,
        parent_right: ProofNodeId,
        lit_idx_left: usize,
        lit_idx_right: usize,
        renaming_offset: VarId,
        unifier: Option<Substitution>,
    },
    /// Positive or negative literal factoring.
    Factoring {
        parent: ProofNodeId,
        retained_lit_idx: usize,
        removed_lit_idx: usize,
        unifier: Option<Substitution>,
    },
    /// Equality resolution: elimination of `t != t`.
    EqualityResolution {
        parent: ProofNodeId,
        lit_idx: usize,
        unifier: Option<Substitution>,
    },
    /// Equality factoring on an equality literal.
    EqualityFactoring {
        parent: ProofNodeId,
        equality_lit_idx: usize,
        other_lit_idx: usize,
        unifier: Option<Substitution>,
    },
    /// Subsumption resolution (backward or forward simplification).
    SubsumptionResolution {
        target: ProofNodeId,
        subsumer: ProofNodeId,
        deleted_lit_idx: usize,
    },
    /// Condensation of duplicate / redundant literals via matching.
    Condensation {
        parent: ProofNodeId,
        retained_indices: Vec<usize>,
    },
    /// First-order superposition (paramodulation).
    Superposition {
        lhs_parent: ProofNodeId,
        rhs_parent: ProofNodeId,
        lhs_lit_idx: usize,
        rhs_lit_idx: usize,
        term_path: Vec<usize>,
        orientation_left: bool,
        unifier: Option<Substitution>,
    },
    /// Multi-step demodulation (deterministic unit rewriting).
    Demodulation {
        target: ProofNodeId,
        rule_parents: Vec<ProofNodeId>,
        steps: Vec<DemodStepWitness>,
    },
    /// Associative-commutative (AC) normalization.
    AcNormalization {
        parent: ProofNodeId,
        ac_symbols: Vec<SymbolId>,
        source_axioms: Vec<ProofNodeId>,
    },
    /// CNF clausification transformation.
    Cnf {
        source_formula: Option<Box<Formula>>,
        source_parents: Vec<ProofNodeId>,
    },
    /// Definitional introduction of fresh symbol.
    Definition {
        symbol: SymbolId,
        defining_formula: Option<Box<Formula>>,
    },
    /// Skolemization step introducing a fresh function or constant symbol.
    Skolemization {
        symbol: SymbolId,
        existential_var: VarId,
        universal_vars: Vec<VarId>,
        parent: Option<ProofNodeId>,
    },
    /// AVATAR propositional branch or split refutation.
    Avatar {
        split_parent: Option<ProofNodeId>,
        branch_roots: Vec<ProofNodeId>,
        context: Vec<u32>,
    },
    /// Cross-strategy lemma exchange.
    SharedLemma {
        original_node: ProofNodeId,
        symbol_mapping: Vec<SymbolId>,
    },
    /// Fallback or legacy inference with rule name and parents.
    Legacy {
        rule: &'static str,
        parents: SmallVec<[ProofNodeId; 2]>,
    },
}

/// Witness for a single demodulation rewrite step.
#[derive(Clone, Debug, PartialEq)]
pub struct DemodStepWitness {
    pub rule_parent: ProofNodeId,
    pub lit_idx: usize,
    pub term_path: Vec<usize>,
    pub substitution: Option<Substitution>,
}

/// A node in the append-only proof arena.
#[derive(Clone, Debug, PartialEq)]
pub struct ProofNode {
    pub id: ProofNodeId,
    pub clause_id: ClauseId,
    pub witness: ProofWitness,
    pub parents: SmallVec<[ProofNodeId; 2]>,
}

/// An append-only proof arena that preserves the entire derivation history
/// independently of given-clause loop index pruning or deletion.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProofArena {
    nodes: Vec<ProofNode>,
    clause_to_node: HashMap<ClauseId, ProofNodeId>,
}

impl ProofArena {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn alloc(
        &mut self,
        clause_id: ClauseId,
        witness: ProofWitness,
        parents: SmallVec<[ProofNodeId; 2]>,
    ) -> ProofNodeId {
        let id = ProofNodeId(self.nodes.len() as u64);
        let node = ProofNode {
            id,
            clause_id,
            witness,
            parents,
        };
        self.nodes.push(node);
        self.clause_to_node.insert(clause_id, id);
        id
    }

    pub fn get(&self, id: ProofNodeId) -> Option<&ProofNode> {
        self.nodes.get(id.0 as usize)
    }

    pub fn node_for_clause(&self, clause_id: ClauseId) -> Option<ProofNodeId> {
        self.clause_to_node.get(&clause_id).copied()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn nodes(&self) -> &[ProofNode] {
        &self.nodes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proof_arena_allocation_and_lookup() {
        let mut arena = ProofArena::new();
        assert!(arena.is_empty());

        let id0 = arena.alloc(
            ClauseId(10),
            ProofWitness::Input {
                name: "c1".to_string(),
                role: "axiom".to_string(),
            },
            smallvec::SmallVec::new(),
        );
        let id1 = arena.alloc(
            ClauseId(11),
            ProofWitness::Input {
                name: "c2".to_string(),
                role: "axiom".to_string(),
            },
            smallvec::SmallVec::new(),
        );

        let id2 = arena.alloc(
            ClauseId(12),
            ProofWitness::Resolution {
                parent_left: id0,
                parent_right: id1,
                lit_idx_left: 0,
                lit_idx_right: 0,
                renaming_offset: 0,
                unifier: None,
            },
            smallvec::smallvec![id0, id1],
        );

        assert_eq!(arena.len(), 3);
        assert_eq!(arena.node_for_clause(ClauseId(10)), Some(id0));
        assert_eq!(arena.node_for_clause(ClauseId(11)), Some(id1));
        assert_eq!(arena.node_for_clause(ClauseId(12)), Some(id2));

        let node2 = arena.get(id2).expect("node2 exists");
        assert_eq!(node2.clause_id, ClauseId(12));
        assert_eq!(node2.parents.as_slice(), &[id0, id1]);
        match &node2.witness {
            ProofWitness::Resolution {
                parent_left,
                parent_right,
                lit_idx_left,
                lit_idx_right,
                ..
            } => {
                assert_eq!(*parent_left, id0);
                assert_eq!(*parent_right, id1);
                assert_eq!(*lit_idx_left, 0);
                assert_eq!(*lit_idx_right, 0);
            }
            other => panic!("expected Resolution witness, got {:?}", other),
        }
    }
}
