//! FVO (FNE-Variable-Only) propositional skeleton refutation.
//!
//! For clause sets where every predicate literal has only variable arguments
//! (and no equality atoms), the first-order problem is propositionally
//! equivalent: replacing each predicate symbol with a propositional variable
//! (ignoring all arguments) is a sound abstraction for this fragment.
//!
//! **Soundness**: For FVO problems, every propositional resolution step lifts
//! to a valid first-order resolution step by variable renaming.  Therefore
//! propositional UNSAT implies FOF UNSAT.
//!
//! The algorithm:
//! 1. Detect FVO (no equality, all predicate args are variables).
//! 2. Use `cadical` as a fast oracle to check propositional UNSAT.
//! 3. If UNSAT, run a BFS resolution prover to produce a step-by-step proof.
//! 4. Lift each propositional resolution step to first-order by introducing
//!    fresh variables for each predicate argument.
//! 5. Return `SearchResult::Refutation` with the TSTP-formatted proof.

use std::collections::{HashMap, HashSet, VecDeque};

use mrs_core::SymbolTable;
use mrs_core::clause::{Clause, ClauseId, ClauseIdGen, ClauseSource, Literal};
use mrs_core::formula::Atom;
use mrs_core::symbol::SymbolId;
use mrs_core::term::Term;
use mrs_proof::tstp::format_tstp;

use crate::SearchResult;

// ---------------------------------------------------------------------------
// FVO detection
// ---------------------------------------------------------------------------

/// Returns `true` if all predicate arguments in the clause are variables.
/// Clauses containing equality atoms (`=`) are rejected.
pub fn is_fvo_clause(clause: &Clause) -> bool {
    clause.literals.iter().all(|lit| match &lit.atom {
        Atom::Pred(_, args) => args.iter().all(|t| matches!(t, Term::Var(_))),
        Atom::Eq(_, _) => false,
    })
}

/// Returns `true` if every clause in the problem is FVO and the slice is
/// non-empty.
pub fn is_fvo_problem(clauses: &[Clause]) -> bool {
    !clauses.is_empty() && clauses.iter().all(is_fvo_clause)
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
}

impl PropAbstraction {
    fn build(clauses: &[Clause]) -> Self {
        let mut sym_to_var: HashMap<u32, u32> = HashMap::new();
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
        }
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
    Resolvent { left: usize, right: usize },
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
    let mut seen: HashSet<PC> = HashSet::new();

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

/// Lifts a propositional clause to a FOF clause by introducing fresh variables.
/// Each literal `±v` becomes `Pred(sym, [Var(0), Var(1), ...])` with one fresh
/// variable per argument position (variables are local to this clause).
fn lift_clause(
    prop_c: &[PL],
    abs: &PropAbstraction,
    id_gen: &mut ClauseIdGen,
    source: ClauseSource,
) -> Clause {
    let mut next_var: u32 = 0;
    let literals: Vec<Literal> = prop_c
        .iter()
        .map(|&pl| {
            let var_idx = (pl.unsigned_abs() - 1) as usize;
            let (sym, arity) = abs.var_to_sym_arity[var_idx];
            let args: Vec<Term> = (0..arity)
                .map(|_| {
                    let v = next_var;
                    next_var += 1;
                    Term::var(v)
                })
                .collect();
            let atom = Atom::pred(sym, args);
            if pl > 0 {
                Literal::pos(atom)
            } else {
                Literal::neg(atom)
            }
        })
        .collect();
    Clause::new(id_gen.next(), literals, source)
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// Attempts to refute an FVO clause set using propositional skeleton resolution.
///
/// Returns `Some(SearchResult::Refutation(id, tstp))` if:
/// - The problem is FVO (all predicate args are variables, no equality), **and**
/// - The propositional skeleton is UNSAT (confirmed by `varisat`), **and**
/// - A BFS resolution proof is found within `MAX_DERIVED` derived clauses.
///
/// Returns `None` in all other cases; the caller should try the regular
/// strategy schedule.
pub fn try_fvo_refutation(
    clauses: &[Clause],
    id_gen: &mut ClauseIdGen,
    symbols: &SymbolTable,
) -> Option<SearchResult> {
    if !is_fvo_problem(clauses) {
        return None;
    }

    let abs = PropAbstraction::build(clauses);

    // Fast oracle: use cadical to check propositional UNSAT before BFS.
    // This avoids O(n²) BFS work when the problem is actually satisfiable.
    {
        let mut solver: cadical::Solver = cadical::Solver::new();
        for pc in &abs.prop_clauses {
            solver.add_clause(pc.iter().map(|&l| l as i32));
        }
        match solver.solve() {
            Some(false) => {} // UNSAT: proceed to proof extraction
            _ => return None, // SAT or solver error: give up
        }
    }

    // BFS resolution prover: generate a step-by-step propositional proof.
    let (prop_clauses, prop_sources, empty_idx) = prop_bfs_refute(&abs.prop_clauses)?;

    // Collect the proof ancestors in topological order (parents before children).
    let mut visited: HashSet<usize> = HashSet::new();
    let mut queue: VecDeque<usize> = VecDeque::new();
    let mut order: Vec<usize> = Vec::new();

    queue.push_back(empty_idx);
    visited.insert(empty_idx);

    while let Some(idx) = queue.pop_front() {
        order.push(idx);
        if let PSrc::Resolvent { left, right } = &prop_sources[idx] {
            if visited.insert(*left) {
                queue.push_back(*left);
            }
            if visited.insert(*right) {
                queue.push_back(*right);
            }
        }
    }

    order.reverse(); // topological: inputs first, empty clause last

    // Build the lifted FOF proof.
    let mut prop_idx_to_fof_id: HashMap<usize, ClauseId> = HashMap::new();
    let mut fof_proof: Vec<Clause> = Vec::with_capacity(order.len());

    for &prop_idx in &order {
        match &prop_sources[prop_idx] {
            PSrc::Input(input_idx) => {
                // Use the original FOF clause unchanged (preserves ClauseId and source).
                let original = &clauses[*input_idx];
                prop_idx_to_fof_id.insert(prop_idx, original.id);
                fof_proof.push(original.clone());
            }
            PSrc::Resolvent { left, right } => {
                let left_id = prop_idx_to_fof_id[left];
                let right_id = prop_idx_to_fof_id[right];
                let source = ClauseSource::Inference {
                    rule: "resolution".to_string(),
                    parents: vec![left_id, right_id],
                };
                let lifted = lift_clause(&prop_clauses[prop_idx], &abs, id_gen, source);
                prop_idx_to_fof_id.insert(prop_idx, lifted.id);
                fof_proof.push(lifted);
            }
        }
    }

    let empty_id = fof_proof.last()?.id;
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
    fn fvo_simple_refutation() {
        // UNSAT problem: p(X) | q(Y), ~p(X), ~q(X)
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
        let result = try_fvo_refutation(&[c1, c2, c3], &mut id_gen, &syms);
        assert!(
            matches!(result, Some(SearchResult::Refutation(..))),
            "expected Refutation, got {:?}",
            result
        );
    }

    #[test]
    fn fvo_sat_returns_none() {
        // Satisfiable: just p(X) (no contradiction)
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let mut id_gen = ClauseIdGen::new();
        let c1 = make_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::var(0)]))],
            "c1",
        );
        let result = try_fvo_refutation(&[c1], &mut id_gen, &syms);
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
        let result = try_fvo_refutation(&[c1], &mut id_gen, &syms);
        assert!(result.is_none(), "expected None for non-FVO problem");
    }

    #[test]
    fn fvo_tstp_contains_resolution_steps() {
        // p(X) | q(Y), ~p(X), ~q(X) → proof should mention "resolution"
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

        if let Some(SearchResult::Refutation(_, tstp)) =
            try_fvo_refutation(&[c1, c2, c3], &mut id_gen, &syms)
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
