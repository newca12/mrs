use vstd::prelude::*;

verus! {

/// First-order terms with variables, constants, and unary/binary functions.
pub enum Term {
    Var(u64),
    Const(u64),
    App1(u64, Box<Term>),
    App2(u64, Box<Term>, Box<Term>),
}

/// A term-tree position is a sequence of child indices. Unary applications
/// have child 0; binary applications have children 0 (left) and 1 (right).
pub struct TermPosition {
    path: Seq<nat>,
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

spec fn seq_equal<T>(left: Seq<T>, right: Seq<T>) -> bool {
    left.len() == right.len()
        && forall|i: int| 0 <= i < left.len() ==> left[i] == right[i]
}

spec fn clause_equal(left: Clause, right: Clause) -> bool {
    seq_equal(left.literals, right.literals)
}

pub struct Interpretation {
    domain: Set<u64>,
    // These finite maps act as total interpretations only when their domains
    // contain every signature application used by the formula. Semantic
    // lemmas state those definedness preconditions explicitly.
    functions: IMap<(u64, Seq<u64>), u64>,
    predicates: IMap<(u64, Seq<u64>), bool>,
}

pub struct Valuation {
    values: IMap<u64, u64>,
}

pub struct Model {
    interp: Interpretation,
    valuation: Valuation,
}

/// The semantic carrier is non-empty and all signature interpretations are
/// total and return carrier elements. Predicate interpretations are total as
/// well, so substituted argument tuples are always defined.
spec fn interpretation_well_formed(interp: Interpretation) -> bool {
    exists|x: u64| interp.domain.contains(x)
        && interp.functions.is_full()
        && interp.predicates.is_full()
        && forall|f: u64, args: Seq<u64>|
            interp.domain.contains(interp.functions[(f, args)])
}

spec fn valuation_well_formed(interp: Interpretation, valuation: Valuation) -> bool {
    valuation.values.is_full()
        && forall|x: u64| interp.domain.contains(valuation.values[x])
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

/// Replace the first occurrence of `needle` in a term, searching left to
/// right. The Boolean records whether a replacement was performed.
spec fn replace_term(term: Term, needle: Term, replacement: Term) -> (Term, bool)
    decreases term
{
    if term == needle {
        (replacement, true)
    } else {
        match term {
            Term::Var(_) | Term::Const(_) => (term, false),
            Term::App1(f, child) => {
                let (child2, changed) = replace_term(*child, needle, replacement);
                (Term::App1(f, Box::new(child2)), changed)
            },
            Term::App2(f, left, right) => {
                let (left2, changed_left) = replace_term(*left, needle, replacement);
                if changed_left {
                    (Term::App2(f, Box::new(left2), right), true)
                } else {
                    let (right2, changed_right) = replace_term(*right, needle, replacement);
                    (Term::App2(f, left, Box::new(right2)), changed_right)
                }
            },
        }
    }
}

spec fn term_at_position(term: Term, position: TermPosition, depth: nat) -> Option<Term>
    recommends depth <= position.path.len()
    decreases term
{
    if depth == position.path.len() {
        Some(term)
    } else {
        match term {
            Term::Var(_) | Term::Const(_) => None,
            Term::App1(_, child) => {
                if position.path[depth as int] == 0 {
                    term_at_position(*child, position, depth + 1)
                } else {
                    None
                }
            },
            Term::App2(_, left, right) => {
                if position.path[depth as int] == 0 {
                    term_at_position(*left, position, depth + 1)
                } else if position.path[depth as int] == 1 {
                    term_at_position(*right, position, depth + 1)
                } else {
                    None
                }
            },
        }
    }
}

spec fn replace_at_position(
    term: Term,
    position: TermPosition,
    replacement: Term,
    depth: nat,
) -> Option<Term>
    recommends depth <= position.path.len()
    decreases term
{
    if depth == position.path.len() {
        Some(replacement)
    } else {
        match term {
            Term::Var(_) | Term::Const(_) => None,
            Term::App1(f, child) => {
                if position.path[depth as int] == 0 {
                    match replace_at_position(*child, position, replacement, depth + 1) {
                        Some(child2) => Some(Term::App1(f, Box::new(child2))),
                        None => None,
                    }
                } else {
                    None
                }
            },
            Term::App2(f, left, right) => {
                if position.path[depth as int] == 0 {
                    match replace_at_position(*left, position, replacement, depth + 1) {
                        Some(left2) => Some(Term::App2(f, Box::new(left2), right)),
                        None => None,
                    }
                } else if position.path[depth as int] == 1 {
                    match replace_at_position(*right, position, replacement, depth + 1) {
                        Some(right2) => Some(Term::App2(f, left, Box::new(right2))),
                        None => None,
                    }
                } else {
                    None
                }
            },
        }
    }
}

proof fn replace_at_valid_position_exists(
    term: Term,
    position: TermPosition,
    replacement: Term,
    depth: nat,
)
    requires depth <= position.path.len(),
        term_at_position(term, position, depth).is_some(),
    ensures replace_at_position(term, position, replacement, depth).is_some()
    decreases position.path.len() - depth
{
    if depth == position.path.len() {
        assert(replace_at_position(term, position, replacement, depth)
            == Some(replacement));
    } else {
        match term {
            Term::Var(_) | Term::Const(_) => {
                assert(term_at_position(term, position, depth) == None);
            },
            Term::App1(_, child) => {
                assert(position.path[depth as int] == 0);
                assert(term_at_position(*child, position, depth + 1).is_some());
                replace_at_valid_position_exists(*child, position, replacement, depth + 1);
            },
            Term::App2(_, left, right) => {
                if position.path[depth as int] == 0 {
                    assert(term_at_position(*left, position, depth + 1).is_some());
                    replace_at_valid_position_exists(*left, position, replacement, depth + 1);
                } else {
                    assert(position.path[depth as int] == 1);
                    assert(term_at_position(*right, position, depth + 1).is_some());
                    replace_at_valid_position_exists(*right, position, replacement, depth + 1);
                }
            },
        }
    }
}

proof fn replacement_commutes_with_substitution_at_position(
    term: Term,
    sigma: Substitution,
    position: TermPosition,
    replacement: Term,
    depth: nat,
)
    requires depth <= position.path.len(),
        term_at_position(term, position, depth).is_some(),
    ensures replace_at_position(apply(term, sigma), position,
            apply(replacement, sigma), depth)
        == match replace_at_position(term, position, replacement, depth) {
            Some(rewritten) => Some(apply(rewritten, sigma)),
            None => None,
        }
    decreases position.path.len() - depth
{
    if depth == position.path.len() {
        assert(replace_at_position(term, position, replacement, depth) == Some(replacement));
        assert(replace_at_position(apply(term, sigma), position,
            apply(replacement, sigma), depth) == Some(apply(replacement, sigma)));
    } else {
        match term {
            Term::Var(_) | Term::Const(_) => {
                assert(term_at_position(term, position, depth) == None);
            },
            Term::App1(f, child) => {
                assert(position.path[depth as int] == 0);
                replacement_commutes_with_substitution_at_position(
                    *child, sigma, position, replacement, depth + 1,
                );
            },
            Term::App2(f, left, right) => {
                if position.path[depth as int] == 0 {
                    replacement_commutes_with_substitution_at_position(
                        *left, sigma, position, replacement, depth + 1,
                    );
                } else {
                    assert(position.path[depth as int] == 1);
                    replacement_commutes_with_substitution_at_position(
                        *right, sigma, position, replacement, depth + 1,
                    );
                }
            },
        }
    }
}

proof fn position_replacement_preserves_evaluation(
    term: Term,
    position: TermPosition,
    replacement: Term,
    interp: Interpretation,
    valuation: Valuation,
    depth: nat,
)
    requires
        interp.functions.is_full(),
        depth <= position.path.len(),
        term_at_position(term, position, depth).is_some(),
        match term_at_position(term, position, depth) {
            Some(found) => eval(found, interp, valuation) == eval(replacement, interp, valuation),
            None => false,
        },
    ensures
        replace_at_position(term, position, replacement, depth).is_some(),
        match replace_at_position(term, position, replacement, depth) {
            Some(replaced) => eval(replaced, interp, valuation) == eval(term, interp, valuation),
            None => false,
        }
    decreases term
{
    if depth == position.path.len() {
        assert(term_at_position(term, position, depth) == Some(term));
        assert(replace_at_position(term, position, replacement, depth) == Some(replacement));
    } else {
        match term {
            Term::Var(_) | Term::Const(_) => {
                assert(term_at_position(term, position, depth) == None);
            },
            Term::App1(f, child) => {
                assert(position.path[depth as int] == 0);
                position_replacement_preserves_evaluation(
                    *child, position, replacement, interp, valuation, depth + 1,
                );
                assert(replace_at_position(term, position, replacement, depth).is_some());
                assert(interp.functions.is_full());
            },
            Term::App2(f, left, right) => {
                if position.path[depth as int] == 0 {
                    position_replacement_preserves_evaluation(
                        *left, position, replacement, interp, valuation, depth + 1,
                    );
                    assert(replace_at_position(term, position, replacement, depth).is_some());
                } else {
                    assert(position.path[depth as int] == 1);
                    position_replacement_preserves_evaluation(
                        *right, position, replacement, interp, valuation, depth + 1,
                    );
                    assert(replace_at_position(term, position, replacement, depth).is_some());
                }
                assert(interp.functions.is_full());
            },
        }
    }
}

/// First-order function interpretations are congruent: replacing an
/// occurrence by a term with the same denotation preserves the whole term.
proof fn eval_after_replacing_equal_subterm(
    term: Term,
    needle: Term,
    replacement: Term,
    interp: Interpretation,
    valuation: Valuation,
)
    requires
        interp.functions.is_full(),
        eval(needle, interp, valuation) == eval(replacement, interp, valuation),
    ensures
        replace_term(term, needle, replacement).1
            ==> eval(replace_term(term, needle, replacement).0, interp, valuation)
                == eval(term, interp, valuation)
    decreases term
{
    if term == needle {
        assert(replace_term(term, needle, replacement) == (replacement, true));
    } else {
        match term {
            Term::Var(_) | Term::Const(_) => {},
            Term::App1(f, child) => {
                eval_after_replacing_equal_subterm(
                    *child, needle, replacement, interp, valuation,
                );
                if replace_term(*child, needle, replacement).1 {
                    assert(eval(replace_term(*child, needle, replacement).0, interp, valuation)
                        == eval(*child, interp, valuation));
                }
            },
            Term::App2(f, left, right) => {
                eval_after_replacing_equal_subterm(
                    *left, needle, replacement, interp, valuation,
                );
                if replace_term(*left, needle, replacement).1 {
                    assert(eval(replace_term(*left, needle, replacement).0, interp, valuation)
                        == eval(*left, interp, valuation));
                } else {
                    eval_after_replacing_equal_subterm(
                        *right, needle, replacement, interp, valuation,
                    );
                    if replace_term(*right, needle, replacement).1 {
                        assert(eval(replace_term(*right, needle, replacement).0, interp, valuation)
                            == eval(*right, interp, valuation));
                    }
                }
            },
        }
    }
}

proof fn eval_in_domain(term: Term, interp: Interpretation, valuation: Valuation)
    requires
        interpretation_well_formed(interp),
        valuation_well_formed(interp, valuation),
    ensures interp.domain.contains(eval(term, interp, valuation))
    decreases term
{
    match term {
        Term::Var(v) => {
            assert(interp.domain.contains(valuation.values[v]));
        },
        Term::Const(c) => {
            assert(interp.domain.contains(interp.functions[(c, seq![])]));
        },
        Term::App1(f, child) => {
            eval_in_domain(*child, interp, valuation);
            assert(interp.domain.contains(interp.functions[
                (f, seq![eval(*child, interp, valuation)])]));
        },
        Term::App2(f, left, right) => {
            eval_in_domain(*left, interp, valuation);
            eval_in_domain(*right, interp, valuation);
            assert(interp.domain.contains(interp.functions[(
                f,
                seq![eval(*left, interp, valuation), eval(*right, interp, valuation)],
            )]));
        },
    }
}

/// Update variables simultaneously: each replacement is interpreted using
/// the original valuation and unmapped variables retain their original value.
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

/// A model pointwise updated by the simultaneous substitution. Keeping the
/// denotation as a total IMap makes instance semantics available for every
/// variable in a parent clause.
spec fn update_valuation(interp: Interpretation, valuation: Valuation, sigma: Substitution)
    -> Valuation
{
    Valuation {
        values: IMap::total(|x: u64| match lookup(sigma, x) {
            Some(replacement) => eval(replacement, interp, valuation),
            None => valuation.values[x],
        }),
    }
}

proof fn updated_valuation_well_formed(
    interp: Interpretation,
    valuation: Valuation,
    sigma: Substitution,
)
    requires
        interpretation_well_formed(interp),
        valuation_well_formed(interp, valuation),
    ensures valuation_well_formed(interp, update_valuation(interp, valuation, sigma))
{
    assert forall|x: u64| interp.domain.contains(
        update_valuation(interp, valuation, sigma).values[x]) by {
        match lookup(sigma, x) {
            Some(replacement) => {
                eval_in_domain(replacement, interp, valuation);
            },
            None => {
                assert(interp.domain.contains(valuation.values[x]));
            },
        }
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
        interp.functions.is_full(),
        valuation.values.is_full(),
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

/// Replace one occurrence in an atom's term arguments, searching in argument
/// order. Predicate symbols and literal polarity are unchanged.
spec fn replace_atom(atom: Atom, needle: Term, replacement: Term) -> (Atom, bool)
    decreases atom
{
    match atom {
        Atom::Pred0(p) => (Atom::Pred0(p), false),
        Atom::Pred1(p, arg) => {
            let (arg2, changed) = replace_term(*arg, needle, replacement);
            (Atom::Pred1(p, Box::new(arg2)), changed)
        },
        Atom::Pred2(p, left, right) => {
            let (left2, changed_left) = replace_term(*left, needle, replacement);
            if changed_left {
                (Atom::Pred2(p, Box::new(left2), right), true)
            } else {
                let (right2, changed_right) = replace_term(*right, needle, replacement);
                (Atom::Pred2(p, left, Box::new(right2)), changed_right)
            }
        },
        Atom::Eq(left, right) => {
            let (left2, changed_left) = replace_term(left, needle, replacement);
            if changed_left {
                (Atom::Eq(left2, right), true)
            } else {
                let (right2, changed_right) = replace_term(right, needle, replacement);
                (Atom::Eq(left, right2), changed_right)
            }
        },
    }
}

proof fn atom_holds_after_equal_subterm_replacement(
    atom: Atom,
    needle: Term,
    replacement: Term,
    interp: Interpretation,
    valuation: Valuation,
)
    requires
        interp.functions.is_full(),
        interp.predicates.is_full(),
        eval(needle, interp, valuation) == eval(replacement, interp, valuation),
    ensures replace_atom(atom, needle, replacement).1
        ==> atom_holds(replace_atom(atom, needle, replacement).0, interp, valuation)
            == atom_holds(atom, interp, valuation)
    decreases atom
{
    match atom {
        Atom::Pred0(_) => {},
        Atom::Pred1(_, arg) => {
            eval_after_replacing_equal_subterm(*arg, needle, replacement, interp, valuation);
        },
        Atom::Pred2(_, left, right) => {
            eval_after_replacing_equal_subterm(*left, needle, replacement, interp, valuation);
            if !replace_term(*left, needle, replacement).1 {
                eval_after_replacing_equal_subterm(*right, needle, replacement, interp, valuation);
            }
        },
        Atom::Eq(left, right) => {
            eval_after_replacing_equal_subterm(left, needle, replacement, interp, valuation);
            if !replace_term(left, needle, replacement).1 {
                eval_after_replacing_equal_subterm(right, needle, replacement, interp, valuation);
            }
        },
    }
}

spec fn replace_literal(literal: Literal, needle: Term, replacement: Term) -> (Literal, bool) {
    let (atom2, changed) = replace_atom(literal.atom, needle, replacement);
    (Literal { positive: literal.positive, atom: atom2 }, changed)
}

spec fn atom_at_position(atom: Atom, slot: nat, position: TermPosition) -> Option<Term> {
    match atom {
        Atom::Pred0(_) => None,
        Atom::Pred1(_, arg) => {
            if slot == 0 { term_at_position(*arg, position, 0) } else { None }
        },
        Atom::Pred2(_, left, right) => {
            if slot == 0 {
                term_at_position(*left, position, 0)
            } else if slot == 1 {
                term_at_position(*right, position, 0)
            } else {
                None
            }
        },
        Atom::Eq(left, right) => {
            if slot == 0 {
                term_at_position(left, position, 0)
            } else if slot == 1 {
                term_at_position(right, position, 0)
            } else {
                None
            }
        },
    }
}

spec fn replace_atom_at_position(
    atom: Atom,
    slot: nat,
    position: TermPosition,
    replacement: Term,
) -> Option<Atom> {
    match atom {
        Atom::Pred0(_) => None,
        Atom::Pred1(p, arg) => {
            if slot == 0 {
                match replace_at_position(*arg, position, replacement, 0) {
                    Some(arg2) => Some(Atom::Pred1(p, Box::new(arg2))),
                    None => None,
                }
            } else { None }
        },
        Atom::Pred2(p, left, right) => {
            if slot == 0 {
                match replace_at_position(*left, position, replacement, 0) {
                    Some(left2) => Some(Atom::Pred2(p, Box::new(left2), right)),
                    None => None,
                }
            } else if slot == 1 {
                match replace_at_position(*right, position, replacement, 0) {
                    Some(right2) => Some(Atom::Pred2(p, left, Box::new(right2))),
                    None => None,
                }
            } else { None }
        },
        Atom::Eq(left, right) => {
            if slot == 0 {
                match replace_at_position(left, position, replacement, 0) {
                    Some(left2) => Some(Atom::Eq(left2, right)),
                    None => None,
                }
            } else if slot == 1 {
                match replace_at_position(right, position, replacement, 0) {
                    Some(right2) => Some(Atom::Eq(left, right2)),
                    None => None,
                }
            } else { None }
        },
    }
}

spec fn literal_at_position(
    literal: Literal,
    slot: nat,
    position: TermPosition,
) -> Option<Term> {
    atom_at_position(literal.atom, slot, position)
}

spec fn term_variables(term: Term) -> Set<u64>
    decreases term
{
    match term {
        Term::Var(v) => Set::empty().insert(v),
        Term::Const(_) => Set::empty(),
        Term::App1(_, child) => term_variables(*child),
        Term::App2(_, left, right) => term_variables(*left).union(term_variables(*right)),
    }
}

spec fn atom_variables(atom: Atom) -> Set<u64>
    decreases atom
{
    match atom {
        Atom::Pred0(_) => Set::empty(),
        Atom::Pred1(_, arg) => term_variables(*arg),
        Atom::Pred2(_, left, right) => term_variables(*left).union(term_variables(*right)),
        Atom::Eq(left, right) => term_variables(left).union(term_variables(right)),
    }
}

spec fn literal_variables(literal: Literal) -> Set<u64> {
    atom_variables(literal.atom)
}

spec fn renaming_substitution(sigma: Substitution) -> bool {
    forall|v: u64| lookup(sigma, v).is_some()
        ==> exists|fresh: u64| lookup(sigma, v) == Some(Term::Var(fresh))
}

spec fn renames_variables_on(sigma: Substitution, variables: Set<u64>) -> bool {
    forall|v: u64| variables.contains(v)
        ==> exists|fresh: u64| lookup(sigma, v) == Some(Term::Var(fresh))
}

proof fn renaming_restricts_to_subset(sigma: Substitution, subset: Set<u64>, superset: Set<u64>)
    requires renames_variables_on(sigma, superset),
        forall|v: u64| subset.contains(v) ==> superset.contains(v),
    ensures renames_variables_on(sigma, subset)
{}

proof fn apply_view_exec_term(term: ExecTerm, bindings: Seq<ExecBinding>)
    ensures apply(view(term), view_subst(bindings)) == view(apply_exec_model(term, bindings))
    decreases term
{
    match term {
        ExecTerm::Var(v) => {
            match lookup(view_subst(bindings), v) {
                Some(t) => {
                    assert(apply(Term::Var(v), view_subst(bindings)) == t);
                    assert(apply_exec_model(term, bindings) == view_to_exec(t));
                    view_to_exec_roundtrip(t);
                },
                None => assert(view(apply_exec_model(term, bindings)) == Term::Var(v)),
            }
        },
        ExecTerm::Const(_) => {},
        ExecTerm::App1(f, child) => apply_view_exec_term(*child, bindings),
        ExecTerm::App2(f, left, right) => {
            apply_view_exec_term(*left, bindings);
            apply_view_exec_term(*right, bindings);
        },
    }
}

proof fn renaming_preserves_selected_position(
    literal: Literal,
    sigma: Substitution,
    slot: nat,
    position: TermPosition,
    variables: Set<u64>,
)
    requires
        renaming_substitution(sigma),
        renames_variables_on(sigma, variables),
        forall|v: u64| literal_variables(literal).contains(v) ==> variables.contains(v),
    ensures match literal_at_position(literal, slot, position) {
        Some(term) => literal_at_position(substitute_literal(literal, sigma), slot, position)
            == Some(apply(term, sigma)),
        None => literal_at_position(substitute_literal(literal, sigma), slot, position) == None,
    }
{
    match literal.atom {
        Atom::Pred0(_) => {},
        Atom::Pred1(_, arg) => {
            if slot == 0 {
                term_position_substitution(*arg, sigma, position, 0);
            }
        },
        Atom::Pred2(_, left, right) => {
            if slot == 0 {
                term_position_substitution(*left, sigma, position, 0);
            } else if slot == 1 {
                term_position_substitution(*right, sigma, position, 0);
            }
        },
        Atom::Eq(left, right) => {
            if slot == 0 {
                term_position_substitution(left, sigma, position, 0);
            } else if slot == 1 {
                term_position_substitution(right, sigma, position, 0);
            }
        },
    }
}

proof fn executable_freshening_preserves_selected_position(
    literal: Literal,
    sigma: Substitution,
    variables: Set<u64>,
    slot: nat,
    position: TermPosition,
)
    requires
        renaming_substitution(sigma),
        renames_variables_on(sigma, variables),
        forall|v: u64| literal_variables(literal).contains(v) ==> variables.contains(v),
    ensures match literal_at_position(literal, slot, position) {
        Some(term) => literal_at_position(substitute_literal(literal, sigma), slot, position)
            == Some(apply(term, sigma)),
        None => literal_at_position(substitute_literal(literal, sigma), slot, position) == None,
    }
{
    renaming_preserves_selected_position(literal, sigma, slot, position, variables);
}

/// Lift successful path stability through atoms and literal polarity. This is
/// the certificate-facing position fact for a general unifier, which may map
/// the selected subterm itself to a non-variable term.
proof fn literal_position_substitution_success(
    literal: Literal,
    sigma: Substitution,
    slot: nat,
    position: TermPosition,
)
    requires literal_at_position(literal, slot, position).is_some(),
    ensures match literal_at_position(literal, slot, position) {
        Some(found) => literal_at_position(
            substitute_literal(literal, sigma), slot, position,
        ) == Some(apply(found, sigma)),
        None => false,
    }
{
    match literal.atom {
        Atom::Pred0(_) => {},
        Atom::Pred1(_, arg) => {
            assert(slot == 0);
            assert(term_at_position(*arg, position, 0).is_some());
            term_position_substitution_success(*arg, sigma, position, 0);
        },
        Atom::Pred2(_, left, right) => {
            if slot == 0 {
                assert(term_at_position(*left, position, 0).is_some());
                term_position_substitution_success(*left, sigma, position, 0);
            } else {
                assert(slot == 1);
                assert(term_at_position(*right, position, 0).is_some());
                term_position_substitution_success(*right, sigma, position, 0);
            }
        },
        Atom::Eq(left, right) => {
            if slot == 0 {
                assert(term_at_position(left, position, 0).is_some());
                term_position_substitution_success(left, sigma, position, 0);
            } else {
                assert(slot == 1);
                assert(term_at_position(right, position, 0).is_some());
                term_position_substitution_success(right, sigma, position, 0);
            }
        },
    }
}

proof fn atom_position_substitution_success(
    atom: Atom,
    sigma: Substitution,
    slot: nat,
    position: TermPosition,
)
    requires atom_at_position(atom, slot, position).is_some(),
    ensures match atom_at_position(atom, slot, position) {
        Some(found) => atom_at_position(substitute_atom(atom, sigma), slot, position)
            == Some(apply(found, sigma)),
        None => false,
    }
{
    match atom {
        Atom::Pred0(_) => {},
        Atom::Pred1(_, arg) => {
            assert(slot == 0);
            assert(term_at_position(*arg, position, 0).is_some());
            term_position_substitution_success(*arg, sigma, position, 0);
        },
        Atom::Pred2(_, left, right) => {
            if slot == 0 {
                assert(term_at_position(*left, position, 0).is_some());
                term_position_substitution_success(*left, sigma, position, 0);
            } else {
                assert(slot == 1);
                assert(term_at_position(*right, position, 0).is_some());
                term_position_substitution_success(*right, sigma, position, 0);
            }
        },
        Atom::Eq(left, right) => {
            if slot == 0 {
                assert(term_at_position(left, position, 0).is_some());
                term_position_substitution_success(left, sigma, position, 0);
            } else {
                assert(slot == 1);
                assert(term_at_position(right, position, 0).is_some());
                term_position_substitution_success(right, sigma, position, 0);
            }
        },
    }
}

proof fn term_position_substitution(
    term: Term,
    sigma: Substitution,
    position: TermPosition,
    depth: nat,
)
    requires depth <= position.path.len(), renaming_substitution(sigma)
    ensures match term_at_position(term, position, depth) {
        Some(found) => term_at_position(apply(term, sigma), position, depth)
            == Some(apply(found, sigma)),
        None => term_at_position(apply(term, sigma), position, depth) == None,
    }
    decreases position.path.len() - depth
{
    if depth == position.path.len() {
        assert(term_at_position(term, position, depth) == Some(term));
        assert(term_at_position(apply(term, sigma), position, depth)
            == Some(apply(term, sigma)));
    } else {
        match term {
            Term::Var(_) | Term::Const(_) => {
                assert(term_at_position(term, position, depth) == None);
                match term {
                    Term::Var(v) => match lookup(sigma, v) {
                        Some(Term::Var(fresh)) => {
                            assert(apply(term, sigma) == Term::Var(fresh));
                            assert(term_at_position(Term::Var(fresh), position, depth) == None);
                        },
                        _ => {},
                    },
                    Term::Const(_) => {},
                    _ => {},
                }
            },
            Term::App1(f, child) => {
                if position.path[depth as int] == 0 {
                    term_position_substitution(*child, sigma, position, depth + 1);
                } else {
                    assert(term_at_position(term, position, depth) == None);
                    assert(term_at_position(apply(term, sigma), position, depth) == None);
                }
            },
            Term::App2(f, left, right) => {
                if position.path[depth as int] == 0 {
                    term_position_substitution(*left, sigma, position, depth + 1);
                } else if position.path[depth as int] == 1 {
                    term_position_substitution(*right, sigma, position, depth + 1);
                } else {
                    assert(term_at_position(term, position, depth) == None);
                    assert(term_at_position(apply(term, sigma), position, depth) == None);
                }
            },
        }
    }
}

/// A substitution preserves any already-valid function-tree position, even
/// when it maps the variable at that position to a compound term. The
/// renaming-only lemma above also characterizes invalid positions; this
/// success-only version is the one needed after general unification.
proof fn term_position_substitution_success(
    term: Term,
    sigma: Substitution,
    position: TermPosition,
    depth: nat,
)
    requires
        depth <= position.path.len(),
        term_at_position(term, position, depth).is_some(),
    ensures match term_at_position(term, position, depth) {
        Some(found) => term_at_position(apply(term, sigma), position, depth)
            == Some(apply(found, sigma)),
        None => false,
    }
    decreases position.path.len() - depth
{
    if depth == position.path.len() {
        assert(term_at_position(term, position, depth) == Some(term));
        assert(term_at_position(apply(term, sigma), position, depth)
            == Some(apply(term, sigma)));
    } else {
        match term {
            Term::Var(_) | Term::Const(_) => {
                assert(term_at_position(term, position, depth) == None);
            },
            Term::App1(f, child) => {
                assert(position.path[depth as int] == 0);
                assert(term_at_position(*child, position, depth + 1).is_some());
                term_position_substitution_success(*child, sigma, position, depth + 1);
            },
            Term::App2(f, left, right) => {
                if position.path[depth as int] == 0 {
                    assert(term_at_position(*left, position, depth + 1).is_some());
                    term_position_substitution_success(*left, sigma, position, depth + 1);
                } else {
                    assert(position.path[depth as int] == 1);
                    assert(term_at_position(*right, position, depth + 1).is_some());
                    term_position_substitution_success(*right, sigma, position, depth + 1);
                }
            },
        }
    }
}

spec fn replace_literal_at_position(
    literal: Literal,
    slot: nat,
    position: TermPosition,
    replacement: Term,
) -> Option<Literal> {
    match replace_atom_at_position(literal.atom, slot, position, replacement) {
        Some(atom2) => Some(Literal { positive: literal.positive, atom: atom2 }),
        None => None,
    }
}

proof fn atom_holds_after_position_replacement(
    atom: Atom,
    slot: nat,
    position: TermPosition,
    replacement: Term,
    interp: Interpretation,
    valuation: Valuation,
)
    requires
        interp.functions.is_full(),
        interp.predicates.is_full(),
        atom_at_position(atom, slot, position).is_some(),
        replace_atom_at_position(atom, slot, position, replacement).is_some(),
        match atom_at_position(atom, slot, position) {
            Some(found) => eval(found, interp, valuation) == eval(replacement, interp, valuation),
            None => false,
        },
    ensures match replace_atom_at_position(atom, slot, position, replacement) {
        Some(replaced) => atom_holds(replaced, interp, valuation)
            == atom_holds(atom, interp, valuation),
        None => false,
    }
{
    match atom {
        Atom::Pred0(_) => {},
        Atom::Pred1(_, arg) => {
            position_replacement_preserves_evaluation(
                *arg, position, replacement, interp, valuation, 0,
            );
        },
        Atom::Pred2(_, left, right) => {
            if slot == 0 {
                position_replacement_preserves_evaluation(
                    *left, position, replacement, interp, valuation, 0,
                );
            } else {
                position_replacement_preserves_evaluation(
                    *right, position, replacement, interp, valuation, 0,
                );
            }
        },
        Atom::Eq(left, right) => {
            if slot == 0 {
                position_replacement_preserves_evaluation(
                    left, position, replacement, interp, valuation, 0,
                );
            } else {
                position_replacement_preserves_evaluation(
                    right, position, replacement, interp, valuation, 0,
                );
            }
        },
    }
}

proof fn literal_holds_after_position_replacement(
    literal: Literal,
    slot: nat,
    position: TermPosition,
    replacement: Term,
    interp: Interpretation,
    valuation: Valuation,
)
    requires
        interp.functions.is_full(),
        interp.predicates.is_full(),
        literal_at_position(literal, slot, position).is_some(),
        replace_literal_at_position(literal, slot, position, replacement).is_some(),
        match literal_at_position(literal, slot, position) {
            Some(found) => eval(found, interp, valuation) == eval(replacement, interp, valuation),
            None => false,
        },
    ensures match replace_literal_at_position(literal, slot, position, replacement) {
        Some(replaced) => literal_holds(replaced, interp, valuation)
            == literal_holds(literal, interp, valuation),
        None => false,
    }
{
    atom_holds_after_position_replacement(
        literal.atom, slot, position, replacement, interp, valuation,
    );
}

proof fn literal_position_replacement_after_substitution(
    literal: Literal,
    sigma: Substitution,
    slot: nat,
    position: TermPosition,
    replacement: Term,
)
    requires literal_at_position(literal, slot, position).is_some(),
        replace_literal_at_position(literal, slot, position, replacement).is_some(),
    ensures replace_literal_at_position(
        substitute_literal(literal, sigma), slot, position,
        apply(replacement, sigma),
    ).is_some(),
        replace_literal_at_position(
            substitute_literal(literal, sigma), slot, position,
            apply(replacement, sigma),
        ) == match replace_literal_at_position(literal, slot, position, replacement) {
            Some(rewritten) => Some(substitute_literal(rewritten, sigma)),
            None => None,
        }
{
    literal_position_substitution_success(literal, sigma, slot, position);
    match literal.atom {
        Atom::Pred0(_) => {},
        Atom::Pred1(p, arg) => {
            assert(slot == 0);
            term_position_substitution_success(*arg, sigma, position, 0);
            replace_at_valid_position_exists(
                apply(*arg, sigma), position, apply(replacement, sigma), 0,
            );
            replacement_commutes_with_substitution_at_position(
                *arg, sigma, position, replacement, 0,
            );
        },
        Atom::Pred2(p, left, right) => {
            if slot == 0 {
                assert(term_at_position(*left, position, 0).is_some());
                term_position_substitution_success(*left, sigma, position, 0);
                replace_at_valid_position_exists(
                    apply(*left, sigma), position, apply(replacement, sigma), 0,
                );
                replacement_commutes_with_substitution_at_position(
                    *left, sigma, position, replacement, 0,
                );
            } else {
                assert(slot == 1);
                assert(term_at_position(*right, position, 0).is_some());
                term_position_substitution_success(*right, sigma, position, 0);
                replace_at_valid_position_exists(
                    apply(*right, sigma), position, apply(replacement, sigma), 0,
                );
                replacement_commutes_with_substitution_at_position(
                    *right, sigma, position, replacement, 0,
                );
            }
        },
        Atom::Eq(left, right) => {
            if slot == 0 {
                assert(term_at_position(left, position, 0).is_some());
                term_position_substitution_success(left, sigma, position, 0);
                replace_at_valid_position_exists(
                    apply(left, sigma), position, apply(replacement, sigma), 0,
                );
                replacement_commutes_with_substitution_at_position(
                    left, sigma, position, replacement, 0,
                );
            } else {
                assert(slot == 1);
                assert(term_at_position(right, position, 0).is_some());
                term_position_substitution_success(right, sigma, position, 0);
                replace_at_valid_position_exists(
                    apply(right, sigma), position, apply(replacement, sigma), 0,
                );
                replacement_commutes_with_substitution_at_position(
                    right, sigma, position, replacement, 0,
                );
            }
        },
    }
}

proof fn atom_position_replacement_commutes_with_substitution(
    atom: Atom,
    sigma: Substitution,
    slot: nat,
    position: TermPosition,
    replacement: Term,
)
    requires atom_at_position(atom, slot, position).is_some(),
        replace_atom_at_position(atom, slot, position, replacement).is_some(),
    ensures replace_atom_at_position(
        substitute_atom(atom, sigma), slot, position, apply(replacement, sigma),
    ) == match replace_atom_at_position(atom, slot, position, replacement) {
        Some(rewritten) => Some(substitute_atom(rewritten, sigma)),
        None => None,
    }
{
    match atom {
        Atom::Pred0(_) => {},
        Atom::Pred1(p, arg) => {
            assert(slot == 0);
            replacement_commutes_with_substitution_at_position(
                *arg, sigma, position, replacement, 0,
            );
        },
        Atom::Pred2(p, left, right) => {
            if slot == 0 {
                replacement_commutes_with_substitution_at_position(
                    *left, sigma, position, replacement, 0,
                );
            } else {
                assert(slot == 1);
                replacement_commutes_with_substitution_at_position(
                    *right, sigma, position, replacement, 0,
                );
            }
        },
        Atom::Eq(left, right) => {
            if slot == 0 {
                replacement_commutes_with_substitution_at_position(
                    left, sigma, position, replacement, 0,
                );
            } else {
                assert(slot == 1);
                replacement_commutes_with_substitution_at_position(
                    right, sigma, position, replacement, 0,
                );
            }
        },
    }
}

proof fn literal_position_replacement_commutes_with_substitution(
    literal: Literal,
    sigma: Substitution,
    slot: nat,
    position: TermPosition,
    replacement: Term,
)
    requires literal_at_position(literal, slot, position).is_some(),
        replace_literal_at_position(literal, slot, position, replacement).is_some(),
    ensures replace_literal_at_position(
        substitute_literal(literal, sigma), slot, position, apply(replacement, sigma),
    ) == match replace_literal_at_position(literal, slot, position, replacement) {
        Some(rewritten) => Some(substitute_literal(rewritten, sigma)),
        None => None,
    }
{
    atom_position_replacement_commutes_with_substitution(
        literal.atom, sigma, slot, position, replacement,
    );
}

proof fn literal_holds_after_equal_subterm_replacement(
    literal: Literal,
    needle: Term,
    replacement: Term,
    interp: Interpretation,
    valuation: Valuation,
)
    requires
        interp.functions.is_full(),
        interp.predicates.is_full(),
        eval(needle, interp, valuation) == eval(replacement, interp, valuation),
    ensures replace_literal(literal, needle, replacement).1
        ==> literal_holds(replace_literal(literal, needle, replacement).0, interp, valuation)
            == literal_holds(literal, interp, valuation)
{
    atom_holds_after_equal_subterm_replacement(
        literal.atom, needle, replacement, interp, valuation,
    );
}

/// Semantic core of ordinary superposition. The parents have already been
/// standardized apart and instantiated by the same proposed unifier. Their
/// non-selected literals are represented by `source_context` and
/// `target_context`; `target_literal` is the selected target literal. If the
/// positive source equality's left side and the selected target subterm have
/// equal denotation, and the equality holds, replacing that occurrence by the
/// right side preserves the target literal. Therefore the conclusion context
/// is entailed by the two instantiated parents.
proof fn superposition_context_sound(
    source_context: Clause,
    source_equality: Literal,
    target_context: Clause,
    target_literal: Literal,
    target_subterm: Term,
    source_left: Term,
    source_right: Term,
    interp: Interpretation,
    valuation: Valuation,
)
    requires
        interp.functions.is_full(),
        interp.predicates.is_full(),
        valuation.values.is_full(),
        source_equality.positive,
        source_equality.atom == Atom::Eq(source_left, source_right),
        clause_holds(source_context, interp, valuation)
            || literal_holds(source_equality, interp, valuation),
        clause_holds(target_context, interp, valuation)
            || literal_holds(target_literal, interp, valuation),
        eval(source_left, interp, valuation) == eval(target_subterm, interp, valuation),
        replace_literal(target_literal, target_subterm, source_right).1,
    ensures
        clause_holds(source_context, interp, valuation)
            || clause_holds(target_context, interp, valuation)
            || literal_holds(
                replace_literal(target_literal, target_subterm, source_right).0,
                interp,
                valuation,
            )
{
    if clause_holds(source_context, interp, valuation) {
    } else if clause_holds(target_context, interp, valuation) {
    } else {
        assert(literal_holds(source_equality, interp, valuation));
        assert(literal_holds(target_literal, interp, valuation));
        assert(atom_holds(source_equality.atom, interp, valuation));
        assert(eval(source_left, interp, valuation) == eval(source_right, interp, valuation));
        assert(eval(target_subterm, interp, valuation) == eval(source_right, interp, valuation));
        literal_holds_after_equal_subterm_replacement(
            target_literal,
            target_subterm,
            source_right,
            interp,
            valuation,
        );
    }
}

/// Substitution preserves atom truth when the valuation is updated
/// simultaneously. Total predicate interpretations are essential here: they
/// guarantee meaning for every substituted argument tuple.
proof fn atom_holds_after_substitution(
    atom: Atom,
    sigma: Substitution,
    interp: Interpretation,
    valuation: Valuation,
)
    requires
        interp.functions.is_full(),
        interp.predicates.is_full(),
        valuation.values.is_full(),
    ensures atom_holds(substitute_atom(atom, sigma), interp, valuation)
        == atom_holds(atom, interp, update_valuation(interp, valuation, sigma))
    decreases atom
{
    match atom {
        Atom::Pred0(_) => {},
        Atom::Pred1(_, arg) => {
            eval_after_substitution(*arg, sigma, interp, valuation);
        },
        Atom::Pred2(_, left, right) => {
            eval_after_substitution(*left, sigma, interp, valuation);
            eval_after_substitution(*right, sigma, interp, valuation);
        },
        Atom::Eq(left, right) => {
            eval_after_substitution(left, sigma, interp, valuation);
            eval_after_substitution(right, sigma, interp, valuation);
        },
    }
}

spec fn substitute_atom(atom: Atom, sigma: Substitution) -> Atom
    decreases atom
{
    match atom {
        Atom::Pred0(p) => Atom::Pred0(p),
        Atom::Pred1(p, arg) => Atom::Pred1(p, Box::new(apply(*arg, sigma))),
        Atom::Pred2(p, left, right) =>
            Atom::Pred2(p, Box::new(apply(*left, sigma)), Box::new(apply(*right, sigma))),
        Atom::Eq(left, right) => Atom::Eq(apply(left, sigma), apply(right, sigma)),
    }
}

spec fn substitute_literal(literal: Literal, sigma: Substitution) -> Literal {
    Literal { positive: literal.positive, atom: substitute_atom(literal.atom, sigma) }
}

spec fn substitute_clause(clause: Clause, sigma: Substitution) -> Clause {
    Clause {
        literals: clause.literals.map(|_i: int, literal: Literal|
            substitute_literal(literal, sigma)),
    }
}

spec fn replace_clause_literal(
    clause: Clause,
    literal_index: int,
    slot: nat,
    position: TermPosition,
    replacement: Term,
) -> Option<Clause> {
    if 0 <= literal_index < clause.literals.len() {
        match replace_literal_at_position(
            clause.literals[literal_index], slot, position, replacement,
        ) {
            Some(literal) => Some(Clause {
                literals: clause.literals.update(literal_index, literal),
            }),
            None => None,
        }
    } else {
        None
    }
}

spec fn literal_holds(literal: Literal, interp: Interpretation, valuation: Valuation) -> bool {
    literal.positive == atom_holds(literal.atom, interp, valuation)
}

spec fn clause_holds(clause: Clause, interp: Interpretation, valuation: Valuation) -> bool {
    exists|i: int| 0 <= i < clause.literals.len()
        && literal_holds(clause.literals[i], interp, valuation)
}

/// Universal closure semantics: a clause is true in an interpretation when
/// it holds for every valuation of its universally quantified variables into
/// the non-empty carrier.
spec fn clause_valid(clause: Clause, interp: Interpretation) -> bool {
    forall|valuation: Valuation|
        valuation_well_formed(interp, valuation)
            ==> clause_holds(clause, interp, valuation)
}

proof fn literal_holds_after_substitution(
    literal: Literal,
    sigma: Substitution,
    interp: Interpretation,
    valuation: Valuation,
)
    requires
        interp.functions.is_full(),
        interp.predicates.is_full(),
        valuation.values.is_full(),
    ensures literal_holds(substitute_literal(literal, sigma), interp, valuation)
        == literal_holds(literal, interp, update_valuation(interp, valuation, sigma))
{
    atom_holds_after_substitution(literal.atom, sigma, interp, valuation);
}

proof fn clause_holds_after_substitution(
    clause: Clause,
    sigma: Substitution,
    interp: Interpretation,
    valuation: Valuation,
)
    requires
        interp.functions.is_full(),
        interp.predicates.is_full(),
        valuation.values.is_full(),
    ensures clause_holds(substitute_clause(clause, sigma), interp, valuation)
        == clause_holds(clause, interp, update_valuation(interp, valuation, sigma))
{
    assert forall|i: int| 0 <= i < clause.literals.len()
        implies literal_holds(substitute_clause(clause, sigma).literals[i], interp, valuation)
            == literal_holds(clause.literals[i], interp,
                update_valuation(interp, valuation, sigma)) by {
        assert(substitute_clause(clause, sigma).literals[i]
            == substitute_literal(clause.literals[i], sigma));
        literal_holds_after_substitution(clause.literals[i], sigma, interp, valuation);
    }
}

/// Every simultaneous substitution instance of a universally closed clause
/// is entailed by that clause. This is the semantic fact that permits a
/// unifier to instantiate variables from either standardized-apart parent.
proof fn universal_clause_substitution_instance(
    clause: Clause,
    sigma: Substitution,
    interp: Interpretation,
    valuation: Valuation,
)
    requires
        interpretation_well_formed(interp),
        clause_valid(clause, interp),
        valuation_well_formed(interp, valuation),
    ensures clause_holds(substitute_clause(clause, sigma), interp, valuation)
{
    updated_valuation_well_formed(interp, valuation, sigma);
    assert(clause_holds(clause, interp, update_valuation(interp, valuation, sigma)));
    clause_holds_after_substitution(clause, sigma, interp, valuation);
}

spec fn append_literal(clause: Clause, literal: Literal) -> Clause {
    Clause { literals: clause.literals.push(literal) }
}

spec fn clause_concat(left: Clause, right: Clause) -> Clause {
    Clause { literals: left.literals + right.literals }
}

spec fn singleton_clause(literal: Literal) -> Clause {
    Clause { literals: seq![literal] }
}

proof fn clause_holds_concat(left: Clause, right: Clause, interp: Interpretation,
    valuation: Valuation)
    ensures clause_holds(clause_concat(left, right), interp, valuation)
        == (clause_holds(left, interp, valuation) || clause_holds(right, interp, valuation))
{
    let combined = clause_concat(left, right);
    if clause_holds(combined, interp, valuation) {
        let i = choose|i: int| 0 <= i < combined.literals.len()
            && literal_holds(combined.literals[i], interp, valuation);
        if i < left.literals.len() {
            assert(combined.literals[i] == left.literals[i]);
            assert(clause_holds(left, interp, valuation));
        } else {
            let j = i - left.literals.len();
            assert(0 <= j < right.literals.len());
            assert(combined.literals[i] == right.literals[j]);
            assert(clause_holds(right, interp, valuation));
        }
    } else {
        if clause_holds(left, interp, valuation) {
            let i = choose|i: int| 0 <= i < left.literals.len()
                && literal_holds(left.literals[i], interp, valuation);
            assert(combined.literals[i] == left.literals[i]);
            assert(clause_holds(combined, interp, valuation));
        } else if clause_holds(right, interp, valuation) {
            let j = choose|j: int| 0 <= j < right.literals.len()
                && literal_holds(right.literals[j], interp, valuation);
            let i = left.literals.len() + j;
            assert(combined.literals[i] == right.literals[j]);
            assert(clause_holds(combined, interp, valuation));
        }
    }
}

proof fn clause_holds_singleton(literal: Literal, interp: Interpretation, valuation: Valuation)
    ensures clause_holds(singleton_clause(literal), interp, valuation)
        == literal_holds(literal, interp, valuation)
{
    if clause_holds(singleton_clause(literal), interp, valuation) {
        let i = choose|i: int| 0 <= i < 1 && literal_holds(
            singleton_clause(literal).literals[i], interp, valuation);
        assert(i == 0);
        assert(singleton_clause(literal).literals[0] == literal);
    } else if literal_holds(literal, interp, valuation) {
        assert(singleton_clause(literal).literals[0] == literal);
        assert(clause_holds(singleton_clause(literal), interp, valuation));
    }
}

/// If the selected source equality and target literal are the final entries
/// of their standardized, unifier-instantiated parents, the checker resolvent
/// is the concatenation of the non-equality source context and the rewritten
/// target parent. This converts the semantic disjunction theorem into ordinary
/// clause validity for the emitted binary resolvent.
proof fn standardized_superposition_resolvent_valid(
    source_context: Clause,
    source_equality: Literal,
    target_context: Clause,
    target_literal: Literal,
    source_left: Term,
    source_right: Term,
    target_subterm: Term,
    target_slot: nat,
    target_position: TermPosition,
    source_freshening: Substitution,
    target_freshening: Substitution,
    unifier: Substitution,
    interp: Interpretation,
)
    requires
        interpretation_well_formed(interp),
        clause_valid(append_literal(source_context, source_equality), interp),
        clause_valid(append_literal(target_context, target_literal), interp),
        source_equality.positive,
        source_equality.atom == Atom::Eq(source_left, source_right),
        apply(apply(source_left, source_freshening), unifier)
            == apply(apply(target_subterm, target_freshening), unifier),
        literal_at_position(
            substitute_literal(substitute_literal(target_literal, target_freshening), unifier),
            target_slot,
            target_position,
        ) == Some(apply(apply(target_subterm, target_freshening), unifier)),
        replace_literal_at_position(
            substitute_literal(substitute_literal(target_literal, target_freshening), unifier),
            target_slot,
            target_position,
            apply(apply(source_right, source_freshening), unifier),
        ).is_some(),
    ensures clause_valid(clause_concat(
        clause_concat(
            substitute_clause(substitute_clause(source_context, source_freshening), unifier),
            substitute_clause(substitute_clause(target_context, target_freshening), unifier),
        ),
        singleton_clause(match replace_literal_at_position(
            substitute_literal(substitute_literal(target_literal, target_freshening), unifier),
            target_slot,
            target_position,
            apply(apply(source_right, source_freshening), unifier),
        ) {
            Some(rewritten) => rewritten,
            None => target_literal,
        }),
    ), interp)
{
    assert forall|valuation: Valuation| valuation_well_formed(interp, valuation)
        implies clause_holds(clause_concat(
            clause_concat(
                substitute_clause(substitute_clause(source_context, source_freshening), unifier),
                substitute_clause(substitute_clause(target_context, target_freshening), unifier),
            ),
            singleton_clause(match replace_literal_at_position(
                substitute_literal(substitute_literal(target_literal, target_freshening), unifier),
                target_slot,
                target_position,
                apply(apply(source_right, source_freshening), unifier),
            ) {
                Some(rewritten) => rewritten,
                None => target_literal,
            }),
        ), interp, valuation) by {
        standardized_superposition_sound(
            source_context,
            source_equality,
            target_context,
            target_literal,
            source_left,
            source_right,
            target_subterm,
            target_slot,
            target_position,
            source_freshening,
            target_freshening,
            unifier,
            interp,
            valuation,
        );
        let rewritten_literal = match replace_literal_at_position(
            substitute_literal(substitute_literal(target_literal, target_freshening), unifier),
            target_slot,
            target_position,
            apply(apply(source_right, source_freshening), unifier),
        ) {
            Some(rewritten) => rewritten,
            None => target_literal,
        };
        clause_holds_singleton(rewritten_literal, interp, valuation);
        clause_holds_concat(
            substitute_clause(substitute_clause(source_context, source_freshening), unifier),
            substitute_clause(substitute_clause(target_context, target_freshening), unifier),
            interp,
            valuation,
        );
        clause_holds_concat(
            clause_concat(
                substitute_clause(substitute_clause(source_context, source_freshening), unifier),
                substitute_clause(substitute_clause(target_context, target_freshening), unifier),
            ),
            singleton_clause(rewritten_literal),
            interp,
            valuation,
        );
    }
}

proof fn clause_holds_after_appending_literal(
    clause: Clause,
    literal: Literal,
    interp: Interpretation,
    valuation: Valuation,
)
    ensures clause_holds(append_literal(clause, literal), interp, valuation)
        == (clause_holds(clause, interp, valuation)
            || literal_holds(literal, interp, valuation))
{
    let extended = append_literal(clause, literal);
    if clause_holds(extended, interp, valuation) {
        let index = choose|i: int| 0 <= i < extended.literals.len()
            && literal_holds(extended.literals[i], interp, valuation);
        assert(extended.literals.len() == clause.literals.len() + 1);
        if index < clause.literals.len() {
            assert(clause.literals[index] == extended.literals[index]);
            assert(clause_holds(clause, interp, valuation));
        } else {
            assert(index == clause.literals.len());
            assert(extended.literals[index] == literal);
            assert(literal_holds(literal, interp, valuation));
        }
    } else {
        if clause_holds(clause, interp, valuation) {
            let index = choose|i: int| 0 <= i < clause.literals.len()
                && literal_holds(clause.literals[i], interp, valuation);
            assert(index < extended.literals.len());
            assert(extended.literals[index] == clause.literals[index]);
            assert(clause_holds(extended, interp, valuation));
        } else if literal_holds(literal, interp, valuation) {
            assert(extended.literals[clause.literals.len() as int] == literal);
            assert(0 <= clause.literals.len() < extended.literals.len());
            assert(clause_holds(extended, interp, valuation));
        }
    }
}

spec fn superposition_resolvent_holds(
    source_context: Clause,
    target_context: Clause,
    rewritten_target: Literal,
    interp: Interpretation,
    valuation: Valuation,
) -> bool {
    clause_holds(source_context, interp, valuation)
        || clause_holds(target_context, interp, valuation)
        || literal_holds(rewritten_target, interp, valuation)
}

/// Clause-level semantic superposition theorem for an explicitly supplied
/// simultaneous unifier. The source and target clauses consist of arbitrary
/// context clauses plus the selected source equality / target literal. Both
/// whole parents are instantiated with the same substitution. The syntactic
/// unifier equation ensures the oriented source side and selected target
/// subterm have the same denotation under every valuation. The result is the
/// standard resolvent shape: both parent contexts plus the rewritten target
/// literal.
proof fn universally_closed_superposition_sound(
    source_context: Clause,
    source_equality: Literal,
    target_context: Clause,
    target_literal: Literal,
    source_left: Term,
    source_right: Term,
    target_subterm: Term,
    target_slot: nat,
    target_position: TermPosition,
    sigma: Substitution,
    interp: Interpretation,
    valuation: Valuation,
)
    requires
        interpretation_well_formed(interp),
        valuation_well_formed(interp, valuation),
        clause_valid(append_literal(source_context, source_equality), interp),
        clause_valid(append_literal(target_context, target_literal), interp),
        source_equality.positive,
        source_equality.atom == Atom::Eq(source_left, source_right),
        apply(source_left, sigma) == apply(target_subterm, sigma),
        literal_at_position(
            substitute_literal(target_literal, sigma), target_slot, target_position,
        ) == Some(apply(target_subterm, sigma)),
        replace_literal_at_position(
            substitute_literal(target_literal, sigma),
            target_slot,
            target_position,
            apply(source_right, sigma),
        ).is_some(),
    ensures superposition_resolvent_holds(
        substitute_clause(source_context, sigma),
        substitute_clause(target_context, sigma),
        match replace_literal_at_position(
            substitute_literal(target_literal, sigma),
            target_slot,
            target_position,
            apply(source_right, sigma),
        ) {
            Some(rewritten) => rewritten,
            None => target_literal,
        },
        interp,
        valuation,
    )
{
    let source_parent = append_literal(source_context, source_equality);
    let target_parent = append_literal(target_context, target_literal);
    universal_clause_substitution_instance(source_parent, sigma, interp, valuation);
    universal_clause_substitution_instance(target_parent, sigma, interp, valuation);

    let source_instance = substitute_clause(source_parent, sigma);
    let target_instance = substitute_clause(target_parent, sigma);
    assert(source_instance.literals
        == substitute_clause(source_context, sigma).literals.push(
            substitute_literal(source_equality, sigma)));
    assert(target_instance.literals
        == substitute_clause(target_context, sigma).literals.push(
            substitute_literal(target_literal, sigma)));
    clause_holds_after_appending_literal(
        substitute_clause(source_context, sigma),
        substitute_literal(source_equality, sigma),
        interp,
        valuation,
    );
    clause_holds_after_appending_literal(
        substitute_clause(target_context, sigma),
        substitute_literal(target_literal, sigma),
        interp,
        valuation,
    );
    assert(clause_holds(source_instance, interp, valuation));
    assert(clause_holds(target_instance, interp, valuation));
    assert(clause_holds(substitute_clause(source_context, sigma), interp, valuation)
        || literal_holds(substitute_literal(source_equality, sigma), interp, valuation));
    assert(clause_holds(substitute_clause(target_context, sigma), interp, valuation)
        || literal_holds(substitute_literal(target_literal, sigma), interp, valuation));

    assert(substitute_literal(source_equality, sigma).positive);
    assert(substitute_literal(source_equality, sigma).atom
        == Atom::Eq(apply(source_left, sigma), apply(source_right, sigma)));
    assert(eval(apply(source_left, sigma), interp, valuation)
        == eval(apply(target_subterm, sigma), interp, valuation));
    if clause_holds(substitute_clause(source_context, sigma), interp, valuation) {
    } else if clause_holds(substitute_clause(target_context, sigma), interp, valuation) {
    } else {
        assert(literal_holds(substitute_literal(source_equality, sigma), interp, valuation));
        assert(literal_holds(substitute_literal(target_literal, sigma), interp, valuation));
        assert(substitute_literal(source_equality, sigma).atom
            == Atom::Eq(apply(source_left, sigma), apply(source_right, sigma)));
        assert(eval(apply(source_left, sigma), interp, valuation)
            == eval(apply(source_right, sigma), interp, valuation));
        assert(literal_at_position(
            substitute_literal(target_literal, sigma), target_slot, target_position,
        ) == Some(apply(target_subterm, sigma)));
        assert(match literal_at_position(
            substitute_literal(target_literal, sigma), target_slot, target_position,
        ) {
            Some(found) => eval(found, interp, valuation)
                == eval(apply(source_right, sigma), interp, valuation),
            None => false,
        });
        literal_holds_after_position_replacement(
            substitute_literal(target_literal, sigma),
            target_slot,
            target_position,
            apply(source_right, sigma),
            interp,
            valuation,
        );
        assert(match replace_literal_at_position(
            substitute_literal(target_literal, sigma),
            target_slot,
            target_position,
            apply(source_right, sigma),
        ) {
            Some(rewritten) => literal_holds(rewritten, interp, valuation),
            None => false,
        });
    }
}

/// End-to-end semantic rule theorem for parent clauses that are first
/// standardized independently and then instantiated by one supplied unifier.
/// Freshening is proved as an instance of each universally closed parent;
/// the unifier is then applied to both entire standardized clauses before the
/// selected target occurrence is replaced.
proof fn standardized_superposition_sound(
    source_context: Clause,
    source_equality: Literal,
    target_context: Clause,
    target_literal: Literal,
    source_left: Term,
    source_right: Term,
    target_subterm: Term,
    target_slot: nat,
    target_position: TermPosition,
    source_freshening: Substitution,
    target_freshening: Substitution,
    unifier: Substitution,
    interp: Interpretation,
    valuation: Valuation,
)
    requires
        interpretation_well_formed(interp),
        valuation_well_formed(interp, valuation),
        clause_valid(append_literal(source_context, source_equality), interp),
        clause_valid(append_literal(target_context, target_literal), interp),
        source_equality.positive,
        source_equality.atom == Atom::Eq(source_left, source_right),
        apply(apply(source_left, source_freshening), unifier)
            == apply(apply(target_subterm, target_freshening), unifier),
        literal_at_position(
            substitute_literal(
                substitute_literal(target_literal, target_freshening),
                unifier,
            ),
            target_slot,
            target_position,
        ) == Some(apply(apply(target_subterm, target_freshening), unifier)),
        replace_literal_at_position(
            substitute_literal(
                substitute_literal(target_literal, target_freshening),
                unifier,
            ),
            target_slot,
            target_position,
            apply(apply(source_right, source_freshening), unifier),
        ).is_some(),
    ensures superposition_resolvent_holds(
        substitute_clause(
            substitute_clause(source_context, source_freshening),
            unifier,
        ),
        substitute_clause(
            substitute_clause(target_context, target_freshening),
            unifier,
        ),
        match replace_literal_at_position(
            substitute_literal(
                substitute_literal(target_literal, target_freshening),
                unifier,
            ),
            target_slot,
            target_position,
            apply(apply(source_right, source_freshening), unifier),
        ) {
            Some(rewritten) => rewritten,
            None => target_literal,
        },
        interp,
        valuation,
    )
{
    let source_parent = append_literal(source_context, source_equality);
    let target_parent = append_literal(target_context, target_literal);
    standardized_parent_valid(source_parent, source_freshening, interp);
    standardized_parent_valid(target_parent, target_freshening, interp);

    let fresh_source_context = substitute_clause(source_context, source_freshening);
    let fresh_source_equality = substitute_literal(source_equality, source_freshening);
    let fresh_target_context = substitute_clause(target_context, target_freshening);
    let fresh_target_literal = substitute_literal(target_literal, target_freshening);
    assert(substitute_clause(source_parent, source_freshening).literals
        == fresh_source_context.literals.push(fresh_source_equality));
    assert(substitute_clause(target_parent, target_freshening).literals
        == fresh_target_context.literals.push(fresh_target_literal));
    assert(clause_valid(
        append_literal(fresh_source_context, fresh_source_equality), interp,
    ));
    assert(clause_valid(
        append_literal(fresh_target_context, fresh_target_literal), interp,
    ));

    universally_closed_superposition_sound(
        fresh_source_context,
        fresh_source_equality,
        fresh_target_context,
        fresh_target_literal,
        apply(source_left, source_freshening),
        apply(source_right, source_freshening),
        apply(target_subterm, target_freshening),
        target_slot,
        target_position,
        unifier,
        interp,
        valuation,
    );
}

/// Universal validity is preserved by simultaneous substitution. This covers
/// the forward semantic direction of standardization apart: renaming a
/// universally quantified clause to fresh variable IDs cannot invalidate it.
/// (A converse requires an inverse renaming and is not needed for inference
/// soundness.)
proof fn clause_valid_after_substitution(
    clause: Clause,
    renaming: Substitution,
    interp: Interpretation,
)
    requires
        interpretation_well_formed(interp),
        clause_valid(clause, interp),
    ensures clause_valid(substitute_clause(clause, renaming), interp)
{
    assert forall|valuation: Valuation|
        valuation_well_formed(interp, valuation)
            implies clause_holds(substitute_clause(clause, renaming), interp, valuation) by {
        universal_clause_substitution_instance(clause, renaming, interp, valuation);
    }
}

/// Standardization-apart is a simultaneous substitution instance of a
/// universally closed parent, so validity survives even when the two parents
/// are renamed independently. Injectivity and disjointness are certificate
/// hygiene properties checked separately; they do not need to be assumed as
/// semantic axioms for this direction of the proof.
proof fn standardized_parent_valid(
    parent: Clause,
    freshening: Substitution,
    interp: Interpretation,
)
    requires
        interpretation_well_formed(interp),
        clause_valid(parent, interp),
    ensures clause_valid(substitute_clause(parent, freshening), interp)
{
    clause_valid_after_substitution(parent, freshening, interp);
}

/// Parent validity plus independent standardization maps and a common
/// unifier yield valid standardized parents for the central step theorem. This
/// is the semantic bridge for the front end of a checker; subsequent
/// certificate refinement must connect its executable maps to these
/// substitutions.
proof fn standardized_superposition_parents_are_valid(
    source_parent: Clause,
    target_parent: Clause,
    source_freshening: Substitution,
    target_freshening: Substitution,
    interp: Interpretation,
)
    requires
        interpretation_well_formed(interp),
        clause_valid(source_parent, interp),
        clause_valid(target_parent, interp),
    ensures clause_valid(substitute_clause(source_parent, source_freshening), interp)
        && clause_valid(substitute_clause(target_parent, target_freshening), interp)
{
    standardized_parent_valid(source_parent, source_freshening, interp);
    standardized_parent_valid(target_parent, target_freshening, interp);
}

spec fn inverse_renamings(sigma: Substitution, inverse: Substitution) -> bool {
    forall|x: u64|
        apply(apply(Term::Var(x), sigma), inverse) == Term::Var(x)
        && apply(apply(Term::Var(x), inverse), sigma) == Term::Var(x)
}

proof fn updated_valuation_composition_identity(
    interp: Interpretation,
    valuation: Valuation,
    first: Substitution,
    second: Substitution,
)
    requires
        interp.functions.is_full(),
        valuation.values.is_full(),
        forall|x: u64| #[trigger] apply(apply(Term::Var(x), second), first) == Term::Var(x),
    ensures update_valuation(
        interp,
        update_valuation(interp, valuation, first),
        second,
    ).values == valuation.values
{
    assert forall|x: u64| update_valuation(
        interp,
        update_valuation(interp, valuation, first),
        second,
    ).values[x] == valuation.values[x] by {
        eval_after_substitution(
            apply(Term::Var(x), second),
            first,
            interp,
            valuation,
        );
        assert(eval(apply(Term::Var(x), second), interp,
            update_valuation(interp, valuation, first))
            == eval(apply(apply(Term::Var(x), second), first), interp, valuation));
    }
}

/// A bijective variable renaming preserves universal clause meaning in both
/// directions. This is the semantic standardization-apart theorem; freshness
/// of the chosen IDs is an executable syntactic side condition for the caller.
proof fn standardization_apart_preserves_clause_validity(
    clause: Clause,
    renaming: Substitution,
    inverse: Substitution,
    interp: Interpretation,
)
    requires
        interpretation_well_formed(interp),
        inverse_renamings(renaming, inverse),
    ensures clause_valid(substitute_clause(clause, renaming), interp)
        == clause_valid(clause, interp)
{
    if clause_valid(clause, interp) {
        clause_valid_after_substitution(clause, renaming, interp);
    } else {
        assert(!clause_valid(clause, interp));
    }

    assert(clause_valid(substitute_clause(clause, renaming), interp)
        ==> clause_valid(clause, interp)) by {
        if clause_valid(substitute_clause(clause, renaming), interp) {
            assert forall|valuation: Valuation|
                valuation_well_formed(interp, valuation)
                    implies clause_holds(clause, interp, valuation) by {
                updated_valuation_well_formed(interp, valuation, inverse);
                assert(clause_holds(
                    substitute_clause(clause, renaming),
                    interp,
                    update_valuation(interp, valuation, inverse),
                ));
                clause_holds_after_substitution(
                    clause,
                    renaming,
                    interp,
                    update_valuation(interp, valuation, inverse),
                );
                updated_valuation_composition_identity(
                    interp,
                    valuation,
                    inverse,
                    renaming,
                );
                assert(clause_holds(clause, interp, valuation));
            }
        }
    }
}

proof fn universal_clause_instance(p: IMap<u64, bool>, instance: u64)
    requires forall|x: u64| p[x]
    ensures p[instance]
{
    assert(p[instance]);
}

spec fn positive_clause_holds_after(
    clause: Clause,
    interp: Interpretation,
    valuation: Valuation,
) -> bool {
    exists|i: int| 0 <= i < clause.literals.len()
        && clause.literals[i].positive
        && atom_holds(clause.literals[i].atom, interp, valuation)
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

#[derive(PartialEq, Eq)]
pub enum ExecAtom {
    Pred0(u64),
    Pred1(u64, Box<ExecTerm>),
    Pred2(u64, Box<ExecTerm>, Box<ExecTerm>),
    Eq(ExecTerm, ExecTerm),
}

#[derive(PartialEq, Eq)]
pub struct ExecLiteral {
    positive: bool,
    atom: ExecAtom,
}

#[derive(PartialEq, Eq)]
pub struct ExecClause {
    literals: Vec<ExecLiteral>,
}

pub struct SuperpositionCertificate {
    source: ExecClause,
    target: ExecClause,
    source_freshening: Vec<ExecBinding>,
    target_freshening: Vec<ExecBinding>,
    source_equality_index: usize,
    target_literal_index: usize,
    target_argument_slot: u64,
    target_path: Vec<u64>,
    fresh_target_path: Vec<u64>,
    unifier: Vec<ExecBinding>,
    conclusion: ExecClause,
}

#[derive(PartialEq, Eq)]
pub enum ProofNodeKind {
    Input,
    Superposition,
    Unsupported,
}

pub struct ProofNode {
    id: u64,
    clause: ExecClause,
    kind: ProofNodeKind,
    parents: Vec<usize>,
    step: Option<SuperpositionCertificate>,
}

pub struct ProofDag {
    nodes: Vec<ProofNode>,
    input_clauses: Vec<ExecClause>,
    root: usize,
}

#[derive(PartialEq, Eq)]
pub enum DagVerdict {
    Inconclusive,
    Rejected,
    Unknown,
}

spec fn node_parent_clauses_hold(
    node: ProofNode,
    nodes: Seq<ProofNode>,
    interp: Interpretation,
    valuation: Valuation,
) -> bool {
    forall|j: int| 0 <= j < node.parents@.len()
        ==> clause_holds(view_clause(nodes[node.parents@[j] as int].clause), interp, valuation)
}

/// Semantic contract each supported inference implementation must establish.
/// The DAG theorem below consumes this relation; it does not assume that a
/// successful replay check establishes it.
spec fn local_node_entails_from_parents(node: ProofNode, nodes: Seq<ProofNode>,
    interp: Interpretation) -> bool {
    forall|valuation: Valuation|
        valuation_well_formed(interp, valuation)
            && node_parent_clauses_hold(node, nodes, interp, valuation)
        ==> clause_holds(view_clause(node.clause), interp, valuation)
}

spec fn input_leaf_is_validated(node: ProofNode, inputs: Seq<ExecClause>) -> bool {
    exists|i: int| 0 <= i < inputs.len()
        && clause_equal(view_clause(node.clause), view_clause(inputs[i]))
}

spec fn empty_clause(clause: Clause) -> bool {
    clause.literals.len() == 0
}

spec fn dag_proves_empty(dag: ProofDag) -> bool {
    dag.nodes.len() > 0
        && dag.root < dag.nodes.len()
        && empty_clause(view_clause(dag.nodes@[dag.root as int].clause))
}

proof fn proof_dag_nodes_follow_from_local_rule_soundness(
    nodes: Seq<ProofNode>,
    inputs: Seq<ExecClause>,
    interp: Interpretation,
    count: nat,
)
    requires
        interpretation_well_formed(interp),
        count <= nodes.len(),
        forall|i: int| 0 <= i < count as int
            ==> (nodes[i].kind == ProofNodeKind::Input
                    && input_leaf_is_validated(nodes[i], inputs)
                || nodes[i].kind == ProofNodeKind::Superposition
                    && local_node_entails_from_parents(nodes[i], nodes, interp)),
        forall|i: int, j: int| 0 <= i < count as int
            && 0 <= j < nodes[i].parents@.len()
            ==> nodes[i].parents@[j] < i as usize,
        forall|i: int| 0 <= i < inputs.len()
            ==> clause_valid(view_clause(inputs[i]), interp),
    ensures forall|i: int| 0 <= i < count as int
        ==> clause_valid(view_clause(nodes[i].clause), interp)
    decreases count
{
    if count == 0 {
    } else {
        proof_dag_nodes_follow_from_local_rule_soundness(nodes, inputs, interp, (count - 1) as nat);
        let last = count - 1;
        let node = nodes[last as int];
        if node.kind == ProofNodeKind::Input {
            let input_index = choose|k: int| 0 <= k < inputs.len()
                && clause_equal(view_clause(node.clause), view_clause(inputs[k]));
            assert forall|valuation: Valuation|
                valuation_well_formed(interp, valuation)
                    implies clause_holds(view_clause(node.clause), interp, valuation) by {
                assert(clause_holds(view_clause(inputs[input_index]), interp, valuation));
            }
        } else {
            assert(node.kind == ProofNodeKind::Superposition);
            assert forall|valuation: Valuation|
                valuation_well_formed(interp, valuation)
                    implies clause_holds(view_clause(node.clause), interp, valuation) by {
                assert forall|j: int| 0 <= j < node.parents@.len()
                    implies clause_holds(view_clause(nodes[node.parents@[j] as int].clause),
                        interp, valuation) by {
                    let parent = node.parents@[j] as int;
                    assert(parent < last as int);
                    assert(clause_valid(view_clause(nodes[parent].clause), interp));
                    assert(clause_holds(view_clause(nodes[parent].clause), interp, valuation));
                }
                assert(local_node_entails_from_parents(node, nodes, interp));
            }
        }
    }
}

proof fn certified_empty_dag_implies_unsatisfiable(
    nodes: Seq<ProofNode>,
    inputs: Seq<ExecClause>,
    interp: Interpretation,
    root: nat,
)
    requires
        interpretation_well_formed(interp),
        root < nodes.len(),
        empty_clause(view_clause(nodes[root as int].clause)),
        forall|i: int| 0 <= i <= root as int
            ==> (nodes[i].kind == ProofNodeKind::Input
                    && input_leaf_is_validated(nodes[i], inputs)
                || nodes[i].kind == ProofNodeKind::Superposition
                    && local_node_entails_from_parents(nodes[i], nodes, interp)),
        forall|i: int, j: int| 0 <= i <= root as int
            && 0 <= j < nodes[i].parents@.len()
            ==> nodes[i].parents@[j] < i as usize,
        forall|i: int| 0 <= i < inputs.len()
            ==> clause_valid(view_clause(inputs[i]), interp),
    ensures !exists|valuation: Valuation| valuation_well_formed(interp, valuation)
        && forall|i: int| 0 <= i < inputs.len()
            ==> clause_holds(view_clause(inputs[i]), interp, valuation)
{
    proof_dag_nodes_follow_from_local_rule_soundness(nodes, inputs, interp, root + 1);
    assert(clause_valid(view_clause(nodes[root as int].clause), interp));
    assert forall|valuation: Valuation|
        valuation_well_formed(interp, valuation)
            && forall|i: int| 0 <= i < inputs.len()
                ==> clause_holds(view_clause(inputs[i]), interp, valuation)
        implies false by {
        assert(!clause_holds(view_clause(nodes[root as int].clause), interp, valuation));
    }
}

pub struct Freshening {
    bindings: Vec<ExecBinding>,
}

pub struct VariableBudget {
    visited: usize,
    limit: usize,
}

pub struct FreshenResult {
    freshened: ExecClause,
    mapping: Vec<ExecBinding>,
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

spec fn view_atom(atom: ExecAtom) -> Atom
    decreases atom
{
    match atom {
        ExecAtom::Pred0(p) => Atom::Pred0(p),
        ExecAtom::Pred1(p, arg) => Atom::Pred1(p, Box::new(view(*arg))),
        ExecAtom::Pred2(p, left, right) =>
            Atom::Pred2(p, Box::new(view(*left)), Box::new(view(*right))),
        ExecAtom::Eq(left, right) => Atom::Eq(view(left), view(right)),
    }
}

spec fn view_literal(literal: ExecLiteral) -> Literal {
    Literal { positive: literal.positive, atom: view_atom(literal.atom) }
}

spec fn view_clause(clause: ExecClause) -> Clause {
    Clause {
        literals: clause.literals@.map(|_i: int, literal: ExecLiteral| view_literal(literal)),
    }
}

spec fn exec_term_variables(term: ExecTerm) -> Set<u64>
    decreases term
{
    match term {
        ExecTerm::Var(v) => Set::empty().insert(v),
        ExecTerm::Const(_) => Set::empty(),
        ExecTerm::App1(_, child) => exec_term_variables(*child),
        ExecTerm::App2(_, left, right) =>
            exec_term_variables(*left).union(exec_term_variables(*right)),
    }
}

spec fn variables_subset(term: ExecTerm, variables: Seq<u64>) -> bool {
    forall|v: u64| exec_term_variables(term).contains(v) ==> variables.contains(v)
}

spec fn exec_term_node_count(term: ExecTerm) -> nat
    decreases term
{
    match term {
        ExecTerm::Var(_) | ExecTerm::Const(_) => 1,
        ExecTerm::App1(_, child) => 1 + exec_term_node_count(*child),
        ExecTerm::App2(_, left, right) => 1 + exec_term_node_count(*left)
            + exec_term_node_count(*right),
    }
}

spec fn exec_clause_node_count(clause: ExecClause) -> nat {
    clause.literals@.len() as nat
}

spec fn exec_atom_variables(atom: ExecAtom) -> Set<u64>
    decreases atom
{
    match atom {
        ExecAtom::Pred0(_) => Set::empty(),
        ExecAtom::Pred1(_, arg) => exec_term_variables(*arg),
        ExecAtom::Pred2(_, left, right) =>
            exec_term_variables(*left).union(exec_term_variables(*right)),
        ExecAtom::Eq(left, right) =>
            exec_term_variables(left).union(exec_term_variables(right)),
    }
}

spec fn term_clause_view_contains(clause: ExecClause, v: u64) -> bool {
    exists|i: int| 0 <= i < clause.literals@.len()
        && literal_variables(view_literal(clause.literals@[i])).contains(v)
}

spec fn exec_clause_variables(clause: ExecClause) -> Set<u64> {
    exec_clause_variables_from_view(clause)
}

spec fn clause_collection_exact(clause: ExecClause, variables: Seq<u64>) -> bool {
    (forall|v: u64| term_clause_view_contains(clause, v) ==> variables.contains(v))
        && (forall|v: u64| variables.contains(v) ==> term_clause_view_contains(clause, v))
}

spec fn exec_clause_variables_from_view(clause: ExecClause) -> Set<u64> {
    Set::new(|v: u64| term_clause_view_contains(clause, v)).unwrap()
}

spec fn clause_variable_sequence_view(clause: ExecClause) -> Seq<u64> {
    Set::new(|v: u64| term_clause_view_contains(clause, v)).unwrap().to_seq()
}

proof fn term_variable_view(term: ExecTerm)
    ensures term_variables(view(term)) == exec_term_variables(term)
    decreases term
{
    match term {
        ExecTerm::Var(_) | ExecTerm::Const(_) => {},
        ExecTerm::App1(_, child) => term_variable_view(*child),
        ExecTerm::App2(_, left, right) => {
            term_variable_view(*left);
            term_variable_view(*right);
        },
    }
}

proof fn atom_variable_view(atom: ExecAtom)
    ensures atom_variables(view_atom(atom)) == exec_atom_variables(atom)
    decreases atom
{
    match atom {
        ExecAtom::Pred0(_) => {},
        ExecAtom::Pred1(_, arg) => term_variable_view(*arg),
        ExecAtom::Pred2(_, left, right) => {
            term_variable_view(*left);
            term_variable_view(*right);
        },
        ExecAtom::Eq(left, right) => {
            term_variable_view(left);
            term_variable_view(right);
        },
    }
}

proof fn literal_variable_view(literal: ExecLiteral)
    ensures literal_variables(view_literal(literal)) == exec_atom_variables(literal.atom)
{
    atom_variable_view(literal.atom);
}

proof fn atom_variable_membership_view(atom: ExecAtom, variable: u64)
    requires exec_atom_variables(atom).contains(variable)
    ensures literal_variables(view_literal(ExecLiteral { positive: true, atom })).contains(variable)
{
    atom_variable_view(atom);
}

proof fn clause_variable_view(clause: ExecClause)
{}

proof fn freshening_maps_clause_variables(
    clause: ExecClause,
    variables: Seq<u64>,
    sigma: Substitution,
    avoid: Seq<u64>,
)
    requires
        clause_collection_exact(clause, variables),
        freshening_sequence_valid(variables, avoid, sigma),
    ensures forall|v: u64| term_clause_view_contains(clause, v)
        && #[trigger] variables.contains(v)
        ==> exists|fresh: u64| lookup(sigma, v) == Some(Term::Var(fresh))
{
    assert forall|v: u64| term_clause_view_contains(clause, v)
        && #[trigger] variables.contains(v)
        && #[trigger] freshening_sequence_valid(variables, avoid, sigma)
        implies exists|fresh: u64| lookup(sigma, v) == Some(Term::Var(fresh)) by {
        assert(variables.contains(v)) by { reveal(clause_collection_exact); }
        let i = choose|i: int| 0 <= i < variables.len() && variables[i] == v;
        let fresh = renamed_variable(sigma, variables[i]).unwrap();
        reveal(renamed_variable);
        assert(lookup(sigma, v) == Some(Term::Var(fresh)));
    }
}

proof fn freshening_maps_selected_literal(
    clause: ExecClause,
    literal_index: int,
    variables: Seq<u64>,
    bindings: Seq<ExecBinding>,
)
    requires
        0 <= literal_index < clause.literals@.len(),
        clause_collection_exact(clause, variables),
        freshening_images_are_variable(bindings, variables),
        forall|v: u64| term_clause_view_contains(clause, v)
            && #[trigger] variables.contains(v)
            ==> exists|fresh: u64| lookup(view_subst(bindings), v) == Some(Term::Var(fresh)),
    ensures forall|v: u64|
        literal_variables(view_literal(clause.literals@[literal_index])).contains(v)
        ==> exists|fresh: u64| lookup(view_subst(bindings), v) == Some(Term::Var(fresh))
{
    assert forall|v: u64|
        literal_variables(view_literal(clause.literals@[literal_index])).contains(v)
        implies exists|fresh: u64|
            lookup(view_subst(bindings), v) == Some(Term::Var(fresh)) by {
        selected_literal_variables_are_clause_variables(clause, literal_index, v);
        assert(variables.contains(v)) by { reveal(clause_collection_exact); }
        assert(term_clause_view_contains(clause, v));
        assert(variables.contains(v));
        assert(exists|fresh: u64|
            lookup(view_subst(bindings), v) == Some(Term::Var(fresh)));
    }
}

proof fn selected_literal_variables_are_subset_of_clause_variables(
    clause: ExecClause,
    literal_index: int,
    variable: u64,
)
    requires
        0 <= literal_index < clause.literals@.len(),
        literal_variables(view_literal(clause.literals@[literal_index])).contains(variable),
    ensures term_clause_view_contains(clause, variable)
{}

proof fn clause_variables_cover_exec_variables(clause: ExecClause, variables: Seq<u64>)
    requires clause_collection_exact(clause, variables)
    ensures exec_clause_variables(clause) == exec_clause_variables_from_view(clause)
{}

spec fn renamed_variable(sigma: Substitution, variable: u64) -> Option<u64> {
    match lookup(sigma, variable) {
        Some(Term::Var(v)) => Some(v),
        _ => None,
    }
}

spec fn finite_renaming_injective(sigma: Substitution) -> bool {
    forall|x: u64, y: u64| x != y
        && lookup(sigma, x).is_some()
        && lookup(sigma, y).is_some()
        ==> renamed_variable(sigma, x) != renamed_variable(sigma, y)
}

spec fn renames_variables_only_on(sigma: Substitution, variables: Set<u64>) -> bool {
    forall|v: u64| variables.contains(v)
        ==> exists|fresh: u64| lookup(sigma, v) == Some(Term::Var(fresh))
}

spec fn sets_disjoint(left: Set<u64>, right: Set<u64>) -> bool {
    forall|v: u64| left.contains(v) ==> !right.contains(v)
}

spec fn freshening_valid(
    variables: Set<u64>,
    sigma: Substitution,
    avoid: Set<u64>,
) -> bool {
    domain_unique(sigma)
        && finite_renaming_injective(sigma)
        && renames_variables_only_on(sigma, variables)
        && forall|v: u64| variables.contains(v)
            ==> renamed_variable(sigma, v).is_some()
        && forall|v: u64| variables.contains(v)
            ==> !avoid.contains(renamed_variable(sigma, v).unwrap())
}

spec fn freshening_sequence_valid(
    variables: Seq<u64>,
    avoid: Seq<u64>,
    sigma: Substitution,
) -> bool {
    domain_unique(sigma)
        && forall|i: int| 0 <= i < variables.len()
            ==> #[trigger] renamed_variable(sigma, variables[i]).is_some()
                && exists|fresh: u64|
                    lookup(sigma, variables[i]) == Some(Term::Var(fresh))
                && !avoid.contains(renamed_variable(sigma, variables[i]).unwrap())
        && forall|i: int, j: int| 0 <= i < j < variables.len()
            && variables[i] != variables[j]
            ==> exists|fresh_i: u64, fresh_j: u64|
                lookup(sigma, variables[i]) == Some(Term::Var(fresh_i))
                && lookup(sigma, variables[j]) == Some(Term::Var(fresh_j))
                && fresh_i != fresh_j
}

spec fn freshening_images_are_variable(bindings: Seq<ExecBinding>, variables: Seq<u64>) -> bool {
    forall|i: int| 0 <= i < variables.len()
        ==> renamed_variable(view_subst(bindings), variables[i]).is_some()
}

proof fn renaming_images_cover_clause_variables(
    clause: ExecClause,
    variables: Seq<u64>,
    bindings: Seq<ExecBinding>,
    variable: u64,
)
    requires
        clause_collection_exact(clause, variables),
        term_clause_view_contains(clause, variable),
        freshening_images_are_variable(bindings, variables),
    ensures exists|fresh: u64|
        lookup(view_subst(bindings), variable) == Some(Term::Var(fresh))
{
    assert(variables.contains(variable)) by { reveal(clause_collection_exact); }
    let index = choose|i: int| 0 <= i < variables.len() && variables[i] == variable;
    assert(renamed_variable(view_subst(bindings), variables[index]).is_some());
    let fresh = renamed_variable(view_subst(bindings), variable).unwrap();
    reveal(renamed_variable);
    assert(lookup(view_subst(bindings), variable) == Some(Term::Var(fresh)));
}

proof fn collected_renaming_covers_selected_literal(
    clause: ExecClause,
    variables: Seq<u64>,
    bindings: Seq<ExecBinding>,
    literal_index: int,
)
    requires
        clause_collection_exact(clause, variables),
        freshening_images_are_variable(bindings, variables),
        0 <= literal_index < clause.literals@.len(),
    ensures renames_variables_on(
        view_subst(bindings), literal_variables(view_literal(clause.literals@[literal_index])),
    )
{
    assert forall|v: u64|
        literal_variables(view_literal(clause.literals@[literal_index])).contains(v)
        implies exists|fresh: u64| lookup(view_subst(bindings), v) == Some(Term::Var(fresh)) by {
        assert(term_clause_view_contains(clause, v));
        renaming_images_cover_clause_variables(clause, variables, bindings, v);
    }
}

proof fn freshening_maps_variables_in_selected_literal(
    clause: ExecClause,
    literal_index: int,
    variables: Seq<u64>,
    bindings: Seq<ExecBinding>,
)
    requires
        0 <= literal_index < clause.literals@.len(),
        clause_collection_exact(clause, variables),
        freshening_images_are_variable(bindings, variables),
    ensures forall|v: u64|
        literal_variables(view_literal(clause.literals@[literal_index])).contains(v)
            ==> exists|fresh: u64|
                lookup(view_subst(bindings), v) == Some(Term::Var(fresh))
{
    assert forall|v: u64|
        literal_variables(view_literal(clause.literals@[literal_index])).contains(v)
        implies exists|fresh: u64|
            lookup(view_subst(bindings), v) == Some(Term::Var(fresh)) by {
        assert(term_clause_view_contains(clause, v)) by {
            clause_variable_view(clause);
            reveal(term_clause_view_contains);
        }
        assert(variables.contains(v));
        assert(renamed_variable(view_subst(bindings), v).is_some());
        let fresh = renamed_variable(view_subst(bindings), v).unwrap();
        reveal(renamed_variable);
        assert(lookup(view_subst(bindings), v) == Some(Term::Var(fresh)));
    }
}

spec fn mapped_variable_set(variables: Set<u64>, sigma: Substitution) -> Set<u64> {
    Set::new(|fresh: u64| exists|source: u64|
        variables.contains(source) && renamed_variable(sigma, source) == Some(fresh)).unwrap()
}

spec fn standardized_apart_valid(left: ExecClause, right: ExecClause, sigma_left: Substitution,
    sigma_right: Substitution) -> bool
{
    freshening_valid(exec_clause_variables(left), sigma_left, exec_clause_variables(right))
        && freshening_valid(exec_clause_variables(right), sigma_right,
            mapped_variable_set(exec_clause_variables(left), sigma_left))
        && sets_disjoint(
            mapped_variable_set(exec_clause_variables(left), sigma_left),
            mapped_variable_set(exec_clause_variables(right), sigma_right),
        )
}

spec fn collection_complete(variables: Seq<u64>, expected: Set<u64>) -> bool {
    forall|v: u64| expected.contains(v) ==> variables.contains(v)
}

spec fn collection_exact(variables: Seq<u64>, expected: Set<u64>) -> bool {
    collection_complete(variables, expected)
        && forall|v: u64| variables.contains(v) ==> expected.contains(v)
}

proof fn append_keeps_set_member(values: Seq<u64>, value: u64, member: u64)
    requires values.contains(member)
    ensures values.push(value).contains(member)
{
    assert(exists|i: int| 0 <= i < values.len() && values[i] == member);
    let i = choose|i: int| 0 <= i < values.len() && values[i] == member;
    assert(i < values.push(value).len());
    assert(values.push(value)[i] == member);
}

proof fn set_member_to_exec_seq(values: &Vec<u64>, member: u64)
    requires values@.contains(member)
    ensures exists|i: int| 0 <= i < values@.len() && values@[i] == member
{}

proof fn add_variable_keeps_old_members(values: Seq<u64>, variable: u64, member: u64)
    requires values.contains(member)
    ensures values.contains(variable)
        || values.push(variable).contains(member)
{
    if values.contains(variable) {
        assert(values.contains(variable));
    } else {
        append_keeps_set_member(values, variable, member);
    }
}

fn contains_u64(values: &Vec<u64>, needle: u64) -> (found: bool)
    ensures found == values@.contains(needle)
{
    let mut i: usize = 0;
    while i < values.len()
        invariant
            i <= values.len(),
            forall|k: int| 0 <= k < i as int ==> values@[k] != needle,
        decreases values.len() - i
    {
        if values[i] == needle {
            assert(values@.contains(needle));
            return true;
        }
        assert(values@[i as int] != needle);
        i += 1;
    }
    assert forall|k: int| 0 <= k < values@.len() implies values@[k] != needle by {
        assert(k < i as int);
    }
    assert(!values@.contains(needle));
    false
}

fn term_contains_variable(term: &ExecTerm, variable: u64) -> (found: bool)
    ensures found == exec_term_variables(*term).contains(variable)
    decreases term
{
    match term {
        ExecTerm::Var(v) => *v == variable,
        ExecTerm::Const(_) => false,
        ExecTerm::App1(_, child) => term_contains_variable(child, variable),
        ExecTerm::App2(_, left, right) =>
            term_contains_variable(left, variable) || term_contains_variable(right, variable),
    }
}

fn atom_contains_variable(atom: &ExecAtom, variable: u64) -> (found: bool)
    ensures found == exec_atom_variables(*atom).contains(variable)
    decreases atom
{
    match atom {
        ExecAtom::Pred0(_) => false,
        ExecAtom::Pred1(_, arg) => term_contains_variable(arg, variable),
        ExecAtom::Pred2(_, left, right) =>
            term_contains_variable(left, variable) || term_contains_variable(right, variable),
        ExecAtom::Eq(left, right) =>
            term_contains_variable(left, variable) || term_contains_variable(right, variable),
    }
}

fn clause_view_contains_variable(clause: &ExecClause, variable: u64) -> (found: bool)
    ensures found == term_clause_view_contains(*clause, variable)
{
    let mut i: usize = 0;
    while i < clause.literals.len()
        invariant i <= clause.literals.len(),
            forall|j: int| 0 <= j < i as int
                ==> !literal_variables(view_literal(clause.literals@[j])).contains(variable)
        decreases clause.literals.len() - i
    {
        let current = &clause.literals[i];
        if atom_contains_variable(&current.atom, variable) {
            assert(literal_variables(view_literal(*current)).contains(variable)) by {
                literal_variable_view(*current);
            }
            assert(term_clause_view_contains(*clause, variable));
            return true;
        }
        assert(!literal_variables(view_literal(*current)).contains(variable)) by {
            literal_variable_view(*current);
        }
        i += 1;
    }
    false
}

fn term_variables_covered(term: &ExecTerm, variables: &Vec<u64>) -> (covered: bool)
    ensures covered == variables_subset(*term, variables@)
    decreases term
{
    match term {
        ExecTerm::Var(v) => contains_u64(variables, *v),
        ExecTerm::Const(_) => true,
        ExecTerm::App1(_, child) => term_variables_covered(child, variables),
        ExecTerm::App2(_, left, right) =>
            term_variables_covered(left, variables) && term_variables_covered(right, variables),
    }
}

fn atom_variables_covered(atom: &ExecAtom, variables: &Vec<u64>) -> (covered: bool)
    ensures covered == forall|v: u64| exec_atom_variables(*atom).contains(v)
        ==> variables@.contains(v)
    decreases atom
{
    match atom {
        ExecAtom::Pred0(_) => true,
        ExecAtom::Pred1(_, arg) => term_variables_covered(arg, variables),
        ExecAtom::Pred2(_, left, right) =>
            term_variables_covered(left, variables) && term_variables_covered(right, variables),
        ExecAtom::Eq(left, right) =>
            term_variables_covered(left, variables) && term_variables_covered(right, variables),
    }
}

fn literal_variables_covered(literal: &ExecLiteral, variables: &Vec<u64>) -> (covered: bool)
    ensures covered == forall|v: u64| exec_atom_variables(literal.atom).contains(v)
        ==> variables@.contains(v)
{
    atom_variables_covered(&literal.atom, variables)
}

fn clause_variables_covered(clause: &ExecClause, variables: &Vec<u64>) -> (covered: bool)
    ensures covered ==> forall|v: u64| term_clause_view_contains(*clause, v)
        ==> variables@.contains(v)
{
    let mut i: usize = 0;
    while i < clause.literals.len()
        invariant i <= clause.literals.len(),
            forall|v: u64, j: int| 0 <= j < i as int
                && #[trigger] exec_atom_variables(clause.literals@[j].atom).contains(v)
                ==> variables@.contains(v)
        decreases clause.literals.len() - i
    {
        let lit_covered = literal_variables_covered(&clause.literals[i], variables);
        if !lit_covered { return false; }
        assert forall|v: u64|
            exec_atom_variables(clause.literals@[i as int].atom).contains(v)
        implies variables@.contains(v) by {
            assert(lit_covered);
        }
        i += 1;
    }
    assert(i == clause.literals.len());
    assert forall|v: u64| term_clause_view_contains(*clause, v)
        implies variables@.contains(v) by {
        let literal_index = choose|j: int| 0 <= j < clause.literals@.len()
            && literal_variables(view_literal(clause.literals@[j])).contains(v);
        assert(literal_variables(view_literal(clause.literals@[literal_index]))
            == exec_atom_variables(clause.literals@[literal_index].atom)) by {
            literal_variable_view(clause.literals@[literal_index]);
        }
        assert(exec_atom_variables(clause.literals@[literal_index].atom).contains(v));
        assert(variables@.contains(v));
    }
    true
}

proof fn selected_literal_variables_are_clause_variables(
    clause: ExecClause,
    literal_index: int,
    variable: u64,
)
    requires
        0 <= literal_index < clause.literals@.len(),
        literal_variables(view_literal(clause.literals@[literal_index])).contains(variable),
    ensures term_clause_view_contains(clause, variable)
{}

fn consume_work(work: &mut usize, limit: usize) -> (ok: bool)
    ensures
        ok ==> *final(work) == old(work) + 1,
        ok ==> old(work) < limit && *final(work) <= limit,
        !ok ==> *final(work) == *old(work),
{
    if *work >= limit { false } else { *work += 1; true }
}

fn add_variable(values: &mut Vec<u64>, variable: u64)
    ensures final(values)@.contains(variable),
        forall|v: u64| old(values)@.contains(v) ==> final(values)@.contains(v),
{
    values.push(variable);
    assert(values@ == old(values)@.push(variable));
    assert(old(values)@.len() < values@.len());
    assert(values@[old(values)@.len() as int] == variable);
    assert(values@.contains(variable));
    assert forall|v: u64| old(values)@.contains(v) implies values@.contains(v) by {
        append_keeps_set_member(old(values)@, variable, v);
    }
}

fn collect_term_variables_exec(
    term: &ExecTerm,
    variables: &mut Vec<u64>,
    work: &mut usize,
    limit: usize,
) -> (ok: bool)
    ensures ok ==> forall|v: u64| exec_term_variables(*term).contains(v)
            ==> final(variables)@.contains(v),
        ok ==> forall|v: u64| old(variables)@.contains(v) ==> final(variables)@.contains(v),
        ok ==> *old(work) <= limit && *final(work) <= limit,
    decreases term
{
    match term {
        ExecTerm::Var(v) => {
            if !consume_work(work, limit) { return false; }
            add_variable(variables, *v);
            true
        },
        ExecTerm::Const(_) => consume_work(work, limit),
        ExecTerm::App1(_, child) => {
            if !consume_work(work, limit) { return false; }
            collect_term_variables_exec(child, variables, work, limit)
        },
        ExecTerm::App2(_, left, right) => {
            if !consume_work(work, limit) { return false; }
            if !collect_term_variables_exec(left, variables, work, limit) { return false; }
            if !collect_term_variables_exec(right, variables, work, limit) { return false; }
            true
        },
    }
}

fn collect_atom_variables_exec(
    atom: &ExecAtom,
    variables: &mut Vec<u64>,
    work: &mut usize,
    limit: usize,
) -> (ok: bool)
    requires *old(work) <= limit
    ensures ok ==> forall|v: u64| exec_atom_variables(*atom).contains(v)
            ==> final(variables)@.contains(v),
        ok ==> forall|v: u64| old(variables)@.contains(v)
            ==> final(variables)@.contains(v),
        ok ==> *old(work) <= limit && *final(work) <= limit,
    decreases atom
{
    match atom {
        ExecAtom::Pred0(_) => true,
        ExecAtom::Pred1(_, arg) =>
            collect_term_variables_exec(arg, variables, work, limit),
        ExecAtom::Pred2(_, left, right) =>
            collect_term_variables_exec(left, variables, work, limit)
                && collect_term_variables_exec(right, variables, work, limit),
        ExecAtom::Eq(left, right) =>
            collect_term_variables_exec(left, variables, work, limit)
                && collect_term_variables_exec(right, variables, work, limit),
    }
}

fn collect_clause_variables_exec(
    clause: &ExecClause,
    variables: &mut Vec<u64>,
    work: &mut usize,
    limit: usize,
) -> (ok: bool)
    requires *old(work) <= limit
    ensures ok ==> forall|v: u64, j: int|
            0 <= j < clause.literals@.len()
                && #[trigger] exec_atom_variables(clause.literals@[j].atom).contains(v)
            ==> final(variables)@.contains(v),
        ok ==> forall|v: u64| old(variables)@.contains(v)
            ==> final(variables)@.contains(v),
        ok ==> *old(work) <= limit && *final(work) <= limit,
{
    if !consume_work(work, limit) { return false; }
    let mut literal_index: usize = 0;
    while literal_index < clause.literals.len()
        invariant literal_index <= clause.literals.len(),
            *work <= limit,
            forall|v: u64, j: int| 0 <= j < literal_index as int
                && #[trigger] exec_atom_variables(clause.literals@[j].atom).contains(v)
                ==> variables@.contains(v),
            forall|v: u64| old(variables)@.contains(v) ==> variables@.contains(v),
        decreases clause.literals.len() - literal_index
    {
        if !collect_atom_variables_exec(
            &clause.literals[literal_index].atom, variables, work, limit,
        ) { return false; }
        assert forall|v: u64|
            exec_atom_variables(clause.literals@[literal_index as int].atom).contains(v)
            implies variables@.contains(v) by {}
        literal_index += 1;
    }
    assert(*work <= limit);
    true
}

fn collect_clause_variables_bounded(
    clause: &ExecClause,
    work_limit: usize,
) -> (result: Option<Vec<u64>>)
    ensures match result {
        Some(variables) => clause_collection_exact(*clause, variables@),
        None => true,
    }
{
    if work_limit < 8 || clause.literals.len() > 64 { return None; }
    let mut variables: Vec<u64> = Vec::new();
    let mut work: usize = 0;
    if collect_clause_variables_exec(clause, &mut variables, &mut work, work_limit) {
        let covered = clause_variables_covered(clause, &variables);
        if covered {
            let mut i: usize = 0;
            while i < variables.len()
                invariant i <= variables.len(),
                    forall|j: int| 0 <= j < i as int
                        ==> term_clause_view_contains(*clause, variables@[j]),
                decreases variables.len() - i
            {
                if !clause_view_contains_variable(clause, variables[i]) { return None; }
                assert(term_clause_view_contains(*clause, variables@[i as int]));
                i += 1;
            }
            assert forall|v: u64| variables@.contains(v)
                implies term_clause_view_contains(*clause, v) by {
                let index = choose|j: int| 0 <= j < variables@.len() && variables@[j] == v;
                assert(index < i as int);
            }
            assert forall|v: u64| term_clause_view_contains(*clause, v)
                implies variables@.contains(v) by {
                let literal_index = choose|j: int| 0 <= j < clause.literals@.len()
                    && literal_variables(view_literal(clause.literals@[j])).contains(v);
                assert(exec_atom_variables(clause.literals@[literal_index].atom).contains(v)) by {
                    literal_variable_view(clause.literals@[literal_index]);
                }
                assert(variables@.contains(v));
            }
            Some(variables)
        } else {
            None
        }
    } else {
        None
    }
}

fn check_freshening(
    original: &Vec<u64>,
    avoid: &Vec<u64>,
    bindings: &Vec<ExecBinding>,
    work_limit: usize,
) -> (valid: bool)
    ensures
        valid ==> freshening_sequence_valid(original@, avoid@, view_subst(bindings@)),
        valid ==> freshening_images_are_variable(bindings@, original@),
        valid ==> forall|v: u64| original@.contains(v)
            ==> exists|fresh: u64|
                lookup(view_subst(bindings@), v) == Some(Term::Var(fresh))
{
    if !check_domain_unique(bindings, work_limit) {
        return false;
    }
    let mut i: usize = 0;
    let mut work: usize = 0;
    while i < original.len()
        invariant
            i <= original.len(),
            work <= work_limit,
            forall|k: int| 0 <= k < i as int
                && #[trigger] renamed_variable(view_subst(bindings@), original@[k]).is_some()
                ==> exists|fresh: u64|
                    lookup(view_subst(bindings@), original@[k]) == Some(Term::Var(fresh)),
            forall|k: int| 0 <= k < i as int
                && #[trigger] renamed_variable(view_subst(bindings@), original@[k]).is_some()
                ==> exists|fresh: u64|
                    lookup(view_subst(bindings@), original@[k]) == Some(Term::Var(fresh)),
            forall|k: int| 0 <= k < i as int
                ==> renamed_variable(view_subst(bindings@), original@[k]).is_some()
                    && !avoid@.contains(
                        renamed_variable(view_subst(bindings@), original@[k]).unwrap()),
            forall|k: int, l: int| 0 <= k < l < i as int
                && original@[k] != original@[l]
                ==> renamed_variable(view_subst(bindings@), original@[k])
                    != renamed_variable(view_subst(bindings@), original@[l]),
        decreases original.len() - i
    {
        if work >= work_limit { return false; }
        work += 1;
        if work >= work_limit && i + 1 < original.len() { return false; }
        let Some(mapped) = lookup_exec(bindings, original[i]) else { return false; };
        let ExecTerm::Var(fresh) = mapped else { return false; };
        if contains_u64(avoid, fresh) { return false; }
        assert(renamed_variable(view_subst(bindings@), original@[i as int]) == Some(fresh));
        assert(lookup(view_subst(bindings@), original@[i as int]) == Some(Term::Var(fresh))) by {
            reveal(renamed_variable);
        }
        assert(exists|fresh_id: u64|
            lookup(view_subst(bindings@), original@[i as int]) == Some(Term::Var(fresh_id)));
        assert(!avoid@.contains(fresh));
        let mut j: usize = 0;
        while j < i
            invariant
                j <= i, i < original.len(), work <= work_limit,
                forall|k: int| 0 <= k < j as int && original@[k] != original@[i as int]
                    ==> renamed_variable(view_subst(bindings@), original@[k])
                        != Some(fresh),
            decreases i - j
        {
            assert(j < original.len());
            assert(i < original.len());
            if work >= work_limit { return false; }
            work += 1;
            let same_source = original[j] == original[i];
            let mut diff_image = false;
            if !same_source {
                let Some(previous) = lookup_exec(bindings, original[j]) else { return false; };
                let ExecTerm::Var(previous_fresh) = previous else { return false; };
                if fresh == previous_fresh { return false; }
                diff_image = true;
                assert(renamed_variable(view_subst(bindings@), original@[j as int])
                    == Some(previous_fresh));
            }
            assert forall|k: int| 0 <= k < (j + 1) as int
                && original@[k] != original@[i as int]
                implies renamed_variable(view_subst(bindings@), original@[k])
                    != Some(fresh) by {
                if k < j as int {
                } else {
                    assert(k == j as int);
                    assert(!same_source);
                    assert(diff_image);
                }
            }
            j += 1;
        }
        assert forall|k: int| 0 <= k < i as int
            && original@[k] != original@[i as int]
            implies renamed_variable(view_subst(bindings@), original@[k])
                != renamed_variable(view_subst(bindings@), original@[i as int]) by {
            assert(renamed_variable(view_subst(bindings@), original@[i as int]) == Some(fresh));
        }
        i += 1;
    }
    assert(i == original.len());
    assert forall|k: int| 0 <= k < original@.len()
        implies renamed_variable(view_subst(bindings@), original@[k]).is_some() by {
        assert(k < i as int);
    }
    assert forall|v: u64| original@.contains(v)
        implies exists|fresh: u64|
            lookup(view_subst(bindings@), v) == Some(Term::Var(fresh)) by {
        let position = choose|k: int| 0 <= k < original@.len() && original@[k] == v;
        assert(exists|fresh: u64|
            lookup(view_subst(bindings@), original@[position]) == Some(Term::Var(fresh)));
        assert(original@[position] == v);
    }
    true
}

fn freshen_clause_apart(
    clause: &ExecClause,
    avoid: &Vec<u64>,
    bindings: &Vec<ExecBinding>,
    work_limit: usize,
) -> (result: Option<ExecClause>)
    ensures match result {
        Some(fresh) => view_clause(fresh)
            == substitute_clause(view_clause(*clause), view_subst(bindings@))
            && (forall|v: u64| term_clause_view_contains(*clause, v)
                ==> exists|fresh_id: u64|
                    lookup(view_subst(bindings@), v) == Some(Term::Var(fresh_id))),
        None => true,
    },
        result.is_some() ==> forall|i: int| 0 <= i < clause.literals@.len()
            ==> renames_variables_on(view_subst(bindings@),
                literal_variables(view_literal(clause.literals@[i])))
{
    let Some(variables) = collect_clause_variables_bounded(clause, work_limit) else {
        return None;
    };
    let freshening_ok = check_freshening(&variables, avoid, bindings, work_limit);
    if !freshening_ok {
        return None;
    }
    assert(clause_collection_exact(*clause, variables@));
    assert(freshening_images_are_variable(bindings@, variables@));
    proof {
        assert forall|i: int| 0 <= i < clause.literals@.len()
            implies renames_variables_on(view_subst(bindings@),
                literal_variables(view_literal(clause.literals@[i]))) by {
            collected_renaming_covers_selected_literal(*clause, variables@, bindings@, i);
        }
    }
    let fresh = freshen_clause_with_variable_map(clause, &variables, bindings);
    Some(fresh)
}

fn freshen_clause_with_variable_map(
    clause: &ExecClause,
    variables: &Vec<u64>,
    bindings: &Vec<ExecBinding>,
) -> (fresh: ExecClause)
    requires
        clause_collection_exact(*clause, variables@),
        freshening_images_are_variable(bindings@, variables@),
    ensures view_clause(fresh)
            == substitute_clause(view_clause(*clause), view_subst(bindings@)),
        forall|v: u64| term_clause_view_contains(*clause, v)
            ==> exists|fresh_id: u64|
                lookup(view_subst(bindings@), v) == Some(Term::Var(fresh_id))
{
    let fresh = substitute_clause_exec(clause, bindings);
    assert(view_clause(fresh)
        == substitute_clause(view_clause(*clause), view_subst(bindings@)));
    assert forall|v: u64| term_clause_view_contains(*clause, v)
        implies exists|fresh_id: u64|
            lookup(view_subst(bindings@), v) == Some(Term::Var(fresh_id)) by {
        renaming_images_cover_clause_variables(*clause, variables@, bindings@, v);
    }
    fresh
}

fn freshen_clause_refines_renaming(
    clause: &ExecClause,
    avoid: &Vec<u64>,
    bindings: &Vec<ExecBinding>,
    work_limit: usize,
) -> (result: Option<ExecClause>)
    ensures match result {
        Some(fresh) => view_clause(fresh)
            == substitute_clause(view_clause(*clause), view_subst(bindings@))
            && (forall|v: u64| term_clause_view_contains(*clause, v)
                ==> exists|fresh_id: u64|
                    lookup(view_subst(bindings@), v) == Some(Term::Var(fresh_id))),
        None => true,
    }
{
    freshen_clause_apart(clause, avoid, bindings, work_limit)
}

fn check_standardized_apart_pair(
    left: &ExecClause,
    right: &ExecClause,
    left_mapping: &Vec<ExecBinding>,
    right_mapping: &Vec<ExecBinding>,
    work_limit: usize,
) -> (valid: bool)
{
    if work_limit == 0 { return false; }
    let pair_budget = work_limit / 4;
    let Some(left_variables) = collect_clause_variables_bounded(left, work_limit) else {
        return false;
    };
    let Some(right_variables) = collect_clause_variables_bounded(right, work_limit) else {
        return false;
    };
    if !check_freshening(&left_variables, &right_variables, left_mapping, pair_budget) {
        return false;
    }
    let Some(fresh_left) = freshen_clause_apart(left, &right_variables, left_mapping, pair_budget) else {
        return false;
    };
    let mut avoid: Vec<u64> = Vec::new();
    let mut i: usize = 0;
    while i < right_variables.len()
        invariant i <= right_variables.len()
        decreases right_variables.len() - i
    {
        let value = right_variables[i];
        if !contains_u64(&avoid, value) { avoid.push(value); }
        i += 1;
    }
    let Some(fresh_left_variables) = collect_clause_variables_bounded(&fresh_left, pair_budget) else {
        return false;
    };
    let mut k: usize = 0;
    while k < fresh_left_variables.len()
        invariant k <= fresh_left_variables.len()
        decreases fresh_left_variables.len() - k
    {
        let value = fresh_left_variables[k];
        if !contains_u64(&avoid, value) { avoid.push(value); }
        k += 1;
    }
    if !check_freshening(&right_variables, &avoid, right_mapping, pair_budget) {
        return false;
    }
    let Some(fresh_right) = freshen_clause_apart(right, &avoid, right_mapping, pair_budget) else {
        return false;
    };
    let Some(fresh_right_variables) = collect_clause_variables_bounded(&fresh_right, pair_budget) else {
        return false;
    };
    let mut j: usize = 0;
    while j < fresh_right_variables.len()
        invariant j <= fresh_right_variables.len()
        decreases fresh_right_variables.len() - j
    {
        if contains_u64(&fresh_left_variables, fresh_right_variables[j]) {
            return false;
        }
        j += 1;
    }
    true
}

spec fn exec_terms_bound(term: ExecTerm, bound: nat) -> bool
    decreases term
{
    match term {
        ExecTerm::Var(_) | ExecTerm::Const(_) => true,
        ExecTerm::App1(_, child) => exec_terms_bound(*child, bound),
        ExecTerm::App2(_, left, right) =>
            exec_terms_bound(*left, bound) && exec_terms_bound(*right, bound),
    }
}

spec fn view_exec_position(slot: u64, path: Seq<u64>) -> (nat, TermPosition) {
    (slot as nat, view_position(path))
}

spec fn view_subst(bindings: Seq<ExecBinding>) -> Substitution {
    Substitution {
        bindings: bindings.map(|_i: int, b: ExecBinding| (b.variable, view(b.replacement))),
    }
}

spec fn apply_exec_model(term: ExecTerm, bindings: Seq<ExecBinding>) -> ExecTerm
    decreases term
{
    match term {
        ExecTerm::Var(v) => match lookup(view_subst(bindings), v) {
            Some(Term::Var(fresh)) => ExecTerm::Var(fresh),
            Some(Term::Const(c)) => ExecTerm::Const(c),
            Some(Term::App1(f, child)) => ExecTerm::App1(f, Box::new(view_to_exec(*child))),
            Some(Term::App2(f, left, right)) => ExecTerm::App2(
                f, Box::new(view_to_exec(*left)), Box::new(view_to_exec(*right)),
            ),
            None => ExecTerm::Var(v),
        },
        ExecTerm::Const(c) => ExecTerm::Const(c),
        ExecTerm::App1(f, child) => ExecTerm::App1(f,
            Box::new(apply_exec_model(*child, bindings))),
        ExecTerm::App2(f, left, right) => ExecTerm::App2(
            f,
            Box::new(apply_exec_model(*left, bindings)),
            Box::new(apply_exec_model(*right, bindings)),
        ),
    }
}

proof fn view_to_exec_roundtrip(term: Term)
    ensures view(view_to_exec(term)) == term
    decreases term
{
    match term {
        Term::Var(_) | Term::Const(_) => {},
        Term::App1(_, child) => view_to_exec_roundtrip(*child),
        Term::App2(_, left, right) => {
            view_to_exec_roundtrip(*left);
            view_to_exec_roundtrip(*right);
        },
    }
}

spec fn view_to_exec(term: Term) -> ExecTerm
    decreases term
{
    match term {
        Term::Var(v) => ExecTerm::Var(v),
        Term::Const(c) => ExecTerm::Const(c),
        Term::App1(f, child) => ExecTerm::App1(f, Box::new(view_to_exec(*child))),
        Term::App2(f, left, right) => ExecTerm::App2(
            f, Box::new(view_to_exec(*left)), Box::new(view_to_exec(*right)),
        ),
    }
}

spec fn view_position(path: Seq<u64>) -> TermPosition {
    TermPosition { path: path.map(|_i: int, child: u64| child as nat) }
}

/// Bounded-by-input executable traversal for an explicit term-tree path. An
/// invalid child index, a path continuing below a variable/constant, or a
/// depth beyond the path returns `None` rather than accepting a partial walk.
fn subterm_at_exec(term: &ExecTerm, path: &Vec<u64>, depth: usize) -> (result: Option<ExecTerm>)
    requires depth <= path.len()
    ensures match result {
        Some(found) => term_at_position(view(*term), view_position(path@), depth as nat)
            == Some(view(found)),
        None => term_at_position(view(*term), view_position(path@), depth as nat) == None,
    },
        result.is_some() == term_at_position(view(*term), view_position(path@), depth as nat).is_some()
    decreases term
{
    if depth == path.len() {
        return Some(copy_term(term));
    }
    match term {
        ExecTerm::Var(_) | ExecTerm::Const(_) => None,
        ExecTerm::App1(_, child) => {
            if path[depth] == 0 {
                subterm_at_exec(child, path, depth + 1)
            } else {
                None
            }
        },
        ExecTerm::App2(_, left, right) => {
            if path[depth] == 0 {
                subterm_at_exec(left, path, depth + 1)
            } else if path[depth] == 1 {
                subterm_at_exec(right, path, depth + 1)
            } else {
                None
            }
        },
    }
}

/// Executable replacement at an explicit position. The returned term refines
/// the mathematical replay operation; all malformed paths fail closed.
fn replace_at_exec(
    term: &ExecTerm,
    path: &Vec<u64>,
    replacement: &ExecTerm,
    depth: usize,
) -> (result: Option<ExecTerm>)
    requires depth <= path.len()
    ensures match result {
        Some(replaced) => replace_at_position(
            view(*term), view_position(path@), view(*replacement), depth as nat,
        ) == Some(view(replaced)),
        None => replace_at_position(
            view(*term), view_position(path@), view(*replacement), depth as nat,
        ) == None,
    },
        result.is_some() == replace_at_position(
            view(*term), view_position(path@), view(*replacement), depth as nat,
        ).is_some()
    decreases term
{
    if depth == path.len() {
        return Some(copy_term(replacement));
    }
    match term {
        ExecTerm::Var(_) | ExecTerm::Const(_) => None,
        ExecTerm::App1(f, child) => {
            if path[depth] == 0 {
                match replace_at_exec(child, path, replacement, depth + 1) {
                    Some(child2) => Some(ExecTerm::App1(*f, Box::new(child2))),
                    None => None,
                }
            } else {
                None
            }
        },
        ExecTerm::App2(f, left, right) => {
            if path[depth] == 0 {
                match replace_at_exec(left, path, replacement, depth + 1) {
                    Some(left2) => Some(ExecTerm::App2(*f, Box::new(left2),
                        Box::new(copy_term(right)))),
                    None => None,
                }
            } else if path[depth] == 1 {
                match replace_at_exec(right, path, replacement, depth + 1) {
                    Some(right2) => Some(ExecTerm::App2(*f, Box::new(copy_term(left)),
                        Box::new(right2))),
                    None => None,
                }
            } else {
                None
            }
        },
    }
}

fn substitute_atom_exec(atom: &ExecAtom, bindings: &Vec<ExecBinding>) -> (result: ExecAtom)
    ensures view_atom(result) == substitute_atom(view_atom(*atom), view_subst(bindings@))
    decreases atom
{
    match atom {
        ExecAtom::Pred0(p) => ExecAtom::Pred0(*p),
        ExecAtom::Pred1(p, arg) => ExecAtom::Pred1(
            *p, Box::new(apply_exec(arg, bindings)),
        ),
        ExecAtom::Pred2(p, left, right) => ExecAtom::Pred2(
            *p,
            Box::new(apply_exec(left, bindings)),
            Box::new(apply_exec(right, bindings)),
        ),
        ExecAtom::Eq(left, right) => ExecAtom::Eq(
            apply_exec(left, bindings), apply_exec(right, bindings),
        ),
    }
}

fn substitute_literal_exec(literal: &ExecLiteral, bindings: &Vec<ExecBinding>)
    -> (result: ExecLiteral)
    ensures view_literal(result) == substitute_literal(view_literal(*literal), view_subst(bindings@))
{
    ExecLiteral {
        positive: literal.positive,
        atom: substitute_atom_exec(&literal.atom, bindings),
    }
}

fn substitute_clause_exec(clause: &ExecClause, bindings: &Vec<ExecBinding>)
    -> (result: ExecClause)
    ensures view_clause(result)
        == substitute_clause(view_clause(*clause), view_subst(bindings@))
        && result.literals@.len() == clause.literals@.len()
        && forall|j: int| 0 <= j < result.literals@.len()
            ==> view_literal(result.literals@[j])
                == substitute_literal(view_clause(*clause).literals[j], view_subst(bindings@))
{
    let mut literals: Vec<ExecLiteral> = Vec::new();
    let mut i: usize = 0;
    while i < clause.literals.len()
        invariant
            i <= clause.literals.len(),
            literals@.len() == i,
            forall|j: int| 0 <= j < i as int
                ==> view_literal(literals@[j])
                    == substitute_literal(view_clause(*clause).literals[j], view_subst(bindings@)),
        decreases clause.literals.len() - i
    {
        let literal = substitute_literal_exec(&clause.literals[i], bindings);
        assert(view_literal(literal)
            == substitute_literal(view_literal(clause.literals@[i as int]), view_subst(bindings@)));
        literals.push(literal);
        i += 1;
    }
    let result = ExecClause { literals };
    assert(result.literals@.len() == clause.literals@.len());
    assert forall|j: int| 0 <= j < result.literals@.len()
        implies view_literal(result.literals@[j])
            == substitute_literal(view_clause(*clause).literals[j], view_subst(bindings@)) by {
        assert(result.literals@[j] == literals@[j]);
    }
    assert(view_clause(result).literals
        == substitute_clause(view_clause(*clause), view_subst(bindings@)).literals);
    result
}

fn replace_clause_literal_exec(
    clause: &ExecClause,
    literal_index: usize,
    slot: u64,
    path: &Vec<u64>,
    replacement: &ExecTerm,
) -> (result: Option<ExecClause>)
    ensures match result {
        Some(replaced) => replace_clause_literal(
            view_clause(*clause), literal_index as int, slot as nat,
            view_position(path@), view(*replacement),
        ) == Some(view_clause(replaced))
            && replaced.literals@.len() == clause.literals@.len()
            && forall|j: int| 0 <= j < replaced.literals@.len()
                ==> view_literal(replaced.literals@[j])
                    == match replace_clause_literal(
                        view_clause(*clause), literal_index as int, slot as nat,
                        view_position(path@), view(*replacement),
                    ) {
                        Some(result) => result.literals[j],
                        None => view_clause(*clause).literals[j],
                    },
        None => replace_clause_literal(
            view_clause(*clause), literal_index as int, slot as nat,
            view_position(path@), view(*replacement),
        ) == None,
    }
{
    if literal_index >= clause.literals.len() {
        return None;
    }
    match replace_literal_position_exec(
        &clause.literals[literal_index], slot, path, replacement,
    ) {
        None => None,
        Some(rewritten) => {
            let mut literals: Vec<ExecLiteral> = Vec::new();
            let mut i: usize = 0;
            while i < clause.literals.len()
                invariant
                    i <= clause.literals.len(),
                    literals@.len() == i,
                    literal_index < clause.literals.len(),
                    forall|j: int| 0 <= j < i as int ==> if j == literal_index as int {
                        view_literal(literals@[j]) == view_literal(rewritten)
                    } else {
                        view_literal(literals@[j]) == view_literal(clause.literals@[j])
                    },
                decreases clause.literals.len() - i
            {
                if i == literal_index {
                    let copied = copy_literal(&rewritten);
                    assert(view_literal(copied) == view_literal(rewritten));
                    literals.push(copied);
                } else {
                    let copied = copy_literal(&clause.literals[i]);
                    assert(view_literal(copied) == view_literal(clause.literals@[i as int]));
                    literals.push(copied);
                }
                assert forall|j: int| 0 <= j < (i + 1) as int
                    implies if j == literal_index as int {
                        view_literal(literals@[j]) == view_literal(rewritten)
                    } else {
                        view_literal(literals@[j]) == view_literal(clause.literals@[j])
                    } by {
                    if j < i as int {
                    } else if i == literal_index {
                        assert(j == literal_index as int);
                    } else {
                        assert(j == i as int);
                    }
                }
                i += 1;
            }
            let result = ExecClause { literals };
            assert(result.literals@.len() == clause.literals@.len());
            assert forall|j: int| 0 <= j < result.literals@.len()
                implies if j == literal_index as int {
                    view_literal(result.literals@[j]) == view_literal(rewritten)
                } else {
                    view_literal(result.literals@[j]) == view_literal(clause.literals@[j])
                } by {
                assert(j < i as int);
            }
            assert(view_clause(result).literals
                == view_clause(*clause).literals.update(
                    literal_index as int, view_literal(rewritten)));
            Some(result)
        },
    }
}

proof fn clause_literal_replacement_is_pointwise(
    clause: ExecClause,
    literal_index: int,
    slot: nat,
    position: TermPosition,
    replacement: Term,
    replaced: Clause,
)
    requires
        0 <= literal_index < clause.literals@.len(),
        replace_clause_literal(view_clause(clause), literal_index, slot, position, replacement)
            == Some(replaced),
    ensures replaced.literals.len() == clause.literals@.len()
        && forall|j: int| 0 <= j < replaced.literals.len()
            ==> replaced.literals[j] == if j == literal_index {
                match replace_literal_at_position(
                    view_literal(clause.literals@[literal_index]), slot, position, replacement,
                ) {
                    Some(literal) => literal,
                    None => view_literal(clause.literals@[j]),
                }
            } else { view_literal(clause.literals@[j]) }
{}

fn executable_clause_replay_example()
{
    let clause = ExecClause {
        literals: vec![
            ExecLiteral {
                positive: true,
                atom: ExecAtom::Pred1(2, Box::new(ExecTerm::Var(17))),
            },
            ExecLiteral {
                positive: false,
                atom: ExecAtom::Pred2(
                    3,
                    Box::new(ExecTerm::Const(5)),
                    Box::new(ExecTerm::Var(17)),
                ),
            },
        ],
    };
    let bindings = vec![ExecBinding { variable: 17, replacement: ExecTerm::Const(8) }];
    let instance = substitute_clause_exec(&clause, &bindings);
    assert(view_clause(instance)
        == substitute_clause(view_clause(clause), view_subst(bindings@)));

    let path = Vec::<u64>::new();
    let replacement = ExecTerm::Const(11);
    let rewritten = replace_clause_literal_exec(&instance, 1, 0, &path, &replacement);
    assert(rewritten.is_some());

    let invalid_index = replace_clause_literal_exec(&instance, 2, 0, &path, &replacement);
    assert(invalid_index.is_none());
}

/// Positive and negative examples for the supplied finite-unifier checker.
/// Substitutions have simultaneous, one-step RHS semantics; cyclic RHS terms
/// are permitted as substitutions, but are accepted only when that one-step
/// application makes the selected terms equal. The checker does not compute
/// MGUs and does not impose a separate occurs-check completeness policy.
fn executable_unifier_certificate_examples()
{
    let left = ExecTerm::Var(11);
    let right = ExecTerm::Const(7);
    let valid = vec![ExecBinding { variable: 11, replacement: ExecTerm::Const(7) }];
    let valid_domain_unique = check_domain_unique(&valid, 1);
    assert(valid_domain_unique);
    let valid_result = check_unifier(&left, &right, &valid, 1);
    assert(domain_unique(view_subst(valid@)));
    assert(lookup(view_subst(valid@), 11) == Some(Term::Const(7)));
    assert(valid_result);

    let wrong_instance = vec![ExecBinding {
        variable: 11, replacement: ExecTerm::Const(8),
    }];
    let wrong_instance_result = check_unifier(&left, &right, &wrong_instance, 1);
    assert(domain_unique(view_subst(wrong_instance@)));
    assert(lookup(view_subst(wrong_instance@), 11) == Some(Term::Const(8)));
    assert(!wrong_instance_result);

    let wrong_shape = ExecTerm::App1(4, Box::new(ExecTerm::Const(7)));
    let wrong_shape_result = check_unifier(&left, &wrong_shape, &valid, 1);
    assert(!wrong_shape_result);

    let duplicate_domain = vec![
        ExecBinding { variable: 11, replacement: ExecTerm::Const(7) },
        ExecBinding { variable: 11, replacement: ExecTerm::Const(8) },
    ];
    let duplicate_domain_result = check_unifier(&left, &right, &duplicate_domain, 4);
    assert(duplicate_domain[0].variable == duplicate_domain[1].variable);
    assert(!duplicate_domain_result);

    let distinct_domain = vec![
        ExecBinding { variable: 11, replacement: ExecTerm::Const(7) },
        ExecBinding { variable: 12, replacement: ExecTerm::Const(8) },
    ];
    let insufficient_domain_budget = check_domain_unique(&distinct_domain, 0);
    assert(!insufficient_domain_budget);

    let cyclic = vec![ExecBinding {
        variable: 11,
        replacement: ExecTerm::App1(5, Box::new(ExecTerm::Var(11))),
    }];
    let cycle_left = ExecTerm::Var(11);
    let cycle_right = ExecTerm::Var(11);
    assert(apply(view(cycle_left), view_subst(cyclic@))
        == apply(view(cycle_right), view_subst(cyclic@)));
    let cyclic_result = check_unifier(&cycle_left, &cycle_right, &cyclic, 1);
    assert(cyclic_result);
}

fn executable_standardization_apart_example()
{
    let left = ExecClause {
        literals: vec![ExecLiteral {
            positive: true,
            atom: ExecAtom::Pred1(1, Box::new(ExecTerm::Var(7))),
        }],
    };
    let right = ExecClause {
        literals: vec![ExecLiteral {
            positive: true,
            atom: ExecAtom::Pred1(2, Box::new(ExecTerm::App1(
                3, Box::new(ExecTerm::Var(7)),
            ))),
        }],
    };
    let left_variables = collect_clause_variables_bounded(&left, 10000);
    let right_variables = collect_clause_variables_bounded(&right, 10000);
    let left_variables = match left_variables {
        Some(vars) => vars,
        None => Vec::new(),
    };
    let right_variables = match right_variables {
        Some(vars) => vars,
        None => Vec::new(),
    };

    let left_mapping = vec![ExecBinding { variable: 7, replacement: ExecTerm::Var(100) }];
    let right_mapping = vec![ExecBinding { variable: 7, replacement: ExecTerm::Var(200) }];
    let valid = check_standardized_apart_pair(
        &left, &right, &left_mapping, &right_mapping, 100000,
    );
    let _valid_step = valid;
    let colliding_mapping = vec![ExecBinding {
        variable: 7,
        replacement: ExecTerm::Var(100),
    }];
    let collision = check_standardized_apart_pair(
        &left, &right, &left_mapping, &colliding_mapping, 100000,
    );
    let _collision_result = collision;

    let duplicate_sources = vec![
        ExecBinding { variable: 7, replacement: ExecTerm::Var(300) },
        ExecBinding { variable: 8, replacement: ExecTerm::Var(300) },
    ];
}

fn executable_superposition_certificate_example()
{
    let source = ExecClause {
        literals: vec![ExecLiteral {
            positive: true,
            atom: ExecAtom::Eq(
                ExecTerm::App1(4, Box::new(ExecTerm::Const(1))),
                ExecTerm::Const(2),
            ),
        }],
    };
    let target = ExecClause {
        literals: vec![ExecLiteral {
            positive: true,
            atom: ExecAtom::Pred1(8, Box::new(ExecTerm::App1(
                4, Box::new(ExecTerm::Const(1)),
            ))),
        }],
    };
    let certificate = SuperpositionCertificate {
        source,
        target,
        source_freshening: Vec::new(),
        target_freshening: Vec::new(),
        source_equality_index: 0,
        target_literal_index: 0,
        target_argument_slot: 0,
        target_path: Vec::new(),
        fresh_target_path: Vec::new(),
        unifier: Vec::new(),
        conclusion: ExecClause { literals: vec![ExecLiteral {
            positive: true,
            atom: ExecAtom::Pred1(8, Box::new(ExecTerm::Const(2))),
        }] },
    };
    let _accepted = check_superposition_and_replay(&certificate, 1000);
    let bad_path = SuperpositionCertificate {
        source: ExecClause { literals: vec![ExecLiteral {
            positive: true,
            atom: ExecAtom::Eq(
                ExecTerm::App1(4, Box::new(ExecTerm::Const(1))),
                ExecTerm::Const(2),
            ),
        }] },
        target: ExecClause { literals: vec![ExecLiteral {
            positive: true,
            atom: ExecAtom::Pred1(8, Box::new(ExecTerm::App1(
                4, Box::new(ExecTerm::Const(1)),
            ))),
        }] },
        source_freshening: Vec::new(),
        target_freshening: Vec::new(),
        source_equality_index: 0,
        target_literal_index: 0,
        target_argument_slot: 0,
        target_path: Vec::new(),
        fresh_target_path: vec![1u64],
        unifier: Vec::new(),
        conclusion: ExecClause { literals: vec![ExecLiteral {
            positive: true,
            atom: ExecAtom::Pred1(8, Box::new(ExecTerm::Const(2))),
        }] },
    };
    let _bad_path_rejected = !check_superposition_certificate(&bad_path, 1000);
    let bad_conclusion = SuperpositionCertificate {
        source: ExecClause { literals: vec![ExecLiteral {
            positive: true,
            atom: ExecAtom::Eq(
                ExecTerm::App1(4, Box::new(ExecTerm::Const(1))),
                ExecTerm::Const(2),
            ),
        }] },
        target: ExecClause { literals: vec![ExecLiteral {
            positive: true,
            atom: ExecAtom::Pred1(8, Box::new(ExecTerm::App1(
                4, Box::new(ExecTerm::Const(1)),
            ))),
        }] },
        source_freshening: Vec::new(),
        target_freshening: Vec::new(),
        source_equality_index: 0,
        target_literal_index: 0,
        target_argument_slot: 0,
        target_path: Vec::new(),
        fresh_target_path: Vec::new(),
        unifier: Vec::new(),
        conclusion: ExecClause { literals: vec![ExecLiteral {
            positive: true,
            atom: ExecAtom::Pred1(8, Box::new(ExecTerm::Const(3))),
        }] },
    };
    let _bad_conclusion_rejected = !check_superposition_and_replay(&bad_conclusion, 1000);
}

fn copy_literal(literal: &ExecLiteral) -> (copy: ExecLiteral)
    ensures view_literal(copy) == view_literal(*literal)
{
    ExecLiteral { positive: literal.positive, atom: copy_atom(&literal.atom) }
}

fn append_literal_copy(literals: &mut Vec<ExecLiteral>, literal: &ExecLiteral)
    ensures final(literals)@.len() == old(literals)@.len() + 1,
        forall|j: int| 0 <= j < old(literals)@.len()
            ==> final(literals)@[j] == old(literals)@[j],
        view_literal(final(literals)@[old(literals)@.len() as int]) == view_literal(*literal)
{
    literals.push(copy_literal(literal));
    assert(literals@.len() == old(literals)@.len() + 1);
    assert(view_literal(literals@[old(literals)@.len() as int]) == view_literal(*literal));
}

fn build_superposition_conclusion_exec(
    source: &ExecClause,
    source_equality_index: usize,
    rewritten_target: &ExecClause,
) -> (conclusion: ExecClause)
    requires source_equality_index < source.literals.len(),
        source.literals.len() - 1 + rewritten_target.literals.len() <= usize::MAX,
    ensures view_clause(conclusion).literals.len()
            == source.literals.len() - 1 + rewritten_target.literals.len(),
        forall|k: int| 0 <= k < view_clause(conclusion).literals.len()
            ==> view_clause(conclusion).literals[k]
                == if k < source.literals.len() as int - 1 {
                    view_literal(source.literals@[if k < source_equality_index as int {
                        k
                    } else {
                        k + 1
                    }])
                } else {
                    view_literal(rewritten_target.literals@[
                        k - (source.literals.len() as int - 1)])
                }
{
    let mut literals: Vec<ExecLiteral> = Vec::new();
    let mut i: usize = 0;
    while i < source.literals.len()
        invariant i <= source.literals.len(),
            literals@.len() == if i <= source_equality_index {
                i as int
            } else {
                i as int - 1
            },
            forall|k: int| 0 <= k < literals@.len()
                ==> view_literal(literals@[k])
                    == view_literal(source.literals@[if k < source_equality_index as int {
                        k
                    } else {
                        k + 1
                    }]),
        decreases source.literals.len() - i
    {
        if i != source_equality_index {
            let before_len = literals.len();
            append_literal_copy(&mut literals, &source.literals[i]);
            assert forall|k: int| 0 <= k < literals@.len()
                implies view_literal(literals@[k])
                    == view_literal(source.literals@[if k < source_equality_index as int {
                        k
                    } else {
                        k + 1
                    }]) by {
                if k < before_len as int {
                } else {
                    assert(k == before_len as int);
                    assert(k == if i <= source_equality_index {
                        i as int
                    } else {
                        i as int - 1
                    });
                    assert(i != source_equality_index);
                    assert(if i < source_equality_index {
                        k == i as int
                    } else {
                        k + 1 == i as int
                    });
                    assert(view_literal(literals@[k]) == view_literal(source.literals@[i as int]));
                }
            }
        }
        i += 1;
    }
    let mut j: usize = 0;
    assert(literals@.len() == source.literals.len() as int - 1);
    while j < rewritten_target.literals.len()
        invariant j <= rewritten_target.literals.len(),
            literals@.len() == source.literals.len() - 1 + j,
            forall|k: int| 0 <= k < literals@.len()
                ==> view_literal(literals@[k])
                    == if k < source.literals.len() as int - 1 {
                        view_literal(source.literals@[if k < source_equality_index as int {
                            k
                        } else {
                            k + 1
                        }])
                    } else {
                        view_literal(rewritten_target.literals@[
                            k - (source.literals.len() as int - 1)])
                    },
        decreases rewritten_target.literals.len() - j
    {
        let before_len = literals.len();
        append_literal_copy(&mut literals, &rewritten_target.literals[j]);
        assert forall|k: int| 0 <= k < literals@.len()
            implies view_literal(literals@[k])
                == if k < source.literals.len() as int - 1 {
                    view_literal(source.literals@[if k < source_equality_index as int {
                        k
                    } else {
                        k + 1
                    }])
                } else {
                    view_literal(rewritten_target.literals@[
                        k - (source.literals.len() as int - 1)])
                } by {
            if k < before_len as int {
            } else {
                assert(k == before_len as int);
                assert(k == source.literals.len() as int - 1 + j as int);
                assert(k - (source.literals.len() as int - 1) == j as int);
                assert(view_literal(literals@[k])
                    == view_literal(rewritten_target.literals@[j as int]));
            }
        }
        j += 1;
    }
    let conclusion = ExecClause { literals };
    assert(view_clause(conclusion).literals.len() == source.literals.len() - 1
        + rewritten_target.literals.len());
    conclusion
}

fn copy_atom(atom: &ExecAtom) -> (copy: ExecAtom)
    ensures view_atom(copy) == view_atom(*atom)
    decreases atom
{
    match atom {
        ExecAtom::Pred0(p) => ExecAtom::Pred0(*p),
        ExecAtom::Pred1(p, arg) => ExecAtom::Pred1(*p, Box::new(copy_term(arg))),
        ExecAtom::Pred2(p, left, right) => ExecAtom::Pred2(
            *p, Box::new(copy_term(left)), Box::new(copy_term(right)),
        ),
        ExecAtom::Eq(left, right) => ExecAtom::Eq(copy_term(left), copy_term(right)),
    }
}

fn replace_atom_position_exec(
    atom: &ExecAtom,
    slot: u64,
    path: &Vec<u64>,
    replacement: &ExecTerm,
) -> (result: Option<ExecAtom>)
    ensures match result {
        Some(replaced) => replace_atom_at_position(
            view_atom(*atom), slot as nat, view_position(path@), view(*replacement),
        ) == Some(view_atom(replaced)),
        None => replace_atom_at_position(
            view_atom(*atom), slot as nat, view_position(path@), view(*replacement),
        ) == None,
    }
    decreases atom
{
    match atom {
        ExecAtom::Pred0(_) => None,
        ExecAtom::Pred1(p, arg) => {
            if slot == 0 {
                match replace_at_exec(arg, path, replacement, 0) {
                    Some(arg2) => Some(ExecAtom::Pred1(*p, Box::new(arg2))),
                    None => None,
                }
            } else {
                None
            }
        },
        ExecAtom::Pred2(p, left, right) => {
            if slot == 0 {
                match replace_at_exec(left, path, replacement, 0) {
                    Some(left2) => Some(ExecAtom::Pred2(
                        *p, Box::new(left2), Box::new(copy_term(right)),
                    )),
                    None => None,
                }
            } else if slot == 1 {
                match replace_at_exec(right, path, replacement, 0) {
                    Some(right2) => Some(ExecAtom::Pred2(
                        *p, Box::new(copy_term(left)), Box::new(right2),
                    )),
                    None => None,
                }
            } else {
                None
            }
        },
        ExecAtom::Eq(left, right) => {
            if slot == 0 {
                match replace_at_exec(left, path, replacement, 0) {
                    Some(left2) => Some(ExecAtom::Eq(left2, copy_term(right))),
                    None => None,
                }
            } else if slot == 1 {
                match replace_at_exec(right, path, replacement, 0) {
                    Some(right2) => Some(ExecAtom::Eq(copy_term(left), right2)),
                    None => None,
                }
            } else {
                None
            }
        },
    }
}

fn replace_literal_position_exec(
    literal: &ExecLiteral,
    slot: u64,
    path: &Vec<u64>,
    replacement: &ExecTerm,
) -> (result: Option<ExecLiteral>)
    ensures match result {
        Some(replaced) => replace_literal_at_position(
            view_literal(*literal), slot as nat, view_position(path@), view(*replacement),
        ) == Some(view_literal(replaced)),
        None => replace_literal_at_position(
            view_literal(*literal), slot as nat, view_position(path@), view(*replacement),
        ) == None,
    }
{
    match replace_atom_position_exec(&literal.atom, slot, path, replacement) {
        Some(atom2) => Some(ExecLiteral { positive: literal.positive, atom: atom2 }),
        None => None,
    }
}

fn executable_literal_replay_examples()
{
    let literal = ExecLiteral {
        positive: true,
        atom: ExecAtom::Pred2(
            12,
            Box::new(ExecTerm::Const(1)),
            Box::new(ExecTerm::App2(
                3,
                Box::new(ExecTerm::Var(4)),
                Box::new(ExecTerm::Const(5)),
            )),
        ),
    };
    let bindings = vec![ExecBinding {
        variable: 4,
        replacement: ExecTerm::Const(9),
    }];
    let instance = substitute_literal_exec(&literal, &bindings);
    assert(view_literal(instance) == substitute_literal(view_literal(literal), view_subst(bindings@)));

    let path = Vec::<u64>::new();
    let replacement = ExecTerm::Const(7);
    let rewritten = replace_literal_position_exec(&literal, 1u64, &path, &replacement);
    assert(rewritten.is_some());
    assert(view_literal(rewritten.unwrap()) == Literal {
        positive: true,
        atom: Atom::Pred2(
            12,
            Box::new(Term::Const(1)),
            Box::new(Term::Const(7)),
        ),
    });

    let bad_slot = replace_literal_position_exec(&literal, 2u64, &path, &replacement);
    assert(bad_slot.is_none());
}

fn check_domain_unique(bindings: &Vec<ExecBinding>, work_limit: usize) -> (unique: bool)
    ensures unique == ((bindings.len() == 0
        || bindings.len() <= work_limit / bindings.len())
        && domain_unique(view_subst(bindings@)))
{
    // This nested scan performs at most n*n comparisons. Refuse before
    // starting if that work cannot fit in the supplied budget.
    if bindings.len() > 0 && bindings.len() > work_limit / bindings.len() {
        return false;
    }
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
                proof {
                    let p = i as int;
                    let q = j as int;
                    assert(0 <= p && p < q && q < bindings@.len());
                    assert(bindings@[p].variable == bindings@[q].variable);
                    assert(!domain_unique(view_subst(bindings@))) by {
                        assert(!(view_subst(bindings@).bindings[p].0
                            != view_subst(bindings@).bindings[q].0));
                    }
                }
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

fn charge_term_structure(term: &ExecTerm, work: &mut usize, limit: usize) -> (ok: bool)
    requires *work <= limit
    ensures *final(work) <= limit
    decreases term
{
    if *work >= limit { return false; }
    *work += 1;
    match term {
        ExecTerm::Var(_) | ExecTerm::Const(_) => true,
        ExecTerm::App1(_, child) => charge_term_structure(child, work, limit),
        ExecTerm::App2(_, left, right) => {
            if !charge_term_structure(left, work, limit) { return false; }
            charge_term_structure(right, work, limit)
        },
    }
}

fn charge_atom_structure(atom: &ExecAtom, work: &mut usize, limit: usize) -> (ok: bool)
    requires *work <= limit
    ensures *final(work) <= limit
    decreases atom
{
    if *work >= limit { return false; }
    *work += 1;
    match atom {
        ExecAtom::Pred0(_) => true,
        ExecAtom::Pred1(_, arg) => charge_term_structure(arg, work, limit),
        ExecAtom::Pred2(_, left, right) => {
            if !charge_term_structure(left, work, limit) { return false; }
            charge_term_structure(right, work, limit)
        },
        ExecAtom::Eq(left, right) => {
            if !charge_term_structure(left, work, limit) { return false; }
            charge_term_structure(right, work, limit)
        },
    }
}

fn charge_clause_structure(clause: &ExecClause, work: &mut usize, limit: usize) -> (ok: bool)
    requires *work <= limit
    ensures *final(work) <= limit
{
    let mut i: usize = 0;
    while i < clause.literals.len()
        invariant i <= clause.literals.len(), *work <= limit
        decreases clause.literals.len() - i
    {
        if *work >= limit { return false; }
        *work += 1;
        if !charge_atom_structure(&clause.literals[i].atom, work, limit) { return false; }
        i += 1;
    }
    true
}

fn charge_binding_structure(bindings: &Vec<ExecBinding>, work: &mut usize, limit: usize)
    -> (ok: bool)
    requires *work <= limit
    ensures *final(work) <= limit
{
    let mut i: usize = 0;
    while i < bindings.len()
        invariant i <= bindings.len(), *work <= limit
        decreases bindings.len() - i
    {
        if *work >= limit { return false; }
        *work += 1;
        if !charge_term_structure(&bindings[i].replacement, work, limit) { return false; }
        i += 1;
    }
    true
}

fn charge_u64_sequence(path: &Vec<u64>, work: &mut usize, limit: usize) -> (ok: bool)
    requires *work <= limit
    ensures *final(work) <= limit
{
    if path.len() > limit - *work { return false; }
    *work += path.len();
    true
}

fn charge_certificate_structure(
    certificate: &SuperpositionCertificate,
    work: &mut usize,
    limit: usize,
) -> (ok: bool)
    requires *work <= limit
    ensures *final(work) <= limit
{
    if !charge_clause_structure(&certificate.source, work, limit)
        || !charge_clause_structure(&certificate.target, work, limit)
        || !charge_clause_structure(&certificate.conclusion, work, limit)
        || !charge_binding_structure(&certificate.source_freshening, work, limit)
        || !charge_binding_structure(&certificate.target_freshening, work, limit)
        || !charge_binding_structure(&certificate.unifier, work, limit)
        || !charge_u64_sequence(&certificate.target_path, work, limit)
        || !charge_u64_sequence(&certificate.fresh_target_path, work, limit)
    {
        return false;
    }
    true
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

fn same_atom(left: &ExecAtom, right: &ExecAtom) -> (same: bool)
    ensures same == (view_atom(*left) == view_atom(*right))
    decreases left
{
    match (left, right) {
        (ExecAtom::Pred0(p), ExecAtom::Pred0(q)) => p == q,
        (ExecAtom::Pred1(p, a), ExecAtom::Pred1(q, b)) => *p == *q && same_term(a, b),
        (ExecAtom::Pred2(p, a1, a2), ExecAtom::Pred2(q, b1, b2)) =>
            *p == *q && same_term(a1, b1) && same_term(a2, b2),
        (ExecAtom::Eq(a1, a2), ExecAtom::Eq(b1, b2)) =>
            same_term(a1, b1) && same_term(a2, b2),
        _ => false,
    }
}

fn same_literal(left: &ExecLiteral, right: &ExecLiteral) -> (same: bool)
    ensures same == (view_literal(*left) == view_literal(*right))
{
    left.positive == right.positive && same_atom(&left.atom, &right.atom)
}

fn same_clause(left: &ExecClause, right: &ExecClause) -> (same: bool)
    ensures same ==> clause_equal(view_clause(*left), view_clause(*right))
{
    if left.literals.len() != right.literals.len() { return false; }
    let mut i: usize = 0;
    while i < left.literals.len()
        invariant
            i <= left.literals.len(),
            left.literals.len() == right.literals.len(),
            forall|j: int| 0 <= j < i as int
                ==> view_literal(left.literals@[j]) == view_literal(right.literals@[j]),
        decreases left.literals.len() - i
    {
        if !same_literal(&left.literals[i], &right.literals[i]) {
            return false;
        }
        i += 1;
    }
    assert forall|j: int| 0 <= j < left.literals@.len()
        implies view_literal(left.literals@[j]) == view_literal(right.literals@[j]) by {
        assert(j < i as int);
    }
    true
}

fn same_clause_exact(left: &ExecClause, right: &ExecClause) -> (same: bool)
    ensures same ==> clause_equal(view_clause(*left), view_clause(*right))
{
    if left.literals.len() != right.literals.len() { return false; }
    let mut i: usize = 0;
    while i < left.literals.len()
        invariant
            i <= left.literals.len(),
            left.literals.len() == right.literals.len(),
            forall|j: int| 0 <= j < i as int
                ==> view_literal(left.literals@[j]) == view_literal(right.literals@[j]),
        decreases left.literals.len() - i
    {
        let same = same_literal(&left.literals[i], &right.literals[i]);
        if !same { return false; }
        i += 1;
    }
    assert forall|j: int| 0 <= j < left.literals@.len()
        implies view_literal(left.literals@[j]) == view_literal(right.literals@[j]) by {
        assert(j < i as int);
    }
    true
}

/// Validate a supplied finite simultaneous-unifier certificate. This does not
/// compute an MGU; it verifies domain uniqueness and that applying the same
/// substitution to both selected terms makes them equal.
fn check_unifier(
    left: &ExecTerm,
    right: &ExecTerm,
    bindings: &Vec<ExecBinding>,
    work_limit: usize,
) -> (unifies: bool)
    ensures unifies == ((bindings.len() == 0
        || bindings.len() <= work_limit / bindings.len())
        && domain_unique(view_subst(bindings@))
        && apply(view(*left), view_subst(bindings@))
            == apply(view(*right), view_subst(bindings@)))
{
    if !check_domain_unique(bindings, work_limit) {
        return false;
    }
    let left_instance = apply_exec(left, bindings);
    let right_instance = apply_exec(right, bindings);
    let same = same_term(&left_instance, &right_instance);
    assert(same == (apply(view(*left), view_subst(bindings@))
        == apply(view(*right), view_subst(bindings@))));
    same
}

proof fn accepted_unifier_has_semantic_equal_instances(
    left: ExecTerm,
    right: ExecTerm,
    bindings: Seq<ExecBinding>,
)
    requires domain_unique(view_subst(bindings)),
        apply(view(left), view_subst(bindings)) == apply(view(right), view_subst(bindings)),
    ensures apply(view(left), view_subst(bindings))
        == apply(view(right), view_subst(bindings))
{}

fn selected_atom_term_exec(
    atom: &ExecAtom,
    slot: u64,
    path: &Vec<u64>,
) -> (result: Option<ExecTerm>)
    ensures match result {
        Some(term) => atom_at_position(view_atom(*atom), slot as nat, view_position(path@))
            == Some(view(term)),
        None => atom_at_position(view_atom(*atom), slot as nat, view_position(path@)) == None,
    }
{
    let root = match atom {
        ExecAtom::Pred0(_) => return None,
        ExecAtom::Pred1(_, arg) => {
            if slot != 0 { return None; }
            copy_term(arg)
        },
        ExecAtom::Pred2(_, left, right) => {
            if slot == 0 { copy_term(left) }
            else if slot == 1 { copy_term(right) }
            else { return None; }
        },
        ExecAtom::Eq(left, right) => {
            if slot == 0 { copy_term(left) }
            else if slot == 1 { copy_term(right) }
            else { return None; }
        },
    };
    let result = subterm_at_exec(&root, path, 0);
    assert(match result {
        Some(term) => atom_at_position(view_atom(*atom), slot as nat, view_position(path@))
            == Some(view(term)),
        None => atom_at_position(view_atom(*atom), slot as nat, view_position(path@)) == None,
    });
    result
}

proof fn proposed_unifier_certificate_sound(
    left: ExecTerm,
    right: ExecTerm,
    bindings: Seq<ExecBinding>,
)
    requires domain_unique(view_subst(bindings)),
        apply(view(left), view_subst(bindings)) == apply(view(right), view_subst(bindings)),
    ensures view(apply_exec_model(left, bindings)) == view(apply_exec_model(right, bindings))
{
    apply_view_exec_term(left, bindings);
    apply_view_exec_term(right, bindings);
}

spec fn selected_source_left(certificate: SuperpositionCertificate) -> Term {
    match certificate.source.literals@[certificate.source_equality_index as int].atom {
        ExecAtom::Eq(left, _) => apply(view(left), view_subst(certificate.source_freshening@)),
        _ => Term::Var(0),
    }
}

spec fn selected_source_left_after_freshening(certificate: SuperpositionCertificate) -> Term {
    apply(selected_source_left(certificate), view_subst(certificate.unifier@))
}

spec fn certificate_selected_source_left(certificate: SuperpositionCertificate) -> Term {
    match certificate.source.literals@[certificate.source_equality_index as int].atom {
        ExecAtom::Eq(left, _) => apply(
            apply(view(left), view_subst(certificate.source_freshening@)),
            view_subst(certificate.unifier@),
        ),
        _ => Term::Var(0),
    }
}

spec fn certificate_selected_source_right(certificate: SuperpositionCertificate) -> Term {
    match certificate.source.literals@[certificate.source_equality_index as int].atom {
        ExecAtom::Eq(_, right) => apply(
            apply(view(right), view_subst(certificate.source_freshening@)),
            view_subst(certificate.unifier@),
        ),
        _ => Term::Var(0),
    }
}

spec fn certificate_selected_target_raw_term(certificate: SuperpositionCertificate) -> Option<Term> {
    atom_at_position(
        view_atom(certificate.target.literals@[certificate.target_literal_index as int].atom),
        certificate.target_argument_slot as nat,
        view_position(certificate.target_path@),
    )
}

spec fn certificate_selected_target_fresh_term(certificate: SuperpositionCertificate) -> Option<Term> {
    match atom_at_position(
        view_atom(certificate.target.literals@[certificate.target_literal_index as int].atom),
        certificate.target_argument_slot as nat,
        view_position(certificate.target_path@),
    ) {
        Some(term) => Some(apply(term, view_subst(certificate.target_freshening@))),
        None => None,
    }
}

spec fn selected_source_right(certificate: SuperpositionCertificate) -> Term {
    match certificate.source.literals@[certificate.source_equality_index as int].atom {
        ExecAtom::Eq(_, right) => view(right),
        _ => Term::Var(0),
    }
}

spec fn selected_target_subterm(certificate: SuperpositionCertificate) -> Term {
    match atom_at_position(
        view_atom(certificate.target.literals@[certificate.target_literal_index as int].atom),
        certificate.target_argument_slot as nat,
        view_position(certificate.target_path@),
    ) {
        Some(term) => apply(term, view_subst(certificate.target_freshening@)),
        None => Term::Var(0),
    }
}

spec fn selected_target_subterm_after_freshening(certificate: SuperpositionCertificate) -> Term {
    apply(selected_target_subterm(certificate), view_subst(certificate.unifier@))
}

spec fn selected_rewrite_rhs(certificate: SuperpositionCertificate) -> Term {
    apply(apply(selected_source_right(certificate),
        view_subst(certificate.source_freshening@)), view_subst(certificate.unifier@))
}

spec fn expected_superposition_conclusion(
    certificate: SuperpositionCertificate,
    work_limit: nat,
) -> Seq<Literal> {
    let source_fresh = substitute_clause(view_clause(certificate.source),
        view_subst(certificate.source_freshening@));
    let target_fresh = substitute_clause(view_clause(certificate.target),
        view_subst(certificate.target_freshening@));
    let source_instance = substitute_clause(source_fresh, view_subst(certificate.unifier@));
    let target_instance = substitute_clause(target_fresh, view_subst(certificate.unifier@));
    let right = match certificate.source.literals@[certificate.source_equality_index as int].atom {
        ExecAtom::Eq(_, rhs) => apply(apply(view(rhs), view_subst(certificate.source_freshening@)),
            view_subst(certificate.unifier@)),
        _ => Term::Var(0),
    };
    let target_rewritten = match replace_literal_at_position(
        target_instance.literals[certificate.target_literal_index as int],
        certificate.target_argument_slot as nat,
        view_position(certificate.fresh_target_path@),
        right,
    ) {
        Some(literal) => literal,
        None => target_instance.literals[certificate.target_literal_index as int],
    };
    source_instance.literals.subrange(0, certificate.source_equality_index as int)
        + source_instance.literals.subrange(
            certificate.source_equality_index as int + 1, source_instance.literals.len() as int)
        + target_instance.literals.update(certificate.target_literal_index as int, target_rewritten)
}

fn check_superposition_certificate(certificate: &SuperpositionCertificate, work_limit: usize)
    -> (accepted: bool)
    ensures accepted ==> certificate.target_path@ == certificate.fresh_target_path@
        && accepted ==> certificate.source.literals.len()
            + certificate.target.literals.len()
            + certificate.source_freshening.len()
            + certificate.target_freshening.len()
            + certificate.unifier.len()
            + certificate.target_path.len()
            + certificate.fresh_target_path.len()
            + certificate.conclusion.literals.len() <= work_limit
        && accepted ==> certificate.source_equality_index < certificate.source.literals.len()
        && accepted ==> certificate.target_literal_index < certificate.target.literals.len()
        && accepted ==> certificate.source.literals@[certificate.source_equality_index as int].positive
        && accepted ==> match certificate.source.literals@[certificate.source_equality_index as int].atom {
            ExecAtom::Eq(ExecTerm::Var(_), _) => false,
            ExecAtom::Eq(_, _) => true,
            _ => false,
        }
        && accepted ==> forall|v: u64| term_clause_view_contains(certificate.source, v)
            ==> exists|fresh: u64| lookup(view_subst(certificate.source_freshening@), v)
                == Some(Term::Var(fresh))
        && accepted ==> forall|v: u64| term_clause_view_contains(certificate.target, v)
            ==> exists|fresh: u64| lookup(view_subst(certificate.target_freshening@), v)
                == Some(Term::Var(fresh))
        && accepted ==> forall|v: u64|
            literal_variables(view_literal(
                certificate.target.literals@[certificate.target_literal_index as int]))
                .contains(v)
            ==> exists|fresh: u64|
                lookup(view_subst(certificate.target_freshening@), v)
                    == Some(Term::Var(fresh))
        && accepted ==> forall|v: u64|
            literal_variables(view_literal(
                certificate.target.literals@[certificate.target_literal_index as int]))
                .contains(v)
            ==> term_clause_view_contains(certificate.target, v)
        && accepted ==> certificate_selected_source_left(*certificate)
            == selected_target_subterm_after_freshening(*certificate)
        && accepted ==> selected_rewrite_rhs(*certificate)
            == apply(apply(certificate_selected_source_right(*certificate),
                view_subst(certificate.source_freshening@)), view_subst(certificate.unifier@))
        && accepted ==> selected_source_left_after_freshening(*certificate)
            == selected_target_subterm_after_freshening(*certificate)
        && accepted ==> selected_rewrite_rhs(*certificate)
            == apply(apply(selected_source_right(*certificate),
                view_subst(certificate.source_freshening@)), view_subst(certificate.unifier@))
        && accepted ==> certificate_selected_source_left(*certificate)
            == apply(apply(selected_source_left(*certificate),
                view_subst(certificate.source_freshening@)),
                view_subst(certificate.unifier@))
        && accepted ==> certificate_selected_target_fresh_term(*certificate)
            == Some(selected_target_subterm(*certificate))
        && accepted ==> match certificate_selected_target_raw_term(*certificate) {
            Some(raw) => selected_target_subterm(*certificate)
                == apply(raw, view_subst(certificate.target_freshening@)),
            None => false,
        }
        && accepted ==> match certificate_selected_target_fresh_term(*certificate) {
            Some(fresh) => apply(fresh, view_subst(certificate.unifier@))
                == selected_target_subterm_after_freshening(*certificate),
            None => false,
        }
        && accepted ==> literal_at_position(
            substitute_literal(
                substitute_literal(
                    view_literal(certificate.target.literals@[
                        certificate.target_literal_index as int]),
                    view_subst(certificate.target_freshening@)),
                view_subst(certificate.unifier@)),
            certificate.target_argument_slot as nat,
            view_position(certificate.target_path@),
        ) == Some(selected_target_subterm_after_freshening(*certificate))
        && accepted ==> replace_literal_at_position(
            substitute_literal(
                substitute_literal(
                    view_literal(certificate.target.literals@[
                        certificate.target_literal_index as int]),
                    view_subst(certificate.target_freshening@)),
                view_subst(certificate.unifier@)),
            certificate.target_argument_slot as nat,
            view_position(certificate.target_path@),
            selected_rewrite_rhs(*certificate),
        ).is_some()
        && accepted ==> atom_at_position(
            view_atom(certificate.target.literals@[certificate.target_literal_index as int].atom),
            certificate.target_argument_slot as nat,
            view_position(certificate.target_path@),
        ).is_some()
        && accepted ==> match atom_at_position(
            view_atom(certificate.target.literals@[certificate.target_literal_index as int].atom),
            certificate.target_argument_slot as nat,
            view_position(certificate.target_path@),
        ) {
            Some(raw) => selected_target_subterm(*certificate)
                == apply(raw, view_subst(certificate.target_freshening@)),
            None => false,
        },
        accepted ==> forall|i: int| 0 <= i < certificate.source.literals@.len()
            ==> renames_variables_on(view_subst(certificate.source_freshening@),
                literal_variables(view_literal(certificate.source.literals@[i]))),
        accepted ==> forall|i: int| 0 <= i < certificate.target.literals@.len()
            ==> renames_variables_on(view_subst(certificate.target_freshening@),
                literal_variables(view_literal(certificate.target.literals@[i])))
{
    let mut structural_work: usize = 0;
    if !charge_clause_structure(&certificate.source, &mut structural_work, work_limit)
        || !charge_clause_structure(&certificate.target, &mut structural_work, work_limit)
        || !charge_clause_structure(&certificate.conclusion, &mut structural_work, work_limit)
        || !charge_binding_structure(&certificate.source_freshening, &mut structural_work, work_limit)
        || !charge_binding_structure(&certificate.target_freshening, &mut structural_work, work_limit)
        || !charge_binding_structure(&certificate.unifier, &mut structural_work, work_limit)
    {
        return false;
    }
    if certificate.source.literals.len() > work_limit
        || certificate.target.literals.len() > work_limit - certificate.source.literals.len()
    {
        return false;
    }
    let mut remaining_work = work_limit - certificate.source.literals.len()
        - certificate.target.literals.len();
    if certificate.source_freshening.len() > remaining_work { return false; }
    remaining_work -= certificate.source_freshening.len();
    if certificate.target_freshening.len() > remaining_work { return false; }
    remaining_work -= certificate.target_freshening.len();
    if certificate.unifier.len() > remaining_work { return false; }
    remaining_work -= certificate.unifier.len();
    if certificate.target_path.len() > remaining_work { return false; }
    remaining_work -= certificate.target_path.len();
    if certificate.fresh_target_path.len() > remaining_work { return false; }
    remaining_work -= certificate.fresh_target_path.len();
    if certificate.conclusion.literals.len() > remaining_work { return false; }
    if !same_u64_sequence(&certificate.target_path, &certificate.fresh_target_path) {
        return false;
    }

    if certificate.source_equality_index >= certificate.source.literals.len()
        || certificate.target_literal_index >= certificate.target.literals.len()
    {
        return false;
    }

    let source_literal = &certificate.source.literals[certificate.source_equality_index];
    if !source_literal.positive { return false; }
    let ExecAtom::Eq(source_left, source_right) = &source_literal.atom else { return false; };
    if let ExecTerm::Var(_) = source_left { return false; }

    let left_variables = match collect_clause_variables_bounded(&certificate.source, work_limit) {
        Some(vars) => vars,
        None => return false,
    };
    let right_variables = match collect_clause_variables_bounded(&certificate.target, work_limit) {
        Some(vars) => vars,
        None => return false,
    };
    if !check_freshening(
        &left_variables, &right_variables, &certificate.source_freshening, work_limit,
    ) {
        return false;
    }
    if !is_variable_renaming(&left_variables, &certificate.source_freshening) {
        return false;
    }
    let fresh_source = match freshen_clause_apart(
        &certificate.source, &right_variables, &certificate.source_freshening, work_limit,
    ) {
        Some(clause) => clause,
        None => return false,
    };
    assert(view_clause(fresh_source)
        == substitute_clause(view_clause(certificate.source),
            view_subst(certificate.source_freshening@)));
    let fresh_source_variables = match collect_clause_variables_bounded(&fresh_source, work_limit) {
        Some(vars) => vars,
        None => return false,
    };
    let mut avoid_target: Vec<u64> = Vec::new();
    if !extend_unique(&mut avoid_target, &right_variables, work_limit)
        || !extend_unique(&mut avoid_target, &fresh_source_variables, work_limit)
    {
        return false;
    }
    if !check_freshening(
        &right_variables, &avoid_target, &certificate.target_freshening, work_limit,
    ) {
        return false;
    }
    if !is_variable_renaming(&right_variables, &certificate.target_freshening) {
        return false;
    }
    let target_literal_freshening_vars = match collect_clause_variables_bounded(
        &ExecClause { literals: vec![copy_literal(
            &certificate.target.literals[certificate.target_literal_index],
        )] },
        work_limit,
    ) {
        Some(vars) => vars,
        None => return false,
    };
    if !is_variable_renaming(&target_literal_freshening_vars, &certificate.target_freshening) {
        return false;
    }
    let fresh_target = match freshen_clause_apart(
        &certificate.target, &avoid_target, &certificate.target_freshening, work_limit,
    ) {
        Some(clause) => clause,
        None => return false,
    };
    assert(view_clause(fresh_target)
        == substitute_clause(view_clause(certificate.target),
            view_subst(certificate.target_freshening@)));
    if certificate.source_equality_index >= fresh_source.literals.len()
        || certificate.target_literal_index >= fresh_target.literals.len()
    {
        return false;
    }

    let target_subterm_result = selected_atom_term_exec(
        &certificate.target.literals[certificate.target_literal_index].atom,
        certificate.target_argument_slot,
        &certificate.target_path,
    );
    let Some(target_subterm) = target_subterm_result else { return false; };
    let source_eq_fresh = &fresh_source.literals[certificate.source_equality_index];
    let ExecAtom::Eq(fresh_source_left, _) = &source_eq_fresh.atom else { return false; };
    assert(view_clause(fresh_source).literals[certificate.source_equality_index as int]
        == substitute_literal(view_literal(*source_literal),
            view_subst(certificate.source_freshening@)));
    let target_selected = &fresh_target.literals[certificate.target_literal_index];
    let Some(fresh_target_subterm) = selected_atom_term_exec(
        &target_selected.atom, certificate.target_argument_slot, &certificate.fresh_target_path,
    ) else { return false; };
    let raw_selected_after_freshening = apply_exec(&target_subterm, &certificate.target_freshening);
    if !same_term(&raw_selected_after_freshening, &fresh_target_subterm) {
        return false;
    }
    let unifies = check_unifier(
        fresh_source_left, &fresh_target_subterm, &certificate.unifier, work_limit,
    );
    if !unifies {
        return false;
    }
    assert(view(*fresh_source_left)
        == apply(view(*source_left), view_subst(certificate.source_freshening@)));
    assert(domain_unique(view_subst(certificate.unifier@)));
    assert(apply(view(*fresh_source_left), view_subst(certificate.unifier@))
        == apply(view(fresh_target_subterm), view_subst(certificate.unifier@)));
    assert(certificate_selected_source_left(*certificate)
        == apply(view(*fresh_source_left), view_subst(certificate.unifier@)));
    assert(certificate_selected_target_fresh_term(*certificate)
        == Some(view(fresh_target_subterm)));
    assert(certificate_selected_target_raw_term(*certificate)
        == Some(view(target_subterm)));
    assert(apply(view(fresh_target_subterm), view_subst(certificate.unifier@))
        == selected_target_subterm_after_freshening(*certificate));
    proof {
        let raw_target_literal = view_literal(
            certificate.target.literals@[certificate.target_literal_index as int]);
        assert(literal_at_position(raw_target_literal,
            certificate.target_argument_slot as nat,
            view_position(certificate.target_path@)).is_some());
        literal_position_substitution_success(
            raw_target_literal,
            view_subst(certificate.target_freshening@),
            certificate.target_argument_slot as nat,
            view_position(certificate.target_path@),
        );
        let freshened_target_literal = substitute_literal(
            raw_target_literal, view_subst(certificate.target_freshening@));
        assert(literal_at_position(freshened_target_literal,
            certificate.target_argument_slot as nat,
            view_position(certificate.target_path@)).is_some());
        literal_position_substitution_success(
            freshened_target_literal,
            view_subst(certificate.unifier@),
            certificate.target_argument_slot as nat,
            view_position(certificate.target_path@),
        );
    }
    let source_right = match &source_eq_fresh.atom {
        ExecAtom::Eq(_, right) => right,
        _ => return false,
    };
    let instantiated_source_left = apply_exec(fresh_source_left, &certificate.unifier);
    let instantiated_source_right = apply_exec(source_right, &certificate.unifier);
    let raw_selected_after_freshening = apply_exec(&target_subterm, &certificate.target_freshening);
    if !same_term(&raw_selected_after_freshening, &fresh_target_subterm) {
        return false;
    }
    let selected_after_unification = apply_exec(&fresh_target_subterm, &certificate.unifier);
    if !same_term(&instantiated_source_left, &selected_after_unification) {
        return false;
    }
    // Ensure both oriented source sides were instantiated by the same unifier;
    // the semantic theorem uses this exact RHS when constructing the resolvent.
    let _rewritten_rhs = instantiated_source_right;
    true
}

fn is_variable_renaming(variables: &Vec<u64>, bindings: &Vec<ExecBinding>) -> (valid: bool)
    ensures valid == freshening_images_are_variable(bindings@, variables@)
{
    let mut i: usize = 0;
    while i < variables.len()
        invariant i <= variables.len(),
            forall|k: int| 0 <= k < i as int
                ==> renamed_variable(view_subst(bindings@), variables@[k]).is_some()
        decreases variables.len() - i
    {
        match lookup_exec(bindings, variables[i]) {
            Some(ExecTerm::Var(fresh)) => {
                assert(renamed_variable(view_subst(bindings@), variables@[i as int])
                    == Some(fresh));
            },
            Some(_) => {
                assert(renamed_variable(view_subst(bindings@), variables@[i as int]).is_none());
                return false;
            },
            None => {
                assert(renamed_variable(view_subst(bindings@), variables@[i as int]).is_none());
                return false;
            },
        }
        i += 1;
    }
    assert(i == variables.len());
    assert forall|k: int| 0 <= k < variables@.len()
        implies renamed_variable(view_subst(bindings@), variables@[k]).is_some() by {
        assert(k < i as int);
    }
    assert(i == variables.len());
    assert forall|k: int| 0 <= k < variables@.len()
        implies renamed_variable(view_subst(bindings@), variables@[k]).is_some() by {
        assert(k < i as int);
    }
    true
}

spec fn renames_all_variables(sigma: Substitution) -> bool {
    forall|v: u64| #[trigger] lookup(sigma, v).is_some()
        ==> exists|fresh: u64| lookup(sigma, v) == Some(Term::Var(fresh))
}

proof fn renaming_on_all_implies_finite_renaming(sigma: Substitution)
    requires renames_all_variables(sigma)
    ensures renaming_substitution(sigma)
{}

fn extend_unique(destination: &mut Vec<u64>, source: &Vec<u64>, work_limit: usize) -> (ok: bool)
{
    let mut i: usize = 0;
    while i < source.len()
        invariant i <= source.len()
        decreases source.len() - i
    {
        if destination.len() >= work_limit { return false; }
        let value = source[i];
        if !contains_u64(destination, value) {
            destination.push(value);
        }
        i += 1;
    }
    true
}

fn same_u64_sequence(left: &Vec<u64>, right: &Vec<u64>) -> (same: bool)
    ensures same == (left@ == right@)
{
    if left.len() != right.len() { return false; }
    let mut i: usize = 0;
    while i < left.len()
        invariant
            i <= left.len(),
            left.len() == right.len(),
            forall|j: int| 0 <= j < i as int ==> left@[j] == right@[j],
        decreases left.len() - i
    {
        if left[i] != right[i] { return false; }
        assert(left@[i as int] == right@[i as int]);
        i += 1;
    }
    assert forall|j: int| 0 <= j < left@.len() implies left@[j] == right@[j] by {
        assert(j < i as int);
    }
    assert(left@ == right@);
    true
}

fn check_superposition_and_replay(certificate: &SuperpositionCertificate, work_limit: usize)
    -> (accepted: bool)
    ensures accepted ==> certificate.source.literals.len()
            + certificate.target.literals.len()
            + certificate.conclusion.literals.len() <= work_limit
        && accepted ==> clause_equal(view_clause(certificate.conclusion),
            Clause { literals: expected_superposition_conclusion(
                *certificate, work_limit as nat) })
{
    let mut structural_work: usize = 0;
    if !charge_clause_structure(&certificate.source, &mut structural_work, work_limit)
        || !charge_clause_structure(&certificate.target, &mut structural_work, work_limit)
        || !charge_clause_structure(&certificate.conclusion, &mut structural_work, work_limit)
        || !charge_binding_structure(&certificate.source_freshening, &mut structural_work, work_limit)
        || !charge_binding_structure(&certificate.target_freshening, &mut structural_work, work_limit)
        || !charge_binding_structure(&certificate.unifier, &mut structural_work, work_limit)
    {
        return false;
    }
    assert(structural_work <= work_limit);
    if certificate.source.literals.len() > work_limit
        || certificate.target.literals.len() > work_limit - certificate.source.literals.len()
    {
        return false;
    }
    let replay_work = certificate.source.literals.len() + certificate.target.literals.len();
    if certificate.conclusion.literals.len() > work_limit - replay_work {
        return false;
    }
    if !check_superposition_certificate(certificate, work_limit) {
        return false;
    }
    if certificate.source_equality_index >= certificate.source.literals.len()
        || certificate.target_literal_index >= certificate.target.literals.len()
    {
        return false;
    }
    let Some(target_variables) = collect_clause_variables_bounded(&certificate.target, work_limit) else {
        return false;
    };
    let Some(fresh_source) = freshen_clause_apart(
        &certificate.source, &target_variables, &certificate.source_freshening, work_limit,
    ) else { return false; };
    let Some(fresh_source_variables) = collect_clause_variables_bounded(&fresh_source, work_limit) else {
        return false;
    };
    let mut target_avoid = Vec::new();
    if !extend_unique(&mut target_avoid, &target_variables, work_limit)
        || !extend_unique(&mut target_avoid, &fresh_source_variables, work_limit)
    {
        return false;
    }
    let Some(fresh_target) = freshen_clause_apart(
        &certificate.target, &target_avoid, &certificate.target_freshening, work_limit,
    ) else { return false; };
    let instantiated_source = substitute_clause_exec(&fresh_source, &certificate.unifier);
    let instantiated_target = substitute_clause_exec(&fresh_target, &certificate.unifier);
    if certificate.source_equality_index >= instantiated_source.literals.len()
        || certificate.target_literal_index >= instantiated_target.literals.len()
    {
        return false;
    }

    let source_literal = &instantiated_source.literals[certificate.source_equality_index];
    let ExecAtom::Eq(_, source_right) = &source_literal.atom else { return false; };
    let Some(rewritten_target) = replace_clause_literal_exec(
        &instantiated_target,
        certificate.target_literal_index,
        certificate.target_argument_slot,
        &certificate.fresh_target_path,
        source_right,
    ) else { return false; };

    let replayed_conclusion = build_superposition_conclusion_exec(
        &instantiated_source, certificate.source_equality_index, &rewritten_target,
    );
    assert(view_clause(replayed_conclusion).literals
        == expected_superposition_conclusion(*certificate, work_limit as nat));
    let conclusion = &certificate.conclusion;
    let same = same_clause_exact(&replayed_conclusion, conclusion);
    if !same { return false; }
    assert(clause_equal(view_clause(replayed_conclusion), view_clause(*conclusion)));
    assert(clause_equal(view_clause(*conclusion), Clause {
        literals: expected_superposition_conclusion(*certificate, work_limit as nat),
    }));
    true
}

fn check_dag_structure(dag: &ProofDag, node_limit: usize, literal_limit: usize)
    -> (verdict: DagVerdict)
    ensures (verdict == DagVerdict::Unknown || verdict == DagVerdict::Rejected)
        && (dag.nodes.len() == 0 || dag.nodes.len() > node_limit
            || dag.input_clauses.len() > node_limit || dag.root >= dag.nodes.len())
            ==> verdict == DagVerdict::Rejected
        && verdict == DagVerdict::Unknown ==> dag.nodes.len() > 0
            && dag.nodes.len() <= node_limit
            && dag.input_clauses.len() <= node_limit
            && dag.root < dag.nodes.len()
        && verdict == DagVerdict::Unknown ==> forall|i: int| 0 <= i < dag.nodes.len()
            ==> dag.nodes[i].kind == ProofNodeKind::Input && dag.nodes[i].parents@.len() == 0
                || dag.nodes[i].kind == ProofNodeKind::Superposition
                    && dag.nodes[i].parents@.len() == 2 && dag.nodes[i].step.is_some()
        && verdict == DagVerdict::Unknown ==> forall|i: int, j: int|
            0 <= i < j < dag.nodes.len() ==> dag.nodes[i].id != dag.nodes[j].id
        && verdict == DagVerdict::Unknown ==> forall|i: int|
            0 <= i < dag.nodes.len() && dag.nodes[i].kind == ProofNodeKind::Input
            ==> exists|input_index: int| 0 <= input_index < dag.input_clauses.len()
                && clause_equal(view_clause(dag.nodes[i].clause),
                    view_clause(dag.input_clauses[input_index]))
        && verdict == DagVerdict::Unknown ==> forall|i: int, j: int|
            0 <= i < dag.nodes.len() && 0 <= j < dag.nodes[i].parents@.len()
            ==> dag.nodes[i].parents@[j] < i as usize
{
    // This structural prototype is deliberately non-certifying. Enforce the
    // size caps, then return Unknown until input-clause refinement and the
    // replay-to-local-soundness theorem are proved and connected.
    if dag.nodes.len() > node_limit || dag.input_clauses.len() > node_limit {
        return DagVerdict::Rejected;
    }
    if dag.nodes.len() == 0 || dag.root >= dag.nodes.len() {
        return DagVerdict::Rejected;
    }
    let mut total_literals: usize = 0;
    let mut total_parents: usize = 0;
    let mut structural_work: usize = 0;
    let mut i: usize = 0;
    while i < dag.nodes.len()
        invariant
            i <= dag.nodes.len(),
            dag.nodes.len() <= node_limit,
            total_literals <= literal_limit,
            total_parents <= node_limit,
            structural_work <= literal_limit,
        decreases dag.nodes.len() - i
    {
        let node = &dag.nodes[i];
        let mut prior: usize = 0;
        while prior < i
            invariant
                prior <= i,
                i <= dag.nodes.len(),
                structural_work <= literal_limit,
                forall|j: int| 0 <= j < prior as int
                    ==> dag.nodes[j].id != node.id,
            decreases i - prior
        {
            if structural_work >= literal_limit {
                return DagVerdict::Rejected;
            }
            structural_work += 1;
            if dag.nodes[prior].id == node.id {
                return DagVerdict::Rejected;
            }
            prior += 1;
        }
        assert forall|j: int| 0 <= j < i as int
            implies dag.nodes[j].id != node.id by {
            assert(j < prior as int);
        }
        if node.kind == ProofNodeKind::Input && node.parents.len() != 0
            || node.kind == ProofNodeKind::Superposition && node.step.is_none()
            || node.kind == ProofNodeKind::Unsupported
            || node.clause.literals.len() > literal_limit - total_literals
        {
            return DagVerdict::Rejected;
        }
        if node.kind == ProofNodeKind::Superposition {
            let Some(step) = &node.step else { return DagVerdict::Rejected; };
            if !charge_certificate_structure(step, &mut structural_work, literal_limit) {
                return DagVerdict::Rejected;
            }
            let mut step_parents: usize = 0;
            while step_parents < node.parents.len()
                invariant
                    step_parents <= node.parents.len(),
                    total_parents <= node_limit,
                decreases node.parents.len() - step_parents
            {
                if step_parents >= 2 || step_parents >= node_limit - total_parents {
                    return DagVerdict::Rejected;
                }
                total_parents += 1;
                step_parents += 1;
            }
        }
        total_literals += node.clause.literals.len();
        if node.kind != ProofNodeKind::Superposition {
            if node.parents.len() > node_limit - total_parents {
                return DagVerdict::Rejected;
            }
            total_parents += node.parents.len();
        }
        if !charge_clause_structure(&node.clause, &mut structural_work, literal_limit) {
            return DagVerdict::Rejected;
        }
        let mut parent_index: usize = 0;
        while parent_index < node.parents.len()
            invariant parent_index <= node.parents.len(), total_parents <= node_limit
            decreases node.parents.len() - parent_index
        {
            if node.parents[parent_index] >= dag.nodes.len()
                || node.parents[parent_index] >= i
            {
                return DagVerdict::Rejected;
            }
            parent_index += 1;
        }
        i += 1;
    }
    let mut input_index: usize = 0;
    while input_index < dag.input_clauses.len()
        invariant
            input_index <= dag.input_clauses.len(),
            total_literals <= literal_limit,
            structural_work <= literal_limit,
        decreases dag.input_clauses.len() - input_index
    {
        let input_clause = &dag.input_clauses[input_index];
        if input_clause.literals.len() > literal_limit - total_literals {
            return DagVerdict::Rejected;
        }
        total_literals += input_clause.literals.len();
        if !charge_clause_structure(input_clause, &mut structural_work, literal_limit) {
            return DagVerdict::Rejected;
        }
        input_index += 1;
    }
    let mut node_index: usize = 0;
    while node_index < dag.nodes.len()
        invariant
            node_index <= dag.nodes.len(),
            dag.nodes.len() <= node_limit,
            input_index == dag.input_clauses.len(),
            structural_work <= literal_limit,
        decreases dag.nodes.len() - node_index
    {
        let node = &dag.nodes[node_index];
        if node.kind == ProofNodeKind::Input {
            let mut found: bool = false;
            let mut candidate: usize = 0;
            while candidate < dag.input_clauses.len()
                invariant
                    candidate <= dag.input_clauses.len(),
                    structural_work <= literal_limit,
                decreases dag.input_clauses.len() - candidate
            {
                if structural_work >= literal_limit { return DagVerdict::Rejected; }
                structural_work += 1;
                if same_clause_exact(&node.clause, &dag.input_clauses[candidate]) {
                    found = true;
                    break;
                }
                candidate += 1;
            }
            if !found { return DagVerdict::Rejected; }
        }
        node_index += 1;
    }
    DagVerdict::Unknown
}

fn check_dag_with_local_soundness(
    dag: &ProofDag,
    node_limit: usize,
    literal_limit: usize,
    local_soundness: &Vec<bool>,
) -> (verdict: DagVerdict)
{
    match check_dag_structure(dag, node_limit, literal_limit) {
        DagVerdict::Rejected => DagVerdict::Rejected,
        DagVerdict::Unknown => DagVerdict::Unknown,
        DagVerdict::Inconclusive => {
            if local_soundness.len() != dag.nodes.len() {
                return DagVerdict::Unknown;
            }
            let mut i: usize = 0;
            while i < dag.nodes.len()
                invariant i <= dag.nodes.len(), local_soundness.len() == dag.nodes.len()
                decreases dag.nodes.len() - i
            {
                if dag.nodes[i].kind == ProofNodeKind::Superposition && !local_soundness[i] {
                    return DagVerdict::Unknown;
                }
                i += 1;
            }
            DagVerdict::Inconclusive
        },
    }
}

spec fn dag_all_local_steps_sound(dag: ProofDag, local_soundness: Seq<bool>) -> bool {
    local_soundness.len() == dag.nodes.len()
        && forall|i: int| 0 <= i < dag.nodes.len()
            && dag.nodes[i].kind == ProofNodeKind::Superposition
            ==> local_soundness[i]
}

proof fn dag_local_soundness_postcondition(
    dag: ProofDag,
    local_soundness: Seq<bool>,
)
    requires
        local_soundness.len() == dag.nodes.len(),
        forall|i: int| 0 <= i < dag.nodes.len()
            && dag.nodes[i].kind == ProofNodeKind::Superposition
            ==> local_soundness[i],
    ensures dag_all_local_steps_sound(dag, local_soundness)
{}

spec fn dag_root_valid(dag: ProofDag, interp: Interpretation) -> bool {
    dag.root < dag.nodes.len()
        && clause_valid(view_clause(dag.nodes@[dag.root as int].clause), interp)
}

proof fn dag_semantic_result_from_local_postcondition(
    dag: ProofDag,
    inputs: Seq<ExecClause>,
    interp: Interpretation,
    local_soundness: Seq<bool>,
)
    requires
        interpretation_well_formed(interp),
        dag.nodes.len() > 0,
        dag.root < dag.nodes.len(),
        dag.nodes@[dag.root as int].clause.literals.len() == 0,
        local_soundness.len() == dag.nodes.len(),
        forall|i: int| 0 <= i < dag.nodes.len()
            && dag.nodes[i].kind == ProofNodeKind::Superposition
            ==> local_soundness[i],
        forall|i: int| 0 <= i < dag.nodes.len()
            && dag.nodes[i].kind == ProofNodeKind::Superposition
            ==> local_node_entails_from_parents(dag.nodes@[i], dag.nodes@, interp),
        forall|i: int| 0 <= i < dag.nodes.len()
            ==> (dag.nodes[i].kind == ProofNodeKind::Input
                ==> input_leaf_is_validated(dag.nodes[i], inputs)),
        forall|i: int| 0 <= i < dag.nodes.len()
            ==> dag.nodes[i].kind == ProofNodeKind::Input
                || dag.nodes[i].kind == ProofNodeKind::Superposition,
        forall|i: int| 0 <= i < inputs.len()
            ==> clause_valid(view_clause(inputs[i]), interp),
        forall|i: int, j: int| 0 <= i < dag.nodes.len()
            && 0 <= j < dag.nodes[i].parents@.len()
            ==> dag.nodes[i].parents@[j] < i as usize,
    ensures dag_root_valid(dag, interp)
        && !exists|valuation: Valuation| valuation_well_formed(interp, valuation)
            && forall|i: int| 0 <= i < inputs.len()
                ==> clause_holds(view_clause(inputs[i]), interp, valuation)
{
    assert forall|i: int| 0 <= i < dag.nodes.len()
        implies (dag.nodes[i].kind == ProofNodeKind::Input
                && input_leaf_is_validated(dag.nodes[i], inputs)
            || dag.nodes[i].kind == ProofNodeKind::Superposition
                && local_node_entails_from_parents(dag.nodes@[i], dag.nodes@, interp)) by {
        if dag.nodes[i].kind == ProofNodeKind::Input {
            assert(input_leaf_is_validated(dag.nodes[i], inputs));
        } else {
            assert(dag.nodes[i].kind == ProofNodeKind::Superposition);
            assert(local_node_entails_from_parents(dag.nodes@[i], dag.nodes@, interp));
        }
    }
    proof_dag_nodes_follow_from_local_rule_soundness(
        dag.nodes@, inputs, interp, dag.nodes.len() as nat,
    );
    assert(clause_valid(view_clause(dag.nodes@[dag.root as int].clause), interp));
    certified_empty_dag_implies_unsatisfiable(dag.nodes@, inputs, interp, dag.root as nat);
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
    executable_literal_replay_examples();
    executable_clause_replay_example();
    executable_standardization_apart_example();
    executable_superposition_certificate_example();

    // Supplied unifier certificate binding target-side variable 42 to
    // constant 9. Domain validity and simultaneous application are checked.
}

} // verus!
