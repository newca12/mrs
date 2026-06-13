//! Clause weight heuristics.
//!
//! Weight functions assign a numeric cost to clauses, used by the clause
//! selection strategy to prefer simpler (lighter) clauses during proof search.
//!
//! The standard weight counts each symbol occurrence (function symbols and
//! variables) as 1, summing over all literals.

use mrs_calculus::ordering::SymbolConfig;
use mrs_core::clause::{Clause, Literal};
use mrs_core::formula::Atom;
use mrs_core::term::Term;
use mrs_core::term_bank::{IdAtom, IdClause, IdLiteral, TermBank, TermId, TermNode};

/// Returns the weight of a clause: the sum of symbol occurrences across all literals.
///
/// Lighter clauses are generally preferred because they represent simpler facts.
///
/// Uses an iterative traversal to avoid stack overflow on deeply nested terms.
pub fn clause_weight(clause: &Clause, config: &SymbolConfig) -> u32 {
    clause
        .literals
        .iter()
        .map(|lit| literal_weight(lit, config))
        .sum()
}

/// Returns true if the clause weight exceeds `max`.
///
/// Bails out early as soon as the running total exceeds the limit, so it is
/// cheaper than computing the full weight when the clause is very heavy.
pub fn clause_weight_exceeds(clause: &Clause, max: u32, config: &SymbolConfig) -> bool {
    let mut total: u32 = 0;
    for lit in &clause.literals {
        total = total.saturating_add(literal_weight(lit, config));
        if total > max {
            return true;
        }
    }
    false
}

/// Returns the weight of a single literal.
fn literal_weight(lit: &Literal, config: &SymbolConfig) -> u32 {
    atom_weight(&lit.atom, config)
}

/// Returns the weight of an atom.
fn atom_weight(atom: &Atom, config: &SymbolConfig) -> u32 {
    match atom {
        Atom::Pred(sym, args) => {
            config.symbol_weight(*sym)
                + args.iter().map(|arg| term_weight(arg, config)).sum::<u32>()
        }
        Atom::Eq(l, r) => term_weight(l, config) + term_weight(r, config),
    }
}

/// Returns the weight of a term using an iterative (explicit-stack) traversal.
///
/// This avoids stack overflow on deeply nested terms that arise during
/// superposition without demodulation simplification.
pub fn term_weight(term: &Term, config: &SymbolConfig) -> u32 {
    let mut stack: Vec<&Term> = vec![term];
    let mut weight: u32 = 0;
    while let Some(t) = stack.pop() {
        match t {
            Term::Var(_) => {
                weight = weight.saturating_add(config.w0);
            }
            Term::App(sym, args) => {
                weight = weight.saturating_add(config.symbol_weight(*sym));
                stack.extend(args.iter());
            }
        }
    }
    weight
}

// ── IdClause / TermBank variants ────────────────────────────────────────────

/// Returns the weight of an `IdClause`.
pub fn clause_weight_id(clause: &IdClause, bank: &TermBank, config: &SymbolConfig) -> u32 {
    clause
        .literals
        .iter()
        .map(|lit| literal_weight_id(lit, bank, config))
        .sum()
}

/// Returns true if the `IdClause` weight exceeds `max`. Bails out early.
pub fn clause_weight_exceeds_id(
    clause: &IdClause,
    max: u32,
    bank: &TermBank,
    config: &SymbolConfig,
) -> bool {
    let mut total: u32 = 0;
    for lit in &clause.literals {
        total = total.saturating_add(literal_weight_id(lit, bank, config));
        if total > max {
            return true;
        }
    }
    false
}

fn literal_weight_id(lit: &IdLiteral, bank: &TermBank, config: &SymbolConfig) -> u32 {
    match &lit.atom {
        IdAtom::Pred(sym, args) => {
            config.symbol_weight(*sym)
                + args
                    .iter()
                    .map(|&a| term_weight_id(a, bank, config))
                    .sum::<u32>()
        }
        IdAtom::Eq(l, r) => term_weight_id(*l, bank, config) + term_weight_id(*r, bank, config),
    }
}

fn term_weight_id(term: TermId, bank: &TermBank, config: &SymbolConfig) -> u32 {
    let mut stack: Vec<TermId> = vec![term];
    let mut weight: u32 = 0;
    while let Some(t) = stack.pop() {
        match bank.get(t) {
            TermNode::Var(_) => {
                weight = weight.saturating_add(config.w0);
            }
            TermNode::App(sym, args) => {
                weight = weight.saturating_add(config.symbol_weight(*sym));
                stack.extend_from_slice(args);
            }
        }
    }
    weight
}

// ── Alternative weight functions ────────────────────────────────────────────

use crate::ClauseWeightFn;
use rustc_hash::FxHashSet;

/// Dispatch: compute the weight of an `IdClause` using the chosen weight function.
///
/// `goal_symbols` is the set of `SymbolId`s that appear in any clause with
/// `distance < 100` (i.e. goal-connected clauses).  It is only consulted by
/// `ConjSymbolBoost`; pass an empty set for the other variants.
pub fn clause_weight_fn(
    clause: &IdClause,
    bank: &TermBank,
    config: &SymbolConfig,
    weight_fn: &ClauseWeightFn,
    goal_symbols: &FxHashSet<mrs_core::SymbolId>,
) -> u32 {
    match weight_fn {
        ClauseWeightFn::Standard => clause_weight_id(clause, bank, config),
        ClauseWeightFn::FunctionDepth => clause_weight_depth(clause, bank, config),
        ClauseWeightFn::HornPenalty => clause_weight_horn(clause, bank, config),
        ClauseWeightFn::ConjSymbolBoost => {
            clause_weight_conj_boost(clause, bank, config, goal_symbols)
        }
    }
}

/// Depth-weighted variant: every symbol occurrence costs `weight * (depth + 1)`.
/// Deeply nested terms are penalised, steering the prover away from term-tower chains.
pub fn clause_weight_depth(clause: &IdClause, bank: &TermBank, config: &SymbolConfig) -> u32 {
    clause
        .literals
        .iter()
        .map(|lit| literal_weight_depth(lit, bank, config))
        .sum()
}

fn literal_weight_depth(lit: &IdLiteral, bank: &TermBank, config: &SymbolConfig) -> u32 {
    match &lit.atom {
        IdAtom::Pred(sym, args) => {
            config.symbol_weight(*sym)
                + args
                    .iter()
                    .map(|&a| term_weight_depth(a, 1, bank, config))
                    .sum::<u32>()
        }
        IdAtom::Eq(l, r) => {
            term_weight_depth(*l, 0, bank, config) + term_weight_depth(*r, 0, bank, config)
        }
    }
}

fn term_weight_depth(term: TermId, depth: u32, bank: &TermBank, config: &SymbolConfig) -> u32 {
    let mut stack: Vec<(TermId, u32)> = vec![(term, depth)];
    let mut weight: u32 = 0;
    while let Some((t, d)) = stack.pop() {
        match bank.get(t) {
            TermNode::Var(_) => {
                weight = weight.saturating_add(config.w0.saturating_mul(d + 1));
            }
            TermNode::App(sym, args) => {
                weight = weight.saturating_add(config.symbol_weight(*sym).saturating_mul(d + 1));
                for &a in args {
                    stack.push((a, d + 1));
                }
            }
        }
    }
    weight
}

/// Horn-penalty variant: same as Standard but clauses with >1 positive literal
/// pay a 3× multiplier.  Horn clauses (≤1 positive literal) are preferred.
pub fn clause_weight_horn(clause: &IdClause, bank: &TermBank, config: &SymbolConfig) -> u32 {
    let base = clause_weight_id(clause, bank, config);
    let pos_count = clause.literals.iter().filter(|l| l.positive).count();
    if pos_count > 1 {
        base.saturating_mul(3)
    } else {
        base
    }
}

/// Conjecture-symbol-boost variant: symbols shared with the negated conjecture
/// closure cost 1; symbols not appearing in any goal-connected clause cost 3.
pub fn clause_weight_conj_boost(
    clause: &IdClause,
    bank: &TermBank,
    config: &SymbolConfig,
    goal_symbols: &FxHashSet<mrs_core::SymbolId>,
) -> u32 {
    if goal_symbols.is_empty() {
        // Fall back to standard when no goal symbol information is available.
        return clause_weight_id(clause, bank, config);
    }
    clause
        .literals
        .iter()
        .map(|lit| literal_weight_conj_boost(lit, bank, config, goal_symbols))
        .sum()
}

fn literal_weight_conj_boost(
    lit: &IdLiteral,
    bank: &TermBank,
    config: &SymbolConfig,
    goal_symbols: &FxHashSet<mrs_core::SymbolId>,
) -> u32 {
    match &lit.atom {
        IdAtom::Pred(sym, args) => {
            let sym_w = if goal_symbols.contains(sym) {
                1
            } else {
                config.symbol_weight(*sym).saturating_mul(3)
            };
            sym_w
                + args
                    .iter()
                    .map(|&a| term_weight_conj_boost(a, bank, config, goal_symbols))
                    .sum::<u32>()
        }
        IdAtom::Eq(l, r) => {
            term_weight_conj_boost(*l, bank, config, goal_symbols)
                + term_weight_conj_boost(*r, bank, config, goal_symbols)
        }
    }
}

fn term_weight_conj_boost(
    term: TermId,
    bank: &TermBank,
    config: &SymbolConfig,
    goal_symbols: &FxHashSet<mrs_core::SymbolId>,
) -> u32 {
    let mut stack: Vec<TermId> = vec![term];
    let mut weight: u32 = 0;
    while let Some(t) = stack.pop() {
        match bank.get(t) {
            TermNode::Var(_) => {
                weight = weight.saturating_add(config.w0);
            }
            TermNode::App(sym, args) => {
                let sym_w = if goal_symbols.contains(sym) {
                    1
                } else {
                    config.symbol_weight(*sym).saturating_mul(3)
                };
                weight = weight.saturating_add(sym_w);
                stack.extend_from_slice(args);
            }
        }
    }
    weight
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::SymbolTable;
    use mrs_core::clause::{ClauseId, ClauseSource};

    fn make_clause(lits: Vec<Literal>) -> Clause {
        Clause::new(
            ClauseId(0),
            lits,
            ClauseSource::Input {
                name: "test".into(),
                role: "axiom".into(),
            },
        )
        .with_distance(0)
    }

    #[test]
    fn weight_of_empty_clause() {
        let c = make_clause(vec![]);
        let config = SymbolConfig::default();
        assert_eq!(clause_weight(&c, &config), 0);
    }

    #[test]
    fn weight_of_propositional_literal() {
        // p() -> predicate symbol counts as 1
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let c = make_clause(vec![Literal::pos(Atom::prop(p))]);
        let config = SymbolConfig::default();
        assert_eq!(clause_weight(&c, &config), 1);
    }

    #[test]
    fn weight_of_unary_predicate() {
        // p(a) -> p=1, a=1 -> weight 2
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let c = make_clause(vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))]);
        let config = SymbolConfig::default();
        assert_eq!(clause_weight(&c, &config), 2);
    }

    #[test]
    fn weight_of_nested_term() {
        // p(f(a, X)) -> p=1, f=1, a=1, X=1 -> weight 4
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let f = syms.intern("f");
        let a = syms.intern("a");
        let c = make_clause(vec![Literal::pos(Atom::pred(
            p,
            vec![Term::app(f, vec![Term::constant(a), Term::var(0)])],
        ))]);
        let config = SymbolConfig::default();
        assert_eq!(clause_weight(&c, &config), 4);
    }

    #[test]
    fn weight_of_equality() {
        // a = b -> weight 2 (two constants, no predicate symbol counted)
        let mut syms = SymbolTable::new();
        let a = syms.intern("a");
        let b = syms.intern("b");
        let c = make_clause(vec![Literal::pos(Atom::eq(
            Term::constant(a),
            Term::constant(b),
        ))]);
        let config = SymbolConfig::default();
        assert_eq!(clause_weight(&c, &config), 2);
    }

    #[test]
    fn weight_of_multi_literal_clause() {
        // p(a) | q(X, b) -> p=1 + a=1 + q=1 + X=1 + b=1 = 5
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let c = make_clause(vec![
            Literal::pos(Atom::pred(p, vec![Term::constant(a)])),
            Literal::pos(Atom::pred(q, vec![Term::var(0), Term::constant(b)])),
        ]);
        let config = SymbolConfig::default();
        assert_eq!(clause_weight(&c, &config), 5);
    }

    #[test]
    fn weight_negative_same_as_positive() {
        // Weight doesn't depend on polarity
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let pos = make_clause(vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))]);
        let neg = make_clause(vec![Literal::neg(Atom::pred(p, vec![Term::constant(a)]))]);
        let config = SymbolConfig::default();
        assert_eq!(clause_weight(&pos, &config), clause_weight(&neg, &config));
    }

    #[test]
    fn weight_exceeds_detects_heavy_clauses() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let c = make_clause(vec![
            Literal::pos(Atom::pred(p, vec![Term::constant(a)])),
            Literal::pos(Atom::pred(p, vec![Term::constant(b)])),
        ]);
        let config = SymbolConfig::default();
        // weight = 4; threshold 3 should trigger, threshold 4 should not
        assert!(clause_weight_exceeds(&c, 3, &config));
        assert!(!clause_weight_exceeds(&c, 4, &config));
    }
}
