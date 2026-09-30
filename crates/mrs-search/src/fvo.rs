//! FVO (FNE-Variable-Only) propositional skeleton refutation.
//!
//! For clause sets where every predicate argument is a variable and there is no
//! equality, replacing each predicate symbol with one propositional variable —
//! the *skeleton* — is an exact satisfiability abstraction, and a refutation of
//! the skeleton can be searched for far more cheaply than of the clause set.
//! This module then turns that skeleton refutation into a real first-order
//! proof.
//!
//! # The abstraction
//!
//! Skeleton UNSAT implies FOL UNSAT, by an argument in both directions:
//!
//! * An FOL model induces a propositional valuation by evaluating each predicate
//!   on a diagonal tuple `(d, ..., d)` for a domain element `d`. Every predicate
//!   argument is a variable, so instantiating all of them to `d` turns any
//!   ground instance of a clause into a literal of the skeleton clause, and the
//!   clause is satisfied.
//! * Conversely a satisfying valuation extends to a model by interpreting each
//!   predicate *uniformly* over all tuples — all of them if the variable is set,
//!   none if not — under which every atom's truth depends only on its predicate,
//!   so a clause is satisfied exactly when its skeleton is.
//!
//! So the abstraction is where the search happens and the *proof* is what has to
//! be earned separately, below.
//!
//! # The proof
//!
//! A skeleton step is not a first-order step, and the two ways it can fail to be
//! one are both bugs this code has had:
//!
//! * The skeleton is a **multiset** of literals. With set semantics `p(X) | p(Y)`
//!   abstracts to the unit `{p}`, so the skeleton resolves it with `~p(Z)` to the
//!   empty clause where first-order resolution on those parents yields `p(Y)`.
//!   No literal of `p(Y)` survives, so emitting the empty clause is not a
//!   weakening of the resolvent: the refutation would be *unsound*, not merely
//!   unprovable. `mrs-cnf` does not merge duplicate literals, so repeated
//!   predicates do reach this code.
//! * Each step is **replayed** in first order by [`fvo_resolve`] rather than
//!   synthesized from the skeleton. Synthesizing generalizes the real resolvent
//!   whenever a surviving literal shares variables with the pivot: resolving
//!   `~r(X,Y) | ~r(X,Y)` with `r(Z,Z)` gives `~r(Z,Z)`, not `~r(A,B)` over fresh
//!   variables. The lifted clause is still a consequence, so the refutation
//!   survives — but the proof cites an inference that did not happen.
//!
//! With both fixed the invariant is:
//!
//! > For every skeleton clause `C'` in the BFS prover's clause vector there is
//! > an emitted first-order clause `C` that is a consequence of the input set
//! > and whose multiset of (predicate, polarity) pairs is exactly `C'`'s.
//!
//! *Base case*: an `Input` step emits the original clause, whose multiset is the
//! skeleton's by construction.
//!
//! *Step*: the skeleton resolvent removes one occurrence of the pivot from each
//! parent, and [`fvo_resolve`] resolves the two emitted parents on that same
//! predicate, removing exactly one matching literal from each. The parents are
//! consequences, so the resolvent is one; and the two pivot atoms always
//! unify, being tuples of distinct variables over one symbol. Replay also
//! standardizes the parents apart first, so two clauses that both number their
//! first variable `0` are not spuriously identified.
//!
//! The empty skeleton clause therefore has an empty first-order counterpart, and
//! the empty clause is arrived at by resolution. The abstraction argument above
//! is not what makes this sound — the proof carries it — but the two agree.
//!
//! Two preconditions, both enforced by [`PropAbstraction::build`]: arguments must
//! be variables and there must be no equality, or the atoms need not unify; and
//! each predicate must have **one** arity, since the skeleton keys its variables
//! on the symbol alone, so `p` and `p(X)` would collapse and the lift would pair
//! a 0-ary atom with a 1-ary one. Such a signature is ill-formed TPTP but the
//! parser accepts it, so it is rejected here rather than trusted.
//!
//! The tests at the bottom of this file run every emitted proof through
//! `mrs-proof-kernel`, which replays each rule against the problem from scratch
//! and shares no code with the prover. That is a stronger statement than either
//! argument above, and it is the one to trust: an argument this narrow is exactly
//! the kind that turns out to be wrong, and it was.
//!
//! # Cost
//!
//! Multiset resolution is refutation complete but weaker in practice than the
//! set calculus, and derived clauses that repeat a predicate — which happens as
//! soon as two parents share a non-pivot predicate, the ordinary case once a
//! problem has three or more predicates — enlarge the clause space enough to
//! matter. On a corpus of over-constrained random 3-CNFs written as 0-arity FOF
//! predicates, the pre-pass found a refutation on 25 instances before and on 0
//! after. No final status changed, because the portfolio covers those problems
//! anyway.
//!
//! That is a real trade, and it is the reason the alternative was rejected: the
//! sound way to keep the set skeleton is to accept a resolvent only when its
//! image matches and **give up on the whole proof** when it does not, which
//! loses the same instances plus every one with a repeated predicate in the
//! input. Keeping multiplicity loses only the search power.
//!
//! Recovering that power means pruning the duplicate-bearing clauses, and the
//! sound way to do it is subsumption rather than deduplication: `q(X) | q(Y)` is
//! subsumed by `q(Z)`, whereas dropping a literal from `p(X,Y) | p(Y,X)` is not
//! sound, because those two atoms have no unifier. Subsumption is separate work
//! and wants its own measurement.
//!
//! What makes the trade acceptable now is that the pre-pass does not fire on the
//! divisions it was written for: across the 800 CASC-J13 FNE, UEQ and FEQ
//! problems, `mrs --profile` reports FVO on none of them, and the `SYN938+1`
//! problem an earlier revision of this comment credited it with is not FVO
//! either (`FVO (Vars-Only Pred):No`). It is currently reachable only on input
//! no competition problem produces, so its coverage is not carrying the
//! schedule. Fixing that, or deleting the pre-pass, is worth more than tuning
//! it.
//!
//! # Algorithm
//!
//! 1. Detect FVO (no equality, all predicate args are variables, one arity per
//!    predicate).
//! 2. Use `mrs-cadical` as a fast oracle to check propositional UNSAT.
//! 3. If UNSAT, run a BFS resolution prover to produce a step-by-step skeleton.
//! 4. Replay each skeleton step as a first-order resolution step over the
//!    clauses the proof emits, so the output is a real derivation.
//! 5. Return `SearchResult::Refutation` with the TSTP-formatted proof.
//!
//! Steps 2 and 3 are bounded: see [`fvo_budget`] and [`MAX_DERIVED`].

use std::time::{Duration, Instant};

use crate::{HashMap, HashSet};

use mrs_core::SymbolTable;
use mrs_core::clause::{Clause, ClauseIdGen, Literal};
use mrs_core::formula::Atom;
use mrs_core::symbol::SymbolId;
use mrs_core::term::Term;
use mrs_proof::tstp::format_tstp;

use crate::SearchResult;
use mrs_cadical::Solver;

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

/// Returns `true` if the slice is a non-empty clause set this module can
/// abstract soundly: every clause FVO, and every predicate symbol used at a
/// single arity.
pub fn is_fvo_problem(clauses: &[Clause]) -> bool {
    !clauses.is_empty() && PropAbstraction::build(clauses, None).is_some()
}

// ---------------------------------------------------------------------------
// Propositional abstraction
// ---------------------------------------------------------------------------

/// Signed propositional literal (DIMACS convention, 1-indexed).
/// `k > 0`: predicate-k is true; `k < 0`: predicate-k is false.
type PL = i32;

/// A propositional clause: a sorted **multiset** of literals.
///
/// Duplicates are kept on purpose — see the module soundness note. Removing
/// them makes the abstraction stronger than first-order resolution can
/// justify, and the resulting proof is rejected by an independent checker.
type PC = Vec<PL>;

/// Propositional abstraction of an FVO clause set.
struct PropAbstraction {
    /// `prop_clauses[i]` is the propositional image of input clause `i`.
    prop_clauses: Vec<PC>,
    /// `var_to_sym_arity[v-1] = (SymbolId, arity)` for prop variable `v`.
    var_to_sym_arity: Vec<(SymbolId, usize)>,
}

impl PropAbstraction {
    /// Builds the abstraction, or returns `None` when the clause set is outside
    /// the fragment this pre-pass is sound for:
    ///
    /// * a predicate argument that is not a variable, or an equality atom
    ///   (then the atoms need not unify, so a skeleton step has no first-order
    ///   counterpart), or
    /// * a predicate symbol used at **more than one arity**. The abstraction
    ///   keys its propositional variable on the symbol alone, so `p` and
    ///   `p(X)` would collapse to one variable and the lift would pair a
    ///   0-ary atom with a 1-ary one. Those do not unify, so the emitted step
    ///   would not be a resolution inference at all. Such a problem is
    ///   ill-formed TPTP, but the parser accepts it, so it is rejected here
    ///   rather than trusted.
    fn build(clauses: &[Clause], deadline: Option<Instant>) -> Option<Self> {
        let mut arities: HashMap<SymbolId, usize> = HashMap::default();
        let mut checked_literals = 0usize;
        for clause in clauses {
            for lit in &clause.literals {
                checked_literals += 1;
                if checked_literals.is_multiple_of(DEADLINE_CHECK_INTERVAL)
                    && deadline.is_some_and(|end| Instant::now() >= end)
                {
                    return None;
                }
                let Atom::Pred(sym, args) = &lit.atom else {
                    return None;
                };
                if args.iter().any(|term| !matches!(term, Term::Var(_))) {
                    return None;
                }
                match arities.get(sym) {
                    Some(&arity) if arity != args.len() => return None,
                    Some(_) => {}
                    None => {
                        arities.insert(*sym, args.len());
                    }
                }
            }
        }

        let mut sym_to_var: HashMap<u32, u32> = HashMap::default();
        let mut var_to_sym_arity: Vec<(SymbolId, usize)> = Vec::new();
        let mut prop_clauses = Vec::with_capacity(clauses.len());
        for clause in clauses {
            let mut lits = Vec::with_capacity(clause.literals.len());
            for lit in &clause.literals {
                checked_literals += 1;
                if checked_literals.is_multiple_of(DEADLINE_CHECK_INTERVAL)
                    && deadline.is_some_and(|end| Instant::now() >= end)
                {
                    return None;
                }
                let Atom::Pred(sym, args) = &lit.atom else {
                    return None;
                };
                let var = *sym_to_var.entry(sym.index()).or_insert_with(|| {
                    let v = var_to_sym_arity.len() as u32 + 1; // 1-indexed
                    var_to_sym_arity.push((*sym, args.len()));
                    v
                });
                lits.push(if lit.positive {
                    var as PL
                } else {
                    -(var as PL)
                });
            }
            lits.sort();
            prop_clauses.push(lits);
        }

        Some(Self {
            prop_clauses,
            var_to_sym_arity,
        })
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
    /// BFS prover's own clause vector) on `pivot`.
    ///
    /// `pivot` is the skeleton literal taken from the left parent; the right
    /// parent supplies its complement. It is recorded rather than recomputed
    /// later so that the first-order resolvent is computed for the *same*
    /// predicate the skeleton step used.
    Resolvent {
        left: usize,
        right: usize,
        pivot: PL,
    },
}

/// Resolve clauses `c1` and `c2` on literal `lit`, which the caller has already
/// checked occurs in `c1` and whose complement occurs in `c2`.
///
/// Resolution removes **one** occurrence of the pivot from each side, which is
/// the whole point of keeping duplicates: with set semantics this would drop
/// every occurrence and let a clause such as `p(X) | p(Y)` stand in for a
/// first-order resolvent that does not exist. See the module soundness note.
///
/// Returns `None` if the resolvent is a tautology.
fn resolve_prop(c1: &[PL], c2: &[PL], lit: PL) -> Option<PC> {
    let mut result: Vec<PL> = Vec::with_capacity(c1.len() + c2.len());
    let mut dropped_pivot = false;
    for &l in c1 {
        if l == lit && !dropped_pivot {
            dropped_pivot = true;
            continue;
        }
        result.push(l);
    }
    let mut dropped_complement = false;
    for &l in c2 {
        if l == -lit && !dropped_complement {
            dropped_complement = true;
            continue;
        }
        result.push(l);
    }
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
///
/// A cap on the clause count, not a time bound: the loop is superlinear in it,
/// so on an instance whose closure lands just below this number the clause cap
/// never fires and the wall clock in [`prop_bfs_refute`] is what stops the work.
/// That is why the caller passes a deadline.
const MAX_DERIVED: usize = 100_000;

/// How many parent-resolution attempts run between two deadline checks. The
/// inner loop is quadratic in the clause count, so a check per outer iteration
/// would leave a single iteration running for seconds near `MAX_DERIVED`.
const DEADLINE_CHECK_INTERVAL: usize = 1 << 12;

/// BFS propositional resolution prover.
///
/// Returns `(all_clauses, all_sources, empty_clause_index)` on success, or
/// `None` if no refutation is found before `deadline` or within `MAX_DERIVED`
/// derived clauses.
fn prop_bfs_refute(input: &[PC], deadline: Instant) -> Option<(Vec<PC>, Vec<PSrc>, usize)> {
    let mut clauses: Vec<PC> = Vec::new();
    let mut sources: Vec<PSrc> = Vec::new();
    let mut seen: HashSet<PC> = HashSet::default();
    // Clauses containing a given literal. Scanning every earlier clause for
    // every head is quadratic in the clause count, and the multiset closure of a
    // small unsatisfiable instance routinely lands just *below* `MAX_DERIVED`,
    // so the scan -- not the cap -- is what decides how long this runs.
    // Indexing by literal makes the loop output-sensitive: only pairs that can
    // actually resolve are visited.
    let mut by_literal: HashMap<PL, Vec<usize>> = HashMap::default();

    let add = |clauses: &mut Vec<PC>,
               sources: &mut Vec<PSrc>,
               seen: &mut HashSet<PC>,
               by_literal: &mut HashMap<PL, Vec<usize>>,
               clause: PC,
               source: PSrc|
     -> bool {
        if !seen.insert(clause.clone()) {
            return false;
        }
        let is_empty = clause.is_empty();
        clauses.push(clause.clone());
        sources.push(source);
        let idx = clauses.len() - 1;
        // A repeated literal resolves against the same bucket once per
        // occurrence, which is exactly right: the two sides of that pivot
        // differ, so the resolvents differ.
        for lit in clause {
            by_literal.entry(lit).or_default().push(idx);
        }
        is_empty
    };

    // Load input clauses, deduplicating identical ones.
    for (i, c) in input.iter().enumerate() {
        if add(
            &mut clauses,
            &mut sources,
            &mut seen,
            &mut by_literal,
            c.clone(),
            PSrc::Input(i),
        ) {
            let idx = clauses.len() - 1;
            return Some((clauses, sources, idx));
        }
    }

    let mut head = 0;
    while head < clauses.len() {
        if Instant::now() >= deadline {
            return None;
        }
        // Clone to avoid holding borrows across the pushes below.
        let c_head = clauses[head].clone();
        for &lit in &c_head {
            // Copy the bucket: the pushes below append to the index.
            let Some(candidates) = by_literal.get(&-lit).cloned() else {
                continue;
            };
            // Only earlier clauses. A repeated pivot literal visits the same
            // bucket twice and recomputes the same resolvent, which `seen`
            // rejects; the clause's multiplicity is far too small for that to
            // matter.
            let mut since_check = 0usize;
            for j in candidates {
                if j >= head {
                    break;
                }
                since_check += 1;
                if since_check >= DEADLINE_CHECK_INTERVAL {
                    since_check = 0;
                    if Instant::now() >= deadline {
                        return None;
                    }
                }
                let c_j = clauses[j].clone();
                let Some(resolvent) = resolve_prop(&c_head, &c_j, lit) else {
                    continue;
                };
                if add(
                    &mut clauses,
                    &mut sources,
                    &mut seen,
                    &mut by_literal,
                    resolvent,
                    PSrc::Resolvent {
                        left: head,
                        right: j,
                        pivot: lit,
                    },
                ) {
                    let idx = clauses.len() - 1;
                    return Some((clauses, sources, idx));
                }
                if clauses.len() > MAX_DERIVED {
                    return None;
                }
            }
        }
        head += 1;
    }
    None // Saturated without empty clause (SAT or exceeded cap)
}

// ---------------------------------------------------------------------------
// First-order proof construction
// ---------------------------------------------------------------------------

/// Resolves two already-emitted first-order parents on the predicate the
/// skeleton step used, and returns the resolvent.
///
/// The resolvent is computed by the ordinary calculus rather than synthesized
/// from the skeleton clause, which is what makes each emitted step a real
/// resolution inference. That distinction is load-bearing. The skeleton step
/// knows only the predicate, so any first-order clause over the right
/// predicate multiset is a *consequence* of the parents; but only one of them
/// is their resolvent. Consider `~r(X,Y) | ~r(X,Y)` and `r(Z,Z)`: the skeleton
/// step leaves one `~r` literal, and lifting that to `~r(A,B)` over fresh
/// variables yields a clause that is a *generalization* of the actual resolvent
/// `~r(Z,Z)`. It still follows from the parents, so the refutation is sound —
/// but the proof cites an inference that did not happen, and an independent
/// checker rejects it. Computing the resolvent removes the whole class.
///
/// The parents' variables are made disjoint before unification (this is what
/// `mrs-calculus`'s resolver does), so two parents that both number their
/// first variable `0` do not get spuriously identified. Every clause here is
/// implicitly universally quantified, so that renaming is free.
fn fvo_resolve(
    left: &Clause,
    right: &Clause,
    pivot: PL,
    abs: &PropAbstraction,
    id_gen: &mut ClauseIdGen,
) -> Option<Clause> {
    let (sym, positive) = {
        let var_idx = (pivot.unsigned_abs() - 1) as usize;
        let (sym, _arity) = *abs.var_to_sym_arity.get(var_idx)?;
        (sym, pivot > 0)
    };
    let matches_pivot = |lit: &Literal, want_positive: bool| {
        lit.positive == want_positive && matches!(&lit.atom, Atom::Pred(s, _) if *s == sym)
    };
    let left_idx = left
        .literals
        .iter()
        .position(|l| matches_pivot(l, positive))?;
    let right_idx = right
        .literals
        .iter()
        .position(|l| matches_pivot(l, !positive))?;
    // The multiset invariant guarantees both parents carry the pivot, so a
    // missing literal means the invariant broke rather than that this step is
    // inapplicable.
    let mut resolvents = mrs_calculus::resolution::resolve_selected(
        left,
        right,
        id_gen,
        Some(std::slice::from_ref(&left_idx)),
        Some(std::slice::from_ref(&right_idx)),
        &HashSet::default(),
    );
    // At most one literal pair was selected, so this is the whole answer. The
    // atoms are tuples of distinct variables over one symbol, so unification
    // cannot fail unless the arity guard in `PropAbstraction::build` was
    // bypassed.
    resolvents.pop()
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// Orders the proof ancestors of `root` so that every parent precedes its
/// child.
///
/// Iterative rather than recursive: the clause vector can hold `MAX_DERIVED`
/// entries and a resolution chain can be as deep as it is long, which is more
/// than the main thread's stack should be asked to hold.
fn topo_order(root: usize, prop_sources: &[PSrc], deadline: Instant) -> Option<Vec<usize>> {
    let mut order: Vec<usize> = Vec::new();
    let mut visited: Vec<bool> = vec![false; prop_sources.len()];
    // (node, stage): stage 0 expands the node, stage 1 emits it, so the
    // emission marker is always popped after every child's subtree.
    let mut stack: Vec<(usize, u8)> = vec![(root, 0)];
    let mut visited_count = 0usize;
    while let Some((node, stage)) = stack.pop() {
        visited_count += 1;
        if visited_count.is_multiple_of(DEADLINE_CHECK_INTERVAL) && Instant::now() >= deadline {
            return None;
        }
        if stage == 1 {
            order.push(node);
            continue;
        }
        if visited[node] {
            continue;
        }
        visited[node] = true;
        stack.push((node, 1));
        if let PSrc::Resolvent { left, right, .. } = prop_sources[node] {
            stack.push((right, 0));
            stack.push((left, 0));
        }
    }
    (Instant::now() < deadline).then_some(order)
}

/// Wall-clock ceiling for the FVO pre-pass, as a share of the schedule budget.
///
/// The pre-pass is a fast path — on the problems it targets it finishes in
/// milliseconds — and it is the only pre-pass that runs before any portfolio
/// thread exists, so its cost is *added* to the run rather than taken out of
/// the strategies' own budgets. A hard UNSAT skeleton with a large resolution
/// closure is exactly the case that would otherwise be free to grind until
/// `MAX_DERIVED`, so the ceiling is what keeps the pre-pass a pre-pass. The
/// floor keeps short runs usable and the cap keeps long runs from spending
/// minutes here.
pub fn fvo_budget(total_budget: Duration) -> Duration {
    total_budget
        .checked_div(50)
        .unwrap_or(Duration::from_millis(500))
        .clamp(Duration::from_millis(200), Duration::from_secs(2))
}

/// Attempts to refute an FVO clause set using propositional skeleton resolution.
///
/// Returns `Some(SearchResult::Refutation(id, tstp))` if:
/// - the problem is FVO (see [`is_fvo_problem`]), **and**
/// - the propositional skeleton is UNSAT (confirmed by `mrs-cadical`), **and**
/// - a BFS resolution proof is found within `budget` and `MAX_DERIVED` derived
///   clauses.
///
/// Returns `None` in all other cases; the caller should try the regular
/// strategy schedule.
pub fn try_fvo_refutation(
    clauses: &[Clause],
    provenance: &[Clause],
    id_gen: &mut ClauseIdGen,
    symbols: &SymbolTable,
    budget: Duration,
) -> Option<SearchResult> {
    let deadline = Instant::now() + budget;
    if Instant::now() >= deadline {
        return None;
    }
    let abs = PropAbstraction::build(clauses, Some(deadline))?;

    // Fast oracle: use CaDiCaL to check propositional UNSAT before BFS.
    // This avoids O(n²) BFS work when the problem is actually satisfiable.
    {
        let mut solver = Solver::new();
        for (index, pc) in abs.prop_clauses.iter().enumerate() {
            if index.is_multiple_of(DEADLINE_CHECK_INTERVAL) && Instant::now() >= deadline {
                return None;
            }
            solver.add_clause(pc.as_slice());
        }
        // Duplicates are irrelevant to a SAT solver, so the multiset images
        // can go in as they are. This stays a sound gate: a skeleton that is
        // satisfiable as a clause *set* is also satisfiable as a multiset, so
        // `Sat`/`Unknown` really does mean there is no refutation to find. The
        // converse does not hold, which is why an `Unsat` verdict only lets the
        // BFS try; it never stands in for one.
        if solver.solve_until(deadline) != mrs_cadical::SolveResult::Unsat {
            return None;
        }
    }

    // BFS resolution prover: generate a step-by-step propositional proof.
    let (_prop_clauses, prop_sources, empty_idx) = prop_bfs_refute(&abs.prop_clauses, deadline)?;

    // Collect the proof ancestors in topological order (parents before children).
    let order = topo_order(empty_idx, &prop_sources, deadline)?;
    debug_assert_eq!(order.last(), Some(&empty_idx));

    // Build the first-order proof, resolving the emitted parents at each step so
    // that every node in the output is a real resolution inference.
    let mut fo_clauses: Vec<Option<Clause>> = vec![None; prop_sources.len()];
    let mut fof_proof: Vec<Clause> = Vec::with_capacity(provenance.len() + order.len());

    // Prepend provenance steps so the proof is fully self-contained back to the conjecture!
    fof_proof.extend(provenance.iter().cloned());

    for &prop_idx in &order {
        if Instant::now() >= deadline {
            return None;
        }
        let step = match &prop_sources[prop_idx] {
            PSrc::Input(input_idx) => {
                // Use the original FOF clause unchanged (preserves ClauseId and source).
                clauses[*input_idx].clone()
            }
            PSrc::Resolvent { left, right, pivot } => {
                let parent = |idx: usize| fo_clauses[idx].as_ref();
                // Both parents precede this step in `order`, so both are present.
                fvo_resolve(parent(*left)?, parent(*right)?, *pivot, &abs, id_gen)?
            }
        };
        fo_clauses[prop_idx] = Some(step.clone());
        fof_proof.push(step);
    }

    // The skeleton says this last clause is empty and the multiset invariant
    // says the first-order resolvent has the same predicate multiset, so it is
    // genuinely empty. Assert it rather than trust it: an emitted `$false` that
    // is not a refutation is the failure this whole module is arranged to avoid.
    let last = fof_proof.last()?;
    if !last.literals.is_empty() {
        debug_assert!(false, "FVO produced a non-empty final clause");
        return None;
    }

    if Instant::now() >= deadline {
        return None;
    }
    let tstp = format_tstp(&fof_proof, symbols);

    Some(SearchResult::Refutation(last.id, tstp))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
//
// The pre-pass's whole contract is "a refutation it reports is a refutation",
// and the only meaningful check of that is a checker that does not share its
// code or its author's reasoning. So the proof-lifting tests below run every
// emitted proof through `mrs-proof-kernel`, which replays each inference rule
// against the problem from scratch. A test that merely re-derives the
// expectation by hand would only confirm the bug it was written next to.
//
// The three properties under test are the ones the module docs claim and the
// ones the soundness argument turns on:
//
// * abstraction — the skeleton is a sound relaxation of the fragment,
// * variable freshness — every lifted atom's variables are locally fresh and
//   the clause is a specialization of whatever parent it came from,
// * proof lifting — every skeleton step is a real first-order resolution step.

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::SymbolTable;
    use mrs_core::clause::ClauseSource;
    use mrs_proof_kernel::{KernelVerdict, VerificationLimits, verify_strict};
    use mrs_tptp::parse_tptp;

    /// Generous enough that a deadline test cannot pass by accident.
    const PLENTY: Duration = Duration::from_secs(30);

    fn make_clause(id_gen: &mut ClauseIdGen, lits: Vec<Literal>, name: &str) -> Clause {
        make_clause_with_role(id_gen, lits, name, "negated_conjecture")
    }

    fn make_clause_with_role(
        id_gen: &mut ClauseIdGen,
        lits: Vec<Literal>,
        name: &str,
        role: &str,
    ) -> Clause {
        Clause::new(
            id_gen.next(),
            lits,
            ClauseSource::Input {
                name: name.to_string(),
                role: role.to_string(),
            },
        )
    }

    /// Runs the pre-pass and, when it claims a refutation, hands the proof to
    /// the independent kernel together with `problem`.
    ///
    /// Returns the refuted clause set's TSTP, or `None` if the pre-pass declined
    /// (non-FVO, satisfiable skeleton, no proof within budget).
    fn refute_and_verify(
        problem: &str,
        clauses: &[Clause],
        id_gen: &mut ClauseIdGen,
        syms: &SymbolTable,
        budget: Duration,
    ) -> Option<String> {
        let tstp = match try_fvo_refutation(clauses, &[], id_gen, syms, budget) {
            Some(SearchResult::Refutation(_, tstp)) => tstp,
            other => {
                assert!(
                    other.is_none(),
                    "FVO returned a non-refutation result: {other:?}"
                );
                return None;
            }
        };
        let parsed_problem =
            parse_tptp(problem).unwrap_or_else(|e| panic!("problem does not parse: {e}"));
        let parsed_proof =
            parse_tptp(&tstp).unwrap_or_else(|e| panic!("proof does not parse: {e}\n{tstp}"));
        let verdict = verify_strict(
            &parsed_problem,
            &parsed_proof,
            VerificationLimits::default(),
        );
        assert!(
            matches!(verdict, KernelVerdict::Certified),
            "the independent proof kernel rejected an FVO proof.\n\
             problem: {problem}\nproof:\n{tstp}\nverdict: {verdict:#?}"
        );
        Some(tstp)
    }

    // -- FVO detection ------------------------------------------------------

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

    // -- Abstraction: multiplicity ------------------------------------------

    /// A clause with a repeated predicate must not collapse to one skeleton
    /// literal. It is the one input shape where set semantics and first-order
    /// resolution disagree: the skeleton would see the unit `{p}`, resolve it
    /// away in one step with `~p`, and derive `$false` from parents whose real
    /// resolvent is `p(Y)`.
    #[test]
    fn fvo_skeleton_keeps_repeated_predicate_literals() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let mut id_gen = ClauseIdGen::new();
        let c1 = make_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(0)])),
                Literal::pos(Atom::pred(p, vec![Term::var(1)])),
            ],
            "c1",
        );
        let abs = PropAbstraction::build(std::slice::from_ref(&c1), None).expect("FVO");
        assert_eq!(
            abs.prop_clauses[0].len(),
            2,
            "p(X) | p(Y) must abstract to two skeleton literals, not one: {:?}",
            abs.prop_clauses[0]
        );
    }

    #[test]
    fn fvo_abstraction_respects_an_expired_deadline() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let clause = make_clause(
            &mut ClauseIdGen::new(),
            vec![Literal::pos(Atom::prop(p)); DEADLINE_CHECK_INTERVAL],
            "wide",
        );
        assert!(
            PropAbstraction::build(&[clause], Some(Instant::now() - Duration::from_secs(1)))
                .is_none()
        );
    }

    /// Resolution removes one pivot occurrence per side, so a repeated-predicate
    /// parent leaves its other occurrence behind.
    #[test]
    fn resolve_prop_removes_one_pivot_occurrence_per_side() {
        let resolvent = resolve_prop(&[1, 1, 2], &[-1], 1).expect("non-tautological");
        assert_eq!(resolvent, vec![1, 2]);
        assert_eq!(resolve_prop(&[1, 2], &[-1, 3], 1).expect("ok"), vec![2, 3]);
        // Tautologies are still rejected.
        assert!(resolve_prop(&[1, 2], &[-1, -2], 1).is_none());
    }

    /// The end-to-end shape of the multiplicity bug. Before the fix the
    /// skeleton was `{p}, {~p}` and one resolution step claimed `$false`; the
    /// kernel rejects that as "resolution conclusion is not a parent
    /// resolvent". The refutation is still available — the multiset skeleton is
    /// `{p,p}, {~p}` — it just takes the intermediate step through `p(Y)`.
    #[test]
    fn fvo_repeated_predicate_refutation_is_kernel_certified() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let mut id_gen = ClauseIdGen::new();
        let c1 = make_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(0)])),
                Literal::pos(Atom::pred(p, vec![Term::var(1)])),
            ],
            "c1",
        );
        let c2 = make_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::pred(p, vec![Term::var(0)]))],
            "c2",
        );
        assert!(is_fvo_problem(&[c1.clone(), c2.clone()]));
        let tstp = refute_and_verify(
            "cnf(c1, negated_conjecture, p(X) | p(Y)).\n\
             cnf(c2, negated_conjecture, ~p(Z)).\n",
            &[c1, c2],
            &mut id_gen,
            &syms,
            PLENTY,
        )
        .expect("the multiset skeleton still refutes this problem");
        assert_eq!(
            tstp.matches("inference(resolution").count(),
            2,
            "the refutation must go through p(Y) rather than leap to $false:\n{tstp}"
        );
    }

    /// A repeated predicate of *higher* arity, where the arguments differ. The
    /// skeleton still cannot tell the two occurrences apart, so this is the same
    /// hazard one arity up.
    #[test]
    fn fvo_repeated_binary_predicate_refutation_is_kernel_certified() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let mut id_gen = ClauseIdGen::new();
        let c1 = make_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(0), Term::var(1)])),
                Literal::pos(Atom::pred(p, vec![Term::var(1), Term::var(0)])),
            ],
            "c1",
        );
        let c2 = make_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::pred(
                p,
                vec![Term::var(0), Term::var(0)],
            ))],
            "c2",
        );
        refute_and_verify(
            "cnf(c1, negated_conjecture, p(X,Y) | p(Y,X)).\n\
             cnf(c2, negated_conjecture, ~p(Z,Z)).\n",
            &[c1, c2],
            &mut id_gen,
            &syms,
            PLENTY,
        )
        .expect("refutable");
    }

    /// The case that decides multiset over set, and the ordinary one: two parents
    /// share a *non-pivot* predicate, so the real resolvent is `q(X) | q(Y)` with a
    /// duplicated literal. A set skeleton drops one of them, the abstract step
    /// looks finished, and the next step resolves a clause that still has a `q`
    /// in it — so a set skeleton cannot realize this derivation at all and has to
    /// discard the proof. Keeping multiplicity resolves the duplicate away
    /// honestly and reaches `$false`.
    #[test]
    fn fvo_shared_non_pivot_predicate_refutation_is_kernel_certified() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let mut id_gen = ClauseIdGen::new();
        // forall x. p(x) | q(x);  forall x. ~p(x) | q(x);  forall x. ~q(x).
        // Unsatisfiable, and every input clause is literal-distinct.
        let c1 = make_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(0)])),
                Literal::pos(Atom::pred(q, vec![Term::var(0)])),
            ],
            "c1",
        );
        let c2 = make_clause(
            &mut id_gen,
            vec![
                Literal::neg(Atom::pred(p, vec![Term::var(0)])),
                Literal::pos(Atom::pred(q, vec![Term::var(0)])),
            ],
            "c2",
        );
        let c3 = make_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::pred(q, vec![Term::var(0)]))],
            "c3",
        );
        let tstp = refute_and_verify(
            "cnf(c1, negated_conjecture, p(X) | q(X)).\n\
             cnf(c2, negated_conjecture, ~p(X) | q(X)).\n\
             cnf(c3, negated_conjecture, ~q(X)).\n",
            &[c1, c2, c3],
            &mut id_gen,
            &syms,
            PLENTY,
        )
        .expect("the multiset skeleton refutes this; a set skeleton cannot");
        assert!(
            !tstp.contains("subsumption"),
            "the pre-pass should refute this itself, not leave it to the search:\n{tstp}"
        );
    }

    // -- Abstraction: arity -------------------------------------------------

    /// A predicate used at two arities collapses to one skeleton variable, so
    /// the lift pairs a 0-ary atom with a 1-ary one. Those do not unify, and
    /// the emitted step is not a resolution inference at all. Rejecting the
    /// problem up front is the only honest response: such a signature is
    /// ill-formed TPTP, but the parser accepts it, and before this check `mrs`
    /// reported `Unsatisfiable` with `$false` derived from `p` and `~p(X)`.
    #[test]
    fn fvo_rejects_predicate_used_at_two_arities() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let mut id_gen = ClauseIdGen::new();
        let c1 = make_clause(&mut id_gen, vec![Literal::pos(Atom::prop(p))], "c1");
        let c2 = make_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::pred(p, vec![Term::var(0)]))],
            "c2",
        );
        // Each clause on its own is FVO; the pair is not.
        assert!(is_fvo_clause(&c1));
        assert!(is_fvo_clause(&c2));
        assert!(!is_fvo_problem(&[c1.clone(), c2.clone()]));
        assert!(PropAbstraction::build(&[c1, c2], None).is_none());
        assert!(
            try_fvo_refutation(
                &[
                    make_clause(&mut id_gen, vec![Literal::pos(Atom::prop(p))], "c1"),
                    make_clause(
                        &mut id_gen,
                        vec![Literal::neg(Atom::pred(p, vec![Term::var(0)]))],
                        "c2",
                    ),
                ],
                &[],
                &mut id_gen,
                &syms,
                PLENTY,
            )
            .is_none()
        );
    }

    #[test]
    fn fvo_rejects_mixed_arity_one_and_two() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let mut id_gen = ClauseIdGen::new();
        let c1 = make_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::var(0)]))],
            "c1",
        );
        let c2 = make_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::pred(
                p,
                vec![Term::var(0), Term::var(1)],
            ))],
            "c2",
        );
        assert!(!is_fvo_problem(&[c1, c2]));
    }

    // -- Abstraction: differential sweep ------------------------------------

    /// A generator over small FVO clause sets, biased towards the shapes that
    /// separate the propositional layer from first-order resolution: repeated
    /// predicates, repeated argument positions, and a predicate at arity 2
    /// where argument *order* carries information the skeleton discards.
    struct InstanceGen {
        state: u64,
        syms: SymbolTable,
        preds: [SymbolId; 3],
    }

    /// The argument patterns a literal can be drawn from. A pattern is a whole
    /// argument tuple, so `[X]` and `[Y]` are distinguishable, as are `[X, X]`
    /// and `[X, Y]`.
    const ARGS: [&[u32]; 5] = [&[], &[0], &[1], &[0, 0], &[0, 1]];
    /// Argument patterns each predicate may use. Arity 2 only for `r`, so the
    /// generated sets mix arities across *different* symbols and never produce
    /// the ill-formed same-symbol-two-arities case, which has its own test.
    const PREDS_MAY_USE: [&[usize]; 3] = [&[0, 1, 2], &[0, 1, 2], &[3, 4]];

    /// One literal as `(predicate index, argument pattern, polarity)`.
    type LitSpec = (usize, usize, bool);

    impl InstanceGen {
        fn new(seed: u64) -> Self {
            let mut syms = SymbolTable::new();
            let preds = [syms.intern("p"), syms.intern("q"), syms.intern("r")];
            Self {
                state: seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1,
                syms,
                preds,
            }
        }

        fn next(&mut self) -> u64 {
            // xorshift64*, so the sweep is reproducible without a dependency.
            let mut x = self.state;
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            self.state = x;
            x.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }

        fn below(&mut self, n: u64) -> usize {
            (self.next() % n) as usize
        }

        fn lit(&mut self) -> LitSpec {
            let pred = self.below(3);
            let pattern = PREDS_MAY_USE[pred][self.below(PREDS_MAY_USE[pred].len() as u64)];
            (pred, pattern, self.below(2) == 0)
        }

        /// Draws one clause of 1..=3 literals. A third of the time the first
        /// literal is drawn twice, so repeated predicates -- the shapes where
        /// the propositional layer and first-order resolution disagree -- are
        /// common rather than incidental.
        fn clause(&mut self) -> Vec<LitSpec> {
            let mut lits = vec![self.lit()];
            if self.below(3) == 0 {
                lits.push(lits[0]);
            }
            for _ in 0..self.below(3) {
                lits.push(self.lit());
            }
            lits
        }

        /// Draws a single literal, for use as a unit clause.
        fn unit(&mut self) -> Vec<LitSpec> {
            vec![self.lit()]
        }

        /// Builds the clause objects and the matching problem text from the
        /// same specs, so the two cannot drift apart.
        fn make(&self, id_gen: &mut ClauseIdGen, specs: &[Vec<LitSpec>]) -> (Vec<Clause>, String) {
            let names = ["p", "q", "r"];
            let mut clauses = Vec::new();
            let mut text = String::new();
            for (i, specs) in specs.iter().enumerate() {
                let name = format!("c{i}");
                let lits: Vec<Literal> = specs
                    .iter()
                    .map(|&(pred, pattern, positive)| {
                        let atom = Atom::pred(
                            self.preds[pred],
                            ARGS[pattern].iter().map(|v| Term::var(*v)).collect(),
                        );
                        if positive {
                            Literal::pos(atom)
                        } else {
                            Literal::neg(atom)
                        }
                    })
                    .collect();
                let rendered: Vec<String> = specs
                    .iter()
                    .map(|&(pred, pattern, positive)| {
                        let args: Vec<&str> = ARGS[pattern]
                            .iter()
                            .map(|v| if *v == 0 { "X" } else { "Y" })
                            .collect();
                        let atom = if args.is_empty() {
                            names[pred].to_string()
                        } else {
                            format!("{}({})", names[pred], args.join(","))
                        };
                        if positive { atom } else { format!("~{atom}") }
                    })
                    .collect();
                text.push_str(&format!(
                    "cnf({name}, negated_conjecture, {}).\n",
                    rendered.join(" | ")
                ));
                clauses.push(make_clause(id_gen, lits, &name));
            }
            (clauses, text)
        }
    }

    /// Every refutation the pre-pass reports has to survive the independent
    /// kernel. This is the differential test for the abstraction: it does not
    /// re-derive what the pre-pass ought to find, it only insists that whatever
    /// it does find is a real first-order refutation of the same problem.
    #[test]
    fn fvo_emitted_proofs_are_kernel_certified() {
        const INSTANCES: u64 = 600;
        let mut refuted = 0usize;
        let mut steps = 0usize;
        let mut insts = InstanceGen::new(0xF0F0);
        for _ in 0..INSTANCES {
            // Half the instances get a guaranteed contradiction so the sweep
            // spends its budget on proof *shape* rather than on hunting for
            // unsatisfiable draws, and half are left to chance, which is where
            // the deep accidental cases turn up.
            let mut specs: Vec<Vec<LitSpec>> =
                (0..2 + insts.below(3)).map(|_| insts.clause()).collect();
            if insts.below(2) == 0 {
                let u = insts.unit();
                let mut negated = u.clone();
                for lit in &mut negated {
                    lit.2 = !lit.2;
                }
                specs.push(u);
                specs.push(negated);
            }
            let mut id_gen = ClauseIdGen::new();
            let (clauses, text) = insts.make(&mut id_gen, &specs);
            if !is_fvo_problem(&clauses) {
                continue;
            }
            if let Some(tstp) = refute_and_verify(&text, &clauses, &mut id_gen, &insts.syms, PLENTY)
            {
                refuted += 1;
                steps += tstp.matches("inference(resolution").count();
            }
        }
        assert!(
            refuted >= INSTANCES as usize / 8,
            "only {refuted} of {INSTANCES} instances were refuted; the sweep is \
             not exercising the pre-pass"
        );
        assert!(
            steps >= refuted,
            "only {steps} resolution steps across {refuted} refutations; the \
             sweep is not reaching multi-step proofs"
        );
    }

    // -- Proof lifting: variable freshness ----------------------------------

    /// Clauses are implicitly universally quantified, so two parents reuse
    /// variable indices for unrelated variables, and unification has to see
    /// disjoint sets before it runs. `p(X,Y) | q(X)` with `~p(Y,X) | r(Y)` is
    /// the case that bites: on the raw numbering the pivot atoms are `p(0,1)`
    /// and `p(1,0)`, whose unifier would have to bind `0 -> 1` and `1 -> 0` at
    /// once, whereas with the right parent shifted first it is an ordinary
    /// unification.
    ///
    /// The end-to-end version of this is [`fvo_aliased_parents_refutation_is_kernel_certified`];
    /// here the point is only that the step resolves at all, and to the shape
    /// the pivot leaves behind.
    #[test]
    fn fvo_resolve_unifies_pivots_across_aliased_parents() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let r = syms.intern("r");
        let mut id_gen = ClauseIdGen::new();
        let abs = PropAbstraction {
            prop_clauses: vec![],
            var_to_sym_arity: vec![(p, 2), (q, 1), (r, 1)],
        };
        let left = make_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(0), Term::var(1)])),
                Literal::pos(Atom::pred(q, vec![Term::var(0)])),
            ],
            "left",
        );
        let right = make_clause(
            &mut id_gen,
            vec![
                Literal::neg(Atom::pred(p, vec![Term::var(1), Term::var(0)])),
                Literal::pos(Atom::pred(r, vec![Term::var(1)])),
            ],
            "right",
        );
        let resolvent = fvo_resolve(&left, &right, 1, &abs, &mut id_gen)
            .expect("the pivots unify once the parents are made disjoint");
        // The pivot is gone from both sides and one literal per parent survives.
        assert_eq!(resolvent.literals.len(), 2, "{:?}", resolvent.literals);
        let mut preds: Vec<SymbolId> = resolvent
            .literals
            .iter()
            .map(|lit| match &lit.atom {
                Atom::Pred(s, args) => {
                    assert!(
                        args.iter().all(|a| matches!(a, Term::Var(_))),
                        "an FVO clause must have only variable arguments"
                    );
                    *s
                }
                Atom::Eq(_, _) => panic!("equality in an FVO clause"),
            })
            .collect();
        preds.sort();
        let mut expected = vec![q, r];
        expected.sort();
        assert_eq!(preds, expected, "{:?}", resolvent.literals);
    }

    /// End-to-end version of the aliasing case, with a contradiction to close.
    /// Both parents number their first variable `0`, so this only works if the
    /// resolver shifts one of them before unifying.
    #[test]
    fn fvo_aliased_parents_refutation_is_kernel_certified() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let mut id_gen = ClauseIdGen::new();
        let c1 = make_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(0), Term::var(1)])),
                Literal::pos(Atom::pred(q, vec![Term::var(0)])),
            ],
            "c1",
        );
        let c2 = make_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::pred(
                p,
                vec![Term::var(1), Term::var(0)],
            ))],
            "c2",
        );
        let c3 = make_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::pred(q, vec![Term::var(0)]))],
            "c3",
        );
        refute_and_verify(
            "cnf(c1, negated_conjecture, p(X,Y) | q(X)).\n\
             cnf(c2, negated_conjecture, ~p(Y,X)).\n\
             cnf(c3, negated_conjecture, ~q(X)).\n",
            &[c1, c2, c3],
            &mut id_gen,
            &syms,
            PLENTY,
        )
        .expect("refutable");
    }

    /// The generalization bug, end to end, on the instance the sweep found.
    ///
    /// `~r(X,Y) | ~r(X,Y)` is a first-order clause with a repeated *atom*, so
    /// its two literals share variables, and resolving it with `r(X,X)` binds
    /// those variables: the real resolvent is `~r(X,X)`. The skeleton step only
    /// sees that one `~r` literal survives, and synthesizing that literal from
    /// the skeleton over fresh variables gives `~r(A,B)` instead. That is still
    /// a consequence of the parents — it is a generalization — so the
    /// refutation is sound, but the proof records an inference that never
    /// happened and the kernel rejects it with "resolution conclusion is not a
    /// parent resolvent". Computing the resolvent from the emitted parents is
    /// what fixes it.
    #[test]
    fn fvo_repeated_atom_refutation_is_kernel_certified() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let r = syms.intern("r");
        let mut id_gen = ClauseIdGen::new();
        let c0 = make_clause(
            &mut id_gen,
            vec![
                Literal::neg(Atom::pred(r, vec![Term::var(0), Term::var(1)])),
                Literal::neg(Atom::pred(r, vec![Term::var(0), Term::var(1)])),
            ],
            "c0",
        );
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
            vec![Literal::pos(Atom::pred(p, vec![Term::var(0)]))],
            "c2",
        );
        let c3 = make_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(
                r,
                vec![Term::var(0), Term::var(0)],
            ))],
            "c3",
        );
        refute_and_verify(
            "cnf(c0, negated_conjecture, ~r(X,Y) | ~r(X,Y)).\n\
             cnf(c1, negated_conjecture, p(X) | q(Y)).\n\
             cnf(c2, negated_conjecture, p(X)).\n\
             cnf(c3, negated_conjecture, r(X,X)).\n",
            &[c0, c1, c2, c3],
            &mut id_gen,
            &syms,
            PLENTY,
        )
        .expect("refutable");
    }

    /// The other direction of the same property: when a surviving literal shares
    /// variables with the pivot, the MGU must reach it. `~r(X,Y) | ~r(X,Y)` with
    /// `r(Z,Z)` resolves to `~r(Z,Z)`, not to `~r(A,B)` -- the skeleton step only
    /// knows the predicate, so it cannot tell the two apart. A clause
    /// generalized out of the resolvent is still a consequence the parents
    /// entail, but it records an inference that never happened.
    #[test]
    fn fvo_resolve_applies_the_unifier_to_surviving_literals() {
        let mut syms = SymbolTable::new();
        let r = syms.intern("r");
        let mut id_gen = ClauseIdGen::new();
        let abs = PropAbstraction {
            prop_clauses: vec![],
            var_to_sym_arity: vec![(r, 2)],
        };
        let left = make_clause(
            &mut id_gen,
            vec![
                Literal::neg(Atom::pred(r, vec![Term::var(0), Term::var(1)])),
                Literal::neg(Atom::pred(r, vec![Term::var(0), Term::var(1)])),
            ],
            "left",
        );
        let right = make_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(
                r,
                vec![Term::var(2), Term::var(2)],
            ))],
            "right",
        );
        let resolvent = fvo_resolve(&left, &right, -1, &abs, &mut id_gen).expect("resolvable");
        assert_eq!(resolvent.literals.len(), 1, "{:?}", resolvent.literals);
        let Atom::Pred(_, args) = &resolvent.literals[0].atom else {
            panic!("expected a predicate atom")
        };
        let bound: Vec<u32> = args
            .iter()
            .map(|a| match a {
                Term::Var(v) => *v,
                other => panic!("non-variable argument: {other:?}"),
            })
            .collect();
        assert_eq!(
            bound[0], bound[1],
            "the surviving literal must keep the unifier's binding, not be \
             re-generalized: {bound:?}"
        );
    }

    // -- Budget -------------------------------------------------------------

    /// The pre-pass runs before any portfolio thread exists, so its cost is
    /// added to the run rather than taken from the strategies' budgets. An
    /// already-expired budget has to stop it dead, even on a problem it would
    /// otherwise refute in microseconds.
    #[test]
    fn fvo_expired_budget_gives_up() {
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
        let clauses = [c1, c2, c3];
        assert!(
            try_fvo_refutation(&clauses, &[], &mut id_gen, &syms, Duration::ZERO).is_none(),
            "an expired budget must stop the pre-pass before it claims a refutation"
        );
        // ...and the same problem still refutes with a real budget.
        assert!(matches!(
            try_fvo_refutation(&clauses, &[], &mut id_gen, &syms, PLENTY),
            Some(SearchResult::Refutation(..))
        ));
    }

    #[test]
    fn fvo_budget_is_a_small_bounded_share() {
        assert_eq!(
            fvo_budget(Duration::from_secs(30)),
            Duration::from_millis(600)
        );
        // A short run still gets a usable slice; a long one is capped.
        assert_eq!(
            fvo_budget(Duration::from_millis(100)),
            Duration::from_millis(200)
        );
        assert_eq!(
            fvo_budget(Duration::from_secs(3600)),
            Duration::from_secs(2)
        );
    }

    // -- Plain behaviour ----------------------------------------------------

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
        let result = try_fvo_refutation(&[c1, c2, c3], &[], &mut id_gen, &syms, PLENTY);
        assert!(
            matches!(result, Some(SearchResult::Refutation(..))),
            "expected Refutation, got {:?}",
            result
        );
    }

    #[test]
    fn fvo_simple_refutation_is_kernel_certified() {
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
        refute_and_verify(
            "cnf(c1, negated_conjecture, p(X) | q(Y)).\n\
             cnf(c2, negated_conjecture, ~p(X)).\n\
             cnf(c3, negated_conjecture, ~q(X)).\n",
            &[c1, c2, c3],
            &mut id_gen,
            &syms,
            PLENTY,
        )
        .expect("refutable");
    }

    /// Nullary predicates — a clause's arguments are vacuously all variables, so
    /// a propositional problem written as 0-arity FOF predicates is FVO and the
    /// abstraction is the entire formula. This is the only shape that reaches the
    /// pre-pass on any input in this repository, so it is worth pinning
    /// separately from the arity-1 cases above.
    #[test]
    fn fvo_nullary_predicates_refutation_is_kernel_certified() {
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
        refute_and_verify(
            "cnf(c1, negated_conjecture, p | q).\n\
             cnf(c2, negated_conjecture, ~p).\n\
             cnf(c3, negated_conjecture, ~q).\n",
            &[c1, c2, c3],
            &mut id_gen,
            &syms,
            PLENTY,
        )
        .expect("refutable");
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
        let result = try_fvo_refutation(&[c1], &[], &mut id_gen, &syms, PLENTY);
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
        let result = try_fvo_refutation(&[c1], &[], &mut id_gen, &syms, PLENTY);
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
            try_fvo_refutation(&[c1, c2, c3], &[], &mut id_gen, &syms, PLENTY)
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

    /// A skeleton tautology must not be used as a proof step, and a skeleton
    /// that is satisfiable must not be refuted. Both matter because the BFS
    /// keeps input clauses verbatim, including tautological ones.
    #[test]
    fn fvo_tautological_input_clause_is_not_a_shortcut() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let mut id_gen = ClauseIdGen::new();
        // p(X) | ~p(Y) is a first-order tautology and a skeleton tautology.
        let taut = make_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(0)])),
                Literal::neg(Atom::pred(p, vec![Term::var(1)])),
            ],
            "c1",
        );
        let c2 = make_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(p, vec![Term::var(0)]))],
            "c2",
        );
        // A tautology plus one positive unit is satisfiable.
        assert!(
            try_fvo_refutation(&[taut.clone(), c2.clone()], &[], &mut id_gen, &syms, PLENTY)
                .is_none(),
            "a satisfiable skeleton must not be refuted"
        );
        // With a contradicting unit it is refutable, and the proof must be real.
        let c3 = make_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::pred(p, vec![Term::var(0)]))],
            "c3",
        );
        refute_and_verify(
            "cnf(c1, negated_conjecture, p(X) | ~p(Y)).\n\
             cnf(c2, negated_conjecture, p(X)).\n\
             cnf(c3, negated_conjecture, ~p(X)).\n",
            &[taut, c2, c3],
            &mut id_gen,
            &syms,
            PLENTY,
        )
        .expect("refutable");
    }
}
