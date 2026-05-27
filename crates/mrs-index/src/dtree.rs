//! Discrimination tree for first-order term indexing.
//!
//! A discrimination tree is a trie where terms are stored based on their
//! pre-order DFS traversal. Each node in the tree corresponds to one cell
//! in the flattened term representation.
//!
//! For example, `f(a, g(X))` is flattened to `[Sym(f,2), Sym(a,0), Sym(g,1), Star]`.
//!
//! The tree supports three retrieval modes:
//! - **Unification** (imperfect): returns a superset of terms unifiable with the query
//! - **Generalization**: returns terms more general than the query (for demodulation)
//! - **Instance**: returns terms more specific than the query

use mrs_core::symbol::SymbolId;
use mrs_core::term::{Term, VarId};
use std::collections::HashMap;

/// A cell in the flattened term representation.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
enum Cell {
    /// Function symbol with its arity.
    Sym(SymbolId, u8),
    /// Variable with its normalized ID.
    Var(VarId),
}

/// Flatten a term into its pre-order DFS cell representation, normalizing variables.
fn flatten(term: &Term) -> Vec<Cell> {
    let mut cells = Vec::new();
    let mut var_map = HashMap::new();
    let mut next_var = 0;
    flatten_into(term, &mut cells, &mut var_map, &mut next_var);
    cells
}

fn flatten_into(
    term: &Term,
    out: &mut Vec<Cell>,
    var_map: &mut HashMap<VarId, VarId>,
    next_var: &mut VarId,
) {
    match term {
        Term::Var(v) => {
            let norm_v = *var_map.entry(*v).or_insert_with(|| {
                let id = *next_var;
                *next_var += 1;
                id
            });
            out.push(Cell::Var(norm_v));
        }
        Term::App(sym, args) => {
            out.push(Cell::Sym(*sym, args.len() as u8));
            for arg in args {
                flatten_into(arg, out, var_map, next_var);
            }
        }
    }
}

/// Returns the position after the subterm starting at `pos` in a flat representation.
fn skip_in_flat(flat: &[Cell], pos: usize) -> usize {
    match flat[pos] {
        Cell::Var(_) => pos + 1,
        Cell::Sym(_, n) => {
            let mut p = pos + 1;
            for _ in 0..n {
                p = skip_in_flat(flat, p);
            }
            p
        }
    }
}

/// A discrimination tree mapping terms to values.
///
/// Supports efficient retrieval of terms that unify with, generalize,
/// or are instances of a query term.
pub struct DTree<V> {
    children: HashMap<Cell, DTree<V>>,
    leaves: Vec<V>,
}

impl<V: Clone + PartialEq> DTree<V> {
    /// Creates an empty discrimination tree.
    pub fn new() -> Self {
        DTree {
            children: HashMap::new(),
            leaves: Vec::new(),
        }
    }

    /// Inserts a term-value pair into the tree.
    pub fn insert(&mut self, term: &Term, value: V) {
        let flat = flatten(term);
        self.insert_flat(&flat, 0, value);
    }

    fn insert_flat(&mut self, flat: &[Cell], pos: usize, value: V) {
        if pos >= flat.len() {
            self.leaves.push(value);
            return;
        }
        let child = self.children.entry(flat[pos]).or_default();
        child.insert_flat(flat, pos + 1, value);
    }

    /// Removes a term-value pair from the tree.
    ///
    /// Returns `true` if the pair was found and removed.
    pub fn remove(&mut self, term: &Term, value: &V) -> bool {
        let flat = flatten(term);
        self.remove_flat(&flat, 0, value)
    }

    fn remove_flat(&mut self, flat: &[Cell], pos: usize, value: &V) -> bool {
        if pos >= flat.len() {
            if let Some(idx) = self.leaves.iter().position(|v| v == value) {
                self.leaves.swap_remove(idx);
                return true;
            }
            return false;
        }
        if let Some(child) = self.children.get_mut(&flat[pos]) {
            child.remove_flat(flat, pos + 1, value)
        } else {
            false
        }
    }

    /// Returns all values associated with terms that may unify with the query.
    ///
    /// This is an *imperfect* filter: it may return false positives (terms that
    /// don't actually unify), but never misses a truly unifiable term. Callers
    /// should verify unifiability on the returned candidates.
    pub fn get_unifiable(&self, query: &Term) -> Vec<V> {
        let flat = flatten(query);
        let mut results = Vec::new();
        self.unify_flat(&flat, 0, &mut results);
        results
    }

    fn unify_flat(&self, query: &[Cell], pos: usize, results: &mut Vec<V>) {
        if pos >= query.len() {
            results.extend_from_slice(&self.leaves);
            return;
        }

        match query[pos] {
            Cell::Sym(f, n) => {
                // Exact match: same symbol in the tree
                if let Some(child) = self.children.get(&Cell::Sym(f, n)) {
                    child.unify_flat(query, pos + 1, results);
                }
                // Var in tree: stored variable unifies with any query subterm
                for (&key, child) in &self.children {
                    if let Cell::Var(_) = key {
                        let skip = skip_in_flat(query, pos);
                        child.unify_flat(query, skip, results);
                    }
                }
            }
            Cell::Var(_) => {
                // Query variable: unifies with anything stored
                for (&key, child) in &self.children {
                    match key {
                        Cell::Var(_) => {
                            // Both variables: advance both
                            child.unify_flat(query, pos + 1, results);
                        }
                        Cell::Sym(_, n) => {
                            // Stored function, query variable: skip stored subterm's children
                            child.skip_stored(n as usize, query, pos + 1, results);
                        }
                    }
                }
            }
        }
    }

    /// Skip `remaining` stored child subterms in the tree, then continue unification.
    fn skip_stored(
        &self,
        remaining: usize,
        query: &[Cell],
        query_pos: usize,
        results: &mut Vec<V>,
    ) {
        if remaining == 0 {
            self.unify_flat(query, query_pos, results);
            return;
        }
        for (&key, child) in &self.children {
            match key {
                Cell::Var(_) => {
                    // One stored variable = one child subterm skipped
                    child.skip_stored(remaining - 1, query, query_pos, results);
                }
                Cell::Sym(_, m) => {
                    // One stored Sym(g, m) = one child subterm, which itself has m children
                    child.skip_stored(remaining - 1 + m as usize, query, query_pos, results);
                }
            }
        }
    }

    /// Returns all values associated with terms that are more general than the query.
    ///
    /// A stored term `t` generalizes query `q` if `∃σ. σ(t) = q`.
    /// Used for demodulation: find rewrite rules whose LHS matches a subterm.
    ///
    /// The query should typically be ground (no variables). If it has variables,
    /// only stored variables can match query variables.
    pub fn get_generalizations(&self, query: &Term) -> Vec<V> {
        let flat = flatten(query);
        let mut results = Vec::new();
        let mut bindings = Vec::new();
        self.gen_flat(&flat, 0, &mut bindings, &mut results);
        results
    }

    fn gen_flat<'a>(
        &self,
        query: &'a [Cell],
        pos: usize,
        bindings: &mut Vec<Option<&'a [Cell]>>,
        results: &mut Vec<V>,
    ) {
        if pos >= query.len() {
            results.extend_from_slice(&self.leaves);
            return;
        }

        match query[pos] {
            Cell::Sym(f, n) => {
                // Exact match
                if let Some(child) = self.children.get(&Cell::Sym(f, n)) {
                    child.gen_flat(query, pos + 1, bindings, results);
                }
                // Var in tree: stored variable generalizes any query subterm
                for (&key, child) in &self.children {
                    if let Cell::Var(vid) = key {
                        let vid = vid as usize;
                        let skip = skip_in_flat(query, pos);
                        let subterm = &query[pos..skip];

                        if vid >= bindings.len() {
                            bindings.resize(vid + 1, None);
                        }

                        if let Some(bound) = bindings[vid] {
                            if bound == subterm {
                                child.gen_flat(query, skip, bindings, results);
                            }
                        } else {
                            bindings[vid] = Some(subterm);
                            child.gen_flat(query, skip, bindings, results);
                            bindings[vid] = None; // backtrack
                        }
                    }
                }
            }
            Cell::Var(_) => {
                // Query variable: only stored variables can generalize a variable
                for (&key, child) in &self.children {
                    if let Cell::Var(vid) = key {
                        let vid = vid as usize;
                        let subterm = &query[pos..pos + 1];

                        if vid >= bindings.len() {
                            bindings.resize(vid + 1, None);
                        }

                        if let Some(bound) = bindings[vid] {
                            if bound == subterm {
                                child.gen_flat(query, pos + 1, bindings, results);
                            }
                        } else {
                            bindings[vid] = Some(subterm);
                            child.gen_flat(query, pos + 1, bindings, results);
                            bindings[vid] = None; // backtrack
                        }
                    }
                }
            }
        }
    }

    /// Returns all values associated with terms that are instances of the query.
    ///
    /// A stored term `t` is an instance of query `q` if `∃σ. σ(q) = t`.
    /// Used for backward subsumption: find stored clauses subsumed by a pattern.
    pub fn get_instances(&self, query: &Term) -> Vec<V> {
        let flat = flatten(query);
        let mut results = Vec::new();
        self.inst_flat(&flat, 0, &mut results);
        results
    }

    fn inst_flat(&self, query: &[Cell], pos: usize, results: &mut Vec<V>) {
        if pos >= query.len() {
            results.extend_from_slice(&self.leaves);
            return;
        }

        match query[pos] {
            Cell::Sym(f, n) => {
                // Exact match only: stored must have the same symbol
                if let Some(child) = self.children.get(&Cell::Sym(f, n)) {
                    child.inst_flat(query, pos + 1, results);
                }
                // Var in tree: a variable is not an instance of a function term
            }
            Cell::Var(_) => {
                // Query variable: matches any stored subterm (it's an instance)
                for (&key, child) in &self.children {
                    match key {
                        Cell::Var(_) => {
                            child.inst_flat(query, pos + 1, results);
                        }
                        Cell::Sym(_, n) => {
                            // Skip stored function's children, then continue
                            child.skip_stored(n as usize, query, pos + 1, results);
                        }
                    }
                }
            }
        }
    }

    /// Returns true if the tree contains no entries.
    pub fn is_empty(&self) -> bool {
        self.leaves.is_empty() && self.children.values().all(|c| c.is_empty())
    }
}

impl<V: Clone + PartialEq> Default for DTree<V> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::symbol::SymbolTable;

    fn make_terms(syms: &mut SymbolTable) -> (SymbolId, SymbolId, SymbolId, SymbolId) {
        let f = syms.intern("f");
        let g = syms.intern("g");
        let a = syms.intern("a");
        let b = syms.intern("b");
        (f, g, a, b)
    }

    #[test]
    fn insert_and_retrieve_exact() {
        let mut syms = SymbolTable::new();
        let (f, _g, a, _b) = make_terms(&mut syms);
        let mut tree = DTree::new();

        // Insert f(a)
        let term = Term::app(f, vec![Term::constant(a)]);
        tree.insert(&term, 1);

        // Query for f(a) — exact match
        let results = tree.get_unifiable(&term);
        assert_eq!(results, vec![1]);
    }

    #[test]
    fn unify_stored_variable() {
        let mut syms = SymbolTable::new();
        let (f, _g, a, _b) = make_terms(&mut syms);
        let mut tree = DTree::new();

        // Store f(X) — variable at arg position
        tree.insert(&Term::app(f, vec![Term::var(0)]), 1);

        // Query f(a) — should find f(X) as unifiable
        let results = tree.get_unifiable(&Term::app(f, vec![Term::constant(a)]));
        assert_eq!(results, vec![1]);
    }

    #[test]
    fn unify_query_variable() {
        let mut syms = SymbolTable::new();
        let (f, _g, a, _b) = make_terms(&mut syms);
        let mut tree = DTree::new();

        // Store f(a)
        tree.insert(&Term::app(f, vec![Term::constant(a)]), 1);

        // Query f(X) — should find f(a) as unifiable
        let results = tree.get_unifiable(&Term::app(f, vec![Term::var(0)]));
        assert_eq!(results, vec![1]);
    }

    #[test]
    fn unify_both_variables() {
        let mut syms = SymbolTable::new();
        let (f, _g, _a, _b) = make_terms(&mut syms);
        let mut tree = DTree::new();

        // Store f(X)
        tree.insert(&Term::app(f, vec![Term::var(0)]), 1);

        // Query f(Y) — should find f(X) as unifiable
        let results = tree.get_unifiable(&Term::app(f, vec![Term::var(1)]));
        assert_eq!(results, vec![1]);
    }

    #[test]
    fn unify_no_match() {
        let mut syms = SymbolTable::new();
        let (f, g, a, _b) = make_terms(&mut syms);
        let mut tree = DTree::new();

        // Store f(a)
        tree.insert(&Term::app(f, vec![Term::constant(a)]), 1);

        // Query g(a) — different symbol, no match
        let results = tree.get_unifiable(&Term::app(g, vec![Term::constant(a)]));
        assert!(results.is_empty());
    }

    #[test]
    fn unify_nested_terms() {
        let mut syms = SymbolTable::new();
        let (f, g, a, _b) = make_terms(&mut syms);
        let mut tree = DTree::new();

        // Store f(g(a), X)
        tree.insert(
            &Term::app(f, vec![Term::app(g, vec![Term::constant(a)]), Term::var(0)]),
            1,
        );
        // Store f(X, a)
        tree.insert(&Term::app(f, vec![Term::var(0), Term::constant(a)]), 2);

        // Query f(g(a), a) — should match both
        let query = Term::app(
            f,
            vec![Term::app(g, vec![Term::constant(a)]), Term::constant(a)],
        );
        let mut results = tree.get_unifiable(&query);
        results.sort();
        assert_eq!(results, vec![1, 2]);
    }

    #[test]
    fn unify_query_var_skips_stored_subtree() {
        let mut syms = SymbolTable::new();
        let (f, g, a, b) = make_terms(&mut syms);
        let mut tree = DTree::new();

        // Store f(g(a, b)) — stored has nested structure
        tree.insert(
            &Term::app(
                f,
                vec![Term::app(g, vec![Term::constant(a), Term::constant(b)])],
            ),
            1,
        );

        // Query f(X) — variable should skip the entire g(a,b) subtree
        let results = tree.get_unifiable(&Term::app(f, vec![Term::var(0)]));
        assert_eq!(results, vec![1]);
    }

    #[test]
    fn generalizations_find_pattern() {
        let mut syms = SymbolTable::new();
        let (f, _g, a, _b) = make_terms(&mut syms);
        let mut tree = DTree::new();

        // Store f(X) — more general pattern
        tree.insert(&Term::app(f, vec![Term::var(0)]), 1);
        // Store f(a) — equally specific
        tree.insert(&Term::app(f, vec![Term::constant(a)]), 2);

        // Query f(a) — both f(X) and f(a) generalize f(a)
        let mut results = tree.get_generalizations(&Term::app(f, vec![Term::constant(a)]));
        results.sort();
        assert_eq!(results, vec![1, 2]);
    }

    #[test]
    fn generalizations_no_over_specific() {
        let mut syms = SymbolTable::new();
        let (f, _g, a, b) = make_terms(&mut syms);
        let mut tree = DTree::new();

        // Store f(a)
        tree.insert(&Term::app(f, vec![Term::constant(a)]), 1);

        // Query f(b) — f(a) does NOT generalize f(b)
        let results = tree.get_generalizations(&Term::app(f, vec![Term::constant(b)]));
        assert!(results.is_empty());
    }

    #[test]
    fn instances_find_specific() {
        let mut syms = SymbolTable::new();
        let (f, _g, a, b) = make_terms(&mut syms);
        let mut tree = DTree::new();

        // Store f(a), f(b), f(X)
        tree.insert(&Term::app(f, vec![Term::constant(a)]), 1);
        tree.insert(&Term::app(f, vec![Term::constant(b)]), 2);
        tree.insert(&Term::app(f, vec![Term::var(0)]), 3);

        // Query f(X) — f(a), f(b), f(X) are all instances of f(X)
        let mut results = tree.get_instances(&Term::app(f, vec![Term::var(0)]));
        results.sort();
        assert_eq!(results, vec![1, 2, 3]);
    }

    #[test]
    fn instances_no_more_general() {
        let mut syms = SymbolTable::new();
        let (f, _g, a, _b) = make_terms(&mut syms);
        let mut tree = DTree::new();

        // Store f(X) — more general than f(a)
        tree.insert(&Term::app(f, vec![Term::var(0)]), 1);

        // Query f(a) — f(X) is NOT an instance of f(a)
        let results = tree.get_instances(&Term::app(f, vec![Term::constant(a)]));
        assert!(results.is_empty());
    }

    #[test]
    fn remove_entry() {
        let mut syms = SymbolTable::new();
        let (f, _g, a, _b) = make_terms(&mut syms);
        let mut tree = DTree::new();

        let term = Term::app(f, vec![Term::constant(a)]);
        tree.insert(&term, 1);
        tree.insert(&term, 2);

        assert!(tree.remove(&term, &1));
        let results = tree.get_unifiable(&term);
        assert_eq!(results, vec![2]);
    }

    #[test]
    fn constant_terms() {
        let mut syms = SymbolTable::new();
        let a = syms.intern("a");
        let b = syms.intern("b");
        let mut tree = DTree::new();

        tree.insert(&Term::constant(a), 1);
        tree.insert(&Term::constant(b), 2);

        assert_eq!(tree.get_unifiable(&Term::constant(a)), vec![1]);
        assert_eq!(tree.get_unifiable(&Term::constant(b)), vec![2]);

        // Variable matches both
        let mut results = tree.get_unifiable(&Term::var(0));
        results.sort();
        assert_eq!(results, vec![1, 2]);
    }
}
