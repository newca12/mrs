//! AC subset superposition: partial-target matches, and the guards around them.
//!
//! # What this pins
//!
//! `superpose_with_id` unifies the equation's side with a *whole* subterm of the
//! target. Under an associative-commutative symbol `f` that restricts it to
//! `σ(s) = flatten(u)` — the entire argument multiset. UI-1 in
//! `docs/policies/unresolved-issues.md` records what that costs: inferences that
//! rewrite *part* of a target term's AC arguments never happen at all.
//!
//! The fix adds the missing shape — `σ(s)` equal to a **non-empty subset** of the
//! target term's AC arguments, with the target's variables rigid throughout and
//! the pairing search under a deterministic budget. That capability has to come
//! with the guards, because every one of them is a way to fabricate a clause the
//! two cited parents do not entail:
//!
//! - an **empty** subset would add an argument for free (`f(a) = f(a, b)`);
//! - **dropping** an argument the match did not consume would delete a
//!   hypothesis for free (`f(a, b) = c` would rewrite `f(a,b,c)` to `f(c)`);
//! - **binding a target variable** would make the conclusion a specialisation of
//!   the resolvent, which is the unsound `ac_superposition` inference UI-1 is
//!   about;
//! - an **exhausted budget** must never be read as a match.
//!
//! Non-AC superposition must be exactly as it was, which is checked here and in
//! the `superposition` unit tests that predate this file.

use rustc_hash::FxHashSet as FxSet;

use mrs_core::clause::{ClauseId, ClauseIdGen, ClauseSource};
use mrs_core::term_bank::{IdAtom, IdClause, IdLiteral, TermBank, TermId, TermNode};
use mrs_core::{SymbolId, SymbolTable};

use mrs_calculus::ordering::TermOrdering;
use mrs_calculus::superposition::{
    AC_SUBSET_MATCH_BUDGET, superpose_selected_id_until, superpose_selected_id_until_budgeted,
};

/// The conclusions of one superposition attempt, rendered back to source form.
struct Run {
    rendered: Vec<String>,
}

impl Run {
    fn contains(&self, needle: &str) -> bool {
        self.rendered.iter().any(|r| r == needle)
    }
}

/// Which symbols of a problem the prover's own detection treats as AC.
#[derive(Clone, Default)]
struct AcSets {
    comm: FxSet<SymbolId>,
    assoc: FxSet<SymbolId>,
}

impl AcSets {
    fn none() -> Self {
        Self::default()
    }

    /// The `bool` marks whether the problem declares the symbol commutative; an
    /// associative-only symbol must not take the subset path.
    fn new(fx: &Fixture, symbols: &[(&str, bool)]) -> Self {
        let mut ac = Self::default();
        for (name, is_comm) in symbols {
            let id = fx.r(name);
            ac.assoc.insert(id);
            if *is_comm {
                ac.comm.insert(id);
            }
        }
        ac
    }
}

struct Fixture {
    symbols: SymbolTable,
    bank: TermBank,
}

impl Fixture {
    fn new(names: &[&str]) -> Self {
        let mut symbols = SymbolTable::new();
        for name in names {
            symbols.intern(name);
        }
        Self {
            symbols,
            bank: TermBank::new(),
        }
    }

    fn r(&self, name: &str) -> SymbolId {
        self.symbols.resolve_name(name).expect(name)
    }

    fn app(&mut self, name: &str, args: Vec<TermId>) -> TermId {
        let id = self.r(name);
        self.bank.intern_app(id, args)
    }

    fn constant(&mut self, name: &str) -> TermId {
        self.app(name, Vec::new())
    }

    fn var(&mut self, v: u32) -> TermId {
        self.bank.intern_var(v)
    }

    fn render(&self, term: TermId) -> String {
        render(&self.bank, &self.symbols, term)
    }

    fn render_clause(&self, clause: &IdClause) -> String {
        clause
            .literals
            .iter()
            .map(|literal| {
                let atom = match &literal.atom {
                    IdAtom::Eq(l, r) => format!("{} = {}", self.render(*l), self.render(*r)),
                    IdAtom::Pred(p, args) => format!(
                        "{}({})",
                        self.symbols.resolve(*p),
                        args.iter()
                            .map(|a| self.render(*a))
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

    fn superpose_both(&mut self, eq: &IdClause, target: &IdClause, ac: &AcSets) -> Run {
        self.superpose_both_budgeted(eq, target, ac, AC_SUBSET_MATCH_BUDGET)
    }

    fn superpose_both_budgeted(
        &mut self,
        eq: &IdClause,
        target: &IdClause,
        ac: &AcSets,
        budget: u32,
    ) -> Run {
        let mut id_gen = ClauseIdGen::new();
        let mut rendered = Vec::new();
        for (a, b) in [(eq, target), (target, eq)] {
            for derived in superpose_selected_id_until_budgeted(
                a,
                b,
                &mut self.bank,
                &TermOrdering::KBO,
                &mut id_gen,
                None,
                &ac.comm,
                &ac.assoc,
                budget,
                None,
            ) {
                rendered.push(self.render_clause(&derived));
            }
        }
        rendered.sort();
        rendered.dedup();
        Run { rendered }
    }

    /// The same run through the un-budgeted entry point, so a test can assert the
    /// two agree at the production budget.
    fn superpose_both_default(&mut self, eq: &IdClause, target: &IdClause, ac: &AcSets) -> Run {
        let mut id_gen = ClauseIdGen::new();
        let mut rendered = Vec::new();
        for (a, b) in [(eq, target), (target, eq)] {
            for derived in superpose_selected_id_until(
                a,
                b,
                &mut self.bank,
                &TermOrdering::KBO,
                &mut id_gen,
                None,
                &ac.comm,
                &ac.assoc,
                None,
            ) {
                rendered.push(self.render_clause(&derived));
            }
        }
        rendered.sort();
        rendered.dedup();
        Run { rendered }
    }
}

fn unit_eq(left: TermId, right: TermId, rule: &'static str) -> IdClause {
    clause(true, IdAtom::Eq(left, right), rule)
}

fn negated_pred(predicate: SymbolId, args: Vec<TermId>) -> IdClause {
    clause(
        false,
        IdAtom::Pred(predicate, args.into()),
        "negated_conjecture",
    )
}

fn clause(positive: bool, atom: IdAtom, rule: &'static str) -> IdClause {
    IdClause::new(
        ClauseId(0),
        vec![IdLiteral { positive, atom }],
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

// ---------------------------------------------------------------------------
// Positive controls: one per UI-1 regression problem.
// ---------------------------------------------------------------------------

/// `problems/group.p` — `mult(inv(X), X) = e` rewrites two of the three
/// arguments of `mult(inv(Y), Y, a)` and leaves `a` where it was.
///
/// The problem declares `mult` associative but not commutative, so the AC subset
/// rule — which needs both, because it consumes an arbitrary sub-multiset —
/// cannot fire on `group.p` as written. The fixture adds the commutativity the
/// rule requires in order to exercise it on this problem's own symbols; see
/// `archived_group_c26_target_specialization_is_still_refused` for what the
/// problem's real lost inference needs.
#[test]
fn positive_partial_target_match_on_group_symbols() {
    let mut fx = Fixture::new(&["mult", "e", "inv", "p", "a"]);
    let v0 = fx.var(0);
    let y = fx.var(7);
    let a = fx.constant("a");
    let inv_v0 = fx.app("inv", vec![v0]);
    let eq = unit_eq(fx.app("mult", vec![inv_v0, v0]), fx.constant("e"), "axiom");
    let inv_y = fx.app("inv", vec![y]);
    let target_arg = fx.app("mult", vec![inv_y, y, a]);
    let target = negated_pred(fx.r("p"), vec![target_arg]);
    let ac = AcSets::new(&fx, &[("mult", true)]);

    let run = fx.superpose_both(&eq, &target, &ac);
    assert!(
        run.contains("~p(mult(a(), e()))"),
        "the partial-target AC superposition was lost; derived: {:#?}",
        run.rendered
    );
}

/// `problems/group_right_inv.p` — `mult(e, X) = X` rewrites the first two of the
/// three arguments of `mult(e, b, c)` and keeps `c`.
#[test]
fn positive_partial_target_match_on_group_right_inv_symbols() {
    let mut fx = Fixture::new(&["mult", "e", "inv", "p", "b", "c"]);
    let v0 = fx.var(0);
    let b = fx.constant("b");
    let c = fx.constant("c");
    let e = fx.constant("e");
    let eq = unit_eq(fx.app("mult", vec![e, v0]), v0, "axiom");
    let target_arg = fx.app("mult", vec![e, b, c]);
    let target = negated_pred(fx.r("p"), vec![target_arg]);
    let ac = AcSets::new(&fx, &[("mult", true)]);

    let run = fx.superpose_both(&eq, &target, &ac);
    assert!(
        run.contains("~p(mult(c(), b()))"),
        "the partial-target AC superposition was lost; derived: {:#?}",
        run.rendered
    );
}

/// `problems/group_unique_inv.p` — the same axiom against a differently ordered
/// target, so the permutation search has to look past the first pairing.
#[test]
fn positive_partial_target_match_on_group_unique_inv_symbols() {
    let mut fx = Fixture::new(&["mult", "e", "inv", "p", "a"]);
    let v0 = fx.var(0);
    let y = fx.var(7);
    let a = fx.constant("a");
    let inv_v0 = fx.app("inv", vec![v0]);
    let eq = unit_eq(fx.app("mult", vec![inv_v0, v0]), fx.constant("e"), "axiom");
    let inv_y = fx.app("inv", vec![y]);
    let target_arg = fx.app("mult", vec![a, inv_y, y]);
    let target = negated_pred(fx.r("p"), vec![target_arg]);
    let ac = AcSets::new(&fx, &[("mult", true)]);

    let run = fx.superpose_both(&eq, &target, &ac);
    assert!(
        run.contains("~p(mult(a(), e()))"),
        "the partial-target AC superposition was lost; derived: {:#?}",
        run.rendered
    );
}

/// `problems/lattice_absorb.p` — `join(X, meet(X, Y)) = X` rewrites the first two
/// of `join(a, meet(a, b), c)`. Both `join` and `meet` are AC in this problem's
/// own axioms, so this is the one regression whose fixture needs no widening.
#[test]
fn positive_partial_target_match_on_lattice_absorb_symbols() {
    let mut fx = Fixture::new(&["join", "meet", "p", "a", "b", "c"]);
    let x = fx.var(0);
    let y = fx.var(1);
    let a = fx.constant("a");
    let b = fx.constant("b");
    let c = fx.constant("c");
    let inner = fx.app("meet", vec![x, y]);
    let eq = unit_eq(fx.app("join", vec![x, inner]), x, "axiom");
    let target_inner = fx.app("meet", vec![a, b]);
    let target_arg = fx.app("join", vec![a, target_inner, c]);
    let target = negated_pred(fx.r("p"), vec![target_arg]);
    let ac = AcSets::new(&fx, &[("join", true), ("meet", true)]);

    let run = fx.superpose_both(&eq, &target, &ac);
    assert!(
        run.contains("~p(join(c(), a()))"),
        "the partial-target AC superposition was lost; derived: {:#?}",
        run.rendered
    );
}

/// `problems/ring_idem.p` — `plus(neg(X), X) = zero` rewrites two of the three
/// arguments of `plus(neg(Y), Y, a)`.
#[test]
fn positive_partial_target_match_on_ring_idem_symbols() {
    let mut fx = Fixture::new(&["plus", "neg", "zero", "p", "a"]);
    let v0 = fx.var(0);
    let y = fx.var(7);
    let a = fx.constant("a");
    let neg_v0 = fx.app("neg", vec![v0]);
    let eq = unit_eq(
        fx.app("plus", vec![neg_v0, v0]),
        fx.constant("zero"),
        "axiom",
    );
    let neg_y = fx.app("neg", vec![y]);
    let target_arg = fx.app("plus", vec![neg_y, y, a]);
    let target = negated_pred(fx.r("p"), vec![target_arg]);
    let ac = AcSets::new(&fx, &[("plus", true)]);

    let run = fx.superpose_both(&eq, &target, &ac);
    assert!(
        run.contains("~p(plus(a(), zero()))"),
        "the partial-target AC superposition was lost; derived: {:#?}",
        run.rendered
    );
}

// ---------------------------------------------------------------------------
// The guards.
// ---------------------------------------------------------------------------

/// The inference the archived pre-fix `group.s` proof used.
///
/// `cnf(c26, …, inference(superposition, …, [c7, c11]))` superposes
/// `mult(inv(V), V) = e` into the subterm `mult(A, B)` of the associativity
/// axiom, producing `mult(e, C) = mult(inv(B), mult(B, C))`. It is refused here
/// and must stay refused: the match it needs sets `A := inv(V)`, instantiating a
/// variable of the clause being rewritten. Subset matching cannot rescue it
/// either — both of the pattern's arguments would still have to land on target
/// variables.
#[test]
fn archived_group_c26_target_specialization_is_still_refused() {
    let mut fx = Fixture::new(&["mult", "e", "inv"]);
    let v = fx.var(0);
    let a = fx.var(3);
    let b = fx.var(4);
    let c = fx.var(5);

    // left_inverse: mult(inv(V), V) = e
    let inv_v = fx.app("inv", vec![v]);
    let eq = unit_eq(fx.app("mult", vec![inv_v, v]), fx.constant("e"), "axiom");
    // associativity: mult(mult(A,B), C) = mult(A, mult(B, C))
    let left = fx.app("mult", vec![a, b]);
    let inner = fx.app("mult", vec![b, c]);
    let target = unit_eq(
        fx.app("mult", vec![left, c]),
        fx.app("mult", vec![a, inner]),
        "axiom",
    );
    let ac = AcSets::new(&fx, &[("mult", true)]);

    let run = fx.superpose_both(&eq, &target, &ac);
    for clause in &run.rendered {
        assert!(
            !clause.contains("X4"),
            "a superposition bound a variable of the clause it rewrote: {clause}\n\
             all derived: {:#?}",
            run.rendered
        );
    }
}

/// The inference that recovers a whole problem.
///
/// `problems/force_subset2.p` — `prod(a, X) = b` superposed into
/// `prod(prod(a,b), c)`. The two terms have the same *direct* arity, so the
/// rigid AC unifier takes its argument-alignment fallback and lines `a` up
/// against `prod(a,b)`, which fails; the flattened arities differ (2 against 3),
/// so it never reaches the element-wise alignment that would have worked either.
/// Only the subset shape gets there: `σ(from) = {a, X}` is a sub-multiset of
/// `{a, b, c}`, with `X := b`, leaving `prod(c, b)`.
///
/// With this inference the problem is a `Theorem`; without it the search
/// saturates and returns `GaveUp`.
#[test]
fn recovers_the_inference_that_solves_force_subset2() {
    let mut fx = Fixture::new(&["prod", "a", "b", "c"]);
    let x0 = fx.var(0);
    let a = fx.constant("a");
    let b = fx.constant("b");
    let c = fx.constant("c");

    // rule: prod(a, X) = b
    let eq = unit_eq(fx.app("prod", vec![a, x0]), b, "axiom");
    // negated goal: prod(prod(a,b), c) != prod(c, b)
    let inner = fx.app("prod", vec![a, b]);
    let subject = fx.app("prod", vec![inner, c]);
    let other = fx.app("prod", vec![c, b]);
    let target = clause(false, IdAtom::Eq(subject, other), "negated_conjecture");
    let ac = AcSets::new(&fx, &[("prod", true)]);

    let run = fx.superpose_both(&eq, &target, &ac);
    assert!(
        run.contains("~prod(c(), b()) = prod(c(), b())"),
        "the partial-target step that solves force_subset2 was lost; derived: {:#?}",
        run.rendered
    );
}

/// A partial-target match may not bind a target variable to make itself work.
///
/// `f(g(X0))` against `f(Y, a)` has exactly one pairing, and it needs `Y` to
/// become `g(X0)`.
#[test]
fn partial_target_match_refuses_to_bind_a_target_variable() {
    let mut fx = Fixture::new(&["f", "g", "p", "a"]);
    let x0 = fx.var(0);
    let y = fx.var(7);
    let a = fx.constant("a");
    let g_x0 = fx.app("g", vec![x0]);
    let eq = unit_eq(fx.app("f", vec![g_x0]), fx.constant("a"), "axiom");
    let target_arg = fx.app("f", vec![y, a]);
    let target = negated_pred(fx.r("p"), vec![target_arg]);
    let ac = AcSets::new(&fx, &[("f", true)]);

    let run = fx.superpose_both(&eq, &target, &ac);
    for clause in &run.rendered {
        assert!(
            !clause.contains("X7"),
            "a superposition instantiated a target variable: {clause}\n\
             all derived: {:#?}",
            run.rendered
        );
    }
}

/// No clause may gain or lose an argument.
///
/// With `f(a, b) = c` against `~p(f(a, b, c))` the one sound step consumes
/// `{a, b}` and appends `c`, giving `~p(f(c, c))`. `~p(f(c))` would drop `c`,
/// which no instance of the equation entails.
#[test]
fn unmatched_target_arguments_are_never_dropped() {
    let mut fx = Fixture::new(&["f", "p", "a", "b", "c"]);
    let a = fx.constant("a");
    let b = fx.constant("b");
    let c = fx.constant("c");
    let eq = unit_eq(fx.app("f", vec![a, b]), c, "axiom");
    let target_arg = fx.app("f", vec![a, b, c]);
    let target = negated_pred(fx.r("p"), vec![target_arg]);
    let ac = AcSets::new(&fx, &[("f", true)]);

    let run = fx.superpose_both(&eq, &target, &ac);
    assert!(
        run.contains("~p(f(c(), c()))"),
        "the sound partial-target step was lost; derived: {:#?}",
        run.rendered
    );
    for forged in [
        "~p(f(c()))",           // `c` dropped
        "~p(f())",              // every argument dropped
        "~p(f(c(), c(), c()))", // an argument gained
    ] {
        assert!(
            !run.contains(forged),
            "an argument was forged: {forged}\nall derived: {:#?}",
            run.rendered
        );
    }
}

/// The empty subset would add an argument for free.
///
/// `f(a) = d` has nothing to consume in `f(b, c)`, so it must produce nothing at
/// all — in particular not `~p(f(b, c, d))`.
#[test]
fn the_empty_subset_is_not_a_match() {
    let mut fx = Fixture::new(&["f", "p", "a", "b", "c", "d"]);
    let a = fx.constant("a");
    let b = fx.constant("b");
    let c = fx.constant("c");
    let d = fx.constant("d");
    let eq = unit_eq(fx.app("f", vec![a]), d, "axiom");
    let target_arg = fx.app("f", vec![b, c]);
    let target = negated_pred(fx.r("p"), vec![target_arg]);
    let ac = AcSets::new(&fx, &[("f", true)]);

    let run = fx.superpose_both(&eq, &target, &ac);
    assert!(
        run.rendered.is_empty(),
        "the empty subset produced an inference: {:#?}",
        run.rendered
    );
}

/// An AC symbol with an identity element keeps its arguments.
///
/// `f(e, e) = e` against `~p(f(e, e, e))` consumes two of the three arguments
/// and appends `e`. Collapsing the result to `f(e)` — or to `e` — would use the
/// identity to delete a hypothesis for free.
#[test]
fn identity_element_arguments_survive_the_rewrite() {
    let mut fx = Fixture::new(&["f", "e", "p"]);
    let e = fx.constant("e");
    let eq = unit_eq(fx.app("f", vec![e, e]), e, "axiom");
    let target_arg = fx.app("f", vec![e, e, e]);
    let target = negated_pred(fx.r("p"), vec![target_arg]);
    let ac = AcSets::new(&fx, &[("f", true)]);

    let run = fx.superpose_both(&eq, &target, &ac);
    assert!(
        run.contains("~p(f(e(), e()))"),
        "the sound partial-target step was lost; derived: {:#?}",
        run.rendered
    );
    for forged in ["~p(f(e()))", "~p(e())"] {
        assert!(
            !run.contains(forged),
            "the identity element was allowed to delete an argument: {forged}\n\
             all derived: {:#?}",
            run.rendered
        );
    }
}

/// Duplicate arguments are consumed as distinct occurrences.
///
/// `f(a, b) = c` against `~p(f(a, a, b, b))` consumes one `a` and one `b`, so
/// the surviving arguments are the other `a` and the other `b`.
#[test]
fn duplicate_arguments_are_matched_as_distinct_occurrences() {
    let mut fx = Fixture::new(&["f", "p", "a", "b", "c"]);
    let a = fx.constant("a");
    let b = fx.constant("b");
    let c = fx.constant("c");
    let eq = unit_eq(fx.app("f", vec![a, b]), c, "axiom");
    let target_arg = fx.app("f", vec![a, a, b, b]);
    let target = negated_pred(fx.r("p"), vec![target_arg]);
    let ac = AcSets::new(&fx, &[("f", true)]);

    let run = fx.superpose_both(&eq, &target, &ac);
    // One `a` and one `b` are consumed, so the other `a` and the other `b`
    // survive, and `c` is appended.
    assert!(
        run.contains("~p(f(a(), f(b(), c())))"),
        "the duplicate-argument partial step was lost; derived: {:#?}",
        run.rendered
    );
    for forged in [
        "~p(f(b(), f(b(), c())))",         // the surviving `a` dropped as well
        "~p(f(a(), c()))",                 // one `b` dropped
        "~p(f(a(), f(b(), f(b(), c()))))", // an argument gained
    ] {
        assert!(
            !run.contains(forged),
            "a duplicate argument was dropped: {forged}\nall derived: {:#?}",
            run.rendered
        );
    }
}

/// Budget exhaustion fails closed.
///
/// The same pair produces `~p(f(b(), c()))` at a real budget and nothing at all
/// at budget zero. An exhausted search must not return a different,
/// half-finished pairing, and must not return the inference anyway.
#[test]
fn budget_exhaustion_produces_no_inference() {
    let mut fx = Fixture::new(&["f", "p", "a", "b", "c", "d"]);
    let a = fx.constant("a");
    let b = fx.constant("b");
    let c = fx.constant("c");
    let d = fx.constant("d");
    let eq = unit_eq(fx.app("f", vec![a, b]), c, "axiom");
    let target_arg = fx.app("f", vec![a, b, d]);
    let target = negated_pred(fx.r("p"), vec![target_arg]);
    let ac = AcSets::new(&fx, &[("f", true)]);

    let with_budget = fx.superpose_both_budgeted(&eq, &target, &ac, 1024);
    assert!(
        with_budget.contains("~p(f(d(), c()))"),
        "the sound partial-target step was lost; derived: {:#?}",
        with_budget.rendered
    );

    let starved = fx.superpose_both_budgeted(&eq, &target, &ac, 0);
    assert!(
        starved.rendered.is_empty(),
        "an exhausted budget still produced an inference: {:#?}",
        starved.rendered
    );
}

/// The budget bounds the AC subset search and nothing else.
///
/// With the budget at zero the ordinary whole-subterm superposition still goes
/// through; only the subset shape is unavailable.
#[test]
fn budget_exhaustion_does_not_affect_non_ac_superposition() {
    let mut fx = Fixture::new(&["f", "p", "a", "b"]);
    let a = fx.constant("a");
    let b = fx.constant("b");
    let f_a = fx.app("f", vec![a]);
    let eq = unit_eq(f_a, b, "axiom");
    let target_arg = fx.app("f", vec![a]);
    let target = negated_pred(fx.r("p"), vec![target_arg]);

    let starved = fx.superpose_both_budgeted(&eq, &target, &AcSets::none(), 0);
    assert!(
        starved.contains("~p(b())"),
        "ordinary superposition depends on the AC subset budget; derived: {:#?}",
        starved.rendered
    );
}

/// Non-AC superposition is unchanged: with no AC symbols the rule never fires
/// and only the ordinary whole-subterm inference is derived.
#[test]
fn non_ac_symbols_are_unaffected() {
    let mut fx = Fixture::new(&["f", "p", "a", "c"]);
    let a = fx.constant("a");
    let c = fx.constant("c");
    let eq = unit_eq(fx.app("f", vec![a]), c, "axiom");
    let target_arg = fx.app("f", vec![a]);
    let target = negated_pred(fx.r("p"), vec![target_arg]);

    let run = fx.superpose_both(&eq, &target, &AcSets::none());
    assert_eq!(
        run.rendered,
        vec!["~p(c())".to_string()],
        "non-AC superposition changed"
    );
}

/// An associative-only symbol never takes the subset path.
///
/// The rule consumes an arbitrary sub-multiset, which only a commutative symbol
/// licenses; an associative-only symbol must keep the ordinary behaviour. This is
/// the shape of `mult` in `group.p`, `group_right_inv.p`, `group_unique_inv.p` and
/// `plus`/`times` in `ring_idem.p`.
#[test]
fn associative_only_symbols_do_not_take_the_subset_path() {
    let mut fx = Fixture::new(&["f", "p", "a", "b", "c"]);
    let a = fx.constant("a");
    let b = fx.constant("b");
    let c = fx.constant("c");
    let eq = unit_eq(fx.app("f", vec![a]), c, "axiom");
    let target_arg = fx.app("f", vec![a, b]);
    let target = negated_pred(fx.r("p"), vec![target_arg]);

    let run = fx.superpose_both(&eq, &target, &AcSets::new(&fx, &[("f", false)]));
    assert!(
        run.rendered.is_empty(),
        "an associative-only symbol took the commutative subset path: {:#?}",
        run.rendered
    );
}

/// The default entry point and the explicit-budget one agree at the production
/// budget, so the budget is a knob rather than a second behaviour.
#[test]
fn default_budget_entry_point_matches_the_explicit_one() {
    let mut fx = Fixture::new(&["f", "p", "a", "b", "c"]);
    let a = fx.constant("a");
    let b = fx.constant("b");
    let c = fx.constant("c");
    let eq = unit_eq(fx.app("f", vec![a]), c, "axiom");
    let target_arg = fx.app("f", vec![a, b]);
    let target = negated_pred(fx.r("p"), vec![target_arg]);
    let ac = AcSets::new(&fx, &[("f", true)]);

    assert_eq!(
        fx.superpose_both_default(&eq, &target, &ac).rendered,
        fx.superpose_both_budgeted(&eq, &target, &ac, AC_SUBSET_MATCH_BUDGET)
            .rendered
    );
}

/// The budget is a real ceiling rather than a formality: a wide target with a
/// tiny budget returns `BudgetExhausted`, not a match.
#[test]
fn a_tiny_budget_on_a_wide_target_is_refused() {
    let mut fx = Fixture::new(&["f", "a"]);
    let a = fx.constant("a");
    let wide = vec![a; 8];
    let pattern = fx.app("f", vec![a, a]);
    let target = fx.app("f", wide);
    let comm: FxSet<SymbolId> = [fx.r("f")].into_iter().collect();
    let assoc: FxSet<SymbolId> = [fx.r("f")].into_iter().collect();

    let mut generous = mrs_unify::ac_subset::AcSubsetBudget::new(4096);
    assert!(
        mrs_unify::ac_subset::match_ac_subset_rigid_id(
            pattern,
            target,
            &fx.bank,
            &comm,
            &assoc,
            None,
            &mut generous,
        )
        .is_ok(),
        "the generous budget should still find the match"
    );

    let mut stingy = mrs_unify::ac_subset::AcSubsetBudget::new(1);
    assert!(
        matches!(
            mrs_unify::ac_subset::match_ac_subset_rigid_id(
                pattern,
                target,
                &fx.bank,
                &comm,
                &assoc,
                None,
                &mut stingy,
            ),
            Err(mrs_unify::UnifyError::BudgetExhausted)
        ),
        "a stingy budget must refuse rather than return a partial match"
    );
}
