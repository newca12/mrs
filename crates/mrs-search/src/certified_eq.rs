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
//! 4. Positive equality clauses must be unit clauses. This is a deliberate
//!    certification boundary: unit equalities can be applied as a complete
//!    congruence normalization before closure. A non-unit positive equality
//!    may be derived later by resolution; supporting that case would require
//!    predicate-congruence axioms throughout the closure, so it fails closed.
//!
//! Why this is complete: the supported expanded set is a finite ground
//! equational problem with unit equality definitions — plain ground
//! resolution (Tier 1, both closures) and propositional SAT (Tier 2, equality
//! as atoms) decide it, and the double-closure agreement plus model re-check
//! carry over verbatim.
//! Everything beyond the expansion caps fails closed as `Limit`.

use std::collections::{HashMap as StdHashMap, VecDeque};
use std::time::Instant;

use crate::TermOrdering;
use crate::certified::CertificationFailure;
use mrs_calculus::ordering::TermComparison;
use mrs_core::clause::{Clause, ClauseId, ClauseIdGen, ClauseSource, Literal};
use mrs_core::formula::Atom;
use mrs_core::term::Term;

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

    /// Every known constant paired with its class representative, sorted.
    /// Handed to the model-certificate builder so the certificate interprets
    /// merged constants as one element.
    pub(crate) fn into_pairs(self) -> Vec<(Term, Term)> {
        let mut pairs: Vec<(Term, Term)> = self
            .parent
            .keys()
            .map(|term| {
                let mut classes = EqClasses {
                    parent: self.parent.clone(),
                    rank: self.rank.clone(),
                    edges: StdHashMap::default(),
                };
                let root = classes.find_root(term);
                (term.clone(), root)
            })
            .collect();
        // `Term` is not `Ord`; the keys are constants, so the symbol id is a
        // total order over them and the output stays deterministic.
        pairs.sort_by_key(|(constant, _)| match constant {
            Term::App(symbol, args) if args.is_empty() => symbol.index(),
            _ => u32::MAX,
        });
        pairs
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
        match &literal.atom {
            Atom::Pred(predicate, args) => {
                let new_args = args
                    .iter()
                    .map(|arg| normalize_term_eq(arg, classes, &mut explains, &mut changed))
                    .collect();
                literals.push(Literal {
                    positive: literal.positive,
                    atom: Atom::Pred(*predicate, new_args),
                });
            }
            Atom::Eq(left, right) => {
                let mut new_left = normalize_term_eq(left, classes, &mut explains, &mut changed);
                let mut new_right = normalize_term_eq(right, classes, &mut explains, &mut changed);
                let class_changed = new_left != *left || new_right != *right;
                if new_left == new_right {
                    if literal.positive {
                        return Normalized::Tautology;
                    }
                    changed = true;
                    continue;
                }
                // Unique-name resolution. Both sides are ground constants, so
                // distinct class representatives denote distinct elements in
                // every model, and the disequality's truth value is known
                // without any congruence reasoning: a *negative* `Eq` between
                // distinct representatives is true, which makes the whole
                // clause valid, and a *positive* one is false, which drops the
                // literal. This is what lets a non-unit positive equality
                // reach the tiers at all — the guard above used to refuse such
                // inputs outright because resolution cannot generate the
                // congruence axioms it would need, and here they are not
                // needed at all.
                //
                // Only ground constants qualify. A non-ground side means the
                // literal survives to the tiers, and the top-of-function guard
                // refuses a non-unit positive one.
                if is_ground_constant(&new_left) && is_ground_constant(&new_right) {
                    if !literal.positive {
                        return Normalized::Tautology;
                    }
                    changed = true;
                    continue;
                }
                // A pure orientation change has no equality parent to cite.
                // Preserve the original orientation in that case; when a
                // unit equality changed a side, the unit parents justify the
                // canonical reorientation as part of the same normalization.
                if class_changed {
                    (new_left, new_right) = canonical_eq_order(&new_left, &new_right, ordering);
                    changed = true;
                }
                literals.push(Literal {
                    positive: literal.positive,
                    atom: Atom::Eq(new_left, new_right),
                });
            }
        }
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

fn normalize_term_eq(
    term: &Term,
    classes: &mut EqClasses,
    explains: &mut Vec<ClauseId>,
    changed: &mut bool,
) -> Term {
    let normalized = match term {
        Term::Var(_) => term.clone(),
        Term::App(symbol, args) if args.is_empty() => classes.representative(term),
        Term::App(symbol, args) => Term::app(
            *symbol,
            args.iter()
                .map(|arg| normalize_term_eq(arg, classes, explains, changed))
                .collect(),
        ),
    };
    if &normalized != term {
        *changed = true;
        if let Some(mut path) = classes.explain(term, &normalized) {
            explains.append(&mut path);
        }
    }
    normalized
}

/// Result of [`expand_equality`]: normalized clauses (with derivation
/// steps) and an optional immediate refutation.
pub(crate) struct ExpandedEq {
    pub clauses: Vec<Clause>,
    /// `Some` empty clause when a unit disequality contradicts its own
    /// class. The caller proves it from provenance plus the full
    /// originals (the empty cites the disequality and its path units).
    pub contradiction: Option<Clause>,
    /// Constant -> the class representative every occurrence of it was
    /// rewritten to.
    ///
    /// This is the *other half* of the normalization and a model certificate
    /// must respect it. A model with a distinct domain element per constant
    /// cannot satisfy a unit equality that merged two of them, so a
    /// certificate built from the pre-merge vocabulary asserts `a != b` for a
    /// clause set containing `a = b`. Handing this map to the certificate
    /// builder makes the two agree: every constant in a class is interpreted
    /// as its representative's element, which is exactly what makes the
    /// disequalities the unique-name pass dropped true in the model.
    pub class_representatives: Vec<(Term, Term)>,
}

/// Expand a grounded clause set with ground equational reasoning:
/// unit-equality union-find normalization (derived
/// `equality_normalization` steps) and reflexivity fast paths. Predicate-only
/// inputs pass through byte-identical (no Eq atoms anywhere): the legacy
/// fragment observes zero behavior change.
/// Is this term a ground constant (a function application with no arguments)?
/// Hoisted to module scope because the unique-name resolution in
/// `normalize_clause_eq` and the fragment guard at the top of
/// `expand_equality` must agree on exactly this notion of "ground".
fn is_ground_constant(term: &Term) -> bool {
    matches!(term, Term::App(_, args) if args.is_empty())
}

pub(crate) fn expand_equality(
    clauses: &[Clause],
    ordering: &TermOrdering,
    id_gen: &mut ClauseIdGen,
    deadline: Instant,
) -> Result<ExpandedEq, CertificationFailure> {
    // The closure does not generate predicate-congruence axioms for positive
    // equality clauses derived later by resolution. Accepting a non-unit
    // positive equality could therefore turn an equality-dependent
    // contradiction into a false saturation claim.
    //
    // `normalize_clause_eq` resolves the ground case outright under the
    // unique-name axiom, so this guard only has to catch what survives that
    // pass: a positive equality with a non-ground side, whose value is not
    // known and would have to be left to resolution.
    if clauses.iter().any(|clause| {
        clause.literals.len() != 1
            && clause.literals.iter().any(|literal| {
                literal.positive
                    && matches!(&literal.atom, Atom::Eq(left, right)
                        if !(is_ground_constant(left) && is_ground_constant(right)))
            })
    }) {
        return Err(CertificationFailure::Unsupported(
            "non-unit positive equality is outside the certified fragment",
        ));
    }

    // Seed every constant occurring in an equality literal so the
    // representative enumeration below covers the whole equational
    // vocabulary, then union the ground positive unit equalities.
    // Non-ground equality sides are skipped defensively (post-grounding
    // inputs are always ground; skipping merely loses completeness,
    // never soundness).
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
    for (index, clause) in clauses.iter().enumerate() {
        // Amortized deadline check. This pass allocates a clause per changed
        // input and unions through every ground term, so on a million-clause
        // grounding it is one of the longest single steps in the certifier and
        // had no way to be interrupted: NLP115-1 spent 90 s here on a 60 s
        // budget, growing to 13.8 GB, and was killed rather than reported.
        if index & 0xFFF == 0 && Instant::now() >= deadline {
            return Err(CertificationFailure::Limit(
                "certification time limit exceeded",
            ));
        }
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
                dedup_clause_ids_preserving_order(&mut parents);
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
                    class_representatives: classes.into_pairs(),
                });
            }
            Normalized::Rewritten { literals, explains } => {
                let mut parents = vec![clause.id];
                parents.extend(explains.iter().copied());
                dedup_clause_ids_preserving_order(&mut parents);
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
    Ok(ExpandedEq {
        clauses: expanded,
        contradiction: None,
        class_representatives: classes.into_pairs(),
    })
}

fn dedup_clause_ids_preserving_order(ids: &mut Vec<ClauseId>) {
    let mut seen = std::collections::HashSet::new();
    ids.retain(|id| seen.insert(*id));
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
        assert_eq!(classes.parent.len(), 3);
        classes.union(&terms[0], &terms[1], ClauseId(7));
        let keys: Vec<_> = classes.parent.keys().cloned().collect();
        assert_eq!(
            keys.iter()
                .map(|term| classes.find_root(term))
                .collect::<std::collections::HashSet<_>>()
                .len(),
            2
        );
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
