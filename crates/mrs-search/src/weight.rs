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

/// Dispatch: compute the weight of an `IdClause` using the chosen weight function.
///
/// `goal_map` maps symbols to their relational distance from the conjecture.
/// It is consulted by `ConjSymbolBoost` and `GoalDistance`; pass an empty map
/// for the other variants.
pub fn clause_weight_fn(
    clause: &IdClause,
    bank: &TermBank,
    config: &SymbolConfig,
    weight_fn: &ClauseWeightFn,
    goal_map: &crate::goal_distance::GoalDistanceMap,
) -> u32 {
    match weight_fn {
        ClauseWeightFn::Standard => clause_weight_id(clause, bank, config),
        ClauseWeightFn::FunctionDepth => clause_weight_depth(clause, bank, config),
        ClauseWeightFn::FunctionWeightPenalty => clause_weight_penalty(clause, bank, config),
        ClauseWeightFn::FunctionWeightPenaltyExp => clause_weight_penalty_exp(clause, bank, config),
        ClauseWeightFn::HornPenalty => clause_weight_horn(clause, bank, config),
        ClauseWeightFn::HornHeuristic => clause_weight_horn_heuristic(clause, bank, config),
        ClauseWeightFn::HornHeuristicExp => clause_weight_horn_heuristic_exp(clause, bank, config),
        ClauseWeightFn::ConjSymbolBoost => clause_weight_conj_boost(clause, bank, config, goal_map),
        ClauseWeightFn::SymbolWeight => clause_weight_symbol(clause, bank, config),
        ClauseWeightFn::GoalDistance => clause_weight_goal_distance(clause, bank, config, goal_map),
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

/// Conjecture-symbol-boost variant: symbols are weighted according to their
/// shortest relational path to the goal:
/// - Distance 0 (conjecture symbols): cost 1
/// - Distance 1 (1-hop neighbors): base symbol weight
/// - Distance 2: 2x base symbol weight
/// - Distance 3+ / unreachable: 3x base symbol weight
pub fn clause_weight_conj_boost(
    clause: &IdClause,
    bank: &TermBank,
    config: &SymbolConfig,
    goal_map: &crate::goal_distance::GoalDistanceMap,
) -> u32 {
    if !goal_map.has_conjecture() {
        // Fall back to standard when no goal symbol information is available.
        return clause_weight_id(clause, bank, config);
    }
    clause
        .literals
        .iter()
        .map(|lit| literal_weight_conj_boost(lit, bank, config, goal_map))
        .sum()
}

fn literal_weight_conj_boost(
    lit: &IdLiteral,
    bank: &TermBank,
    config: &SymbolConfig,
    goal_map: &crate::goal_distance::GoalDistanceMap,
) -> u32 {
    match &lit.atom {
        IdAtom::Pred(sym, args) => {
            let base_w = config.symbol_weight(*sym);
            let sym_w = match goal_map.symbol_distance(*sym) {
                Some(0) => 1,
                Some(1) => base_w,
                Some(2) => base_w.saturating_mul(2),
                _ => base_w.saturating_mul(3),
            };
            sym_w
                + args
                    .iter()
                    .map(|&a| term_weight_conj_boost(a, bank, config, goal_map))
                    .sum::<u32>()
        }
        IdAtom::Eq(l, r) => {
            term_weight_conj_boost(*l, bank, config, goal_map)
                + term_weight_conj_boost(*r, bank, config, goal_map)
        }
    }
}

fn term_weight_conj_boost(
    term: TermId,
    bank: &TermBank,
    config: &SymbolConfig,
    goal_map: &crate::goal_distance::GoalDistanceMap,
) -> u32 {
    let mut stack: Vec<TermId> = vec![term];
    let mut weight: u32 = 0;
    while let Some(t) = stack.pop() {
        match bank.get(t) {
            TermNode::Var(_) => {
                weight = weight.saturating_add(config.w0);
            }
            TermNode::App(sym, args) => {
                let base_w = config.symbol_weight(*sym);
                let sym_w = match goal_map.symbol_distance(*sym) {
                    Some(0) => 1,
                    Some(1) => base_w,
                    Some(2) => base_w.saturating_mul(2),
                    _ => base_w.saturating_mul(3),
                };
                weight = weight.saturating_add(sym_w);
                stack.extend_from_slice(args);
            }
        }
    }
    weight
}

/// Goal-distance-weighted variant: scales the base clause weight by its distance
/// from the goal in the symbol-reachability graph and derivation DAG.
pub fn clause_weight_goal_distance(
    clause: &IdClause,
    bank: &TermBank,
    config: &SymbolConfig,
    goal_map: &crate::goal_distance::GoalDistanceMap,
) -> u32 {
    if !goal_map.has_conjecture() {
        return clause_weight_id(clause, bank, config);
    }
    let base_w = clause_weight_conj_boost(clause, bank, config, goal_map);
    let dist = goal_map.clause_goal_distance(clause, bank);
    let mult = match dist {
        0 => 2, // 1.0x (conjecture)
        1 => 3, // 1.5x (direct neighbor axiom)
        2 => 4, // 2.0x (2-hop axiom)
        3 => 5, // 2.5x
        _ => 6, // 3.0x (distant / disconnected)
    };
    base_w.saturating_mul(mult) / 2
}

// ── Quadratic Function Depth Penalty ────────────────────────────────────────

pub fn clause_weight_penalty(clause: &IdClause, bank: &TermBank, config: &SymbolConfig) -> u32 {
    clause
        .literals
        .iter()
        .map(|lit| literal_weight_penalty(lit, bank, config))
        .sum()
}

fn literal_weight_penalty(lit: &IdLiteral, bank: &TermBank, config: &SymbolConfig) -> u32 {
    match &lit.atom {
        IdAtom::Pred(sym, args) => {
            config.symbol_weight(*sym)
                + args
                    .iter()
                    .map(|&a| term_weight_penalty(a, 1, bank, config))
                    .sum::<u32>()
        }
        IdAtom::Eq(l, r) => {
            term_weight_penalty(*l, 0, bank, config) + term_weight_penalty(*r, 0, bank, config)
        }
    }
}

fn term_weight_penalty(term: TermId, depth: u32, bank: &TermBank, config: &SymbolConfig) -> u32 {
    let mut stack: Vec<(TermId, u32)> = vec![(term, depth)];
    let mut weight: u32 = 0;
    while let Some((t, d)) = stack.pop() {
        match bank.get(t) {
            TermNode::Var(_) => {
                let factor = (d + 1).saturating_mul(d + 1);
                weight = weight.saturating_add(config.w0.saturating_mul(factor));
            }
            TermNode::App(sym, args) => {
                let factor = (d + 1).saturating_mul(d + 1);
                weight = weight.saturating_add(config.symbol_weight(*sym).saturating_mul(factor));
                for &a in args {
                    stack.push((a, d + 1));
                }
            }
        }
    }
    weight
}

// ── Exponential Function Depth Penalty ──────────────────────────────────────

pub fn clause_weight_penalty_exp(clause: &IdClause, bank: &TermBank, config: &SymbolConfig) -> u32 {
    clause
        .literals
        .iter()
        .map(|lit| literal_weight_penalty_exp(lit, bank, config))
        .sum()
}

fn literal_weight_penalty_exp(lit: &IdLiteral, bank: &TermBank, config: &SymbolConfig) -> u32 {
    match &lit.atom {
        IdAtom::Pred(sym, args) => {
            config.symbol_weight(*sym)
                + args
                    .iter()
                    .map(|&a| term_weight_penalty_exp(a, 1, bank, config))
                    .sum::<u32>()
        }
        IdAtom::Eq(l, r) => {
            term_weight_penalty_exp(*l, 0, bank, config)
                + term_weight_penalty_exp(*r, 0, bank, config)
        }
    }
}

fn term_weight_penalty_exp(
    term: TermId,
    depth: u32,
    bank: &TermBank,
    config: &SymbolConfig,
) -> u32 {
    let mut stack: Vec<(TermId, u32)> = vec![(term, depth)];
    let mut weight: u32 = 0;
    while let Some((t, d)) = stack.pop() {
        match bank.get(t) {
            TermNode::Var(_) => {
                let factor = 1u32 << d.min(30);
                weight = weight.saturating_add(config.w0.saturating_mul(factor));
            }
            TermNode::App(sym, args) => {
                let factor = 1u32 << d.min(30);
                weight = weight.saturating_add(config.symbol_weight(*sym).saturating_mul(factor));
                for &a in args {
                    stack.push((a, d + 1));
                }
            }
        }
    }
    weight
}

// ── Horn Progressive Heuristic ──────────────────────────────────────────────

/// Horn-heuristic variant: same as Standard but clauses with >1 positive literal
/// pay a progressive `pos_count` multiplier penalty.
pub fn clause_weight_horn_heuristic(
    clause: &IdClause,
    bank: &TermBank,
    config: &SymbolConfig,
) -> u32 {
    let base = clause_weight_id(clause, bank, config);
    let pos_count = clause.literals.iter().filter(|l| l.positive).count();
    if pos_count > 1 {
        base.saturating_mul(pos_count as u32)
    } else {
        base
    }
}

// ── Horn Exponential Heuristic ──────────────────────────────────────────────

/// Horn-heuristic exponential variant: same as Standard but clauses with >1 positive literal
/// pay an exponential `2^(pos_count - 1)` multiplier penalty.
pub fn clause_weight_horn_heuristic_exp(
    clause: &IdClause,
    bank: &TermBank,
    config: &SymbolConfig,
) -> u32 {
    let base = clause_weight_id(clause, bank, config);
    let pos_count = clause.literals.iter().filter(|l| l.positive).count();
    if pos_count > 1 {
        let shift = (pos_count - 1).min(30) as u32;
        base.saturating_mul(1u32 << shift)
    } else {
        base
    }
}

// ── Precedence-Based Symbol Weight ──────────────────────────────────────────

/// Weights each symbol by its KBO/LPO precedence rank.
/// Rare symbols have higher precedence → higher cost; common symbols have
/// lower precedence → lower cost.  Variables always cost `w0` (standard).
pub fn clause_weight_symbol(clause: &IdClause, bank: &TermBank, config: &SymbolConfig) -> u32 {
    clause
        .literals
        .iter()
        .map(|lit| literal_weight_symbol(lit, bank, config))
        .sum()
}

fn literal_weight_symbol(lit: &IdLiteral, bank: &TermBank, config: &SymbolConfig) -> u32 {
    match &lit.atom {
        IdAtom::Pred(sym, args) => {
            let sym_w = get_symbol_weight_precedence(*sym, config);
            sym_w
                + args
                    .iter()
                    .map(|&a| term_weight_symbol(a, bank, config))
                    .sum::<u32>()
        }
        IdAtom::Eq(l, r) => {
            term_weight_symbol(*l, bank, config) + term_weight_symbol(*r, bank, config)
        }
    }
}

fn term_weight_symbol(term: TermId, bank: &TermBank, config: &SymbolConfig) -> u32 {
    let mut stack: Vec<TermId> = vec![term];
    let mut weight: u32 = 0;
    while let Some(t) = stack.pop() {
        match bank.get(t) {
            TermNode::Var(_) => {
                weight = weight.saturating_add(config.w0);
            }
            TermNode::App(sym, args) => {
                let sym_w = get_symbol_weight_precedence(*sym, config);
                weight = weight.saturating_add(sym_w);
                stack.extend_from_slice(args);
            }
        }
    }
    weight
}

fn get_symbol_weight_precedence(sym: mrs_core::SymbolId, config: &SymbolConfig) -> u32 {
    if config.precedence.is_empty() {
        2
    } else {
        config.symbol_precedence(sym).max(1)
    }
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

    #[test]
    fn test_new_weight_heuristics() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let f = syms.intern("f");
        let a = syms.intern("a");
        let b = syms.intern("b");

        // Clause 1: p(f(a, b)) -> nested term
        let c1 = make_clause(vec![Literal::pos(Atom::pred(
            p,
            vec![Term::app(f, vec![Term::constant(a), Term::constant(b)])],
        ))]);

        let mut bank = TermBank::new();
        let id_c1 = bank.clause_from_legacy(&c1);
        let config = SymbolConfig::default();

        // 1. FunctionDepth:
        // Pred p has sym_weight(p) = 1.
        // arg: f(a, b) at depth 1.
        // f at depth 1 -> sym_weight(f) = 1 * (1 + 1) = 2
        // a at depth 2 -> sym_weight(a) = 1 * (2 + 1) = 3
        // b at depth 2 -> sym_weight(b) = 1 * (2 + 1) = 3
        // sum = 2 + 3 + 3 = 8.
        // Total Literal weight = 1 + 8 = 9.
        assert_eq!(clause_weight_depth(&id_c1, &bank, &config), 9);

        // 2. FunctionWeightPenalty (Quadratic):
        // Pred p: sym_weight(p) = 1.
        // arg: f(a, b) at depth 1.
        // f at depth 1 -> sym_weight(f) = 1 * (1 + 1)^2 = 4
        // a at depth 2 -> sym_weight(a) = 1 * (2 + 1)^2 = 9
        // b at depth 2 -> sym_weight(b) = 1 * (2 + 1)^2 = 9
        // sum = 4 + 9 + 9 = 22.
        // Total = 1 + 22 = 23.
        assert_eq!(clause_weight_penalty(&id_c1, &bank, &config), 23);

        // 3. FunctionWeightPenaltyExp (Exponential):
        // Pred p: sym_weight(p) = 1.
        // arg: f(a, b) at depth 1.
        // f at depth 1 -> sym_weight(f) = 1 * 2^1 = 2
        // a at depth 2 -> sym_weight(a) = 1 * 2^2 = 4
        // b at depth 2 -> sym_weight(b) = 1 * 2^2 = 4
        // sum = 2 + 4 + 4 = 10.
        // Total = 1 + 10 = 11.
        assert_eq!(clause_weight_penalty_exp(&id_c1, &bank, &config), 11);

        // Clause 2: non-Horn clause with 2 positive literals: p(a) | p(b)
        let c2 = make_clause(vec![
            Literal::pos(Atom::pred(p, vec![Term::constant(a)])),
            Literal::pos(Atom::pred(p, vec![Term::constant(b)])),
        ]);
        let id_c2 = bank.clause_from_legacy(&c2);
        // Base weight = 2 (for first p(a)) + 2 (for second p(b)) = 4.
        // pos_count = 2.
        // HornPenalty: base * 3 = 12.
        assert_eq!(clause_weight_horn(&id_c2, &bank, &config), 12);
        // HornHeuristic: base * pos_count = 8.
        assert_eq!(clause_weight_horn_heuristic(&id_c2, &bank, &config), 8);
        // HornHeuristicExp: base * 2^(pos_count - 1) = 4 * 2^1 = 8.
        assert_eq!(clause_weight_horn_heuristic_exp(&id_c2, &bank, &config), 8);

        // Clause 3: non-Horn clause with 3 positive literals: p(a) | p(b) | p(a)
        let c3 = make_clause(vec![
            Literal::pos(Atom::pred(p, vec![Term::constant(a)])),
            Literal::pos(Atom::pred(p, vec![Term::constant(b)])),
            Literal::pos(Atom::pred(p, vec![Term::constant(a)])),
        ]);
        let id_c3 = bank.clause_from_legacy(&c3);
        // Base weight = 2 + 2 + 2 = 6.
        // pos_count = 3.
        // HornHeuristic: base * 3 = 18.
        assert_eq!(clause_weight_horn_heuristic(&id_c3, &bank, &config), 18);
        // HornHeuristicExp: base * 2^(pos_count - 1) = 6 * 4 = 24.
        assert_eq!(clause_weight_horn_heuristic_exp(&id_c3, &bank, &config), 24);

        // 4. SymbolWeight (precedence-based):
        // Custom config with precedence values:
        // p has precedence 5 (common symbol — high precedence → high weight)
        // a has precedence 1 (rare symbol  — low precedence  → low weight)
        // b has precedence 2
        let mut custom_config = SymbolConfig {
            precedence: vec![0; 100],
            ..Default::default()
        };
        custom_config.precedence[p.index() as usize] = 5;
        custom_config.precedence[a.index() as usize] = 1;
        custom_config.precedence[b.index() as usize] = 2;

        // p(a) → weight(p) = 5, weight(a) = 1. Total = 6.
        let c4 = make_clause(vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))]);
        let id_c4 = bank.clause_from_legacy(&c4);
        assert_eq!(clause_weight_symbol(&id_c4, &bank, &custom_config), 6);
    }

    #[test]
    fn test_goal_distance_weight_fn() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let mut bank = TermBank::new();
        let config = SymbolConfig::default();

        // Conj: ~p(a)
        let conj = Clause {
            id: mrs_core::clause::ClauseId(1),
            literals: [Literal::neg(Atom::pred(p, vec![Term::constant(a)]))]
                .as_slice()
                .into(),
            source: mrs_core::clause::ClauseSource::Input {
                name: "c".into(),
                role: "conjecture".into(),
            },
            avatar: vec![],
            distance: 0,
            formula: None,
            certificate: None,
        };

        // Ax1: p(X) | ~q(X)  (shares p -> dist 1, q gets dist 1)
        let ax1 = Clause {
            id: mrs_core::clause::ClauseId(2),
            literals: [
                Literal::pos(Atom::pred(p, vec![Term::var(0)])),
                Literal::neg(Atom::pred(q, vec![Term::var(0)])),
            ]
            .as_slice()
            .into(),
            source: mrs_core::clause::ClauseSource::Input {
                name: "ax1".into(),
                role: "axiom".into(),
            },
            avatar: vec![],
            distance: 100,
            formula: None,
            certificate: None,
        };

        // Ax2: q(b)  (shares q -> dist 2, b gets dist 2)
        let ax2 = Clause {
            id: mrs_core::clause::ClauseId(3),
            literals: [Literal::pos(Atom::pred(q, vec![Term::constant(b)]))]
                .as_slice()
                .into(),
            source: mrs_core::clause::ClauseSource::Input {
                name: "ax2".into(),
                role: "axiom".into(),
            },
            avatar: vec![],
            distance: 100,
            formula: None,
            certificate: None,
        };

        let map = crate::goal_distance::GoalDistanceMap::compute(&[
            conj.clone(),
            ax1.clone(),
            ax2.clone(),
        ]);

        let id_conj = bank.clause_from_legacy(&conj);
        let id_ax1 = bank.clause_from_legacy(&ax1);
        let id_ax2 = bank.clause_from_legacy(&ax2);

        // In conj: p (dist 0 -> cost 1), a (dist 0 -> cost 1). Total conj_boost = 2.
        assert_eq!(clause_weight_conj_boost(&id_conj, &bank, &config, &map), 2);
        // Goal distance for conj = 0 -> mult = 2 -> 2 * 2 / 2 = 2.
        assert_eq!(
            clause_weight_goal_distance(&id_conj, &bank, &config, &map),
            2
        );

        // In ax1: p (dist 0 -> 1), X (var -> 1), q (dist 1 -> base 1), X (var -> 1). Total = 4.
        assert_eq!(clause_weight_conj_boost(&id_ax1, &bank, &config, &map), 4);
        // Goal dist for ax1 = 1 -> mult = 3 -> 4 * 3 / 2 = 6.
        assert_eq!(
            clause_weight_goal_distance(&id_ax1, &bank, &config, &map),
            6
        );

        // In ax2: q (dist 1 -> base 1), b (dist 2 -> 2*base = 2). Total = 3.
        assert_eq!(clause_weight_conj_boost(&id_ax2, &bank, &config, &map), 3);
        // Goal dist for ax2 = 2 -> mult = 4 -> 3 * 4 / 2 = 6.
        assert_eq!(
            clause_weight_goal_distance(&id_ax2, &bank, &config, &map),
            6
        );
    }
}
