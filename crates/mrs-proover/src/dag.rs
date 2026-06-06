//! Build a DAG of proof nodes and check structural well-formedness.
//!
//! A *proof node* is a single annotated formula in the proof file. Each node
//! carries:
//!
//! - its name (e.g. `s1`),
//! - its role (`axiom`, `conjecture`, `negated_conjecture`, `plain`),
//! - the names of its parents (from the `inference(rule, info, [parents])`
//!   source), if any,
//! - the inference rule name, if any,
//! - the SZS status (`thm`, `esa`, `cth`), if any.
//!
//! Structural checks performed here:
//! 1. Names are unique.
//! 2. All parent references resolve to a known node.
//! 3. The parent graph is acyclic.
//! 4. There is exactly one *root* `$false` step (i.e. a `$false` clause not
//!    used as a parent by any other node). Vampire's AVATAR mode legitimately
//!    emits multiple `$false` clauses per proof (one per SAT-level component
//!    refutation, plus a final `avatar_sat_refutation` rolling them up); we
//!    accept this provided the final $false is unique in being unused as a
//!    parent. The internal `$false` clauses still get verified on their own
//!    merits via `check_node()` in the verify loop, so soundness is
//!    preserved: a tampered extra $false at an interior position must still
//!    pass its own inference check, and a tampered extra $false at the root
//!    yields a second unparented $false and is rejected here.

use std::collections::{HashMap, HashSet};

use mrs_tptp::{
    AnnotatedFormula, CNFAtomicFormula, CNFFormula, CNFLiteral, CNFStatement, FOFAtomicFormula,
    FOFFormula, FOFStatement, FormulaRole,
};

/// A single node in the proof DAG. We keep only `FOF` nodes — anything else
/// is reported as a structural failure upstream.
pub struct Node<'p> {
    pub name: &'p str,
    pub role: FormulaRole,
    pub parents: Vec<&'p str>,
    /// Parallel to `parents`: `true` iff the pedigree wraps the parent
    /// in `inference(assume_negation, _, _)`, in which case the
    /// asserted premise is the *negation* of the parent's formula.
    /// See [`mrs_tptp::proover::ParentRef`] for the why.
    pub negated_parents: Vec<bool>,
    pub inference_rule: Option<&'p str>,
    pub status: Option<&'p str>,
    pub is_false: bool,
    pub formula: &'p mrs_tptp::AnnotatedFormula<'p>,
}

/// The proof DAG.
pub struct Dag<'p> {
    pub nodes: Vec<Node<'p>>,
    /// Name → index in `nodes`.
    pub by_name: HashMap<&'p str, usize>,
    /// Topological order: parents before children.
    pub topo: Vec<usize>,
    /// Index of the unique `$false` node, if any.
    pub root: Option<usize>,
}

/// A structural defect found while building the DAG.
#[derive(Debug)]
pub enum DagError {
    /// Non-FOF dialect node in the proof.
    UnsupportedDialect(String),
    /// Two nodes share the same name.
    DuplicateName(String),
    /// A parent reference does not resolve.
    UnknownParent { node: String, parent: String },
    /// The parent graph contains a cycle.
    Cycle,
    /// The proof has no `$false` step.
    NoFalseRoot,
    /// The proof has more than one `$false` step.
    MultipleFalseRoots(Vec<String>),
    /// The root `$false` step is not reachable as the topological maximum
    /// (or there are orphan nodes after the root that depend on nothing).
    /// We accept this case but report it as a warning category — see
    /// `Dag::is_root_reaching_all_used`.
    #[allow(dead_code)]
    DanglingNodes(Vec<String>),
}

impl std::fmt::Display for DagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DagError::UnsupportedDialect(n) => write!(f, "node {n} is not FOF"),
            DagError::DuplicateName(n) => write!(f, "duplicate node name {n}"),
            DagError::UnknownParent { node, parent } => {
                write!(f, "node {node} references unknown parent {parent}")
            }
            DagError::Cycle => write!(f, "cycle in parent graph"),
            DagError::NoFalseRoot => write!(f, "proof does not derive $false"),
            DagError::MultipleFalseRoots(ns) => {
                write!(f, "multiple $false steps: {}", ns.join(", "))
            }
            DagError::DanglingNodes(ns) => write!(f, "dangling nodes: {}", ns.join(", ")),
        }
    }
}

/// Build the DAG from a parsed proof.
pub fn build<'p>(proof: &'p mrs_tptp::TPTPProblem<'p>) -> Result<Dag<'p>, DagError> {
    let mut nodes: Vec<Node<'p>> = Vec::with_capacity(proof.formulas.len());
    let mut by_name: HashMap<&'p str, usize> = HashMap::with_capacity(proof.formulas.len());

    for af in &proof.formulas {
        if !af.is_fof() && !af.is_cnf() {
            return Err(DagError::UnsupportedDialect(af.name().to_string()));
        }
        let name = af.name();
        if by_name.contains_key(name) {
            return Err(DagError::DuplicateName(name.to_string()));
        }
        let (parents, negated_parents, rule, status) = if let Some(ann) = af.annotations() {
            let refs = ann.parent_refs();
            let rule = ann.inference_rule();
            let status = ann.status();
            let mut names = Vec::with_capacity(refs.len());
            let mut negs = Vec::with_capacity(refs.len());
            for r in refs {
                names.push(r.name);
                negs.push(r.negated);
            }
            (names, negs, rule, status)
        } else {
            (Vec::new(), Vec::new(), None, None)
        };
        let is_false = is_false_formula(af);
        by_name.insert(name, nodes.len());
        nodes.push(Node {
            name,
            role: af.role(),
            parents,
            negated_parents,
            inference_rule: rule,
            status,
            is_false,
            formula: af,
        });
    }

    // Validate parent references.
    for (idx, n) in nodes.iter().enumerate() {
        for p in &n.parents {
            if !by_name.contains_key(p) {
                return Err(DagError::UnknownParent {
                    node: n.name.to_string(),
                    parent: p.to_string(),
                });
            }
        }
        let _ = idx;
    }

    // Topological sort (Kahn).
    let topo = topo_sort(&nodes, &by_name)?;

    // Locate $false node(s). A proof is sound if at most one $false clause
    // is "unused" (i.e. not referenced as a parent by any other clause); that
    // one is the refutation root. All other $false clauses must be referenced
    // by something (vampire's AVATAR pattern: per-component $false clauses
    // fed into a final `avatar_sat_refutation`). Such internal $false nodes
    // are verified individually by the main verify loop, so an attacker
    // cannot use them to smuggle in unsound derivations.
    let falses: Vec<usize> = nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.is_false)
        .map(|(i, _)| i)
        .collect();
    if falses.is_empty() {
        return Err(DagError::NoFalseRoot);
    }
    // Determine which nodes are referenced as parents anywhere in the proof.
    let mut used_as_parent: HashSet<usize> = HashSet::with_capacity(nodes.len());
    for n in &nodes {
        for p in &n.parents {
            if let Some(&pi) = by_name.get(p) {
                used_as_parent.insert(pi);
            }
        }
    }
    let unused_falses: Vec<usize> = falses
        .iter()
        .copied()
        .filter(|i| !used_as_parent.contains(i))
        .collect();
    let root = match unused_falses.as_slice() {
        [single] => Some(*single),
        [] => {
            // Every $false is consumed by some downstream node — there is no
            // refutation root. Treat as no root: structurally the proof
            // doesn't conclude.
            return Err(DagError::NoFalseRoot);
        }
        many => {
            // Multiple unparented $false clauses: ambiguous which is "the"
            // refutation. Reject — this is the case a tampered file would
            // need to land in to inject a fake refutation.
            return Err(DagError::MultipleFalseRoots(
                many.iter().map(|&i| nodes[i].name.to_string()).collect(),
            ));
        }
    };

    Ok(Dag {
        nodes,
        by_name,
        topo,
        root,
    })
}

fn topo_sort<'p>(
    nodes: &[Node<'p>],
    by_name: &HashMap<&'p str, usize>,
) -> Result<Vec<usize>, DagError> {
    let n = nodes.len();
    let mut indeg: Vec<usize> = vec![0; n];
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, node) in nodes.iter().enumerate() {
        for p in &node.parents {
            let pi = *by_name.get(p).unwrap();
            indeg[i] += 1;
            children[pi].push(i);
        }
    }
    let mut order = Vec::with_capacity(n);
    let mut ready: Vec<usize> = (0..n).filter(|i| indeg[*i] == 0).collect();
    let mut visited: HashSet<usize> = HashSet::new();
    while let Some(i) = ready.pop() {
        if !visited.insert(i) {
            continue;
        }
        order.push(i);
        for &c in &children[i] {
            indeg[c] -= 1;
            if indeg[c] == 0 {
                ready.push(c);
            }
        }
    }
    if order.len() != n {
        return Err(DagError::Cycle);
    }
    Ok(order)
}

fn is_false_formula(af: &AnnotatedFormula<'_>) -> bool {
    match af {
        AnnotatedFormula::FOF(f) => match &f.formula {
            FOFStatement::Logical(form) => is_false_fof_formula(form),
            _ => false,
        },
        AnnotatedFormula::CNF(c) => match &c.formula {
            CNFStatement::Logical(form) => is_false_cnf_formula(form),
        },
        _ => false,
    }
}

fn is_false_cnf_formula(f: &CNFFormula<'_>) -> bool {
    match f {
        CNFFormula::Disjunction(lits) => {
            lits.is_empty()
                || lits
                    .iter()
                    .all(|lit| matches!(lit, CNFLiteral::Positive(CNFAtomicFormula::False)))
        }
        CNFFormula::Parens(inner) => is_false_cnf_formula(inner),
    }
}

fn is_false_fof_formula(f: &FOFFormula<'_>) -> bool {
    match f {
        FOFFormula::Atomic(FOFAtomicFormula::False) => true,
        FOFFormula::Parens(inner) => is_false_fof_formula(inner),
        _ => false,
    }
}
