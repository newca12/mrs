//! Multi-hop relational symbol-goal distance graph and clause distance estimation.
//!
//! Non-equational and mixed problems (FNE, FEQ, EPR) often feature large axiom sets
//! where only a small subset of axioms interact with the conjecture predicates.
//! Goal-distance guidance calculates the shortest path from each symbol to the
//! conjecture in the clause-symbol bipartite incidence graph:
//!
//! - **Distance 0**: Symbols directly present in the negated conjecture.
//! - **Distance 1**: Symbols appearing in axioms that share at least one distance-0 symbol.
//! - **Distance 2**: Symbols appearing in axioms that share at least one distance-1 symbol.
//! - ... up to [`MAX_GOAL_RADIUS`].
//!
//! Clauses are then assigned a goal distance based on the minimum distance of the symbols
//! they contain, allowing goal-directed queues and clause weight functions to prioritize
//! proof-relevant axioms and derived clauses over irrelevant problem background axioms.

use mrs_core::clause::{Clause, ClauseSource};
use mrs_core::formula::Atom;
use mrs_core::symbol::SymbolId;
use mrs_core::term::Term;
use mrs_core::term_bank::{IdAtom, IdClause, TermBank, TermId, TermNode};
use rustc_hash::{FxHashMap, FxHashSet};

/// Maximum BFS radius for symbol reachability from the conjecture.
pub const MAX_GOAL_RADIUS: u8 = 5;

/// Sentinel goal distance for disconnected clauses or symbols.
pub const DISCONNECTED_GOAL_DISTANCE: u8 = 100;

/// Maps symbols to their relational distance from the problem's conjecture.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GoalDistanceMap {
    /// Distance of each reachable symbol from the conjecture (0 = in conjecture).
    symbol_distances: FxHashMap<SymbolId, u8>,
    /// Fast lookup set of symbols appearing directly in the conjecture (distance == 0).
    conjecture_symbols: FxHashSet<SymbolId>,
    /// Whether any conjecture was present in the problem.
    has_conjecture: bool,
}

impl GoalDistanceMap {
    /// Computes the symbol-goal distance map from the initial problem clauses.
    pub fn compute(initial_clauses: &[Clause]) -> Self {
        let mut conjecture_symbols = FxHashSet::default();
        let mut symbol_distances = FxHashMap::default();
        let mut has_conjecture = false;

        // 1. Identify conjecture clauses and collect Level-0 symbols
        let mut non_conjecture_clauses: Vec<Vec<SymbolId>> = Vec::new();

        for c in initial_clauses {
            let is_goal = matches!(&c.source, ClauseSource::Input { role, .. } if role == "conjecture" || role == "negated_conjecture")
                || c.distance == 0;

            let syms = extract_clause_symbols(c);
            if is_goal {
                has_conjecture = true;
                for &s in &syms {
                    conjecture_symbols.insert(s);
                    symbol_distances.insert(s, 0);
                }
            } else if !syms.is_empty() {
                non_conjecture_clauses.push(syms);
            }
        }

        if !has_conjecture || conjecture_symbols.is_empty() {
            return Self {
                symbol_distances,
                conjecture_symbols,
                has_conjecture,
            };
        }

        // 2. BFS iteratively outward through axioms up to MAX_GOAL_RADIUS
        let mut current_radius: u8 = 0;
        let mut remaining_clauses = non_conjecture_clauses;

        while current_radius < MAX_GOAL_RADIUS && !remaining_clauses.is_empty() {
            let next_radius = current_radius + 1;
            let mut next_remaining = Vec::new();
            let mut newly_discovered = Vec::new();

            for syms in remaining_clauses {
                let connects = syms
                    .iter()
                    .any(|s| symbol_distances.get(s).copied() == Some(current_radius));
                if connects {
                    for &s in &syms {
                        if let std::collections::hash_map::Entry::Vacant(e) =
                            symbol_distances.entry(s)
                        {
                            e.insert(next_radius);
                            newly_discovered.push(s);
                        }
                    }
                } else {
                    next_remaining.push(syms);
                }
            }

            if newly_discovered.is_empty() {
                break;
            }
            remaining_clauses = next_remaining;
            current_radius = next_radius;
        }

        Self {
            symbol_distances,
            conjecture_symbols,
            has_conjecture,
        }
    }

    /// Returns `true` if the problem contained a conjecture.
    #[inline]
    pub fn has_conjecture(&self) -> bool {
        self.has_conjecture
    }

    /// Returns the symbol distance for a given symbol, if reachable.
    #[inline]
    pub fn symbol_distance(&self, sym: SymbolId) -> Option<u8> {
        self.symbol_distances.get(&sym).copied()
    }

    /// Returns true if the symbol appeared directly in the conjecture.
    #[inline]
    pub fn is_conjecture_symbol(&self, sym: SymbolId) -> bool {
        self.conjecture_symbols.contains(&sym)
    }

    /// Returns a reference to the set of conjecture symbols.
    #[inline]
    pub fn conjecture_symbols(&self) -> &FxHashSet<SymbolId> {
        &self.conjecture_symbols
    }

    /// Computes the goal distance for an `IdClause`.
    ///
    /// - Distance 0: conjecture clause (`clause.distance == 0`).
    /// - Distance 1: shares a symbol directly with the conjecture.
    /// - Distance 2: shares a symbol with a distance-1 clause.
    /// - Distance d: minimum symbol distance in clause + 1.
    /// - Distance 100: disconnected from the conjecture.
    pub fn clause_goal_distance(&self, clause: &IdClause, bank: &TermBank) -> u8 {
        if !self.has_conjecture {
            return DISCONNECTED_GOAL_DISTANCE;
        }
        if clause.distance == 0 {
            return 0;
        }

        let mut min_sym_dist: Option<u8> = None;

        for lit in &clause.literals {
            match &lit.atom {
                IdAtom::Pred(p, args) => {
                    if let Some(&d) = self.symbol_distances.get(p) {
                        min_sym_dist = Some(min_sym_dist.map_or(d, |m| m.min(d)));
                    }
                    for &arg in args {
                        inspect_term_symbols(arg, bank, &self.symbol_distances, &mut min_sym_dist);
                    }
                }
                IdAtom::Eq(l, r) => {
                    inspect_term_symbols(*l, bank, &self.symbol_distances, &mut min_sym_dist);
                    inspect_term_symbols(*r, bank, &self.symbol_distances, &mut min_sym_dist);
                }
            }
        }

        let sym_dist = match min_sym_dist {
            Some(0) => 1,
            Some(d) => (d + 1).min(DISCONNECTED_GOAL_DISTANCE),
            None => DISCONNECTED_GOAL_DISTANCE,
        };

        // Combine derivation distance and symbol distance
        let deriv_dist = (clause.distance.min(DISCONNECTED_GOAL_DISTANCE as u32)) as u8;
        if deriv_dist == 0 {
            0
        } else {
            deriv_dist.min(sym_dist)
        }
    }
}

fn extract_clause_symbols(c: &Clause) -> Vec<SymbolId> {
    let mut syms = FxHashSet::default();
    for lit in &c.literals {
        match &lit.atom {
            Atom::Pred(p, args) => {
                syms.insert(*p);
                for arg in args {
                    collect_fof_term_symbols(arg, &mut syms);
                }
            }
            Atom::Eq(l, r) => {
                collect_fof_term_symbols(l, &mut syms);
                collect_fof_term_symbols(r, &mut syms);
            }
        }
    }
    syms.into_iter().collect()
}

fn collect_fof_term_symbols(term: &Term, syms: &mut FxHashSet<SymbolId>) {
    let mut stack = vec![term];
    while let Some(t) = stack.pop() {
        if let Term::App(f, args) = t {
            syms.insert(*f);
            stack.extend(args.iter());
        }
    }
}

fn inspect_term_symbols(
    term: TermId,
    bank: &TermBank,
    sym_dists: &FxHashMap<SymbolId, u8>,
    min_dist: &mut Option<u8>,
) {
    let mut stack = vec![term];
    while let Some(t) = stack.pop() {
        if let TermNode::App(f, args) = bank.get(t) {
            if let Some(&d) = sym_dists.get(f) {
                *min_dist = Some(min_dist.map_or(d, |m| m.min(d)));
                if *min_dist == Some(0) {
                    return;
                }
            }
            stack.extend_from_slice(args);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::clause::ClauseId;
    use mrs_core::{Literal, SymbolTable};

    #[test]
    fn test_goal_distance_reachability() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let r = syms.intern("r");
        let s = syms.intern("s");
        let c = syms.intern("c");
        let d = syms.intern("d");
        let e = syms.intern("e");
        let f = syms.intern("f");

        let mut bank = TermBank::new();

        // Conjecture: ~p(c)
        let conj = Clause {
            id: ClauseId(1),
            literals: [Literal::neg(Atom::pred(p, vec![Term::constant(c)]))]
                .as_slice()
                .into(),
            source: ClauseSource::Input {
                name: "conj".into(),
                role: "conjecture".into(),
            },
            avatar: vec![],
            distance: 0,
            formula: None,
            certificate: None,
        };

        // Axiom 1: p(X) | ~q(X, d)  (shares 'p' with conjecture -> dist 1)
        let ax1 = Clause {
            id: ClauseId(2),
            literals: [
                Literal::pos(Atom::pred(p, vec![Term::var(0)])),
                Literal::neg(Atom::pred(q, vec![Term::var(0), Term::constant(d)])),
            ]
            .as_slice()
            .into(),
            source: ClauseSource::Input {
                name: "ax1".into(),
                role: "axiom".into(),
            },
            avatar: vec![],
            distance: 100,
            formula: None,
            certificate: None,
        };

        // Axiom 2: q(e, Y) | ~r(Y)  (shares 'q' with ax1 -> dist 2)
        let ax2 = Clause {
            id: ClauseId(3),
            literals: [
                Literal::pos(Atom::pred(q, vec![Term::constant(e), Term::var(1)])),
                Literal::neg(Atom::pred(r, vec![Term::var(1)])),
            ]
            .as_slice()
            .into(),
            source: ClauseSource::Input {
                name: "ax2".into(),
                role: "axiom".into(),
            },
            avatar: vec![],
            distance: 100,
            formula: None,
            certificate: None,
        };

        // Axiom 3: s(f)  (completely disconnected)
        let ax3 = Clause {
            id: ClauseId(4),
            literals: [Literal::pos(Atom::pred(s, vec![Term::constant(f)]))]
                .as_slice()
                .into(),
            source: ClauseSource::Input {
                name: "ax3".into(),
                role: "axiom".into(),
            },
            avatar: vec![],
            distance: 100,
            formula: None,
            certificate: None,
        };

        let map = GoalDistanceMap::compute(&[conj.clone(), ax1.clone(), ax2.clone(), ax3.clone()]);
        assert!(map.has_conjecture());

        // Level 0: p, c
        assert_eq!(map.symbol_distance(p), Some(0));
        assert_eq!(map.symbol_distance(c), Some(0));
        assert!(map.is_conjecture_symbol(p));
        assert!(map.is_conjecture_symbol(c));

        // Level 1: q, d
        assert_eq!(map.symbol_distance(q), Some(1));
        assert_eq!(map.symbol_distance(d), Some(1));
        assert!(!map.is_conjecture_symbol(q));

        // Level 2: e, r
        assert_eq!(map.symbol_distance(e), Some(2));
        assert_eq!(map.symbol_distance(r), Some(2));

        // Disconnected: s, f
        assert_eq!(map.symbol_distance(s), None);
        assert_eq!(map.symbol_distance(f), None);

        // Test clause goal distance
        let id_conj = bank.clause_from_legacy(&conj);
        let id_ax1 = bank.clause_from_legacy(&ax1);
        let id_ax2 = bank.clause_from_legacy(&ax2);
        let id_ax3 = bank.clause_from_legacy(&ax3);

        assert_eq!(map.clause_goal_distance(&id_conj, &bank), 0);
        assert_eq!(map.clause_goal_distance(&id_ax1, &bank), 1);
        assert_eq!(map.clause_goal_distance(&id_ax2, &bank), 2);
        assert_eq!(map.clause_goal_distance(&id_ax3, &bank), 100);
    }
}
