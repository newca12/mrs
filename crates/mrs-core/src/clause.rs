//! Clausal representation: literals and clauses.
//!
//! A [`Literal`] is a signed atomic formula (positive or negative).
//! A [`Clause`] is a disjunction of literals, implicitly universally quantified
//! over all its variables. This is the working format for resolution-based provers.

use crate::HashSet;
use smallvec::SmallVec;

use crate::Formula;
use crate::formula::Atom;
use crate::term::VarId;

/// Unique identifier for a clause within a proof search.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct ClauseId(pub u64);

/// A literal: an atom with a polarity (positive or negative).
///
/// A positive literal `L` asserts that `L` holds.
/// A negative literal `¬L` asserts that `L` does not hold.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Literal {
    /// `true` for a positive literal, `false` for a negative literal.
    pub positive: bool,
    /// The underlying atomic formula.
    pub atom: Atom,
}

impl Literal {
    /// Creates a positive literal.
    pub fn pos(atom: Atom) -> Self {
        Literal {
            positive: true,
            atom,
        }
    }

    /// Creates a negative literal.
    pub fn neg(atom: Atom) -> Self {
        Literal {
            positive: false,
            atom,
        }
    }

    /// Returns the complement of this literal (flipped polarity, same atom).
    pub fn complement(&self) -> Self {
        Literal {
            positive: !self.positive,
            atom: self.atom.clone(),
        }
    }

    /// Returns `true` if this literal is positive.
    pub fn is_positive(&self) -> bool {
        self.positive
    }

    /// Returns `true` if this literal is negative.
    pub fn is_negative(&self) -> bool {
        !self.positive
    }

    /// Collects all free variable IDs in this literal.
    pub fn collect_vars(&self, vars: &mut HashSet<VarId>) {
        self.atom.collect_vars(vars);
    }
}

/// Records how a clause was derived.
///
/// This is essential for proof reconstruction: every clause knows
/// its origin, whether it was an input axiom or the result of an inference.
#[derive(Clone, Debug)]
pub enum ClauseSource {
    /// Input clause from the problem (axiom, hypothesis, negated conjecture, etc.).
    Input {
        /// The original name from the TPTP file.
        name: String,
        /// The role of this input formula.
        role: String,
    },
    /// Derived by an inference rule from parent clauses.
    Inference {
        /// Name of the inference rule (e.g., "resolution", "factoring").
        rule: &'static str,
        /// IDs of the parent clauses.
        parents: SmallVec<[ClauseId; 2]>,
    },
}

/// A clause: a disjunction of literals.
///
/// All variables in a clause are implicitly universally quantified.
/// The empty clause (no literals) represents a contradiction (⊥).
#[derive(Clone, Debug)]
pub struct Clause {
    /// Unique identifier for this clause.
    pub id: ClauseId,
    /// The literals in this clause (their disjunction).
    pub literals: SmallVec<[Literal; 4]>,
    /// How this clause was derived.
    pub source: ClauseSource,
    /// AVATAR assertions (boolean variables from the SAT solver).
    pub avatar: Vec<u32>,
    /// Distance to conjecture (0 for conjectures, +1 for generated, large for axioms).
    pub distance: u32,
    /// If set, this "clause" is actually a non-clausal, FOF-level proof step
    /// (e.g. an NNF conversion or Skolemization result) rather than a real
    /// disjunction of literals. `literals` is unused/empty in that case.
    ///
    /// This exists because TSTP derivations of FOF problems commonly need to
    /// cite intermediate formula-level transformation steps (which may still
    /// contain quantifiers or nested and/or structure) before the formula
    /// reaches clausal (CNF) shape — see `mrs-cnf::clausify`'s `fof_nnf_transformation`
    /// / `skolemisation` steps. Proof formatting (`mrs-proof::tstp`) prints
    /// these as `fof(...)` annotated formulas instead of `cnf(...)`.
    ///
    /// **Never add a `Clause` with `formula: Some(_)` to the live given-clause
    /// search** (`processed`/`unprocessed`): its `literals` field is empty,
    /// which is indistinguishable from the empty clause (a refutation) to
    /// the search loop. These clauses exist only for proof-provenance lookup
    /// (`clause_store`) at proof-extraction time.
    pub formula: Option<Formula>,
}

impl Clause {
    /// Creates a new clause with the given ID, literals, and source, with empty AVATAR assertions.
    pub fn new<L>(id: ClauseId, literals: L, source: ClauseSource) -> Self
    where
        L: Into<SmallVec<[Literal; 4]>>,
    {
        Clause {
            id,
            literals: literals.into(),
            source,
            avatar: Vec::new(),
            distance: 1000,
            formula: None,
        }
    }

    /// Creates a non-clausal, FOF-level proof step: a formula that has not
    /// (yet) reached clausal shape, cited via `source` (`Input` for the
    /// original leaf formula, `Inference` for a named transformation like
    /// `fof_nnf_transformation`/`skolemisation`/`negated_conjecture`).
    ///
    /// See the [`Clause::formula`] doc comment for the critical caveat about
    /// never feeding these into the live given-clause search.
    pub fn new_formula_step(id: ClauseId, formula: Formula, source: ClauseSource) -> Self {
        Clause {
            id,
            literals: SmallVec::new(),
            source,
            avatar: Vec::new(),
            distance: 1000,
            formula: Some(formula),
        }
    }

    /// Creates a new clause with AVATAR assertions.
    pub fn new_avatar<L>(
        id: ClauseId,
        literals: L,
        source: ClauseSource,
        mut avatar: Vec<u32>,
    ) -> Self
    where
        L: Into<SmallVec<[Literal; 4]>>,
    {
        avatar.sort_unstable();
        avatar.dedup();
        Clause {
            id,
            literals: literals.into(),
            source,
            avatar,
            distance: 1000,
            formula: None,
        }
    }

    /// Set the distance for this clause.
    pub fn with_distance(mut self, distance: u32) -> Self {
        self.distance = distance;
        self
    }

    /// Returns `true` if `self.avatar` is a subset of `other.avatar`.
    ///
    /// In AVATAR, a clause `C1` can only destructively simplify or subsume `C2`
    /// if `C1`'s avatar assertions are a subset of `C2`'s. Otherwise, `C1` might be
    /// false in a SAT model where `C2` is true, meaning `C2` was incorrectly deleted.
    pub fn avatar_is_subset_of(&self, other: &Clause) -> bool {
        // avatar is always sorted and deduplicated
        let mut i = 0;
        let mut j = 0;
        while i < self.avatar.len() && j < other.avatar.len() {
            if self.avatar[i] < other.avatar[j] {
                return false;
            } else if self.avatar[i] == other.avatar[j] {
                i += 1;
                j += 1;
            } else {
                j += 1;
            }
        }
        i == self.avatar.len()
    }

    /// Returns `true` if this is the empty clause (contradiction).
    pub fn is_empty(&self) -> bool {
        self.literals.is_empty()
    }

    /// Returns the number of literals in this clause.
    pub fn len(&self) -> usize {
        self.literals.len()
    }

    /// Returns `true` if this is a unit clause (exactly one literal).
    pub fn is_unit(&self) -> bool {
        self.literals.len() == 1
    }

    /// Collects all variable IDs occurring in this clause.
    pub fn free_vars(&self) -> HashSet<VarId> {
        let mut vars = HashSet::default();
        for lit in &self.literals {
            lit.collect_vars(&mut vars);
        }
        vars
    }

    /// Removes duplicate literals from this clause, keeping the first occurrence of each.
    pub fn deduplicate(&mut self) {
        let mut seen = Vec::new();
        self.literals.retain(|lit| {
            if seen.contains(lit) {
                false
            } else {
                seen.push(lit.clone());
                true
            }
        });
    }

    /// Returns `true` if this clause is a tautology.
    ///
    /// Detects two kinds of tautology:
    /// - Complementary literals: the clause contains both `L` and `¬L`.
    /// - Equality reflexivity: the clause contains a positive `s = s` literal.
    pub fn is_tautology(&self) -> bool {
        // Check for positive s = s (equality reflexivity)
        for lit in &self.literals {
            if lit.is_positive()
                && let Atom::Eq(l, r) = &lit.atom
                && l == r
            {
                return true;
            }
        }
        // Check for complementary literals
        for (i, lit1) in self.literals.iter().enumerate() {
            for lit2 in &self.literals[i + 1..] {
                if lit1.positive != lit2.positive && lit1.atom == lit2.atom {
                    return true;
                }
            }
        }
        false
    }
}

/// A counter for generating unique clause IDs.
#[derive(Clone, Debug, Default)]
pub struct ClauseIdGen {
    next: u64,
}

impl ClauseIdGen {
    /// Creates a new generator starting at 0.
    pub fn new() -> Self {
        Self { next: 0 }
    }

    /// Returns the next unique clause ID.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> ClauseId {
        let id = ClauseId(self.next);
        self.next += 1;
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::SymbolTable;
    use crate::term::Term;

    #[test]
    fn empty_clause() {
        let c = Clause::new(
            ClauseId(0),
            vec![],
            ClauseSource::Input {
                name: "empty".into(),
                role: "axiom".into(),
            },
        );
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }

    #[test]
    fn unit_clause() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let c = Clause::new(
            ClauseId(0),
            vec![Literal::pos(Atom::pred(p, vec![Term::var(0)]))],
            ClauseSource::Input {
                name: "unit".into(),
                role: "axiom".into(),
            },
        );
        assert!(c.is_unit());
        assert!(!c.is_empty());
    }

    #[test]
    fn tautology_detection() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let atom = Atom::pred(p, vec![Term::var(0)]);

        // { p(X), ¬p(X) } is a tautology
        let c = Clause::new(
            ClauseId(0),
            vec![Literal::pos(atom.clone()), Literal::neg(atom)],
            ClauseSource::Input {
                name: "taut".into(),
                role: "axiom".into(),
            },
        );
        assert!(c.is_tautology());
    }

    #[test]
    fn non_tautology() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");

        // { p(X), ¬q(X) } is not a tautology
        let c = Clause::new(
            ClauseId(0),
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(0)])),
                Literal::neg(Atom::pred(q, vec![Term::var(0)])),
            ],
            ClauseSource::Input {
                name: "nontaut".into(),
                role: "axiom".into(),
            },
        );
        assert!(!c.is_tautology());
    }

    #[test]
    fn clause_id_gen() {
        let mut id_gen = ClauseIdGen::new();
        assert_eq!(id_gen.next(), ClauseId(0));
        assert_eq!(id_gen.next(), ClauseId(1));
        assert_eq!(id_gen.next(), ClauseId(2));
    }

    #[test]
    fn equality_tautology() {
        let mut syms = SymbolTable::new();
        let a = syms.intern("a");

        // { a = a } is a tautology
        let c = Clause::new(
            ClauseId(0),
            vec![Literal::pos(Atom::eq(Term::constant(a), Term::constant(a)))],
            ClauseSource::Input {
                name: "taut".into(),
                role: "axiom".into(),
            },
        );
        assert!(c.is_tautology());
    }

    #[test]
    fn negative_equality_not_tautology() {
        let mut syms = SymbolTable::new();
        let a = syms.intern("a");

        // { a != a } is NOT a tautology (it's unsatisfiable, but not a tautology)
        let c = Clause::new(
            ClauseId(0),
            vec![Literal::neg(Atom::eq(Term::constant(a), Term::constant(a)))],
            ClauseSource::Input {
                name: "test".into(),
                role: "axiom".into(),
            },
        );
        assert!(!c.is_tautology());
    }

    #[test]
    fn deduplicate_removes_duplicates() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let atom = Atom::pred(p, vec![Term::var(0)]);

        // { p(X), p(X), ~q(X) } -> { p(X), ~q(X) } after dedup
        let q = syms.intern("q");
        let mut c = Clause::new(
            ClauseId(0),
            vec![
                Literal::pos(atom.clone()),
                Literal::pos(atom),
                Literal::neg(Atom::pred(q, vec![Term::var(0)])),
            ],
            ClauseSource::Input {
                name: "test".into(),
                role: "axiom".into(),
            },
        );
        assert_eq!(c.len(), 3);
        c.deduplicate();
        assert_eq!(c.len(), 2);
    }
}
