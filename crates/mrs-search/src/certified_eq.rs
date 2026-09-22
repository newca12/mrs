//! Ground equality expansion for the certified EPR+Eq fragment.
//!
//! Function-free EPR with equality (Bernays-Schönfinkel with equality) is
//! decidable by grounding plus ground equational reasoning. This module
//! turns a grounded clause set into an equationally complete one that the
//! existing Tier-1/2 machinery decides unchanged:
//!
//! 1. **Union-find** over ground unit equalities (adjacency + BFS, so every
//!    merge carries its explaining unit-equation ancestry).
//! 2. **Normalization** of every clause by class representatives, with
//!    canonical (name-ordered) equality orientation. Each changed clause
//!    becomes a derived step (`equality_normalization`) citing the original
//!    plus exactly the unit equations on the used paths; unchanged clauses
//!    pass through untouched (in particular, predicate-only inputs are
//!    byte-identical on output, so the legacy fragment sees zero behavior
//!    change).
//! 3. **Reflexivity fast paths**: positive `Eq(r, r)` clauses are valid and
//!    dropped silently (like subsumed clauses); a negative `Eq(r, r)`
//!    elaborates to the empty clause with path ancestry (the UEQ killer).
//! 4. **Transitivity cubes** `~Eq(a,b) | ~Eq(b,c) | Eq(a,c)` over distinct
//!    representatives (parentless validities, rule `equality_transitivity`).
//!    Symmetry needs no cubes: canonical orientation makes `Eq(a,b)` and
//!    `Eq(b,a)` syntactically identical. Predicate congruence needs no
//!    cubes either: with no function symbols, distinct representatives are
//!    fully uninterpreted.
//!
//! Why this is complete: the expanded set is a finite ground equational
//! problem — plain ground resolution (Tier 1, both closures) and
//! propositional SAT (Tier 2, equality as atoms) decide it, and the
//! double-closure agreement plus model re-check carry over verbatim.
//! Everything beyond the expansion caps fails closed as `Limit`.

use std::collections::{HashMap as StdHashMap, HashSet as StdHashSet, VecDeque};

use crate::TermOrdering;
use crate::certified::CertificationFailure;
use mrs_calculus::ordering::TermComparison;
use mrs_core::clause::{Clause, ClauseId, ClauseIdGen, ClauseSource, Literal};
use mrs_core::formula::Atom;
use mrs_core::term::Term;

/// Cap on generated transitivity-cube clauses: `reps^3` grows fast, and
/// anything past this fails closed downstream via the clause ceiling
/// anyway. Estimated arithmetically before materializing.
const MAX_TRANSITIVITY_CUBES: usize = 100_000;

/// Union-find over ground terms with proof-carrying merges: every union
/// records the unit-equation clause that justified it, so any derived
/// equality explains itself as a path of input units.
pub(crate) struct EqClasses {
    parent: StdHashMap<Term, Term>,
    rank: StdHashMap<Term, usize>,
    /// Adjacency for proof paths: term -> Vec<(neighbor, unit clause id)>.
    edges: StdHashMap<Term, Vec<(Term, ClauseId)>>,
}

impl EqClasses {
    pub(crate) fn new() -> Self {
        Self {
            parent: StdHashMap::new(),
            rank: StdHashMap::new(),
            edges: StdHashMap::new(),
        }
    }

    fn find_root(&mut self, term: &Term) -> Term {
        let parent = self
            .parent
            .get(term)
            .cloned()
            .unwrap_or_else(|| term.clone());
        if parent == *term {
            return parent;
        }
        let root = self.find_root(&parent);
        self.parent.insert(term.clone(), root.clone());
        root
    }

    /// Merge the classes of `left` and `right`, justified by `equation`.
    pub(crate) fn union(&mut self, left: &Term, right: &Term, equation: ClauseId) {
        let left_root = self.find_root(left);
        let right_root = self.find_root(right);
        for term in [left, right] {
            self.parent
                .entry(term.clone())
                .or_insert_with(|| term.clone());
            self.rank.entry(term.clone()).or_insert(0);
        }
        self.edges
            .entry(left.clone())
            .or_default()
            .push((right.clone(), equation));
        self.edges
            .entry(right.clone())
            .or_default()
            .push((left.clone(), equation));
        if left_root == right_root {
            return;
        }
        let left_rank = self.rank.get(&left_root).copied().unwrap_or(0);
        let right_rank = self.rank.get(&right_root).copied().unwrap_or(0);
        if left_rank < right_rank {
            self.parent.insert(left_root, right_root);
        } else {
            self.parent.insert(right_root, left_root.clone());
            if left_rank == right_rank {
                self.rank.insert(left_root, left_rank + 1);
            }
        }
    }

    /// Representative of `term`'s class (the term itself if unseen).
    pub(crate) fn representative(&mut self, term: &Term) -> Term {
        if !self.parent.contains_key(term) {
            return term.clone();
        }
        self.find_root(term)
    }

    /// Ensure `term` is a known singleton class (no merges, no edges).
    /// Used to seed every constant occurring in an equality literal so
    /// representative enumeration covers the whole equational vocabulary,
    /// not just unit-equation sides.
    pub(crate) fn singleton(&mut self, term: &Term) {
        self.parent
            .entry(term.clone())
            .or_insert_with(|| term.clone());
        self.rank.entry(term.clone()).or_insert(0);
    }

    /// Unit-equation ids forming a path from `from` to `to`, or `None`
    /// when disconnected. Breadth-first over the union adjacency, so the
    /// explaining set is small (not necessarily minimal). The start node
    /// is never collected: reaching `to` in zero steps yields the empty
    /// path, and every collected edge is a genuine union step.
    pub(crate) fn explain(&self, from: &Term, to: &Term) -> Option<Vec<ClauseId>> {
        if from == to {
            return Some(Vec::new());
        }
        let mut predecessor: StdHashMap<Term, (Term, ClauseId)> = StdHashMap::new();
        let mut queue = VecDeque::from([from.clone()]);
        predecessor.insert(from.clone(), (from.clone(), ClauseId(0)));
        while let Some(current) = queue.pop_front() {
            if current == *to {
                let mut path = Vec::new();
                let mut node = to.clone();
                while node != *from {
                    let (previous, equation) = predecessor.get(&node)?.clone();
                    path.push(equation);
                    node = previous;
                }
                return Some(path);
            }
            if let Some(neighbors) = self.edges.get(&current) {
                for (neighbor, equation) in neighbors {
                    if !predecessor.contains_key(neighbor) {
                        predecessor.insert(neighbor.clone(), (current.clone(), *equation));
                        queue.push_back(neighbor.clone());
                    }
                }
            }
        }
        None
    }

    /// Distinct class representatives currently known.
    pub(crate) fn representatives(&mut self) -> Vec<Term> {
        let terms: Vec<Term> = self.parent.keys().cloned().collect();
        let mut seen = StdHashSet::new();
        let mut reps = Vec::new();
        for term in &terms {
            let root = self.find_root(term);
            if seen.insert(root.clone()) {
                reps.push(root);
            }
        }
        reps
    }
}

/// Canonical equality orientation: heavier side first under the active
/// ordering, deterministic Debug tiebreak below that. Purely a
/// representation invariant so `Eq(a,b)` and `Eq(b,a)` resolve as the same
/// atom (orienting heavier-first also keeps maximality well-behaved); the
/// kernel re-derives orientation-insensitively (unordered-pair matching),
/// so no cross-boundary ordering convention is needed.
fn canonical_eq_order(left: &Term, right: &Term, ordering: &TermOrdering) -> (Term, Term) {
    match ordering.compare(left, right) {
        TermComparison::Greater | TermComparison::Equal => (left.clone(), right.clone()),
        TermComparison::Less => (right.clone(), left.clone()),
        TermComparison::Incomparable => {
            if format!("{left:?}") <= format!("{right:?}") {
                (left.clone(), right.clone())
            } else {
                (right.clone(), left.clone())
            }
        }
    }
}

/// Outcome of normalizing one grounded clause against unit classes.
enum Normalized {
    /// Unchanged (no equality content affected): pass through untouched.
    Unchanged,
    /// Rewritten form plus the explaining unit-equation ids.
    Rewritten {
        literals: Vec<Literal>,
        explains: Vec<ClauseId>,
    },
    /// Valid clause (positive reflexive equality): drop silently.
    Tautology,
    /// Derives the empty clause (negative reflexive equality): the
    /// explaining unit ids become the refutation parents.
    Contradiction { explains: Vec<ClauseId> },
}

fn normalize_clause_eq(
    clause: &Clause,
    classes: &mut EqClasses,
    ordering: &TermOrdering,
) -> Normalized {
    let mut changed = false;
    let mut literals = Vec::with_capacity(clause.literals.len());
    let mut explains: Vec<ClauseId> = Vec::new();
    for literal in &clause.literals {
        let Atom::Eq(left, right) = &literal.atom else {
            literals.push(literal.clone());
            continue;
        };
        let mut new_left = classes.representative(left);
        let mut new_right = classes.representative(right);
        if new_left != *left || new_right != *right {
            changed = true;
            if let Some(mut path) = classes.explain(left, &new_left) {
                explains.append(&mut path);
            }
            if let Some(mut path) = classes.explain(right, &new_right) {
                explains.append(&mut path);
            }
        }
        if new_left == new_right {
            if literal.positive {
                // Positive reflexive equality: the clause is valid.
                return Normalized::Tautology;
            }
            // Negative reflexive equality: this literal is false; the
            // remaining literals decide. Handled below by filtering.
            changed = true;
            continue;
        }
        (new_left, new_right) = canonical_eq_order(&new_left, &new_right, ordering);
        if new_left != *left || new_right != *right {
            changed = true;
        }
        literals.push(Literal {
            positive: literal.positive,
            atom: Atom::Eq(new_left, new_right),
        });
    }
    if !changed {
        return Normalized::Unchanged;
    }
    if literals.is_empty() {
        // Every literal was a false reflexive disequality: contradiction
        // with path ancestry (deduplicated for stable output).
        explains.sort_unstable_by_key(|id| id.0);
        explains.dedup();
        return Normalized::Contradiction { explains };
    }
    explains.sort_unstable_by_key(|id| id.0);
    explains.dedup();
    Normalized::Rewritten { literals, explains }
}

/// Result of [`expand_equality`]: normalized clauses (with derivation
/// steps), transitivity cubes, and an optional immediate refutation.
pub(crate) struct ExpandedEq {
    pub clauses: Vec<Clause>,
    /// `Some` empty clause when a unit disequality contradicts its own
    /// class. The caller proves it from provenance plus the full
    /// originals (the empty cites the disequality and its path units).
    pub contradiction: Option<Clause>,
}

/// Expand a grounded clause set with ground equational reasoning:
/// union-find normalization (derived `equality_normalization` steps),
/// reflexivity fast paths, and transitivity cubes over distinct
/// representatives (`equality_transitivity` validities). Predicate-only
/// inputs pass through byte-identical (no Eq atoms anywhere): the legacy
/// fragment observes zero behavior change.
pub(crate) fn expand_equality(
    clauses: &[Clause],
    ordering: &TermOrdering,
    id_gen: &mut ClauseIdGen,
) -> Result<ExpandedEq, CertificationFailure> {
    // Seed every constant occurring in an equality literal so the
    // representative enumeration below covers the whole equational
    // vocabulary, then union the ground positive unit equalities.
    // Non-ground equality sides are skipped defensively (post-grounding
    // inputs are always ground; skipping merely loses completeness,
    // never soundness).
    fn is_ground_constant(term: &Term) -> bool {
        matches!(term, Term::App(_, args) if args.is_empty())
    }
    let mut classes = EqClasses::new();
    for clause in clauses {
        for literal in &clause.literals {
            let Atom::Eq(left, right) = &literal.atom else {
                continue;
            };
            for side in [left, right] {
                if is_ground_constant(side) {
                    classes.singleton(side);
                }
            }
        }
    }
    for clause in clauses {
        if clause.literals.len() != 1 || !clause.literals[0].positive {
            continue;
        }
        if let Atom::Eq(left, right) = &clause.literals[0].atom
            && is_ground_constant(left)
            && is_ground_constant(right)
        {
            classes.union(left, right, clause.id);
        }
    }
    let mut expanded = Vec::with_capacity(clauses.len());
    for clause in clauses {
        match normalize_clause_eq(clause, &mut classes, ordering) {
            Normalized::Unchanged => {
                expanded.push(clause.clone());
            }
            Normalized::Tautology => {
                // Valid clause: drop silently (like subsumed clauses).
            }
            Normalized::Contradiction { explains } => {
                let mut parents = vec![clause.id];
                parents.extend(explains.iter().copied());
                parents.sort_unstable_by_key(|id| id.0);
                parents.dedup();
                let empty = Clause::new(
                    id_gen.next(),
                    Vec::new(),
                    ClauseSource::Inference {
                        rule: "equality_normalization",
                        parents: parents.into(),
                    },
                );
                return Ok(ExpandedEq {
                    clauses: expanded,
                    contradiction: Some(empty),
                });
            }
            Normalized::Rewritten { literals, explains } => {
                let mut parents = vec![clause.id];
                parents.extend(explains.iter().copied());
                parents.sort_unstable_by_key(|id| id.0);
                parents.dedup();
                expanded.push(Clause::new(
                    id_gen.next(),
                    literals,
                    ClauseSource::Inference {
                        rule: "equality_normalization",
                        parents: parents.into(),
                    },
                ));
            }
        }
    }
    // Transitivity cubes over pairwise-distinct representatives. Degenerate
    // triples (repeated reps) are valid-but-useless: normalization already
    // covers reflexivity, so only distinct triples are generated.
    let reps = classes.representatives();
    let cube_estimate = reps
        .len()
        .checked_mul(reps.len())
        .and_then(|square| square.checked_mul(reps.len()));
    if let Some(estimate) = cube_estimate
        && estimate > MAX_TRANSITIVITY_CUBES
    {
        return Err(CertificationFailure::Limit(
            "equality transitivity cube limit exceeded",
        ));
    }
    for (i, first) in reps.iter().enumerate() {
        for (j, second) in reps.iter().enumerate() {
            if i == j {
                continue;
            }
            for (k, third) in reps.iter().enumerate() {
                if k == i || k == j {
                    continue;
                }
                let mut lits = vec![
                    Literal::neg(Atom::Eq(first.clone(), second.clone())),
                    Literal::neg(Atom::Eq(second.clone(), third.clone())),
                    Literal::pos(Atom::Eq(first.clone(), third.clone())),
                ];
                // Canonical orientation per literal (same convention as
                // normalization, so cubes resolve against rewritten sets).
                for lit in &mut lits {
                    let Atom::Eq(left, right) = &lit.atom else {
                        continue;
                    };
                    let (ordered_left, ordered_right) = canonical_eq_order(left, right, ordering);
                    lit.atom = Atom::Eq(ordered_left, ordered_right);
                }
                expanded.push(Clause::new(
                    id_gen.next(),
                    lits,
                    ClauseSource::Inference {
                        rule: "equality_transitivity",
                        parents: smallvec::SmallVec::new(),
                    },
                ));
            }
        }
    }
    Ok(ExpandedEq {
        clauses: expanded,
        contradiction: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::symbol::SymbolTable;

    fn constants(symbols: &mut SymbolTable, names: &[&str]) -> Vec<Term> {
        names
            .iter()
            .map(|name| Term::constant(symbols.intern(name)))
            .collect()
    }

    #[test]
    fn union_find_merges_and_explains_paths() {
        let mut symbols = SymbolTable::new();
        let terms = constants(&mut symbols, &["a", "b", "c", "d"]);
        let (a, b, c, d) = (&terms[0], &terms[1], &terms[2], &terms[3]);
        let mut classes = EqClasses::new();
        // Disconnected at first.
        assert!(classes.explain(a, b).is_none());
        classes.union(a, b, ClauseId(10));
        classes.union(b, c, ClauseId(11));
        // Transitive path a-b-c explains a~c with both equations.
        let mut path = classes.explain(a, c).expect("a and c are connected");
        path.sort_unstable_by_key(|id| id.0);
        assert_eq!(path, vec![ClauseId(10), ClauseId(11)]);
        // d stays disconnected.
        assert!(classes.explain(a, d).is_none());
    }

    #[test]
    fn representatives_cover_singletons_and_merges() {
        let mut symbols = SymbolTable::new();
        let terms = constants(&mut symbols, &["a", "b", "c"]);
        let mut classes = EqClasses::new();
        for term in &terms {
            classes.singleton(term);
        }
        assert_eq!(classes.representatives().len(), 3);
        classes.union(&terms[0], &terms[1], ClauseId(7));
        assert_eq!(classes.representatives().len(), 2);
    }

    #[test]
    fn canonical_orientation_is_symmetric_and_deterministic() {
        use crate::TermOrdering;
        let mut symbols = SymbolTable::new();
        let terms = constants(&mut symbols, &["a", "b"]);
        for ordering in [TermOrdering::KBO, TermOrdering::LPO] {
            let (first, second) = canonical_eq_order(&terms[0], &terms[1], &ordering);
            let (first_swapped, second_swapped) =
                canonical_eq_order(&terms[1], &terms[0], &ordering);
            assert_eq!(
                (first.clone(), second.clone()),
                (first_swapped, second_swapped)
            );
            // Repeatability: same inputs, same outputs.
            let again = canonical_eq_order(&terms[0], &terms[1], &ordering);
            assert_eq!((first, second), again);
        }
    }
}

/// Ordering view of an atom for maximal-literal selection and totality
/// validation: predicates as before, equalities wrapped in the reserved
/// pseudo-symbol (ordering math only — never interned, rendered, or
/// collected into signatures).
pub(crate) fn atom_term_eq(atom: &Atom) -> Term {
    match atom {
        Atom::Pred(predicate, args) => Term::app(*predicate, args.clone()),
        Atom::Eq(left, right) => Term::app(
            mrs_core::symbol::SymbolId::RESERVED_EQ_ORDER,
            vec![left.clone(), right.clone()],
        ),
    }
}
