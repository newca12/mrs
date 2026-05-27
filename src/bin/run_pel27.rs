use mrs_core::clause::{Clause, ClauseIdGen, ClauseSource};
use mrs_core::{Atom, Literal, SymbolTable, Term};
use mrs_search::state::SearchState;
use mrs_search::{LiteralSelection, SearchConfig, SelectionStrategy, TermOrdering};
use std::sync::Arc;

fn input_clause(id_gen: &mut ClauseIdGen, lits: Vec<Literal>, name: &str) -> Clause {
    Clause::new(
        id_gen.next(),
        lits,
        ClauseSource::Input {
            name: name.into(),
            role: "axiom".into(),
        },
    )
}

fn main() {
    let mut syms = SymbolTable::new();
    let f_sym = syms.intern("f");
    let g_sym = syms.intern("g");
    let h_sym = syms.intern("h");
    let i_sym = syms.intern("i");
    let j_sym = syms.intern("j");
    let sk1 = syms.intern("sk_ax1_0");
    let sk2 = syms.intern("sk_goal_0");
    let mut id_gen = ClauseIdGen::new();

    let clauses = vec![
        input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(f_sym, vec![Term::constant(sk1)]))],
            "ax1_0",
        ),
        input_clause(
            &mut id_gen,
            vec![Literal::neg(Atom::pred(g_sym, vec![Term::constant(sk1)]))],
            "ax1_1",
        ),
        input_clause(
            &mut id_gen,
            vec![
                Literal::neg(Atom::pred(f_sym, vec![Term::var(0)])),
                Literal::pos(Atom::pred(h_sym, vec![Term::var(0)])),
            ],
            "ax2",
        ),
        input_clause(
            &mut id_gen,
            vec![
                Literal::neg(Atom::pred(j_sym, vec![Term::var(0)])),
                Literal::neg(Atom::pred(i_sym, vec![Term::var(0)])),
                Literal::pos(Atom::pred(f_sym, vec![Term::var(0)])),
            ],
            "ax3",
        ),
        input_clause(
            &mut id_gen,
            vec![
                Literal::neg(Atom::pred(h_sym, vec![Term::var(0)])),
                Literal::pos(Atom::pred(g_sym, vec![Term::var(0)])),
                Literal::neg(Atom::pred(i_sym, vec![Term::var(1)])),
                Literal::neg(Atom::pred(h_sym, vec![Term::var(1)])),
            ],
            "ax4",
        ),
        input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(j_sym, vec![Term::constant(sk2)]))],
            "goal_0",
        ),
        input_clause(
            &mut id_gen,
            vec![Literal::pos(Atom::pred(i_sym, vec![Term::constant(sk2)]))],
            "goal_1",
        ),
    ];

    let mut state = SearchState::new(
        clauses.clone(),
        id_gen.clone(),
        Arc::new(mrs_calculus::ordering::SymbolConfig::default()),
    );
    let config = SearchConfig {
        time_limit: std::time::Duration::from_secs(5),
        max_clauses: 50_000,
        selection: SelectionStrategy::AgeWeight(5),
        literal_selection: LiteralSelection::AllNegative,
        ordering: TermOrdering::KBO,
    };
    let result = mrs_search::given_clause::search(&mut state, &config);
    println!("Result: {:?}", result);
    assert!(
        matches!(result, mrs_search::SearchResult::Refutation(..)),
        "pel27 should be refuted"
    );
}
