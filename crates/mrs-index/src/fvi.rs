use mrs_core::clause::{Clause, ClauseId};
use mrs_core::formula::Atom;
use mrs_core::symbol::SymbolId;
use mrs_core::term::Term;
use mrs_core::term_bank::{IdAtom, IdClause, TermBank, TermId, TermNode};

/// A feature vector representing the symbol frequencies of a clause.
/// Used for fast subsumption filtering.
///
/// If clause A subsumes clause B (i.e. A sigma subset B), then:
/// - A has <= literals than B
/// - A has <= positive literals than B
/// - A has <= negative literals than B
/// - For every function/predicate symbol S, A has <= occurrences of S than B.
///
/// To optimize for speed and hardware-compatible SIMD vectorization on stable Rust,
/// we represent symbol occurrences using a fixed dense array of 64 buckets [u16; 64].
/// SymbolIds are mapped to a bucket via (sym.index() & 63). Occurrences in the same
/// bucket are summed. This bucketed sum is a mathematically sound necessary condition:
/// If for every symbol S, count_A(S) <= count_B(S), then for every bucket i,
/// sum_{S in bucket i} count_A(S) <= sum_{S in bucket i} count_B(S).
/// Thus, comparing the bucketed sums has zero false negatives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeatureVector {
    pub num_lits: u32,
    pub pos_lits: u32,
    pub neg_lits: u32,
    pub sym_counts: [u16; 64],
}

impl Default for FeatureVector {
    fn default() -> Self {
        FeatureVector {
            num_lits: 0,
            pos_lits: 0,
            neg_lits: 0,
            sym_counts: [0; 64],
        }
    }
}

impl FeatureVector {
    /// Computes the feature vector for a clause.
    pub fn from_clause(clause: &Clause) -> Self {
        let mut fv = FeatureVector {
            num_lits: clause.len() as u32,
            ..Default::default()
        };

        for lit in &clause.literals {
            if lit.positive {
                fv.pos_lits += 1;
            } else {
                fv.neg_lits += 1;
            }

            match &lit.atom {
                Atom::Pred(sym, args) => {
                    fv.increment(*sym);
                    for arg in args {
                        fv.count_term(arg);
                    }
                }
                Atom::Eq(l, r) => {
                    fv.count_term(l);
                    fv.count_term(r);
                }
            }
        }
        fv
    }

    fn increment(&mut self, sym: SymbolId) {
        let bucket = (sym.index() as usize) & 63;
        self.sym_counts[bucket] = self.sym_counts[bucket].saturating_add(1);
    }

    pub fn get_count(&self, sym: SymbolId) -> u16 {
        self.sym_counts[(sym.index() as usize) & 63]
    }

    fn count_term(&mut self, term: &Term) {
        match term {
            Term::Var(_) => {} // Variables can map to anything, don't count them
            Term::App(sym, args) => {
                self.increment(*sym);
                for arg in args {
                    self.count_term(arg);
                }
            }
        }
    }

    /// Computes the feature vector for an `IdClause`.
    pub fn from_id_clause(clause: &IdClause, bank: &TermBank) -> Self {
        let mut fv = FeatureVector {
            num_lits: clause.literals.len() as u32,
            ..Default::default()
        };
        for lit in &clause.literals {
            if lit.positive {
                fv.pos_lits += 1;
            } else {
                fv.neg_lits += 1;
            }
            match &lit.atom {
                IdAtom::Pred(sym, args) => {
                    fv.increment(*sym);
                    for &arg in args {
                        fv.count_term_id(arg, bank);
                    }
                }
                IdAtom::Eq(l, r) => {
                    fv.count_term_id(*l, bank);
                    fv.count_term_id(*r, bank);
                }
            }
        }
        fv
    }

    fn count_term_id(&mut self, term: TermId, bank: &TermBank) {
        match bank.get(term) {
            TermNode::Var(_) => {}
            TermNode::App(sym, args) => {
                self.increment(*sym);
                for &arg in args {
                    self.count_term_id(arg, bank);
                }
            }
        }
    }

    #[inline(always)]
    fn sym_counts_le(&self, other: &FeatureVector) -> bool {
        let mut any_greater = 0;
        for i in 0..64 {
            any_greater |= if self.sym_counts[i] > other.sym_counts[i] {
                1
            } else {
                0
            };
        }
        any_greater == 0
    }

    /// Returns true if this feature vector could potentially subsume `other`.
    /// This is a fast necessary (but not sufficient) condition for subsumption.
    ///
    /// Comparing the 64-element u16 arrays is fully branchless and compiles
    /// into 256-bit AVX2 vector instructions by LLVM auto-vectorizer on stable Rust.
    pub fn can_subsume(&self, other: &FeatureVector) -> bool {
        if self.num_lits > other.num_lits {
            return false;
        }
        if self.pos_lits > other.pos_lits {
            return false;
        }
        if self.neg_lits > other.neg_lits {
            return false;
        }

        self.sym_counts_le(other)
    }

    /// Returns true if this feature vector could potentially subsumption-resolve a target
    /// that has `other` as its feature vector.
    /// This means `self` could subsume `other` if exactly one literal's polarity was flipped.
    pub fn can_subsumption_resolve(&self, other: &FeatureVector) -> bool {
        if self.num_lits > other.num_lits {
            return false;
        }

        if self.pos_lits > other.pos_lits + 1 {
            return false;
        }
        if self.neg_lits > other.neg_lits + 1 {
            return false;
        }

        self.sym_counts_le(other)
    }

    /// Returns the feature value at dimension `dim`.
    /// Dimensions 0, 1, 2 correspond to `num_lits`, `pos_lits`, and `neg_lits`.
    /// Dimensions 3..67 correspond to symbol bucket counts 0..64.
    #[inline(always)]
    pub fn get_feature(&self, dim: usize) -> u16 {
        match dim {
            0 => self.num_lits.min(u16::MAX as u32) as u16,
            1 => self.pos_lits.min(u16::MAX as u32) as u16,
            2 => self.neg_lits.min(u16::MAX as u32) as u16,
            d if d < 67 => self.sym_counts[d - 3],
            _ => 0,
        }
    }
}

const LEAF_CAPACITY: usize = 8;
const MAX_DEPTH: usize = 67;

#[derive(Clone, Debug)]
enum FvtNode<T> {
    Leaf { items: Vec<(T, FeatureVector)> },
    Internal { children: Vec<(u16, FvtNode<T>)> },
}

impl<T> Default for FvtNode<T> {
    fn default() -> Self {
        FvtNode::Leaf { items: Vec::new() }
    }
}

impl<T> FvtNode<T> {
    fn is_empty(&self) -> bool {
        match self {
            FvtNode::Leaf { items } => items.is_empty(),
            FvtNode::Internal { children } => children.is_empty(),
        }
    }
}

/// A hierarchical Feature Vector Tree (FVT) for indexing clauses by their feature vectors.
///
/// Implements Stephan Schulz's Feature Vector Indexing algorithm (Schulz 2002).
/// Internal nodes branch on individual feature dimensions (num_lits, pos_lits, neg_lits,
/// and symbol bucket frequencies). Because children are kept sorted by their feature value:
/// - Forward subsumption prunes branches where child feature value > target feature value.
/// - Backward subsumption prunes branches where child feature value < subsumer feature value.
/// - Leaves store at most `LEAF_CAPACITY` items, validated against full SIMD `can_subsume`.
#[derive(Clone, Debug)]
pub struct FeatureVectorTree<T = ClauseId> {
    root: FvtNode<T>,
    len: usize,
}

impl<T: Copy + PartialEq> Default for FeatureVectorTree<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Copy + PartialEq> FeatureVectorTree<T> {
    /// Creates an empty feature vector tree.
    pub fn new() -> Self {
        Self {
            root: FvtNode::default(),
            len: 0,
        }
    }

    /// Returns the number of items stored in the tree.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the tree contains no items.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Clears all entries from the tree.
    pub fn clear(&mut self) {
        self.root = FvtNode::default();
        self.len = 0;
    }

    /// Inserts an item with its feature vector into the tree.
    pub fn insert(&mut self, val: T, fv: FeatureVector) {
        Self::insert_rec(&mut self.root, 0, val, fv);
        self.len += 1;
    }

    fn insert_rec(node: &mut FvtNode<T>, depth: usize, val: T, fv: FeatureVector) {
        match node {
            FvtNode::Leaf { items } => {
                if items.len() < LEAF_CAPACITY || depth >= MAX_DEPTH {
                    items.push((val, fv));
                } else {
                    if items.iter().all(|(_, item_fv)| item_fv == &fv) {
                        items.push((val, fv));
                        return;
                    }

                    let mut old_items = std::mem::take(items);
                    old_items.push((val, fv));

                    let mut children: Vec<(u16, FvtNode<T>)> = Vec::new();
                    for (item_val, item_fv) in old_items {
                        let feature_val = item_fv.get_feature(depth);
                        match children.binary_search_by_key(&feature_val, |&(k, _)| k) {
                            Ok(idx) => {
                                Self::insert_rec(
                                    &mut children[idx].1,
                                    depth + 1,
                                    item_val,
                                    item_fv,
                                );
                            }
                            Err(idx) => {
                                let mut new_child = FvtNode::Leaf { items: Vec::new() };
                                Self::insert_rec(&mut new_child, depth + 1, item_val, item_fv);
                                children.insert(idx, (feature_val, new_child));
                            }
                        }
                    }
                    *node = FvtNode::Internal { children };
                }
            }
            FvtNode::Internal { children } => {
                let feature_val = fv.get_feature(depth);
                match children.binary_search_by_key(&feature_val, |&(k, _)| k) {
                    Ok(idx) => {
                        Self::insert_rec(&mut children[idx].1, depth + 1, val, fv);
                    }
                    Err(idx) => {
                        let mut new_child = FvtNode::Leaf { items: Vec::new() };
                        Self::insert_rec(&mut new_child, depth + 1, val, fv);
                        children.insert(idx, (feature_val, new_child));
                    }
                }
            }
        }
    }

    /// Removes an item from the tree by value and feature vector.
    /// Returns true if the item was found and removed.
    pub fn remove(&mut self, val: T, fv: &FeatureVector) -> bool {
        let removed = Self::remove_rec(&mut self.root, 0, val, fv);
        if removed {
            self.len -= 1;
        }
        removed
    }

    fn remove_rec(node: &mut FvtNode<T>, depth: usize, val: T, fv: &FeatureVector) -> bool {
        match node {
            FvtNode::Leaf { items } => {
                if let Some(pos) = items.iter().position(|&(v, _)| v == val) {
                    items.swap_remove(pos);
                    true
                } else {
                    false
                }
            }
            FvtNode::Internal { children } => {
                let feature_val = fv.get_feature(depth);
                if let Ok(idx) = children.binary_search_by_key(&feature_val, |&(k, _)| k) {
                    let removed = Self::remove_rec(&mut children[idx].1, depth + 1, val, fv);
                    if children[idx].1.is_empty() {
                        children.remove(idx);
                    }
                    removed
                } else {
                    false
                }
            }
        }
    }

    /// Finds all values in the tree whose feature vectors could subsume `target_fv`.
    /// Prunes branches where child feature value > target feature value.
    pub fn query_subsumers(&self, target_fv: &FeatureVector, out: &mut Vec<T>) {
        Self::query_subsumers_rec(&self.root, 0, target_fv, out);
    }

    fn query_subsumers_rec(
        node: &FvtNode<T>,
        depth: usize,
        target_fv: &FeatureVector,
        out: &mut Vec<T>,
    ) {
        match node {
            FvtNode::Leaf { items } => {
                for (val, fv) in items {
                    if fv.can_subsume(target_fv) {
                        out.push(*val);
                    }
                }
            }
            FvtNode::Internal { children } => {
                let max_val = target_fv.get_feature(depth);
                for (feature_val, child) in children {
                    if *feature_val > max_val {
                        break;
                    }
                    Self::query_subsumers_rec(child, depth + 1, target_fv, out);
                }
            }
        }
    }

    /// Finds all values in the tree that could BE subsumed by `subsumer_fv`.
    /// Prunes branches where child feature value < subsumer feature value.
    pub fn query_subsumed(&self, subsumer_fv: &FeatureVector, out: &mut Vec<T>) {
        Self::query_subsumed_rec(&self.root, 0, subsumer_fv, out);
    }

    fn query_subsumed_rec(
        node: &FvtNode<T>,
        depth: usize,
        subsumer_fv: &FeatureVector,
        out: &mut Vec<T>,
    ) {
        match node {
            FvtNode::Leaf { items } => {
                for (val, fv) in items {
                    if subsumer_fv.can_subsume(fv) {
                        out.push(*val);
                    }
                }
            }
            FvtNode::Internal { children } => {
                let min_val = subsumer_fv.get_feature(depth);
                let start = children.partition_point(|&(k, _)| k < min_val);
                for (_, child) in &children[start..] {
                    Self::query_subsumed_rec(child, depth + 1, subsumer_fv, out);
                }
            }
        }
    }

    /// Finds all values in the tree that could subsumption-resolve `target_fv`.
    pub fn query_subsumption_resolution(&self, target_fv: &FeatureVector, out: &mut Vec<T>) {
        Self::query_subsumption_resolution_rec(&self.root, 0, target_fv, out);
    }

    fn query_subsumption_resolution_rec(
        node: &FvtNode<T>,
        depth: usize,
        target_fv: &FeatureVector,
        out: &mut Vec<T>,
    ) {
        match node {
            FvtNode::Leaf { items } => {
                for (val, fv) in items {
                    if fv.can_subsumption_resolve(target_fv) {
                        out.push(*val);
                    }
                }
            }
            FvtNode::Internal { children } => {
                let max_val = match depth {
                    0 => target_fv.num_lits.min(u16::MAX as u32) as u16,
                    1 => (target_fv.pos_lits.saturating_add(1).min(u16::MAX as u32)) as u16,
                    2 => (target_fv.neg_lits.saturating_add(1).min(u16::MAX as u32)) as u16,
                    d if d < 67 => target_fv.sym_counts[d - 3],
                    _ => u16::MAX,
                };
                for (feature_val, child) in children {
                    if *feature_val > max_val {
                        break;
                    }
                    Self::query_subsumption_resolution_rec(child, depth + 1, target_fv, out);
                }
            }
        }
    }

    /// Finds all values in the tree that could BE subsumption-resolved by `simplifier_fv`.
    pub fn query_backward_subsumption_resolution(
        &self,
        simplifier_fv: &FeatureVector,
        out: &mut Vec<T>,
    ) {
        Self::query_backward_subsumption_resolution_rec(&self.root, 0, simplifier_fv, out);
    }

    fn query_backward_subsumption_resolution_rec(
        node: &FvtNode<T>,
        depth: usize,
        simplifier_fv: &FeatureVector,
        out: &mut Vec<T>,
    ) {
        match node {
            FvtNode::Leaf { items } => {
                for (val, fv) in items {
                    if simplifier_fv.can_subsumption_resolve(fv) {
                        out.push(*val);
                    }
                }
            }
            FvtNode::Internal { children } => {
                let min_val = match depth {
                    0 => simplifier_fv.num_lits.min(u16::MAX as u32) as u16,
                    1 => simplifier_fv.pos_lits.saturating_sub(1) as u16,
                    2 => simplifier_fv.neg_lits.saturating_sub(1) as u16,
                    d if d < 67 => simplifier_fv.sym_counts[d - 3],
                    _ => 0,
                };
                let start = children.partition_point(|&(k, _)| k < min_val);
                for (_, child) in &children[start..] {
                    Self::query_backward_subsumption_resolution_rec(
                        child,
                        depth + 1,
                        simplifier_fv,
                        out,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::symbol::SymbolTable;

    #[test]
    fn test_feature_vector_basic() {
        let mut fv1 = FeatureVector::default();
        let mut fv2 = FeatureVector::default();

        let mut st = SymbolTable::new();
        let s1 = st.intern("s1");
        let s2 = st.intern("s2");

        fv1.increment(s1);
        fv2.increment(s1);
        fv2.increment(s2);

        fv1.num_lits = 1;
        fv1.pos_lits = 1;

        fv2.num_lits = 2;
        fv2.pos_lits = 2;

        assert!(fv1.can_subsume(&fv2));
        assert!(!fv2.can_subsume(&fv1));
    }

    #[test]
    fn test_subsumption_resolution() {
        let mut fv1 = FeatureVector::default();
        let mut fv2 = FeatureVector::default();

        let mut st = SymbolTable::new();
        let s1 = st.intern("s1");
        let s2 = st.intern("s2");

        fv1.increment(s1);
        fv2.increment(s1);
        fv2.increment(s2);

        fv1.num_lits = 2;
        fv1.pos_lits = 1;
        fv1.neg_lits = 1;

        fv2.num_lits = 2;
        fv2.pos_lits = 2;

        assert!(fv1.can_subsumption_resolve(&fv2));
    }

    #[test]
    fn test_fvt_empty_and_single() {
        let mut tree = FeatureVectorTree::new();
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);

        let mut fv1 = FeatureVector::default();
        fv1.num_lits = 2;
        fv1.pos_lits = 1;
        fv1.neg_lits = 1;
        let id1 = ClauseId(1);

        tree.insert(id1, fv1);
        assert!(!tree.is_empty());
        assert_eq!(tree.len(), 1);

        let mut target = FeatureVector::default();
        target.num_lits = 3;
        target.pos_lits = 2;
        target.neg_lits = 1;

        let mut out = Vec::new();
        tree.query_subsumers(&target, &mut out);
        assert_eq!(out, vec![id1]);

        let mut out_subsumed = Vec::new();
        tree.query_subsumed(&target, &mut out_subsumed);
        assert!(out_subsumed.is_empty());

        let removed = tree.remove(id1, &fv1);
        assert!(removed);
        assert_eq!(tree.len(), 0);
        assert!(tree.is_empty());
    }

    #[test]
    fn test_fvt_differential_with_linear_scan() {
        let mut tree = FeatureVectorTree::new();
        let mut linear: Vec<(ClauseId, FeatureVector)> = Vec::new();

        // Deterministic pseudo-random sequence
        let mut seed: u64 = 0x123456789ABCDEF0;
        let mut rand_u16 = || -> u16 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            (seed >> 48) as u16
        };

        // Insert 60 clauses to trigger multiple splits and depth branching
        for i in 1..=60 {
            let id = ClauseId(i);
            let mut fv = FeatureVector::default();
            fv.num_lits = (rand_u16() % 8 + 1) as u32;
            fv.pos_lits = (rand_u16() % (fv.num_lits as u16 + 1)) as u32;
            fv.neg_lits = fv.num_lits - fv.pos_lits;
            for b in 0..10 {
                fv.sym_counts[b] = rand_u16() % 5;
            }

            tree.insert(id, fv);
            linear.push((id, fv));
        }
        assert_eq!(tree.len(), 60);

        // Run 40 query tests and compare results with exact linear scan
        for _ in 0..40 {
            let mut query_fv = FeatureVector::default();
            query_fv.num_lits = (rand_u16() % 10 + 1) as u32;
            query_fv.pos_lits = (rand_u16() % (query_fv.num_lits as u16 + 1)) as u32;
            query_fv.neg_lits = query_fv.num_lits - query_fv.pos_lits;
            for b in 0..10 {
                query_fv.sym_counts[b] = rand_u16() % 5;
            }

            // 1. query_subsumers vs linear
            let mut tree_subsumers = Vec::new();
            tree.query_subsumers(&query_fv, &mut tree_subsumers);
            tree_subsumers.sort();

            let mut linear_subsumers: Vec<ClauseId> = linear
                .iter()
                .filter(|(_, fv)| fv.can_subsume(&query_fv))
                .map(|(id, _)| *id)
                .collect();
            linear_subsumers.sort();
            assert_eq!(tree_subsumers, linear_subsumers);

            // 2. query_subsumed vs linear
            let mut tree_subsumed = Vec::new();
            tree.query_subsumed(&query_fv, &mut tree_subsumed);
            tree_subsumed.sort();

            let mut linear_subsumed: Vec<ClauseId> = linear
                .iter()
                .filter(|(_, fv)| query_fv.can_subsume(fv))
                .map(|(id, _)| *id)
                .collect();
            linear_subsumed.sort();
            assert_eq!(tree_subsumed, linear_subsumed);

            // 3. query_subsumption_resolution vs linear
            let mut tree_sr = Vec::new();
            tree.query_subsumption_resolution(&query_fv, &mut tree_sr);
            tree_sr.sort();

            let mut linear_sr: Vec<ClauseId> = linear
                .iter()
                .filter(|(_, fv)| fv.can_subsumption_resolve(&query_fv))
                .map(|(id, _)| *id)
                .collect();
            linear_sr.sort();
            assert_eq!(tree_sr, linear_sr);

            // 4. query_backward_subsumption_resolution vs linear
            let mut tree_bsr = Vec::new();
            tree.query_backward_subsumption_resolution(&query_fv, &mut tree_bsr);
            tree_bsr.sort();

            let mut linear_bsr: Vec<ClauseId> = linear
                .iter()
                .filter(|(_, fv)| query_fv.can_subsumption_resolve(fv))
                .map(|(id, _)| *id)
                .collect();
            linear_bsr.sort();
            assert_eq!(tree_bsr, linear_bsr);
        }

        // Test removals
        for (id, fv) in linear.drain(0..30) {
            let removed = tree.remove(id, &fv);
            assert!(removed);
        }
        assert_eq!(tree.len(), 30);
    }
}
