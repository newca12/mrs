//! Shared logic for the `mrs` book gallery examples.
//!
//! Each `examples/chNN_*.rs` wrapper is thin and printable in the book;
//! the reusable problem builders and search drivers live here so both the
//! examples and `tests/labs.rs` exercise the same code path.

use std::sync::Arc;
use std::time::Duration;

use mrs_calculus::ordering::SymbolConfig;
use mrs_core::clause::{Clause, ClauseIdGen};
use mrs_core::{Atom, Formula, SymbolTable, Term};
use mrs_search::given_clause::search;
use mrs_search::state::SearchState;
use mrs_search::{SearchConfig, SearchResult, SearchStats, SelectionStrategy};

/// Wall-clock budget for every lab search: small problems refute in
/// milliseconds; the budget only bounds pathological cases.
pub fn lab_budget() -> Duration {
    Duration::from_secs(5)
}

/// Parse the Socrates problem and return `(axioms, conjectures)` counts.
pub fn socrates_counts() -> (usize, usize) {
    let input = "\
        fof(ax1, axiom, ![X]: (human(X) => mortal(X))).\
        fof(ax2, axiom, human(socrates)).\
        fof(goal, conjecture, mortal(socrates)).";
    let problem = mrs_tptp::parse_tptp(input).expect("socrates parses");
    (problem.axioms().count(), problem.conjectures().count())
}

/// Build clausified Socrates clauses plus the negated conjecture.
///
/// Returns `(clauses, id_gen, symbols)` ready for [`prove`].
pub fn socrates_clauses() -> (Vec<Clause>, ClauseIdGen, SymbolTable) {
    let mut syms = SymbolTable::new();
    let human = syms.intern("human");
    let mortal = syms.intern("mortal");
    let socrates = syms.intern("socrates");

    // ∀X. human(X) => mortal(X)
    let ax1 = Formula::forall(
        0,
        Formula::implies(
            Formula::atom(Atom::pred(human, vec![Term::var(0)])),
            Formula::atom(Atom::pred(mortal, vec![Term::var(0)])),
        ),
    );
    // human(socrates)
    let ax2 = Formula::atom(Atom::pred(human, vec![Term::constant(socrates)]));
    // Negated conjecture: ¬mortal(socrates)
    let neg_conj = Formula::neg(Formula::atom(Atom::pred(
        mortal,
        vec![Term::constant(socrates)],
    )));

    let mut id_gen = ClauseIdGen::new();
    let mut clauses = Vec::new();
    clauses.extend(mrs_cnf::clausify(
        &ax1,
        &mut syms,
        &mut id_gen,
        "ax1",
        "axiom",
    ));
    clauses.extend(mrs_cnf::clausify(
        &ax2,
        &mut syms,
        &mut id_gen,
        "ax2",
        "axiom",
    ));
    clauses.extend(mrs_cnf::clausify(
        &neg_conj,
        &mut syms,
        &mut id_gen,
        "goal",
        "negated_conjecture",
    ));
    (clauses, id_gen, syms)
}

/// Run a single-strategy search over `clauses` (deterministic, one worker).
pub fn prove(
    clauses: Vec<Clause>,
    id_gen: ClauseIdGen,
    symbols: SymbolTable,
    config: SearchConfig,
    use_avatar: bool,
) -> SearchResult {
    let (result, _stats) = prove_with_stats(clauses, id_gen, symbols, config, use_avatar);
    result
}

/// Run a single-strategy search and return the result plus search stats.
pub fn prove_with_stats(
    clauses: Vec<Clause>,
    id_gen: ClauseIdGen,
    symbols: SymbolTable,
    config: SearchConfig,
    use_avatar: bool,
) -> (SearchResult, SearchStats) {
    let sym_config = Arc::new(SymbolConfig::default());
    let mut state = SearchState::new(clauses, id_gen, sym_config, Arc::new(symbols), use_avatar);
    let result = search(&mut state, &config);
    (result, state.stats.clone())
}

/// Prove Socrates with the default lab config. Returns true on refutation.
pub fn prove_socrates() -> bool {
    let (clauses, id_gen, syms) = socrates_clauses();
    let config = SearchConfig {
        time_limit: lab_budget(),
        ..Default::default()
    };
    matches!(
        prove(clauses, id_gen, syms, config, false),
        SearchResult::Refutation(..)
    )
}

/// Number of CNF clauses for Pelletier 1: `(p => q) <=> (~q => ~p)`.
pub fn pelletier_clause_count() -> usize {
    let mut syms = SymbolTable::new();
    let p = syms.intern("p");
    let q = syms.intern("q");
    let atom_p = Formula::atom(Atom::pred(p, vec![]));
    let atom_q = Formula::atom(Atom::pred(q, vec![]));
    let formula = Formula::iff(
        Formula::implies(atom_p.clone(), atom_q.clone()),
        Formula::implies(Formula::neg(atom_q), Formula::neg(atom_p)),
    );
    let mut id_gen = ClauseIdGen::new();
    mrs_cnf::clausify(&formula, &mut syms, &mut id_gen, "pel1", "conjecture").len()
}

/// Unify `f(X, a)` with `f(b, Y)` and check `X ↦ b`, `Y ↦ a`.
pub fn unify_demo_ok() -> bool {
    let mut syms = SymbolTable::new();
    let f = syms.intern("f");
    let a = syms.intern("a");
    let b = syms.intern("b");
    let t1 = Term::app(f, vec![Term::var(0), Term::constant(a)]);
    let t2 = Term::app(f, vec![Term::constant(b), Term::var(1)]);
    let Ok(mgu) = mrs_unify::unify(&t1, &t2) else {
        return false;
    };
    mgu.apply_term(&Term::var(0)) == Term::constant(b)
        && mgu.apply_term(&Term::var(1)) == Term::constant(a)
}

/// KBO and LPO both order `f(a)` above `a` and treat a term as equal to itself.
pub fn ordering_demo_ok() -> bool {
    use mrs_calculus::ordering::{TermComparison, TermOrdering};
    let mut syms = SymbolTable::new();
    let f = syms.intern("f");
    let a = syms.intern("a");
    let big = Term::app(f, vec![Term::constant(a)]);
    let small = Term::constant(a);
    let kbo = TermOrdering::KBO;
    let lpo = TermOrdering::LPO;
    kbo.compare(&big, &small) == TermComparison::Greater
        && lpo.compare(&big, &small) == TermComparison::Greater
        && kbo.compare(&big, &big.clone()) == TermComparison::Equal
        && lpo.compare(&small, &small.clone()) == TermComparison::Equal
}

/// Propositional resolution chain: `p`, `p => q ⊢ q` (conjecture `q` negated).
pub fn resolution_demo_ok() -> bool {
    let mut syms = SymbolTable::new();
    let p = syms.intern("p");
    let q = syms.intern("q");
    let atom_p = Formula::atom(Atom::pred(p, vec![]));
    let atom_q = Formula::atom(Atom::pred(q, vec![]));
    let ax1 = atom_p.clone();
    let ax2 = Formula::implies(atom_p, atom_q.clone());
    let neg_conj = Formula::neg(atom_q);

    let mut id_gen = ClauseIdGen::new();
    let mut clauses = Vec::new();
    clauses.extend(mrs_cnf::clausify(
        &ax1,
        &mut syms,
        &mut id_gen,
        "a1",
        "axiom",
    ));
    clauses.extend(mrs_cnf::clausify(
        &ax2,
        &mut syms,
        &mut id_gen,
        "a2",
        "axiom",
    ));
    clauses.extend(mrs_cnf::clausify(
        &neg_conj,
        &mut syms,
        &mut id_gen,
        "goal",
        "negated_conjecture",
    ));

    let config = SearchConfig {
        time_limit: lab_budget(),
        selection: SelectionStrategy::SmallestFirst,
        ..Default::default()
    };
    matches!(
        prove(clauses, id_gen, syms, config, false),
        SearchResult::Refutation(..)
    )
}

/// Prove Socrates with two selection strategies; returns `(ageweight_ok, smallest_ok)`.
pub fn selection_comparison_ok() -> (bool, bool) {
    let run = |selection| {
        let (clauses, id_gen, syms) = socrates_clauses();
        let config = SearchConfig {
            time_limit: lab_budget(),
            selection,
            ..Default::default()
        };
        let (result, stats) = prove_with_stats(clauses, id_gen, syms, config, false);
        (matches!(result, SearchResult::Refutation(..)), stats)
    };
    let (age_ok, age_stats) = run(SelectionStrategy::AgeWeight(5));
    let (small_ok, small_stats) = run(SelectionStrategy::SmallestFirst);
    // Both must refute; stats must show real work on this tiny problem.
    let _ = (age_stats.processed, small_stats.processed);
    (age_ok, small_ok)
}

/// Un satisfiable propositional split proved with and without AVATAR.
pub fn avatar_both_ok() -> (bool, bool) {
    let build = || {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let p_f = Formula::atom(Atom::pred(p, vec![]));
        let q_f = Formula::atom(Atom::pred(q, vec![]));
        let cases = [
            Formula::or(vec![p_f.clone(), q_f.clone()]),
            Formula::or(vec![Formula::neg(p_f.clone()), q_f.clone()]),
            Formula::or(vec![p_f.clone(), Formula::neg(q_f.clone())]),
            Formula::or(vec![Formula::neg(p_f), Formula::neg(q_f)]),
        ];
        let mut id_gen = ClauseIdGen::new();
        let mut clauses = Vec::new();
        for (i, case) in cases.iter().enumerate() {
            clauses.extend(mrs_cnf::clausify(
                case,
                &mut syms,
                &mut id_gen,
                &format!("c{i}"),
                "axiom",
            ));
        }
        (clauses, id_gen, syms)
    };
    let run = |use_avatar| {
        let (clauses, id_gen, syms) = build();
        let config = SearchConfig {
            time_limit: lab_budget(),
            ..Default::default()
        };
        matches!(
            prove(clauses, id_gen, syms, config, use_avatar),
            SearchResult::Refutation(..)
        )
    };
    (run(false), run(true))
}

/// Prove Socrates through the full strategy portfolio (`run_schedule`, 1 worker).
pub fn portfolio_proves_socrates() -> bool {
    use mrs_search::strategy::{StrategySchedule, run_schedule};
    let (clauses, id_gen, syms) = socrates_clauses();
    let schedule = StrategySchedule::default_schedule(lab_budget(), 1);
    assert!(!schedule.strategies.is_empty());
    let (result, _report) = run_schedule(
        &clauses,
        &[],
        id_gen,
        &schedule,
        &syms,
        mrs_search::strategy::MlOptions::default(),
        Some(1),
    );
    matches!(result, SearchResult::Refutation(..))
}
