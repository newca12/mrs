//! FVO (FNE-Variable-Only) propositional skeleton refutation.
//!
//! For clause sets where every predicate argument is a variable (and there is
//! no equality), replacing each predicate symbol with a propositional variable
//! is an exact satisfiability abstraction, provided each predicate has one
//! consistent arity. An FOL model induces a propositional valuation by
//! evaluating each predicate on a diagonal tuple `(d, ..., d)` for any domain
//! element `d`; conversely, a satisfying propositional valuation extends to a
//! model by interpreting each predicate uniformly on all tuples.
//!
//! **Soundness**: Propositional UNSAT implies FOL UNSAT by the diagonal
//! grounding argument above. Proof steps are not lifted by independently
//! inventing variables: each is replayed with ordinary first-order resolution,
//! which standardizes parents apart and preserves variable sharing. If a
//! resulting clause does not have the expected propositional image, the
//! specialized prover gives up.
//!
//! The algorithm:
//! 1. Detect FVO (no equality, all predicate args are variables, consistent
//!    predicate arities).
//! 2. Use `mrs-cadical` as a fast oracle to check propositional UNSAT.
//! 3. If UNSAT, run a BFS resolution prover to produce a step-by-step proof.
//! 4. Replay each propositional resolution step using first-order resolution.
//! 5. Return `SearchResult::Refutation` with the TSTP-formatted proof.

use crate::{HashMap, HashSet};

use mrs_calculus::resolution::resolve_selected;
use mrs_core::SymbolTable;
use mrs_core::clause::{Clause, ClauseIdGen, Literal};
use mrs_core::formula::Atom;
use mrs_core::symbol::SymbolId;
use mrs_core::term::Term;
use mrs_proof::tstp::format_tstp;

use crate::SearchResult;
use mrs_cadical::{SolveResult, Solver};

// ---------------------------------------------------------------------------
// FVO detection
// ---------------------------------------------------------------------------

/// Returns `true` if every predicate argument in the clause is a variable.
/// Clauses containing equality atoms (`=`) are rejected.
pub fn is_fvo_clause(clause: &Clause) -> bool {
    clause.literals.iter().all(|lit| match &lit.atom {
        Atom::Pred(_, args) => args.iter().all(|term| matches!(term, Term::Var(_))),
        Atom::Eq(_, _) => false,
    })
}

/// Returns `true` if every clause in the problem is FVO and the slice is
/// non-empty.
pub fn is_fvo_problem(clauses: &[Clause]) -> bool {
    if clauses.is_empty() {
        return false;
    }
    let mut arities: HashMap<u32, usize> = HashMap::default();
    clauses.iter().all(|clause| {
        is_fvo_clause(clause)
            && clause.literals.iter().all(|lit| match &lit.atom {
                Atom::Pred(sym, args) => match arities.get(&sym.index()) {
                    Some(arity) => *arity == args.len(),
                    None => {
                        arities.insert(sym.index(), args.len());
                        true
                    }
                },
                Atom::Eq(_, _) => false,
            })
    })
}

// ---------------------------------------------------------------------------
// Propositional abstraction
// ---------------------------------------------------------------------------

/// Signed propositional literal (DIMACS convention, 1-indexed).
/// `k > 0`: predicate-k is true; `k < 0`: predicate-k is false.
type PL = i32;

/// A propositional clause: sorted, deduplicated literals.
type PC = Vec<PL>;

/// Propositional abstraction of an FVO clause set.
struct PropAbstraction {
    /// `prop_clauses[i]` is the propositional image of input clause `i`.
    prop_clauses: Vec<PC>,
    /// `var_to_sym_arity[v-1] = (SymbolId, arity)` for prop variable `v`.
    var_to_sym_arity: Vec<(SymbolId, usize)>,
    /// `sym_to_var[symbol.index()] = prop variable` (1-indexed).
    sym_to_var: HashMap<u32, u32>,
}

impl PropAbstraction {
    fn build(clauses: &[Clause]) -> Self {
        let mut sym_to_var: HashMap<u32, u32> = HashMap::default();
        let mut var_to_sym_arity: Vec<(SymbolId, usize)> = Vec::new();

        let prop_clauses: Vec<PC> = clauses
            .iter()
            .map(|clause| {
                let mut lits: Vec<PL> = clause
                    .literals
                    .iter()
                    .filter_map(|lit| {
                        if let Atom::Pred(sym, args) = &lit.atom {
                            let var = *sym_to_var.entry(sym.index()).or_insert_with(|| {
                                let v = var_to_sym_arity.len() as u32 + 1; // 1-indexed
                                var_to_sym_arity.push((*sym, args.len()));
                                v
                            });
                            Some(if lit.positive {
                                var as PL
                            } else {
                                -(var as PL)
                            })
                        } else {
                            None
                        }
                    })
                    .collect::<HashSet<PL>>()
                    .into_iter()
                    .collect();
                lits.sort();
                lits
            })
            .collect();

        Self {
            prop_clauses,
            var_to_sym_arity,
            sym_to_var,
        }
    }

    /// Maps a first-order FVO clause back to its propositional skeleton.
    /// Returns `None` if resolution left the admitted fragment.
    fn abstract_clause(&self, clause: &Clause) -> Option<PC> {
        let mut literals = Vec::with_capacity(clause.literals.len());
        for lit in &clause.literals {
            let Atom::Pred(sym, args) = &lit.atom else {
                return None;
            };
            let var = *self.sym_to_var.get(&sym.index())? as PL;
            let (_, arity) = self.var_to_sym_arity[(var.unsigned_abs() - 1) as usize];
            if args.len() != arity || args.iter().any(|term| !matches!(term, Term::Var(_))) {
                return None;
            }
            literals.push(if lit.positive { var } else { -var });
        }
        literals.sort_unstable();
        literals.dedup();
        Some(literals)
    }

    fn abstract_literal(&self, literal: &Literal) -> Option<PL> {
        let Atom::Pred(sym, args) = &literal.atom else {
            return None;
        };
        let var = *self.sym_to_var.get(&sym.index())? as PL;
        let (_, arity) = self.var_to_sym_arity[(var.unsigned_abs() - 1) as usize];
        if args.len() != arity || args.iter().any(|term| !matches!(term, Term::Var(_))) {
            return None;
        }
        Some(if literal.positive { var } else { -var })
    }
}

// ---------------------------------------------------------------------------
// Propositional BFS resolution prover
// ---------------------------------------------------------------------------

/// How a propositional clause was derived.
#[derive(Clone)]
enum PSrc {
    /// Index `i` into the original input slice.
    Input(usize),
    /// Derived by resolving the clauses at `left` and `right` (indices in the
    /// BFS prover's own clause vector).
    Resolvent {
        left: usize,
        right: usize,
        pivot: PL,
    },
}

/// Resolve clauses `c1` and `c2` on literal `lit`.
/// Returns `None` if the resolvent is a tautology.
fn resolve_prop(c1: &[PL], c2: &[PL], lit: PL) -> Option<PC> {
    let mut result: Vec<PL> = c1
        .iter()
        .chain(c2.iter())
        .copied()
        .filter(|&l| l != lit && l != -lit)
        .collect::<HashSet<PL>>()
        .into_iter()
        .collect();
    result.sort();
    // Reject tautologies: both l and ~l present.
    for &l in &result {
        if l > 0 && result.binary_search(&-l).is_ok() {
            return None;
        }
    }
    Some(result)
}

/// Maximum number of derived clauses before giving up.
const MAX_DERIVED: usize = 100_000;

/// BFS propositional resolution prover.
///
/// Returns `(all_clauses, all_sources, empty_clause_index)` on success,
/// or `None` if no refutation is found within `MAX_DERIVED` derived clauses.
fn prop_bfs_refute(input: &[PC]) -> Option<(Vec<PC>, Vec<PSrc>, usize)> {
    let mut clauses: Vec<PC> = Vec::new();
    let mut sources: Vec<PSrc> = Vec::new();
    let mut seen: HashSet<PC> = HashSet::default();

    // Load input clauses, deduplicating identical ones.
    for (i, c) in input.iter().enumerate() {
        if seen.insert(c.clone()) {
            let is_empty = c.is_empty();
            clauses.push(c.clone());
            sources.push(PSrc::Input(i));
            if is_empty {
                let idx = clauses.len() - 1;
                return Some((clauses, sources, idx));
            }
        }
    }

    let mut head = 0;
    while head < clauses.len() {
        // Clone to avoid holding a borrow across the push below.
        let c_head = clauses[head].clone();
        for j in 0..head {
            // Clone to avoid holding two borrows when we later push.
            let c_j = clauses[j].clone();
            for &lit in &c_head {
                if c_j.binary_search(&-lit).is_err() {
                    continue;
                }
                let Some(resolvent) = resolve_prop(&c_head, &c_j, lit) else {
                    continue;
                };
                if seen.insert(resolvent.clone()) {
                    let is_empty = resolvent.is_empty();
                    clauses.push(resolvent);
                    sources.push(PSrc::Resolvent {
                        left: head,
                        right: j,
                        pivot: lit,
                    });
                    if is_empty {
                        let idx = clauses.len() - 1;
                        return Some((clauses, sources, idx));
                    }
                    if clauses.len() > MAX_DERIVED {
                        return None;
                    }
                }
            }
        }
        head += 1;
    }
    None // Saturated without empty clause (SAT or exceeded cap)
}

// ---------------------------------------------------------------------------
// FOF proof lifting
// ---------------------------------------------------------------------------

/// Replays an abstract resolution step using first-order resolution. This is
/// deliberately fail-closed: the actual resolvent must have exactly the
/// expected propositional image, otherwise this proof path cannot justify the
/// abstract step.
fn replay_resolution(
    left: &Clause,
    right: &Clause,
    expected: &[PL],
    pivot: PL,
    abs: &PropAbstraction,
    id_gen: &mut ClauseIdGen,
) -> Option<Clause> {
    let left_selected: Vec<_> = left
        .literals
        .iter()
        .enumerate()
        .filter_map(|(index, literal)| {
            (abs.abstract_literal(literal) == Some(pivot)).then_some(index)
        })
        .collect();
    let right_selected: Vec<_> = right
        .literals
        .iter()
        .enumerate()
        .filter_map(|(index, literal)| {
            (abs.abstract_literal(literal) == Some(-pivot)).then_some(index)
        })
        .collect();
    if left_selected.is_empty() || right_selected.is_empty() {
        return None;
    }
    resolve_selected(
        left,
        right,
        id_gen,
        Some(&left_selected),
        Some(&right_selected),
        &HashSet::default(),
    )
    .into_iter()
    .find(|resolvent| {
        abs.abstract_clause(resolvent)
            .is_some_and(|image| image == expected)
    })
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

fn dfs_topo(
    idx: usize,
    prop_sources: &[PSrc],
    visited: &mut HashSet<usize>,
    order: &mut Vec<usize>,
) {
    if !visited.insert(idx) {
        return;
    }
    if let PSrc::Resolvent { left, right, .. } = &prop_sources[idx] {
        dfs_topo(*left, prop_sources, visited, order);
        dfs_topo(*right, prop_sources, visited, order);
    }
    order.push(idx);
}

/// Attempts to refute an FVO clause set using propositional skeleton resolution.
///
/// Returns `Some(SearchResult::Refutation(id, tstp))` if:
/// - The problem is FVO (all predicate args are variables, no equality), **and**
/// - The propositional skeleton is UNSAT (confirmed by `mrs-cadical`), **and**
/// - A BFS resolution proof is found within `MAX_DERIVED` derived clauses.
///
/// Returns `None` in all other cases; the caller should try the regular
/// strategy schedule.
pub fn try_fvo_refutation(
    clauses: &[Clause],
    provenance: &[Clause],
    id_gen: &mut ClauseIdGen,
    symbols: &SymbolTable,
) -> Option<SearchResult> {
    if !is_fvo_problem(clauses) {
        return None;
    }

    let abs = PropAbstraction::build(clauses);

    // Fast oracle: use CaDiCaL to check propositional UNSAT before BFS.
    // This avoids O(n²) BFS work when the problem is actually satisfiable.
    {
        let mut solver = Solver::new();
        for pc in &abs.prop_clauses {
            solver.add_clause(pc.as_slice());
        }
        match solver.solve() {
            SolveResult::Unsat => {} // UNSAT: proceed to proof extraction
            _ => return None,        // SAT or solver error: give up
        }
    }

    // BFS resolution prover: generate a step-by-step propositional proof.
    let (prop_clauses, prop_sources, empty_idx) = prop_bfs_refute(&abs.prop_clauses)?;

    // Collect the proof ancestors in topological order (parents before children).
    let mut visited: HashSet<usize> = HashSet::default();
    let mut order: Vec<usize> = Vec::new();
    dfs_topo(empty_idx, &prop_sources, &mut visited, &mut order);

    // Build the lifted FOF proof.
    let mut prop_idx_to_fof_clause: HashMap<usize, Clause> = HashMap::default();
    let mut fof_proof: Vec<Clause> = Vec::with_capacity(provenance.len() + order.len());

    // Prepend provenance steps so the proof is fully self-contained back to the conjecture!
    fof_proof.extend(provenance.iter().cloned());

    for &prop_idx in &order {
        match &prop_sources[prop_idx] {
            PSrc::Input(input_idx) => {
                // Use the original FOF clause unchanged (preserves ClauseId and source).
                let original = &clauses[*input_idx];
                prop_idx_to_fof_clause.insert(prop_idx, original.clone());
                fof_proof.push(original.clone());
            }
            PSrc::Resolvent { left, right, pivot } => {
                let lifted = replay_resolution(
                    &prop_idx_to_fof_clause[left],
                    &prop_idx_to_fof_clause[right],
                    &prop_clauses[prop_idx],
                    *pivot,
                    &abs,
                    id_gen,
                )?;
                prop_idx_to_fof_clause.insert(prop_idx, lifted.clone());
                fof_proof.push(lifted);
            }
        }
    }

    let empty = prop_idx_to_fof_clause.get(&empty_idx)?;
    if !empty.is_empty() {
        return None;
    }
    let empty_id = empty.id;
    let tstp = format_tstp(&fof_proof, symbols);

    Some(SearchResult::Refutation(empty_id, tstp))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::SymbolTable;
    use mrs_core::clause::{ClauseIdGen, ClauseSource};

    fn make_clause(id_gen: &mut ClauseIdGen, lits: Vec<Literal>, name: &str) -> Clause {
        Clause::new(
            id_gen.next(),
            lits,
            ClauseSource::Input {
                name: name.to_string(),
                role: "negated_conjecture".to_string(),
            },
        )
    }

    #[test]
    fn fvo_rejects_equality_atom() {
        let mut syms = SymbolTable::new();
        let a = syms.intern("a");
        let mut id_gen = ClauseIdGen::new();
        let c = make_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::eq(Term::constant(a), Term::constant(a)))],
            "c1",
        );
        assert!(!is_fvo_clause(&c));
        assert!(!is_fvo_problem(&[c]));
    }

    #[test]
    fn fvo_rejects_function_argument() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let f = syms.intern("f");
        let mut id_gen = ClauseIdGen::new();
        let c = make_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(
                p,
                vec![Term::app(f, vec![Term::var(0)])],
            ))],
            "c1",
        );
        assert!(!is_fvo_clause(&c));
    }

    #[test]
    fn fvo_accepts_variable_args() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let mut id_gen = ClauseIdGen::new();
        let c = make_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(0)])),
                Literal::neg(Atom::pred(q, vec![Term::var(1), Term::var(2)])),
            ],
            "c1",
        );
        assert!(is_fvo_clause(&c));
    }

    #[test]
    fn fvo_accepts_propositional_atoms() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let mut id_gen = ClauseIdGen::new();
        let c = make_clause(&mut id_gen, vec![Literal::pos(Atom::prop(p))], "c1");
        assert!(is_fvo_clause(&c));
    }

    #[test]
    fn fvo_replay_preserves_shared_variables() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let r = syms.intern("r");
        let mut id_gen = ClauseIdGen::new();
        let left = make_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(4)])),
                Literal::pos(Atom::pred(q, vec![Term::var(4)])),
            ],
            "left",
        );
        let right = make_clause(
            &mut id_gen,
            vec![
                Literal::neg(Atom::pred(p, vec![Term::var(0)])),
                Literal::pos(Atom::pred(r, vec![Term::var(0)])),
            ],
            "right",
        );
        let abstraction = PropAbstraction::build(&[left.clone(), right.clone()]);
        let p_var = abstraction.sym_to_var[&p.index()] as PL;
        let q_var = abstraction.sym_to_var[&q.index()] as PL;
        let r_var = abstraction.sym_to_var[&r.index()] as PL;
        let resolvent = replay_resolution(
            &left,
            &right,
            &[q_var, r_var],
            p_var,
            &abstraction,
            &mut id_gen,
        )
        .expect("the shared-variable resolvent must replay");
        assert_eq!(resolvent.literals.len(), 2);
        let q_arg = match &resolvent.literals[0].atom {
            Atom::Pred(sym, args) if *sym == q => &args[0],
            _ => match &resolvent.literals[1].atom {
                Atom::Pred(sym, args) if *sym == q => &args[0],
                _ => panic!("resolvent should contain q"),
            },
        };
        let r_arg = match &resolvent.literals[0].atom {
            Atom::Pred(sym, args) if *sym == r => &args[0],
            _ => match &resolvent.literals[1].atom {
                Atom::Pred(sym, args) if *sym == r => &args[0],
                _ => panic!("resolvent should contain r"),
            },
        };
        assert_eq!(q_arg, r_arg, "resolution must preserve variable sharing");
    }

    #[test]
    fn fvo_resolution_standardizes_parent_variables_apart() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let r = syms.intern("r");
        let mut id_gen = ClauseIdGen::new();
        // Both parents use Var(0) for the pivot and Var(1) for a residual
        // literal. The same variable IDs are clause-local, so parent
        // standardization apart must keep the residual arguments distinct.
        let left = make_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(0)])),
                Literal::pos(Atom::pred(q, vec![Term::var(1)])),
            ],
            "left",
        );
        let right = make_clause(
            &mut id_gen,
            vec![
                Literal::neg(Atom::pred(p, vec![Term::var(0)])),
                Literal::pos(Atom::pred(r, vec![Term::var(1)])),
            ],
            "right",
        );
        let abstraction = PropAbstraction::build(&[left.clone(), right.clone()]);
        let p_var = abstraction.sym_to_var[&p.index()] as PL;
        let q_var = abstraction.sym_to_var[&q.index()] as PL;
        let r_var = abstraction.sym_to_var[&r.index()] as PL;
        let resolvent = replay_resolution(
            &left,
            &right,
            &[q_var, r_var],
            p_var,
            &abstraction,
            &mut id_gen,
        )
        .expect("standardized-apart resolution must replay");
        let q_arg = resolvent
            .literals
            .iter()
            .find_map(|literal| match &literal.atom {
                Atom::Pred(sym, args) if *sym == q => args.first(),
                _ => None,
            })
            .expect("resolvent should contain q");
        let r_arg = resolvent
            .literals
            .iter()
            .find_map(|literal| match &literal.atom {
                Atom::Pred(sym, args) if *sym == r => args.first(),
                _ => None,
            })
            .expect("resolvent should contain r");
        assert_ne!(q_arg, r_arg, "different parents' residual vars stay apart");
    }

    #[test]
    fn fvo_simple_variable_refutation_replays_in_first_order() {
        // p(X) | q(Y), ~p(X), ~q(X). The abstract proof lifts through real
        // first-order resolution even though the variables are independent in
        // the first clause.
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let mut id_gen = ClauseIdGen::new();

        let c1 = make_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(0)])),
                Literal::pos(Atom::pred(q, vec![Term::var(1)])),
            ],
            "c1",
        );
        let c2 = make_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::pred(p, vec![Term::var(0)]))],
            "c2",
        );
        let c3 = make_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::pred(q, vec![Term::var(0)]))],
            "c3",
        );

        assert!(is_fvo_problem(&[c1.clone(), c2.clone(), c3.clone()]));
        let result = try_fvo_refutation(&[c1, c2, c3], &[], &mut id_gen, &syms);
        assert!(
            matches!(result, Some(SearchResult::Refutation(..))),
            "expected a first-order refutation, got {:?}",
            result
        );
    }

    #[test]
    fn fvo_resolves_shared_variable_counterexample_with_valid_lifting() {
        // Propositional resolution derives q | r from p | q and ~p | r.
        // The actual first-order resolvent is q(X) | r(X), so the emitted
        // resolution DAG must preserve this variable sharing.
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let r = syms.intern("r");
        let mut id_gen = ClauseIdGen::new();
        let clauses = vec![
            make_clause(
                &mut id_gen,
                vec![
                    Literal::pos(Atom::pred(p, vec![Term::var(0)])),
                    Literal::pos(Atom::pred(q, vec![Term::var(0)])),
                ],
                "shared_left",
            ),
            make_clause(
                &mut id_gen,
                vec![
                    Literal::neg(Atom::pred(p, vec![Term::var(0)])),
                    Literal::pos(Atom::pred(r, vec![Term::var(0)])),
                ],
                "shared_right",
            ),
            make_clause(
                &mut id_gen,
                vec![Literal::neg(Atom::pred(q, vec![Term::var(0)]))],
                "not_q",
            ),
            make_clause(
                &mut id_gen,
                vec![Literal::neg(Atom::pred(r, vec![Term::var(0)]))],
                "not_r",
            ),
        ];

        assert!(is_fvo_problem(&clauses));
        let Some(SearchResult::Refutation(_, tstp)) =
            try_fvo_refutation(&clauses, &[], &mut id_gen, &syms)
        else {
            panic!("expected shared-variable FVO refutation");
        };
        assert!(tstp.contains("inference(resolution"));
    }

    #[test]
    fn fvo_still_refutes_nullary_propositional_clauses() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let mut id_gen = ClauseIdGen::new();
        let clauses = vec![
            make_clause(
                &mut id_gen,
                vec![Literal::pos(Atom::prop(p)), Literal::pos(Atom::prop(q))],
                "p_or_q",
            ),
            make_clause(&mut id_gen, vec![Literal::neg(Atom::prop(p))], "not_p"),
            make_clause(&mut id_gen, vec![Literal::neg(Atom::prop(q))], "not_q"),
        ];

        assert!(is_fvo_problem(&clauses));
        let result = try_fvo_refutation(&clauses, &[], &mut id_gen, &syms);
        assert!(matches!(result, Some(SearchResult::Refutation(..))));
    }

    #[test]
    fn fvo_rejects_inconsistent_predicate_arities() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let mut id_gen = ClauseIdGen::new();
        let unary = make_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::var(0)]))],
            "unary_p",
        );
        let binary = make_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::pred(
                p,
                vec![Term::var(0), Term::var(1)],
            ))],
            "binary_p",
        );
        assert!(!is_fvo_problem(&[unary, binary]));
    }

    #[test]
    fn fvo_sat_returns_none() {
        // Satisfiable: just p(X) (no contradiction)
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let mut id_gen = ClauseIdGen::new();
        let c1 = make_clause(&mut id_gen, vec![Literal::pos(Atom::prop(p))], "c1");
        let result = try_fvo_refutation(&[c1], &[], &mut id_gen, &syms);
        assert!(result.is_none(), "expected None for SAT problem");
    }

    #[test]
    fn fvo_non_fvo_returns_none() {
        // Non-FVO: contains equality atom
        let mut syms = SymbolTable::new();
        let a = syms.intern("a");
        let b = syms.intern("b");
        let mut id_gen = ClauseIdGen::new();
        let c1 = make_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::eq(Term::constant(a), Term::constant(b)))],
            "c1",
        );
        let result = try_fvo_refutation(&[c1], &[], &mut id_gen, &syms);
        assert!(result.is_none(), "expected None for non-FVO problem");
    }

    #[test]
    fn fvo_tstp_contains_resolution_steps() {
        // p | q, ~p, ~q → proof should mention "resolution"
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let mut id_gen = ClauseIdGen::new();

        let c1 = make_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::prop(p)), Literal::pos(Atom::prop(q))],
            "c1",
        );
        let c2 = make_clause(&mut id_gen, vec![Literal::neg(Atom::prop(p))], "c2");
        let c3 = make_clause(&mut id_gen, vec![Literal::neg(Atom::prop(q))], "c3");

        if let Some(SearchResult::Refutation(_, tstp)) =
            try_fvo_refutation(&[c1, c2, c3], &[], &mut id_gen, &syms)
        {
            assert!(
                tstp.contains("resolution"),
                "TSTP should use 'resolution' rule"
            );
            assert!(
                tstp.contains("$false"),
                "TSTP should contain empty clause ($false)"
            );
        } else {
            panic!("expected Refutation");
        }
    }
}
