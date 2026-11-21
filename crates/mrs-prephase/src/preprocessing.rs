//! Preprocessing for clausal theorem proving: Blocked Clause Elimination (BCE),
//! Pure Literal Elimination (PLE), and Tautology Elimination.
//!
//! # Soundness and Completeness
//!
//! - **Tautology Elimination**: A clause containing equality reflexivity `s = s`
//!   or complementary literals `L` and `¬L` is universally valid and can never
//!   contribute to a minimal unsatisfiable refutation.
//!
//! - **Pure Literal Elimination (PLE)**: A predicate symbol `P` that appears only
//!   with positive polarity (or only with negative polarity) across all active
//!   clauses can never be resolved away by any binary resolution or superposition
//!   step. Any axiom clause containing `P` will always pass `P` to all its
//!   descendants, and thus can never derive the empty clause $\Box$. Eliminating
//!   such axiom clauses preserves unsatisfiability.
//!
//! - **Blocked Clause Elimination (BCE)**: In first-order logic (Kiesl et al.,
//!   LPAR 2017), blocking requires every **L-resolvent**, including resolvents
//!   against jointly unifiable sets of opposite literals, to be valid. Binary
//!   resolvents alone are insufficient (the paper's Example 4). This bounded
//!   implementation enumerates all nonempty partner subsets; exhausted checks
//!   leave the clause untouched. Eliminating genuinely blocked clauses preserves
//!   satisfiability:
//!   $F \setminus \{C\}$ is SAT $\iff F$ is SAT.
//!   That guarantee covers resolution only: with equality present,
//!   paramodulation can use a "blocked" literal, so BCE is skipped entirely
//!   whenever any clause contains an equality literal.
//!
//! - **Conjecture Protection**: Conjectures and negated conjectures
//!   (`clause.distance == 0` or input role `conjecture` / `negated_conjecture`)
//!   are strictly protected and never eliminated, ensuring refutation search
//!   remains goal-directed and CASC proof certificates remain intact.

use std::time::{Duration, Instant};

use mrs_calculus::rename::{max_var, rename_clause};
use mrs_core::clause::{Clause, ClauseSource, Literal};
use mrs_core::formula::Atom;
use mrs_core::symbol::SymbolId;
use mrs_core::term::Term;
use rustc_hash::FxHashMap;

mod bce;

/// Configuration options for clause preprocessing.
#[derive(Clone, Debug)]
pub struct PreprocessingConfig {
    /// Whether to eliminate tautological clauses (positive `s = s` or `L ∨ ¬L`).
    pub enable_tautology: bool,
    /// Whether to perform Pure Literal Elimination (PLE).
    pub enable_ple: bool,
    /// Whether to perform First-Order Blocked Clause Elimination (BCE).
    pub enable_bce: bool,
    /// Maximum number of resolution partners to check for a single candidate literal.
    /// Clauses with high-degree partner lists are skipped to preserve sub-millisecond speed.
    pub max_bce_partners: usize,
    /// Maximum number of interleaved PLE + BCE rounds before stopping.
    pub max_rounds: usize,
    /// Time budget in milliseconds for the entire preprocessing phase.
    pub time_limit_ms: u64,
}

impl Default for PreprocessingConfig {
    fn default() -> Self {
        Self {
            enable_tautology: true,
            enable_ple: true,
            enable_bce: true,
            max_bce_partners: 100,
            max_rounds: 5,
            time_limit_ms: 100,
        }
    }
}

/// Statistics reported after clause preprocessing.
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct PreprocessingStats {
    /// Number of tautological clauses removed.
    pub tautologies_removed: usize,
    /// Number of clauses removed by Pure Literal Elimination.
    pub pure_clauses_removed: usize,
    /// Number of clauses removed by Blocked Clause Elimination.
    pub blocked_clauses_removed: usize,
    /// Total number of clauses removed.
    pub total_removed: usize,
}

/// Determines whether a clause is protected from elimination.
///
/// Protected clauses include:
/// 1. The empty clause $\Box$ (already a refutation).
/// 2. Conjectures and negated conjectures (`distance == 0` or role `conjecture`/`negated_conjecture`).
pub fn is_protected(clause: &Clause) -> bool {
    if clause.literals.is_empty() {
        return true;
    }
    if clause.distance == 0 {
        return true;
    }
    match &clause.source {
        ClauseSource::Input { role, .. } => role == "conjecture" || role == "negated_conjecture",
        _ => false,
    }
}

/// Converts a predicate atom to a term for Robinson unification.
fn atom_to_term(atom: &Atom) -> Option<Term> {
    match atom {
        Atom::Pred(p, args) => Some(Term::app(*p, args.clone())),
        Atom::Eq(_, _) => None,
    }
}

/// Returns `true` if a literal is an equality reflexivity tautology (`s = s`).
fn literal_is_tautology(lit: &Literal) -> bool {
    if lit.positive
        && let Atom::Eq(l, r) = &lit.atom
    {
        l == r
    } else {
        false
    }
}

/// Returns `true` if two literals are complementary (opposite polarities and identical atoms,
/// taking equality symmetry into account).
fn literals_are_complementary(l1: &Literal, l2: &Literal) -> bool {
    if l1.positive == l2.positive {
        return false;
    }
    match (&l1.atom, &l2.atom) {
        (Atom::Pred(p1, args1), Atom::Pred(p2, args2)) => p1 == p2 && args1 == args2,
        (Atom::Eq(l1, r1), Atom::Eq(l2, r2)) => (l1 == l2 && r1 == r2) || (l1 == r2 && r1 == l2),
        _ => false,
    }
}

/// Checks whether a resolvent literal list is a tautology.
///
/// A resolvent is a tautology if:
/// 1. It contains a positive equality between identical terms `s = s`.
/// 2. It contains two complementary literals `K` and `¬K`.
fn is_resolvent_tautology(lits: &[Literal]) -> bool {
    for lit in lits {
        if literal_is_tautology(lit) {
            return true;
        }
    }
    for (i, lit1) in lits.iter().enumerate() {
        for lit2 in &lits[i + 1..] {
            if literals_are_complementary(lit1, lit2) {
                return true;
            }
        }
    }
    false
}

/// Executes a single pass of Pure Literal Elimination (PLE).
///
/// Counts positive and negative occurrences of non-equality predicates across all active clauses.
/// Any non-protected axiom clause containing a predicate with only positive (or only negative)
/// occurrences across the whole problem is marked dead.
fn run_ple_pass(clauses: &[Clause], alive: &mut [bool]) -> usize {
    let mut pos_counts: FxHashMap<SymbolId, usize> = FxHashMap::default();
    let mut neg_counts: FxHashMap<SymbolId, usize> = FxHashMap::default();

    // 1. Count occurrences across all alive clauses (both axioms and conjectures)
    for (i, clause) in clauses.iter().enumerate() {
        if !alive[i] {
            continue;
        }
        for lit in &clause.literals {
            if let Atom::Pred(sym, _) = &lit.atom {
                if lit.positive {
                    *pos_counts.entry(*sym).or_insert(0) += 1;
                } else {
                    *neg_counts.entry(*sym).or_insert(0) += 1;
                }
            }
        }
    }

    // 2. Identify pure predicate symbols
    let mut pure_symbols: FxHashMap<SymbolId, bool> = FxHashMap::default();
    for &sym in pos_counts.keys() {
        let neg = neg_counts.get(&sym).copied().unwrap_or(0);
        if neg == 0 {
            pure_symbols.insert(sym, true); // pure positive
        }
    }
    for &sym in neg_counts.keys() {
        let pos = pos_counts.get(&sym).copied().unwrap_or(0);
        if pos == 0 {
            pure_symbols.insert(sym, false); // pure negative
        }
    }

    if pure_symbols.is_empty() {
        return 0;
    }

    // 3. Mark non-protected axiom clauses containing pure symbols as dead
    let mut removed = 0;
    for (i, clause) in clauses.iter().enumerate() {
        if !alive[i] || is_protected(clause) {
            continue;
        }
        let has_pure = clause.literals.iter().any(|lit| {
            if let Atom::Pred(sym, _) = &lit.atom {
                pure_symbols.contains_key(sym)
            } else {
                false
            }
        });
        if has_pure {
            alive[i] = false;
            removed += 1;
        }
    }

    removed
}

/// Executes a single pass of First-Order Blocked Clause Elimination (BCE).
///
/// Check the paper's L-resolvents, not only binary resolvents. Caps can only
/// miss a deletion: no incomplete check is treated as a blocking certificate.
fn run_bce_pass(
    clauses: &[Clause],
    alive: &mut [bool],
    config: &PreprocessingConfig,
    deadline: Instant,
) -> usize {
    // 1. Index all currently alive literals by (SymbolId, polarity)
    let mut lit_index: FxHashMap<(SymbolId, bool), Vec<(usize, usize)>> = FxHashMap::default();
    for (i, clause) in clauses.iter().enumerate() {
        if !alive[i] {
            continue;
        }
        for (lit_idx, lit) in clause.literals.iter().enumerate() {
            if let Atom::Pred(sym, _) = &lit.atom {
                lit_index
                    .entry((*sym, lit.positive))
                    .or_default()
                    .push((i, lit_idx));
            }
        }
    }

    let mut removed = 0;

    // 2. Check each non-protected clause for a blocking literal
    for i in 0..clauses.len() {
        if Instant::now() >= deadline {
            break;
        }
        if !alive[i] || is_protected(&clauses[i]) {
            continue;
        }
        let c = &clauses[i];

        let mut clause_is_blocked = false;
        let mut remaining = 4096;

        for (lit_idx, lit) in c.literals.iter().enumerate() {
            let Atom::Pred(sym, args) = &lit.atom else {
                continue;
            };
            let pol = lit.positive;
            let opp_key = (*sym, !pol);

            let Some(partners) = lit_index.get(&opp_key) else {
                // Zero opposite partners exist in the active clause set.
                // The literal is vacuously blocked!
                clause_is_blocked = true;
                break;
            };

            // Collect active (still alive) partners
            let active_partners: Vec<(usize, usize)> = partners
                .iter()
                .copied()
                .filter(|&(p_idx, _)| alive[p_idx] && p_idx != i)
                .collect();

            if active_partners.is_empty() {
                clause_is_blocked = true;
                break;
            }

            if active_partners.len() > config.max_bce_partners {
                // Skip checking this literal if partner list is too large
                continue;
            }

            let mut literal_blocks = true;
            let offset = max_var(c);
            let t1 = Term::app(*sym, args.clone());

            let partner_clauses = active_partners
                .iter()
                .map(|&(p_idx, _)| p_idx)
                .collect::<std::collections::BTreeSet<_>>();
            for p_idx in partner_clauses {
                let d = &clauses[p_idx];
                let d_renamed = rename_clause(d, offset);
                if bce::all_l_resolvents_valid(
                    c,
                    lit_idx,
                    &t1,
                    &d_renamed,
                    deadline,
                    &mut remaining,
                ) != Some(true)
                {
                    literal_blocks = false;
                    break;
                }
            }

            if literal_blocks {
                clause_is_blocked = true;
                break;
            }
        }

        if clause_is_blocked && Instant::now() < deadline {
            alive[i] = false;
            removed += 1;
        }
    }

    removed
}

/// Preprocesses a slice of clauses using Tautology Elimination, Pure Literal Elimination (PLE),
/// and First-Order Blocked Clause Elimination (BCE).
///
/// Returns the remaining clauses and preprocessing statistics.
pub fn preprocess_clauses(
    clauses: &[Clause],
    config: &PreprocessingConfig,
) -> (Vec<Clause>, PreprocessingStats) {
    let mut stats = PreprocessingStats::default();
    let mut alive = vec![true; clauses.len()];

    // Phase 1: Tautology Elimination
    if config.enable_tautology {
        for (i, c) in clauses.iter().enumerate() {
            if !is_protected(c) && c.is_tautology() {
                alive[i] = false;
                stats.tautologies_removed += 1;
            }
        }
    }

    // Phase 2: Interleaved PLE and BCE until fixpoint or resource limits
    //
    // BCE is disabled whenever any clause contains an equality literal. The
    // blocked-literal check only considers binary resolution partners, but an
    // equality clause elsewhere can enable paramodulation/superposition
    // through the "blocked" literal. Deleting it then turns UNSAT into SAT:
    // in {a = b, p(a), ~p(b)}, `p(a)` has no unifiable `~p` partner (a and b
    // do not unify) so it looks blocked, yet removing it leaves the
    // satisfiable {a = b, ~p(b)}. (PLE and tautology elimination remain
    // sound with equality: a pure predicate persists in every descendant,
    // and valid clauses can never contribute to a refutation.)
    let has_equality = clauses
        .iter()
        .any(|c| c.literals.iter().any(|l| matches!(l.atom, Atom::Eq(..))));
    let enable_bce = config.enable_bce && !has_equality;
    let start_time = Instant::now();
    let time_limit = Duration::from_millis(config.time_limit_ms);

    for _ in 0..config.max_rounds {
        if start_time.elapsed() >= time_limit {
            break;
        }
        let mut progress = false;

        if config.enable_ple {
            let ple_removed = run_ple_pass(clauses, &mut alive);
            if ple_removed > 0 {
                stats.pure_clauses_removed += ple_removed;
                progress = true;
            }
        }

        if start_time.elapsed() >= time_limit {
            break;
        }

        if enable_bce {
            let deadline = start_time.checked_add(time_limit).unwrap_or(start_time);
            let bce_removed = run_bce_pass(clauses, &mut alive, config, deadline);
            if bce_removed > 0 {
                stats.blocked_clauses_removed += bce_removed;
                progress = true;
            }
        }

        if !progress {
            break;
        }
    }

    stats.total_removed =
        stats.tautologies_removed + stats.pure_clauses_removed + stats.blocked_clauses_removed;

    let remaining: Vec<Clause> = clauses
        .iter()
        .enumerate()
        .filter(|(i, _)| alive[*i])
        .map(|(_, c)| c.clone())
        .collect();

    (remaining, stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::clause::{ClauseId, ClauseSource};
    use mrs_core::symbol::SymbolTable;

    #[test]
    fn bce_retains_the_papers_nonbinary_resolvent_counterexample() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let atom = |x, y| Atom::pred(p, vec![Term::var(x), Term::var(y)]);
        let clauses = vec![
            make_clause(
                0,
                vec![Literal::pos(atom(0, 1)), Literal::pos(atom(1, 0))],
                1,
            ),
            make_clause(
                1,
                vec![Literal::neg(atom(0, 1)), Literal::neg(atom(1, 0))],
                1,
            ),
        ];
        let (remaining, stats) = preprocess_clauses(&clauses, &PreprocessingConfig::default());
        assert_eq!(remaining.len(), 2);
        assert_eq!(stats.total_removed, 0);
    }

    #[test]
    fn bce_preserves_small_first_order_models_against_an_exhaustive_oracle() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let atom = |x, y| Atom::pred(p, vec![Term::var(x), Term::var(y)]);
        let shapes = [
            vec![Literal::pos(atom(0, 1))],
            vec![Literal::neg(atom(0, 1))],
            vec![Literal::pos(atom(0, 1)), Literal::pos(atom(1, 0))],
            vec![Literal::neg(atom(0, 1)), Literal::neg(atom(1, 0))],
            vec![Literal::pos(atom(0, 1)), Literal::neg(atom(1, 0))],
            vec![Literal::pos(atom(0, 0)), Literal::pos(atom(1, 1))],
            vec![Literal::neg(atom(0, 0)), Literal::neg(atom(1, 1))],
        ];
        // Independent two-element model enumeration, with four possible p atoms
        // and universally quantified variables (not a resolution oracle).
        let has_model = |clauses: &[Clause]| {
            (0u32..16).any(|model| {
                clauses.iter().all(|clause| {
                    (0..4).all(|assignment| {
                        clause.literals.iter().any(|literal| {
                            let Atom::Pred(_, args) = &literal.atom else {
                                panic!("predicate fixture");
                            };
                            let value = |term: &Term| match term {
                                Term::Var(0) => assignment / 2,
                                Term::Var(1) => assignment % 2,
                                _ => panic!("two-variable fixture"),
                            };
                            let bit = value(&args[0]) * 2 + value(&args[1]);
                            literal.positive == (model & (1 << bit) != 0)
                        })
                    })
                })
            })
        };
        for left in &shapes {
            for right in &shapes {
                let clauses = vec![
                    make_clause(0, left.clone(), 1),
                    make_clause(1, right.clone(), 1),
                ];
                let config = PreprocessingConfig {
                    enable_ple: false,
                    ..Default::default()
                };
                let (reduced, _) = preprocess_clauses(&clauses, &config);
                assert_eq!(
                    has_model(&clauses),
                    has_model(&reduced),
                    "clauses={clauses:?}"
                );
            }
        }
    }

    #[test]
    fn an_exhausted_l_resolvent_check_cannot_delete_a_clause() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let literal = Literal::pos(Atom::pred(p, vec![Term::var(0)]));
        let c = make_clause(0, vec![literal.clone()], 1);
        let d = make_clause(1, vec![Literal::neg(literal.atom.clone()); 13], 1);
        let term = atom_to_term(&literal.atom).unwrap();
        let deadline = Instant::now() + Duration::from_secs(1);
        assert_eq!(
            bce::all_l_resolvents_valid(&c, 0, &term, &d, deadline, &mut 4096),
            None
        );
        assert_eq!(
            bce::all_l_resolvents_valid(&c, 0, &term, &d, Instant::now(), &mut 0),
            None
        );
    }

    #[test]
    fn bce_refuses_function_expansion_shapes_before_legacy_fallbacks() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let f = symbols.intern("f");
        let mut term = Term::var(0);
        for _ in 0..32 {
            term = Term::app(f, vec![term]);
        }
        let literal = Literal::pos(Atom::pred(p, vec![term]));
        let clauses = vec![
            make_clause(0, vec![literal.clone()], 1),
            make_clause(1, vec![Literal::neg(literal.atom)], 1),
        ];
        let (retained, stats) = preprocess_clauses(&clauses, &Default::default());
        assert_eq!(retained.len(), 2);
        assert_eq!(stats.total_removed, 0);
    }

    fn make_clause(id: u64, lits: Vec<Literal>, distance: u32) -> Clause {
        Clause::new(
            ClauseId(id),
            lits,
            ClauseSource::Input {
                name: format!("c{}", id),
                role: if distance == 0 {
                    "conjecture".to_string()
                } else {
                    "axiom".to_string()
                },
            },
        )
        .with_distance(distance)
    }

    #[test]
    fn test_tautology_elimination() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");

        let c1 = make_clause(
            1,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::constant(a)])),
                Literal::neg(Atom::pred(p, vec![Term::constant(a)])),
            ],
            100,
        );
        let c2 = make_clause(
            2,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
            100,
        );

        let config = PreprocessingConfig {
            enable_ple: false,
            enable_bce: false,
            ..Default::default()
        };
        let (res, stats) = preprocess_clauses(&[c1, c2], &config);
        assert_eq!(stats.tautologies_removed, 1);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, ClauseId(2));
    }

    #[test]
    fn test_pure_literal_elimination_cascading() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let dead = syms.intern("dead");
        let unused = syms.intern("unused");
        let x = Term::var(0);

        // c1: p(X) | q(X)
        let c1 = make_clause(
            1,
            vec![
                Literal::pos(Atom::pred(p, vec![x.clone()])),
                Literal::pos(Atom::pred(q, vec![x.clone()])),
            ],
            100,
        );
        // c2: ~p(X)
        let c2 = make_clause(2, vec![Literal::neg(Atom::pred(p, vec![x.clone()]))], 100);
        // c3: dead(X) | unused(X)  -- `dead` is pure positive!
        let c3 = make_clause(
            3,
            vec![
                Literal::pos(Atom::pred(dead, vec![x.clone()])),
                Literal::pos(Atom::pred(unused, vec![x.clone()])),
            ],
            100,
        );
        // c4: ~unused(X) | q(X)    -- after c3 is eliminated, `unused` becomes pure negative!
        let c4 = make_clause(
            4,
            vec![
                Literal::neg(Atom::pred(unused, vec![x.clone()])),
                Literal::pos(Atom::pred(q, vec![x.clone()])),
            ],
            100,
        );
        // c_conj: ~q(X) (conjecture, distance = 0)
        let c_conj = make_clause(5, vec![Literal::neg(Atom::pred(q, vec![x]))], 0);

        // Run with PLE only to verify pure literal cascading across multiple rounds
        let config = PreprocessingConfig {
            enable_bce: false,
            ..Default::default()
        };
        let (res, stats) = preprocess_clauses(&[c1, c2, c3, c4, c_conj], &config);

        // c3 is eliminated in round 1 because `dead` is pure positive.
        // c4 is eliminated in round 2 because `unused` became pure negative after c3 was removed!
        assert_eq!(stats.pure_clauses_removed, 2);
        assert_eq!(res.len(), 3);
        let ids: Vec<u64> = res.iter().map(|c| c.id.0).collect();
        assert_eq!(ids, vec![1, 2, 5]);
    }

    #[test]
    fn test_conjecture_protection() {
        let mut syms = SymbolTable::new();
        let pure_pred = syms.intern("pure_in_conjecture");
        let x = Term::var(0);

        // Conjecture containing a pure literal must NEVER be deleted!
        let c_conj = make_clause(
            1,
            vec![Literal::neg(Atom::pred(pure_pred, vec![x]))],
            0, // distance = 0
        );

        let (res, stats) = preprocess_clauses(&[c_conj], &PreprocessingConfig::default());
        assert_eq!(stats.pure_clauses_removed, 0);
        assert_eq!(stats.blocked_clauses_removed, 0);
        assert_eq!(res.len(), 1);
    }

    #[test]
    fn test_blocked_clause_elimination_tautological_resolvent() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let x = Term::var(0);
        let y = Term::var(0);

        // c1: p(X) | ~q(X)
        // c2: ~p(Y) | q(Y)
        // Resolvent on p: ~q(Y) | q(Y) is a tautology!
        // So c1 is blocked on p!
        let c1 = make_clause(
            1,
            vec![
                Literal::pos(Atom::pred(p, vec![x.clone()])),
                Literal::neg(Atom::pred(q, vec![x])),
            ],
            100,
        );
        let c2 = make_clause(
            2,
            vec![
                Literal::neg(Atom::pred(p, vec![y.clone()])),
                Literal::pos(Atom::pred(q, vec![y])),
            ],
            100,
        );

        let (res, stats) = preprocess_clauses(&[c1, c2], &PreprocessingConfig::default());
        // One clause is eliminated by BCE, which then makes the other pure and eliminated by PLE!
        assert_eq!(res.len(), 0);
        assert_eq!(stats.total_removed, 2);
    }

    #[test]
    fn test_non_blocked_clause_retained() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let r = syms.intern("r");
        let x = Term::var(0);

        // c1: p(X) | q(X)
        // c2: ~p(X) | r(X)
        // Resolvent on p: q(X) | r(X) is NOT a tautology!
        // Neither c1 nor c2 should be eliminated as blocked!
        let c1 = make_clause(
            1,
            vec![
                Literal::pos(Atom::pred(p, vec![x.clone()])),
                Literal::pos(Atom::pred(q, vec![x.clone()])),
            ],
            100,
        );
        let c2 = make_clause(
            2,
            vec![
                Literal::neg(Atom::pred(p, vec![x.clone()])),
                Literal::pos(Atom::pred(r, vec![x.clone()])),
            ],
            100,
        );
        // c3: ~q(X)
        let c3 = make_clause(3, vec![Literal::neg(Atom::pred(q, vec![x.clone()]))], 100);
        // c4: ~r(X)
        let c4 = make_clause(4, vec![Literal::neg(Atom::pred(r, vec![x]))], 0);

        let (res, stats) = preprocess_clauses(&[c1, c2, c3, c4], &PreprocessingConfig::default());
        assert_eq!(stats.blocked_clauses_removed, 0);
        assert_eq!(res.len(), 4);
    }

    #[test]
    fn test_bce_skipped_with_equality() {
        // BCE only checks binary resolution partners, so with equality
        // present it can delete a clause needed for a paramodulation step.
        // Regression case: {a = b, p(a), ~p(b)} is UNSAT, but `p(a)` looks
        // blocked (a and b do not unify) and deleting it leaves SAT.
        // BCE must not run at all when any clause contains equality.
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let b = syms.intern("b");

        // c1: a = b
        let c1 = make_clause(
            1,
            vec![Literal::pos(Atom::eq(Term::constant(a), Term::constant(b)))],
            100,
        );
        // c2: p(a)
        let c2 = make_clause(
            2,
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
            100,
        );
        // c3: ~p(b) (negated conjecture, protected)
        let c3 = make_clause(
            3,
            vec![Literal::neg(Atom::pred(p, vec![Term::constant(b)]))],
            0,
        );

        let (res, stats) = preprocess_clauses(&[c1, c2, c3], &PreprocessingConfig::default());
        assert_eq!(stats.blocked_clauses_removed, 0);
        assert_eq!(res.len(), 3);
    }

    #[test]
    fn test_equality_clauses_never_blocked_eliminated() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let a = syms.intern("a");
        let b = syms.intern("b");
        let c = syms.intern("c");
        let x = Term::var(0);

        // c1: p(X) | X = a
        // c2: ~p(a) | b = c
        // The only binary resolvent on p is the tautology a = a | b = c, so
        // resolution-only BCE would call c1 blocked. But superposition from
        // b = c into X = a yields non-tautological conclusions, so neither
        // clause may be eliminated while equality is present.
        let c1 = make_clause(
            1,
            vec![
                Literal::pos(Atom::pred(p, vec![x])),
                Literal::pos(Atom::eq(Term::var(0), Term::constant(a))),
            ],
            100,
        );
        let c2 = make_clause(
            2,
            vec![
                Literal::neg(Atom::pred(p, vec![Term::constant(a)])),
                Literal::pos(Atom::eq(Term::constant(b), Term::constant(c))),
            ],
            100,
        );

        let (res, stats) = preprocess_clauses(&[c1, c2], &PreprocessingConfig::default());
        // BCE is skipped with equality: both clauses are retained.
        assert_eq!(stats.blocked_clauses_removed, 0);
        assert_eq!(res.len(), 2);
        assert_eq!(stats.total_removed, 0);
    }
}
