use mrs_core::clause::Clause;
use mrs_core::formula::Atom;
use mrs_core::symbol::SymbolId;
use mrs_core::term::Term;

/// A feature vector representing the symbol frequencies of a clause.
/// Used for fast subsumption filtering.
///
/// If clause A subsumes clause B (i.e. A sigma subset B), then:
/// - A has <= literals than B
/// - A has <= positive literals than B
/// - A has <= negative literals than B
/// - For every function/predicate symbol S, A has <= occurrences of S than B.
#[derive(Clone, Default, Debug)]
pub struct FeatureVector {
    pub num_lits: u32,
    pub pos_lits: u32,
    pub neg_lits: u32,
    pub sym_counts: Vec<(SymbolId, u32)>,
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
        if let Some(pos) = self.sym_counts.iter().position(|(s, _)| *s == sym) {
            self.sym_counts[pos].1 += 1;
        } else {
            self.sym_counts.push((sym, 1));
        }
    }

    fn get_count(&self, sym: SymbolId) -> u32 {
        self.sym_counts.iter().find(|(s, _)| *s == sym).map(|(_, c)| *c).unwrap_or(0)
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

    /// Returns true if this feature vector could potentially subsume `other`.
    /// This is a fast necessary (but not sufficient) condition for subsumption.
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

        for &(sym, count) in &self.sym_counts {
            let other_count = other.get_count(sym);
            if count > other_count {
                return false;
            }
        }

        true
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

        // Symbols don't change when flipping polarity.
        for &(sym, _) in &self.sym_counts {
            if other.get_count(sym) == 0 {
                return false;
            }
        }

        true
    }
}
