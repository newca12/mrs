use super::*;

fn terms(f: SymbolId, a: SymbolId) -> Vec<Term> {
    let mut terms = vec![Term::constant(a), Term::var(0), Term::var(1), Term::var(7)];
    for arity in [1, 2, 4, 5, 8, 256, 512] {
        terms.push(Term::app(
            f,
            (0..arity)
                .map(|i| {
                    if i % 3 == 0 {
                        Term::constant(a)
                    } else {
                        Term::var((i % 5) as VarId)
                    }
                })
                .collect(),
        ));
    }
    let mut nested = Term::var(0);
    for _ in 0..12 {
        nested = Term::app(f, vec![nested, Term::var(1), Term::constant(a)]);
        terms.push(nested.clone());
    }
    terms
}

fn replace_reference(term: &Term, variable: VarId, replacement: &Term) -> Term {
    match term {
        Term::Var(v) if *v == variable => replacement.clone(),
        Term::Var(_) => term.clone(),
        Term::App(symbol, args) => Term::app(
            *symbol,
            args.iter()
                .map(|arg| replace_reference(arg, variable, replacement))
                .collect(),
        ),
    }
}

#[test]
fn substitutions_match_legacy_oracles_on_shared_deep_and_wide_terms() {
    let mut symbols = crate::SymbolTable::new();
    let f = symbols.intern("f");
    let a = symbols.intern("a");
    let b = symbols.intern("b");
    let mut bank = TermBank::new();
    let inputs = terms(f, a);
    let ids: Vec<_> = inputs.iter().map(|t| bank.from_legacy(t)).collect();
    // Acyclic but genuinely chained: 0 -> 3 -> b and 1 -> f(4) -> f(7).
    let bindings = [
        (0, Term::var(3)),
        (1, Term::app(f, vec![Term::var(4)])),
        (2, Term::constant(a)),
        (3, Term::constant(b)),
        (4, Term::var(7)),
    ];
    for mask in 0..32 {
        let mut substitution = IdSubstitution::new();
        for (index, (var, term)) in bindings.iter().enumerate() {
            if mask & (1 << index) != 0 {
                let term = bank.from_legacy(term);
                substitution.bind(*var, term);
            }
        }
        let legacy = substitution.to_legacy(&bank);
        for (&id, input) in ids.iter().zip(&inputs) {
            let result = substitution.apply_term(id, &mut bank);
            assert_eq!(
                bank.to_legacy(result),
                legacy.apply_term(input),
                "mask={mask}"
            );
        }
    }
    let replacement = Term::app(f, vec![Term::constant(b), Term::var(7)]);
    let replacement_id = bank.from_legacy(&replacement);
    for var in [0, 1, 4, 7, 99] {
        for (&id, input) in ids.iter().zip(&inputs) {
            let result = bank.substitute_var(id, var, replacement_id);
            assert_eq!(
                bank.to_legacy(result),
                replace_reference(input, var, &replacement)
            );
        }
    }
}

#[test]
fn map_app_args_preserves_order_and_identity_across_arena_growth() {
    let mut symbols = crate::SymbolTable::new();
    let f = symbols.intern("f");
    let mut bank = TermBank::new();
    let args: SmallVec<[TermId; 4]> = (0..512).map(|v| bank.intern_var(v)).collect();
    let original = bank.intern_app(f, args.clone());
    let mut visited = Vec::new();
    let mapped = bank.map_app_args(original, |arg, bank| {
        visited.push(arg);
        // Force arena growth without changing any existing node or argument.
        for v in 0..16 {
            bank.intern_var(10_000 + arg.0 * 16 + v);
        }
        arg
    });
    assert_eq!(mapped, original);
    assert_eq!(visited, args.as_slice());
    let replacement = bank.intern_var(99_999);
    let mapped = bank.map_app_args(original, |arg, _| {
        if arg == args[0] || arg == args[511] {
            replacement
        } else {
            arg
        }
    });
    let mut expected = args.clone();
    expected[0] = replacement;
    expected[511] = replacement;
    assert_eq!(bank.get(mapped), &TermNode::App(f, expected));
    assert_eq!(bank.get(original), &TermNode::App(f, args));
    assert_eq!(
        bank.map_app_args(replacement, |_, _| panic!("variable has no children")),
        replacement
    );
    let constant = bank.intern_app(f, smallvec![]);
    assert_eq!(
        bank.map_app_args(constant, |_, _| panic!("constant has no children")),
        constant
    );
}
