//! Path-compressed discrimination tree (substitution tree) for first-order term indexing.
//!
//! `STreeId<V>` is a drop-in replacement for `DTreeId<V>` that reduces node count by
//! compressing linear chains of single-child trie nodes into multi-cell *edge labels*.
//!
//! # Structure
//!
//! Every trie node stores a `BTreeMap<Cell, (Vec<Cell>, STreeId<V>)>`:
//! - **Key** = first `Cell` of the edge (fast O(1) dispatch on symbol / variable).
//! - **Value** = `(edge_rest, child)` where `edge_rest` is the remainder of the
//!   compressed edge label (may be empty, equivalent to `DTreeId`).
//!
//! ## Example
//!
//! For three stored terms with flat paths:
//! ```text
//! f(g(a))  →  [f/2, g/1, a/0]
//! f(g(b))  →  [f/2, g/1, b/0]
//! f(h(X))  →  [f/2, h/1, Var(0)]
//! ```
//!
//! `DTreeId` creates 5 single-child nodes before branching.
//! `STreeId` stores one node with key `f/2`, edge label `[g/1]` / `[h/1]`,
//! branching at the second level — cutting node count in half for this prefix.
//!
//! More dramatically: if 50 000 clauses all begin with `f(g(h(`, the three-cell
//! common prefix is stored once in a single compressed edge instead of three
//! nested `HashMap` allocations (≥144 bytes saved per stored term on a 64-bit
//! platform where `HashMap` baseline is ≥48 bytes).
//!
//! # Retrieval semantics
//!
//! Identical to `DTreeId<V>`: imperfect unification (superset), exact
//! generalization.  Variable bindings are threaded and checked for consistency.

use std::collections::BTreeMap;
use std::ops::Range;

use mrs_core::term_bank::{IdAtom, TermBank, TermId};

use crate::dtree::{Cell, flatten_atom_id, flatten_id, skip_in_flat};

// ── helpers ──────────────────────────────────────────────────────────────────

/// Length of the longest common prefix between two `Cell` slices (exact equality).
#[inline]
fn common_prefix_len(a: &[Cell], b: &[Cell]) -> usize {
    a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count()
}

// ── STreeId ──────────────────────────────────────────────────────────────────

/// A path-compressed discrimination tree for `TermId`-keyed terms.
///
/// See the module documentation for a full structural description.
pub struct STreeId<V> {
    /// Compressed edges to children.
    ///
    /// Key   = first `Cell` of the edge (O(1) dispatch).
    /// Value = `(edge_rest, child)`:
    ///   - `edge_rest`: remaining cells on this edge after the key cell.
    ///   - `child`:     sub-tree rooted after the full edge `[key] ++ edge_rest`.
    children: BTreeMap<Cell, (Vec<Cell>, STreeId<V>)>,
    /// Values stored at this node (full path consumed).
    leaves: Vec<V>,
}

impl<V: Clone + PartialEq> STreeId<V> {
    /// Creates an empty substitution tree.
    pub fn new() -> Self {
        STreeId {
            children: BTreeMap::new(),
            leaves: Vec::new(),
        }
    }

    // ── insert ────────────────────────────────────────────────────────────────

    /// Insert `value` indexed by the pre-order flattening of `atom`.
    pub fn insert_atom(&mut self, atom: &IdAtom, bank: &TermBank, value: V) {
        let flat = flatten_atom_id(atom, bank);
        self.insert_flat(&flat, 0, value);
    }

    /// Insert `value` indexed by the pre-order flattening of `term`.
    pub fn insert(&mut self, term: TermId, bank: &TermBank, value: V) {
        let flat = flatten_id(term, bank);
        self.insert_flat(&flat, 0, value);
    }

    fn insert_flat(&mut self, flat: &[Cell], pos: usize, value: V) {
        if pos == flat.len() {
            if !self.leaves.contains(&value) {
                self.leaves.push(value);
            }
            return;
        }

        let first = flat[pos];
        let new_rest = &flat[pos + 1..]; // cells after `first`

        match self.children.get_mut(&first) {
            None => {
                // No existing edge with this first cell — create a leaf child.
                let mut leaf = STreeId::new();
                leaf.leaves.push(value);
                self.children.insert(first, (new_rest.to_vec(), leaf));
            }
            Some((edge_rest, child)) => {
                // Find the longest common prefix of new_rest and edge_rest.
                let k = common_prefix_len(new_rest, edge_rest);

                if k == edge_rest.len() {
                    // edge_rest is a full prefix of new_rest → recurse into child.
                    child.insert_flat(flat, pos + 1 + k, value);
                } else {
                    // Partial match: split the edge at divergence position k.
                    //
                    // Before: first --[edge_rest]--> old_child
                    // After:  first --[edge_rest[..k]]--> split_node
                    //                                       |
                    //             edge_rest[k] --[edge_rest[k+1..]]--> old_child
                    //             new_rest[k]  --[new_rest [k+1..]]--> new_leaf
                    //
                    // (If k == new_rest.len() the new term ends at the split node.)

                    let diverge_old = edge_rest[k];
                    let old_tail = edge_rest[k + 1..].to_vec();
                    let common = edge_rest[..k].to_vec();

                    // Take the old child out so we can mutably reborrow.
                    let old_child = std::mem::replace(child, STreeId::new());
                    {
                        let entry = self.children.get_mut(&first).unwrap();
                        entry.0 = common;
                        // entry.1 is a placeholder STreeId::new(); filled below.
                    }

                    let mut split_node = STreeId::new();
                    split_node
                        .children
                        .insert(diverge_old, (old_tail, old_child));

                    if k < new_rest.len() {
                        let diverge_new = new_rest[k];
                        let new_tail = new_rest[k + 1..].to_vec();
                        let mut new_leaf = STreeId::new();
                        new_leaf.leaves.push(value);
                        split_node
                            .children
                            .insert(diverge_new, (new_tail, new_leaf));
                    } else {
                        // New term ends exactly at the split node.
                        split_node.leaves.push(value);
                    }

                    self.children.get_mut(&first).unwrap().1 = split_node;
                }
            }
        }
    }

    // ── remove ────────────────────────────────────────────────────────────────

    /// Remove `value` indexed by the pre-order flattening of `atom`.
    pub fn remove_atom(&mut self, atom: &IdAtom, bank: &TermBank, value: &V) -> bool {
        let flat = flatten_atom_id(atom, bank);
        self.remove_flat(&flat, 0, value)
    }

    /// Remove `value` indexed by the pre-order flattening of `term`.
    pub fn remove(&mut self, term: TermId, bank: &TermBank, value: &V) -> bool {
        let flat = flatten_id(term, bank);
        self.remove_flat(&flat, 0, value)
    }

    fn remove_flat(&mut self, flat: &[Cell], pos: usize, value: &V) -> bool {
        if pos == flat.len() {
            let before = self.leaves.len();
            self.leaves.retain(|v| v != value);
            return self.leaves.len() < before;
        }

        let first = flat[pos];
        if let Some((edge_rest, child)) = self.children.get_mut(&first) {
            let k = edge_rest.len();
            // The edge must exactly match the next k cells.
            if flat.len() < pos + 1 + k {
                return false;
            }
            if flat[pos + 1..pos + 1 + k] != *edge_rest {
                return false;
            }
            let removed = child.remove_flat(flat, pos + 1 + k, value);
            if removed && child.is_empty() {
                self.children.remove(&first);
            }
            removed
        } else {
            false
        }
    }

    // ── predicates ────────────────────────────────────────────────────────────

    pub fn is_empty(&self) -> bool {
        self.leaves.is_empty() && self.children.is_empty()
    }

    // ── unification retrieval ─────────────────────────────────────────────────

    /// Returns all values whose stored term *may* unify with `query`.
    ///
    /// Imperfect filter: may include false positives but never misses a truly
    /// unifiable term.  Callers must verify unifiability on the returned set.
    pub fn get_unifications(&self, query: TermId, bank: &TermBank) -> Vec<V> {
        let flat = flatten_id(query, bank);
        let mut results = Vec::new();
        let mut bindings: Vec<Option<Range<usize>>> = Vec::new();
        self.unify_flat(&flat, 0, &mut results, &mut bindings);
        results
    }

    /// Same as [`get_unifications`] but takes a predicate atom as query.
    pub fn get_unifications_atom(&self, atom: &IdAtom, bank: &TermBank) -> Vec<V> {
        let flat = flatten_atom_id(atom, bank);
        let mut results = Vec::new();
        let mut bindings: Vec<Option<Range<usize>>> = Vec::new();
        self.unify_flat(&flat, 0, &mut results, &mut bindings);
        results
    }

    fn unify_flat(
        &self,
        query: &[Cell],
        pos: usize,
        results: &mut Vec<V>,
        bindings: &mut Vec<Option<Range<usize>>>,
    ) {
        if pos >= query.len() {
            results.extend_from_slice(&self.leaves);
            return;
        }

        for (first_cell, (edge_rest, child)) in &self.children {
            // Walk the edge label [*first_cell, edge_rest...] against query[pos..],
            // collecting results into `results` when the edge is fully consumed.
            unify_walk_edge(
                *first_cell,
                edge_rest,
                0,
                query,
                pos,
                child,
                results,
                bindings,
            );
        }
    }

    // ── generalization retrieval ──────────────────────────────────────────────

    /// Returns all values whose stored term is a *generalisation* of `query`
    /// (∃σ: σ(stored) = query).
    ///
    /// Used for forward demodulation: the rewrite-rule LHS must generalise
    /// the subterm being rewritten.
    pub fn get_generalizations(&self, query: TermId, bank: &TermBank) -> Vec<V> {
        let flat = flatten_id(query, bank);
        let mut results = Vec::new();
        let mut bindings: Vec<Option<&[Cell]>> = Vec::new();
        self.gen_flat(&flat, 0, &mut results, &mut bindings);
        results
    }

    fn gen_flat<'a>(
        &self,
        flat: &'a [Cell],
        pos: usize,
        out: &mut Vec<V>,
        bindings: &mut Vec<Option<&'a [Cell]>>,
    ) {
        if pos == flat.len() {
            out.extend_from_slice(&self.leaves);
            return;
        }

        for (first_cell, (edge_rest, child)) in &self.children {
            gen_walk_edge(*first_cell, edge_rest, 0, flat, pos, child, out, bindings);
        }
    }
}

impl<V> Default for STreeId<V> {
    fn default() -> Self {
        Self {
            children: BTreeMap::new(),
            leaves: Vec::new(),
        }
    }
}

impl<V: Clone + PartialEq> STreeId<V> {
    fn skip_stored(
        &self,
        remaining: usize,
        query: &[Cell],
        q: usize,
        results: &mut Vec<V>,
        bindings: &mut Vec<Option<Range<usize>>>,
    ) {
        if remaining == 0 {
            self.unify_flat(query, q, results, bindings);
            return;
        }

        for (first_cell, (edge_rest, child)) in &self.children {
            skip_walk_edge(
                *first_cell,
                edge_rest,
                0,
                remaining,
                query,
                q,
                child,
                results,
                bindings,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn skip_walk_edge<V: Clone + PartialEq>(
    first: Cell,
    rest: &[Cell],
    e: usize,
    mut remaining: usize,
    query: &[Cell],
    q: usize,
    child: &STreeId<V>,
    results: &mut Vec<V>,
    bindings: &mut Vec<Option<Range<usize>>>,
) {
    let edge_len = 1 + rest.len();
    let mut e = e;

    while e < edge_len && remaining > 0 {
        let sc = if e == 0 { first } else { rest[e - 1] };
        e += 1;
        match sc {
            Cell::Var(_) => {
                remaining -= 1;
            }
            Cell::Sym(_, m) => {
                remaining = remaining - 1 + m as usize;
            }
        }
    }

    if remaining == 0 {
        unify_walk_edge(first, rest, e, query, q, child, results, bindings);
    } else {
        child.skip_stored(remaining, query, q, results, bindings);
    }
}
// ── edge walking — unification ────────────────────────────────────────────────
//
// `unify_walk_edge` mirrors `DTreeId::unify_flat` / `DTreeId::skip_stored` but
// operates on a *compressed edge* `[first] ++ rest` instead of a single-level
// trie.
//
// Parameters
// ----------
// first   : the key cell of this edge (logical position e=0 in the edge)
// rest    : cells 1..m of the edge
// e       : current position within the logical edge sequence [first]++rest
// query   : the flat query term
// q       : current position in `query`
// child   : the sub-tree to recurse into when the edge is fully consumed (e == edge_len)
// results : accumulated values
// bindings: stored-variable bindings (indexed by variable ID, range into `query`)

#[allow(clippy::too_many_arguments)]
fn unify_walk_edge<V: Clone + PartialEq>(
    first: Cell,
    rest: &[Cell],
    e: usize,
    query: &[Cell],
    q: usize,
    child: &STreeId<V>,
    results: &mut Vec<V>,
    bindings: &mut Vec<Option<Range<usize>>>,
) {
    let edge_len = 1 + rest.len();

    if e == edge_len {
        // Edge fully consumed — recurse into the child node.
        child.unify_flat(query, q, results, bindings);
        return;
    }

    if q >= query.len() {
        // Query exhausted but edge has more cells — no match possible.
        return;
    }

    let sc = if e == 0 { first } else { rest[e - 1] }; // stored cell
    let qc = query[q]; // query cell

    match (sc, qc) {
        (Cell::Sym(sf, sn), Cell::Sym(qf, qn)) => {
            // Exact symbol match: both advance by one.
            if sf == qf && sn == qn {
                unify_walk_edge(first, rest, e + 1, query, q + 1, child, results, bindings);
            }
        }

        (Cell::Sym(_, _sn), Cell::Var(_)) => {
            // Query variable matches the entire stored subterm in the edge.
            // Advance query by 1; skip the whole stored subterm in the edge.
            skip_walk_edge(
                first,
                rest,
                e + 1,
                _sn as usize,
                query,
                q + 1,
                child,
                results,
                bindings,
            );
        }

        (Cell::Var(v), Cell::Sym(_, _)) => {
            // Stored variable binds to the query subterm at q.
            let skip_q = skip_in_flat(query, q);
            let v_idx = v as usize;
            if v_idx >= bindings.len() {
                bindings.resize(v_idx + 1, None);
            }
            let old = bindings[v_idx].clone();
            match &bindings[v_idx] {
                Some(bound) => {
                    let r1 = &query[bound.clone()];
                    let r2 = &query[q..skip_q];
                    let r1_has_var = r1.iter().any(|c| matches!(c, Cell::Var(_)));
                    let r2_has_var = r2.iter().any(|c| matches!(c, Cell::Var(_)));
                    if r1_has_var || r2_has_var || r1 == r2 {
                        unify_walk_edge(
                            first,
                            rest,
                            e + 1,
                            query,
                            skip_q,
                            child,
                            results,
                            bindings,
                        );
                    }
                }
                None => {
                    bindings[v_idx] = Some(q..skip_q);
                    unify_walk_edge(first, rest, e + 1, query, skip_q, child, results, bindings);
                    bindings[v_idx] = old;
                }
            }
        }

        (Cell::Var(v), Cell::Var(_)) => {
            // Both are variables.
            let v_idx = v as usize;
            if v_idx >= bindings.len() {
                bindings.resize(v_idx + 1, None);
            }
            let old = bindings[v_idx].clone();
            match &bindings[v_idx] {
                Some(bound) => {
                    let r1 = &query[bound.clone()];
                    let r2 = &query[q..q + 1];
                    let r1_has_var = r1.iter().any(|c| matches!(c, Cell::Var(_)));
                    let r2_has_var = r2.iter().any(|c| matches!(c, Cell::Var(_)));
                    if r1_has_var || r2_has_var || r1 == r2 {
                        unify_walk_edge(first, rest, e + 1, query, q + 1, child, results, bindings);
                    }
                }
                None => {
                    bindings[v_idx] = Some(q..q + 1);
                    unify_walk_edge(first, rest, e + 1, query, q + 1, child, results, bindings);
                    bindings[v_idx] = old;
                }
            }
        }
    }
}

// ── edge walking — skip_stored for query-variable case ────────────────────────
//
// When a query variable must match `remaining` stored subterms (because a
// stored `Sym(f, n)` was matched by a query `Var`, consuming `n` more stored
// sub-children), we need to skip those `n` stored subterms in the edge.
//
// This mirrors `DTreeId::skip_stored` but operates on a compressed edge.
// `unify_walk_edge` already handles skipping stored subterms properly.

// ── edge walking — generalization ─────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn gen_walk_edge<'a, V: Clone + PartialEq>(
    first: Cell,
    rest: &[Cell],
    e: usize,
    query: &'a [Cell],
    q: usize,
    child: &STreeId<V>,
    out: &mut Vec<V>,
    bindings: &mut Vec<Option<&'a [Cell]>>,
) {
    let edge_len = 1 + rest.len();

    if e == edge_len {
        child.gen_flat(query, q, out, bindings);
        return;
    }

    if q >= query.len() {
        return;
    }

    let sc = if e == 0 { first } else { rest[e - 1] };
    let qc = query[q];

    match (sc, qc) {
        (Cell::Sym(sf, sn), Cell::Sym(qf, qn)) => {
            if sf == qf && sn == qn {
                gen_walk_edge(first, rest, e + 1, query, q + 1, child, out, bindings);
            }
            // Stored Sym vs Query Sym mismatch: no match.
        }

        (Cell::Sym(_, _), Cell::Var(_)) => {
            // Query variable with stored symbol → stored is NOT more general.
            // Generalization requires stored to be ≥ general, but a concrete
            // stored symbol cannot generalise a query variable.
        }

        (Cell::Var(v), _) => {
            // Stored variable matches any query subterm.
            let skip_q = skip_in_flat(query, q);
            let subterm = &query[q..skip_q];
            let v_idx = v as usize;
            if v_idx >= bindings.len() {
                bindings.resize(v_idx + 1, None);
            }
            let old = bindings[v_idx];
            match bindings[v_idx] {
                Some(bound) => {
                    if bound == subterm {
                        gen_walk_edge(first, rest, e + 1, query, skip_q, child, out, bindings);
                    }
                }
                None => {
                    bindings[v_idx] = Some(subterm);
                    gen_walk_edge(first, rest, e + 1, query, skip_q, child, out, bindings);
                    bindings[v_idx] = old;
                }
            }
        }
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::symbol::{SymbolId, SymbolTable};
    use mrs_core::term::{Term, VarId};
    use mrs_core::term_bank::TermBank;

    fn intern_term(bank: &mut TermBank, t: &Term) -> TermId {
        bank.from_legacy(t)
    }

    fn make_symbols() -> (SymbolTable, SymbolId, SymbolId, SymbolId, SymbolId) {
        let mut st = SymbolTable::new();
        let f = st.intern("f");
        let g = st.intern("g");
        let a = st.intern("a");
        let b = st.intern("b");
        (st, f, g, a, b)
    }

    fn app(sym: SymbolId, args: Vec<Term>) -> Term {
        Term::app(sym, args)
    }
    fn var(v: VarId) -> Term {
        Term::var(v)
    }
    fn cst(sym: SymbolId) -> Term {
        Term::constant(sym)
    }

    // ── ported DTreeId tests ──────────────────────────────────────────────────

    #[test]
    fn insert_and_retrieve_exact() {
        let (_, f, _, a, b) = make_symbols();
        let mut bank = TermBank::new();
        let term = intern_term(&mut bank, &app(f, vec![cst(a), cst(b)]));

        let mut tree: STreeId<i32> = STreeId::new();
        tree.insert(term, &bank, 42);

        let results = tree.get_unifications(term, &bank);
        assert!(results.contains(&42));
    }

    #[test]
    fn unify_stored_variable() {
        let (_, f, _, a, _) = make_symbols();
        let mut bank = TermBank::new();
        let stored = intern_term(&mut bank, &app(f, vec![var(0)]));
        let query = intern_term(&mut bank, &app(f, vec![cst(a)]));

        let mut tree: STreeId<i32> = STreeId::new();
        tree.insert(stored, &bank, 1);

        let results = tree.get_unifications(query, &bank);
        assert!(
            results.contains(&1),
            "stored f(X) should unify with query f(a)"
        );
    }

    #[test]
    fn unify_query_variable() {
        let (_, f, _, a, _) = make_symbols();
        let mut bank = TermBank::new();
        let stored = intern_term(&mut bank, &app(f, vec![cst(a)]));
        let query = intern_term(&mut bank, &app(f, vec![var(0)]));

        let mut tree: STreeId<i32> = STreeId::new();
        tree.insert(stored, &bank, 2);

        let results = tree.get_unifications(query, &bank);
        assert!(
            results.contains(&2),
            "stored f(a) should unify with query f(X)"
        );
    }

    #[test]
    fn unify_both_variables() {
        let (_, f, _, _, _) = make_symbols();
        let mut bank = TermBank::new();
        let stored = intern_term(&mut bank, &app(f, vec![var(0)]));
        let query = intern_term(&mut bank, &app(f, vec![var(1)]));

        let mut tree: STreeId<i32> = STreeId::new();
        tree.insert(stored, &bank, 3);

        let results = tree.get_unifications(query, &bank);
        assert!(results.contains(&3));
    }

    #[test]
    fn unify_no_match() {
        let (_, f, g, a, _) = make_symbols();
        let mut bank = TermBank::new();
        let stored = intern_term(&mut bank, &app(f, vec![cst(a)]));
        let query = intern_term(&mut bank, &app(g, vec![cst(a)]));

        let mut tree: STreeId<i32> = STreeId::new();
        tree.insert(stored, &bank, 99);

        let results = tree.get_unifications(query, &bank);
        assert!(results.is_empty(), "different top symbols should not unify");
    }

    #[test]
    fn unify_nested_terms() {
        let (_, f, g, a, _) = make_symbols();
        let mut bank = TermBank::new();
        let t1 = intern_term(&mut bank, &app(f, vec![app(g, vec![cst(a)]), cst(a)]));
        let t2 = intern_term(&mut bank, &app(f, vec![var(0), cst(a)]));
        let query = intern_term(&mut bank, &app(f, vec![app(g, vec![cst(a)]), var(0)]));

        let mut tree: STreeId<i32> = STreeId::new();
        tree.insert(t1, &bank, 10);
        tree.insert(t2, &bank, 20);

        let results = tree.get_unifications(query, &bank);
        assert!(
            results.contains(&10),
            "f(g(a),a) should unify with f(g(a),X)"
        );
        assert!(results.contains(&20), "f(X,a) should unify with f(g(a),X)");
    }

    #[test]
    fn unify_query_var_skips_stored_subtree() {
        let (_, f, g, a, b) = make_symbols();
        let mut bank = TermBank::new();
        let stored = intern_term(&mut bank, &app(f, vec![app(g, vec![cst(a), cst(b)])]));
        let query = intern_term(&mut bank, &app(f, vec![var(0)]));

        let mut tree: STreeId<i32> = STreeId::new();
        tree.insert(stored, &bank, 7);

        let results = tree.get_unifications(query, &bank);
        assert!(
            results.contains(&7),
            "f(X) should unify with f(g(a,b)) via query-var skip"
        );
    }

    #[test]
    fn generalizations_find_pattern() {
        let (_, f, _, a, _) = make_symbols();
        let mut bank = TermBank::new();
        let pattern = intern_term(&mut bank, &app(f, vec![var(0)]));
        let concrete = intern_term(&mut bank, &app(f, vec![cst(a)]));
        let query = intern_term(&mut bank, &app(f, vec![cst(a)]));

        let mut tree: STreeId<i32> = STreeId::new();
        tree.insert(pattern, &bank, 1);
        tree.insert(concrete, &bank, 2);

        let results = tree.get_generalizations(query, &bank);
        assert!(results.contains(&1), "f(X) generalises f(a)");
        assert!(results.contains(&2), "f(a) generalises f(a)");
    }

    #[test]
    fn generalizations_no_over_specific() {
        let (_, f, _, a, b) = make_symbols();
        let mut bank = TermBank::new();
        let stored = intern_term(&mut bank, &app(f, vec![cst(a)]));
        let query = intern_term(&mut bank, &app(f, vec![cst(b)]));

        let mut tree: STreeId<i32> = STreeId::new();
        tree.insert(stored, &bank, 5);

        let results = tree.get_generalizations(query, &bank);
        assert!(!results.contains(&5), "f(a) should NOT generalise f(b)");
    }

    #[test]
    fn remove_entry() {
        let (_, f, _, a, b) = make_symbols();
        let mut bank = TermBank::new();
        let t1 = intern_term(&mut bank, &app(f, vec![cst(a)]));
        let t2 = intern_term(&mut bank, &app(f, vec![cst(b)]));

        let mut tree: STreeId<i32> = STreeId::new();
        tree.insert(t1, &bank, 10);
        tree.insert(t2, &bank, 20);
        tree.remove(t1, &bank, &10);

        assert!(!tree.get_unifications(t1, &bank).contains(&10));
        assert!(tree.get_unifications(t2, &bank).contains(&20));
    }

    #[test]
    fn constant_terms() {
        let (_, _, _, a, b) = make_symbols();
        let mut bank = TermBank::new();
        let ta = intern_term(&mut bank, &cst(a));
        let tb = intern_term(&mut bank, &cst(b));
        let qv = intern_term(&mut bank, &var(0));

        let mut tree: STreeId<i32> = STreeId::new();
        tree.insert(ta, &bank, 1);
        tree.insert(tb, &bank, 2);

        let results = tree.get_unifications(qv, &bank);
        assert!(results.contains(&1));
        assert!(results.contains(&2));
    }

    // ── path-compression specific tests ──────────────────────────────────────

    #[test]
    fn path_compression_common_prefix() {
        // f(g(a)) and f(g(b)) share the 2-cell prefix [f/2, g/1].
        let (_, f, g, a, b) = make_symbols();
        let mut bank = TermBank::new();
        let t1 = intern_term(&mut bank, &app(f, vec![app(g, vec![cst(a)])]));
        let t2 = intern_term(&mut bank, &app(f, vec![app(g, vec![cst(b)])]));

        let mut tree: STreeId<i32> = STreeId::new();
        tree.insert(t1, &bank, 1);
        tree.insert(t2, &bank, 2);

        assert!(tree.get_unifications(t1, &bank).contains(&1));
        assert!(tree.get_unifications(t2, &bank).contains(&2));
        assert!(tree.get_generalizations(t1, &bank).contains(&1));
        assert!(tree.get_generalizations(t2, &bank).contains(&2));
    }

    #[test]
    fn path_compression_split_mid_edge() {
        // f(g(a)), f(g(b)), f(h(a)): first two share [f/2,g/1]; insert of f(h(a))
        // must split the edge.
        let mut st = SymbolTable::new();
        let f = st.intern("f");
        let g = st.intern("g");
        let a = st.intern("a");
        let b = st.intern("b");
        let h = st.intern("h");
        let mut bank = TermBank::new();
        let t1 = intern_term(&mut bank, &app(f, vec![app(g, vec![cst(a)])]));
        let t2 = intern_term(&mut bank, &app(f, vec![app(g, vec![cst(b)])]));
        let t3 = intern_term(&mut bank, &app(f, vec![app(h, vec![cst(a)])]));

        let mut tree: STreeId<i32> = STreeId::new();
        tree.insert(t1, &bank, 1);
        tree.insert(t2, &bank, 2);
        tree.insert(t3, &bank, 3);

        assert!(tree.get_unifications(t1, &bank).contains(&1));
        assert!(tree.get_unifications(t2, &bank).contains(&2));
        assert!(tree.get_unifications(t3, &bank).contains(&3));
        assert!(!tree.get_unifications(t3, &bank).contains(&1));
    }

    #[test]
    fn path_compression_variable_in_edge() {
        // f(a,X) and f(b,X) share only the f/2 prefix; they diverge at the 2nd cell.
        let (_, f, _, a, b) = make_symbols();
        let mut bank = TermBank::new();
        let t1 = intern_term(&mut bank, &app(f, vec![cst(a), var(0)]));
        let t2 = intern_term(&mut bank, &app(f, vec![cst(b), var(0)]));

        let mut tree: STreeId<i32> = STreeId::new();
        tree.insert(t1, &bank, 1);
        tree.insert(t2, &bank, 2);

        let qa = intern_term(&mut bank, &app(f, vec![cst(a), cst(a)]));
        let qb = intern_term(&mut bank, &app(f, vec![cst(b), cst(a)]));
        assert!(tree.get_unifications(qa, &bank).contains(&1));
        assert!(!tree.get_unifications(qa, &bank).contains(&2));
        assert!(!tree.get_unifications(qb, &bank).contains(&1));
        assert!(tree.get_unifications(qb, &bank).contains(&2));
    }

    #[test]
    fn multiple_inserts_same_term() {
        let (_, f, _, a, _) = make_symbols();
        let mut bank = TermBank::new();
        let t = intern_term(&mut bank, &app(f, vec![cst(a)]));

        let mut tree: STreeId<i32> = STreeId::new();
        tree.insert(t, &bank, 100);
        tree.insert(t, &bank, 200);

        let results = tree.get_unifications(t, &bank);
        assert!(results.contains(&100));
        assert!(results.contains(&200));
    }

    #[test]
    fn remove_from_compressed_edge() {
        let (_, f, g, a, b) = make_symbols();
        let mut bank = TermBank::new();
        let t1 = intern_term(&mut bank, &app(f, vec![app(g, vec![cst(a)])]));
        let t2 = intern_term(&mut bank, &app(f, vec![app(g, vec![cst(b)])]));

        let mut tree: STreeId<i32> = STreeId::new();
        tree.insert(t1, &bank, 1);
        tree.insert(t2, &bank, 2);
        tree.remove(t1, &bank, &1);

        assert!(!tree.get_unifications(t1, &bank).contains(&1));
        assert!(tree.get_unifications(t2, &bank).contains(&2));
    }

    #[test]
    fn deep_common_prefix() {
        // f(g(h(a))) and f(g(h(b))): 4-cell common prefix before split.
        let mut st = SymbolTable::new();
        let f = st.intern("f");
        let g = st.intern("g");
        let a = st.intern("a");
        let b = st.intern("b");
        let h = st.intern("h");

        let mut bank = TermBank::new();
        let t1 = intern_term(&mut bank, &app(f, vec![app(g, vec![app(h, vec![cst(a)])])]));
        let t2 = intern_term(&mut bank, &app(f, vec![app(g, vec![app(h, vec![cst(b)])])]));

        let mut tree: STreeId<i32> = STreeId::new();
        tree.insert(t1, &bank, 1);
        tree.insert(t2, &bank, 2);

        assert!(tree.get_unifications(t1, &bank).contains(&1));
        assert!(tree.get_unifications(t2, &bank).contains(&2));
        assert!(!tree.get_unifications(t1, &bank).contains(&2));
        assert!(!tree.get_unifications(t2, &bank).contains(&1));

        assert!(tree.get_generalizations(t1, &bank).contains(&1));
        assert!(tree.get_generalizations(t2, &bank).contains(&2));
    }

    #[test]
    fn gen_repeated_variable() {
        // Stored: f(X, X) — only generalises f(a,a), not f(a,b).
        let (_, f, _, a, b) = make_symbols();
        let mut bank = TermBank::new();
        let stored = intern_term(&mut bank, &app(f, vec![var(0), var(0)]));
        let qa = intern_term(&mut bank, &app(f, vec![cst(a), cst(a)]));
        let qab = intern_term(&mut bank, &app(f, vec![cst(a), cst(b)]));

        let mut tree: STreeId<i32> = STreeId::new();
        tree.insert(stored, &bank, 99);

        assert!(
            tree.get_generalizations(qa, &bank).contains(&99),
            "f(X,X) must generalise f(a,a)"
        );
        assert!(
            !tree.get_generalizations(qab, &bank).contains(&99),
            "f(X,X) must NOT generalise f(a,b)"
        );
    }
}
