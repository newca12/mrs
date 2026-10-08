//! Superposition may not instantiate the clause it rewrites.
//!
//! # The defect this pins
//!
//! `superpose_with_id` unifies the equation clause's side with a subterm *of
//! the target* and then applies the resulting substitution to the whole target.
//! That is only a superposition while the unifier binds **only the equation
//! clause's variables**; if it also binds a variable of the target, the derived
//! clause is a *specialisation* of the intended resolvent, which the two cited
//! parents do not entail.
//!
//! `mrs_unify::robinson::unify_ac_id` did bind target variables: its
//! `(_, TermNode::Var(v))` arm binds `v` unconditionally, and the AC/assoc
//! argument-alignment fallback reaches it whenever the equation side has fewer
//! AC arguments than the target subterm. The archived CASC-30 `KLE145-10`
//! proof (`cnf(c431338, …)`) and the casc-j13 `LAT044-1` / `LAT241-10` proofs
//! each contain an `ac_superposition` node that the strict kernel correctly
//! refused for exactly this reason.
//!
//! Each case below is quoted verbatim from those archived proofs: the two
//! clauses the node cites, the AC symbols of its problem, and the conclusion
//! the node carries. Before the fix, feeding the cited pair to the prover's own
//! superposition reproduced the archived conclusion exactly; after the fix it
//! cannot, because the match it needed would have had to bind a target
//! variable.

use std::collections::HashSet;

use rustc_hash::FxHashSet as FxSet;

use mrs_core::clause::{Clause, ClauseId, ClauseIdGen, ClauseSource};
use mrs_core::term_bank::{IdAtom, IdClause, TermBank, TermId, TermNode};
use mrs_core::{Atom, Literal, SymbolId, SymbolTable, Term};

use mrs_calculus::ordering::TermOrdering;
use mrs_calculus::superposition::superpose_selected_id_until;

/// Builds a clause from a `(lhs, rhs)` body produced with the symbol resolver.
fn unit_eq_clause(
    id: ClauseId,
    rule: &'static str,
    body: impl FnOnce(&dyn Fn(&str) -> SymbolId) -> (Term, Term),
    resolve: &dyn Fn(&str) -> SymbolId,
) -> Clause {
    let (lhs, rhs) = body(resolve);
    Clause::new(
        id,
        vec![Literal::pos(Atom::eq(lhs, rhs))],
        ClauseSource::Inference {
            rule,
            parents: vec![ClauseId(u64::MAX - 1)].into(),
        },
    )
}

fn render(bank: &TermBank, symbols: &SymbolTable, term: TermId) -> String {
    match bank.get(term) {
        TermNode::Var(v) => format!("X{v}"),
        TermNode::App(symbol, args) => format!(
            "{}({})",
            symbols.resolve(*symbol),
            args.iter()
                .map(|a| render(bank, symbols, *a))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn render_clause(bank: &TermBank, symbols: &SymbolTable, clause: &IdClause) -> String {
    clause
        .literals
        .iter()
        .map(|literal| {
            let atom = match &literal.atom {
                IdAtom::Eq(l, r) => format!(
                    "{} = {}",
                    render(bank, symbols, *l),
                    render(bank, symbols, *r)
                ),
                IdAtom::Pred(p, args) => format!(
                    "{}({})",
                    symbols.resolve(*p),
                    args.iter()
                        .map(|a| render(bank, symbols, *a))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            };
            if literal.positive {
                atom
            } else {
                format!("~{atom}")
            }
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

/// A `(lhs, rhs)` clause body, built with the problem's symbol resolver.
type ClauseBody = fn(&dyn Fn(&str) -> SymbolId) -> (Term, Term);

/// Everything `superposition` needs to replay one archived node.
struct Case {
    label: &'static str,
    /// The conclusion the archived proof carries for this node.
    archived_conclusion: &'static str,
    symbols: &'static [&'static str],
    ac: &'static [&'static str],
    equation: ClauseBody,
    target: ClauseBody,
}

/// Every conclusion this pair of cited parents produces, in both orientations.
fn derive(case: &Case) -> Vec<String> {
    let mut symbols = SymbolTable::new();
    for name in case.symbols {
        symbols.intern(name);
    }
    let resolve = |name: &str| symbols.resolve_name(name).expect(name);
    let equation = unit_eq_clause(
        ClauseId(0),
        "ac_normalization",
        |r| (case.equation)(r),
        &resolve,
    );
    let target = unit_eq_clause(
        ClauseId(1),
        "ac_normalization",
        |r| (case.target)(r),
        &resolve,
    );

    let mut bank = TermBank::new();
    let eq_id = bank.clause_from_legacy(&equation);
    let tgt_id = bank.clause_from_legacy(&target);

    let mut comm: FxSet<SymbolId> = FxSet::default();
    let mut assoc: FxSet<SymbolId> = FxSet::default();
    for name in case.ac {
        comm.insert(resolve(name));
        assoc.insert(resolve(name));
    }

    let mut id_gen = ClauseIdGen::new();
    let ordering = TermOrdering::KBO;
    let mut out = Vec::new();
    for (a, b) in [(&eq_id, &tgt_id), (&tgt_id, &eq_id)] {
        for derived in superpose_selected_id_until(
            a,
            b,
            &mut bank,
            &ordering,
            &mut id_gen,
            None,
            &comm,
            &assoc,
            None,
        ) {
            out.push(render_clause(&bank, &symbols, &derived));
        }
    }
    out
}

fn squash(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

/// CASC-30 UEQ `KLE145-10`, archived node `c431338`, equation parent `c425221`.
fn kle145_10_equation(resolve: &dyn Fn(&str) -> SymbolId) -> (Term, Term) {
    // c425221: true = leq(X8, addition(X7, X8))
    (
        Term::constant(resolve("true")),
        Term::app(
            resolve("leq"),
            vec![
                Term::var(8),
                Term::app(resolve("addition"), vec![Term::var(7), Term::var(8)]),
            ],
        ),
    )
}

/// CASC-30 UEQ `KLE145-10`, archived node `c431338`, target parent `c2409`.
fn kle145_10_target(resolve: &dyn Fn(&str) -> SymbolId) -> (Term, Term) {
    // c2409: true = ifeq( leq(addition(one,X3),
    //   addition(X5, addition(X6, multiplication(X5,X3)))), true,
    //   leq(addition(one,X3), multiplication(strong_iteration(X5),X6)), true )
    let left = Term::app(
        resolve("leq"),
        vec![
            Term::app(
                resolve("addition"),
                vec![Term::constant(resolve("one")), Term::var(3)],
            ),
            Term::app(
                resolve("addition"),
                vec![
                    Term::var(5),
                    Term::app(
                        resolve("addition"),
                        vec![
                            Term::var(6),
                            Term::app(resolve("multiplication"), vec![Term::var(5), Term::var(3)]),
                        ],
                    ),
                ],
            ),
        ],
    );
    let right = Term::app(
        resolve("leq"),
        vec![
            Term::app(
                resolve("addition"),
                vec![Term::constant(resolve("one")), Term::var(3)],
            ),
            Term::app(
                resolve("multiplication"),
                vec![
                    Term::app(resolve("strong_iteration"), vec![Term::var(5)]),
                    Term::var(6),
                ],
            ),
        ],
    );
    (
        Term::constant(resolve("true")),
        Term::app(
            resolve("ifeq"),
            vec![
                left,
                Term::constant(resolve("true")),
                right,
                Term::constant(resolve("true")),
            ],
        ),
    )
}

/// CASC-J13 UEQ `LAT044-1`, archived node `c659171`.
fn lat044_1_equation(resolve: &dyn Fn(&str) -> SymbolId) -> (Term, Term) {
    // c197935: n1 = join(goal_d5, join(X4, complement(goal_d4)))
    (
        Term::constant(resolve("n1")),
        Term::app(
            resolve("join"),
            vec![
                Term::constant(resolve("goal_d5")),
                Term::app(
                    resolve("join"),
                    vec![
                        Term::var(4),
                        Term::app(
                            resolve("complement"),
                            vec![Term::constant(resolve("goal_d4"))],
                        ),
                    ],
                ),
            ],
        ),
    )
}

fn lat044_1_target(resolve: &dyn Fn(&str) -> SymbolId) -> (Term, Term) {
    // c24789: meet(X0, complement(join(X2, X4))) = meet(X0, complement(join(X2,
    //   meet(X0, meet(complement(X2), join(complement(X0), join(X2, X4)))))))
    let rhs = Term::app(
        resolve("join"),
        vec![
            Term::app(resolve("complement"), vec![Term::var(0)]),
            Term::app(resolve("join"), vec![Term::var(2), Term::var(4)]),
        ],
    );
    (
        Term::app(
            resolve("meet"),
            vec![
                Term::var(0),
                Term::app(
                    resolve("complement"),
                    vec![Term::app(resolve("join"), vec![Term::var(2), Term::var(4)])],
                ),
            ],
        ),
        Term::app(
            resolve("meet"),
            vec![
                Term::var(0),
                Term::app(
                    resolve("complement"),
                    vec![Term::app(
                        resolve("join"),
                        vec![
                            Term::var(2),
                            Term::app(
                                resolve("meet"),
                                vec![
                                    Term::var(0),
                                    Term::app(
                                        resolve("meet"),
                                        vec![
                                            Term::app(resolve("complement"), vec![Term::var(2)]),
                                            rhs,
                                        ],
                                    ),
                                ],
                            ),
                        ],
                    )],
                ),
            ],
        ),
    )
}

/// CASC-J13 UEQ `LAT241-10`, archived node `c53874`.
fn lat241_10_equation(resolve: &dyn Fn(&str) -> SymbolId) -> (Term, Term) {
    // c53707: join(X3, X4) = join(X3, join(meet(X4, X5), X4))
    (
        Term::app(resolve("join"), vec![Term::var(3), Term::var(4)]),
        Term::app(
            resolve("join"),
            vec![
                Term::var(3),
                Term::app(
                    resolve("join"),
                    vec![
                        Term::app(resolve("meet"), vec![Term::var(4), Term::var(5)]),
                        Term::var(4),
                    ],
                ),
            ],
        ),
    )
}

fn lat241_10_target(resolve: &dyn Fn(&str) -> SymbolId) -> (Term, Term) {
    // c26281: meet(X1, join(X2, X4)) = meet(X1, join(X2, join(X4, meet(X1, X4))))
    (
        Term::app(
            resolve("meet"),
            vec![
                Term::var(1),
                Term::app(resolve("join"), vec![Term::var(2), Term::var(4)]),
            ],
        ),
        Term::app(
            resolve("meet"),
            vec![
                Term::var(1),
                Term::app(
                    resolve("join"),
                    vec![
                        Term::var(2),
                        Term::app(
                            resolve("join"),
                            vec![
                                Term::var(4),
                                Term::app(resolve("meet"), vec![Term::var(1), Term::var(4)]),
                            ],
                        ),
                    ],
                ),
            ],
        ),
    )
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            label: "casc-30 KLE145-10 c431338",
            archived_conclusion: "true = ifeq(true, true, leq(addition(one, X14), \
                                 multiplication(strong_iteration(X14), one)), true)",
            symbols: &[
                "addition",
                "multiplication",
                "leq",
                "ifeq",
                "true",
                "one",
                "strong_iteration",
            ],
            ac: &["addition"],
            equation: kle145_10_equation,
            target: kle145_10_target,
        },
        Case {
            label: "casc-j13 LAT044-1 c659171",
            archived_conclusion: "meet(goal_d4, complement(join(X7, goal_d5))) = \
                                 meet(goal_d4, complement(join(X7, meet(goal_d4, \
                                 meet(complement(X7), n1)))))",
            symbols: &["meet", "join", "complement", "n1", "goal_d4", "goal_d5"],
            ac: &["meet", "join"],
            equation: lat044_1_equation,
            target: lat044_1_target,
        },
        Case {
            label: "casc-j13 LAT241-10 c53874",
            archived_conclusion: "meet(X7, join(X8, meet(X8, X5))) = \
                                 meet(X7, join(meet(X7, meet(X8, X5)), X8))",
            symbols: &["meet", "join"],
            ac: &["meet", "join"],
            equation: lat241_10_equation,
            target: lat241_10_target,
        },
    ]
}

/// The three archived `ac_superposition` conclusions whose cited parents cannot
/// produce them under any superposition the kernel could accept.
#[test]
fn archived_ac_superposition_conclusions_are_not_derived_anymore() {
    for case in cases() {
        let derived = derive(&case);
        let wanted = squash(case.archived_conclusion);
        assert!(
            !derived.iter().any(|d| squash(d) == wanted),
            "{}: superposition still derives the archived conclusion {}\n  derived: {derived:#?}",
            case.label,
            case.archived_conclusion,
        );
    }
}

/// The guard is a side condition on the *rule*, not a blanket refusal: a genuine
/// AC superposition — one whose match binds no target variable — still goes
/// through, and it binds the equation's variables as before.
#[test]
fn valid_ac_superposition_still_goes_through() {
    // add(X, g(Y)) = c          (X, Y are the equation clause's variables)
    // ~p(add(a, g(Z)))          (Z is the *target's* variable)
    // The AC match pairs g(Y) with g(Z) and binds X := a, Y := Z: no target
    // variable is bound, so this is a genuine superposition onto ~p(c).
    let mut symbols = SymbolTable::new();
    for name in ["add", "g", "p", "a", "c"] {
        symbols.intern(name);
    }
    let resolve = |name: &str| symbols.resolve_name(name).expect(name);
    let mut bank = TermBank::new();

    let equation = Clause::new(
        ClauseId(0),
        vec![Literal::pos(Atom::eq(
            Term::app(
                resolve("add"),
                vec![Term::var(0), Term::app(resolve("g"), vec![Term::var(1)])],
            ),
            Term::constant(resolve("c")),
        ))],
        ClauseSource::Input {
            name: "eq".into(),
            role: "axiom".into(),
        },
    );
    let target = Clause::new(
        ClauseId(1),
        vec![Literal::neg(Atom::pred(
            resolve("p"),
            vec![Term::app(
                resolve("add"),
                vec![
                    Term::constant(resolve("a")),
                    Term::app(resolve("g"), vec![Term::var(9)]),
                ],
            )],
        ))],
        ClauseSource::Input {
            name: "goal".into(),
            role: "negated_conjecture".into(),
        },
    );
    let eq_id = bank.clause_from_legacy(&equation);
    let tgt_id = bank.clause_from_legacy(&target);

    let mut comm: FxSet<SymbolId> = FxSet::default();
    let mut assoc: FxSet<SymbolId> = FxSet::default();
    comm.insert(resolve("add"));
    assoc.insert(resolve("add"));

    let mut id_gen = ClauseIdGen::new();
    let results = superpose_selected_id_until(
        &eq_id,
        &tgt_id,
        &mut bank,
        &TermOrdering::KBO,
        &mut id_gen,
        None,
        &comm,
        &assoc,
        None,
    );
    let rendered: Vec<String> = results
        .iter()
        .map(|r| render_clause(&bank, &symbols, r))
        .collect();
    assert!(
        rendered.iter().any(|r| r == "~p(c())"),
        "a valid AC superposition was lost: {rendered:#?}"
    );
}

/// The unifier itself: with the target's variables declared rigid the offending
/// match fails closed, and without it the same pair still binds them (which is
/// what the rule must never accept).
#[test]
fn rigid_unifier_refuses_target_bindings() {
    use mrs_unify::robinson::{unify_ac_id, unify_ac_rigid_id};

    let mut symbols = SymbolTable::new();
    for name in ["addition", "multiplication", "leq", "one"] {
        symbols.intern(name);
    }
    let resolve = |name: &str| symbols.resolve_name(name).expect(name);
    let mut bank = TermBank::new();

    // leq(V8, addition(V7, V8))  —  the equation side of KLE145-10 c425221
    let src_v8 = bank.intern_var(8);
    let src_v7 = bank.intern_var(7);
    let inner = bank.intern_app(resolve("addition"), vec![src_v7, src_v8]);
    let src = bank.intern_app(resolve("leq"), vec![src_v8, inner]);

    // leq(addition(one, X3), addition(X5, addition(X6, multiplication(X5, X3))))
    let one_t = bank.intern_app(resolve("one"), Vec::new());
    let x3 = bank.intern_var(3);
    let x5 = bank.intern_var(5);
    let x6 = bank.intern_var(6);
    let mult = bank.intern_app(resolve("multiplication"), vec![x5, x3]);
    let add3 = bank.intern_app(resolve("addition"), vec![x6, mult]);
    let add2 = bank.intern_app(resolve("addition"), vec![x5, add3]);
    let left = bank.intern_app(resolve("addition"), vec![one_t, x3]);
    let tgt = bank.intern_app(resolve("leq"), vec![left, add2]);

    let mut comm: HashSet<SymbolId> = HashSet::new();
    let mut assoc: HashSet<SymbolId> = HashSet::new();
    comm.insert(resolve("addition"));
    assoc.insert(resolve("addition"));

    let mut rigid: HashSet<u32> = HashSet::new();
    for id in 3..=6u32 {
        rigid.insert(id);
    }

    let unrestricted = unify_ac_id(src, tgt, &bank, &comm, &assoc)
        .expect("the unrestricted AC unifier accepts the pair");
    let mut probe = bank.clone();
    let mut bound = Vec::new();
    for id in 3..=6u32 {
        let t = probe.intern_var(id);
        let applied = unrestricted.apply_term(t, &mut probe);
        if applied != t {
            bound.push(id);
        }
    }
    assert!(
        !bound.is_empty(),
        "expected the unrestricted AC unifier to bind target variables; \
         if it no longer does this test is measuring nothing"
    );

    assert!(
        unify_ac_rigid_id(src, tgt, &bank, &comm, &assoc, Some(&rigid)).is_err(),
        "the rigid AC unifier accepted a match that binds a target variable"
    );
}
