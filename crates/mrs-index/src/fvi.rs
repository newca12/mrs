use mrs_core::clause::Clause;
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
}
