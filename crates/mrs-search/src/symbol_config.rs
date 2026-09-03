//! Dynamic problem-specific symbol precedence and weight calculation.
//!
//! Superposition and resolution provers rely heavily on term orderings (KBO / LPO)
//! to orient equalities and restrict inference search spaces. Choosing the right symbol
//! precedence and weights can make orders-of-magnitude difference in solver performance.
//!
//! This module provides problem-specific heuristics:
//! - **Precedence schemes**: Inverse frequency, frequency, maximal arity, minimal arity,
//!   and goal-boosted precedence.
//! - **Symbol weight schemes**: Uniform, arity-based, inverse-frequency-based, and
//!   conjecture-bonus weighting.

use std::sync::Arc;

use mrs_calculus::ordering::SymbolConfig;
use mrs_core::clause::{Clause, ClauseSource};
use mrs_core::formula::Atom;
use mrs_core::symbol::SymbolId;
use mrs_core::term::Term;

use crate::HashMap;

/// Heuristic scheme for ordering symbols in KBO / LPO reduction orderings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PrecedenceScheme {
    /// Inverse frequency: rarest symbols in the problem get highest precedence.
    /// This causes rare symbols to be rewritten/eliminated first. (Default in E and Vampire).
    #[default]
    InvFreq,
    /// Frequency: most frequent symbols get highest precedence.
    Freq,
    /// Maximum arity: symbols with higher arity get highest precedence.
    ArityMax,
    /// Minimum arity: constants and low-arity symbols get highest precedence.
    ArityMin,
    /// Goal-boosted: symbols appearing in the negated conjecture get highest precedence.
    GoalBoost,
}

/// Heuristic scheme for assigning symbol weights in KBO reduction orderings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SymbolWeightScheme {
    /// Non-variable symbols have weight 2, variables have weight 1.
    #[default]
    Uniform,
    /// Symbols have weight proportional to their arity (arity + 1).
    Arity,
    /// Rarest symbols have higher weights (3), common symbols weigh 2.
    InvFreq,
    /// Symbols occurring in conjecture have lower weight (1 vs 2 for others).
    ConjectureBonus,
}

#[derive(Default, Debug, Clone)]
struct SymbolStats {
    freq: u32,
    max_arity: usize,
    in_goal: bool,
}

fn collect_term_symbols(term: &Term, in_goal: bool, stats: &mut HashMap<SymbolId, SymbolStats>) {
    let mut stack = vec![term];
    while let Some(t) = stack.pop() {
        if let Term::App(f, args) = t {
            let entry = stats.entry(*f).or_default();
            entry.freq += 1;
            entry.max_arity = entry.max_arity.max(args.len());
            if in_goal {
                entry.in_goal = true;
            }
            stack.extend(args.iter());
        }
    }
}

/// Computes a tailored `SymbolConfig` (precedence array and weights array) for a set
/// of problem clauses using the specified precedence and weight heuristics.
pub fn compute_symbol_config(
    clauses: &[Clause],
    precedence_scheme: PrecedenceScheme,
    weight_scheme: SymbolWeightScheme,
) -> Arc<SymbolConfig> {
    let mut stats: HashMap<SymbolId, SymbolStats> = HashMap::default();

    for clause in clauses {
        let in_goal = matches!(&clause.source, ClauseSource::Input { role, .. } if role == "conjecture" || role == "negated_conjecture")
            || clause.distance < 100;

        for lit in &clause.literals {
            match &lit.atom {
                Atom::Pred(p, args) => {
                    let entry = stats.entry(*p).or_default();
                    entry.freq += 1;
                    entry.max_arity = entry.max_arity.max(args.len());
                    if in_goal {
                        entry.in_goal = true;
                    }
                    for arg in args {
                        collect_term_symbols(arg, in_goal, &mut stats);
                    }
                }
                Atom::Eq(l, r) => {
                    collect_term_symbols(l, in_goal, &mut stats);
                    collect_term_symbols(r, in_goal, &mut stats);
                }
            }
        }
    }

    if stats.is_empty() {
        return Arc::new(SymbolConfig::default());
    }

    let max_sym = stats.keys().map(|s| s.index() as usize).max().unwrap_or(0);
    let mut syms: Vec<(SymbolId, &SymbolStats)> = stats.iter().map(|(&s, st)| (s, st)).collect();

    match precedence_scheme {
        PrecedenceScheme::InvFreq => {
            syms.sort_unstable_by(|a, b| {
                a.1.freq
                    .cmp(&b.1.freq)
                    .then_with(|| b.1.max_arity.cmp(&a.1.max_arity))
                    .then_with(|| a.0.cmp(&b.0))
            });
        }
        PrecedenceScheme::Freq => {
            syms.sort_unstable_by(|a, b| {
                b.1.freq
                    .cmp(&a.1.freq)
                    .then_with(|| b.1.max_arity.cmp(&a.1.max_arity))
                    .then_with(|| a.0.cmp(&b.0))
            });
        }
        PrecedenceScheme::ArityMax => {
            syms.sort_unstable_by(|a, b| {
                b.1.max_arity
                    .cmp(&a.1.max_arity)
                    .then_with(|| a.1.freq.cmp(&b.1.freq))
                    .then_with(|| a.0.cmp(&b.0))
            });
        }
        PrecedenceScheme::ArityMin => {
            syms.sort_unstable_by(|a, b| {
                a.1.max_arity
                    .cmp(&b.1.max_arity)
                    .then_with(|| a.1.freq.cmp(&b.1.freq))
                    .then_with(|| a.0.cmp(&b.0))
            });
        }
        PrecedenceScheme::GoalBoost => {
            syms.sort_unstable_by(|a, b| {
                b.1.in_goal
                    .cmp(&a.1.in_goal)
                    .then_with(|| a.1.freq.cmp(&b.1.freq))
                    .then_with(|| b.1.max_arity.cmp(&a.1.max_arity))
                    .then_with(|| a.0.cmp(&b.0))
            });
        }
    }

    let mut precedence = vec![0; max_sym + 1];
    for (rank, (sym, _)) in syms.iter().enumerate() {
        precedence[sym.index() as usize] = (syms.len() - rank) as u32;
    }

    let mut weights = vec![1; max_sym + 1];
    let median_freq = if syms.is_empty() {
        0
    } else {
        let mut freqs: Vec<u32> = syms.iter().map(|(_, st)| st.freq).collect();
        freqs.sort_unstable();
        freqs[freqs.len() / 2]
    };

    for (sym, st) in &syms {
        let idx = sym.index() as usize;
        weights[idx] = match weight_scheme {
            SymbolWeightScheme::Uniform => 2,
            SymbolWeightScheme::Arity => (st.max_arity as u32).saturating_add(1).max(1),
            SymbolWeightScheme::InvFreq => {
                if st.freq <= median_freq {
                    3
                } else {
                    2
                }
            }
            SymbolWeightScheme::ConjectureBonus => {
                if st.in_goal {
                    1
                } else {
                    2
                }
            }
        };
    }

    Arc::new(SymbolConfig {
        precedence,
        weights,
        w0: 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::clause::Literal;
    use mrs_core::symbol::SymbolTable;

    #[test]
    fn test_inv_freq_gives_rare_symbols_higher_precedence() {
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let a = syms.intern("a");
        let b = syms.intern("b");

        // Clause 1: f(a, a) = a (a occurs 3 times, f occurs 1 time)
        let mut cl1 = Clause::new(
            mrs_core::clause::ClauseId(1),
            vec![Literal::pos(Atom::Eq(
                Term::App(f, vec![Term::App(a, vec![]), Term::App(a, vec![])]),
                Term::App(a, vec![]),
            ))],
            ClauseSource::Input {
                name: "c1".into(),
                role: "axiom".into(),
            },
        );
        cl1.distance = 100;

        // Clause 2: b = a (b occurs 1 time)
        let mut cl2 = Clause::new(
            mrs_core::clause::ClauseId(2),
            vec![Literal::pos(Atom::Eq(
                Term::App(b, vec![]),
                Term::App(a, vec![]),
            ))],
            ClauseSource::Input {
                name: "c2".into(),
                role: "axiom".into(),
            },
        );
        cl2.distance = 100;

        let cfg_inv = compute_symbol_config(
            &[cl1.clone(), cl2.clone()],
            PrecedenceScheme::InvFreq,
            SymbolWeightScheme::Uniform,
        );
        let cfg_freq = compute_symbol_config(
            &[cl1, cl2],
            PrecedenceScheme::Freq,
            SymbolWeightScheme::Uniform,
        );

        // Under InvFreq, 'a' (frequent, count=4) must have LOWER precedence than 'f' (count=1)
        assert!(cfg_inv.symbol_precedence(a) < cfg_inv.symbol_precedence(f));
        // Under Freq, 'a' must have HIGHER precedence than 'f'
        assert!(cfg_freq.symbol_precedence(a) > cfg_freq.symbol_precedence(f));
    }

    #[test]
    fn test_arity_max_and_min() {
        let mut syms = SymbolTable::new();
        let f_bin = syms.intern("f");
        let c_const = syms.intern("c");

        let mut cl = Clause::new(
            mrs_core::clause::ClauseId(1),
            vec![Literal::pos(Atom::Eq(
                Term::App(
                    f_bin,
                    vec![Term::App(c_const, vec![]), Term::App(c_const, vec![])],
                ),
                Term::App(c_const, vec![]),
            ))],
            ClauseSource::Input {
                name: "c1".into(),
                role: "axiom".into(),
            },
        );
        cl.distance = 100;

        let cfg_max = compute_symbol_config(
            &[cl.clone()],
            PrecedenceScheme::ArityMax,
            SymbolWeightScheme::Arity,
        );
        let cfg_min =
            compute_symbol_config(&[cl], PrecedenceScheme::ArityMin, SymbolWeightScheme::Arity);

        assert!(cfg_max.symbol_precedence(f_bin) > cfg_max.symbol_precedence(c_const));
        assert!(cfg_min.symbol_precedence(f_bin) < cfg_min.symbol_precedence(c_const));

        // Arity weights: f_bin has arity 2 -> weight 3; c_const has arity 0 -> weight 1
        assert_eq!(cfg_max.symbol_weight(f_bin), 3);
        assert_eq!(cfg_max.symbol_weight(c_const), 1);
    }

    #[test]
    fn test_goal_boost_prioritizes_conjecture_symbols() {
        let mut syms = SymbolTable::new();
        let axiom_sym = syms.intern("ax");
        let goal_sym = syms.intern("goal");

        let mut cl_axiom = Clause::new(
            mrs_core::clause::ClauseId(1),
            vec![Literal::pos(Atom::Pred(axiom_sym, vec![]))],
            ClauseSource::Input {
                name: "ax".into(),
                role: "axiom".into(),
            },
        );
        cl_axiom.distance = 100;

        let mut cl_goal = Clause::new(
            mrs_core::clause::ClauseId(2),
            vec![Literal::pos(Atom::Pred(goal_sym, vec![]))],
            ClauseSource::Input {
                name: "conj".into(),
                role: "negated_conjecture".into(),
            },
        );
        cl_goal.distance = 0;

        let cfg_goal = compute_symbol_config(
            &[cl_axiom, cl_goal],
            PrecedenceScheme::GoalBoost,
            SymbolWeightScheme::ConjectureBonus,
        );

        assert!(cfg_goal.symbol_precedence(goal_sym) > cfg_goal.symbol_precedence(axiom_sym));
        assert_eq!(cfg_goal.symbol_weight(goal_sym), 1);
        assert_eq!(cfg_goal.symbol_weight(axiom_sym), 2);
    }
}
