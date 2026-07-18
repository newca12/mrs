//! Componentwise AVATAR (CWA): refutation by independent sub-searches.
//!
//! Some problems produced by definitional CNF have the structure of a single
//! large positive disjunction `def_1(X̄₁) ∨ def_2(X̄₂) ∨ ... ∨ def_N(X̄_N)` whose
//! literals are variable-disjoint, together with a set of "definition clauses"
//! `~def_k(X̄) ∨ body_kᵢ` encoding each disjunct's body.
//!
//! Standard AVATAR splits the top clause into N components but then runs a
//! single shared given-clause loop where all N sub-problems compete for the
//! same passive set.  For problems like SYN938+1 this collapses the search.
//!
//! **Componentwise AVATAR** detects this pattern and runs N independent
//! sub-searches: for each `def_k`, it gathers the transitively reachable
//! definition clauses (those mentioning `def_k`, plus their transitive
//! dependencies on other `def_j`), adds `def_k(X̄)` as a positive unit, and
//! runs a focused proof search.  If every sub-search refutes, the original
//! problem is UNSAT.
//!
//! **Soundness**.  Let `T = def_1 ∨ ... ∨ def_N` be the top clause and let
//! `Dₖ = {Dₖ,ᵢ}` be the definition clauses for `def_k`.  The input set is
//! `T ∧ ⋀ₖ Dₖ`.  A model satisfies `T` iff some `def_k` is true; then `Dₖ`
//! forces the body of conjunct `k` to hold.  UNSAT therefore means that
//! `{def_k(X̄)} ∪ Dₖ ∪ (other Dⱼ reachable from Dₖ)` is UNSAT for every k.
//! Componentwise refutation establishes exactly this fact and is sound.
//!
//! **Conservatism**.  The pattern detector is intentionally narrow.  CWA only
//! fires when (a) one clause has ≥ `MIN_BRANCHES` literals and (b) each of
//! its literals is a `Pred` atom with a distinct predicate symbol.  When the
//! pattern doesn't match, `try_componentwise_refute` returns `None` and the
//! caller falls through to the regular strategy schedule.

use crate::{HashMap, HashSet};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use mrs_calculus::ordering::SymbolConfig;
use mrs_core::SymbolId;
use mrs_core::SymbolTable;
use mrs_core::clause::{Clause, ClauseIdGen, ClauseSource};
use mrs_core::formula::Atom;
use mrs_proof::extract::extract_proof;
use mrs_proof::tstp::format_tstp;

use crate::given_clause::search;
use crate::state::SearchState;
use crate::{LiteralSelection, SearchConfig, SearchResult, SelectionStrategy, TermOrdering};

/// Minimum number of literals in a candidate top-disjunction clause.
const MIN_BRANCHES: usize = 5;

/// Per-branch wall-clock budget (must total well under the schedule budget).
const PER_BRANCH_TIME: Duration = Duration::from_millis(500);

/// Per-branch passive-set growth cap (sub-search weight limit).
const PER_BRANCH_WEIGHT: u32 = 50;

// ---------------------------------------------------------------------------
// Pattern detection
// ---------------------------------------------------------------------------

/// A detected "definitional CNF top disjunction".
struct TopDisjunction {
    /// Index of the top clause within the input slice.
    top_idx: usize,
    /// The predicate symbols of each disjunct, in order.  All distinct.
    branch_predicates: Vec<SymbolId>,
}

/// Collects the set of predicate symbols mentioned by a clause's literals.
fn clause_predicate_symbols(clause: &Clause) -> HashSet<SymbolId> {
    let mut out = HashSet::default();
    for lit in &clause.literals {
        if let Atom::Pred(sym, _) = &lit.atom {
            out.insert(*sym);
        }
    }
    out
}

/// Looks for a single clause `T` in the input that matches the pattern:
/// - has ≥ `MIN_BRANCHES` literals,
/// - every literal is a `Pred(p_k, args_k)` atom, positive or negative (the
///   polarity is preserved verbatim into the branch's unit clause by
///   `make_branch_unit` — see its doc comment for why flipping it would be
///   unsound),
/// - all predicate symbols `p_k` are distinct,
/// - literals are pairwise variable-disjoint.
///
/// If multiple clauses qualify, returns the one with the most literals.
fn detect_top_disjunction(clauses: &[Clause]) -> Option<TopDisjunction> {
    let mut best: Option<TopDisjunction> = None;
    for (idx, clause) in clauses.iter().enumerate() {
        if clause.literals.len() < MIN_BRANCHES {
            continue;
        }
        // All literals must be Pred with distinct predicate symbols.
        // Literals must also be pairwise variable-disjoint to ensure soundness
        // when split into independent universally-quantified branches.
        let mut preds: Vec<SymbolId> = Vec::with_capacity(clause.literals.len());
        let mut seen_preds: HashSet<SymbolId> = HashSet::default();
        let mut seen_vars: HashSet<mrs_core::term::VarId> = HashSet::default();
        let mut ok = true;
        let mut why_rejected = "";
        for lit in &clause.literals {
            let mut lit_vars = HashSet::default();
            lit.collect_vars(&mut lit_vars);
            for v in lit_vars {
                if !seen_vars.insert(v) {
                    ok = false;
                    why_rejected = "shared variable across literals";
                    break;
                }
            }
            if !ok {
                break;
            }

            match &lit.atom {
                Atom::Pred(sym, _) => {
                    if !seen_preds.insert(*sym) {
                        ok = false;
                        why_rejected = "duplicate predicate";
                        break;
                    }
                    preds.push(*sym);
                }
                Atom::Eq(_, _) => {
                    ok = false;
                    why_rejected = "equality literal";
                    break;
                }
            }
        }
        if !ok {
            if std::env::var("TRACE_CWA").is_ok() && clause.literals.len() >= MIN_BRANCHES {
                eprintln!(
                    "[CWA] reject clause {} ({} lits): {}",
                    clause.id.0,
                    clause.literals.len(),
                    why_rejected
                );
            }
            continue;
        }
        let candidate = TopDisjunction {
            top_idx: idx,
            branch_predicates: preds,
        };
        if std::env::var("TRACE_CWA").is_ok() {
            eprintln!(
                "[CWA] candidate clause {} with {} branches",
                clause.id.0,
                candidate.branch_predicates.len()
            );
        }
        match &best {
            None => best = Some(candidate),
            Some(b) if candidate.branch_predicates.len() > b.branch_predicates.len() => {
                best = Some(candidate);
            }
            _ => {}
        }
    }
    best
}

// ---------------------------------------------------------------------------
// Per-branch clause extraction
// ---------------------------------------------------------------------------

/// Returns, for each branch predicate `def_k`, the indices of input clauses
/// transitively reachable from `def_k` through "shared predicate symbol"
/// edges.  Includes the unit fact `def_k(X̄)` (as a positive literal copy of
/// the original top clause's k-th literal).
///
/// The transitive closure is computed once over the full set of definition
/// clauses (all input clauses except the top clause itself, restricted to
/// those whose predicate set intersects the set of branch predicates).
fn extract_branch_clauses(top: &TopDisjunction, clauses: &[Clause]) -> Vec<Vec<usize>> {
    let branch_pred_set: HashSet<SymbolId> = top.branch_predicates.iter().copied().collect();

    // Definition clauses: index → set of branch predicates mentioned.
    let mut def_clauses: Vec<(usize, HashSet<SymbolId>)> = Vec::new();
    // Other (axiom) clauses: not the top, not mentioning any branch predicate.
    let mut other_clauses: Vec<usize> = Vec::new();
    for (i, c) in clauses.iter().enumerate() {
        if i == top.top_idx {
            continue;
        }
        let preds = clause_predicate_symbols(c);
        let branch_preds: HashSet<SymbolId> =
            preds.intersection(&branch_pred_set).copied().collect();
        if branch_preds.is_empty() {
            other_clauses.push(i);
        } else {
            def_clauses.push((i, branch_preds));
        }
    }

    // For each branch predicate, collect indices of definition clauses
    // mentioning it (direct).  Then BFS transitively through the
    // predicate-sharing graph.
    let mut pred_to_clauses: HashMap<SymbolId, Vec<usize>> = HashMap::default();
    for (idx, preds) in &def_clauses {
        for p in preds {
            pred_to_clauses.entry(*p).or_default().push(*idx);
        }
    }

    let mut result: Vec<Vec<usize>> = Vec::with_capacity(top.branch_predicates.len());
    for &start_pred in &top.branch_predicates {
        let mut included: HashSet<usize> = HashSet::default();
        let mut frontier: VecDeque<SymbolId> = VecDeque::new();
        let mut seen_preds: HashSet<SymbolId> = HashSet::default();
        frontier.push_back(start_pred);
        seen_preds.insert(start_pred);

        while let Some(p) = frontier.pop_front() {
            if let Some(idxs) = pred_to_clauses.get(&p) {
                for &idx in idxs {
                    if included.insert(idx) {
                        // Add all branch predicates of this clause to the frontier.
                        let preds = &def_clauses
                            .iter()
                            .find(|(i, _)| *i == idx)
                            .map(|(_, p)| p.clone())
                            .unwrap_or_default();
                        for &q in preds {
                            if seen_preds.insert(q) {
                                frontier.push_back(q);
                            }
                        }
                    }
                }
            }
        }

        let mut branch: Vec<usize> = included.into_iter().collect();
        branch.sort();
        // Prepend other (axiom) clauses too.
        let mut all = other_clauses.clone();
        all.extend(branch);
        result.push(all);
    }
    result
}

/// Builds the unit clause `def_k(X̄)` (or `~def_k(X̄)`) from the k-th literal
/// of the top clause, **preserving its original polarity**.
///
/// Forcing the literal positive here is unsound: if `T = ... ∨ ~def_k(X̄) ∨
/// ...`, a model satisfying `T` via this disjunct has `def_k(X̄)` *false*, so
/// the branch must attempt to refute `{~def_k(X̄)} ∪ Dₖ`, not `{def_k(X̄)} ∪
/// Dₖ`. Silently flipping the sign here would search for (and potentially
/// "refute") an entirely different, unrelated branch, which is exactly the
/// class of bug that produced the false `Theorem` verdict on `PRO013+3.p`.
fn make_branch_unit(top_clause: &Clause, k: usize, id_gen: &mut ClauseIdGen) -> Clause {
    let branch_lit = top_clause.literals[k].clone();
    Clause::new(
        id_gen.next(),
        vec![branch_lit],
        ClauseSource::Inference {
            rule: "split_component",
            parents: vec![top_clause.id].into(),
        },
    )
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// Attempts a componentwise refutation.
///
/// Returns:
/// - `Some(SearchResult::Refutation)` if every branch refutes.
/// - `Some(SearchResult::Saturated)` if any branch genuinely saturates
///   without weight bound (currently unreachable because we always use a
///   weight bound; reserved for future use).
/// - `None` if the input doesn't match the pattern, any branch times out, or
///   any branch saturates under the weight bound (which is incomplete).
///
/// The TSTP proof string returned on refutation simply concatenates the
/// per-branch proofs with explanatory comments — it is informational only.
pub fn try_componentwise_refute(
    clauses: &[Clause],
    id_gen: &mut ClauseIdGen,
    symbols: std::sync::Arc<SymbolTable>,
    sym_config: Arc<SymbolConfig>,
) -> Option<SearchResult> {
    let top = detect_top_disjunction(clauses)?;
    let n = top.branch_predicates.len();
    let branches = extract_branch_clauses(&top, clauses);

    if std::env::var("TRACE_CWA").is_ok() {
        eprintln!(
            "[CWA] detected top disjunction with {} branches at idx {}",
            n, top.top_idx
        );
    }

    let top_clause = &clauses[top.top_idx];

    let sub_config = SearchConfig {
        time_limit: PER_BRANCH_TIME,
        selection: SelectionStrategy::AgeWeight(5),
        literal_selection: LiteralSelection::All,
        ordering: TermOrdering::CustomKBO(sym_config.clone()),
        max_term_weight: Some(PER_BRANCH_WEIGHT),
        use_avatar: false,
        unit_only_resolution: false,
        ..SearchConfig::default()
    };

    let mut proof_parts: Vec<String> = Vec::with_capacity(n);

    #[allow(clippy::needless_range_loop)]
    for k in 0..n {
        let mut branch_clauses: Vec<Clause> =
            branches[k].iter().map(|&i| clauses[i].clone()).collect();
        let unit = make_branch_unit(top_clause, k, id_gen);
        branch_clauses.push(unit);

        let mut state = SearchState::new(
            branch_clauses,
            id_gen.clone(),
            sym_config.clone(),
            symbols.clone(),
            false,
        );
        let result = search(&mut state, &sub_config);

        // Advance the caller's id_gen past any IDs used by this sub-search.
        *id_gen = state.id_gen.clone();

        if std::env::var("TRACE_CWA").is_ok() {
            let outcome = match &result {
                SearchResult::Refutation(..) => "Refutation",
                SearchResult::Saturated => "Saturated",
                SearchResult::Timeout => "Timeout",
                SearchResult::GaveUp => "GaveUp",
            };
            eprintln!(
                "[CWA] branch {}/{} ({:?}, {} clauses): {}",
                k + 1,
                n,
                top.branch_predicates[k],
                branches[k].len() + 1,
                outcome
            );
        }

        match result {
            SearchResult::Refutation(empty_id, _) => {
                // Convert IdClause store → legacy Clause store for proof extraction
                let legacy_store: HashMap<_, _> = state
                    .clause_store
                    .iter()
                    .map(|(&cid, ic)| (cid, state.term_bank.clause_to_legacy(ic)))
                    .collect();
                let proof = extract_proof(empty_id, &legacy_store);
                let tstp = format_tstp(&proof, &symbols);
                proof_parts.push(format!(
                    "% --- componentwise branch {}/{}: predicate {:?} ---\n{}",
                    k + 1,
                    n,
                    top.branch_predicates[k],
                    tstp
                ));
            }
            // Any non-refutation outcome aborts CWA: if a single branch
            // doesn't refute within its budget we cannot conclude UNSAT.
            _ => return None,
        }
    }

    let combined = proof_parts.join("\n");
    // Reuse the top clause's id as the "empty clause" id for the overall
    // refutation: this is just a placeholder for SearchResult::Refutation;
    // the actual chain of evidence is in the TSTP string.
    Some(SearchResult::Refutation(top_clause.id, combined))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::SymbolTable;
    use mrs_core::clause::{ClauseIdGen, Literal};
    use mrs_core::term::Term;

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

    fn make_sym_config() -> Arc<SymbolConfig> {
        Arc::new(SymbolConfig {
            precedence: vec![1; 256],
            weights: vec![2; 256],
            w0: 1,
        })
    }

    /// Builds a tiny definitional-CNF UNSAT problem with 5 branches:
    /// Top: def_1(X1) ∨ def_2(X2) ∨ def_3(X3) ∨ def_4(X4) ∨ def_5(X5)
    /// For each k, two clauses: ~def_k(Y) ∨ pk    and   ~def_k(Y) ∨ ~pk
    /// Each branch unit conflicts trivially: refutable.
    #[test]
    fn cwa_solves_small_definitional_cnf() {
        let mut syms = SymbolTable::new();
        let mut id_gen = ClauseIdGen::new();
        let mut clauses: Vec<Clause> = Vec::new();
        let mut def_preds: Vec<SymbolId> = Vec::new();
        let mut top_lits: Vec<Literal> = Vec::new();

        for k in 0..5u32 {
            let dname = format!("def_{}", k);
            let pname = format!("p_{}", k);
            let d = syms.intern(&dname);
            let p = syms.intern(&pname);
            def_preds.push(d);

            top_lits.push(Literal::pos(Atom::pred(d, vec![Term::var(k)])));

            clauses.push(make_clause(
                &mut id_gen,
                vec![
                    Literal::neg(Atom::pred(d, vec![Term::var(100 + k)])),
                    Literal::pos(Atom::prop(p)),
                ],
                &format!("d_{}_pos", k),
            ));
            clauses.push(make_clause(
                &mut id_gen,
                vec![
                    Literal::neg(Atom::pred(d, vec![Term::var(200 + k)])),
                    Literal::neg(Atom::prop(p)),
                ],
                &format!("d_{}_neg", k),
            ));
        }

        let top = make_clause(&mut id_gen, top_lits, "top");
        clauses.push(top);

        let result = try_componentwise_refute(
            &clauses,
            &mut id_gen,
            std::sync::Arc::new(syms.clone()),
            make_sym_config(),
        );
        assert!(
            matches!(result, Some(SearchResult::Refutation(..))),
            "expected Refutation, got {:?}",
            result
        );
    }

    /// Regression test for the PRO013+3.p unsoundness: `make_branch_unit`
    /// must preserve the original literal's polarity instead of forcing it
    /// positive. A negative disjunct `~def_k(X)` must produce a *negative*
    /// branch unit clause, not `def_k(X)`.
    #[test]
    fn make_branch_unit_preserves_polarity() {
        let mut syms = SymbolTable::new();
        let mut id_gen = ClauseIdGen::new();
        let d = syms.intern("def_0");

        let top_lits = vec![
            Literal::neg(Atom::pred(d, vec![Term::var(0)])),
            Literal::pos(Atom::pred(syms.intern("def_1"), vec![Term::var(1)])),
        ];
        let top = make_clause(&mut id_gen, top_lits, "top");

        let branch = make_branch_unit(&top, 0, &mut id_gen);
        assert_eq!(branch.literals.len(), 1);
        assert!(
            !branch.literals[0].positive,
            "expected branch unit for a negative top-clause literal to stay negative"
        );

        let branch_pos = make_branch_unit(&top, 1, &mut id_gen);
        assert!(
            branch_pos.literals[0].positive,
            "expected branch unit for a positive top-clause literal to stay positive"
        );
    }

    #[test]
    fn cwa_rejects_problem_without_top_disjunction() {
        let mut syms = SymbolTable::new();
        let mut id_gen = ClauseIdGen::new();
        let p = syms.intern("p");
        let clauses = vec![
            make_clause(&mut id_gen, vec![Literal::pos(Atom::prop(p))], "c1"),
            make_clause(&mut id_gen, vec![Literal::neg(Atom::prop(p))], "c2"),
        ];
        let result = try_componentwise_refute(
            &clauses,
            &mut id_gen,
            std::sync::Arc::new(syms.clone()),
            make_sym_config(),
        );
        assert!(result.is_none(), "expected None for non-CWA problem");
    }

    #[test]
    fn cwa_rejects_top_with_repeated_predicates() {
        // A 5-literal clause where two literals share a predicate symbol — must NOT match.
        let mut syms = SymbolTable::new();
        let mut id_gen = ClauseIdGen::new();
        let p1 = syms.intern("p1");
        let p2 = syms.intern("p2");
        let p3 = syms.intern("p3");
        let p4 = syms.intern("p4");

        let top = make_clause(
            &mut id_gen,
            vec![
                Literal::pos(Atom::pred(p1, vec![Term::var(0)])),
                Literal::pos(Atom::pred(p2, vec![Term::var(1)])),
                Literal::pos(Atom::pred(p3, vec![Term::var(2)])),
                Literal::pos(Atom::pred(p4, vec![Term::var(3)])),
                Literal::pos(Atom::pred(p1, vec![Term::var(4)])), // repeat
            ],
            "top",
        );
        assert!(detect_top_disjunction(&[top]).is_none());
    }

    #[test]
    fn cwa_rejects_top_with_shared_variables() {
        // 5 distinct predicates but literals share variables — REJECTED.
        // Independent componentwise refutation is fundamentally unsound if the
        // branches share variables (∀x(P(x) ∨ Q(x)) ≠ ∀x P(x) ∨ ∀x Q(x)).
        let mut syms = SymbolTable::new();
        let mut id_gen = ClauseIdGen::new();
        let preds: Vec<SymbolId> = (0..5).map(|i| syms.intern(&format!("p{}", i))).collect();
        let top = make_clause(
            &mut id_gen,
            preds
                .iter()
                .map(|&p| Literal::pos(Atom::pred(p, vec![Term::var(0)]))) // all share var(0)
                .collect(),
            "top",
        );
        let detected = detect_top_disjunction(&[top]);
        assert!(detected.is_none());
    }
}
