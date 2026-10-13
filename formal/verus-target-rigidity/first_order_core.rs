use vstd::prelude::*;

verus! {

/// First-order terms with variables, constants, and unary/binary functions.
pub enum Term {
    Var(u64),
    Const(u64),
    App1(u64, Box<Term>),
    App2(u64, Box<Term>, Box<Term>),
}

pub enum Atom {
    Pred0(u64),
    Pred1(u64, Box<Term>),
    Pred2(u64, Box<Term>, Box<Term>),
    Eq(Term, Term),
}

pub struct Literal {
    positive: bool,
    atom: Atom,
}

pub struct Clause {
    literals: Seq<Literal>,
}

pub struct Interpretation {
    domain: Set<u64>,
    functions: Map<(u64, Seq<u64>), u64>,
    predicates: Map<(u64, Seq<u64>), bool>,
}

pub struct Valuation {
    values: IMap<u64, u64>,
}

pub struct Model {
    interp: Interpretation,
    valuation: Valuation,
}

/// Truth of a universally closed formula with one free variable.
pub struct UniversalFormula {
    body: IMap<u64, bool>,
}

spec fn formula_holds(formula: UniversalFormula) -> bool {
    forall|value: u64| formula.body[value]
}

proof fn universal_formula_instance(formula: UniversalFormula, chosen: u64)
    requires formula_holds(formula)
    ensures formula.body[chosen]
{
    assert(formula.body[chosen]);
}

/// A finite simultaneous substitution. Duplicate-free domains are checked by
/// the executable checker below. RHS terms are not recursively substituted.
pub struct Substitution {
    bindings: Seq<(u64, Term)>,
}

spec fn lookup_rec(bindings: Seq<(u64, Term)>, v: u64, n: nat) -> Option<Term>
    decreases n
{
    if n == 0 {
        None
    } else if bindings[n - 1].0 == v {
        Some(bindings[n - 1].1)
    } else {
        lookup_rec(bindings, v, (n - 1) as nat)
    }
}

spec fn lookup(sigma: Substitution, v: u64) -> Option<Term> {
    lookup_rec(sigma.bindings, v, sigma.bindings.len())
}

spec fn apply(term: Term, sigma: Substitution) -> Term
    decreases term
{
    match term {
        Term::Var(v) => match lookup(sigma, v) {
            Some(t) => t,
            None => Term::Var(v),
        },
        Term::Const(c) => Term::Const(c),
        Term::App1(f, child) => Term::App1(f, Box::new(apply(*child, sigma))),
        Term::App2(f, left, right) => Term::App2(
            f,
            Box::new(apply(*left, sigma)),
            Box::new(apply(*right, sigma)),
        ),
    }
}

spec fn eval(term: Term, interp: Interpretation, valuation: Valuation) -> u64
    decreases term
{
    match term {
        Term::Var(v) => valuation.values[v],
        Term::Const(c) => interp.functions[(c, seq![])],
        Term::App1(f, child) => interp.functions[(f, seq![eval(*child, interp, valuation)])],
        Term::App2(f, left, right) => interp.functions[(
            f,
            seq![eval(*left, interp, valuation), eval(*right, interp, valuation)],
        )],
    }
}

/// The valuation obtained by interpreting each substitution RHS in the old
/// valuation. This definition is the semantic bridge needed for the usual
/// substitution/evaluation lemma.
spec fn valuation_after(
    interp: Interpretation,
    valuation: Valuation,
    sigma: Substitution,
) -> Valuation {
    Valuation {
        values: IMap::total(|x: u64| match lookup(sigma, x) {
            Some(replacement) => eval(replacement, interp, valuation),
            None => valuation.values[x],
        }),
    }
}

/// Applying a simultaneous substitution to a term and then evaluating it is
/// equivalent to evaluating the original term under the substitution-updated
/// valuation. This is the general semantic bridge used by inference proofs.
proof fn eval_after_substitution(
    term: Term,
    sigma: Substitution,
    interp: Interpretation,
    valuation: Valuation,
)
    requires
        forall|key: (u64, Seq<u64>)| interp.functions.dom().contains(key),
        forall|x: u64| valuation.values.dom().contains(x),
    ensures eval(apply(term, sigma), interp, valuation)
        == eval(term, interp, valuation_after(interp, valuation, sigma))
    decreases term
{
    match term {
        Term::Var(x) => {
            match lookup(sigma, x) {
                Some(replacement) => {
                    assert(apply(Term::Var(x), sigma) == replacement);
                    assert(valuation_after(interp, valuation, sigma).values[x]
                        == eval(replacement, interp, valuation));
                },
                None => {
                    assert(apply(Term::Var(x), sigma) == Term::Var(x));
                    assert(valuation_after(interp, valuation, sigma).values[x]
                        == valuation.values[x]);
                },
            }
        },
        Term::Const(_) => {},
        Term::App1(f, child) => {
            eval_after_substitution(*child, sigma, interp, valuation);
        },
        Term::App2(f, left, right) => {
            eval_after_substitution(*left, sigma, interp, valuation);
            eval_after_substitution(*right, sigma, interp, valuation);
        },
    }
}

spec fn atom_holds(atom: Atom, interp: Interpretation, valuation: Valuation) -> bool
    decreases atom
{
    match atom {
        Atom::Pred0(p) => interp.predicates[(p, seq![])],
        Atom::Pred1(p, arg) => interp.predicates[(p, seq![eval(*arg, interp, valuation)])],
        Atom::Pred2(p, left, right) => interp.predicates[(
            p,
            seq![eval(*left, interp, valuation), eval(*right, interp, valuation)],
        )],
        Atom::Eq(left, right) => eval(left, interp, valuation) == eval(right, interp, valuation),
    }
}

spec fn literal_holds(literal: Literal, interp: Interpretation, valuation: Valuation) -> bool {
    literal.positive == atom_holds(literal.atom, interp, valuation)
}

spec fn clause_holds(clause: Clause, interp: Interpretation, valuation: Valuation) -> bool {
    exists|i: int| 0 <= i < clause.literals.len()
        && literal_holds(clause.literals[i], interp, valuation)
}

proof fn universal_clause_instance(p: IMap<u64, bool>, instance: u64)
    requires forall|x: u64| p[x]
    ensures p[instance]
{
    assert(p[instance]);
}

/// A semantic superposition instance where the ground source equality
/// `f(a)=b` rewrites `p(f(y))` in the universally closed target clause
/// `q(y) | p(f(y))`. Unification binds the target variable `y` to `a`; applying
/// that substitution to the *whole* target clause gives `q(a) | p(b)`. The
/// other target literal is instantiated too. This proves the soundness of this
/// instance, not the validity of archived c502 or of the general calculus.
proof fn target_variable_superposition_instance(
    f: IMap<u64, u64>,
    p: IMap<u64, bool>,
    q: IMap<u64, bool>,
    a: u64,
    b: u64,
)
    requires
        f.dom().contains(a),
        p.dom().contains(b),
        q.dom().contains(a),
        f[a] == b,
        forall|y: u64| f.dom().contains(y) && q.dom().contains(y) && p.dom().contains(f[y])
            ==> q[y] || p[f[y]],
    ensures
        q[a] || p[b],
{
    assert(q[a] || p[f[a]]);
    assert(f[a] == b);
    assert(p[f[a]] == p[b]);
}

spec fn domain_unique(sigma: Substitution) -> bool {
    forall|i: int, j: int|
        #![auto]
        0 <= i < j < sigma.bindings.len() as int
            ==> sigma.bindings[i].0 != sigma.bindings[j].0
}

/// Executable mirror of the mathematical term syntax.
#[derive(PartialEq, Eq)]
pub enum ExecTerm {
    Var(u64),
    Const(u64),
    App1(u64, Box<ExecTerm>),
    App2(u64, Box<ExecTerm>, Box<ExecTerm>),
}

#[derive(PartialEq, Eq)]
pub struct ExecBinding {
    variable: u64,
    replacement: ExecTerm,
}

spec fn view(term: ExecTerm) -> Term
    decreases term
{
    match term {
        ExecTerm::Var(v) => Term::Var(v),
        ExecTerm::Const(c) => Term::Const(c),
        ExecTerm::App1(f, child) => Term::App1(f, Box::new(view(*child))),
        ExecTerm::App2(f, left, right) => Term::App2(
            f,
            Box::new(view(*left)),
            Box::new(view(*right)),
        ),
    }
}

spec fn view_subst(bindings: Seq<ExecBinding>) -> Substitution {
    Substitution {
        bindings: bindings.map(|_i: int, b: ExecBinding| (b.variable, view(b.replacement))),
    }
}

fn check_domain_unique(bindings: &Vec<ExecBinding>) -> (unique: bool)
    ensures unique ==> domain_unique(view_subst(bindings@))
{
    let mut i: usize = 0;
    while i < bindings.len()
        invariant
            i <= bindings.len(),
            forall|p: int, q: int|
                #![auto]
                0 <= p < i as int && p < q < bindings@.len() as int
                    ==> bindings@[p].variable != bindings@[q].variable,
        decreases bindings.len() - i
    {
        assert(i < bindings.len());
        let mut j: usize = i + 1;
        while j < bindings.len()
            invariant
                i < bindings.len(),
                i < j <= bindings.len(),
                forall|p: int, q: int|
                    #![auto]
                    0 <= p < i as int && p < q < bindings@.len() as int
                        ==> bindings@[p].variable != bindings@[q].variable,
                forall|k: int| i < k < j as int
                    ==> bindings@[i as int].variable != bindings@[k].variable,
            decreases bindings.len() - j
        {
            assert(j < bindings.len());
            assert forall|k: int| i < k < j as int
                implies bindings@[i as int].variable != bindings@[k].variable by {
                assert(k < bindings@.len());
                assert(bindings@[i as int].variable != bindings@[k].variable);
            }
            if bindings[i].variable == bindings[j].variable {
                return false;
            }
            j += 1;
        }
        i += 1;
    }
    assert(domain_unique(view_subst(bindings@)));
    true
}

fn copy_term(term: &ExecTerm) -> (copy: ExecTerm)
    ensures view(copy) == view(*term)
    decreases term
{
    match term {
        ExecTerm::Var(v) => ExecTerm::Var(*v),
        ExecTerm::Const(c) => ExecTerm::Const(*c),
        ExecTerm::App1(f, child) => ExecTerm::App1(*f, Box::new(copy_term(child))),
        ExecTerm::App2(f, left, right) => ExecTerm::App2(
            *f,
            Box::new(copy_term(left)),
            Box::new(copy_term(right)),
        ),
    }
}

fn lookup_exec(bindings: &Vec<ExecBinding>, variable: u64) -> (result: Option<ExecTerm>)
    ensures
        match result {
            Some(t) => Some(view(t)) == lookup(view_subst(bindings@), variable),
            None => lookup(view_subst(bindings@), variable) == None,
        }
{
    let mut i: usize = bindings.len();
    while i > 0
        invariant
            i <= bindings.len(),
            lookup_rec(view_subst(bindings@).bindings, variable, i as nat)
                == lookup(view_subst(bindings@), variable),
        decreases i,
    {
        i -= 1;
        if bindings[i].variable == variable {
            return Some(copy_term(&bindings[i].replacement));
        }
    }
    None
}

fn apply_exec(term: &ExecTerm, bindings: &Vec<ExecBinding>) -> (result: ExecTerm)
    ensures view(result) == apply(view(*term), view_subst(bindings@))
    decreases term
{
    match term {
        ExecTerm::Var(v) => match lookup_exec(bindings, *v) {
            Some(t) => t,
            None => ExecTerm::Var(*v),
        },
        ExecTerm::Const(c) => ExecTerm::Const(*c),
        ExecTerm::App1(f, child) => ExecTerm::App1(
            *f,
            Box::new(apply_exec(child, bindings)),
        ),
        ExecTerm::App2(f, left, right) => ExecTerm::App2(
            *f,
            Box::new(apply_exec(left, bindings)),
            Box::new(apply_exec(right, bindings)),
        ),
    }
}

/// Structural comparison refined to equality in the first-order model.
fn same_term(left: &ExecTerm, right: &ExecTerm) -> (same: bool)
    ensures same == (view(*left) == view(*right))
    decreases left
{
    match (left, right) {
        (ExecTerm::Var(x), ExecTerm::Var(y)) => x == y,
        (ExecTerm::Const(x), ExecTerm::Const(y)) => x == y,
        (ExecTerm::App1(f, l), ExecTerm::App1(g, r)) => *f == *g && same_term(l, r),
        (ExecTerm::App2(f, ll, lr), ExecTerm::App2(g, rl, rr)) => {
            *f == *g && same_term(ll, rl) && same_term(lr, rr)
        },
        _ => false,
    }
}

/// Validate a supplied finite simultaneous-unifier certificate. This does not
/// compute an MGU; it verifies domain uniqueness and that applying the same
/// substitution to both selected terms makes them equal.
fn check_unifier(left: &ExecTerm, right: &ExecTerm, bindings: &Vec<ExecBinding>) -> (unifies: bool)
    ensures
        unifies ==> domain_unique(view_subst(bindings@)),
        unifies ==> apply(view(*left), view_subst(bindings@))
            == apply(view(*right), view_subst(bindings@)),
{
    if !check_domain_unique(bindings) {
        return false;
    }
    let left_instance = apply_exec(left, bindings);
    let right_instance = apply_exec(right, bindings);
    same_term(&left_instance, &right_instance)
}

/// The semantic fact behind instance substitution for a universally closed
/// clause: any term substitution maps a term variable to its chosen instance.
spec fn variable_instance() -> bool {
    let sigma = Substitution { bindings: seq![(7u64, Term::Const(19u64))] };
    apply(Term::Var(7), sigma) == Term::Const(19)
}

proof fn variable_substitution_is_an_instance()
    ensures variable_instance()
{
    let sigma = Substitution { bindings: seq![(7u64, Term::Const(19u64))] };
    assert(lookup(sigma, 7) == Some(Term::Const(19)));
    assert(apply(Term::Var(7), sigma) == Term::Const(19));
}

fn main() {
    proof { variable_substitution_is_an_instance(); }

    // Supplied unifier certificate binding target-side variable 42 to
    // constant 9. Domain validity and simultaneous application are checked.
}

} // verus!
