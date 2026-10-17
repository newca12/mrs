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
    unifier: Vec<ExecBinding>,
    conclusion: ExecClause,
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

spec fn exec_clause_variables(clause: ExecClause) -> Set<u64> {
    Set::new(|v: u64| exists|i: int| 0 <= i < clause.literals@.len()
        && exec_atom_variables(clause.literals@[i].atom).contains(v)).unwrap()
}

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
            ==> renamed_variable(sigma, variables[i]).is_some()
                && !avoid.contains(renamed_variable(sigma, variables[i]).unwrap())
        && forall|i: int, j: int| 0 <= i < j < variables.len()
            && variables[i] != variables[j]
            ==> renamed_variable(sigma, variables[i]).is_some()
                && renamed_variable(sigma, variables[j]).is_some()
                && renamed_variable(sigma, variables[i])
                    != renamed_variable(sigma, variables[j])
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

fn consume_work(work: &mut usize, limit: usize) -> (ok: bool)
    ensures
        ok ==> *final(work) == old(work) + 1,
        !ok ==> *final(work) == *old(work),
{
    if *work >= limit { false } else { *work += 1; true }
}

fn add_variable(values: &mut Vec<u64>, variable: u64) {
    let found = contains_u64(values, variable);
    if !found {
        values.push(variable);
    }
}

fn collect_term_variables_exec(
    term: &ExecTerm,
    variables: &mut Vec<u64>,
    work: &mut usize,
    limit: usize,
) -> (ok: bool)
    decreases term
{
    match term {
        ExecTerm::Var(v) => {
            if *work >= limit { return false; }
            *work += 1;
            add_variable(variables, *v);
            true
        },
        ExecTerm::Const(_) => {
            if *work >= limit { false } else { *work += 1; true }
        },
        ExecTerm::App1(_, child) => {
            if *work >= limit { return false; }
            *work += 1;
            collect_term_variables_exec(child, variables, work, limit)
        },
        ExecTerm::App2(_, left, right) => {
            if *work >= limit { return false; }
            *work += 1;
            collect_term_variables_exec(left, variables, work, limit)
                && collect_term_variables_exec(right, variables, work, limit)
        },
    }
}

fn collect_clause_variables_exec(
    clause: &ExecClause,
    variables: &mut Vec<u64>,
    work: &mut usize,
    limit: usize,
) -> (ok: bool)
{
    if *work >= limit { return false; }
    *work += 1;
    let mut literal_index: usize = 0;
    while literal_index < clause.literals.len()
        invariant literal_index <= clause.literals.len()
        decreases clause.literals.len() - literal_index
    {
        match &clause.literals[literal_index].atom {
            ExecAtom::Pred0(_) => {},
            ExecAtom::Pred1(_, arg) => {
                if !collect_term_variables_exec(arg, variables, work, limit) { return false; }
            },
            ExecAtom::Pred2(_, left, right) => {
                if !collect_term_variables_exec(left, variables, work, limit) { return false; }
                if !collect_term_variables_exec(right, variables, work, limit) { return false; }
            },
            ExecAtom::Eq(left, right) => {
                if !collect_term_variables_exec(left, variables, work, limit) { return false; }
                if !collect_term_variables_exec(right, variables, work, limit) { return false; }
            },
        }
        literal_index += 1;
    }
    true
}

fn collect_clause_variables_bounded(
    clause: &ExecClause,
    work_limit: usize,
) -> (result: Option<Vec<u64>>)
{
    if work_limit < 8 || clause.literals.len() > 64 { return None; }
    let mut variables: Vec<u64> = Vec::new();
    let mut work: usize = 0;
    if collect_clause_variables_exec(clause, &mut variables, &mut work, work_limit) {
        Some(variables)
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
{
    if !check_domain_unique(bindings) {
        return false;
    }
    let mut i: usize = 0;
    let mut work: usize = 0;
    while i < original.len()
        invariant
            i <= original.len(),
        decreases original.len() - i
    {
        if work >= work_limit { return false; }
        work += 1;
        let Some(mapped) = lookup_exec(bindings, original[i]) else { return false; };
        let ExecTerm::Var(fresh) = mapped else { return false; };
        if contains_u64(avoid, fresh) { return false; }
        assert(renamed_variable(view_subst(bindings@), original@[i as int]) == Some(fresh));
        assert(!avoid@.contains(fresh));
        let mut j: usize = 0;
        while j < i
            invariant j <= i, i < original.len(), work <= work_limit,
            decreases i - j
        {
            assert(j < original.len());
            assert(i < original.len());
            let same_source = original[j] == original[i];
            let mut different_fresh = false;
            if !same_source {
                let Some(previous) = lookup_exec(bindings, original[j]) else { return false; };
                let ExecTerm::Var(previous_fresh) = previous else { return false; };
                if fresh == previous_fresh { return false; }
                different_fresh = true;
                assert(renamed_variable(view_subst(bindings@), original@[j as int])
                    == Some(previous_fresh));
            }
            j += 1;
        }
        i += 1;
    }
    true
}

fn freshen_clause_apart(
    clause: &ExecClause,
    avoid: &Vec<u64>,
    bindings: &Vec<ExecBinding>,
    work_limit: usize,
) -> (result: Option<ExecClause>)
{
    let Some(variables) = collect_clause_variables_bounded(clause, work_limit) else {
        return None;
    };
    if !check_freshening(&variables, avoid, bindings, work_limit) {
        return None;
    }
    Some(substitute_clause_exec(clause, bindings))
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
        ) == Some(view_clause(replaced)),
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
            assert(view_clause(result).literals
                == view_clause(*clause).literals.update(
                    literal_index as int, view_literal(rewritten)));
            Some(result)
        },
    }
}

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
        source_freshening: vec![ExecBinding {
            variable: 0,
            replacement: ExecTerm::Var(0),
        }],
        target_freshening: vec![ExecBinding {
            variable: 0,
            replacement: ExecTerm::Var(0),
        }],
        source_equality_index: 0,
        target_literal_index: 0,
        target_argument_slot: 0,
        target_path: Vec::new(),
        unifier: Vec::new(),
        conclusion: ExecClause { literals: vec![ExecLiteral {
            positive: true,
            atom: ExecAtom::Pred1(8, Box::new(ExecTerm::Const(2))),
        }] },
    };
    let _accepted = check_superposition_and_replay(&certificate, 1000);
}

fn copy_literal(literal: &ExecLiteral) -> (copy: ExecLiteral)
    ensures view_literal(copy) == view_literal(*literal)
{
    ExecLiteral { positive: literal.positive, atom: copy_atom(&literal.atom) }
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

fn check_domain_unique(bindings: &Vec<ExecBinding>) -> (unique: bool)
    ensures unique == domain_unique(view_subst(bindings@))
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
    ensures unifies == (domain_unique(view_subst(bindings@))
        && apply(view(*left), view_subst(bindings@))
            == apply(view(*right), view_subst(bindings@)))
{
    if !check_domain_unique(bindings) {
        return false;
    }
    let left_instance = apply_exec(left, bindings);
    let right_instance = apply_exec(right, bindings);
    let same = same_term(&left_instance, &right_instance);
    assert(same == (apply(view(*left), view_subst(bindings@))
        == apply(view(*right), view_subst(bindings@))));
    same
}

fn selected_atom_term_exec(
    atom: &ExecAtom,
    slot: u64,
    path: &Vec<u64>,
) -> (result: Option<ExecTerm>)
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
    subterm_at_exec(&root, path, 0)
}

fn check_superposition_certificate(certificate: &SuperpositionCertificate, work_limit: usize)
    -> (accepted: bool)
{
    if certificate.source.literals.len() > work_limit
        || certificate.target.literals.len() > work_limit - certificate.source.literals.len()
    {
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
    let fresh_source = match freshen_clause_apart(
        &certificate.source, &right_variables, &certificate.source_freshening, work_limit,
    ) {
        Some(clause) => clause,
        None => return false,
    };
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
    let fresh_target = match freshen_clause_apart(
        &certificate.target, &avoid_target, &certificate.target_freshening, work_limit,
    ) {
        Some(clause) => clause,
        None => return false,
    };
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
    let target_selected = &fresh_target.literals[certificate.target_literal_index];
    let Some(fresh_target_subterm) = selected_atom_term_exec(
        &target_selected.atom, certificate.target_argument_slot, &certificate.target_path,
    ) else { return false; };
    let unifies = check_unifier(fresh_source_left, &fresh_target_subterm, &certificate.unifier);
    if !unifies {
        return false;
    }
    true
}

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

fn check_superposition_and_replay(certificate: &SuperpositionCertificate, work_limit: usize)
    -> (accepted: bool)
{
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
        &certificate.target_path,
        source_right,
    ) else { return false; };

    let mut conclusion_literals = Vec::new();
    let mut source_index: usize = 0;
    while source_index < instantiated_source.literals.len()
        invariant source_index <= instantiated_source.literals.len()
        decreases instantiated_source.literals.len() - source_index
    {
        if source_index != certificate.source_equality_index {
            conclusion_literals.push(copy_literal(&instantiated_source.literals[source_index]));
        }
        source_index += 1;
    }
    let mut target_index: usize = 0;
    while target_index < rewritten_target.literals.len()
        invariant target_index <= rewritten_target.literals.len()
        decreases rewritten_target.literals.len() - target_index
    {
        conclusion_literals.push(copy_literal(&rewritten_target.literals[target_index]));
        target_index += 1;
    }
    let replayed_conclusion = ExecClause { literals: conclusion_literals };
    let conclusion = &certificate.conclusion;
    if replayed_conclusion.literals.len() != conclusion.literals.len() {
        return false;
    }
    let mut i: usize = 0;
    while i < replayed_conclusion.literals.len()
        invariant
            i <= replayed_conclusion.literals.len(),
            replayed_conclusion.literals.len() == conclusion.literals.len(),
        decreases replayed_conclusion.literals.len() - i
    {
        if replayed_conclusion.literals[i] != conclusion.literals[i] {
            return false;
        }
        i += 1;
    }
    true
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
